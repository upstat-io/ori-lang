//! Regression tests for BUG-04-013: AOT wrapper extraction methods must
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
// We reject compile failures (exit_code == -1), clean exits (0), and
// non-SIGABRT signal crashes (SIGSEGV, SIGBUS). SIGABRT is the expected
// panic termination path on Linux (ori_panic_cstr → _Unwind_RaiseException
// → fallback abort).

/// Assert that `exit_code` represents a panic (not compile failure, not clean
/// exit, not a crash signal other than SIGABRT).
fn assert_panic_exit(exit_code: i32, label: &str, stderr: &str) {
    assert_ne!(exit_code, -1, "{label}: compilation failed:\n{stderr}");
    assert_ne!(exit_code, 0, "{label}: should panic, but exited 0");
    // Reject SIGSEGV (-139) and SIGBUS (-135) but accept SIGABRT (-134).
    let is_bad_signal = exit_code <= -128 && exit_code != -134;
    assert!(
        !is_bad_signal,
        "{label}: killed by signal {} (exit {exit_code}), expected clean panic:\n{stderr}",
        -(exit_code + 128),
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
