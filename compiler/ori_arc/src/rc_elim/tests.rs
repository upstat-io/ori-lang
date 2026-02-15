use ori_ir::Name;
use ori_types::Idx;

use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue};
use crate::test_helpers::{
    b, count_block_rc_ops as count_rc_ops, count_dec, count_inc, make_func, owned_param, v,
};

use super::eliminate_rc_ops;

// Helpers

/// Total instruction count in a block (including RC ops).
fn body_len(func: &ArcFunction, block_idx: usize) -> usize {
    func.blocks[block_idx].body.len()
}

// Basic elimination

/// Adjacent `RcInc(x); RcDec(x)` → both eliminated.
#[test]
fn adjacent_inc_dec_eliminated() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 1);
    assert_eq!(count_rc_ops(&func, 0), 0);
}

/// `RcInc(x); [unrelated instruction]; RcDec(x)` → eliminated
/// (intervening instruction doesn't use x).
#[test]
fn non_adjacent_pair_no_use_eliminated() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                // Unrelated instruction — doesn't use v(0).
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(42)),
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR, Idx::INT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 1);
    assert_eq!(count_rc_ops(&func, 0), 0);
    // The Let instruction remains.
    assert_eq!(body_len(&func, 0), 1);
}

/// `RcInc(x); use(x); RcDec(x)` → NOT eliminated (x is used between them).
#[test]
fn intervening_use_prevents_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                // Uses v(0) — prevents elimination.
                ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::UNIT,
                    func: Name::from_raw(99),
                    args: vec![v(0)],
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::UNIT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
    assert_eq!(count_inc(&func, 0, v(0)), 1);
    assert_eq!(count_dec(&func, 0, v(0)), 1);
}

// Dec before Inc (unsafe)

/// `RcDec(x); RcInc(x)` → NOT eliminated (Dec might free x).
#[test]
fn dec_before_inc_not_eliminated() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
    assert_eq!(count_inc(&func, 0, v(0)), 1);
    assert_eq!(count_dec(&func, 0, v(0)), 1);
}

// Multiple independent pairs

/// Two independent pairs: `RcInc(x); RcDec(x); RcInc(y); RcDec(y)`.
#[test]
fn multiple_independent_pairs() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::RcInc {
                    var: v(1),
                    count: 1,
                },
                ArcInstr::RcDec { var: v(1) },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(LitValue::Unit),
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::STR, Idx::UNIT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 2);
    assert_eq!(count_rc_ops(&func, 0), 0);
}

/// Interleaved vars: `RcInc(x); RcInc(y); RcDec(x); RcDec(y)`.
/// Both pairs eliminated — different vars don't interfere.
#[test]
fn interleaved_vars_both_eliminated() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::RcInc {
                    var: v(1),
                    count: 1,
                },
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::RcDec { var: v(1) },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(LitValue::Unit),
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::STR, Idx::UNIT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 2);
    assert_eq!(count_rc_ops(&func, 0), 0);
}

// Cascading elimination

/// Nested pairs: `RcInc(x); RcInc(x); RcDec(x); RcDec(x)`.
/// First pass eliminates the inner pair, second pass eliminates the outer.
#[test]
fn nested_pairs_cascading() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 2);
    assert_eq!(count_rc_ops(&func, 0), 0);
}

// Edge cases

/// No RC ops at all → no elimination.
#[test]
fn no_rc_ops_no_changes() {
    let mut func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::INT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
}

/// Only Inc, no Dec → no elimination.
#[test]
fn only_inc_no_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::RcInc {
                var: v(0),
                count: 1,
            }],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
    assert_eq!(count_inc(&func, 0, v(0)), 1);
}

/// Only Dec, no Inc → no elimination.
#[test]
fn only_dec_no_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(LitValue::Unit),
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::UNIT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
    assert_eq!(count_dec(&func, 0, v(0)), 1);
}

/// `RcInc(x, count: 2)` → NOT matched (batched Inc, conservative).
#[test]
fn batched_inc_not_matched() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 2,
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
    assert_eq!(count_inc(&func, 0, v(0)), 1);
    assert_eq!(count_dec(&func, 0, v(0)), 1);
}

// Multi-block

/// Each block analyzed independently — pairs within a block are
/// eliminated, cross-block pairs are not.
#[test]
fn multi_block_independent() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    // Eliminable pair in block 0.
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![
                    // Non-eliminable: use between Inc and Dec.
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    ArcInstr::Apply {
                        dst: v(1),
                        ty: Idx::UNIT,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                    },
                    ArcInstr::RcDec { var: v(0) },
                ],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::STR, Idx::UNIT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    // Only block 0's pair is eliminated.
    assert_eq!(eliminated, 1);
    assert_eq!(count_rc_ops(&func, 0), 0);
    assert_eq!(count_inc(&func, 1, v(0)), 1);
    assert_eq!(count_dec(&func, 1, v(0)), 1);
}

// Non-RC instruction preservation

/// Non-RC instructions are preserved after elimination.
#[test]
fn non_rc_instructions_preserved() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(1)),
                },
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(2)),
                },
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::Let {
                    dst: v(3),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(3)),
                },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR, Idx::INT, Idx::INT, Idx::INT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 1);
    // 5 original - 2 removed = 3 Let instructions.
    assert_eq!(body_len(&func, 0), 3);
    assert!(matches!(func.blocks[0].body[0], ArcInstr::Let { .. }));
    assert!(matches!(func.blocks[0].body[1], ArcInstr::Let { .. }));
    assert!(matches!(func.blocks[0].body[2], ArcInstr::Let { .. }));
}

// Construct / Project use

/// `RcInc(x); Construct(..., x, ...); RcDec(x)` → NOT eliminated.
/// x is used in the Construct.
#[test]
fn construct_use_prevents_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::Construct {
                    dst: v(1),
                    ty: Idx::STR,
                    ctor: CtorKind::ListLiteral,
                    args: vec![v(0)],
                },
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(LitValue::Unit),
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::STR, Idx::UNIT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
}

/// `RcInc(x); Project(y = x.0); RcDec(x)` → NOT eliminated.
/// x is used in the Project.
#[test]
fn project_use_prevents_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::INT,
                    value: v(0),
                    field: 0,
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::INT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
}

// Partial elimination

/// One pair eliminable, one not — only the eliminable one is removed.
#[test]
fn partial_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                // Eliminable: Inc(x), Dec(x) with no use between.
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::RcDec { var: v(0) },
                // NOT eliminable: Inc(y), use(y), Dec(y).
                ArcInstr::RcInc {
                    var: v(1),
                    count: 1,
                },
                ArcInstr::Apply {
                    dst: v(2),
                    ty: Idx::UNIT,
                    func: Name::from_raw(99),
                    args: vec![v(1)],
                },
                ArcInstr::RcDec { var: v(1) },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::STR, Idx::UNIT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 1);
    assert_eq!(count_inc(&func, 0, v(0)), 0);
    assert_eq!(count_dec(&func, 0, v(0)), 0);
    assert_eq!(count_inc(&func, 0, v(1)), 1);
    assert_eq!(count_dec(&func, 0, v(1)), 1);
}

// Reuse-related patterns

/// Pattern from reuse expansion: `IsShared` + `RcInc`/`RcDec` in slow path.
/// The Inc/Dec pair around an `IsShared` that uses a DIFFERENT var is eliminable.
#[test]
fn reuse_pattern_different_var_eliminated() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                // IsShared uses v(1), not v(0) — doesn't block elimination.
                ArcInstr::IsShared {
                    dst: v(2),
                    var: v(1),
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR, Idx::STR, Idx::BOOL],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 1);
    assert_eq!(count_rc_ops(&func, 0), 0);
}

/// `IsShared` that uses the SAME var blocks elimination.
#[test]
fn is_shared_same_var_prevents_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::IsShared {
                    dst: v(1),
                    var: v(0),
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR, Idx::BOOL],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
}

// Sequential same-var pairs

/// `Inc(x); Dec(x); Inc(x); Dec(x)` — two sequential pairs, both eliminated.
#[test]
fn sequential_same_var_pairs() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 2);
    assert_eq!(count_rc_ops(&func, 0), 0);
}

// Empty block

/// Empty block body (only terminator) → no crash, no changes.
#[test]
fn empty_block_body() {
    let mut func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::INT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
}

// Span preservation

/// Span vectors are correctly maintained after elimination.
#[test]
fn spans_preserved_after_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(42)),
                },
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR, Idx::INT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 1);
    // 1 Let instruction remains.
    assert_eq!(body_len(&func, 0), 1);
    // Spans length matches body length.
    assert_eq!(func.spans[0].len(), func.blocks[0].body.len());
}

// Set / SetTag operations

/// Set instruction using the tracked var prevents elimination.
#[test]
fn set_instruction_prevents_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::Set {
                    base: v(0),
                    field: 0,
                    value: v(1),
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR, Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
}

/// `SetTag` instruction using the tracked var prevents elimination.
#[test]
fn set_tag_prevents_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::SetTag { base: v(0), tag: 1 },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
}

// ApplyIndirect

/// Indirect call using the tracked var as closure prevents elimination.
#[test]
fn apply_indirect_closure_prevents_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::ApplyIndirect {
                    dst: v(1),
                    ty: Idx::UNIT,
                    closure: v(0),
                    args: vec![],
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::UNIT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
}

/// Indirect call using the tracked var as an argument prevents elimination.
#[test]
fn apply_indirect_arg_prevents_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::ApplyIndirect {
                    dst: v(2),
                    ty: Idx::UNIT,
                    closure: v(1),
                    args: vec![v(0)],
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::STR, Idx::UNIT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
}

// PartialApply

/// `PartialApply` capturing the tracked var prevents elimination.
#[test]
fn partial_apply_prevents_elimination() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
                ArcInstr::PartialApply {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(0)],
                },
                ArcInstr::RcDec { var: v(0) },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
}

// Return value

/// `eliminate_rc_ops` returns 0 for functions with no RC ops.
#[test]
fn return_value_zero_when_nothing_eliminated() {
    let mut func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: v(1),
                ty: Idx::INT,
                value: ArcValue::Literal(LitValue::Int(42)),
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::INT, Idx::INT],
    );

    assert_eq!(eliminate_rc_ops(&mut func), 0);
}

// Cross-block edge pair elimination

/// `RcInc(x)` at end of B0, `RcDec(x)` at start of B1 (single
/// predecessor) → eliminated.
#[test]
fn cross_block_edge_pair_eliminated() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                }],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::RcDec { var: v(0) }],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 1);
    assert_eq!(count_rc_ops(&func, 0), 0);
    assert_eq!(count_rc_ops(&func, 1), 0);
}

/// Cross-block pair where `x` IS used by the terminator → NOT eliminated.
#[test]
fn cross_block_terminator_uses_var_not_eliminated() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::RcInc {
                var: v(0),
                count: 1,
            }],
            // Return uses v(0) — blocks cross-block elimination.
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    // Only intra-block analysis can run here; no cross-block target.
    // The Inc has no matching Dec in the same block.
    assert_eq!(eliminated, 0);
}

/// Multi-predecessor block: `RcDec(x)` at start of merge block
/// reached from two different predecessors → NOT eliminated
/// (conservative, would need Inc in ALL predecessors).
#[test]
fn cross_block_diamond_not_eliminated() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            // B0: branch to B1 or B2
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::BOOL,
                    value: ArcValue::Literal(LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            // B1: Inc(x) then jump to merge
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                }],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            // B2: no Inc, also jumps to merge
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            // B3 (merge): Dec(x) at start — has TWO predecessors
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![ArcInstr::RcDec { var: v(0) }],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::STR, Idx::BOOL],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    // B3 has 2 predecessors → cross-block won't eliminate.
    // B1's Inc has no matching Dec in B1.
    assert_eq!(eliminated, 0);
    assert_eq!(count_inc(&func, 1, v(0)), 1);
    assert_eq!(count_dec(&func, 3, v(0)), 1);
}

/// Cross-block chain: Inc at end of B0, no use in B1, Dec at start
/// of B1 → eliminated (B1 is single-predecessor).
#[test]
fn cross_block_with_intervening_unrelated_instr() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    // Unrelated instruction, then Inc(x)
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(42)),
                    },
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::RcDec { var: v(0) }],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::STR, Idx::INT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 1);
    assert_eq!(count_rc_ops(&func, 0), 0);
    assert_eq!(count_rc_ops(&func, 1), 0);
    // The Let instruction in B0 remains.
    assert_eq!(body_len(&func, 0), 1);
}

/// Cross-block: Inc NOT at end of B0 (instruction uses x after Inc)
/// → NOT eliminated.
#[test]
fn cross_block_use_after_inc_in_pred_not_eliminated() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::RcInc {
                        var: v(0),
                        count: 1,
                    },
                    // Uses v(0) AFTER the Inc — blocks cross-block elimination.
                    ArcInstr::Apply {
                        dst: v(1),
                        ty: Idx::UNIT,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::RcDec { var: v(0) }],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::STR, Idx::UNIT],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    assert_eq!(eliminated, 0);
}

/// Self-loop: block jumps to itself → NOT eliminated.
#[test]
fn cross_block_self_loop_not_eliminated() {
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                },
            ],
            terminator: ArcTerminator::Jump {
                target: b(0),
                args: vec![],
            },
        }],
        vec![Idx::STR],
    );

    let eliminated = eliminate_rc_ops(&mut func);

    // Dec→Inc in same block is NOT safe (Dec might free).
    // Self-loop cross-block is skipped.
    assert_eq!(eliminated, 0);
}

// ── Dataflow-enhanced elimination tests ──────────────────────

use crate::ownership::DerivedOwnership;

use super::eliminate_rc_ops_dataflow;

/// `BorrowedFrom` variable: `RcInc`/`RcDec` are redundant while source is alive.
#[test]
fn dataflow_borrowed_eliminates_inc() {
    // Block 0:
    //   v1 = project(v0, 0)   -- BorrowedFrom(v0)
    //   RcInc(v1)             -- redundant: v0 is alive
    //   apply f(v1)
    //   RcDec(v1)             -- redundant: v0 is alive
    //   return v0
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::STR,
                    value: v(0),
                    field: 0,
                },
                ArcInstr::RcInc {
                    var: v(1),
                    count: 1,
                },
                ArcInstr::Apply {
                    dst: v(2),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(1)],
                },
                ArcInstr::RcDec { var: v(1) },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR],
    );

    // v0: Owned, v1: BorrowedFrom(v0), v2: Owned
    let ownership = vec![
        DerivedOwnership::Owned,
        DerivedOwnership::BorrowedFrom(v(0)),
        DerivedOwnership::Owned,
    ];

    let eliminated = eliminate_rc_ops_dataflow(&mut func, &ownership);
    assert!(eliminated > 0, "should eliminate borrowed RC ops");

    // v1's RcInc and RcDec should both be gone.
    let inc_count = count_inc(&func, 0, v(1));
    let dec_count = count_dec(&func, 0, v(1));
    assert_eq!(inc_count, 0, "RcInc(v1) should be eliminated");
    assert_eq!(dec_count, 0, "RcDec(v1) should be eliminated");
}

/// Diamond pattern: `RcInc` in both branches, `RcDec` at merge.
#[test]
fn dataflow_diamond_join() {
    // Block 0: branch on v1
    // Block 1: RcInc(v0); jump b3
    // Block 2: RcInc(v0); jump b3
    // Block 3: RcDec(v0); return v0
    //
    // Both branches Inc v0, and the merge Dec's v0 → eliminate.
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::BOOL,
                    value: ArcValue::Literal(LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                }],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                }],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![ArcInstr::RcDec { var: v(0) }],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::STR, Idx::BOOL],
    );

    let ownership = vec![DerivedOwnership::Owned, DerivedOwnership::Owned];

    let eliminated = eliminate_rc_ops_dataflow(&mut func, &ownership);
    assert!(eliminated > 0, "should eliminate diamond join pattern");

    // The RcDec at block 3 and RcInc in blocks 1+2 should be eliminated.
    assert_eq!(
        count_inc(&func, 1, v(0)),
        0,
        "b1 RcInc should be eliminated"
    );
    assert_eq!(
        count_inc(&func, 2, v(0)),
        0,
        "b2 RcInc should be eliminated"
    );
    assert_eq!(
        count_dec(&func, 3, v(0)),
        0,
        "b3 RcDec should be eliminated"
    );
}
