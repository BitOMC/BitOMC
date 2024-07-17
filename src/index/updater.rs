use {
  self::rune_updater::RuneUpdater,
  super::{fetcher::Fetcher, *},
  futures::future::try_join_all,
  tokio::sync::{
    broadcast::{self, error::TryRecvError},
    mpsc::{self},
  },
};

mod rune_updater;

pub(crate) struct BlockData {
  pub(crate) header: Header,
  pub(crate) txdata: Vec<(Transaction, Txid)>,
}

impl From<Block> for BlockData {
  fn from(block: Block) -> Self {
    BlockData {
      header: block.header,
      txdata: block
        .txdata
        .into_iter()
        .map(|transaction| {
          let txid = transaction.txid();
          (transaction, txid)
        })
        .collect(),
    }
  }
}

pub(crate) struct Updater<'index> {
  pub(super) height: u32,
  pub(super) index: &'index Index,
  pub(super) outputs_cached: u64,
}

impl<'index> Updater<'index> {
  pub(crate) fn update_index(&mut self, mut wtx: WriteTransaction) -> Result {
    let start = Instant::now();
    let starting_height = u32::try_from(self.index.client.get_block_count()?).unwrap() + 1;
    let starting_index_height = self.height;

    wtx
      .open_table(WRITE_TRANSACTION_STARTING_BLOCK_COUNT_TO_TIMESTAMP)?
      .insert(
        &self.height,
        &SystemTime::now()
          .duration_since(SystemTime::UNIX_EPOCH)
          .map(|duration| duration.as_millis())
          .unwrap_or(0),
      )?;

    let mut progress_bar = if cfg!(test)
      || log_enabled!(log::Level::Info)
      || starting_height <= self.height
      || self.index.settings.integration_test()
    {
      None
    } else {
      let progress_bar = ProgressBar::new(starting_height.into());
      progress_bar.set_position(self.height.into());
      progress_bar.set_style(
        ProgressStyle::with_template("[indexing blocks] {wide_bar} {pos}/{len}").unwrap(),
      );
      Some(progress_bar)
    };

    let rx = Self::fetch_blocks_from(self.index, self.height)?;

    let (mut output_sender, mut address_txout_receiver) =
      Self::spawn_fetcher(&self.index.settings)?;

    let mut uncommitted = 0;
    let mut utxo_cache = HashMap::new();
    while let Ok(block) = rx.recv() {
      self.index_block(
        &mut output_sender,
        &mut address_txout_receiver,
        &mut wtx,
        block,
        &mut utxo_cache,
      )?;

      if let Some(progress_bar) = &mut progress_bar {
        progress_bar.inc(1);

        if progress_bar.position() > progress_bar.length().unwrap() {
          if let Ok(count) = self.index.client.get_block_count() {
            progress_bar.set_length(count + 1);
          } else {
            log::warn!("Failed to fetch latest block height");
          }
        }
      }

      uncommitted += 1;

      if uncommitted == self.index.settings.commit_interval() {
        self.commit(wtx, utxo_cache)?;
        utxo_cache = HashMap::new();
        uncommitted = 0;
        wtx = self.index.begin_write()?;
        let height = wtx
          .open_table(HEIGHT_TO_BLOCK_HEADER)?
          .range(0..)?
          .next_back()
          .transpose()?
          .map(|(height, _hash)| height.value() + 1)
          .unwrap_or(0);
        if height != self.height {
          // another update has run between committing and beginning the new
          // write transaction
          break;
        }
        wtx
          .open_table(WRITE_TRANSACTION_STARTING_BLOCK_COUNT_TO_TIMESTAMP)?
          .insert(
            &self.height,
            &SystemTime::now()
              .duration_since(SystemTime::UNIX_EPOCH)?
              .as_millis(),
          )?;
      }

      if SHUTTING_DOWN.load(atomic::Ordering::Relaxed) {
        break;
      }
    }

    if starting_index_height == 0 && self.height > 0 {
      wtx.open_table(STATISTIC_TO_COUNT)?.insert(
        Statistic::InitialSyncTime.key(),
        &u64::try_from(start.elapsed().as_micros())?,
      )?;
    }

    if uncommitted > 0 {
      self.commit(wtx, utxo_cache)?;
    }

    if let Some(progress_bar) = &mut progress_bar {
      progress_bar.finish_and_clear();
    }

    Ok(())
  }

  fn fetch_blocks_from(
    index: &Index,
    mut height: u32,
  ) -> Result<std::sync::mpsc::Receiver<BlockData>> {
    let (tx, rx) = std::sync::mpsc::sync_channel(32);

    let height_limit = index.height_limit;

    let client = index.settings.bitcoin_rpc_client(None)?;

    let first_rune_height = index.first_rune_height;

    thread::spawn(move || loop {
      if let Some(height_limit) = height_limit {
        if height >= height_limit {
          break;
        }
      }

      match Self::get_block_with_retries(&client, height, first_rune_height) {
        Ok(Some(block)) => {
          if let Err(err) = tx.send(block.into()) {
            log::info!("Block receiver disconnected: {err}");
            break;
          }
          height += 1;
        }
        Ok(None) => break,
        Err(err) => {
          log::error!("failed to fetch block {height}: {err}");
          break;
        }
      }
    });

    Ok(rx)
  }

  fn get_block_with_retries(
    client: &Client,
    height: u32,
    first_rune_height: u32,
  ) -> Result<Option<Block>> {
    let mut errors = 0;
    loop {
      match client
        .get_block_hash(height.into())
        .into_option()
        .and_then(|option| {
          option
            .map(|hash| {
              if height >= first_rune_height {
                Ok(client.get_block(&hash)?)
              } else {
                Ok(Block {
                  header: client.get_block_header(&hash)?,
                  txdata: Vec::new(),
                })
              }
            })
            .transpose()
        }) {
        Err(err) => {
          if cfg!(test) {
            return Err(err);
          }

          errors += 1;
          let seconds = 1 << errors;
          log::warn!("failed to fetch block {height}, retrying in {seconds}s: {err}");

          if seconds > 120 {
            log::error!("would sleep for more than 120s, giving up");
            return Err(err);
          }

          thread::sleep(Duration::from_secs(seconds));
        }
        Ok(result) => return Ok(result),
      }
    }
  }

  fn spawn_fetcher(
    settings: &Settings,
  ) -> Result<(mpsc::Sender<OutPoint>, Option<broadcast::Receiver<TxOut>>)> {
    let fetcher = Fetcher::new(settings)?;

    // A block probably has no more than 20k inputs
    const CHANNEL_BUFFER_SIZE: usize = 20_000;

    // Batch 2048 missing inputs at a time, arbitrarily chosen size
    const BATCH_SIZE: usize = 2048;

    let (outpoint_sender, mut outpoint_receiver) = mpsc::channel::<OutPoint>(CHANNEL_BUFFER_SIZE);

    let (txout_sender, _) = broadcast::channel::<TxOut>(CHANNEL_BUFFER_SIZE);

    let address_txout_receiver = if settings.index_addresses() {
      Some(txout_sender.subscribe())
    } else {
      None
    };

    // Default rpcworkqueue in bitcoind is 16, meaning more than 16 concurrent requests will be rejected.
    // Since we are already requesting blocks on a separate thread, and we don't want to break if anything
    // else runs a request, we keep this to 12.
    let parallel_requests: usize = settings.bitcoin_rpc_limit().try_into().unwrap();

    thread::spawn(move || {
      let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();
      rt.block_on(async move {
        loop {
          let Some(outpoint) = outpoint_receiver.recv().await else {
            log::debug!("OutPoint channel closed");
            return;
          };

          // There's no try_iter on tokio::sync::mpsc::Receiver like std::sync::mpsc::Receiver.
          // So we just loop until BATCH_SIZE doing try_recv until it returns None.
          let mut outpoints = vec![outpoint];
          for _ in 0..BATCH_SIZE - 1 {
            let Ok(outpoint) = outpoint_receiver.try_recv() else {
              break;
            };
            outpoints.push(outpoint);
          }

          // Break outputs into chunks for parallel requests
          let chunk_size = (outpoints.len() / parallel_requests) + 1;
          let mut futs = Vec::with_capacity(parallel_requests);
          for chunk in outpoints.chunks(chunk_size) {
            let txids = chunk.iter().map(|outpoint| outpoint.txid).collect();
            let fut = fetcher.get_transactions(txids);
            futs.push(fut);
          }

          let txs = match try_join_all(futs).await {
            Ok(txs) => txs,
            Err(e) => {
              log::error!("Couldn't receive txs {e}");
              return;
            }
          };

          // Send all tx outputs back in order
          for (i, tx) in txs.iter().flatten().enumerate() {
            let Ok(_) =
              txout_sender.send(tx.output[usize::try_from(outpoints[i].vout).unwrap()].clone())
            else {
              log::error!("Value channel closed unexpectedly");
              return;
            };
          }
        }
      })
    });

    Ok((outpoint_sender, address_txout_receiver))
  }

  fn index_block(
    &mut self,
    output_sender: &mut mpsc::Sender<OutPoint>,
    address_txout_receiver: &mut Option<broadcast::Receiver<TxOut>>,
    wtx: &mut WriteTransaction,
    block: BlockData,
    utxo_cache: &mut HashMap<OutPoint, TxOut>,
  ) -> Result<()> {
    Reorg::detect_reorg(&block, self.height, self.index)?;

    log::info!(
      "Block {} at {} with {} transactions…",
      self.height,
      timestamp(block.header.time.into()),
      block.txdata.len()
    );

    let mut outpoint_to_txout = wtx.open_table(OUTPOINT_TO_TXOUT)?;

    if let Some(receiver) = address_txout_receiver {
      assert!(
        matches!(receiver.try_recv(), Err(TryRecvError::Empty)),
        "Previous block did not consume all inputs"
      );
    }

    if self.index.index_addresses {
      // Send all missing input outpoints to be fetched
      let txids = block
        .txdata
        .iter()
        .map(|(_, txid)| txid)
        .collect::<HashSet<_>>();

      for (tx, _) in &block.txdata {
        for input in &tx.input {
          let prev_output = input.previous_output;
          // We don't need coinbase inputs
          if prev_output.is_null() {
            continue;
          }
          // We don't need inputs from txs earlier in the block, since
          // they'll be added to cache when the tx is indexed
          if txids.contains(&prev_output.txid) {
            continue;
          }
          // We don't need inputs we already have in our cache from earlier blocks
          if utxo_cache.contains_key(&prev_output) {
            continue;
          }
          // We don't need inputs we already have in our database
          if outpoint_to_txout.get(&prev_output.store())?.is_some() {
            continue;
          }
          // Send this outpoint to background thread to be fetched
          output_sender.blocking_send(prev_output)?;
        }
      }
    }

    if let Some(address_txout_receiver) = address_txout_receiver {
      let mut script_pubkey_to_outpoint = wtx.open_multimap_table(SCRIPT_PUBKEY_TO_OUTPOINT)?;
      for (tx, txid) in &block.txdata {
        self.index_transaction_output_script_pubkeys(
          tx,
          txid,
          address_txout_receiver,
          utxo_cache,
          &mut script_pubkey_to_outpoint,
          &mut outpoint_to_txout,
        )?;
      }
    };

    let mut height_to_block_header = wtx.open_table(HEIGHT_TO_BLOCK_HEADER)?;

    if self.index.index_runes && self.height >= self.index.settings.first_rune_height() {
      let mut outpoint_to_rune_balances = wtx.open_table(OUTPOINT_TO_RUNE_BALANCES)?;
      let mut rune_id_to_rune_entry = wtx.open_table(RUNE_ID_TO_RUNE_ENTRY)?;
      let mut state_change_to_last_outpoint = wtx.open_table(STATE_CHANGE_TO_LAST_OUTPOINT)?;
      let mut util_entry_table = wtx.open_table(UTIL_ENTRY)?;

      let mut rune_updater = RuneUpdater {
        event_sender: self.index.event_sender.as_ref(),
        burned: HashMap::new(),
        height: self.height,
        id_to_entry: &mut rune_id_to_rune_entry,
        outpoint_to_balances: &mut outpoint_to_rune_balances,
        state_change_to_last_outpoint: &mut state_change_to_last_outpoint,
        require_conversion_outpoint: true,
      };

      for (tx, txid) in block.txdata.iter() {
        rune_updater.index_runes(tx, *txid)?;
      }

      rune_updater.update()?;

      if let Some(state) = rune_updater.get_state()? {
        let mut util_entry = UtilEntry::load(util_entry_table.get(0)?.unwrap().value());
        util_entry.update(state.supply0, state.supply1);
        util_entry_table.insert(0, util_entry.store())?;
      }
    }

    height_to_block_header.insert(&self.height, &block.header.store())?;

    self.height += 1;

    Ok(())
  }

  fn index_transaction_output_script_pubkeys(
    &mut self,
    tx: &Transaction,
    txid: &Txid,
    txout_receiver: &mut broadcast::Receiver<TxOut>,
    utxo_cache: &mut HashMap<OutPoint, TxOut>,
    script_pubkey_to_outpoint: &mut MultimapTable<&[u8], OutPointValue>,
    outpoint_to_txout: &mut Table<&OutPointValue, TxOutValue>,
  ) -> Result {
    for txin in &tx.input {
      let output = txin.previous_output;
      if output.is_null() {
        continue;
      }

      // multi-level cache for UTXO set to get to the script pubkey
      let txout = if let Some(txout) = utxo_cache.get(&txin.previous_output) {
        txout.clone()
      } else if let Some(value) = outpoint_to_txout.get(&txin.previous_output.store())? {
        TxOut::load(value.value())
      } else {
        txout_receiver.blocking_recv().map_err(|err| {
          anyhow!(
            "failed to get transaction for {}: {err}",
            txin.previous_output.txid
          )
        })?
      };

      utxo_cache.remove(&output);
      outpoint_to_txout.remove(&output.store())?;
      script_pubkey_to_outpoint.remove(&txout.script_pubkey.as_bytes(), output.store())?;
    }

    for (vout, txout) in tx.output.iter().enumerate() {
      let vout: u32 = vout.try_into().unwrap();
      script_pubkey_to_outpoint.insert(
        txout.script_pubkey.as_bytes(),
        OutPoint { txid: *txid, vout }.store(),
      )?;

      utxo_cache.insert(OutPoint { txid: *txid, vout }, txout.clone());
    }

    Ok(())
  }

  fn commit(&mut self, wtx: WriteTransaction, utxo_cache: HashMap<OutPoint, TxOut>) -> Result {
    log::info!(
      "Committing at block height {}, {} outputs cached",
      self.height,
      self.outputs_cached
    );

    {
      log::info!("Flushing utxo cache with {} entries", utxo_cache.len());

      let mut outpoint_to_txout = wtx.open_table(OUTPOINT_TO_TXOUT)?;

      for (outpoint, txout) in utxo_cache {
        outpoint_to_txout.insert(&outpoint.store(), txout.store())?;
      }
    }

    Index::increment_statistic(&wtx, Statistic::Commits, 1)?;
    wtx.commit()?;

    Reorg::update_savepoints(self.index, self.height)?;

    Ok(())
  }

  pub fn simulate(
    wtx: WriteTransaction,
    index: &'index Index,
    height: u32,
    transactions: Vec<Transaction>,
  ) -> Result<Vec<api::SupplyState>> {
    if !index.index_runes && height < index.settings.first_rune_height() {
      return Ok(Vec::new());
    }

    let mut id_to_entry = wtx.open_table(RUNE_ID_TO_RUNE_ENTRY)?;
    let mut outpoint_to_balances = wtx.open_table(OUTPOINT_TO_RUNE_BALANCES)?;
    let mut state_change_to_last_outpoint = wtx.open_table(STATE_CHANGE_TO_LAST_OUTPOINT)?;

    let mut rune_updater = RuneUpdater {
      event_sender: None,
      burned: HashMap::new(),
      height,
      id_to_entry: &mut id_to_entry,
      outpoint_to_balances: &mut outpoint_to_balances,
      state_change_to_last_outpoint: &mut state_change_to_last_outpoint,
      require_conversion_outpoint: true,
    };

    let mut states = Vec::new();
    for tx in transactions {
      rune_updater.index_runes(&tx, tx.txid())?;

      if let Some(state) = rune_updater.get_state()? {
        states.push(state);
      }
    }

    Ok(states)
  }
}
