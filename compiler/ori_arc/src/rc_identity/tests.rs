//! Tests for the RC identity propagation pass.

use ori_types::Idx;

use crate::ir::{
    ArcBlock, ArcBlockId, ArcInstr, ArcTerminator, ArcVarId, ArgOwnership, CtorKind, RcStrategy,
    ValueRepr,
};
use crate::ownership::DerivedOwnership;
use crate::test_helpers::{count_rc_ops, make_func, owned_param, v};

use super::RcIdentityMap;

/// Helper: create a `Project` instruction with the common `ty: Idx::UNIT`.
fn project(dst: ArcVarId, value: ArcVarId, field: u32) -> ArcInstr {
    ArcInstr::Project {
        dst,
        ty: Idx::UNIT,
        value,
        field,
    }
}

// ---------------------------------------------------------------------------
// RcIdentityMap::build
// ---------------------------------------------------------------------------

/// Simple projection: `v1 = Project(v0, 0)` → identity[v1] == v0.
#[test]
fn identity_simple_projection() {
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![project(v(1), v(0), 0)],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::UNIT; 2],
    );

    let ownership = vec![
        DerivedOwnership::Owned,              // v0: owned param
        DerivedOwnership::BorrowedFrom(v(0)), // v1: projection of v0
    ];

    let map = RcIdentityMap::build(&func, &ownership);
    assert_eq!(map.root(v(0)), v(0), "v0 is its own root");
    assert_eq!(map.root(v(1)), v(0), "v1's root is v0");
    assert!(map.is_root(v(0)));
    assert!(!map.is_root(v(1)));
    assert_eq!(map.normalized_count(), 1);
}

/// Chain projection: v0 → v1 → v2, all should resolve to v0.
#[test]
fn identity_chain_projection() {
    let func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![project(v(1), v(0), 0), project(v(2), v(1), 0)],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::UNIT; 3],
    );

    // infer_derived_ownership resolves transitively: v2 borrows from v0
    // (not v1), because v1 already borrows from v0.
    let ownership = vec![
        DerivedOwnership::Owned,
        DerivedOwnership::BorrowedFrom(v(0)),
        DerivedOwnership::BorrowedFrom(v(0)), // transitively resolved
    ];

    let map = RcIdentityMap::build(&func, &ownership);
    assert_eq!(map.root(v(0)), v(0));
    assert_eq!(map.root(v(1)), v(0));
    assert_eq!(map.root(v(2)), v(0));
    assert_eq!(map.normalized_count(), 2);
}

/// Independent variables: each is its own root.
#[test]
fn identity_independent_vars() {
    let func = make_func(
        vec![owned_param(0, Idx::UNIT), owned_param(1, Idx::UNIT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::UNIT; 2],
    );

    let ownership = vec![DerivedOwnership::Owned, DerivedOwnership::Owned];

    let map = RcIdentityMap::build(&func, &ownership);
    assert_eq!(map.root(v(0)), v(0));
    assert_eq!(map.root(v(1)), v(1));
    assert_eq!(map.normalized_count(), 0);
}

/// Fresh variables (from Construct) are their own root.
#[test]
fn identity_fresh_is_root() {
    let func = make_func(
        vec![],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![ArcInstr::Construct {
                dst: v(0),
                ty: Idx::UNIT,
                ctor: CtorKind::Tuple,
                args: vec![],
            }],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::UNIT],
    );

    let ownership = vec![DerivedOwnership::Fresh];

    let map = RcIdentityMap::build(&func, &ownership);
    assert_eq!(map.root(v(0)), v(0));
    assert!(map.is_root(v(0)));
}

/// Mixed: some projected, some independent.
#[test]
fn identity_mixed() {
    let func = make_func(
        vec![owned_param(0, Idx::UNIT), owned_param(1, Idx::UNIT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![project(v(2), v(0), 0), project(v(3), v(1), 1)],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::UNIT; 4],
    );

    let ownership = vec![
        DerivedOwnership::Owned,              // v0
        DerivedOwnership::Owned,              // v1
        DerivedOwnership::BorrowedFrom(v(0)), // v2 from v0
        DerivedOwnership::BorrowedFrom(v(1)), // v3 from v1
    ];

    let map = RcIdentityMap::build(&func, &ownership);
    assert_eq!(map.root(v(0)), v(0));
    assert_eq!(map.root(v(1)), v(1));
    assert_eq!(map.root(v(2)), v(0));
    assert_eq!(map.root(v(3)), v(1));
    assert_eq!(map.normalized_count(), 2);
}

// ---------------------------------------------------------------------------
// propagate_rc_identity (without Pool — strategy-less tests)
// ---------------------------------------------------------------------------

/// `RcInc` on a projected var is not normalized when `var_reprs` is empty
/// (test-only: no strategy update available → safety guard returns None).
#[test]
fn propagation_noop_without_var_reprs() {
    let mut func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                project(v(1), v(0), 0),
                ArcInstr::RcInc {
                    var: v(1),
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                },
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(1) },
        }],
        vec![Idx::UNIT; 2],
    );

    let ownership = vec![
        DerivedOwnership::Owned,
        DerivedOwnership::BorrowedFrom(v(0)),
    ];

    let map = RcIdentityMap::build(&func, &ownership);
    assert_eq!(map.root(v(1)), v(0));

    // var_reprs is empty → root_strategy returns None → no normalization
    let pool = ori_types::Pool::new();
    let count = super::propagate_rc_identity(&mut func, &map, &pool);
    assert_eq!(count, 0, "no normalization without var_reprs");

    // RcInc still targets v(1)
    assert!(matches!(
        func.blocks[0].body[1],
        ArcInstr::RcInc { var, .. } if var == v(1)
    ));
}

/// Pass is idempotent: running twice produces the same result.
#[test]
fn propagation_idempotent() {
    let mut func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                project(v(1), v(0), 0),
                ArcInstr::RcInc {
                    var: v(1),
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::UNIT; 2],
    );

    let ownership = vec![
        DerivedOwnership::Owned,
        DerivedOwnership::BorrowedFrom(v(0)),
    ];

    let map = RcIdentityMap::build(&func, &ownership);
    let pool = ori_types::Pool::new();

    // First run: no normalization (no var_reprs)
    let c1 = super::propagate_rc_identity(&mut func, &map, &pool);

    // Second run: same result
    let c2 = super::propagate_rc_identity(&mut func, &map, &pool);

    assert_eq!(c1, c2, "idempotent — same result on second run");
}

/// Non-projected vars are left unchanged.
#[test]
fn propagation_leaves_owned_vars_unchanged() {
    // NOTE: This test uses empty var_reprs (no Pool-based repr computation),
    // which means propagation is a no-op by design (safety guard returns None).
    let mut func = make_func(
        vec![owned_param(0, Idx::UNIT)],
        Idx::UNIT,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                ArcInstr::RcInc {
                    var: v(0),
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                },
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(0) },
        }],
        vec![Idx::UNIT],
    );

    let ownership = vec![DerivedOwnership::Owned];
    let map = RcIdentityMap::build(&func, &ownership);
    let pool = ori_types::Pool::new();

    let count = super::propagate_rc_identity(&mut func, &map, &pool);
    assert_eq!(count, 0, "owned vars are never normalized");

    // Both ops still target v(0)
    assert!(matches!(
        func.blocks[0].body[0],
        ArcInstr::RcInc { var, .. } if var == v(0)
    ));
    assert!(matches!(
        func.blocks[0].body[1],
        ArcInstr::RcDec { var, .. } if var == v(0)
    ));
}

// ---------------------------------------------------------------------------
// Integration: identity propagation enables more elimination
// ---------------------------------------------------------------------------

/// Demonstrates the core value proposition of RC identity propagation.
///
/// Scenario: `v1 = Project(v0, 0)` then `RcInc(v1); Apply(f, [v1]); RcDec(v0)`.
///
/// Without identity propagation:
///   - Phase 1 (pair matching): `RcInc(v1)` and `RcDec(v0)` target different
///     variables → no pair found.
///   - Phase 2 (ownership-aware): `RcInc(v1)` removed because v1 is
///     `BorrowedFrom(v0)` and v0 is still alive. But `RcDec(v0)` stays
///     because v0 is Owned.
///   - Result: 1 RC op remains (`RcDec(v0)`).
///
/// With identity propagation:
///   - `RcInc(v1)` normalized to `RcInc(v0)`.
///   - Phase 1: `RcInc(v0)` at pos 1, `Apply(f, [v1])` at pos 2 (uses v1
///     not v0 → no invalidation), `RcDec(v0)` at pos 3 → pair found and
///     eliminated.
///   - Result: 0 RC ops remain.
///
/// The Inc/Dec pair semantically represents: "increment refcount to give
/// the callee a reference via v1, then decrement when v0 goes out of scope."
/// Cancelling both transfers v0's ownership to the callee through the
/// projection — net refcount change is zero.
#[test]
fn integration_identity_enables_pair_elimination() {
    use ori_ir::Name;

    // fn foo(x: T) -> T
    //   v1 = Project(v0, 0)         ← v1 borrows from v0
    //   RcInc(v1)                   ← keep v1 alive past v0's Dec
    //   v2 = Apply(f, [v1])         ← consumes v1
    //   RcDec(v0)                   ← v0 is dead after Apply
    //   Return v2
    let base_func = make_func(
        vec![owned_param(0, Idx::STR)],
        Idx::STR,
        vec![ArcBlock {
            id: ArcBlockId::new(0),
            params: vec![],
            body: vec![
                project(v(1), v(0), 0),
                ArcInstr::RcInc {
                    var: v(1),
                    count: 1,
                    strategy: RcStrategy::HeapPointer,
                },
                ArcInstr::Apply {
                    dst: v(2),
                    ty: Idx::STR,
                    func: Name::from_raw(99),
                    args: vec![v(1)],
                    arg_ownership: vec![ArgOwnership::Owned],
                },
                ArcInstr::RcDec {
                    var: v(0),
                    strategy: RcStrategy::HeapPointer,
                },
            ],
            terminator: ArcTerminator::Return { value: v(2) },
        }],
        vec![Idx::STR; 3],
    );

    let ownership = vec![
        DerivedOwnership::Owned,              // v0: owned param
        DerivedOwnership::BorrowedFrom(v(0)), // v1: projection of v0
        DerivedOwnership::Fresh,              // v2: fresh from Apply
    ];

    // --- Without identity propagation ---
    let mut func_without = base_func.clone();
    let rc_before = count_rc_ops(&func_without);
    assert_eq!(rc_before, 2, "starts with RcInc + RcDec");

    crate::rc_elim::eliminate_rc_ops_dataflow(&mut func_without, &ownership);
    let remaining_without = count_rc_ops(&func_without);

    // Phase 2 removes RcInc(v1) (BorrowedFrom, source alive), but
    // RcDec(v0) stays (Owned variable → not touched by Phase 2).
    assert_eq!(
        remaining_without, 1,
        "without identity propagation: Phase 2 removes Inc, Dec stays"
    );

    // --- With identity propagation ---
    let mut func_with = base_func;
    // Populate var_reprs so root_strategy can compute the new strategy.
    // RcPointer → HeapPointer, no pool query needed.
    func_with.var_reprs = vec![ValueRepr::RcPointer; 3];

    let map = RcIdentityMap::build(&func_with, &ownership);
    let pool = ori_types::Pool::new();
    let normalized = super::propagate_rc_identity(&mut func_with, &map, &pool);
    assert_eq!(normalized, 1, "RcInc(v1) normalized to RcInc(v0)");

    // Verify the normalization happened
    assert!(matches!(
        func_with.blocks[0].body[1],
        ArcInstr::RcInc { var, .. } if var == v(0)
    ));

    crate::rc_elim::eliminate_rc_ops_dataflow(&mut func_with, &ownership);
    let remaining_with = count_rc_ops(&func_with);

    // Phase 1 pairs RcInc(v0)/RcDec(v0) — Apply between them uses v1
    // not v0, so no intervening use → pair eliminated.
    assert_eq!(
        remaining_with, 0,
        "with identity propagation: Inc/Dec pair fully eliminated"
    );

    // The improvement: identity propagation eliminated 1 more RC op
    assert!(
        remaining_with < remaining_without,
        "identity propagation enables strictly more elimination: \
         {remaining_with} < {remaining_without}"
    );
}
