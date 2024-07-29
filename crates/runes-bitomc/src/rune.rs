use super::*;

#[derive(
  Default, Debug, PartialEq, Copy, Clone, PartialOrd, Ord, Eq, DeserializeFromStr, SerializeDisplay,
)]
pub struct Rune(pub u128);

impl Rune {
  pub fn n(self) -> u128 {
    self.0
  }

  pub fn first_rune_height(network: Network) -> u32 {
    match network {
      Network::Bitcoin => 855_000,
      Network::Regtest => 2,
      Network::Signet => 2,
      Network::Testnet => 2_869_900,
      _ => 2,
    }
  }

  pub fn commitment(self) -> Vec<u8> {
    let bytes = self.0.to_le_bytes();

    let mut end = bytes.len();

    while end > 0 && bytes[end - 1] == 0 {
      end -= 1;
    }

    bytes[..end].into()
  }
}

impl Display for Rune {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    if self.0 == 0 {
      write!(f, "TIGHTEN")
    } else if self.0 == 1 {
      write!(f, "EASE")
    } else {
      let mut n = self.0;
      if n == u128::MAX {
        return write!(f, "BCGDENLQRQWDSLRUGSNLBTMFIJAV");
      }

      n += 1;
      let mut symbol = String::new();
      while n > 0 {
        symbol.push(
          "ABCDEFGHIJKLMNOPQRSTUVWXYZ"
            .chars()
            .nth(((n - 1) % 26) as usize)
            .unwrap(),
        );
        n = (n - 1) / 26;
      }

      for c in symbol.chars().rev() {
        write!(f, "{c}")?;
      }

      Ok(())
    }
  }
}

impl FromStr for Rune {
  type Err = Error;

  fn from_str(s: &str) -> Result<Self, Error> {
    if s == "TIGHTEN" {
      Ok(Rune(0))
    } else if s == "EASE" {
      Ok(Rune(1))
    } else {
      let mut x = 0u128;
      for (i, c) in s.chars().enumerate() {
        if i > 0 {
          x = x.checked_add(1).ok_or(Error::Range)?;
        }
        x = x.checked_mul(26).ok_or(Error::Range)?;
        match c {
          'A'..='Z' => {
            x = x.checked_add(c as u128 - 'A' as u128).ok_or(Error::Range)?;
          }
          _ => return Err(Error::Character(c)),
        }
      }
      Ok(Rune(x))
    }
  }
}

#[derive(Debug, PartialEq)]
pub enum Error {
  Character(char),
  Range,
}

impl Display for Error {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    match self {
      Self::Character(c) => write!(f, "invalid character `{c}`"),
      Self::Range => write!(f, "name out of range"),
    }
  }
}

impl std::error::Error for Error {}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn round_trip() {
    fn case(n: u128, s: &str) {
      assert_eq!(Rune(n).to_string(), s);
      assert_eq!(s.parse::<Rune>().unwrap(), Rune(n));
    }

    case(0, "TIGHTEN");
    case(1, "EASE");
    case(2, "C");
    case(3, "D");
    case(4, "E");
    case(5, "F");
    case(6, "G");
    case(7, "H");
    case(8, "I");
    case(9, "J");
    case(10, "K");
    case(11, "L");
    case(12, "M");
    case(13, "N");
    case(14, "O");
    case(15, "P");
    case(16, "Q");
    case(17, "R");
    case(18, "S");
    case(19, "T");
    case(20, "U");
    case(21, "V");
    case(22, "W");
    case(23, "X");
    case(24, "Y");
    case(25, "Z");
    case(26, "AA");
    case(27, "AB");
    case(51, "AZ");
    case(52, "BA");
    case(u128::MAX - 2, "BCGDENLQRQWDSLRUGSNLBTMFIJAT");
    case(u128::MAX - 1, "BCGDENLQRQWDSLRUGSNLBTMFIJAU");
    case(u128::MAX, "BCGDENLQRQWDSLRUGSNLBTMFIJAV");
  }

  #[test]
  fn from_str_error() {
    assert_eq!(
      "BCGDENLQRQWDSLRUGSNLBTMFIJAW".parse::<Rune>().unwrap_err(),
      Error::Range,
    );
    assert_eq!(
      "BCGDENLQRQWDSLRUGSNLBTMFIJAVX".parse::<Rune>().unwrap_err(),
      Error::Range,
    );
    assert_eq!("x".parse::<Rune>().unwrap_err(), Error::Character('x'));
  }

  #[test]
  fn serde() {
    let rune = Rune(0);
    let json = "\"TIGHTEN\"";
    assert_eq!(serde_json::to_string(&rune).unwrap(), json);
    assert_eq!(serde_json::from_str::<Rune>(json).unwrap(), rune);
  }

  #[test]
  fn commitment() {
    #[track_caller]
    fn case(rune: u128, bytes: &[u8]) {
      assert_eq!(Rune(rune).commitment(), bytes);
    }

    case(0, &[]);
    case(1, &[1]);
    case(255, &[255]);
    case(256, &[0, 1]);
    case(65535, &[255, 255]);
    case(65536, &[0, 0, 1]);
    case(u128::MAX, &[255; 16]);
  }
}
