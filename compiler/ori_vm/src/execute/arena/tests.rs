use super::{Aggregate, IteratorSourceState, IteratorState, ValueArena};
use crate::execute::value::VmValue;
use crate::ExecutionError;

#[test]
fn aggregate_allocations_receive_distinct_handles_until_reclamation() {
    let mut arena = ValueArena::default();

    let first = must_succeed(arena.allocate_aggregate(Aggregate::default(), 2));
    let second = must_succeed(arena.allocate_aggregate(Aggregate::default(), 2));

    assert_ne!(first, second);
    assert_eq!(must_succeed(first.aggregate_handle()).index(), 0);
    assert_eq!(must_succeed(second.aggregate_handle()).index(), 1);
    assert_eq!(arena.metrics().live_entries, 2);
    assert_eq!(arena.metrics().slots, 2);
    assert_eq!(arena.metrics().cumulative_aggregate_allocations, 2);
}

#[test]
fn arena_limit_rejects_before_aggregate_or_iterator_entry_is_allocated() {
    let mut arena = ValueArena::default();
    let aggregate_error = must_fail(arena.allocate_aggregate(Aggregate::default(), 0));
    let iterator_error = must_fail(arena.allocate_iterator(IteratorState::default(), 0));

    assert!(matches!(
        aggregate_error,
        ExecutionError::ResourceLimit {
            resource: "live value arena entries",
            limit: 0,
        }
    ));
    assert!(matches!(
        iterator_error,
        ExecutionError::ResourceLimit {
            resource: "live value arena entries",
            limit: 0,
        }
    ));
    assert_eq!(arena.metrics().live_entries, 0);
    assert_eq!(arena.metrics().slots, 0);
    assert_eq!(arena.metrics().owned_bytes, 0);
    assert_eq!(arena.metrics().cumulative_allocations, 0);
}

#[test]
fn live_entry_quota_is_checked_after_collection_and_before_allocation() {
    let mut arena = ValueArena::default();
    let live = must_succeed(arena.allocate_aggregate(Aggregate::default(), 1));
    must_succeed(arena.collect([live], 1));

    let error = must_fail(arena.allocate_aggregate(Aggregate::default(), 1));

    assert!(matches!(
        error,
        ExecutionError::ResourceLimit {
            resource: "live value arena entries",
            limit: 1,
        }
    ));
    assert_eq!(arena.metrics().cumulative_allocations, 1);
    assert_eq!(arena.metrics().live_entries, 1);
    assert_eq!(arena.metrics().slots, 1);
}

#[test]
fn unreachable_entry_is_reused_and_its_stale_handle_is_rejected() {
    let mut arena = ValueArena::default();
    let reachable = must_succeed(arena.allocate_aggregate(Aggregate::default(), 2));
    let unreachable = must_succeed(arena.allocate_aggregate(Aggregate::default(), 2));
    let stale_handle = must_succeed(unreachable.aggregate_handle());

    must_succeed(arena.collect([reachable], 2));
    let replacement = must_succeed(arena.allocate_aggregate(Aggregate::default(), 2));
    let replacement_handle = must_succeed(replacement.aggregate_handle());

    assert_eq!(replacement_handle.index(), stale_handle.index());
    assert_ne!(replacement_handle.generation(), stale_handle.generation());
    assert!(matches!(
        must_fail(arena.aggregate(unreachable)),
        ExecutionError::StaleValueArenaHandle { .. }
    ));
    must_succeed(arena.aggregate(reachable));
    must_succeed(arena.aggregate(replacement));
    assert_eq!(arena.metrics().cumulative_collections, 1);
    assert_eq!(arena.metrics().cumulative_reclaimed_entries, 1);
    assert_eq!(arena.metrics().cumulative_reused_entries, 1);
    assert_eq!(arena.metrics().live_entries, 2);
    assert_eq!(arena.metrics().peak_live_entries, 2);
    assert_eq!(arena.metrics().peak_slots, 2);
}

#[test]
fn tracing_preserves_nested_aggregate_reachability() {
    let mut arena = ValueArena::default();
    let mut child = Aggregate {
        length: 1,
        ..Aggregate::default()
    };
    child.fields[0] = VmValue::int(73);
    let child = must_succeed(arena.allocate_aggregate(child, 2));
    let mut parent = Aggregate {
        length: 1,
        ..Aggregate::default()
    };
    parent.fields[0] = child;
    let parent = must_succeed(arena.allocate_aggregate(parent, 2));

    must_succeed(arena.collect([parent], 2));

    assert_eq!(arena.metrics().live_entries, 2);
    let child = must_succeed(arena.aggregate(child));
    assert_eq!(must_succeed(child.fields[0].as_int()), 73);
}

#[test]
fn iterator_state_is_owned_by_its_arena_handle() {
    let mut arena = ValueArena::default();
    let state = IteratorState {
        source: IteratorSourceState::Range {
            current: 3,
            end: 7,
            step: 2,
            inclusive: true,
        },
        live: true,
    };
    let iterator = must_succeed(arena.allocate_iterator(state, 1));

    let stored = must_succeed(arena.iterator_mut(iterator));

    let IteratorSourceState::Range {
        current,
        end,
        step,
        inclusive,
    } = stored.source
    else {
        panic!("stored iterator should preserve its range source")
    };
    assert_eq!(current, 3);
    assert_eq!(end, 7);
    assert_eq!(step, 2);
    assert!(inclusive);
    assert!(stored.live);
}

#[test]
fn failed_collection_clears_marks_and_preserves_metrics_before_retry() {
    let mut arena = ValueArena::default();
    let stale = must_succeed(arena.allocate_aggregate(Aggregate::default(), 1));
    must_succeed(arena.collect([], 1));
    let live = must_succeed(arena.allocate_aggregate(Aggregate::default(), 1));
    let before = arena.metrics();

    let error = must_fail(arena.collect([live, stale], 1));

    assert!(matches!(
        error,
        ExecutionError::StaleValueArenaHandle { .. }
    ));
    let after_failure = arena.metrics();
    assert_eq!(
        after_failure.cumulative_allocations,
        before.cumulative_allocations
    );
    assert_eq!(
        after_failure.cumulative_collections,
        before.cumulative_collections
    );
    assert_eq!(
        after_failure.cumulative_reclaimed_entries,
        before.cumulative_reclaimed_entries
    );
    assert_eq!(after_failure.live_entries, before.live_entries);
    assert_eq!(after_failure.slots, before.slots);
    assert_eq!(after_failure.peak_owned_bytes, after_failure.owned_bytes);
    assert!(arena.mark_stack.is_empty());
    assert!(arena.slots.iter().all(|slot| !slot.marked));

    must_succeed(arena.collect([], 1));
    assert!(matches!(
        must_fail(arena.aggregate(live)),
        ExecutionError::StaleValueArenaHandle { .. }
    ));
}

#[test]
fn collection_rejects_unrooted_live_iterator_before_mutating_the_arena() {
    let mut arena = ValueArena::default();
    let iterator = must_succeed(arena.allocate_iterator(
        IteratorState {
            source: IteratorSourceState::Range {
                current: 0,
                end: 1,
                step: 1,
                inclusive: false,
            },
            live: true,
        },
        1,
    ));
    let before = arena.metrics();

    let error = must_fail(arena.collect([], 1));

    assert!(matches!(
        error,
        ExecutionError::LiveIteratorReclamation { index: 0 }
    ));
    assert_eq!(arena.metrics(), before);
    assert!(arena.mark_stack.is_empty());
    assert!(arena.slots.iter().all(|slot| !slot.marked));
    assert!(must_succeed(arena.iterator(iterator)).live);

    must_succeed(arena.iterator_mut(iterator)).live = false;
    must_succeed(arena.collect([], 1));
    assert!(matches!(
        must_fail(arena.iterator(iterator)),
        ExecutionError::StaleValueArenaHandle { .. }
    ));
}

#[test]
fn iterator_step_root_preserves_its_wrapped_aggregate_handle() {
    let mut arena = ValueArena::default();
    let child = must_succeed(arena.allocate_aggregate(Aggregate::default(), 2));
    let dead_iterator = must_succeed(arena.allocate_iterator(IteratorState::default(), 2));
    let step = must_succeed(VmValue::iterator_step(Some(child)));

    must_succeed(arena.collect([step], 2));

    must_succeed(arena.aggregate(child));
    assert!(matches!(
        must_fail(arena.iterator(dead_iterator)),
        ExecutionError::StaleValueArenaHandle { .. }
    ));
}

fn must_succeed<T>(result: Result<T, ExecutionError>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("arena operation should succeed: {error}"),
    }
}

fn must_fail<T>(result: Result<T, ExecutionError>) -> ExecutionError {
    match result {
        Ok(_) => panic!("arena operation should fail"),
        Err(error) => error,
    }
}
