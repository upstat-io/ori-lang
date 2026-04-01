//! Struct AOT Tests
//!
//! Tests for struct construction, field access, update syntax, nested structs,
//! structs as function parameters/returns, struct with various field types,
//! and struct interaction with closures and control flow.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Basic construction & field access ───

#[test]
fn test_struct_two_fields() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_two_fields.ori"),
        "struct_two_fields",
    );
}

#[test]
fn test_struct_three_fields() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_three_fields.ori"),
        "struct_three_fields",
    );
}

#[test]
fn test_struct_single_field() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_single_field.ori"),
        "struct_single_field",
    );
}

#[test]
fn test_struct_bool_fields() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_bool_fields.ori"),
        "struct_bool_fields",
    );
}

#[test]
fn test_struct_mixed_fields() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_mixed_fields.ori"),
        "struct_mixed_fields",
    );
}

// ─── String fields ───

#[test]
fn test_struct_string_field() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_string_field.ori"),
        "struct_str_field",
    );
}

#[test]
fn test_struct_two_string_fields() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_two_string_fields.ori"),
        "struct_two_strs",
    );
}

#[test]
fn test_struct_string_field_method() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_string_field_method.ori"),
        "struct_str_method",
    );
}

// ─── Update syntax ───

#[test]
fn test_struct_update_one_field() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_update_one_field.ori"),
        "struct_update_one",
    );
}

#[test]
fn test_struct_update_all_fields() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_update_all_fields.ori"),
        "struct_update_all",
    );
}

#[test]
fn test_struct_update_preserves_original() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_update_preserves_original.ori"),
        "struct_update_preserves",
    );
}

#[test]
fn test_struct_update_chain() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_update_chain.ori"),
        "struct_update_chain",
    );
}

#[test]
fn test_struct_update_with_string() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_update_with_string.ori"),
        "struct_update_str",
    );
}

// ─── Nested structs ───

#[test]
fn test_struct_nested_basic() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_nested_basic.ori"),
        "struct_nested_basic",
    );
}

#[test]
fn test_struct_nested_three_levels() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_nested_three_levels.ori"),
        "struct_nested_3",
    );
}

#[test]
fn test_struct_nested_with_string() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_nested_with_string.ori"),
        "struct_nested_str",
    );
}

// ─── Struct as function param/return ───

#[test]
fn test_struct_as_param() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_as_param.ori"),
        "struct_as_param",
    );
}

#[test]
fn test_struct_as_return() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_as_return.ori"),
        "struct_as_return",
    );
}

#[test]
fn test_struct_param_and_return() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_param_and_return.ori"),
        "struct_param_return",
    );
}

#[test]
fn test_struct_multiple_params() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_multiple_params.ori"),
        "struct_multi_param",
    );
}

// ─── Struct in control flow ───

#[test]
fn test_struct_from_if() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_from_if.ori"),
        "struct_from_if",
    );
}

#[test]
fn test_struct_in_loop() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_in_loop.ori"),
        "struct_in_loop",
    );
}

// ─── Struct with closures ───

#[test]
fn test_struct_closure_field_access() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_closure_field_access.ori"),
        "struct_closure_field",
    );
}

#[test]
fn test_struct_returned_from_closure() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_returned_from_closure.ori"),
        "struct_from_closure",
    );
}

// ─── Struct equality ───

#[test]
fn test_struct_derived_eq() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_derived_eq.ori"),
        "struct_derived_eq",
    );
}

#[test]
fn test_struct_derived_eq_string() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_derived_eq_string.ori"),
        "struct_eq_string",
    );
}

// ─── Multiple struct types ───

#[test]
fn test_struct_multiple_types() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_multiple_types.ori"),
        "struct_multi_types",
    );
}

// ─── Struct with list field ───

#[test]
fn test_struct_list_field() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_list_field.ori"),
        "struct_list_field",
    );
}

// ─── Struct field computation ───

#[test]
fn test_struct_computed_fields() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_computed_fields.ori"),
        "struct_computed",
    );
}

#[test]
fn test_struct_field_from_function() {
    assert_aot_success(
        include_str!("fixtures/structs/struct_field_from_function.ori"),
        "struct_field_from_fn",
    );
}
