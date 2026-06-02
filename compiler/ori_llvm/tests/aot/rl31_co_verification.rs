//! RL-31 metadata × Phase-6 co-verification sweep — AOT mirror.
//!
//! Spec: Annex E §AIMS RL-31 + RL-22..RL-27. Mirrors the Ori spec test at
//! `tests/spec/aims/rl31_co_verification.ori` through full Phase-5 emission +
//! Phase-6 elimination/motion/flush + LLVM lowering + execution. Each shape
//! exercises a distinct Phase-6 interaction (`KnownSafe` pair elimination,
//! PRE-style RC motion across a barrier, selective barrier flush) while
//! passing two DISTINCT-type collection params to a function the RL-31
//! 8-clause rule proves disjoint.
//!
//! Two assertions per shape:
//! - **IR**: the RL-31-claimed function (`_ori_merge`) carries NO `noalias` on
//!   its pointer params (Spec: Annex E §AIMS RL-31 (P2) dual-facet). The
//!   type-level facet (b) proves the distinct-type closures disjoint, but the
//!   function-attribute `noalias` requires the per-call-site provenance facet
//!   (a) too, and (a) is unshipped — so `noalias` is conservatively omitted.
//!   The Phase-6 passes operate on RC-op placement, never on this gate, so the
//!   omission is invariant to each interaction.
//! - **Execution**: the program runs with the correct result and zero leaks
//!   (the Phase-6 interaction did not corrupt RC balance).

use crate::util::{compile_and_capture_ir, compile_and_run_capture, extract_function_ir};

/// Assert `_ori_merge` carries NO `noalias` on its pointer params in `source`'s
/// emitted IR — the RL-31 (P2) dual-facet gate omits the function-attribute
/// `noalias` while the per-call-site provenance facet (a) is unshipped, and the
/// Phase-6 interaction does not change that. If facet (a) ships, this flips to
/// asserting `noalias` survives on both merge params.
fn assert_merge_noalias_omitted(source: &str, shape: &str) {
    let ir = compile_and_capture_ir(source);
    let merge_ir = extract_function_ir(&ir, "_ori_merge");
    let define_line = merge_ir
        .lines()
        .find(|l| l.starts_with("define ") && l.contains("@_ori_merge("))
        .unwrap_or_else(|| panic!("{shape}: no _ori_merge define line.\nIR:\n{merge_ir}"));
    let noalias_count = define_line.matches("ptr noalias").count();
    assert_eq!(
        noalias_count, 0,
        "{shape}: RL-31 noalias must be OMITTED on merge params while facet (a) \
         is unshipped (found {noalias_count} `ptr noalias` on define line).\nDefine: {define_line}"
    );
}

/// Assert the program compiles, runs to exit 0, and leaks zero memory.
fn assert_runs_clean(source: &str, expected_stdout: &str, shape: &str) {
    let (exit_code, stdout, stderr) = compile_and_run_capture(source);
    assert_eq!(
        exit_code, 0,
        "{shape}: expected clean exit (0=ok, 2=leak), got {exit_code}.\nstderr:\n{stderr}"
    );
    assert_eq!(
        stdout.trim(),
        expected_stdout,
        "{shape}: wrong result.\nstderr:\n{stderr}"
    );
}

/// Shape (a): `KnownSafe` pair elimination does not perturb the RL-31 omission.
/// `m["x"]=10, l.len()=3` → first=13; again=10 → 23.
#[test]
fn rl31_known_safe_pair_elim() {
    let source = include_str!("fixtures/rl31_co_verification/known_safe_pair_elim.ori");
    assert_merge_noalias_omitted(source, "known_safe_pair_elim");
    assert_runs_clean(source, "23", "known_safe_pair_elim");
}

/// Shape (b): PRE-style RC motion blocked at an RC-observable barrier
/// does not perturb the RL-31 omission. merged=7+2=9; consumed=7 → 16.
#[test]
fn rl31_pre_motion_barrier_blocked() {
    let source = include_str!("fixtures/rl31_co_verification/pre_motion_barrier_blocked.ori");
    assert_merge_noalias_omitted(source, "pre_motion_barrier_blocked");
    assert_runs_clean(source, "16", "pre_motion_barrier_blocked");
}

/// Shape (c): selective barrier flush does not perturb the RL-31 omission
/// (the param-level gate is invariant to RC-op flush). shared=5; merged=2+1=3 → 8.
#[test]
fn rl31_selective_barrier_flush() {
    let source = include_str!("fixtures/rl31_co_verification/selective_barrier_flush.ori");
    assert_merge_noalias_omitted(source, "selective_barrier_flush");
    assert_runs_clean(source, "8", "selective_barrier_flush");
}
