//! Tests for [`AimsStateMap`].

use rustc_hash::FxHashMap;

use crate::ir::{ArcBlock, ArcBlockId, ArcFunction, ArcTerminator, ArcVarId};

use super::super::super::lattice::{
    AccessClass, AimsState, BorrowSource, Cardinality, Consumption, Uniqueness,
};
use super::{AimsEvent, AimsStateMap, InvokeEdgeState};

// Test helpers

/// Create a state map for testing with the given number of blocks and variables.
fn make_state_map(num_blocks: usize, num_vars: usize) -> AimsStateMap {
    use ori_ir::Name;
    use ori_types::Idx;

    let blocks: Vec<ArcBlock> = (0..num_blocks)
        .map(|i| ArcBlock {
            #[expect(
                clippy::cast_possible_truncation,
                reason = "test helper, block count fits u32"
            )]
            id: ArcBlockId::new(i as u32),
            params: vec![],
            body: vec![],
            terminator: ArcTerminator::Unreachable,
        })
        .collect();

    let func = ArcFunction {
        name: Name::from_raw(0),
        return_type: Idx::from_raw(0),
        blocks,
        entry: ArcBlockId::new(0),
        var_types: vec![Idx::from_raw(0); num_vars],
        ..Default::default()
    };

    AimsStateMap::new(&func)
}

fn block(n: u32) -> ArcBlockId {
    ArcBlockId::new(n)
}

fn var(n: u32) -> ArcVarId {
    ArcVarId::new(n)
}

// Constructor tests

#[test]
fn new_creates_empty_state_map() {
    let map = make_state_map(3, 5);
    assert_eq!(map.num_blocks(), 3);
    assert_eq!(map.num_vars(), 5);
    assert!(map.is_converged()); // no changes yet
}

// Scalar tests

#[test]
fn scalar_variables_return_scalar_state() {
    let mut map = make_state_map(2, 3);
    map.set_permanent_scalar(var(1));

    assert!(map.is_scalar(var(1)));
    assert!(!map.is_scalar(var(0)));
    assert!(!map.is_scalar(var(2)));

    // Scalar vars always return SCALAR regardless of stored state
    assert_eq!(
        map.var_state_at_block_entry(block(0), var(1)),
        AimsState::SCALAR
    );
    assert_eq!(
        map.var_state_at_block_exit(block(0), var(1)),
        AimsState::SCALAR
    );
}

#[test]
fn non_scalar_variables_default_to_bottom() {
    let map = make_state_map(2, 3);
    assert_eq!(
        map.var_state_at_block_entry(block(0), var(0)),
        AimsState::BOTTOM
    );
    assert_eq!(
        map.var_state_at_block_exit(block(1), var(2)),
        AimsState::BOTTOM
    );
}

// Block state update tests

#[test]
fn update_block_entry_tracks_changes() {
    let mut map = make_state_map(2, 3);

    let mut entry = FxHashMap::default();
    entry.insert(var(0), AimsState::FRESH);

    assert!(map.update_block_entry(block(0), entry.clone()));
    assert!(!map.is_converged()); // change was made

    // Same update again → no change
    map.reset_changed();
    assert!(!map.update_block_entry(block(0), entry));
    assert!(map.is_converged());
}

#[test]
fn update_block_entry_stores_retrievable_state() {
    let mut map = make_state_map(2, 3);

    let mut entry = FxHashMap::default();
    entry.insert(var(0), AimsState::FRESH);
    entry.insert(var(2), AimsState::TOP);

    map.update_block_entry(block(1), entry);

    assert_eq!(
        map.var_state_at_block_entry(block(1), var(0)),
        AimsState::FRESH
    );
    assert_eq!(
        map.var_state_at_block_entry(block(1), var(2)),
        AimsState::TOP
    );
    // var(1) not in map → BOTTOM
    assert_eq!(
        map.var_state_at_block_entry(block(1), var(1)),
        AimsState::BOTTOM
    );
}

#[test]
fn update_block_exit_stores_retrievable_state() {
    let mut map = make_state_map(2, 3);

    let mut exit = FxHashMap::default();
    exit.insert(var(1), AimsState::FRESH);

    map.update_block_exit(block(0), exit);

    assert_eq!(
        map.var_state_at_block_exit(block(0), var(1)),
        AimsState::FRESH
    );
    assert_eq!(
        map.var_state_at_block_exit(block(0), var(0)),
        AimsState::BOTTOM
    );
}

#[test]
fn out_of_bounds_block_returns_false() {
    let mut map = make_state_map(2, 3);
    let entry = FxHashMap::default();
    assert!(!map.update_block_entry(block(5), entry));
}

// Convergence tests

#[test]
fn convergence_resets_between_iterations() {
    let mut map = make_state_map(2, 3);

    let mut entry = FxHashMap::default();
    entry.insert(var(0), AimsState::FRESH);
    map.update_block_entry(block(0), entry);
    assert!(!map.is_converged());

    map.reset_changed();
    assert!(map.is_converged());
}

// Borrow provenance tests

#[test]
fn borrow_source_basic_operations() {
    let mut map = make_state_map(2, 5);

    // Initially no borrow source
    assert!(map.borrow_source(var(1)).is_none());

    // Set exact source
    map.set_borrow_source(var(1), BorrowSource::exact(var(0)));
    assert_eq!(
        map.borrow_source(var(1)),
        Some(&BorrowSource::exact(var(0)))
    );

    // Clear
    map.clear_borrow_source(var(1));
    assert!(map.borrow_source(var(1)).is_none());
}

#[test]
fn borrow_source_join_same_source_stays_exact() {
    let mut map = make_state_map(2, 5);

    map.set_borrow_source(var(1), BorrowSource::exact(var(0)));
    map.join_borrow_sources(var(1), BorrowSource::exact(var(0)));

    assert_eq!(
        map.borrow_source(var(1)),
        Some(&BorrowSource::exact(var(0)))
    );
}

#[test]
fn borrow_source_join_different_sources_becomes_unknown() {
    let mut map = make_state_map(2, 5);

    map.set_borrow_source(var(1), BorrowSource::exact(var(0)));
    map.join_borrow_sources(var(1), BorrowSource::exact(var(2)));

    assert_eq!(map.borrow_source(var(1)), Some(&BorrowSource::Unknown));
}

#[test]
fn borrow_source_join_into_empty_sets_source() {
    let mut map = make_state_map(2, 5);

    map.join_borrow_sources(var(1), BorrowSource::exact(var(0)));

    assert_eq!(
        map.borrow_source(var(1)),
        Some(&BorrowSource::exact(var(0)))
    );
}

// Invoke edge state tests

#[test]
fn invoke_edge_state_basic_operations() {
    let mut map = make_state_map(3, 5);

    assert!(map.invoke_edge_state(block(1)).is_none());

    let mut normal = FxHashMap::default();
    normal.insert(var(0), AimsState::FRESH);
    normal.insert(var(3), AimsState::FRESH); // dst var

    let mut unwind = FxHashMap::default();
    unwind.insert(var(0), AimsState::FRESH);
    // var(3) NOT in unwind — dst not defined on unwind path

    let edge_state = InvokeEdgeState { normal, unwind };
    map.set_invoke_edge_state(block(1), edge_state);

    let retrieved = map.invoke_edge_state(block(1)).expect("should exist");
    assert!(retrieved.normal.contains_key(&var(3)));
    assert!(!retrieved.unwind.contains_key(&var(3)));
}

// Event table tests

#[test]
fn events_empty_by_default() {
    let map = make_state_map(3, 5);
    assert!(map.events_in_block(block(0)).is_empty());
    assert!(map.events_in_block(block(2)).is_empty());
}

#[test]
fn record_event_appends_to_block() {
    let mut map = make_state_map(3, 5);

    map.record_event(AimsEvent::ReusableAllocation {
        block: block(1),
        instr: 2,
        var: var(0),
    });
    map.record_event(AimsEvent::LocalAllocCandidate {
        block: block(1),
        instr: 5,
        var: var(1),
    });
    map.record_event(AimsEvent::FipGate {
        block: block(2),
        instr: 0,
    });

    assert_eq!(map.events_in_block(block(1)).len(), 2);
    assert_eq!(map.events_in_block(block(2)).len(), 1);
    assert!(map.events_in_block(block(0)).is_empty());
}

// Full accessor tests

#[test]
fn block_entry_states_returns_full_map() {
    let mut map = make_state_map(2, 3);

    let mut entry = FxHashMap::default();
    entry.insert(var(0), AimsState::FRESH);
    entry.insert(var(2), AimsState::TOP);
    map.update_block_entry(block(0), entry);

    let states = map.block_entry_states(block(0)).expect("block exists");
    assert_eq!(states.len(), 2);
    assert_eq!(states.get(&var(0)), Some(&AimsState::FRESH));
    assert_eq!(states.get(&var(2)), Some(&AimsState::TOP));
}

#[test]
fn block_exit_states_returns_full_map() {
    let mut map = make_state_map(2, 3);

    let mut exit = FxHashMap::default();
    exit.insert(var(1), AimsState::FRESH);
    map.update_block_exit(block(1), exit);

    let states = map.block_exit_states(block(1)).expect("block exists");
    assert_eq!(states.len(), 1);
    assert_eq!(states.get(&var(1)), Some(&AimsState::FRESH));
}

// Integration: scalar + state update interaction

#[test]
fn scalar_overrides_stored_state() {
    let mut map = make_state_map(2, 3);

    // Store a non-scalar state for var(1)
    let mut entry = FxHashMap::default();
    entry.insert(var(1), AimsState::FRESH);
    map.update_block_entry(block(0), entry);

    // Now mark var(1) as scalar
    map.set_permanent_scalar(var(1));

    // Scalar sentinel overrides the stored state
    assert_eq!(
        map.var_state_at_block_entry(block(0), var(1)),
        AimsState::SCALAR
    );
}

// State transitions for demand analysis pattern

#[test]
fn iterative_refinement_converges() {
    let mut map = make_state_map(2, 2);

    // Iteration 1: var(0) gets FRESH at block 0 entry
    let mut entry = FxHashMap::default();
    entry.insert(var(0), AimsState::FRESH);
    map.update_block_entry(block(0), entry.clone());
    assert!(!map.is_converged());

    // Iteration 2: same state → converged
    map.reset_changed();
    map.update_block_entry(block(0), entry.clone());
    assert!(map.is_converged());

    // Iteration 3: state refined (joined with more demand) → not converged
    map.reset_changed();
    let mut refined = entry;
    refined.insert(
        var(0),
        AimsState {
            cardinality: Cardinality::Many,
            ..AimsState::FRESH
        },
    );
    map.update_block_entry(block(0), refined);
    assert!(!map.is_converged());
}

// Demand propagation direction test

#[test]
fn backward_analysis_pattern_exit_from_successor_entry() {
    let mut map = make_state_map(3, 2);

    // In backward analysis: block 2 successor's entry state
    // becomes block 1's exit state. Verify the storage is correct.

    // Successor (block 2) has entry demand for var(0)
    let mut succ_entry = FxHashMap::default();
    succ_entry.insert(
        var(0),
        AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            ..AimsState::BOTTOM
        },
    );
    map.update_block_entry(block(2), succ_entry);

    // Block 1's exit state = successor's entry state (single successor)
    let exit_state = map
        .block_entry_states(block(2))
        .expect("block exists")
        .clone();
    map.update_block_exit(block(1), exit_state);

    // Verify: block 1 exit matches block 2 entry
    assert_eq!(
        map.var_state_at_block_exit(block(1), var(0)),
        map.var_state_at_block_entry(block(2), var(0))
    );
}

// FIP token balance

#[test]
fn fip_balance_defaults_to_zero() {
    let map = make_state_map(1, 2);
    assert!(
        map.fip_token_balanced(),
        "0 constructs, 0 consumed → balanced"
    );
    assert_eq!(map.fip_net_allocation(), 0);
}

#[test]
fn fip_balance_set_balanced() {
    let mut map = make_state_map(1, 2);
    map.set_fip_balance(3, 3);
    assert!(
        map.fip_token_balanced(),
        "3 constructs, 3 consumed → balanced"
    );
    assert_eq!(map.fip_net_allocation(), 0);
}

#[test]
fn fip_balance_surplus_consumed() {
    let mut map = make_state_map(1, 2);
    map.set_fip_balance(2, 5);
    assert!(
        map.fip_token_balanced(),
        "2 constructs, 5 consumed → balanced (surplus)"
    );
    assert_eq!(map.fip_net_allocation(), 0);
}

#[test]
fn fip_balance_deficit() {
    let mut map = make_state_map(1, 2);
    map.set_fip_balance(5, 2);
    assert!(
        !map.fip_token_balanced(),
        "5 constructs, 2 consumed → not balanced"
    );
    assert_eq!(map.fip_net_allocation(), 3);
}

#[test]
fn fip_balance_no_constructs() {
    let mut map = make_state_map(1, 2);
    map.set_fip_balance(0, 4);
    assert!(
        map.fip_token_balanced(),
        "0 constructs → trivially balanced (FBIP)"
    );
    assert_eq!(map.fip_net_allocation(), 0);
}

#[test]
fn alloc_credit_balance_event_recorded() {
    let mut map = make_state_map(3, 2);
    map.record_event(AimsEvent::AllocCreditBalance {
        block: block(0),
        successor_idx: 0,
        balance: -1,
    });
    map.record_event(AimsEvent::AllocCreditBalance {
        block: block(0),
        successor_idx: 1,
        balance: 2,
    });

    let events = map.events_in_block(block(0));
    assert_eq!(events.len(), 2, "two branch events recorded");

    // Verify the balance values.
    if let AimsEvent::AllocCreditBalance { balance, .. } = &events[0] {
        assert_eq!(*balance, -1, "first branch has surplus deaths");
    } else {
        panic!("expected AllocCreditBalance event");
    }
    if let AimsEvent::AllocCreditBalance { balance, .. } = &events[1] {
        assert_eq!(*balance, 2, "second branch needs tokens");
    } else {
        panic!("expected AllocCreditBalance event");
    }
}

// Per-variable shape tests (Section 09.2 Shape Activation)

#[test]
fn var_shape_defaults_to_non_reusable() {
    let map = make_state_map(2, 3);
    assert_eq!(
        map.var_shape(var(0)),
        super::super::super::lattice::ShapeClass::NonReusable
    );
}

#[test]
fn var_shape_set_and_retrieved() {
    use super::super::super::lattice::{ReuseCtorKind, ShapeClass};

    let mut map = make_state_map(2, 3);
    map.set_var_shape(var(0), ShapeClass::ReusableCtor(ReuseCtorKind::Struct));
    map.set_var_shape(var(1), ShapeClass::CollectionBuffer);

    assert_eq!(
        map.var_shape(var(0)),
        ShapeClass::ReusableCtor(ReuseCtorKind::Struct)
    );
    assert_eq!(map.var_shape(var(1)), ShapeClass::CollectionBuffer);
    // var(2) not set → NonReusable
    assert_eq!(map.var_shape(var(2)), ShapeClass::NonReusable);
}

#[test]
fn var_shape_non_reusable_not_stored() {
    use super::super::super::lattice::ShapeClass;

    let mut map = make_state_map(2, 3);
    map.set_var_shape(var(0), ShapeClass::NonReusable);
    // NonReusable is not stored (optimization: sparse map)
    assert_eq!(map.var_shape(var(0)), ShapeClass::NonReusable);
}

// TF-6 contract-narrowed call-result side tables (BUG-04-065)
//
// These tests exercise the `var_uniqueness` and `var_locality` side tables
// that mirror `var_shapes`, populated by the post-convergence pass
// `populate_call_result_states` per Plan TPR Round 5 architectural shape.
//
// Sparse filter is BOTTOM-default per Plan TPR Round 1 F1 correction:
//   - Uniqueness: skip Unique (BOTTOM); store MaybeShared / Shared.
//   - Locality:   skip BlockLocal (BOTTOM); store FunctionLocal / HeapEscaping
//                 / Unknown (CONSERVATIVE).
//   - Shape:      skip NonReusable (BOTTOM = CONSERVATIVE for shape — handled
//                 by existing var_shapes; sparse filter unchanged).
//
// Presence-aware lookup distinguishes "unset" (None) from "set to BOTTOM"
// (also None — BOTTOM never inserts) from "set to a narrower value" (Some(...)).
// This is load-bearing for `effective_uniqueness_at_block_*` JOIN semantics
// per Plan TPR Round 0 F1.

#[test]
fn set_var_uniqueness_unique_does_not_insert() {
    // BOTTOM-default sparse filter: Uniqueness::Unique IS BOTTOM, so
    // set_var_uniqueness(Unique) is a no-op. contract_uniqueness returns
    // None — effective_uniqueness will fall through to lattice (Unique).
    use super::super::super::lattice::Uniqueness;

    let mut map = make_state_map(2, 3);
    map.set_var_uniqueness(var(0), Uniqueness::Unique);
    assert_eq!(map.contract_uniqueness(var(0)), None);
}

#[test]
fn set_var_uniqueness_maybe_shared_inserts() {
    // Plan TPR Round 1 F1 LOAD-BEARING: MaybeShared IS the value the side
    // table must store — without this insert, `ori_list_slice_drop`'s
    // contract is silently dropped, effective falls through to lattice
    // BOTTOM=Unique, drop_hints classify as Unique, codegen routes to
    // ori_buffer_drop_unique → BUG-04-086 slice-rest panic UNFIXED.
    use super::super::super::lattice::Uniqueness;

    let mut map = make_state_map(2, 3);
    map.set_var_uniqueness(var(0), Uniqueness::MaybeShared);
    assert_eq!(
        map.contract_uniqueness(var(0)),
        Some(Uniqueness::MaybeShared)
    );
}

#[test]
fn set_var_uniqueness_shared_inserts() {
    use super::super::super::lattice::Uniqueness;

    let mut map = make_state_map(2, 3);
    map.set_var_uniqueness(var(0), Uniqueness::Shared);
    assert_eq!(map.contract_uniqueness(var(0)), Some(Uniqueness::Shared));
}

#[test]
fn set_var_locality_block_local_does_not_insert() {
    // BOTTOM-default sparse filter: Locality::BlockLocal IS BOTTOM.
    use super::super::super::lattice::Locality;

    let mut map = make_state_map(2, 3);
    map.set_var_locality(var(0), Locality::BlockLocal);
    assert_eq!(map.contract_locality(var(0)), None);
}

#[test]
fn set_var_locality_function_local_inserts() {
    use super::super::super::lattice::Locality;

    let mut map = make_state_map(2, 3);
    map.set_var_locality(var(0), Locality::FunctionLocal);
    assert_eq!(map.contract_locality(var(0)), Some(Locality::FunctionLocal));
}

#[test]
fn set_var_locality_heap_escaping_inserts() {
    use super::super::super::lattice::Locality;

    let mut map = make_state_map(2, 3);
    map.set_var_locality(var(0), Locality::HeapEscaping);
    assert_eq!(map.contract_locality(var(0)), Some(Locality::HeapEscaping));
}

#[test]
fn set_var_locality_unknown_inserts() {
    // CONSERVATIVE.locality = Unknown — must insert (it differs from
    // BOTTOM=BlockLocal). Without this, `populate_call_result_states`
    // populating direct-no-contract Apply with CONSERVATIVE would silently
    // drop the Unknown locality, leaving optimistic BlockLocal in effect.
    use super::super::super::lattice::Locality;

    let mut map = make_state_map(2, 3);
    map.set_var_locality(var(0), Locality::Unknown);
    assert_eq!(map.contract_locality(var(0)), Some(Locality::Unknown));
}

#[test]
fn contract_uniqueness_returns_none_when_unset() {
    // Presence-aware lookup: distinguishes "unset" from "set to BOTTOM"
    // (which doesn't insert). Without presence-awareness, an unset var
    // would conflate with a set-to-Unique var, breaking JOIN semantics
    // per Plan TPR Round 0 F1.
    let map = make_state_map(2, 3);
    assert_eq!(map.contract_uniqueness(var(0)), None);
    assert_eq!(map.contract_uniqueness(var(1)), None);
}

#[test]
fn contract_locality_returns_none_when_unset() {
    let map = make_state_map(2, 3);
    assert_eq!(map.contract_locality(var(0)), None);
    assert_eq!(map.contract_locality(var(2)), None);
}

// Effective-state helper tests — JOIN semantics (Plan TPR Round 0 F1)
//
// Per §1.4, Uniqueness join is `max` (Unique < MaybeShared
// < Shared). `effective_uniqueness_at_block_*` uses presence-aware match:
// - None (no side-table entry) → return lattice value directly
// - Some(contract) → contract.join(lattice) — JOIN preserves widening
//
// This is the unified-model semantics per CLAUDE.md §AIMS Invariant 5:
// the side table FEEDS INTO the lattice via JOIN, never overrides it.

#[test]
fn effective_uniqueness_at_block_entry_no_side_table_returns_lattice() {
    // No side-table entry → fall through to var_state_at_block_entry.
    // For an unset block-state the lattice returns BOTTOM.uniqueness=Unique.
    use super::super::super::lattice::Uniqueness;

    let map = make_state_map(2, 3);
    assert_eq!(
        map.effective_uniqueness_at_block_entry(block(0), var(0)),
        Uniqueness::Unique
    );
}

#[test]
fn effective_uniqueness_at_block_exit_no_side_table_returns_lattice() {
    use super::super::super::lattice::Uniqueness;

    let map = make_state_map(2, 3);
    assert_eq!(
        map.effective_uniqueness_at_block_exit(block(0), var(0)),
        Uniqueness::Unique
    );
}

#[test]
fn effective_uniqueness_join_contract_unique_lattice_unique() {
    // Contract Unique × lattice Unique → Unique (no widening).
    // Note: contract Unique is filtered out (not stored), so this is the
    // no-side-table path. Documented for completeness of the JOIN matrix.
    use super::super::super::lattice::Uniqueness;

    let mut map = make_state_map(2, 3);
    map.set_var_uniqueness(var(0), Uniqueness::Unique);
    assert_eq!(
        map.effective_uniqueness_at_block_entry(block(0), var(0)),
        Uniqueness::Unique
    );
}

#[test]
fn effective_uniqueness_join_contract_unique_lattice_maybe_shared() {
    // Contract Unique × lattice MaybeShared → MaybeShared. This is the critical
    // JOIN-vs-override pin: an OVERRIDE pattern would return Unique here,
    // suppressing the backward demand widening that pushed lattice to
    // MaybeShared. Plan TPR Round 0 F1 caught this as a critical AIMS
    // Invariant 5 violation; the JOIN semantics are the durable fix.
    use super::super::super::lattice::{
        AccessClass, AimsState, Cardinality, Consumption, Uniqueness,
    };

    let mut map = make_state_map(2, 3);
    // Lattice state at block(0) entry shows MaybeShared (widened by demand).
    let mut entry = FxHashMap::default();
    entry.insert(
        var(0),
        AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Unrestricted,
            cardinality: Cardinality::Many,
            uniqueness: Uniqueness::MaybeShared,
            ..AimsState::BOTTOM
        },
    );
    map.update_block_entry(block(0), entry);
    // Contract claims Unique. Filter strips it, so contract_uniqueness is
    // None and effective falls through to lattice = MaybeShared.
    map.set_var_uniqueness(var(0), Uniqueness::Unique);
    assert_eq!(
        map.effective_uniqueness_at_block_entry(block(0), var(0)),
        Uniqueness::MaybeShared
    );
}

#[test]
fn effective_uniqueness_join_contract_maybe_shared_lattice_unique() {
    // Contract MaybeShared × lattice Unique → MaybeShared.
    // BUG-04-086 ORI_LIST_SLICE_DROP CASE: contract narrows the call-result
    // dst from optimistic lattice BOTTOM=Unique to MaybeShared, ensuring
    // drop_hints route through ori_buffer_rc_dec (slice-aware path).
    use super::super::super::lattice::Uniqueness;

    let mut map = make_state_map(2, 3);
    // Lattice has no entry → returns BOTTOM.uniqueness = Unique.
    map.set_var_uniqueness(var(0), Uniqueness::MaybeShared);
    assert_eq!(
        map.effective_uniqueness_at_block_entry(block(0), var(0)),
        Uniqueness::MaybeShared
    );
}

/// Plan TPR Round 2 codex F1 (critical INVERTED-TDD):pseudo-tested-method.
/// The prior JOIN tests set `contract = Unique` which the BOTTOM-default
/// sparse filter strips, so `contract_uniqueness` returned None and
/// effective fell through to lattice — never exercising the JOIN code path.
/// This test pins the case where the side table HAS a present narrower
/// value AND the lattice value is wider: max(MaybeShared, Shared) = Shared
/// must win. If `effective_uniqueness_at_block_*` were rewritten as an
/// override (return contract value when Some), this test would fail.
#[test]
fn effective_uniqueness_join_contract_maybe_shared_lattice_shared_lattice_wins() {
    use super::super::super::lattice::{
        AccessClass, AimsState, Cardinality, Consumption, Uniqueness,
    };

    let mut map = make_state_map(2, 3);
    // Lattice value at block(0) entry shows Shared (most conservative —
    // backward demand widened past MaybeShared). This is reachable when a
    // call result is later passed to a may_share=true callee or stored
    // into a heap-escaping aggregate.
    let mut entry = FxHashMap::default();
    entry.insert(
        var(0),
        AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Unrestricted,
            cardinality: Cardinality::Many,
            uniqueness: Uniqueness::Shared,
            ..AimsState::BOTTOM
        },
    );
    map.update_block_entry(block(0), entry);
    // Contract narrowed to MaybeShared from a prior call's return_info.
    map.set_var_uniqueness(var(0), Uniqueness::MaybeShared);
    // JOIN must preserve lattice widening: max(MaybeShared, Shared) = Shared.
    // An override implementation (return contract when Some) would return
    // MaybeShared here — that's the rejected pre-Plan-TPR-Round-0-F1 design.
    assert_eq!(
        map.effective_uniqueness_at_block_entry(block(0), var(0)),
        Uniqueness::Shared,
        "JOIN(contract=MaybeShared, lattice=Shared) MUST = Shared (lattice wider \
         than contract; lattice widening preserved). If this returns MaybeShared, \
         the helper has been rewritten as override semantics."
    );
}

/// Plan TPR Round 2 codex F1 (critical, locality dimension): symmetric pin
/// for `effective_locality_at_block_*` — present contract value MUST lose
/// to a wider lattice value. `max(FunctionLocal, HeapEscaping) = HeapEscaping`.
#[test]
fn effective_locality_join_contract_function_local_lattice_heap_escaping_lattice_wins() {
    use super::super::super::lattice::{AimsState, Locality};

    let mut map = make_state_map(2, 3);
    // Lattice value at block(0) entry shows HeapEscaping (call result was
    // stored into a heap aggregate and that demand widened backward).
    let mut entry = FxHashMap::default();
    entry.insert(
        var(0),
        AimsState {
            locality: Locality::HeapEscaping,
            ..AimsState::FRESH
        },
    );
    map.update_block_entry(block(0), entry);
    // Contract narrowed to FunctionLocal from a prior call's return_info.
    map.set_var_locality(var(0), Locality::FunctionLocal);
    // JOIN must preserve lattice widening: max(FunctionLocal, HeapEscaping)
    // = HeapEscaping per §1.5 ordering BlockLocal <
    // FunctionLocal < HeapEscaping < Unknown.
    assert_eq!(
        map.effective_locality_at_block_entry(block(0), var(0)),
        Locality::HeapEscaping,
        "JOIN(contract=FunctionLocal, lattice=HeapEscaping) MUST = HeapEscaping \
         (lattice wider; lattice widening preserved). Override semantics would \
         return FunctionLocal."
    );
}

/// Plan TPR Round 2 codex F1 (locality, Unknown-wins case): max(HeapEscaping,
/// Unknown) = Unknown. Pins JOIN at the lattice top end.
#[test]
fn effective_locality_join_contract_heap_escaping_lattice_unknown_lattice_wins() {
    use super::super::super::lattice::{AimsState, Locality};

    let mut map = make_state_map(2, 3);
    let mut entry = FxHashMap::default();
    entry.insert(
        var(0),
        AimsState {
            locality: Locality::Unknown,
            ..AimsState::FRESH
        },
    );
    map.update_block_entry(block(0), entry);
    map.set_var_locality(var(0), Locality::HeapEscaping);
    assert_eq!(
        map.effective_locality_at_block_entry(block(0), var(0)),
        Locality::Unknown,
        "JOIN(contract=HeapEscaping, lattice=Unknown) MUST = Unknown."
    );
}

#[test]
fn effective_uniqueness_join_contract_shared_lattice_unique() {
    // Contract Shared × lattice Unique → Shared (max-join).
    use super::super::super::lattice::Uniqueness;

    let mut map = make_state_map(2, 3);
    map.set_var_uniqueness(var(0), Uniqueness::Shared);
    assert_eq!(
        map.effective_uniqueness_at_block_entry(block(0), var(0)),
        Uniqueness::Shared
    );
}

#[test]
fn effective_uniqueness_entry_exit_asymmetry() {
    // Entry-side and exit-side helpers MUST query different lattice maps.
    // Pin against any future "share one helper for both sides" refactor
    // that would silently couple consumer sites that read different sides.
    // Per Plan TPR Round 0 gemini F2.
    use super::super::super::lattice::{
        AccessClass, AimsState, Cardinality, Consumption, Uniqueness,
    };

    let mut map = make_state_map(2, 3);
    // Entry has var(0) with Unique uniqueness.
    let mut entry = FxHashMap::default();
    entry.insert(
        var(0),
        AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Linear,
            cardinality: Cardinality::Once,
            uniqueness: Uniqueness::Unique,
            ..AimsState::BOTTOM
        },
    );
    map.update_block_entry(block(0), entry);
    // Exit has var(0) with MaybeShared uniqueness (widened by successor).
    let mut exit = FxHashMap::default();
    exit.insert(
        var(0),
        AimsState {
            access: AccessClass::Owned,
            consumption: Consumption::Unrestricted,
            cardinality: Cardinality::Many,
            uniqueness: Uniqueness::MaybeShared,
            ..AimsState::BOTTOM
        },
    );
    map.update_block_exit(block(0), exit);
    // No side-table entry → effective returns the asymmetric lattice values.
    assert_eq!(
        map.effective_uniqueness_at_block_entry(block(0), var(0)),
        Uniqueness::Unique
    );
    assert_eq!(
        map.effective_uniqueness_at_block_exit(block(0), var(0)),
        Uniqueness::MaybeShared
    );
}

#[test]
fn effective_locality_join_contract_function_local_lattice_block_local() {
    // Contract FunctionLocal × lattice BlockLocal → FunctionLocal
    // (max-join — narrowed contract widens optimistic lattice).
    use super::super::super::lattice::Locality;

    let mut map = make_state_map(2, 3);
    // No block-state entry → lattice returns BOTTOM.locality = BlockLocal.
    map.set_var_locality(var(0), Locality::FunctionLocal);
    assert_eq!(
        map.effective_locality_at_block_entry(block(0), var(0)),
        Locality::FunctionLocal
    );
}

#[test]
fn effective_locality_join_contract_unknown_lattice_block_local() {
    // CONSERVATIVE.locality=Unknown × lattice BlockLocal → Unknown.
    // This is the direct-no-contract Apply path: TF-5 says CONSERVATIVE,
    // sparse filter inserts Unknown (it's NOT BOTTOM).
    use super::super::super::lattice::Locality;

    let mut map = make_state_map(2, 3);
    map.set_var_locality(var(0), Locality::Unknown);
    assert_eq!(
        map.effective_locality_at_block_entry(block(0), var(0)),
        Locality::Unknown
    );
}

#[test]
fn effective_locality_entry_exit_asymmetry() {
    use super::super::super::lattice::{AimsState, Locality};

    let mut map = make_state_map(2, 3);
    // Entry has var(0) with FunctionLocal locality.
    let mut entry = FxHashMap::default();
    entry.insert(
        var(0),
        AimsState {
            locality: Locality::FunctionLocal,
            ..AimsState::FRESH
        },
    );
    map.update_block_entry(block(0), entry);
    // Exit has var(0) with HeapEscaping locality.
    let mut exit = FxHashMap::default();
    exit.insert(
        var(0),
        AimsState {
            locality: Locality::HeapEscaping,
            ..AimsState::FRESH
        },
    );
    map.update_block_exit(block(0), exit);
    assert_eq!(
        map.effective_locality_at_block_entry(block(0), var(0)),
        Locality::FunctionLocal
    );
    assert_eq!(
        map.effective_locality_at_block_exit(block(0), var(0)),
        Locality::HeapEscaping
    );
}
