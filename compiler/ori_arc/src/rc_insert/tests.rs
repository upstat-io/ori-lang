use ori_ir::Name;
use ori_types::{Idx, Pool};

use rustc_hash::FxHashMap;

use crate::borrow::infer_derived_ownership;
use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue};
use crate::liveness::compute_liveness;
use crate::test_helpers::{
    b, borrowed_param, count_block_rc_ops as count_rc_ops, count_dec, count_inc, make_func,
    owned_param, v,
};
use crate::ArcClassifier;

use super::{insert_rc_ops, insert_rc_ops_with_ownership};

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

// --- insert_rc_ops_with_ownership tests ---

/// Run ownership-enhanced RC insertion on a function (empty sigs).
fn run_rc_insert_enhanced(mut func: ArcFunction) -> ArcFunction {
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let liveness = compute_liveness(&func, &classifier);
    let sigs = FxHashMap::default();
    let ownership = infer_derived_ownership(&func, &sigs);
    insert_rc_ops_with_ownership(&mut func, &classifier, &liveness, &ownership, &sigs);
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
    insert_rc_ops_with_ownership(&mut func, &classifier, &liveness, &ownership, sigs);
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

// --- Closure capture analysis tests (Step 2.4) ---

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
