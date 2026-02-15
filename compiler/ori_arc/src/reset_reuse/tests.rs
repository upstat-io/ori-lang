use ori_ir::Name;
use ori_types::{Idx, Pool};

use crate::ir::{ArcBlock, ArcFunction, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue};
use crate::test_helpers::{b, make_func, owned_param, v};
use crate::ArcClassifier;

use super::detect_reset_reuse;

// Helpers

fn run_detect(mut func: ArcFunction) -> ArcFunction {
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    detect_reset_reuse(&mut func, &classifier);
    func
}

// Tests

/// Test 1: Basic pair — RcDec{x}; Construct{ty==typeof(x)} → Reset/Reuse.
#[test]
fn basic_pair() {
    // v0: str (param), v1: str (construct result)
    // Body: RcDec{v0}; Construct{dst:v1, ty:STR, ...}
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::Construct {
                    dst: v(1),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::STR],
    );

    let result = run_detect(func);
    let body = &result.blocks[0].body;

    // Should be Reset/Reuse, not RcDec/Construct.
    assert!(
        matches!(&body[0], ArcInstr::Reset { var, token } if *var == v(0) && token.raw() == 2),
        "expected Reset, got {:?}",
        body[0]
    );
    assert!(
        matches!(&body[1], ArcInstr::Reuse { token, dst, ty, .. } if token.raw() == 2 && *dst == v(1) && *ty == Idx::STR),
        "expected Reuse, got {:?}",
        body[1]
    );
}

/// Test 2: Different type — no reuse.
#[test]
fn different_type_no_reuse() {
    // v0: STR, construct type: INT (different).
    // Use a type that needs_rc for the construct. Since INT is scalar,
    // use two different ref types. We'll use STR for dec and UNIT placeholder
    // for construct (UNIT is scalar, so this won't match).
    // Actually, to test properly, both need to be RC types but different.
    // Let's just check that STR dec + INT construct doesn't match.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::Construct {
                    dst: v(1),
                    ty: Idx::INT, // Different type (and scalar — won't match STR)
                    ctor: CtorKind::Tuple,
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::INT],
    );

    let result = run_detect(func);
    let body = &result.blocks[0].body;

    // Should remain RcDec/Construct (no match).
    assert!(
        matches!(&body[0], ArcInstr::RcDec { .. }),
        "expected RcDec, got {:?}",
        body[0]
    );
    assert!(
        matches!(&body[1], ArcInstr::Construct { .. }),
        "expected Construct, got {:?}",
        body[1]
    );
}

/// Test 3: Aliased — use of dec'd var between Dec and Construct → no reuse.
#[test]
fn aliased_no_reuse() {
    // RcDec{v0}; Apply{args:[v0]}; Construct{ty==typeof(v0)}
    // v0 is used in the Apply between Dec and Construct → unsafe to reuse.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::Apply {
                    dst: v(1),
                    ty: Idx::INT,
                    func: Name::from_raw(99),
                    args: vec![v(0)],
                },
                ArcInstr::Construct {
                    dst: v(2),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::INT, Idx::STR],
    );

    let result = run_detect(func);
    let body = &result.blocks[0].body;

    // Should remain unchanged.
    assert!(matches!(&body[0], ArcInstr::RcDec { .. }));
    assert!(matches!(&body[1], ArcInstr::Apply { .. }));
    assert!(matches!(&body[2], ArcInstr::Construct { .. }));
}

/// Test 4: Intervening non-aliasing instruction — reuse is OK.
#[test]
fn intervening_ok() {
    // RcDec{v0}; Let{v2: int = 42}; Construct{ty==typeof(v0)}
    // The Let doesn't use v0 → safe to reuse.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(42)),
                },
                ArcInstr::Construct {
                    dst: v(2),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::INT, Idx::STR],
    );

    let result = run_detect(func);
    let body = &result.blocks[0].body;

    // Reset at index 0, Let at index 1, Reuse at index 2.
    assert!(
        matches!(&body[0], ArcInstr::Reset { var, .. } if *var == v(0)),
        "expected Reset, got {:?}",
        body[0]
    );
    assert!(matches!(&body[1], ArcInstr::Let { .. }));
    assert!(
        matches!(&body[2], ArcInstr::Reuse { dst, ty, .. } if *dst == v(2) && *ty == Idx::STR),
        "expected Reuse, got {:?}",
        body[2]
    );
}

/// Test 5: First Construct wins — two Constructs after Dec, only first paired.
#[test]
fn first_construct_wins() {
    // RcDec{v0}; Construct{v1:STR}; Construct{v2:STR}
    // Only the first Construct should be paired.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::Construct {
                    dst: v(1),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::Construct {
                    dst: v(2),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_detect(func);
    let body = &result.blocks[0].body;

    // First pair: Reset/Reuse.
    assert!(matches!(&body[0], ArcInstr::Reset { .. }));
    assert!(matches!(&body[1], ArcInstr::Reuse { .. }));
    // Second Construct: unchanged.
    assert!(
        matches!(&body[2], ArcInstr::Construct { .. }),
        "expected Construct, got {:?}",
        body[2]
    );
}

/// Test 6: Multiple pairs — two Dec/Construct pairs, both replaced.
#[test]
fn multiple_pairs() {
    // RcDec{v0}; Construct{v2:STR}; RcDec{v1}; Construct{v3:STR}
    let func = make_func(
        vec![owned_param(0, Idx::STR), owned_param(1, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::Construct {
                    dst: v(2),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
                ArcInstr::RcDec { var: v(1) },
                ArcInstr::Construct {
                    dst: v(3),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(3) },
        }],
        vec![Idx::STR, Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_detect(func);
    let body = &result.blocks[0].body;

    // Both pairs should be replaced.
    assert!(
        matches!(&body[0], ArcInstr::Reset { var, .. } if *var == v(0)),
        "expected Reset(v0), got {:?}",
        body[0]
    );
    assert!(
        matches!(&body[1], ArcInstr::Reuse { dst, .. } if *dst == v(2)),
        "expected Reuse(v2), got {:?}",
        body[1]
    );
    assert!(
        matches!(&body[2], ArcInstr::Reset { var, .. } if *var == v(1)),
        "expected Reset(v1), got {:?}",
        body[2]
    );
    assert!(
        matches!(&body[3], ArcInstr::Reuse { dst, .. } if *dst == v(3)),
        "expected Reuse(v3), got {:?}",
        body[3]
    );
}

/// Test 7: Fresh token ID doesn't collide with existing vars.
#[test]
fn fresh_token_id() {
    // var_types has 3 entries (v0, v1, v2). Token should be v3.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Let {
                    dst: v(1),
                    ty: Idx::INT,
                    value: ArcValue::Literal(LitValue::Int(0)),
                },
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::Construct {
                    dst: v(2),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR, Idx::INT, Idx::STR],
    );

    let result = run_detect(func);

    // Token should be ArcVarId(3) — next after v2.
    let body = &result.blocks[0].body;
    match &body[1] {
        ArcInstr::Reset { token, .. } => {
            assert_eq!(token.raw(), 3, "token should be v3");
            // And it should be in var_types.
            assert_eq!(result.var_types.len(), 4);
            assert_eq!(result.var_types[3], Idx::STR);
        }
        other => panic!("expected Reset, got {other:?}"),
    }
}

// ── Cross-block reset/reuse tests ──────────────────────────

use crate::graph::DominatorTree;
use crate::liveness::compute_refined_liveness;

use super::detect_reset_reuse_cfg;

fn run_detect_cfg(mut func: ArcFunction) -> ArcFunction {
    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let dom = DominatorTree::build(&func);
    let (refined, _) = compute_refined_liveness(&func, &classifier);
    detect_reset_reuse_cfg(&mut func, &classifier, &dom, &refined);
    func
}

/// Cross-block: `RcDec` in entry, Construct in dominated block.
/// The canonical linked-list `map` pattern.
#[test]
fn cross_block_basic() {
    // Block 0:
    //   v1 = apply f(v0)     -- transforms element
    //   RcDec(v0)            -- decrement original node
    //   jump b1
    // Block 1 (dominated by b0):
    //   v2 = Construct(Struct, [v1])  -- allocate new node
    //   return v2
    //
    // After cross-block detection:
    //   Block 0: Reset(v0, token), jump b1
    //   Block 1: Reuse(token, ...), return v2
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![
                    ArcInstr::Apply {
                        dst: v(1),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
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
                body: vec![ArcInstr::Construct {
                    dst: v(2),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![v(1)],
                }],
                terminator: ArcTerminator::Return { value: v(2) },
            },
        ],
        vec![Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_detect_cfg(func);

    // Block 0: RcDec should be replaced with Reset.
    let has_reset = result.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Reset { .. }));
    assert!(
        has_reset,
        "block 0 should have Reset after cross-block detection"
    );

    // Block 1: Construct should be replaced with Reuse.
    let has_reuse = result.blocks[1]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Reuse { .. }));
    assert!(
        has_reuse,
        "block 1 should have Reuse after cross-block detection"
    );

    // No RcDec should remain (it was replaced by Reset).
    let has_rc_dec = result.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::RcDec { .. }));
    assert!(!has_rc_dec, "block 0 should not have RcDec after pairing");
}

/// Cross-block: aliasing prevents pairing (var is live-for-use in target).
#[test]
fn cross_block_aliasing_prevents() {
    // Block 0:
    //   RcDec(v0)
    //   jump b1
    // Block 1:
    //   v1 = apply f(v0)     -- v0 is read here → live-for-use
    //   v2 = Construct(Struct, [v1])
    //   return v2
    //
    // v0 is used in b1 → cannot reset in b0.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![
            ArcBlock {
                id: b(0),
                params: vec![],
                body: vec![ArcInstr::RcDec { var: v(0) }],
                terminator: ArcTerminator::Jump {
                    target: b(1),
                    args: vec![],
                },
            },
            ArcBlock {
                id: b(1),
                params: vec![],
                body: vec![
                    ArcInstr::Apply {
                        dst: v(1),
                        ty: Idx::STR,
                        func: Name::from_raw(99),
                        args: vec![v(0)],
                    },
                    ArcInstr::Construct {
                        dst: v(2),
                        ty: Idx::STR,
                        ctor: CtorKind::Struct(Name::from_raw(10)),
                        args: vec![v(1)],
                    },
                ],
                terminator: ArcTerminator::Return { value: v(2) },
            },
        ],
        vec![Idx::STR, Idx::STR, Idx::STR],
    );

    let result = run_detect_cfg(func);

    // Pairing should NOT happen — v0 is used in b1.
    let has_reset = result
        .blocks
        .iter()
        .flat_map(|bl| bl.body.iter())
        .any(|i| matches!(i, ArcInstr::Reset { .. }));
    assert!(!has_reset, "should not pair when aliasing exists");
}

/// Cross-block: all existing intra-block tests still pass after cfg detection.
#[test]
fn cross_block_preserves_intra_block() {
    // Exact same setup as basic_pair test — intra-block pair should still work.
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec { var: v(0) },
                ArcInstr::Construct {
                    dst: v(1),
                    ty: Idx::STR,
                    ctor: CtorKind::Struct(Name::from_raw(10)),
                    args: vec![],
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::STR, Idx::STR],
    );

    let result = run_detect_cfg(func);

    // Should pair intra-block.
    let has_reset = result.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Reset { .. }));
    let has_reuse = result.blocks[0]
        .body
        .iter()
        .any(|i| matches!(i, ArcInstr::Reuse { .. }));
    assert!(has_reset, "intra-block Reset should still be detected");
    assert!(has_reuse, "intra-block Reuse should still be detected");
}
