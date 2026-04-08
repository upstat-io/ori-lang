//! Generic Function AOT Tests
//!
//! Tests for generic function monomorphization through the AOT pipeline.
//! Covers: identity/pair basics, string/struct type arguments (RC-managed),
//! generic-calling-generic chains, multiple specializations, generic with
//! Option/Result, and generic functions returning tuples.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Generic with string arguments (RC-managed) ───

#[test]
fn test_generic_identity_string() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_identity_string.ori"),
        "generic_identity_string",
    );
}

#[test]
fn test_generic_pair_with_strings() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_pair_with_strings.ori"),
        "generic_pair_strings",
    );
}

#[test]
fn test_generic_swap_strings() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_swap_strings.ori"),
        "generic_swap_strings",
    );
}

// ─── Generic with struct arguments ───

#[test]
fn test_generic_identity_struct() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_identity_struct.ori"),
        "generic_identity_struct",
    );
}

#[test]
fn test_generic_pair_mixed_struct_int() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_pair_mixed_struct_int.ori"),
        "generic_pair_struct_int",
    );
}

// ─── Multiple specializations in same program ───

#[test]
fn test_generic_four_specializations() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_four_specializations.ori"),
        "generic_four_specializations",
    );
}

#[test]
fn test_generic_same_type_multiple_calls() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_same_type_multiple_calls.ori"),
        "generic_same_type_multiple",
    );
}

// ─── Generic calling generic ───

#[test]
fn test_generic_calling_generic() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_calling_generic.ori"),
        "generic_calling_generic",
    );
}

#[test]
fn test_generic_chain_three_levels() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_three_levels.ori"),
        "generic_chain_three_levels",
    );
}

// ─── Generic calling generic: multi-type-param ───

#[test]
fn test_generic_chain_two_type_params() {
    // A<T> calls B<T> which has TWO type params — one from caller, one concrete.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_two_type_params.ori"),
        "generic_chain_two_params",
    );
}

#[test]
fn test_generic_chain_swap_params() {
    // Caller has <A,B>, callee has <X,Y> — params cross over.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_swap_params.ori"),
        "generic_chain_swap_params",
    );
}

// ─── Generic calling generic: multiple callees ───

#[test]
fn test_generic_calls_two_different_generics() {
    // One generic caller invokes two different generic callees.
    assert_aot_success(
        include_str!("fixtures/generics/generic_calls_two_different_generics.ori"),
        "generic_calls_two_generics",
    );
}

// ─── Generic calling generic: string (RC-managed) ───

#[test]
fn test_generic_chain_with_strings() {
    // Generic chain with RC-managed types to verify retain/release correctness.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_with_strings.ori"),
        "generic_chain_strings",
    );
}

#[test]
fn test_generic_chain_three_levels_string() {
    // Three-level chain with strings — stress-tests RC in deep chains.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_three_levels_string.ori"),
        "generic_chain_three_levels_string",
    );
}

// ─── Generic calling generic: mixed specializations ───

#[test]
fn test_generic_chain_multiple_specializations() {
    // Same chain instantiated with different types in one program.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_multiple_specializations.ori"),
        "generic_chain_four_specializations",
    );
}

// ─── Generic calling generic: with conditional logic ───

#[test]
fn test_generic_chain_choose() {
    // Chain where the intermediate generic uses conditional logic.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_choose.ori"),
        "generic_chain_choose",
    );
}

// ─── Generic calling generic: with struct types ───

#[test]
fn test_generic_chain_with_struct() {
    // Chain passing structs through multiple generic functions.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_with_struct.ori"),
        "generic_chain_struct",
    );
}

// ─── Generic with conditional logic ───

#[test]
fn test_generic_with_bool_condition() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_with_bool_condition.ori"),
        "generic_choose",
    );
}

#[test]
fn test_generic_choose_strings() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_choose_strings.ori"),
        "generic_choose_strings",
    );
}

// ─── Generic returning tuple ───

#[test]
fn test_generic_duplicate() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_duplicate.ori"),
        "generic_duplicate",
    );
}

#[test]
fn test_generic_duplicate_string() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_duplicate_string.ori"),
        "generic_duplicate_string",
    );
}

// ─── Generic with Option ───

#[test]
fn test_generic_with_option_some() {
    // NOTE: Uses .is_some()/.unwrap() instead of match to avoid
    // pre-existing ARC leak in Option match codegen (not generic-specific).
    assert_aot_success(
        include_str!("fixtures/generics/generic_with_option_some.ori"),
        "generic_option_some",
    );
}

#[test]
fn test_generic_wrap_in_option() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_wrap_in_option.ori"),
        "generic_wrap_option",
    );
}

#[test]
fn test_generic_option_match_leak() {
    // This test documents the leak — match on Option leaks even without generics.
    assert_aot_success(
        include_str!("fixtures/generics/generic_option_match_leak.ori"),
        "generic_option_match_leak",
    );
}

// ─── Generic with Result ───

#[test]
fn test_generic_ok_result() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_ok_result.ori"),
        "generic_ok_result",
    );
}

// ─── Generic with HOF ───

#[test]
fn test_generic_apply_fn() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_apply_fn.ori"),
        "generic_apply_fn",
    );
}

// ─── Generic with for-yield ───

#[test]
fn test_generic_in_for_yield() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_in_for_yield.ori"),
        "generic_in_for_yield",
    );
}

// ─── Generic with user-defined struct params (indirect type vars) ───

#[test]
fn test_generic_struct_field_access() {
    // Type param T nested in struct param — tests indirect var extraction.
    // first(p: Pair{42, 10}) + 1 should return 43, not 1.
    assert_aot_success(
        include_str!("fixtures/generics/generic_struct_field_access.ori"),
        "generic_struct_field_access",
    );
}

#[test]
fn test_generic_struct_two_fields() {
    // Verify both fields of a generic struct are accessible.
    assert_aot_success(
        include_str!("fixtures/generics/generic_struct_two_fields.ori"),
        "generic_struct_two_fields",
    );
}

#[test]
fn test_generic_struct_nested() {
    // Generic struct containing another generic struct.
    assert_aot_success(
        include_str!("fixtures/generics/generic_struct_nested.ori"),
        "generic_struct_nested",
    );
}

#[test]
fn test_generic_struct_with_string_field() {
    // Generic struct with RC-managed field — tests ARC interaction.
    assert_aot_success(
        include_str!("fixtures/generics/generic_struct_with_string_field.ori"),
        "generic_struct_string_field",
    );
}

// ─── Monomorphized nounwind analysis ───

#[test]
fn test_mono_nounwind_callee_uses_call_not_invoke() {
    // After the two-pass nounwind fix, _ori_main should call identity$m$int
    // with `call` (not `invoke`) because identity is trivially nounwind.
    let ir = crate::util::compile_and_capture_ir(include_str!(
        "fixtures/generics/mono_nounwind_callee_uses_call_not_invoke.ori"
    ));
    // Find the _ori_main function in the IR and check for call vs invoke
    // to the monomorphized identity function.
    let main_section = crate::util::extract_function_ir(&ir, "_ori_main");

    // The monomorphized function name contains identity and $m$
    assert!(
        main_section.contains("call fastcc"),
        "expected `call fastcc` for nounwind monomorphized callee in _ori_main, \
         but found invoke or missing call.\nIR:\n{main_section}"
    );
    // Verify it's NOT using invoke for the identity call
    assert!(
        !main_section.contains("invoke fastcc"),
        "expected NO `invoke fastcc` for nounwind monomorphized callee in _ori_main — \
         the two-pass nounwind analysis should have proven identity nounwind.\nIR:\n{main_section}"
    );
}

// ─── Generic debug/str on compound types (regression) ───

/// Regression: generic `debug()` on `[int]` through LLVM.
#[test]
fn test_generic_debug_list() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_debug_list.ori"),
        "generic_debug_list",
    );
}

/// Regression: `str()` prelude function on compound types
/// in generic bodies with string concat + `debug`.
#[test]
fn test_generic_str_compound() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_str_compound.ori"),
        "generic_str_compound",
    );
}

#[test]
#[ignore = "Pre-existing: nounwind analysis doesn't yet distinguish may-unwind monomorphized callees"]
fn test_mono_may_unwind_callee_uses_invoke() {
    // A generic function that calls panic should still use `invoke`.
    let ir = crate::util::compile_and_capture_ir(include_str!(
        "fixtures/generics/mono_may_unwind_callee_uses_invoke.ori"
    ));

    let main_section = crate::util::extract_function_ir(&ir, "_ori_main");

    // _ori_main should use `invoke` for may_panic$m$int because it calls panic
    assert!(
        main_section.contains("invoke fastcc"),
        "expected `invoke fastcc` for may-unwind monomorphized callee in _ori_main.\n\
         IR:\n{main_section}"
    );
}
