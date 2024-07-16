use super::*;

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone, Default)]
#[serde(deny_unknown_fields)]
pub struct File {
  pub inscriptions: Vec<Entry>,
  pub mode: Mode,
  pub parent: Option<InscriptionId>,
  pub postage: Option<u64>,
  #[serde(default)]
  pub reinscribe: bool,
  pub etching: Option<batch::Etching>,
  pub sat: Option<Sat>,
  pub satpoint: Option<SatPoint>,
}
