use super::*;

#[test]
fn dumped_descriptors_match_wallet_descriptors() {
  let core = mockcore::spawn();
  let bitomc = TestServer::spawn(&core);

  create_wallet(&core, &bitomc);

  let output = CommandBuilder::new("wallet dump")
    .core(&core)
    .bitomc(&bitomc)
    .stderr_regex(".*")
    .run_and_deserialize_output::<ListDescriptorsResult>();

  assert!(core
    .descriptors()
    .iter()
    .zip(output.descriptors.iter())
    .all(|(wallet_descriptor, output_descriptor)| *wallet_descriptor == output_descriptor.desc));
}

#[test]
fn dumped_descriptors_restore() {
  let core = mockcore::spawn();
  let bitomc = TestServer::spawn(&core);

  create_wallet(&core, &bitomc);

  let output = CommandBuilder::new("wallet dump")
    .core(&core)
    .bitomc(&bitomc)
    .stderr_regex(".*")
    .run_and_deserialize_output::<ListDescriptorsResult>();

  let core = mockcore::spawn();

  CommandBuilder::new("wallet restore --from descriptor")
    .stdin(serde_json::to_string(&output).unwrap().as_bytes().to_vec())
    .core(&core)
    .bitomc(&bitomc)
    .run_and_extract_stdout();

  assert!(core
    .descriptors()
    .iter()
    .zip(output.descriptors.iter())
    .all(|(wallet_descriptor, output_descriptor)| *wallet_descriptor == output_descriptor.desc));
}

#[test]
fn dump_and_restore_descriptors_with_minify() {
  let core = mockcore::spawn();
  let bitomc = TestServer::spawn(&core);

  create_wallet(&core, &bitomc);

  let output = CommandBuilder::new("--format minify wallet dump")
    .core(&core)
    .bitomc(&bitomc)
    .stderr_regex(".*")
    .run_and_deserialize_output::<ListDescriptorsResult>();

  let core = mockcore::spawn();

  CommandBuilder::new("wallet restore --from descriptor")
    .stdin(serde_json::to_string(&output).unwrap().as_bytes().to_vec())
    .core(&core)
    .bitomc(&bitomc)
    .run_and_extract_stdout();

  assert!(core
    .descriptors()
    .iter()
    .zip(output.descriptors.iter())
    .all(|(wallet_descriptor, output_descriptor)| *wallet_descriptor == output_descriptor.desc));
}
