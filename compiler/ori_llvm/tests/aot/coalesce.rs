//! Coalesce Operator (`??`) AOT Tests
//!
//! Dual-execution parity coverage for the `??` operator through the LLVM path.
//! Covers both unwrap forms (`Option<T> ?? T`, `Result<T,E> ?? T`) and
//! same-type wrapper chaining (`Option<T> ?? Option<T>`, `Result<T,E> ?? Result<T,E>`).
//!
//! Regression: BUG-02-002 — coalesce same-type wrapper chaining was typed as `T`
//! instead of the wrapper type. The chaining tests here pin the fix on the LLVM path.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::assert_aot_success;

// ─── Unwrap form: Option<T> ?? T ───

#[test]
fn test_coalesce_option_unwrap_some() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_option_unwrap_some.ori"),
        "coalesce_option_unwrap_some",
    );
}

#[test]
fn test_coalesce_option_unwrap_none() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_option_unwrap_none.ori"),
        "coalesce_option_unwrap_none",
    );
}

// ─── Unwrap form: Result<T, E> ?? T ───

#[test]
fn test_coalesce_result_unwrap_ok() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_result_unwrap_ok.ori"),
        "coalesce_result_unwrap_ok",
    );
}

#[test]
fn test_coalesce_result_unwrap_err() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_result_unwrap_err.ori"),
        "coalesce_result_unwrap_err",
    );
}

// ─── Same-type wrapper chaining (BUG-02-002 regression pins) ───

#[test]
fn test_coalesce_option_chain_some() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_option_chain_some.ori"),
        "coalesce_option_chain_some",
    );
}

#[test]
fn test_coalesce_option_chain_none() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_option_chain_none.ori"),
        "coalesce_option_chain_none",
    );
}

#[test]
fn test_coalesce_option_chain_both_none() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_option_chain_both_none.ori"),
        "coalesce_option_chain_both_none",
    );
}

#[test]
fn test_coalesce_result_chain_ok() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_result_chain_ok.ori"),
        "coalesce_result_chain_ok",
    );
}

#[test]
fn test_coalesce_result_chain_err() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_result_chain_err.ori"),
        "coalesce_result_chain_err",
    );
}

#[test]
fn test_coalesce_result_chain_both_err() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_result_chain_both_err.ori"),
        "coalesce_result_chain_both_err",
    );
}

// ─── Cross-type: str wrapper chaining ───

#[test]
fn test_coalesce_option_chain_str() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_option_chain_str.ori"),
        "coalesce_option_chain_str",
    );
}

// ─── Compound: chain then unwrap ───

#[test]
fn test_coalesce_chain_then_unwrap() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_chain_then_unwrap.ori"),
        "coalesce_chain_then_unwrap",
    );
}

#[test]
fn test_coalesce_chain_then_unwrap_all_none() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_chain_then_unwrap_all_none.ori"),
        "coalesce_chain_then_unwrap_all_none",
    );
}

// ─── Polymorphic RHS constructors (TPR-02-003 regression pins) ───

#[test]
fn test_coalesce_option_chain_bare_none() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_option_chain_bare_none.ori"),
        "coalesce_option_chain_bare_none",
    );
}

#[test]
fn test_coalesce_result_chain_bare_err() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_result_chain_bare_err.ori"),
        "coalesce_result_chain_bare_err",
    );
}

#[test]
fn test_coalesce_result_chain_bare_ok() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_result_chain_bare_ok.ori"),
        "coalesce_result_chain_bare_ok",
    );
}

// ─── Triple chain ───

#[test]
fn test_coalesce_triple_chain() {
    assert_aot_success(
        include_str!("fixtures/coalesce/coalesce_triple_chain.ori"),
        "coalesce_triple_chain",
    );
}
