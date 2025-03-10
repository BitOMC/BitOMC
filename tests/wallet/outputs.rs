use {super::*, bitomc::subcommand::wallet::outputs::Output};

#[test]
fn outputs() {
  let core = mockcore::spawn();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &bitomc);

  let coinbase_tx = &core.mine_blocks_with_subsidy(1, 1_000_000)[0].txdata[0];
  let outpoint = OutPoint::new(coinbase_tx.txid(), 0);
  let amount = coinbase_tx.output[0].value;

  let output = CommandBuilder::new("wallet outputs")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<Vec<Output>>();

  assert_eq!(output[0].output, outpoint);
  assert_eq!(output[0].amount, amount);
}

#[test]
fn outputs_includes_locked_outputs() {
  let core = mockcore::spawn();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &bitomc);

  let coinbase_tx = &core.mine_blocks_with_subsidy(1, 1_000_000)[0].txdata[0];
  let outpoint = OutPoint::new(coinbase_tx.txid(), 0);
  let amount = coinbase_tx.output[0].value;

  core.lock(outpoint);

  let output = CommandBuilder::new("wallet outputs")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<Vec<Output>>();

  assert_eq!(output[0].output, outpoint);
  assert_eq!(output[0].amount, amount);
}

#[test]
fn outputs_includes_unbound_outputs() {
  let core = mockcore::spawn();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &bitomc);

  let coinbase_tx = &core.mine_blocks_with_subsidy(1, 1_000_000)[0].txdata[0];
  let outpoint = OutPoint::new(coinbase_tx.txid(), 0);
  let amount = coinbase_tx.output[0].value;

  core.lock(outpoint);

  let output = CommandBuilder::new("wallet outputs")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<Vec<Output>>();

  assert_eq!(output[0].output, outpoint);
  assert_eq!(output[0].amount, amount);
}
