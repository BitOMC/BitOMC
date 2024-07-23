use {super::*, bitomc::subcommand::wallet::receive};

#[test]
fn receive() {
  let core = mockcore::spawn();
  let bitomc = TestServer::spawn(&core);

  create_wallet(&core, &bitomc);

  let output = CommandBuilder::new("wallet receive")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<receive::Output>();

  assert!(output
    .addresses
    .first()
    .unwrap()
    .is_valid_for_network(Network::Bitcoin));
}
