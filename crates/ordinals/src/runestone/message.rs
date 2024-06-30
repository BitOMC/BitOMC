use super::*;

pub(super) struct Message {
  pub(super) flaw: Option<Flaw>,
  pub(super) edicts: Vec<Edict>,
  pub(super) fields: HashMap<u128, VecDeque<u128>>,
}

impl Message {
  pub(super) fn from_integers(tx: &Transaction, payload: &[u128]) -> Self {
    let mut edicts = Vec::new();
    let mut fields = HashMap::<u128, VecDeque<u128>>::new();
    let mut flaw = None;

    for i in (0..payload.len()).step_by(2) {
      let tag = payload[i];

      if Tag::Body == tag {
        for chunk in payload[i + 1..].chunks(2) {
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
        break;
      }

      let Some(&value) = payload.get(i + 1) else {
        flaw.get_or_insert(Flaw::TruncatedField);
        break;
      };

      fields.entry(tag).or_default().push_back(value);
    }

    Self {
      flaw,
      edicts,
      fields,
    }
  }
}
