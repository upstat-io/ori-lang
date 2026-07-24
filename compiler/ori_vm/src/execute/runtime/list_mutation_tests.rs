use super::{Interpreter, VmValue};
use crate::bytecode::Register;
use crate::execute::heap::HeapObject;
use crate::execute::tests::verified_aggregate_program;
use crate::{ExecutionConfig, ExecutionError, VerifiedProgram};
use ori_arc::{ArcVarId, ArgOwnership};

#[test]
fn list_concat_consumes_distinct_inputs_and_preserves_nested_ownership() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let left_child = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let right_child = allocate_list(&mut interpreter, vec![VmValue::int(2)]);
    let left = allocate_list(&mut interpreter, vec![left_child]);
    let right = allocate_list(&mut interpreter, vec![right_child]);
    let left_register = register(0);
    let right_register = register(1);
    interpreter.set_register(0, left_register, left);
    interpreter.set_register(0, right_register, right);

    let result = must_succeed(interpreter.list_concat(0, left_register, right_register));

    assert_released(&interpreter, left);
    assert_released(&interpreter, right);
    assert_eq!(
        list_values(&interpreter, result),
        &[left_child, right_child]
    );
    assert_eq!(must_succeed(interpreter.heap.references(left_child)), 1);
    assert_eq!(must_succeed(interpreter.heap.references(right_child)), 1);
    must_succeed(interpreter.release_owned_value(result));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn list_concat_same_handle_consumes_two_obligations_transactionally() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let child = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let list = allocate_list(&mut interpreter, vec![child]);
    must_succeed(interpreter.heap.increment(list, 1));
    let left_register = register(0);
    let right_register = register(1);
    interpreter.set_register(0, left_register, list);
    interpreter.set_register(0, right_register, list);

    let result = must_succeed(interpreter.list_concat(0, left_register, right_register));

    assert_released(&interpreter, list);
    assert_eq!(list_values(&interpreter, result), &[child, child]);
    assert_eq!(must_succeed(interpreter.heap.references(child)), 2);
    must_succeed(interpreter.release_owned_value(result));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn list_concat_validation_failure_preserves_both_inputs() {
    let program = verified_aggregate_program();
    let config = ExecutionConfig {
        max_collection_elements: 1,
        ..ExecutionConfig::default()
    };
    let mut interpreter = Interpreter::new(&program, config);
    must_succeed(interpreter.push_root());
    interpreter.frames[0].registers.resize(3, VmValue::UNIT);
    let left = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let right = allocate_list(&mut interpreter, vec![VmValue::int(2)]);
    let left_register = register(0);
    let right_register = register(1);
    interpreter.set_register(0, left_register, left);
    interpreter.set_register(0, right_register, right);
    let before = interpreter.heap.metrics();

    let error = must_fail(interpreter.list_concat(0, left_register, right_register));

    assert!(matches!(
        error,
        ExecutionError::ResourceLimit {
            resource: "collection elements",
            limit: 1,
        }
    ));
    assert_eq!(interpreter.heap.metrics(), before);
    assert_eq!(must_succeed(interpreter.heap.references(left)), 1);
    assert_eq!(must_succeed(interpreter.heap.references(right)), 1);
    must_succeed(interpreter.release_owned_value(left));
    must_succeed(interpreter.release_owned_value(right));
}

#[test]
fn list_concat_rejects_unfunded_same_handle_before_mutation() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let list = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let left_register = register(0);
    let right_register = register(1);
    interpreter.set_register(0, left_register, list);
    interpreter.set_register(0, right_register, list);
    let before = interpreter.heap.metrics();

    let error = must_fail(interpreter.list_concat(0, left_register, right_register));

    assert!(matches!(error, ExecutionError::ReferenceCountUnderflow));
    assert_eq!(interpreter.heap.metrics(), before);
    assert_eq!(must_succeed(interpreter.heap.references(list)), 1);
    assert_eq!(list_values(&interpreter, list), &[VmValue::int(1)]);
    must_succeed(interpreter.release_owned_value(list));
}

#[test]
fn unique_insert_prepend_and_remove_preserve_order_in_place() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let list = allocate_list(&mut interpreter, vec![VmValue::int(2), VmValue::int(3)]);
    let list_register = register(0);
    let index_register = register(1);
    let value_register = register(2);
    interpreter.set_register(0, list_register, list);
    interpreter.set_register(0, index_register, VmValue::int(1));
    interpreter.set_register(0, value_register, VmValue::int(9));

    assert_eq!(
        must_succeed(interpreter.list_insert(
            0,
            list_register,
            index_register,
            value_register,
            ArgOwnership::Owned,
            ArgOwnership::Owned,
        )),
        list,
    );
    interpreter.set_register(0, value_register, VmValue::int(1));
    assert_eq!(
        must_succeed(interpreter.list_prepend(
            0,
            list_register,
            value_register,
            ArgOwnership::Owned,
            ArgOwnership::Owned,
        )),
        list,
    );
    interpreter.set_register(0, index_register, VmValue::int(2));
    assert_eq!(
        must_succeed(interpreter.list_remove(
            0,
            list_register,
            index_register,
            ArgOwnership::Owned,
        )),
        list,
    );

    assert_eq!(
        list_values(&interpreter, list),
        &[VmValue::int(1), VmValue::int(2), VmValue::int(3)]
    );
    must_succeed(interpreter.release_owned_value(list));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn unique_set_and_remove_release_replaced_nested_values() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let replaced = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let removed = allocate_list(&mut interpreter, vec![VmValue::int(2)]);
    let kept = allocate_list(&mut interpreter, vec![VmValue::int(3)]);
    let replacement = allocate_list(&mut interpreter, vec![VmValue::int(4)]);
    let list = allocate_list(&mut interpreter, vec![replaced, removed, kept]);
    let list_register = register(0);
    let index_register = register(1);
    let value_register = register(2);
    interpreter.set_register(0, list_register, list);
    interpreter.set_register(0, index_register, VmValue::int(0));
    interpreter.set_register(0, value_register, replacement);

    assert_eq!(
        must_succeed(interpreter.replace_list_element(
            0,
            list_register,
            index_register,
            value_register,
            ArgOwnership::Owned,
            ArgOwnership::Owned,
        )),
        list,
    );
    assert_released(&interpreter, replaced);
    interpreter.set_register(0, index_register, VmValue::int(1));
    assert_eq!(
        must_succeed(interpreter.list_remove(
            0,
            list_register,
            index_register,
            ArgOwnership::Owned,
        )),
        list,
    );
    assert_released(&interpreter, removed);
    assert_eq!(list_values(&interpreter, list), &[replacement, kept]);

    must_succeed(interpreter.release_owned_value(list));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn shared_insert_retains_copied_children_but_not_the_moved_value() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let copied = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let moved = allocate_list(&mut interpreter, vec![VmValue::int(2)]);
    let original = allocate_list(&mut interpreter, vec![copied]);
    must_succeed(interpreter.heap.increment(original, 1));
    let list_register = register(0);
    let index_register = register(1);
    let value_register = register(2);
    interpreter.set_register(0, list_register, original);
    interpreter.set_register(0, index_register, VmValue::int(1));
    interpreter.set_register(0, value_register, moved);

    let inserted = must_succeed(interpreter.list_insert(
        0,
        list_register,
        index_register,
        value_register,
        ArgOwnership::Owned,
        ArgOwnership::Owned,
    ));

    assert_ne!(inserted, original);
    assert_eq!(must_succeed(interpreter.heap.references(original)), 1);
    assert_eq!(must_succeed(interpreter.heap.references(copied)), 2);
    assert_eq!(must_succeed(interpreter.heap.references(moved)), 1);
    assert_eq!(list_values(&interpreter, original), &[copied]);
    assert_eq!(list_values(&interpreter, inserted), &[copied, moved]);
    must_succeed(interpreter.release_owned_value(original));
    assert_eq!(must_succeed(interpreter.heap.references(copied)), 1);
    must_succeed(interpreter.release_owned_value(inserted));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn shared_prepend_preserves_original_and_nested_ownership() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let copied = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let moved = allocate_list(&mut interpreter, vec![VmValue::int(2)]);
    let original = allocate_list(&mut interpreter, vec![copied]);
    must_succeed(interpreter.heap.increment(original, 1));
    let list_register = register(0);
    let value_register = register(2);
    interpreter.set_register(0, list_register, original);
    interpreter.set_register(0, value_register, moved);

    let prepended = must_succeed(interpreter.list_prepend(
        0,
        list_register,
        value_register,
        ArgOwnership::Owned,
        ArgOwnership::Owned,
    ));

    assert_eq!(list_values(&interpreter, original), &[copied]);
    assert_eq!(list_values(&interpreter, prepended), &[moved, copied]);
    assert_eq!(must_succeed(interpreter.heap.references(copied)), 2);
    assert_eq!(must_succeed(interpreter.heap.references(moved)), 1);
    must_succeed(interpreter.release_owned_value(original));
    must_succeed(interpreter.release_owned_value(prepended));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn shared_remove_retains_kept_children_only() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let removed = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let kept = allocate_list(&mut interpreter, vec![VmValue::int(2)]);
    let original = allocate_list(&mut interpreter, vec![removed, kept]);
    must_succeed(interpreter.heap.increment(original, 1));
    let list_register = register(0);
    let index_register = register(1);
    interpreter.set_register(0, list_register, original);
    interpreter.set_register(0, index_register, VmValue::int(0));

    let shortened = must_succeed(interpreter.list_remove(
        0,
        list_register,
        index_register,
        ArgOwnership::Owned,
    ));

    assert_eq!(list_values(&interpreter, original), &[removed, kept]);
    assert_eq!(list_values(&interpreter, shortened), &[kept]);
    assert_eq!(must_succeed(interpreter.heap.references(removed)), 1);
    assert_eq!(must_succeed(interpreter.heap.references(kept)), 2);
    must_succeed(interpreter.release_owned_value(original));
    assert_released(&interpreter, removed);
    assert_eq!(must_succeed(interpreter.heap.references(kept)), 1);
    must_succeed(interpreter.release_owned_value(shortened));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn shared_set_retains_unchanged_children_only() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let replaced = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let kept = allocate_list(&mut interpreter, vec![VmValue::int(2)]);
    let moved = allocate_list(&mut interpreter, vec![VmValue::int(3)]);
    let original = allocate_list(&mut interpreter, vec![replaced, kept]);
    must_succeed(interpreter.heap.increment(original, 1));
    let list_register = register(0);
    let index_register = register(1);
    let value_register = register(2);
    interpreter.set_register(0, list_register, original);
    interpreter.set_register(0, index_register, VmValue::int(0));
    interpreter.set_register(0, value_register, moved);

    let updated = must_succeed(interpreter.replace_list_element(
        0,
        list_register,
        index_register,
        value_register,
        ArgOwnership::Owned,
        ArgOwnership::Owned,
    ));

    assert_eq!(list_values(&interpreter, original), &[replaced, kept]);
    assert_eq!(list_values(&interpreter, updated), &[moved, kept]);
    assert_eq!(must_succeed(interpreter.heap.references(replaced)), 1);
    assert_eq!(must_succeed(interpreter.heap.references(kept)), 2);
    assert_eq!(must_succeed(interpreter.heap.references(moved)), 1);
    must_succeed(interpreter.release_owned_value(original));
    assert_released(&interpreter, replaced);
    must_succeed(interpreter.release_owned_value(updated));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn borrowed_receiver_and_value_preserve_both_callers() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let copied = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let borrowed = allocate_list(&mut interpreter, vec![VmValue::int(2)]);
    let original = allocate_list(&mut interpreter, vec![copied]);
    let list_register = register(0);
    let value_register = register(2);
    interpreter.set_register(0, list_register, original);
    interpreter.set_register(0, value_register, borrowed);

    let prepended = must_succeed(interpreter.list_prepend(
        0,
        list_register,
        value_register,
        ArgOwnership::Borrowed,
        ArgOwnership::Borrowed,
    ));

    assert_ne!(prepended, original);
    assert_eq!(must_succeed(interpreter.heap.references(original)), 1);
    assert_eq!(must_succeed(interpreter.heap.references(copied)), 2);
    assert_eq!(must_succeed(interpreter.heap.references(borrowed)), 2);
    assert_eq!(list_values(&interpreter, original), &[copied]);
    assert_eq!(list_values(&interpreter, prepended), &[borrowed, copied]);
    must_succeed(interpreter.release_owned_value(original));
    must_succeed(interpreter.release_owned_value(borrowed));
    must_succeed(interpreter.release_owned_value(prepended));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn owned_shared_receiver_consumes_one_reference_and_retains_borrowed_value() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let copied = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let borrowed = allocate_list(&mut interpreter, vec![VmValue::int(2)]);
    let original = allocate_list(&mut interpreter, vec![copied]);
    must_succeed(interpreter.heap.increment(original, 1));
    let list_register = register(0);
    let index_register = register(1);
    let value_register = register(2);
    interpreter.set_register(0, list_register, original);
    interpreter.set_register(0, index_register, VmValue::int(1));
    interpreter.set_register(0, value_register, borrowed);

    let inserted = must_succeed(interpreter.list_insert(
        0,
        list_register,
        index_register,
        value_register,
        ArgOwnership::Owned,
        ArgOwnership::Borrowed,
    ));

    assert_eq!(must_succeed(interpreter.heap.references(original)), 1);
    assert_eq!(must_succeed(interpreter.heap.references(copied)), 2);
    assert_eq!(must_succeed(interpreter.heap.references(borrowed)), 2);
    must_succeed(interpreter.release_owned_value(borrowed));
    must_succeed(interpreter.release_owned_value(original));
    must_succeed(interpreter.release_owned_value(inserted));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn unique_in_capacity_insert_does_not_change_heap_payload_metrics() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let borrowed = allocate_list(&mut interpreter, vec![VmValue::int(2)]);
    let mut values = Vec::with_capacity(4);
    values.push(VmValue::int(1));
    let list = allocate_list(&mut interpreter, values);
    let list_register = register(0);
    let index_register = register(1);
    let value_register = register(2);
    interpreter.set_register(0, list_register, list);
    interpreter.set_register(0, index_register, VmValue::int(1));
    interpreter.set_register(0, value_register, borrowed);
    let before = interpreter.heap.metrics();

    let inserted = must_succeed(interpreter.list_insert(
        0,
        list_register,
        index_register,
        value_register,
        ArgOwnership::Owned,
        ArgOwnership::Borrowed,
    ));

    assert_eq!(inserted, list);
    assert_eq!(interpreter.heap.metrics(), before);
    assert_eq!(must_succeed(interpreter.heap.references(borrowed)), 2);
    assert_eq!(
        list_values(&interpreter, list),
        &[VmValue::int(1), borrowed]
    );
    must_succeed(interpreter.release_owned_value(borrowed));
    must_succeed(interpreter.release_owned_value(list));
    assert_eq!(interpreter.heap.metrics().live_objects, 0);
}

#[test]
fn list_mutation_indices_fail_closed_without_mutation() {
    let program = verified_aggregate_program();
    let mut interpreter = started(&program);
    let list = allocate_list(&mut interpreter, vec![VmValue::int(1)]);
    let list_register = register(0);
    let index_register = register(1);
    let value_register = register(2);
    interpreter.set_register(0, list_register, list);
    interpreter.set_register(0, index_register, VmValue::int(1));
    interpreter.set_register(0, value_register, VmValue::int(2));

    let set_error = must_fail(interpreter.replace_list_element(
        0,
        list_register,
        index_register,
        value_register,
        ArgOwnership::Owned,
        ArgOwnership::Owned,
    ));
    assert!(matches!(
        set_error,
        ExecutionError::CollectionIndexOutOfBounds {
            index: 1,
            length: 1,
        }
    ));
    let remove_error =
        must_fail(interpreter.list_remove(0, list_register, index_register, ArgOwnership::Owned));
    assert!(matches!(
        remove_error,
        ExecutionError::CollectionIndexOutOfBounds {
            index: 1,
            length: 1,
        }
    ));
    assert_eq!(list_values(&interpreter, list), &[VmValue::int(1)]);
    must_succeed(interpreter.release_owned_value(list));
}

fn started(program: &VerifiedProgram) -> Interpreter<'_> {
    let mut interpreter = Interpreter::new(program, ExecutionConfig::default());
    must_succeed(interpreter.push_root());
    interpreter.frames[0].registers.resize(3, VmValue::UNIT);
    interpreter
}

fn allocate_list(interpreter: &mut Interpreter<'_>, values: Vec<VmValue>) -> VmValue {
    must_succeed(interpreter.heap.allocate(
        HeapObject::List(values),
        interpreter.config.max_heap_objects,
    ))
}

fn list_values<'a>(interpreter: &'a Interpreter<'_>, list: VmValue) -> &'a [VmValue] {
    match &must_succeed(interpreter.heap.get(list)).object {
        HeapObject::List(values) => values,
        HeapObject::Builder(_)
        | HeapObject::String(_)
        | HeapObject::Closure { .. }
        | HeapObject::Vacant => {
            panic!("test value should be a list")
        }
    }
}

fn assert_released(interpreter: &Interpreter<'_>, value: VmValue) {
    assert!(matches!(
        interpreter.heap.references(value),
        Err(ExecutionError::ReleasedHeap { .. })
    ));
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
