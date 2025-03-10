use {super::*, std::ffi::OsString, tempfile::TempDir};

pub(crate) struct ContextBuilder {
  args: Vec<OsString>,
  chain: Chain,
  event_sender: Option<tokio::sync::mpsc::Sender<Event>>,
  tempdir: Option<TempDir>,
}

impl ContextBuilder {
  pub(crate) fn build(self) -> Context {
    self.try_build().unwrap()
  }

  pub(crate) fn try_build(self) -> Result<Context> {
    let core = mockcore::builder().network(self.chain.network()).build();

    let tempdir = self.tempdir.unwrap_or_else(|| TempDir::new().unwrap());
    let cookie_file = tempdir.path().join("cookie");
    fs::write(&cookie_file, "username:password").unwrap();

    let command: Vec<OsString> = vec![
      "bitomc".into(),
      "--bitcoin-rpc-url".into(),
      core.url().into(),
      "--datadir".into(),
      tempdir.path().into(),
      "--cookie-file".into(),
      cookie_file.into(),
      format!("--chain={}", self.chain).into(),
    ];

    let options = Options::try_parse_from(command.into_iter().chain(self.args)).unwrap();
    let index = Index::open_with_event_sender(
      &Settings::from_options(options).or_defaults().unwrap(),
      self.event_sender,
    )?;
    index.update().unwrap();

    Ok(Context {
      index,
      core,
      tempdir,
    })
  }

  pub(crate) fn arg(mut self, arg: impl Into<OsString>) -> Self {
    self.args.push(arg.into());
    self
  }

  pub(crate) fn args<T: Into<OsString>, I: IntoIterator<Item = T>>(mut self, args: I) -> Self {
    self.args.extend(args.into_iter().map(|arg| arg.into()));
    self
  }

  pub(crate) fn chain(mut self, chain: Chain) -> Self {
    self.chain = chain;
    self
  }

  pub(crate) fn event_sender(mut self, sender: tokio::sync::mpsc::Sender<Event>) -> Self {
    self.event_sender = Some(sender);
    self
  }
}

pub(crate) struct Context {
  pub(crate) index: Index,
  pub(crate) core: mockcore::Handle,
  #[allow(unused)]
  pub(crate) tempdir: TempDir,
}

impl Context {
  pub(crate) fn builder() -> ContextBuilder {
    ContextBuilder {
      args: Vec::new(),
      chain: Chain::Regtest,
      event_sender: None,
      tempdir: None,
    }
  }

  #[track_caller]
  pub(crate) fn mine_blocks(&self, n: u64) -> Vec<Block> {
    self.mine_blocks_with_update(n, true)
  }

  #[track_caller]
  pub(crate) fn mine_blocks_with_update(&self, n: u64, update: bool) -> Vec<Block> {
    let blocks = self.core.mine_blocks(n);
    if update {
      self.index.update().unwrap();
    }
    blocks
  }

  #[track_caller]
  pub(crate) fn assert_runes(
    &self,
    mut runes: impl AsMut<[(RuneId, RuneEntry)]>,
    mut balances: impl AsMut<[(OutPoint, Vec<(RuneId, u128)>)]>,
  ) {
    let runes = runes.as_mut();
    runes.sort_by_key(|(id, _)| *id);

    let balances = balances.as_mut();
    balances.sort_by_key(|(outpoint, _)| *outpoint);

    for (_, balances) in balances.iter_mut() {
      balances.sort_by_key(|(id, _)| *id);
    }

    pretty_assert_eq!(runes, self.index.runes().unwrap());

    pretty_assert_eq!(balances, self.index.get_rune_balances().unwrap());

    let mut outstanding: HashMap<RuneId, u128> = HashMap::new();

    for (_, balances) in balances {
      for (id, balance) in balances {
        *outstanding.entry(*id).or_default() += *balance;
      }
    }

    for (id, entry) in runes {
      pretty_assert_eq!(
        outstanding.get(id).copied().unwrap_or_default(),
        entry.supply() - entry.burned
      );
    }
  }

  pub(crate) fn get_block_count(&self) -> usize {
    usize::try_from(self.index.block_count().unwrap()).unwrap()
  }
}
