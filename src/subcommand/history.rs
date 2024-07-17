use super::*;

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
  pub median_interest_rate: Decimal,
  pub history: Vec<Decimal>,
}

pub(crate) fn run(settings: Settings) -> SubcommandResult {
  let index = Index::open(&settings)?;

  index.update()?;

  let rate_history = index.get_rate_history()?;

  Ok(Some(Box::new(Output {
    median_interest_rate: Decimal {
      value: rate_history.median_interest_rate,
      scale: 12,
    },
    history: rate_history
      .history
      .iter()
      .map(|&rate| Decimal {
        value: rate,
        scale: 12,
      })
      .collect(),
  })))
}
