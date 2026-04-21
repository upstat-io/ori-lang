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

// ─── Generic chain root-extension regression matrix (§04.2.B) ───
//
// Coverage for the union-find root-extension fix that lets deferred
// monomorphization resolve callees whose scheme var is NOT the
// representative of its equivalence class. The fix landed in
// `extend_var_subst_with_roots` (`pool/substitute/mod.rs`) and is invoked
// at the three mono call sites: eager typeck, deferred typeck exports,
// and JIT imported-mono. Each fixture exercises a different shape through
// the 3+ hop chain; pre-fix all of these either silently miscompiled or
// fired the §04.2 PC-2 assertion at codegen.

#[test]
fn test_generic_chain_four_levels() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_four_levels.ori"),
        "generic_chain_four_levels",
    );
}

#[test]
fn test_generic_chain_four_levels_string() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_four_levels_string.ori"),
        "generic_chain_four_levels_string",
    );
}

#[test]
fn test_generic_chain_five_levels() {
    // Guards against off-by-one in the root-extension recursion.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_five_levels.ori"),
        "generic_chain_five_levels",
    );
}

#[test]
fn test_generic_chain_option_wrapped() {
    // 3-hop with Option<T> as the type argument.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_option_wrapped.ori"),
        "generic_chain_option_wrapped",
    );
}

#[test]
fn test_generic_chain_result_wrapped() {
    // 3-hop with Result<T, E>.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_result_wrapped.ori"),
        "generic_chain_result_wrapped",
    );
}

#[test]
#[ignore = "blocked-by: BUG-04-090 — AOT codegen generic forwarder applied to [T] causes \
            ori_rc_dec on already-freed allocation. Minimal repro: `@id<T>(x: T) -> T = x; \
            let xs = id(x: [1,2,3]); xs.len()` aborts with 'ori_rc_dec called on \
            already-freed allocation at …'. Reproduces at 2-hop (generic_calling_generic \
            with [int] element), so this is NOT a §04.2.B regression — root-extension fix \
            landed cleanly; the [T] through-generic RC path was already broken. \
            Tracked: plans/bug-tracker/section-04-codegen-llvm.md BUG-04-090."]
fn test_generic_chain_list_element() {
    // 3-hop with [T] — RC-managed element type.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_list_element.ori"),
        "generic_chain_list_element",
    );
}

#[test]
fn test_generic_chain_tuple_element() {
    // 3-hop with (T, T).
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_tuple_element.ori"),
        "generic_chain_tuple_element",
    );
}

#[test]
fn test_generic_chain_user_struct() {
    // 3-hop with user-defined struct. The existing
    // `test_generic_chain_with_struct` is 2-hop (main → apply_identity →
    // identity); this one is 3-hop so the deferred-mono root-extension
    // path is exercised end-to-end with a struct type argument.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_user_struct.ori"),
        "generic_chain_user_struct",
    );
}

#[test]
fn test_generic_chain_forwarded_in_return_only() {
    // T appears only in the return position; the scheme var flows
    // through deferred mono on the return channel.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_forwarded_in_return_only.ori"),
        "generic_chain_forwarded_in_return_only",
    );
}

#[test]
fn test_generic_chain_forwarded_in_deep_field() {
    // T appears in a nested field position — Option<(int, T)>.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_forwarded_in_deep_field.ori"),
        "generic_chain_forwarded_in_deep_field",
    );
}

#[test]
fn test_generic_multiple_deferred_callees() {
    // Two deferred callees in one body — each gets root extension.
    assert_aot_success(
        include_str!("fixtures/generics/generic_multiple_deferred_callees.ori"),
        "generic_multiple_deferred_callees",
    );
}

#[test]
fn test_generic_recursive_chain() {
    // Self-recursive generic — each recursive call site is a deferred
    // mono resolution on the same scheme var.
    assert_aot_success(
        include_str!("fixtures/generics/generic_recursive_chain.ori"),
        "generic_recursive_chain",
    );
}

#[test]
fn test_generic_mutual_recursion_scc() {
    // Two mutually-recursive generics in one SCC.
    assert_aot_success(
        include_str!("fixtures/generics/generic_mutual_recursion_scc.ori"),
        "generic_mutual_recursion_scc",
    );
}

#[test]
fn test_generic_trait_dispatch_through_forwarder() {
    // T: Printable called from a 3-hop chain — ensures trait-method
    // dispatch resolution doesn't bypass root-extension.
    assert_aot_success(
        include_str!("fixtures/generics/generic_trait_dispatch_through_forwarder.ori"),
        "generic_trait_dispatch_through_forwarder",
    );
}

#[test]
fn test_generic_iterator_item_only_positional() {
    // T appears only via an iterator element channel (.iter() on [T]) —
    // not as a direct scalar parameter.
    assert_aot_success(
        include_str!("fixtures/generics/generic_iterator_item_only_positional.ori"),
        "generic_iterator_item_only_positional",
    );
}

#[test]
fn test_generic_closure_capture_forwarded() {
    // Generic forwarder captures T in a lambda; the returned closure is
    // invoked at the call site.
    assert_aot_success(
        include_str!("fixtures/generics/generic_closure_capture_forwarded.ori"),
        "generic_closure_capture_forwarded",
    );
}

#[test]
#[ignore = "blocked: inherent method on generic type (impl<T> Box<T> { @m (self) -> T }) \
            triggers 'unresolved function in apply — missing mono instance' at codegen; \
            method-level generic parameter (@map<U>) additionally not parsed \
            (parser: `expected (, found <`). Both are pre-existing gaps unrelated to the \
            §04.2.B root-extension fix. Reduced shape (3-hop chain on a user-defined \
            generic struct via field access) is covered by test_generic_chain_user_struct; \
            the method-dispatch shape cannot be reduced without losing the test's purpose \
            (two-level rigid-var scoping)."]
fn test_generic_method_on_generic_type() {
    // impl<T> Box<T> { @map<U> ... } — generic method on a generic type.
    // Two-level rigid-var scoping through deferred-mono resolution.
}
