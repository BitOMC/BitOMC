use super::*;

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct EtchingEntry {
  pub commit: Transaction,
  pub reveal: Transaction,
  pub output: batch::Output,
}

pub(super) type EtchingEntryValue = (
  Vec<u8>, // commit
  Vec<u8>, // reveal
  Vec<u8>, // output
);

impl Entry for EtchingEntry {
  type Value = EtchingEntryValue;

  fn load((commit, reveal, output): EtchingEntryValue) -> Self {
    Self {
      commit: consensus::encode::deserialize::<Transaction>(&commit).unwrap(),
      reveal: consensus::encode::deserialize::<Transaction>(&reveal).unwrap(),
      output: serde_json::from_slice(&output).unwrap(),
    }
  }

  fn store(self) -> Self::Value {
    (
      consensus::encode::serialize(&self.commit),
      consensus::encode::serialize(&self.reveal),
      serde_json::to_string(&self.output)
        .unwrap()
        .as_bytes()
        .to_owned(),
    )
  }
}
