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
pub struct Output {
  pub utils_per_sat: Decimal,
  pub sats_per_util: Decimal,
  pub interest_rate: Decimal,
  pub bonds_per_sat: Decimal,
  pub utils_per_bond: Decimal,
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

  index.update()?;

  let util_state = index.get_util_state()?;

  Ok(Some(Box::new(Output {
    utils_per_sat: Decimal {
      value: util_state.utils_per_sat,
      scale: 12,
    },
    sats_per_util: Decimal {
      value: util_state.decimals * util_state.decimals / util_state.utils_per_sat,
      scale: 12,
    },
    interest_rate: Decimal {
      value: util_state.interest_rate,
      scale: 12,
    },
    bonds_per_sat: Decimal {
      value: util_state.bonds_per_sat,
      scale: 12,
    },
    utils_per_bond: Decimal {
      value: util_state.utils_per_bond,
      scale: 12,
    },
  })))
}

impl SatToUtilInput {
  pub(crate) fn run(self, settings: Settings) -> SubcommandResult {
    let index = Index::open(&settings)?;

    index.update()?;

    let state = index.get_util_state()?;
    let utils = self.sats * state.utils_per_sat / state.decimals;

    Ok(Some(Box::new(SatToUtilOutput { utils })))
  }
}

impl UtilToSatInput {
  pub(crate) fn run(self, settings: Settings) -> SubcommandResult {
    let index = Index::open(&settings)?;

    index.update()?;

    let state = index.get_util_state()?;
    let sats = (self.utils * state.decimals + state.utils_per_sat - 1) / state.utils_per_sat;

    Ok(Some(Box::new(UtilToSatOutput { sats })))
  }
}
