//! Inc-symmetry regression guards (BUG-04-104 §3.4).
//!
//! Tests for §3.4 inc-symmetry cells from the BUG-04-104 fix plan TDD
//! matrix. Per Round 1 reclassification, Sub-phase A class-aware Inc
//! skip was DROPPED at PIN-1 — per-name compensating Inc is correct
//! per RL-1; class-aware Inc skip would cause use-after-free. These
//! cells are regression guards: they assert that future changes do
//! not accidentally re-introduce class-aware `RcInc` skip. They pass
//! clean BOTH pre-fix AND post-fix; `assert_aot_success` already
//! enables `ORI_CHECK_LEAKS=1` so any re-introduction of Sub-phase A
//! that would produce over-Inc / under-Dec imbalance surfaces here as
//! exit code 2 (leak detected).
//!
//! Fixtures are reused from §3.1 + §3.2 per §03 §3.4 cell-ID overlap.
//! The harness re-runs them as standalone tests to provide
//! independent failure attribution if the regression-guard signal
//! diverges from the in-class / borrow-class signal.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

#[test]
fn test_inc_symmetry_jumparg_alias_loop_break() {
    assert_aot_success(
        include_str!("fixtures/match_alias/jumparg_alias_loop_break.ori"),
        "inc_symmetry_jumparg_alias_loop_break",
    );
}

#[test]
fn test_inc_symmetry_multivariant_match_alias() {
    assert_aot_success(
        include_str!("fixtures/match_alias/multivariant_match_alias.ori"),
        "inc_symmetry_multivariant_match_alias",
    );
}

#[test]
fn test_inc_symmetry_borrow_then_let_alias() {
    assert_aot_success(
        include_str!("fixtures/borrow_independence/borrow_then_let_alias.ori"),
        "inc_symmetry_borrow_then_let_alias",
    );
}
