use {
  super::*,
  bitcoin::BlockHash,
};

#[test]
fn json_request_fails_when_disabled() {
  let core = mockcore::spawn();

  let response = TestServer::spawn_with_server_args(&core, &[], &["--disable-json-api"])
    .json_request("/sat/2099999997689999");

  assert_eq!(response.status(), StatusCode::NOT_ACCEPTABLE);
}

#[test]
fn get_block() {
  let core = mockcore::spawn();

  core.mine_blocks(1);

  let response = TestServer::spawn_with_server_args(&core, &[], &[]).json_request("/block/0");

  assert_eq!(response.status(), StatusCode::OK);

  let block_json: api::Block = serde_json::from_str(&response.text().unwrap()).unwrap();

  assert_eq!(
    block_json,
    api::Block {
      hash: "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f"
        .parse::<BlockHash>()
        .unwrap(),
      target: "00000000ffff0000000000000000000000000000000000000000000000000000"
        .parse::<BlockHash>()
        .unwrap(),
      best_height: 1,
      height: 0,
      inscriptions: Vec::new(),
      runes: Vec::new(),
      transactions: block_json.transactions.clone(),
    }
  );
}

#[test]
fn get_blocks() {
  let core = mockcore::spawn();
  let ord = TestServer::spawn(&core);

  let blocks: Vec<BlockHash> = core
    .mine_blocks(101)
    .iter()
    .rev()
    .take(100)
    .map(|block| block.block_hash())
    .collect();

  ord.sync_server();

  let response = ord.json_request("/blocks");

  assert_eq!(response.status(), StatusCode::OK);

  let blocks_json: api::Blocks = serde_json::from_str(&response.text().unwrap()).unwrap();

  pretty_assert_eq!(
    blocks_json,
    api::Blocks {
      last: 101,
      blocks: blocks.clone(),
      featured_blocks: blocks
        .into_iter()
        .take(5)
        .map(|block_hash| (block_hash, Vec::new()))
        .collect(),
    }
  );
}

#[test]
fn get_transaction() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn(&core);

  let transaction = core.mine_blocks(1)[0].txdata[0].clone();

  let txid = transaction.txid();

  let response = ord.json_request(format!("/tx/{txid}"));

  assert_eq!(response.status(), StatusCode::OK);

  assert_eq!(
    serde_json::from_str::<api::Transaction>(&response.text().unwrap()).unwrap(),
    api::Transaction {
      chain: Chain::Mainnet,
      etching: None,
      inscription_count: 0,
      transaction,
      txid,
    }
  );
}

#[test]
fn get_status() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let ord =
    TestServer::spawn_with_server_args(&core, &["--regtest", "--index-sats", "--index-runes"], &[]);

  create_wallet(&core, &ord);
  core.mine_blocks(1);

  let response = ord.json_request("/status");

  assert_eq!(response.status(), StatusCode::OK);

  let mut status_json: api::Status = serde_json::from_str(&response.text().unwrap()).unwrap();

  let dummy_started = "2012-12-12 12:12:12+00:00"
    .parse::<DateTime<Utc>>()
    .unwrap();

  let dummy_duration = Duration::from_secs(1);

  status_json.initial_sync_time = dummy_duration;
  status_json.started = dummy_started;
  status_json.uptime = dummy_duration;

  pretty_assert_eq!(
    status_json,
    api::Status {
      address_index: false,
      blessed_inscriptions: 0,
      chain: Chain::Regtest,
      content_type_counts: vec![],
      cursed_inscriptions: 0,
      height: Some(1),
      initial_sync_time: dummy_duration,
      inscriptions: 0,
      lost_sats: 0,
      rune_index: true,
      runes: 2,
      sat_index: true,
      started: dummy_started,
      transaction_index: false,
      unrecoverably_reorged: false,
      uptime: dummy_duration,
      last_mint_outpoint: OutPoint::null(),
      last_conversion_outpoint: OutPoint::null(),
    }
  );
}

#[test]
fn get_runes() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let ord = TestServer::spawn_with_server_args(&core, &["--index-runes", "--regtest"], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let response = ord.json_request(format!("/rune/{}", 0));
  assert_eq!(response.status(), StatusCode::OK);

  let rune_json: api::Rune = serde_json::from_str(&response.text().unwrap()).unwrap();

  pretty_assert_eq!(
    rune_json,
    api::Rune {
      entry: RuneEntry {
        spaced_rune: SpacedRune {
          rune: Rune(TIGHTEN),
          spacers: 0
        },
        ..default()
      },
      id: ID0,
      mintable: true,
      parent: None,
    }
  );

  let response = ord.json_request("/runes");

  assert_eq!(response.status(), StatusCode::OK);

  let runes_json: api::Runes = serde_json::from_str(&response.text().unwrap()).unwrap();

  pretty_assert_eq!(
    runes_json,
    api::Runes {
      entries: vec![
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0
            },
            ..default()
          }
        ),
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0
            },
            ..default()
          }
        ),
      ],
      more: false,
      next: None,
      prev: None,
    }
  );
}

#[test]
fn get_runes_balances() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let ord = TestServer::spawn_with_server_args(&core, &["--index-runes", "--regtest"], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let txid = core.broadcast_tx(TransactionTemplate {
    inputs: &[(1, 0, 0, Witness::new())],
    outputs: 2,
    mint: true,
    op_return: Some(Runestone { ..default() }.encipher()),
    ..default()
  });

  core.mine_blocks(1);

  let rune_balances: BTreeMap<Rune, BTreeMap<OutPoint, u128>> = vec![(
    Rune(TIGHTEN),
    vec![(OutPoint { txid, vout: 1 }, 50 * 100000000)]
      .into_iter()
      .collect(),
  )]
  .into_iter()
  .collect();

  let response = ord.json_request("/runes/balances");
  assert_eq!(response.status(), StatusCode::OK);

  let runes_balance_json: BTreeMap<Rune, BTreeMap<OutPoint, u128>> =
    serde_json::from_str(&response.text().unwrap()).unwrap();

  pretty_assert_eq!(runes_balance_json, rune_balances);
}
