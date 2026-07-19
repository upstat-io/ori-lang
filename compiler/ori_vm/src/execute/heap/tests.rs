use super::{Heap, HeapObject};
use crate::execute::value::VmValue;

#[test]
fn heap_allocate_release_and_reuse_preserves_accounting_invariants() {
    let values = Vec::<VmValue>::with_capacity(4);
    let list_bytes = values.capacity() * std::mem::size_of::<VmValue>();
    let mut heap = Heap::default();
    let first = must_succeed(heap.allocate(HeapObject::List(values), 1));

    assert_eq!(heap.metrics().cumulative_allocations, 1);
    assert_eq!(heap.metrics().live_objects, 1);
    assert_eq!(heap.metrics().live_payload_bytes, list_bytes);

    must_succeed(heap.decrement(first));
    assert_eq!(heap.metrics().live_objects, 0);
    assert_eq!(heap.metrics().live_payload_bytes, 0);

    let text = String::with_capacity(7);
    let string_bytes = text.capacity();
    let second = must_succeed(heap.allocate(HeapObject::String(text), 1));
    assert_eq!(
        must_succeed(second.heap_index()),
        must_succeed(first.heap_index())
    );
    assert_eq!(heap.metrics().cumulative_allocations, 2);
    assert_eq!(
        heap.metrics().cumulative_payload_bytes_lower_bound,
        (list_bytes + string_bytes) as u128
    );
    assert_eq!(heap.metrics().peak_live_objects, 1);
}

#[test]
fn heap_builder_growth_updates_live_peak_and_cumulative_payload_bytes() {
    let mut heap = Heap::default();
    let builder = must_succeed(heap.allocate(HeapObject::Builder(Vec::new()), 1));

    let values = vec![VmValue::UNIT];
    must_succeed(heap.replace_object(builder, HeapObject::Builder(values)));

    let metrics = heap.metrics();
    assert!(metrics.live_payload_bytes >= std::mem::size_of::<VmValue>());
    assert_eq!(metrics.peak_live_payload_bytes, metrics.live_payload_bytes);
    assert_eq!(
        metrics.cumulative_payload_bytes_lower_bound,
        metrics.live_payload_bytes as u128
    );
}

#[test]
fn failed_payload_metric_preflight_preserves_object_and_metrics() {
    let mut heap = Heap::default();
    let builder = must_succeed(heap.allocate(HeapObject::Builder(Vec::new()), 1));
    heap.live_payload_bytes = usize::MAX;
    let before = heap.metrics();
    let replacement = HeapObject::Builder(Vec::with_capacity(1));

    let error = must_fail(heap.replace_object(builder, replacement));

    assert!(matches!(
        error,
        crate::ExecutionError::MetricOverflow {
            metric: "live heap payload bytes",
        }
    ));
    assert_eq!(heap.metrics(), before);
    let HeapObject::Builder(values) = &must_succeed(heap.get(builder)).object else {
        panic!("failed replacement should preserve the builder")
    };
    assert!(values.is_empty());
    assert_eq!(values.capacity(), 0);
}

#[test]
fn failed_cumulative_payload_preflight_preserves_object_and_metrics() {
    let mut heap = Heap::default();
    let builder = must_succeed(heap.allocate(HeapObject::Builder(Vec::new()), 1));
    heap.cumulative_payload_bytes_lower_bound = u128::MAX;
    let before = heap.metrics();
    let replacement = HeapObject::Builder(Vec::with_capacity(1));

    let error = must_fail(heap.replace_object(builder, replacement));

    assert!(matches!(
        error,
        crate::ExecutionError::MetricOverflow {
            metric: "cumulative heap payload bytes",
        }
    ));
    assert_eq!(heap.metrics(), before);
    let HeapObject::Builder(values) = &must_succeed(heap.get(builder)).object else {
        panic!("failed replacement should preserve the builder")
    };
    assert!(values.is_empty());
    assert_eq!(values.capacity(), 0);
}

#[test]
fn retain_plan_late_count_overflow_preserves_every_reference_count() {
    let mut heap = Heap::default();
    let first = must_succeed(heap.allocate(HeapObject::String("first".to_owned()), 2));
    let second = must_succeed(heap.allocate(HeapObject::String("second".to_owned()), 2));
    let second_index = must_succeed(second.heap_index());
    heap.slots[second_index].references = u32::MAX;
    let before_first = must_succeed(heap.references(first));
    let before_second = must_succeed(heap.references(second));

    let error = must_fail(heap.validate_increment_plan(&[first, second], 1));

    assert!(matches!(
        error,
        crate::ExecutionError::ReferenceCountOverflow
    ));
    assert_eq!(must_succeed(heap.references(first)), before_first);
    assert_eq!(must_succeed(heap.references(second)), before_second);

    heap.slots[second_index].references = 1;
    must_succeed(heap.decrement(first));
    must_succeed(heap.decrement(second));
    assert_eq!(heap.metrics().live_objects, 0);
}

fn must_succeed<T>(result: Result<T, crate::ExecutionError>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("heap operation should succeed: {error}"),
    }
}

fn must_fail<T>(result: Result<T, crate::ExecutionError>) -> crate::ExecutionError {
    match result {
        Ok(_) => panic!("heap operation should fail"),
        Err(error) => error,
    }
}
