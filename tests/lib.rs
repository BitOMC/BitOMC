#![allow(clippy::type_complexity)]

use {
  self::{command_builder::CommandBuilder, expected::Expected, test_server::TestServer},
  bitcoin::{
    address::{Address, NetworkUnchecked},
    blockdata::constants::COIN_VALUE,
    Network, OutPoint, Witness,
  },
  bitcoincore_rpc::bitcoincore_rpc_json::ListDescriptorsResult,
  bitomc::{api, chain::Chain, outgoing::Outgoing, RuneEntry},
  chrono::{DateTime, Utc},
  executable_path::executable_path,
  mockcore::TransactionTemplate,
  pretty_assertions::assert_eq as pretty_assert_eq,
  regex::Regex,
  reqwest::{StatusCode, Url},
  runes_bitomc::{Artifact, Edict, Pile, Rune, RuneId, Runestone, SpacedRune},
  serde::de::DeserializeOwned,
  std::sync::Arc,
  std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs,
    io::{BufRead, BufReader, Write},
    net::TcpListener,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    str, thread,
    time::Duration,
  },
  tempfile::TempDir,
};

macro_rules! assert_regex_match {
  ($value:expr, $pattern:expr $(,)?) => {
    let regex = Regex::new(&format!("^(?s){}$", $pattern)).unwrap();
    let string = $value.to_string();

    if !regex.is_match(string.as_ref()) {
      eprintln!("Regex did not match:");
      pretty_assert_eq!(regex.as_str(), string);
    }
  };
}

mod command_builder;
mod expected;
mod test_server;

mod balances;
mod index;
mod info;
mod json_api;
mod server;
mod settings;
mod version;
mod wallet;

const TIGHTEN: u128 = 0;
const EASE: u128 = 1;

const ID0: RuneId = RuneId { block: 1, tx: 0 };
const ID1: RuneId = RuneId { block: 1, tx: 1 };

const RUNE_COIN_VALUE: u128 = 100000000;

type Balance = bitomc::subcommand::wallet::balance::Output;
type Create = bitomc::subcommand::wallet::create::Output;
type Send = bitomc::subcommand::wallet::send::Output;

fn create_wallet(core: &mockcore::Handle, bitomc: &TestServer) {
  CommandBuilder::new(format!("--chain {} wallet create", core.network()))
    .core(core)
    .bitomc(bitomc)
    .stdout_regex(".*")
    .run_and_extract_stdout();
}

fn drain(core: &mockcore::Handle, bitomc: &TestServer) {
  let balance = CommandBuilder::new("--regtest wallet balance")
    .core(core)
    .bitomc(bitomc)
    .run_and_deserialize_output::<Balance>();

  CommandBuilder::new(format!(
    "
      --chain regtest
      wallet send
      --fee-rate 0
      bcrt1pyrmadgg78e38ewfv0an8c6eppk2fttv5vnuvz04yza60qau5va0saknu8k
      {}sat
    ",
    balance.cardinal
  ))
  .core(core)
  .bitomc(bitomc)
  .run_and_deserialize_output::<Send>();

  core.mine_blocks_with_subsidy(1, 0);

  let balance = CommandBuilder::new("--regtest wallet balance")
    .core(core)
    .bitomc(bitomc)
    .run_and_deserialize_output::<Balance>();

  pretty_assert_eq!(balance.cardinal, 0);
}

fn default<T: Default>() -> T {
  Default::default()
}
