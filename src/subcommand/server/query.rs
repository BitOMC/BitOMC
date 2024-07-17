use super::*;

pub(super) enum Block {
  Height(u32),
  Hash(BlockHash),
}

impl FromStr for Block {
  type Err = Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    Ok(if s.len() == 64 {
      Self::Hash(s.parse()?)
    } else {
      Self::Height(s.parse()?)
    })
  }
}

#[derive(Debug)]
pub(super) enum Rune {
  Spaced(SpacedRune),
  Id(RuneId),
  Number(u64),
}

impl FromStr for Rune {
  type Err = Error;

  fn from_str(s: &str) -> Result<Self, Self::Err> {
    if s.contains(':') {
      Ok(Self::Id(s.parse()?))
    } else if re::RUNE_NUMBER.is_match(s) {
      Ok(Self::Number(s.parse()?))
    } else {
      Ok(Self::Spaced(s.parse()?))
    }
  }
}
