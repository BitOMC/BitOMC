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
    let id0 = RuneId { block: 1, tx: 0 };
    let id1 = RuneId { block: 1, tx: 1 };

    let artifact = Runestone::decipher(tx);

    let mut unallocated = self.unallocated(tx)?;

    let mut allocated: Vec<HashMap<RuneId, Lot>> = vec![HashMap::new(); tx.output.len()];

    let mut converted: HashMap<RuneId, Lot> = HashMap::new();
    let mut allocated_conversion: Vec<HashMap<RuneId, Lot>> = vec![HashMap::new(); tx.output.len()];

    let mut burned: HashMap<RuneId, Lot> = HashMap::new();

    if let Some(artifact) = &artifact {
      if artifact.mint().is_some() {
        if let Some((amount0, amount1)) = self.mint(id0, id1)? {
          *unallocated.entry(id0).or_default() += amount0;
          *unallocated.entry(id1).or_default() += amount1;

          if let Some(sender) = self.event_sender {
            sender.blocking_send(Event::RuneMinted {
              block_height: self.height,
              txid,
              amount0: amount0.n(),
              amount1: amount1.n(),
            })?;
          }
        }
      }

      if let Artifact::Runestone(runestone) = artifact {
        for Edict { id, amount, output } in runestone.edicts.iter().copied() {
          let amount = Lot(amount);

          // edicts with output values greater than the number of outputs
          // should never be produced by the edict parser
          let output = usize::try_from(output).unwrap();
          assert!(output <= tx.output.len());

          let Some(balance) = unallocated.get_mut(&id) else {
            *converted.entry(id).or_default() += amount;
            *allocated_conversion[output].entry(id).or_default() += amount;
            continue;
          };

          let mut allocate = |balance: &mut Lot, amount: Lot, output: usize| {
            if amount > 0 {
              *balance -= amount;
              *allocated[output].entry(id).or_default() += amount;
            }
          };

          if output == tx.output.len() {
            // find non-OP_RETURN outputs
            let destinations = tx
              .output
              .iter()
              .enumerate()
              .filter_map(|(output, tx_out)| {
                (!tx_out.script_pubkey.is_op_return()).then_some(output)
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
                  if amount > *balance {
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
      // OP_RETURN output if there is no default
      if let Some(vout) = pointer
        .map(|pointer| pointer.into_usize())
        .inspect(|&pointer| assert!(pointer < allocated.len()))
        .or_else(|| {
          tx.output
            .iter()
            .enumerate()
            .find(|(_vout, tx_out)| !tx_out.script_pubkey.is_op_return())
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
    
    let mut conversion_output_id: Option<RuneId> = None;
    let mut residual: Option<(RuneId, Lot)> = None;

    // update fields if burned amount and converted amount represent valid conversion
    if burned.entry(id0).or_default().0 > 0 && converted.entry(id1).or_default().0 > 0 {
      let input_amt = burned.entry(id0).or_default();
      let min_output_amt = converted.entry(id1).or_default();
      if let Some(output_amt) = self.convert(id0, id1, *input_amt, *min_output_amt)? {
        // set burned amount to zero if conversion successful and allocate converted amount
        *burned.entry(id0).or_default() = Lot(0);
        conversion_output_id = Some(id1);
        residual = Some((id1, output_amt - *min_output_amt));
      }
    } else if burned.entry(id1).or_default().0 > 0 && converted.entry(id0).or_default().0 > 0  {
      let input_amt = burned.entry(id1).or_default();
      let min_output_amt = converted.entry(id0).or_default();
      if let Some(output_amt) = self.convert(id1, id0, *input_amt, *min_output_amt)? {
        // set burned amount to zero if conversion successful and allocate converted amount
        *burned.entry(id1).or_default() = Lot(0);
        conversion_output_id = Some(id0);
        residual = Some((id0, output_amt - *min_output_amt));
      }
    }

    if let Some(conversion_output_id) = conversion_output_id {
      let mut residual_vout: Option<usize> = None;

        for (vout, balances) in allocated_conversion.into_iter().enumerate() {
          let amount = balances[&conversion_output_id];
          if vout == tx.output.len() {
            // find non-OP_RETURN outputs
            let destinations = tx
            .output
            .iter()
            .enumerate()
            .filter_map(|(vout, tx_out)| {
              (!tx_out.script_pubkey.is_op_return()).then_some(vout)
            })
            .collect::<Vec<usize>>();

            if !destinations.is_empty() {
              // divide amount between eligible outputs
              let sub_amount = amount / destinations.len() as u128;
              let remainder = usize::try_from(amount % destinations.len() as u128).unwrap();

              for (i, output) in destinations.iter().enumerate() {
                let rounded_sub_amount = if i < remainder { sub_amount + 1 } else { sub_amount };
                *allocated[*output].entry(conversion_output_id).or_default() += rounded_sub_amount;

                if residual_vout.is_none() {
                  residual_vout = Some(*output);
                }
              }
            } else {
              *burned.entry(conversion_output_id).or_default() += amount;
            }
          } else {
            *allocated[vout].entry(conversion_output_id).or_default() += amount;

            if residual_vout.is_none() {
              residual_vout = Some(vout);
            }
          }
        }

        // add residual amount to residual vout
        if let Some((residual_id, residual_amt)) = residual {
          if let Some(residual_vout) = residual_vout {
            *allocated[residual_vout].entry(residual_id).or_default() += residual_amt;
          } else {
            *burned.entry(residual_id).or_default() += residual_amt;
          }
        }
    }

    // update outpoint balances
    let mut buffer: Vec<u8> = Vec::new();
    for (vout, balances) in allocated.into_iter().enumerate() {
      if balances.is_empty() {
        continue;
      }

      // increment burned balances
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

    Ok(())
  }

  pub(super) fn update(self) -> Result {
    for (rune_id, burned) in self.burned {
      let mut entry = RuneEntry::load(self.id_to_entry.get(&rune_id.store())?.unwrap().value());
      entry.burned = entry.burned.checked_add(burned.n()).unwrap();
      entry.supply = entry.supply.checked_sub(burned.n()).unwrap();
      self.id_to_entry.insert(&rune_id.store(), entry.store())?;
    }

    Ok(())
  }

  fn mint(&mut self, id0: RuneId, id1: RuneId) -> Result<Option<(Lot, Lot)>> {
    let Some(entry0) = self.id_to_entry.get(&id0.store())? else {
      return Ok(None);
    };
    let Some(entry1) = self.id_to_entry.get(&id1.store())? else {
      return Ok(None);
    };

    let mut rune_entry0 = RuneEntry::load(entry0.value());
    let mut rune_entry1 = RuneEntry::load(entry1.value());

    let reward = self.reward(self.height.into());
    let sum_of_sq =
      rune_entry0.supply * rune_entry0.supply + rune_entry1.supply * rune_entry1.supply;
    let amount0;
    let amount1;
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

    rune_entry0.mints += 1;
    rune_entry1.mints += 1;

    rune_entry0.supply += amount0;
    rune_entry1.supply += amount1;

    self.id_to_entry.insert(&id0.store(), rune_entry0.store())?;
    self.id_to_entry.insert(&id1.store(), rune_entry1.store())?;

    Ok(Some((Lot(amount0), Lot(amount1))))
  }

  fn reward(&self, height: u128) -> u128 {
    let halvings = height / 210000;
    // Force reward to zero when right shift is undefined
    if halvings >= 128 {
      return 0;
    }
    // Cut reward in half every 210,000 blocks
    let reward = 50 * 100000000;
    reward >> halvings
  }

  fn convert(&mut self, input_id: RuneId, output_id: RuneId, input_amt: Lot, min_output_amt: Lot) -> Result<Option<Lot>> {
    let Some(entry0) = self.id_to_entry.get(&input_id.store())? else {
      return Ok(None);
    };
    let Some(entry1) = self.id_to_entry.get(&output_id.store())? else {
      return Ok(None);
    };

    let mut rune_entry0 = RuneEntry::load(entry0.value());
    let mut rune_entry1 = RuneEntry::load(entry1.value());

    if input_amt.0 > rune_entry0.supply {
      return Ok(None);
    }

    let invariant = rune_entry0.supply * rune_entry0.supply + rune_entry1.supply * rune_entry1.supply;
    let new_input_sq = (rune_entry0.supply - input_amt.0) * (rune_entry0.supply - input_amt.0);
    let new_output = (invariant - new_input_sq).sqrt();
    let output_amt = new_output - rune_entry1.supply;

    if output_amt < min_output_amt.0 {
      return Ok(None);
    }

    rune_entry0.supply -= input_amt.0;
    rune_entry1.supply += output_amt;

    Ok(Some(Lot(output_amt)))
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
