use {
  self::{
    entry::{
      Entry, HeaderValue,
      OutPointValue, RuneEntryValue, RuneIdValue, TxOutValue, TxidValue, UtilEntry,
      UtilEntryValue,
    },
    event::Event,
    lot::Lot,
    reorg::Reorg,
    updater::Updater,
  },
  super::*,
  crate::templates::StatusHtml,
  bitcoin::block::Header,
  bitcoincore_rpc::{
    json::{GetBlockHeaderResult, GetBlockStatsResult},
    Client,
  },
  chrono::SubsecRound,
  indicatif::{ProgressBar, ProgressStyle},
  log::log_enabled,
  redb::{
    Database, DatabaseError, MultimapTable, MultimapTableDefinition, MultimapTableHandle,
    ReadOnlyTable, ReadableTable, ReadableTableMetadata, RepairSession, StorageError, Table,
    TableDefinition, TableHandle, TableStats, WriteTransaction,
  },
  std::{
    collections::HashMap,
    sync::Once,
  },
};

pub use self::entry::RuneEntry;

pub(crate) mod entry;
pub mod event;
mod fetcher;
mod lot;
mod reorg;
mod rtx;
mod updater;

#[cfg(test)]
pub(crate) mod testing;

const SCHEMA_VERSION: u64 = 26;

define_multimap_table! { SCRIPT_PUBKEY_TO_OUTPOINT, &[u8], OutPointValue }
define_table! { HEIGHT_TO_BLOCK_HEADER, u32, &HeaderValue }
define_table! { OUTPOINT_TO_RUNE_BALANCES, &OutPointValue, &[u8] }
define_table! { OUTPOINT_TO_TXOUT, &OutPointValue, TxOutValue }
define_table! { RUNE_ID_TO_RUNE_ENTRY, RuneIdValue, RuneEntryValue }
define_table! { RUNE_TO_RUNE_ID, u128, RuneIdValue }
define_table! { STATISTIC_TO_COUNT, u64, u64 }
define_table! { TRANSACTION_ID_TO_RUNE, &TxidValue, u128 }
define_table! { TRANSACTION_ID_TO_TRANSACTION, &TxidValue, &[u8] }
define_table! { WRITE_TRANSACTION_STARTING_BLOCK_COUNT_TO_TIMESTAMP, u32, u128 }
define_table! { STATE_CHANGE_TO_LAST_OUTPOINT, u8, &OutPointValue }
define_table! { UTIL_ENTRY, u8, UtilEntryValue }

#[derive(Copy, Clone)]
pub(crate) enum Statistic {
  Schema = 0,
  Commits = 1,
  Runes = 2,
  IndexTransactions = 3,
  InitialSyncTime = 4,
  IndexAddresses = 5,
}

impl Statistic {
  fn key(self) -> u64 {
    self.into()
  }
}

impl From<Statistic> for u64 {
  fn from(statistic: Statistic) -> Self {
    statistic as u64
  }
}

#[derive(Copy, Clone)]
pub enum StateChange {
  Mint = 0,
  Convert = 1,
}

impl StateChange {
  fn key(self) -> u8 {
    self.into()
  }
}

impl From<StateChange> for u8 {
  fn from(state_change: StateChange) -> Self {
    state_change as u8
  }
}

#[derive(Serialize)]
pub struct Info {
  blocks_indexed: u32,
  branch_pages: u64,
  fragmented_bytes: u64,
  index_file_size: u64,
  index_path: PathBuf,
  leaf_pages: u64,
  metadata_bytes: u64,
  outputs_traversed: u64,
  page_size: usize,
  sat_ranges: u64,
  stored_bytes: u64,
  tables: BTreeMap<String, TableInfo>,
  total_bytes: u64,
  pub transactions: Vec<TransactionInfo>,
  tree_height: u32,
  utxos_indexed: u64,
}

#[derive(Serialize)]
pub(crate) struct TableInfo {
  branch_pages: u64,
  fragmented_bytes: u64,
  leaf_pages: u64,
  metadata_bytes: u64,
  proportion: f64,
  stored_bytes: u64,
  total_bytes: u64,
  tree_height: u32,
}

impl From<TableStats> for TableInfo {
  fn from(stats: TableStats) -> Self {
    Self {
      branch_pages: stats.branch_pages(),
      fragmented_bytes: stats.fragmented_bytes(),
      leaf_pages: stats.leaf_pages(),
      metadata_bytes: stats.metadata_bytes(),
      proportion: 0.0,
      stored_bytes: stats.stored_bytes(),
      total_bytes: stats.stored_bytes() + stats.metadata_bytes() + stats.fragmented_bytes(),
      tree_height: stats.tree_height(),
    }
  }
}

#[derive(Serialize)]
pub struct TransactionInfo {
  pub starting_block_count: u32,
  pub starting_timestamp: u128,
}

pub(crate) trait BitcoinCoreRpcResultExt<T> {
  fn into_option(self) -> Result<Option<T>>;
}

impl<T> BitcoinCoreRpcResultExt<T> for Result<T, bitcoincore_rpc::Error> {
  fn into_option(self) -> Result<Option<T>> {
    match self {
      Ok(ok) => Ok(Some(ok)),
      Err(bitcoincore_rpc::Error::JsonRpc(bitcoincore_rpc::jsonrpc::error::Error::Rpc(
        bitcoincore_rpc::jsonrpc::error::RpcError { code: -8, .. },
      ))) => Ok(None),
      Err(bitcoincore_rpc::Error::JsonRpc(bitcoincore_rpc::jsonrpc::error::Error::Rpc(
        bitcoincore_rpc::jsonrpc::error::RpcError { message, .. },
      )))
        if message.ends_with("not found") =>
      {
        Ok(None)
      }
      Err(err) => Err(err.into()),
    }
  }
}

pub struct Index {
  pub(crate) client: Client,
  database: Database,
  durability: redb::Durability,
  event_sender: Option<tokio::sync::mpsc::Sender<Event>>,
  first_rune_height: u32,
  genesis_block_coinbase_transaction: Transaction,
  genesis_block_coinbase_txid: Txid,
  height_limit: Option<u32>,
  index_addresses: bool,
  index_runes: bool,
  index_sats: bool,
  index_transactions: bool,
  path: PathBuf,
  settings: Settings,
  started: DateTime<Utc>,
  unrecoverably_reorged: AtomicBool,
}

impl Index {
  pub fn open(settings: &Settings) -> Result<Self> {
    Index::open_with_event_sender(settings, None)
  }

  pub fn open_with_event_sender(
    settings: &Settings,
    event_sender: Option<tokio::sync::mpsc::Sender<Event>>,
  ) -> Result<Self> {
    let client = settings.bitcoin_rpc_client(None)?;

    let path = settings.index().to_owned();

    if let Err(err) = fs::create_dir_all(path.parent().unwrap()) {
      bail!(
        "failed to create data dir `{}`: {err}",
        path.parent().unwrap().display()
      );
    }

    let index_cache_size = settings.index_cache_size();

    log::info!("Setting index cache size to {} bytes", index_cache_size);

    let durability = if cfg!(test) {
      redb::Durability::None
    } else {
      redb::Durability::Immediate
    };

    let index_path = path.clone();
    let once = Once::new();
    let progress_bar = Mutex::new(None);
    let integration_test = settings.integration_test();

    let repair_callback = move |progress: &mut RepairSession| {
      once.call_once(|| println!("Index file `{}` needs recovery. This can take a long time, especially for the --index-sats index.", index_path.display()));

      if !(cfg!(test) || log_enabled!(log::Level::Info) || integration_test) {
        let mut guard = progress_bar.lock().unwrap();

        let progress_bar = guard.get_or_insert_with(|| {
          let progress_bar = ProgressBar::new(100);
          progress_bar.set_style(
            ProgressStyle::with_template("[repairing database] {wide_bar} {pos}/{len}").unwrap(),
          );
          progress_bar
        });

        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        progress_bar.set_position((progress.progress() * 100.0) as u64);
      }
    };

    let database = match Database::builder()
      .set_cache_size(index_cache_size)
      .set_repair_callback(repair_callback)
      .open(&path)
    {
      Ok(database) => {
        {
          let schema_version = database
            .begin_read()?
            .open_table(STATISTIC_TO_COUNT)?
            .get(&Statistic::Schema.key())?
            .map(|x| x.value())
            .unwrap_or(0);

          match schema_version.cmp(&SCHEMA_VERSION) {
            cmp::Ordering::Less =>
              bail!(
                "index at `{}` appears to have been built with an older, incompatible version of ord, consider deleting and rebuilding the index: index schema {schema_version}, ord schema {SCHEMA_VERSION}",
                path.display()
              ),
            cmp::Ordering::Greater =>
              bail!(
                "index at `{}` appears to have been built with a newer, incompatible version of ord, consider updating ord: index schema {schema_version}, ord schema {SCHEMA_VERSION}",
                path.display()
              ),
            cmp::Ordering::Equal => {
            }
          }
        }

        database
      }
      Err(DatabaseError::Storage(StorageError::Io(error)))
        if error.kind() == io::ErrorKind::NotFound =>
      {
        let database = Database::builder()
          .set_cache_size(index_cache_size)
          .create(&path)?;

        let mut tx = database.begin_write()?;

        tx.set_durability(durability);

        tx.open_multimap_table(SCRIPT_PUBKEY_TO_OUTPOINT)?;
        tx.open_table(HEIGHT_TO_BLOCK_HEADER)?;
        tx.open_table(OUTPOINT_TO_RUNE_BALANCES)?;
        tx.open_table(OUTPOINT_TO_TXOUT)?;
        tx.open_table(RUNE_ID_TO_RUNE_ENTRY)?;
        tx.open_table(RUNE_TO_RUNE_ID)?;
        tx.open_table(TRANSACTION_ID_TO_RUNE)?;
        tx.open_table(WRITE_TRANSACTION_STARTING_BLOCK_COUNT_TO_TIMESTAMP)?;
        tx.open_table(STATE_CHANGE_TO_LAST_OUTPOINT)?;
        tx.open_table(UTIL_ENTRY)?
          .insert(0, UtilEntry::new().store())?;

        {
          let mut statistics = tx.open_table(STATISTIC_TO_COUNT)?;

          Self::set_statistic(
            &mut statistics,
            Statistic::IndexAddresses,
            u64::from(settings.index_addresses()),
          )?;

          Self::set_statistic(
            &mut statistics,
            Statistic::IndexTransactions,
            u64::from(settings.index_transactions()),
          )?;

          Self::set_statistic(&mut statistics, Statistic::Schema, SCHEMA_VERSION)?;
        }

        if settings.index_runes() {
          let rune0 = Rune(0); // tighten
          let rune1 = Rune(1); // ease

          let id0 = RuneId { block: 1, tx: 0 };
          let id1 = RuneId { block: 1, tx: 1 };

          tx.open_table(RUNE_TO_RUNE_ID)?
            .insert(rune0.store(), id0.store())?;
          tx.open_table(RUNE_TO_RUNE_ID)?
            .insert(rune1.store(), id1.store())?;

          let mut statistics = tx.open_table(STATISTIC_TO_COUNT)?;

          Self::set_statistic(&mut statistics, Statistic::Runes, 2)?;

          tx.open_table(RUNE_ID_TO_RUNE_ENTRY)?.insert(
            id0.store(),
            RuneEntry {
              divisibility: 8,
              spaced_rune: SpacedRune {
                rune: rune0,
                spacers: 0,
              },
              ..default()
            }
            .store(),
          )?;

          tx.open_table(RUNE_ID_TO_RUNE_ENTRY)?.insert(
            id1.store(),
            RuneEntry {
              divisibility: 8,
              spaced_rune: SpacedRune {
                rune: rune1,
                spacers: 0,
              },
              ..default()
            }
            .store(),
          )?;
        }

        tx.commit()?;

        database
      }
      Err(error) => bail!("failed to open index: {error}"),
    };

    let index_addresses;
    let index_transactions;

    {
      let tx = database.begin_read()?;
      let statistics = tx.open_table(STATISTIC_TO_COUNT)?;
      index_addresses = Self::is_statistic_set(&statistics, Statistic::IndexAddresses)?;
      index_transactions = Self::is_statistic_set(&statistics, Statistic::IndexTransactions)?;
    }

    let genesis_block_coinbase_transaction =
      settings.chain().genesis_block().coinbase().unwrap().clone();

    Ok(Self {
      genesis_block_coinbase_txid: genesis_block_coinbase_transaction.txid(),
      client,
      database,
      durability,
      event_sender,
      first_rune_height: settings.first_rune_height(),
      genesis_block_coinbase_transaction,
      height_limit: settings.height_limit(),
      index_addresses,
      index_runes: true,
      index_sats: false,
      index_transactions,
      settings: settings.clone(),
      path,
      started: Utc::now(),
      unrecoverably_reorged: AtomicBool::new(false),
    })
  }

  #[cfg(test)]
  fn set_durability(&mut self, durability: redb::Durability) {
    self.durability = durability;
  }

  pub fn contains_output(&self, output: &OutPoint) -> Result<bool> {
    Ok(
      self
        .database
        .begin_read()?
        .open_table(OUTPOINT_TO_TXOUT)?
        .get(&output.store())?
        .is_some(),
    )
  }

  pub fn has_address_index(&self) -> bool {
    self.index_addresses
  }

  pub fn has_rune_index(&self) -> bool {
    self.index_runes
  }

  pub fn has_sat_index(&self) -> bool {
    self.index_sats
  }

  pub fn status(&self) -> Result<StatusHtml> {
    let rtx = self.database.begin_read()?;

    let statistic_to_count = rtx.open_table(STATISTIC_TO_COUNT)?;

    let statistic = |statistic: Statistic| -> Result<u64> {
      Ok(
        statistic_to_count
          .get(statistic.key())?
          .map(|guard| guard.value())
          .unwrap_or_default(),
      )
    };

    let height = rtx
      .open_table(HEIGHT_TO_BLOCK_HEADER)?
      .range(0..)?
      .next_back()
      .transpose()?
      .map(|(height, _header)| height.value());

    let initial_sync_time = statistic(Statistic::InitialSyncTime)?;

    Ok(StatusHtml {
      address_index: self.has_address_index(),
      chain: self.settings.chain(),
      height,
      initial_sync_time: Duration::from_micros(initial_sync_time),
      inscriptions: 0,
      lost_sats: 0,
      rune_index: self.has_rune_index(),
      runes: statistic(Statistic::Runes)?,
      sat_index: false,
      started: self.started,
      transaction_index: statistic(Statistic::IndexTransactions)? != 0,
      unrecoverably_reorged: self.unrecoverably_reorged.load(atomic::Ordering::Relaxed),
      uptime: (Utc::now() - self.started).to_std()?,
      last_mint_outpoint: self.get_last_outpoint_for_state_change(StateChange::Mint)?,
      last_conversion_outpoint: self.get_last_outpoint_for_state_change(StateChange::Convert)?,
    })
  }

  pub fn info(&self) -> Result<Info> {
    let stats = self.database.begin_write()?.stats()?;

    let rtx = self.database.begin_read()?;

    let mut tables: BTreeMap<String, TableInfo> = BTreeMap::new();

    for handle in rtx.list_tables()? {
      let name = handle.name().into();
      let stats = rtx.open_untyped_table(handle)?.stats()?;
      tables.insert(name, stats.into());
    }

    for handle in rtx.list_multimap_tables()? {
      let name = handle.name().into();
      let stats = rtx.open_untyped_multimap_table(handle)?.stats()?;
      tables.insert(name, stats.into());
    }

    for table in rtx.list_tables()? {
      assert!(tables.contains_key(table.name()));
    }

    for table in rtx.list_multimap_tables()? {
      assert!(tables.contains_key(table.name()));
    }

    let total_bytes = tables
      .values()
      .map(|table_info| table_info.total_bytes)
      .sum();

    tables.values_mut().for_each(|table_info| {
      table_info.proportion = table_info.total_bytes as f64 / total_bytes as f64
    });

    let info = {
      Info {
        index_path: self.path.clone(),
        blocks_indexed: rtx
          .open_table(HEIGHT_TO_BLOCK_HEADER)?
          .range(0..)?
          .next_back()
          .transpose()?
          .map(|(height, _header)| height.value() + 1)
          .unwrap_or(0),
        branch_pages: stats.branch_pages(),
        fragmented_bytes: stats.fragmented_bytes(),
        index_file_size: fs::metadata(&self.path)?.len(),
        leaf_pages: stats.leaf_pages(),
        metadata_bytes: stats.metadata_bytes(),
        sat_ranges: 0,
        outputs_traversed: 0,
        page_size: stats.page_size(),
        stored_bytes: stats.stored_bytes(),
        total_bytes,
        tables,
        transactions: rtx
          .open_table(WRITE_TRANSACTION_STARTING_BLOCK_COUNT_TO_TIMESTAMP)?
          .range(0..)?
          .flat_map(|result| {
            result.map(
              |(starting_block_count, starting_timestamp)| TransactionInfo {
                starting_block_count: starting_block_count.value(),
                starting_timestamp: starting_timestamp.value(),
              },
            )
          })
          .collect(),
        tree_height: stats.tree_height(),
        utxos_indexed: rtx
          .open_table(OUTPOINT_TO_TXOUT)?
          .len()?
          .max(rtx.open_table(OUTPOINT_TO_RUNE_BALANCES)?.len()?),
      }
    };

    Ok(info)
  }

  pub fn get_util_state(&self) -> Result<api::UtilState> {
    Ok(
      self
        .database
        .begin_read()?
        .open_table(UTIL_ENTRY)?
        .get(0)?
        .map(|e| UtilEntry::load(e.value()))
        .map(|u| api::UtilState {
          bonds_per_sat: u.bonds_per_sat(),
          utils_per_bond: u.utils_per_bond(),
          utils_per_sat: u.utils_per_sat(),
          interest_rate: u.interest_rate(),
          decimals: u.decimals(),
        })
        .unwrap(),
    )
  }

  pub fn get_rate_history(&self) -> Result<api::RateHistory> {
    Ok(
      self
        .database
        .begin_read()?
        .open_table(UTIL_ENTRY)?
        .get(0)?
        .map(|e| UtilEntry::load(e.value()))
        .map(|u| api::RateHistory {
          median_interest_rate: u.interest_rate(),
          history: u.history(),
        })
        .unwrap(),
    )
  }

  pub fn simulate(&self, transactions: Vec<Transaction>) -> Result<Vec<api::SupplyState>> {
    let wtx = self.begin_write()?;

    let height = wtx
      .open_table(HEIGHT_TO_BLOCK_HEADER)?
      .range(0..)?
      .next_back()
      .transpose()?
      .map(|(height, _header)| height.value() + 1)
      .unwrap_or(0);

    Updater::simulate(wtx, self, height, transactions)
  }

  pub fn update(&self) -> Result {
    loop {
      let wtx = self.begin_write()?;

      let mut updater = Updater {
        height: wtx
          .open_table(HEIGHT_TO_BLOCK_HEADER)?
          .range(0..)?
          .next_back()
          .transpose()?
          .map(|(height, _header)| height.value() + 1)
          .unwrap_or(0),
        index: self,
        outputs_cached: 0,
      };

      match updater.update_index(wtx) {
        Ok(ok) => return Ok(ok),
        Err(err) => {
          log::info!("{}", err.to_string());

          match err.downcast_ref() {
            Some(&reorg::Error::Recoverable { height, depth }) => {
              Reorg::handle_reorg(self, height, depth)?;
            }
            Some(&reorg::Error::Unrecoverable) => {
              self
                .unrecoverably_reorged
                .store(true, atomic::Ordering::Relaxed);
              return Err(anyhow!(reorg::Error::Unrecoverable));
            }
            _ => return Err(err),
          };
        }
      }
    }
  }

  fn begin_read(&self) -> Result<rtx::Rtx> {
    Ok(rtx::Rtx(self.database.begin_read()?))
  }

  fn begin_write(&self) -> Result<WriteTransaction> {
    let mut tx = self.database.begin_write()?;
    tx.set_durability(self.durability);
    Ok(tx)
  }

  fn increment_statistic(wtx: &WriteTransaction, statistic: Statistic, n: u64) -> Result {
    let mut statistic_to_count = wtx.open_table(STATISTIC_TO_COUNT)?;
    let value = statistic_to_count
      .get(&(statistic.key()))?
      .map(|x| x.value())
      .unwrap_or_default()
      + n;
    statistic_to_count.insert(&statistic.key(), &value)?;
    Ok(())
  }

  pub(crate) fn set_statistic(
    statistics: &mut Table<u64, u64>,
    statistic: Statistic,
    value: u64,
  ) -> Result<()> {
    statistics.insert(&statistic.key(), &value)?;
    Ok(())
  }

  pub(crate) fn is_statistic_set(
    statistics: &ReadOnlyTable<u64, u64>,
    statistic: Statistic,
  ) -> Result<bool> {
    Ok(
      statistics
        .get(&statistic.key())?
        .map(|guard| guard.value())
        .unwrap_or_default()
        != 0,
    )
  }

  #[cfg(test)]
  pub(crate) fn statistic(&self, statistic: Statistic) -> u64 {
    self
      .database
      .begin_read()
      .unwrap()
      .open_table(STATISTIC_TO_COUNT)
      .unwrap()
      .get(&statistic.key())
      .unwrap()
      .map(|x| x.value())
      .unwrap_or_default()
  }

  pub fn get_last_outpoint_for_state_change(&self, state_change: StateChange) -> Result<OutPoint> {
    Ok(
      self
        .database
        .begin_read()?
        .open_table(STATE_CHANGE_TO_LAST_OUTPOINT)?
        .get(&state_change.key())?
        .map(|entry| OutPoint::load(*entry.value()))
        .unwrap_or(OutPoint::null()),
    )
  }

  pub fn block_count(&self) -> Result<u32> {
    self.begin_read()?.block_count()
  }

  pub fn block_height(&self) -> Result<Option<Height>> {
    self.begin_read()?.block_height()
  }

  pub fn block_hash(&self, height: Option<u32>) -> Result<Option<BlockHash>> {
    self.begin_read()?.block_hash(height)
  }

  pub fn blocks(&self, take: usize) -> Result<Vec<(u32, BlockHash)>> {
    let rtx = self.begin_read()?;

    let block_count = rtx.block_count()?;

    let height_to_block_header = rtx.0.open_table(HEIGHT_TO_BLOCK_HEADER)?;

    let mut blocks = Vec::with_capacity(block_count.try_into().unwrap());

    for next in height_to_block_header
      .range(0..block_count)?
      .rev()
      .take(take)
    {
      let next = next?;
      blocks.push((next.0.value(), Header::load(*next.1.value()).block_hash()));
    }

    Ok(blocks)
  }

  pub fn get_rune_by_id(&self, id: RuneId) -> Result<Option<Rune>> {
    Ok(
      self
        .database
        .begin_read()?
        .open_table(RUNE_ID_TO_RUNE_ENTRY)?
        .get(&id.store())?
        .map(|entry| RuneEntry::load(entry.value()).spaced_rune.rune),
    )
  }

  pub fn get_rune_by_number(&self, number: usize) -> Result<Option<Rune>> {
    match self
      .database
      .begin_read()?
      .open_table(RUNE_ID_TO_RUNE_ENTRY)?
      .iter()?
      .nth(number)
    {
      Some(result) => {
        let rune_result =
          result.map(|(_id, entry)| RuneEntry::load(entry.value()).spaced_rune.rune);
        Ok(rune_result.ok())
      }
      None => Ok(None),
    }
  }

  pub fn rune(&self, rune: Rune) -> Result<Option<(RuneId, RuneEntry)>> {
    let rtx = self.database.begin_read()?;

    let Some(id) = rtx
      .open_table(RUNE_TO_RUNE_ID)?
      .get(rune.0)?
      .map(|guard| guard.value())
    else {
      return Ok(None);
    };

    let entry = RuneEntry::load(
      rtx
        .open_table(RUNE_ID_TO_RUNE_ENTRY)?
        .get(id)?
        .unwrap()
        .value(),
    );

    Ok(Some((RuneId::load(id), entry)))
  }

  pub fn runes(&self) -> Result<Vec<(RuneId, RuneEntry)>> {
    let mut entries = Vec::new();

    for result in self
      .database
      .begin_read()?
      .open_table(RUNE_ID_TO_RUNE_ENTRY)?
      .iter()?
    {
      let (id, entry) = result?;
      entries.push((RuneId::load(id.value()), RuneEntry::load(entry.value())));
    }

    Ok(entries)
  }

  pub fn runes_paginated(
    &self,
    page_size: usize,
    page_index: usize,
  ) -> Result<(Vec<(RuneId, RuneEntry)>, bool)> {
    let mut entries = Vec::new();

    for result in self
      .database
      .begin_read()?
      .open_table(RUNE_ID_TO_RUNE_ENTRY)?
      .iter()?
      .rev()
      .skip(page_index.saturating_mul(page_size))
      .take(page_size.saturating_add(1))
    {
      let (id, entry) = result?;
      entries.push((RuneId::load(id.value()), RuneEntry::load(entry.value())));
    }

    let more = entries.len() > page_size;

    Ok((entries, more))
  }

  pub fn encode_rune_balance(id: RuneId, balance: u128, buffer: &mut Vec<u8>) {
    varint::encode_to_vec(id.block.into(), buffer);
    varint::encode_to_vec(id.tx.into(), buffer);
    varint::encode_to_vec(balance, buffer);
  }

  pub fn decode_rune_balance(buffer: &[u8]) -> Result<((RuneId, u128), usize)> {
    let mut len = 0;
    let (block, block_len) = varint::decode(&buffer[len..])?;
    len += block_len;
    let (tx, tx_len) = varint::decode(&buffer[len..])?;
    len += tx_len;
    let id = RuneId {
      block: block.try_into()?,
      tx: tx.try_into()?,
    };
    let (balance, balance_len) = varint::decode(&buffer[len..])?;
    len += balance_len;
    Ok(((id, balance), len))
  }

  pub fn get_rune_balances_for_output(
    &self,
    outpoint: OutPoint,
  ) -> Result<BTreeMap<SpacedRune, Pile>> {
    let rtx = self.database.begin_read()?;

    let outpoint_to_balances = rtx.open_table(OUTPOINT_TO_RUNE_BALANCES)?;

    let id_to_rune_entries = rtx.open_table(RUNE_ID_TO_RUNE_ENTRY)?;

    let Some(balances) = outpoint_to_balances.get(&outpoint.store())? else {
      return Ok(BTreeMap::new());
    };

    let balances_buffer = balances.value();

    let mut balances = BTreeMap::new();
    let mut i = 0;
    while i < balances_buffer.len() {
      let ((id, amount), length) = Index::decode_rune_balance(&balances_buffer[i..]).unwrap();
      i += length;

      let entry = RuneEntry::load(id_to_rune_entries.get(id.store())?.unwrap().value());

      balances.insert(
        entry.spaced_rune,
        Pile {
          amount,
          divisibility: entry.divisibility,
          symbol: entry.symbol,
        },
      );
    }

    Ok(balances)
  }

  pub fn get_rune_balance_map(&self) -> Result<BTreeMap<SpacedRune, BTreeMap<OutPoint, Pile>>> {
    let outpoint_balances = self.get_rune_balances()?;

    let rtx = self.database.begin_read()?;

    let rune_id_to_rune_entry = rtx.open_table(RUNE_ID_TO_RUNE_ENTRY)?;

    let mut rune_balances_by_id: BTreeMap<RuneId, BTreeMap<OutPoint, u128>> = BTreeMap::new();

    for (outpoint, balances) in outpoint_balances {
      for (rune_id, amount) in balances {
        *rune_balances_by_id
          .entry(rune_id)
          .or_default()
          .entry(outpoint)
          .or_default() += amount;
      }
    }

    let mut rune_balances = BTreeMap::new();

    for (rune_id, balances) in rune_balances_by_id {
      let RuneEntry {
        divisibility,
        spaced_rune,
        symbol,
        ..
      } = RuneEntry::load(
        rune_id_to_rune_entry
          .get(&rune_id.store())?
          .unwrap()
          .value(),
      );

      rune_balances.insert(
        spaced_rune,
        balances
          .into_iter()
          .map(|(outpoint, amount)| {
            (
              outpoint,
              Pile {
                amount,
                divisibility,
                symbol,
              },
            )
          })
          .collect(),
      );
    }

    Ok(rune_balances)
  }

  pub fn get_rune_balances(&self) -> Result<Vec<(OutPoint, Vec<(RuneId, u128)>)>> {
    let mut result = Vec::new();

    for entry in self
      .database
      .begin_read()?
      .open_table(OUTPOINT_TO_RUNE_BALANCES)?
      .iter()?
    {
      let (outpoint, balances_buffer) = entry?;
      let outpoint = OutPoint::load(*outpoint.value());
      let balances_buffer = balances_buffer.value();

      let mut balances = Vec::new();
      let mut i = 0;
      while i < balances_buffer.len() {
        let ((id, balance), length) = Index::decode_rune_balance(&balances_buffer[i..]).unwrap();
        i += length;
        balances.push((id, balance));
      }

      result.push((outpoint, balances));
    }

    Ok(result)
  }

  pub fn block_header(&self, hash: BlockHash) -> Result<Option<Header>> {
    self.client.get_block_header(&hash).into_option()
  }

  pub fn block_header_info(&self, hash: BlockHash) -> Result<Option<GetBlockHeaderResult>> {
    self.client.get_block_header_info(&hash).into_option()
  }

  pub fn block_stats(&self, height: u64) -> Result<Option<GetBlockStatsResult>> {
    self.client.get_block_stats(height).into_option()
  }

  pub fn get_block_by_height(&self, height: u32) -> Result<Option<Block>> {
    Ok(
      self
        .client
        .get_block_hash(height.into())
        .into_option()?
        .map(|hash| self.client.get_block(&hash))
        .transpose()?,
    )
  }

  pub fn get_block_by_hash(&self, hash: BlockHash) -> Result<Option<Block>> {
    self.client.get_block(&hash).into_option()
  }

  pub fn get_transaction(&self, txid: Txid) -> Result<Option<Transaction>> {
    if txid == self.genesis_block_coinbase_txid {
      return Ok(Some(self.genesis_block_coinbase_transaction.clone()));
    }

    if self.index_transactions {
      if let Some(transaction) = self
        .database
        .begin_read()?
        .open_table(TRANSACTION_ID_TO_TRANSACTION)?
        .get(&txid.store())?
      {
        return Ok(Some(consensus::encode::deserialize(transaction.value())?));
      }
    }

    self.client.get_raw_transaction(&txid, None).into_option()
  }

  pub fn is_output_spent(&self, outpoint: OutPoint) -> Result<bool> {
    Ok(
      outpoint != OutPoint::null()
        && outpoint != self.settings.chain().genesis_coinbase_outpoint()
        && if self.settings.index_addresses() {
          self
            .database
            .begin_read()?
            .open_table(OUTPOINT_TO_TXOUT)?
            .get(&outpoint.store())?
            .is_none()
        } else {
          self
            .client
            .get_tx_out(&outpoint.txid, outpoint.vout, Some(true))?
            .is_none()
        },
    )
  }

  pub fn is_output_in_active_chain(&self, outpoint: OutPoint) -> Result<bool> {
    if outpoint == OutPoint::null() {
      return Ok(true);
    }

    if outpoint == self.settings.chain().genesis_coinbase_outpoint() {
      return Ok(true);
    }

    let Some(info) = self
      .client
      .get_raw_transaction_info(&outpoint.txid, None)
      .into_option()?
    else {
      return Ok(false);
    };

    if info.blockhash.is_none() {
      return Ok(false);
    }

    if outpoint.vout.into_usize() >= info.vout.len() {
      return Ok(false);
    }

    Ok(true)
  }

  pub fn block_time(&self, height: Height) -> Result<Blocktime> {
    let height = height.n();

    let rtx = self.database.begin_read()?;

    let height_to_block_header = rtx.open_table(HEIGHT_TO_BLOCK_HEADER)?;

    if let Some(guard) = height_to_block_header.get(height)? {
      return Ok(Blocktime::confirmed(Header::load(*guard.value()).time));
    }

    let current = height_to_block_header
      .range(0..)?
      .next_back()
      .transpose()?
      .map(|(height, _header)| height)
      .map(|x| x.value())
      .unwrap_or(0);

    let expected_blocks = height
      .checked_sub(current)
      .with_context(|| format!("current {current} height is greater than sat height {height}"))?;

    Ok(Blocktime::Expected(
      Utc::now()
        .round_subsecs(0)
        .checked_add_signed(
          chrono::Duration::try_seconds(10 * 60 * i64::from(expected_blocks))
            .context("timestamp out of range")?,
        )
        .context("timestamp out of range")?,
    ))
  }

  pub fn get_runes_in_block(&self, block_height: u64) -> Result<Vec<SpacedRune>> {
    let rtx = self.database.begin_read()?;

    let rune_id_to_rune_entry = rtx.open_table(RUNE_ID_TO_RUNE_ENTRY)?;

    let min_id = RuneId {
      block: block_height,
      tx: 0,
    };

    let max_id = RuneId {
      block: block_height,
      tx: u32::MAX,
    };

    let runes = rune_id_to_rune_entry
      .range(min_id.store()..=max_id.store())?
      .map(|result| result.map(|(_, entry)| RuneEntry::load(entry.value()).spaced_rune))
      .collect::<Result<Vec<SpacedRune>, StorageError>>()?;

    Ok(runes)
  }

  pub fn get_address_info(&self, address: &Address) -> Result<Vec<OutPoint>> {
    self
      .database
      .begin_read()?
      .open_multimap_table(SCRIPT_PUBKEY_TO_OUTPOINT)?
      .get(address.script_pubkey().as_bytes())?
      .map(|result| {
        result
          .map_err(|err| anyhow!(err))
          .map(|value| OutPoint::load(value.value()))
      })
      .collect()
  }

  pub(crate) fn get_sat_balances_for_outputs(&self, outputs: &Vec<OutPoint>) -> Result<u64> {
    let outpoint_to_txout = self.database.begin_read()?.open_table(OUTPOINT_TO_TXOUT)?;

    let mut acc = 0;
    for output in outputs {
      if let Some(value) = outpoint_to_txout.get(&output.store())? {
        acc += TxOut::load(value.value()).value;
      };
    }

    Ok(acc)
  }

  pub(crate) fn get_output_info(&self, outpoint: OutPoint) -> Result<Option<(api::Output, TxOut)>> {
    let indexed;

    let txout = if outpoint == OutPoint::null() || outpoint == unbound_outpoint() {
      indexed = true;

      TxOut {
        value: 0,
        script_pubkey: ScriptBuf::new(),
      }
    } else {
      indexed = self.contains_output(&outpoint)?;

      let Some(tx) = self.get_transaction(outpoint.txid)? else {
        return Ok(None);
      };

      let Some(txout) = tx.output.into_iter().nth(outpoint.vout as usize) else {
        return Ok(None);
      };

      txout
    };

    let runes = self.get_rune_balances_for_output(outpoint)?;

    let spent = self.is_output_spent(outpoint)?;

    Ok(Some((
      api::Output::new(
        self.settings.chain(),
        vec![],
        outpoint,
        txout.clone(),
        indexed,
        runes,
        None,
        spent,
      ),
      txout,
    )))
  }
}

#[cfg(test)]
mod tests {
  use {super::*, crate::index::testing::Context, num_integer::Roots};

  #[test]
  fn height_limit() {
    {
      let context = Context::builder().args(["--height-limit", "0"]).build();
      context.mine_blocks(1);
      assert_eq!(context.index.block_height().unwrap(), None);
      assert_eq!(context.index.block_count().unwrap(), 0);
    }

    {
      let context = Context::builder().args(["--height-limit", "1"]).build();
      context.mine_blocks(1);
      assert_eq!(context.index.block_height().unwrap(), Some(Height(0)));
      assert_eq!(context.index.block_count().unwrap(), 1);
    }

    {
      let context = Context::builder().args(["--height-limit", "2"]).build();
      context.mine_blocks(2);
      assert_eq!(context.index.block_height().unwrap(), Some(Height(1)));
      assert_eq!(context.index.block_count().unwrap(), 2);
    }
  }

  #[test]
  fn recover_from_reorg() {
    const TIGHTEN: u128 = 0;
    const EASE: u128 = 1;
    const COIN_VALUE: u128 = 100000000;

    let mut context = Context::builder().arg("--index-runes").build();

    context.index.set_durability(redb::Durability::Immediate);

    context.mine_blocks(1);

    let txid = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(1, 0, 0, Witness::new())],
      mint: true,
      outputs: 2,
      op_return: Some(Runestone::default().encipher()),
      ..default()
    });

    context.mine_blocks(6);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: 50 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: 0,
            ..default()
          },
        ),
      ],
      [(OutPoint { txid, vout: 1 }, vec![(ID0, 50 * COIN_VALUE)])],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 1, 0, Witness::new()), (2, 1, 1, Witness::new())],
      mint: true,
      outputs: 2,
      op_return: Some(Runestone::default().encipher()),
      ..default()
    });

    context.mine_blocks(1);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 7,
            supply: 350 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 7,
            supply: 0,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 1,
        },
        vec![(ID0, 350 * COIN_VALUE)],
      )],
    );

    context.core.invalidate_tip();
    context.mine_blocks(2);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: 50 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: 0,
            ..default()
          },
        ),
      ],
      [(OutPoint { txid, vout: 1 }, vec![(ID0, 50 * COIN_VALUE)])],
    );
  }

  #[test]
  fn recover_from_3_block_deep_and_consecutive_reorg() {
    const TIGHTEN: u128 = 0;
    const EASE: u128 = 1;
    const COIN_VALUE: u128 = 100000000;

    let mut context = Context::builder().arg("--index-runes").build();

    context.index.set_durability(redb::Durability::Immediate);

    context.mine_blocks(1);

    let txid = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(1, 0, 0, Witness::new())],
      mint: true,
      outputs: 2,
      op_return: Some(Runestone::default().encipher()),
      ..default()
    });

    context.mine_blocks(10);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: 50 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: 0,
            ..default()
          },
        ),
      ],
      [(OutPoint { txid, vout: 1 }, vec![(ID0, 50 * COIN_VALUE)])],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new()), (2, 1, 0, Witness::new())],
      mint: true,
      outputs: 2,
      op_return: Some(Runestone::default().encipher()),
      ..default()
    });

    context.mine_blocks(1);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 11,
            supply: 550 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 11,
            supply: 0,
            ..default()
          },
        ),
      ],
      [
        (OutPoint { txid, vout: 1 }, vec![(ID0, 50 * COIN_VALUE)]),
        (
          OutPoint {
            txid: txid1,
            vout: 1,
          },
          vec![(ID0, 500 * COIN_VALUE)],
        ),
      ],
    );

    context.core.invalidate_tip();
    context.core.invalidate_tip();
    context.core.invalidate_tip();
    context.mine_blocks(4);

    context.core.invalidate_tip();
    context.mine_blocks(2);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: 50 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: 0,
            ..default()
          },
        ),
      ],
      [(OutPoint { txid, vout: 1 }, vec![(ID0, 50 * COIN_VALUE)])],
    );
  }

  #[test]
  fn recover_from_very_unlikely_7_block_deep_reorg() {
    const TIGHTEN: u128 = 0;
    const EASE: u128 = 1;
    const COIN_VALUE: u128 = 100000000;

    let mut context = Context::builder().arg("--index-runes").build();

    context.index.set_durability(redb::Durability::Immediate);

    context.mine_blocks(1);

    let txid = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(1, 0, 0, Witness::new())],
      mint: true,
      outputs: 2,
      op_return: Some(Runestone::default().encipher()),
      ..default()
    });

    context.mine_blocks(10);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: 50 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: 0,
            ..default()
          },
        ),
      ],
      [(OutPoint { txid, vout: 1 }, vec![(ID0, 50 * COIN_VALUE)])],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new()), (2, 1, 0, Witness::new())],
      mint: true,
      outputs: 2,
      op_return: Some(Runestone::default().encipher()),
      ..default()
    });

    context.mine_blocks(7);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 11,
            supply: 550 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 11,
            supply: 0,
            ..default()
          },
        ),
      ],
      [
        (OutPoint { txid, vout: 1 }, vec![(ID0, 50 * COIN_VALUE)]),
        (
          OutPoint {
            txid: txid1,
            vout: 1,
          },
          vec![(ID0, 500 * COIN_VALUE)],
        ),
      ],
    );

    for _ in 0..7 {
      context.core.invalidate_tip();
    }

    context.mine_blocks(9);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: 50 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: 0,
            ..default()
          },
        ),
      ],
      [(OutPoint { txid, vout: 1 }, vec![(ID0, 50 * COIN_VALUE)])],
    );
  }

  #[test]
  fn is_output_spent() {
    let context = Context::builder().build();

    assert!(!context.index.is_output_spent(OutPoint::null()).unwrap());
    assert!(!context
      .index
      .is_output_spent(Chain::Mainnet.genesis_coinbase_outpoint())
      .unwrap());

    context.mine_blocks(1);

    assert!(!context
      .index
      .is_output_spent(OutPoint {
        txid: context.core.tx(1, 0).txid(),
        vout: 0,
      })
      .unwrap());

    context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(1, 0, 0, Default::default())],
      ..default()
    });

    context.mine_blocks(1);

    assert!(context
      .index
      .is_output_spent(OutPoint {
        txid: context.core.tx(1, 0).txid(),
        vout: 0,
      })
      .unwrap());
  }

  #[test]
  fn is_output_in_active_chain() {
    let context = Context::builder().build();

    assert!(context
      .index
      .is_output_in_active_chain(OutPoint::null())
      .unwrap());

    assert!(context
      .index
      .is_output_in_active_chain(Chain::Mainnet.genesis_coinbase_outpoint())
      .unwrap());

    context.mine_blocks(1);

    assert!(context
      .index
      .is_output_in_active_chain(OutPoint {
        txid: context.core.tx(1, 0).txid(),
        vout: 0,
      })
      .unwrap());

    assert!(!context
      .index
      .is_output_in_active_chain(OutPoint {
        txid: context.core.tx(1, 0).txid(),
        vout: 1,
      })
      .unwrap());

    assert!(!context
      .index
      .is_output_in_active_chain(OutPoint {
        txid: Txid::all_zeros(),
        vout: 0,
      })
      .unwrap());
  }

  #[test]
  fn output_addresses_are_updated() {
    let context = Context::builder().arg("--index-addresses").build();

    context.mine_blocks(2);

    let txid = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(1, 0, 0, Witness::new()), (2, 0, 0, Witness::new())],
      outputs: 2,
      ..Default::default()
    });

    context.mine_blocks(1);

    let transaction = context.index.get_transaction(txid).unwrap().unwrap();

    let first_address = context
      .index
      .settings
      .chain()
      .address_from_script(&transaction.output[0].script_pubkey)
      .unwrap();

    let first_address_second_output = OutPoint {
      txid: transaction.txid(),
      vout: 1,
    };

    assert_eq!(
      context.index.get_address_info(&first_address).unwrap(),
      [
        OutPoint {
          txid: transaction.txid(),
          vout: 0
        },
        first_address_second_output
      ]
    );

    let txid = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(3, 1, 0, Witness::new())],
      p2tr: true,
      ..Default::default()
    });

    context.mine_blocks(1);

    let transaction = context.index.get_transaction(txid).unwrap().unwrap();

    let second_address = context
      .index
      .settings
      .chain()
      .address_from_script(&transaction.output[0].script_pubkey)
      .unwrap();

    assert_eq!(
      context.index.get_address_info(&first_address).unwrap(),
      [first_address_second_output]
    );

    assert_eq!(
      context.index.get_address_info(&second_address).unwrap(),
      [OutPoint {
        txid: transaction.txid(),
        vout: 0
      }]
    );
  }

  #[allow(clippy::cast_possible_truncation)]
  #[test]
  fn rune_event_sender_channel() {
    const TIGHTEN: u128 = 0;
    const EASE: u128 = 1;
    const COIN_VALUE: u128 = 100000000;

    let (event_sender, mut event_receiver) = tokio::sync::mpsc::channel(1024);
    let context = Context::builder()
      .arg("--index-runes")
      .event_sender(event_sender)
      .build();

    context.mine_blocks(1);

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(1, 0, 0, Witness::new())],
      mint: true,
      outputs: 2,
      op_return: Some(Runestone { ..default() }.encipher()),
      ..default()
    });

    context.mine_blocks(1);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: 50 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: 0,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid0,
          vout: 1,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    assert_eq!(
      event_receiver.blocking_recv().unwrap(),
      Event::RuneMinted {
        block_height: (context.get_block_count() as u32) - 1,
        txid: txid0,
        amount0: 50 * COIN_VALUE,
        amount1: 0
      }
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[
        (context.get_block_count() - 1, 1, 0, Witness::new()),
        (context.get_block_count() - 1, 1, 1, Witness::new()),
      ],
      op_return: None,
      ..default()
    });

    context.mine_blocks(1);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: 50 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: 0,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    event_receiver.blocking_recv().unwrap();

    pretty_assert_eq!(
      event_receiver.blocking_recv().unwrap(),
      Event::RuneTransferred {
        block_height: (context.get_block_count() as u32) - 1,
        txid: txid1,
        rune_id: ID0,
        amount: 50 * COIN_VALUE,
        outpoint: OutPoint {
          txid: txid1,
          vout: 0,
        },
      }
    );

    let txid2 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      op_return: Some(
        Runestone {
          edicts: vec![Edict {
            id: ID0,
            amount: 111,
            output: 0,
          }],
          ..default()
        }
        .encipher(),
      ),
      op_return_index: Some(0),
      ..default()
    });

    context.mine_blocks(1);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            burned: 111,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: 50 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: 0,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid2,
          vout: 1,
        },
        vec![(ID0, 50 * COIN_VALUE - 111)],
      )],
    );

    event_receiver.blocking_recv().unwrap();

    pretty_assert_eq!(
      event_receiver.blocking_recv().unwrap(),
      Event::RuneBurned {
        block_height: (context.get_block_count() as u32) - 1,
        txid: txid2,
        amount: 111,
        rune_id: ID0,
      }
    );
  }

  #[test]
  fn util_state_updates_each_block() {
    const TIGHTEN: u128 = 0;
    const EASE: u128 = 1;
    const COIN_VALUE: u128 = 100000000;
    const UTIL_BASE_VALUE: u128 = 1_000_000_000_000;
    const BLOCKS_PER_YEAR: u128 = 52_595;

    let context = Context::builder().arg("--index-runes").build();

    let interest_rate0 = UTIL_BASE_VALUE;
    let interest0 = UTIL_BASE_VALUE * interest_rate0 / UTIL_BASE_VALUE / BLOCKS_PER_YEAR;
    let bonds_per_sat0 = UTIL_BASE_VALUE + interest0;
    let utils_per_bond0 = UTIL_BASE_VALUE * UTIL_BASE_VALUE / interest_rate0;
    let utils_per_sat0 = bonds_per_sat0 * utils_per_bond0 / UTIL_BASE_VALUE;

    pretty_assert_eq!(
      context.index.get_util_state().unwrap(),
      api::UtilState {
        bonds_per_sat: bonds_per_sat0,
        utils_per_bond: utils_per_bond0,
        utils_per_sat: utils_per_sat0,
        interest_rate: interest_rate0,
        decimals: UTIL_BASE_VALUE,
      }
    );

    context.mine_blocks(1);

    let interest_rate1 = UTIL_BASE_VALUE;
    let interest1 = bonds_per_sat0 * interest_rate1 / UTIL_BASE_VALUE / BLOCKS_PER_YEAR;
    let bonds_per_sat1 = bonds_per_sat0 + interest1;
    let utils_per_bond1 = UTIL_BASE_VALUE * UTIL_BASE_VALUE / interest_rate1;
    let utils_per_sat1 = bonds_per_sat1 * utils_per_bond1 / UTIL_BASE_VALUE;

    pretty_assert_eq!(
      context.index.get_util_state().unwrap(),
      api::UtilState {
        bonds_per_sat: bonds_per_sat1,
        utils_per_bond: utils_per_bond1,
        utils_per_sat: utils_per_sat1,
        interest_rate: interest_rate1,
        decimals: UTIL_BASE_VALUE,
      }
    );

    // Mints 40 TIGHTEN and 30 EASE
    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(1, 0, 0, Witness::new())],
      mint: true,
      convert: true,
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: 40 * COIN_VALUE,
              output: 1,
            },
            Edict {
              id: ID1,
              amount: 30 * COIN_VALUE,
              output: 1,
            },
          ],
          pointer: Some(2),
        }
        .encipher(),
      ),
      ..default()
    });

    context.mine_blocks(1);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: 40 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: 30 * COIN_VALUE,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid0,
          vout: 1,
        },
        vec![(ID0, 40 * COIN_VALUE), (ID1, 30 * COIN_VALUE)],
      )],
    );

    let interest_rate2 = UTIL_BASE_VALUE * (40 - 30) / (40 + 30);
    let interest2 = bonds_per_sat1 * interest_rate2 / UTIL_BASE_VALUE / BLOCKS_PER_YEAR;
    let bonds_per_sat2 = bonds_per_sat1 + interest2;
    let utils_per_bond2 = UTIL_BASE_VALUE * UTIL_BASE_VALUE / interest_rate2;
    let utils_per_sat2 = bonds_per_sat2 * utils_per_bond2 / UTIL_BASE_VALUE;

    pretty_assert_eq!(
      context.index.get_util_state().unwrap(),
      api::UtilState {
        bonds_per_sat: bonds_per_sat2,
        utils_per_bond: utils_per_bond2,
        utils_per_sat: utils_per_sat2,
        interest_rate: interest_rate2,
        decimals: UTIL_BASE_VALUE,
      }
    );

    let balance0 = (50 * COIN_VALUE * 50 * COIN_VALUE - 100 * COIN_VALUE * COIN_VALUE).sqrt();
    // Convert 20 EASE to sqrt(50^2 - 10^2) TIGHTEN
    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[
        (context.get_block_count() - 1, 1, 0, Witness::new()),
        (context.get_block_count() - 1, 1, 1, Witness::new()),
      ],
      convert: true,
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID1,
              amount: 10 * COIN_VALUE,
              output: 1,
            },
            Edict {
              id: ID0,
              amount: 0,
              output: 1,
            },
            Edict {
              id: ID0,
              amount: 1,
              output: 1,
            },
          ],
          pointer: Some(2),
        }
        .encipher(),
      ),
      ..default()
    });

    context.mine_blocks(1);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: balance0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: 10 * COIN_VALUE,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 1,
        },
        vec![(ID0, balance0), (ID1, 10 * COIN_VALUE)],
      )],
    );

    let rate3 = UTIL_BASE_VALUE * (balance0 - 10 * COIN_VALUE) / (balance0 + 10 * COIN_VALUE);
    let interest_rate3 = (interest_rate2 + rate3) / 2;
    let interest3 = bonds_per_sat2 * interest_rate3 / UTIL_BASE_VALUE / BLOCKS_PER_YEAR;
    let bonds_per_sat3 = bonds_per_sat2 + interest3;
    let utils_per_bond3 = UTIL_BASE_VALUE * UTIL_BASE_VALUE / interest_rate3;
    let utils_per_sat3 = bonds_per_sat3 * utils_per_bond3 / UTIL_BASE_VALUE;

    pretty_assert_eq!(
      context.index.get_util_state().unwrap(),
      api::UtilState {
        bonds_per_sat: bonds_per_sat3,
        utils_per_bond: utils_per_bond3,
        utils_per_sat: utils_per_sat3,
        interest_rate: interest_rate3,
        decimals: UTIL_BASE_VALUE,
      }
    );

    context.mine_blocks(1);

    let interest_rate4 = rate3;
    let interest4 = bonds_per_sat3 * interest_rate4 / UTIL_BASE_VALUE / BLOCKS_PER_YEAR;
    let bonds_per_sat4 = bonds_per_sat3 + interest4;
    let utils_per_bond4 = UTIL_BASE_VALUE * UTIL_BASE_VALUE / interest_rate4;
    let utils_per_sat4 = bonds_per_sat4 * utils_per_bond4 / UTIL_BASE_VALUE;

    pretty_assert_eq!(
      context.index.get_util_state().unwrap(),
      api::UtilState {
        bonds_per_sat: bonds_per_sat4,
        utils_per_bond: utils_per_bond4,
        utils_per_sat: utils_per_sat4,
        interest_rate: interest_rate4,
        decimals: UTIL_BASE_VALUE,
      }
    );

    // Convert to 40 EASE and 30 TIGHTEN
    let balance1 = (balance0 * balance0 + 100 * COIN_VALUE * COIN_VALUE
      - 30 * 30 * COIN_VALUE * COIN_VALUE)
      .sqrt();
    let txid2 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[
        (context.get_block_count() - 2, 1, 0, Witness::new()),
        (context.get_block_count() - 2, 1, 1, Witness::new()),
      ],
      convert: true,
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: 30 * COIN_VALUE,
              output: 1,
            },
            Edict {
              id: ID1,
              amount: 0,
              output: 1,
            },
            Edict {
              id: ID1,
              amount: 1,
              output: 1,
            },
          ],
          pointer: Some(2),
        }
        .encipher(),
      ),
      ..default()
    });

    context.mine_blocks(1);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: 30 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: balance1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid2,
          vout: 1,
        },
        vec![(ID0, 30 * COIN_VALUE), (ID1, balance1)],
      )],
    );

    let interest_rate5 = rate3;
    let interest5 = bonds_per_sat4 * interest_rate5 / UTIL_BASE_VALUE / BLOCKS_PER_YEAR;
    let bonds_per_sat5 = bonds_per_sat4 + interest5;
    let utils_per_bond5 = UTIL_BASE_VALUE * UTIL_BASE_VALUE / interest_rate5;
    let utils_per_sat5 = bonds_per_sat5 * utils_per_bond5 / UTIL_BASE_VALUE;

    pretty_assert_eq!(
      context.index.get_util_state().unwrap(),
      api::UtilState {
        bonds_per_sat: bonds_per_sat5,
        utils_per_bond: utils_per_bond5,
        utils_per_sat: utils_per_sat5,
        interest_rate: interest_rate5,
        decimals: UTIL_BASE_VALUE,
      }
    );
  }
}
