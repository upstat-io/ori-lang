//! AOT tests for the Value-trait empty-burden path.
//!
//! These tests pin the END-TO-END Value empty-burden story: a `Value`-marked
//! (or all-scalar) program emits ZERO RC operations because the empty
//! `UserBurdenSpec` (registered at `populate_value_burden_if_applicable`)
//! carries no heap to release; and mixed-Value/Heap composition
//! (`Result<int, str>`) emits RC ops ONLY on the heap-bearing variant.
//!
//! Two oracle layers:
//! - IR-level: `compile_and_capture_ir` counts `ori_rc_inc`/`ori_rc_dec`
//!   call sites (the positive/negative RC pins).
//! - Implicit: `assert_aot_success` runs the AOT binary with
//!   `ORI_CHECK_LEAKS=1` (zero-leak verification) and eval/LLVM parity is
//!   confirmed by comparing the interpreter exit code to the AOT exit code.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use std::process::Command;

use crate::util::{
    assert_aot_success, compile_and_capture_ir, compile_and_run_capture, ori_binary, stdlib_path,
};

/// Run a program through the interpreter (`ori run`) and return its exit code.
/// Used for eval/LLVM parity assertions alongside the AOT run.
fn interpreter_exit_code(source: &str) -> i32 {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let source_path = temp_dir.path().join("value_empty_burden_parity.ori");
    std::fs::write(&source_path, source).expect("write source");
    let run = Command::new(ori_binary())
        .args(["run", source_path.to_str().unwrap()])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("execute ori run");
    run.status.code().unwrap_or(-1)
}

/// Count `ori_rc_inc` + `ori_rc_dec` call sites across every RC variant
/// (`ori_rc_*`, `ori_str_rc_*`, `ori_buffer_rc_*`, `ori_list_rc_*`).
fn rc_op_count(ir: &str) -> usize {
    ir.lines()
        .filter(|line| {
            let l = line.trim_start();
            // Only count CALL sites, not the `declare`/`define` lines.
            l.starts_with("call ") && (l.contains("_rc_inc(") || l.contains("_rc_dec("))
        })
        .count()
}

/// Positive (Value empty-burden) — a program using only scalar-`Value` types
/// (`int`/`float`/`bool`) emits ZERO RC operations: the empty `UserBurdenSpec`
/// for Value types means the burden walker lands no `BurdenInc`/`BurdenDec`,
/// so codegen emits no `ori_rc_inc`/`ori_rc_dec`. Clean AOT run, zero leaks,
/// eval/LLVM parity.
#[test]
fn test_value_only_program_emits_zero_rc_ops() {
    // `@main` returns 0 — the process exit code is `@main`'s return value, so
    // a non-zero return would be misread as a panic/leak signal by the AOT
    // harness. The Value-typed bindings still exercise the empty-burden path.
    let source = r#"
@main () -> int = {
    let a = 1;
    let b = 2.5;
    let c = true;
    let d = a + 1;
    0
}
"#;
    let ir = compile_and_capture_ir(source);
    assert_eq!(
        rc_op_count(&ir),
        0,
        "Value-only program must emit zero RC operations; IR:\n{ir}"
    );

    // Zero-leak AOT run (assert_aot_success enables ORI_CHECK_LEAKS=1).
    assert_aot_success(source, "value_only_program_emits_zero_rc_ops");

    // Eval/LLVM parity: interpreter and AOT agree on exit code.
    let (aot_exit, _, _) = compile_and_run_capture(source);
    let eval_exit = interpreter_exit_code(source);
    assert_eq!(
        eval_exit, aot_exit,
        "eval/LLVM parity: interpreter exit {eval_exit} != AOT exit {aot_exit}"
    );
}

/// Negative-space (Value empty-burden) — the empty-burden path is NOT
/// vacuous: a heap-bearing program (a `str` binding) DOES emit at least one
/// RC dec. This clamps the positive pin from below — proving the zero-RC
/// result above is the Value path, not a codegen that never emits RC.
#[test]
fn test_heap_program_emits_rc_ops() {
    let source = r#"
@main () -> int = {
    let s = "heap-bearing";
    s.length()
}
"#;
    let ir = compile_and_capture_ir(source);
    assert!(
        rc_op_count(&ir) >= 1,
        "heap-bearing program must emit at least one RC operation (clamps the Value zero-RC pin from below); IR:\n{ir}"
    );
}

/// Mixed-Value/Heap composition — `Result<int, str>`: the `Ok(int)` arm
/// inherits the empty Value burden (no drop), the `Err(str)` arm inherits the
/// full heap burden (str RC dec). The Err-path program emits `ori_str_rc_dec`;
/// zero-leak AOT run on BOTH arms; eval/LLVM parity.
#[test]
fn test_result_value_heap_emits_asymmetric_drop() {
    // Err arm taken — the str must be released (heap burden). `@main` returns
    // 0 (exit-code constraint); the matched value is bound + discarded so the
    // Result is genuinely constructed, matched, and dropped along the heap arm.
    let err_source = r#"
@make (ok: bool) -> Result<int, str> = {
    if ok then Ok(42) else Err("boom")
}
@main () -> int = {
    let r = make(ok: false);
    let _v = match r {
        Ok(v) -> v,
        Err(_) -> 0,
    };
    0
}
"#;
    let ir = compile_and_capture_ir(err_source);
    assert!(
        ir.contains("ori_str_rc_dec"),
        "Result<int, str> must emit str RC dec for the heap-bearing Err arm; IR:\n{ir}"
    );
    assert_aot_success(err_source, "result_value_heap_err_arm_releases_str");

    // Ok arm taken — the int payload carries no drop; still zero leaks.
    let ok_source = r#"
@make (ok: bool) -> Result<int, str> = {
    if ok then Ok(42) else Err("boom")
}
@main () -> int = {
    let r = make(ok: true);
    let _v = match r {
        Ok(v) -> v,
        Err(_) -> 0,
    };
    0
}
"#;
    assert_aot_success(ok_source, "result_value_heap_ok_arm_no_int_drop");

    // Eval/LLVM parity on both arms.
    let (err_aot, _, _) = compile_and_run_capture(err_source);
    assert_eq!(
        interpreter_exit_code(err_source),
        err_aot,
        "eval/LLVM parity (Err arm): interpreter != AOT"
    );
    let (ok_aot, _, _) = compile_and_run_capture(ok_source);
    assert_eq!(
        interpreter_exit_code(ok_source),
        ok_aot,
        "eval/LLVM parity (Ok arm): interpreter != AOT"
    );
}
