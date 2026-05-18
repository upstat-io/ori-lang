//! Lattice alias-tracking on the BUG-04-118 shape — §04B Criterion 3
//! evaluation harness.
//!
//! Verifies §04A.2 `eliminate_burden_ops` consuming DP-2/DP-3 from the
//! converged `AimsStateMap` does NOT over-eliminate `inner`'s `BurdenDec`
//! when `inner` survives the Result's destructure. Mirrors the Ori spec
//! test at `tests/spec/aims/burden_alias_tracking.ori` through full
//! Phase 5 emission + §04A.2 elimination + LLVM lowering + execution.

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
