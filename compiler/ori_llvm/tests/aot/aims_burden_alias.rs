//! Lattice alias-tracking on the Result inner-alias destructure shape —
//! burden-path evaluation harness.
//!
//! Verifies `eliminate_burden_ops` consuming DP-2/DP-3 from the
//! converged `AimsStateMap` does NOT over-eliminate `inner`'s `BurdenDec`
//! when `inner` survives the Result's destructure. Mirrors the Ori spec
//! test at `tests/spec/aims/burden_alias_tracking.ori` through full
//! Phase 5 emission + burden-op elimination + LLVM lowering + execution.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

#[test]
fn test_burden_alias_inner_survives_result_destructure() {
    assert_aot_success(
        include_str!("fixtures/aims_burden_alias/inner_survives_result_destructure.ori"),
        "burden_alias_inner_survives_result_destructure",
    );
}

/// Semantic pin: `ORI_DISABLE_SOLE_CARRIER_BORROWED_INVOKE_CLAIM=1` restores
/// the `wrap_ok(m: m)` mint-shape early release — proves the sole-carrier
/// claim (the alias's release relocated to the Category-2 `deadAtSucc` edge
/// after the borrowed carrier `Invoke`) is the cure surface for this cell.
#[test]
fn toggle_disables_sole_carrier_claim_inner_survives_crashes_again() {
    use crate::util::compile_and_run_with_build_env;
    let (exit, _stdout, stderr) = compile_and_run_with_build_env(
        include_str!("fixtures/aims_burden_alias/inner_survives_result_destructure.ori"),
        &[("ORI_DISABLE_SOLE_CARRIER_BORROWED_INVOKE_CLAIM", "1")],
    );
    assert_ne!(
        exit, 0,
        "with the sole-carrier borrowed-invoke claim disabled, the mint-shape \
         pin must regress (early release before the borrowed carrier call, \
         exit != 0)\nstderr:\n{stderr}"
    );
}

/// Negative pin: the positive pin ACTIVELY catches the over-elimination
/// regression class (matrix-testing rule — every positive pin needs a negative
/// pin proving it fails on broken behavior). `ORI_FORCE_OVERELIMINATE=1` drops
/// every whole-var `BurdenDec*` release the DP-2 guard preserves — an inner
/// field destructured out of a `Result` and kept alive past the container's
/// own drop loses its release. Under the burden-sole probe env the forced
/// shape drops `inner`'s release and the surviving map leaks (`ORI_CHECK_LEAKS`
/// trips, exit != 0); the env-unset control keeps the guard and stays clean
/// (exit 0). SPEC-63 path-exercising test for the `ORI_FORCE_OVERELIMINATE`
/// env var.
#[test]
fn force_overeliminate_inner_survives_result_leaks_negative_pin() {
    use crate::util::compile_and_run_with_build_env;
    let src = include_str!("fixtures/aims_burden_alias/inner_survives_result_destructure.ori");
    let probe: &[(&str, &str)] = &[
        ("ORI_DISABLE_PREDICATE_STACK_RC", "1"),
        ("ORI_VERIFY_ARC", "1"),
        ("ORI_VERIFY_EACH", "1"),
    ];

    // Control: burden-sole probe, DP-2 guard intact — the positive pin holds.
    let (control_exit, _stdout, control_stderr) = compile_and_run_with_build_env(src, probe);
    assert_eq!(
        control_exit, 0,
        "burden-sole probe with the DP-2 guard intact must run clean (no leak, \
         no double-free)\nstderr:\n{control_stderr}"
    );

    // Forced over-elimination: DP-2 guard dropped — inner's RL-2 scope-exit
    // release is elided and the surviving map leaks under ORI_CHECK_LEAKS.
    let mut forced: Vec<(&str, &str)> = probe.to_vec();
    forced.push(("ORI_FORCE_OVERELIMINATE", "1"));
    let (forced_exit, _stdout, forced_stderr) = compile_and_run_with_build_env(src, &forced);
    assert_ne!(
        forced_exit, 0,
        "forcing burden over-elimination must drop inner's release and trip the \
         leak/double-free checker (exit != 0) — proves the positive pin actively \
         catches the over-elimination regression\nstderr:\n{forced_stderr}"
    );
}
