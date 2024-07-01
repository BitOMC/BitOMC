use super::*;

#[derive(Debug, PartialEq)]
pub enum MintError {
  Cap(u128),
  End(u64),
  Start(u64),
  Unmintable,
}

impl Display for MintError {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      MintError::Cap(cap) => write!(f, "limited to {cap} mints"),
      MintError::End(end) => write!(f, "mint ended on block {end}"),
      MintError::Start(start) => write!(f, "mint starts on block {start}"),
      MintError::Unmintable => write!(f, "not mintable"),
    }
  }
}

#[cfg(test)]
mod tests {
  use {super::*, crate::index::testing::Context, num_integer::Roots};

  const TIGHTEN: u128 = 0;
  const EASE: u128 = 1;

  const ID0: RuneId = RuneId { block: 1, tx: 0 };
  const ID1: RuneId = RuneId { block: 1, tx: 1 };

  const COIN_VALUE: u128 = 100000000;

  #[test]
  fn index_starts_with_runes() {
    let context = Context::builder().arg("--index-runes").build();
    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            ..default()
          },
        ),
      ],
      [],
    );
  }

  #[test]
  fn input_runes_may_be_allocated() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![Edict {
            id: ID0,
            amount: 50 * COIN_VALUE,
            output: 0,
          }],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
  }

  #[test]
  fn input_runes_are_burned_if_an_unrecognized_even_tag_is_encountered() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      op_return: Some(
        Runestone {
          pointer: Some(10),
          ..default()
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
            divisibility: 8,
            burned: 50 * COIN_VALUE,
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
            divisibility: 8,
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
      [],
    );
  }

  #[test]
  fn unallocated_runes_are_assigned_to_first_non_op_return_output() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      op_return: Some(Runestone::default().encipher()),
      ..default()
    });

    context.mine_blocks(1);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            divisibility: 8,
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
            divisibility: 8,
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
  }

  #[test]
  fn unallocated_runes_are_burned_if_no_non_op_return_output_is_present() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      op_return: Some(Runestone::default().encipher()),
      outputs: 0,
      ..default()
    });

    context.mine_blocks(1);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            divisibility: 8,
            burned: 50 * COIN_VALUE,
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
            divisibility: 8,
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
      [],
    );
  }

  #[test]
  fn unallocated_runes_are_assigned_to_default_output() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          pointer: Some(1),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 1,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );
  }

  #[test]
  fn unallocated_runes_are_burned_if_default_output_is_op_return() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            burned: 50 * COIN_VALUE,
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
            divisibility: 8,
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
      [],
    );
  }

  #[test]
  fn unallocated_runes_in_transactions_with_no_runestone_are_assigned_to_first_non_op_return_output(
  ) {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      op_return: None,
      ..default()
    });

    context.mine_blocks(1);

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            divisibility: 8,
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
            divisibility: 8,
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
  }

  #[test]
  fn convert_exact_input() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 20 TIGHTEN to at least 30 EASE (expect 40 EASE)
    let supply0 = 50 * COIN_VALUE;
    let input_amt = 20 * COIN_VALUE;
    let min_output_amt = 30 * COIN_VALUE;

    let expected_balance0 = supply0 - input_amt;
    let expected_balance1 = (supply0 * supply0 - expected_balance0 * expected_balance0).sqrt();

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: expected_balance0,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: min_output_amt,
              output: 0,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, expected_balance0), (ID1, expected_balance1)],
      )],
    );

    // Convert 10 EASE to at least 5 TIGHTEN (expect 10 TIGHTEN)
    let input_amt_2 = 10 * COIN_VALUE;
    let min_output_amt_2 = 5 * COIN_VALUE;

    let expected_balance1_2 = expected_balance1 - input_amt_2;
    let expected_balance0_2 = (expected_balance0 * expected_balance0
      + expected_balance1 * expected_balance1
      - expected_balance1_2 * expected_balance1_2)
      .sqrt();

    let txid2 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID1,
              amount: expected_balance1_2,
              output: 0,
            },
            Edict {
              id: ID0,
              amount: expected_balance0 + min_output_amt_2,
              output: 0,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance0_2,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance1_2,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid2,
          vout: 0,
        },
        vec![(ID0, expected_balance0_2), (ID1, expected_balance1_2)],
      )],
    );
  }

  #[test]
  fn convert_exact_output() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert to exactly 30 EASE with at most 20 TIGHTEN (expect 10 TIGHTEN)
    let supply0 = 50 * COIN_VALUE;
    let max_input_amt = 20 * COIN_VALUE;
    let output_amt = 30 * COIN_VALUE;

    let expected_balance1 = output_amt;
    let expected_balance0 = (supply0 * supply0 - expected_balance1 * expected_balance1).sqrt();

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID1,
              amount: expected_balance1,
              output: 0,
            },
            Edict {
              id: ID0,
              amount: supply0 - max_input_amt,
              output: 0,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, expected_balance0), (ID1, expected_balance1)],
      )],
    );
  }

  #[test]
  fn convert_exact_input_and_split() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 20 TIGHTEN to 40 EASE
    let supply0 = 50 * COIN_VALUE;
    let input_amt = 20 * COIN_VALUE;

    let expected_supply0 = supply0 - input_amt;
    let min_output_amt = (supply0 * supply0 - expected_supply0 * expected_supply0).sqrt();
    let expected_supply1 = min_output_amt;

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: expected_supply0,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: expected_supply1 / 2,
              output: 3,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_supply0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_supply1,
            ..default()
          },
        ),
      ],
      [
        (
          OutPoint {
            txid: txid1,
            vout: 0,
          },
          vec![(ID0, expected_supply0), (ID1, expected_supply1 / 2)],
        ),
        (
          OutPoint {
            txid: txid1,
            vout: 1,
          },
          vec![(ID1, expected_supply1 / 2)],
        ),
      ],
    );
  }

  #[test]
  fn convert_exact_input_and_split_with_remainder_assigned_to_first_conversion_output() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 20 TIGHTEN to at least 30 EASE (expect 40 EASE)
    let supply0 = 50 * COIN_VALUE;
    let input_amt = 20 * COIN_VALUE;
    let min_output_amt = 30 * COIN_VALUE;

    let expected_supply0 = supply0 - input_amt;
    let expected_supply1 = (supply0 * supply0 - expected_supply0 * expected_supply0).sqrt();

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: expected_supply0,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: min_output_amt / 2,
              output: 3,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_supply0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_supply1,
            ..default()
          },
        ),
      ],
      [
        (
          OutPoint {
            txid: txid1,
            vout: 0,
          },
          vec![
            (ID0, expected_supply0),
            (ID1, min_output_amt / 2 + expected_supply1 - min_output_amt),
          ],
        ),
        (
          OutPoint {
            txid: txid1,
            vout: 1,
          },
          vec![(ID1, min_output_amt / 2)],
        ),
      ],
    );
  }

  #[test]
  fn convert_even_if_output_is_provided_as_an_input() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 10 TIGHTEN to 30 EASE)
    let supply0 = 50 * COIN_VALUE;
    let input_amt = 20 * COIN_VALUE;

    let expected_balance0 = supply0 - input_amt;
    let expected_balance1 = (supply0 * supply0 - expected_balance0 * expected_balance0).sqrt();

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: expected_balance0,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: expected_balance1,
              output: 0,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, expected_balance0), (ID1, expected_balance1)],
      )],
    );

    // Transfer 10 EASE and convert 10 TIGHTEN to 10 EASE
    let input_amt_2 = 10 * COIN_VALUE;

    let expected_balance0_2 = expected_balance0 - input_amt_2;
    let expected_balance1_2 = (expected_balance0 * expected_balance0
      + expected_balance1 * expected_balance1
      - expected_balance0_2 * expected_balance0_2)
      .sqrt();

    let txid2 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: expected_balance0_2,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: expected_balance1_2,
              output: 0,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance0_2,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance1_2,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid2,
          vout: 0,
        },
        vec![(ID0, expected_balance0_2), (ID1, expected_balance1_2)],
      )],
    );
  }

  #[test]
  fn convert_even_if_output_is_provided_as_an_input_and_split() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 10 TIGHTEN to 30 EASE)
    let supply0 = 50 * COIN_VALUE;
    let input_amt = 10 * COIN_VALUE;

    let expected_balance0 = supply0 - input_amt;
    let expected_balance1 = (supply0 * supply0 - expected_balance0 * expected_balance0).sqrt();

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: expected_balance0,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: expected_balance1,
              output: 0,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, expected_balance0), (ID1, expected_balance1)],
      )],
    );

    // Transfer EASE balance and convert 10 TIGHTEN to 10 EASE
    let input_amt_2 = 10 * COIN_VALUE;

    let expected_balance0_2 = expected_balance0 - input_amt_2;
    let expected_balance1_2 = (expected_balance0 * expected_balance0
      + expected_balance1 * expected_balance1
      - expected_balance0_2 * expected_balance0_2)
      .sqrt();

    let txid2 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID1,
              amount: expected_balance1_2,
              output: 3,
            },
            Edict {
              id: ID0,
              amount: expected_balance0_2,
              output: 0,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance0_2,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance1_2,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid2,
          vout: 0,
        },
        vec![(ID0, expected_balance0_2), (ID1, expected_balance1_2)],
      )],
    );
  }

  #[test]
  fn convert_and_burn_if_conversion_output_is_op_return() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 10 TIGHTEN to 30 EASE
    let supply0 = 50 * COIN_VALUE;
    let input_amt = 10 * COIN_VALUE;

    let expected_balance0 = supply0 - input_amt;
    let expected_balance1 = (supply0 * supply0 - expected_balance0 * expected_balance0).sqrt();

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: expected_balance0,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: expected_balance1,
              output: 2,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            burned: expected_balance1,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, expected_balance0)],
      )],
    );
  }

  #[test]
  fn convert_undo_burn_input_if_min_output_not_met() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 10 TIGHTEN to and require 80 EASE
    let supply0 = 50 * COIN_VALUE;
    let input_amt = 10 * COIN_VALUE;

    let expected_balance0 = supply0 - input_amt;
    let min_output1 = (supply0 * supply0 - expected_balance0 * expected_balance0).sqrt() * 2;

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: expected_balance0,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: min_output1,
              output: 0,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: supply0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, supply0)],
      )],
    );
  }

  #[test]
  fn convert_undo_burn_input_if_desired_output_exceeds_max_output() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert at most 10 TIGHTEN to 100 EASE
    let supply0 = 50 * COIN_VALUE;
    let max_input_amt = 10 * COIN_VALUE;
    let output_amt = 100 * COIN_VALUE;

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: supply0 - max_input_amt,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: output_amt,
              output: 0,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: supply0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, supply0)],
      )],
    );
  }

  #[test]
  fn convert_undo_burn_input_if_input_exceeds_max_input() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert at most 10 TIGHTEN to 100 EASE
    let supply0 = 50 * COIN_VALUE;
    let max_input_amt = 10 * COIN_VALUE;
    let output_amt = 40 * COIN_VALUE;

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: supply0 - max_input_amt,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: output_amt,
              output: 0,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: supply0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, supply0)],
      )],
    );
  }

  #[test]
  fn convert_undo_burn_input_if_min_output_not_met_and_missing_output_with_input_id() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 50 TIGHTEN and require 100 EASE
    let supply0 = 50 * COIN_VALUE;
    let min_output1 = supply0 * 2;

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![Edict {
            id: ID1,
            amount: min_output1,
            output: 1,
          }],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: supply0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            supply: 0,
            mints: 1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 1,
        },
        vec![(ID0, supply0)],
      )],
    );
  }

  #[test]
  fn convert_undo_burn_input_if_min_output_not_met_and_missing_output_with_input_id_and_output_id_invalid(
  ) {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 50 TIGHTEN and require 100 EASE
    let supply0 = 50 * COIN_VALUE;
    let min_output1 = supply0 * 2;

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![Edict {
            id: ID1,
            amount: min_output1,
            output: 3,
          }],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: supply0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            supply: 0,
            mints: 1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, supply0)],
      )],
    );
  }

  #[test]
  fn convert_burn_input_if_min_output_not_met_and_no_output_exists() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 50 TIGHTEN and require 100 EASE
    let supply0 = 50 * COIN_VALUE;
    let min_output1 = supply0 * 2;

    context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 0,
      op_return: Some(
        Runestone {
          edicts: vec![Edict {
            id: ID1,
            amount: min_output1,
            output: 0,
          }],
          pointer: Some(0),
          ..default()
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
            divisibility: 8,
            burned: supply0,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: supply0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            ..default()
          },
        ),
      ],
      [],
    );
  }

  #[test]
  fn mint_after_conversion() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 10 TIGHTEN to 30 EASE
    let supply0 = 50 * COIN_VALUE;
    let output_amt = 30 * COIN_VALUE;

    let expected_balance1 = output_amt;
    let expected_balance0 = (supply0 * supply0 - expected_balance1 * expected_balance1).sqrt();

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: expected_balance0,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: expected_balance1,
              output: 0,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance1,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, expected_balance0), (ID1, expected_balance1)],
      )],
    );

    let txid2 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 2,
            supply: expected_balance0 * 2,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 2,
            supply: expected_balance1 * 2,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid2,
          vout: 0,
        },
        vec![(ID0, expected_balance0 * 2), (ID1, expected_balance1 * 2)],
      )],
    );
  }

  #[test]
  fn mint_receives_burnt_runes() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    // Mint 30 EASE, burning 1 EASE, using at most 11 TIGHTEN, burning 1 TIGHTEN
    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          edicts: vec![
            Edict {
              id: ID1,
              amount: 29 * COIN_VALUE,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: 1 * COIN_VALUE,
              output: 1,
            },
            Edict {
              id: ID0,
              amount: 1 * COIN_VALUE,
              output: 1,
            },
            Edict {
              id: ID0,
              amount: 39 * COIN_VALUE,
              output: 0,
            },
          ],
          pointer: Some(1),
          ..default()
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
            divisibility: 8,
            burned: 1 * COIN_VALUE,
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
            divisibility: 8,
            burned: 1 * COIN_VALUE,
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
          vout: 0,
        },
        vec![(ID0, 39 * COIN_VALUE), (ID1, 29 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 2,
            supply: 80 * COIN_VALUE,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 2,
            supply: 60 * COIN_VALUE,
            ..default()
          },
        ),
      ],
      [(
        OutPoint {
          txid: txid1,
          vout: 0,
        },
        vec![(ID0, 80 * COIN_VALUE), (ID1, 60 * COIN_VALUE)],
      )],
    );
  }

  #[test]
  fn multiple_input_runes_on_different_inputs_may_be_allocated() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    // Convert 20 TIGHTEN to at least 30 EASE (expect 40 EASE)
    let supply0 = 50 * COIN_VALUE;
    let input_amt = 20 * COIN_VALUE;
    let min_output_amt = 30 * COIN_VALUE;

    let expected_balance0 = supply0 - input_amt;
    let expected_balance1 = (supply0 * supply0 - expected_balance0 * expected_balance0).sqrt();

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: expected_balance0,
              output: 0,
            },
            Edict {
              id: ID1,
              amount: min_output_amt,
              output: 1,
            },
          ],
          pointer: Some(2),
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance1,
            ..default()
          },
        ),
      ],
      [
        (
          OutPoint {
            txid: txid1,
            vout: 0,
          },
          vec![(ID0, expected_balance0)],
        ),
        (
          OutPoint {
            txid: txid1,
            vout: 1,
          },
          vec![(ID1, expected_balance1)],
        ),
      ],
    );

    let txid2 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[
        (context.get_block_count() - 1, 1, 0, Witness::new()),
        (context.get_block_count() - 1, 1, 1, Witness::new()),
      ],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: expected_balance0 / 2,
              output: 1,
            },
            Edict {
              id: ID1,
              amount: expected_balance1 / 2,
              output: 1,
            },
          ],
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance0,
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            mints: 1,
            supply: expected_balance1,
            ..default()
          },
        ),
      ],
      [
        (
          OutPoint {
            txid: txid2,
            vout: 0,
          },
          vec![(ID0, expected_balance0 / 2), (ID1, expected_balance1 / 2)],
        ),
        (
          OutPoint {
            txid: txid2,
            vout: 1,
          },
          vec![(ID0, expected_balance0 / 2), (ID1, expected_balance1 / 2)],
        ),
      ],
    );
  }

  #[test]
  fn unallocated_runes_are_assigned_to_first_non_op_return_output_when_op_return_is_not_last_output(
  ) {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      op_return: Some(
        script::Builder::new()
          .push_opcode(opcodes::all::OP_RETURN)
          .into_script(),
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 1,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );
  }

  #[test]
  fn edicts_over_max_inputs_are_ignored() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      op_return: Some(
        Runestone {
          edicts: vec![Edict {
            id: ID0,
            amount: u128::MAX,
            output: 0,
          }],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
  }

  #[test]
  fn split() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![Edict {
            id: ID0,
            amount: 0,
            output: 3,
          }],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
      [
        (
          OutPoint {
            txid: txid1,
            vout: 0,
          },
          vec![(ID0, 25 * COIN_VALUE)],
        ),
        (
          OutPoint {
            txid: txid1,
            vout: 1,
          },
          vec![(ID0, 25 * COIN_VALUE)],
        ),
      ],
    );
  }

  #[test]
  fn split_with_preceding_edict() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: 1001,
              output: 0,
            },
            Edict {
              id: ID0,
              amount: 0,
              output: 3,
            },
          ],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
      [
        (
          OutPoint {
            txid: txid1,
            vout: 0,
          },
          vec![(ID0, 1001 + (50 * COIN_VALUE - 1001) / 2 + 1)],
        ),
        (
          OutPoint {
            txid: txid1,
            vout: 1,
          },
          vec![(ID0, (50 * COIN_VALUE - 1001) / 2)],
        ),
      ],
    );
  }

  #[test]
  fn split_with_following_edict() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: 0,
              output: 3,
            },
            Edict {
              id: ID0,
              amount: 1000,
              output: 0,
            },
          ],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
      [
        (
          OutPoint {
            txid: txid1,
            vout: 0,
          },
          vec![(ID0, 25 * COIN_VALUE)],
        ),
        (
          OutPoint {
            txid: txid1,
            vout: 1,
          },
          vec![(ID0, 25 * COIN_VALUE)],
        ),
      ],
    );
  }

  #[test]
  fn split_with_amount() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![Edict {
            id: ID0,
            amount: 1000,
            output: 3,
          }],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
      [
        (
          OutPoint {
            txid: txid1,
            vout: 0,
          },
          vec![(ID0, 50 * COIN_VALUE - 1000)],
        ),
        (
          OutPoint {
            txid: txid1,
            vout: 1,
          },
          vec![(ID0, 1000)],
        ),
      ],
    );
  }

  #[test]
  fn split_with_amount_with_preceding_edict() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 4,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: 50 * COIN_VALUE - 2000,
              output: 0,
            },
            Edict {
              id: ID0,
              amount: 1000,
              output: 5,
            },
          ],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
      [
        (
          OutPoint {
            txid: txid1,
            vout: 0,
          },
          vec![(ID0, 50 * COIN_VALUE - 2000 + 1000)],
        ),
        (
          OutPoint {
            txid: txid1,
            vout: 1,
          },
          vec![(ID0, 1000)],
        ),
      ],
    );
  }

  #[test]
  fn split_with_amount_with_following_edict() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 4,
      op_return: Some(
        Runestone {
          edicts: vec![
            Edict {
              id: ID0,
              amount: 1000,
              output: 5,
            },
            Edict {
              id: ID0,
              amount: u128::MAX,
              output: 0,
            },
          ],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
      [
        (
          OutPoint {
            txid: txid1,
            vout: 0,
          },
          vec![(ID0, 50 * COIN_VALUE - 4000 + 1000)],
        ),
        (
          OutPoint {
            txid: txid1,
            vout: 1,
          },
          vec![(ID0, 1000)],
        ),
        (
          OutPoint {
            txid: txid1,
            vout: 2,
          },
          vec![(ID0, 1000)],
        ),
        (
          OutPoint {
            txid: txid1,
            vout: 3,
          },
          vec![(ID0, 1000)],
        ),
      ],
    );
  }

  #[test]
  fn allocate_all_remaining_runes_in_inputs() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    let txid0 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 0,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );

    let txid1 = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(context.get_block_count() - 1, 1, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          edicts: vec![Edict {
            id: ID0,
            amount: 0,
            output: 1,
          }],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
          vout: 1,
        },
        vec![(ID0, 50 * COIN_VALUE)],
      )],
    );
  }

  #[test]
  fn rune_can_be_minted_without_edict() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            ..default()
          },
        ),
      ],
      [],
    );

    let txid = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
      [(OutPoint { txid, vout: 0 }, vec![(ID0, 50 * COIN_VALUE)])],
    );
  }

  #[test]
  fn rune_cannot_be_minted_less_than_limit_amount() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            ..default()
          },
        ),
      ],
      [],
    );

    let txid = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          edicts: vec![Edict {
            id: ID0,
            amount: 100,
            output: 0,
          }],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
      [(OutPoint { txid, vout: 0 }, vec![(ID0, 50 * COIN_VALUE)])],
    );
  }

  #[test]
  fn open_mint_claims_can_use_split() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            ..default()
          },
        ),
      ],
      [],
    );

    let txid = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          edicts: vec![Edict {
            id: ID0,
            amount: 0,
            output: 3,
          }],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
      [
        (OutPoint { txid, vout: 0 }, vec![(ID0, 25 * COIN_VALUE)]),
        (OutPoint { txid, vout: 1 }, vec![(ID0, 25 * COIN_VALUE)]),
      ],
    );
  }

  #[test]
  fn transactions_cannot_claim_more_than_mint_amount() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            ..default()
          },
        ),
      ],
      [],
    );

    let txid = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          edicts: vec![Edict {
            id: ID0,
            amount: 100 * COIN_VALUE,
            output: 0,
          }],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
      [(OutPoint { txid, vout: 0 }, vec![(ID0, 50 * COIN_VALUE)])],
    );
  }

  #[test]
  fn multiple_edicts_in_one_transaction_may_claim_open_mint() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            ..default()
          },
        ),
      ],
      [],
    );

    let txid = context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      outputs: 2,
      op_return: Some(
        Runestone {
          mint: Some(ID0),
          edicts: vec![
            Edict {
              id: ID0,
              amount: 10 * COIN_VALUE,
              output: 0,
            },
            Edict {
              id: ID0,
              amount: 10 * COIN_VALUE,
              output: 0,
            },
            Edict {
              id: ID0,
              amount: 30 * COIN_VALUE,
              output: 0,
            },
          ],
          ..default()
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
            divisibility: 8,
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
            divisibility: 8,
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
      [(OutPoint { txid, vout: 0 }, vec![(ID0, 50 * COIN_VALUE)])],
    );
  }

  #[test]
  fn edict_with_amount_zero_and_no_destinations_is_ignored() {
    let context = Context::builder().arg("--index-runes").build();

    context.mine_balance();

    context.assert_runes(
      [
        (
          ID0,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            ..default()
          },
        ),
      ],
      [],
    );

    context.core.broadcast_tx(TransactionTemplate {
      inputs: &[(2, 0, 0, Witness::new())],
      op_return: Some(
        Runestone {
          edicts: vec![Edict {
            id: ID0,
            amount: 0,
            output: 1,
          }],
          ..default()
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
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(TIGHTEN),
              spacers: 0,
            },
            ..default()
          },
        ),
        (
          ID1,
          RuneEntry {
            divisibility: 8,
            spaced_rune: SpacedRune {
              rune: Rune(EASE),
              spacers: 0,
            },
            ..default()
          },
        ),
      ],
      [],
    );
  }

  #[test]
  fn genesis_rune() {
    assert_eq!(
      Chain::Mainnet.first_rune_height(),
      SUBSIDY_HALVING_INTERVAL * 4,
    );

    Context::builder()
      .chain(Chain::Mainnet)
      .arg("--index-runes")
      .build()
      .assert_runes(
        [
          (
            ID0,
            RuneEntry {
              divisibility: 8,
              spaced_rune: SpacedRune {
                rune: Rune(TIGHTEN),
                spacers: 0,
              },
              ..default()
            },
          ),
          (
            ID1,
            RuneEntry {
              divisibility: 8,
              spaced_rune: SpacedRune {
                rune: Rune(EASE),
                spacers: 0,
              },
              ..default()
            },
          ),
        ],
        [],
      );
  }
}
