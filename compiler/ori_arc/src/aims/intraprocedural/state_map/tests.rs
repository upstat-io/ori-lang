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
