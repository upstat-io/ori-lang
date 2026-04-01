//! AOT Spec Conformance Tests
//!
//! End-to-end tests that compile Ori programs through the full AOT pipeline
//! (compile → link → execute) and verify correct behavior.
//!
//! These tests mirror patterns from `tests/spec/` but run through AOT instead
//! of the interpreter or JIT backends.
//!
//! These tests can run in parallel - each test uses unique temp files via
//! atomic counters, and the AOT compiler uses `tempfile::TempDir` for
//! intermediate object files.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_capture};

#[test]
fn test_aot_let_binding_basic() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_let_binding_basic.ori"),
        "let_binding_basic",
    );
}

#[test]
fn test_aot_let_binding_annotated() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_let_binding_annotated.ori"),
        "let_binding_annotated",
    );
}

#[test]
fn test_aot_let_shadowing() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_let_shadowing.ori"),
        "let_shadowing",
    );
}

#[test]
fn test_aot_if_then_else() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_if_then_else.ori"),
        "if_then_else",
    );
}

#[test]
fn test_aot_nested_conditionals() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_nested_conditionals.ori"),
        "nested_conditionals",
    );
}

#[test]
fn test_aot_comparison_conditions() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_comparison_conditions.ori"),
        "comparison_conditions",
    );
}

#[test]
fn test_aot_arithmetic_add_sub() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_arithmetic_add_sub.ori"),
        "arithmetic_add_sub",
    );
}

#[test]
fn test_aot_arithmetic_mul_div() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_arithmetic_mul_div.ori"),
        "arithmetic_mul_div",
    );
}

#[test]
fn test_aot_arithmetic_modulo() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_arithmetic_modulo.ori"),
        "arithmetic_modulo",
    );
}

#[test]
fn test_aot_arithmetic_negation() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_arithmetic_negation.ori"),
        "arithmetic_negation",
    );
}

#[test]
fn test_aot_arithmetic_precedence() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_arithmetic_precedence.ori"),
        "arithmetic_precedence",
    );
}

#[test]
fn test_aot_boolean_and() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_boolean_and.ori"),
        "boolean_and",
    );
}

#[test]
fn test_aot_boolean_or() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_boolean_or.ori"),
        "boolean_or",
    );
}

#[test]
fn test_aot_boolean_not() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_boolean_not.ori"),
        "boolean_not",
    );
}

#[test]
fn test_aot_function_call() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_function_call.ori"),
        "function_call",
    );
}

#[test]
fn test_aot_function_multiple_params() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_function_multiple_params.ori"),
        "function_multiple_params",
    );
}

#[test]
fn test_aot_function_recursion() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_function_recursion.ori"),
        "function_recursion",
    );
}

#[test]
fn test_aot_function_nested_calls() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_function_nested_calls.ori"),
        "function_nested_calls",
    );
}

#[test]
fn test_aot_comparison_equality() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_comparison_equality.ori"),
        "comparison_equality",
    );
}

#[test]
fn test_aot_comparison_ordering() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_comparison_ordering.ori"),
        "comparison_ordering",
    );
}

#[test]
fn test_aot_print_string() {
    let source = r#"@main () -> void = print(msg: "Hello AOT!");"#;
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(exit_code, 0, "print_string failed: {stderr}");
    assert!(
        stdout.contains("Hello AOT!"),
        "Expected output to contain 'Hello AOT!', got stdout: '{stdout}', stderr: '{stderr}'"
    );
}

#[test]
fn test_aot_complex_expression() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_complex_expression.ori"),
        "complex_expression",
    );
}

#[test]
fn test_aot_fibonacci() {
    assert_aot_success(include_str!("fixtures/spec/aot_fibonacci.ori"), "fibonacci");
}

// Duration and Size Literals

#[test]
fn test_aot_duration_literals() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_duration_literals.ori"),
        "duration_literals",
    );
}

#[test]
fn test_aot_duration_negative() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_duration_negative.ori"),
        "duration_negative",
    );
}

#[test]
fn test_aot_size_literals() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_size_literals.ori"),
        "size_literals",
    );
}

// Duration and Size Arithmetic

#[test]
fn test_aot_duration_arithmetic() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_duration_arithmetic.ori"),
        "duration_arithmetic",
    );
}

#[test]
fn test_aot_duration_comparison() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_duration_comparison.ori"),
        "duration_comparison",
    );
}

#[test]
fn test_aot_size_arithmetic() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_size_arithmetic.ori"),
        "size_arithmetic",
    );
}

#[test]
fn test_aot_size_comparison() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_size_comparison.ori"),
        "size_comparison",
    );
}

// Float Primitives

#[test]
fn test_aot_float_literals() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_float_literals.ori"),
        "float_literals",
    );
}

#[test]
fn test_aot_float_arithmetic() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_float_arithmetic.ori"),
        "float_arithmetic",
    );
}

#[test]
fn test_aot_float_comparison() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_float_comparison.ori"),
        "float_comparison",
    );
}

#[test]
fn test_aot_float_negation() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_float_negation.ori"),
        "float_negation",
    );
}

// Char Primitives

#[test]
fn test_aot_char_literals() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_char_literals.ori"),
        "char_literals",
    );
}

#[test]
fn test_aot_char_comparison() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_char_comparison.ori"),
        "char_comparison",
    );
}

// Byte Primitives

#[test]
fn test_aot_byte_basics() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_byte_basics.ori"),
        "byte_basics",
    );
}

// Never Type Coercion

#[test]
fn test_aot_never_panic_coercion() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_never_panic_coercion.ori"),
        "never_panic_coercion",
    );
}

#[test]
fn test_aot_never_conditional_branches() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_never_conditional_branches.ori"),
        "never_conditional_branches",
    );
}

// Loop, Break, Continue — Never Type Coercion

#[test]
fn test_aot_loop_break_value() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_loop_break_value.ori"),
        "loop_break_value",
    );
}

#[test]
fn test_aot_loop_conditional_break() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_loop_conditional_break.ori"),
        "loop_conditional_break",
    );
}

#[test]
fn test_aot_loop_break_never_coercion() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_loop_break_never_coercion.ori"),
        "loop_break_never_coercion",
    );
}

#[test]
fn test_aot_loop_continue_never_coercion() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_loop_continue_never_coercion.ori"),
        "loop_continue_never_coercion",
    );
}

#[test]
fn test_aot_loop_break_and_continue_combined() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_loop_break_and_continue_combined.ori"),
        "loop_break_and_continue_combined",
    );
}

// Result/Option Constructors and ? Operator

#[test]
fn test_aot_result_ok_unwrap() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_result_ok_unwrap.ori"),
        "result_ok_unwrap",
    );
}

#[test]
fn test_aot_result_err_check() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_result_err_check.ori"),
        "result_err_check",
    );
}

/// C4 regression: Option match tag inversion — switch labels must match construction tags.
/// Construction: Some=tag 0, None=tag 1. Match must use the same mapping.
#[test]
fn test_aot_option_match_tag_correctness() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_option_match_tag_correctness.ori"),
        "option_match_tag_correctness",
    );
}

/// C4 regression: match on Option inside if/else producing Option values.
#[test]
fn test_aot_option_match_with_construction() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_option_match_with_construction.ori"),
        "option_match_with_construction",
    );
}

#[test]
fn test_aot_option_some_unwrap() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_option_some_unwrap.ori"),
        "option_some_unwrap",
    );
}

#[test]
fn test_aot_option_none_check() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_option_none_check.ori"),
        "option_none_check",
    );
}

#[test]
fn test_aot_try_result_ok_unwraps() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_try_result_ok_unwraps.ori"),
        "try_result_ok_unwraps",
    );
}

#[test]
fn test_aot_try_result_err_propagates() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_try_result_err_propagates.ori"),
        "try_result_err_propagates",
    );
}

#[test]
fn test_aot_try_option_some_unwraps() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_try_option_some_unwraps.ori"),
        "try_option_some_unwraps",
    );
}

#[test]
fn test_aot_try_option_none_propagates() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_try_option_none_propagates.ori"),
        "try_option_none_propagates",
    );
}

#[test]
fn test_aot_try_chained_result() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_try_chained_result.ori"),
        "try_chained_result",
    );
}

#[test]
fn test_aot_try_chained_first_fails() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_try_chained_first_fails.ori"),
        "try_chained_first_fails",
    );
}

// String Escape Sequences

#[test]
fn test_aot_string_escape_tab() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_string_escape_tab.ori"),
        "string_escape_tab",
    );
}

#[test]
fn test_aot_string_escape_backslash() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_string_escape_backslash.ori"),
        "string_escape_backslash",
    );
}

#[test]
fn test_aot_string_escape_quote() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_string_escape_quote.ori"),
        "string_escape_quote",
    );
}

#[test]
fn test_aot_string_escape_newline() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_string_escape_newline.ori"),
        "string_escape_newline",
    );
}

// Unit / Void

#[test]
fn test_aot_unit_return() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_unit_return.ori"),
        "unit_return",
    );
}

#[test]
fn test_aot_unit_in_conditional() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_unit_in_conditional.ori"),
        "unit_in_conditional",
    );
}

// Match Expressions

#[test]
fn test_aot_match_int_literal() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_match_int_literal.ori"),
        "match_int_literal",
    );
}

#[test]
fn test_aot_match_wildcard() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_match_wildcard.ori"),
        "match_wildcard",
    );
}

#[test]
fn test_aot_match_nested_with_if() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_match_nested_with_if.ori"),
        "match_nested_with_if",
    );
}

#[test]
fn test_aot_match_bool() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_match_bool.ori"),
        "match_bool",
    );
}

#[test]
fn test_aot_match_expression_valued() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_match_expression_valued.ori"),
        "match_expression_valued",
    );
}

// Mutual Recursion

#[test]
fn test_aot_mutual_recursion() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_mutual_recursion.ori"),
        "mutual_recursion",
    );
}

#[test]
fn test_aot_mutual_recursion_deeper() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_mutual_recursion_deeper.ori"),
        "mutual_recursion_deeper",
    );
}

// Nested Control Flow

#[test]
fn test_aot_nested_match_in_if() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_nested_match_in_if.ori"),
        "nested_match_in_if",
    );
}

#[test]
fn test_aot_nested_if_in_match() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_nested_if_in_match.ori"),
        "nested_if_in_match",
    );
}

#[test]
fn test_aot_loop_with_match() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_loop_with_match.ori"),
        "loop_with_match",
    );
}

// Deep Recursion (stress)

#[test]
fn test_aot_deep_recursion() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_deep_recursion.ori"),
        "deep_recursion",
    );
}

// =========================================================================
// Match: char patterns
// =========================================================================

#[test]
fn test_aot_match_char() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_match_char.ori"),
        "match_char",
    );
}

// =========================================================================
// Bitwise operators
// =========================================================================

#[test]
fn test_aot_bitwise_and_or_xor() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_bitwise_and_or_xor.ori"),
        "bitwise_and_or_xor",
    );
}

#[test]
fn test_aot_bitwise_shift() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_bitwise_shift.ori"),
        "bitwise_shift",
    );
}

#[test]
fn test_aot_bitwise_not() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_bitwise_not.ori"),
        "bitwise_not",
    );
}

// =========================================================================
// String operations
// =========================================================================

#[test]
fn test_aot_string_equality() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_string_equality.ori"),
        "string_equality",
    );
}

#[test]
fn test_aot_string_length() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_string_length.ori"),
        "string_length",
    );
}

#[test]
fn test_aot_string_concat() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_string_concat.ori"),
        "string_concat",
    );
}

// =========================================================================
// Tuples
// =========================================================================

#[test]
fn test_aot_tuple_construction_destructure() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_tuple_construction_destructure.ori"),
        "tuple_construct_destruct",
    );
}

#[test]
fn test_aot_tuple_field_access() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_tuple_field_access.ori"),
        "tuple_field_access",
    );
}

#[test]
fn test_aot_tuple_from_function() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_tuple_from_function.ori"),
        "tuple_from_function",
    );
}

// =========================================================================
// Structs
// =========================================================================

#[test]
fn test_aot_struct_construction() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_struct_construction.ori"),
        "struct_construction",
    );
}

#[test]
fn test_aot_struct_update() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_struct_update.ori"),
        "struct_update",
    );
}

#[test]
fn test_aot_struct_as_parameter() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_struct_as_parameter.ori"),
        "struct_as_parameter",
    );
}

// =========================================================================
// Closures and higher-order functions
// =========================================================================

#[test]
fn test_aot_closure_capture() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_closure_capture.ori"),
        "closure_capture",
    );
}

#[test]
fn test_aot_higher_order_function() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_higher_order_function.ori"),
        "higher_order_function",
    );
}

#[test]
fn test_aot_function_returning_closure() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_function_returning_closure.ori"),
        "function_returning_closure",
    );
}

#[test]
fn test_aot_closure_composition() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_closure_composition.ori"),
        "closure_composition",
    );
}

// =========================================================================
// For-in loops
// =========================================================================

#[test]
fn test_aot_for_in_range() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_for_in_range.ori"),
        "for_in_range",
    );
}

#[test]
fn test_aot_for_in_list() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_for_in_list.ori"),
        "for_in_list",
    );
}

// =========================================================================
// Collections: list
// =========================================================================

#[test]
fn test_aot_list_literal_length() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_list_literal_length.ori"),
        "list_literal_length",
    );
}

#[test]
fn test_aot_list_map_collect() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_list_map_collect.ori"),
        "list_map_collect",
    );
}

// Enum variant constructors

#[test]
fn test_aot_enum_construction() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_enum_construction.ori"),
        "enum_construction",
    );
}

#[test]
fn test_aot_enum_unit_variants() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_enum_unit_variants.ori"),
        "enum_unit_variants",
    );
}

#[test]
fn test_aot_enum_mixed_variants() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_enum_mixed_variants.ori"),
        "enum_mixed_variants",
    );
}

#[test]
fn test_aot_enum_as_param_and_return() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_enum_as_param_and_return.ori"),
        "enum_param_return",
    );
}

// =========================================================================
// Known AOT gaps (ignored until codegen supports them)
// =========================================================================

#[test]
fn test_aot_derive_eq_struct() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_derive_eq_struct.ori"),
        "derive_eq_struct",
    );
}

#[test]
fn test_aot_recursive_enum_tree() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_recursive_enum_tree.ori"),
        "recursive_enum_tree",
    );
}

/// Deeper recursive enum: 3 levels of nesting.
#[test]
fn test_aot_recursive_enum_tree_deep() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_recursive_enum_tree_deep.ori"),
        "recursive_enum_tree_deep",
    );
}

/// Recursive enum with a single-field variant (linked list).
#[test]
fn test_aot_recursive_enum_linked_list() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_recursive_enum_linked_list.ori"),
        "recursive_enum_linked_list",
    );
}

#[test]
fn test_aot_derive_eq_enum() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_derive_eq_enum.ori"),
        "derive_eq_enum",
    );
}

#[test]
fn test_aot_derive_eq_enum_all_variants() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_derive_eq_enum_all_variants.ori"),
        "derive_eq_enum_all",
    );
}

#[test]
fn test_aot_list_index() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_list_index.ori"),
        "list_index",
    );
}

#[test]
fn test_aot_string_interpolation() {
    assert_aot_success(
        r#"
@main () -> int = {
    let name = "world";
    let greeting = `hello {name}`;
    if greeting == "hello world" then 0 else 1
}
"#,
        "string_interpolation",
    );
}

// While-like loops (loop + conditional break)

#[test]
fn test_aot_while_pattern_basic() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_while_pattern_basic.ori"),
        "while_pattern_basic",
    );
}

#[test]
fn test_aot_while_pattern_with_accumulator() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_while_pattern_with_accumulator.ori"),
        "while_pattern_accumulator",
    );
}

// catch(expr:) panic recovery

#[test]
fn test_aot_catch_success() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_catch_success.ori"),
        "catch_success",
    );
}

#[test]
fn test_aot_catch_panic() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_catch_panic.ori"),
        "catch_panic",
    );
}

#[test]
#[ignore = "AOT gap: inline panic in catch — invoke only intercepts callee-function panics, not same-function inline code"]
fn test_aot_catch_div_by_zero() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_catch_div_by_zero.ori"),
        "catch_div_by_zero",
    );
}

// Generic functions

#[test]
fn test_aot_generic_identity() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_generic_identity.ori"),
        "generic_identity",
    );
}

#[test]
fn test_aot_generic_pair() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_generic_pair.ori"),
        "generic_pair",
    );
}

#[test]
fn test_aot_generic_three_type_params() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_generic_three_type_params.ori"),
        "generic_three_params",
    );
}

#[test]
fn test_aot_generic_calling_non_generic() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_generic_calling_non_generic.ori"),
        "generic_calling_non_generic",
    );
}

#[test]
fn test_aot_generic_two_specializations() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_generic_two_specializations.ori"),
        "generic_two_specializations",
    );
}

// Map collection operations

#[test]
fn test_aot_map_literal_length() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_map_literal_length.ori"),
        "map_literal_length",
    );
}

#[test]
fn test_aot_map_is_empty() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_map_is_empty.ori"),
        "map_is_empty",
    );
}

// List operations

#[test]
fn test_aot_list_push() {
    assert_aot_success(include_str!("fixtures/spec/aot_list_push.ori"), "list_push");
}

#[test]
fn test_aot_list_concat() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_list_concat.ori"),
        "list_concat",
    );
}

#[test]
fn test_aot_list_first_last() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_list_first_last.ori"),
        "list_first_last",
    );
}

#[test]
fn test_aot_list_empty_operations() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_list_empty_operations.ori"),
        "list_empty_operations",
    );
}

// Struct with RC fields (ARC stress)

#[test]
fn test_aot_struct_with_list_field() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_struct_with_list_field.ori"),
        "struct_with_list_field",
    );
}

#[test]
fn test_aot_list_of_strings() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_list_of_strings.ori"),
        "list_of_strings",
    );
}

#[test]
fn test_aot_struct_with_string_fields_shared() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_struct_with_string_fields_shared.ori"),
        "struct_string_fields_shared",
    );
}

// Closures: zero capture, multiple capture, nested

#[test]
fn test_aot_closure_zero_capture() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_closure_zero_capture.ori"),
        "closure_zero_capture",
    );
}

#[test]
fn test_aot_closure_capturing_closure() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_closure_capturing_closure.ori"),
        "closure_capturing_closure",
    );
}

// Enumerate iterator (produces tuples)

#[test]
fn test_aot_iter_enumerate() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_iter_enumerate.ori"),
        "iter_enumerate",
    );
}

// Deep nesting stress

#[test]
fn test_aot_match_inside_loop_inside_if() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_match_inside_loop_inside_if.ori"),
        "match_inside_loop_inside_if",
    );
}

// Comparison operators on structs (via trait dispatch)

#[test]
fn test_aot_derive_eq_struct_not_equal() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_derive_eq_struct_not_equal.ori"),
        "derive_eq_struct_neq",
    );
}

#[test]
fn test_aot_derive_eq_struct_with_strings() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_derive_eq_struct_with_strings.ori"),
        "derive_eq_struct_strings",
    );
}

#[test]
fn test_aot_derive_comparable_struct() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_derive_comparable_struct.ori"),
        "derive_comparable_struct",
    );
}

// Panic and error handling (non-catch)

#[test]
fn test_aot_panic_basic() {
    let source = include_str!("fixtures/spec/aot_panic_basic.ori");
    // Should succeed (panic branch not taken)
    assert_aot_success(source, "panic_basic");
}

#[test]
fn test_aot_option_unwrap_some() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_option_unwrap_some.ori"),
        "option_unwrap_some",
    );
}

#[test]
fn test_aot_result_unwrap_ok() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_result_unwrap_ok.ori"),
        "result_unwrap_ok",
    );
}

// ARC: collections of RC'd values (more patterns)

#[test]
fn test_aot_struct_with_list_and_string() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_struct_with_list_and_string.ori"),
        "struct_with_list_and_string",
    );
}

#[test]
fn test_aot_nested_struct_with_strings() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_nested_struct_with_strings.ori"),
        "nested_struct_with_strings",
    );
}

// For-yield with complex expressions

#[test]
fn test_aot_for_yield_with_filter() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_for_yield_with_filter.ori"),
        "for_yield_with_filter",
    );
}

#[test]
fn test_aot_for_yield_transform() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_for_yield_transform.ori"),
        "for_yield_transform",
    );
}

// =========================================================================
// Prelude builtin functions (str, int, float, byte, hash_combine)
// =========================================================================

#[test]
fn test_aot_str_from_int() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(r#"@main () -> void = print(msg: str(42));"#);
    assert_eq!(exit_code, 0, "str_from_int failed: {stderr}");
    assert!(
        stdout.contains("42"),
        "Expected '42' in output, got: '{stdout}'"
    );
}

#[test]
fn test_aot_str_from_bool() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(r#"@main () -> void = print(msg: str(true));"#);
    assert_eq!(exit_code, 0, "str_from_bool failed: {stderr}");
    assert!(
        stdout.contains("true"),
        "Expected 'true' in output, got: '{stdout}'"
    );
}

#[test]
fn test_aot_str_from_float() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(r#"@main () -> void = print(msg: str(3.14));"#);
    assert_eq!(exit_code, 0, "str_from_float failed: {stderr}");
    assert!(
        stdout.contains("3.14"),
        "Expected '3.14' in output, got: '{stdout}'"
    );
}

#[test]
fn test_aot_str_from_str() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(r#"@main () -> void = print(msg: str("hello"));"#);
    assert_eq!(exit_code, 0, "str_from_str failed: {stderr}");
    assert!(
        stdout.contains("hello"),
        "Expected 'hello' in output, got: '{stdout}'"
    );
}

#[test]
fn test_aot_int_from_float() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_int_from_float.ori"),
        "int_from_float",
    );
}

#[test]
fn test_aot_int_from_bool() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_int_from_bool.ori"),
        "int_from_bool",
    );
}

#[test]
fn test_aot_float_from_int() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_float_from_int.ori"),
        "float_from_int",
    );
}

#[test]
fn test_aot_byte_from_int() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_byte_from_int.ori"),
        "byte_from_int",
    );
}

#[test]
fn test_aot_hash_combine_basic() {
    assert_aot_success(
        include_str!("fixtures/spec/aot_hash_combine_basic.ori"),
        "hash_combine_basic",
    );
}
