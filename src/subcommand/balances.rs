use super::*;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Output {
  pub runes: BTreeMap<SpacedRune, BTreeMap<OutPoint, Pile>>,
}

pub(crate) fn run(settings: Settings) -> SubcommandResult {
  let index = Index::open(&settings)?;

  index.update()?;

  Ok(Some(Box::new(Output {
    runes: index.get_rune_balance_map()?,
  })))
}
