//! Pattern error tests.
//!
//! Tests for the error types and error factory functions in `ori_patterns::errors`.
//! These tests validate:
//! - `EvalError` basic functionality and control flow signals
//! - Error factory functions for all error categories
//! - Error message content and distinctness

// Tests use single-char patterns for concise assertions and inline format args for brevity
#![allow(clippy::single_char_pattern, clippy::uninlined_format_args)]

use ori_ir::BinaryOp;
use ori_patterns::{ControlAction, EvalError, Value};

// Import all error factory functions
use ori_patterns::{
    all_requires_list, any_requires_list, await_not_supported, binary_type_mismatch,
    cannot_access_field, cannot_assign_immutable, cannot_get_length, cannot_index,
    collect_requires_range, collection_too_large, default_requires_type_context, division_by_zero,
    expected_list, expected_struct, expected_tuple, field_assignment_not_implemented,
    filter_entries_not_implemented, filter_entries_requires_map, filter_requires_collection,
    find_requires_list, fold_requires_collection, for_pattern_requires_list, for_requires_iterable,
    hash_outside_index, index_assignment_not_implemented, index_out_of_bounds, integer_overflow,
    invalid_assignment_target, invalid_binary_op_for, invalid_literal_pattern, invalid_tuple_field,
    key_not_found, list_pattern_too_long, map_entries_not_implemented, map_entries_requires_map,
    map_key_not_hashable, map_requires_collection, missing_struct_field, modulo_by_zero,
    no_field_on_struct, no_member_in_module, no_such_method, non_exhaustive_match,
    non_integer_in_index, not_callable, operator_not_supported_in_index, parse_error,
    propagated_error_message, range_bound_not_int, recursion_limit_exceeded, self_outside_method,
    tuple_index_out_of_bounds, tuple_pattern_mismatch, unbounded_range_end, undefined_const,
    undefined_function, undefined_variable, unknown_pattern, wrong_arg_count, wrong_arg_type,
    wrong_function_args,
};

// -- EvalError basic functionality --

#[test]
fn test_eval_error_new() {
    let err = EvalError::new("test message");
    assert_eq!(err.message, "test message");
}

// -- ControlAction tests --

#[test]
fn test_control_action_break() {
    let action = ControlAction::Break(Value::int(42));
    assert!(!action.is_error());
    if let ControlAction::Break(v) = action {
        assert_eq!(v, Value::int(42));
    } else {
        panic!("expected ControlAction::Break");
    }
}

#[test]
fn test_control_action_continue() {
    let action = ControlAction::Continue(Value::Void);
    assert!(!action.is_error());
    if let ControlAction::Continue(v) = action {
        assert!(matches!(v, Value::Void));
    } else {
        panic!("expected ControlAction::Continue");
    }
}

#[test]
fn test_control_action_propagate() {
    let action = ControlAction::Propagate(Value::int(42));
    assert!(!action.is_error());
    if let ControlAction::Propagate(v) = action {
        assert_eq!(v, Value::int(42));
    } else {
        panic!("expected ControlAction::Propagate");
    }
}

#[test]
fn test_control_action_error() {
    let action = ControlAction::from(EvalError::new("error"));
    assert!(action.is_error());

    let err = action.into_eval_error();
    assert_eq!(err.message, "error");
}

#[test]
fn test_control_action_from_eval_error() {
    let err = EvalError::new("test");
    let action: ControlAction = err.into();
    assert!(action.is_error());
}

// -- Binary Operation Errors --

#[test]
fn test_invalid_binary_op_for() {
    let err = invalid_binary_op_for("Option", BinaryOp::Add);
    assert!(err.message.contains("operator"));
    assert!(err.message.contains("+"));
    assert!(err.message.contains("Option"));
}

#[test]
fn test_binary_type_mismatch() {
    let err = binary_type_mismatch("int", "string");
    assert!(err.message.contains("int"));
    assert!(err.message.contains("string"));
}

#[test]
fn test_division_by_zero() {
    let err = division_by_zero();
    assert!(err.message.contains("division by zero"));
}

#[test]
fn test_modulo_by_zero() {
    let err = modulo_by_zero();
    assert!(err.message.contains("modulo by zero"));
}

#[test]
fn test_integer_overflow() {
    let err = integer_overflow("addition");
    assert!(err.message.contains("overflow"));
    assert!(err.message.contains("addition"));
}

#[test]
fn test_recursion_limit_exceeded() {
    let err = recursion_limit_exceeded(200);
    assert!(err.message.contains("recursion"));
    assert!(err.message.contains("200"));
    assert!(err.message.contains("limit"));
    assert_eq!(
        err.kind,
        ori_patterns::EvalErrorKind::StackOverflow { depth: 200 }
    );
}

// -- Method Call Errors --

#[test]
fn test_no_such_method() {
    let err = no_such_method("foo", "int");
    assert!(err.message.contains("foo"));
    assert!(err.message.contains("int"));
}

#[test]
fn test_wrong_arg_count() {
    let err = wrong_arg_count("map", 1, 2);
    assert!(err.message.contains("map"));
    assert!(err.message.contains("1"));
    assert!(err.message.contains("2"));
}

#[test]
fn test_wrong_arg_type() {
    let err = wrong_arg_type("filter", "function");
    assert!(err.message.contains("filter"));
    assert!(err.message.contains("function"));
}

// -- Variable and Function Errors --

#[test]
fn test_undefined_variable() {
    let err = undefined_variable("x");
    assert!(err.message.contains("undefined"));
    assert!(err.message.contains("x"));
}

#[test]
fn test_undefined_function() {
    let err = undefined_function("foo");
    assert!(err.message.contains("undefined"));
    assert!(err.message.contains("@foo"));
}

#[test]
fn test_undefined_const() {
    let err = undefined_const("PORT");
    assert!(err.message.contains("constant"));
    assert!(err.message.contains("$PORT"));
}

#[test]
fn test_not_callable() {
    let err = not_callable("int");
    assert!(err.message.contains("int"));
    assert!(err.message.contains("callable"));
}

#[test]
fn test_wrong_function_args() {
    let err = wrong_function_args(2, 3);
    assert!(err.message.contains("2"));
    assert!(err.message.contains("3"));
}

// -- Index and Field Access Errors --

#[test]
fn test_index_out_of_bounds() {
    let err = index_out_of_bounds(10);
    assert!(err.message.contains("10"));
    assert!(err.message.contains("bounds"));
}

#[test]
fn test_key_not_found() {
    let err = key_not_found("missing");
    assert!(err.message.contains("missing"));
}

#[test]
fn test_cannot_index() {
    let err = cannot_index("int", "string");
    assert!(err.message.contains("int"));
    assert!(err.message.contains("string"));
}

#[test]
fn test_cannot_get_length() {
    let err = cannot_get_length("int");
    assert!(err.message.contains("int"));
    assert!(err.message.contains("length"));
}

#[test]
fn test_no_field_on_struct() {
    let err = no_field_on_struct("missing");
    assert!(err.message.contains("missing"));
}

#[test]
fn test_invalid_tuple_field() {
    let err = invalid_tuple_field("abc");
    assert!(err.message.contains("abc"));
}

#[test]
fn test_tuple_index_out_of_bounds() {
    let err = tuple_index_out_of_bounds(5);
    assert!(err.message.contains("5"));
}

#[test]
fn test_cannot_access_field() {
    let err = cannot_access_field("int");
    assert!(err.message.contains("int"));
}

#[test]
fn test_no_member_in_module() {
    let err = no_member_in_module("foo");
    assert!(err.message.contains("foo"));
    assert!(err.message.contains("module"));
}

// -- Type Conversion Errors --

#[test]
fn test_range_bound_not_int() {
    let err = range_bound_not_int("start");
    assert!(err.message.contains("start"));
    assert!(err.message.contains("integer"));
}

#[test]
fn test_unbounded_range_end() {
    let err = unbounded_range_end();
    assert!(err.message.contains("unbounded"));
}

#[test]
fn test_map_key_not_hashable() {
    let err = map_key_not_hashable();
    assert!(err.message.contains("hashable"));
}

// -- Control Flow Errors --

#[test]
fn test_non_exhaustive_match() {
    let err = non_exhaustive_match();
    assert!(err.message.contains("non-exhaustive"));
}

#[test]
fn test_cannot_assign_immutable() {
    let err = cannot_assign_immutable("x");
    assert!(err.message.contains("immutable"));
    assert!(err.message.contains("x"));
}

#[test]
fn test_invalid_assignment_target() {
    let err = invalid_assignment_target();
    assert!(err.message.contains("assignment"));
}

#[test]
fn test_for_requires_iterable() {
    let err = for_requires_iterable();
    assert!(err.message.contains("iterable"));
}

// -- Pattern Binding Errors --

#[test]
fn test_tuple_pattern_mismatch() {
    let err = tuple_pattern_mismatch();
    assert!(err.message.contains("tuple"));
    assert!(err.message.contains("mismatch"));
}

#[test]
fn test_expected_tuple() {
    let err = expected_tuple();
    assert!(err.message.contains("tuple"));
}

#[test]
fn test_expected_struct() {
    let err = expected_struct();
    assert!(err.message.contains("struct"));
}

#[test]
fn test_expected_list() {
    let err = expected_list();
    assert!(err.message.contains("list"));
}

#[test]
fn test_list_pattern_too_long() {
    let err = list_pattern_too_long();
    assert!(err.message.contains("list"));
    assert!(err.message.contains("too long"));
}

#[test]
fn test_missing_struct_field() {
    let err = missing_struct_field();
    assert!(err.message.contains("struct"));
    assert!(err.message.contains("field"));
}

// -- Miscellaneous Errors --

#[test]
fn test_self_outside_method() {
    let err = self_outside_method();
    assert!(err.message.contains("self"));
}

#[test]
fn test_parse_error() {
    let err = parse_error();
    assert!(err.message.contains("parse"));
}

#[test]
fn test_hash_outside_index() {
    let err = hash_outside_index();
    assert!(err.message.contains("#"));
}

#[test]
fn test_await_not_supported() {
    let err = await_not_supported();
    assert!(err.message.contains("await"));
}

#[test]
fn test_invalid_literal_pattern() {
    let err = invalid_literal_pattern();
    assert!(err.message.contains("literal"));
}

// -- Collection Method Errors --

#[test]
fn test_map_requires_collection() {
    let err = map_requires_collection();
    assert!(err.message.contains("map"));
    assert!(err.message.contains("collection"));
}

#[test]
fn test_filter_requires_collection() {
    let err = filter_requires_collection();
    assert!(err.message.contains("filter"));
    assert!(err.message.contains("collection"));
}

#[test]
fn test_fold_requires_collection() {
    let err = fold_requires_collection();
    assert!(err.message.contains("fold"));
    assert!(err.message.contains("collection"));
}

#[test]
fn test_find_requires_list() {
    let err = find_requires_list();
    assert!(err.message.contains("find"));
    assert!(err.message.contains("list"));
}

#[test]
fn test_collect_requires_range() {
    let err = collect_requires_range();
    assert!(err.message.contains("collect"));
    assert!(err.message.contains("range"));
}

#[test]
fn test_any_requires_list() {
    let err = any_requires_list();
    assert!(err.message.contains("any"));
    assert!(err.message.contains("list"));
}

#[test]
fn test_all_requires_list() {
    let err = all_requires_list();
    assert!(err.message.contains("all"));
    assert!(err.message.contains("list"));
}

#[test]
fn test_map_entries_requires_map() {
    let err = map_entries_requires_map();
    assert!(err.message.contains("map"));
}

#[test]
fn test_filter_entries_requires_map() {
    let err = filter_entries_requires_map();
    assert!(err.message.contains("filter"));
    assert!(err.message.contains("map"));
}

// -- Not Implemented Errors --

#[test]
fn test_map_entries_not_implemented() {
    let err = map_entries_not_implemented();
    assert!(err.message.contains("not yet"));
}

#[test]
fn test_filter_entries_not_implemented() {
    let err = filter_entries_not_implemented();
    assert!(err.message.contains("not yet"));
}

#[test]
fn test_index_assignment_not_implemented() {
    let err = index_assignment_not_implemented();
    assert!(err.message.contains("not yet"));
}

#[test]
fn test_field_assignment_not_implemented() {
    let err = field_assignment_not_implemented();
    assert!(err.message.contains("not yet"));
}

#[test]
fn test_default_requires_type_context() {
    let err = default_requires_type_context();
    assert!(err.message.contains("default"));
    assert!(err.message.contains("type"));
}

// -- Index Context Errors --

#[test]
fn test_operator_not_supported_in_index() {
    let err = operator_not_supported_in_index();
    assert!(err.message.contains("operator"));
    assert!(err.message.contains("index"));
}

#[test]
fn test_non_integer_in_index() {
    let err = non_integer_in_index();
    assert!(err.message.contains("integer"));
    assert!(err.message.contains("index"));
}

#[test]
fn test_collection_too_large() {
    let err = collection_too_large();
    assert!(err.message.contains("collection"));
    assert!(err.message.contains("large"));
}

// -- Pattern Errors --

#[test]
fn test_unknown_pattern() {
    let err = unknown_pattern("weird");
    assert!(err.message.contains("weird"));
}

#[test]
fn test_for_pattern_requires_list() {
    let err = for_pattern_requires_list("int");
    assert!(err.message.contains("for"));
    assert!(err.message.contains("list"));
    assert!(err.message.contains("int"));
}

// -- Propagation Helpers --

#[test]
fn test_propagated_error_message() {
    let msg = propagated_error_message(&Value::int(42));
    assert!(msg.contains("propagated"));
    assert!(msg.contains("42"));
}

// -- Test that errors are distinct --

#[test]
fn test_errors_are_distinct() {
    let errors = vec![
        division_by_zero().message,
        modulo_by_zero().message,
        non_exhaustive_match().message,
        invalid_assignment_target().message,
        for_requires_iterable().message,
        tuple_pattern_mismatch().message,
        expected_tuple().message,
        expected_struct().message,
        expected_list().message,
        list_pattern_too_long().message,
        missing_struct_field().message,
        self_outside_method().message,
        no_member_in_module("test").message,
        parse_error().message,
        hash_outside_index().message,
        await_not_supported().message,
        invalid_literal_pattern().message,
        map_requires_collection().message,
        filter_requires_collection().message,
        fold_requires_collection().message,
        find_requires_list().message,
        collect_requires_range().message,
        any_requires_list().message,
        all_requires_list().message,
        unbounded_range_end().message,
        map_key_not_hashable().message,
    ];

    // Ensure all messages are unique
    let mut seen = std::collections::HashSet::new();
    for msg in &errors {
        assert!(seen.insert(msg.clone()), "Duplicate error message: {}", msg);
    }
}
