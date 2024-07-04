use {super::*, message::Message};

mod message;

#[derive(Default, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub struct Runestone {
  pub edicts: Vec<Edict>,
  pub pointer: Option<u32>,
}

#[derive(Debug, PartialEq)]
enum Payload {
  Valid(Vec<u8>),
  Invalid(Flaw),
}

impl Runestone {
  pub const MAGIC_NUMBER: opcodes::All = opcodes::all::OP_PUSHNUM_14;
  pub const COMMIT_CONFIRMATIONS: u16 = 6;

  pub fn decipher(transaction: &Transaction) -> Option<Artifact> {
    let payload = match Runestone::payload(transaction) {
      Some(Payload::Valid(payload)) => payload,
      Some(Payload::Invalid(flaw)) => {
        return Some(Artifact::Cenotaph(Cenotaph { flaw: Some(flaw) }));
      }
      None => return None,
    };

    let Ok(integers) = Runestone::integers(&payload) else {
      return Some(Artifact::Cenotaph(Cenotaph {
        flaw: Some(Flaw::Varint),
      }));
    };

    let Message {
      flaw,
      edicts,
      mut pointer,
    } = Message::from_integers(transaction, &integers);

    if let Some(p) = pointer {
      if u64::from(p) >= u64::try_from(transaction.output.len()).unwrap() {
        pointer = None;
      }
    }

    if let Some(flaw) = flaw {
      return Some(Artifact::Cenotaph(Cenotaph { flaw: Some(flaw) }));
    }

    Some(Artifact::Runestone(Self { edicts, pointer }))
  }

  pub fn encipher(&self) -> ScriptBuf {
    let mut payload = Vec::new();

    if let Some(pointer) = self.pointer {
      varint::encode_to_vec(pointer.into(), &mut payload);
    }

    if !self.edicts.is_empty() {
      for mut edict in self.edicts.clone() {
        if edict.amount >= u128::MAX / 2 {
          edict.amount = u128::MAX / 2 - 1;
        }
        let id0 = RuneId { block: 1, tx: 0 };
        let encoded_id: u128 = if edict.id == id0 { 0 } else { 1 };
        let encoded_amt: u128 = 2 * edict.amount + encoded_id;
        varint::encode_to_vec(encoded_amt, &mut payload);
        varint::encode_to_vec(edict.output.into(), &mut payload);
      }
    }

    let mut builder = script::Builder::new()
      .push_opcode(opcodes::all::OP_RETURN)
      .push_opcode(Runestone::MAGIC_NUMBER);

    for chunk in payload.chunks(MAX_SCRIPT_ELEMENT_SIZE) {
      let push: &script::PushBytes = chunk.try_into().unwrap();
      builder = builder.push_slice(push);
    }

    builder.into_script()
  }

  fn payload(transaction: &Transaction) -> Option<Payload> {
    // search transaction outputs for payload
    for output in &transaction.output {
      let mut instructions = output.script_pubkey.instructions();

      // payload starts with OP_RETURN
      if instructions.next() != Some(Ok(Instruction::Op(opcodes::all::OP_RETURN))) {
        continue;
      }

      // followed by the protocol identifier, ignoring errors, since OP_RETURN
      // scripts may be invalid
      if instructions.next() != Some(Ok(Instruction::Op(Runestone::MAGIC_NUMBER))) {
        continue;
      }

      // construct the payload by concatenating remaining data pushes
      let mut payload = Vec::new();

      for result in instructions {
        match result {
          Ok(Instruction::PushBytes(push)) => {
            payload.extend_from_slice(push.as_bytes());
          }
          Ok(Instruction::Op(_)) => {
            return Some(Payload::Invalid(Flaw::Opcode));
          }
          Err(_) => {
            return Some(Payload::Invalid(Flaw::InvalidScript));
          }
        }
      }

      return Some(Payload::Valid(payload));
    }

    None
  }

  fn integers(payload: &[u8]) -> Result<Vec<u128>, varint::Error> {
    let mut integers = Vec::new();
    let mut i = 0;

    while i < payload.len() {
      let (integer, length) = varint::decode(&payload[i..])?;
      integers.push(integer);
      i += length;
    }

    Ok(integers)
  }
}
