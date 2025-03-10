use {super::*, num_integer::Roots};

#[derive(Debug, Parser)]
pub(crate) struct Mint {
  #[arg(long, help = "Don't sign or broadcast transaction")]
  pub(crate) dry_run: bool,
  #[clap(long, help = "Use <FEE_RATE> sats/vbyte for mint transaction.")]
  fee_rate: FeeRate,
  #[clap(
    long,
    help = "Include <AMOUNT> postage with mint output. [default: 10000sat]"
  )]
  postage: Option<Amount>,
  #[clap(
    long,
    help = "Include <AMOUNT> dust with anyone-can-spend output. [default: 330sat]"
  )]
  dust: Option<Amount>,
  #[clap(long, help = "Send minted runes to <DESTINATION>.")]
  destination: Option<Address<NetworkUnchecked>>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Output {
  pub rune0: SpacedRune,
  pub rune1: SpacedRune,
  pub pile0: Pile,
  pub pile1: Pile,
  pub mint: Txid,
  pub connected: bool,
}

impl Mint {
  pub(crate) fn run(self, wallet: Wallet) -> SubcommandResult {
    let bitcoin_client = wallet.bitcoin_client();

    let block_height = bitcoin_client.get_block_count()?;

    let Some((_, rune_entry0, _)) = wallet.get_rune(Rune(0))? else {
      bail!("rune has not been etched");
    };
    let Some((_, rune_entry1, _)) = wallet.get_rune(Rune(1))? else {
      bail!("rune has not been etched");
    };
    let (last_mint_outpoint, last_mint_txout_value) = wallet.get_last_mint_outpoint()?;

    let postage = self.postage.unwrap_or(TARGET_POSTAGE);
    let p2wsh_dust = self.dust.unwrap_or(TARGET_P2WSH_DUST);

    let reward = rune_entry0.reward(block_height as u128 + 1);

    let sum_of_sq =
      rune_entry0.supply * rune_entry0.supply + rune_entry1.supply * rune_entry1.supply;
    let mut amount0 = rune_entry0.burned;
    let mut amount1 = rune_entry1.burned;
    if sum_of_sq == 0 {
      // Assign entire reward to amount0
      amount0 += reward;
    } else {
      // Split reward between runes such that converted supply increases by `reward`
      let k = sum_of_sq.sqrt();
      amount0 += rune_entry0.supply * reward / k;
      amount1 += rune_entry1.supply * reward / k;
    }

    if amount0 == 0 && amount1 == 0 {
      bail!(
        "No reward for minting. Wait until block {}.",
        rune_entry0.block
      );
    }

    let chain = wallet.chain();

    let destination = match self.destination {
      Some(destination) => destination.require_network(chain.network())?,
      None => wallet.get_change_address()?,
    };

    ensure!(
      destination.script_pubkey().dust_value() <= postage,
      "postage below dust limit of {}sat",
      destination.script_pubkey().dust_value().to_sat()
    );

    let runestone = Runestone { ..default() };

    let op_return_script_pubkey = runestone.encipher();

    ensure!(
      op_return_script_pubkey.len() <= 82,
      "runestone greater than maximum OP_RETURN size: {} > 82",
      op_return_script_pubkey.len()
    );

    // 1 CHECKSEQUENCEVERIFY (anyone can spend after 1 block)
    let mint_script = ScriptBuf::from_bytes(Vec::from(&[0x51, 0xb2]));
    let mint_script_pubkey = ScriptBuf::new_v0_p2wsh(&mint_script.clone().wscript_hash());

    let input = TxIn {
      previous_output: last_mint_outpoint,
      script_sig: ScriptBuf::new(),
      sequence: Sequence::from_height(1),
      witness: Witness::from_slice(&[mint_script.clone().into_bytes()]),
    };

    let mut fee_for_input = 0;
    if last_mint_outpoint != OutPoint::null() {
      let input_vb = (input.segwit_weight() + 2) / 4; // include 2WU for segwit marker
      fee_for_input = self.fee_rate.fee(input_vb).to_sat();
    }

    let unfunded_transaction = Transaction {
      version: 2,
      lock_time: LockTime::ZERO,
      input: Vec::new(),
      output: vec![
        TxOut {
          script_pubkey: mint_script_pubkey,
          value: p2wsh_dust.to_sat() + fee_for_input,
        },
        TxOut {
          script_pubkey: destination.script_pubkey(),
          value: postage.to_sat(),
        },
        TxOut {
          script_pubkey: op_return_script_pubkey,
          value: 0,
        },
      ],
    };

    wallet.lock_non_cardinal_outputs()?;

    let fund_transaction_result =
      fund_raw_transaction_result(bitcoin_client, self.fee_rate, &unfunded_transaction)?;

    let mut unsigned_transaction = fund_transaction_result.transaction()?;

    // Add previous mint output as an input
    if last_mint_outpoint != OutPoint::null() {
      unsigned_transaction.output[0].value -= fee_for_input;
      if unsigned_transaction.output.len() > 3 {
        // If change output exists, add input amount to it
        unsigned_transaction.output[3].value += last_mint_txout_value;
      } else {
        // Otherwise, add input amount to runic output
        unsigned_transaction.output[1].value += last_mint_txout_value;
      }
      unsigned_transaction.input.push(input);
    }

    let signed_transaction =
      bitcoin_client.sign_raw_transaction_with_wallet(&unsigned_transaction, None, None)?;

    assert_eq!(
      Runestone::decipher(&consensus::encode::deserialize(&signed_transaction.hex)?),
      Some(Artifact::Runestone(runestone)),
    );

    let transaction = if self.dry_run {
      signed_transaction.transaction()?.txid()
    } else {
      bitcoin_client.send_raw_transaction(&signed_transaction.hex)?
    };

    Ok(Some(Box::new(Output {
      rune0: rune_entry0.spaced_rune,
      rune1: rune_entry1.spaced_rune,
      pile0: Pile {
        amount: amount0,
        divisibility: rune_entry0.divisibility,
        symbol: rune_entry0.symbol,
      },
      pile1: Pile {
        amount: amount1,
        divisibility: rune_entry1.divisibility,
        symbol: rune_entry1.symbol,
      },
      mint: transaction,
      connected: last_mint_outpoint != OutPoint::null(),
    })))
  }
}
