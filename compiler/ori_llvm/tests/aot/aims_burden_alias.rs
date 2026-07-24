//! Lattice alias-tracking on the Result inner-alias destructure shape —
//! burden-path evaluation harness.
//!
//! Verifies class-ledger Step-4b emission preserves `inner`'s ownership
//! obligation when `inner` survives the Result's destructure. Mirrors the Ori
//! spec test at `tests/spec/aims/burden_alias_tracking.ori` through class-ledger
//! burden-op emission, mechanical Phase-7 lowering, LLVM lowering, and execution.

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
