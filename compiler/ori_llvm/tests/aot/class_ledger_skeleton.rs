//! End-to-end correctness for the class-ledger emitter's walking-skeleton
//! program family (the emitter is unconditional at the Step-4b slot).
//!
//! Each fixture compiles + runs under the gated burden-sole probe
//! (`ORI_DISABLE_PREDICATE_STACK_RC=1 ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1`).
//! `ORI_VERIFY_ARC=1` makes VF-1 / VF-1.1 burden-balance violations abort
//! compilation, so a clean compile + run IS the burden-balance-zero
//! evidence. The run step always sets `ORI_CHECK_LEAKS=1` (see
//! `compile_and_run_with_build_env`), so a leak-free stderr is the
//! zero-leak evidence. Each fixture also runs through the interpreter
//! (`ori run`) and asserts interp stdout == AOT stdout.
//!
//! Matrix dimensions (walking-skeleton shapes): straight-line fresh construct
//! moved through return; fresh value read then dead; fresh aggregate with a
//! funded heap field read then dead; dead-on-arrival owned argument; a branch
//! where a value dies on one arm; a loop threading a loop-invariant value.
//!
//! Per-function replacement ENGAGEMENT (which functions the emitter actually
//! replaces vs falls back on) is pinned by the `ori_arc` pipeline tests
//! (`pipeline/tests.rs` + `aims/class_ledger/tests.rs`); this file pins
//! end-to-end behavior of the whole binary.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use std::process::Command;

use crate::util::{compile_and_run_with_build_env, ori_binary, stdlib_path};

/// The gated burden-sole probe — the sanctioned RC/AOT verdict surface.
const PROBE: &[(&str, &str)] = &[
    ("ORI_DISABLE_PREDICATE_STACK_RC", "1"),
    ("ORI_VERIFY_ARC", "1"),
    ("ORI_VERIFY_EACH", "1"),
];

/// Assert `stderr` carries no leak / double-free report (the run step always
/// sets `ORI_CHECK_LEAKS=1`).
fn assert_leak_free(stderr: &str, label: &str, leg: &str) {
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[{label}] {leg} run reported a leak / double-free\nstderr:\n{stderr}"
    );
}

/// Run `source` through the interpreter (`ori run`) and return
/// `(exit_code, stdout)`. Used for eval/LLVM parity alongside the AOT runs.
fn interpreter_run(source: &str) -> (i32, String) {
    let temp_dir = tempfile::TempDir::new().expect("temp dir");
    let source_path = temp_dir.path().join("class_ledger_skeleton_parity.ori");
    std::fs::write(&source_path, source).expect("write source");
    let run = Command::new(ori_binary())
        .args(["run", source_path.to_str().unwrap()])
        .env("ORI_STDLIB", stdlib_path())
        .output()
        .expect("execute ori run");
    let stdout = String::from_utf8_lossy(&run.stdout).to_string();
    (run.status.code().unwrap_or(-1), stdout)
}

/// Compile + run `source` under the gated burden-sole probe, asserting:
///
/// 1. The run compiles and exits 0 (under `ORI_VERIFY_ARC=1` a VF-1 / VF-1.1
///    burden-balance violation aborts compilation, so exit 0 IS the
///    burden-balance-zero evidence).
/// 2. The run reports zero leaks under the always-on `ORI_CHECK_LEAKS=1`.
/// 3. The interpreter produces the same stdout and exit code as the AOT
///    binary (eval/LLVM parity).
fn assert_class_ledger_green(source: &str, label: &str) {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(source, PROBE);
    assert_eq!(
        exit, 0,
        "[{label}] burden-sole run must compile and exit 0 (ORI_VERIFY_ARC \
         aborts compilation on a burden-balance violation, so a non-zero \
         exit here is a class-ledger planning defect), got \
         {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert_leak_free(&stderr, label, "burden-sole");
    assert!(
        !stdout.trim().is_empty(),
        "[{label}] fixture must print a deterministic value (empty stdout \
         would make the assertions vacuous)"
    );

    // Eval-side parity: interpreter output == AOT output.
    let (interp_exit, interp_stdout) = interpreter_run(source);
    assert_eq!(
        interp_exit, exit,
        "[{label}] eval/LLVM parity: interpreter exit {interp_exit} != \
         AOT exit {exit}"
    );
    assert_eq!(
        interp_stdout, stdout,
        "[{label}] eval/LLVM parity: interpreter stdout != AOT stdout"
    );
}

/// Straight-line fresh construct moved through return: a fresh heap str born
/// in the callee transfers out via `Return`; the caller prints and drops it.
#[test]
fn skeleton_fresh_construct_move_return() {
    let src = r#"
@make () -> str = {
    let s = "fresh heap string well past the sso inline threshold here";
    s
}

@main () -> int = {
    let r = make();
    print(msg: r);
    0
}
"#;
    assert_class_ledger_green(src, "fresh_construct_move_return");
}

/// Fresh value read then dead: a fresh heap str borrow-read once, then dead
/// at scope exit — the class owes exactly one release after the last read.
#[test]
fn skeleton_fresh_read_dead() {
    let src = r#"
@main () -> int = {
    let s = "fresh heap string read once then dead past sso threshold";
    let n = s.len();
    print(msg: `{n}`);
    0
}
"#;
    assert_class_ledger_green(src, "fresh_read_dead");
}

/// Fresh aggregate with a funded heap field read then dead: the struct's
/// construct funds its field slot; a borrow-read of the field precedes the
/// aggregate's single release. Single heap field keeps the fixture inside
/// the walking-skeleton family (no mixed scalar/heap projection interplay).
#[test]
fn skeleton_aggregate_funded_field_read_dead() {
    let src = r#"
type Holder = { label: str }

@main () -> int = {
    let h = Holder {
        label: "single field heap string well past the sso threshold",
    };
    let n = h.label.len();
    print(msg: `{n}`);
    0
}
"#;
    assert_class_ledger_green(src, "aggregate_funded_field_read_dead");
}

/// Dead-on-arrival value: an owned heap argument the callee never uses —
/// the class owes one release at entry, nothing else.
#[test]
fn skeleton_dead_on_arrival_argument() {
    let src = r#"
@double_only (s: str, n: int) -> int = n * 2;

@main () -> int = {
    let r = double_only(
        s: "dead on arrival heap string well past the sso threshold",
        n: 21,
    );
    print(msg: `{r}`);
    0
}
"#;
    assert_class_ledger_green(src, "dead_on_arrival_argument");
}

/// Branch where a value dies on one arm: the fresh str is read on the then
/// arm and dies unread on the else arm; both arms are exercised.
#[test]
fn skeleton_branch_value_dies_on_one_arm() {
    let src = r#"
@classify (flag: bool) -> int = {
    let s = "branch arm heap string well past the sso inline threshold";
    if flag then s.len() else 7
}

@main () -> int = {
    let a = classify(flag: true);
    let b = classify(flag: false);
    print(msg: `{a} {b}`);
    0
}
"#;
    assert_class_ledger_green(src, "branch_value_dies_on_one_arm");
}

/// Loop threading a loop-invariant value: a fresh heap str born before the
/// loop is borrow-read each iteration and released once after the loop.
#[test]
fn skeleton_loop_invariant_value_threaded() {
    let src = r#"
@main () -> int = {
    let s = "loop invariant heap string well past the sso threshold";
    let total = 0;
    for i in 0..4 do {
        total = total + s.len() + i;
    };
    print(msg: `{total}`);
    0
}
"#;
    assert_class_ledger_green(src, "loop_invariant_value_threaded");
}

/// Adversarial family row: a struct with a heap field through a NESTED
/// identity-forwarder chain (single hop is green everywhere). The
/// class-ledger planner's forwarder CREDIT-passthrough family accounts the
/// chain correctly.
#[test]
fn skeleton_nested_forwarder_struct_ledger_green() {
    let src = r#"
type Named = { name: str, id: int }

@id_named (p: Named) -> Named = p;

@main () -> int = {
    let p = id_named(p: id_named(p: Named { name: "abcdefghijklmnopqrstuvwxyz1234", id: 7 }));
    let n = p.name.length();
    print(msg: `{n}`);

    if n == 30 && p.id == 7 then 0 else 1
}
"#;
    assert_class_ledger_green(src, "nested_forwarder_struct");
}

/// Adversarial family row: a closure capturing a heap payload, invoked
/// twice then dead. The closure-env release must DOMINATE the captured
/// payload's release — the env's single scope-exit dec cascade-frees the
/// capture exactly once; the class-ledger emitter accounts the env class
/// correctly.
#[test]
fn skeleton_closure_env_release_dominates_capture_ledger_green() {
    let src = r#"
@main () -> int = {
    let payload = "captured heap payload string exceeding sso threshold";
    let f = (n: int) -> int = payload.length() + n;
    let a = f(1);
    let b = f(2);
    print(msg: `{a} {b}`);

    if a + b == payload.length() * 2 + 3 then 0 else 1
}
"#;
    assert_class_ledger_green(src, "closure_env_release_dominates_capture");
}
