use super::*;

#[test]
fn send_amount_does_not_select_runic_utxos() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let ord = TestServer::spawn_with_server_args(&core, &["--regtest", "--index-runes"], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  CommandBuilder::new("--chain regtest --index-runes wallet mint --fee-rate 1")
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<ord::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  drain(&core, &ord);

  CommandBuilder::new("--regtest --index-runes wallet send --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 600sat")
    .core(&core)
    .ord(&ord)
    .expected_exit_code(1)
    .expected_stderr("error: not enough cardinal utxos\n")
    .run_and_extract_stdout();
}

#[test]
fn mint_does_not_select_runic_utxos() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let ord = TestServer::spawn_with_server_args(&core, &["--index-runes", "--regtest"], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  CommandBuilder::new("--chain regtest --index-runes wallet mint --fee-rate 1")
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<ord::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  drain(&core, &ord);

  CommandBuilder::new("--chain regtest --index-runes wallet mint --fee-rate 0")
    .core(&core)
    .ord(&ord)
    .expected_exit_code(1)
    .expected_stderr("error: not enough cardinal utxos\n")
    .run_and_extract_stdout();
}

#[test]
fn sending_rune_does_not_send_runic_utxos() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let ord = TestServer::spawn_with_server_args(&core, &["--index-runes", "--regtest"], &[]);

  create_wallet(&core, &ord);

  let rune = Rune(TIGHTEN);

  core.mine_blocks_with_subsidy(1, 10000);

  pretty_assert_eq!(
    CommandBuilder::new("--regtest --index-runes wallet balance")
      .core(&core)
      .ord(&ord)
      .run_and_deserialize_output::<Balance>(),
    Balance {
      cardinal: 10000,
      runic: Some(0),
      runes: Some(BTreeMap::new()),
      total: 10000,
    }
  );

  core.mine_blocks(1);

  CommandBuilder::new("--chain regtest --index-runes wallet mint --fee-rate 1")
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<ord::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  drain(&core, &ord);

  CommandBuilder::new(format!(
    "
       --chain regtest
       --index-runes
       wallet send
       --postage 11111sat
       --fee-rate 0
       bcrt1pyrmadgg78e38ewfv0an8c6eppk2fttv5vnuvz04yza60qau5va0saknu8k
       5:{rune}
     ",
  ))
  .core(&core)
  .ord(&ord)
  .expected_exit_code(1)
  .expected_stderr("error: not enough cardinal utxos\n")
  .run_and_extract_stdout();
}
