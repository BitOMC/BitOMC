use {super::*, bitomc::subcommand::wallet::balance::Output};

#[test]
fn authentication() {
  let core = mockcore::spawn();

  let bitomc = TestServer::spawn_with_server_args(
    &core,
    &["--server-username", "foo", "--server-password", "bar"],
    &[],
  );

  create_wallet(&core, &bitomc);

  assert_eq!(
    CommandBuilder::new("--server-username foo --server-password bar wallet balance")
      .core(&core)
      .ord(&bitomc)
      .run_and_deserialize_output::<Output>()
      .cardinal,
    0
  );

  core.mine_blocks(1);

  assert_eq!(
    CommandBuilder::new("--server-username foo --server-password bar wallet balance")
      .core(&core)
      .ord(&bitomc)
      .run_and_deserialize_output::<Output>(),
    Output {
      cardinal: 50 * COIN_VALUE,
      runic: 0,
      runes: BTreeMap::new(),
      total: 50 * COIN_VALUE,
    }
  );
}
