//! Tuple AOT Tests
//!
//! Tests for tuple construction, field access, destructuring, nested tuples,
//! tuples as function parameters/returns, tuples in collections, and
//! tuple interaction with other language features.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Construction ───

#[test]
fn test_tuple_pair_int() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_pair_int.ori"),
        "tuple_pair_int",
    );
}

#[test]
fn test_tuple_triple_int() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_triple_int.ori"),
        "tuple_triple_int",
    );
}

#[test]
fn test_tuple_mixed_types() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_mixed_types.ori"),
        "tuple_mixed_types",
    );
}

#[test]
fn test_tuple_single_element() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_single_element.ori"),
        "tuple_single",
    );
}

#[test]
fn test_tuple_bool_pair() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_bool_pair.ori"),
        "tuple_bool_pair",
    );
}

// ─── Destructuring ───

#[test]
fn test_tuple_destructure_pair() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_destructure_pair.ori"),
        "tuple_destr_pair",
    );
}

#[test]
fn test_tuple_destructure_triple() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_destructure_triple.ori"),
        "tuple_destr_triple",
    );
}

#[test]
fn test_tuple_destructure_mixed() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_destructure_mixed.ori"),
        "tuple_destr_mixed",
    );
}

#[test]
fn test_tuple_destructure_from_variable() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_destructure_from_variable.ori"),
        "tuple_destr_from_var",
    );
}

// ─── Field access ───

#[test]
fn test_tuple_field_access_4() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_field_access_4.ori"),
        "tuple_field_4",
    );
}

#[test]
fn test_tuple_field_in_expression() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_field_in_expression.ori"),
        "tuple_field_expr",
    );
}

#[test]
fn test_tuple_field_as_function_arg() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_field_as_function_arg.ori"),
        "tuple_field_fn_arg",
    );
}

// ─── As function parameter/return ───

#[test]
fn test_tuple_as_param() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_as_param.ori"),
        "tuple_as_param",
    );
}

#[test]
fn test_tuple_as_return() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_as_return.ori"),
        "tuple_as_return",
    );
}

#[test]
fn test_tuple_return_and_access() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_return_and_access.ori"),
        "tuple_return_access",
    );
}

#[test]
fn test_tuple_return_triple() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_return_triple.ori"),
        "tuple_return_triple",
    );
}

// ─── Nested tuples ───

#[test]
#[ignore = "Parser gap: chained tuple field access t.0.0 lexed as float (Section 05 § 5.7 open item)"]
fn test_tuple_nested_pair_of_pairs() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_nested_pair_of_pairs.ori"),
        "tuple_nested_pairs",
    );
}

#[test]
fn test_tuple_nested_destructure() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_nested_destructure.ori"),
        "tuple_nested_destr",
    );
}

#[test]
#[ignore = "Parser gap: chained tuple field access t.1.0 lexed as float (Section 05 § 5.7 open item)"]
fn test_tuple_nested_mixed() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_nested_mixed.ori"),
        "tuple_nested_mixed",
    );
}

// ─── Tuples with strings ───

#[test]
fn test_tuple_string_field() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_string_field.ori"),
        "tuple_str_field",
    );
}

#[test]
fn test_tuple_two_strings() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_two_strings.ori"),
        "tuple_two_strs",
    );
}

// ─── Tuples in control flow ───

#[test]
fn test_tuple_from_if_expression() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_from_if_expression.ori"),
        "tuple_from_if",
    );
}

#[test]
fn test_tuple_in_loop() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_in_loop.ori"),
        "tuple_in_loop",
    );
}

#[test]
fn test_tuple_destructure_in_loop() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_destructure_in_loop.ori"),
        "tuple_destr_loop",
    );
}

// ─── Tuple with closures ───

#[test]
fn test_tuple_closure_capture() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_closure_capture.ori"),
        "tuple_closure_capture",
    );
}

#[test]
fn test_tuple_returned_from_closure() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_returned_from_closure.ori"),
        "tuple_from_closure",
    );
}

// ─── Tuple comparison ───

#[test]
fn test_tuple_equality() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_equality.ori"),
        "tuple_equality",
    );
}

#[test]
fn test_tuple_equality_triple() {
    assert_aot_success(
        include_str!("fixtures/tuples/tuple_equality_triple.ori"),
        "tuple_eq_triple",
    );
}
