//! Scale & Stress AOT Tests
//!
//! Tests that push the AOT pipeline on **scale**: large collections, many fields,
//! deep nesting, and high allocation counts. These stress the ARC pipeline,
//! LLVM codegen, and runtime memory management at volumes that basic tests
//! don't reach.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Large collection construction via for-yield ───

#[test]
fn test_stress_large_list_100_elements() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_large_list_100_elements.ori"),
        "stress_large_list_100",
    );
}

#[test]
fn test_stress_large_list_500_elements() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_large_list_500_elements.ori"),
        "stress_large_list_500",
    );
}

#[test]
fn test_stress_large_list_filter_collect() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_large_list_filter_collect.ori"),
        "stress_large_list_filter",
    );
}

// ─── Struct with many fields ───

#[test]
fn test_stress_struct_8_fields() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_struct_8_fields.ori"),
        "stress_struct_8_fields",
    );
}

#[test]
fn test_stress_struct_mixed_field_types() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_struct_mixed_field_types.ori"),
        "stress_struct_mixed_types",
    );
}

#[test]
fn test_stress_struct_update_many_fields() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_struct_update_many_fields.ori"),
        "stress_struct_update_many",
    );
}

// ─── Deep struct nesting ───

#[test]
fn test_stress_nested_structs_4_levels() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_nested_structs_4_levels.ori"),
        "stress_nested_4_levels",
    );
}

#[test]
fn test_stress_nested_struct_with_strings_at_every_level() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_nested_struct_with_strings_at_every_level.ori"),
        "stress_nested_strings_all_levels",
    );
}

// ─── Large tuples ───

#[test]
fn test_stress_tuple_5_elements() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_tuple_5_elements.ori"),
        "stress_tuple_5",
    );
}

#[test]
fn test_stress_tuple_6_elements_field_access() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_tuple_6_elements_field_access.ori"),
        "stress_tuple_6_access",
    );
}

#[test]
fn test_stress_tuple_mixed_types() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_tuple_mixed_types.ori"),
        "stress_tuple_mixed",
    );
}

#[test]
fn test_stress_tuple_from_function() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_tuple_from_function.ori"),
        "stress_tuple_5_from_fn",
    );
}

// ─── ARC allocation stress ───

#[test]
fn test_stress_arc_1000_struct_allocations() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_arc_1000_struct_allocations.ori"),
        "stress_arc_1000_allocs",
    );
}

#[test]
fn test_stress_arc_1000_string_struct_allocations() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_arc_1000_string_struct_allocations.ori"),
        "stress_arc_1000_string_allocs",
    );
}

#[test]
fn test_stress_arc_nested_struct_in_loop() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_arc_nested_struct_in_loop.ori"),
        "stress_arc_nested_in_loop",
    );
}

// ─── Deep recursion with struct parameters ───

#[test]
fn test_stress_deep_recursion_200_levels() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_deep_recursion_200_levels.ori"),
        "stress_deep_recursion_200",
    );
}

#[test]
fn test_stress_deep_recursion_500_levels() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_deep_recursion_500_levels.ori"),
        "stress_deep_recursion_500",
    );
}

// ─── String concatenation stress ───

#[test]
fn test_stress_string_concat_100() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_string_concat_100.ori"),
        "stress_string_concat_100",
    );
}

#[test]
fn test_stress_string_concat_varied() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_string_concat_varied.ori"),
        "stress_string_concat_varied",
    );
}

// Semantic pin: capacity must not grow exponentially in repeated concat loops.
// Before fix, 50 iterations produced 52TB allocations; now max ~210 bytes.
#[test]
fn test_stress_string_concat_loop_100() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_string_concat_loop_100.ori"),
        "stress_string_concat_loop_100",
    );
}

#[test]
fn test_stress_string_concat_two_per_iter() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_string_concat_two_per_iter.ori"),
        "stress_string_concat_two_per_iter",
    );
}

#[test]
fn test_stress_string_concat_three_per_iter() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_string_concat_three_per_iter.ori"),
        "stress_string_concat_three_per_iter",
    );
}

#[test]
fn test_stress_string_concat_with_interpolation() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_string_concat_with_interpolation.ori"),
        "stress_string_concat_with_interpolation",
    );
}

#[test]
fn test_stress_string_concat_empty_operands() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_string_concat_empty_operands.ori"),
        "stress_string_concat_empty_operands",
    );
}

#[test]
fn test_stress_string_concat_sso_to_heap() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_string_concat_sso_to_heap.ori"),
        "stress_string_concat_sso_to_heap",
    );
}

// ─── List of structs ───

#[test]
fn test_stress_list_of_structs_iteration() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_list_of_structs_iteration.ori"),
        "stress_list_of_structs",
    );
}

#[test]
fn test_stress_list_of_strings() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_list_of_strings.ori"),
        "stress_list_of_strings",
    );
}

// ─── Multiple block scopes with allocations ───

#[test]
fn test_stress_many_block_scopes() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_many_block_scopes.ori"),
        "stress_many_block_scopes",
    );
}

// ─── Multiple function calls in loop ───

#[test]
fn test_stress_many_function_calls() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_many_function_calls.ori"),
        "stress_many_function_calls",
    );
}

// ─── Shared ownership stress ───

#[test]
fn test_stress_shared_struct_many_refs() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_shared_struct_many_refs.ori"),
        "stress_shared_many_refs",
    );
}

// ─── Iterator pipeline stress ───

#[test]
fn test_stress_iterator_long_chain() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_iterator_long_chain.ori"),
        "stress_iterator_long_chain",
    );
}

#[test]
fn test_stress_iterator_fold_large() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_iterator_fold_large.ori"),
        "stress_iterator_fold_large",
    );
}

// ─── Struct passed through function chain ───

#[test]
fn test_stress_struct_function_chain() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_struct_function_chain.ori"),
        "stress_struct_fn_chain",
    );
}

// ─── Combined: structs + closures + iteration ───

#[test]
fn test_stress_combined_struct_closure_iteration() {
    assert_aot_success(
        include_str!("fixtures/stress/stress_combined_struct_closure_iteration.ori"),
        "stress_combined_struct_closure_iter",
    );
}
