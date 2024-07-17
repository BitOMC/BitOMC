use super::*;

#[derive(Copy, Clone)]
pub enum Blocktime {
  Confirmed(DateTime<Utc>),
  Expected(DateTime<Utc>),
}

impl Blocktime {
  pub(crate) fn confirmed(seconds: u32) -> Self {
    Self::Confirmed(timestamp(seconds.into()))
  }

  pub(crate) fn unix_timestamp(self) -> i64 {
    match self {
      Self::Confirmed(timestamp) | Self::Expected(timestamp) => timestamp.timestamp(),
    }
  }
}
