use super::*;

#[derive(Serialize, Deserialize, Default, PartialEq, Debug, Clone)]
#[serde(deny_unknown_fields)]
pub struct Entry {
  pub delegate: Option<InscriptionId>,
  pub destination: Option<Address<NetworkUnchecked>>,
  pub file: Option<PathBuf>,
  pub metadata: Option<serde_yaml::Value>,
  pub metaprotocol: Option<String>,
  pub satpoint: Option<SatPoint>,
}
