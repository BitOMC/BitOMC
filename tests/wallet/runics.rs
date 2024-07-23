use {
  super::*,
  bitomc::{decimal::Decimal, subcommand::wallet::runics::RunicUtxo},
};

#[test]
fn wallet_runics() {
  let core = mockcore::builder().network(Network::Regtest).build();
  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  create_wallet(&core, &bitomc);

  core.mine_blocks(1);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .ord(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  pretty_assert_eq!(
    CommandBuilder::new("--regtest wallet runics")
      .core(&core)
      .ord(&bitomc)
      .run_and_deserialize_output::<Vec<RunicUtxo>>()
      .first()
      .unwrap()
      .runes,
    vec![(
      SpacedRune {
        rune: Rune(TIGHTEN),
        spacers: 0
      },
      Decimal {
        value: 50,
        scale: 0
      }
    )]
    .into_iter()
    .collect()
  );
}
