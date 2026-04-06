//! Higher-Order Function AOT Tests
//!
//! Tests for functions as arguments, return values, closures with
//! captures, closure composition, multi-parameter HOFs, function
//! chaining patterns, and callbacks in loops.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_capture_ir};

// ─── Functions as arguments ───

#[test]
fn test_hof_apply_identity() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_apply_identity.ori"),
        "hof_apply_identity",
    );
}

#[test]
fn test_hof_apply_named_function() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_apply_named_function.ori"),
        "hof_apply_named",
    );
}

#[test]
fn test_hof_apply_lambda() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_apply_lambda.ori"),
        "hof_apply_lambda",
    );
}

#[test]
fn test_hof_two_function_args() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_two_function_args.ori"),
        "hof_two_fn_args",
    );
}

#[test]
fn test_hof_apply_twice() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_apply_twice.ori"),
        "hof_apply_twice",
    );
}

// ─── Functions returning functions ───

#[test]
fn test_hof_make_adder() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_make_adder.ori"),
        "hof_make_adder",
    );
}

#[test]
fn test_hof_make_multiplier() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_make_multiplier.ori"),
        "hof_make_multiplier",
    );
}

#[test]
fn test_hof_make_predicate() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_make_predicate.ori"),
        "hof_make_predicate",
    );
}

// ─── Composition ───

#[test]
fn test_hof_compose() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_compose.ori"),
        "hof_compose",
    );
}

#[test]
fn test_hof_compose_reverse_order() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_compose_reverse_order.ori"),
        "hof_compose_rev",
    );
}

#[test]
fn test_hof_pipeline_three() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_pipeline_three.ori"),
        "hof_pipe3",
    );
}

// ─── Closures with captures ───

#[test]
fn test_hof_closure_capture_computation() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_closure_capture_computation.ori"),
        "hof_closure_compute",
    );
}

#[test]
fn test_hof_closure_capture_in_loop() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_closure_capture_in_loop.ori"),
        "hof_closure_in_loop",
    );
}

#[test]
fn test_hof_closure_bool_capture() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_closure_bool_capture.ori"),
        "hof_closure_bool",
    );
}

// ─── Callbacks ───

#[test]
fn test_hof_callback_pattern() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_callback_pattern.ori"),
        "hof_callback",
    );
}

#[test]
fn test_hof_predicate_filter_count() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_predicate_filter_count.ori"),
        "hof_pred_filter",
    );
}

// ─── Multiple return closure usage ───

#[test]
fn test_hof_two_closures_same_capture() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_two_closures_same_capture.ori"),
        "hof_two_closures",
    );
}

// ─── Nested closures ───

#[test]
fn test_hof_nested_closure_deep() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_nested_closure_deep.ori"),
        "hof_nested_deep",
    );
}

// ─── Named functions as values ───

#[test]
fn test_hof_named_fn_as_arg() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_named_fn_as_arg.ori"),
        "hof_named_fn_arg",
    );
}

// ─── Closure in conditional ───

#[test]
fn test_hof_closure_conditional_select() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_closure_conditional_select.ori"),
        "hof_cond_select",
    );
}

// ─── Accumulator with HOF ───

#[test]
fn test_hof_fold_manual() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_fold_manual.ori"),
        "hof_fold_manual",
    );
}

#[test]
fn test_hof_fold_product() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_fold_product.ori"),
        "hof_fold_product",
    );
}

// ─── Chained closures ───

#[test]
fn test_hof_chain_closures() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_chain_closures.ori"),
        "hof_chain_closures",
    );
}

// ─── Closure with complex capture ───

#[test]
fn test_hof_closure_capture_from_function() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_closure_capture_from_function.ori"),
        "hof_capture_from_fn",
    );
}

// ─── Lambda returning bool ───

#[test]
fn test_hof_bool_lambda() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_bool_lambda.ori"),
        "hof_bool_lambda",
    );
}

// ─── Multi-param lambda ───

#[test]
fn test_hof_multi_param_lambda() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_multi_param_lambda.ori"),
        "hof_multi_param",
    );
}

// ─── Closure return value chaining ───

#[test]
fn test_hof_closure_return_chain() {
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_closure_return_chain.ori"),
        "hof_closure_return_chain",
    );
}

// ─── C1: Mixed closure types across functions ───
// Bug: lambda names are per-function (`__lambda_0` in each), but stored
// in a global HashMap. Lambdas in different functions collide.

#[test]
fn test_hof_cross_function_lambda_collision() {
    // C1 bug: lambda in @main collides with lambda in @make_adder
    // Both produce __lambda_0 → HashMap collision → wrong calling convention
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_cross_function_lambda_collision.ori"),
        "hof_cross_function_collision",
    );
}

#[test]
fn test_hof_journey5_reproduction() {
    // Exact Journey 5 reproduction — AOT should compute 27
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_journey5_reproduction.ori"),
        "hof_journey5",
    );
}

#[test]
fn test_hof_multiple_functions_with_lambdas() {
    // Three separate functions each containing a lambda
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_multiple_functions_with_lambdas.ori"),
        "hof_multiple_fn_lambdas",
    );
}

// ─── Root Cause E: multi-lambda arity/type selection ───

#[test]
fn test_hof_multi_lambda_different_arities() {
    // Two lambdas with different arities in the same function.
    // Regression: find_concrete_copy_of must not pick a 2-param type for a 1-param lambda.
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_multi_lambda_different_arities.ori"),
        "hof_multi_lambda_diff_arities",
    );
}

#[test]
fn test_hof_multi_lambda_same_arity_diff_types() {
    // Two lambdas with same arity but different param types.
    // Regression: find_concrete_copy_of must match type, not just arity.
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_multi_lambda_same_arity_diff_types.ori"),
        "hof_multi_lambda_same_arity",
    );
}

#[test]
fn test_hof_three_lambdas_mixed() {
    // Three lambdas (1-param, 2-param, 3-param) in one function.
    // Regression: type selection handles 3+ lambdas without cross-contamination.
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_three_lambdas_mixed.ori"),
        "hof_three_lambdas_mixed",
    );
}

// ─── Root Cause F: curried concat calling convention ───

#[test]
#[ignore = "BUG-04-036: curried lambda + list concat COW double-free"]
fn test_hof_curried_list_concat() {
    // Curried lambda with list `+` (ori_list_concat_cow).
    // Regression: wrong type selection caused invalid elem_ty → SIGSEGV.
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_curried_list_concat.ori"),
        "hof_curried_list_concat",
    );
}

#[test]
fn test_hof_curried_str_concat() {
    // Curried lambda with string `+` (sret calling convention).
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_curried_str_concat.ori"),
        "hof_curried_str_concat",
    );
}

#[test]
fn test_hof_multi_lambda_semantic_pin() {
    // Semantic pin: negate (unary) and diff (binary) must each get the correct
    // concrete type. If arity-blind type selection picks the wrong one, this fails.
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_multi_lambda_semantic_pin.ori"),
        "hof_multi_lambda_semantic_pin",
    );
}

#[test]
fn test_hof_two_noncapturing_in_different_functions() {
    // Non-capturing lambdas in different functions — tests name collision
    // even when both are non-capturing (same phantom env convention)
    assert_aot_success(
        include_str!("fixtures/higher_order/hof_two_noncapturing_in_different_functions.ori"),
        "hof_two_noncapturing_diff_fns",
    );
}

// Closure captures of non-scalar (fat pointer) types.
//
// These tests verify that lambdas capturing heap-allocated values (str, [T])
// have correct RC management. The root cause of the bug was that
// declare_and_process_lambda() did not apply AIMS param ownership to lambda
// params before running the ARC pipeline, causing collect_all_borrowed_defs()
// to miss borrowed params and their Let aliases. Edge cleanup then emitted
// spurious RcDec for borrowed-param aliases, causing double-free.

/// Closure capturing a heap-allocated str (>23 bytes, exceeds SSO).
/// Without the fix, `ori_rc_dec` is called on already-freed allocation.
#[test]
fn test_closure_capture_heap_str() {
    assert_aot_success(
        include_str!("fixtures/higher_order/closure_capture_heap_str.ori"),
        "closure_capture_heap_str",
    );
}

/// Closure capturing `[int]` and calling `.length()`.
#[test]
fn test_closure_capture_list() {
    assert_aot_success(
        include_str!("fixtures/higher_order/closure_capture_list.ori"),
        "closure_capture_list",
    );
}

/// Closure with str capture and a str parameter — the J17 pattern.
#[test]
fn test_closure_capture_str_with_param() {
    assert_aot_success(
        include_str!("fixtures/higher_order/closure_capture_str_with_param.ori"),
        "closure_capture_str_with_param",
    );
}

/// Closure passed as argument with a heap str capture — higher-order with
/// fat pointer capture.
#[test]
fn test_closure_passed_with_str_capture() {
    assert_aot_success(
        include_str!("fixtures/higher_order/closure_passed_with_str_capture.ori"),
        "closure_passed_with_str_capture",
    );
}

/// Multiple non-scalar captures (str + [int]).
#[test]
fn test_closure_multi_capture() {
    assert_aot_success(
        include_str!("fixtures/higher_order/closure_multi_capture.ori"),
        "closure_multi_capture",
    );
}

/// Closure capturing another closure and calling it.
///
/// The outer closure's env holds `{ drop_fn, inner_closure }` where
/// `inner_closure` is `{ fn_ptr, env_ptr }`. The env drop function
/// must extract `env_ptr` from the inner closure, not pass the whole
/// `{ ptr, ptr }` to `ori_rc_dec`. Semantic pin for closure-in-closure RC.
#[test]
fn test_closure_capturing_closure() {
    assert_aot_success(
        include_str!("fixtures/higher_order/closure_capturing_closure.ori"),
        "closure_capturing_closure",
    );
}

/// Nested closures — outer captures str, inner captures outer's captured str.
#[test]
fn test_nested_closure_fat_capture() {
    assert_aot_success(
        include_str!("fixtures/higher_order/nested_closure_fat_capture.ori"),
        "nested_closure_fat_capture",
    );
}

/// Closure returned from a function with a fat pointer capture.
#[test]
fn test_closure_returned_from_function() {
    assert_aot_success(
        r#"
@make_greeter (greeting: str) -> (str) -> str = {
    name -> `{greeting}, {name}!`
}

@main () -> int = {
    let $greet = make_greeter(greeting: "Hello");
    let $result = greet("world");
    if result == "Hello, world!" then 0 else 1
}
"#,
        "closure_returned_from_function",
    );
}

/// Closure capturing `Option<str>` and pattern matching on it.
#[test]
fn test_closure_capturing_option_str_match() {
    assert_aot_success(
        include_str!("fixtures/higher_order/closure_capturing_option_str_match.ori"),
        "closure_capturing_option_str_match",
    );
}

// Nested closure RC matrix — covers double-free regression from wrapper
// RcInc fix. Every test exercises a different type through the nested-capture
// path: outer closure captures a value, inner closure re-captures it.

/// Semantic pin: nested closure with str capture (the exact pattern that caused
/// the double-free). Without the wrapper `RcInc` fix, this crashes with
/// "`ori_rc_dec` called on already-freed allocation".
#[test]
fn test_nested_closure_str_semantic_pin() {
    assert_aot_success(
        include_str!("fixtures/higher_order/nested_closure_str_semantic_pin.ori"),
        "nested_closure_str_semantic_pin",
    );
}

/// Nested closure capturing a list (fat pointer, RC-tracked).
#[test]
fn test_nested_closure_list_capture() {
    assert_aot_success(
        include_str!("fixtures/higher_order/nested_closure_list_capture.ori"),
        "nested_closure_list_capture",
    );
}

/// Nested closure capturing a closure (closure-in-closure-in-closure).
#[test]
fn test_nested_closure_closure_capture() {
    assert_aot_success(
        include_str!("fixtures/higher_order/nested_closure_closure_capture.ori"),
        "nested_closure_closure_capture",
    );
}

/// Nested closure with multiple captures (str + int).
#[test]
fn test_nested_closure_multi_capture() {
    assert_aot_success(
        include_str!("fixtures/higher_order/nested_closure_multi_capture.ori"),
        "nested_closure_multi_capture",
    );
}

/// Three levels of nested closure capture (outer → middle → inner).
#[test]
fn test_triple_nested_closure_capture() {
    assert_aot_success(
        include_str!("fixtures/higher_order/triple_nested_closure_capture.ori"),
        "triple_nested_closure_capture",
    );
}

/// Semantic pin: nested closure re-captures a borrowed `str` parameter.
/// The outer function receives `s` as a parameter (borrowed), the outer closure
/// captures it, and the inner closure re-captures it. This exercises the
/// `lambda_capture_ownership` path for borrowed-vs-owned capture handling
/// (`define_phase.rs`, `context.rs`, `closures.rs`).
#[test]
fn test_nested_closure_borrowed_str_param() {
    assert_aot_success(
        include_str!("fixtures/higher_order/nested_closure_borrowed_str_param.ori"),
        "nested_closure_borrowed_str_param",
    );
}

/// Nested closure re-captures a borrowed `[int]` parameter — second RC-managed
/// type through the same borrowed-parameter re-capture path.
#[test]
fn test_nested_closure_borrowed_list_param() {
    assert_aot_success(
        include_str!("fixtures/higher_order/nested_closure_borrowed_list_param.ori"),
        "nested_closure_borrowed_list_param",
    );
}

// ─── Multi-instantiation lambda (TPR-04B-007 regression) ───

/// Semantic pin: zero-arg polymorphic lambda returning None, instantiated at
/// Option<int> and Option<str>. The original generic lambda must be removed
/// after cloning — only specialized clones should be compiled.
#[test]
fn test_multi_inst_none_lambda() {
    assert_aot_success(
        include_str!("fixtures/higher_order/multi_inst_none_lambda.ori"),
        "multi_inst_none_lambda",
    );
}

/// Semantic pin: unary polymorphic lambda wrapping a value in Some, instantiated
/// at int and bool. Verifies both specializations produce correct results.
#[test]
fn test_multi_inst_wrap_lambda() {
    assert_aot_success(
        include_str!("fixtures/higher_order/multi_inst_wrap_lambda.ori"),
        "multi_inst_wrap_lambda",
    );
}

/// Semantic pin: TPR-05-001 — polymorphic lambda returning a tuple, instantiated
/// at int and str. Exercises the `Tag::Tuple` branches in `type_predicates.rs`
/// (`contains_var`, `contains_bound_var`, `map_types_structural`).
#[test]
fn test_multi_inst_tuple_lambda() {
    assert_aot_success(
        include_str!("fixtures/higher_order/multi_inst_tuple_lambda.ori"),
        "multi_inst_tuple_lambda",
    );
}

/// Semantic pin: TPR-05-001 — polymorphic lambda returning a map, instantiated
/// at int and str value types. Exercises the `Tag::Map` branches in `type_predicates.rs`
/// (`contains_var`, `contains_bound_var`, `map_types_structural`).
#[test]
fn test_multi_inst_map_lambda() {
    assert_aot_success(
        include_str!("fixtures/higher_order/multi_inst_map_lambda.ori"),
        "multi_inst_map_lambda",
    );
}

/// Negative pin for TPR-04B-007: verify that multi-inst originals are NOT
/// present in the emitted LLVM IR. Only specialized clones (with `$` suffix)
/// should survive. The stale original would contain unresolved type variables
/// and produce `unresolved type variable at codegen` errors.
#[test]
fn test_multi_inst_no_stale_original_in_ir() {
    let ir = compile_and_capture_ir(include_str!(
        "fixtures/higher_order/multi_inst_none_lambda.ori"
    ));

    // The original generic lambda should NOT be in the IR.
    // Only specialized clones with `$NNN` suffixes should exist.
    let has_original = ir
        .lines()
        .any(|l| l.starts_with("define ") && l.contains("@_ori___lambda_main_0("));
    assert!(
        !has_original,
        "stale original multi-inst lambda found in IR — \
         should have been removed after cloning. \
         IR functions:\n{}",
        ir.lines()
            .filter(|l| l.starts_with("define "))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // Verify specialized clones DO exist (with __monoN suffix).
    let has_clone = ir
        .lines()
        .any(|l| l.starts_with("define ") && l.contains("@_ori___lambda_main_0__mono"));
    assert!(
        has_clone,
        "no specialized lambda clones found in IR — \
         expected _ori___lambda_main_0__monoN functions"
    );

    // Verify no "unresolved type variable" error in compilation output.
    assert!(
        !ir.contains("unresolved type variable"),
        "compilation produced 'unresolved type variable' error — \
         stale original lambda was compiled"
    );

    // Negative pin for TPR-04B-009: verify no "callee not found" warnings.
    // The original PartialApply instruction must be removed alongside the
    // original lambda function, otherwise the emitter falls back to a null
    // closure for the stale callee reference.
    assert!(
        !ir.contains("callee not found"),
        "compilation produced 'callee not found' warning — \
         stale PartialApply instruction survived rewriting"
    );
}
