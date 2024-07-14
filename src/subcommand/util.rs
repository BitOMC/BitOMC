use super::*;

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
