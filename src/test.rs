pub(crate) use {
  super::*,
  bitcoin::{constants::COIN_VALUE, opcodes, WPubkeyHash},
  mockcore::TransactionTemplate,
  pretty_assertions::assert_eq as pretty_assert_eq,
  std::iter,
  tempfile::TempDir,
  unindent::Unindent,
};

pub(crate) fn txid(n: u64) -> Txid {
  let hex = format!("{n:x}");

  if hex.is_empty() || hex.len() > 1 {
    panic!();
  }

  hex.repeat(64).parse().unwrap()
}

pub(crate) fn outpoint(n: u64) -> OutPoint {
  format!("{}:{}", txid(n), n).parse().unwrap()
}

pub(crate) fn change(n: u64) -> Address {
  match n {
    0 => "tb1qjsv26lap3ffssj6hfy8mzn0lg5vte6a42j75ww",
    1 => "tb1qakxxzv9n7706kc3xdcycrtfv8cqv62hnwexc0l",
    2 => "tb1qxz9yk0td0yye009gt6ayn7jthz5p07a75luryg",
    3 => "tb1qe62s57n77pfhlw2vtqlhm87dwj75l6fguavjjq",
    _ => panic!(),
  }
  .parse::<Address<NetworkUnchecked>>()
  .unwrap()
  .assume_checked()
}

pub(crate) fn inscription_id(n: u32) -> InscriptionId {
  let hex = format!("{n:x}");

  if hex.is_empty() || hex.len() > 1 {
    panic!();
  }

  format!("{}i{n}", hex.repeat(64)).parse().unwrap()
}

#[allow(dead_code)]
pub(crate) fn default_address(chain: Chain) -> Address {
  Address::from_script(
    &ScriptBuf::new_v0_p2wpkh(&WPubkeyHash::all_zeros()),
    chain.network(),
  )
  .unwrap()
}
