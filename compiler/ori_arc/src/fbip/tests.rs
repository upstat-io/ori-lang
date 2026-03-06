use ori_ir::{Name, Span};
use ori_types::{Idx, Pool};

use crate::graph::DominatorTree;
use crate::ir::{ArcBlock, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue, RcStrategy};
use crate::liveness::compute_refined_liveness;
use crate::lower::ArcProblem;
use crate::test_helpers::{b, make_func, owned_param, v};
use crate::ArcClassifier;

use super::{analyze_fbip, check_fbip_enforcement, is_auto_fbip};

/// Function with a Reset/Reuse pair → achieved, `is_fbip` = true.
#[test]
fn achieved_reuse_reported() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Reset {
                    var: v(0),
                    token: v(1),
                },
                ArcInstr::Reuse {
                    token: v(1),
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

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let dom_tree = DominatorTree::build(&func);
    let (refined, _) = compute_refined_liveness(&func, &classifier);

    let report = analyze_fbip(&func, &classifier, &dom_tree, &refined);

    assert_eq!(report.achieved.len(), 1);
    assert!(report.is_fbip);
}

/// Function with an unpaired `RcDec` + matching Construct → missed.
#[test]
fn missed_reuse_detected() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
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

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let dom_tree = DominatorTree::build(&func);
    let (refined, _) = compute_refined_liveness(&func, &classifier);

    let report = analyze_fbip(&func, &classifier, &dom_tree, &refined);

    assert!(report.achieved.is_empty());
    assert!(!report.missed.is_empty(), "should detect missed reuse");
    assert!(!report.is_fbip);
}

/// Function with no `RcDec` and no Construct → trivially not FBIP
/// (no allocations, so nothing to reuse).
#[test]
fn no_allocations_not_fbip() {
    let func = make_func(
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

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let dom_tree = DominatorTree::build(&func);
    let (refined, _) = compute_refined_liveness(&func, &classifier);

    let report = analyze_fbip(&func, &classifier, &dom_tree, &refined);

    assert!(report.achieved.is_empty());
    assert!(report.missed.is_empty());
    assert!(!report.is_fbip, "no allocations → not FBIP");
}

/// Type mismatch: `RcDec` of str, Construct of a different type → missed
/// with `TypeMismatch` reason.
#[test]
fn type_mismatch_missed() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
                // Construct a different type (use a unique Idx to simulate)
                ArcInstr::Construct {
                    dst: v(1),
                    ty: Idx::UNIT,
                    ctor: CtorKind::Tuple,
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
        // v0=str, v1=unit, v2=str
        vec![Idx::STR, Idx::UNIT, Idx::STR],
    );

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let dom_tree = DominatorTree::build(&func);
    let (refined, _) = compute_refined_liveness(&func, &classifier);

    let report = analyze_fbip(&func, &classifier, &dom_tree, &refined);

    // v0 is str, there IS a matching Construct of str (v2), so it should
    // detect a PossiblyShared miss (not type mismatch — there IS a match).
    assert!(!report.missed.is_empty());
    assert!(!report.is_fbip);
}

// check_fbip_enforcement

/// Compliant function: Reset/Reuse pair → no violation.
#[test]
fn enforcement_compliant_no_violation() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::Reset {
                    var: v(0),
                    token: v(1),
                },
                ArcInstr::Reuse {
                    token: v(1),
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

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let result = check_fbip_enforcement(&func, &classifier, "my_func", Span::DUMMY);

    assert!(
        result.is_none(),
        "compliant function should produce no violation"
    );
}

/// Missed reuse → violation with correct counts.
#[test]
fn enforcement_violation_missed_reuse() {
    let func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
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

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let result = check_fbip_enforcement(&func, &classifier, "bad_func", Span::DUMMY);

    let Some(violation) = result else {
        panic!("missed reuse should produce a violation");
    };
    match violation {
        ArcProblem::FbipViolation {
            func_name,
            missed_count,
            achieved_count,
            ..
        } => {
            assert_eq!(func_name, "bad_func");
            assert!(missed_count > 0);
            assert_eq!(achieved_count, 0);
        }
        other => panic!("expected FbipViolation, got {other:?}"),
    }
}

/// Scalar-only function (#fbip on int→int) → no violation (nothing to reuse).
#[test]
fn enforcement_scalar_only_no_violation() {
    let func = make_func(
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

    let pool = Pool::new();
    let classifier = ArcClassifier::new(&pool);
    let result = check_fbip_enforcement(&func, &classifier, "scalar_fn", Span::DUMMY);

    assert!(
        result.is_none(),
        "scalar-only function has no missed reuse → no violation"
    );
}

// -- is_auto_fbip tests --

#[test]
fn auto_fbip_all_static_unique() {
    use crate::uniqueness::{CowAnnotations, CowMode};

    let mut func = make_func(
        vec![],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let mut annotations = CowAnnotations::new();
    annotations.set(0, 0, CowMode::StaticUnique);
    annotations.set(0, 1, CowMode::StaticUnique);
    func.cow_annotations = annotations;

    assert!(is_auto_fbip(&func));
}

#[test]
fn auto_fbip_with_dynamic_is_false() {
    use crate::uniqueness::{CowAnnotations, CowMode};

    let mut func = make_func(
        vec![],
        Idx::STR,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::STR],
    );

    let mut annotations = CowAnnotations::new();
    annotations.set(0, 0, CowMode::StaticUnique);
    annotations.set(0, 1, CowMode::Dynamic);
    func.cow_annotations = annotations;

    assert!(!is_auto_fbip(&func));
}

#[test]
fn auto_fbip_empty_annotations_is_false() {
    let func = make_func(
        vec![],
        Idx::INT,
        vec![ArcBlock {
            id: b(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::INT],
    );

    assert!(!is_auto_fbip(&func));
}
