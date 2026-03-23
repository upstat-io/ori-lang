//! Tests for RC operation counting.

use crate::ir::{
    ArcBlock, ArcBlockId, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArcVarId, LitValue,
    RcStrategy,
};

use super::{count_rc_ops, RcOpCount};

fn function_with_blocks(blocks: Vec<ArcBlock>) -> ArcFunction {
    ArcFunction {
        blocks,
        ..Default::default()
    }
}

fn block_with_body(id: u32, body: Vec<ArcInstr>) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body,
        terminator: ArcTerminator::Unreachable,
    }
}

#[test]
fn empty_function_has_zero_rc_ops() {
    let func = ArcFunction::default();
    let count = count_rc_ops(&func);
    assert_eq!(count, RcOpCount { inc: 0, dec: 0 });
    assert_eq!(count.total(), 0);
}

#[test]
fn counts_single_inc_and_dec() {
    let block = block_with_body(
        0,
        vec![
            ArcInstr::RcInc {
                var: ArcVarId::new(0),
                count: 1,
                strategy: RcStrategy::HeapPointer,
            },
            ArcInstr::RcDec {
                var: ArcVarId::new(0),
                strategy: RcStrategy::HeapPointer,
            },
        ],
    );
    let func = function_with_blocks(vec![block]);
    let count = count_rc_ops(&func);
    assert_eq!(count, RcOpCount { inc: 1, dec: 1 });
    assert_eq!(count.total(), 2);
}

#[test]
fn counts_batched_inc() {
    let block = block_with_body(
        0,
        vec![ArcInstr::RcInc {
            var: ArcVarId::new(0),
            count: 3,
            strategy: RcStrategy::HeapPointer,
        }],
    );
    let func = function_with_blocks(vec![block]);
    let count = count_rc_ops(&func);
    assert_eq!(count.inc, 3);
    assert_eq!(count.dec, 0);
    assert_eq!(count.total(), 3);
}

#[test]
fn counts_across_multiple_blocks() {
    let block0 = ArcBlock {
        id: ArcBlockId::new(0),
        params: Vec::new(),
        body: vec![ArcInstr::RcInc {
            var: ArcVarId::new(0),
            count: 1,
            strategy: RcStrategy::HeapPointer,
        }],
        terminator: ArcTerminator::Jump {
            target: ArcBlockId::new(1),
            args: Vec::new(),
        },
    };
    let block1 = block_with_body(
        1,
        vec![
            ArcInstr::RcDec {
                var: ArcVarId::new(0),
                strategy: RcStrategy::HeapPointer,
            },
            ArcInstr::RcDec {
                var: ArcVarId::new(1),
                strategy: RcStrategy::HeapPointer,
            },
        ],
    );
    let func = function_with_blocks(vec![block0, block1]);
    let count = count_rc_ops(&func);
    assert_eq!(count, RcOpCount { inc: 1, dec: 2 });
}

#[test]
fn ignores_non_rc_instructions() {
    let block = block_with_body(
        0,
        vec![
            ArcInstr::Let {
                dst: ArcVarId::new(0),
                ty: ori_types::Idx::from_raw(0),
                value: ArcValue::Literal(LitValue::Unit),
            },
            ArcInstr::RcInc {
                var: ArcVarId::new(0),
                count: 1,
                strategy: RcStrategy::HeapPointer,
            },
        ],
    );
    let func = function_with_blocks(vec![block]);
    let count = count_rc_ops(&func);
    assert_eq!(count, RcOpCount { inc: 1, dec: 0 });
}

#[test]
fn display_format() {
    let count = RcOpCount { inc: 3, dec: 2 };
    assert_eq!(format!("{count}"), "3inc/2dec (5total)");
}
