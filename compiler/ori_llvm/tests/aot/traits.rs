//! AOT Trait and Method Codegen Tests
//!
//! End-to-end tests verifying that trait methods, impl methods, and built-in
//! method dispatch produce correct native code through the LLVM backend.
//!
//! Covers roadmap Section 3 items:
//! - 3.0: Core library traits (Len, `IsEmpty`, Option, Result, Comparable, Eq)
//! - 3.1: Trait declarations (default methods)
//! - 3.2: Trait implementations (inherent impl, trait impl, method resolution)
//! - 3.14: Comparable/Hashable for compound types (Option, Result, Tuple, List)
//! - 3.21: Operator traits (user-defined +, -, *, /, %, //, &, |, ^, <<, >>)

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_capture};

// 3.0.1: Len Trait — .len() codegen

#[test]
fn test_aot_list_len_basic() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_list_len_basic.ori"),
        "list_len_basic",
    );
}

#[test]
fn test_aot_list_len_empty() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_list_len_empty.ori"),
        "list_len_empty",
    );
}

#[test]
fn test_aot_list_len_single() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_list_len_single.ori"),
        "list_len_single",
    );
}

#[test]
fn test_aot_string_len() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_string_len.ori"),
        "string_len",
    );
}

#[test]
fn test_aot_string_len_empty() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_string_len_empty.ori"),
        "string_len_empty",
    );
}

// 3.0.2: IsEmpty Trait — .is_empty() codegen

#[test]
fn test_aot_list_is_empty_true() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_list_is_empty_true.ori"),
        "list_is_empty_true",
    );
}

#[test]
fn test_aot_list_is_empty_false() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_list_is_empty_false.ori"),
        "list_is_empty_false",
    );
}

#[test]
fn test_aot_string_is_empty_true() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_string_is_empty_true.ori"),
        "string_is_empty_true",
    );
}

#[test]
fn test_aot_string_is_empty_false() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_string_is_empty_false.ori"),
        "string_is_empty_false",
    );
}

// 3.0.3: Option Methods — .is_some(), .is_none(), .unwrap(), .unwrap_or() codegen

#[test]
fn test_aot_option_is_some_true() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_is_some_true.ori"),
        "option_is_some_true",
    );
}

#[test]
fn test_aot_option_is_some_false() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_is_some_false.ori"),
        "option_is_some_false",
    );
}

#[test]
fn test_aot_option_is_none_true() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_is_none_true.ori"),
        "option_is_none_true",
    );
}

#[test]
fn test_aot_option_is_none_false() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_is_none_false.ori"),
        "option_is_none_false",
    );
}

#[test]
fn test_aot_option_unwrap_some() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_unwrap_some.ori"),
        "option_unwrap_some",
    );
}

#[test]
fn test_aot_option_unwrap_or_some() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_unwrap_or_some.ori"),
        "option_unwrap_or_some",
    );
}

#[test]
fn test_aot_option_unwrap_or_none() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_unwrap_or_none.ori"),
        "option_unwrap_or_none",
    );
}

// 3.0.4: Result Methods — .is_ok(), .is_err(), .unwrap() codegen

#[test]
fn test_aot_result_is_ok_true() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_result_is_ok_true.ori"),
        "result_is_ok_true",
    );
}

#[test]
fn test_aot_result_is_ok_false() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_result_is_ok_false.ori"),
        "result_is_ok_false",
    );
}

#[test]
fn test_aot_result_is_err_true() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_result_is_err_true.ori"),
        "result_is_err_true",
    );
}

#[test]
fn test_aot_result_is_err_false() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_result_is_err_false.ori"),
        "result_is_err_false",
    );
}

#[test]
fn test_aot_result_unwrap_ok() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_result_unwrap_ok.ori"),
        "result_unwrap_ok",
    );
}

// 3.0.5: Comparable Trait — .compare() codegen

#[test]
fn test_aot_int_compare_less() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_int_compare_less.ori"),
        "int_compare_less",
    );
}

#[test]
fn test_aot_int_compare_equal() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_int_compare_equal.ori"),
        "int_compare_equal",
    );
}

#[test]
fn test_aot_int_compare_greater() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_int_compare_greater.ori"),
        "int_compare_greater",
    );
}

#[test]
fn test_aot_ordering_reverse() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_ordering_reverse.ori"),
        "ordering_reverse",
    );
}

#[test]
fn test_aot_ordering_predicates() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_ordering_predicates.ori"),
        "ordering_predicates",
    );
}

#[test]
fn test_aot_ordering_is_less_or_equal() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_ordering_is_less_or_equal.ori"),
        "ordering_is_less_or_equal",
    );
}

#[test]
fn test_aot_ordering_is_greater_or_equal() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_ordering_is_greater_or_equal.ori"),
        "ordering_is_greater_or_equal",
    );
}

// 3.0.6: Eq Trait — == and != codegen (explicit coverage)

#[test]
fn test_aot_eq_int() {
    assert_aot_success(include_str!("fixtures/traits/aot_eq_int.ori"), "eq_int");
}

#[test]
fn test_aot_eq_bool() {
    assert_aot_success(include_str!("fixtures/traits/aot_eq_bool.ori"), "eq_bool");
}

#[test]
fn test_aot_eq_string() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_eq_string.ori"),
        "eq_string",
    );
}

// Structural equality for user types without #derive(Eq)

#[test]
fn test_aot_enum_structural_eq() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_enum_structural_eq.ori"),
        "enum_structural_eq",
    );
}

#[test]
fn test_aot_payload_enum_structural_eq() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_payload_enum_structural_eq.ori"),
        "payload_enum_structural_eq",
    );
}

#[test]
fn test_aot_option_ordering() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_ordering.ori"),
        "option_ordering",
    );
}

// 3.2: Trait Implementations — Inherent impl codegen

#[test]
fn test_aot_inherent_impl_method() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_inherent_impl_method.ori"),
        "inherent_impl_method",
    );
}

#[test]
fn test_aot_inherent_impl_with_params() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_inherent_impl_with_params.ori"),
        "inherent_impl_with_params",
    );
}

// 3.2: Trait Implementations — Trait impl codegen

#[test]
fn test_aot_trait_impl_method() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_trait_impl_method.ori"),
        "trait_impl_method",
    );
}

#[test]
fn test_aot_trait_impl_multiple_methods() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_trait_impl_multiple_methods.ori"),
        "trait_impl_multiple_methods",
    );
}

// 3.1: Trait Declarations — Default method codegen

#[test]
fn test_aot_trait_default_method() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_trait_default_method.ori"),
        "trait_default_method",
    );
}

// 3.2: Method resolution — inherent methods take priority over trait methods

#[test]
fn test_aot_method_resolution_inherent_over_trait() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_method_resolution_inherent_over_trait.ori"),
        "method_resolution_inherent_over_trait",
    );
}

// 3.2: User-defined impl method dispatch — struct field access in methods

#[test]
fn test_aot_impl_method_field_access() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_impl_method_field_access.ori"),
        "impl_method_field_access",
    );
}

// 3.2: Trait impl method accessing self struct fields — regression for
// Trait impl methods must not be registered in the bare `functions` map or
// field-type method calls inside the impl body resolve to the wrong function.
#[test]
fn test_aot_trait_impl_field_access() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_trait_impl_field_access.ori"),
        "trait_impl_field_access",
    );
}

// 3.2: Multiple impl blocks on same type

#[test]
fn test_aot_multiple_impl_blocks() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_multiple_impl_blocks.ori"),
        "multiple_impl_blocks",
    );
}

// -----------------------------------------------------------------------
// 3.21: Operator Traits — user-defined operator dispatch
// -----------------------------------------------------------------------

#[test]
fn test_aot_operator_trait_add() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_operator_trait_add.ori"),
        "operator_trait_add",
    );
}

#[test]
fn test_aot_operator_trait_sub() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_operator_trait_sub.ori"),
        "operator_trait_sub",
    );
}

#[test]
fn test_aot_operator_trait_neg() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_operator_trait_neg.ori"),
        "operator_trait_neg",
    );
}

#[test]
fn test_aot_operator_trait_mul_mixed() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_operator_trait_mul_mixed.ori"),
        "operator_trait_mul_mixed",
    );
}

#[test]
fn test_aot_operator_trait_chained() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_operator_trait_chained.ori"),
        "operator_trait_chained",
    );
}

#[test]
fn test_aot_operator_trait_bitwise() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_operator_trait_bitwise.ori"),
        "operator_trait_bitwise",
    );
}

#[test]
fn test_aot_operator_trait_not() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_operator_trait_not.ori"),
        "operator_trait_not",
    );
}

// =========================================================================
// 3.14: Comparable/Hashable compound type methods
// =========================================================================

// -- String methods --

#[test]
fn test_aot_str_compare() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_str_compare.ori"),
        "str_compare",
    );
}

#[test]
fn test_aot_str_equals() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_str_equals.ori"),
        "str_equals",
    );
}

#[test]
fn test_aot_str_hash() {
    assert_aot_success(include_str!("fixtures/traits/aot_str_hash.ori"), "str_hash");
}

// -- Bool hash --

#[test]
fn test_aot_bool_hash() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_bool_hash.ori"),
        "bool_hash",
    );
}

// -- Ordering compare --

#[test]
fn test_aot_ordering_compare() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_ordering_compare.ori"),
        "ordering_compare",
    );
}

// -- Float hash --

#[test]
fn test_aot_float_hash() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_float_hash.ori"),
        "float_hash",
    );
}

// -- hash_combine --

#[test]
fn test_aot_hash_combine() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_hash_combine.ori"),
        "hash_combine",
    );
}

// -- Option compare --

#[test]
fn test_aot_option_compare() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_compare.ori"),
        "option_compare",
    );
}

// -- Option equals --

#[test]
fn test_aot_option_equals() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_equals.ori"),
        "option_equals",
    );
}

// -- Option hash --

#[test]
fn test_aot_option_hash() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_hash.ori"),
        "option_hash",
    );
}

// -- Result compare --

#[test]
fn test_aot_result_compare() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_result_compare.ori"),
        "result_compare",
    );
}

// -- Result equals --

#[test]
fn test_aot_result_equals() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_result_equals.ori"),
        "result_equals",
    );
}

// -- Result hash --

#[test]
fn test_aot_result_hash() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_result_hash.ori"),
        "result_hash",
    );
}

// -- Result equals (heterogeneous: Ok=int, Err=str) --

#[test]
fn test_aot_result_equals_heterogeneous() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_result_equals_heterogeneous.ori"),
        "result_equals_heterogeneous",
    );
}

// -- Result compare (heterogeneous: Ok=int, Err=str) --

#[test]
fn test_aot_result_compare_heterogeneous() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_result_compare_heterogeneous.ori"),
        "result_compare_heterogeneous",
    );
}

// -- Result hash (heterogeneous: Ok=int, Err=str) --

#[test]
fn test_aot_result_hash_heterogeneous() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_result_hash_heterogeneous.ori"),
        "result_hash_heterogeneous",
    );
}

// -- Tuple compare --

#[test]
fn test_aot_tuple_compare() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_tuple_compare.ori"),
        "tuple_compare",
    );
}

// -- Tuple equals --

#[test]
fn test_aot_tuple_equals() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_tuple_equals.ori"),
        "tuple_equals",
    );
}

// -- Tuple hash --

#[test]
fn test_aot_tuple_hash() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_tuple_hash.ori"),
        "tuple_hash",
    );
}

// -- Primitive equals methods --

#[test]
fn test_aot_int_equals() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_int_equals.ori"),
        "int_equals",
    );
}

#[test]
fn test_aot_byte_compare() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_byte_compare.ori"),
        "byte_compare",
    );
}

#[test]
fn test_aot_char_hash() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_char_hash.ori"),
        "char_hash",
    );
}

// =========================================================================
// 3.14: Hash contract edge cases (hygiene fixes)
// =========================================================================

// Float ±0.0 hash contract: -0.0 == 0.0 → hash must match

#[test]
fn test_aot_float_hash_neg_zero() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_float_hash_neg_zero.ori"),
        "float_hash_neg_zero",
    );
}

// Byte hash: values ≥ 128 must use unsigned extension

#[test]
fn test_aot_byte_hash_high_value() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_byte_hash_high_value.ori"),
        "byte_hash_high_value",
    );
}

// String hash quality: different strings of same length must hash differently

#[test]
fn test_aot_str_hash_same_length_different_content() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_str_hash_same_length_different_content.ori"),
        "str_hash_same_length_different",
    );
}

// Nested Option: Option<Option<int>> compare/equals/hash

#[test]
fn test_aot_nested_option_equals() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_nested_option_equals.ori"),
        "nested_option_equals",
    );
}

#[test]
fn test_aot_nested_option_compare() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_nested_option_compare.ori"),
        "nested_option_compare",
    );
}

#[test]
fn test_aot_nested_option_hash() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_nested_option_hash.ori"),
        "nested_option_hash",
    );
}

// Tuple inside Option: Option<(int, int)> compare/equals

#[test]
fn test_aot_option_tuple_equals() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_option_tuple_equals.ori"),
        "option_tuple_equals",
    );
}

// -- List compare --

#[test]
fn test_aot_list_compare() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_list_compare.ori"),
        "list_compare",
    );
}

#[test]
fn test_aot_list_compare_empty() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_list_compare_empty.ori"),
        "list_compare_empty",
    );
}

// -- List equals --

#[test]
fn test_aot_list_equals() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_list_equals.ori"),
        "list_equals",
    );
}

#[test]
fn test_aot_list_equals_empty() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_list_equals_empty.ori"),
        "list_equals_empty",
    );
}

// -- List hash --

#[test]
fn test_aot_list_hash() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_list_hash.ori"),
        "list_hash",
    );
}

#[test]
fn test_aot_list_hash_empty() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_list_hash_empty.ori"),
        "list_hash_empty",
    );
}

// 3.17: Into Trait — .into() codegen

#[test]
fn test_aot_int_into_float() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_int_into_float.ori"),
        "int_into_float",
    );
}

#[test]
fn test_aot_int_into_float_negative() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_int_into_float_negative.ori"),
        "int_into_float_neg",
    );
}

#[test]
fn test_aot_int_into_float_zero() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_int_into_float_zero.ori"),
        "int_into_float_zero",
    );
}

// -----------------------------------------------------------------------
// Regression: Analysis-only ARC lowering path coverage
//
// These tests exercise the impl-method analysis-only ARC lowering path
// that feeds §03.5 range analysis. The codegen pipeline lowers impl method
// bodies into analysis-only ARC functions with type-qualified names. These
// tests verify that the analysis-only lowering doesn't interfere with
// normal codegen, including default trait methods and multi-impl types.
// -----------------------------------------------------------------------

/// Regression: multiple trait impls on the same type exercise
/// the analysis-only ARC lowering path with ordinal-qualified names.
///
/// The analysis-only path creates type-qualified names for each impl method.
/// Multiple trait impls on the same type exercise the ordinal counter and
/// ensure the analysis path doesn't interfere with codegen dispatch.
///
/// Note: trait impl methods avoid field access on `self` due to
/// (trait impl methods with field access produce LLVM verification errors).
/// Inherent methods test field access; trait methods test constant returns.
#[test]
fn test_aot_multi_trait_impl_analysis_path() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_multi_trait_impl_analysis_path.ori"),
        "multi_trait_impl_analysis_path",
    );
}

/// Regression: default trait method in impl block is correctly
/// analyzed through the analysis-only ARC lowering path.
///
/// The impl block for `Describable` uses the default method `@describe`
/// without overriding it. The analysis-only path must resolve the default
/// method body (not skip it or use the wrong body).
#[test]
fn test_aot_default_trait_method_analysis_path() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_default_trait_method_analysis_path.ori"),
        "default_trait_method_analysis_path",
    );
}

/// Regression: combined inherent, trait, and default methods
/// on a single type, exercising the full analysis-only path complexity.
///
/// This program has 4 impl blocks on the same type (inherent + 3 traits,
/// one with default), producing multiple analysis-only ARC functions.
/// Verifies the analysis path processes all bodies without interference.
///
/// Note: trait impl methods use constant returns (workaround).
/// Inherent methods exercise full field access + computation.
#[test]
fn test_aot_impl_analysis_combined_scenario() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_impl_analysis_combined_scenario.ori"),
        "impl_analysis_combined_scenario",
    );
}

/// Regression: multiple types each with multiple impls,
/// exercising the analysis-only path across the full module.
///
/// Ensures that the analysis-only ARC lowering processes impl methods
/// from multiple distinct types without cross-type interference.
///
/// Note: trait impl methods use constant returns (workaround).
#[test]
fn test_aot_impl_analysis_multiple_types() {
    assert_aot_success(
        include_str!("fixtures/traits/aot_impl_analysis_multiple_types.ori"),
        "impl_analysis_multiple_types",
    );
}

// Wrapper debug with compound/str payloads

/// Regression: `Option<str>.debug()` must use Debug semantics (quotes).
/// Interpreter prints `Some("hi")`, AOT was printing `Some(hi)`.
#[test]
fn test_aot_option_debug_str_payload() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/traits/aot_option_debug_str_payload.ori"
    ));
    assert_eq!(exit_code, 0, "option_debug_str_payload failed: {stderr}");
    assert!(
        stdout.contains(r#"Some("hi")"#),
        "Expected 'Some(\"hi\")' in output, got: '{stdout}'"
    );
}

/// Regression: `Option<[int]>.debug()` must format list payloads.
/// Interpreter prints `Some([1, 2, 3])`, AOT was printing empty line.
#[test]
fn test_aot_option_debug_list_payload() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/traits/aot_option_debug_list_payload.ori"
    ));
    assert_eq!(exit_code, 0, "option_debug_list_payload failed: {stderr}");
    assert!(
        stdout.contains("Some([1, 2, 3])"),
        "Expected 'Some([1, 2, 3])' in output, got: '{stdout}'"
    );
}

/// Regression: `Result<[int], str>.debug()` must format list payloads.
#[test]
fn test_aot_result_debug_list_payload() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/traits/aot_result_debug_list_payload.ori"
    ));
    assert_eq!(exit_code, 0, "result_debug_list_payload failed: {stderr}");
    assert!(
        stdout.contains("Ok([4, 5])"),
        "Expected 'Ok([4, 5])' in output, got: '{stdout}'"
    );
}

/// Edge case: `None.debug()` must produce "None".
#[test]
fn test_aot_option_debug_none() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(include_str!("fixtures/traits/aot_option_debug_none.ori"));
    assert_eq!(exit_code, 0, "option_debug_none failed: {stderr}");
    assert!(
        stdout.contains("None"),
        "Expected 'None' in output, got: '{stdout}'"
    );
}

/// Regression: `Err(str).debug()` must quote the string.
#[test]
fn test_aot_result_debug_err_str() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(include_str!("fixtures/traits/aot_result_debug_err_str.ori"));
    assert_eq!(exit_code, 0, "result_debug_err_str failed: {stderr}");
    assert!(
        stdout.contains(r#"Err("oops")"#),
        "Expected 'Err(\"oops\")' in output, got: '{stdout}'"
    );
}

/// Regression: nested `Option<Option<int>>.debug()` must work recursively.
#[test]
fn test_aot_option_debug_nested() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(include_str!("fixtures/traits/aot_option_debug_nested.ori"));
    assert_eq!(exit_code, 0, "option_debug_nested failed: {stderr}");
    assert!(
        stdout.contains("Some(Some(42))"),
        "Expected 'Some(Some(42))' in output, got: '{stdout}'"
    );
}

/// Edge case: `Option<[int]>.debug()` with empty list must produce "Some([])".
#[test]
fn test_aot_option_debug_empty_list() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/traits/aot_option_debug_empty_list.ori"
    ));
    assert_eq!(exit_code, 0, "option_debug_empty_list failed: {stderr}");
    assert!(
        stdout.contains("Some([])"),
        "Expected 'Some([])' in output, got: '{stdout}'"
    );
}

// Map debug formatting tests

/// Map debug should format as `{key: value, ...}` not `<?>`.
/// Keys use Printable semantics (unquoted strings), values use Debug semantics.
#[test]
fn test_aot_map_debug_str_keys() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(include_str!("fixtures/traits/aot_map_debug_str_keys.ori"));
    assert_eq!(exit_code, 0, "map_debug_str_keys failed: {stderr}");
    // Map iteration order may vary; check both entries are present
    assert!(
        stdout.contains("x: 1") && stdout.contains("y: 2"),
        "Expected map entries 'x: 1' and 'y: 2' in output, got: '{stdout}'"
    );
}

/// Empty map debug should produce `{}`.
#[test]
fn test_aot_map_debug_empty() {
    let (exit_code, stdout, stderr) =
        compile_and_run_capture(include_str!("fixtures/traits/aot_map_debug_empty.ori"));
    assert_eq!(exit_code, 0, "map_debug_empty failed: {stderr}");
    assert!(
        stdout.contains("{}"),
        "Expected '{{}}' in output, got: '{stdout}'"
    );
}

/// Map debug with int keys and string values — values must be quoted.
#[test]
fn test_aot_map_debug_int_keys_str_values() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/traits/aot_map_debug_int_keys_str_values.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "map_debug_int_keys_str_values failed: {stderr}"
    );
    assert!(
        stdout.contains(r#""hello""#) && stdout.contains(r#""world""#),
        "Expected quoted string values in output, got: '{stdout}'"
    );
}

/// Map debug with nested list values — recursive formatting.
#[test]
fn test_aot_map_debug_nested_list_value() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/traits/aot_map_debug_nested_list_value.ori"
    ));
    assert_eq!(exit_code, 0, "map_debug_nested_list_value failed: {stderr}");
    assert!(
        stdout.contains("a: [1, 2]"),
        "Expected 'a: [1, 2]' in output, got: '{stdout}'"
    );
}

/// semantic pin: Option<Map> must format map payload, not `<?>`.
/// This is the exact bug repro from the issue.
#[test]
fn test_aot_option_debug_map_payload() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/traits/aot_option_debug_map_payload.ori"
    ));
    assert_eq!(exit_code, 0, "option_debug_map_payload failed: {stderr}");
    assert!(
        stdout.contains("Some({x: 1})"),
        "Expected 'Some({{x: 1}})' in output, got: '{stdout}'"
    );
    // Negative pin: must NOT contain the old broken output
    assert!(
        !stdout.contains("<?>"),
        "Must not contain '<?>' placeholder, got: '{stdout}'"
    );
}
