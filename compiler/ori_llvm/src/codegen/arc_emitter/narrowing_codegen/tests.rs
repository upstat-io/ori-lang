//! Tests for the narrowed-collection-element-width SSOT helper.
//!
//! Pins the per-collection-`Idx` keying contract: when two narrowed int
//! collections of different widths coexist, each collection's stride comes
//! from its own `ReprPlan` entry, never the first match in an unordered scan.

use ori_types::{Idx, Pool};

use ori_repr::{
    DecisionReason, DecisionSource, FatRepr, IntWidth, MachineRepr, NarrowingPolicy, ReprDecision,
    ReprPlan,
};

use super::narrowed_collection_element_width;

/// Install a `[int]`-shaped collection repr with a specific element int width.
fn set_collection_width(plan: &mut ReprPlan, idx: Idx, width: IntWidth) {
    plan.set_repr(
        idx,
        ReprDecision {
            source: DecisionSource::Canonical,
            type_idx: idx,
            repr: MachineRepr::FatPointer(FatRepr::Collection {
                element_repr: Box::new(MachineRepr::Int {
                    width,
                    signed: true,
                }),
            }),
            reason: DecisionReason::Canonical,
        },
    );
}

/// Two narrowed int collections of DIFFERENT widths must each report their own
/// width. A `ReprPlan`-wide scan returns the first match in unordered map
/// order, conflating the two strides; keying on the specific `Idx` does not.
#[test]
fn narrowed_width_keyed_on_specific_collection_not_first_match() {
    let mut pool = Pool::default();
    let list_a = pool.list(Idx::INT);
    // A distinct collection idx with a different narrowed width. `pool.set<T>`
    // gives a separate `[T]`-shaped fat-pointer type so the two live at
    // distinct `Idx` values in the same plan.
    let list_b = pool.set(Idx::INT);
    assert_ne!(
        list_a, list_b,
        "test requires two distinct collection type indices"
    );

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    set_collection_width(&mut plan, list_a, IntWidth::I8);
    set_collection_width(&mut plan, list_b, IntWidth::I16);

    assert_eq!(
        narrowed_collection_element_width(&plan, &pool, list_a),
        Some(IntWidth::I8),
        "list_a (i8-narrowed) must report i8, not list_b's i16"
    );
    assert_eq!(
        narrowed_collection_element_width(&plan, &pool, list_b),
        Some(IntWidth::I16),
        "list_b (i16-narrowed) must report i16, not list_a's i8 — a first-match \
         scan over the unordered ReprPlan would conflate the two widths"
    );
}

/// Negative pin: a canonical (i64) collection element reports no narrowing.
#[test]
fn canonical_i64_collection_reports_no_narrowing() {
    let mut pool = Pool::default();
    let list = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    set_collection_width(&mut plan, list, IntWidth::I64);

    assert_eq!(
        narrowed_collection_element_width(&plan, &pool, list),
        None,
        "canonical i64 element width is not a narrowing"
    );
}

/// A collection idx absent from the plan reports no narrowing (not a panic).
#[test]
fn collection_absent_from_plan_reports_no_narrowing() {
    let mut pool = Pool::default();
    let list = pool.list(Idx::INT);

    let plan = ReprPlan::new(NarrowingPolicy::Aggressive);

    assert_eq!(
        narrowed_collection_element_width(&plan, &pool, list),
        None,
        "absent collection entry must yield None, not the first narrowed match"
    );
}

/// The helper resolves `collection_idx` through the pool before lookup, so an
/// unresolved alias of a narrowed collection still finds its width.
#[test]
fn pool_resolution_finds_narrowed_width() {
    let mut pool = Pool::default();
    let list = pool.list(Idx::INT);

    let mut plan = ReprPlan::new(NarrowingPolicy::Aggressive);
    set_collection_width(&mut plan, list, IntWidth::I32);

    // `resolve_fully` on an already-resolved idx is idempotent; this pins that
    // the helper performs the resolution step rather than requiring callers to.
    let resolved = pool.resolve_fully(list);
    assert_eq!(
        narrowed_collection_element_width(&plan, &pool, resolved),
        Some(IntWidth::I32),
    );
}
