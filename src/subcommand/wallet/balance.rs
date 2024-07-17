use super::*;

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub struct Output {
  pub cardinal: u64,
  pub runes: BTreeMap<SpacedRune, Decimal>,
  pub runic: u64,
  pub total: u64,
}

pub(crate) fn run(wallet: Wallet) -> SubcommandResult {
  let unspent_outputs = wallet.utxos();

  let mut cardinal = 0;
  let mut runes = BTreeMap::new();
  let mut runic = 0;

  for (output, txout) in unspent_outputs {
    let rune_balances = wallet.get_runes_balances_in_output(output)?;

    let is_runic = !rune_balances.is_empty();

    if is_runic {
      for (spaced_rune, pile) in rune_balances {
        runes
          .entry(spaced_rune)
          .and_modify(|decimal: &mut Decimal| {
            assert_eq!(decimal.scale, pile.divisibility);
            decimal.value += pile.amount;
          })
          .or_insert(Decimal {
            value: pile.amount,
            scale: pile.divisibility,
          });
      }
      runic += txout.value;
    }

    if !is_runic {
      cardinal += txout.value;
    }
  }

  Ok(Some(Box::new(Output {
    cardinal,
    runes,
    runic,
    total: cardinal + runic,
  })))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn runes_and_runic_fields_are_not_present_if_none() {
    assert_eq!(
      serde_json::to_string(&Output {
        cardinal: 0,
        runes: BTreeMap::new(),
        runic: 0,
        total: 0
      })
      .unwrap(),
      r#"{"cardinal":0,"runes":{},"runic":0,"total":0}"#
    );
  }
}
