//! Panic Handler AOT Tests
//!
//! Tests for `@panic` entry point: handler registration, invocation,
//! `PanicInfo` construction, re-entrancy protection, and default behavior.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::{assert_aot_success, compile_and_run_capture};

// ─── Default behavior (no handler) ───

#[test]
fn test_panic_default_nonzero_exit() {
    let (exit_code, _stdout, stderr) = compile_and_run_capture(
        r#"
@main () -> int = {
    panic(msg: "deliberate test panic")
}
"#,
    );
    assert_ne!(exit_code, 0, "panic should cause non-zero exit");
    assert!(
        stderr.contains("deliberate test panic"),
        "stderr should contain panic message, got: {stderr}"
    );
}

#[test]
fn test_panic_default_unreachable_branch() {
    assert_aot_success(
        r#"
@main () -> int = {
    let x = 42;
    if x == 42 then 0 else panic(msg: "unreachable")
}
"#,
        "panic_unreachable_branch",
    );
}

// ─── Handler compilation (no panic triggered) ───

#[test]
fn test_panic_handler_compiles_without_panic() {
    let source = make_panic_program(r#"print(msg: "handler called")"#, r#"{ 0 }"#);
    assert_aot_success(&source, "panic_handler_no_panic");
}

// ─── Handler invocation ───

#[test]
fn test_panic_handler_invoked() {
    let source = make_panic_program(
        r#"print(msg: "HANDLER_RAN")"#,
        r#"{ panic(msg: "trigger") }"#,
    );
    let (exit_code, stdout, stderr) = compile_and_run_capture(&source);
    assert_ne!(
        exit_code, 0,
        "program should still exit non-zero after handler"
    );
    assert!(
        stdout.contains("HANDLER_RAN") || stderr.contains("HANDLER_RAN"),
        "handler output should appear in stdout or stderr.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

#[test]
fn test_panic_handler_receives_message() {
    let source = make_panic_program(
        r#"print(msg: info.message)"#,
        r#"{ panic(msg: "hello from panic") }"#,
    );
    let (exit_code, stdout, stderr) = compile_and_run_capture(&source);
    assert_ne!(exit_code, 0, "should still exit non-zero");
    assert!(
        stdout.contains("hello from panic") || stderr.contains("hello from panic"),
        "handler should print the panic message.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

// ─── Re-entrancy protection ───

#[test]
fn test_panic_handler_re_entrancy() {
    let source = make_panic_program(
        r#"panic(msg: "handler panicked too")"#,
        r#"{ panic(msg: "initial panic") }"#,
    );
    let (exit_code, _stdout, stderr) = compile_and_run_capture(&source);
    assert_ne!(exit_code, 0, "should exit non-zero");
    // The key assertion: the program terminates (no infinite loop).
    // If re-entrancy guard is broken, this test would hang/timeout.
    assert!(
        stderr.contains("initial panic") || stderr.contains("handler panicked"),
        "stderr should contain a panic message.\nstderr: {stderr}"
    );
}

// ─── Handler with no access to fields ───

#[test]
fn test_panic_handler_ignores_info() {
    let source = make_panic_program(
        r#"print(msg: "caught a panic")"#,
        r#"{
    let x = 10;
    if x > 100 then 0 else panic(msg: "x too small")
}"#,
    );
    let (exit_code, stdout, stderr) = compile_and_run_capture(&source);
    assert_ne!(exit_code, 0, "should exit non-zero");
    assert!(
        stdout.contains("caught a panic") || stderr.contains("caught a panic"),
        "handler should have run.\nstdout: {stdout}\nstderr: {stderr}"
    );
}

// ─── Test helper ───

/// Build a complete Ori program with `PanicInfo` types, `@panic` handler, and `@main`.
///
/// `handler_body` is the expression inside the @panic handler body.
/// `main_body` is the full body expression for @main (including braces).
fn make_panic_program(handler_body: &str, main_body: &str) -> String {
    format!(
        r#"
type TraceEntry = {{
    function_name: str,
    file: str,
    line: int,
    column: int,
}}

type PanicInfo = {{
    message: str,
    location: TraceEntry,
    stack_trace: [TraceEntry],
    thread_id: Option<int>,
}}

@panic (info: PanicInfo) -> void = {{
    {handler_body}
}}

@main () -> int = {main_body}
"#
    )
}
