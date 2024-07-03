use {super::*, ord::subcommand::wallet::send, pretty_assertions::assert_eq};

fn receive(core: &mockcore::Handle, ord: &TestServer) -> Address {
  let address = CommandBuilder::new("wallet receive")
    .core(core)
    .ord(ord)
    .run_and_deserialize_output::<ord::subcommand::wallet::receive::Output>()
    .addresses
    .into_iter()
    .next()
    .unwrap();

  address.require_network(core.state().network).unwrap()
}

#[test]
fn batch_inscribe_fails_if_batchfile_has_no_inscriptions() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  CommandBuilder::new("wallet batch --fee-rate 2.1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("batch.yaml", "mode: shared-output\ninscriptions: []\n")
    .core(&core)
    .ord(&ord)
    .stderr_regex(".*batchfile must contain at least one inscription.*")
    .expected_exit_code(1)
    .run_and_extract_stdout();
}

#[test]
fn batch_inscribe_can_create_one_inscription() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let output = CommandBuilder::new("wallet batch --fee-rate 2.1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write(
      "batch.yaml",
      "mode: shared-output\ninscriptions:\n- file: inscription.txt\n  metadata: 123\n  metaprotocol: foo",
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  assert_eq!(core.descriptors().len(), 3);

  let request = ord.request(format!("/content/{}", output.inscriptions[0].id));

  assert_eq!(request.status(), 200);
  assert_eq!(
    request.headers().get("content-type").unwrap(),
    "text/plain;charset=utf-8"
  );
  assert_eq!(request.text().unwrap(), "Hello World");

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[0].id),
    r".*<dt>metadata</dt>\s*<dd>\n    123\n  </dd>.*<dt>metaprotocol</dt>\s*<dd>foo</dd>.*",
  );
}

#[test]
fn batch_inscribe_with_multiple_inscriptions() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let output = CommandBuilder::new("wallet batch --batch batch.yaml --fee-rate 55")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      "mode: shared-output\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n"
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  assert_eq!(core.descriptors().len(), 3);

  let request = ord.request(format!("/content/{}", output.inscriptions[0].id));
  assert_eq!(request.status(), 200);
  assert_eq!(
    request.headers().get("content-type").unwrap(),
    "text/plain;charset=utf-8"
  );
  assert_eq!(request.text().unwrap(), "Hello World");

  let request = ord.request(format!("/content/{}", output.inscriptions[1].id));
  assert_eq!(request.status(), 200);
  assert_eq!(request.headers().get("content-type").unwrap(), "image/png");

  let request = ord.request(format!("/content/{}", output.inscriptions[2].id));
  assert_eq!(request.status(), 200);
  assert_eq!(request.headers().get("content-type").unwrap(), "audio/wav");
}

#[test]
fn batch_inscribe_with_multiple_inscriptions_with_parent() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let parent_output = CommandBuilder::new("wallet inscribe --fee-rate 5.0 --file parent.png")
    .write("parent.png", [1; 520])
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  assert_eq!(core.descriptors().len(), 3);

  let parent_id = parent_output.inscriptions[0].id;

  let output = CommandBuilder::new("wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      format!("parent: {parent_id}\nmode: shared-output\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n")
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[0].id),
    r".*<dt>parents</dt>\s*<dd>.*</dd>.*",
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[1].id),
    r".*<dt>parents</dt>\s*<dd>.*</dd>.*",
  );

  let request = ord.request(format!("/content/{}", output.inscriptions[2].id));
  assert_eq!(request.status(), 200);
  assert_eq!(request.headers().get("content-type").unwrap(), "audio/wav");
}

#[test]
fn batch_inscribe_respects_dry_run_flag() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let output = CommandBuilder::new("wallet batch --fee-rate 2.1 --batch batch.yaml --dry-run")
    .write("inscription.txt", "Hello World")
    .write(
      "batch.yaml",
      "mode: shared-output\ninscriptions:\n- file: inscription.txt\n",
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  assert!(core.mempool().is_empty());

  let request = ord.request(format!("/content/{}", output.inscriptions[0].id));

  assert_eq!(request.status(), 404);
}

#[test]
fn batch_in_same_output_but_different_satpoints() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let output = CommandBuilder::new("wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      "mode: shared-output\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n"
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  let outpoint = output.inscriptions[0].location.outpoint;
  for (i, inscription) in output.inscriptions.iter().enumerate() {
    assert_eq!(
      inscription.location,
      SatPoint {
        outpoint,
        offset: u64::try_from(i).unwrap() * 10_000,
      }
    );
  }

  core.mine_blocks(1);

  let outpoint = output.inscriptions[0].location.outpoint;

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[0].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[1].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:10000</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[2].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:20000</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/output/{}", output.inscriptions[0].location.outpoint),
    format!(r".*<a href=/inscription/{}>.*</a>.*<a href=/inscription/{}>.*</a>.*<a href=/inscription/{}>.*</a>.*", output.inscriptions[0].id, output.inscriptions[1].id, output.inscriptions[2].id),
  );
}

#[test]
fn batch_in_same_output_with_non_default_postage() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let output = CommandBuilder::new("wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      "mode: shared-output\npostage: 777\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n"
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  let outpoint = output.inscriptions[0].location.outpoint;

  for (i, inscription) in output.inscriptions.iter().enumerate() {
    assert_eq!(
      inscription.location,
      SatPoint {
        outpoint,
        offset: u64::try_from(i).unwrap() * 777,
      }
    );
  }

  core.mine_blocks(1);

  let outpoint = output.inscriptions[0].location.outpoint;

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[0].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[1].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:777</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[2].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:1554</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/output/{}", output.inscriptions[0].location.outpoint),
    format!(r".*<a href=/inscription/{}>.*</a>.*<a href=/inscription/{}>.*</a>.*<a href=/inscription/{}>.*</a>.*", output.inscriptions[0].id, output.inscriptions[1].id, output.inscriptions[2].id),
  );
}

#[test]
fn batch_in_separate_outputs_with_parent() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let parent_output = CommandBuilder::new("wallet inscribe --fee-rate 5.0 --file parent.png")
    .write("parent.png", [1; 520])
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  assert_eq!(core.descriptors().len(), 3);

  let parent_id = parent_output.inscriptions[0].id;

  let output = CommandBuilder::new("wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      format!("parent: {parent_id}\nmode: separate-outputs\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n")
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  for inscription in &output.inscriptions {
    assert_eq!(inscription.location.offset, 0);
  }
  let mut outpoints = output
    .inscriptions
    .iter()
    .map(|inscription| inscription.location.outpoint)
    .collect::<Vec<OutPoint>>();
  outpoints.sort();
  outpoints.dedup();
  assert_eq!(outpoints.len(), output.inscriptions.len());

  core.mine_blocks(1);

  let output_1 = output.inscriptions[0].location.outpoint;
  let output_2 = output.inscriptions[1].location.outpoint;
  let output_3 = output.inscriptions[2].location.outpoint;

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[0].id),
    format!(
      r".*<dt>parents</dt>\s*<dd>.*{parent_id}.*</dd>.*<dt>value</dt>.*<dd>10000</dd>.*.*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      output_1
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[1].id),
    format!(
      r".*<dt>parents</dt>\s*<dd>.*{parent_id}.*</dd>.*<dt>value</dt>.*<dd>10000</dd>.*.*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      output_2
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[2].id),
    format!(
      r".*<dt>parents</dt>\s*<dd>.*{parent_id}.*</dd>.*<dt>value</dt>.*<dd>10000</dd>.*.*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      output_3
    ),
  );
}

#[test]
fn batch_in_separate_outputs_with_parent_and_non_default_postage() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let parent_output = CommandBuilder::new("wallet inscribe --fee-rate 5.0 --file parent.png")
    .write("parent.png", [1; 520])
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  assert_eq!(core.descriptors().len(), 3);

  let parent_id = parent_output.inscriptions[0].id;

  let output = CommandBuilder::new("wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      format!("parent: {parent_id}\nmode: separate-outputs\npostage: 777\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n")
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  for inscription in &output.inscriptions {
    assert_eq!(inscription.location.offset, 0);
  }

  let mut outpoints = output
    .inscriptions
    .iter()
    .map(|inscription| inscription.location.outpoint)
    .collect::<Vec<OutPoint>>();
  outpoints.sort();
  outpoints.dedup();
  assert_eq!(outpoints.len(), output.inscriptions.len());

  core.mine_blocks(1);

  let output_1 = output.inscriptions[0].location.outpoint;
  let output_2 = output.inscriptions[1].location.outpoint;
  let output_3 = output.inscriptions[2].location.outpoint;

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[0].id),
    format!(
      r".*<dt>parents</dt>\s*<dd>.*{parent_id}.*</dd>.*<dt>value</dt>.*<dd>777</dd>.*.*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      output_1
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[1].id),
    format!(
      r".*<dt>parents</dt>\s*<dd>.*{parent_id}.*</dd>.*<dt>value</dt>.*<dd>777</dd>.*.*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      output_2
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[2].id),
    format!(
      r".*<dt>parents</dt>\s*<dd>.*{parent_id}.*</dd>.*<dt>value</dt>.*<dd>777</dd>.*.*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      output_3
    ),
  );
}

#[test]
fn batch_inscribe_fails_if_invalid_network_destination_address() {
  let core = mockcore::builder().network(Network::Regtest).build();

  let ord = TestServer::spawn_with_server_args(&core, &["--regtest"], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  CommandBuilder::new("--regtest wallet batch --fee-rate 2.1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("batch.yaml", "mode: separate-outputs\ninscriptions:\n- file: inscription.txt\n  destination: bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4")
    .core(&core)
    .ord(&ord)
    .stderr_regex("error: address bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4 belongs to network bitcoin which is different from required regtest\n")
    .expected_exit_code(1)
    .run_and_extract_stdout();
}

#[test]
fn batch_inscribe_fails_with_shared_output_or_same_sat_and_destination_set() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  CommandBuilder::new("wallet batch --fee-rate 2.1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", "")
    .write("batch.yaml", "mode: shared-output\ninscriptions:\n- file: inscription.txt\n  destination: bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4\n- file: tulip.png")
    .core(&core)
    .ord(&ord)
    .expected_exit_code(1)
    .stderr_regex("error: individual inscription destinations cannot be set in `shared-output` or `same-sat` mode\n")
    .run_and_extract_stdout();

  CommandBuilder::new("wallet batch --fee-rate 2.1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", "")
    .write("batch.yaml", "mode: same-sat\nsat: 5000000000\ninscriptions:\n- file: inscription.txt\n  destination: bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4\n- file: tulip.png")
    .core(&core)
    .ord(&ord)
    .expected_exit_code(1)
    .stderr_regex("error: individual inscription destinations cannot be set in `shared-output` or `same-sat` mode\n")
    .run_and_extract_stdout();
}

#[test]
fn batch_inscribe_works_with_some_destinations_set_and_others_not() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let output = CommandBuilder::new("wallet batch --batch batch.yaml --fee-rate 55")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      "\
mode: separate-outputs
inscriptions:
- file: inscription.txt
  destination: bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4
- file: tulip.png
- file: meow.wav
  destination: bc1pxwww0ct9ue7e8tdnlmug5m2tamfn7q06sahstg39ys4c9f3340qqxrdu9k
",
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  assert_eq!(core.descriptors().len(), 3);

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[0].id),
    ".*
  <dt>address</dt>
  <dd class=monospace><a href=/address/bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4>bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4</a></dd>.*",
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[1].id),
    format!(
      ".*
  <dt>address</dt>
  <dd class=monospace><a href=/address/{0}>{0}</a></dd>.*",
      core.state().change_addresses[0],
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[2].id),
    ".*
  <dt>address</dt>
  <dd class=monospace><a href=/address/bc1pxwww0ct9ue7e8tdnlmug5m2tamfn7q06sahstg39ys4c9f3340qqxrdu9k>bc1pxwww0ct9ue7e8tdnlmug5m2tamfn7q06sahstg39ys4c9f3340qqxrdu9k</a></dd>.*",
  );
}

#[test]
fn batch_same_sat() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let output = CommandBuilder::new("wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      "mode: same-sat\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n"
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  assert_eq!(
    output.inscriptions[0].location,
    output.inscriptions[1].location
  );
  assert_eq!(
    output.inscriptions[1].location,
    output.inscriptions[2].location
  );

  core.mine_blocks(1);

  let outpoint = output.inscriptions[0].location.outpoint;

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[0].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[1].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[2].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/output/{}", output.inscriptions[0].location.outpoint),
    format!(r".*<a href=/inscription/{}>.*</a>.*<a href=/inscription/{}>.*</a>.*<a href=/inscription/{}>.*</a>.*", output.inscriptions[0].id, output.inscriptions[1].id, output.inscriptions[2].id),
  );
}

#[test]
fn batch_same_sat_with_parent() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let parent_output = CommandBuilder::new("wallet inscribe --fee-rate 5.0 --file parent.png")
    .write("parent.png", [1; 520])
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  let parent_id = parent_output.inscriptions[0].id;

  let output = CommandBuilder::new("wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      format!("mode: same-sat\nparent: {parent_id}\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n")
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  assert_eq!(
    output.inscriptions[0].location,
    output.inscriptions[1].location
  );
  assert_eq!(
    output.inscriptions[1].location,
    output.inscriptions[2].location
  );

  core.mine_blocks(1);

  let txid = output.inscriptions[0].location.outpoint.txid;

  ord.assert_response_regex(
    format!("/inscription/{}", parent_id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:0:0</dd>.*",
      txid
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[0].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:1:0</dd>.*",
      txid
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[1].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:1:0</dd>.*",
      txid
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[2].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:1:0</dd>.*",
      txid
    ),
  );

  ord.assert_response_regex(
    format!("/output/{}", output.inscriptions[0].location.outpoint),
    format!(r".*<a href=/inscription/{}>.*</a>.*<a href=/inscription/{}>.*</a>.*<a href=/inscription/{}>.*</a>.*", output.inscriptions[0].id, output.inscriptions[1].id, output.inscriptions[2].id),
  );
}

#[test]
fn batch_same_sat_with_satpoint_and_reinscription() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let output = CommandBuilder::new("wallet inscribe --fee-rate 5.0 --file parent.png")
    .write("parent.png", [1; 520])
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  let inscription_id = output.inscriptions[0].id;
  let satpoint = output.inscriptions[0].location;

  CommandBuilder::new("wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      format!("mode: same-sat\nsatpoint: {}\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n", satpoint)
    )
    .core(&core)
    .ord(&ord)
    .expected_exit_code(1)
    .stderr_regex(".*error: sat at .*:0:0 already inscribed.*")
    .run_and_extract_stdout();

  let output = CommandBuilder::new("wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      format!("mode: same-sat\nsatpoint: {}\nreinscribe: true\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n", satpoint)
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  assert_eq!(
    output.inscriptions[0].location,
    output.inscriptions[1].location
  );
  assert_eq!(
    output.inscriptions[1].location,
    output.inscriptions[2].location
  );

  core.mine_blocks(1);

  let outpoint = output.inscriptions[0].location.outpoint;

  ord.assert_response_regex(
    format!("/inscription/{}", inscription_id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[0].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[1].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/inscription/{}", output.inscriptions[2].id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:0</dd>.*",
      outpoint
    ),
  );

  ord.assert_response_regex(
    format!("/output/{}", output.inscriptions[0].location.outpoint),
    format!(r".*<a href=/inscription/{}>.*</a>.*<a href=/inscription/{}>.*</a>.*<a href=/inscription/{}>.*</a>.*<a href=/inscription/{}>.*</a>.*", inscription_id, output.inscriptions[0].id, output.inscriptions[1].id, output.inscriptions[2].id),
  );
}

#[test]
fn batch_inscribe_with_sat_argument_with_parent() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &["--index-sats"], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let parent_output =
    CommandBuilder::new("--index-sats wallet inscribe --fee-rate 5.0 --file parent.png")
      .write("parent.png", [1; 520])
      .core(&core)
      .ord(&ord)
      .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  assert_eq!(core.descriptors().len(), 3);

  let parent_id = parent_output.inscriptions[0].id;

  let output = CommandBuilder::new("--index-sats wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      format!("parent: {parent_id}\nmode: same-sat\nsat: 5000111111\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n")
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  ord.assert_response_regex(
    "/sat/5000111111",
    format!(
      ".*<a href=/inscription/{}>.*<a href=/inscription/{}>.*<a href=/inscription/{}>.*",
      output.inscriptions[0].id, output.inscriptions[1].id, output.inscriptions[2].id
    ),
  );
}

#[test]
fn batch_inscribe_with_sat_arg_fails_if_wrong_mode() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  CommandBuilder::new("wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      "mode: shared-output\nsat: 5000111111\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n"
    )
    .core(&core)
    .ord(&ord)
    .expected_exit_code(1)
    .expected_stderr("error: neither `sat` nor `satpoint` can be set in `same-sat` mode\n")
    .run_and_extract_stdout();
}

#[test]
fn batch_inscribe_with_satpoint() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &["--index-sats"], &[]);

  create_wallet(&core, &ord);

  let txid = core.mine_blocks(1)[0].txdata[0].txid();

  let output = CommandBuilder::new("wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      format!("mode: same-sat\nsatpoint: {txid}:0:55555\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n", )
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  ord.assert_response_regex(
    "/sat/5000055555",
    format!(
      ".*<a href=/inscription/{}>.*<a href=/inscription/{}>.*<a href=/inscription/{}>.*",
      output.inscriptions[0].id, output.inscriptions[1].id, output.inscriptions[2].id
    ),
  );
}

#[test]
fn batch_inscribe_with_fee_rate() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &["--index-sats"], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(2);

  let set_fee_rate = 1.0;

  let output = CommandBuilder::new(format!("--index-sats wallet batch --fee-rate {set_fee_rate} --batch batch.yaml"))
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      "mode: same-sat\nsat: 5000111111\ninscriptions:\n- file: inscription.txt\n- file: tulip.png\n- file: meow.wav\n"
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  let commit_tx = &core.mempool()[0];
  let mut fee = 0;
  for input in &commit_tx.input {
    fee += core
      .get_utxo_amount(&input.previous_output)
      .unwrap()
      .to_sat();
  }
  for output in &commit_tx.output {
    fee -= output.value;
  }
  let fee_rate = fee as f64 / commit_tx.vsize() as f64;
  pretty_assert_eq!(fee_rate, set_fee_rate);

  let reveal_tx = &core.mempool()[1];
  let mut fee = 0;
  for input in &reveal_tx.input {
    fee += &commit_tx.output[input.previous_output.vout as usize].value;
  }
  for output in &reveal_tx.output {
    fee -= output.value;
  }
  let fee_rate = fee as f64 / reveal_tx.vsize() as f64;
  pretty_assert_eq!(fee_rate, set_fee_rate);

  assert_eq!(
    ord::FeeRate::try_from(set_fee_rate)
      .unwrap()
      .fee(commit_tx.vsize() + reveal_tx.vsize())
      .to_sat(),
    output.total_fees
  );
}

#[test]
fn batch_inscribe_with_delegate_inscription() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let (delegate, _) = inscribe(&core, &ord);

  let inscribe = CommandBuilder::new("wallet batch --fee-rate 1.0 --batch batch.yaml")
    .write("inscription.txt", "INSCRIPTION")
    .write(
      "batch.yaml",
      format!(
        "mode: shared-output
inscriptions:
- delegate: {delegate}
"
      ),
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  ord.assert_response_regex(
    format!("/inscription/{}", inscribe.inscriptions[0].id),
    format!(r#".*<dt>delegate</dt>\s*<dd><a href=/inscription/{delegate}>{delegate}</a></dd>.*"#,),
  );

  ord.assert_response(format!("/content/{}", inscribe.inscriptions[0].id), "FOO");
}

#[test]
fn batch_inscribe_with_non_existent_delegate_inscription() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &[], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let delegate = "0000000000000000000000000000000000000000000000000000000000000000i0";

  CommandBuilder::new("wallet batch --fee-rate 1.0 --batch batch.yaml")
    .write("hello.txt", "Hello, world!")
    .write(
      "batch.yaml",
      format!(
        "mode: shared-output
inscriptions:
- delegate: {delegate}
  file: hello.txt
"
      ),
    )
    .core(&core)
    .ord(&ord)
    .expected_stderr(format!("error: delegate {delegate} does not exist\n"))
    .expected_exit_code(1)
    .run_and_extract_stdout();
}

#[test]
fn batch_inscribe_with_satpoints_with_parent() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &["--index-sats"], &[]);

  create_wallet(&core, &ord);

  core.mine_blocks(1);

  let parent_output =
    CommandBuilder::new("--index-sats wallet inscribe --fee-rate 5.0 --file parent.png")
      .write("parent.png", [1; 520])
      .core(&core)
      .ord(&ord)
      .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  let txids = core
    .mine_blocks(3)
    .iter()
    .map(|block| block.txdata[0].txid())
    .collect::<Vec<Txid>>();

  let satpoint_1 = SatPoint {
    outpoint: OutPoint {
      txid: txids[0],
      vout: 0,
    },
    offset: 0,
  };

  let satpoint_2 = SatPoint {
    outpoint: OutPoint {
      txid: txids[1],
      vout: 0,
    },
    offset: 0,
  };

  let satpoint_3 = SatPoint {
    outpoint: OutPoint {
      txid: txids[2],
      vout: 0,
    },
    offset: 0,
  };

  let sat_1 = serde_json::from_str::<api::Output>(
    &ord
      .json_request(format!("/output/{}", satpoint_1.outpoint))
      .text()
      .unwrap(),
  )
  .unwrap()
  .sat_ranges
  .unwrap()[0]
    .0;

  let sat_2 = serde_json::from_str::<api::Output>(
    &ord
      .json_request(format!("/output/{}", satpoint_2.outpoint))
      .text()
      .unwrap(),
  )
  .unwrap()
  .sat_ranges
  .unwrap()[0]
    .0;

  let sat_3 = serde_json::from_str::<api::Output>(
    &ord
      .json_request(format!("/output/{}", satpoint_3.outpoint))
      .text()
      .unwrap(),
  )
  .unwrap()
  .sat_ranges
  .unwrap()[0]
    .0;

  let parent_id = parent_output.inscriptions[0].id;

  let output = CommandBuilder::new("--index-sats wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 555])
    .write("meow.wav", [0; 2048])
    .write(
      "batch.yaml",
      format!(
        r#"
mode: satpoints
parent: {parent_id}
inscriptions:
- file: inscription.txt
  satpoint: {}
- file: tulip.png
  satpoint: {}
- file: meow.wav
  satpoint: {}
"#,
        satpoint_1, satpoint_2, satpoint_3
      ),
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  ord.assert_response_regex(
    format!("/inscription/{}", parent_id),
    format!(
      r".*<dt>location</dt>.*<dd class=monospace>{}:0:0</dd>.*",
      output.reveal
    ),
  );

  for inscription in &output.inscriptions {
    assert_eq!(inscription.location.offset, 0);
  }

  let outpoints = output
    .inscriptions
    .iter()
    .map(|inscription| inscription.location.outpoint)
    .collect::<Vec<OutPoint>>();

  assert_eq!(outpoints.len(), output.inscriptions.len());

  let inscription_1 = &output.inscriptions[0];
  let inscription_2 = &output.inscriptions[1];
  let inscription_3 = &output.inscriptions[2];

  ord.assert_response_regex(
    format!("/inscription/{}", inscription_1.id),
    format!(r".*<dt>parents</dt>\s*<dd>.*{parent_id}.*</dd>.*<dt>value</dt>.*<dd>{}</dd>.*<dt>sat</dt>.*<dd>.*{}.*</dd>.*<dt>location</dt>.*<dd class=monospace>{}</dd>.*",
      50 * COIN_VALUE,
      sat_1,
      inscription_1.location,
    ),
  );

  ord.assert_response_regex(
      format!("/inscription/{}", inscription_2.id),
      format!(r".*<dt>parents</dt>\s*<dd>.*{parent_id}.*</dd>.*<dt>value</dt>.*<dd>{}</dd>.*<dt>sat</dt>.*<dd>.*{}.*</dd>.*<dt>location</dt>.*<dd class=monospace>{}</dd>.*",
         50 * COIN_VALUE,
         sat_2,
         inscription_2.location
      ),
    );

  ord.assert_response_regex(
      format!("/inscription/{}", inscription_3.id),
      format!(r".*<dt>parents</dt>\s*<dd>.*{parent_id}.*</dd>.*<dt>value</dt>.*<dd>{}</dd>.*<dt>sat</dt>.*<dd>.*{}.*</dd>.*<dt>location</dt>.*<dd class=monospace>{}</dd>.*",
        50 * COIN_VALUE,
        sat_3,
        inscription_3.location
      ),
    );
}

#[test]
fn batch_inscribe_with_satpoints_with_different_sizes() {
  let core = mockcore::spawn();

  let ord = TestServer::spawn_with_server_args(&core, &["--index-sats"], &[]);

  create_wallet(&core, &ord);

  let address_1 = receive(&core, &ord);
  let address_2 = receive(&core, &ord);
  let address_3 = receive(&core, &ord);

  core.mine_blocks(3);

  let outpoint_1 = OutPoint {
    txid: CommandBuilder::new(format!(
      "--index-sats wallet send --fee-rate 1 {address_1} 25btc"
    ))
    .core(&core)
    .ord(&ord)
    .stdout_regex(r".*")
    .run_and_deserialize_output::<send::Output>()
    .txid,
    vout: 0,
  };

  core.mine_blocks(1);

  let outpoint_2 = OutPoint {
    txid: CommandBuilder::new(format!(
      "--index-sats wallet send --fee-rate 1 {address_2} 1btc"
    ))
    .core(&core)
    .ord(&ord)
    .stdout_regex(r".*")
    .run_and_deserialize_output::<send::Output>()
    .txid,
    vout: 0,
  };

  core.mine_blocks(1);

  let outpoint_3 = OutPoint {
    txid: CommandBuilder::new(format!(
      "--index-sats wallet send --fee-rate 1 {address_3} 3btc"
    ))
    .core(&core)
    .ord(&ord)
    .stdout_regex(r".*")
    .run_and_deserialize_output::<send::Output>()
    .txid,
    vout: 0,
  };

  core.mine_blocks(1);

  let satpoint_1 = SatPoint {
    outpoint: outpoint_1,
    offset: 0,
  };

  let satpoint_2 = SatPoint {
    outpoint: outpoint_2,
    offset: 0,
  };

  let satpoint_3 = SatPoint {
    outpoint: outpoint_3,
    offset: 0,
  };

  let output_1 = serde_json::from_str::<api::Output>(
    &ord
      .json_request(format!("/output/{}", satpoint_1.outpoint))
      .text()
      .unwrap(),
  )
  .unwrap();
  assert_eq!(output_1.value, 25 * COIN_VALUE);

  let output_2 = serde_json::from_str::<api::Output>(
    &ord
      .json_request(format!("/output/{}", satpoint_2.outpoint))
      .text()
      .unwrap(),
  )
  .unwrap();
  assert_eq!(output_2.value, COIN_VALUE);

  let output_3 = serde_json::from_str::<api::Output>(
    &ord
      .json_request(format!("/output/{}", satpoint_3.outpoint))
      .text()
      .unwrap(),
  )
  .unwrap();
  assert_eq!(output_3.value, 3 * COIN_VALUE);

  let sat_1 = output_1.sat_ranges.unwrap()[0].0;
  let sat_2 = output_2.sat_ranges.unwrap()[0].0;
  let sat_3 = output_3.sat_ranges.unwrap()[0].0;

  let output = CommandBuilder::new("--index-sats wallet batch --fee-rate 1 --batch batch.yaml")
    .write("inscription.txt", "Hello World")
    .write("tulip.png", [0; 5])
    .write("meow.wav", [0; 2])
    .write(
      "batch.yaml",
      format!(
        r#"
mode: satpoints
inscriptions:
- file: inscription.txt
  satpoint: {}
- file: tulip.png
  satpoint: {}
- file: meow.wav
  satpoint: {}
"#,
        satpoint_1, satpoint_2, satpoint_3
      ),
    )
    .core(&core)
    .ord(&ord)
    .run_and_deserialize_output::<Batch>();

  core.mine_blocks(1);

  for inscription in &output.inscriptions {
    assert_eq!(inscription.location.offset, 0);
  }

  let outpoints = output
    .inscriptions
    .iter()
    .map(|inscription| inscription.location.outpoint)
    .collect::<Vec<OutPoint>>();

  assert_eq!(outpoints.len(), output.inscriptions.len());

  let inscription_1 = &output.inscriptions[0];
  let inscription_2 = &output.inscriptions[1];
  let inscription_3 = &output.inscriptions[2];

  ord.assert_response_regex(
     format!("/inscription/{}", inscription_1.id),
     format!(
       r".*<dt>value</dt>.*<dd>{}</dd>.*<dt>sat</dt>.*<dd>.*{}.*</dd>.*<dt>location</dt>.*<dd class=monospace>{}</dd>.*",
       25 * COIN_VALUE,
       sat_1,
       inscription_1.location
     ),
   );

  ord.assert_response_regex(
      format!("/inscription/{}", inscription_2.id),
      format!(
        r".*<dt>value</dt>.*<dd>{}</dd>.*<dt>sat</dt>.*<dd>.*{}.*</dd>.*<dt>location</dt>.*<dd class=monospace>{}</dd>.*",
        COIN_VALUE,
        sat_2,
        inscription_2.location
      ),
    );

  ord.assert_response_regex(
         format!("/inscription/{}", inscription_3.id),
         format!(
           r".*<dt>value</dt>.*<dd>{}</dd>.*<dt>sat</dt>.*<dd>.*{}.*</dd>.*<dt>location</dt>.*<dd class=monospace>{}</dd>.*",
           3 * COIN_VALUE,
           sat_3,
           inscription_3.location
         ),
  );
}
