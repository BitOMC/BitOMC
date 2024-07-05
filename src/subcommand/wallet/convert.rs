use {
  super::*, crate::outgoing::Outgoing, base64::Engine, bitcoin::psbt::Psbt, num_integer::Roots,
};

#[derive(Debug, Parser)]
pub(crate) struct Convert {
  #[arg(long, help = "Don't sign or broadcast transaction")]
  pub(crate) dry_run: bool,
  #[arg(long, help = "Use fee rate of <FEE_RATE> sats/vB")]
  fee_rate: FeeRate,
  #[arg(
    long,
    help = "Target <AMOUNT> postage with sent inscriptions. [default: 10000 sat]"
  )]
  pub(crate) postage: Option<Amount>,
  outgoing: Outgoing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
  pub txid: Txid,
  pub psbt: String,
  pub outgoing: Outgoing,
  pub fee: u64,
}

impl Convert {
  pub(crate) fn run(self, wallet: Wallet) -> SubcommandResult {
    let unsigned_transaction = match self.outgoing {
      Outgoing::Rune { decimal, rune } => Self::create_unsigned_convert_runes_transaction(
        &wallet,
        rune,
        decimal,
        self.postage.unwrap_or(TARGET_POSTAGE),
        self.fee_rate,
      )?,
      _ => {
        panic!("invalid outgoing")
      }
    };

    let unspent_outputs = wallet.utxos();

    let (txid, psbt) = if self.dry_run {
      let psbt = wallet
        .bitcoin_client()
        .wallet_process_psbt(
          &base64::engine::general_purpose::STANDARD
            .encode(Psbt::from_unsigned_tx(unsigned_transaction.clone())?.serialize()),
          Some(false),
          None,
          None,
        )?
        .psbt;

      (unsigned_transaction.txid(), psbt)
    } else {
      let psbt = wallet
        .bitcoin_client()
        .wallet_process_psbt(
          &base64::engine::general_purpose::STANDARD
            .encode(Psbt::from_unsigned_tx(unsigned_transaction.clone())?.serialize()),
          Some(true),
          None,
          None,
        )?
        .psbt;

      let signed_tx = wallet
        .bitcoin_client()
        .finalize_psbt(&psbt, None)?
        .hex
        .ok_or_else(|| anyhow!("unable to sign transaction"))?;

      (
        wallet.bitcoin_client().send_raw_transaction(&signed_tx)?,
        psbt,
      )
    };

    let mut fee = 0;
    for txin in unsigned_transaction.input.iter() {
      let Some(txout) = unspent_outputs.get(&txin.previous_output) else {
        panic!("input {} not found in utxos", txin.previous_output);
      };
      fee += txout.value;
    }

    for txout in unsigned_transaction.output.iter() {
      fee = fee.checked_sub(txout.value).unwrap();
    }

    Ok(Some(Box::new(Output {
      txid,
      psbt,
      outgoing: self.outgoing,
      fee,
    })))
  }

  fn create_unsigned_convert_runes_transaction(
    wallet: &Wallet,
    spaced_rune: SpacedRune,
    decimal: Decimal,
    postage: Amount,
    fee_rate: FeeRate,
  ) -> Result<Transaction> {
    ensure!(
      wallet.has_rune_index(),
      "sending runes with `ord send` requires index created with `--index-runes` flag",
    );

    wallet.lock_non_cardinal_outputs()?;

    let (id, entry, _parent) = wallet
      .get_rune(spaced_rune.rune)?
      .with_context(|| format!("rune `{}` has not been etched", spaced_rune.rune))?;

    let amount = decimal.to_integer(entry.divisibility)?;

    let inscribed_outputs = wallet
      .inscriptions()
      .keys()
      .map(|satpoint| satpoint.outpoint)
      .collect::<HashSet<OutPoint>>();

    let balances = wallet
      .get_runic_outputs()?
      .into_iter()
      .filter(|output| !inscribed_outputs.contains(output))
      .map(|output| {
        wallet.get_runes_balances_in_output(&output).map(|balance| {
          (
            output,
            balance
              .into_iter()
              .map(|(spaced_rune, pile)| (spaced_rune.rune, pile))
              .collect(),
          )
        })
      })
      .collect::<Result<BTreeMap<OutPoint, BTreeMap<Rune, Pile>>>>()?;

    let mut inputs = Vec::new();
    let mut input_rune_balances: BTreeMap<Rune, u128> = BTreeMap::new();

    for (output, runes) in balances {
      if let Some(balance) = runes.get(&spaced_rune.rune) {
        if balance.amount > 0 {
          *input_rune_balances.entry(spaced_rune.rune).or_default() += balance.amount;

          inputs.push(output);
        }
      }

      if input_rune_balances
        .get(&spaced_rune.rune)
        .cloned()
        .unwrap_or_default()
        >= amount
      {
        break;
      }
    }

    let input_rune_balance = input_rune_balances
      .get(&spaced_rune.rune)
      .cloned()
      .unwrap_or_default();

    let needs_runes_change_output = input_rune_balance > amount || input_rune_balances.len() > 1;

    ensure! {
      input_rune_balance >= amount,
      "insufficient `{}` balance, only {} in wallet",
      spaced_rune,
      Pile {
        amount: input_rune_balance,
        divisibility: entry.divisibility,
        symbol: entry.symbol
      },
    }

    let Some((_, rune_entry_in, _)) = wallet.get_rune(spaced_rune.rune)? else {
      bail!("rune has not been etched");
    };
    let Some((id_out, rune_entry_out, _)) = wallet.get_rune(Rune(1 - spaced_rune.rune.n()))? else {
      bail!("rune has not been etched");
    };

    let invariant =
      rune_entry_in.supply * rune_entry_in.supply + rune_entry_out.supply * rune_entry_out.supply;
    let new_input_sq = (rune_entry_in.supply - amount) * (rune_entry_in.supply - amount);
    let expected_output_amt = (invariant - new_input_sq).sqrt() - rune_entry_out.supply;

    let allowable_slippage = 20; // 20bps
    let min_output_amt = expected_output_amt * (10000 - allowable_slippage) / 10000;

    let runestone = Runestone {
      edicts: if needs_runes_change_output {
        vec![
          Edict {
            amount: input_rune_balance - amount,
            id,
            output: 1,
          },
          Edict {
            amount: min_output_amt,
            id: id_out,
            output: 1,
          },
        ]
      } else {
        vec![Edict {
          amount: min_output_amt,
          id: id_out,
          output: 1,
        }]
      },
      pointer: Some(0),
    };

    let unfunded_transaction = Transaction {
      version: 2,
      lock_time: LockTime::ZERO,
      input: inputs
        .into_iter()
        .map(|previous_output| TxIn {
          previous_output,
          script_sig: ScriptBuf::new(),
          sequence: Sequence::MAX,
          witness: Witness::new(),
        })
        .collect(),
      output: vec![
        TxOut {
          script_pubkey: runestone.encipher(),
          value: 0,
        },
        TxOut {
          script_pubkey: wallet.get_change_address()?.script_pubkey(),
          value: postage.to_sat(),
        },
      ],
    };

    let unsigned_transaction =
      fund_raw_transaction(wallet.bitcoin_client(), fee_rate, &unfunded_transaction)?;

    let unsigned_transaction = consensus::encode::deserialize(&unsigned_transaction)?;

    assert_eq!(
      Runestone::decipher(&unsigned_transaction),
      Some(Artifact::Runestone(runestone)),
    );

    Ok(unsigned_transaction)
  }
}
