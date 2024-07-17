use super::*;

pub(crate) trait Entry: Sized {
  type Value;

  fn load(value: Self::Value) -> Self;

  fn store(self) -> Self::Value;
}

pub(super) type HeaderValue = [u8; 80];

impl Entry for Header {
  type Value = HeaderValue;

  fn load(value: Self::Value) -> Self {
    consensus::encode::deserialize(&value).unwrap()
  }

  fn store(self) -> Self::Value {
    let mut buffer = Cursor::new([0; 80]);
    let len = self
      .consensus_encode(&mut buffer)
      .expect("in-memory writers don't error");
    let buffer = buffer.into_inner();
    debug_assert_eq!(len, buffer.len());
    buffer
  }
}

impl Entry for Rune {
  type Value = u128;

  fn load(value: Self::Value) -> Self {
    Self(value)
  }

  fn store(self) -> Self::Value {
    self.0
  }
}

#[derive(Debug, PartialEq, Copy, Clone, Serialize, Deserialize)]
pub struct RuneEntry {
  pub block: u64,
  pub burned: u128,
  pub divisibility: u8,
  pub etching: Txid,
  pub mints: u128,
  pub number: u64,
  pub supply: u128,
  pub spaced_rune: SpacedRune,
  pub symbol: Option<char>,
  pub terms: Option<Terms>,
  pub timestamp: u64,
  pub turbo: bool,
}

impl RuneEntry {
  pub fn mintable(&self, height: u64) -> u128 {
    if height < self.block {
      return 0;
    }

    // Mintable amount is running sum of available amount since last mint
    let mut amount = 0;
    for mint in self.mints..((height as u128) - (self.block as u128) + 1) {
      amount += self.reward(mint);
    }
    amount
  }

  fn reward(&self, height: u128) -> u128 {
    let halvings = height / 210000;
    // Force reward to zero when right shift is undefined
    if halvings >= 128 {
      return 0;
    }
    // Cut reward in half every 210,000 blocks
    let reward = 50 * 100000000;
    reward >> halvings
  }

  pub fn supply(&self) -> u128 {
    self.supply
  }

  pub fn max_supply(&self) -> u128 {
    self.terms.and_then(|terms| terms.cap).unwrap_or_default()
      * self
        .terms
        .and_then(|terms| terms.amount)
        .unwrap_or_default()
  }

  pub fn pile(&self, amount: u128) -> Pile {
    Pile {
      amount,
      divisibility: self.divisibility,
      symbol: self.symbol,
    }
  }

  pub fn start(&self) -> Option<u64> {
    let terms = self.terms?;

    let relative = terms
      .offset
      .0
      .map(|offset| self.block.saturating_add(offset));

    let absolute = terms.height.0;

    relative
      .zip(absolute)
      .map(|(relative, absolute)| relative.max(absolute))
      .or(relative)
      .or(absolute)
  }

  pub fn end(&self) -> Option<u64> {
    let terms = self.terms?;

    let relative = terms
      .offset
      .1
      .map(|offset| self.block.saturating_add(offset));

    let absolute = terms.height.1;

    relative
      .zip(absolute)
      .map(|(relative, absolute)| relative.min(absolute))
      .or(relative)
      .or(absolute)
  }
}

type TermsEntryValue = (
  Option<u128>,               // cap
  (Option<u64>, Option<u64>), // height
  Option<u128>,               // amount
  (Option<u64>, Option<u64>), // offset
);

pub(super) type RuneEntryValue = (
  u64,                     // block
  u128,                    // burned
  u8,                      // divisibility
  (u128, u128),            // etching
  u128,                    // mints
  u64,                     // number
  u128,                    // supply
  (u128, u32),             // spaced rune
  Option<char>,            // symbol
  Option<TermsEntryValue>, // terms
  u64,                     // timestamp
  bool,                    // turbo
);

impl Default for RuneEntry {
  fn default() -> Self {
    Self {
      block: 2,
      burned: 0,
      divisibility: 8,
      etching: Txid::all_zeros(),
      mints: 0,
      number: 0,
      supply: 0,
      spaced_rune: SpacedRune::default(),
      symbol: None,
      terms: None,
      timestamp: 0,
      turbo: false,
    }
  }
}

impl Entry for RuneEntry {
  type Value = RuneEntryValue;

  fn load(
    (
      block,
      burned,
      divisibility,
      etching,
      mints,
      number,
      supply,
      (rune, spacers),
      symbol,
      terms,
      timestamp,
      turbo,
    ): RuneEntryValue,
  ) -> Self {
    Self {
      block,
      burned,
      divisibility,
      etching: {
        let low = etching.0.to_le_bytes();
        let high = etching.1.to_le_bytes();
        Txid::from_byte_array([
          low[0], low[1], low[2], low[3], low[4], low[5], low[6], low[7], low[8], low[9], low[10],
          low[11], low[12], low[13], low[14], low[15], high[0], high[1], high[2], high[3], high[4],
          high[5], high[6], high[7], high[8], high[9], high[10], high[11], high[12], high[13],
          high[14], high[15],
        ])
      },
      mints,
      number,
      supply,
      spaced_rune: SpacedRune {
        rune: Rune(rune),
        spacers,
      },
      symbol,
      terms: terms.map(|(cap, height, amount, offset)| Terms {
        cap,
        height,
        amount,
        offset,
      }),
      timestamp,
      turbo,
    }
  }

  fn store(self) -> Self::Value {
    (
      self.block,
      self.burned,
      self.divisibility,
      {
        let bytes = self.etching.to_byte_array();
        (
          u128::from_le_bytes([
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
            bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
          ]),
          u128::from_le_bytes([
            bytes[16], bytes[17], bytes[18], bytes[19], bytes[20], bytes[21], bytes[22], bytes[23],
            bytes[24], bytes[25], bytes[26], bytes[27], bytes[28], bytes[29], bytes[30], bytes[31],
          ]),
        )
      },
      self.mints,
      self.number,
      self.supply,
      (self.spaced_rune.rune.0, self.spaced_rune.spacers),
      self.symbol,
      self.terms.map(
        |Terms {
           cap,
           height,
           amount,
           offset,
         }| (cap, height, amount, offset),
      ),
      self.timestamp,
      self.turbo,
    )
  }
}

pub(super) type RuneIdValue = (u64, u32);

impl Entry for RuneId {
  type Value = RuneIdValue;

  fn load((block, tx): Self::Value) -> Self {
    Self { block, tx }
  }

  fn store(self) -> Self::Value {
    (self.block, self.tx)
  }
}

#[derive(Debug, PartialEq, Clone)]
pub struct UtilEntry {
  index: u32,
  rates: Vec<u128>,
  bonds_per_sat: u128,
}

impl UtilEntry {
  const BASE_VALUE: u128 = 1_000_000_000_000;
  const BLOCKS_PER_YEAR: u128 = 52_595;

  pub fn new() -> UtilEntry {
    UtilEntry {
      index: 0,
      rates: vec![0; 100],
      bonds_per_sat: Self::BASE_VALUE,
    }
  }

  pub fn update(&mut self, supply0: u128, supply1: u128) {
    if supply0 > supply1 {
      let rate = Self::BASE_VALUE * (supply0 - supply1) / (supply0 + supply1);
      if rate > 0 {
        self.rates[self.index as usize % 100] = rate;
        self.index = (self.index + 1) % 100;
      }
    }

    self.bonds_per_sat +=
      self.bonds_per_sat * self.interest_rate() / Self::BASE_VALUE / Self::BLOCKS_PER_YEAR;
  }

  pub fn interest_rate(&self) -> u128 {
    let mut non_zero: Vec<u128> = self.rates.clone().into_iter().filter(|&x| x > 0).collect();
    if non_zero.is_empty() {
      return Self::BASE_VALUE;
    }

    non_zero.sort_unstable();
    let len = non_zero.len();

    if len % 2 == 0 {
      let mid_right = len / 2;
      let mid_left = mid_right - 1;
      (non_zero[mid_left] + non_zero[mid_right]) / 2
    } else {
      non_zero[len / 2]
    }
  }

  pub fn bonds_per_sat(&self) -> u128 {
    self.bonds_per_sat
  }

  pub fn utils_per_bond(&self) -> u128 {
    Self::BASE_VALUE * Self::BASE_VALUE / self.interest_rate()
  }

  pub fn utils_per_sat(&self) -> u128 {
    self.bonds_per_sat * self.utils_per_bond() / Self::BASE_VALUE
  }

  pub fn decimals(&self) -> u128 {
    Self::BASE_VALUE
  }

  pub fn history(&self) -> Vec<u128> {
    let i = self.index as usize;

    (0..self.rates.len())
      .map(|j| self.rates[(self.rates.len() + i - j - 1) % self.rates.len()])
      .filter(|&x| x != 0)
      .collect()
  }
}

pub(crate) type UtilEntryValue = (
  u32,       // index
  Vec<u128>, // rates
  u128,      // bonds_per_sat
);

impl Entry for UtilEntry {
  type Value = UtilEntryValue;

  fn load((index, rates, bonds_per_sat): Self::Value) -> Self {
    Self {
      index,
      rates,
      bonds_per_sat,
    }
  }

  fn store(self) -> Self::Value {
    (self.index, self.rates, self.bonds_per_sat)
  }
}

#[derive(Debug, Eq, PartialEq, Clone)]
pub struct InscriptionEntry {
  pub charms: u16,
  pub fee: u64,
  pub height: u32,
  pub id: InscriptionId,
  pub inscription_number: i32,
  pub parents: Vec<u32>,
  pub sat: Option<Sat>,
  pub sequence_number: u32,
  pub timestamp: u32,
}

pub(crate) type InscriptionEntryValue = (
  u16,                // charms
  u64,                // fee
  u32,                // height
  InscriptionIdValue, // inscription id
  i32,                // inscription number
  Vec<u32>,           // parents
  Option<u64>,        // sat
  u32,                // sequence number
  u32,                // timestamp
);

impl Entry for InscriptionEntry {
  type Value = InscriptionEntryValue;

  #[rustfmt::skip]
  fn load(
    (
      charms,
      fee,
      height,
      id,
      inscription_number,
      parents,
      sat,
      sequence_number,
      timestamp,
    ): InscriptionEntryValue,
  ) -> Self {
    Self {
      charms,
      fee,
      height,
      id: InscriptionId::load(id),
      inscription_number,
      parents,
      sat: sat.map(Sat),
      sequence_number,
      timestamp,
    }
  }

  fn store(self) -> Self::Value {
    (
      self.charms,
      self.fee,
      self.height,
      self.id.store(),
      self.inscription_number,
      self.parents,
      self.sat.map(Sat::n),
      self.sequence_number,
      self.timestamp,
    )
  }
}

pub(crate) type InscriptionIdValue = (u128, u128, u32);

impl Entry for InscriptionId {
  type Value = InscriptionIdValue;

  fn load(value: Self::Value) -> Self {
    let (head, tail, index) = value;
    let head_array = head.to_le_bytes();
    let tail_array = tail.to_le_bytes();
    let array = [
      head_array[0],
      head_array[1],
      head_array[2],
      head_array[3],
      head_array[4],
      head_array[5],
      head_array[6],
      head_array[7],
      head_array[8],
      head_array[9],
      head_array[10],
      head_array[11],
      head_array[12],
      head_array[13],
      head_array[14],
      head_array[15],
      tail_array[0],
      tail_array[1],
      tail_array[2],
      tail_array[3],
      tail_array[4],
      tail_array[5],
      tail_array[6],
      tail_array[7],
      tail_array[8],
      tail_array[9],
      tail_array[10],
      tail_array[11],
      tail_array[12],
      tail_array[13],
      tail_array[14],
      tail_array[15],
    ];

    Self {
      txid: Txid::from_byte_array(array),
      index,
    }
  }

  fn store(self) -> Self::Value {
    let txid_entry = self.txid.store();
    let little_end = u128::from_le_bytes(txid_entry[..16].try_into().unwrap());
    let big_end = u128::from_le_bytes(txid_entry[16..].try_into().unwrap());
    (little_end, big_end, self.index)
  }
}

pub(super) type OutPointValue = [u8; 36];

impl Entry for OutPoint {
  type Value = OutPointValue;

  fn load(value: Self::Value) -> Self {
    Decodable::consensus_decode(&mut Cursor::new(value)).unwrap()
  }

  fn store(self) -> Self::Value {
    let mut value = [0; 36];
    self.consensus_encode(&mut value.as_mut_slice()).unwrap();
    value
  }
}

pub(super) type TxOutValue = (
  u64,     // value
  Vec<u8>, // script_pubkey
);

impl Entry for TxOut {
  type Value = TxOutValue;

  fn load(value: Self::Value) -> Self {
    Self {
      value: value.0,
      script_pubkey: ScriptBuf::from_bytes(value.1),
    }
  }

  fn store(self) -> Self::Value {
    (self.value, self.script_pubkey.to_bytes())
  }
}

pub(super) type SatPointValue = [u8; 44];

impl Entry for SatPoint {
  type Value = SatPointValue;

  fn load(value: Self::Value) -> Self {
    Decodable::consensus_decode(&mut Cursor::new(value)).unwrap()
  }

  fn store(self) -> Self::Value {
    let mut value = [0; 44];
    self.consensus_encode(&mut value.as_mut_slice()).unwrap();
    value
  }
}

pub(super) type SatRange = (u64, u64);

impl Entry for SatRange {
  type Value = [u8; 11];

  fn load([b0, b1, b2, b3, b4, b5, b6, b7, b8, b9, b10]: Self::Value) -> Self {
    let raw_base = u64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, 0]);

    // 51 bit base
    let base = raw_base & ((1 << 51) - 1);

    let raw_delta = u64::from_le_bytes([b6, b7, b8, b9, b10, 0, 0, 0]);

    // 33 bit delta
    let delta = raw_delta >> 3;

    (base, base + delta)
  }

  fn store(self) -> Self::Value {
    let base = self.0;
    let delta = self.1 - self.0;
    let n = u128::from(base) | u128::from(delta) << 51;
    n.to_le_bytes()[0..11].try_into().unwrap()
  }
}

pub(super) type TxidValue = [u8; 32];

impl Entry for Txid {
  type Value = TxidValue;

  fn load(value: Self::Value) -> Self {
    Txid::from_byte_array(value)
  }

  fn store(self) -> Self::Value {
    Txid::to_byte_array(self)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn txout_entry() {
    let txout = TxOut {
      value: u64::MAX,
      script_pubkey: change(0).script_pubkey(),
    };

    let value = (u64::MAX, change(0).script_pubkey().to_bytes());

    assert_eq!(txout.clone().store(), value);
    assert_eq!(TxOut::load(value), txout);
  }

  #[test]
  fn inscription_entry() {
    let id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdefi0"
      .parse::<InscriptionId>()
      .unwrap();

    let entry = InscriptionEntry {
      charms: 0,
      fee: 1,
      height: 2,
      id,
      inscription_number: 3,
      parents: vec![4, 5, 6],
      sat: Some(Sat(7)),
      sequence_number: 8,
      timestamp: 9,
    };

    let value = (0, 1, 2, id.store(), 3, vec![4, 5, 6], Some(7), 8, 9);

    assert_eq!(entry.clone().store(), value);
    assert_eq!(InscriptionEntry::load(value), entry);
  }

  #[test]
  fn inscription_id_entry() {
    let inscription_id = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdefi0"
      .parse::<InscriptionId>()
      .unwrap();

    assert_eq!(
      inscription_id.store(),
      (
        0x0123456789abcdef0123456789abcdef,
        0x0123456789abcdef0123456789abcdef,
        0
      )
    );

    assert_eq!(
      InscriptionId::load((
        0x0123456789abcdef0123456789abcdef,
        0x0123456789abcdef0123456789abcdef,
        0
      )),
      inscription_id
    );
  }

  #[test]
  fn parent_entry_index() {
    let inscription_id = "0000000000000000000000000000000000000000000000000000000000000000i1"
      .parse::<InscriptionId>()
      .unwrap();

    assert_eq!(inscription_id.store(), (0, 0, 1));

    assert_eq!(InscriptionId::load((0, 0, 1)), inscription_id);

    let inscription_id = "0000000000000000000000000000000000000000000000000000000000000000i256"
      .parse::<InscriptionId>()
      .unwrap();

    assert_eq!(inscription_id.store(), (0, 0, 256));

    assert_eq!(InscriptionId::load((0, 0, 256)), inscription_id);
  }

  #[test]
  fn rune_entry() {
    let entry = RuneEntry {
      block: 12,
      burned: 1,
      divisibility: 3,
      etching: Txid::from_byte_array([
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
        0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D,
        0x1E, 0x1F,
      ]),
      terms: Some(Terms {
        cap: Some(1),
        height: (Some(2), Some(3)),
        amount: Some(4),
        offset: (Some(5), Some(6)),
      }),
      mints: 11,
      number: 6,
      supply: 12,
      spaced_rune: SpacedRune {
        rune: Rune(7),
        spacers: 8,
      },
      symbol: Some('a'),
      timestamp: 10,
      turbo: true,
    };

    let value = (
      12,
      1,
      3,
      (
        0x0F0E0D0C0B0A09080706050403020100,
        0x1F1E1D1C1B1A19181716151413121110,
      ),
      11,
      6,
      12,
      (7, 8),
      Some('a'),
      Some((Some(1), (Some(2), Some(3)), Some(4), (Some(5), Some(6)))),
      10,
      true,
    );

    assert_eq!(entry.store(), value);
    assert_eq!(RuneEntry::load(value), entry);
  }

  #[test]
  fn rune_id_entry() {
    assert_eq!(RuneId { block: 1, tx: 2 }.store(), (1, 2),);

    assert_eq!(RuneId { block: 1, tx: 2 }, RuneId::load((1, 2)),);
  }

  #[test]
  fn header() {
    let expected = [
      0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
      26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
      49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71,
      72, 73, 74, 75, 76, 77, 78, 79,
    ];

    let header = Header::load(expected);
    let actual = header.store();

    assert_eq!(actual, expected);
  }
}
