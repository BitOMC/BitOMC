use {super::*, bitomc::decimal::Decimal};

#[test]
fn wallet_balance() {
  let core = mockcore::spawn();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &bitomc);

  assert_eq!(
    CommandBuilder::new("wallet balance")
      .core(&core)
      .bitomc(&bitomc)
      .run_and_deserialize_output::<Balance>()
      .cardinal,
    0
  );

  core.mine_blocks(1);

  assert_eq!(
    CommandBuilder::new("wallet balance")
      .core(&core)
      .bitomc(&bitomc)
      .run_and_deserialize_output::<Balance>(),
    Balance {
      cardinal: 50 * COIN_VALUE,
      runic: 0,
      runes: BTreeMap::new(),
      total: 50 * COIN_VALUE,
    }
  );
}

#[test]
fn unsynced_wallet_fails_with_unindexed_output() {
  let core = mockcore::spawn();
  let bitomc = TestServer::spawn(&core);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  assert_eq!(
    CommandBuilder::new("wallet balance")
      .bitomc(&bitomc)
      .core(&core)
      .run_and_deserialize_output::<Balance>(),
    Balance {
      cardinal: 50 * COIN_VALUE,
      runic: 0,
      runes: BTreeMap::new(),
      total: 50 * COIN_VALUE,
    }
  );

  let no_sync_ord = TestServer::spawn_with_server_args(&core, &[], &["--no-sync"]);

  // inscribe(&core, &bitomc);

  CommandBuilder::new("wallet balance")
    .bitomc(&no_sync_ord)
    .core(&core)
    .expected_exit_code(1)
    .expected_stderr("error: wallet failed to synchronize with `bitomc server` after 20 attempts\n")
    .run_and_extract_stdout();
}

#[test]
fn runic_utxos_are_deducted_from_cardinal_and_displayed_with_decimal_amount() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  create_wallet(&core, &bitomc);

  pretty_assert_eq!(
    CommandBuilder::new("--regtest wallet balance")
      .core(&core)
      .bitomc(&bitomc)
      .run_and_deserialize_output::<Balance>(),
    Balance {
      cardinal: 0,
      runic: 0,
      runes: BTreeMap::new(),
      total: 0,
    }
  );

  core.mine_blocks(1);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  pretty_assert_eq!(
    CommandBuilder::new("--regtest wallet balance")
      .core(&core)
      .bitomc(&bitomc)
      .run_and_deserialize_output::<Balance>(),
    Balance {
      cardinal: 50 * COIN_VALUE * 2 - 10_000 - 330,
      runic: 10_000,
      runes: vec![(
        SpacedRune {
          rune: Rune(TIGHTEN),
          spacers: 0
        },
        Decimal {
          value: 50,
          scale: 0,
        }
      )]
      .into_iter()
      .collect(),
      total: 50 * COIN_VALUE * 2 - 330,
    }
  );
}
