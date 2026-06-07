//! AOT tests for closure-capture drop emission.
//!
//! These tests exercise the END-TO-END closure-drop story: closure capture
//! composition (registry-side) + env-header `drop_fn` transport (codegen side
//! via existing `emit_rc_dec_closure` at `rc_ops.rs:256-287`) + `ori_rc_dec`
//! invocation at refcount-zero (runtime side).
//!
//! The codegen path is shipped (the existing
//! `DropKind::ClosureEnv(fields) | DropKind::Fields(fields)` shared arm at
//! `compiler_repo/compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs:91`
//! materializes the closure drop body once burden composition populates
//! `UserBurdenSpec.compiled_drop` for closure types). The burden walker at
//! `ori_arc/src/lower/burden_lower.rs` consumes the registered closure
//! burden — pinned by the 5 `burden_lower` closure-burden tests.
//!
//! The closure-burden algorithmic deliverables are pinned by:
//! - `ori_types::registry::burden_compose::closure::tests` — 11-cell matrix
//!   over capture-by-value, capture-by-reference, captures-of-captures,
//!   capture-of-projection, `compiled_drop` `FnSym` uniqueness, and default-shape
//!   invariants.
//! - `ori_arc::lower::burden_lower::tests::{
//!     closure_capture_by_value_of_owned_str_emits_burden_inc_at_partial_apply,
//!     closure_capture_by_reference_emits_no_burden_inc,
//!     nested_closure_emits_recursive_burden_inc_through_outer_env,
//!     closure_capture_of_projection_emits_borrowed_field_with_parent_lifetime,
//!     partial_apply_owned_capture_passed_to_owned_callee_emits_two_transfer_point_burden_inc,
//!   }` — burden-walker emission pins.
//!
//! The lambda-side wiring at `ori_types::infer::expr::infer_lambda` auto-registers
//! each closure's `UserBurdenSpec` via `compose_closure_burden_spec` at
//! lambda-type-check time, so the two AOT tests below exercise the full
//! end-to-end drop: env owns a copy of every captured value and decrements it
//! exactly once via the env-header `drop_fn` at refcount zero.

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
/// `ori_rc_dec` (via `emit_rc_dec_closure` at `rc_ops.rs:256-287`) loads the
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
