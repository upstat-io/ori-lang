//! AOT tests for closure-capture drop emission (§04.2 of
//! `plans/aims-burden-tracking/section-04-recursive-closures-drop-value.md`).
//!
//! These tests exercise the END-TO-END closure-drop story: closure capture
//! composition (registry-side) + env-header `drop_fn` transport (codegen side
//! via existing `emit_rc_dec_closure` at `rc_ops.rs:256-287`) + `ori_rc_dec`
//! invocation at refcount-zero (runtime side).
//!
//! Per the §04.2 design, the codegen path is shipped (the existing
//! `DropKind::ClosureEnv(fields) | DropKind::Fields(fields)` shared arm at
//! `compiler_repo/compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs:91`
//! materializes the closure drop body once §04.2 populates
//! `UserBurdenSpec.compiled_drop` for closure types). The burden walker at
//! `ori_arc/src/lower/burden_lower.rs` consumes the registered closure
//! burden — pinned by the 5 `burden_lower` tests added in §04.2.
//!
//! The §04.2 algorithmic deliverables are pinned by:
//! - `ori_types::registry::burden_compose::closure::tests` — 11-cell matrix
//!   over capture-by-value, capture-by-reference, captures-of-captures,
//!   capture-of-projection, `compiled_drop` `FnSym` uniqueness, and default-shape
//!   invariants.
//! - `ori_arc::lower::burden_lower::tests::{
//!     closure_capture_by_value_of_owned_str_emits_burden_inc_at_partial_apply,
//!     closure_capture_by_reference_emits_no_burden_inc,
//!     nested_closure_emits_recursive_burden_inc_through_outer_env,
//!     closure_capture_of_projection_emits_borrowed_field_with_parent_lifetime,
//!     partial_apply_owned_capture_passed_to_owned_callee_emits_zero_net_burden_per_03_3_rule_5,
//!   }` — burden-walker emission pins.
//!
//! The AOT slice below activates once the lambda-side wiring at
//! `ori_types::infer::expr::infer_lambda` (`blocks.rs:223`) populates the
//! closure's `UserBurdenSpec` via `compose_closure_burden_spec` at
//! lambda-type-check time. Until that wiring lands (target-only — the
//! composer + tests are §04.2; the lambda-side wiring is plan-tracked under
//! BUG-04-118 closure-capture-burden registration follow-up; the
//! recursive-drop AOT blocker BUG-04-043 also gates any closure capturing
//! recursive struct types). The two AOT tests below scaffold the expected
//! end-to-end shape; re-enable in the same commit that closes the lambda-
//! side wiring blocker.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

/// Regression: §04.2 deliverable for closure capture-by-value of an owned
/// str. Verifies a closure capturing a heap-allocated `str` releases the
/// captured env at closure scope exit without leak. `ORI_CHECK_LEAKS=1`
/// reports zero leaks; `ORI_TRACE_RC=1` shows matching alloc/dec pairs for
/// both the captured str and the closure env.
///
/// Blocked by the §04.2 lambda-side wiring follow-up: until
/// `ori_types::infer::expr::infer_lambda` calls
/// `compose_closure_burden_spec` + `register_user_burden` at lambda-type-
/// check time, the closure type carries an empty `UserBurdenSpec` and the
/// burden walker emits no `BurdenInc` for the captured str. The 5
/// `burden_lower` matrix pins added in §04.2 cover the walker behavior with
/// a registered closure burden; the AOT slice re-enables once the
/// auto-registration wiring lands.
#[test]
#[ignore = "BUG-02-033: closure AOT slice blocked by the infer_lambda closure-UserBurdenSpec auto-registration wiring (blocks.rs:223); recursive-codegen blocker BUG-04-043 is resolved, so this re-enables when the lambda-side burden-wiring lands"]
fn test_closure_capture_by_value_str_drops_at_scope_exit() {
    let source = r#"
@t tests _ () -> void = {
    let s = "hello";
    let c = () -> s.length();
    let _len = c();
    ()
}
"#;
    assert_aot_success(source, "closure_capture_by_value_str_drops_at_scope_exit");
}

/// Regression: §04.2 shared-reference pin — closure with captured str gets
/// its env refcount-decremented at scope exit; refcount-zero branch of
/// `ori_rc_dec` (via `emit_rc_dec_closure` at `rc_ops.rs:256-287`) loads the
/// closure's `drop_fn` from `env_ptr` field 0 and invokes it to walk owned
/// captures.
///
/// Blocked by the same §04.2 lambda-side wiring follow-up as the test above.
#[test]
#[ignore = "BUG-02-033: closure AOT slice blocked by the infer_lambda closure-UserBurdenSpec auto-registration wiring (blocks.rs:223); recursive-codegen blocker BUG-04-043 is resolved, so this re-enables when the lambda-side burden-wiring lands"]
fn test_closure_env_header_drop_fn_invokes_at_refcount_zero() {
    let source = r#"
@t tests _ () -> void = {
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
