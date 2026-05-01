//! Regression tests for AOT wrapper extraction methods must
//! RC-retain payload bytes when the payload contains RC-tracked fields.
//!
//! Each test uses heap strings (>23 bytes, bypassing SSO) or lists as
//! payloads to trigger double-frees if RC retain is missing. The test
//! harness runs with `ORI_CHECK_LEAKS=1`, so both double-frees and leaks
//! are caught.

use crate::util::{assert_aot_success, compile_and_run_capture};

// --- Option.unwrap ---

#[test]
fn test_option_unwrap_heap_str_rc_retain() {
    assert_aot_success(
        include_str!("fixtures/wrapper_rc_retain/option_unwrap_heap_str.ori"),
        "option_unwrap_heap_str_rc_retain",
    );
}

#[test]
fn test_option_unwrap_list_payload_rc_retain() {
    assert_aot_success(
        include_str!("fixtures/wrapper_rc_retain/option_unwrap_list_payload.ori"),
        "option_unwrap_list_payload_rc_retain",
    );
}

// --- Result.unwrap ---

#[test]
fn test_result_unwrap_heap_str_rc_retain() {
    assert_aot_success(
        include_str!("fixtures/wrapper_rc_retain/result_unwrap_heap_str.ori"),
        "result_unwrap_heap_str_rc_retain",
    );
}

#[test]
fn test_result_unwrap_list_payload_rc_retain() {
    assert_aot_success(
        include_str!("fixtures/wrapper_rc_retain/result_unwrap_list_payload.ori"),
        "result_unwrap_list_payload_rc_retain",
    );
}

// --- Result.unwrap_err ---

#[test]
fn test_result_unwrap_err_heap_str_rc_retain() {
    assert_aot_success(
        include_str!("fixtures/wrapper_rc_retain/result_unwrap_err_heap_str.ori"),
        "result_unwrap_err_heap_str_rc_retain",
    );
}

#[test]
fn test_result_unwrap_err_list_payload_rc_retain() {
    assert_aot_success(
        include_str!("fixtures/wrapper_rc_retain/result_unwrap_err_list_payload.ori"),
        "result_unwrap_err_list_payload_rc_retain",
    );
}

// --- List.first ---

#[test]
fn test_list_first_heap_str_rc_retain() {
    assert_aot_success(
        include_str!("fixtures/wrapper_rc_retain/list_first_heap_str.ori"),
        "list_first_heap_str_rc_retain",
    );
}

#[test]
fn test_list_first_list_payload_rc_retain() {
    assert_aot_success(
        include_str!("fixtures/wrapper_rc_retain/list_first_list_payload.ori"),
        "list_first_list_payload_rc_retain",
    );
}

// --- List.last ---

#[test]
fn test_list_last_heap_str_rc_retain() {
    assert_aot_success(
        include_str!("fixtures/wrapper_rc_retain/list_last_heap_str.ori"),
        "list_last_heap_str_rc_retain",
    );
}

#[test]
fn test_list_last_list_payload_rc_retain() {
    assert_aot_success(
        include_str!("fixtures/wrapper_rc_retain/list_last_list_payload.ori"),
        "list_last_list_payload_rc_retain",
    );
}

// --- Negative pins: unwrap on wrong variant must panic ---
//
// These tests verify that the new `emit_unwrap_branch` actually fires.
// Valid panic termination paths, in order of "correctness":
//
// 1. Exit code 1 — `ori_run_main` / LLVM-generated `main()` caught the
//    panic cleanly via `invoke`/`landingpad` (Itanium) or SEH (MSVC).
//    This is the designed flow; §Unwinding ABI and
//    `ori_run_main` doc comment: "1: panic".
// 2. SIGABRT (exit code 134 / signal -134) — panic bubbled past the
//    main wrapper and hit `abort()` (e.g., uncaught unwind on an
//    older toolchain path, or when the caller path lacks a landingpad).
//    Historically produced on Linux when a panic unwound through a
//    `nounwind`-attributed frame (UB, but happened to terminate here).
// 3. Windows `STATUS_STACK_BUFFER_OVERRUN` (0xC0000409 /
//    -1_073_740_791) — MSVC `abort()` triggering
//    `__fastfail(FAST_FAIL_FATAL_APP_EXIT)`.
// 4. Windows exit code 3 — traditional MSVC `abort()` exit code.
//
// We reject compile failures (exit_code == -1), clean exits (0), and
// other crash signals (SIGSEGV -139/139, SIGBUS -135/135) that would
// indicate a memory-safety bug rather than a controlled panic.

/// Assert that `exit_code` represents a panic (not compile failure, not clean
/// exit, not a non-panic crash signal).
fn assert_panic_exit(exit_code: i32, label: &str, stderr: &str) {
    const STATUS_STACK_BUFFER_OVERRUN: i32 = -1_073_740_791; // 0xC0000409
    assert_ne!(exit_code, -1, "{label}: compilation failed:\n{stderr}");
    assert_ne!(exit_code, 0, "{label}: should panic, but exited 0");
    // Accept the four valid panic-termination codes enumerated above.
    let is_panic_exit = exit_code == 1
        || exit_code == -134
        || exit_code == 134
        || exit_code == STATUS_STACK_BUFFER_OVERRUN
        || exit_code == 3;
    assert!(
        is_panic_exit,
        "{label}: expected panic exit (1, SIGABRT 134/-134, Windows 0xC0000409/3), \
         got exit code {exit_code}:\n{stderr}",
    );
}

#[test]
fn test_option_unwrap_none_panics() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/wrapper_rc_retain/option_unwrap_none_panics.ori"
    ));
    assert_panic_exit(exit_code, "Option.unwrap(None)", &stderr);
    assert!(
        stderr.contains("unwrap") || stderr.contains("None"),
        "panic message should mention unwrap/None, got: {stderr}"
    );
}

#[test]
fn test_result_unwrap_on_err_panics() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/wrapper_rc_retain/result_unwrap_err_panics.ori"
    ));
    assert_panic_exit(exit_code, "Result.unwrap(Err)", &stderr);
    assert!(
        stderr.contains("unwrap") || stderr.contains("Err"),
        "panic message should mention unwrap/Err, got: {stderr}"
    );
}

#[test]
fn test_result_unwrap_err_on_ok_panics() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(include_str!(
        "fixtures/wrapper_rc_retain/result_unwrap_err_on_ok_panics.ori"
    ));
    assert_panic_exit(exit_code, "Result.unwrap_err(Ok)", &stderr);
    assert!(
        stderr.contains("unwrap_err") || stderr.contains("Ok"),
        "panic message should mention unwrap_err/Ok, got: {stderr}"
    );
}

// --- Semantic pin: stdout verification ---

#[test]
fn test_option_unwrap_heap_str_correct_value() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(
        r#"
@make () -> Option<str> = Some("hello world this is a long heap string");

@main () -> void = {
    let o = make();
    let v = o.unwrap();
    print(msg: v)
}
"#,
    );
    assert_eq!(
        exit_code, 0,
        "option unwrap heap str failed (exit {exit_code}):\nstderr: {stderr}"
    );
    assert_eq!(stdout.trim(), "hello world this is a long heap string");
}

#[test]
fn test_list_first_heap_str_correct_value() {
    let (exit_code, stdout, stderr) = compile_and_run_capture(
        r#"
@make () -> [str] = ["hello world this is a long heap string", "second"];

@main () -> void = {
    let items = make();
    let f = items.first();
    let v = f.unwrap();
    print(msg: v)
}
"#,
    );
    assert_eq!(
        exit_code, 0,
        "list first heap str failed (exit {exit_code}):\nstderr: {stderr}"
    );
    assert_eq!(stdout.trim(), "hello world this is a long heap string");
}
