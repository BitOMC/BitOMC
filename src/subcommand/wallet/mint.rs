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
  pub rune: SpacedRune,
  pub pile: Pile,
  pub mint: Txid,
}

impl Mint {
  pub(crate) fn run(self, wallet: Wallet) -> SubcommandResult {
    ensure!(
      wallet.has_rune_index(),
      "`ord wallet mint` requires index created with `--index-runes` flag",
    );

    let bitcoin_client = wallet.bitcoin_client();

    let block_height = bitcoin_client.get_block_count()?;

    let Some((_, rune_entry, _)) = wallet.get_rune(Rune(0))? else {
      bail!("rune has not been etched");
    };

    let postage = self.postage.unwrap_or(TARGET_POSTAGE);

    let amount = rune_entry.mintable(block_height + 1);

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
    let mint_script_pubkey = ScriptBuf::new_p2sh(&mint_script.clone().script_hash());

    let input = Vec::new();

    // if rune_entry.etching != Txid::all_zeros() {
    //   input.push(TxIn {
    //     previous_output: OutPoint::new(rune_entry.etching, 0),
    //     script_sig: mint_script.clone(),
    //     sequence: Sequence::from_height(1),
    //     witness: witness.clone(),
    //   });
    // }

    let unfunded_transaction = Transaction {
      version: 2,
      lock_time: LockTime::ZERO,
      input,
      output: vec![
        TxOut {
          script_pubkey: mint_script_pubkey,
          value: postage.to_sat(),
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

    let unsigned_transaction =
      fund_raw_transaction(bitcoin_client, self.fee_rate, &unfunded_transaction)?;

    let signed_transaction = bitcoin_client
      .sign_raw_transaction_with_wallet(&unsigned_transaction, None, None)?
      .hex;

    let signed_transaction = consensus::encode::deserialize(&signed_transaction)?;

    assert_eq!(
      Runestone::decipher(&signed_transaction),
      Some(Artifact::Runestone(runestone)),
    );

    let transaction = bitcoin_client.send_raw_transaction(&signed_transaction)?;

    Ok(Some(Box::new(Output {
      rune: rune_entry.spaced_rune,
      pile: Pile {
        amount,
        divisibility: rune_entry.divisibility,
        symbol: rune_entry.symbol,
      },
      mint: transaction,
    })))
  }
}
