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

use crate::util::{assert_aot_success, compile_and_run_capture};

// Generic with string arguments (RC-managed)
#[test]
fn test_generic_identity_string() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_identity_string.ori"),
        "generic_identity_string",
    );
}

/// Regression: an imported prelude generic `len<T: Len>` monomorphized on an
/// aggregate receiver ([int]/{str:int}) left the borrowed receiver a pointer-only
/// param whose LLVM value at use sites is a zero {i64,i64,ptr} placeholder; the
/// len dispatch did `extract_value(FIELD_LEN)` on that placeholder -> 0 under AOT
/// (3 under interp = parity break). The str line is the embedded negative control
/// (str.len reads via `str_to_ptr_forwarded` and was always correct). Fix:
/// `emit_collection_length_forwarded` reads `FIELD_LEN` via a `struct_gep+load` on the
/// `borrowed_param_ptrs` source pointer (mirroring `emit_str_length_forwarded`), so
/// the param stays pointer-only (no RC-flow change); `compute_pointer_only_params`
/// is unchanged.
#[test]
fn test_imported_generic_len_aggregate_receiver() {
    assert_aot_success(
        include_str!("fixtures/generics/imported_generic_len_aggregate.ori"),
        "imported_generic_len_aggregate",
    );
}

/// Regression: the imported prelude generic `is_empty<T>(collection: [T])`
/// monomorphized on a list receiver routes through the same borrowed-receiver
/// forwarder (`emit_collection_is_empty_forwarded`) as `len`. A borrowed receiver
/// left a raw ptr would make `is_empty` read the zero-placeholder len (0) -> TRUE
/// under AOT for a non-empty list (false under interp = parity break). Sibling AOT
/// pin to `test_imported_generic_len_aggregate_receiver`; `is_empty` is list-only in
/// the prelude, so the pin covers the [int] (scalar) + [str] (heap) element dims.
#[test]
fn test_imported_generic_is_empty_aggregate_receiver() {
    assert_aot_success(
        include_str!("fixtures/generics/imported_generic_is_empty_aggregate.ori"),
        "imported_generic_is_empty_aggregate",
    );
}

// Regression: method-level type generic on a concrete-receiver impl returning
// a layout-bearing [T]. A rigid_var-at-codegen ICE regressed only the LLVM/AOT
// path (the interpreter always passed). Exercises int + str (heap-element)
// instantiations of one method-level binder; assert_aot_success also leak-checks
// (exit 2 = leak).
#[test]
fn test_method_generic_layout() {
    assert_aot_success(
        include_str!("fixtures/generics/method_generic_layout.ori"),
        "method_generic_layout",
    );
}

/// Regression: method-mono dispatch on a nested-generic receiver
/// `Option<[int]>.unwrap()`. The recorder substitutes the full concrete receiver
/// `Idx` (`Applied(Option, [List(int)])`), distinct from `Option<int>`, so the
/// collector/mangler/lookup resolve a dedicated `MonoFunction`; a re-collapse of the
/// nested `[int]` to the generic shell re-surfaces a missing-mono-instance E5001.
#[test]
fn test_option_complex_arg_unwrap() {
    assert_aot_success(
        include_str!("fixtures/generics/option_complex_arg_unwrap.ori"),
        "option_complex_arg_unwrap",
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

// Generic with struct arguments
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

// Multiple specializations in same program
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

// Generic calling generic
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

// Generic calling generic: multi-type-param
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

// Generic calling generic: multiple callees
#[test]
fn test_generic_calls_two_different_generics() {
    // One generic caller invokes two different generic callees.
    assert_aot_success(
        include_str!("fixtures/generics/generic_calls_two_different_generics.ori"),
        "generic_calls_two_generics",
    );
}

// Generic calling generic: string (RC-managed)
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

// Generic calling generic: mixed specializations
#[test]
fn test_generic_chain_multiple_specializations() {
    // Same chain instantiated with different types in one program.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_multiple_specializations.ori"),
        "generic_chain_four_specializations",
    );
}

// Generic calling generic: with conditional logic
#[test]
fn test_generic_chain_choose() {
    // Chain where the intermediate generic uses conditional logic.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_choose.ori"),
        "generic_chain_choose",
    );
}

// Generic calling generic: with struct types
#[test]
fn test_generic_chain_with_struct() {
    // Chain passing structs through multiple generic functions.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_with_struct.ori"),
        "generic_chain_struct",
    );
}

// Generic with conditional logic
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

// Generic returning tuple
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

// Generic with Option
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

// Generic with Result
#[test]
fn test_generic_ok_result() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_ok_result.ori"),
        "generic_ok_result",
    );
}

// Generic with HOF
#[test]
fn test_generic_apply_fn() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_apply_fn.ori"),
        "generic_apply_fn",
    );
}

// Generic with for-yield
#[test]
fn test_generic_in_for_yield() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_in_for_yield.ori"),
        "generic_in_for_yield",
    );
}

// Generic with user-defined struct params (indirect type vars)
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

// Monomorphized nounwind analysis
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

// Generic debug/str on compound types (regression)
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
#[ignore = "BUG-04-105: nounwind-analysis does not distinguish may-unwind monomorphized callees; unrelated to BUG-04-090's generic-forwarder surface"]
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

// Generic chain root-extension regression matrix
//
// Coverage for the union-find root-extension fix that lets deferred
// monomorphization resolve callees whose scheme var is NOT the
// representative of its equivalence class. The fix landed in
// `extend_var_subst_with_roots` (`pool/substitute/mod.rs`) and is invoked
// at the three mono call sites: eager typeck, deferred typeck exports,
// and JIT imported-mono. Each fixture exercises a different shape through
// the 3+ hop chain; pre-fix all of these either silently miscompiled or
// fired the PC-2 assertion at codegen.

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
fn test_generic_chain_list_element() {
    // 3-hop with [T] — RC-managed element type.
    assert_aot_success(
        include_str!("fixtures/generics/generic_chain_list_element.ori"),
        "generic_chain_list_element",
    );
}

// BUG-04-090: generic forwarder over [T] hop-count axis
// Each forwarder hop currently emits a spurious scope-exit RcDec on the param
// because AIMS interprocedural analysis lacks `transfers_through_return`. The
// fix completes the Lean 4 `ownParamsUsingArgs` pattern — eliminate BOTH the
// caller-side inc and the callee-side dec on owned-arg → owned-callee-param
// transfer through Return. Tests live without `#[ignore]` so they fail today
// (matching the existing ignored 3-hop case) and pass after the fix activates.

/// Single-hop generic identity forwarder applied to `[int]`. The minimal
/// repro for BUG-04-090: even one hop produces a use-after-free because the
/// callee's spurious scope-exit dec frees the freshly-allocated list before
/// the caller's scope exits.
#[test]
fn test_generic_forwarder_hop1_returns_list_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_hop1_list_int.ori"),
        "generic_forwarder_hop1_list_int",
    );
}

/// Two-hop generic forwarder chain over `[int]`. Each hop adds one spurious
/// dec; verifies the bug surfaces at the hop count below the existing 3-hop
/// fixture so the fix's scope is bounded by the hop-count axis, not the
/// 3-hop SCC depth specifically.
#[test]
fn test_generic_forwarder_hop2_returns_list_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_hop2_list_int.ori"),
        "generic_forwarder_hop2_list_int",
    );
}

/// Four-hop generic forwarder chain over `[int]`. Stresses fixpoint
/// convergence at depth ≥ 4 — guards against off-by-one in the SCC
/// convergence bound (the `IC-7` height-weighted iteration-limit formula in
/// `ori_arc` interprocedural analysis).
#[test]
fn test_generic_forwarder_hop4_returns_list_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_hop4_list_int.ori"),
        "generic_forwarder_hop4_list_int",
    );
}

// BUG-04-090: cross-type axis
// 1-hop generic forwarder applied to every relevant heap-typed shape.
// Heap types fail today (use-after-free); scalar must NOT regress.

/// Generic forwarder over `[str]` — heap list of heap elements; verifies
/// element-level decs still fire correctly (`elem_dec_fn` for inner str).
#[test]
fn test_generic_forwarder_list_str_returns_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_list_str.ori"),
        "generic_forwarder_list_str",
    );
}

/// Generic forwarder over `str` — fat-pointer-shaped heap value;
/// param-return-alias rule fires on str just as it does on `[T]`.
#[test]
fn test_generic_forwarder_str_returns_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_str.ori"),
        "generic_forwarder_str",
    );
}

/// Generic forwarder over `{int: str}` — heap map mixing scalar keys
/// and heap values. Same fat-pointer shape; verifies the rule does not
/// key on collection kind.
#[test]
fn test_generic_forwarder_map_int_str_returns_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_map_int_str.ori"),
        "generic_forwarder_map_int_str",
    );
}

/// Generic forwarder over `Set<int>` — heap set, fat-pointer-backed.
/// Set construction goes through `.iter().collect()` per the standard
/// Ori pattern; the param-return-alias rule fires on the outer Set.
#[test]
fn test_generic_forwarder_set_int_returns_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_set_int.ori"),
        "generic_forwarder_set_int",
    );
}

/// Generic forwarder over a struct containing a heap field. The struct
/// itself flows through Return as Owned; the [int] field's heap buffer
/// must still be properly RC-managed via the struct's drop function.
#[test]
fn test_generic_forwarder_struct_with_list_returns_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_struct_with_list.ori"),
        "generic_forwarder_struct_with_list",
    );
}

/// Generic forwarder over a closure capturing `[int]`. The closure's
/// `env_ptr` (heap-allocated) carries the `[int]` capture; tests
/// closure-aware RC interaction (RE-1 closure-aware RC).
#[test]
fn test_generic_forwarder_closure_capturing_list_returns_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_closure_capturing_list.ori"),
        "generic_forwarder_closure_capturing_list",
    );
}

/// Generic forwarder over `int` — scalar regression clamp. Scalars have
/// NO RC ops (RE-2 scalar exemption); this test must continue to
/// pass both pre-fix and post-fix. The fix introduces no scalar-path
/// changes.
#[test]
fn test_generic_forwarder_int_scalar_must_not_regress() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_int_scalar_no_regression.ori"),
        "generic_forwarder_int_scalar_no_regression",
    );
}

/// Generic forwarder over `Option<[int]>` — sum-type wrapping a heap
/// payload. The outer Option's discriminant + payload flow through
/// Return; payload's heap buffer must be properly RC-managed.
#[test]
fn test_generic_forwarder_option_list_returns_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_option_list.ori"),
        "generic_forwarder_option_list",
    );
}

/// Generic forwarder over `Result<[int], str>` — two-constructor
/// sum-type with heap payloads on BOTH arms. Verifies the rule fires
/// correctly when either arm carries a heap allocation.
#[test]
fn test_generic_forwarder_result_list_str_returns_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_result_list_str.ori"),
        "generic_forwarder_result_list_str",
    );
}

/// Generic forwarder over `[[int]]` — nested heap collection. Outer
/// list owns inner lists; element-drop must recursively dec inner heap
/// buffers when the outer is freed at the caller's scope-exit.
#[test]
fn test_generic_forwarder_nested_list_returns_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_nested_list.ori"),
        "generic_forwarder_nested_list",
    );
}

/// Generic forwarder over `Box<[int]>` — single-field newtype-style
/// struct wrapping a heap value. Per Ori convention, `Box` is a
/// user-defined struct, not a built-in pointer; verifies the rule
/// fires uniformly on user-defined heap-carrying structs.
#[test]
fn test_generic_forwarder_box_list_returns_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_forwarder_box_list.ori"),
        "generic_forwarder_box_list",
    );
}

// BUG-04-090: forwarder shape axis
// The param-return-alias rule must apply uniformly across forwarder
// shapes. Generics are merely the most common pattern; non-generic
// forwarders and inherent methods must also be covered.

/// Non-generic forwarder applied to `[int]`. The fix's gate is
/// STRUCTURAL (param flows to Return), not generics-specific. Verifies
/// the rule does not key on the presence of a type parameter.
#[test]
fn test_non_generic_forwarder_list_returns_intact() {
    assert_aot_success(
        include_str!("fixtures/generics/non_generic_forwarder_list.ori"),
        "non_generic_forwarder_list",
    );
}

/// Inherent method `@inner (self) -> Self = self` on a struct holding
/// a heap field. Verifies the `self` param triggers the same
/// param-return-alias rule as a free-function param.
#[test]
fn test_inherent_method_forwarder_self_returns_self() {
    assert_aot_success(
        include_str!("fixtures/generics/inherent_method_forwarder_self_returns_self.ori"),
        "inherent_method_forwarder_self_returns_self",
    );
}

// BUG-04-090: path-sensitivity
// The fix's suppression gate MUST be path-sensitive. A naive global-
// suppression implementation (suppress all decs unconditionally for
// params with transfers_through_return=true) would LEAK the not-returned
// param on multi-return conditional flow. These cells pin the
// path-sensitivity requirement.

/// `select<T>(cond, x, y) -> T = if cond then x else y` over `[int]`.
/// Both x and y are candidates for return; only ONE is returned per
/// path. The not-returned param must be dec'd at scope-exit. Naive
/// global-suppression leaks; correct path-sensitive fix balances RC.
#[test]
fn test_path_sensitive_select_list_balances_rc_on_both_paths() {
    assert_aot_success(
        include_str!("fixtures/generics/path_sensitive_select_list.ori"),
        "path_sensitive_select_list",
    );
}

/// `dispatch<T>(tag, x, y) -> T = match tag { 0 -> x, _ -> y }` —
/// match-desugar code path. Verifies the fix's terminator-based path
/// check correctly handles match arms (Branch + Jump-arg + merge-block
/// edges per alias-tracking Rules 3+4).
#[test]
fn test_path_sensitive_match_dispatch_list_balances_rc() {
    assert_aot_success(
        include_str!("fixtures/generics/path_sensitive_match_dispatch_list.ori"),
        "path_sensitive_match_dispatch_list",
    );
}

/// Three-way conditional: `triple<T>(c, x, y, z) -> T`. Stresses
/// fixpoint convergence with three `consumed_params`; each path returns
/// exactly one — the other two need decs. Naive global-suppression
/// leaks two heap allocations per call.
#[test]
fn test_path_sensitive_triple_list_balances_rc_on_all_three_paths() {
    assert_aot_success(
        include_str!("fixtures/generics/path_sensitive_triple_list.ori"),
        "path_sensitive_triple_list",
    );
}

// Dead-owned-param-branch release (compute_dead_owned_param_branch_releases): a callee
// Owned non-scalar PARAM that is returned on one branch but DEAD on others leaks on the
// dead branches — the Phase-5 walk emits no per-edge RL-4 release. The positive pins clear
// the leak (the two-param `pick`/`select` family); the negative pin clamps the
// boundary (borrow-read-keeps-live) so the per-edge release never double-frees. Spec:
// Annex E §AIMS RL-4 (edge dec) + RL-2 (release exactly once).

/// Positive matrix pin (type dimension `[str]`): the two-param `pick(c, x, y)` shape over
/// a HEAP-ELEMENT list. On each branch the non-returned `[str]` param is dead; the
/// per-edge release frees the list buffer AND its `str` elements (drop-glue recursion).
/// Reverting (`ORI_DISABLE_DEAD_OWNED_PARAM_BRANCH_RELEASE=1`) leaks the dead `[str]` param.
#[test]
fn test_dead_param_pick_str_list_balances_rc() {
    assert_aot_success(
        include_str!("fixtures/generics/dead_param_pick_str_list_balances_rc.ori"),
        "dead_param_pick_str_list_balances_rc",
    );
}

/// Positive + no-double-free pin: a two-param `pick(c, x, y)` where each `[int]` param is
/// returned on one branch and dead on the other; the per-edge release fires EXACTLY ONCE
/// per param per dead branch (x on the else-edge, y on the then-edge). Reverting
/// (`ORI_DISABLE_DEAD_OWNED_PARAM_BRANCH_RELEASE=1`) leaks; an over-emitting per-edge dec
/// (twice, or on the return edge) double-frees the transferred value (exit-134).
#[test]
fn test_dead_param_returned_on_all_paths_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/generics/dead_param_returned_on_all_paths_no_double_free.ori"),
        "dead_param_returned_on_all_paths_no_double_free",
    );
}

/// Negative over-fire pin (borrow-read keeps it live): a param borrow-read on a branch is
/// LIVE there (in `live_in`), so the dead-param-branch pass MUST NOT emit a release at
/// that block's entry. Reverting the `dead-at-entry` (`p ∉ live_in(B)`) gate emits a
/// spurious release on the read branch → double-free.
#[test]
fn test_dead_param_borrow_read_then_dead_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/generics/dead_param_borrow_read_then_dead_no_double_free.ori"),
        "dead_param_borrow_read_then_dead_no_double_free",
    );
}

/// Negative pin for path-sensitivity: stress-test exercising
/// `select<T>` across multiple call sites with mixed path selection.
/// Surfaces leak-detection failures under naive global-suppression.
/// Phase 5 verification step runs this with `ORI_CHECK_LEAKS=1`; this
/// AOT-success test pins the runtime crash today.
#[test]
fn test_path_sensitive_select_negative_pin_completes_without_uaf() {
    assert_aot_success(
        include_str!("fixtures/generics/path_sensitive_select_negative_pin.ori"),
        "path_sensitive_select_negative_pin",
    );
}

// Forwarder-result RL-2 release (compute_forwarder_result_under_release): a
// transfer-through-return forwarder RESULT whose monomorphized result-type burden is
// empty leaks its transferred-in allocation when borrow-consumed then dead, because the
// result lineage gets neither a FRESH inc nor a scope-exit dec. The positive pin clears
// the leak; the two negative pins clamp the over-emission boundary (gate c moved-onward,
// gate e FRESH-Construct owner) so the release never double-frees.

/// Positive pin (semantic): `Set<int>` from `@__collect_set` (a CALL RESULT, no
/// in-class `Construct` owner) through a forwarder, borrow-consumed then dead. The
/// forwarder-result release frees the 72-byte Set buffer; reverting it
/// (`ORI_DISABLE_FORWARDER_RESULT_RELEASE=1`) re-surfaces the leak.
#[test]
fn test_forwarder_result_collect_set_borrow_consumed_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/forwarder_result_collect_set_borrow_consumed.ori"),
        "forwarder_result_collect_set_borrow_consumed",
    );
}

/// Negative over-fire pin (gate c — alt-consumer): a forwarder result MOVED ONWARD into
/// a second forwarder must NOT get the release (the downstream lineage owns + releases
/// the allocation). Reverting gate c double-frees (exit-134).
#[test]
fn test_forwarder_result_collect_set_moved_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/generics/forwarder_result_collect_set_moved_no_double_free.ori"),
        "forwarder_result_collect_set_moved_no_double_free",
    );
}

/// Negative over-fire pin (gate e — FRESH-Construct owner): a `[int]` built by a
/// `Construct List` flows into a forwarder; the forwarder class owns its allocation via
/// the in-class Construct (retained release), so the forwarder-result release must NOT
/// fire. Reverting gate e double-frees (the imported-generic `host` shape, exit-134).
#[test]
fn test_forwarder_result_list_construct_owner_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/generics/forwarder_result_list_construct_owner_no_double_free.ori"),
        "forwarder_result_list_construct_owner_no_double_free",
    );
}

// BUG-04-090: edge cases
// Each cell pins a specific architectural decision the fix must
// thread between.

/// Param both returned AND used in a non-return statement.
/// `transfers_through_return` remains `true`, but the fix gates only
/// the SCOPE-EXIT dec, not all decs. Verifies the fix
/// doesn't suppress decs at non-return use sites.
#[test]
fn test_edge_param_returned_and_used_balances_rc() {
    assert_aot_success(
        include_str!("fixtures/generics/edge_param_returned_and_used.ori"),
        "edge_param_returned_and_used",
    );
}

/// Param flowing through Project — return value is `b.value`, NOT `b`.
/// `b` is NOT marked `transfers_through_return`; the existing scope-exit
/// dec on `b` is correct and must not be suppressed.
#[test]
fn test_edge_project_return_not_param_keeps_outer_dec() {
    assert_aot_success(
        include_str!("fixtures/generics/edge_project_return_not_param.ori"),
        "edge_project_return_not_param",
    );
}

/// TRMC tail-recursive forwarder. TRMC uses internal jumps, not Return
/// terminators, bypassing the param-return-alias rule. Must be
/// unaffected by the fix.
#[test]
fn test_edge_trmc_tail_recursive_unaffected_by_fix() {
    assert_aot_success(
        include_str!("fixtures/generics/edge_trmc_tail_recursive.ori"),
        "edge_trmc_tail_recursive",
    );
}

/// L8 AOT: prelude free fn `compare` shares the interned source
/// name of the Comparable method it calls. On the native AOT path the codegen
/// mono name+arg-types fallback must NOT self-target the builtin-method call
/// into an infinite branch; the receiver-scoped discriminator lets it fall
/// through to int's real `compare`. A cons-recursive TRMC fn coexists and must
/// still recurse (no over-fire). A regression makes the binary hang.
#[test]
fn test_trmc_prelude_compare_no_self_loop() {
    assert_aot_success(
        include_str!("fixtures/generics/trmc_prelude_compare_no_self_loop.ori"),
        "trmc_prelude_compare_no_self_loop",
    );
}

/// `for...yield` return. yield-style return uses Invoke + Resume
/// terminators; verify the gate does NOT suppress decs on Resume
/// blocks (`is_unwind_block` should stay true).
#[test]
fn test_edge_for_yield_return_preserves_unwind_decs() {
    assert_aot_success(
        include_str!("fixtures/generics/edge_for_yield_return.ori"),
        "edge_for_yield_return",
    );
}

/// `try{}` return. try-block returns may use Invoke terminators with
/// unwind paths; verify the gate doesn't suppress decs on the unwind
/// side.
#[test]
fn test_edge_try_block_return_preserves_unwind_decs() {
    assert_aot_success(
        include_str!("fixtures/generics/edge_try_block_return.ori"),
        "edge_try_block_return",
    );
}

/// Branch-merge of different params. For heap T: lowering produces
/// Branch+Jump+merge (NOT Select). Both x and y alias the merge-block's param via Jump-arg →
/// block-param edges; both must be marked `transfers_through_return=true`
/// (verifies multi-valued `alias_to_param` + Jump-arg → block-param
/// transitivity).
#[test]
fn test_edge_branch_merge_two_params_routes_via_jump_arg() {
    assert_aot_success(
        include_str!("fixtures/generics/edge_branch_merge_two_params.ori"),
        "edge_branch_merge_two_params",
    );
}

/// Param consumed by callee AND returned (load-bearing correctness
/// fix). x is consumed by `iter()` (Apply to Owned parameter) AND
/// returned. Verifies `transfers_through_return`
/// is gated on `return_flow_params`, NOT full `consumed_params`.
#[test]
fn test_edge_consumed_and_returned_uses_return_flow_only() {
    assert_aot_success(
        include_str!("fixtures/generics/edge_consumed_and_returned.ori"),
        "edge_consumed_and_returned",
    );
}

/// Param iter-consumed AND returned, str elements (heap element type). The
/// iter-consume/transfer-through-return overlap cure keeps one keep-alive inc
/// so the param's allocation (and its owned element strings) survive the
/// iterator drop and remain intact as the returned list. Type-matrix companion
/// to `test_edge_consumed_and_returned_uses_return_flow_only` ([int] elements):
/// str elements exercise the `elem_dec_fn` propagation path the int case does not.
#[test]
fn test_iter_then_return_str_elements_survive_iter_consume() {
    assert_aot_success(
        include_str!("fixtures/generics/iter_then_return_str.ori"),
        "iter_then_return_str",
    );
}

/// Function declaring `uses Suspend` capability returning its param.
/// Capability handlers lower to standard Return terminators; the
/// param-return-alias rule applies uniformly.
#[test]
#[ignore = "BUG-04-208: cross-function capability provision unimplemented; typeck E2014 rejects main calling uses-Suspend callee; can't reach BUG-04-090's AIMS RC surface until BUG-04-208 lands"]
fn test_edge_capability_handler_returns_param_uniformly() {
    assert_aot_success(
        include_str!("fixtures/generics/edge_capability_handler_returns_param.ori"),
        "edge_capability_handler_returns_param",
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

/// Inherent method on a generic type (`impl<T> Box<T> { @unwrap (self) -> T }`)
/// monomorphizes per concrete receiver and runs end-to-end through AOT. Pins the
/// borrow-passed-receiver-to-by-value-self path: when the caller holds the
/// receiver as a borrowed pointer (`Reference` ABI) but the mono'd method takes
/// `self` by value (`Direct` ABI), the call site loads the aggregate from the
/// source pointer rather than passing a zero placeholder.
#[test]
fn test_generic_method_on_generic_type() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_method_on_generic_type.ori"),
        "generic_method_on_generic_type",
    );
}

// Inherent method on a generic type — broadened coverage across receiver shape
// and method-body shape (int element type, the layout that monomorphizes
// end-to-end today).

/// Inherent method on a generic type called on two distinct `Box<int>`
/// receivers; both call sites share one monomorphized `unwrap` body.
#[test]
fn test_box_int_unwrap() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_method_box_int_unwrap.ori"),
        "generic_method_box_int_unwrap",
    );
}

/// Inherent method on a generic type taking a closure parameter (`f: T -> T`)
/// whose body carries an internal `let` binding; exercises body lowering of a
/// monomorphized method with both a closure-typed parameter and locals.
#[test]
fn test_holder_int_map_int() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_method_holder_int_internal_let.ori"),
        "generic_method_holder_int_internal_let",
    );
}

/// Inherent method on a generic type with a heap element type (`Box<str>`); the
/// monomorphized `unwrap` returns a fat-pointer `str` by value.
#[test]
fn test_box_str_unwrap() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_method_box_str_unwrap.ori"),
        "generic_method_box_str_unwrap",
    );
}

/// Inherent method on a generic type whose element is a collection (`Box<[int]>`);
/// the monomorphized `unwrap` returns a list fat pointer by value.
#[test]
fn test_box_list_int_unwrap() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_method_box_list_int_unwrap.ori"),
        "generic_method_box_list_int_unwrap",
    );
}

/// Inherent method on a generic type whose element is a user struct (`Box<Point>`);
/// the monomorphized `unwrap` returns an aggregate by value (sret path for >16B).
#[test]
fn test_box_struct_unwrap() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_method_box_struct_unwrap.ori"),
        "generic_method_box_struct_unwrap",
    );
}

/// Nested generic-receiver inherent methods: `Wrapper<Box<int>>.inner()` returns
/// the inner `Box<int>`, then `Box<int>.unwrap()` returns the int — two distinct
/// monomorphized inherent-method bodies on two distinct generic types in one call.
#[test]
fn test_wrapper_box_int_inner() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_method_wrapper_box_int_inner.ori"),
        "generic_method_wrapper_box_int_inner",
    );
}

/// Negative pin: a call to a zero-argument inherent method on a generic type
/// that passes an extra argument is rejected at type-check (clean arity error
/// E2004), never reaching codegen as a crash or silent miscompilation. Pins
/// that drifted method-call arguments are caught BEFORE the monomorphization /
/// codegen path, so the failure is a diagnostic and not an E5001 / SIGSEGV.
#[test]
fn test_box_method_with_drifted_args_emits_typeck_error() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/generics/generic_method_drifted_args_typeck_error.ori"
    ));

    // Compilation MUST fail at type-check. A zero exit would mean the arity
    // mismatch was accepted and a binary produced — a silent miscompilation.
    assert_eq!(
        exit_code, -1,
        "drifted-arg method call compiled cleanly (silent miscompilation); \
         stdout: {stdout}; stderr: {stderr}"
    );

    // The failure MUST be a clean type-check diagnostic, not an empty-stderr
    // codegen crash. An E5001 codegen abort or a signal kill on this input
    // would mean the arity check did not run before monomorphization.
    assert!(
        stderr.contains("E2004"),
        "expected arity diagnostic E2004 (expected 0 arguments, found 1); got: {stderr}"
    );
    assert!(
        !stderr.contains("E5001"),
        "drifted-arg call reached codegen (E5001) instead of being caught at type-check; \
         got: {stderr}"
    );
}

// BUG-04-111: (B-1) Return-as-transfer extensions
// Borrowed param + use(s) + Return. Verifies the new pre-compute set
// for borrow-flow scope-exit RcDec covers (B-1) shape across types
// and use patterns beyond the str-flavored snapshot (use_twice).

/// Regression: borrowed `[int]` param + 2 borrow uses + return. Extends the
/// str-flavored `use_twice` snapshot to `[int]` element type — verifies the
/// borrow-flow scope-exit `RcDec` elision is fixed for collection elements.
#[test]
fn test_borrow_list_int_use_twice_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_use_twice_then_return.ori"),
        "borrow_list_int_use_twice_then_return",
    );
}

/// Regression: borrowed `[int]` param + 3 borrow uses + return. N-use
/// boundary catches off-by-one in the pre-compute set for borrow-flow
/// values (`use_count` > 2).
#[test]
fn test_borrow_list_int_use_thrice_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_use_thrice_then_return.ori"),
        "borrow_list_int_use_thrice_then_return",
    );
}

/// Composition pin: borrowed `[int]` param + use + Return through a
/// `?`-Resume terminator path. Verifies the existing
/// `should_suppress_return_transfer_dec` gating still fires after the
/// new pre-compute set lands. NOT a true (B-1) case.
#[test]
fn test_borrow_list_int_resume_terminator_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_resume_terminator_then_return.ori"),
        "borrow_list_int_resume_terminator_then_return",
    );
}

/// Regression: borrowed `[int]` param + use in `if` arm + Return.
/// Branched control-flow within one branch — both arms independently
/// exercise Return-as-transfer.
#[test]
fn test_borrow_list_int_if_arm_use_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_if_arm_use_then_return.ori"),
        "borrow_list_int_if_arm_use_then_return",
    );
}

/// Regression: borrowed `[int]` param + use in `match` arm + Return.
/// Pattern-dispatch through Maranget decision tree — verifies the
/// pre-compute set covers Return-as-transfer across decision-tree paths.
#[test]
fn test_borrow_list_int_match_arm_use_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_match_arm_use_then_return.ori"),
        "borrow_list_int_match_arm_use_then_return",
    );
}

// BUG-04-111: (B-3) Borrow-only access
// Value used as borrow inside function-call boundaries; verifies the
// pre-compute set distinguishes borrow Applies (no dec at site) from
// Return-as-transfer (dec required at function exit).

/// Regression: borrowed `[int]` param + 1 borrow Apply + Return.
/// Single borrow access through a function-call boundary.
#[test]
fn test_borrow_list_int_one_borrow_apply_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_one_borrow_apply_then_return.ori"),
        "borrow_list_int_one_borrow_apply_then_return",
    );
}

/// Regression: borrowed `[int]` param + 2 borrow Applies + Return.
/// Multi-call borrow uses; catches off-by-one in borrow-Apply enumeration.
#[test]
fn test_borrow_list_int_two_borrow_applies_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_two_borrow_applies_then_return.ori"),
        "borrow_list_int_two_borrow_applies_then_return",
    );
}

/// Regression: borrowed `[int]` param + 1 borrow Apply + 1 owned Apply
/// (passthrough) + Return. Mixed access modes within one function body.
#[test]
fn test_borrow_list_int_mixed_apply_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_mixed_apply_then_return.ori"),
        "borrow_list_int_mixed_apply_then_return",
    );
}

// BUG-04-111: Cross-pattern cells
// Distinct control-flow shapes that exercise Return-as-transfer across
// genuinely uncovered patterns: Project consumer, depth-3 alias chain,
// loop-body Let-alias, multi-block CFG, for...yield iteration.

/// Regression: borrowed `[int]` param + Project consumer (index access)
/// + Return. Verifies Project access (`xs[0]`) does NOT consume the borrow.
#[test]
fn test_borrow_list_int_project_consumer_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_project_consumer_then_return.ori"),
        "borrow_list_int_project_consumer_then_return",
    );
}

/// Regression: borrowed `[int]` param + depth-3 SSA-alias chain + Return.
/// Alias-chain depth >= 3 generalizes the bug shape.
#[test]
fn test_borrow_list_int_multi_let_alias_chain_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_multi_let_alias_chain_then_return.ori"),
        "borrow_list_int_multi_let_alias_chain_then_return",
    );
}

/// Regression: borrowed `[int]` param + Let-alias inside `for` loop body +
/// Return. Verifies cross-iteration borrow tracking.
#[test]
fn test_borrow_list_int_loop_body_let_alias_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_loop_body_let_alias_then_return.ori"),
        "borrow_list_int_loop_body_let_alias_then_return",
    );
}

/// Regression: borrowed `[int]` param + 3-way nested branching + Return
/// from each leaf block. Multi-block CFG with 3+ exit paths.
#[test]
fn test_borrow_list_int_multi_block_cfg_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_multi_block_cfg_then_return.ori"),
        "borrow_list_int_multi_block_cfg_then_return",
    );
}

/// Regression: borrowed `[int]` param + for...yield iteration + Return
/// original param. Iterator-driven control flow over the borrow.
#[test]
fn test_borrow_list_int_for_yield_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_for_yield_then_return.ori"),
        "borrow_list_int_for_yield_then_return",
    );
}

// Live-across borrowed-collection family — a transfer-through-return Owned param
// that is ALSO iter-consumed (for...yield) or multi-hop read-aliased inside the
// body. One allocation, two terminal consumes (iter-free / scope-read + Return
// transfer): the iter-consume is an RL-1 duplication needing one BurdenInc, and
// a multi-hop read-alias is transparent (no spurious dec). Positive cross-type
// pins + over-fire negatives (consumed-but-NOT-returned must keep status quo).

/// Regression: for...yield iter-consume over a heap-element `[str]` param +
/// Return the param. The iterator frees the param buffer; the duplication inc
/// (RL-1) leaves the original reference for the Return transfer. Cross-type
/// sibling of `test_borrow_list_int_for_yield_then_return_no_leak`.
#[test]
fn test_borrow_list_str_for_yield_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_str_for_yield_then_return.ori"),
        "borrow_list_str_for_yield_then_return",
    );
}

/// Regression: loop-body multi-hop Let-alias of a heap-element `[str]` param +
/// Return the param. The deepest alias is the same allocation the Return
/// transfers out — it carries no spurious last-use dec. Cross-type sibling of
/// `test_borrow_list_int_loop_body_let_alias_then_return_no_leak`.
#[test]
fn test_borrow_list_str_loop_body_let_alias_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_str_loop_body_let_alias_then_return.ori"),
        "borrow_list_str_loop_body_let_alias_then_return",
    );
}

/// Over-fire negative: for...yield iter-consume of a `[int]` param whose result
/// is NOT returned — the param dies at the iter-consume (single transfer). The
/// duplication-inc cure MUST NOT fire here (the `ori_iter_drop` is the sole
/// release; an extra inc would leak). Mutation-verified guard.
#[test]
fn test_borrow_list_int_for_yield_consume_only_no_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_for_yield_consume_only_no_return.ori"),
        "borrow_list_int_for_yield_consume_only_no_return",
    );
}

/// Over-fire negative: loop-body multi-hop Let-alias of a `[int]` param that is
/// borrow-read but NOT returned — the param's release stays the scope-exit dec.
/// The multi-hop transparent-alias treatment MUST NOT suppress it (would leak).
#[test]
fn test_borrow_list_int_loop_body_let_alias_no_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_loop_body_let_alias_no_return.ori"),
        "borrow_list_int_loop_body_let_alias_no_return",
    );
}

// BUG-04-111: Canonical-rep semantics pins
// Positive pins exercising post-fix canonical-rep selection logic.
// May not be RED at HEAD (they verify NEW semantics introduced by the
// fix); pre-fix passing is acceptable for these.

/// Positive pin: `canonical_rep_for` picks the LATEST-emitting member
/// when multiple SSA alias class members exist.
#[test]
fn test_borrow_list_int_latest_emission_site_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_latest_emission_site_then_return.ori"),
        "borrow_list_int_latest_emission_site_then_return",
    );
}

/// Positive pin: canonical-rep selection deterministic across repeated
/// invocations (Salsa determinism contract).
#[test]
fn test_borrow_list_int_determinism_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_determinism_then_return.ori"),
        "borrow_list_int_determinism_then_return",
    );
}

/// Positive pin: bypass-safe interaction — `canonical_rep` returns the
/// actual bypass-safe Let-alias rep when a class has bypass-safe-entry
/// members.
#[test]
fn test_borrow_list_int_bypass_safe_interaction_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_bypass_safe_interaction_then_return.ori"),
        "borrow_list_int_bypass_safe_interaction_then_return",
    );
}

// BUG-04-111: (A)-shape new pins
// Positive pins exercising post-fix PIN-6 chain semantics.

/// Positive pin: 3-alias merge at CFG join — verifies
/// post-fix expanded input set doesn't break PIN-6 chain resolution
/// when the SSA alias class has 3 distinct members merging.
#[test]
fn test_borrow_list_int_three_alias_merge_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_three_alias_merge_then_return.ori"),
        "borrow_list_int_three_alias_merge_then_return",
    );
}

/// Positive pin: nested PIN-6 chain
/// B→A→C — ancestor class B's transitive-drop covers class A's slot,
/// in turn covered by ancestor class C. Verifies `pin6_same_emission_covers`
/// ancestor-walk loop survives the expanded input.
#[test]
fn test_borrow_list_int_nested_pin6_chain_then_return_no_leak() {
    assert_aot_success(
        include_str!("fixtures/generics/borrow_list_int_nested_pin6_chain_then_return.ori"),
        "borrow_list_int_nested_pin6_chain_then_return",
    );
}

// As-compiled impl-method contract pins
// Caller-side `transfers_through_return` binding for impl-method callees.

/// Negative over-fire pin: a CONDITIONAL impl-method forwarder (both params
/// may flow to the return) must not double-free under the caller-side
/// as-compiled contract binding — the returned Wrapper is released once,
/// the non-returned one by its own path release.
#[test]
fn test_impl_method_conditional_forwarder_no_double_free() {
    assert_aot_success(
        include_str!("fixtures/generics/impl_method_conditional_forwarder_no_double_free.ori"),
        "impl_method_conditional_forwarder_no_double_free",
    );
}

// Forwarder-identity alias transparency
// A `Let { Var }` alias of a transfers-through-return Owned param is a
// same-allocation view of the moved-through value, not an RL-1 duplication.
// Classifying it emits a paired alias inc/dec the per-var DP-2/DP-3
// elimination splits (inc elided, dec kept) — an over-release double-free.
// Toggle: ORI_DISABLE_FORWARDER_IDENTITY_ALIAS_DEDUP=1 restores the
// classification (the double-free returns — mutation-verified).

/// Multi-use-then-return impl forwarder: `self.items.len()` borrow-read plus
/// `if .. then self else self` — the branch aliases of the moved-through
/// `self` must carry no RC pair of their own; the allocation is released
/// exactly once by the caller of the bound result.
#[test]
fn test_impl_method_multi_use_then_return_forwarder_releases_once() {
    assert_aot_success(
        include_str!("fixtures/generics/impl_method_multi_use_forwarder.ori"),
        "impl_method_multi_use_forwarder",
    );
}

/// Free-function twin of the multi-use-then-return forwarder: the defect is
/// contract-independent (the real interprocedural contract already proved
/// `transfers_through_return`, yet the alias classification ignored it), so
/// the transparency rule must fire uniformly on free functions.
#[test]
fn test_free_fn_multi_use_then_return_forwarder_releases_once() {
    assert_aot_success(
        include_str!("fixtures/generics/free_fn_multi_use_forwarder.ori"),
        "free_fn_multi_use_forwarder",
    );
}

// Genuine-duplication pair coupling
// A ttr-param alias consumed at an owned Construct position while the source
// is read after the store and returned on BOTH arms is a GENUINE RL-1
// duplication — the store creates a real second reference. The pair is
// atomic: the alias-site inc backs the container's release (kept even though
// the alias's own dec is transfer-suppressed), and the per-var DP-2/DP-3
// elimination must never split a param-rooted alias pair (inc elided per the
// alias's Once state, dec kept — net -1 over-release).
// Toggle: ORI_DISABLE_GENUINE_DUP_PAIR_COUPLING=1 restores the decoupled
// treatment (the double-free returns — mutation-verified).

/// Stash-and-return: `let a = w; Holder { kept: a }` stores the duplicate
/// while `w.items` is read after the store and `w` returns on both arms.
/// The allocation must be released exactly twice across both owners — once
/// by the Holder's drop, once by the caller of the returned value.
#[test]
fn test_genuine_dup_stash_and_return_releases_each_owner_once() {
    assert_aot_success(
        include_str!("fixtures/generics/stash_and_return_genuine_dup.ori"),
        "stash_and_return_genuine_dup",
    );
}

// A generic body monomorphized over the builtin niche-scalar
// `Ordering` sum constructs and projects/matches an `Ordering` variant.
// `Ordering` lowers to a scalar `i8` but `Tag::Ordering` is a primitive sum
// with no tagged-ptr/tagless/niche-encoded branch, so the
// `CtorKind::EnumVariant` emitter fell through to the explicit-tag struct path
// (`insert_value on non-struct value (index 0)`, construct side) and the
// `Project { field: 0 }` tag read fell through to `extract_value` on the `i8`
// (`extract_value on non-struct value (index 0)`, project/match side). The
// interpreter passed; LLVM failed the whole module. The cure is a scalar-int
// LLVM-type guard at both emission sites.

/// Regression: a generic body over the builtin niche-scalar `Ordering` sum
/// constructs an `Ordering` (`cmp<T>` body), projects its tag through a `match`
/// (`classify`), and calls the `is_less`/`is_equal`/`is_greater` predicates on
/// the result. Pre-fix this hit `insert_value` (construct) and `extract_value`
/// (project) on the scalar `i8`; the scalar-int-LLVM-type guards in
/// `emit_construct` and `emit_project` emit / read the `i8` discriminant
/// directly (Less=0/Equal=1/Greater=2).
#[test]
fn test_generic_body_ordering_scalar_construct_and_match_returns_correct_ordering() {
    assert_aot_success(
        include_str!("fixtures/generics/generic_body_ordering_scalar_construct.ori"),
        "generic_body_ordering_scalar_construct",
    );
}

/// Guard-ordering pin: a tagged-pointer enum lowers to a bare
/// LLVM `i64`, so `is_scalar_int_type` is TRUE for it. The scalar-int guard in
/// `emit_construct` (and the symmetric one in `emit_project`) MUST fire AFTER
/// the tagged-ptr check, never before — otherwise the constructor emits a bare
/// `const_int(i64, variant)` and drops the `(payload | tag)` pointer bits,
/// losing the iterator payload. Constructs `Holds(it: ...)` (Construct side)
/// and matches the payload back out + consumes it (Project side); the program
/// returns 0 ONLY when the payload pointer survived both emission sites. Fails
/// on the pre-reorder (scalar-guard-first) code; passes after the re-order.
#[test]
fn test_tagged_ptr_construct_project_payload_survives() {
    assert_aot_success(
        include_str!("fixtures/generics/tagged_ptr_construct_project_payload_survives.ori"),
        "tagged_ptr_construct_project_payload_survives",
    );
}

/// Negative pin: the scalar-int guard must fire ONLY on
/// scalar-int dst (Ordering). Generic bodies constructing aggregate sum
/// variants — a user-defined `Triple`, `Option`, and `Result` — must keep the
/// tagged-struct construction path and stay green (the guard must NOT fire on
/// an aggregate dst).
#[test]
fn test_imported_generic_aggregate_construct_no_regression() {
    assert_aot_success(
        include_str!("fixtures/generics/imported_generic_aggregate_construct_no_regression.ori"),
        "imported_generic_aggregate_construct_no_regression",
    );
}

// Local terminal-move store
// A fresh local read once (borrow projection) then STORED into a Holder as
// its terminal use. The store IS the move — the Holder's release is the
// single release of the whole tree; the read-alias pair must never be split
// by the per-var elimination (inc elided per the alias's Once state, dec
// kept — net -1, an over-release double-free on top of the transfer).
// Toggle: ORI_DISABLE_LOCAL_CONSTRUCT_PAIR_COUPLING=1 restores the split
// (the double-free returns — mutation-verified).

/// Read-then-store: `let $n = w.items.len(); Holder { kept: w }` — the
/// borrow read's alias pair stays whole, the store transfers the original
/// reference, and the holder's drop releases the tree exactly once.
#[test]
fn test_local_terminal_move_store_releases_once() {
    assert_aot_success(
        include_str!("fixtures/generics/local_terminal_move_store.ori"),
        "local_terminal_move_store",
    );
}
