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
