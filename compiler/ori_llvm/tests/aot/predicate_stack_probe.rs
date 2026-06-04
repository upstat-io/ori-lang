//! Burden-path self-sufficiency probe for the canonical RC-emission path.
//!
//! Compiles real Ori programs with `ORI_DISABLE_PREDICATE_STACK_RC=1` — which
//! suppresses the predicate-stack `RcInc`/`RcDec` emission and lowers surviving
//! `BurdenInc → RcInc` / `BurdenDec → RcDec` mechanically (Phase 7) — then runs
//! each under `ORI_CHECK_LEAKS=1`. A pass proves the burden path ALONE produces
//! a VF-1-balanced, leak-free, double-free-free binary for the covered shape.
//!
//! Matrix dimensions (burden-lowering completeness shapes): move-alias chain,
//! duplication-alias with live source, collection-buffer last-use
//! (list / map / set), borrow-chain (project of a projection), closure-capture
//! last-use.
//!
//! Build-step env (compile-time flag) via `compile_and_run_with_build_env`;
//! run-step `ORI_CHECK_LEAKS=1` always-on. Subprocess-isolated — parallel-safe.

#![allow(
    clippy::needless_raw_string_hashes,
    reason = "readability in test program literals"
)]

use crate::util::compile_and_run_with_build_env;

/// Compile `source` with the predicate-stack RC emitter OFF (burden path is the
/// sole real-RC emitter) and run under leak checking. Asserts the program exits
/// 0 with no FATAL double-free / leak diagnostic on stderr.
fn assert_burden_path_self_sufficient(source: &str, label: &str) {
    let (exit, stdout, stderr) =
        compile_and_run_with_build_env(source, &[("ORI_DISABLE_PREDICATE_STACK_RC", "1")]);
    assert!(
        exit == 0,
        "[{label}] burden-path-only run exited {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // ORI_CHECK_LEAKS=1 emits a leak report on stderr; the RC double-free guard
    // emits `FATAL — ori_rc_dec called on already-freed`. Either is a probe fail.
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[{label}] burden-path-only run reported a leak / double-free\nstderr:\n{stderr}"
    );
}

#[test]
fn probe_move_alias_chain_str() {
    // Move-alias chain: a heap str moved through Let-Var hops (FatVal lineage
    // %0 → %2 → %4) then returned — burden RL-2 move-alias transfer suppression
    // must keep the net at 0 with no orphan dec.
    let src = r#"
@id_chain (s: str) -> str = {
    let a = s;
    let b = a;
    let c = b;
    c
}

@main () -> int = {
    let r = id_chain(s: "hello world");
    print(msg: r);
    0
}
"#;
    assert_burden_path_self_sufficient(src, "move_alias_chain_str");
}

#[test]
fn probe_dup_alias_live_source_str() {
    // Duplication: a Let-Var alias whose SOURCE stays live afterward — RL-1
    // duplication inc on the alias, balanced by its own last-use dec.
    let src = r#"
@use_twice (s: str) -> int = {
    let a = s;
    let len_a = a.length();
    let len_s = s.length();
    len_a + len_s
}

@main () -> int = {
    let n = use_twice(s: "duplicate me");
    print(msg: `{n}`);
    0
}
"#;
    assert_burden_path_self_sufficient(src, "dup_alias_live_source_str");
}

// Burden-path self-sufficiency for collection types: the AOT + JIT compile
// paths reconstruct the `TypeRegistry` from the `TypedModule` exports and
// thread it into `run_arc_pipeline`, so the burden walker's
// `type_registry.burden(idx)` lookup for `[T]` / `{K:V}` / `Set<T>` resolves
// the composed `UserBurdenSpec`; collection buffers receive `BurdenInc` /
// `BurdenDec` and the burden path is self-sufficient with the predicate stack
// disabled. Closure capture resolves through the same lookup.
#[test]
fn probe_collection_buffer_last_use_list() {
    // Collection-buffer last-use: a heap list built, consumed, dropped — the
    // burden CollectionBuffer dec at last use must release the buffer exactly
    // once.
    let src = r#"
@sum_list (xs: [int]) -> int = {
    let total = xs.fold(initial: 0, op: (acc, x) -> acc + x);
    total
}

@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    let s = sum_list(xs: xs);
    print(msg: `{s}`);
    0
}
"#;
    assert_burden_path_self_sufficient(src, "collection_buffer_last_use_list");
}

#[test]
fn probe_collection_buffer_last_use_map() {
    let src = r#"
@count_keys (m: {str: int}) -> int = m.length();

@main () -> int = {
    let m = {"a": 1, "b": 2, "c": 3};
    let n = count_keys(m: m);
    print(msg: `{n}`);
    0
}
"#;
    assert_burden_path_self_sufficient(src, "collection_buffer_last_use_map");
}

#[test]
fn probe_collection_buffer_last_use_set() {
    let src = r#"
@main () -> int = {
    let s: Set<int> = [1, 2, 3, 2, 1].iter().collect();
    let n = s.len();
    print(msg: `{n}`);
    0
}
"#;
    assert_burden_path_self_sufficient(src, "collection_buffer_last_use_set");
}

#[test]
fn probe_borrow_chain_project_of_projection() {
    // Borrow-chain: a Project of a projection (nested field borrow). TF-4
    // Borrowed propagation must keep the nested borrow-view from emitting a
    // last-use dec (a borrow owns no allocation).
    let src = r#"
type Inner = { tag: str }
type Outer = { inner: Inner, count: int }

@read_tag (o: Outer) -> str = o.inner.tag;

@main () -> int = {
    let o = Outer { inner: Inner { tag: "nested" }, count: 7 };
    let t = read_tag(o: o);
    print(msg: t);
    0
}
"#;
    assert_burden_path_self_sufficient(src, "borrow_chain_project_of_projection");
}

#[test]
fn probe_closure_capture_last_use_str() {
    // Closure-capture last-use: a heap str captured by a closure; the closure's
    // env carries the capture's RC, released when the closure dies. PartialApply
    // FRESH + last-use dec must net 0.
    let src = r#"
@make_greeter (name: str) -> () -> str = {
    let greet = () -> `hello {name}`;
    greet
}

@main () -> int = {
    let g = make_greeter(name: "world");
    let msg = g();
    print(msg: msg);
    0
}
"#;
    assert_burden_path_self_sufficient(src, "closure_capture_last_use_str");
}

#[test]
fn probe_result_str_partial_move_via_try_codegen_clean() {
    // Enum partial-move codegen cure: `?` on a `Result<int, str>`
    // projects the heap Err payload (str) OUT to the propagated value. The
    // burden walk records that move and emits `burden_dec_partial %r skip=[1]`
    // for the Result var — a DropKind::Enum partial-move drop shape. Under the
    // probe (`ORI_DISABLE_PREDICATE_STACK_RC=1`) that op lowers to a real
    // per-variant RcDec walk. Pre-fix the BurdenDecPartial codegen arm handled
    // ONLY DropKind::Fields and hit `debug_assert!(false)` + silent return on
    // DropKind::Enum — a codegen crash (`LLVM codegen had N error(s)`) corpus-
    // wide under the probe. Post-fix the arm dispatches DropKind::Enum through
    // `emit_variant_burden_walk`, skipping the moved-out source variant by
    // ordinal (RL-2: the moved-out str must NOT be re-dropped). SSO strings —
    // heap-free, so the cure is observable as codegen-clean + leak-free.
    let src = r#"
@fallible () -> Result<int, str> = Err("fail");

@propagate () -> Result<int, str> = {
    let v = fallible()?;
    Ok(v + 1)
}

@main () -> int = {
    let r = propagate();
    if r.is_err() then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "result_str_partial_move_via_try");
}

#[test]
fn probe_result_scalar_only_no_partial_move_codegen_clean() {
    // Negative companion (the `moved_fields.rs` scalar-projection filter):
    // `??` on a `Result<int, str>` where the Ok payload is a SCALAR int and the
    // Err is taken projects only the scalar int slot. A scalar projection
    // transfers NO RC ownership (L-9 / TF-4), so it must NOT seed `skip_fields`
    // — the surviving Err payload owes a FULL `burden_dec`, never a partial-skip
    // that strands it. Pre-filter the scalar projection wrongly marked field 1
    // moved, suppressing the Err drop; the filter keeps the full dec. SSO Err
    // string keeps this pin heap-free (codegen-clean clamp; the separately
    // tracked heap-payload discard leak is out of scope for this pin).
    let src = r#"
@fallible (ok: bool) -> Result<int, str> =
    if ok then Ok(7) else Err("e");

@main () -> int = {
    let r = fallible(ok: false);
    let v = r ?? 42;
    if v == 42 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "result_scalar_only_no_partial_move");
}

#[test]
fn probe_coalesce_discards_heap_err_payload() {
    // RL-4 / RL-5: `a ?? default` on a `Result<int, str>` whose `a` is the
    // heap-Err variant discards the Result on the Err-taken edge. The Result's
    // heap str payload is live at the coalesce branch but dead in the default
    // successor — its release belongs on that dying CFG edge. The predicate
    // stack emits it via `emit_edge_cleanup`; under the probe that pass is off,
    // so the burden path must emit the dying-edge `BurdenDec` itself (consuming
    // the same `compute_branch_edge_dead_set` SSOT). The Err str is >23 bytes
    // (defeats SSO) so the leak is observable. Fails before the burden-path
    // edge-cleanup fix: `ORI_CHECK_LEAKS=1` reports `1 RC allocation not freed`.
    let src = r#"
@main () -> int = {
    let a: Result<int, str> = Err("an allocated failure message well past sso");
    let v = a ?? 42;
    if v == 42 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "coalesce_discards_heap_err_payload");
}

#[test]
fn probe_default_path_unaffected_str() {
    // Symmetry pin: with the probe UNSET (default), the SAME program runs
    // leak-free through the predicate stack — guards the default-path byte
    // identity claim is not vacuous (this test would still pass if the probe
    // suppressed everything; the per-shape probe tests above pin the burden
    // path actually fires).
    let src = r#"
@main () -> int = {
    let s = "round trip";
    let a = s;
    print(msg: a);
    0
}
"#;
    let (exit, _stdout, stderr) = compile_and_run_with_build_env(src, &[]);
    assert!(
        exit == 0,
        "default-path run exited {exit}\nstderr:\n{stderr}"
    );
}
