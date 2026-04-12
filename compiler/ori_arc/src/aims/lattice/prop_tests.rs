// proptest macros (`prop_oneof!`, `proptest!`) internally use `Arc` for strategy
// composition. This is unavoidable and does not affect production code.
#![expect(
    clippy::disallowed_types,
    reason = "proptest macros use Arc internally"
)]
#![expect(
    clippy::unwrap_used,
    reason = "test code: unwrap on guaranteed-non-empty proptest-generated collections"
)]

//! Property-based tests for the AIMS lattice using proptest.
//!
//! Verifies algebraic lattice properties (join commutativity, associativity,
//! idempotence), canonicalization idempotence, transfer function properties,
//! and fixpoint convergence bounds across randomly generated `AimsState` values.
//!
//! # Canonical vs raw strategies
//!
//! The AIMS lattice `join()` applies `canonicalize()` after componentwise max.
//! All lattice laws (commutativity, associativity, idempotence) hold on
//! canonical states. Canonicalization rules are monotone (they only move
//! dimensions toward top / more conservative). The formerly anti-monotone
//! Rule 4 was removed by BUG-04-057 fix; Rule 6 was widened to `>= HeapEscaping`
//! by BUG-04-058 fix.
//!
//! - `canonical_aims_state_strategy()` — for lattice law, transfer, fixpoint
//! - `raw_aims_state_strategy()` — for canonicalization-specific tests

use proptest::prelude::*;

use super::{
    AccessClass, AimsState, Cardinality, Consumption, EffectClass, Locality, ReuseCtorKind,
    ShapeClass, Uniqueness,
};

// Dimension strategies

fn access_class_strategy() -> impl Strategy<Value = AccessClass> {
    prop_oneof![Just(AccessClass::Borrowed), Just(AccessClass::Owned)]
}

fn consumption_strategy() -> impl Strategy<Value = Consumption> {
    prop_oneof![
        Just(Consumption::Dead),
        Just(Consumption::Linear),
        Just(Consumption::Affine),
        Just(Consumption::Unrestricted),
    ]
}

fn cardinality_strategy() -> impl Strategy<Value = Cardinality> {
    prop_oneof![
        Just(Cardinality::Absent),
        Just(Cardinality::Once),
        Just(Cardinality::Many),
    ]
}

fn uniqueness_strategy() -> impl Strategy<Value = Uniqueness> {
    prop_oneof![
        Just(Uniqueness::Unique),
        Just(Uniqueness::MaybeShared),
        Just(Uniqueness::Shared),
    ]
}

fn locality_strategy() -> impl Strategy<Value = Locality> {
    prop_oneof![
        Just(Locality::BlockLocal),
        Just(Locality::FunctionLocal),
        Just(Locality::HeapEscaping),
        Just(Locality::Unknown),
    ]
}

fn shape_class_strategy() -> impl Strategy<Value = ShapeClass> {
    prop_oneof![
        Just(ShapeClass::NonReusable),
        Just(ShapeClass::ReusableCtor(ReuseCtorKind::Struct)),
        Just(ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant)),
        Just(ShapeClass::CollectionBuffer),
        Just(ShapeClass::ContextHole),
    ]
}

fn effect_class_strategy() -> impl Strategy<Value = EffectClass> {
    (any::<bool>(), any::<bool>(), any::<bool>()).prop_map(|(may_alloc, may_share, may_throw)| {
        EffectClass {
            may_alloc,
            may_share,
            may_throw,
        }
    })
}

// Composite strategies

/// Raw `AimsState` — may be non-canonical. Used for canonicalization tests.
fn raw_aims_state_strategy() -> impl Strategy<Value = AimsState> {
    (
        access_class_strategy(),
        consumption_strategy(),
        cardinality_strategy(),
        uniqueness_strategy(),
        locality_strategy(),
        shape_class_strategy(),
        effect_class_strategy(),
    )
        .prop_map(
            |(access, consumption, cardinality, uniqueness, locality, shape, effect)| AimsState {
                access,
                consumption,
                cardinality,
                uniqueness,
                locality,
                shape,
                effect,
            },
        )
        .prop_filter("exclude SCALAR sentinel", |s| !s.is_scalar())
}

/// Targeted strategy for states near canonicalization rule boundaries.
///
/// Supplements uniform generation to ensure adequate coverage of
/// cross-dimension interaction trigger zones. The uniform
/// `raw_aims_state_strategy()` generates all 11,519 non-SCALAR raw states
/// uniformly, but after canonicalization many raw states collapse to the
/// same canonical form. States triggering Rules 3/4/6/8 are
/// underrepresented in the canonical distribution.
fn rule_boundary_aims_state_strategy() -> impl Strategy<Value = AimsState> {
    prop_oneof![
        // Rule 3 trigger zone: Shared + ReusableCtor → NonReusable
        (
            consumption_strategy(),
            cardinality_strategy(),
            locality_strategy(),
            effect_class_strategy(),
        )
            .prop_map(|(c, card, loc, eff)| AimsState {
                access: AccessClass::Owned,
                consumption: c,
                cardinality: card,
                uniqueness: Uniqueness::Shared,
                locality: loc,
                shape: ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
                effect: eff,
            }),
        // Rule 4 trigger zone: BlockLocal + Owned + ≤Once + MaybeShared → Unique
        (effect_class_strategy(), shape_class_strategy()).prop_map(|(eff, shape)| AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::MaybeShared,
            locality: Locality::BlockLocal,
            shape,
            effect: eff,
        }),
        // Rule 6 trigger zone: Owned + HeapEscaping + Unique → MaybeShared.
        // Rule 8 fires before Rule 6 in canonicalize_single_pass(). Rule 8
        // clamps Borrowed+HeapEscaping to Borrowed+FunctionLocal, so Borrowed
        // values NEVER reach Rule 6. Only Owned+HeapEscaping+Unique triggers
        // Rule 6.
        (
            consumption_strategy(),
            cardinality_strategy(),
            effect_class_strategy(),
            shape_class_strategy(),
        )
            .prop_map(|(c, card, eff, shape)| AimsState {
                access: AccessClass::Owned,
                consumption: c,
                cardinality: card,
                uniqueness: Uniqueness::Unique,
                locality: Locality::HeapEscaping,
                shape,
                effect: eff,
            }),
        // Rule 8 trigger zone: Borrowed + HeapEscaping → FunctionLocal
        (
            consumption_strategy(),
            cardinality_strategy(),
            uniqueness_strategy(),
            effect_class_strategy(),
            shape_class_strategy(),
        )
            .prop_map(|(c, card, uniq, eff, shape)| AimsState {
                access: AccessClass::Borrowed,
                consumption: c,
                cardinality: card,
                uniqueness: uniq,
                locality: Locality::HeapEscaping,
                shape,
                effect: eff,
            }),
    ]
    .prop_filter("exclude SCALAR sentinel", |s| !s.is_scalar())
}

/// Canonical `AimsState` — a proper lattice element. Used for lattice law
/// tests, transfer function properties, and fixpoint convergence.
fn canonical_aims_state_strategy() -> impl Strategy<Value = AimsState> {
    raw_aims_state_strategy().prop_map(|mut s| {
        s.canonicalize();
        s
    })
}

/// Enriched canonical strategy: 75% uniform + 25% rule-boundary targeted.
///
/// Use this for lattice-law and partial-order tests where adequate coverage
/// of canonicalization rule trigger zones is critical.
fn enriched_canonical_strategy() -> impl Strategy<Value = AimsState> {
    prop_oneof![
        3 => raw_aims_state_strategy(),
        1 => rule_boundary_aims_state_strategy(),
    ]
    .prop_map(|mut s| {
        s.canonicalize();
        s
    })
}

/// Lattice partial order: `a <= b` iff `a.join(b) == b`.
///
/// Standard lattice-theoretic definition. Only meaningful for canonical states.
///
/// Join-based partial order (now transitive after BUG-04-057 fix). See
/// `lattice_leq_transitive` test (ignored). Use [`componentwise_leq`] for
/// downstream monotonicity tests that require a valid partial order.
fn lattice_leq(a: &AimsState, b: &AimsState) -> bool {
    a.join(b) == *b
}

/// Componentwise partial order: `a <= b` iff every dimension of `a` is `<=`
/// the corresponding dimension of `b`.
///
/// This is always a valid partial order (reflexive, antisymmetric, transitive)
/// regardless of join associativity. Used for monotonicity tests (04.4) since
/// After BUG-04-057 fix, the join-based [`lattice_leq`] is transitive.
fn componentwise_leq(a: &AimsState, b: &AimsState) -> bool {
    a.access <= b.access
        && a.consumption <= b.consumption
        && a.cardinality <= b.cardinality
        && a.uniqueness <= b.uniqueness
        && a.locality <= b.locality
        && shape_leq(a.shape, b.shape)
        && effect_leq(a.effect, b.effect)
}

/// `ShapeClass` uses a flat lattice with `NonReusable` at top.
fn shape_leq(a: ShapeClass, b: ShapeClass) -> bool {
    a == b || b == ShapeClass::NonReusable
}

/// `EffectClass`: componentwise boolean OR lattice (`false` <= `true`).
fn effect_leq(a: EffectClass, b: EffectClass) -> bool {
    (a.may_alloc <= b.may_alloc) && (a.may_share <= b.may_share) && (a.may_throw <= b.may_throw)
}

// Smoke tests

proptest! {
    #[test]
    fn smoke_raw_generates_non_scalar(s in raw_aims_state_strategy()) {
        assert!(!s.is_scalar(), "strategy must not generate SCALAR");
    }

    #[test]
    fn smoke_canonical_is_fixpoint(s in canonical_aims_state_strategy()) {
        let mut re = s;
        re.canonicalize();
        assert_eq!(s, re, "canonical strategy must produce already-canonical states");
    }

    #[test]
    fn smoke_rule_boundary_generates_non_scalar(s in rule_boundary_aims_state_strategy()) {
        assert!(!s.is_scalar(), "rule boundary strategy must not generate SCALAR");
    }

    #[test]
    fn smoke_enriched_canonical_is_fixpoint(s in enriched_canonical_strategy()) {
        let mut re = s;
        re.canonicalize();
        assert_eq!(s, re, "enriched canonical strategy must produce already-canonical states");
    }
}

// Join law properties (canonical states only)

proptest! {
    #[test]
    fn join_commutative(
        a in canonical_aims_state_strategy(),
        b in canonical_aims_state_strategy(),
    ) {
        assert_eq!(
            a.join(&b),
            b.join(&a),
            "join must be commutative: a={a:?}, b={b:?}"
        );
    }

    /// BUG-04-057: join is non-associative due to canonicalization Rule 4
    /// (BlockLocal+Owned+≤Once+MaybeShared → Unique) creating order-dependent
    /// intermediate states. The uniqueness dimension diverges depending on
    /// whether the intermediate join result triggers the rule.
    #[test]
    // BUG-04-057 FIXED: Rule 4 removed from canonicalize, join is now associative.
    fn join_associative(
        a in canonical_aims_state_strategy(),
        b in canonical_aims_state_strategy(),
        c in canonical_aims_state_strategy(),
    ) {
        let ab_c = a.join(&b).join(&c);
        let a_bc = a.join(&b.join(&c));
        assert_eq!(
            ab_c, a_bc,
            "join must be associative: a={a:?}, b={b:?}, c={c:?}"
        );
    }

    #[test]
    fn join_idempotent(a in canonical_aims_state_strategy()) {
        assert_eq!(
            a.join(&a),
            a,
            "join must be idempotent: a={a:?}"
        );
    }

    #[test]
    fn join_with_bottom_geq_input(a in canonical_aims_state_strategy()) {
        let mut bottom = AimsState::BOTTOM;
        bottom.canonicalize();
        let result = a.join(&bottom);
        assert!(
            lattice_leq(&a, &result),
            "join with BOTTOM must not decrease: a={a:?}, result={result:?}"
        );
    }

    #[test]
    fn join_with_top_is_top(a in canonical_aims_state_strategy()) {
        let result = a.join(&AimsState::TOP);
        assert_eq!(
            result,
            AimsState::TOP,
            "join with TOP must be TOP: a={a:?}"
        );
    }
}

// Partial-order axioms (canonical states only)
//
// Two orderings are tested:
// 1. `lattice_leq`: `a.join(b) == b` — reflexive, antisymmetric, and transitive
//    (after BUG-04-057 fix removed anti-monotone Rule 4). Valid partial order.
// 2. `componentwise_leq`: per-dimension `<=` — always a valid partial order.
//    Used for downstream monotonicity tests (04.4).

proptest! {
    // --- lattice_leq axioms (reflexive + antisymmetric only) ---

    #[test]
    fn lattice_leq_reflexive(a in enriched_canonical_strategy()) {
        assert!(
            lattice_leq(&a, &a),
            "lattice_leq must be reflexive: a={a:?}"
        );
    }

    #[test]
    fn lattice_leq_antisymmetric(
        a in enriched_canonical_strategy(),
        b in enriched_canonical_strategy(),
    ) {
        if lattice_leq(&a, &b) && lattice_leq(&b, &a) {
            assert_eq!(
                a, b,
                "lattice_leq must be antisymmetric: a={a:?}, b={b:?}"
            );
        }
    }

    // --- componentwise_leq axioms (full partial order) ---

    #[test]
    fn componentwise_leq_reflexive(a in enriched_canonical_strategy()) {
        assert!(
            componentwise_leq(&a, &a),
            "componentwise_leq must be reflexive: a={a:?}"
        );
    }

    #[test]
    fn componentwise_leq_antisymmetric(
        a in enriched_canonical_strategy(),
        b in enriched_canonical_strategy(),
    ) {
        if componentwise_leq(&a, &b) && componentwise_leq(&b, &a) {
            assert_eq!(
                a, b,
                "componentwise_leq must be antisymmetric: a={a:?}, b={b:?}"
            );
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    /// BUG-04-057: join-based `lattice_leq` is NOT transitive.
    ///
    /// Minimal counterexample: a={Borrowed,Dead,Absent,MaybeShared,BlockLocal},
    /// b={Owned,Dead,Absent,Unique,BlockLocal}, c={...,FunctionLocal}.
    /// Rule 4 fires at BlockLocal (a<=b) but not at FunctionLocal (a<=c fails).
    #[test]
    // BUG-04-057 FIXED: Rule 4 removed, lattice_leq is now transitive.
    fn lattice_leq_transitive(
        a in canonical_aims_state_strategy(),
        diff_ab in canonical_aims_state_strategy(),
        diff_bc in canonical_aims_state_strategy(),
    ) {
        let b = a.join(&diff_ab);
        let c = b.join(&diff_bc);
        prop_assume!(lattice_leq(&a, &b) && lattice_leq(&b, &c));
        assert!(
            lattice_leq(&a, &c),
            "lattice_leq must be transitive: a={a:?}, b={b:?}, c={c:?}"
        );
    }

    /// Componentwise partial order IS transitive (always valid).
    ///
    /// Uses constructive generation: build chains a <= b <= c via join,
    /// then verify componentwise ordering holds transitively.
    #[test]
    fn componentwise_leq_transitive(
        a in canonical_aims_state_strategy(),
        diff_ab in canonical_aims_state_strategy(),
        diff_bc in canonical_aims_state_strategy(),
    ) {
        let b = a.join(&diff_ab);
        let c = b.join(&diff_bc);
        // Only test triples where the componentwise ordering holds
        prop_assume!(componentwise_leq(&a, &b) && componentwise_leq(&b, &c));
        assert!(
            componentwise_leq(&a, &c),
            "componentwise_leq must be transitive: a={a:?}, b={b:?}, c={c:?}"
        );
    }
}

// Canonicalization properties (raw states)

proptest! {
    #[test]
    fn canonicalize_idempotent(a in raw_aims_state_strategy()) {
        let mut once = a;
        once.canonicalize();
        let mut twice = once;
        twice.canonicalize();
        assert_eq!(
            once, twice,
            "canonicalize must be idempotent: input={a:?}, once={once:?}, twice={twice:?}"
        );
    }

    #[test]
    fn join_result_is_canonical(
        a in raw_aims_state_strategy(),
        b in raw_aims_state_strategy(),
    ) {
        let joined = a.join(&b);
        let mut re_canonicalized = joined;
        re_canonicalized.canonicalize();
        assert_eq!(
            joined, re_canonicalized,
            "join result must already be canonical: a={a:?}, b={b:?}, joined={joined:?}"
        );
    }

    #[test]
    fn canonicalize_converges_within_bound(a in raw_aims_state_strategy()) {
        let mut state = a;
        let feedback = state.canonicalize_with_feedback();
        assert!(
            feedback.rounds <= 3,
            "canonicalize must converge within 3 rounds: input={a:?}, rounds={}",
            feedback.rounds
        );
    }

    /// Per-dimension canonicalization direction guarantees.
    ///
    /// Canonicalization enforces cross-dimension feasibility rules. Some
    /// dimensions can only move toward BOTTOM (deflating), while others
    /// may move in either direction depending on cross-dimension interactions:
    ///
    /// - access: unchanged (no rule touches it)
    /// - consumption: only decreases (Rule 1: → Dead for Absent)
    /// - cardinality: only decreases (Rule 1: → Absent for Dead)
    /// - uniqueness: EITHER direction (Rule 4: ↓Unique, Rule 6: ↑MaybeShared)
    /// - locality: only decreases (Rule 8: → ≤FunctionLocal for Borrowed)
    /// - shape: can move to NonReusable/top (Rule 3: Shared+ReusableCtor → NonReusable)
    /// - effect: unchanged (no rule touches it)
    #[test]
    fn canonicalize_dimension_guarantees(a in raw_aims_state_strategy()) {
        let mut canonical = a;
        canonical.canonicalize();

        // Access: unchanged
        assert_eq!(
            canonical.access, a.access,
            "canonicalize must not change access: input={a:?}"
        );

        // Consumption: only decreases (toward Dead)
        assert!(
            canonical.consumption <= a.consumption,
            "canonicalize must not increase consumption: \
             input={:?}, canonical={:?}", a.consumption, canonical.consumption
        );

        // Cardinality: only decreases (toward Absent)
        assert!(
            canonical.cardinality <= a.cardinality,
            "canonicalize must not increase cardinality: \
             input={:?}, canonical={:?}", a.cardinality, canonical.cardinality
        );

        // Locality: only decreases (toward BlockLocal)
        assert!(
            canonical.locality <= a.locality,
            "canonicalize must not increase locality: \
             input={:?}, canonical={:?}", a.locality, canonical.locality
        );

        // Effect: unchanged
        assert_eq!(
            canonical.effect, a.effect,
            "canonicalize must not change effect: input={a:?}"
        );

        // Uniqueness: can move either direction (Rule 4 down, Rule 6 up),
        // but Shared is never changed by any canonicalization rule.
        if a.uniqueness == Uniqueness::Shared {
            assert_eq!(
                canonical.uniqueness,
                Uniqueness::Shared,
                "canonicalize must preserve Shared uniqueness: input={a:?}"
            );
        }

        // Shape: can move to NonReusable (top of flat lattice) via Rule 3
        if canonical.shape != a.shape {
            assert_eq!(
                canonical.shape,
                ShapeClass::NonReusable,
                "canonicalize may only change shape to NonReusable: \
                 input={:?}, canonical={:?}", a.shape, canonical.shape
            );
        }
    }
}

// Transfer function properties (canonical states)

// Transfer functions in `aims/transfer/mod.rs` operate on ARC IR
// instructions, not raw AimsState → AimsState. The pure state-level
// decision functions test whether specific optimizations are safe:
//
//   - is_rc_dec_unnecessary: true when variable is dead or absent
//   - is_rc_inc_elidable: true when used exactly once and consumed linearly
//   - can_mutate_in_place: true when owned and unique
//   - capture_state_update: computes state for captured closure variables
//
// These are optimization-decision predicates, not lattice morphisms. Their
// correctness depends on giving conservative (safe) answers, not on
// monotonicity w.r.t. the lattice partial order.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    /// is_rc_dec_unnecessary must be true at BOTTOM (Dead+Absent) and false
    /// at TOP (Unrestricted+Many).
    #[test]
    fn rc_dec_unnecessary_semantic_contract(a in canonical_aims_state_strategy()) {
        use crate::aims::transfer::is_rc_dec_unnecessary;
        let result = is_rc_dec_unnecessary(&a);
        let expected = a.cardinality == Cardinality::Absent
            || a.consumption == Consumption::Dead;
        assert_eq!(
            result, expected,
            "is_rc_dec_unnecessary must match semantic definition: a={a:?}"
        );
    }

    /// is_rc_inc_elidable must match: exactly Once use with Linear consumption.
    #[test]
    fn rc_inc_elidable_semantic_contract(a in canonical_aims_state_strategy()) {
        use crate::aims::transfer::is_rc_inc_elidable;
        let result = is_rc_inc_elidable(&a);
        let expected = a.cardinality == Cardinality::Once
            && a.consumption == Consumption::Linear;
        assert_eq!(
            result, expected,
            "is_rc_inc_elidable must match semantic definition: a={a:?}"
        );
    }

    /// can_mutate_in_place must match: Owned access AND Unique reference.
    #[test]
    fn can_mutate_in_place_semantic_contract(a in canonical_aims_state_strategy()) {
        use crate::aims::transfer::can_mutate_in_place;
        let result = can_mutate_in_place(&a);
        let expected = a.access == AccessClass::Owned
            && a.uniqueness == Uniqueness::Unique;
        assert_eq!(
            result, expected,
            "can_mutate_in_place must match semantic definition: a={a:?}"
        );
    }

    /// capture_state_update must produce a canonical state.
    #[test]
    fn capture_state_update_produces_canonical(
        current in canonical_aims_state_strategy(),
        closure in canonical_aims_state_strategy(),
    ) {
        use crate::aims::transfer::capture_state_update;
        let result = capture_state_update(&current, &closure);
        let mut re = result;
        re.canonicalize();
        assert_eq!(
            result, re,
            "capture_state_update must produce canonical state: \
             current={current:?}, closure={closure:?}, result={result:?}"
        );
    }

    // Intrinsic AimsState decision predicates

    /// is_rc_needed: Owned + not-Dead + not-SCALAR.
    #[test]
    fn is_rc_needed_semantic_contract(a in canonical_aims_state_strategy()) {
        let expected = a.access == AccessClass::Owned
            && a.consumption != Consumption::Dead
            && !a.is_scalar();
        assert_eq!(
            a.is_rc_needed(), expected,
            "is_rc_needed must match semantic definition: a={a:?}"
        );
    }

    /// needs_cow_check: uniqueness == MaybeShared.
    #[test]
    fn needs_cow_check_semantic_contract(a in canonical_aims_state_strategy()) {
        let expected = a.uniqueness == Uniqueness::MaybeShared;
        assert_eq!(
            a.needs_cow_check(), expected,
            "needs_cow_check must match semantic definition: a={a:?}"
        );
    }

    /// is_reuse_candidate: Owned + not-Shared + reusable shape.
    #[test]
    fn is_reuse_candidate_semantic_contract(a in canonical_aims_state_strategy()) {
        let expected = a.access == AccessClass::Owned
            && a.uniqueness != Uniqueness::Shared
            && !matches!(a.shape, ShapeClass::NonReusable);
        assert_eq!(
            a.is_reuse_candidate(), expected,
            "is_reuse_candidate must match semantic definition: a={a:?}"
        );
    }

    /// is_rc_skip_eligible: local + Owned + Linear + not-SCALAR.
    #[test]
    fn is_rc_skip_eligible_semantic_contract(a in canonical_aims_state_strategy()) {
        let expected = a.is_local()
            && a.access == AccessClass::Owned
            && a.consumption == Consumption::Linear
            && !a.is_scalar();
        assert_eq!(
            a.is_rc_skip_eligible(), expected,
            "is_rc_skip_eligible must match semantic definition: a={a:?}"
        );
    }

    /// is_local: BlockLocal or FunctionLocal.
    #[test]
    fn is_local_semantic_contract(a in canonical_aims_state_strategy()) {
        let expected = matches!(a.locality, Locality::BlockLocal | Locality::FunctionLocal);
        assert_eq!(
            a.is_local(), expected,
            "is_local must match semantic definition: a={a:?}"
        );
    }
}

// Transfer function monotonicity (uses componentwise_leq since lattice_leq is non-transitive)

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    /// BUG-04-058: `capture_state_update` is NOT monotone in its first argument.
    ///
    /// Rule 6 fires for HeapEscaping but not Unknown. Since capture widens
    /// locality to HeapEscaping for small-locality inputs but preserves
    /// larger localities like Unknown, the function is non-monotone.
    #[test]
    // BUG-04-058 FIXED: Rule 6 widened to >= HeapEscaping, capture_state_update is now monotone.
    fn capture_state_update_monotone_in_current(
        a in canonical_aims_state_strategy(),
        diff in canonical_aims_state_strategy(),
        closure in canonical_aims_state_strategy(),
    ) {
        use crate::aims::transfer::capture_state_update;
        let b = a.join(&diff);
        prop_assume!(componentwise_leq(&a, &b));
        let fa = capture_state_update(&a, &closure);
        let fb = capture_state_update(&b, &closure);
        assert!(
            componentwise_leq(&fa, &fb),
            "capture_state_update must be monotone in current: \
             a={a:?}, b={b:?}, closure={closure:?}, f(a)={fa:?}, f(b)={fb:?}"
        );
    }

    /// BUG-04-058: `capture_state_update` is NOT monotone in its second argument.
    ///
    /// Same root cause as first-argument non-monotonicity: Rule 6 + locality.
    #[test]
    // BUG-04-058 FIXED: Rule 6 widened to >= HeapEscaping, capture_state_update is now monotone.
    fn capture_state_update_monotone_in_closure(
        current in canonical_aims_state_strategy(),
        c1 in canonical_aims_state_strategy(),
        diff in canonical_aims_state_strategy(),
    ) {
        use crate::aims::transfer::capture_state_update;
        let c2 = c1.join(&diff);
        prop_assume!(componentwise_leq(&c1, &c2));
        let fc1 = capture_state_update(&current, &c1);
        let fc2 = capture_state_update(&current, &c2);
        assert!(
            componentwise_leq(&fc1, &fc2),
            "capture_state_update must be monotone in closure: \
             current={current:?}, c1={c1:?}, c2={c2:?}, f(c1)={fc1:?}, f(c2)={fc2:?}"
        );
    }
}

// Fixpoint convergence bounds

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    #[test]
    fn ascending_chain_converges_within_height(
        states in proptest::collection::vec(canonical_aims_state_strategy(), 1..30),
    ) {
        let mut current = AimsState::BOTTOM;
        current.canonicalize();
        let mut steps_until_stable = 0;
        let mut last = current;

        for s in &states {
            current = current.join(s);
            if current != last {
                steps_until_stable += 1;
                last = current;
            }
        }

        assert!(
            steps_until_stable <= 15,
            "ascending chain must converge within lattice height 15, \
             but took {steps_until_stable} steps. Final state: {current:?}"
        );
    }

    #[test]
    fn top_is_fixpoint(
        states in proptest::collection::vec(canonical_aims_state_strategy(), 1..20),
    ) {
        let current = AimsState::TOP;
        for s in &states {
            let next = current.join(s);
            assert_eq!(
                next,
                AimsState::TOP,
                "TOP must be a fixpoint: joining with {s:?} changed it to {next:?}"
            );
        }
    }

    #[test]
    fn cardinality_seq_add_reaches_many_within_2(
        cards in proptest::collection::vec(
            prop_oneof![
                Just(Cardinality::Absent),
                Just(Cardinality::Once),
                Just(Cardinality::Many),
            ],
            1..10,
        ),
    ) {
        let mut current = Cardinality::Absent;
        let mut non_trivial_steps = 0;
        for c in &cards {
            let next = current.seq_add(*c);
            if next != current {
                non_trivial_steps += 1;
            }
            current = next;
        }
        // Cardinality has chain height 2 (Absent -> Once -> Many).
        assert!(
            non_trivial_steps <= 2,
            "seq_add chain must converge within 2 steps, took {non_trivial_steps}"
        );
    }
}

// Permutation invariance — critical for deterministic analysis

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    /// BUG-04-057: N-ary join fold is NOT permutation-invariant.
    ///
    /// Minimal counterexample: 3 states where forward fold triggers Rule 4
    /// (BlockLocal intermediate) but reverse fold doesn't (FunctionLocal
    /// intermediate). Analysis IS order-dependent.
    #[test]
    // BUG-04-057 FIXED: Rule 4 removed, n-ary join is permutation-invariant.
    fn nary_join_permutation_invariant(
        states in proptest::collection::vec(canonical_aims_state_strategy(), 2..8),
    ) {
        let fold_join = |order: &[AimsState]| -> AimsState {
            let mut iter = order.iter();
            let first = *iter.next().unwrap();
            let mut acc = first;
            for s in iter {
                acc = acc.join(s);
            }
            acc
        };

        let forward = fold_join(&states);
        let reversed: Vec<_> = states.iter().copied().rev().collect();
        let backward = fold_join(&reversed);

        assert_eq!(
            forward, backward,
            "n-ary join must be permutation-invariant: \
             forward={forward:?}, backward={backward:?}, states={states:?}"
        );
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1000))]

    /// BUG-04-057: Shuffled permutation test also fails (same root cause).
    #[test]
    // BUG-04-057 FIXED: Rule 4 removed, n-ary join is permutation-invariant.
    fn nary_join_shuffled_permutations(
        states in proptest::collection::vec(canonical_aims_state_strategy(), 3..6),
        seed in any::<u64>(),
    ) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let fold_join = |order: &[AimsState]| -> AimsState {
            let mut iter = order.iter();
            let first = *iter.next().unwrap();
            let mut acc = first;
            for s in iter {
                acc = acc.join(s);
            }
            acc
        };

        let baseline = fold_join(&states);

        let mut shuffled = states.clone();
        for i in (1..shuffled.len()).rev() {
            let mut hasher = DefaultHasher::new();
            seed.hash(&mut hasher);
            i.hash(&mut hasher);
            // i < 6 (proptest range 3..6), so modulo result always fits in usize
            #[expect(clippy::cast_possible_truncation, reason = "i < 6, result always fits")]
            let j = (hasher.finish() % (i as u64 + 1)) as usize;
            shuffled.swap(i, j);
        }

        let permuted = fold_join(&shuffled);
        assert_eq!(
            baseline, permuted,
            "n-ary join must produce same result regardless of fold order"
        );
    }

    /// `Cardinality::seq_add` distributes over `Cardinality::alt_join`.
    ///
    /// 3 values = 27 triples total — exhaustive test exists in tests.rs.
    /// This proptest exists for regression safety with better shrinking.
    #[test]
    fn cardinality_seq_add_distributes_over_alt_join(
        a in cardinality_strategy(),
        b in cardinality_strategy(),
        c in cardinality_strategy(),
    ) {
        let lhs = a.seq_add(b.alt_join(c));
        let rhs = a.seq_add(b).alt_join(a.seq_add(c));
        assert_eq!(
            lhs, rhs,
            "seq_add must distribute over alt_join: a={a:?}, b={b:?}, c={c:?}"
        );
    }
}

// BUG-04-057 soundness analysis — exhaustive characterization

/// Collect all canonical `AimsState` values by exhaustive enumeration.
fn collect_all_canonical_states() -> Vec<AimsState> {
    use std::collections::HashSet;
    let accesses = [AccessClass::Borrowed, AccessClass::Owned];
    let consumptions = [
        Consumption::Dead,
        Consumption::Linear,
        Consumption::Affine,
        Consumption::Unrestricted,
    ];
    let cardinalities = [Cardinality::Absent, Cardinality::Once, Cardinality::Many];
    let uniquenesses = [
        Uniqueness::Unique,
        Uniqueness::MaybeShared,
        Uniqueness::Shared,
    ];
    let localities = [
        Locality::BlockLocal,
        Locality::FunctionLocal,
        Locality::HeapEscaping,
        Locality::Unknown,
    ];
    let shapes = [
        ShapeClass::NonReusable,
        ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        ShapeClass::ReusableCtor(ReuseCtorKind::EnumVariant),
        ShapeClass::CollectionBuffer,
        ShapeClass::ContextHole,
    ];
    let effects = [
        EffectClass {
            may_alloc: false,
            may_share: false,
            may_throw: false,
        },
        EffectClass {
            may_alloc: true,
            may_share: false,
            may_throw: false,
        },
        EffectClass {
            may_alloc: false,
            may_share: true,
            may_throw: false,
        },
        EffectClass {
            may_alloc: true,
            may_share: true,
            may_throw: false,
        },
        EffectClass {
            may_alloc: false,
            may_share: false,
            may_throw: true,
        },
        EffectClass {
            may_alloc: true,
            may_share: false,
            may_throw: true,
        },
        EffectClass {
            may_alloc: false,
            may_share: true,
            may_throw: true,
        },
        EffectClass {
            may_alloc: true,
            may_share: true,
            may_throw: true,
        },
    ];

    let mut seen = HashSet::new();
    for &access in &accesses {
        for &consumption in &consumptions {
            for &cardinality in &cardinalities {
                for &uniqueness in &uniquenesses {
                    for &locality in &localities {
                        for &shape in &shapes {
                            for &effect in &effects {
                                let mut s = AimsState {
                                    access,
                                    consumption,
                                    cardinality,
                                    uniqueness,
                                    locality,
                                    shape,
                                    effect,
                                };
                                if s.is_scalar() {
                                    continue;
                                }
                                s.canonicalize();
                                seen.insert(s);
                            }
                        }
                    }
                }
            }
        }
    }
    seen.into_iter().collect()
}

/// BUG-04-057: Exhaustive characterization of associativity divergence.
///
/// Two-tier: (1) find sensitive (a,b) pairs where the intermediate has
/// MaybeShared/Unique uniqueness, (2) sweep all c values for each.
/// Checks whether divergence affects downstream RC/COW/reuse decisions.
///
/// RESULT: Decision divergence found on first sensitive triple. Option B
/// (fix required) confirmed — uniqueness divergence (`MaybeShared` vs `Unique`)
/// changes `needs_cow_check`, `can_mutate_in_place`, and reuse eligibility.
/// BUG-04-057 FIXED: With associative join, this scan finds 0 divergences.
/// O(n³) exhaustive — too slow for CI (~3928³ ≈ 60B operations). The
/// proptest `join_associative` provides fast regression coverage (5000 cases).
/// Run this manually when modifying canonicalization rules.
#[test]
#[ignore = "exhaustive O(n^3): confirms 0 associativity divergences — run manually after lattice changes"]
fn associativity_divergence_with_canonical_triples_detects_decision_impact() {
    use crate::aims::transfer::can_mutate_in_place;

    let canonical_states = collect_all_canonical_states();
    let n = canonical_states.len();
    eprintln!("canonical state count: {n}");

    // Tier 1: find sensitive pairs
    let mut sensitive_pairs: Vec<(usize, usize)> = Vec::new();
    for (i, a) in canonical_states.iter().enumerate() {
        for (j, b) in canonical_states.iter().enumerate() {
            let ab = a.join(b);
            if ab.uniqueness == Uniqueness::MaybeShared || ab.uniqueness == Uniqueness::Unique {
                sensitive_pairs.push((i, j));
            }
        }
    }
    eprintln!("sensitive pairs: {}", sensitive_pairs.len());

    // Tier 2: sweep c for sensitive pairs, early-exit on first decision divergence.
    // Full enumeration exceeds 150s (8.5M pairs × 3928 c = ~33.5B operations).
    // Early exit is sound: one decision divergence proves Option B (fix required).
    let mut failure_count = 0u64;
    let mut decision_divergence_example: Option<String> = None;

    'outer: for &(i, j) in &sensitive_pairs {
        let a = &canonical_states[i];
        let b = &canonical_states[j];
        let ab = a.join(b);
        for c in &canonical_states {
            let ab_c = ab.join(c);
            let a_bc = a.join(&b.join(c));
            if ab_c != a_bc {
                failure_count += 1;

                let diverges_non_uniqueness = ab_c.access != a_bc.access
                    || ab_c.consumption != a_bc.consumption
                    || ab_c.cardinality != a_bc.cardinality
                    || ab_c.locality != a_bc.locality
                    || ab_c.shape != a_bc.shape
                    || ab_c.effect != a_bc.effect;

                assert!(
                    !diverges_non_uniqueness,
                    "associativity failure in non-uniqueness dimension — \
                     escalate BUG-04-057: a={a:?}, b={b:?}, c={c:?}, \
                     ab_c={ab_c:?}, a_bc={a_bc:?}"
                );

                // Check ALL downstream uniqueness consumers
                let predicates_match = ab_c.is_rc_needed() == a_bc.is_rc_needed()
                    && ab_c.needs_cow_check() == a_bc.needs_cow_check()
                    && ab_c.is_reuse_candidate() == a_bc.is_reuse_candidate()
                    && ab_c.is_rc_skip_eligible() == a_bc.is_rc_skip_eligible()
                    && ab_c.is_local() == a_bc.is_local()
                    && can_mutate_in_place(&ab_c) == can_mutate_in_place(&a_bc)
                    && (ab_c.uniqueness == Uniqueness::Unique)
                        == (a_bc.uniqueness == Uniqueness::Unique);
                if !predicates_match {
                    decision_divergence_example = Some(format!(
                        "a={a:?}, b={b:?}, c={c:?}, \
                         ab_c.uniqueness={:?}, a_bc.uniqueness={:?}",
                        ab_c.uniqueness, a_bc.uniqueness
                    ));
                    break 'outer;
                }
            }
        }
    }

    eprintln!(
        "scanned {failure_count} associativity failures before {}",
        if decision_divergence_example.is_some() {
            "finding decision divergence (early exit)"
        } else {
            "completing scan (no decision divergence)"
        }
    );

    assert!(
        decision_divergence_example.is_none(),
        "BUG-04-057 affects downstream decisions — fix required (Option B). \
         Example: {}",
        decision_divergence_example.unwrap_or_default()
    );
}
