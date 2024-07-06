#![allow(clippy::type_complexity)]

use {
  self::{command_builder::CommandBuilder, expected::Expected, test_server::TestServer},
  bitcoin::{
    address::{Address, NetworkUnchecked},
    blockdata::constants::COIN_VALUE,
    Network, OutPoint, Txid, Witness,
  },
  bitcoincore_rpc::bitcoincore_rpc_json::ListDescriptorsResult,
  chrono::{DateTime, Utc},
  executable_path::executable_path,
  mockcore::TransactionTemplate,
  ord::{api, chain::Chain, outgoing::Outgoing, InscriptionId, RuneEntry},
  ordinals::{
    Artifact, Charm, Edict, Pile, Rarity, Rune, RuneId, Runestone, Sat, SatPoint, SpacedRune,
  },
  pretty_assertions::assert_eq as pretty_assert_eq,
  regex::Regex,
  reqwest::{StatusCode, Url},
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
    str::{self, FromStr},
    thread,
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
mod decode;
mod epochs;
mod find;
mod index;
mod info;
mod json_api;
mod list;
mod parse;
mod runes;
mod server;
mod settings;
mod subsidy;
mod supply;
mod traits;
mod version;
mod wallet;

const TIGHTEN: u128 = 0;
const EASE: u128 = 1;

const ID0: RuneId = RuneId { block: 1, tx: 0 };
const ID1: RuneId = RuneId { block: 1, tx: 1 };

const RUNE_COIN_VALUE: u128 = 100000000;

type Balance = ord::subcommand::wallet::balance::Output;
type Batch = ord::wallet::batch::Output;
type Create = ord::subcommand::wallet::create::Output;
type Inscriptions = Vec<ord::subcommand::wallet::inscriptions::Output>;
type Send = ord::subcommand::wallet::send::Output;
type Supply = ord::subcommand::supply::Output;

fn create_wallet(core: &mockcore::Handle, ord: &TestServer) {
  CommandBuilder::new(format!("--chain {} wallet create", core.network()))
    .core(core)
    .ord(ord)
    .stdout_regex(".*")
    .run_and_extract_stdout();
}

fn sats(
  core: &mockcore::Handle,
  ord: &TestServer,
) -> Vec<ord::subcommand::wallet::sats::OutputRare> {
  CommandBuilder::new(format!("--chain {} wallet sats", core.network()))
    .core(core)
    .ord(ord)
    .run_and_deserialize_output::<Vec<ord::subcommand::wallet::sats::OutputRare>>()
}

fn inscribe(core: &mockcore::Handle, ord: &TestServer) -> (InscriptionId, Txid) {
  core.mine_blocks(1);

  let output = CommandBuilder::new(format!(
    "--chain {} wallet inscribe --fee-rate 1 --file foo.txt",
    core.network()
  ))
  .write("foo.txt", "FOO")
  .core(core)
  .ord(ord)
  .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  assert_eq!(output.inscriptions.len(), 1);

  (output.inscriptions[0].id, output.reveal)
}

fn drain(core: &mockcore::Handle, ord: &TestServer) {
  let balance = CommandBuilder::new("--regtest --index-runes wallet balance")
    .core(core)
    .ord(ord)
    .run_and_deserialize_output::<Balance>();

  CommandBuilder::new(format!(
    "
      --chain regtest
      --index-runes
      wallet send
      --fee-rate 0
      bcrt1pyrmadgg78e38ewfv0an8c6eppk2fttv5vnuvz04yza60qau5va0saknu8k
      {}sat
    ",
    balance.cardinal
  ))
  .core(core)
  .ord(ord)
  .run_and_deserialize_output::<Send>();

  core.mine_blocks_with_subsidy(1, 0);

  let balance = CommandBuilder::new("--regtest --index-runes wallet balance")
    .core(core)
    .ord(ord)
    .run_and_deserialize_output::<Balance>();

  pretty_assert_eq!(balance.cardinal, 0);
}

fn envelope(payload: &[&[u8]]) -> Witness {
  let mut builder = bitcoin::script::Builder::new()
    .push_opcode(bitcoin::opcodes::OP_FALSE)
    .push_opcode(bitcoin::opcodes::all::OP_IF);

  for data in payload {
    let mut buf = bitcoin::script::PushBytesBuf::new();
    buf.extend_from_slice(data).unwrap();
    builder = builder.push_slice(buf);
  }

  let script = builder
    .push_opcode(bitcoin::opcodes::all::OP_ENDIF)
    .into_script();

  Witness::from_slice(&[script.into_bytes(), Vec::new()])
}

fn default<T: Default>() -> T {
  Default::default()
}
