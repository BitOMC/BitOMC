use super::*;

#[test]
fn send_amount_does_not_select_runic_utxos() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  create_wallet(&core, &bitomc);

  core.mine_blocks(1);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  drain(&core, &bitomc);

  CommandBuilder::new("--regtest wallet send --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 600sat")
    .core(&core)
    .bitomc(&bitomc)
    .expected_exit_code(1)
    .expected_stderr("error: not enough cardinal utxos\n")
    .run_and_extract_stdout();
}

#[test]
fn mint_does_not_select_runic_utxos() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  create_wallet(&core, &bitomc);

  core.mine_blocks(1);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  drain(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 0")
    .core(&core)
    .bitomc(&bitomc)
    .expected_exit_code(1)
    .expected_stderr("error: not enough cardinal utxos\n")
    .run_and_extract_stdout();
}

#[test]
fn sending_rune_does_not_send_runic_utxos() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  create_wallet(&core, &bitomc);

  let rune = Rune(TIGHTEN);

  core.mine_blocks_with_subsidy(1, 10000);

  pretty_assert_eq!(
    CommandBuilder::new("--regtest wallet balance")
      .core(&core)
      .bitomc(&bitomc)
      .run_and_deserialize_output::<Balance>(),
    Balance {
      cardinal: 10000,
      runic: 0,
      runes: BTreeMap::new(),
      total: 10000,
    }
  );

  core.mine_blocks(1);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  drain(&core, &bitomc);

  CommandBuilder::new(format!(
    "
       --chain regtest
       wallet send
       --postage 11111sat
       --fee-rate 0
       bcrt1pyrmadgg78e38ewfv0an8c6eppk2fttv5vnuvz04yza60qau5va0saknu8k
       5:{rune}
     ",
  ))
  .core(&core)
  .bitomc(&bitomc)
  .expected_exit_code(1)
  .expected_stderr("error: not enough cardinal utxos\n")
  .run_and_extract_stdout();
}
