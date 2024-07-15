use super::*;

#[derive(Debug, Parser)]
pub(crate) struct SatToUtilInput {
  sats: u128,
}

#[derive(Debug, Parser)]
pub(crate) struct UtilToSatInput {
  utils: u128,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SatToUtilOutput {
  pub utils: u128,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UtilToSatOutput {
  pub sats: u128,
}

pub(crate) fn run(settings: Settings) -> SubcommandResult {
  let index = Index::open(&settings)?;

  ensure!(
    index.has_rune_index(),
    "`ord balances` requires index created with `--index-runes` flag",
  );

  index.update()?;

  let util_state = index.get_util_state()?;

  Ok(Some(Box::new(util_state)))
}

impl SatToUtilInput {
  pub(crate) fn run(self, settings: Settings) -> SubcommandResult {
    let index = Index::open(&settings)?;

    ensure!(
      index.has_rune_index(),
      "`ord balances` requires index created with `--index-runes` flag",
    );

    index.update()?;

    let state = index.get_util_state()?;
    let utils = self.sats * state.utils_per_sat / 1_000_000_000_000;

    Ok(Some(Box::new(SatToUtilOutput { utils })))
  }
}

impl UtilToSatInput {
  pub(crate) fn run(self, settings: Settings) -> SubcommandResult {
    let index = Index::open(&settings)?;

    ensure!(
      index.has_rune_index(),
      "`ord balances` requires index created with `--index-runes` flag",
    );

    index.update()?;

    let state = index.get_util_state()?;
    let base_value = 1_000_000_000_000;
    let sats = (self.utils * base_value + state.utils_per_sat - 1) / state.utils_per_sat;

    Ok(Some(Box::new(UtilToSatOutput { sats })))
  }
}
