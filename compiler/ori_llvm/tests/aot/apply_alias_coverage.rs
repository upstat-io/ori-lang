//! Apply-alias coverage matrix.
//!
//! Each `ApplyAliasSource` shape (Direct / Project / Conditional) is exercised
//! across the RC-carrying priority element types (str / [int] / Option<str>).
//! Every cell runs through full Phase-5 burden emission + class-ledger
//! replacement + LLVM lowering + execution under `ORI_CHECK_LEAKS=1`
//! (`assert_aot_success`), and a control that re-builds with
//! `ORI_DISABLE_BURDEN_OPS=1` and asserts compilation FAILS LOUD — burden
//! emission is the sole RC-emission input (no fallback emitter exists), so
//! disabling it must abort compilation, never emit silent wrong code.
//!
//! The sibling spec tests at `tests/spec/aims/apply_alias_coverage/*.ori`
//! exercise the SAME shapes on the interpreter (`cargo st`) for dual-backend
//! parity.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_with_build_env};

/// Assert the `ORI_DISABLE_BURDEN_OPS=1` control build FAILS LOUD: with
/// Step-4b burden emission skipped there is no RC-emission input, no fallback
/// emitter exists, and realize must abort with the migration-gate message —
/// never produce a silently-unmanaged binary.
fn assert_burden_ops_disabled_fails_loud(source: &str, test_name: &str) {
    let (exit_code, _stdout, stderr) =
        compile_and_run_with_build_env(source, &[("ORI_DISABLE_BURDEN_OPS", "1")]);
    assert_ne!(
        exit_code, 0,
        "{test_name} under ORI_DISABLE_BURDEN_OPS=1 MUST fail compilation (burden \
         emission is the sole RC-emission input; no fallback emitter exists). stderr:\n{stderr}",
    );
    assert!(
        stderr.contains("non-class-ledger function"),
        "{test_name} under ORI_DISABLE_BURDEN_OPS=1 MUST abort via the fail-loud \
         migration gate, not an unrelated error. stderr:\n{stderr}",
    );
}

// --- Direct shape (`@id<T>(x: T) -> T = x`) ---

#[test]
fn test_apply_alias_direct_str() {
    let src = include_str!("fixtures/apply_alias_coverage/direct_str.ori");
    assert_aot_success(src, "apply_alias_direct_str");
    assert_burden_ops_disabled_fails_loud(src, "apply_alias_direct_str");
}

#[test]
fn test_apply_alias_direct_intlist() {
    let src = include_str!("fixtures/apply_alias_coverage/direct_intlist.ori");
    assert_aot_success(src, "apply_alias_direct_intlist");
    assert_burden_ops_disabled_fails_loud(src, "apply_alias_direct_intlist");
}

#[test]
fn test_apply_alias_direct_option_str() {
    let src = include_str!("fixtures/apply_alias_coverage/direct_option_str.ori");
    assert_aot_success(src, "apply_alias_direct_option_str");
    assert_burden_ops_disabled_fails_loud(src, "apply_alias_direct_option_str");
}

// --- Project shape (`@unwrap<T>(b: Box<T>) -> T = b.inner`) ---

#[test]
fn test_apply_alias_project_str() {
    let src = include_str!("fixtures/apply_alias_coverage/project_str.ori");
    assert_aot_success(src, "apply_alias_project_str");
    assert_burden_ops_disabled_fails_loud(src, "apply_alias_project_str");
}

// --- Conditional shape (multi-param path-conditional alias) ---

#[test]
fn test_apply_alias_conditional_str() {
    let src = include_str!("fixtures/apply_alias_coverage/conditional_str.ori");
    assert_aot_success(src, "apply_alias_conditional_str");
    assert_burden_ops_disabled_fails_loud(src, "apply_alias_conditional_str");
}

#[test]
fn test_apply_alias_conditional_intlist() {
    let src = include_str!("fixtures/apply_alias_coverage/conditional_intlist.ori");
    assert_aot_success(src, "apply_alias_conditional_intlist");
    assert_burden_ops_disabled_fails_loud(src, "apply_alias_conditional_intlist");
}

#[test]
fn test_apply_alias_conditional_option_str() {
    let src = include_str!("fixtures/apply_alias_coverage/conditional_option_str.ori");
    assert_aot_success(src, "apply_alias_conditional_option_str");
    assert_burden_ops_disabled_fails_loud(src, "apply_alias_conditional_option_str");
}
