use {super::*, num_integer::Roots};

pub(super) struct RuneUpdater<'a, 'tx> {
  pub(super) burned: HashMap<RuneId, Lot>,
  pub(super) event_sender: Option<&'a mpsc::Sender<Event>>,
  pub(super) height: u32,
  pub(super) id_to_entry: &'a mut Table<'tx, RuneIdValue, RuneEntryValue>,
  pub(super) outpoint_to_balances: &'a mut Table<'tx, &'static OutPointValue, &'static [u8]>,
}

impl<'a, 'tx> RuneUpdater<'a, 'tx> {
  pub(super) fn index_runes(&mut self, tx: &Transaction, txid: Txid) -> Result<()> {
    let artifact = Runestone::decipher(tx);

    let mut unallocated = self.unallocated(tx)?;

    let mut allocated: Vec<HashMap<RuneId, Lot>> = vec![HashMap::new(); tx.output.len()];

    let mut last_id: Option<RuneId> = None;
    let mut converted: HashMap<RuneId, Lot> = HashMap::new();
    let mut allocated_conversion: Vec<HashMap<RuneId, Lot>> = vec![HashMap::new(); tx.output.len()];

    let mut burned: HashMap<RuneId, Lot> = HashMap::new();
    let mut edicts: Vec<Edict> = Vec::new();
    let mut contains_mint = false;

    self.update_status_of_last_mint_output(tx)?;

    if let Some(artifact) = &artifact {
      if let Some((amount0, amount1)) = self.mint(tx, txid)? {
        *unallocated.entry(ID0).or_default() += amount0;
        *unallocated.entry(ID1).or_default() += amount1;
        contains_mint = true;

        if let Some(sender) = self.event_sender {
          sender.blocking_send(Event::RuneMinted {
            block_height: self.height,
            txid,
            amount0: amount0.n(),
            amount1: amount1.n(),
          })?;
        }
      }

      if let Artifact::Runestone(runestone) = artifact {
        edicts.clone_from(&runestone.edicts);
        for Edict { id, amount, output } in runestone.edicts.iter().copied() {
          let amount = Lot(amount);
          last_id = Some(id);

          // edicts with output values greater than the number of outputs
          // should never be produced by the edict parser
          let output = usize::try_from(output).unwrap();
          assert!(output <= tx.output.len());

          let Some(balance) = unallocated.get_mut(&id) else {
            if amount > 0 {
              if output < tx.output.len() {
                *allocated_conversion[output].entry(id).or_default() += amount;
                *converted.entry(id).or_default() += amount;
              } else {
                // find non-OP_RETURN and non-mint outputs
                let destinations = tx
                  .output
                  .iter()
                  .enumerate()
                  .filter_map(|(output, tx_out)| {
                    (!(tx_out.script_pubkey.is_op_return() || contains_mint && output == 0))
                      .then_some(output)
                  })
                  .collect::<Vec<usize>>();

                if !destinations.is_empty() {
                  for output in &destinations {
                    *allocated_conversion[*output].entry(id).or_default() += amount;
                  }
                  *converted.entry(id).or_default() +=
                    amount * destinations.len().try_into().unwrap();
                }
              }
            }
            continue;
          };

          let mut allocate = |balance: &mut Lot, amount: Lot, output: usize| {
            if amount > 0 {
              *balance -= amount;
              *allocated[output].entry(id).or_default() += amount;
            }
          };

          if output == tx.output.len() {
            // find non-OP_RETURN and non-mint outputs
            let destinations = tx
              .output
              .iter()
              .enumerate()
              .filter_map(|(output, tx_out)| {
                (!(tx_out.script_pubkey.is_op_return() || contains_mint && output == 0))
                  .then_some(output)
              })
              .collect::<Vec<usize>>();

            if !destinations.is_empty() {
              if amount == 0 {
                // if amount is zero, divide balance between eligible outputs
                let amount = *balance / destinations.len() as u128;
                let remainder = usize::try_from(*balance % destinations.len() as u128).unwrap();

                for (i, output) in destinations.iter().enumerate() {
                  allocate(
                    balance,
                    if i < remainder { amount + 1 } else { amount },
                    *output,
                  );
                }
              } else {
                // if amount is non-zero, distribute amount to eligible outputs
                for output in destinations {
                  if *balance > 0 && amount > *balance {
                    // if amount exceeds balance, add remaining amount to (potential) conversion output amount
                    *converted.entry(id).or_default() += amount - *balance;
                    *allocated_conversion[output].entry(id).or_default() += amount - *balance;
                  }
                  allocate(balance, amount.min(*balance), output);
                }
              }
            }
          } else {
            // if amount exceeds balance, add remaining amount to (potential) conversion output amount
            if amount > *balance {
              *converted.entry(id).or_default() += amount - *balance;
              *allocated_conversion[output].entry(id).or_default() += amount - *balance;
            }

            // Get the allocatable amount
            let amount = if amount == 0 {
              *balance
            } else {
              amount.min(*balance)
            };

            allocate(balance, amount, output);
          }
        }
      }
    }

    if let Some(Artifact::Cenotaph(_)) = artifact {
      for (id, balance) in unallocated {
        *burned.entry(id).or_default() += balance;
      }
    } else {
      let pointer = artifact
        .map(|artifact| match artifact {
          Artifact::Runestone(runestone) => runestone.pointer,
          Artifact::Cenotaph(_) => unreachable!(),
        })
        .unwrap_or_default();

      // assign all un-allocated runes to the default output, or the first non
      // OP_RETURN and non-mint output if there is no default
      if let Some(vout) = pointer
        .map(|pointer| pointer.into_usize())
        .inspect(|&pointer| assert!(pointer < allocated.len()))
        .or_else(|| {
          tx.output
            .iter()
            .enumerate()
            .find(|(_vout, tx_out)| {
              !(tx_out.script_pubkey.is_op_return() || contains_mint && *_vout == 0)
            })
            .map(|(vout, _tx_out)| vout)
        })
      {
        for (id, balance) in unallocated {
          if balance > 0 {
            *allocated[vout].entry(id).or_default() += balance;
          }
        }
      } else {
        for (id, balance) in unallocated {
          if balance > 0 {
            *burned.entry(id).or_default() += balance;
          }
        }
      }
    }

    // increment burned balances
    for (vout, balances) in allocated.clone().into_iter().enumerate() {
      if !balances.is_empty() && tx.output[vout].script_pubkey.is_op_return() {
        for (id, balance) in &balances {
          *burned.entry(*id).or_default() += *balance;
          // zero out allocation so that burned balance does not increment a second time
          *allocated[vout].entry(*id).or_default() = Lot(0);
        }
      }
    }

    // check if this transaction contains a conversion
    let input_id: Option<RuneId>;
    let output_id: Option<RuneId>;
    if burned.entry(ID0).or_default().0 > 0 && converted.entry(ID1).or_default().0 > 0 {
      input_id = Some(ID0);
      output_id = Some(ID1);
    } else if burned.entry(ID1).or_default().0 > 0 && converted.entry(ID0).or_default().0 > 0 {
      input_id = Some(ID1);
      output_id = Some(ID0);
    } else {
      input_id = None;
      output_id = None;
    }

    if let (Some(input_id), Some(output_id), Some(residual_id)) = (input_id, output_id, last_id) {
      if residual_id == output_id {
        // convert exact input
        let input_amt = burned.entry(input_id).or_default();
        let min_output_amt = converted.entry(output_id).or_default();
        if let Some(output_amt) =
          self.convert_exact_input(input_id, output_id, *input_amt, *min_output_amt)?
        {
          // undo burned entry if conversion successful
          *burned.entry(input_id).or_default() = Lot(0);

          // allocate conversion outputs and assign residual output
          let mut residual_vout: Option<usize> = None;
          for (vout, balances) in allocated_conversion.clone().into_iter().enumerate() {
            for (id, balance) in &balances {
              if *id != output_id {
                continue;
              }

              // conversion output values greater than or equal to the number of outputs
              // should never be produced by the initial edict scan
              assert!(vout < tx.output.len());

              *allocated[vout].entry(*id).or_default() += *balance;

              // residual output is first conversion output
              if residual_vout.is_none() {
                residual_vout = Some(vout);
              }
            }
          }

          // add residual amount to residual vout
          if output_amt > *min_output_amt {
            if let Some(residual_vout) = residual_vout {
              *allocated[residual_vout].entry(output_id).or_default() +=
                output_amt - *min_output_amt;
            } else {
              *burned.entry(output_id).or_default() += output_amt - *min_output_amt;
            }
          }
        }
      } else {
        // convert exact output
        let max_input_amt = burned.entry(input_id).or_default();
        let output_amt = converted.entry(output_id).or_default();
        if let Some(input_amt) =
          self.convert_exact_output(input_id, output_id, *output_amt, *max_input_amt)?
        {
          // allocate conversion outputs
          for (vout, balances) in allocated_conversion.clone().into_iter().enumerate() {
            for (id, balance) in &balances {
              if *id != output_id {
                continue;
              }

              // conversion output values greater than or equal to the number of outputs
              // should never be produced by the initial edict scan
              assert!(vout < tx.output.len());

              *allocated[vout].entry(*id).or_default() += *balance;
            }
          }

          // assign residual to input balance by adding it to burned entry
          *burned.entry(input_id).or_default() = *max_input_amt - input_amt;
        }
      }

      // add burned entry back to input balance
      if burned.entry(input_id).or_default().0 > 0 {
        // allocate input amount to output of first edict of input_id with valid output
        for Edict { id, output, .. } in edicts.iter().copied() {
          let output = usize::try_from(output).unwrap();
          if id == input_id {
            // allocate only if output is valid
            if output < tx.output.len() {
              *allocated[output].entry(input_id).or_default() +=
                *burned.entry(input_id).or_default();
              *burned.entry(input_id).or_default() = Lot(0);
            }
            // NOTE: If output equals length, we allocate to first non-OP_RETURN and non-mint output.
            // Since inputs have been burnt and the first input edict has an output equal to length,
            // we know the first non-OP_RETURN and non-mint output has a balance.
            // Therefore, it is safe to break this loop. We will find this output in the next loop.
            break;
          }
        }

        // if unallocated, allocate to first output with non-zero balance for input_id
        if burned.entry(input_id).or_default().0 > 0 {
          let mut cloned_allocated = allocated.clone();
          for vout in 0..tx.output.len() {
            if cloned_allocated[vout].entry(input_id).or_default().0 > 0 {
              *allocated[vout].entry(input_id).or_default() += *burned.entry(input_id).or_default();
              *burned.entry(input_id).or_default() = Lot(0);
              break;
            }
          }
        }

        // if unallocated, allocate input amount to output of first edict
        if burned.entry(input_id).or_default().0 > 0 {
          let mut output = usize::try_from(edicts[0].output).unwrap();
          if output == tx.output.len() {
            // find first non-OP_RETURN and non-mint output
            let destinations = tx
              .output
              .iter()
              .enumerate()
              .filter_map(|(output, tx_out)| {
                (!(tx_out.script_pubkey.is_op_return() || contains_mint && output == 0))
                  .then_some(output)
              })
              .collect::<Vec<usize>>();

            if !destinations.is_empty() {
              output = destinations[0];
            }
          }

          if output < tx.output.len() {
            *allocated[output].entry(input_id).or_default() += *burned.entry(input_id).or_default();
            *burned.entry(input_id).or_default() = Lot(0);
          }
        }
      }
    }

    // update outpoint balances
    let mut buffer: Vec<u8> = Vec::new();
    for (vout, balances) in allocated.into_iter().enumerate() {
      if balances.is_empty() {
        continue;
      }

      // increment burned balances created by conversion
      if tx.output[vout].script_pubkey.is_op_return() {
        for (id, balance) in &balances {
          *burned.entry(*id).or_default() += *balance;
        }
        continue;
      }

      buffer.clear();

      let mut balances = balances.into_iter().collect::<Vec<(RuneId, Lot)>>();

      // Sort balances by id so tests can assert balances in a fixed order
      balances.sort();

      let outpoint = OutPoint {
        txid,
        vout: vout.try_into().unwrap(),
      };

      for (id, balance) in balances {
        Index::encode_rune_balance(id, balance.n(), &mut buffer);

        if let Some(sender) = self.event_sender {
          sender.blocking_send(Event::RuneTransferred {
            outpoint,
            block_height: self.height,
            txid,
            rune_id: id,
            amount: balance.0,
          })?;
        }
      }

      self
        .outpoint_to_balances
        .insert(&outpoint.store(), buffer.as_slice())?;
    }

    // increment entries with burned runes
    for (id, amount) in burned {
      if amount > 0 {
        *self.burned.entry(id).or_default() += amount;

        if let Some(sender) = self.event_sender {
          sender.blocking_send(Event::RuneBurned {
            block_height: self.height,
            txid,
            rune_id: id,
            amount: amount.n(),
          })?;
        }
      }
    }

    Ok(())
  }

  pub(super) fn update(self) -> Result {
    for (rune_id, burned) in self.burned {
      let mut entry = RuneEntry::load(self.id_to_entry.get(&rune_id.store())?.unwrap().value());
      entry.burned = entry.burned.checked_add(burned.n()).unwrap();
      self.id_to_entry.insert(&rune_id.store(), entry.store())?;
    }

    Ok(())
  }

  fn update_status_of_last_mint_output(&mut self, tx: &Transaction) -> Result {
    let Some(entry0) = self.id_to_entry.get(&ID0.store())? else {
      return Ok(());
    };
    let Some(entry1) = self.id_to_entry.get(&ID1.store())? else {
      return Ok(());
    };

    let mut rune_entry0 = RuneEntry::load(entry0.value());
    let mut rune_entry1 = RuneEntry::load(entry1.value());

    // Skip if outpoint of last mint is spent
    if rune_entry0.etching == Txid::all_zeros() {
      return Ok(());
    }

    // Skip if outpoint of last mint is not an input to this transaction
    for input in &tx.input {
      if input.previous_output.txid == rune_entry0.etching && input.previous_output.vout == 0 {
        drop(entry0);
        drop(entry1);

        // Update last mint txid so that we know it's been spent
        rune_entry0.etching = Txid::all_zeros();
        rune_entry1.etching = Txid::all_zeros();

        self.id_to_entry.insert(&ID0.store(), rune_entry0.store())?;
        self.id_to_entry.insert(&ID1.store(), rune_entry1.store())?;

        break;
      }
    }

    Ok(())
  }

  fn mint(&mut self, tx: &Transaction, txid: Txid) -> Result<Option<(Lot, Lot)>> {
    // First output must have p2wsh for 1 CHECKSEQUENCEVERIFY (anyone can spend after 1 block)
    let mint_script = ScriptBuf::from_bytes(Vec::from(&[0x51, 0xb2]));
    if tx.output[0].script_pubkey != ScriptBuf::new_v0_p2wsh(&mint_script.wscript_hash())
    {
      return Ok(None);
    }

    let Some(entry0) = self.id_to_entry.get(&ID0.store())? else {
      return Ok(None);
    };
    let Some(entry1) = self.id_to_entry.get(&ID1.store())? else {
      return Ok(None);
    };

    let mut rune_entry0 = RuneEntry::load(entry0.value());
    let mut rune_entry1 = RuneEntry::load(entry1.value());

    // Outpoint of last mint must be spent (either by this tx or a previous one)
    if rune_entry0.etching != Txid::all_zeros() {
      return Ok(None);
    }

    // Reward must be non-zero
    let reward = rune_entry0.mintable(self.height.into());
    if reward == 0 {
      return Ok(None);
    }

    let sum_of_sq =
      rune_entry0.supply * rune_entry0.supply + rune_entry1.supply * rune_entry1.supply;
    let mut amount0;
    let mut amount1;
    if sum_of_sq == 0 {
      // Assign entire reward to amount0
      amount0 = reward;
      amount1 = 0;
    } else {
      // Split reward between runes such that converted supply increases by `reward`
      let k = sum_of_sq.sqrt();
      amount0 = rune_entry0.supply * reward / k;
      amount1 = rune_entry1.supply * reward / k;
    }

    drop(entry0);
    drop(entry1);

    rune_entry0.mints = (self.height as u128) + 1 - (rune_entry0.block as u128);
    rune_entry1.mints = rune_entry0.mints;

    rune_entry0.supply += amount0;
    rune_entry1.supply += amount1;

    // Assign any burned amounts
    if rune_entry0.burned > 0 {
      amount0 += rune_entry0.burned;
      rune_entry0.burned = 0;
    }
    if rune_entry1.burned > 0 {
      amount1 += rune_entry1.burned;
      rune_entry1.burned = 0;
    }

    // Update last mint txid
    rune_entry0.etching = txid;
    rune_entry1.etching = txid;

    self.id_to_entry.insert(&ID0.store(), rune_entry0.store())?;
    self.id_to_entry.insert(&ID1.store(), rune_entry1.store())?;

    Ok(Some((Lot(amount0), Lot(amount1))))
  }

  fn convert_exact_input(
    &mut self,
    input_id: RuneId,
    output_id: RuneId,
    input_amt: Lot,
    min_output_amt: Lot,
  ) -> Result<Option<Lot>> {
    let Some(entry_in) = self.id_to_entry.get(&input_id.store())? else {
      return Ok(None);
    };
    let Some(entry_out) = self.id_to_entry.get(&output_id.store())? else {
      return Ok(None);
    };

    let mut rune_entry_in = RuneEntry::load(entry_in.value());
    let mut rune_entry_out = RuneEntry::load(entry_out.value());

    if input_amt.0 > rune_entry_in.supply {
      return Ok(None);
    }

    let invariant =
      rune_entry_in.supply * rune_entry_in.supply + rune_entry_out.supply * rune_entry_out.supply;
    let new_input_sq = (rune_entry_in.supply - input_amt.0) * (rune_entry_in.supply - input_amt.0);
    let output_amt = (invariant - new_input_sq).sqrt() - rune_entry_out.supply;

    if output_amt < min_output_amt.0 {
      return Ok(None);
    }

    drop(entry_in);
    drop(entry_out);

    rune_entry_in.supply -= input_amt.0;
    rune_entry_out.supply += output_amt;

    self
      .id_to_entry
      .insert(&input_id.store(), rune_entry_in.store())?;
    self
      .id_to_entry
      .insert(&output_id.store(), rune_entry_out.store())?;

    Ok(Some(Lot(output_amt)))
  }

  fn convert_exact_output(
    &mut self,
    input_id: RuneId,
    output_id: RuneId,
    output_amt: Lot,
    max_input_amt: Lot,
  ) -> Result<Option<Lot>> {
    let Some(entry_in) = self.id_to_entry.get(&input_id.store())? else {
      return Ok(None);
    };
    let Some(entry_out) = self.id_to_entry.get(&output_id.store())? else {
      return Ok(None);
    };

    let mut rune_entry_in = RuneEntry::load(entry_in.value());
    let mut rune_entry_out = RuneEntry::load(entry_out.value());

    let invariant =
      rune_entry_in.supply * rune_entry_in.supply + rune_entry_out.supply * rune_entry_out.supply;
    let new_output_sq =
      (rune_entry_out.supply + output_amt.0) * (rune_entry_out.supply + output_amt.0);

    if new_output_sq > invariant {
      return Ok(None);
    }

    let input_amt = rune_entry_in.supply - (invariant - new_output_sq).sqrt();

    if input_amt > max_input_amt.0 {
      return Ok(None);
    }

    drop(entry_in);
    drop(entry_out);

    rune_entry_in.supply -= input_amt;
    rune_entry_out.supply += output_amt.0;

    self
      .id_to_entry
      .insert(&input_id.store(), rune_entry_in.store())?;
    self
      .id_to_entry
      .insert(&output_id.store(), rune_entry_out.store())?;

    Ok(Some(Lot(input_amt)))
  }

  fn unallocated(&mut self, tx: &Transaction) -> Result<HashMap<RuneId, Lot>> {
    // map of rune ID to un-allocated balance of that rune
    let mut unallocated: HashMap<RuneId, Lot> = HashMap::new();

    // increment unallocated runes with the runes in tx inputs
    for input in &tx.input {
      if let Some(guard) = self
        .outpoint_to_balances
        .remove(&input.previous_output.store())?
      {
        let buffer = guard.value();
        let mut i = 0;
        while i < buffer.len() {
          let ((id, balance), len) = Index::decode_rune_balance(&buffer[i..]).unwrap();
          i += len;
          *unallocated.entry(id).or_default() += balance;
        }
      }
    }

    Ok(unallocated)
  }
}
