use {
  super::*,
  crate::outgoing::Outgoing,
  api::SupplyState,
  base64::Engine,
  bitcoin::{ecdsa, key::Secp256k1, psbt::Psbt, sighash::SighashCache, Denomination, PrivateKey},
  bitcoincore_rpc::{bitcoincore_rpc_json::GetMempoolEntryResult, Error::JsonRpc},
  num_integer::Roots,
  petgraph::{algo::toposort, Directed, Graph},
};

#[derive(Debug, Parser)]
pub(crate) struct Convert {
  #[arg(long, help = "Don't sign or broadcast transaction")]
  pub(crate) dry_run: bool,
  #[arg(long, help = "Use fee rate of <FEE_RATE> sats/vB")]
  fee_rate: FeeRate,
  #[arg(
    long,
    help = "Target <AMOUNT> postage with sent inscriptions. [default: 10000 sat]"
  )]
  pub(crate) postage: Option<Amount>,
  outgoing: Outgoing,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Output {
  pub txid: Txid,
  pub psbt: String,
  pub outgoing: Outgoing,
  pub fee: u64,
}

#[derive(Debug, Clone, PartialEq)]
struct OutPointTxOut {
  pub outpoint: OutPoint,
  pub output: TxOut,
}

impl Convert {
  pub(crate) fn run(self, wallet: Wallet) -> SubcommandResult {
    ensure!(
      wallet.has_rune_index(),
      "sending runes with `ord send` requires index created with `--index-runes` flag",
    );

    wallet.lock_non_cardinal_outputs()?;

    let (unsigned_transaction, unsigned_psbt, fee) = match self.outgoing {
      Outgoing::Rune { decimal, rune } => Self::create_best_unsigned_convert_runes_transaction(
        &wallet,
        rune,
        decimal,
        self.postage.unwrap_or(TARGET_POSTAGE),
        self.fee_rate,
      )?,
      _ => {
        panic!("invalid outgoing")
      }
    };

    let (txid, psbt) = if self.dry_run {
      let psbt = wallet
        .bitcoin_client()
        .wallet_process_psbt(
          &base64::engine::general_purpose::STANDARD.encode(unsigned_psbt.serialize()),
          Some(false),
          None,
          None,
        )?
        .psbt;

      (unsigned_transaction.txid(), psbt)
    } else {
      let psbt = wallet
        .bitcoin_client()
        .wallet_process_psbt(
          &base64::engine::general_purpose::STANDARD.encode(unsigned_psbt.serialize()),
          Some(true),
          None,
          None,
        )?
        .psbt;

      let signed_tx = wallet
        .bitcoin_client()
        .finalize_psbt(&psbt, None)?
        .hex
        .ok_or_else(|| anyhow!("unable to sign transaction"))?;

      (
        wallet.bitcoin_client().send_raw_transaction(&signed_tx)?,
        psbt,
      )
    };

    Ok(Some(Box::new(Output {
      txid,
      psbt,
      outgoing: self.outgoing,
      fee: fee.to_sat(),
    })))
  }

  // returns (input_id, input_amount, min_output_amount)
  fn get_conversion_parameters(
    wallet: &Wallet,
    spaced_rune: SpacedRune,
    decimal: Decimal,
  ) -> Result<(
    Rune,
    Rune,
    RuneId,
    RuneId,
    RuneEntry,
    RuneEntry,
    u128,
    u128,
    SupplyState,
  )> {
    let input_rune = spaced_rune.rune;
    let output_rune = Rune(1 - spaced_rune.rune.n());
    let Some((input_id, rune_entry_in, _)) = wallet.get_rune(input_rune)? else {
      bail!("rune has not been etched");
    };
    let Some((output_id, rune_entry_out, _)) = wallet.get_rune(output_rune)? else {
      bail!("rune has not been etched");
    };

    let input_amt = decimal.to_integer(rune_entry_in.divisibility)?;
    let expected_output_amt =
      Self::get_expected_output(rune_entry_in.supply, rune_entry_out.supply, input_amt);

    let allowable_slippage = 20; // 20bps
    let min_output_amt = expected_output_amt * (10000 - allowable_slippage) / 10000;

    let (entry0, entry1) = if input_id == ID0 {
      (rune_entry_in, rune_entry_out)
    } else {
      (rune_entry_out, rune_entry_in)
    };

    let supply_state = SupplyState {
      supply0: entry0.supply,
      supply1: entry1.supply,
      burned0: entry0.supply,
      burned1: entry1.supply,
    };

    Ok((
      input_rune,
      output_rune,
      input_id,
      output_id,
      rune_entry_in,
      rune_entry_out,
      input_amt,
      min_output_amt,
      supply_state,
    ))
  }

  fn create_unfunded_convert_transaction(
    wallet: &Wallet,
    spaced_rune: SpacedRune,
    decimal: Decimal,
    postage: Amount,
  ) -> Result<Transaction> {
    let (input_rune, output_rune, id_in, id_out, rune_entry_in, _, amount, min_output_amt, _) =
      Self::get_conversion_parameters(wallet, spaced_rune, decimal)?;

    let inscribed_outputs = wallet
      .inscriptions()
      .keys()
      .map(|satpoint| satpoint.outpoint)
      .collect::<HashSet<OutPoint>>();

    let balances = wallet
      .get_runic_outputs()?
      .into_iter()
      .filter(|output| !inscribed_outputs.contains(output))
      .map(|output| {
        wallet.get_runes_balances_in_output(&output).map(|balance| {
          (
            output,
            balance
              .into_iter()
              .map(|(spaced_rune, pile)| (spaced_rune.rune, pile))
              .collect(),
          )
        })
      })
      .collect::<Result<BTreeMap<OutPoint, BTreeMap<Rune, Pile>>>>()?;

    let mut inputs = Vec::new();
    let mut input_rune_balances: BTreeMap<Rune, u128> = BTreeMap::new();
    let mut output_rune_balances: BTreeMap<Rune, u128> = BTreeMap::new();

    for (output, runes) in balances {
      if let Some(input_balance) = runes.get(&input_rune) {
        if input_balance.amount > 0 {
          *input_rune_balances.entry(input_rune).or_default() += input_balance.amount;

          inputs.push(output);
        }
      }

      if let Some(output_balance) = runes.get(&output_rune) {
        if output_balance.amount > 0 {
          *output_rune_balances.entry(output_rune).or_default() += output_balance.amount;
        }
      }

      if input_rune_balances
        .get(&input_rune)
        .cloned()
        .unwrap_or_default()
        >= amount
      {
        break;
      }
    }

    let input_rune_balance = input_rune_balances
      .get(&input_rune)
      .cloned()
      .unwrap_or_default();

    let output_rune_balance = output_rune_balances
      .get(&output_rune)
      .cloned()
      .unwrap_or_default();

    let needs_runes_change_output = input_rune_balance > amount || input_rune_balances.len() > 1;

    ensure! {
      input_rune_balance >= amount,
      "insufficient `{}` balance, only {} in wallet",
      spaced_rune,
      Pile {
        amount: input_rune_balance,
        divisibility: rune_entry_in.divisibility,
        symbol: rune_entry_in.symbol
      },
    }

    let runestone = Runestone {
      edicts: if needs_runes_change_output {
        vec![
          Edict {
            amount: input_rune_balance - amount,
            id: id_in,
            output: 2,
          },
          Edict {
            amount: output_rune_balance + min_output_amt,
            id: id_out,
            output: 2,
          },
        ]
      } else {
        vec![Edict {
          amount: min_output_amt,
          id: id_out,
          output: 2,
        }]
      },
      pointer: Some(0),
    };

    let unfunded_transaction = Transaction {
      version: 2,
      lock_time: LockTime::ZERO,
      input: inputs
        .into_iter()
        .map(|previous_output| TxIn {
          previous_output,
          script_sig: ScriptBuf::new(),
          sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
          witness: Witness::new(),
        })
        .collect(),
      output: vec![
        TxOut {
          script_pubkey: runestone.encipher(),
          value: 0,
        },
        TxOut {
          script_pubkey: Self::get_convert_script(),
          value: 294,
        },
        TxOut {
          script_pubkey: wallet.get_change_address()?.script_pubkey(),
          value: postage.to_sat(),
        },
      ],
    };

    assert_eq!(
      Runestone::decipher(&unfunded_transaction),
      Some(Artifact::Runestone(runestone)),
    );

    Ok(unfunded_transaction)
  }

  fn fund_convert_transaction(
    wallet: &Wallet,
    mut unfunded_transaction: Transaction,
    fee_rate: FeeRate,
    prev_outpoint: Option<OutPointTxOut>,
  ) -> Result<(Transaction, Psbt)> {
    let mut convert_input_vb = 68; // max size of p2wpkh input
    if unfunded_transaction.input.len() == 0 {
      convert_input_vb -= 27; // tx requires 27 fewer bytes if no other inputs
    }
    if prev_outpoint != None {
      // Add fee for conversion input (used solely for fee calculation during funding)
      assert!(unfunded_transaction.output.len() > 0);
      unfunded_transaction.output[0].value += fee_rate.fee(convert_input_vb).to_sat();
    }

    let unsigned_transaction =
      fund_raw_transaction(wallet.bitcoin_client(), fee_rate, &unfunded_transaction)?;

    let mut unsigned_transaction: Transaction =
      consensus::encode::deserialize(&unsigned_transaction)?;

    let unsigned_psbt: Psbt;
    if let Some(prev_outpoint) = prev_outpoint {
      // Deduct input fee from first output
      unsigned_transaction.output[0].value -= fee_rate.fee(convert_input_vb).to_sat();
      // Add conversion input amount to last output (change output or runic if no change)
      let outputs = unsigned_transaction.output.len();
      unsigned_transaction.output[outputs - 1].value += prev_outpoint.output.value;
      // Insert conversion input
      let convert_input = TxIn {
        previous_output: prev_outpoint.outpoint,
        script_sig: ScriptBuf::new(),
        sequence: Sequence::ENABLE_RBF_NO_LOCKTIME,
        witness: Witness::new(),
      };
      unsigned_transaction.input.insert(0, convert_input);
      unsigned_psbt = Self::create_psbt_with_signed_conversion_input(
        unsigned_transaction.clone(),
        prev_outpoint.output,
      )?;
    } else {
      unsigned_psbt = Psbt::from_unsigned_tx(unsigned_transaction.clone())?;
    }

    Ok((unsigned_transaction, unsigned_psbt))
  }

  fn create_psbt_with_signed_conversion_input(tx: Transaction, input_utxo: TxOut) -> Result<Psbt> {
    let secp = Secp256k1::new();
    let privkey = Self::get_convert_script_private_key();
    let pubkey = privkey.public_key(&secp);
    let mut sighash_cache = SighashCache::new(tx.clone());
    let mut psbt = Psbt::from_unsigned_tx(tx.clone())?;
    psbt.inputs[0].witness_utxo = Some(input_utxo);
    let (msg, sighash_ty) = psbt.sighash_ecdsa(0, &mut sighash_cache)?;
    let sig = ecdsa::Signature {
      sig: secp.sign_ecdsa(&msg, &privkey.inner),
      hash_ty: sighash_ty,
    };
    psbt.inputs[0].partial_sigs.insert(pubkey, sig);

    Ok(psbt)
  }

  fn create_best_unsigned_convert_runes_transaction(
    wallet: &Wallet,
    spaced_rune: SpacedRune,
    decimal: Decimal,
    postage: Amount,
    fee_rate: FeeRate,
  ) -> Result<(Transaction, Psbt, Amount)> {
    let (_, _, input_id, _, _, _, input_amount, min_output_amount, initial_state) =
      Self::get_conversion_parameters(wallet, spaced_rune, decimal)?;

    let mut prev_outpoint: Option<OutPointTxOut> = None;
    let last_conversion_outpoint = wallet.get_last_conversion_outpoint()?;
    if last_conversion_outpoint != OutPoint::null() {
      let output = wallet
        .bitcoin_client()
        .get_raw_transaction(&last_conversion_outpoint.txid, None)?
        .output[last_conversion_outpoint.vout as usize]
        .clone();

      prev_outpoint = Some(OutPointTxOut {
        outpoint: last_conversion_outpoint,
        output,
      });
    }

    let unfunded_transaction =
      Self::create_unfunded_convert_transaction(&wallet, spaced_rune, decimal, postage)?;

    let (mut unsigned_transaction, mut unsigned_psbt) = Self::fund_convert_transaction(
      &wallet,
      unfunded_transaction.clone(),
      fee_rate,
      prev_outpoint.clone(),
    )?;

    let fee = Self::get_fee(wallet, unsigned_transaction.clone(), prev_outpoint.clone());

    let Some(prev_outpoint) = prev_outpoint else {
      // No conversion outpoint exists
      // In the future, throw an error if the fee rate is too low for the next block
      return Ok((unsigned_transaction, unsigned_psbt, fee));
    };

    let (txs, entries, outpoints) = Self::find_current_conversion_chain(
      wallet,
      prev_outpoint.clone(),
    )?;

    if txs.len() == 0 {
      // Empty conversion chain means tx will be accepted by mempool using `prev_outpoint`
      return Ok((unsigned_transaction, unsigned_psbt, fee));
    }

    assert!(txs.len() == entries.len());

    let mut outpoints_by_txid = HashMap::new();
    let mut chain = Vec::new();
    for i in 0..(txs.len()) {
      outpoints_by_txid.insert(txs[i].txid(), outpoints[i].clone());
      chain.push((txs[i].clone(), entries[i].clone()));
    }

    let chain_with_ancestors =
      Self::get_conversion_chain_with_ancestors_in_topological_order(wallet, chain)?;

    // Simulate the supply state after each transaction
    let mini_block: Vec<Transaction> = chain_with_ancestors
      .iter()
      .map(|(tx, _)| tx.clone())
      .collect();
    let simulation = wallet.simulate(&mini_block)?;
    assert!(chain_with_ancestors.len() == simulation.len());

    // Create an array consisting of:
    // 1. the conversion transaction entry in the mempool
    // 2. the outpoint it uses
    // 3. the supply state prior to the conversion
    let mut prior_state = initial_state;
    let mut state_chain = Vec::new();
    for (i, state) in simulation.into_iter().enumerate() {
      let (tx, entry) = &chain_with_ancestors[i];
      if let Some(outpoint) = outpoints_by_txid.get(&tx.txid()) {
        state_chain.push((Some(entry.clone()), outpoint.clone(), prior_state));
        prior_state = state;
      }
    }

    // Outpoint array exceeds entries array by one if the final conversion leaves an outpoint
    if outpoints.len() == entries.len() + 1 {
      // Check if adding this transaction would exceed package limit
      let mut exceeds_package_limit = false;
      for entry in &entries {
        if entry.descendant_count >= DESCENDANT_COUNT_LIMIT
          || entry.descendant_size + unsigned_transaction.vsize() as u64 > DESCENDANT_SIZE_LIMIT
        {
          exceeds_package_limit = true;
          break;
        }
      }

      let last_entry = entries[entries.len() - 1].clone();
      if last_entry.ancestor_count >= ANCESTOR_COUNT_LIMIT
        || last_entry.ancestor_size + unsigned_transaction.vsize() as u64 > ANCESTOR_SIZE_LIMIT
      {
        exceeds_package_limit = true;
      }

      if !exceeds_package_limit {
        state_chain.push((None, outpoints[entries.len()].clone(), prior_state));
      }
    }

    let best_outpoint_and_state = Self::get_best_outpoint_in_conversion_chain(
      state_chain,
      input_id,
      input_amount,
      min_output_amount,
      fee,
      unsigned_transaction.vsize(),
    )?;

    let Some((best_outpoint, _)) = best_outpoint_and_state else {
      // Every conversion in mempool results in a state where `min_output_amount` is not
      // a satisfied.
      return Ok((unsigned_transaction, unsigned_psbt, fee));
    };

    // modify transaction using `best_outpoint`
    let outputs = unsigned_transaction.output.len();
    unsigned_transaction.output[outputs - 1].value -= prev_outpoint.output.value;
    unsigned_transaction.output[outputs - 1].value += best_outpoint.output.value;
    unsigned_transaction.input[0].previous_output = best_outpoint.outpoint;
    unsigned_psbt = Self::create_psbt_with_signed_conversion_input(
      unsigned_transaction.clone(),
      best_outpoint.output,
    )?;

    Ok((unsigned_transaction, unsigned_psbt, fee))
  }

  fn find_current_conversion_chain(
    wallet: &Wallet,
    prev_outpoint: OutPointTxOut,
  ) -> Result<(
    Vec<Transaction>,
    Vec<GetMempoolEntryResult>,
    Vec<OutPointTxOut>,
  )> {
    let raw_mempool = wallet.bitcoin_client().get_raw_mempool_verbose()?;
    let potential_spenders = Self::find_potential_spenders(
      wallet,
      prev_outpoint.clone(),
      raw_mempool,
      FeeRate::try_from(1.0)?,
    )?;

    Self::find_conversion_chain(
      wallet,
      prev_outpoint.clone(),
      potential_spenders,
      Self::get_convert_script(),
    )
  }

  // uses unsigned tx based on last outpoint to construct chain of conversions in mempool
  fn find_potential_spenders(
    wallet: &Wallet,
    prev_outpoint: OutPointTxOut,
    raw_mempool: HashMap<Txid, GetMempoolEntryResult>,
    test_fee_rate: FeeRate,
  ) -> Result<Vec<Txid>> {
    let unfunded_test_transaction = Transaction {
      version: 2,
      lock_time: LockTime::ZERO,
      input: Vec::new(),
      output: vec![TxOut {
        script_pubkey: Self::get_convert_script(),
        value: 294,
      }],
    };

    let (funded_tx, _) = Self::fund_convert_transaction(
      &wallet,
      unfunded_test_transaction,
      test_fee_rate,
      Some(prev_outpoint.clone()),
    )?;

    let signed_tx = wallet
      .bitcoin_client()
      .sign_raw_transaction_with_wallet(&funded_tx, None, None)?
      .hex;
    
    // Get conflicting txid
    let test_accept = wallet.bitcoin_client().test_mempool_accept(&[&signed_tx])?[0].clone();
    let Some(reject_reason) = test_accept.reject_reason else {
      return Ok(vec![]);
    };
    if reject_reason != "insufficient fee" {
      return Ok(vec![]);
    }
    let Err(JsonRpc(error)) = wallet.bitcoin_client().send_raw_transaction(&signed_tx) else {
      return Ok(vec![]);
    };
    let error_str = error.to_string();

    if !error_str.contains("rejecting replacement") {
      return Ok(vec![]);
    }

    let mut potential_spenders = Vec::new();
    if error_str.contains("old feerate") {
      // check mempool for entries with this fee rate
      let re = Regex::new(r"old feerate (\d+\.\d+) BTC/kvB").unwrap();
      if let Some(caps) = re.captures(&error_str) {
        if let Some(fee_rate_str) = caps.get(1) {
          let btc_per_kvb = Amount::from_str_in(fee_rate_str.as_str(), Denomination::Bitcoin)?;
          let filtered_mempool: HashMap<Txid, GetMempoolEntryResult> = raw_mempool
            .into_iter()
            .filter(|(_, entry)| {
              entry.depends.is_empty()
                && entry.vsize > 0
                && entry.fees.modified.to_sat() * 1000 / entry.vsize == btc_per_kvb.to_sat()
            })
            .collect();

          if filtered_mempool.keys().len() > 0 {
            // Re-run test transaction with feerate equal to old feerate + 100 sat per kvb
            // This will be rejected because the fee difference is insufficient for relay,
            // but this will filter `potential_spenders` by the original's total fee.
            return Self::find_potential_spenders(
              wallet,
              prev_outpoint.clone(),
              filtered_mempool,
              FeeRate::try_from((btc_per_kvb.to_sat() as f64 + 100.0) / 1000.0)?,
            );
          } else {
            potential_spenders = filtered_mempool.into_iter().map(|(txid, _)| txid).collect();
          }
        }
      }
    } else if error_str.contains("less fees than conflicting txs") {
      // check mempool for entries with this descendant fee
      let re = Regex::new(r"< (\d+\.\d+)").unwrap();
      if let Some(caps) = re.captures(&error_str) {
        if let Some(fee_str) = caps.get(1) {
          let fee = Amount::from_str_in(fee_str.as_str(), Denomination::Bitcoin)?;
          potential_spenders = raw_mempool
            .into_iter()
            .filter(|(_, entry)| entry.fees.descendant == fee)
            .map(|(txid, _)| txid)
            .collect();
        }
      }
    } else if error_str.contains("not enough additional fees to relay") {
      // check mempool for entries with descendant fee equal to replacement fee - fee difference
      let re = Regex::new(r" (\d+\.\d+) <").unwrap();
      if let Some(caps) = re.captures(&error_str) {
        if let Some(fee_str) = caps.get(1) {
          let fee = Self::get_fee(wallet, funded_tx.clone(), Some(prev_outpoint.clone()));
          let fee_difference = Amount::from_str_in(fee_str.as_str(), Denomination::Bitcoin)?;
          potential_spenders = raw_mempool
            .into_iter()
            .filter(|(_, entry)| entry.fees.descendant + fee_difference == fee)
            .map(|(txid, _)| txid)
            .collect();
        }
      }
    }

    Ok(potential_spenders)
  }

  fn find_conversion_chain(
    wallet: &Wallet,
    outpoint: OutPointTxOut,
    potential_spenders: Vec<Txid>,
    convert_script_pubkey: ScriptBuf,
  ) -> Result<(
    Vec<Transaction>,
    Vec<GetMempoolEntryResult>,
    Vec<OutPointTxOut>,
  )> {
    let mut spending_tx: Option<Transaction> = None;
    for txid in potential_spenders {
      let tx = wallet.bitcoin_client().get_raw_transaction(&txid, None)?;
      if tx
        .input
        .iter()
        .any(|vin| vin.previous_output == outpoint.outpoint)
      {
        spending_tx = Some(tx);
        break;
      }
    }

    let Some(spending_tx) = spending_tx else {
      return Ok((vec![], vec![], vec![outpoint]));
    };

    let next_outpoint = spending_tx
      .output
      .iter()
      .enumerate()
      .find(|(_, output)| output.script_pubkey == convert_script_pubkey)
      .map(|(vout, output)| OutPointTxOut {
        outpoint: OutPoint {
          txid: spending_tx.txid(),
          vout: vout as u32,
        },
        output: output.clone(),
      });

    let spending_entry = wallet
      .bitcoin_client()
      .get_mempool_entry(&spending_tx.txid())?;

    if let Some(next_outpoint) = next_outpoint {
      let next_spent_by = spending_entry.clone().spent_by;
      let (txs, entries, outpoints) =
        Self::find_conversion_chain(wallet, next_outpoint, next_spent_by, convert_script_pubkey)?;
      return Ok((
        vec![spending_tx].into_iter().chain(txs).collect(),
        vec![spending_entry].into_iter().chain(entries).collect(),
        vec![outpoint].into_iter().chain(outpoints).collect(),
      ));
    } else {
      return Ok((vec![spending_tx], vec![spending_entry], vec![outpoint]));
    }
  }

  // Returns conversion chain with any ancestors in the mempool in topological order
  fn get_conversion_chain_with_ancestors_in_topological_order(
    wallet: &Wallet,
    chain: Vec<(Transaction, GetMempoolEntryResult)>,
  ) -> Result<Vec<(Transaction, GetMempoolEntryResult)>> {
    let mut txid_to_node = HashMap::new();
    let mut graph: Graph<(Transaction, GetMempoolEntryResult), (), Directed> = Graph::new();
    for i in 0..(chain.len()) {
      txid_to_node.insert(chain[i].0.txid(), graph.add_node(chain[i].clone()));
    }

    let mut queue = VecDeque::from(chain.clone());
    while let Some(tx) = queue.pop_front() {
      let node = txid_to_node.get(&tx.0.txid()).unwrap().clone();
      for ancestor_txid in &tx.1.depends {
        if let Some(ancestor_node) = txid_to_node.get(&ancestor_txid.clone()) {
          graph.add_edge(*ancestor_node, node, ());
        } else {
          let ancestor_tx = wallet
            .bitcoin_client()
            .get_raw_transaction(&ancestor_txid.clone(), None)?;
          let ancestor_entry = wallet
            .bitcoin_client()
            .get_mempool_entry(&ancestor_txid.clone())?;
          let ancestor_node = graph.add_node((ancestor_tx.clone(), ancestor_entry.clone()));
          txid_to_node.insert(ancestor_tx.txid(), ancestor_node);
          graph.add_edge(ancestor_node, node, ());
          queue.push_front((ancestor_tx, ancestor_entry));
        }
      }
    }

    Ok(
      toposort(&graph, None)
        .unwrap()
        .into_iter()
        .filter_map(|index| graph.node_weight(index).cloned())
        .collect(),
    )
  }

  // Returns the outpoint deepest in the chain that leads to a valid conversion.
  // If `fee` is not sufficient to replace any conversion, returns the cheapest
  // outpoint that results in a valid conversion.
  fn get_best_outpoint_in_conversion_chain(
    chain: Vec<(Option<GetMempoolEntryResult>, OutPointTxOut, SupplyState)>,
    input_id: RuneId,
    input: u128,
    min_output: u128,
    fee: Amount,
    size_in_vb: usize,
  ) -> Result<Option<(OutPointTxOut, SupplyState)>> {
    let sats_per_kvb = fee.to_sat() * 1000 / (size_in_vb as u64);
    let replacement_relay_fee_rate = FeeRate::try_from(1.0).unwrap();
    let replacement_relay_fee = replacement_relay_fee_rate.fee(size_in_vb);

    let mut best_replacement = None;

    for i in 0..(chain.len()) {
      let (entry, outpoint, state) = &chain[chain.len() - i - 1];
      if best_replacement != None {
        let entry = entry.clone().unwrap();
        if (entry.vsize > 0 && sats_per_kvb <= entry.fees.modified.to_sat() * 1000 / entry.vsize)
          || fee < entry.fees.descendant + replacement_relay_fee
        {
          break;
        }
      }

      let (input_supply, output_supply) = if input_id == ID0 {
        (state.supply0, state.supply1)
      } else {
        (state.supply1, state.supply0)
      };

      let expected_output = Self::get_expected_output(input_supply, output_supply, input);
      if min_output < expected_output {
        best_replacement = Some((outpoint.clone(), state.clone()));
      }
    }

    Ok(best_replacement)
  }

  fn get_expected_output(input_supply: u128, output_supply: u128, input: u128) -> u128 {
    if input > input_supply {
      return 0;
    }

    let invariant = input_supply * input_supply + output_supply * output_supply;
    let new_input_sq = (input_supply - input) * (input_supply - input);
    return (invariant - new_input_sq).sqrt() - output_supply;
  }

  #[allow(unused)]
  fn get_expected_input(input_supply: u128, output_supply: u128, output: u128) -> u128 {
    let invariant = input_supply * input_supply + output_supply * output_supply;
    let new_output_sq = (output_supply + output) * (output_supply + output);

    if new_output_sq > invariant {
      return u128::MAX;
    }

    return input_supply - (invariant - new_output_sq).sqrt();
  }

  fn get_convert_script_private_key() -> PrivateKey {
    PrivateKey::from_slice(&[1; 32], Network::Bitcoin).unwrap()
  }

  fn get_convert_script() -> ScriptBuf {
    let secp = Secp256k1::new();
    let pubkey = Self::get_convert_script_private_key().public_key(&secp);
    let wpubkey_hash = pubkey.wpubkey_hash().unwrap();
    ScriptBuf::new_v0_p2wpkh(&wpubkey_hash)
  }

  fn get_fee(wallet: &Wallet, tx: Transaction, prev_outpoint: Option<OutPointTxOut>) -> Amount {
    let mut fee = 0;
    let previous_outpoint = prev_outpoint
      .clone()
      .map_or(OutPoint::null(), |prev| prev.outpoint);
    let unspent_outputs = wallet.utxos();
    for txin in tx.input.iter() {
      if let Some(txout) = unspent_outputs.get(&txin.previous_output) {
        fee += txout.value;
      } else if txin.previous_output == previous_outpoint {
        fee += prev_outpoint.clone().unwrap().output.value;
      } else {
        panic!("input {} not found in utxos", txin.previous_output);
      }
    }

    for txout in tx.output.iter() {
      fee = fee.checked_sub(txout.value).unwrap();
    }

    Amount::from_sat(fee)
  }
}
