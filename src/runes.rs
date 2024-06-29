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
  use {super::*, crate::index::testing::Context};

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
            supply: 0,
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
            supply: 0,
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
            supply: 0,
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

  // Implement
  #[test]
  fn output_may_hold_multiple_runes() {}

  // Implement
  #[test]
  fn multiple_input_runes_on_the_same_input_may_be_allocated() {
    // let context = Context::builder().arg("--index-runes").build();

    // let (txid0, _) = context.etch(
    //   Runestone {
    //     edicts: vec![Edict {
    //       id: RuneId::default(),
    //       amount: u128::MAX,
    //       output: 0,
    //     }],
    //     etching: Some(Etching {
    //       rune: Some(Rune(RUNE)),
    //       ..default()
    //     }),
    //     ..default()
    //   },
    //   1,
    // );

    // context.assert_runes(
    //   [(
    //     ID0,
    //     RuneEntry {
    //       block: ID0.block,
    //       etching: txid0,
    //       spaced_rune: SpacedRune {
    //         rune: Rune(RUNE),
    //         spacers: 0,
    //       },
    //       timestamp: ID0.block,
    //       ..default()
    //     },
    //   )],
    //   [(
    //     OutPoint {
    //       txid: txid0,
    //       vout: 0,
    //     },
    //     vec![(ID0, u128::MAX)],
    //   )],
    // );

    // let (txid1, _) = context.etch(
    //   Runestone {
    //     edicts: vec![Edict {
    //       id: RuneId::default(),
    //       amount: u128::MAX,
    //       output: 0,
    //     }],
    //     etching: Some(Etching {
    //       rune: Some(Rune(RUNE + 1)),
    //       ..default()
    //     }),
    //     ..default()
    //   },
    //   1,
    // );

    // context.assert_runes(
    //   [
    //     (
    //       ID0,
    //       RuneEntry {
    //         block: ID0.block,
    //         etching: txid0,
    //         spaced_rune: SpacedRune {
    //           rune: Rune(RUNE),
    //           spacers: 0,
    //         },
    //         timestamp: ID0.block,
    //         ..default()
    //       },
    //     ),
    //     (
    //       ID1,
    //       RuneEntry {
    //         block: ID1.block,
    //         etching: txid1,
    //         spaced_rune: SpacedRune {
    //           rune: Rune(RUNE + 1),
    //           spacers: 0,
    //         },
    //         timestamp: ID1.block,
    //         number: 1,
    //         ..default()
    //       },
    //     ),
    //   ],
    //   [
    //     (
    //       OutPoint {
    //         txid: txid0,
    //         vout: 0,
    //       },
    //       vec![(ID0, u128::MAX)],
    //     ),
    //     (
    //       OutPoint {
    //         txid: txid1,
    //         vout: 0,
    //       },
    //       vec![(ID1, u128::MAX)],
    //     ),
    //   ],
    // );

    // let txid2 = context.core.broadcast_tx(TransactionTemplate {
    //   inputs: &[
    //     (ID0.block.try_into().unwrap(), 1, 0, Witness::new()),
    //     (ID1.block.try_into().unwrap(), 1, 0, Witness::new()),
    //   ],
    //   ..default()
    // });

    // context.mine_blocks(1);

    // context.assert_runes(
    //   [
    //     (
    //       ID0,
    //       RuneEntry {
    //         block: ID0.block,
    //         etching: txid0,
    //         spaced_rune: SpacedRune {
    //           rune: Rune(RUNE),
    //           spacers: 0,
    //         },
    //         timestamp: ID0.block,
    //         ..default()
    //       },
    //     ),
    //     (
    //       ID1,
    //       RuneEntry {
    //         block: ID1.block,
    //         etching: txid1,
    //         spaced_rune: SpacedRune {
    //           rune: Rune(RUNE + 1),
    //           spacers: 0,
    //         },
    //         timestamp: ID1.block,
    //         number: 1,
    //         ..default()
    //       },
    //     ),
    //   ],
    //   [(
    //     OutPoint {
    //       txid: txid2,
    //       vout: 0,
    //     },
    //     vec![(ID0, u128::MAX), (ID1, u128::MAX)],
    //   )],
    // );

    // let txid3 = context.core.broadcast_tx(TransactionTemplate {
    //   inputs: &[((ID1.block + 1).try_into().unwrap(), 1, 0, Witness::new())],
    //   outputs: 2,
    //   op_return: Some(
    //     Runestone {
    //       edicts: vec![
    //         Edict {
    //           id: ID0,
    //           amount: u128::MAX / 2,
    //           output: 1,
    //         },
    //         Edict {
    //           id: ID1,
    //           amount: u128::MAX / 2,
    //           output: 1,
    //         },
    //       ],
    //       ..default()
    //     }
    //     .encipher(),
    //   ),
    //   ..default()
    // });

    // context.mine_blocks(1);

    // context.assert_runes(
    //   [
    //     (
    //       ID0,
    //       RuneEntry {
    //         block: ID0.block,
    //         etching: txid0,
    //         spaced_rune: SpacedRune {
    //           rune: Rune(RUNE),
    //           spacers: 0,
    //         },
    //         timestamp: ID0.block,
    //         ..default()
    //       },
    //     ),
    //     (
    //       ID1,
    //       RuneEntry {
    //         block: ID1.block,
    //         etching: txid1,
    //         spaced_rune: SpacedRune {
    //           rune: Rune(RUNE + 1),
    //           spacers: 0,
    //         },
    //         timestamp: ID1.block,
    //         number: 1,
    //         ..default()
    //       },
    //     ),
    //   ],
    //   [
    //     (
    //       OutPoint {
    //         txid: txid3,
    //         vout: 0,
    //       },
    //       vec![(ID0, u128::MAX / 2 + 1), (ID1, u128::MAX / 2 + 1)],
    //     ),
    //     (
    //       OutPoint {
    //         txid: txid3,
    //         vout: 1,
    //       },
    //       vec![(ID0, u128::MAX / 2), (ID1, u128::MAX / 2)],
    //     ),
    //   ],
    // );
  }

  // Implement
  #[test]
  fn multiple_input_runes_on_different_inputs_may_be_allocated() {
    // let context = Context::builder().arg("--index-runes").build();

    // let (txid0, _) = context.etch(
    //   Runestone {
    //     edicts: vec![Edict {
    //       id: RuneId::default(),
    //       amount: u128::MAX,
    //       output: 0,
    //     }],
    //     etching: Some(Etching {
    //       rune: Some(Rune(RUNE)),
    //       ..default()
    //     }),
    //     ..default()
    //   },
    //   1,
    // );

    // context.assert_runes(
    //   [(
    //     ID0,
    //     RuneEntry {
    //       block: ID0.block,
    //       etching: txid0,
    //       spaced_rune: SpacedRune {
    //         rune: Rune(RUNE),
    //         spacers: 0,
    //       },
    //       timestamp: ID0.block,
    //       ..default()
    //     },
    //   )],
    //   [(
    //     OutPoint {
    //       txid: txid0,
    //       vout: 0,
    //     },
    //     vec![(ID0, u128::MAX)],
    //   )],
    // );

    // let (txid1, _) = context.etch(
    //   Runestone {
    //     edicts: vec![Edict {
    //       id: RuneId::default(),
    //       amount: u128::MAX,
    //       output: 0,
    //     }],
    //     etching: Some(Etching {
    //       rune: Some(Rune(RUNE + 1)),
    //       ..default()
    //     }),
    //     ..default()
    //   },
    //   1,
    // );

    // context.assert_runes(
    //   [
    //     (
    //       ID0,
    //       RuneEntry {
    //         block: ID0.block,
    //         etching: txid0,
    //         spaced_rune: SpacedRune {
    //           rune: Rune(RUNE),
    //           spacers: 0,
    //         },
    //         timestamp: ID0.block,
    //         ..default()
    //       },
    //     ),
    //     (
    //       ID1,
    //       RuneEntry {
    //         block: ID1.block,
    //         etching: txid1,
    //         spaced_rune: SpacedRune {
    //           rune: Rune(RUNE + 1),
    //           spacers: 0,
    //         },
    //         timestamp: ID1.block,
    //         number: 1,
    //         ..default()
    //       },
    //     ),
    //   ],
    //   [
    //     (
    //       OutPoint {
    //         txid: txid0,
    //         vout: 0,
    //       },
    //       vec![(ID0, u128::MAX)],
    //     ),
    //     (
    //       OutPoint {
    //         txid: txid1,
    //         vout: 0,
    //       },
    //       vec![(ID1, u128::MAX)],
    //     ),
    //   ],
    // );

    // let txid2 = context.core.broadcast_tx(TransactionTemplate {
    //   inputs: &[
    //     (ID0.block.try_into().unwrap(), 1, 0, Witness::new()),
    //     (ID1.block.try_into().unwrap(), 1, 0, Witness::new()),
    //   ],
    //   op_return: Some(
    //     Runestone {
    //       edicts: vec![
    //         Edict {
    //           id: ID0,
    //           amount: u128::MAX,
    //           output: 0,
    //         },
    //         Edict {
    //           id: ID1,
    //           amount: u128::MAX,
    //           output: 0,
    //         },
    //       ],
    //       ..default()
    //     }
    //     .encipher(),
    //   ),
    //   ..default()
    // });

    // context.mine_blocks(1);

    // context.assert_runes(
    //   [
    //     (
    //       ID0,
    //       RuneEntry {
    //         block: ID0.block,
    //         etching: txid0,
    //         spaced_rune: SpacedRune {
    //           rune: Rune(RUNE),
    //           spacers: 0,
    //         },
    //         timestamp: ID0.block,
    //         ..default()
    //       },
    //     ),
    //     (
    //       ID1,
    //       RuneEntry {
    //         block: ID1.block,
    //         etching: txid1,
    //         spaced_rune: SpacedRune {
    //           rune: Rune(RUNE + 1),
    //           spacers: 0,
    //         },
    //         timestamp: ID1.block,
    //         number: 1,
    //         ..default()
    //       },
    //     ),
    //   ],
    //   [(
    //     OutPoint {
    //       txid: txid2,
    //       vout: 0,
    //     },
    //     vec![(ID0, u128::MAX), (ID1, u128::MAX)],
    //   )],
    // );
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
