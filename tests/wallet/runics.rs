use {
  super::*,
  ord::{decimal::Decimal, subcommand::wallet::runics::RunicUtxo},
};

#[test]
fn wallet_runics() {
  let core = mockcore::builder().network(Network::Regtest).build();
  let ord = TestServer::spawn_with_server_args(&core, &["--regtest", "--index-runes"], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  CommandBuilder::new("--chain regtest --index-runes wallet mint --fee-rate 1")
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<ord::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  pretty_assert_eq!(
    CommandBuilder::new("--regtest --index-runes wallet runics")
      .core(&core)
      .ord(&ord)
      .run_and_deserialize_output::<Vec<RunicUtxo>>()
      .first()
      .unwrap()
      .runes,
    vec![(
      SpacedRune { rune: Rune(TIGHTEN), spacers: 0 },
      Decimal {
        value: 50,
        scale: 0
      }
    )]
    .into_iter()
    .collect()
  );
}
