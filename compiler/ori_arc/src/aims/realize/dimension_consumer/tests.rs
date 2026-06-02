//! Dimension-to-consumer matrix — positive + negative pins.
//!
//! Each dimension's consumer is a pure decision predicate over the converged
//! lattice state. Burden ops are TF-N/A in both transfer directions (verified
//! at `aims/transfer/mod.rs`), so the burden-emitted baseline does NOT change
//! any dimension's meaning at a consumer site — the consumer reads the same
//! lattice dimension whether or not a burden op sits at the site. These pins
//! clamp that each consumer routes its dimension's value to the correct
//! decision (positive) and rejects the wrong value (negative).
//!
//! Per the DP-6/DP-7/DP-8/DP-9 decision-predicate truth tables.

use crate::aims::lattice::{
    AccessClass, AimsState, Cardinality, Consumption, EffectClass, Locality, ReuseCtorKind,
    ShapeClass, Uniqueness,
};
use crate::aims::realize::decide::{decide_cow, AnnotationSiteContext};
use crate::ir::ArcVarId;
use crate::uniqueness::CowMode;
use rustc_hash::FxHashSet;

fn v(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

/// Build an `AimsState` with the given dimensions; the rest default to a
/// plausible owned-value shape. Each pin overrides only the dimension(s) under
/// test so the predicate's response is attributable to that dimension.
fn state(
    access: AccessClass,
    uniqueness: Uniqueness,
    locality: Locality,
    shape: ShapeClass,
    consumption: Consumption,
    cardinality: Cardinality,
) -> AimsState {
    AimsState {
        access,
        consumption,
        cardinality,
        uniqueness,
        locality,
        shape,
        effect: EffectClass::NONE,
    }
}

/// Build an `AnnotationSiteContext` for `decide_cow` with the given uniqueness.
/// `is_borrow_disjoint` / `has_active_borrows` default to the no-borrow case so
/// the Uniqueness dimension alone drives the DP-9 `cow_mode` routing. The
/// borrowed `rc_incremented_set` is empty (no physical-refcount confound).
fn cow_ctx(uniqueness: Uniqueness, empty: &FxHashSet<ArcVarId>) -> AnnotationSiteContext<'_> {
    AnnotationSiteContext {
        var: v(0),
        uniqueness,
        rc_incremented: false,
        is_param: false,
        is_param_borrowed: false,
        is_borrowed_call_arg: false,
        rc_incremented_set: empty,
        is_excluded: false,
        access: AccessClass::Owned,
        consumption: Consumption::Linear,
        cardinality: Cardinality::Once,
        shape: ShapeClass::NonReusable,
        is_borrow_disjoint: false,
        has_active_borrows: false,
        is_collection: false,
    }
}

// Uniqueness → DP-9 `decide_cow` (`realize/decide.rs`).
//
// Plan matrix row: positive — `Unique` → `StaticUnique` (no IsShared);
// `Shared` → `StaticShared` (unconditional copy). negative — `MaybeShared` is
// NOT forced to `StaticUnique` absent the disjoint-borrow / IC-3 unique proof
// (would emit unguarded mutation = UAF pin).

/// DP-9 positive pin: `Unique` (no active borrow) routes to `StaticUnique` —
/// the in-place fast path with no runtime `IsShared` check.
#[test]
fn uniqueness_unique_decides_static_unique_cow() {
    let empty = FxHashSet::default();
    let ctx = cow_ctx(Uniqueness::Unique, &empty);
    assert_eq!(
        decide_cow(&ctx),
        CowMode::StaticUnique,
        "Uniqueness=Unique with no active borrow must drive StaticUnique COW"
    );
}

/// DP-9 positive pin: `Shared` routes to `StaticShared` — the unconditional
/// copy path.
#[test]
fn uniqueness_shared_decides_static_shared_cow() {
    let empty = FxHashSet::default();
    let ctx = cow_ctx(Uniqueness::Shared, &empty);
    assert_eq!(
        decide_cow(&ctx),
        CowMode::StaticShared,
        "Uniqueness=Shared must drive StaticShared COW (always copy)"
    );
}

/// DP-9 negative pin: `MaybeShared` without the disjoint-borrow proof is NOT
/// forced to `StaticUnique` — it falls to `Dynamic` (runtime `IsShared`
/// check). Forcing `StaticUnique` here would emit an unguarded in-place
/// mutation on a possibly-aliased value (UAF). This pin rejects that.
#[test]
fn uniqueness_maybe_shared_does_not_decide_static_unique_cow() {
    let empty = FxHashSet::default();
    let ctx = cow_ctx(Uniqueness::MaybeShared, &empty);
    let mode = decide_cow(&ctx);
    assert_ne!(
        mode,
        CowMode::StaticUnique,
        "Uniqueness=MaybeShared absent disjoint-borrow proof must NOT drive StaticUnique (UAF risk)"
    );
    assert_eq!(
        mode,
        CowMode::Dynamic,
        "Uniqueness=MaybeShared falls to Dynamic (runtime IsShared check)"
    );
}

// Shape + Uniqueness → DP-6 `is_reuse_candidate` (RL-11/RL-11a).
//
// Plan matrix row: positive — `Owned ∧ ≠Shared ∧ ReusableCtor` → Reset/Reuse
// emitted. negative — `Shared` shape → NonReusable (CN-3); reuse NOT emitted
// (non-unique reuse corrupts aliases pin).

/// DP-6 positive pin: `Owned ∧ Unique ∧ ReusableCtor(Struct)` is a reuse
/// candidate — Reset/Reuse may be emitted.
#[test]
fn shape_reusable_owned_unique_is_reuse_candidate() {
    let s = state(
        AccessClass::Owned,
        Uniqueness::Unique,
        Locality::BlockLocal,
        ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        Consumption::Linear,
        Cardinality::Once,
    );
    assert!(
        s.is_reuse_candidate(),
        "Owned + Unique + ReusableCtor(Struct) must be a reuse candidate (DP-6)"
    );
}

/// DP-6 positive pin: `MaybeShared` with a reusable shape is STILL a candidate
/// (RL-11a dynamic-uniqueness path — runtime `IsShared` guards the reuse).
/// Distinguishes DP-6's `uniqueness != Shared` gate from a stricter
/// `uniqueness == Unique` gate.
#[test]
fn shape_reusable_maybe_shared_is_reuse_candidate() {
    let s = state(
        AccessClass::Owned,
        Uniqueness::MaybeShared,
        Locality::BlockLocal,
        ShapeClass::CollectionBuffer,
        Consumption::Linear,
        Cardinality::Once,
    );
    assert!(
        s.is_reuse_candidate(),
        "Owned + MaybeShared + CollectionBuffer is a reuse candidate via RL-11a dynamic path"
    );
}

/// DP-6 negative pin: `Shared` is NEVER a reuse candidate (CN-3 forces
/// `NonReusable` shape; even pre-canonicalization the predicate's
/// `uniqueness != Shared` gate rejects it). Reuse on a non-unique value would
/// corrupt aliases. This pin rejects that.
#[test]
fn shape_shared_is_not_reuse_candidate() {
    let s = state(
        AccessClass::Owned,
        Uniqueness::Shared,
        Locality::BlockLocal,
        ShapeClass::ReusableCtor(ReuseCtorKind::Struct),
        Consumption::Linear,
        Cardinality::Once,
    );
    assert!(
        !s.is_reuse_candidate(),
        "Shared uniqueness must NOT be a reuse candidate (non-unique reuse corrupts aliases)"
    );
}

/// DP-6 negative pin: `NonReusable` shape is never a reuse candidate even when
/// Owned + Unique — the shape dimension gates the decision.
#[test]
fn shape_nonreusable_owned_unique_is_not_reuse_candidate() {
    let s = state(
        AccessClass::Owned,
        Uniqueness::Unique,
        Locality::BlockLocal,
        ShapeClass::NonReusable,
        Consumption::Linear,
        Cardinality::Once,
    );
    assert!(
        !s.is_reuse_candidate(),
        "NonReusable shape must NOT be a reuse candidate regardless of Owned+Unique"
    );
}

// Locality → DP-7 `is_rc_skip_eligible` + DP-8 `is_local`.
//
// Plan matrix row: positive — `Locality ≤ FunctionLocal ∧ Unique` → headerless
// stack alloca, zero RC ops; order-based predicate holds across all shipped
// Locality variants. negative — `Locality ≥ HeapEscaping` → heap alloc with
// full RC header; NOT stack-promoted (promoting an escaping value = dangling
// pointer pin).
//
// "Unchanged = the dimension's ROLE": RL-14/RL-17/RL-18 + DP-7/DP-8 consume
// Locality by ORDER (`≤ FunctionLocal` / `≥ HeapEscaping`), so the role is
// variant-count-agnostic and survives the shipped 4-variant Locality and the
// target 5-variant lattice identically. The 5th variant (`ArgEscaping`) is
// target-only per the ArgEscaping carve-out.

/// DP-8 positive pin: `BlockLocal` is local — eligible for stack promotion.
#[test]
fn locality_block_local_is_local() {
    let s = state(
        AccessClass::Owned,
        Uniqueness::Unique,
        Locality::BlockLocal,
        ShapeClass::NonReusable,
        Consumption::Linear,
        Cardinality::Once,
    );
    assert!(s.is_local(), "BlockLocal must be local (DP-8)");
}

/// DP-8 positive pin: `FunctionLocal` is local — the upper boundary of the
/// order-based `≤ FunctionLocal` predicate.
#[test]
fn locality_function_local_is_local() {
    let s = state(
        AccessClass::Owned,
        Uniqueness::Unique,
        Locality::FunctionLocal,
        ShapeClass::NonReusable,
        Consumption::Linear,
        Cardinality::Once,
    );
    assert!(
        s.is_local(),
        "FunctionLocal must be local (DP-8 upper boundary)"
    );
}

/// DP-7 positive pin: `Locality ≤ FunctionLocal ∧ Owned ∧ Linear ∧ Once ∧
/// Unique` is RC-skip eligible — zero RC ops, the inc/dec pair cancels.
#[test]
fn locality_function_local_unique_is_rc_skip_eligible() {
    let s = state(
        AccessClass::Owned,
        Uniqueness::Unique,
        Locality::FunctionLocal,
        ShapeClass::NonReusable,
        Consumption::Linear,
        Cardinality::Once,
    );
    assert!(
        s.is_rc_skip_eligible(),
        "FunctionLocal + Owned + Linear + Once + Unique must be RC-skip eligible (DP-7)"
    );
}

/// DP-8 negative pin: `HeapEscaping` is NOT local — promoting it to a stack
/// alloca would leave a dangling pointer when the value escapes. This pin
/// rejects stack promotion of an escaping value.
#[test]
fn locality_heap_escaping_is_not_local() {
    let s = state(
        AccessClass::Owned,
        Uniqueness::Unique,
        Locality::HeapEscaping,
        ShapeClass::NonReusable,
        Consumption::Linear,
        Cardinality::Once,
    );
    assert!(
        !s.is_local(),
        "HeapEscaping must NOT be local (stack-promoting an escaping value = dangling pointer)"
    );
}

/// DP-7 negative pin: `HeapEscaping` is NOT RC-skip eligible even when
/// Owned + Linear + Once + Unique — the Locality dimension gates the skip, so
/// an escaping value keeps its full RC header (RL-16).
#[test]
fn locality_heap_escaping_is_not_rc_skip_eligible() {
    let s = state(
        AccessClass::Owned,
        Uniqueness::Unique,
        Locality::HeapEscaping,
        ShapeClass::NonReusable,
        Consumption::Linear,
        Cardinality::Once,
    );
    assert!(
        !s.is_rc_skip_eligible(),
        "HeapEscaping must NOT be RC-skip eligible (escaping value keeps full RC header per RL-16)"
    );
}

/// DP-7 negative pin: `Unknown` locality (conservative default) is NOT
/// RC-skip eligible — the order-based predicate rejects everything above
/// `FunctionLocal`, so the conservative default never wrongly skips.
#[test]
fn locality_unknown_is_not_rc_skip_eligible() {
    let s = state(
        AccessClass::Owned,
        Uniqueness::Unique,
        Locality::Unknown,
        ShapeClass::NonReusable,
        Consumption::Linear,
        Cardinality::Once,
    );
    assert!(
        !s.is_rc_skip_eligible(),
        "Unknown locality must NOT be RC-skip eligible (conservative default)"
    );
}
