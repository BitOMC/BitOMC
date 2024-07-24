use {super::*, bitomc::subcommand::wallet::mint};

#[test]
fn minting_rune_with_destination() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  let destination: Address<NetworkUnchecked> = "bcrt1qs758ursh4q9z627kt3pp5yysm78ddny6txaqgw"
    .parse()
    .unwrap();

  let output = CommandBuilder::new(format!(
    "--chain regtest wallet mint --fee-rate 1 --destination {}",
    destination.clone().assume_checked()
  ))
  .core(&core)
  .bitomc(&bitomc)
  .run_and_deserialize_output::<mint::Output>();

  pretty_assert_eq!(
    output.pile0,
    Pile {
      amount: 50 * RUNE_COIN_VALUE,
      divisibility: 8,
      symbol: None,
    }
  );

  pretty_assert_eq!(
    output.pile1,
    Pile {
      amount: 0,
      divisibility: 8,
      symbol: None,
    }
  );

  pretty_assert_eq!(output.connected, false);

  assert_eq!(
    core.mempool()[0].output[1].script_pubkey,
    destination.payload.script_pubkey()
  );

  core.mine_blocks(1);

  let balances = CommandBuilder::new("--regtest balances")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::balances::Output>();

  pretty_assert_eq!(
    balances,
    bitomc::subcommand::balances::Output {
      runes: vec![(
        output.rune0,
        vec![(
          OutPoint {
            txid: output.mint,
            vout: 1
          },
          output.pile0,
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
fn minting_rune_with_postage() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1 --postage 2222sat")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<mint::Output>();

  core.mine_blocks(1);

  let balance = CommandBuilder::new("--chain regtest wallet balance")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::wallet::balance::Output>();

  assert_eq!(balance.runic, 2222);
}

#[test]
fn minting_rune_with_postage_dust() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  CommandBuilder::new("--chain regtest wallet mint --fee-rate 1 --postage 300sat")
    .core(&core)
    .bitomc(&bitomc)
    .expected_exit_code(1)
    .expected_stderr("error: postage below dust limit of 330sat\n")
    .run_and_extract_stdout();
}

#[test]
fn minting_is_allowed_on_first_mint() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  let output = CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<mint::Output>();

  core.mine_blocks(1);

  let balances = CommandBuilder::new("--regtest balances")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::balances::Output>();

  pretty_assert_eq!(
    output.pile0,
    Pile {
      amount: 50 * RUNE_COIN_VALUE,
      divisibility: 8,
      symbol: None,
    }
  );

  pretty_assert_eq!(
    output.pile1,
    Pile {
      amount: 0,
      divisibility: 8,
      symbol: None,
    }
  );

  pretty_assert_eq!(output.connected, false);

  pretty_assert_eq!(
    balances,
    bitomc::subcommand::balances::Output {
      runes: vec![(
        output.rune0,
        vec![(
          OutPoint {
            txid: output.mint,
            vout: 1
          },
          output.pile0,
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
fn minting_is_allowed_using_output_of_first_mint_as_input() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let bitomc = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  core.mine_blocks(1);

  create_wallet(&core, &bitomc);

  let output0 = CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<mint::Output>();

  core.mine_blocks(1);

  pretty_assert_eq!(output0.connected, false);

  let output1 = CommandBuilder::new("--chain regtest wallet mint --fee-rate 1")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<mint::Output>();

  core.mine_blocks(1);

  let balances = CommandBuilder::new("--regtest balances")
    .core(&core)
    .bitomc(&bitomc)
    .run_and_deserialize_output::<bitomc::subcommand::balances::Output>();

  pretty_assert_eq!(
    output1.pile0,
    Pile {
      amount: 50 * RUNE_COIN_VALUE,
      divisibility: 8,
      symbol: None,
    }
  );

  pretty_assert_eq!(
    output1.pile1,
    Pile {
      amount: 0,
      divisibility: 8,
      symbol: None,
    }
  );

  pretty_assert_eq!(output1.connected, true);

  pretty_assert_eq!(
    balances,
    bitomc::subcommand::balances::Output {
      runes: vec![(
        output0.rune0,
        vec![
          (
            OutPoint {
              txid: output0.mint,
              vout: 1
            },
            output0.pile0,
          ),
          (
            OutPoint {
              txid: output1.mint,
              vout: 1
            },
            output1.pile0,
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
