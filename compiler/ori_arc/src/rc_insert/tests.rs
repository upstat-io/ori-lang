use ori_ir::{Name, StringInterner};
use ori_types::{Idx, Pool};

use rustc_hash::FxHashMap;
use rustc_hash::FxHashSet;

use crate::borrow::infer_derived_ownership;
use crate::ir::{
    ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, ArgOwnership, CtorKind, LitValue,
};
use crate::liveness::compute_liveness;
use crate::test_helpers::{
    b, borrowed_param, count_block_rc_ops as count_rc_ops, count_dec, count_inc, make_func,
    owned_param, v,
};
use crate::ArcClassifier;

use super::{
    annotate_arg_ownership, insert_external_invoke_cleanup, insert_rc_ops,
    insert_rc_ops_with_ownership,
};

// Helpers

/// Run RC insertion on a function, returning the transformed function.
fn run_rc_insert(mut func: ArcFunction) -> ArcFunction {
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let liveness = compute_liveness(&func, &classifier);
    insert_rc_ops(&mut func, &classifier, &liveness);
    func
}

// Tests

/// Passthrough — `fn(x: str) -> str { x }`.
/// Ownership transfers through return, no RC ops needed.
#[test]
fn passthrough_no_ops() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let result = run_rc_insert(func);

    // x is owned, used exactly once in return → no Inc, no Dec.
    assert_eq!(count_rc_ops(&result, 0), 0);
}

/// Dead definition — `fn() { let s = "hello"; unit }`.
/// String is created but not used → `RcDec`.
#[test]
fn dead_definition_gets_dec() {
    let func = make_func(
        vec![],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: v(0),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(Name::from_raw(100))),
                },
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

    let result = run_rc_insert(func);

    // v0 (str) is defined but never used → Dec.
    assert_eq!(count_dec(&result, 0, v(0)), 1);
    // v1 (unit) is scalar → no RC ops.
    assert_eq!(count_inc(&result, 0, v(1)), 0);
    assert_eq!(count_dec(&result, 0, v(1)), 0);
}

/// Multiple uses — `fn(x: str) { g(x, x) }`.
/// x is used twice in the same Apply → 1 `RcInc`.
#[test]
fn multiple_uses_get_inc() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::UNIT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: v(1),
                ty: Idx::UNIT,
                func: Name::from_raw(99),
                args: vec![v(0), v(0)],
                arg_ownership: vec![ArgOwnership::Owned; 2],
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::UNIT],
    );

    let result = run_rc_insert(func);

    // x used twice in Apply → 1 Inc (second occurrence).
    assert_eq!(count_inc(&result, 0, v(0)), 1);
}

/// Borrowed param — `fn(@borrow x: str) -> int { len(x) }`.
/// Borrowed parameter: zero RC ops.
#[test]
fn borrowed_param_no_ops() {
    let func = make_func(
        vec![borrowed_param(0, Idx::STR)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: v(1),
                ty: Idx::INT,
                func: Name::from_raw(99),
                args: vec![v(0)],
                arg_ownership: vec![ArgOwnership::Owned; 1],
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::INT],
    );

    let result = run_rc_insert(func);

    // Borrowed param: no Inc, no Dec.
    assert_eq!(count_rc_ops(&result, 0), 0);
}

/// Borrowed param returned — `fn(@borrow x: str) -> str { x }`.
/// Borrowed param being returned needs Inc (transfer ownership to caller).
#[test]
fn borrowed_returned_gets_inc() {
    let func = make_func(
        vec![borrowed_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let result = run_rc_insert(func);

    // Borrowed x returned → Inc (transfer to caller).
    assert_eq!(count_inc(&result, 0, v(0)), 1);
    // No Dec (borrowed params are never Dec'd).
    assert_eq!(count_dec(&result, 0, v(0)), 0);
}

/// Project from borrowed — `fn(@borrow p: T) { use p.field }`.
/// Projected field from borrowed param: no RC ops (borrows set).
#[test]
fn project_from_borrowed_no_ops() {
    // Project an int field from a borrowed param → scalar, no RC ops.
    let func = make_func(
        vec![borrowed_param(0, Idx::STR)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::INT,
                    value: v(0),
                    field: 0,
                },
                ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::INT,
                    value: ArcValue::PrimOp {
                        op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                        args: vec![v(1), v(1)],
                    },
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::INT, Idx::INT],
    );

    let result = run_rc_insert(func);

    // v1 is int (scalar) → no RC. v0 is borrowed → no RC. Zero ops.
    assert_eq!(count_rc_ops(&result, 0), 0);
}

/// Project from borrowed stored — `fn(@borrow p: T) { Construct(p.field) }`.
/// Projected RC field from borrowed, stored in Construct → Inc.
#[test]
fn project_from_borrowed_stored() {
    // Project str field from borrowed → store in Construct → owned position.
    let func = make_func(
        vec![borrowed_param(0, Idx::STR)],
        Idx::UNIT,
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
                ArcInstr::Construct {
                    dst: v(2),
                    ty: Idx::UNIT,
                    ctor: CtorKind::Tuple,
                    args: vec![v(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::STR, Idx::UNIT],
    );

    let result = run_rc_insert(func);

    // field (v1) is borrowed-derived but stored in Construct → Inc.
    assert_eq!(count_inc(&result, 0, v(1)), 1);
    // p (v0) is borrowed → no RC ops.
    assert_eq!(count_inc(&result, 0, v(0)), 0);
    assert_eq!(count_dec(&result, 0, v(0)), 0);
}

/// Unused owned param — `fn(x: str, y: str) -> str { x }`.
/// y is never used → Dec at entry.
#[test]
fn unused_owned_param_dec() {
    let func = make_func(
        vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR, Idx::STR],
    );

    let result = run_rc_insert(func);

    // x (v0) used in return → no Inc/Dec (single use, ownership transfers).
    assert_eq!(count_inc(&result, 0, v(0)), 0);
    assert_eq!(count_dec(&result, 0, v(0)), 0);
    // y (v1) never used → Dec.
    assert_eq!(count_dec(&result, 0, v(1)), 1);
}

/// Diamond branch — if/else both using a str var.
/// Each branch path should have correct RC balance.
#[test]
fn diamond_branch() {
    // Block 0: branch on v1 (bool) → b1 or b2
    // Block 1: let v2 = apply f(v0); jump to b3 with v2
    // Block 2: jump to b3 with v0
    // Block 3: param v3: str; return v3
    let func = make_func(
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
                body: vec![ArcInstr::Apply {
                    dst: v(2),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned; 1],
                }],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![v(2)],
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(3),
                    args: vec![v(0)],
                },
            },
            ArcBlock {
                id: b(3),
                params: vec![(v(3), Idx::STR)],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(3) },
            },
        ],
        vec![Idx::STR, Idx::BOOL, Idx::STR, Idx::STR],
    );

    let result = run_rc_insert(func);

    // v0 in block 0: just live, no inc/dec.
    assert_eq!(count_rc_ops(&result, 0), 0);
    // Block 3: v3 returned, single use.
    assert_eq!(count_rc_ops(&result, 3), 0);
}

/// Loop variable — variable live across loop iterations.
#[test]
fn loop_variable() {
    // Block 0: jump to b1 with v0 (str param)
    // Block 1: param v1: str; branch on v2 (bool) → b2 or b3
    // Block 2: let v3 = apply f(v1); jump to b1 with v3
    // Block 3: return v1
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::BOOL,
                    value: ArcValue::Literal(LitValue::Bool(true)),
                }],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![v(0)],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![(v(1), Idx::STR)],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(2),
                    then_block: b(2),
                    else_block: b(3),
                },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(3),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned; 1],
                }],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![v(3)],
                },
            },
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
        ],
        vec![Idx::STR, Idx::STR, Idx::BOOL, Idx::STR],
    );

    let result = run_rc_insert(func);

    // No Inc for v1 in b2 — it's a single use (consumed by Apply).
    assert_eq!(count_inc(&result, 2, v(1)), 0);
    // Block 3: v1 used in Return. Single use, transfers ownership.
    assert_eq!(count_rc_ops(&result, 3), 0);
}

/// Unused block param — block param never used in block body.
#[test]
fn unused_block_param_dec() {
    // Block 0: jump to b1 with v0 (str)
    // Block 1: param v1: str; let v2 = "other"; return v2
    //
    // v1 is a block param but never used → Dec.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![v(0)],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![(v(1), Idx::STR)],
                body: vec![ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::STR,
                    value: ArcValue::Literal(LitValue::String(Name::from_raw(100))),
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            },
        ],
        vec![Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_rc_insert(func);

    // v1 (block param) unused in b1 → Dec.
    assert_eq!(count_dec(&result, 1, v(1)), 1);
    // v2 used in return → no extra ops.
    assert_eq!(count_dec(&result, 1, v(2)), 0);
}

/// All-int function — zero RC ops.
#[test]
fn scalars_untouched() {
    let func = make_func(
        vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: v(2),
                ty: Idx::INT,
                value: ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                    args: vec![v(0), v(1)],
                },
            }],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::INT, Idx::INT, Idx::INT],
    );

    let result = run_rc_insert(func);

    assert_eq!(count_rc_ops(&result, 0), 0);
}

/// Early exit cleanup: one branch returns early, the other continues.
/// The early-exit branch must Dec all live RC'd variables that it
/// doesn't return. This demonstrates that the liveness-based RC
/// insertion naturally handles break/continue/early-return patterns
/// (Section 07.5).
///
/// ```text
/// block_0:
///   %s1 = construct str  // live in both branches
///   %s2 = construct str  // live in both branches
///   %cond = ...
///   branch %cond → b1, b2
///
/// block_1 (early exit):  // returns s1, must Dec s2
///   return s1
///
/// block_2 (continues):   // uses both, consumes both
///   %r = apply f(s1, s2)
///   return r
/// ```
#[test]
fn early_exit_cleanup() {
    let func = make_func(
        vec![],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(50)),
                        args: vec![],
                    },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(51)),
                        args: vec![],
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(2),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            // Early exit: returns s1, does NOT use s2.
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
            // Normal path: uses both s1 and s2.
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(3),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(0), v(1)],
                    arg_ownership: vec![ArgOwnership::Owned; 2],
                }],
                terminator: ArcTerminator::Return { value: v(3) },
            },
        ],
        vec![Idx::STR, Idx::STR, Idx::BOOL, Idx::STR],
    );

    let result = run_rc_insert(func);

    // Block 1 (early exit): s2 (v1) is live but not used → must be Dec'd.
    assert_eq!(count_dec(&result, 1, v(1)), 1);
    // Block 1: s1 (v0) is returned → no Dec.
    assert_eq!(count_dec(&result, 1, v(0)), 0);

    // Block 2 (normal): both s1 and s2 consumed by Apply → no extra Dec.
    assert_eq!(count_dec(&result, 2, v(0)), 0);
    assert_eq!(count_dec(&result, 2, v(1)), 0);
}

/// Early exit in a loop (break pattern): loop body uses s1, but the
/// break branch exits while s1 is still live. Must Dec s1 on exit.
///
/// ```text
/// block_0: let s1 = "hello"; jump to b1
/// block_1: branch cond → b2 (break), b3 (body)
/// block_2 (break exit): return unit  // must Dec s1
/// block_3 (body): apply f(s1); jump to b1
/// ```
#[test]
fn break_from_loop_cleanup() {
    let func = make_func(
        vec![],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(50)),
                        args: vec![],
                    },
                    ArcInstr::Let {
                        dst: v(1),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            // Loop header: branch to break or body.
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(1),
                    then_block: b(2),
                    else_block: b(3),
                },
            },
            // Break exit: return unit. s1 is live here → must Dec.
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(2),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(LitValue::Unit),
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            },
            // Loop body: uses s1, then loops back.
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(3),
                    ty: Idx::UNIT,
                    func: Name::from_raw(99),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned; 1],
                }],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
        ],
        vec![Idx::STR, Idx::BOOL, Idx::UNIT, Idx::UNIT],
    );

    let result = run_rc_insert(func);

    // Block 2 (break): s1 (v0) is live but not used → Dec.
    assert_eq!(count_dec(&result, 2, v(0)), 1);
    // Block 3 (body): s1 used in Apply. It's also live out (loops back
    // to b1 where s1 is live), so it gets an Inc before Apply.
    assert_eq!(count_inc(&result, 3, v(0)), 1);
}

/// Duplicate var in single instruction — `Apply { args: [x, x] }`.
/// Should produce exactly 1 Inc.
#[test]
fn duplicate_in_single_instr() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: v(1),
                ty: Idx::STR,
                func: Name::from_raw(99),
                args: vec![v(0), v(0)],
                arg_ownership: vec![ArgOwnership::Owned; 2],
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::STR],
    );

    let result = run_rc_insert(func);

    // x appears twice → 1 Inc.
    assert_eq!(count_inc(&result, 0, v(0)), 1);
    // No Dec for x (it's used).
    assert_eq!(count_dec(&result, 0, v(0)), 0);
}

/// Switch with asymmetric edge cleanup: three branches where only
/// some paths use each variable.
///
/// ```text
/// block_0: v0(str), v1(str), v2(int); switch v2 → b1(0), b2(1), b3(default)
/// block_1: return v0         // must Dec v1
/// block_2: return v1         // must Dec v0
/// block_3: apply f(v0, v1)   // uses both
/// ```
#[test]
fn switch_asymmetric_cleanup() {
    let func = make_func(
        vec![],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(50)),
                        args: vec![],
                    },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(51)),
                        args: vec![],
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::INT,
                        value: ArcValue::Literal(LitValue::Int(0)),
                    },
                ],
                terminator: ArcTerminator::Switch {
                    scrutinee: v(2),
                    cases: vec![(0, b(1)), (1, b(2))],
                    default: b(3),
                },
            },
            // Case 0: returns v0, doesn't use v1.
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
            // Case 1: returns v1, doesn't use v0.
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            // Default: uses both.
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(3),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(0), v(1)],
                    arg_ownership: vec![ArgOwnership::Owned; 2],
                }],
                terminator: ArcTerminator::Return { value: v(3) },
            },
        ],
        vec![Idx::STR, Idx::STR, Idx::INT, Idx::STR],
    );

    let result = run_rc_insert(func);

    // Block 1: v1 not used → Dec v1 (edge cleanup).
    assert_eq!(count_dec(&result, 1, v(1)), 1);
    assert_eq!(count_dec(&result, 1, v(0)), 0);

    // Block 2: v0 not used → Dec v0 (edge cleanup).
    assert_eq!(count_dec(&result, 2, v(0)), 1);
    assert_eq!(count_dec(&result, 2, v(1)), 0);

    // Block 3: both consumed by Apply → no extra Dec.
    assert_eq!(count_dec(&result, 3, v(0)), 0);
    assert_eq!(count_dec(&result, 3, v(1)), 0);
}

/// Multiple RC'd vars in edge cleanup: early exit must Dec ALL
/// stranded variables, not just one.
///
/// ```text
/// block_0: v0(str), v1(str), v2(str); branch → b1, b2
/// block_1: return unit       // must Dec v0, v1, v2
/// block_2: apply f(v0, v1, v2)
/// ```
#[test]
fn edge_cleanup_multiple_vars() {
    let func = make_func(
        vec![],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(50)),
                        args: vec![],
                    },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(51)),
                        args: vec![],
                    },
                    ArcInstr::Construct {
                        dst: v(2),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(52)),
                        args: vec![],
                    },
                    ArcInstr::Let {
                        dst: v(3),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(3),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            // Early exit: no RC'd vars used.
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Let {
                    dst: v(4),
                    ty: Idx::UNIT,
                    value: ArcValue::Literal(LitValue::Unit),
                }],
                terminator: ArcTerminator::Return { value: v(4) },
            },
            // Normal: uses all three.
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(5),
                    ty: Idx::UNIT,
                    func: Name::from_raw(99),
                    args: vec![v(0), v(1), v(2)],
                    arg_ownership: vec![ArgOwnership::Owned; 3],
                }],
                terminator: ArcTerminator::Return { value: v(5) },
            },
        ],
        vec![
            Idx::STR,
            Idx::STR,
            Idx::STR,
            Idx::BOOL,
            Idx::UNIT,
            Idx::UNIT,
        ],
    );

    let result = run_rc_insert(func);

    // Block 1: all three str vars need Dec.
    assert_eq!(count_dec(&result, 1, v(0)), 1);
    assert_eq!(count_dec(&result, 1, v(1)), 1);
    assert_eq!(count_dec(&result, 1, v(2)), 1);
}

/// Edge cleanup with multi-predecessor same gap: merge block b3
/// reached by two branches (b1 and b2) that both have the same
/// stranded variable v1. Both b1 and b2 also branch to blocks
/// that DO use v1, keeping it in their `live_out`.
///
/// ```text
/// block_0: v0(str), v1(str), v2(bool), v3(bool)
///          branch v2 → b1, b2
/// block_1: branch v3 → b3, b4
/// block_2: branch v3 → b3, b5
/// block_3: return v0           // v1 stranded from BOTH b1 and b2
/// block_4: return v1           // uses v1 (keeps it live in b1)
/// block_5: return v1           // uses v1 (keeps it live in b2)
/// ```
///
/// Here v1 is in `live_out[b1]` and `live_out[b2]` because b4/b5 need
/// it, but b3 only uses v0. So gap(b1→b3) = gap(b2→b3) = {v1}.
/// Since b3 has two predecessors with the same gap, edge cleanup
/// inserts Dec v1 at b3's start.
#[test]
fn multi_pred_same_gap() {
    let func = make_func(
        vec![],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(50)),
                        args: vec![],
                    },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(51)),
                        args: vec![],
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                    ArcInstr::Let {
                        dst: v(3),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(false)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(2),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            // b1: branches to b3 (uses v0, not v1) and b4 (uses v1)
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(3),
                    then_block: b(3),
                    else_block: b(4),
                },
            },
            // b2: branches to b3 (uses v0, not v1) and b5 (uses v1)
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Branch {
                    cond: v(3),
                    then_block: b(3),
                    else_block: b(5),
                },
            },
            // b3: uses v0, v1 is stranded (from both b1 and b2)
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
            // b4: uses v1
            ArcBlock {
                id: b(4),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            // b5: uses v1
            ArcBlock {
                id: b(5),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
        ],
        vec![Idx::STR, Idx::STR, Idx::BOOL, Idx::BOOL],
    );

    let result = run_rc_insert(func);

    // b3 has two predecessors (b1, b2). Both have gap = {v1}.
    // Since gaps are identical, edge cleanup inserts Dec v1 at b3's start.
    assert_eq!(count_dec(&result, 3, v(1)), 1);
    // v0 is returned in b3 → no Dec for v0.
    assert_eq!(count_dec(&result, 3, v(0)), 0);

    // b4: v0 is stranded (b1's live_out has v0, b4 only uses v1).
    assert_eq!(count_dec(&result, 4, v(0)), 1);
    // b5: v0 is stranded similarly.
    assert_eq!(count_dec(&result, 5, v(0)), 1);
}

/// Invoke: live str variable gets `RcDec` in unwind block.
///
/// When an Invoke's unwind block is reached, all RC'd variables that
/// were live at the invoke point (but NOT the invoke's dst) must be
/// Dec'd for cleanup.
///
/// ```text
/// block_0:
///   %s = construct str "hello"
///   invoke f(%s) → dst=%r, normal=b1, unwind=b2
///
/// block_1 (normal):
///   return %r
///
/// block_2 (unwind):
///   resume   // edge cleanup must insert RcDec(%s) here
/// ```
#[test]
fn invoke_unwind_cleanup() {
    let func = make_func(
        vec![],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::Construct {
                    dst: v(0),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(50)),
                    args: vec![],
                }],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned; 1],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            // Normal continuation: return the invoke result.
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            // Unwind block: initially just Resume.
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::STR, Idx::STR],
    );

    let result = run_rc_insert(func);

    // v0 (str) is consumed by the Invoke call (it's an arg),
    // so it's NOT stranded — no RcDec needed in unwind.
    // But if v0 had survived (e.g., used after invoke in normal block),
    // it would need cleanup. Let's verify no spurious Decs.

    // v1 (invoke dst) should NOT be Dec'd in unwind — it's never
    // produced on the unwind path.
    // Check that the unwind block's body is handled properly.
    let unwind_idx = 2;
    assert_eq!(
        count_dec(&result, unwind_idx, v(1)),
        0,
        "invoke dst must NOT be Dec'd in unwind block"
    );
}

/// Invoke with multiple live variables: ALL stranded vars get cleanup.
///
/// ```text
/// block_0:
///   %s1 = construct str
///   %s2 = construct str
///   invoke f() → dst=%r, normal=b1, unwind=b2
///
/// block_1 (normal):
///   apply g(%s1, %s2)
///   return %r
///
/// block_2 (unwind):
///   resume   // must insert RcDec(%s1), RcDec(%s2)
/// ```
#[test]
fn invoke_unwind_cleanup_multiple_vars() {
    let func = make_func(
        vec![],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Construct {
                        dst: v(0),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(50)),
                        args: vec![],
                    },
                    ArcInstr::Construct {
                        dst: v(1),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(51)),
                        args: vec![],
                    },
                ],
                // Invoke with NO args (doesn't consume s1 or s2).
                terminator: ArcTerminator::Invoke {
                    dst: v(2),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![],
                    arg_ownership: vec![],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            // Normal: uses s1, s2, and the invoke result.
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(3),
                    ty: Idx::UNIT,
                    func: Name::from_raw(98),
                    args: vec![v(0), v(1)],
                    arg_ownership: vec![ArgOwnership::Owned; 2],
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            },
            // Unwind: just Resume.
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::STR, Idx::STR, Idx::STR, Idx::UNIT],
    );

    let result = run_rc_insert(func);

    // The unwind block may have been replaced by an edge-split trampoline.
    // Find the block that predecessors b0 and has Resume terminator.
    // The edge cleanup may create a trampoline block that Decs and jumps
    // to the unwind block, OR it may insert Decs directly in the unwind
    // block if it's single-predecessor.
    //
    // With single predecessor (b0 is the only pred of unwind b2),
    // Decs are inserted at the start of b2.
    let unwind_idx = 2;
    assert_eq!(
        count_dec(&result, unwind_idx, v(0)),
        1,
        "s1 must be Dec'd in unwind block"
    );
    assert_eq!(
        count_dec(&result, unwind_idx, v(1)),
        1,
        "s2 must be Dec'd in unwind block"
    );
    // Invoke dst must NOT be Dec'd in unwind.
    assert_eq!(
        count_dec(&result, unwind_idx, v(2)),
        0,
        "invoke dst must NOT be Dec'd in unwind block"
    );
}

/// Invoke where dst is unused in normal block → gets Dec'd there.
#[test]
fn invoke_unused_dst_gets_dec_in_normal() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![],
                    arg_ownership: vec![],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            // Normal: returns v0 (param), ignores v1 (invoke dst).
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::STR, Idx::STR],
    );

    let result = run_rc_insert(func);

    // v1 (invoke dst) is unused in normal block → Dec it there.
    let normal_idx = 1;
    assert_eq!(
        count_dec(&result, normal_idx, v(1)),
        1,
        "unused invoke dst should be Dec'd in normal block"
    );
}

/// No edge cleanup needed when all paths use the same variables.
/// Diamond where both branches consume all live vars.
#[test]
fn no_edge_cleanup_symmetric() {
    let func = make_func(
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
            // Both branches use v0.
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(2),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned; 1],
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            },
        ],
        vec![Idx::STR, Idx::BOOL, Idx::STR],
    );

    let result = run_rc_insert(func);

    // No edge cleanup needed — v0 is used in both branches.
    assert_eq!(count_dec(&result, 1, v(0)), 0);
    assert_eq!(count_dec(&result, 2, v(0)), 0);
}

// insert_rc_ops_with_ownership tests

/// Run ownership-enhanced RC insertion on a function (empty sigs).
fn run_rc_insert_enhanced(mut func: ArcFunction) -> ArcFunction {
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let liveness = compute_liveness(&func, &classifier);
    let sigs = FxHashMap::default();
    let ownership = infer_derived_ownership(&func, &sigs);
    insert_rc_ops_with_ownership(&mut func, &classifier, &liveness, &ownership, &sigs, &pool);
    func
}

/// Run ownership-enhanced RC insertion with provided signatures.
fn run_rc_insert_enhanced_with_sigs(
    mut func: ArcFunction,
    sigs: &FxHashMap<Name, crate::ownership::AnnotatedSig>,
) -> ArcFunction {
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let liveness = compute_liveness(&func, &classifier);
    let ownership = infer_derived_ownership(&func, sigs);
    insert_rc_ops_with_ownership(&mut func, &classifier, &liveness, &ownership, sigs, &pool);
    func
}

/// Single-block passthrough: enhanced produces same result as original.
#[test]
fn enhanced_passthrough_matches_original() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let result = run_rc_insert_enhanced(func);

    assert_eq!(count_rc_ops(&result, 0), 0);
}

/// Single-block borrowed projection: enhanced matches original behavior.
#[test]
fn enhanced_borrowed_projection_stored() {
    let func = make_func(
        vec![borrowed_param(0, Idx::STR)],
        Idx::UNIT,
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
                ArcInstr::Construct {
                    dst: v(2),
                    ty: Idx::UNIT,
                    ctor: CtorKind::Tuple,
                    args: vec![v(1)],
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::STR, Idx::UNIT],
    );

    let result = run_rc_insert_enhanced(func);

    // v1 borrowed-derived, stored in Construct (owned position) → Inc.
    assert_eq!(count_inc(&result, 0, v(1)), 1);
    // v0 borrowed → no RC ops.
    assert_eq!(count_inc(&result, 0, v(0)), 0);
    assert_eq!(count_dec(&result, 0, v(0)), 0);
}

/// Cross-block borrow propagation: borrowed-derived var used in owned
/// position in a different block gets the necessary Inc.
///
/// ```text
/// block_0: v0 = @borrow param(str); v1 = project v0.0 (str);
///          branch → b1, b2
/// block_1: apply f(v1) → v1 needs Inc (owned position, cross-block)
/// block_2: return v0
/// ```
///
/// The per-block `compute_borrows` misses v1's borrowed status in B1
/// because the Project defining v1 is in B0. `DerivedOwnership` knows
/// v1 is `BorrowedFrom(v0)` globally.
#[test]
fn enhanced_cross_block_borrow_inc() {
    let func = make_func(
        vec![borrowed_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(2),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Apply {
                    dst: v(3),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned; 1],
                }],
                terminator: ArcTerminator::Return { value: v(3) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(0) },
            },
        ],
        vec![Idx::STR, Idx::STR, Idx::BOOL, Idx::STR],
    );

    let result = run_rc_insert_enhanced(func);

    // v1 in B1: BorrowedFrom(v0) globally → Apply is owned position → Inc.
    assert_eq!(
        count_inc(&result, 1, v(1)),
        1,
        "cross-block borrowed-derived v1 needs Inc at owned position in B1"
    );
}

// Closure capture analysis tests (Step 2.4)

/// Closure capturing borrowed-derived var at a Borrowed callee position:
/// the Inc is skipped because the closure borrows (not owns) the value,
/// and the closure is consumed in the same block (non-escaping).
///
/// ```text
/// fn outer(@borrow p: str) -> str {
///     let field = p.0              // BorrowedFrom(p)
///     let closure = partial_apply(inner, field)
///     apply(closure)               // consumed immediately
/// }
/// // inner(@borrow x: str) -> str  ← param is Borrowed
/// ```
#[test]
fn closure_borrowed_capture_no_inc() {
    use crate::ownership::{AnnotatedParam, AnnotatedSig};

    let inner_name = Name::from_raw(42);

    // inner's signature: @borrow param of str → str
    let mut sigs = FxHashMap::default();
    sigs.insert(
        inner_name,
        AnnotatedSig {
            params: vec![AnnotatedParam {
                name: Name::from_raw(100),
                ty: Idx::STR,
                ownership: crate::ownership::Ownership::Borrowed,
            }],
            return_type: Idx::STR,
        },
    );

    let func = make_func(
        vec![borrowed_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                // v1 = project v0.0 (str) — BorrowedFrom(v0)
                ArcInstr::Project {
                    dst: v(1),
                    ty: Idx::STR,
                    value: v(0),
                    field: 0,
                },
                // v2 = partial_apply(inner, v1) — capture v1
                ArcInstr::PartialApply {
                    dst: v(2),
                    ty: Idx::STR,
                    func: inner_name,
                    args: vec![v(1)],
                },
                // v3 = apply(v2) — consume closure immediately
                ArcInstr::ApplyIndirect {
                    dst: v(3),
                    ty: Idx::STR,
                    closure: v(2),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(3) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_rc_insert_enhanced_with_sigs(func, &sigs);

    // v1 is BorrowedFrom(v0), captured at a Borrowed callee position,
    // and the closure doesn't escape → no Inc needed.
    assert_eq!(
        count_inc(&result, 0, v(1)),
        0,
        "borrowed capture at Borrowed position should skip Inc"
    );
}

/// Closure capturing borrowed-derived var at an Owned callee position:
/// the Inc is required (callee will consume the value).
#[test]
fn closure_owned_capture_gets_inc() {
    use crate::ownership::{AnnotatedParam, AnnotatedSig};

    let inner_name = Name::from_raw(42);

    // inner's signature: owned param of str → str
    let mut sigs = FxHashMap::default();
    sigs.insert(
        inner_name,
        AnnotatedSig {
            params: vec![AnnotatedParam {
                name: Name::from_raw(100),
                ty: Idx::STR,
                ownership: crate::ownership::Ownership::Owned,
            }],
            return_type: Idx::STR,
        },
    );

    let func = make_func(
        vec![borrowed_param(0, Idx::STR)],
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
                ArcInstr::PartialApply {
                    dst: v(2),
                    ty: Idx::STR,
                    func: inner_name,
                    args: vec![v(1)],
                },
                ArcInstr::ApplyIndirect {
                    dst: v(3),
                    ty: Idx::STR,
                    closure: v(2),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(3) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_rc_insert_enhanced_with_sigs(func, &sigs);

    // v1 captured at Owned position → Inc required.
    assert_eq!(
        count_inc(&result, 0, v(1)),
        1,
        "borrowed capture at Owned position needs Inc"
    );
}

/// Escaping closure: even if callee param is Borrowed, the closure
/// escapes the block (used in a later block), so Inc is required.
#[test]
fn closure_escaping_borrowed_still_inc() {
    use crate::ownership::{AnnotatedParam, AnnotatedSig};

    let inner_name = Name::from_raw(42);

    let mut sigs = FxHashMap::default();
    sigs.insert(
        inner_name,
        AnnotatedSig {
            params: vec![AnnotatedParam {
                name: Name::from_raw(100),
                ty: Idx::STR,
                ownership: crate::ownership::Ownership::Borrowed,
            }],
            return_type: Idx::STR,
        },
    );

    // b0: project v1 from v0, partial_apply → v2, jump to b1
    // b1: apply_indirect(v2) → closure escapes b0
    let func = make_func(
        vec![borrowed_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::STR,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::PartialApply {
                        dst: v(2),
                        ty: Idx::STR,
                        func: inner_name,
                        args: vec![v(1)],
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
                body: vec![ArcInstr::ApplyIndirect {
                    dst: v(3),
                    ty: Idx::STR,
                    closure: v(2),
                    args: vec![],
                }],
                terminator: ArcTerminator::Return { value: v(3) },
            },
        ],
        vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_rc_insert_enhanced_with_sigs(func, &sigs);

    // v2 (closure) is live_out of b0 → escapes → must Inc v1.
    assert_eq!(
        count_inc(&result, 0, v(1)),
        1,
        "escaping closure must Inc borrowed capture even at Borrowed position"
    );
}

// External invoke cleanup tests

/// External invoke — `fn(x: str) { ori_print(x) }`.
/// Runtime functions borrow args without Dec — caller must Dec after invoke.
#[test]
fn external_invoke_args_get_dec() {
    // Block 0: invoke ori_print(v0:str) → normal=b(1), unwind=b(2)
    // Block 1: return unit
    // Block 2: unreachable (unwind landing)
    let interner = StringInterner::new();
    let external_fn = interner.intern("ori_print");
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::UNIT,
                    func: external_fn,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned; 1],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        vec![Idx::STR, Idx::UNIT],
    );

    // Pre-pass: annotate arg ownership from sigs/interner for external callees.
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let sigs = FxHashMap::default();
    annotate_arg_ownership(
        &mut func,
        &sigs,
        &interner,
        &crate::BuiltinOwnershipSets::empty(),
        &pool,
    );

    // Run RC insertion.
    let ownership = infer_derived_ownership(&func, &sigs);
    let liveness = compute_liveness(&func, &classifier);
    insert_rc_ops_with_ownership(&mut func, &classifier, &liveness, &ownership, &sigs, &pool);

    // Post-pass: insert Dec for last-use args to external invokes.
    insert_external_invoke_cleanup(&mut func, &classifier, &liveness, &pool);

    // Block 1 (normal successor) should have RcDec for v0 (the str arg).
    assert_eq!(
        count_dec(&func, 1, v(0)),
        1,
        "external invoke's RC-typed arg must get RcDec in normal successor"
    );
}

/// External invoke with scalar arg — no cleanup needed.
#[test]
fn external_invoke_scalar_arg_no_dec() {
    let interner = StringInterner::new();
    let external_fn = interner.intern("ori_print_int");
    let mut func = make_func(
        vec![owned_param(0, Idx::INT)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::UNIT,
                    func: external_fn,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned; 1],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        vec![Idx::INT, Idx::UNIT],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let sigs = FxHashMap::default();
    annotate_arg_ownership(
        &mut func,
        &sigs,
        &interner,
        &crate::BuiltinOwnershipSets::empty(),
        &pool,
    );
    let ownership = infer_derived_ownership(&func, &sigs);
    let liveness = compute_liveness(&func, &classifier);
    insert_rc_ops_with_ownership(&mut func, &classifier, &liveness, &ownership, &sigs, &pool);
    insert_external_invoke_cleanup(&mut func, &classifier, &liveness, &pool);

    // v0 is int (scalar) → no cleanup needed.
    assert_eq!(
        count_dec(&func, 1, v(0)),
        0,
        "scalar args should not get external invoke cleanup"
    );
}

/// Internal invoke — function IS in sigs → no cleanup (callee handles Dec).
#[test]
fn internal_invoke_no_cleanup() {
    let interner = StringInterner::new();
    let internal_fn = interner.intern("user_function");
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::UNIT,
                    func: internal_fn,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned; 1],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        vec![Idx::STR, Idx::UNIT],
    );

    // Register internal_fn in sigs → it's a known Ori function.
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let mut sigs = FxHashMap::default();
    sigs.insert(
        internal_fn,
        crate::ownership::AnnotatedSig {
            params: vec![crate::ownership::AnnotatedParam {
                name: internal_fn,
                ty: Idx::STR,
                ownership: crate::ownership::Ownership::Owned,
            }],
            return_type: Idx::UNIT,
        },
    );
    let ownership = infer_derived_ownership(&func, &sigs);
    let liveness = compute_liveness(&func, &classifier);
    insert_rc_ops_with_ownership(&mut func, &classifier, &liveness, &ownership, &sigs, &pool);
    insert_external_invoke_cleanup(&mut func, &classifier, &liveness, &pool);

    // Internal function is in sigs → callee handles Dec → no cleanup.
    assert_eq!(
        count_dec(&func, 1, v(0)),
        0,
        "internal invoke should not get external cleanup"
    );
}

/// User function NOT in sigs but without ori_ prefix → no cleanup.
/// Covers derived trait methods, default trait methods, etc.
#[test]
fn user_function_not_in_sigs_no_cleanup() {
    let interner = StringInterner::new();
    let user_fn = interner.intern("compare");
    let mut func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::UNIT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::UNIT,
                    func: user_fn,
                    args: vec![v(0)],
                    arg_ownership: vec![ArgOwnership::Owned; 1],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Unreachable,
            },
        ],
        vec![Idx::STR, Idx::UNIT],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let sigs = FxHashMap::default(); // empty — "compare" is NOT in sigs
    let ownership = infer_derived_ownership(&func, &sigs);
    let liveness = compute_liveness(&func, &classifier);
    insert_rc_ops_with_ownership(&mut func, &classifier, &liveness, &ownership, &sigs, &pool);
    insert_external_invoke_cleanup(&mut func, &classifier, &liveness, &pool);

    // "compare" has no ori_ prefix → treated as user function → no cleanup.
    assert_eq!(
        count_dec(&func, 1, v(0)),
        0,
        "user function without ori_ prefix should not get external cleanup"
    );
}

// Cross-block borrowing projection tests (Section 04.5)

/// Try operator (`?`) pattern: scalar tag Project borrows scrut across the
/// branch, consuming non-scalar Project on the error path transfers ownership.
///
/// Models `Result<int, str>`:
/// ```text
/// B0 (entry):
///   v0 = param(Result)           [owned, needs RC]
///   v1 = Project(v0, field=0)    [ty=INT → scalar → borrowing]
///   v2 = Literal(true)
///   Branch(v2, B1, B2)
///
/// B1 (ok):
///   v3 = Project(v0, field=1)    [ty=INT → scalar → borrowing]
///   Return v3
///
/// B2 (err):
///   v4 = Project(v0, field=1)    [ty=STR → non-scalar → consuming]
///   v5 = Construct Err(v4)
///   Return v5
/// ```
///
/// Expected:
/// - B0: v0 live-out to both successors → no Dec
/// - B1: v0's last use is the borrowing Project → Dec(v0)
/// - B2: non-scalar Project consumes v0 → no Dec(v0)
#[test]
fn try_operator_pattern_result_int_str() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            // B0: tag extraction + branch
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::INT,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(2),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            // B1 (ok): scalar payload extraction — borrowing
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: v(3),
                    ty: Idx::INT,
                    value: v(0),
                    field: 1,
                }],
                terminator: ArcTerminator::Return { value: v(3) },
            },
            // B2 (err): non-scalar payload extraction — consuming
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(4),
                        ty: Idx::STR,
                        value: v(0),
                        field: 1,
                    },
                    ArcInstr::Construct {
                        dst: v(5),
                        ty: Idx::STR,
                        ctor: CtorKind::EnumVariant {
                            enum_name: Name::from_raw(50),
                            variant: 1,
                        },
                        args: vec![v(4)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(5) },
            },
        ],
        vec![Idx::STR, Idx::INT, Idx::BOOL, Idx::INT, Idx::STR, Idx::STR],
    );

    let result = run_rc_insert(func);

    // B0: v0 is live-out (both successors use it) → no Dec.
    assert_eq!(
        count_dec(&result, 0, v(0)),
        0,
        "scrut must NOT be Dec'd at branch — still live-out"
    );

    // B1 (ok): scalar Project borrows v0, last use on this path → Dec.
    assert_eq!(
        count_dec(&result, 1, v(0)),
        1,
        "scrut must be Dec'd in ok block after last borrowing use"
    );

    // B2 (err): non-scalar Project consumes v0 (ownership → v4) → no Dec.
    assert_eq!(
        count_dec(&result, 2, v(0)),
        0,
        "scrut must NOT be Dec'd in err block — consuming Project transfers ownership"
    );
}

/// Cross-block liveness with borrowing projections on both paths.
///
/// When BOTH successors use only scalar (borrowing) projections, the
/// scrutinee must be Dec'd at the last use in EACH successor separately.
///
/// ```text
/// B0:
///   v0 = param(str-like)         [owned, needs RC]
///   v1 = Project(v0, field=0)    [ty=INT → borrowing]
///   v2 = Literal(true)
///   Branch(v2, B1, B2)
///
/// B1: v3 = Project(v0, field=1) [ty=INT → borrowing]; Return v3
/// B2: v4 = Project(v0, field=2) [ty=INT → borrowing]; Return v4
/// ```
#[test]
fn borrowing_projection_cross_block_liveness() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::INT,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(1),
                        ty: Idx::INT,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: v(2),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(2),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: v(3),
                    ty: Idx::INT,
                    value: v(0),
                    field: 1,
                }],
                terminator: ArcTerminator::Return { value: v(3) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: v(4),
                    ty: Idx::INT,
                    value: v(0),
                    field: 2,
                }],
                terminator: ArcTerminator::Return { value: v(4) },
            },
        ],
        vec![Idx::STR, Idx::INT, Idx::BOOL, Idx::INT, Idx::INT],
    );

    let result = run_rc_insert(func);

    // B0: v0 live-out → no Dec.
    assert_eq!(
        count_dec(&result, 0, v(0)),
        0,
        "v0 in live_out(B0) — no Dec at branch"
    );

    // B1: borrowing Project is last use → Dec(v0).
    assert_eq!(
        count_dec(&result, 1, v(0)),
        1,
        "v0 must be Dec'd at last borrowing use in B1"
    );

    // B2: borrowing Project is last use → Dec(v0).
    assert_eq!(
        count_dec(&result, 2, v(0)),
        1,
        "v0 must be Dec'd at last borrowing use in B2"
    );
}

/// Chained `?` — two sequential try patterns with no leaks.
///
/// ```text
/// B0:
///   v0 = param(Result1)  v1 = param(Result2)
///   v2 = Project(v0, 0) [scalar→borrow]  v3 = Literal(true)
///   Branch(v3, B1, B2)
///
/// B1 (first ok):
///   v4 = Project(v0, 1) [scalar→borrow]  ← Dec(v0) here
///   v5 = Project(v1, 0) [scalar→borrow]  v6 = Literal(true)
///   Branch(v6, B3, B4)
///
/// B2 (first err):
///   v7 = Project(v0, 1) [non-scalar→consume]
///   v8 = Construct Err(v7)
///   Return v8                              ← Dec(v1) here (stranded)
///
/// B3 (second ok):
///   v9 = Project(v1, 1) [scalar→borrow]   ← Dec(v1) here
///   Return v9
///
/// B4 (second err):
///   v10 = Project(v1, 1) [non-scalar→consume]
///   v11 = Construct Err(v10)
///   Return v11
/// ```
#[test]
#[expect(
    clippy::too_many_lines,
    reason = "integration test with complex multi-block CFG"
)]
fn chained_try_no_leak() {
    let func = make_func(
        vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
        Idx::STR,
        vec![
            // B0: first tag extraction + branch
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(2),
                        ty: Idx::INT,
                        value: v(0),
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: v(3),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(3),
                    then_block: b(1),
                    else_block: b(2),
                },
            },
            // B1 (first ok): extract first payload, then try second
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(4),
                        ty: Idx::INT,
                        value: v(0),
                        field: 1,
                    },
                    ArcInstr::Project {
                        dst: v(5),
                        ty: Idx::INT,
                        value: v(1),
                        field: 0,
                    },
                    ArcInstr::Let {
                        dst: v(6),
                        ty: Idx::BOOL,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                terminator: ArcTerminator::Branch {
                    cond: v(6),
                    then_block: b(3),
                    else_block: b(4),
                },
            },
            // B2 (first err): consume first, strand second
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(7),
                        ty: Idx::STR,
                        value: v(0),
                        field: 1,
                    },
                    ArcInstr::Construct {
                        dst: v(8),
                        ty: Idx::STR,
                        ctor: CtorKind::EnumVariant {
                            enum_name: Name::from_raw(50),
                            variant: 1,
                        },
                        args: vec![v(7)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(8) },
            },
            // B3 (second ok): extract second payload (scalar → borrowing)
            ArcBlock {
                id: b(3),
                params: vec![],
                body: vec![ArcInstr::Project {
                    dst: v(9),
                    ty: Idx::INT,
                    value: v(1),
                    field: 1,
                }],
                terminator: ArcTerminator::Return { value: v(9) },
            },
            // B4 (second err): consume second
            ArcBlock {
                id: b(4),
                params: vec![],
                body: vec![
                    ArcInstr::Project {
                        dst: v(10),
                        ty: Idx::STR,
                        value: v(1),
                        field: 1,
                    },
                    ArcInstr::Construct {
                        dst: v(11),
                        ty: Idx::STR,
                        ctor: CtorKind::EnumVariant {
                            enum_name: Name::from_raw(50),
                            variant: 1,
                        },
                        args: vec![v(10)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(11) },
            },
        ],
        vec![
            Idx::STR,  // v0: first Result
            Idx::STR,  // v1: second Result
            Idx::INT,  // v2: first tag
            Idx::BOOL, // v3: branch cond
            Idx::INT,  // v4: first ok payload
            Idx::INT,  // v5: second tag
            Idx::BOOL, // v6: branch cond
            Idx::STR,  // v7: first err payload
            Idx::STR,  // v8: Construct result
            Idx::INT,  // v9: second ok payload
            Idx::STR,  // v10: second err payload
            Idx::STR,  // v11: Construct result
        ],
    );

    let result = run_rc_insert(func);

    // First Result (v0)

    // B0: v0 live-out (used in B1, B2) → no Dec.
    assert_eq!(
        count_dec(&result, 0, v(0)),
        0,
        "first scrut live-out from B0"
    );
    // B1: scalar Project borrows v0, last use → Dec.
    assert_eq!(
        count_dec(&result, 1, v(0)),
        1,
        "first scrut Dec'd at last borrowing use in B1"
    );
    // B2: non-scalar Project consumes v0 → no Dec.
    assert_eq!(
        count_dec(&result, 2, v(0)),
        0,
        "first scrut consumed by non-scalar Project in B2"
    );

    // Second Result (v1)

    // B0: v1 live-out (used in B1, and transitively in B3/B4 via B1) → no Dec.
    assert_eq!(
        count_dec(&result, 0, v(1)),
        0,
        "second scrut live-out from B0"
    );
    // B1: v1 live-out (used in B3, B4) → no Dec.
    assert_eq!(
        count_dec(&result, 1, v(1)),
        0,
        "second scrut live-out from B1"
    );
    // B2: v1 stranded (not used in B2) → edge cleanup Dec.
    assert_eq!(
        count_dec(&result, 2, v(1)),
        1,
        "second scrut stranded in B2 — edge cleanup Dec"
    );
    // B3: scalar Project borrows v1, last use → Dec.
    assert_eq!(
        count_dec(&result, 3, v(1)),
        1,
        "second scrut Dec'd at last borrowing use in B3"
    );
    // B4: non-scalar Project consumes v1 → no Dec.
    assert_eq!(
        count_dec(&result, 4, v(1)),
        0,
        "second scrut consumed by non-scalar Project in B4"
    );
}

// List Binary(Add) consuming semantics

/// List `+` operator — operands must NOT get `RcDec` because the COW concat
/// function (`ori_list_concat_cow`) consumes both the receiver and list2.
///
/// ```text
/// fn f(xs: [int], ys: [int]) -> [int] {
///     xs + ys   // PrimOp::Binary(Add) on list types
/// }
/// ```
///
/// Before the fix, `is_borrowing_instr` treated ALL `PrimOps` as borrowing,
/// which emitted `RcDec` for both operands (double-free). After the fix,
/// `Binary(Add)` on list types is consuming — no `RcDec` on operands.
#[test]
fn list_add_consuming_no_dec_on_operands() {
    let mut pool = Pool::new();
    let list_int = pool.list(Idx::INT);

    let func = make_func(
        vec![owned_param(0, list_int), owned_param(1, list_int)],
        list_int,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: v(2),
                ty: list_int,
                value: ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                    args: vec![v(0), v(1)],
                },
            }],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![list_int, list_int, list_int],
    );

    let classifier = ArcClassifier::new(&pool);
    let liveness = compute_liveness(&func, &classifier);
    let sigs = FxHashMap::default();
    let ownership = infer_derived_ownership(&func, &sigs);
    let mut result = func;
    insert_rc_ops_with_ownership(
        &mut result,
        &classifier,
        &liveness,
        &ownership,
        &sigs,
        &pool,
    );

    // List + list → consuming: NO RcDec on either operand.
    // The COW concat function handles RC lifecycle internally.
    assert_eq!(
        count_dec(&result, 0, v(0)),
        0,
        "list Add receiver must NOT get RcDec — consumed by COW concat"
    );
    assert_eq!(
        count_dec(&result, 0, v(1)),
        0,
        "list Add second arg must NOT get RcDec — consumed by COW concat"
    );
}

/// Int `+` operator — operands are scalar, no RC ops at all.
/// Sanity check that the list-specific consuming override doesn't affect scalars.
#[test]
fn int_add_still_scalar_no_rc() {
    let pool = Pool::new();

    let func = make_func(
        vec![owned_param(0, Idx::INT), owned_param(1, Idx::INT)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: v(2),
                ty: Idx::INT,
                value: ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Add),
                    args: vec![v(0), v(1)],
                },
            }],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::INT, Idx::INT, Idx::INT],
    );

    let classifier = ArcClassifier::new(&pool);
    let liveness = compute_liveness(&func, &classifier);
    let sigs = FxHashMap::default();
    let ownership = infer_derived_ownership(&func, &sigs);
    let mut result = func;
    insert_rc_ops_with_ownership(
        &mut result,
        &classifier,
        &liveness,
        &ownership,
        &sigs,
        &pool,
    );

    // Scalars: zero RC ops total (same as `scalars_untouched` test).
    assert_eq!(
        count_rc_ops(&result, 0),
        0,
        "int Add should have zero RC ops"
    );
}

// Duplicate arg dedup tests

/// Duplicate borrowing arg — `PrimOp { args: [v0, v0] }` should produce
/// exactly 1 `RcDec`, not 2 (double-free).
///
/// A `PrimOp` borrows its args. When the same variable appears twice in the
/// arg list and it's at its last use, the backward walk would emit 2 `RcDec`
/// without dedup — a double-free. The `seen` set prevents this.
#[test]
fn duplicate_borrowing_arg_single_dec() {
    // fn(x: str) -> int { x == x }
    //
    // v0: str (owned param)
    // v1: int (result of PrimOp Eq [v0, v0])
    //
    // v0 appears twice in the PrimOp args. It's at its last use (not in
    // the return), so it should get exactly 1 RcDec.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Let {
                dst: v(1),
                ty: Idx::INT,
                value: ArcValue::PrimOp {
                    op: crate::ir::PrimOp::Binary(ori_ir::BinaryOp::Eq),
                    args: vec![v(0), v(0)],
                },
            }],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::INT],
    );

    let result = run_rc_insert(func);

    // Exactly 1 RcDec for v0 — not 2.
    assert_eq!(
        count_dec(&result, 0, v(0)),
        1,
        "duplicate PrimOp arg should produce exactly 1 RcDec, not 2"
    );
}

/// Duplicate borrowed Invoke args — `Invoke { args: [v0, v0], ownership:
/// [Borrowed, Borrowed] }` should produce exactly 1 cleanup `RcDec` at the
/// normal successor, not 2 (double-free).
#[test]
fn duplicate_invoke_borrowed_arg_single_cleanup() {
    // fn(x: str) -> str { invoke f(x, x) → normal b1, unwind b2 }
    //
    // v0: str (owned param)
    // v1: str (invoke result)
    //
    // Both Invoke args are v0, both marked Borrowed. v0 is at its last use
    // (not live-out of block 0). The invoke cleanup pass should emit exactly
    // 1 RcDec for v0 at the normal successor, not 2.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Invoke {
                    dst: v(1),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(0), v(0)],
                    arg_ownership: vec![ArgOwnership::Borrowed, ArgOwnership::Borrowed],
                    normal: b(1),
                    unwind: b(2),
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Return { value: v(1) },
            },
            ArcBlock {
                id: b(2),
                params: vec![],
                body: vec![],
                terminator: ArcTerminator::Resume,
            },
        ],
        vec![Idx::STR, Idx::STR],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let liveness = compute_liveness(&func, &classifier);

    let mut result = func;
    insert_external_invoke_cleanup(&mut result, &classifier, &liveness, &pool);

    // Normal successor (b1) should have exactly 1 RcDec for v0 — not 2.
    assert_eq!(
        count_dec(&result, 1, v(0)),
        1,
        "duplicate borrowed Invoke arg should produce exactly 1 cleanup RcDec, not 2"
    );
}

// Type resolution in annotation tests

/// Consuming-receiver override through a linked type variable.
///
/// When a receiver's type is `Var → Link → List<int>`, `pool.tag()` returns
/// `Tag::Var` unless `resolve_fully` is called first. Without resolution,
/// the consuming-receiver override is silently skipped and arg[0] stays
/// `Borrowed` instead of `Owned` — preventing the ARC pipeline from emitting
/// the correct RC for COW list methods.
#[test]
fn consuming_receiver_through_alias() {
    use crate::borrow::BuiltinOwnershipSets;
    use ori_ir::StringInterner;

    let mut pool = Pool::new();
    let interner = StringInterner::new();

    // Create List<int> via a type variable link:
    // Var(id=0) → Link → List<int>
    let list_int = pool.list(Idx::INT);
    let var = pool.fresh_var();
    let var_id = pool.data(var);
    *pool.var_state_mut(var_id) = ori_types::VarState::Link { target: list_int };

    // Callee name for a consuming-receiver method (e.g., "push").
    let push_name = interner.intern("push");

    // Build a function with: Apply { func: "push", args: [v0, v1] }
    // where v0 has the linked Var type (should resolve to List<int>).
    let mut func = make_func(
        vec![owned_param(0, var), owned_param(1, Idx::INT)],
        var,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![ArcInstr::Apply {
                dst: v(2),
                ty: var,
                func: push_name,
                args: vec![v(0), v(1)],
                arg_ownership: vec![],
            }],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![var, Idx::INT, var],
    );

    let sigs = FxHashMap::default();
    let mut builtins = BuiltinOwnershipSets {
        borrowing: FxHashSet::default(),
        consuming_receiver: FxHashSet::default(),
        consuming_second_arg: FxHashSet::default(),
        consuming_receiver_only: FxHashSet::default(),
        protocol: FxHashMap::default(),
    };
    builtins.consuming_receiver.insert(push_name);

    annotate_arg_ownership(&mut func, &sigs, &interner, &builtins, &pool);

    // After annotation, arg[0] (the receiver) should be Owned because
    // the resolved type is List<int> and "push" is a consuming-receiver method.
    let body_instr = &func.blocks[0].body[0];
    if let ArcInstr::Apply { arg_ownership, .. } = body_instr {
        assert_eq!(
            arg_ownership[0],
            ArgOwnership::Owned,
            "consuming-receiver override should apply through type variable link"
        );
    } else {
        panic!("expected Apply instruction");
    }
}
