//! Tests for `follow_jump_chain` trampoline-skipping.
//!
//! Edge cleanup inserts trampoline blocks whose body is the burden-faithful
//! release sequence (a paired `BurdenDec` adjacent to each release `RcDec`
//! whose var carries burden ops — Spec: Annex E §AIMS RL-4). `follow_jump_chain`
//! must classify a `[RcDec, BurdenDec]` body as a trampoline and skip it to
//! find the real merge block. Reverting `is_release_cleanup_instr` to RcDec-only
//! must fail the burden-trampoline semantic pin.

use super::follow_jump_chain;
use crate::ir::{ArcBlock, ArcInstr, ArcTerminator, ArcValue, LitValue, RcAtomicity, RcStrategy};
use crate::test_helpers::{b, make_func, owned_param, v};
use ori_types::Idx;

fn rc_dec(var: u32) -> ArcInstr {
    ArcInstr::RcDec {
        var: v(var),
        strategy: RcStrategy::HeapPointer,
        atomicity: RcAtomicity::default_atomic(),
    }
}

fn burden_dec(var: u32) -> ArcInstr {
    ArcInstr::BurdenDec { var: v(var) }
}

fn let_int(dst: u32) -> ArcInstr {
    ArcInstr::Let {
        dst: v(dst),
        ty: Idx::INT,
        value: ArcValue::Literal(LitValue::Int(0)),
    }
}

/// Build a one-arg pass-through trampoline block `id` carrying `body`, jumping
/// to `target` with its param `param`.
fn trampoline_block(id: u32, param: u32, body: Vec<ArcInstr>, target: u32) -> ArcBlock {
    ArcBlock {
        id: b(id),
        params: vec![(v(param), Idx::INT)],
        body,
        terminator: ArcTerminator::Jump {
            target: b(target),
            args: vec![v(param)],
        },
    }
}

/// Real merge block — receives the chain and returns its param.
fn merge_block(id: u32, param: u32) -> ArcBlock {
    ArcBlock {
        id: b(id),
        params: vec![(v(param), Idx::INT)],
        body: vec![],
        terminator: ArcTerminator::Return { value: v(param) },
    }
}

#[test]
fn burden_paired_trampoline_is_skipped() {
    // Semantic pin: a `[RcDec, BurdenDec]` trampoline (burden-faithful edge
    // release) is skipped; the chain resolves to the real merge block.
    //
    // bb0: (%1) RcDec(2); BurdenDec(2); Jump bb1(%1)   — burden trampoline
    // bb1: (%3) Return %3                               — real merge
    let blocks = vec![
        trampoline_block(0, 1, vec![rc_dec(2), burden_dec(2)], 1),
        merge_block(1, 3),
    ];
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 4],
    );

    assert_eq!(
        follow_jump_chain(&func, 0),
        1,
        "[RcDec, BurdenDec] trampoline must be skipped to the real merge block"
    );
}

#[test]
fn rc_dec_only_trampoline_is_skipped() {
    // Preserves prior behavior: a bare `[RcDec]` trampoline still skips.
    let blocks = vec![
        trampoline_block(0, 1, vec![rc_dec(2)], 1),
        merge_block(1, 3),
    ];
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 4],
    );

    assert_eq!(follow_jump_chain(&func, 0), 1);
}

#[test]
fn non_release_body_is_not_skipped() {
    // Negative pin: a block whose body carries a non-release instruction (Let)
    // is NOT a trampoline — `follow_jump_chain` must return it, not over-match.
    //
    // bb0: (%1) RcDec(2); %4 = let 0; Jump bb1(%1)   — NOT a trampoline (Let)
    // bb1: (%3) Return %3
    let blocks = vec![
        trampoline_block(0, 1, vec![rc_dec(2), let_int(4)], 1),
        merge_block(1, 3),
    ];
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 5],
    );

    assert_eq!(
        follow_jump_chain(&func, 0),
        0,
        "a block with a non-release instruction must NOT be skipped as a trampoline"
    );
}

#[test]
fn empty_body_block_is_not_a_trampoline() {
    // An empty-body block is not a trampoline (matches the `!is_empty()`
    // guard) — the chain stops at it.
    let blocks = vec![
        ArcBlock {
            id: b(0),
            params: vec![(v(1), Idx::INT)],
            body: vec![],
            terminator: ArcTerminator::Jump {
                target: b(1),
                args: vec![v(1)],
            },
        },
        merge_block(1, 3),
    ];
    let func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        blocks,
        vec![Idx::INT; 4],
    );

    assert_eq!(follow_jump_chain(&func, 0), 0);
}
