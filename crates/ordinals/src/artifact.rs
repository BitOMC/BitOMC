use super::*;

#[derive(Serialize, Eq, PartialEq, Deserialize, Debug)]
pub enum Artifact {
  Cenotaph(Cenotaph),
  Runestone(Runestone),
}
