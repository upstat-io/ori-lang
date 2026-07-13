//! AOT tests for closure-capture drop emission.
//!
//! These tests exercise the END-TO-END closure-drop story: closure capture
//! composition (registry-side) + env-header `drop_fn` transport (codegen side
//! via existing `emit_rc_dec_closure`) + `ori_rc_dec`
//! invocation at refcount-zero (runtime side).
//!
//! Lambda type checking registers each closure's `UserBurdenSpec`; class-ledger
//! Step-4b emission places the verified closure-env release, and Phase 7 lowers
//! it mechanically. `DropKind::ClosureEnv(fields)` materializes the closure drop
//! body, while `emit_rc_dec_closure` loads and invokes the env-header `drop_fn`
//! at refcount zero. These AOT cases pin the complete path.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// Regression: closure capture-by-value of an owned
/// str. Verifies a closure capturing a heap-allocated `str` releases the
/// captured env at closure scope exit without leak. `ORI_CHECK_LEAKS=1`
/// reports zero leaks; `ORI_TRACE_RC=1` shows matching alloc/dec pairs for
/// both the captured str and the closure env.
#[test]
fn test_closure_capture_by_value_str_drops_at_scope_exit() {
    let source = r#"
@main () -> void = {
    let s = "hello";
    let c = () -> s.length();
    let _len = c();
    ()
}
"#;
    assert_aot_success(source, "closure_capture_by_value_str_drops_at_scope_exit");
}

/// Regression: shared-reference pin — closure with captured str gets
/// its env refcount-decremented at scope exit; refcount-zero branch of
/// `ori_rc_dec` (via `emit_rc_dec_closure`) loads the
/// closure's `drop_fn` from `env_ptr` field 0 and invokes it to walk owned
/// captures.
#[test]
fn test_closure_env_header_drop_fn_invokes_at_refcount_zero() {
    let source = r#"
@main () -> void = {
    let a = "alpha";
    let b = "beta";
    let c = () -> a.length() + b.length();
    let _len = c();
    ()
}
"#;
    assert_aot_success(
        source,
        "closure_env_header_drop_fn_invokes_at_refcount_zero",
    );
}
