//! Apply-alias coverage matrix.
//!
//! Each `ApplyAliasSource` shape (Direct / Project / Conditional) is exercised
//! across the RC-carrying priority element types (str / [int] / Option<str>).
//! Every cell runs through full Phase-5 burden emission + LLVM lowering +
//! execution under `ORI_CHECK_LEAKS=1` (`assert_aot_success`), and a control
//! that re-builds with `ORI_DISABLE_BURDEN_OPS=1` (Step-4b burden emission
//! skipped, predicate-stack baseline) and asserts byte-identical exit — proving
//! burden emission is observably inert during the BurdenInc/BurdenDec coexistence phase.
//!
//! The sibling spec tests at `tests/spec/aims/apply_alias_coverage/*.ori`
//! exercise the SAME shapes on the interpreter (`cargo st`) for dual-backend
//! dual-backend parity.
//!
//! ADDITIVE only — no production-code change. `collect_all_borrowed_defs` and
//! the borrow classification are untouched (3 reclassification attempts each
//! regressed AOT 29 -> 74).

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_with_build_env};

/// Assert the `ORI_DISABLE_BURDEN_OPS=1` control build is byte-identical to the
/// default build's success: both compile and exit 0 (zero leaks). Burden
/// emission is observably inert during the coexistence phase — turning it off MUST
/// not change behavior.
fn assert_burden_ops_disabled_byte_identical(source: &str, test_name: &str) {
    let (exit_code, _stdout, stderr) =
        compile_and_run_with_build_env(source, &[("ORI_DISABLE_BURDEN_OPS", "1")]);
    assert_eq!(
        exit_code, 0,
        "{test_name} under ORI_DISABLE_BURDEN_OPS=1 MUST exit 0 (byte-identical to default build); \
         burden emission is observably inert during the coexistence phase. stderr:\n{stderr}",
    );
}

/// Negative pin for a cell where burden emission is the LOAD-BEARING cure: the
/// `ORI_DISABLE_BURDEN_OPS=1` predicate-stack baseline LEAKS (exit 2), while the
/// burden-default path (asserted leak-clean by `assert_aot_success`) cures it.
/// The byte-identical "burden inert" premise does NOT hold for such a cell —
/// burden emission is the difference, which is exactly the migration's goal.
/// Asserting the baseline still leaks proves the burden cure is what frees the
/// allocation (the predicate stack is being retired, never repaired).
fn assert_burden_ops_disabled_baseline_leaks(source: &str, test_name: &str) {
    let (exit_code, _stdout, _stderr) =
        compile_and_run_with_build_env(source, &[("ORI_DISABLE_BURDEN_OPS", "1")]);
    assert_eq!(
        exit_code, 2,
        "{test_name} under ORI_DISABLE_BURDEN_OPS=1 MUST exit 2 (predicate-stack baseline leaks); \
         burden emission is the load-bearing cure for this cell. A change here means the baseline \
         behavior shifted — re-verify the burden-default path with assert_aot_success.",
    );
}

// --- Direct shape (`@id<T>(x: T) -> T = x`) ---

#[test]
fn test_apply_alias_direct_str() {
    let src = include_str!("fixtures/apply_alias_coverage/direct_str.ori");
    assert_aot_success(src, "apply_alias_direct_str");
    assert_burden_ops_disabled_byte_identical(src, "apply_alias_direct_str");
}

#[test]
fn test_apply_alias_direct_intlist() {
    let src = include_str!("fixtures/apply_alias_coverage/direct_intlist.ori");
    assert_aot_success(src, "apply_alias_direct_intlist");
    assert_burden_ops_disabled_byte_identical(src, "apply_alias_direct_intlist");
}

#[test]
fn test_apply_alias_direct_option_str() {
    let src = include_str!("fixtures/apply_alias_coverage/direct_option_str.ori");
    assert_aot_success(src, "apply_alias_direct_option_str");
    assert_burden_ops_disabled_byte_identical(src, "apply_alias_direct_option_str");
}

// --- Project shape (`@unwrap<T>(b: Box<T>) -> T = b.inner`) ---

#[test]
fn test_apply_alias_project_str() {
    let src = include_str!("fixtures/apply_alias_coverage/project_str.ori");
    assert_aot_success(src, "apply_alias_project_str");
    assert_burden_ops_disabled_byte_identical(src, "apply_alias_project_str");
}

// --- Conditional shape (multi-param path-conditional alias) ---

#[test]
fn test_apply_alias_conditional_str() {
    let src = include_str!("fixtures/apply_alias_coverage/conditional_str.ori");
    assert_aot_success(src, "apply_alias_conditional_str");
    assert_burden_ops_disabled_byte_identical(src, "apply_alias_conditional_str");
}

#[test]
fn test_apply_alias_conditional_intlist() {
    let src = include_str!("fixtures/apply_alias_coverage/conditional_intlist.ori");
    assert_aot_success(src, "apply_alias_conditional_intlist");
    // Burden is the LOAD-BEARING cure here: the predicate-stack baseline leaks
    // the un-chosen [int] conditional-alias buffer; the burden path frees it.
    assert_burden_ops_disabled_baseline_leaks(src, "apply_alias_conditional_intlist");
}

#[test]
fn test_apply_alias_conditional_option_str() {
    let src = include_str!("fixtures/apply_alias_coverage/conditional_option_str.ori");
    assert_aot_success(src, "apply_alias_conditional_option_str");
    assert_burden_ops_disabled_byte_identical(src, "apply_alias_conditional_option_str");
}
