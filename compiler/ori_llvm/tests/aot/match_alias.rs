//! SSA-alias dec-dedup matrix tests (BUG-04-104).
//!
//! Tests for §3.1 in-class alias cells and §3.3 semantic-pin cells from
//! the BUG-04-104 fix plan TDD matrix. Pre-fix: AIMS realize emits RcDec
//! per SSA name, causing double-free on aliased aggregates with heap
//! children. Post-fix: SSA-alias equivalence-class union-find collapses
//! dec emission to one canonical dec per class.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

#[test]
fn test_match_arm_alias_result_str() {
    assert_aot_success(
        include_str!("fixtures/match_alias/match_arm_alias_result_str.ori"),
        "match_arm_alias_result_str",
    );
}

#[test]
fn test_let_alias_struct_str() {
    assert_aot_success(
        include_str!("fixtures/match_alias/let_alias_struct_str.ori"),
        "let_alias_struct_str",
    );
}

#[test]
fn test_unwind_path_alias() {
    assert_aot_success(
        include_str!("fixtures/match_alias/unwind_path_alias.ori"),
        "unwind_path_alias",
    );
}

#[test]
fn test_match_arm_alias_option_str() {
    assert_aot_success(
        include_str!("fixtures/match_alias/match_arm_alias_option_str.ori"),
        "match_arm_alias_option_str",
    );
}

// Section §3.1 in-class cells — additional aggregate × heap-child × alias × CFG combinations

#[test]
fn test_let_alias_struct_intlist() {
    assert_aot_success(
        include_str!("fixtures/match_alias/let_alias_struct_intlist.ori"),
        "let_alias_struct_intlist",
    );
}

#[test]
fn test_jumparg_alias_loop_break() {
    assert_aot_success(
        include_str!("fixtures/match_alias/jumparg_alias_loop_break.ori"),
        "jumparg_alias_loop_break",
    );
}

#[test]
fn test_apply_alias_result_strmap() {
    assert_aot_success(
        include_str!("fixtures/match_alias/apply_alias_result_strmap.ori"),
        "apply_alias_result_strmap",
    );
}

#[test]
fn test_multivariant_match_alias() {
    assert_aot_success(
        include_str!("fixtures/match_alias/multivariant_match_alias.ori"),
        "multivariant_match_alias",
    );
}

#[test]
fn test_closure_env_alias() {
    assert_aot_success(
        include_str!("fixtures/match_alias/closure_env_alias.ori"),
        "closure_env_alias",
    );
}

#[test]
fn test_nested_result_option_alias() {
    assert_aot_success(
        include_str!("fixtures/match_alias/nested_result_option_alias.ori"),
        "nested_result_option_alias",
    );
}

#[test]
fn test_set_value_alias() {
    assert_aot_success(
        include_str!("fixtures/match_alias/set_value_alias.ori"),
        "set_value_alias",
    );
}

#[test]
fn test_let_alias_filter_protected() {
    assert_aot_success(
        include_str!("fixtures/match_alias/let_alias_filter_protected.ori"),
        "let_alias_filter_protected",
    );
}

#[test]
fn test_result_str_letvar_forloop_over_alias() {
    assert_aot_success(
        include_str!("fixtures/match_alias/result_str_letvar_forloop_over_alias.ori"),
        "result_str_letvar_forloop_over_alias",
    );
}

#[test]
fn test_result_str_jumparg_apply_owned_param() {
    assert_aot_success(
        include_str!("fixtures/match_alias/result_str_jumparg_apply_owned_param.ori"),
        "result_str_jumparg_apply_owned_param",
    );
}

// Section §3.3 semantic-pin cells — preservation guard for Select-independence per PIN-2

#[test]
fn test_option_intlist_select_branch_return() {
    assert_aot_success(
        include_str!("fixtures/match_alias/option_intlist_select_branch_return.ori"),
        "option_intlist_select_branch_return",
    );
}

// BUG-04-106 — closure_env_alias spurious-RcInc fix
//
// Regression: closure capturing `Result<int, str>::Err(string)` invoked
// twice via `lookup()` previously leaked 28 + 40 bytes under
// `ORI_CHECK_LEAKS=1`. AIMS realize emitted a spurious `RcInc` on the
// closure receiver between the two `ApplyIndirect` calls, leaving the
// closure RC at 1 + 1 - 1 = 1 (instead of 0) when the function exited —
// the LastUse `RcDec` freed only one reference, leaking the underlying
// allocation. Post-fix: realize-layer borrow-recognition for ApplyIndirect
// closure receivers (§05 edit sites 1-4) suppresses the spurious Inc;
// LastUse Dec frees the allocation; zero leaks. `assert_aot_success`
// already enables `ORI_CHECK_LEAKS=1`, so a regression in any of the
// realize-layer fixes resurfaces as exit code 2 (leak detected).

#[test]
fn test_closure_env_alias_no_leak() {
    assert_aot_success(
        include_str!("fixtures/match_alias/closure_env_alias.ori"),
        "closure_env_alias_no_leak",
    );
}

// BUG-04-106 §03:37 + §03:44-45 — direct-call closure matrix.
// Each fixture invokes a closure capturing an RC-tracked value 2-3 times
// via direct `lookup()` calls (NOT via iterator pipeline). The same-block
// multi-call ApplyIndirect pattern is exactly what triggers BUG-04-106's
// spurious RcInc on the closure receiver. `assert_aot_success` enables
// `ORI_CHECK_LEAKS=1` — pre-fix all three leak (closure env + captured value);
// post-fix zero leaks.

#[test]
fn test_closure_three_call_no_leak() {
    assert_aot_success(
        include_str!("fixtures/match_alias/closure_three_call_no_leak.ori"),
        "closure_three_call_no_leak",
    );
}

#[test]
fn test_closure_str_capture_two_call_no_leak() {
    assert_aot_success(
        include_str!("fixtures/match_alias/closure_str_capture_two_call_no_leak.ori"),
        "closure_str_capture_two_call_no_leak",
    );
}

#[test]
fn test_closure_list_capture_two_call_no_leak() {
    assert_aot_success(
        include_str!("fixtures/match_alias/closure_list_capture_two_call_no_leak.ori"),
        "closure_list_capture_two_call_no_leak",
    );
}

#[test]
fn test_closure_single_call_no_regression() {
    // Stays-green pin: pre-fix already passes (single use); post-fix
    // MUST continue to pass — fix must not break the simple case.
    assert_aot_success(
        include_str!("fixtures/match_alias/closure_single_call_no_regression.ori"),
        "closure_single_call_no_regression",
    );
}

#[test]
fn test_closure_zero_capture_two_call_no_leak() {
    assert_aot_success(
        include_str!("fixtures/match_alias/closure_zero_capture_two_call_no_leak.ori"),
        "closure_zero_capture_two_call_no_leak",
    );
}

#[test]
fn test_closure_option_str_capture_two_call_no_leak() {
    assert_aot_success(
        include_str!("fixtures/match_alias/closure_option_str_capture_two_call_no_leak.ori"),
        "closure_option_str_capture_two_call_no_leak",
    );
}

#[test]
fn test_closure_struct_capture_two_call_no_leak() {
    assert_aot_success(
        include_str!("fixtures/match_alias/closure_struct_capture_two_call_no_leak.ori"),
        "closure_struct_capture_two_call_no_leak",
    );
}

#[test]
fn test_closure_nested_closure_capture_two_call_no_leak() {
    assert_aot_success(
        include_str!("fixtures/match_alias/closure_nested_closure_capture_two_call_no_leak.ori"),
        "closure_nested_closure_capture_two_call_no_leak",
    );
}

#[test]
fn test_closure_scalar_only_capture_two_call() {
    // Stays-green pin: scalar-only capture is not RC-tracked (AIMS:L-9
    // scalar sentinel). Pre-fix and post-fix both pass.
    assert_aot_success(
        include_str!("fixtures/match_alias/closure_scalar_only_capture_two_call.ori"),
        "closure_scalar_only_capture_two_call",
    );
}
