use {super::*, base64::Engine, bitcoin::psbt::Psbt, bitcoin::Amount};

#[test]
fn send_on_mainnnet_works_with_wallet_named_foo() {
  let core = mockcore::spawn();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  core.mine_blocks(1);

  CommandBuilder::new("wallet --name foo create")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<Create>();

  CommandBuilder::new(
    "wallet --name foo send --fee-rate 1 bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4 1btc",
  )
  .core(&core)
  .bitomc(&bitomc)
  .run_and_deserialize_output::<Send>();
}

#[test]
fn send_addresses_must_be_valid_for_network() {
  let core = mockcore::builder().build();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &bitomc);

  core.mine_blocks_with_subsidy(1, 1_000);

  CommandBuilder::new(
    "wallet send --fee-rate 1 tb1q6en7qjxgw4ev8xwx94pzdry6a6ky7wlfeqzunz 1btc"
  )
  .core(&core)
    .bitomc(&bitomc)
  .expected_stderr(
    "error: address tb1q6en7qjxgw4ev8xwx94pzdry6a6ky7wlfeqzunz belongs to network testnet which is different from required bitcoin\n",
  )
  .expected_exit_code(1)
  .run_and_extract_stdout();
}

#[test]
fn send_on_mainnnet_works_with_wallet_named_ord() {
  let core = mockcore::builder().build();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &bitomc);

  core.mine_blocks_with_subsidy(1, 1_000_000);

  let output = CommandBuilder::new(
    "wallet send --fee-rate 1 bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4 1000sat",
  )
  .core(&core)
  .bitomc(&bitomc)
  .run_and_deserialize_output::<Send>();

  assert_eq!(core.mempool()[0].txid(), output.txid);
}

#[test]
fn send_btc_fails_if_lock_unspent_fails() {
  let core = mockcore::builder().fail_lock_unspent(true).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &bitomc);

  core.mine_blocks(1);

  CommandBuilder::new("wallet send --fee-rate 1 bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4 1btc")
    .core(&core)
    .bitomc(&bitomc)
    .expected_stderr("error: failed to lock UTXOs\n")
    .expected_exit_code(1)
    .run_and_extract_stdout();
}

#[test]
fn wallet_send_with_fee_rate() {
  let core = mockcore::spawn();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &bitomc);

  core.mine_blocks(1);

  CommandBuilder::new("wallet send bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4 1btc --fee-rate 2.0")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<Send>();

  let tx = &core.mempool()[0];
  let mut fee = 0;
  for input in &tx.input {
    fee += core
      .get_utxo_amount(&input.previous_output)
      .unwrap()
      .to_sat();
  }
  for output in &tx.output {
    fee -= output.value;
  }

  let fee_rate = fee as f64 / tx.vsize() as f64;

  pretty_assert_eq!(fee_rate, 2.0);
}

#[test]
fn user_must_provide_fee_rate_to_send() {
  let core = mockcore::spawn();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &bitomc);

  core.mine_blocks(1);

  CommandBuilder::new("wallet send bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4 1btc")
    .core(&core)
    .bitomc(&bitomc)
    .expected_exit_code(2)
    .stderr_regex(
      ".*error: the following required arguments were not provided:
.*--fee-rate <FEE_RATE>.*",
    )
    .run_and_extract_stdout();
}

#[test]
fn send_btc_does_not_send_locked_utxos() {
  let core = mockcore::spawn();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &bitomc);

  let coinbase_tx = &core.mine_blocks(1)[0].txdata[0];
  let outpoint = OutPoint::new(coinbase_tx.txid(), 0);

  core.lock(outpoint);

  CommandBuilder::new("wallet send --fee-rate 1 bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4 1btc")
    .core(&core)
    .bitomc(&bitomc)
    .expected_exit_code(1)
    .stderr_regex("error:.*")
    .run_and_extract_stdout();
}

#[test]
fn send_dry_run() {
  let core = mockcore::spawn();

  let bitomc = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &bitomc);

  core.mine_blocks(1);

  let output = CommandBuilder::new(
    "wallet send --fee-rate 1 bc1qcqgs2pps4u4yedfyl5pysdjjncs8et5utseepv --dry-run 100sats",
  )
  .core(&core)
  .bitomc(&bitomc)
  .run_and_deserialize_output::<Send>();

  assert!(core.mempool().is_empty());
  assert_eq!(
    Psbt::deserialize(
      &base64::engine::general_purpose::STANDARD
        .decode(output.psbt)
        .unwrap()
    )
    .unwrap()
    .fee()
    .unwrap()
    .to_sat(),
    output.fee
  );
  assert_eq!(output.outgoing, Outgoing::Amount(Amount::from_sat(100)));
}

#[test]
fn sending_rune_that_has_not_been_etched_is_an_error() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  create_wallet(&core, &bitomc);

  let coinbase_tx = &core.mine_blocks(1)[0].txdata[0];
  let outpoint = OutPoint::new(coinbase_tx.txid(), 0);

  core.lock(outpoint);

  CommandBuilder::new(
    "--chain regtest wallet send --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 1:FOO",
  )
  .core(&core)
  .bitomc(&bitomc)
  .expected_exit_code(1)
  .expected_stderr("error: rune `FOO` has not been etched\n")
  .run_and_extract_stdout();
}

#[test]
fn sending_rune_with_excessive_precision_is_an_error() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  CommandBuilder::new(format!(
    "--chain regtest wallet send --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 1.000000001:{}",
    Rune(TIGHTEN)
  ))
  .core(&core)
    .bitomc(&bitomc)
  .expected_exit_code(1)
  .expected_stderr("error: excessive precision\n")
  .run_and_extract_stdout();
}

#[test]
fn sending_rune_with_insufficient_balance_is_an_error() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  CommandBuilder::new(format!(
    "--chain regtest wallet send --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 1000:{}",
    Rune(TIGHTEN)
  ))
  .core(&core)
  .bitomc(&bitomc)
  .expected_exit_code(1)
  .expected_stderr("error: insufficient `TIGHTEN` balance, only 50 in wallet\n")
  .run_and_extract_stdout();
}

#[test]
fn sending_rune_works() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  let output = CommandBuilder::new(format!(
    "--chain regtest wallet send --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 5:{}",
    Rune(TIGHTEN)
  ))
  .core(&core)
  .bitomc(&bitomc)
  .run_and_deserialize_output::<Send>();

  core.mine_blocks(1);

  let balances = CommandBuilder::new("--regtest balances")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::balances::Output>();

  pretty_assert_eq!(
    balances,
    bitomc::subcommand::balances::Output {
      runes: vec![(
        SpacedRune::new(Rune(TIGHTEN), 0),
        vec![
          (
            OutPoint {
              txid: output.txid,
              vout: 1
            },
            Pile {
              amount: 45 * RUNE_COIN_VALUE,
              divisibility: 8,
              symbol: None
            },
          ),
          (
            OutPoint {
              txid: output.txid,
              vout: 2
            },
            Pile {
              amount: 5 * RUNE_COIN_VALUE,
              divisibility: 8,
              symbol: None
            },
          ),
        ]
        .into_iter()
        .collect()
      ),]
      .into_iter()
      .collect(),
    }
  );
}

#[test]
fn sending_rune_with_change_works() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  let output = CommandBuilder::new(format!(
    "--chain regtest wallet send --postage 1234sat --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 5:{}",
    Rune(TIGHTEN)
  ))
  .core(&core)
  .bitomc(&bitomc)
  .run_and_deserialize_output::<Send>();

  core.mine_blocks(1);

  let tx = core.tx_by_id(output.txid);

  assert_eq!(tx.output[1].value, 1234);
  assert_eq!(tx.output[2].value, 1234);

  let balances = CommandBuilder::new("--regtest balances")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::balances::Output>();

  pretty_assert_eq!(
    balances,
    bitomc::subcommand::balances::Output {
      runes: vec![(
        SpacedRune::new(Rune(TIGHTEN), 0),
        vec![
          (
            OutPoint {
              txid: output.txid,
              vout: 1
            },
            Pile {
              amount: 45 * RUNE_COIN_VALUE,
              divisibility: 8,
              symbol: None
            },
          ),
          (
            OutPoint {
              txid: output.txid,
              vout: 2
            },
            Pile {
              amount: 5 * RUNE_COIN_VALUE,
              divisibility: 8,
              symbol: None
            },
          )
        ]
        .into_iter()
        .collect()
      )]
      .into_iter()
      .collect(),
    }
  );
}

#[test]
fn sending_spaced_rune_works_with_no_change() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  let output = CommandBuilder::new(
    "--chain regtest wallet send --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 50:TIGHTEN",
  )
  .core(&core)
  .bitomc(&bitomc)
  .run_and_deserialize_output::<Send>();

  core.mine_blocks(1);

  let tx = core.tx_by_id(output.txid);

  assert_eq!(tx.output.len(), 1);

  let balances = CommandBuilder::new("--regtest balances")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::balances::Output>();

  assert_eq!(
    balances,
    bitomc::subcommand::balances::Output {
      runes: vec![(
        SpacedRune::new(Rune(TIGHTEN), 0),
        vec![(
          OutPoint {
            txid: output.txid,
            vout: 0
          },
          Pile {
            amount: 50 * RUNE_COIN_VALUE,
            divisibility: 8,
            symbol: None
          },
        )]
        .into_iter()
        .collect()
      ),]
      .into_iter()
      .collect(),
    }
  );
}

#[test]
fn sending_rune_with_divisibility_works() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  let output = CommandBuilder::new(format!(
    "--chain regtest wallet send --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 5.5:{}",
    Rune(TIGHTEN)
  ))
  .core(&core)
  .bitomc(&bitomc)
  .run_and_deserialize_output::<Send>();

  core.mine_blocks(1);

  let balances = CommandBuilder::new("--regtest balances")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::balances::Output>();

  pretty_assert_eq!(
    balances,
    bitomc::subcommand::balances::Output {
      runes: vec![(
        SpacedRune::new(Rune(TIGHTEN), 0),
        vec![
          (
            OutPoint {
              txid: output.txid,
              vout: 1
            },
            Pile {
              amount: 445 * RUNE_COIN_VALUE / 10,
              divisibility: 8,
              symbol: None
            },
          ),
          (
            OutPoint {
              txid: output.txid,
              vout: 2
            },
            Pile {
              amount: 55 * RUNE_COIN_VALUE / 10,
              divisibility: 8,
              symbol: None
            },
          )
        ]
        .into_iter()
        .collect()
      ),]
      .into_iter()
      .collect(),
    }
  );
}

#[test]
fn sending_rune_leaves_unspent_runes_in_wallet() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  let output = CommandBuilder::new(format!(
    "--chain regtest wallet send --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 5:{}",
    Rune(TIGHTEN)
  ))
  .core(&core)
  .bitomc(&bitomc)
  .run_and_deserialize_output::<Send>();

  core.mine_blocks(1);

  let balances = CommandBuilder::new("--regtest balances")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::balances::Output>();

  assert_eq!(
    balances,
    bitomc::subcommand::balances::Output {
      runes: vec![(
        SpacedRune::new(Rune(TIGHTEN), 0),
        vec![
          (
            OutPoint {
              txid: output.txid,
              vout: 1
            },
            Pile {
              amount: 45 * RUNE_COIN_VALUE,
              divisibility: 8,
              symbol: None
            },
          ),
          (
            OutPoint {
              txid: output.txid,
              vout: 2
            },
            Pile {
              amount: 5 * RUNE_COIN_VALUE,
              divisibility: 8,
              symbol: None
            },
          )
        ]
        .into_iter()
        .collect()
      ),]
      .into_iter()
      .collect(),
    }
  );

  let tx = core.tx_by_id(output.txid);

  let address = Address::from_script(&tx.output[1].script_pubkey, Network::Regtest).unwrap();

  assert!(core.state().change_addresses.contains(&address));
}

#[test]
fn sending_rune_creates_transaction_with_expected_runestone() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  let output = CommandBuilder::new(format!(
    "
      --chain regtest
      wallet
      send
      --fee-rate 1
      bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 5:{}
    ",
    Rune(TIGHTEN),
  ))
  .core(&core)
  .bitomc(&bitomc)
  .run_and_deserialize_output::<Send>();

  core.mine_blocks(1);

  let balances = CommandBuilder::new("--regtest balances")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::balances::Output>();

  assert_eq!(
    balances,
    bitomc::subcommand::balances::Output {
      runes: vec![(
        SpacedRune::new(Rune(TIGHTEN), 0),
        vec![
          (
            OutPoint {
              txid: output.txid,
              vout: 1
            },
            Pile {
              amount: 45 * RUNE_COIN_VALUE,
              divisibility: 8,
              symbol: None
            },
          ),
          (
            OutPoint {
              txid: output.txid,
              vout: 2
            },
            Pile {
              amount: 5 * RUNE_COIN_VALUE,
              divisibility: 8,
              symbol: None
            },
          )
        ]
        .into_iter()
        .collect()
      ),]
      .into_iter()
      .collect(),
    }
  );

  let tx = core.tx_by_id(output.txid);

  pretty_assert_eq!(
    Runestone::decipher(&tx).unwrap(),
    Artifact::Runestone(Runestone {
      pointer: None,
      edicts: vec![Edict {
        id: ID0,
        amount: 5 * RUNE_COIN_VALUE,
        output: 2
      }],
    }),
  );
}

#[test]
fn error_messages_use_spaced_runes() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::mint::Output>();

  core.mine_blocks(1);

  CommandBuilder::new(
    "--chain regtest wallet send --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 1001:TIGHTEN",
  )
  .core(&core)
    .bitomc(&bitomc)
  .expected_exit_code(1)
  .expected_stderr("error: insufficient `TIGHTEN` balance, only 50 in wallet\n")
  .run_and_extract_stdout();

  CommandBuilder::new(
    "--chain regtest wallet send --fee-rate 1 bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw 1:F•OO",
  )
  .core(&core)
  .bitomc(&bitomc)
  .expected_exit_code(1)
  .expected_stderr("error: rune `FOO` has not been etched\n")
  .run_and_extract_stdout();
}
