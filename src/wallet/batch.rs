use super::*;

pub use {entry::Entry, etching::Etching, file::File, mode::Mode, range::Range, terms::Terms};

pub mod entry;
mod etching;
pub mod file;
pub mod mode;
mod range;
mod terms;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Output {
  pub commit: Txid,
  pub commit_psbt: Option<String>,
  pub inscriptions: Vec<InscriptionInfo>,
  pub parent: Option<InscriptionId>,
  pub reveal: Txid,
  pub reveal_broadcast: bool,
  pub reveal_psbt: Option<String>,
  pub rune: Option<RuneInfo>,
  pub total_fees: u64,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct InscriptionInfo {
  pub destination: Address<NetworkUnchecked>,
  pub id: InscriptionId,
  pub location: SatPoint,
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RuneInfo {
  pub destination: Option<Address<NetworkUnchecked>>,
  pub location: Option<OutPoint>,
  pub rune: SpacedRune,
}

#[derive(Clone, Debug)]
pub struct ParentInfo {
  pub destination: Address,
  pub id: InscriptionId,
  pub location: SatPoint,
  pub tx_out: TxOut,
}
