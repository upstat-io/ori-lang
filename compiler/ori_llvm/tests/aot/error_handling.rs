//! Error Handling AOT Tests
//!
//! Tests for ? operator, Result/Option chaining, error propagation through
//! multiple function calls, error handling in loops, and combined
//! Result+Option patterns.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_capture};

// ─── Result: basic patterns ───

#[test]
fn test_err_result_ok_unwrap() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_ok_unwrap.ori"),
        "err_result_ok",
    );
}

#[test]
fn test_err_result_err_check() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_err_check.ori"),
        "err_result_err",
    );
}

#[test]
fn test_err_result_unwrap_or_ok() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_unwrap_or_ok.ori"),
        "err_result_unwrap_or_ok",
    );
}

#[test]
fn test_err_result_unwrap_or_err() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_unwrap_or_err.ori"),
        "err_result_unwrap_or_err",
    );
}

// ─── Option: basic patterns ───

#[test]
fn test_err_option_some_unwrap() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_option_some_unwrap.ori"),
        "err_option_some",
    );
}

#[test]
fn test_err_option_none_check() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_option_none_check.ori"),
        "err_option_none",
    );
}

#[test]
fn test_err_option_unwrap_or_some() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_option_unwrap_or_some.ori"),
        "err_option_unwrap_or_some",
    );
}

#[test]
fn test_err_option_unwrap_or_none() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_option_unwrap_or_none.ori"),
        "err_option_unwrap_or_none",
    );
}

// ─── ? operator: Result ───

#[test]
fn test_err_try_result_ok() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_result_ok.ori"),
        "err_try_result_ok",
    );
}

#[test]
fn test_err_try_result_err() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_result_err.ori"),
        "err_try_result_err",
    );
}

#[test]
fn test_err_try_result_chain() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_result_chain.ori"),
        "err_try_chain",
    );
}

#[test]
fn test_err_try_result_early_exit() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_result_early_exit.ori"),
        "err_try_early_exit",
    );
}

// ─── ? operator: Option ───

#[test]
fn test_err_try_option_some() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_option_some.ori"),
        "err_try_option_some",
    );
}

#[test]
fn test_err_try_option_none() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_try_option_none.ori"),
        "err_try_option_none",
    );
}

// ─── Result with conditional logic ───

#[test]
fn test_err_result_conditional() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_conditional.ori"),
        "err_result_conditional",
    );
}

#[test]
fn test_err_result_chain_conditional() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_chain_conditional.ori"),
        "err_chain_conditional",
    );
}

// ─── Deep ? chains ───

#[test]
fn test_err_deep_try_chain() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_deep_try_chain.ori"),
        "err_deep_try",
    );
}

// ─── Result in loops ───

#[test]
fn test_err_result_in_loop() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_in_loop.ori"),
        "err_result_in_loop",
    );
}

// ─── Option patterns ───

#[test]
fn test_err_option_chain_unwrap() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_option_chain_unwrap.ori"),
        "err_option_chain",
    );
}

// ─── Mixed Result + Option ───

#[test]
fn test_err_result_with_option_payload() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_with_option_payload.ori"),
        "err_result_option",
    );
}

// ─── Return type variants ───

#[test]
fn test_err_result_int_err() {
    assert_aot_success(
        include_str!("fixtures/error_handling/err_result_int_err.ori"),
        "err_result_int_err",
    );
}

// ─── catch(expr:): panic recovery ───

#[test]
fn test_catch_panic_returns_err() {
    // Suppress stderr (ori_panic prints the panic message before unwinding).
    let (exit_code, _stdout, _stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_panic_returns_err.ori"
    ));
    assert_eq!(exit_code, 0, "catch should produce Err on panic");
}

#[test]
fn test_catch_success_returns_ok() {
    assert_aot_success(
        include_str!("fixtures/error_handling/catch_success_returns_ok.ori"),
        "catch_success",
    );
}

#[test]
fn test_catch_ok_unwrap_value() {
    assert_aot_success(
        include_str!("fixtures/error_handling/catch_ok_unwrap_value.ori"),
        "catch_ok_unwrap",
    );
}

#[test]
fn test_catch_simple_expression() {
    assert_aot_success(
        include_str!("fixtures/error_handling/catch_simple_expression.ori"),
        "catch_simple_expr",
    );
}

#[test]
fn test_catch_panic_explicit() {
    let (exit_code, _stdout, _stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_panic_explicit.ori"
    ));
    assert_eq!(exit_code, 0, "catch should capture explicit panic");
}

#[test]
fn test_catch_multiple_independent() {
    let (exit_code, _stdout, _stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_multiple_independent.ori"
    ));
    assert_eq!(exit_code, 0, "independent catches should work");
}

#[test]
fn test_catch_in_conditional() {
    let (exit_code, _stdout, _stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_in_conditional.ori"
    ));
    assert_eq!(exit_code, 0, "catch with conditional panics");
}
