use super::*;

#[derive(Debug, Parser)]
pub(crate) struct Mint {
  #[clap(long, help = "Use <FEE_RATE> sats/vbyte for mint transaction.")]
  fee_rate: FeeRate,
  #[clap(
    long,
    help = "Include <AMOUNT> postage with mint output. [default: 10000sat]"
  )]
  postage: Option<Amount>,
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
}

impl Mint {
  #[allow(clippy::cast_possible_truncation)]
  #[allow(clippy::cast_sign_loss)]
  pub(crate) fn run(self, wallet: Wallet) -> SubcommandResult {
    ensure!(
      wallet.has_rune_index(),
      "`ord wallet mint` requires index created with `--index-runes` flag",
    );

    let bitcoin_client = wallet.bitcoin_client();

    let block_height = bitcoin_client.get_block_count()?;

    let Some((_, rune_entry0, _)) = wallet.get_rune(Rune(0))? else {
      bail!("rune has not been etched");
    };
    let Some((_, rune_entry1, _)) = wallet.get_rune(Rune(1))? else {
      bail!("rune has not been etched");
    };

    let postage = self.postage.unwrap_or(TARGET_POSTAGE);

    let amount = rune_entry0.mintable(block_height + 1);

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
      previous_output: OutPoint::new(rune_entry0.etching, 0),
      script_sig: ScriptBuf::new(),
      sequence: Sequence::from_height(1),
      witness: Witness::from_slice(&[mint_script.clone().into_bytes()]),
    };

    let mut fee_for_input = 0;
    let mut input_amount = 0;
    if rune_entry0.etching != Txid::all_zeros() {
      let input_tx = bitcoin_client.get_transaction(&rune_entry0.etching, None)?;
      if !input_tx.details.is_empty() {
        input_amount = input_tx.details[0].amount.to_sat().unsigned_abs();
      }
      let input_vb = (input.segwit_weight() + 2) / 4; // include 2WU for segwit marker

      // #[allow(clippy::cast_possible_truncation)]
      // #[allow(clippy::cast_sign_loss)]
      // fee_for_input = (self.fee_rate.n().round() as u64) * (input_vb as u64);

      fee_for_input = (self.fee_rate.n().round() * input_vb as f64).round() as u64;
    }

    let unfunded_transaction = Transaction {
      version: 2,
      lock_time: LockTime::ZERO,
      input: Vec::new(),
      output: vec![
        TxOut {
          script_pubkey: mint_script_pubkey,
          value: postage.to_sat() + fee_for_input,
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
    if rune_entry0.etching != Txid::all_zeros() {
      unsigned_transaction.output[0].value -= fee_for_input;
      if unsigned_transaction.output.len() > 3 {
        // If change output exists, add input amount to it
        unsigned_transaction.output[3].value += input_amount;
      } else {
        // Otherwise, add input amount to runic output
        unsigned_transaction.output[1].value += input_amount;
      }
      unsigned_transaction.input.push(input);
    }

    let signed_transaction = bitcoin_client
      .sign_raw_transaction_with_wallet(&unsigned_transaction, None, None)?
      .hex;

    assert_eq!(
      Runestone::decipher(&consensus::encode::deserialize(&signed_transaction)?),
      Some(Artifact::Runestone(runestone)),
    );

    let transaction = bitcoin_client.send_raw_transaction(&signed_transaction)?;

    Ok(Some(Box::new(Output {
      rune0: rune_entry0.spaced_rune,
      rune1: rune_entry1.spaced_rune,
      pile0: Pile {
        amount,
        divisibility: rune_entry0.divisibility,
        symbol: rune_entry0.symbol,
      },
      pile1: Pile {
        amount: 0,
        divisibility: rune_entry1.divisibility,
        symbol: rune_entry1.symbol,
      },
      mint: transaction,
    })))
  }
}
