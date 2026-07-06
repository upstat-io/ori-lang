//! End-to-end behavior parity for the class-ledger emitter's walking-skeleton
//! program family under `ORI_CLASS_LEDGER_EMITTER=1`.
//!
//! Each fixture compiles + runs under the gated burden-sole probe
//! (`ORI_DISABLE_PREDICATE_STACK_RC=1 ORI_VERIFY_ARC=1 ORI_VERIFY_EACH=1`)
//! twice — once as the baseline, once with `ORI_CLASS_LEDGER_EMITTER=1`
//! added — and asserts identical exit codes and stdout between the two runs.
//! `ORI_VERIFY_ARC=1` makes VF-1 / VF-1.1 burden-balance violations abort
//! compilation, so a clean compile + run under the toggle IS the
//! burden-balance-zero evidence. The run step always sets `ORI_CHECK_LEAKS=1`
//! (see `compile_and_run_with_build_env`), so a leak-free toggled stderr is
//! the zero-leak evidence. Each fixture also runs through the interpreter
//! (`ori run`) and asserts interp stdout == toggled-AOT stdout.
//!
//! Matrix dimensions (walking-skeleton shapes): straight-line fresh construct
//! moved through return; fresh value read then dead; fresh aggregate with a
//! funded heap field read then dead; dead-on-arrival owned argument; a branch
//! where a value dies on one arm; a loop threading a loop-invariant value.
//!
//! Per-function replacement ENGAGEMENT (which functions the emitter actually
//! replaces vs falls back on) is pinned by the `ori_arc` pipeline tests
//! (`pipeline/tests.rs` + `aims/class_ledger/tests.rs`); this file pins
//! end-to-end behavior parity of the whole binary under the toggle.

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

/// Compile + run `source` under (a) the gated burden-sole probe alone and
/// (b) the probe plus `ORI_CLASS_LEDGER_EMITTER=1`, asserting:
///
/// 1. Both legs compile and exit 0 (under `ORI_VERIFY_ARC=1` a VF-1 / VF-1.1
///    burden-balance violation aborts compilation, so exit 0 on the toggled
///    leg IS the burden-balance-zero evidence).
/// 2. Exit codes and stdout are byte-identical between the two legs
///    (zero-new vs baseline on the covered family).
/// 3. Both legs report zero leaks under the always-on `ORI_CHECK_LEAKS=1`.
/// 4. The interpreter produces the same stdout and exit code as the toggled
///    AOT leg (eval/LLVM parity).
fn assert_class_ledger_toggle_parity(source: &str, label: &str) {
    // (a) Baseline: gated burden-sole probe alone.
    let (base_exit, base_stdout, base_stderr) = compile_and_run_with_build_env(source, PROBE);
    assert_eq!(
        base_exit, 0,
        "[{label}] baseline burden-sole run must compile and exit 0, got \
         {base_exit}\nstdout:\n{base_stdout}\nstderr:\n{base_stderr}"
    );
    assert_leak_free(&base_stderr, label, "baseline");

    // (b) Probe + class-ledger toggle.
    let mut toggled: Vec<(&str, &str)> = PROBE.to_vec();
    toggled.push(("ORI_CLASS_LEDGER_EMITTER", "1"));
    let (led_exit, led_stdout, led_stderr) = compile_and_run_with_build_env(source, &toggled);
    assert_eq!(
        led_exit, 0,
        "[{label}] class-ledger-toggled run must compile and exit 0 \
         (ORI_VERIFY_ARC aborts compilation on a burden-balance violation, so \
         a non-zero exit here is a class-ledger planning defect), got \
         {led_exit}\nstdout:\n{led_stdout}\nstderr:\n{led_stderr}"
    );
    assert_leak_free(&led_stderr, label, "class-ledger-toggled");

    // Identical observable behavior between the two legs.
    assert_eq!(
        base_stdout, led_stdout,
        "[{label}] stdout diverged between baseline and class-ledger-toggled \
         runs\nbaseline stderr:\n{base_stderr}\ntoggled stderr:\n{led_stderr}"
    );
    assert!(
        !led_stdout.trim().is_empty(),
        "[{label}] fixture must print a deterministic value (empty stdout \
         would make the parity assertions vacuous)"
    );

    // Eval-side parity: interpreter output == toggled AOT output.
    let (interp_exit, interp_stdout) = interpreter_run(source);
    assert_eq!(
        interp_exit, led_exit,
        "[{label}] eval/LLVM parity: interpreter exit {interp_exit} != \
         toggled AOT exit {led_exit}"
    );
    assert_eq!(
        interp_stdout, led_stdout,
        "[{label}] eval/LLVM parity: interpreter stdout != toggled AOT stdout"
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
    assert_class_ledger_toggle_parity(src, "fresh_construct_move_return");
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
    assert_class_ledger_toggle_parity(src, "fresh_read_dead");
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
    assert_class_ledger_toggle_parity(src, "aggregate_funded_field_read_dead");
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
    assert_class_ledger_toggle_parity(src, "dead_on_arrival_argument");
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
    assert_class_ledger_toggle_parity(src, "branch_value_dies_on_one_arm");
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
    assert_class_ledger_toggle_parity(src, "loop_invariant_value_threaded");
}
