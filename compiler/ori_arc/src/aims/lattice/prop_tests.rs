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
//! Rule 4 was removed (anti-monotone, broke associativity); Rule 6 was widened
//! to `>= HeapEscaping` (monotonicity fix).
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
/// same canonical form. States triggering active canonicalization rules
/// (CN-3, CN-6, CN-8) are underrepresented in the canonical distribution.
/// The former Rule 4 zone is preserved as a regression guard.
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
        // Former Rule 4 zone (CN-4 REMOVED — anti-monotone): BlockLocal + Owned +
        // ≤Once + MaybeShared. Preserved as regression guard — these states must
        // NOT be promoted to Unique by canonicalization after the fix.
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
/// Join-based partial order (now transitive after Rule 4 removal). See
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
/// After Rule 4 removal, the join-based [`lattice_leq`] is transitive.
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

    /// Join associativity (L-2): `(a join b) join c == a join (b join c)`.
    /// Regression guard: Rule 4 removal must not be reverted.
    #[test]
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
//    (after anti-monotone Rule 4 was removed). Valid partial order.
// 2. `componentwise_leq`: per-dimension `<=` — always a valid partial order.
//    Used for downstream monotonicity tests (04.4).

proptest! {
    // lattice_leq axioms (reflexive + antisymmetric only)

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

    // componentwise_leq axioms (full partial order)

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

    /// Partial-order transitivity (L-4): `a <= b && b <= c => a <= c`.
    /// Regression guard: Rule 4 removal ensures transitivity holds.
    #[test]
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
    /// - uniqueness: only increases (CN-6: ↑MaybeShared at HeapEscaping+; CN-4 REMOVED)
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

        // Uniqueness: can only move up (CN-6: Unique→MaybeShared at
        // HeapEscaping+). Shared is never changed by canonicalization.
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
//   - is_release_event_unnecessary: true when variable is dead or absent
//   - is_additional_credit_elidable: true when used exactly once and consumed linearly
//   - is_owned_and_unique: true when owned and unique
//   - capture_state_update: computes state for captured closure variables
//
// These are optimization-decision predicates, not lattice morphisms. Their
// correctness depends on giving conservative (safe) answers, not on
// monotonicity w.r.t. the lattice partial order.

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    /// is_release_event_unnecessary must be true at BOTTOM (Dead+Absent) and false
    /// at TOP (Unrestricted+Many).
    #[test]
    fn rc_dec_unnecessary_semantic_contract(a in canonical_aims_state_strategy()) {
        use crate::aims::transfer::is_release_event_unnecessary;
        let result = is_release_event_unnecessary(&a);
        let expected = a.cardinality == Cardinality::Absent
            || a.consumption == Consumption::Dead;
        assert_eq!(
            result, expected,
            "is_release_event_unnecessary must match semantic definition: a={a:?}"
        );
    }

    /// is_additional_credit_elidable must match DP-3:
    /// `Once ∧ (Linear ∨ Affine)`. Historical theorem metadata retains
    /// `DP3_is_rc_inc_elidable_table`.
    #[test]
    fn rc_inc_elidable_semantic_contract(a in canonical_aims_state_strategy()) {
        use crate::aims::transfer::is_additional_credit_elidable;
        let result = is_additional_credit_elidable(&a);
        let expected = a.cardinality == Cardinality::Once
            && (a.consumption == Consumption::Linear || a.consumption == Consumption::Affine);
        assert_eq!(
            result, expected,
            "is_additional_credit_elidable must match semantic definition: a={a:?}"
        );
    }

    /// is_owned_and_unique lattice-level check: Owned + Unique (DP-5 subset).
    /// NOTE: The full DP-5 also requires no overlapping active borrows, which
    /// is checked via the borrow_sources side table at the intraprocedural
    /// level — not testable at the AimsState lattice level.
    #[test]
    fn is_owned_and_unique_semantic_contract(a in canonical_aims_state_strategy()) {
        use crate::aims::transfer::is_owned_and_unique;
        let result = is_owned_and_unique(&a);
        let expected = a.access == AccessClass::Owned
            && a.uniqueness == Uniqueness::Unique;
        assert_eq!(
            result, expected,
            "is_owned_and_unique must match semantic definition: a={a:?}"
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

    /// needs_ownership_events: Owned + not-Dead + not-SCALAR.
    #[test]
    fn is_rc_needed_semantic_contract(a in canonical_aims_state_strategy()) {
        let expected = a.access == AccessClass::Owned
            && a.consumption != Consumption::Dead
            && !a.is_scalar();
        assert_eq!(
            a.needs_ownership_events(), expected,
            "needs_ownership_events must match semantic definition: a={a:?}"
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

    /// is_event_pair_elision_eligible: local + Owned + Linear + Unique +
    /// not-SCALAR (DP-7).
    #[test]
    fn is_rc_skip_eligible_semantic_contract(a in canonical_aims_state_strategy()) {
        let expected = a.is_local()
            && a.access == AccessClass::Owned
            && a.consumption == Consumption::Linear
            && a.uniqueness == Uniqueness::Unique
            && !a.is_scalar();
        assert_eq!(
            a.is_event_pair_elision_eligible(), expected,
            "is_event_pair_elision_eligible must match DP-7 semantic definition: a={a:?}"
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

// Transfer function monotonicity (TF-13, L-6)

proptest! {
    #![proptest_config(ProptestConfig::with_cases(5000))]

    /// capture_state_update monotonicity in first arg (TF-13).
    /// Regression guard: Rule 6 widening to `>= HeapEscaping` ensures monotonicity.
    #[test]
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

    /// capture_state_update monotonicity in second arg (TF-13).
    /// Regression guard: Rule 6 widening to `>= HeapEscaping` ensures monotonicity.
    #[test]
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

    /// N-ary join permutation invariance (IA-9): fold order doesn't matter.
    /// Regression guard: Rule 4 removal ensures join invariance.
    #[test]
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

    /// N-ary join shuffled permutation invariance (IA-9, shuffle variant).
    /// Regression guard: Rule 4 removal.
    #[test]
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

        let mut shuffled = states;
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

// Soundness analysis — exhaustive characterization of associativity

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DemandFactor {
    consumption: Consumption,
    cardinality: Cardinality,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OwnershipFactor {
    access: AccessClass,
    uniqueness: Uniqueness,
    locality: Locality,
    shape: ShapeClass,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct StateFactors {
    demand: DemandFactor,
    ownership: OwnershipFactor,
    effect: EffectClass,
}

impl StateFactors {
    fn from_state(state: AimsState) -> Self {
        Self {
            demand: DemandFactor {
                consumption: state.consumption,
                cardinality: state.cardinality,
            },
            ownership: OwnershipFactor {
                access: state.access,
                uniqueness: state.uniqueness,
                locality: state.locality,
                shape: state.shape,
            },
            effect: state.effect,
        }
    }

    fn into_state(self) -> AimsState {
        AimsState {
            access: self.ownership.access,
            consumption: self.demand.consumption,
            cardinality: self.demand.cardinality,
            uniqueness: self.ownership.uniqueness,
            locality: self.ownership.locality,
            shape: self.ownership.shape,
            effect: self.effect,
        }
    }
}

struct JoinTable<T> {
    values: Vec<T>,
    results: Vec<usize>,
}

impl<T: Copy> JoinTable<T> {
    fn result_index(&self, left: usize, right: usize) -> usize {
        let flat_index = left
            .checked_mul(self.values.len())
            .and_then(|offset| offset.checked_add(right));
        let Some(flat_index) = flat_index else {
            panic!("join-table index arithmetic must fit usize");
        };
        let Some(result) = self.results.get(flat_index) else {
            panic!("join-table operands must be in the canonical domain");
        };
        *result
    }

    fn result(&self, left: usize, right: usize) -> T {
        self.values[self.result_index(left, right)]
    }
}

fn push_unique<T: Copy + PartialEq>(values: &mut Vec<T>, value: T) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn collect_factors(
    states: &[AimsState],
) -> (Vec<DemandFactor>, Vec<OwnershipFactor>, Vec<EffectClass>) {
    let mut demands = Vec::new();
    let mut ownerships = Vec::new();
    let mut effects = Vec::new();
    for &state in states {
        let factors = StateFactors::from_state(state);
        push_unique(&mut demands, factors.demand);
        push_unique(&mut ownerships, factors.ownership);
        push_unique(&mut effects, factors.effect);
    }
    (demands, ownerships, effects)
}

fn factor_index<T: PartialEq + std::fmt::Debug>(values: &[T], value: &T) -> usize {
    values
        .iter()
        .position(|candidate| candidate == value)
        .unwrap_or_else(|| panic!("factor escaped canonical domain: {value:?}"))
}

fn build_join_table<T: Copy + PartialEq + std::fmt::Debug>(
    values: &[T],
    state_for: impl Fn(T) -> AimsState,
    project: impl Fn(AimsState) -> T,
) -> JoinTable<T> {
    let Some(table_len) = values.len().checked_mul(values.len()) else {
        panic!("join-table size must fit usize");
    };
    let mut results = Vec::with_capacity(table_len);
    for &left in values {
        for &right in values {
            let left_state = state_for(left);
            let right_state = state_for(right);
            let joined = project(left_state.join(&right_state));
            results.push(factor_index(values, &joined));
        }
    }
    JoinTable {
        values: values.to_vec(),
        results,
    }
}

fn first_missing_product_state(
    state_set: &std::collections::HashSet<AimsState>,
    demands: &[DemandFactor],
    ownerships: &[OwnershipFactor],
    effects: &[EffectClass],
) -> Option<AimsState> {
    for &demand in demands {
        for &ownership in ownerships {
            for &effect in effects {
                let state = StateFactors {
                    demand,
                    ownership,
                    effect,
                }
                .into_state();
                if !state_set.contains(&state) {
                    return Some(state);
                }
            }
        }
    }
    None
}

fn assert_canonical_product_complete(
    states: &[AimsState],
    demands: &[DemandFactor],
    ownerships: &[OwnershipFactor],
    effects: &[EffectClass],
) {
    use std::collections::HashSet;

    let state_set: HashSet<_> = states.iter().copied().collect();
    let product_size = demands
        .len()
        .checked_mul(ownerships.len())
        .and_then(|size| size.checked_mul(effects.len()))
        .unwrap_or_else(|| panic!("canonical product size overflowed usize"));
    assert_eq!(
        state_set.len(),
        product_size,
        "canonical factor product size"
    );

    assert_eq!(
        first_missing_product_state(&state_set, demands, ownerships, effects),
        None,
        "canonical factor product omitted a state"
    );
}

fn assert_join_factorizes(
    states: &[AimsState],
    indexed: &[(usize, usize, usize)],
    demand_table: &JoinTable<DemandFactor>,
    ownership_table: &JoinTable<OwnershipFactor>,
    effect_table: &JoinTable<EffectClass>,
) {
    for (left_index, left) in states.iter().enumerate() {
        let (left_demand, left_ownership, left_effect) = indexed[left_index];
        for (right_index, right) in states.iter().enumerate() {
            let (right_demand, right_ownership, right_effect) = indexed[right_index];
            let expected = StateFactors {
                demand: demand_table.result(left_demand, right_demand),
                ownership: ownership_table.result(left_ownership, right_ownership),
                effect: effect_table.result(left_effect, right_effect),
            };
            assert_eq!(
                StateFactors::from_state(left.join(right)),
                expected,
                "join must factorize: left={left:?}, right={right:?}"
            );
        }
    }
}

fn index_triples(size: usize) -> impl Iterator<Item = (usize, usize, usize)> {
    (0..size).flat_map(move |left| {
        (0..size).flat_map(move |middle| (0..size).map(move |right| (left, middle, right)))
    })
}

fn check_join_table_associative<
    T: Copy + PartialEq + std::fmt::Debug,
    I: IntoIterator<Item = (usize, usize, usize)>,
>(
    name: &str,
    table: &JoinTable<T>,
    triples: I,
) -> Result<(), String> {
    let size = table.values.len();
    let combination_count = size
        .checked_pow(3)
        .ok_or_else(|| format!("{name} triple count overflowed usize"))?;
    let mut covered = vec![false; combination_count];

    for (left, middle, right) in triples {
        if left >= size || middle >= size || right >= size {
            return Err(format!(
                "{name} associativity proof received out-of-domain triple ({left}, {middle}, {right})"
            ));
        }
        let flat_index = left
            .checked_mul(size)
            .and_then(|offset| offset.checked_add(middle))
            .and_then(|prefix| prefix.checked_mul(size))
            .and_then(|offset| offset.checked_add(right))
            .ok_or_else(|| format!("{name} associativity index overflowed usize"))?;
        if std::mem::replace(&mut covered[flat_index], true) {
            return Err(format!(
                "{name} associativity proof repeated triple ({left}, {middle}, {right})"
            ));
        }

        let left_grouped = table.result_index(table.result_index(left, middle), right);
        let right_grouped = table.result_index(left, table.result_index(middle, right));
        if table.values[left_grouped] != table.values[right_grouped] {
            return Err(format!(
                "{name} join is not associative: left={:?}, middle={:?}, right={:?}",
                table.values[left], table.values[middle], table.values[right]
            ));
        }
    }

    if let Some(flat_index) = covered.iter().position(|was_covered| !was_covered) {
        let size_squared = size
            .checked_mul(size)
            .ok_or_else(|| format!("{name} associativity domain overflowed usize"))?;
        let left = flat_index / size_squared;
        let middle = (flat_index / size) % size;
        let right = flat_index % size;
        return Err(format!(
            "{name} associativity proof omitted triple ({left}, {middle}, {right})"
        ));
    }
    Ok(())
}

fn assert_join_table_associative<T: Copy + PartialEq + std::fmt::Debug>(
    name: &str,
    table: &JoinTable<T>,
) {
    check_join_table_associative(name, table, index_triples(table.values.len()))
        .unwrap_or_else(|message| panic!("{message}"));
}

fn assert_exhaustiveness_negative_witnesses(
    states: &[AimsState],
    demands: &[DemandFactor],
    ownerships: &[OwnershipFactor],
    effects: &[EffectClass],
    effect_table: &JoinTable<EffectClass>,
) {
    let omitted_state = states
        .last()
        .copied()
        .unwrap_or_else(|| panic!("canonical state universe must not be empty"));
    let incomplete_state_set = states
        .iter()
        .copied()
        .filter(|state| *state != omitted_state)
        .collect();
    assert_eq!(
        first_missing_product_state(&incomplete_state_set, demands, ownerships, effects,),
        Some(omitted_state),
        "negative witness: omitting one canonical combination must be detected"
    );

    let last = effect_table
        .values
        .len()
        .checked_sub(1)
        .unwrap_or_else(|| panic!("effect factor universe must not be empty"));
    let omitted_triple = (last, last, last);
    let incomplete_triples =
        index_triples(effect_table.values.len()).filter(|triple| *triple != omitted_triple);
    assert_eq!(
        check_join_table_associative("effect", effect_table, incomplete_triples),
        Err(format!(
            "effect associativity proof omitted triple ({last}, {last}, {last})"
        )),
        "negative witness: omitting one factor triple must be detected"
    );
}

/// Exhaustively proves associativity for every canonical `AimsState` triple.
///
/// The canonical universe is first proven to be the Cartesian product of the
/// demand, ownership, and effect factors. Every full-state pair is then checked
/// against the three production-join factor tables. Exhaustive associativity
/// over each table therefore represents every triple in the full O(n³) matrix
/// without repeating the same independent factor joins for each combination.
#[test]
fn canonical_state_join_is_exhaustively_associative() {
    let states = collect_all_canonical_states();
    let (demands, ownerships, effects) = collect_factors(&states);
    assert_canonical_product_complete(&states, &demands, &ownerships, &effects);

    let first = StateFactors::from_state(
        states
            .first()
            .copied()
            .unwrap_or_else(|| panic!("canonical state universe must not be empty")),
    );
    let demand_table = build_join_table(
        &demands,
        |demand| StateFactors { demand, ..first }.into_state(),
        |state| StateFactors::from_state(state).demand,
    );
    let ownership_table = build_join_table(
        &ownerships,
        |ownership| StateFactors { ownership, ..first }.into_state(),
        |state| StateFactors::from_state(state).ownership,
    );
    let effect_table = build_join_table(
        &effects,
        |effect| StateFactors { effect, ..first }.into_state(),
        |state| StateFactors::from_state(state).effect,
    );
    assert_exhaustiveness_negative_witnesses(
        &states,
        &demands,
        &ownerships,
        &effects,
        &effect_table,
    );

    let indexed: Vec<_> = states
        .iter()
        .map(|&state| {
            let factors = StateFactors::from_state(state);
            (
                factor_index(&demands, &factors.demand),
                factor_index(&ownerships, &factors.ownership),
                factor_index(&effects, &factors.effect),
            )
        })
        .collect();
    assert_join_factorizes(
        &states,
        &indexed,
        &demand_table,
        &ownership_table,
        &effect_table,
    );
    assert_join_table_associative("demand", &demand_table);
    assert_join_table_associative("ownership", &ownership_table);
    assert_join_table_associative("effect", &effect_table);

    let state_count = u128::try_from(states.len())
        .unwrap_or_else(|_| panic!("canonical state count must fit in u128"));
    let full_triple_count = state_count
        .checked_mul(state_count)
        .and_then(|square| square.checked_mul(state_count))
        .unwrap_or_else(|| panic!("canonical triple count overflowed u128"));
    let represented_triple_count = [demands.len(), ownerships.len(), effects.len()]
        .into_iter()
        .try_fold(1_u128, |product, factor_size| {
            let factor_size = u128::try_from(factor_size).ok()?;
            let factor_triples = factor_size
                .checked_mul(factor_size)?
                .checked_mul(factor_size)?;
            product.checked_mul(factor_triples)
        })
        .unwrap_or_else(|| panic!("factorized triple count overflowed u128"));
    assert_eq!(
        represented_triple_count, full_triple_count,
        "factor proof must represent every canonical state triple"
    );
    eprintln!(
        "verified {} canonical states representing {} associative triples",
        states.len(),
        represented_triple_count
    );
}
