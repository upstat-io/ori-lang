//! Tests for interprocedural uniqueness analysis.

use ori_ir::Name;
use ori_types::Idx;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::ir::{ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcValue, CtorKind, LitValue};
use crate::test_helpers::{make_func_named, owned_param, v};
use crate::{ArcClass, ArcClassification};

use super::{analyze_program, build_cow_summaries, compute_return_uniqueness};
use crate::uniqueness::intra::{analyze_intraprocedural, analyze_with_summaries};
use crate::uniqueness::{Uniqueness, UniquenessSummary};

// -- Test classifier --

/// Test classifier: `Idx::STR` (index 3) is `DefiniteRef`, everything else Scalar.
struct TestClassifier;

impl ArcClassification for TestClassifier {
    fn arc_class(&self, idx: Idx) -> ArcClass {
        if idx == Idx::STR {
            ArcClass::DefiniteRef
        } else {
            ArcClass::Scalar
        }
    }
}

/// Type shorthand: RC'd type (str).
const RC: Idx = Idx::STR;
/// Type shorthand: scalar type (int).
const SCALAR: Idx = Idx::INT;

// -- Helpers --

fn block(id: u32, body: Vec<ArcInstr>, terminator: ArcTerminator) -> ArcBlock {
    ArcBlock {
        id: ArcBlockId::new(id),
        params: Vec::new(),
        body,
        terminator,
    }
}

fn construct(dst: u32) -> ArcInstr {
    ArcInstr::Construct {
        dst: v(dst),
        ty: RC,
        ctor: CtorKind::ListLiteral,
        args: Vec::new(),
    }
}

fn apply(dst: u32, callee: Name, args: Vec<u32>) -> ArcInstr {
    ArcInstr::Apply {
        dst: v(dst),
        ty: RC,
        func: callee,
        args: args.into_iter().map(v).collect(),
        arg_ownership: Vec::new(),
    }
}

fn ret(var: u32) -> ArcTerminator {
    ArcTerminator::Return { value: v(var) }
}

/// Named function helper.
fn n(raw: u32) -> Name {
    Name::from_raw(raw)
}

// -- Tests: return value uniqueness --

#[test]
fn non_recursive_returning_construct_is_unique() {
    // @f() -> str = Construct([])
    // Return value is a fresh allocation → Unique.
    let func_f = make_func_named(
        n(10),
        vec![],
        RC,
        vec![block(0, vec![construct(0)], ret(0))],
        vec![RC],
    );

    let classifier = TestClassifier;
    let summaries = analyze_program(&[func_f], &classifier, &FxHashMap::default());

    let summary = &summaries[&n(10)];
    assert_eq!(summary.return_val, Uniqueness::Unique);
    assert!(summary.preserves_freshness);
}

#[test]
fn function_returning_parameter_is_maybe_shared() {
    // @f(v0: str) -> str = Return(v0)
    // Return value is a parameter → MaybeShared (can't prove uniqueness).
    let func_f = make_func_named(
        n(10),
        vec![owned_param(0, RC)],
        RC,
        vec![block(0, vec![], ret(0))],
        vec![RC],
    );

    let classifier = TestClassifier;
    let summaries = analyze_program(&[func_f], &classifier, &FxHashMap::default());

    let summary = &summaries[&n(10)];
    assert_eq!(summary.return_val, Uniqueness::MaybeShared);
    assert!(!summary.preserves_freshness);
}

#[test]
fn caller_uses_callee_summary() {
    // @callee() -> str = Construct([])     → Unique return
    // @caller() -> str = Apply(callee, []) → should be Unique (callee returns Unique)
    let callee = make_func_named(
        n(10),
        vec![],
        RC,
        vec![block(0, vec![construct(0)], ret(0))],
        vec![RC],
    );

    let caller = make_func_named(
        n(20),
        vec![],
        RC,
        vec![block(0, vec![apply(0, n(10), vec![])], ret(0))],
        vec![RC],
    );

    let classifier = TestClassifier;
    let summaries = analyze_program(&[callee, caller], &classifier, &FxHashMap::default());

    // Callee returns Unique
    assert_eq!(summaries[&n(10)].return_val, Uniqueness::Unique);

    // Caller calls callee and returns its result → also Unique
    assert_eq!(summaries[&n(20)].return_val, Uniqueness::Unique);
}

#[test]
fn caller_of_maybe_shared_is_maybe_shared() {
    // @callee(v0: str) -> str = Return(v0)  → MaybeShared return
    // @caller(v0: str) -> str = Apply(callee, [v0]) → MaybeShared
    let callee = make_func_named(
        n(10),
        vec![owned_param(0, RC)],
        RC,
        vec![block(0, vec![], ret(0))],
        vec![RC],
    );

    let caller = make_func_named(
        n(20),
        vec![owned_param(0, RC)],
        RC,
        vec![block(0, vec![apply(1, n(10), vec![0])], ret(1))],
        vec![RC, RC],
    );

    let classifier = TestClassifier;
    let summaries = analyze_program(&[callee, caller], &classifier, &FxHashMap::default());

    assert_eq!(summaries[&n(10)].return_val, Uniqueness::MaybeShared);
    assert_eq!(summaries[&n(20)].return_val, Uniqueness::MaybeShared);
}

// -- Tests: COW builtin summaries --

#[test]
fn cow_builtin_refines_apply_to_unique() {
    // @f() -> str
    //   v0 = Construct([])         → Unique
    //   v1 = Apply(cow_push, [v0]) → Unique (COW builtin)
    //   Return(v1)
    let cow_push = n(99);
    let func_f = make_func_named(
        n(10),
        vec![],
        RC,
        vec![block(
            0,
            vec![construct(0), apply(1, cow_push, vec![0])],
            ret(1),
        )],
        vec![RC, RC],
    );

    // Provide COW builtin summary
    let mut cow_names = FxHashSet::default();
    cow_names.insert(cow_push);
    let builtin_summaries = build_cow_summaries(&cow_names, &FxHashSet::default());

    let classifier = TestClassifier;
    let summaries = analyze_program(&[func_f], &classifier, &builtin_summaries);

    assert_eq!(summaries[&n(10)].return_val, Uniqueness::Unique);
}

#[test]
fn slice_builtin_is_maybe_shared() {
    // @f() -> str
    //   v0 = Construct([])
    //   v1 = Apply(slice, [v0])  → MaybeShared (slice shares backing)
    //   Return(v1)
    let slice_name = n(98);
    let func_f = make_func_named(
        n(10),
        vec![],
        RC,
        vec![block(
            0,
            vec![construct(0), apply(1, slice_name, vec![0])],
            ret(1),
        )],
        vec![RC, RC],
    );

    let mut shared_names = FxHashSet::default();
    shared_names.insert(slice_name);
    let builtin_summaries = build_cow_summaries(&FxHashSet::default(), &shared_names);

    let classifier = TestClassifier;
    let summaries = analyze_program(&[func_f], &classifier, &builtin_summaries);

    assert_eq!(summaries[&n(10)].return_val, Uniqueness::MaybeShared);
}

// -- Tests: recursive functions --

#[test]
fn self_recursive_returning_construct_converges_to_unique() {
    // @f(v0: str) -> str
    //   v1 = Let(Bool(true))  (cond)
    //   v2 = Construct([])    → Unique
    //   v3 = Apply(f, [v0])   → depends on summary of f
    //   // In reality this is a branch, but for simplicity:
    //   // The function always constructs a fresh value
    //   Return(v2)
    //
    // Even though f calls itself, it always returns a fresh construct.
    let func_f = make_func_named(
        n(10),
        vec![owned_param(0, RC)],
        RC,
        vec![block(
            0,
            vec![
                ArcInstr::Let {
                    dst: v(1),
                    ty: SCALAR,
                    value: ArcValue::Literal(LitValue::Bool(true)),
                },
                construct(2),
                apply(3, n(10), vec![0]),
            ],
            ret(2),
        )],
        vec![RC, SCALAR, RC, RC],
    );

    let classifier = TestClassifier;
    let summaries = analyze_program(&[func_f], &classifier, &FxHashMap::default());

    // f always returns a Construct → Unique
    assert_eq!(summaries[&n(10)].return_val, Uniqueness::Unique);
}

#[test]
fn mutually_recursive_converges() {
    // @f() -> str = Apply(g, [])  → depends on g's summary
    // @g() -> str = Construct([]) → Unique
    //
    // f calls g which returns Unique, so f also returns Unique.
    // (g doesn't call f, so this isn't truly mutually recursive,
    // but the SCC algorithm may put them together if the graph
    // is constructed that way. This tests the fixpoint path.)

    // Actually, let's make them truly mutually recursive:
    // @f() -> str
    //   v0 = Construct([])        → Unique
    //   v1 = Apply(g, [])         → depends on g's return
    //   Return(v0)                → always returns fresh
    //
    // @g() -> str
    //   v0 = Construct([])        → Unique
    //   v1 = Apply(f, [])         → depends on f's return
    //   Return(v0)                → always returns fresh
    let func_f = make_func_named(
        n(10),
        vec![],
        RC,
        vec![block(
            0,
            vec![construct(0), apply(1, n(20), vec![])],
            ret(0),
        )],
        vec![RC, RC],
    );

    let func_g = make_func_named(
        n(20),
        vec![],
        RC,
        vec![block(
            0,
            vec![construct(0), apply(1, n(10), vec![])],
            ret(0),
        )],
        vec![RC, RC],
    );

    let classifier = TestClassifier;
    let summaries = analyze_program(&[func_f, func_g], &classifier, &FxHashMap::default());

    // Both always return fresh constructs → Unique
    assert_eq!(summaries[&n(10)].return_val, Uniqueness::Unique);
    assert_eq!(summaries[&n(20)].return_val, Uniqueness::Unique);
}

// -- Tests: transitive uniqueness --

#[test]
fn transitive_chain_preserves_uniqueness() {
    // @a() -> str = Construct([])   → Unique
    // @b() -> str = Apply(a, [])    → Unique (a returns Unique)
    // @c() -> str = Apply(b, [])    → Unique (b returns Unique)
    let func_a = make_func_named(
        n(10),
        vec![],
        RC,
        vec![block(0, vec![construct(0)], ret(0))],
        vec![RC],
    );

    let func_b = make_func_named(
        n(20),
        vec![],
        RC,
        vec![block(0, vec![apply(0, n(10), vec![])], ret(0))],
        vec![RC],
    );

    let func_c = make_func_named(
        n(30),
        vec![],
        RC,
        vec![block(0, vec![apply(0, n(20), vec![])], ret(0))],
        vec![RC],
    );

    let classifier = TestClassifier;
    let summaries = analyze_program(
        &[func_a, func_b, func_c],
        &classifier,
        &FxHashMap::default(),
    );

    assert_eq!(summaries[&n(10)].return_val, Uniqueness::Unique);
    assert_eq!(summaries[&n(20)].return_val, Uniqueness::Unique);
    assert_eq!(summaries[&n(30)].return_val, Uniqueness::Unique);
}

// -- Tests: analyze_with_summaries --

#[test]
fn analyze_with_summaries_refines_apply_result() {
    // Verify that analyze_with_summaries correctly uses callee summaries
    // to refine Apply results from MaybeShared to the summary's return value.
    //
    // @f() -> str
    //   v0 = Apply(callee_unique, [])  → should be Unique with summary
    //   Return(v0)
    let callee_name = n(99);
    let func = make_func_named(
        n(10),
        vec![],
        RC,
        vec![block(0, vec![apply(0, callee_name, vec![])], ret(0))],
        vec![RC],
    );

    let classifier = TestClassifier;
    let liveness = crate::liveness::compute_liveness(&func, &classifier);

    // Without summaries: Apply result is MaybeShared
    let result_no_summary = analyze_intraprocedural(&func, &classifier, &liveness);
    assert_eq!(
        result_no_summary.block_out[0].get(v(0)),
        Uniqueness::MaybeShared
    );

    // With summary: Apply result uses callee's return uniqueness
    let mut summaries = FxHashMap::default();
    summaries.insert(
        callee_name,
        UniquenessSummary {
            params: Vec::new(),
            return_val: Uniqueness::Unique,
            preserves_freshness: true,
        },
    );

    let result_with_summary = analyze_with_summaries(&func, &classifier, &liveness, &summaries);
    assert_eq!(
        result_with_summary.block_out[0].get(v(0)),
        Uniqueness::Unique
    );
}

// -- Tests: compute_return_uniqueness --

#[test]
fn return_uniqueness_from_single_return() {
    let func = make_func_named(
        n(10),
        vec![],
        RC,
        vec![block(0, vec![construct(0)], ret(0))],
        vec![RC],
    );

    let classifier = TestClassifier;
    let liveness = crate::liveness::compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    assert_eq!(
        compute_return_uniqueness(&func, &result),
        Uniqueness::Unique
    );
}

#[test]
fn return_uniqueness_joins_multiple_returns() {
    // Two return paths: one Unique (construct), one MaybeShared (param).
    // block0:
    //   v1 = Construct([])
    //   v2 = Let(Bool(true))
    //   Branch(v2, block1, block2)
    // block1: Return(v1)    → Unique
    // block2: Return(v0)    → MaybeShared (param)
    // Join: MaybeShared
    let func = make_func_named(
        n(10),
        vec![owned_param(0, RC)],
        RC,
        vec![
            block(
                0,
                vec![
                    construct(1),
                    ArcInstr::Let {
                        dst: v(2),
                        ty: SCALAR,
                        value: ArcValue::Literal(LitValue::Bool(true)),
                    },
                ],
                ArcTerminator::Branch {
                    cond: v(2),
                    then_block: ArcBlockId::new(1),
                    else_block: ArcBlockId::new(2),
                },
            ),
            block(1, vec![], ret(1)),
            block(2, vec![], ret(0)),
        ],
        vec![RC, RC, SCALAR],
    );

    let classifier = TestClassifier;
    let liveness = crate::liveness::compute_liveness(&func, &classifier);
    let result = analyze_intraprocedural(&func, &classifier, &liveness);

    // One path returns Unique, the other MaybeShared → join = MaybeShared
    assert_eq!(
        compute_return_uniqueness(&func, &result),
        Uniqueness::MaybeShared
    );
}

// -- Tests: build_cow_summaries --

#[test]
fn build_cow_summaries_marks_cow_as_unique() {
    let mut cow = FxHashSet::default();
    cow.insert(n(1));
    cow.insert(n(2));

    let mut shared = FxHashSet::default();
    shared.insert(n(3));

    let summaries = build_cow_summaries(&cow, &shared);

    assert_eq!(summaries[&n(1)].return_val, Uniqueness::Unique);
    assert_eq!(summaries[&n(2)].return_val, Uniqueness::Unique);
    assert_eq!(summaries[&n(3)].return_val, Uniqueness::MaybeShared);
    assert!(!summaries.contains_key(&n(4))); // Unknown → not in map
}

// -- Tests: UniquenessSummary --

#[test]
fn conservative_summary() {
    let summary = UniquenessSummary::conservative(3);
    assert_eq!(summary.params.len(), 3);
    assert!(summary.params.iter().all(|u| *u == Uniqueness::MaybeShared));
    assert_eq!(summary.return_val, Uniqueness::MaybeShared);
    assert!(!summary.preserves_freshness);
}
