//! Closure `RcDec` edge-placement probe — an OWNED closure (`PartialApply` result)
//! borrowed at a terminator-`Invoke` arg (the `op` of `xs.fold(init, op)`) is
//! alive AT the Invoke and dead at BOTH successors. Per Annex E §AIMS RL-4
//! (`RL4_edge_dec_decision`) + RL-2 (`RL2_borrowed_param_emits_caller_dec`) the
//! caller's compensating `RcDec` lands once on EVERY dead successor edge (the
//! normal `Return` edge AND the unwind `Resume` edge), never just one.
//!
//! Defect (burden-sole path): the Phase-5 burden walk places the owned closure's
//! release as an INLINE self-cancelling `BurdenInc`/`BurdenDec` pair in the
//! defining block; Phase-3 coalesce erases it, losing the NORMAL-path release.
//! Phase-6.98 (`emit_invoke_unwind_pair_release`) supplies the UNWIND edge only;
//! Phase-6.69 (`emit_owned_closure_scope_exit_dec`) skips the closure via its
//! `reps_with_burden` guard -> the env (32-byte `PartialApply` allocation + its
//! RC-bearing captures) leaks on every NORMAL execution. Capture-less lambdas
//! mask it (null env -> `RcDec [Closure]` is a no-op). Fix: burden-path Phase-5/
//! 6.6x edge relocation (relocate the inline dec to the normal successor; sibling
//! to Phase-6.65 `relocate_borrowed_terminator_arg_dec_to_edges`) — NOT the legacy
//! predicate-stack `compute_invoke_edge_dead_set` (narrowing Cat-2 would double-dec).
//!
//! VERDICT SURFACE — burden-sole ONLY (arc.md §STOP): the default path runs the
//! predicate stack, which masks this burden-path leak (FALSE-GREEN). Every cell
//! compiles with `ORI_DISABLE_PREDICATE_STACK_RC=1` (burden is the sole real-RC
//! emitter) + runs under the always-on `ORI_CHECK_LEAKS=1`. `assert_aot_success`
//! (default path) is WRONG for this bug; this probe uses the burden-sole harness.
//!
//! Matrix: captured-type dimension (heap str / [int] / Option<str> / {str:int} /
//! nested [[int]]) over the `fold` borrowed-closure consumer (positive pins,
//! leak pre-fix); the capture-less fold (negative pin — the gate must NOT
//! over-fire where the env is null). Subprocess-isolated — parallel-safe.
//! Spec: Annex E §AIMS RL-2 + RL-4. Regression: BUG-04-158.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::compile_and_run_with_build_env;

/// Compile `source` with the predicate-stack RC emitter OFF (burden path is the
/// sole real-RC emitter, the only valid RC verdict surface per arc.md §STOP) and
/// run under the always-on `ORI_CHECK_LEAKS=1`. Asserts exit 0 with no FATAL
/// double-free / leak diagnostic on stderr.
fn assert_no_closure_leak_burden_sole(source: &str, label: &str) {
    let (exit, stdout, stderr) =
        compile_and_run_with_build_env(source, &[("ORI_DISABLE_PREDICATE_STACK_RC", "1")]);
    assert!(
        exit == 0,
        "[{label}] burden-sole run exited {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[{label}] burden-sole run reported a closure-env leak / double-free\nstderr:\n{stderr}"
    );
}

// ----- Positive pins: capture-bearing closure folded over a list. The closure
// env holds the RC-bearing capture; the missing normal-edge RcDec leaks the env
// + cascades to the capture. Each LEAKS pre-fix on the burden-sole path. -----

/// Captured-type = heap `str` (>23 bytes, no SSO). Env (32B) + str buffer leak
/// on the normal path pre-fix. The confirmed repro shape.
#[test]
fn fold_capture_heap_str_no_leak() {
    let src = r#"
@fold_capture (items: [int], base: str) -> int = {
    items.fold(initial: 0, op: (acc, x) -> acc + x + base.length())
}

@main () -> int = {
    let r = fold_capture(items: [10, 20, 30], base: "a heap string longer than twenty-three bytes");
    print(msg: `{r}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "fold_capture_heap_str");
}

/// Captured-type = `[int]` (heap list buffer). Env + list buffer leak pre-fix.
#[test]
fn fold_capture_list_no_leak() {
    let src = r#"
@fold_capture_list (items: [int], extra: [int]) -> int = {
    items.fold(initial: 0, op: (acc, x) -> acc + x + extra.length())
}

@main () -> int = {
    let r = fold_capture_list(items: [1, 2, 3], extra: [9, 8, 7, 6]);
    print(msg: `{r}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "fold_capture_list");
}

/// Captured-type = `Option<str>` (sum-aggregate capture; niche-payload `elem_dec`).
/// Uses a `match` on the captured `tag` (the `.map().unwrap_or()` combinator chain
/// does not infer inside the fold lambda — E2005). Leaks #1 size=44 (str) + #2
/// size=40 (env) pre-fix; result `98`.
#[test]
fn fold_capture_option_str_no_leak() {
    let src = r#"
@fold_capture_opt (items: [int], tag: Option<str>) -> int = {
    items.fold(initial: 0, op: (acc, x) -> acc + x + match tag {
        Some(s) -> s.length(),
        None -> 0
    })
}

@main () -> int = {
    let r = fold_capture_opt(items: [5, 5], tag: Some("a heap string longer than twenty-three bytes"));
    print(msg: `{r}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "fold_capture_option_str");
}

/// Captured-type = `{str: int}` map (fat-pointer buffer + key `elem_dec`).
#[test]
fn fold_capture_map_no_leak() {
    let src = r#"
@fold_capture_map (items: [int], lut: {str: int}) -> int = {
    items.fold(initial: 0, op: (acc, x) -> acc + x + lut.length())
}

@main () -> int = {
    let r = fold_capture_map(items: [2, 2, 2], lut: {"alpha heap key longer than twentythree": 1});
    print(msg: `{r}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "fold_capture_map");
}

/// Captured-type = nested `[[int]]` (deep `elem_dec` cascade through the closure
/// env drop).
#[test]
fn fold_capture_nested_list_no_leak() {
    let src = r#"
@fold_capture_nested (items: [int], grid: [[int]]) -> int = {
    items.fold(initial: 0, op: (acc, x) -> acc + x + grid.length())
}

@main () -> int = {
    let r = fold_capture_nested(items: [4, 4], grid: [[1, 2], [3, 4], [5, 6]]);
    print(msg: `{r}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "fold_capture_nested_list");
}

// ----- Scalar-only-capture cell: ISOLATES the env-allocation dec from the
// capture cascade. The closure captures a SCALAR (`int offset`) — the env is a
// non-null 32-byte `PartialApply` allocation, but the capture carries NO RC, so
// the missing normal-edge dec leaks ONLY the env (one 32B allocation, no
// cascade). Proves the leak is the closure ENV itself, not merely the captured
// heap value. Distinct from the capture-less negative pin (null env, no leak).
// -----

/// Captured-type = `int` scalar (non-null env, no RC capture). Leaks ONLY the
/// 32-byte env pre-fix — the env-alloc-dec isolation control.
#[test]
fn fold_capture_scalar_env_only_no_leak() {
    let src = r#"
@fold_capture_scalar (items: [int], offset: int) -> int = {
    items.fold(initial: 0, op: (acc, x) -> acc + x + offset)
}

@main () -> int = {
    let r = fold_capture_scalar(items: [1, 2, 3], offset: 100);
    print(msg: `{r}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "fold_capture_scalar_env_only");
}

// ----- Second consumer axis: `all` (a borrowed predicate-closure consumer
// distinct from `fold`). Confirms the edge-dec gap is the consumer-agnostic
// borrowed-closure shape, not a fold-specific lowering. -----

/// Consumer = `all` (capture-bearing predicate closure). Same borrowed-closure
/// terminator-Invoke shape as fold; leaks the env + capture pre-fix.
#[test]
fn all_capture_heap_str_no_leak() {
    let src = r#"
@all_capture (items: [int], base: str) -> bool = {
    items.all(pred: (x) -> x < base.length())
}

@main () -> int = {
    let r = all_capture(items: [1, 2, 3], base: "a heap string longer than twenty-three bytes");
    print(msg: `{r}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "all_capture_heap_str");
}

// ----- User-defined-struct capture cell: covers the NON-BUILTIN aggregate capture
// path through the closure env drop. The closure captures a `Rec { val: str, count:
// int }` — the env drop must cascade through the user-struct's drop glue to free the
// captured `str`. Distinct from the builtin-aggregate cells (`[int]`/`{str:int}`/
// `[[int]]`): exercises the user-`type` drop-function generation, not a builtin
// elem_dec_fn. -----

/// Captured-type = user-defined struct `Rec { val: str, count: int }`. Env drop
/// cascades through the struct's drop glue to the captured str. Leaks #1 size=44
/// (str) + #2 size=40 (struct/env) pre-fix on the burden-sole path.
#[test]
fn fold_capture_user_struct_no_leak() {
    let src = r#"
type Rec = { val: str, count: int }

@fold_capture_struct (items: [int], rec: Rec) -> int = {
    items.fold(initial: 0, op: (acc, x) -> acc + x + rec.val.length() + rec.count)
}

@main () -> int = {
    let r = fold_capture_struct(items: [1, 2, 3], rec: Rec { val: "a heap string longer than twenty-three bytes", count: 42 });
    print(msg: `{r}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "fold_capture_user_struct");
}

// ----- Negative pin: capture-less fold. The env is null -> the missing
// normal-edge dec is a no-op -> clean BEFORE and AFTER the fix. The cure's
// normal-edge gate MUST NOT over-fire here (no spurious dec on a null env). This
// is the masking shape from the seed snapshot. -----

/// Capture-less lambda folded over a list — the gate emits NO closure dec (null
/// env). Clean on the burden-sole path before AND after the fix.
#[test]
fn fold_captureless_clean_no_regression() {
    let src = r#"
@sum_list (items: [int]) -> int = {
    items.fold(initial: 0, op: (acc, x) -> acc + x)
}

@main () -> int = {
    let r = sum_list(items: [10, 20, 30]);
    print(msg: `{r}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "fold_captureless_clean");
}
