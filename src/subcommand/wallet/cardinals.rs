use super::*;

#[derive(Serialize, Deserialize)]
pub struct CardinalUtxo {
  pub output: OutPoint,
  pub amount: u64,
}

#[allow(dead_code)]
pub(crate) fn run(wallet: Wallet) -> SubcommandResult {
  let unspent_outputs = wallet.utxos();

  let runic_utxos = wallet.get_runic_outputs()?;

  let cardinal_utxos = unspent_outputs
    .iter()
    .filter_map(|(output, txout)| {
      if runic_utxos.contains(output) {
        None
      } else {
        Some(CardinalUtxo {
          output: *output,
          amount: txout.value,
        })
      }
    })
    .collect::<Vec<CardinalUtxo>>();

  Ok(Some(Box::new(cardinal_utxos)))
}
