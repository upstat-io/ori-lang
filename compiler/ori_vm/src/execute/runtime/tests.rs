use ori_arc::{ArcVarId, ArgOwnership, RcStrategy};
use ori_repr::executable::IteratorSource;

use super::{Interpreter, VmValue};
use crate::bytecode::Register;
use crate::execute::heap::HeapObject;
use crate::execute::tests::{verified_aggregate_program, verified_constant_string_program};
use crate::{ExecutionConfig, ExecutionError, ValueKind, VerifiedProgram};

#[test]
fn shared_list_push_quota_failure_preserves_owner_children_and_continuation() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_heap_objects: 2,
        ..ExecutionConfig::default()
    };
    let mut interpreter = started(&program, config);
    let child = must_succeed(
        interpreter
            .heap
            .allocate(HeapObject::List(vec![VmValue::int(3)]), 2),
    );
    let list = must_succeed(interpreter.heap.allocate(HeapObject::List(vec![child]), 2));
    must_succeed(interpreter.heap.increment(list, 1));
    let list_register = register(0);
    let value_register = register(1);
    interpreter.set_register(0, list_register, list);
    interpreter.set_register(0, value_register, VmValue::int(9));
    let before = interpreter.heap.metrics();

    let error = must_fail(interpreter.list_push(
        0,
        list_register,
        value_register,
        ArgOwnership::Owned,
        ArgOwnership::Owned,
    ));

    assert!(matches!(
        error,
        ExecutionError::ResourceLimit {
            resource: "live heap objects",
            limit: 2,
        }
    ));
    assert_eq!(interpreter.heap.metrics(), before);
    assert_eq!(must_succeed(interpreter.heap.references(list)), 2);
    assert_eq!(must_succeed(interpreter.heap.references(child)), 1);
    assert!(interpreter.ownership_scratch.is_empty());
    assert_eq!(
        must_succeed(interpreter.length(0, list_register)),
        VmValue::int(1),
    );
    must_succeed(interpreter.release_owned_value(list));
    must_succeed(interpreter.release_owned_value(list));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn shared_list_replacement_quota_failure_rolls_back_prepared_retains() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_heap_objects: 2,
        ..ExecutionConfig::default()
    };
    let mut interpreter = started(&program, config);
    let child = must_succeed(
        interpreter
            .heap
            .allocate(HeapObject::List(vec![VmValue::int(3)]), 2),
    );
    let list = must_succeed(
        interpreter
            .heap
            .allocate(HeapObject::List(vec![child, VmValue::int(7)]), 2),
    );
    must_succeed(interpreter.heap.increment(list, 1));
    let list_register = register(0);
    let index_register = register(1);
    let value_register = register(2);
    interpreter.set_register(0, list_register, list);
    interpreter.set_register(0, index_register, VmValue::int(1));
    interpreter.set_register(0, value_register, VmValue::int(11));
    let before = interpreter.heap.metrics();

    let error = must_fail(interpreter.replace_list_element(
        0,
        list_register,
        index_register,
        value_register,
        ArgOwnership::Owned,
        ArgOwnership::Owned,
    ));

    assert!(matches!(
        error,
        ExecutionError::ResourceLimit {
            resource: "live heap objects",
            limit: 2,
        }
    ));
    assert_eq!(interpreter.heap.metrics(), before);
    assert_eq!(must_succeed(interpreter.heap.references(list)), 2);
    assert_eq!(must_succeed(interpreter.heap.references(child)), 1);
    let values = match &must_succeed(interpreter.heap.get(list)).object {
        HeapObject::List(values) => values,
        HeapObject::Builder(_)
        | HeapObject::String(_)
        | HeapObject::Closure { .. }
        | HeapObject::Vacant => {
            panic!("test owner should remain a list")
        }
    };
    assert_eq!(values.as_slice(), &[child, VmValue::int(7)]);
    assert!(interpreter.ownership_scratch.is_empty());
    must_succeed(interpreter.release_owned_value(list));
    must_succeed(interpreter.release_owned_value(list));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn unique_list_push_collection_quota_failure_preserves_list_and_continuation() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_collection_elements: 1,
        ..ExecutionConfig::default()
    };
    let mut interpreter = started(&program, config);
    let list = must_succeed(interpreter.heap.allocate(
        HeapObject::List(vec![VmValue::int(3)]),
        interpreter.config.max_heap_objects,
    ));
    let list_register = register(0);
    let value_register = register(1);
    interpreter.set_register(0, list_register, list);
    interpreter.set_register(0, value_register, VmValue::int(9));
    let before = interpreter.heap.metrics();

    let error = must_fail(interpreter.list_push(
        0,
        list_register,
        value_register,
        ArgOwnership::Owned,
        ArgOwnership::Owned,
    ));

    assert!(matches!(
        error,
        ExecutionError::ResourceLimit {
            resource: "collection elements",
            limit: 1,
        }
    ));
    assert_eq!(interpreter.heap.metrics(), before);
    assert_eq!(
        must_succeed(interpreter.length(0, list_register)),
        VmValue::int(1),
    );
    let values = match &must_succeed(interpreter.heap.get(list)).object {
        HeapObject::List(values) => values,
        HeapObject::Builder(_)
        | HeapObject::String(_)
        | HeapObject::Closure { .. }
        | HeapObject::Vacant => {
            panic!("failed push should preserve the list")
        }
    };
    assert_eq!(values.as_slice(), &[VmValue::int(3)]);
    must_succeed(interpreter.release_owned_value(list));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn string_split_batch_quota_failure_allocates_nothing_and_can_continue() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_heap_objects: 2,
        ..ExecutionConfig::default()
    };
    let mut interpreter = started(&program, config);
    let value = must_succeed(
        interpreter
            .heap
            .allocate(HeapObject::String("a,b".to_owned()), 2),
    );
    let separator = must_succeed(
        interpreter
            .heap
            .allocate(HeapObject::String(",".to_owned()), 2),
    );
    let value_register = register(0);
    let separator_register = register(1);
    interpreter.set_register(0, value_register, value);
    interpreter.set_register(0, separator_register, separator);
    let before = interpreter.heap.metrics();

    let error = must_fail(interpreter.string_split(0, value_register, separator_register));

    assert!(matches!(
        error,
        ExecutionError::ResourceLimit {
            resource: "live heap objects",
            limit: 2,
        }
    ));
    assert_eq!(interpreter.heap.metrics(), before);
    assert_eq!(
        must_succeed(interpreter.length(0, value_register)),
        VmValue::int(3),
    );
    must_succeed(interpreter.release_owned_value(value));
    must_succeed(interpreter.release_owned_value(separator));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn string_split_constant_validation_rejects_before_piece_allocation() {
    let program = verified_constant_string_program("a,b");
    let config = ExecutionConfig {
        max_collection_elements: 1,
        ..ExecutionConfig::default()
    };
    let mut interpreter = started(&program, config);
    let Some(constant_index) = interpreter
        .program
        .strings
        .iter()
        .position(|value| value == "a,b")
    else {
        panic!("test literal should be in the verified string table")
    };
    let value = must_succeed(VmValue::constant_string(constant_index));
    let separator = must_succeed(interpreter.heap.allocate(
        HeapObject::String(",".to_owned()),
        interpreter.config.max_heap_objects,
    ));
    let value_register = register(0);
    let separator_register = register(1);
    interpreter.set_register(0, value_register, value);
    interpreter.set_register(0, separator_register, separator);
    let before = interpreter.heap.metrics();

    let error = must_fail(interpreter.string_split(0, value_register, separator_register));

    assert!(matches!(
        error,
        ExecutionError::ResourceLimit {
            resource: "collection elements",
            limit: 1,
        }
    ));
    assert_eq!(interpreter.heap.metrics(), before);
    assert_eq!(
        must_succeed(interpreter.length(0, value_register)),
        VmValue::int(3),
    );
    must_succeed(interpreter.release_owned_value(separator));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn length_supports_constant_strings_heap_strings_and_lists() {
    let program = verified_constant_string_program("café");
    let mut interpreter = started(&program, ExecutionConfig::default());
    let constant_index = interpreter
        .program
        .strings
        .iter()
        .position(|value| value == "café");
    let Some(constant_index) = constant_index else {
        panic!("test literal should be in the verified string table")
    };
    let constant = must_succeed(VmValue::constant_string(constant_index));
    let heap_string = must_succeed(interpreter.heap.allocate(
        HeapObject::String("hello".to_owned()),
        interpreter.config.max_heap_objects,
    ));
    let list = must_succeed(interpreter.heap.allocate(
        HeapObject::List(vec![VmValue::int(1), VmValue::int(2)]),
        interpreter.config.max_heap_objects,
    ));
    let builder = must_succeed(interpreter.heap.allocate(
        HeapObject::Builder(vec![VmValue::int(1), VmValue::int(2), VmValue::int(3)]),
        interpreter.config.max_heap_objects,
    ));
    let target = register(0);

    interpreter.set_register(0, target, constant);
    assert_eq!(must_succeed(interpreter.length(0, target)), VmValue::int(5));
    interpreter.set_register(0, target, heap_string);
    assert_eq!(must_succeed(interpreter.length(0, target)), VmValue::int(5));
    interpreter.set_register(0, target, list);
    assert_eq!(must_succeed(interpreter.length(0, target)), VmValue::int(2));
    interpreter.set_register(0, target, builder);
    assert_eq!(must_succeed(interpreter.length(0, target)), VmValue::int(3));
    interpreter.set_register(0, target, VmValue::int(4));
    assert!(matches!(
        must_fail(interpreter.length(0, target)),
        ExecutionError::TypeMismatch {
            expected: ValueKind::Heap,
            found: ValueKind::Int,
        }
    ));

    must_succeed(interpreter.release_owned_value(heap_string));
    must_succeed(interpreter.release_owned_value(list));
    must_succeed(interpreter.release_owned_value(builder));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn owned_list_iterator_yields_exact_bool_values_and_drops_early_once() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program, ExecutionConfig::default());
    let list = must_succeed(interpreter.heap.allocate(
        HeapObject::List(vec![VmValue::bool(true), VmValue::bool(false)]),
        interpreter.config.max_heap_objects,
    ));
    let source = register(0);
    let destination = register(1);
    interpreter.set_register(0, source, list);

    let iterator = must_succeed(interpreter.create_iterator(
        0,
        destination,
        IteratorSource::List,
        source,
        ArgOwnership::Owned,
    ));
    interpreter.set_register(0, destination, iterator);
    assert_eq!(must_succeed(interpreter.heap.references(list)), 1);

    let first = must_succeed(interpreter.iter_next(0, destination));
    assert_eq!(must_succeed(interpreter.project(first, 0)), VmValue::int(1));
    assert_eq!(
        must_succeed(interpreter.project(first, 1)),
        VmValue::bool(true),
    );

    must_succeed(interpreter.drop_iterator(iterator));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
    must_succeed(interpreter.drop_iterator(iterator));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn owned_shared_list_iterator_releases_only_its_transferred_reference() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program, ExecutionConfig::default());
    let list = must_succeed(interpreter.heap.allocate(
        HeapObject::List(vec![VmValue::int(7)]),
        interpreter.config.max_heap_objects,
    ));
    must_succeed(interpreter.heap.increment(list, 1));
    let source = register(0);
    let destination = register(1);
    interpreter.set_register(0, source, list);

    let iterator = must_succeed(interpreter.create_iterator(
        0,
        destination,
        IteratorSource::List,
        source,
        ArgOwnership::Owned,
    ));
    assert_eq!(must_succeed(interpreter.heap.references(list)), 2);

    must_succeed(interpreter.drop_iterator(iterator));
    assert_eq!(must_succeed(interpreter.heap.references(list)), 1);
    must_succeed(interpreter.release_owned_value(list));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn borrowed_list_iterator_neither_retains_nor_releases_the_source() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program, ExecutionConfig::default());
    let list = must_succeed(interpreter.heap.allocate(
        HeapObject::List(vec![VmValue::int(11)]),
        interpreter.config.max_heap_objects,
    ));
    let source = register(0);
    let destination = register(1);
    interpreter.set_register(0, source, list);

    let iterator = must_succeed(interpreter.create_iterator(
        0,
        destination,
        IteratorSource::List,
        source,
        ArgOwnership::Borrowed,
    ));
    assert_eq!(must_succeed(interpreter.heap.references(list)), 1);

    must_succeed(interpreter.drop_iterator(iterator));
    assert_eq!(must_succeed(interpreter.heap.references(list)), 1);
    must_succeed(interpreter.release_owned_value(list));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn iterator_step_wraps_heap_values_and_obeys_aggregate_field_rc() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program, ExecutionConfig::default());
    let child = must_succeed(interpreter.heap.allocate(
        HeapObject::String("owned child".to_owned()),
        interpreter.config.max_heap_objects,
    ));
    let step = must_succeed(VmValue::iterator_step(Some(child)));

    assert_eq!(must_succeed(interpreter.project(step, 1)), child);
    must_succeed(interpreter.retain_with_strategy(step, 1, RcStrategy::AggregateFields));
    assert_eq!(must_succeed(interpreter.heap.references(child)), 2);
    must_succeed(interpreter.release_with_strategy(step, RcStrategy::AggregateFields));
    assert_eq!(must_succeed(interpreter.heap.references(child)), 1);
    must_succeed(interpreter.release_with_strategy(step, RcStrategy::AggregateFields));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn releasing_scalar_list_keeps_ownership_worklist_constant_sized() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program, ExecutionConfig::default());
    let list = must_succeed(interpreter.heap.allocate(
        HeapObject::List(vec![VmValue::bool(true); 10_000]),
        interpreter.config.max_heap_objects,
    ));

    must_succeed(interpreter.release_owned_value(list));

    assert_eq!(interpreter.heap.metrics().live_objects, 0);
    assert!(interpreter.ownership_scratch.is_empty());
    assert!(interpreter.ownership_scratch.capacity() <= 4);
}

fn started(program: &VerifiedProgram, config: ExecutionConfig) -> Interpreter<'_> {
    let mut interpreter = Interpreter::new(program, config);
    must_succeed(interpreter.push_root());
    interpreter.frames[0].registers.resize(3, VmValue::UNIT);
    interpreter
}

fn register(index: u32) -> Register {
    Register::from_arc(ArcVarId::new(index))
}

fn must_succeed<T>(result: Result<T, ExecutionError>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => panic!("runtime operation should succeed: {error}"),
    }
}

fn must_fail<T>(result: Result<T, ExecutionError>) -> ExecutionError {
    match result {
        Ok(_) => panic!("runtime operation should fail"),
        Err(error) => error,
    }
}
