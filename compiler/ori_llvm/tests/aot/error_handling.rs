//! Error Handling AOT Tests
//!
//! Tests for ? operator, Result/Option chaining, error propagation through
//! multiple function calls, error handling in loops, and combined
//! Result+Option patterns.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, assert_panic_exit, compile_and_run_capture};

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

// ─── panic-message ownership: ori_panic owns + releases its message ───

/// Regression: a heap panic message aliased past the catch boundary must
/// survive the caught panic (the panic transfer dup-incs the still-live
/// message; the runtime releases only the transferred reference).
#[test]
fn test_catch_panic_aliased_message_survives_catch() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_panic_aliased_message_survives.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "aliased message must stay valid after the caught panic, leak-clean:\n{stderr}"
    );
}

/// Regression: nested catches each recover their own heap panic message;
/// every message buffer is released exactly once (no leak, no double-free).
#[test]
fn test_nested_catch_panic_heap_messages_release_once() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/nested_catch_panic_heap_messages.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "nested catch must recover both messages, leak-clean:\n{stderr}"
    );
}

/// Over-fire negative: an SSO panic message has no heap buffer — the
/// runtime release must no-op (no corruption, no spurious free).
#[test]
fn test_catch_panic_sso_message_no_heap_release() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/catch_panic_sso_message.ori"
    ));
    assert_eq!(
        exit_code, 0,
        "SSO message catch must stay clean (release no-ops on SSO):\n{stderr}"
    );
}

/// Over-fire negative: the uncaught-panic exit path is unchanged — panic
/// exit code, message on stderr, and the message buffer released before
/// the unwind (leak-clean even though the process is exiting).
#[test]
fn test_uncaught_panic_heap_message_exit_unchanged() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/uncaught_panic_heap_message.ori"
    ));
    assert_panic_exit(exit_code, "uncaught heap-message panic", &stderr);
    assert!(
        stderr.contains("uncaught heap panic message"),
        "panic message must reach stderr intact:\n{stderr}"
    );
    assert!(
        !stderr.contains("not freed"),
        "uncaught panic path must not leak the message buffer:\n{stderr}"
    );
}

/// Over-fire negative: `expect` panics route the user message through the
/// inline panic emission (manufactured transfer inc) — the panic exit and
/// message are unchanged.
#[test]
fn test_expect_panic_heap_message_exit_unchanged() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/error_handling/expect_panic_heap_message_uncaught.ori"
    ));
    assert_panic_exit(exit_code, "uncaught expect panic", &stderr);
    assert!(
        stderr.contains("expect failure message"),
        "expect panic message must reach stderr intact:\n{stderr}"
    );
}
