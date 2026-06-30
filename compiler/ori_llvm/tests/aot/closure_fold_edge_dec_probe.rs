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
//! VERDICT SURFACE — burden-sole ONLY: the default codegen path co-emits the
//! legacy predicate-stack RC, which masks this burden-path leak (FALSE-GREEN), so
//! it is NOT a valid RC verdict for these cells. Every cell compiles with
//! `ORI_DISABLE_PREDICATE_STACK_RC=1` (burden is the sole real-RC emitter) + runs
//! under the always-on `ORI_CHECK_LEAKS=1`. `assert_aot_success` (default path) is
//! WRONG for this bug; this probe uses the burden-sole harness.
//!
//! Matrix: captured-type dimension (heap str / [int] / Option<str> / {str:int} /
//! nested [[int]] / user-struct / scalar-env) over the `fold` + `all` borrowed-
//! closure consumers (positive pins, leak pre-fix). Negative pins — the gate must
//! NOT over-fire: capture-less fold (null env), and the EXCLUSION boundary
//! (param closure / borrowed-view closure / iter-element closure), each capturing
//! a heap str so an over-fire double-frees it. Subprocess-isolated — parallel-safe.
//! Spec: Annex E §AIMS RL-2 + RL-4. Regression: closure-arg edge-dec placement.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::compile_and_run_with_build_env;

/// Compile `source` on the COMPLETE gated burden probe surface (the only valid RC
/// verdict surface for these cells): predicate-stack RC emitter OFF
/// (`ORI_DISABLE_PREDICATE_STACK_RC=1` — burden is the sole real-RC emitter) PLUS the per-
/// function LLVM verification (`ORI_VERIFY_ARC=1`) + post-pass verification
/// (`ORI_VERIFY_EACH=1`) so a burden-imbalance that compiles to wrong-but-non-
/// crashing IR (VR-1 checkpoints + VF-1 burden-balance residual) is caught, not
/// silently passed. Runs under the always-on `ORI_CHECK_LEAKS=1`. Asserts exit 0
/// with no FATAL double-free / leak diagnostic on stderr.
fn assert_no_closure_leak_burden_sole(source: &str, label: &str) {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(
        source,
        &[
            ("ORI_DISABLE_PREDICATE_STACK_RC", "1"),
            ("ORI_VERIFY_ARC", "1"),
            ("ORI_VERIFY_EACH", "1"),
        ],
    );
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

// Positive pins: capture-bearing closure folded over a list. The closure
// env holds the RC-bearing capture; the missing normal-edge RcDec leaks the env
// + cascades to the capture. Each LEAKS pre-fix on the burden-sole path.

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

// Scalar-only-capture cell: ISOLATES the env-allocation dec from the
// capture cascade. The closure captures a SCALAR (`int offset`) — the env is a
// non-null 32-byte `PartialApply` allocation, but the capture carries NO RC, so
// the missing normal-edge dec leaks ONLY the env (one 32B allocation, no
// cascade). Proves the leak is the closure ENV itself, not merely the captured
// heap value. Distinct from the capture-less negative pin (null env, no leak).

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

// Second consumer axis: `all` (a borrowed predicate-closure consumer
// distinct from `fold`). Confirms the edge-dec gap is the consumer-agnostic
// borrowed-closure shape, not a fold-specific lowering.

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

// User-defined-struct capture cell: covers the NON-BUILTIN aggregate capture
// path through the closure env drop. The closure captures a `Rec { val: str, count:
// int }` — the env drop must cascade through the user-struct's drop glue to free the
// captured `str`. Distinct from the builtin-aggregate cells (`[int]`/`{str:int}`/
// `[[int]]`): exercises the user-`type` drop-function generation, not a builtin
// elem_dec_fn.

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

// Negative pin: capture-less fold. The env is null -> the missing
// normal-edge dec is a no-op -> clean BEFORE and AFTER the fix. The cure's
// normal-edge gate MUST NOT over-fire here (no spurious dec on a null env). This
// is the masking shape from the seed snapshot.

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

// Negative pins: the EXCLUSION boundary the relocation gate must DECLINE.
// Each folds/threads a closure of an EXCLUDED category (param / borrowed-view /
// iter-element) capturing a heap `str`. The gate keys its `params` /
// `all_borrowed_defs` / `iter_element_defs` exclusion on `rep_of(arg)`, so an
// aliased excluded closure does NOT slip into a relocation. If the gate
// over-fired here it would relocate a dec it does not own -> double-free of the
// captured str. Each is clean BEFORE and AFTER the fix (the relocation never
// fires); a regression that widens the gate onto these categories crashes /
// leaks under the burden-sole probe. Spec: Annex E §AIMS RL-2 + RL-4.

/// Excluded category = PARAMETER closure. The `op` folded inside `apply_fold` is
/// a borrowed PARAM (in `params`), created + owned by `@main`. The relocation
/// MUST decline (`params.contains(rep_of(op))`) — `apply_fold` does not own `op`,
/// so relocating its dec would free the caller's still-live closure env.
#[test]
fn fold_param_closure_no_overfire() {
    let src = r#"
@apply_fold (items: [int], op: (int, int) -> int) -> int = {
    items.fold(initial: 0, op: op)
}

@main () -> int = {
    let base = "a heap string longer than twenty-three bytes";
    let r = apply_fold(items: [1, 2, 3], op: (acc, x) -> acc + x + base.length());
    print(msg: `{r}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "fold_param_closure");
}

/// Excluded category = BORROWED-VIEW closure. `red.f` is a `Project` of a struct
/// field (a borrow-view, in `all_borrowed_defs` / not an owned closure value).
/// The struct `red` owns the closure env; its drop frees the captured str. The
/// relocation MUST decline — relocating a dec onto the borrowed view would
/// double-free the captured str the struct still owns.
#[test]
fn fold_borrowed_view_closure_no_overfire() {
    let src = r#"
type Reducer = { f: (int, int) -> int }

@run_reducer (items: [int], red: Reducer) -> int = {
    items.fold(initial: 0, op: red.f)
}

@main () -> int = {
    let base = "a heap string longer than twenty-three bytes";
    let red = Reducer { f: (acc, x) -> acc + x + base.length() };
    let out = run_reducer(items: [4, 5, 6], red: red);
    print(msg: `{out}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "fold_borrowed_view_closure");
}

/// Excluded category = ITER-ELEMENT closure. `g` is each element of the `[closure]`
/// list `fns` (an iter-element view, in `iter_element_defs`), passed as the
/// borrowed terminator-`Invoke` arg of the INNER `items.fold(.. op: g)`. The list
/// `fns` owns the element closures; iterating it frees each via the element
/// `elem_dec`. The relocation MUST decline on `g` — over-firing would double-free
/// the element closure's captured str the list still owns.
#[test]
fn fold_iter_element_closure_no_overfire() {
    let src = r#"
@apply_all (fns: [(int, int) -> int], items: [int]) -> int = {
    fns.fold(initial: 0, op: (acc, g) -> acc + items.fold(initial: 0, op: g))
}

@main () -> int = {
    let base = "a heap string longer than twenty-three bytes";
    let fns: [(int, int) -> int] = [(acc, x) -> acc + x + base.length(), (acc, x) -> acc + x];
    let r = apply_all(fns: fns, items: [1, 2, 3]);
    print(msg: `{r}`);
    0
}
"#;
    assert_no_closure_leak_burden_sole(src, "fold_iter_element_closure");
}
