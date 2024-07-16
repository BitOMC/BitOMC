use {super::*, ord::decimal::Decimal};

#[test]
fn wallet_balance() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  assert_eq!(
    CommandBuilder::new("wallet balance")
      .core(&core)
      .ord(&ord)
      .run_and_deserialize_output::<Balance>()
      .cardinal,
    0
  );

  core.mine_blocks(1);

  assert_eq!(
    CommandBuilder::new("wallet balance")
      .core(&core)
      .ord(&ord)
      .run_and_deserialize_output::<Balance>(),
    Balance {
      cardinal: 50 * COIN_VALUE,
      runic: None,
      runes: None,
      total: 50 * COIN_VALUE,
    }
  );
}

#[test]
fn unsynced_wallet_fails_with_unindexed_output() {
  let core = mockcore::spawn();
  let ord = TestServer::spawn(&core);

  core.mine_blocks(1);

  create_wallet(&core, &ord);

  assert_eq!(
    CommandBuilder::new("wallet balance")
      .ord(&ord)
      .core(&core)
      .run_and_deserialize_output::<Balance>(),
    Balance {
      cardinal: 50 * COIN_VALUE,
      runic: None,
      runes: None,
      total: 50 * COIN_VALUE,
    }
  );

  let no_sync_ord = TestServer::spawn_with_server_args(&core, &[], &["--no-sync"]);

  // inscribe(&core, &ord);

  CommandBuilder::new("wallet balance")
    .ord(&no_sync_ord)
    .core(&core)
    .expected_exit_code(1)
    .expected_stderr("error: wallet failed to synchronize with `ord server` after 20 attempts\n")
    .run_and_extract_stdout();
}

#[test]
fn runic_utxos_are_deducted_from_cardinal_and_displayed_with_decimal_amount() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let ord = TestServer::spawn_with_server_args(&core, &["--regtest", "--index-runes"], &[]);

  create_wallet(&core, &ord);

  pretty_assert_eq!(
    CommandBuilder::new("--regtest --index-runes wallet balance")
      .core(&core)
      .ord(&ord)
      .run_and_deserialize_output::<Balance>(),
    Balance {
      cardinal: 0,
      runic: Some(0),
      runes: Some(BTreeMap::new()),
      total: 0,
    }
  );

  core.mine_blocks(1);

  CommandBuilder::new("--chain regtest --index-runes wallet mint --fee-rate 1")
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<ord::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  pretty_assert_eq!(
    CommandBuilder::new("--regtest --index-runes wallet balance")
      .core(&core)
      .ord(&ord)
      .run_and_deserialize_output::<Balance>(),
    Balance {
      cardinal: 50 * COIN_VALUE * 2 - 10_000 - 330,
      runic: Some(10_000),
      runes: Some(
        vec![(
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
        .collect()
      ),
      total: 50 * COIN_VALUE * 2 - 330,
    }
  );
}
