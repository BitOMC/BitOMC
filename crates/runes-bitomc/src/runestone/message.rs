use super::*;

pub(super) struct Message {
  pub(super) flaw: Option<Flaw>,
  pub(super) edicts: Vec<Edict>,
  pub(super) pointer: Option<u32>,
}

impl Message {
  pub(super) fn from_integers(tx: &Transaction, payload: &[u128]) -> Self {
    let mut edicts = Vec::new();
    let mut pointer = None;
    let mut flaw = None;

    let mut offset = 0;
    if payload.len() % 2 == 1 {
      if let Some(&value) = payload.first() {
        pointer = u32::try_from(value).ok();
        offset = 1;
      };
    }

    for chunk in payload[offset..].chunks(2) {
      if chunk.len() != 2 {
        flaw.get_or_insert(Flaw::TrailingIntegers);
        break;
      }

      let id = RuneId {
        block: 1,
        tx: (chunk[0] % 2) as u32,
      };
      let amount = chunk[0] / 2;

      let Some(edict) = Edict::from_integers(tx, id, amount, chunk[1]) else {
        flaw.get_or_insert(Flaw::EdictOutput);
        break;
      };

      edicts.push(edict);
    }

    Self {
      flaw,
      edicts,
      pointer,
    }
  }
}
