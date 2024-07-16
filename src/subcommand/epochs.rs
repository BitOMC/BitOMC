use super::*;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Output {
  pub starting_sats: Vec<Sat>,
}

#[allow(dead_code)]
pub(crate) fn run() -> SubcommandResult {
  let mut starting_sats = Vec::new();
  for sat in Epoch::STARTING_SATS {
    starting_sats.push(sat);
  }

  Ok(Some(Box::new(Output { starting_sats })))
}
