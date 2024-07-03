use {super::*, ord::subcommand::balances::Output};

const COIN_VALUE: u128 = 100000000;

#[test]
fn flag_is_required() {
  let core = mockcore::builder().network(Network::Regtest).build();

  CommandBuilder::new("--regtest balances")
    .core(&core)
    .expected_exit_code(1)
    .expected_stderr("error: `ord balances` requires index created with `--index-runes` flag\n")
    .run_and_extract_stdout();
}

#[test]
fn no_runes() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let output = CommandBuilder::new("--regtest --index-runes balances")
    .core(&core)
    .run_and_deserialize_output::<Output>();

  assert_eq!(
    output,
    Output {
      runes: BTreeMap::new()
    }
  );
}

#[test]
fn with_runes() {
  let core = mockcore::builder().network(Network::Regtest).build();

  core.mine_blocks(1);

  // Mint 50 TIGHTEN and convert 20 TIGHTEN to 40 EASE
  let txid = core.broadcast_tx(TransactionTemplate {
    inputs: &[(1, 0, 0, Witness::new())],
    mint: true,
    outputs: 2,
    op_return: Some(
      Runestone {
        edicts: vec![
          Edict {
            id: ID0,
            amount: 30 * COIN_VALUE,
            output: 1,
          },
          Edict {
            id: ID1,
            amount: 40 * COIN_VALUE,
            output: 1,
          },
        ],
        pointer: Some(2),
      }
      .encipher(),
    ),
    ..default()
  });

  core.mine_blocks(1);

  let ord = TestServer::spawn_with_server_args(&core, &["--regtest", "--index-runes"], &[]);

  create_wallet(&core, &ord);

  let output = CommandBuilder::new("--regtest --index-runes balances")
    .core(&core)
    .run_and_deserialize_output::<Output>();

  assert_eq!(
    output,
    Output {
      runes: vec![
        (
          SpacedRune::new(Rune(TIGHTEN), 0),
          vec![(
            OutPoint { txid, vout: 1 },
            Pile {
              amount: 30 * COIN_VALUE,
              divisibility: 8,
              symbol: None
            },
          )]
          .into_iter()
          .collect()
        ),
        (
          SpacedRune::new(Rune(EASE), 0),
          vec![(
            OutPoint { txid, vout: 1 },
            Pile {
              amount: 40 * COIN_VALUE,
              divisibility: 8,
              symbol: None
            },
          )]
          .into_iter()
          .collect()
        ),
      ]
      .into_iter()
      .collect(),
    }
  );
}
