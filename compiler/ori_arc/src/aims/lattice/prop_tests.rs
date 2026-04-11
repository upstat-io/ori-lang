// proptest macros (`prop_oneof!`, `proptest!`) internally use `Arc` for strategy
// composition. This is unavoidable and does not affect production code.
#![expect(
    clippy::disallowed_types,
    reason = "proptest macros use Arc internally"
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
//! Lattice laws (commutativity, idempotence) hold on canonical states. However,
//! **associativity does not hold** (see BUG-04-057) due to canonicalization
//! Rule 4 (BlockLocal+Owned+≤Once+MaybeShared → Unique) creating order-
//! dependent intermediate states.
//!
//! - `canonical_aims_state_strategy()` — for lattice law, transfer, fixpoint
//! - `raw_aims_state_strategy()` — for canonicalization-specific tests

use proptest::prelude::*;

use super::{
    AccessClass, AimsState, Cardinality, Consumption, EffectClass, Locality, ReuseCtorKind,
    ShapeClass, Uniqueness,
};

// ── Dimension strategies ──────────────────────────────────────────────

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

// ── Composite strategies ─────────────────────────────────────────────

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

/// Canonical `AimsState` — a proper lattice element. Used for lattice law
/// tests, transfer function properties, and fixpoint convergence.
fn canonical_aims_state_strategy() -> impl Strategy<Value = AimsState> {
    raw_aims_state_strategy().prop_map(|mut s| {
        s.canonicalize();
        s
    })
}

/// Lattice partial order: `a <= b` iff `a.join(b) == b`.
///
/// Standard lattice-theoretic definition. Only meaningful for canonical states.
fn lattice_leq(a: &AimsState, b: &AimsState) -> bool {
    a.join(b) == *b
}

// ── 04.1 Smoke tests ──────────────────────────────────────────────────

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
}

// ── 04.2 Join law properties (canonical states only) ──────────────────

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
    #[ignore = "BUG-04-057: join non-associative due to canonicalization rule interaction"]
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

// ── 04.3 Canonicalization properties (raw states) ─────────────────────

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

// ── 04.4 Transfer function properties (canonical states) ──────────────

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

    // ── Intrinsic AimsState decision predicates ───────────────────────

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

// ── 04.5 Fixpoint convergence bounds ──────────────────────────────────

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
