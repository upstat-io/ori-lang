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

// Step-B' dead-collection-source freeing (iterator-conversion shapes)
//
// A `m.keys()` / `m.values()` / `s.split()` / `set.to_list()` BORROWS its source
// collection, returns a fresh owned `[T]`, iterates it (consuming the result via
// the iterator), then the SOURCE dies at the post-loop block. Under the probe,
// the burden path must free that dead source — AND, because the freeing dec
// lowers to a whole-var `RcDec { HeapPointer }` routed through
// `ori_buffer_rc_dec`, it must compose with the V5-header `elem_dec_fn` to ALSO
// free the source's owned element strings (the map's heap key/value strings).
// Without the dead-collection-source pass these leak; without the elem_dec_fn
// composition the buffer frees but its element strings leak.

#[test]
fn probe_map_keys_str_source_freed_with_elements() {
    // `m.keys()`: the map source is borrowed, the keys list iterated. The dead
    // map at the loop exit must be freed VIA `ori_buffer_rc_dec` so its two heap
    // key strings (the V5 `elem_dec_fn` walk) are freed too — 3 frees needed.
    let src = r#"
@main () -> int = {
    let m = {
        "this is a very long key exceeding SSO threshold here": 10,
        "another very long key that also exceeds the SSO thresh": 20
    };
    let keys = m.keys();
    let total = 0;
    for k in keys do {
        total = total + k.len();
    };
    if total == 106 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "map_keys_str_source_freed_with_elements");
}

#[test]
fn probe_set_to_list_str_source_freed_no_double_free() {
    // `set.to_list()`: the set source is borrowed, the list iterated. The dead
    // set at the loop exit must be freed exactly once (a second dec aborts) — its
    // element strings are slice/heap-aware via `elem_dec_fn`.
    let src = r#"
@main () -> int = {
    let s: Set<str> = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ].iter().collect();
    let list = s.to_list();
    let total = 0;
    for item in list do {
        total = total + item.len();
    };
    if total == 109 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "set_to_list_str_source_freed_no_double_free");
}

#[test]
fn probe_str_split_source_freed() {
    // `s.split()`: the str source is borrowed, the parts (slice-views into `s`)
    // iterated. The dead source string at the loop exit must be freed (slice
    // provenance handled by `ori_rc_dec` on the FatPointer data).
    let src = r#"
@main () -> int = {
    let s = "this is a very long string that exceeds SSO threshold,another very long string also exceeds";
    let parts = s.split(sep: ",");
    let total = 0;
    for p in parts do {
        total = total + p.len();
    };
    if total == 90 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "str_split_source_freed");
}

#[test]
fn probe_map_keys_str_loop_managed_not_double_freed() {
    // Negative pin (the for-loop-cluster guard): the keys RESULT is iterator-
    // managed (freed by `ori_iter_drop`); the dead-collection-source pass must
    // free ONLY the borrowed map source, never the iter-consumed keys list — a
    // dec there would double-free. Covered by the positive pin's leak-free exit,
    // but pinned separately for the double-free shape (a second dec aborts).
    let src = r#"
@main () -> int = {
    let m = { "alpha key exceeding the sso inline threshold here": 1 };
    let ks = m.keys();
    let n = 0;
    for k in ks do { n = n + k.len(); };
    if n > 0 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "map_keys_str_loop_managed_not_double_freed");
}

// Dead mutation-result / scope-exit owned-collection freeing.
//
// A `let ys = xs.<mut>(...)` (sort / set / insert / remove / push / concat) or a
// read-only `let m = {..}; m.<read>(..)` binds an OWNED collection that is
// last-used at a BORROWED position (`@length` / `@first` / `@contains_key`) then
// dies at function scope exit. The burden walk emits inc/dec pairs that net the
// EXPLICIT ops to 0 but never releases the allocation's implicit `+1` — a LEAK
// (RL-2 `ScopeExit` / `ApplyToBorrowedParam` mandates a release dec; the impl
// omits it). The dead mutation-result / scope-exit owned-collection pass emits
// ONE whole-var `BurdenDec` netting the lineage to `-1`; the `RcDec { HeapPointer
// }` it lowers to routes through `ori_buffer_rc_dec` so a heap-str-element
// collection ALSO frees its element strings via the V5 `elem_dec_fn` walk.

#[test]
fn probe_list_sort_result_freed_at_scope_exit() {
    // `xs.sort()` reuses the buffer in place at rc=1 → the sorted RESULT is the
    // same allocation, last-used via borrowed `@length`/`@first`/`@last`, dead at
    // scope exit. Without the freeing dec the buffer leaks.
    let src = r#"
@main () -> int = {
    let xs = [3, 1, 4, 1, 5, 9, 2, 6];
    let sorted = xs.sort();
    if sorted.length() == 8 && sorted.first().unwrap() == 1 && sorted.last().unwrap() == 9 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_sort_result_freed_at_scope_exit");
}

#[test]
fn probe_list_set_result_freed_at_scope_exit() {
    // `xs.set(i, v)` mutation-result owned-collection dead at scope exit.
    let src = r#"
@main () -> int = {
    let xs = [10, 20, 30];
    let ys = xs.set(1, 99);
    if ys.length() == 3 && ys.first().unwrap() == 10 && ys.last().unwrap() == 30 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_set_result_freed_at_scope_exit");
}

#[test]
fn probe_list_insert_result_freed_at_scope_exit() {
    // `xs.insert(i, v)` mutation-result owned-collection dead at scope exit.
    let src = r#"
@main () -> int = {
    let xs = [1, 2];
    let ys = xs.insert(2, 3);
    if ys.length() == 3 && ys.last().unwrap() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_insert_result_freed_at_scope_exit");
}

#[test]
fn probe_list_remove_result_freed_at_scope_exit() {
    // `xs.remove(i)` mutation-result owned-collection dead at scope exit.
    let src = r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.remove(2);
    if ys.length() == 2 && ys.last().unwrap() == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_remove_result_freed_at_scope_exit");
}

#[test]
fn probe_map_read_only_owned_source_freed_at_scope_exit() {
    // Read-only `m.contains_key(..)`: the owned map is NEVER mutated, borrowed at
    // every use, dead at scope exit — the simplest whole-buffer leak shape (alloc
    // `+1` unreleased). Int keys keep this a pure buffer-freeing case (the
    // heap-str-element-arg layer is the separate residual leaf).
    let src = r#"
@main () -> int = {
    let m = {1: 10, 2: 20, 3: 30};
    if m.contains_key(2) then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "map_read_only_owned_source_freed_at_scope_exit");
}

#[test]
fn probe_map_int_index_result_freed_at_scope_exit() {
    // `m[k]` on an int-keyed int-value map: the owned map is borrowed by the index
    // read, dead at scope exit — the whole-buffer leak shape.
    let src = r#"
@main () -> int = {
    let m = {1: 100, 2: 200};
    if m[1].unwrap_or(0) == 100 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "map_int_index_result_freed_at_scope_exit");
}

#[test]
fn probe_list_int_sort_negative_no_extra_release() {
    // Negative pin: a sort result that IS subsequently returned (ownership
    // transfer, RL-2 transfer kind) must NOT receive a scope-exit release — the
    // caller inherits the obligation. A double-release here aborts.
    let src = r#"
@build () -> [int] = {
    let xs = [3, 1, 2];
    xs.sort()
}

@main () -> int = {
    let ys = build();
    if ys.length() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_int_sort_negative_no_extra_release");
}

// CHAINED COW-mutation results: `xs.push(a).push(b)` / `xs.concat(..).reverse()`
// build a single fresh-local allocation transformed in place by each COW op. The
// receiver of the SECOND mutation is itself a mutation RESULT (not a direct
// `Construct`), so the dead-owned-collection candidate set previously excluded the
// final result and leaked the buffer. The fresh-local-equivalence transitive
// closure over a COW-mutator chain rooted at a fresh local Construct makes the
// chain tail freeable at its borrowed-read scope-exit sink (RL-2 ApplyToBorrowedParam).

#[test]
fn probe_list_push_chain_result_freed_at_scope_exit() {
    // `[1].push(2).push(3)`: the second push's receiver is the first push RESULT,
    // not a Construct. The chain tail is borrowed-read by `@length`/`@first`/`@last`,
    // dead at scope exit. Without the transitive closure the realloc'd buffer leaks.
    let src = r#"
@main () -> int = {
    let xs = [1].push(2).push(3);
    if xs.length() == 3 && xs.first().unwrap() == 1 && xs.last().unwrap() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_push_chain_result_freed_at_scope_exit");
}

#[test]
fn probe_list_concat_reverse_chain_result_freed_at_scope_exit() {
    // `([1,2] + [3]).reverse()`: reverse's receiver is the concat result. The
    // chain tail is borrowed-read at scope exit.
    let src = r#"
@main () -> int = {
    let xs = ([1, 2] + [3]).reverse();
    if xs.length() == 3 && xs.first().unwrap() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_concat_reverse_chain_result_freed_at_scope_exit");
}

#[test]
fn probe_list_reverse_reverse_chain_result_freed_at_scope_exit() {
    // `xs.reverse().reverse()`: a two-COW chain whose tail is borrowed-read, dead
    // at scope exit.
    let src = r#"
@main () -> int = {
    let xs = [1, 2, 3].reverse().reverse();
    if xs.length() == 3 && xs.first().unwrap() == 1 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(
        src,
        "list_reverse_reverse_chain_result_freed_at_scope_exit",
    );
}

#[test]
fn probe_list_push_chain_negative_returned_no_extra_release() {
    // Negative pin: a push-chain result that IS returned (ownership transfer, RL-2
    // transfer kind) must NOT receive a scope-exit release — the caller inherits
    // the obligation. A double-release here aborts.
    let src = r#"
@build () -> [int] = {
    [1].push(2).push(3)
}

@main () -> int = {
    let ys = build();
    if ys.length() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_push_chain_negative_returned_no_extra_release");
}

// SET-ALGEBRA results (`a.union(b)` / `a.difference(b)` / `a.intersection(b)`)
// return a FRESH owned `{T}` Set the runtime allocates from the two operands'
// contents — distinct from both operands (neither aliases the result). A
// `let s = a.union(b); s.len()` borrowed-reads the result then drops it dead at
// scope exit; the burden walk emits ZERO freeing ops on it, leaking the result
// buffer. The fresh-owned-collection recognizer must classify set-algebra
// results so the alloc-aware net fires ONE scope-exit dec (RL-2
// `RL2_release_exactly_once`).

#[test]
fn probe_set_union_result_freed_at_scope_exit() {
    // `a.union(b)` returns a fresh owned `{int}`, borrowed-read by `@len`, dead
    // at scope exit. Without the set-algebra recognizer the result buffer leaks.
    let src = r#"
@main () -> int = {
    let a: Set<int> = [1, 2].iter().collect();
    let b: Set<int> = [2, 3].iter().collect();
    if a.union(b).len() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "set_union_result_freed_at_scope_exit");
}

#[test]
fn probe_set_difference_result_freed_at_scope_exit() {
    // `a.difference(b)` fresh owned `{int}` result, borrowed-read then dead.
    let src = r#"
@main () -> int = {
    let a: Set<int> = [1, 2, 3].iter().collect();
    let b: Set<int> = [2].iter().collect();
    if a.difference(b).len() == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "set_difference_result_freed_at_scope_exit");
}

#[test]
fn probe_set_intersection_result_freed_at_scope_exit() {
    // `a.intersection(b)` fresh owned `{int}` result, borrowed-read then dead.
    let src = r#"
@main () -> int = {
    let a: Set<int> = [1, 2, 3].iter().collect();
    let b: Set<int> = [2, 3, 4].iter().collect();
    if a.intersection(b).len() == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "set_intersection_result_freed_at_scope_exit");
}

#[test]
fn probe_set_union_result_returned_negative_no_extra_release() {
    // Negative pin: a set-algebra result that IS returned (ownership transfer,
    // RL-2 transfer kind) must NOT receive a scope-exit release — the caller
    // inherits the obligation. The `returned` exclusion must hold; a
    // double-release here aborts.
    let src = r#"
@combine (a: Set<int>, b: Set<int>) -> Set<int> = {
    a.union(b)
}

@main () -> int = {
    let x: Set<int> = [1, 2].iter().collect();
    let y: Set<int> = [2, 3].iter().collect();
    let z = combine(a: x, b: y);
    if z.len() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "set_union_result_returned_negative_no_extra_release");
}

// PER-ELEMENT heap-str ownership: a fresh heap `str` passed at a BORROWED arg
// position to a COW-mutator / lookup builtin (`insert` key/value, `remove` key,
// `contains_key` key) is COPIED into the collection (`key_inc`/`val_inc`) OR merely
// borrowed for the lookup — in BOTH cases the LOCAL str reference survives the
// borrowed call and is dead afterward (RL-2 ApplyToBorrowedParam mandates a dec).
// The burden walk emits a self-cancelling inc/dec pair (coalesces to nothing), so
// the local str leaks. A str MOVED into a `Construct` (collection-literal element,
// an OWNED position) is the collection's only reference — its `elem_dec_fn` frees
// it, and a local dec there double-frees: the negative pin clamps that boundary.

#[test]
fn probe_map_insert_heap_str_key_local_freed() {
    // The inserted KEY str is copied into the map (key_inc); the local survives the
    // borrowed `@insert` and is dead at scope exit. Without the local dec it leaks.
    let src = r#"
@main () -> int = {
    let m: {str: int} = {};
    let m = m.insert("this is a long key that definitely exceeds the SSO threshold", 42);
    if m.len() == 1 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "map_insert_heap_str_key_local_freed");
}

#[test]
fn probe_map_insert_heap_str_value_local_freed() {
    // The inserted VALUE str is copied into the map (val_inc); the local leaks.
    let src = r#"
@main () -> int = {
    let m: {int: str} = {};
    let m = m.insert(1, "a very long value string that exceeds SSO threshold for sure");
    if m.len() == 1 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "map_insert_heap_str_value_local_freed");
}

#[test]
fn probe_map_remove_str_key_lookup_local_freed() {
    // The `remove` lookup KEY str is borrowed for the search (never stored); the
    // local is the only reference, dead after the borrowed call — it leaks without
    // a last-use dec.
    let src = r#"
@main () -> int = {
    let m = {
        "this is a very long heap string that exceeds the SSO threshold for sure": 1,
        "another long heap allocated string for map remove testing purposes here": 2
    };
    let m2 = m.remove(key: "another long heap allocated string for map remove testing purposes here");
    if m2.len() == 1 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "map_remove_str_key_lookup_local_freed");
}

#[test]
fn probe_map_construct_heap_str_keys_negative_no_double_free() {
    // Negative pin: str keys MOVED into a `Construct Map` literal (an OWNED position)
    // are the map's only reference — the map's `elem_dec_fn` frees them. A per-element
    // local dec here double-frees. The map is read-only (`contains_key`), dead at
    // scope exit; the buffer-freeing pass frees the buffer + elements via the V5
    // walk, and the per-element pass MUST NOT also free the moved keys.
    let src = r#"
@main () -> int = {
    let m = {
        "this is a very long heap string key that exceeds the SSO threshold here": 1,
        "another long heap allocated string key for construct testing purposes": 2
    };
    if m.contains_key("this is a very long heap string key that exceeds the SSO threshold here") then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "map_construct_heap_str_keys_negative_no_double_free");
}

#[test]
fn probe_set_to_list_conversion_result_freed() {
    // A collection-CONVERSION result (`set.to_list()` / `m.keys()` / `m.values()`)
    // is a FRESH owned collection the runtime allocates from the receiver; bound to
    // a local, borrowed-read by `@length`, dead at scope exit — it leaks the result
    // buffer under sole-emitter lowering without a scope-exit release dec.
    let src = r#"
@main () -> int = {
    let s: Set<int> = [1, 2, 3].iter().collect();
    let xs = s.to_list();
    if xs.length() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "set_to_list_conversion_result_freed");
}

// Conversion source borrowed by a terminator-position conversion builtin, NO loop.
//
// `let vals = m.values()` borrows the map `m` at the `@values` terminator-Invoke
// arg, then `m` is dead. Under the probe the burden walk emits `m`'s scope-exit
// BurdenDec INLINE in the call's block BEFORE the borrowed-Invoke terminator
// (`Invoke @values(m [borrow])`) instead of on the normal/unwind successor edge.
// The map (and its heap value strings) free before `@values` reads them; the
// runtime's val_inc then reads a freed refcount → UAF surfacing as a leak. The
// fix relocates the source's dec to the successor edge (RL-4). Heap str VALUES
// exercise the val_inc-on-freed path.

#[test]
fn probe_map_values_heap_str_source_borrowed_no_loop() {
    let src = r#"
@main () -> int = {
    let m: {int: str} = {};
    let m = m.insert(1, "this is a very long heap string value that exceeds the SSO threshold for sure");
    let m = m.insert(2, "another long heap allocated string value for map values testing purposes");
    let m = m.insert(3, "third heap string value to check all elements are inc'd by val_inc_fn");
    let vals = m.values();
    if vals.len() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "map_values_heap_str_source_borrowed_no_loop");
}

#[test]
fn probe_set_str_union_owned_consumed() {
    // `a.union(b)`: a set union consumes its operand sets; the heap-str elements'
    // ownership is handled by the union/result lineage. A caller-side freeing dec
    // on a union operand double-frees against the union's own consume. Pins that
    // the single-borrow conversion-source relocation does not over-fire on a
    // union-operand shape (the receiver is owned-consumed, not a borrowed
    // conversion source, so the relocation leaves it untouched).
    let src = r#"
@main () -> int = {
    let a: Set<str> = ["alpha string exceeding sso threshold here now", "beta string also exceeding the sso threshold"].iter().collect();
    let b: Set<str> = ["gamma string exceeding sso threshold here now", "alpha string exceeding sso threshold here now"].iter().collect();
    let u = a.union(other: b);
    if u.len() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "set_str_union_owned_consumed");
}

// ITER-CONSUME inward-transfer (RL-2 `ApplyToIterConsumingParam`): a collection
// passed to a callee that iter-consumes it (`for x in coll` → `@iter [own]` →
// `ori_iter_drop` frees the collection INSIDE the callee on EVERY exit, normal
// AND unwind) transfers ownership inward — the caller emits NO scope-exit dec
// (the callee's iterator machinery releases). The borrow-read case (`xs.fold(..)`
// borrows, does NOT free) presents an IDENTICAL contract on every other
// dimension; `ParamContract.iter_consumes` is the sole discriminator
// (`AimsProof.Realization::RL2_iter_consuming_caller_dec_splits`).

#[test]
fn probe_map_str_passed_to_iter_consuming_fn_no_double_free() {
    // `sum_values(m)` iter-consumes the map via `for entry in m`. The caller's
    // scope-exit dec on `m` double-frees against the callee's `ori_iter_drop`.
    // The iter-consume verdict suppresses the caller dec.
    let src = r#"
@sum_values (m: {str: int}) -> int = {
    let total = 0;
    for entry in m do {
        let (k, v) = entry;
        total = total + v
    };
    total
}

@main () -> int = {
    let m = {
        "a very long key string that exceeds the twenty three byte SSO threshold for testing": 10,
        "another long key string to exercise function parameter map cleanup": 20
    };
    let result = sum_values(m: m);
    if result == 30 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "map_str_passed_to_iter_consuming_fn");
}

#[test]
fn probe_set_str_passed_to_iter_consuming_fn_no_double_free() {
    // `count_items(s)` iter-consumes the set via `for x in s`. Same inward-transfer
    // as the map case; the caller dec is suppressed.
    let src = r#"
@count_items (s: Set<str>) -> int = {
    let total = 0;
    for x in s do {
        total = total + 1
    };
    total
}

@main () -> int = {
    let s: Set<str> = [
        "a very long set element string exceeding the sso threshold for sure",
        "another long set element string for function parameter set cleanup"
    ].iter().collect();
    let result = count_items(s: s);
    if result == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "set_str_passed_to_iter_consuming_fn");
}

#[test]
fn probe_iter_consume_call_inside_catch_then_normal_call_no_leak() {
    // CATCH-UNWIND pin: `process_all(words)` iter-consumes `words` and is called
    // FIRST inside a `catch` (whose body may panic mid-iteration → the Invoke's
    // unwind successor IS the `@ori_catch_recover` catch landing pad), then a
    // SECOND `process_all([...])` runs on the normal path after the catch.
    //
    // The iter-consume verdict's normal-edge dec removal is sound ONLY when the
    // Invoke's unwind does not reach a catch landing pad: a catch-intercepted
    // unwind fragments the normal-path edge placement so the 2nd call's buffer
    // BurdenInc nets +1 (leak of the 2nd-call collection + its heap elements).
    // The catch-aware gate keeps the dec inline for catch-intercepted iter-consume
    // calls (the burden walk's net-0 inc/dec pair manages the value, exactly as the
    // base path without the verdict). The callee's own `ori_iter_drop` frees the
    // collection on BOTH the normal and the panic-unwind exit — no caller dec is
    // double-freed; the 2nd call's normal completion frees cleanly.
    let src = r#"
@process_word (w: str) -> int = {
    if w.starts_with(prefix: "panic") then {
        panic(msg: "word triggered panic")
    };
    w.len()
}

@process_all (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + process_word(w: w)
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "panic trigger long string for heap allocation needed",
        "unreachable very long string that is also on the heap"
    ];
    let r1 = catch(expr: process_all(words: words));
    let r2 = process_all(words: [
        "first safe long string exceeding SSO threshold here",
        "second safe long string also exceeding the threshold"
    ]);
    let caught = match r1 { Ok(_) -> false, Err(_) -> true };
    if caught && r2 == 103 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "iter_consume_call_inside_catch_then_normal_call");
}

#[test]
fn probe_borrow_read_fold_call_keeps_caller_dec_no_leak() {
    // NEGATIVE pin (iter-consume relocation over-fire boundary): `@sum_list`
    // BORROWS its list via `xs.fold(..)` — it does NOT iter-consume-and-free
    // (no `@iter [own]` → `ori_iter_drop` on the param). Its
    // `ParamContract.iter_consumes` is FALSE,
    // so the caller MUST KEEP its scope-exit dec — suppressing it leaks the list.
    // Pins that the iter-consume verdict does NOT over-fire onto a borrow-read
    // callee with an identical (`access=Borrowed`, scalar-return) contract.
    let src = r#"
@sum_list (xs: [int]) -> int = {
    xs.fold(initial: 0, op: (acc, x) -> acc + x)
}

@main () -> int = {
    let xs = [10, 20, 30];
    let result = sum_list(xs: xs);
    if result == 60 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "borrow_read_fold_call_keeps_caller_dec");
}

// INLINE for-loop MULTI-BORROW iter-consume source accounting. The `function_param`
// multi-borrow cures the case where N >= 2 USER callees iter-consume a source
// (`ParamContract.iter_consumes`). These pins cover the SAME mechanism over an
// INLINE for-loop iter-consume: `for s in items do ...` lowers to
// `Apply @iter(items [own])` -> `ori_iter_drop`, a PROTOCOL-BUILTIN iter-consume
// position (no user `MemoryContract`) that the contract-keyed recognizer misses.
// A source `items` (RcPtr collection) iter-consumed by N >= 2 inline for-loops
// SURVIVES the earlier loops, so the oracle emits (N-1) keep-alive incs and zero
// normal-path source decs (each loop's `@iter [own]` -> `ori_iter_drop` frees the
// buffer; the Nth drop is the single release). Under sole-emitter lowering the
// burden walk emits a net-0 inc/dec pair on the source -> zero real RC -> the
// FIRST `ori_iter_drop` frees the source at rc=1 -> the SECOND `@iter` double-frees.
// RL-1 keep-alive (`RL1_emit_iff_not_elidable`) + RL-2 single release
// (`RL2_iter_consuming_no_caller_dec` + `RL2_release_exactly_once`).

#[test]
fn probe_inline_for_loop_str_list_two_call_no_double_free() {
    // `[str]` source iter-consumed by TWO inline `for s in items do` loops.
    let src = r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "second string also exceeds the SSO threshold of twenty three bytes"
    ];
    let count1 = 0;
    for s in items do count1 = count1 + 1;
    let count2 = 0;
    for s in items do count2 = count2 + 1;
    if count1 == 2 && count2 == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "inline_for_loop_str_list_two_call");
}

#[test]
fn probe_inline_for_loop_map_two_call_no_double_free() {
    // `{str: int}` source iter-consumed by TWO inline `for entry in m do` loops —
    // the keep-alive composes with the map buffer's `elem_dec_fn` key-string walk.
    let src = r#"
@main () -> int = {
    let m = {
        "long key string that exceeds the SSO threshold of twenty three": 10,
        "another long key string that also exceeds SSO threshold clearly": 20
    };
    let count1 = 0;
    for entry in m do count1 = count1 + 1;
    let count2 = 0;
    for entry in m do count2 = count2 + 1;
    if count1 == 2 && count2 == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "inline_for_loop_map_two_call");
}

#[test]
fn probe_inline_for_loop_single_loop_negative_no_extra_release() {
    // NEGATIVE pin (the inline-`@iter`-recognizer lower boundary): a SINGLE inline
    // `for s in items do` loop iter-consumes the source ONCE (N=1, dead-after-call).
    // The multi-borrow keep-alive pass MUST NOT fire (N < 2) — a spurious keep-alive
    // inc with no matching drop would LEAK the source buffer + its heap strings.
    // Pins that the inline-`@iter [own]` recognizer counts the position but the
    // multi-borrow gate (uses.len() >= 2) keeps the single-loop source on the base
    // path (its net-0 burden pair + the for-loop's own `ori_iter_drop` are correct).
    let src = r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "second string also exceeds the SSO threshold of twenty three bytes"
    ];
    let count = 0;
    for s in items do count = count + 1;
    if count == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "inline_for_loop_single_loop_negative");
}

#[test]
fn probe_bare_unused_iterator_handle_freed_at_scope_exit() {
    // In-function iterator-handle (RL-2): `[..].iter()` produces a FRESH owned
    // `DoubleEndedIterator` handle (the buffer moved INTO the iterator state). It
    // is never consumed by a for-loop / `iter_next` / `ori_iter_drop`, so it must
    // be freed by a scope-exit `RcDec [Iterator]` (= `ori_iter_drop`). The default
    // path emits it; the burden path must emit a standalone `BurdenDec` on the
    // handle lineage that lowers (via `RcStrategy::from_var` Iterator) to the same.
    let src = r#"
@main () -> int = {
    let _it = [1, 2, 3, 4].iter();
    0
}
"#;
    assert_burden_path_self_sufficient(src, "bare_unused_iterator_handle");
}

#[test]
fn probe_iterator_handle_in_tuple_freed_at_scope_exit() {
    // Iterator handle MOVED into a tuple field: the handle transfers ownership
    // into the `Construct Tuple`, so the freeing burden is on the AGGREGATE — the
    // tuple's scope-exit `RcDec [AggFields]` walks to the iterator field and
    // `ori_iter_drop`s it (freeing the iterator-owned buffer). The burden path
    // must emit a `BurdenDec` on the fresh iterator-bearing tuple lineage.
    let src = r#"
@main () -> int = {
    let _t: (int, Iterator<int>) = (42, [7, 8, 9].iter());
    0
}
"#;
    assert_burden_path_self_sufficient(src, "iterator_handle_in_tuple");
}

#[test]
fn probe_iterator_handle_in_struct_freed_at_scope_exit() {
    // Iterator handle MOVED into a struct field — same AGGREGATE-drop mechanism
    // as the tuple shape, exercising `Tag::Struct` `RcStrategy::AggregateFields`.
    let src = r#"
type Holder = { it: Iterator<int> };

@main () -> int = {
    let _h = Holder { it: [1, 2, 3].iter() };
    0
}
"#;
    assert_burden_path_self_sufficient(src, "iterator_handle_in_struct");
}

#[test]
fn probe_for_loop_iterator_handle_negative_no_double_free() {
    // NEGATIVE pin (the SEED-not-reuse boundary): a `for x in coll` lowering
    // already emits an explicit `@ori_iter_drop` Apply on every loop-exit path.
    // The new in-function-iterator-handle pass MUST NOT also emit a freeing dec on
    // that for-loop-managed handle — doing so double-frees the iterator-owned
    // buffer. The handle's lineage is in `compute_iter_drop_handle_lineages`
    // (it is an `ori_iter_drop` arg) and must be excluded.
    let src = r#"
@main () -> int = {
    let nums = [1, 2, 3, 4, 5];
    let total = 0;
    for n in nums do {
        total = total + n
    };
    if total == 15 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_loop_iterator_handle_negative");
}

// for_yield RESULT freeing — the `for x in coll yield expr` comprehension lowers
// to `ori_list_new` (scratch) → loop `ori_list_push` → `ori_list_take` (FRESH
// owned result at rc=1, moving the data buffer out of the scratch). The result
// is a FRESH self-allocation produced by a no-contract builtin `Apply`, so its
// fresh-site `BurdenInc` must be alloc-aware-net-elided when the lineage nets +1
// (the redundant fresh inc over the alloc baseline). When the result is indexed
// ≥2x the dup-alias incs net 0 among themselves, leaving the surplus fresh inc
// as a leak under sole-emitter lowering (RL-1 + the compiled-Lean `rcBalance`).

#[test]
fn probe_for_yield_int_result_dup_indexed_freed() {
    // `for w in words yield w.length()` builds an `[int]` result indexed twice.
    // The `ori_list_take` result fresh-site inc is the surplus over the
    // alloc-aware net (alloc(+1) + 1 fresh inc + N dup incs − N dup decs = +1) →
    // the result buffer leaks. The net-keyed elision removes exactly the surplus
    // fresh inc, restoring rc balance to 0.
    let src = r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let lengths = for w in words yield w.length();
    if lengths[0] == 53 && lengths[1] == 56 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_yield_int_result_dup_indexed");
}

#[test]
fn probe_for_yield_int_result_triple_indexed_freed() {
    // `for i in nums yield i * i` builds an `[int]` result indexed THREE times.
    // The `ori_list_take` fresh-result over-count nets +1 regardless of index
    // multiplicity (the dup-index incs net 0 among themselves); the net-keyed
    // elision removes the single surplus fresh inc.
    let src = r#"
@main () -> int = {
    let nums = [2, 3, 4];
    let squares = for i in nums yield i * i;
    if squares[0] == 4 && squares[1] == 9 && squares[2] == 16 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_yield_int_result_triple_indexed");
}

#[test]
fn probe_for_yield_int_result_single_use_negative_no_double_free() {
    // NEGATIVE pin (the alloc-aware-net boundary): a SINGLE-use for_yield result
    // (`.length()` only, no dup-index) has a net != 1 once the move-alias dec is
    // counted — the fresh inc is load-bearing there. The net-keyed elision MUST
    // NOT elide it (eliding a net-0 lineage's inc would net −1 = a double-free).
    let src = r#"
@main () -> int = {
    let nums = for i in [10, 20, 30] yield i * 2;
    if nums.length() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_yield_int_result_single_use_negative");
}

// JUMP-THREADED `ori_list_take` result (the `for_yield_*_two_call` shape). TWO
// for_yields over the same source build two `[int]` results; the FIRST result's
// `ori_list_take` value flows through a Jump-arg → block-param POSITIONAL rename
// (the 2nd loop's scratch-init block carries it forward) before its lone TRUE
// release fires on the threaded block-param's `Let` alias. The fresh-site
// `BurdenInc` + paired premature `BurdenDec` net 0 at the alloc site; the threaded
// downstream dec is the genuine single release. `compute_same_alloc_reps` EXCLUDES
// the Jump-phi BY DESIGN, so the unthreaded net for the result is +1 → the
// fresh-inc-elision over-fires, leaving alloc(+1) − premature-dec − true-dec =
// −1 = a double-free of the first result. The phi-aware lineage net threads the
// edge so the chain nets 0 and the fresh inc is kept.
// Spec: Annex E §AIMS RL-1 + RL-2 (`RL2_release_exactly_once`).

#[test]
fn probe_for_yield_int_two_call_jump_threaded_result_no_double_free() {
    // Two for_yields over the SAME source, each `yield s.len()` → two `[int]`
    // results, both `.length()`-checked. The first result is jump-threaded across
    // the second loop's init block; its premature fresh-site dec must NOT survive
    // the fresh-inc elision (which is correctly suppressed by the phi-aware net).
    let src = r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "second string also exceeds the SSO threshold of twenty three bytes"
    ];
    let r1 = for s in items yield s.len();
    let r2 = for s in items yield s.len();
    if r1.length() == 2 && r2.length() == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_yield_int_two_call_jump_threaded");
}

#[test]
fn probe_for_yield_int_three_call_jump_threaded_result_no_double_free() {
    // THREE for_yields over the same source → the first TWO results are each
    // jump-threaded forward through the later loops' init blocks. Both threaded
    // results must keep their fresh incs (phi-aware net == 0 each), so neither
    // double-frees.
    let src = r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "second string also exceeds the SSO threshold of twenty three bytes",
        "third string also well beyond the SSO inline storage threshold here"
    ];
    let r1 = for s in items yield s.len();
    let r2 = for s in items yield s.len();
    let r3 = for s in items yield s.len();
    if r1.length() == 3 && r2.length() == 3 && r3.length() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_yield_int_three_call_jump_threaded");
}

#[test]
fn probe_for_yield_int_single_call_not_threaded_negative_no_double_free() {
    // NEGATIVE pin (the phi-threading lower boundary): a SINGLE for_yield result
    // dup-indexed (NOT jump-threaded — straight-line flow, no Jump-arg → param
    // rename of the result). The phi-aware net MUST collapse to the unthreaded net
    // here (no phi edge to thread), keeping the alloc-aware-net fresh-inc elision
    // intact (net +1 → elide the surplus fresh inc, no leak). The phi-aware
    // extension must not perturb the non-threaded single-result case.
    let src = r#"
@main () -> int = {
    let items = [
        "this string exceeds SSO threshold by being very long indeed",
        "second string also exceeds the SSO threshold of twenty three bytes"
    ];
    let r = for s in items yield s.len();
    if r[0] == 59 && r[1] == 66 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_yield_int_single_call_not_threaded");
}

// YIELD-ELEMENT into an INDEX-consumed for_yield result (the joint 3-mechanism
// shape). `for w in words yield w` moves each borrowed `Project @__iter_next.1`
// element view into `ori_list_push(result, w [own])`. When the result is then
// INDEXED (`result[i]` → `@__index`) rather than iter-consumed, the result owns
// its own copies of the heap elements: the source's `IterState::Drop` frees the
// source copies, the result needs (a) a yield-element `RcInc` (the push
// duplicates the element into the result buffer — RL-1) and (b) a per-`__index`
// view release (RL-2). The burden path EXCLUDES the iter-element view via the
// iter-element-view exclusion → both are missing → the result+source double-free
// the elements / leak. The move-vs-borrow discriminator is the result's
// consumption kind: an INDEX-consumed result transfers element ownership inward
// (needs the inc + dec), an ITER-consumed result (a second for-loop /
// `ori_iter_drop`) frees the elements itself (no inc — the
// `yield_identity_str_list` canary).
// Spec: Annex E §AIMS RL-1 (`RL1_emits_inc = !incElidable`) + RL-2
// (`RL2_release_exactly_once`).

#[test]
fn probe_for_yield_str_identity_indexed_no_double_free() {
    // `for w in words yield w` then `result[0]`/`result[1]` (INDEX-consumed). The
    // yielded str elements are copied into the result buffer; each indexed view
    // needs its own release, and the yielded element needs the duplicating inc.
    let src = r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let copied = for w in words yield w;
    if copied[0].length() + copied[1].length() == 109 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_yield_str_identity_indexed");
}

#[test]
fn probe_for_yield_break_str_no_double_free() {
    // `for w in words yield { if ..break; w }` — early-exit yield of the heap str
    // element. The result is length-checked (index-equivalent consumption: the
    // result owns its yielded copies; the source retains the un-yielded ones).
    let src = r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold",
        "third string for good measure and beyond SSO"
    ];
    let result = for w in words yield {
        if w.length() > 55 then break;
        w
    };
    if result.length() == 1 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_yield_break_str");
}

#[test]
fn probe_for_yield_str_identity_iter_consumed_negative_no_double_free() {
    // NEGATIVE pin (the move-vs-borrow discriminator boundary): `for w in words
    // yield w` then a SECOND for-loop consumes the result (`for w in copy do ...`).
    // The result is ITER-consumed (`@iter [own]` → `ori_iter_drop` frees its
    // elements), so the yield-element inc + per-view dec MUST NOT fire (adding the
    // inc would double-free against the iter-drop). Mirrors
    // `yield_identity_str_list`.
    let src = r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let copy = for w in words yield w;
    let total = 0;
    for w in copy do {
        total = total + w.len();
    };
    if total == 109 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_yield_str_identity_iter_consumed");
}

// MULTI-BORROW iter-consume source (RL-1 keep-alive inc + RL-2 single release).
// A source `coll` borrowed by N iter-consuming `[own]` calls (each callee frees
// it via `@iter [own]` → `ori_iter_drop`): the oracle emits (N-1) keep-alive
// `RcInc` on the source (the first N-1 uses are DUPLICATING per
// `AimsProof.Realization::RL1_emit_iff_not_elidable`) and ZERO source `RcDec`
// (each callee's iter-drop is the release per `RL2_iter_consuming_no_caller_dec`;
// the Nth call's iter-drop is the genuine final free per `RL2_release_exactly_once`).
// Under the flag the burden walk emits a spurious source dec at the multi-use
// move-alias point → double-free against the callee iter-drops. The multi-borrow
// suppression cures it.

#[test]
fn probe_str_list_two_iter_consuming_calls_no_double_free() {
    // `sum_lens(words)` iter-consumes `words`; called TWICE on the same source.
    let src = r#"
@sum_lens (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = sum_lens(words: words);
    let b = sum_lens(words: words);
    if a == 109 && b == 109 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "str_list_two_iter_consuming_calls");
}

#[test]
fn probe_int_list_two_iter_consuming_calls_no_double_free() {
    // Type-dimension cell: `[int]` source (scalar elements, but the buffer is
    // still RcPtr) borrowed by two iter-consuming calls.
    let src = r#"
@sum_list (xs: [int]) -> int = {
    let total = 0;
    for x in xs do {
        total = total + x;
    };
    total
}

@main () -> int = {
    let xs = [10, 20, 30, 40];
    let a = sum_list(xs: xs);
    let b = sum_list(xs: xs);
    if a == 100 && b == 100 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "int_list_two_iter_consuming_calls");
}

#[test]
fn probe_two_distinct_iter_consumed_sources_no_double_free() {
    // Pattern cell: two DISTINCT sources, one borrowed twice + one borrowed once,
    // interleaved — each source's keep-alive accounting is independent.
    let src = r#"
@sum_lens (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@main () -> int = {
    let a = [
        "this is a very long string that exceeds SSO threshold"
    ];
    let b = [
        "another very long string that also exceeds the threshold"
    ];
    let ra = sum_lens(words: a);
    let rb = sum_lens(words: b);
    let ra2 = sum_lens(words: a);
    if ra == 53 && rb == 56 && ra2 == 53 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "two_distinct_iter_consumed_sources");
}

#[test]
fn probe_chained_iter_consuming_callee_no_double_free() {
    // Pattern cell: the iter-consume is one call deep (a `wrapper` forwards the
    // borrowed source to the iter-consuming `iterate_words`); `wrapper` is called
    // twice on the same source.
    let src = r#"
@iterate_words (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@wrapper (words: [str]) -> int = {
    iterate_words(words: words)
}

@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold",
        "another very long string that also exceeds the threshold"
    ];
    let a = wrapper(words: words);
    let b = wrapper(words: words);
    if a == 109 && b == 109 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "chained_iter_consuming_callee");
}

#[test]
fn probe_single_iter_consuming_call_negative_still_freed() {
    // NEGATIVE pin (the multi-borrow lower boundary): a SINGLE iter-consuming call
    // where the source DIES after the call (the single-borrow `Suppress` shape).
    // The multi-borrow suppression must NOT change this — the callee's iter-drop
    // is still the sole release; emitting a keep-alive inc here would LEAK.
    let src = r#"
@sum_list (xs: [int]) -> int = {
    let total = 0;
    for x in xs do {
        total = total + x;
    };
    total
}

@main () -> int = {
    let xs = [10, 20, 30, 40];
    let r = sum_list(xs: xs);
    if r == 100 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "single_iter_consuming_call_negative");
}

#[test]
fn probe_iter_consumed_twice_inside_borrowed_callee_no_double_free() {
    // Pattern cell: the two iter-consuming calls happen INSIDE a borrowed-param
    // callee (`call_twice` borrows `words`, calls iter-consuming `sum_lens` twice)
    // — the multi-borrow keep-alive accounting must hold one call-frame deep, on
    // the borrowed param's lineage.
    let src = r#"
@sum_lens (words: [str]) -> int = {
    let total = 0;
    for w in words do {
        total = total + w.len();
    };
    total
}

@call_twice (words: [str]) -> int = {
    sum_lens(words: words) + sum_lens(words: words)
}

@main () -> int = {
    let words = ["this is a very long string that exceeds SSO threshold"];
    let r = call_twice(words: words);
    if r == 106 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "iter_consumed_twice_inside_borrowed_callee");
}

// Compound-source-element views: the for-loop body PROJECTS a sub-value
// OUT of a COMPOUND iter-element-view (a variant payload / struct field / inner
// collection of a list element) and uses it. The projected interior is a BORROW
// into the collection buffer — the source's `elem_dec_fn` frees it via
// `ori_iter_drop`, so a burden dec on the projected interior double-frees. The
// joint Project-chain + Let-alias fixpoint in `collect_iter_element_defs` must
// classify the nested projection as a borrow-view so NO interior dec is emitted.

#[test]
fn probe_for_yield_option_str_match_projected_interior_no_double_free() {
    // `match item { Some(s) -> s.length() }` projects `s` (str) out of the
    // `Option<str>` iter-element-view via `Project (variant payload).1`. The str
    // view is a borrow; the source list's `elem_dec_fn` frees the interior strs.
    let src = r#"
@main () -> int = {
    let items: [Option<str>] = [
        Some("this is a very long string that exceeds SSO threshold"),
        None,
        Some("another very long string exceeding threshold here")
    ];
    let lengths = for item in items yield match item {
        Some(s) -> s.length(),
        None -> 0,
    };
    if lengths[0] == 53 && lengths[1] == 0 && lengths[2] == 49 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_yield_option_str_match_projected_interior");
}

#[test]
fn probe_for_yield_struct_field_projected_interior_no_double_free() {
    // `item.name.length()` projects the `name: str` field out of the `Item`
    // iter-element-view via `Project (struct).0`. The field view is a borrow.
    let src = r#"
type Item = { name: str }

@main () -> int = {
    let items = [
        Item { name: "this is a very long name that exceeds SSO threshold" },
        Item { name: "another long name exceeding SSO for sure here" }
    ];
    let lengths = for item in items yield item.name.length();
    if lengths[0] == 51 && lengths[1] == 45 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "for_yield_struct_field_projected_interior");
}

// Nested-loop iter-element-view keep-alive: the inner loop's source is a
// `Project @__iter_next.1` of the OUTER source (an iter-element-view) consumed
// `[own]` by the inner `@iter`. The inner element view owns no allocation (the
// outer `elem_dec_fn` frees it), yet the inner `@iter [own]` -> `ori_iter_drop`
// ALSO frees it -> double-free WITHOUT a keep-alive inc on the inner view.

#[test]
fn probe_nested_for_do_str_inner_list_keepalive_no_double_free() {
    let src = r#"
@main () -> int = {
    let outer = [
        ["this string exceeds SSO threshold by being very long indeed",
         "second string also exceeds the SSO threshold for heap alloc"],
        ["third string that is long enough to require heap allocation"]
    ];
    let count = 0;
    for inner in outer do {
        for s in inner do count = count + 1;
    };
    if count == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "nested_for_do_str_inner_list_keepalive");
}

#[test]
fn probe_nested_for_do_three_level_int_list_keepalive_no_double_free() {
    // Three nesting levels: each level's `Project @__iter_next.1` inner-list view
    // is iter-consumed by the next `@iter [own]` and needs its own keep-alive.
    let src = r#"
@main () -> int = {
    let outer = [[[1, 2], [3, 4]], [[5, 6]]];
    let count = 0;
    for middle in outer do {
        for inner in middle do count = count + 1;
    };
    if count == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "nested_for_do_three_level_int_list_keepalive");
}

#[test]
fn probe_for_yield_inner_list_user_callee_iter_consume_keepalive_no_double_free() {
    // `for l in lists yield sum_list(l)` — the inner `[int]` element view `l` is
    // passed to a USER callee `sum_list` whose `ParamContract.iter_consumes` is
    // true (its body `for x in xs` -> `@iter [own]` -> `ori_iter_drop` frees the
    // arg). The view needs a keep-alive inc before the iter-consuming call.
    let src = r#"
@sum_list (xs: [int]) -> int = {
    let total = 0;
    for x in xs do {
        total = total + x;
    };
    total
}

@main () -> int = {
    let lists = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    let sums = for l in lists yield sum_list(l);
    if sums[0] == 6 && sums[1] == 15 && sums[2] == 24 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(
        src,
        "for_yield_inner_list_user_callee_iter_consume_keepalive",
    );
}

#[test]
fn probe_flat_str_yield_not_keepalive_negative() {
    // NEGATIVE / canary: a FLAT `[str]` source whose element `s` is yielded
    // IDENTITY (`yield s`) — `s` is NOT projected out of a compound element, and
    // the result owns its own copies. The nested keep-alive must NOT fire here
    // (no inner iter-consume of a borrow-view); the joint-fixpoint classifier
    // must NOT over-exclude the flat element. Guards the flat-element yield-RC
    // path against an over-fire from the compound-projection cure.
    let src = r#"
@main () -> int = {
    let words = [
        "this is a very long string that exceeds SSO threshold here",
        "another very long string that also exceeds the SSO bound"
    ];
    let copy = for w in words yield w;
    if copy[0].length() == 58 && copy[1].length() == 56 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "flat_str_yield_not_keepalive_negative");
}

// --- Accessor-result payload retention (Spec: Annex E §AIMS RL-2 / RL-4) ---
//
// `@unwrap` / `@unwrap_err` / `@first` / `@last` / `@get` extract an OWNED heap
// payload out of a wrapper and RETAIN it (codegen `inc_value_rc` on the extracted
// element/payload). The wrapper/source is passed at a BORROWED `Invoke` terminator
// arg position; per RL-2/RL-4 it SURVIVES the accessor call and is released on the
// normal+unwind successor EDGES — never inline before the borrowed call. Emitting
// the source dec inline frees the payload BEFORE the accessor's retain runs ->
// use-after-free. The oracle (predicate-stack ON) relocates the source dec to both
// successor edges; the burden path must match.

#[test]
fn probe_option_unwrap_heap_str_payload_retained() {
    // `o.unwrap()` extracts an owned heap str out of `Option<str>`. The wrapper
    // `o` is borrowed by `@unwrap`; its dec must land on the successor edge AFTER
    // `@unwrap` retains the payload, not inline before -> else the payload is
    // freed under the still-aliasing result (UAF; masked at the floor only by
    // same-size allocator slot reuse).
    let src = r#"
@make () -> Option<str> = Some("hello world this is a long heap string");

@main () -> int = {
    let o = make();
    let v = o.unwrap();
    if v == "hello world this is a long heap string" then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "option_unwrap_heap_str_payload_retained");
}

#[test]
fn probe_option_unwrap_heap_str_different_size_literal_no_double_free() {
    // The different-size-literal variant: the `==` comparison literal is a
    // DIFFERENT length than the unwrapped payload, so the freed payload slot is
    // NOT reused by the literal alloc -> the latent UAF surfaces as a hard
    // double-free at the clean floor (the same-size variant only passes by
    // allocator slot reuse). The comparison is intentionally false; the program
    // still returns 0 so the probe asserts a clean exit. Proves the fix is robust
    // (not re-masked by a different allocator coincidence).
    let src = r#"
@make () -> Option<str> = Some("hello world this is a long heap string");

@main () -> int = {
    let o = make();
    let v = o.unwrap();
    let matched = v == "a different length comparison literal that is quite a bit longer than the payload";
    if matched then 1 else 0
}
"#;
    assert_burden_path_self_sufficient(
        src,
        "option_unwrap_heap_str_different_size_literal_no_double_free",
    );
}

#[test]
fn probe_result_unwrap_heap_str_payload_retained() {
    let src = r#"
@make () -> Result<str, str> = Ok("hello world this is a long heap string");

@main () -> int = {
    let r = make();
    let v = r.unwrap();
    if v == "hello world this is a long heap string" then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "result_unwrap_heap_str_payload_retained");
}

#[test]
fn probe_result_unwrap_err_list_payload_retained() {
    let src = r#"
@make () -> Result<str, [int]> = Err([10, 20, 30]);

@main () -> int = {
    let r = make();
    let v = r.unwrap_err();
    if v == [10, 20, 30] then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "result_unwrap_err_list_payload_retained");
}

#[test]
fn probe_list_first_heap_str_payload_retained() {
    // `items.first()` returns `Option<str>` whose Some payload is a RETAINED copy
    // of the first element. The list `items` is borrowed by `@first`; its dec
    // belongs on the successor edge AFTER `@first` retains the element copy.
    let src = r#"
@make () -> [str] = ["hello world this is a long heap string", "another long heap string here"];

@main () -> int = {
    let items = make();
    let f = items.first();
    let v = f.unwrap();
    if v == "hello world this is a long heap string" then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_first_heap_str_payload_retained");
}

#[test]
fn probe_list_last_list_payload_retained() {
    let src = r#"
@make () -> [[int]] = [[1, 2], [3, 4, 5]];

@main () -> int = {
    let items = make();
    let l = items.last();
    let v = l.unwrap();
    if v == [3, 4, 5] then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_last_list_payload_retained");
}

#[test]
fn probe_eq_comparison_literal_stays_elidable_negative() {
    // NEGATIVE / over-fire guard: a heap str compared by `==` against a heap
    // literal, with NO accessor in play. The comparison literal is a borrow-read
    // operand (RL-1 `!incElidable`) — relocating accessor source decs to edges
    // must NOT disturb the comparison-literal balance, and the `==`-literal must
    // stay leak-free. Clamps the cure to accessor-source decs only.
    let src = r#"
@main () -> int = {
    let a = "this is a long heap string that exceeds the SSO inline threshold";
    let b = "this is a long heap string that exceeds the SSO inline threshold";
    if a == b then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "eq_comparison_literal_stays_elidable_negative");
}

#[test]
fn probe_list_contains_borrowed_read_no_payload_negative() {
    // NEGATIVE / clamp: `.contains(value:)` is a borrowed-read returning a SCALAR
    // (`bool`) — it extracts NO heap payload. The source list dec must keep its
    // existing balance and the heap-str literal arg must stay leak-free; the
    // accessor-source relocation must NOT over-fire on a no-payload borrowed read.
    let src = r#"
@main () -> int = {
    let items = ["one long string element here that exceeds sso", "two long string element here that exceeds sso"];
    let has = items.contains(value: "one long string element here that exceeds sso");
    if has then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_contains_borrowed_read_no_payload_negative");
}

// No-use dead-owned-collection scope-exit cleanup dec (RL-2 unused-owned).
//
// `let x = [Outer { .. }]` constructs an owned `[T]` that is DEAD at scope exit
// with ZERO uses. RL-2 mandates an immediate scope-exit cleanup dec on an unused
// owned non-scalar definition ("unused owned non-scalar (Dead/Absent) -> immediate
// RcDec at definition", Spec: Annex E §AIMS RL-2). The predicate-stack ORACLE emits
// a straight-line `RcDec %x [HeapPtr]` at scope exit; the burden walk omitted it
// (the dead-owned-collection sink required a borrowed-read LAST use, which a
// never-used value lacks). The omission leaks the value AND silently elides the
// element's user `@drop` side-effects. When the element type has a panicking
// `@drop`, the missing dec is observable: exit 0 + no drop print, instead of the
// drop running (its print appears) + unwind exit 1. One whole-collection dec walks
// `elem_dec_fn` recursively through nested struct / map / enum payloads and the
// runtime drop-glue handles the panic-during-drop continuation.

/// Compile `source` with the predicate-stack RC emitter OFF and assert the dead
/// no-use owned value's user `@drop` actually runs (its `expect_print` appears in
/// stdout — the cleanup dec fired), the program unwinds (exit 1, not abort 134 or
/// silent leak exit 0), and no leak / double-free diagnostic surfaces.
fn assert_burden_dead_no_use_drop_runs(source: &str, expect_print: &str, label: &str) {
    let (exit, stdout, stderr) =
        compile_and_run_with_build_env(source, &[("ORI_DISABLE_PREDICATE_STACK_RC", "1")]);
    assert!(
        stdout.contains(expect_print),
        "[{label}] dead no-use owned value's @drop must run (cleanup dec must fire); \
         expected `{expect_print}` in stdout\nexit: {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        exit == 1,
        "[{label}] single @drop panic must unwind (1), not abort or silently leak; \
         exit was {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[{label}] dead no-use owned run reported a leak / double-free\nstderr:\n{stderr}"
    );
}

#[test]
fn probe_dead_no_use_list_struct_drop_runs_at_scope_exit() {
    // Canonical shape: `let r = [Resource { .. }]`, dead with NO use. The oracle
    // emits `RcDec %r` at scope exit; the burden walk must emit the same dec so the
    // owned field's @drop (print) runs on the unwind path.
    let src = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

type Resource = { a: Logged }

impl Resource: Drop {
    @drop (self) -> void = panic(msg: "intentional");
}

@main () -> void = {
    let r = [Resource { a: Logged { tag: "a" } }]
}
"#;
    assert_burden_dead_no_use_drop_runs(src, "drop-a", "dead_no_use_list_struct");
}

#[test]
fn probe_dead_no_use_list_struct_user_panic_drop_runs() {
    // `[Holder { payload: <heap-str> }]`, dead no-use. The user @drop body prints
    // then panics — the print proves the scope-exit dec fired and ran the drop glue.
    let src = r#"
type Holder = { payload: str }

impl Holder: Drop {
    @drop (self) -> void = {
        print(msg: "drop-user");
        panic(msg: "boom")
    }
}

@main () -> void = {
    let h = [Holder { payload: "owned-heap-string-not-sso-xxxxxxxxxxxxxxxx" }]
}
"#;
    assert_burden_dead_no_use_drop_runs(src, "drop-user", "dead_no_use_list_struct_user_panic");
}

#[test]
fn probe_dead_no_use_list_map_value_drop_runs() {
    // `[Wrap { m: {"k": Boom{..}} }]`, dead no-use. The single outer-List dec walks
    // `elem_dec_fn` -> Wrap field walk -> Map two-channel teardown -> Boom @drop.
    let src = r#"
type Boom = { tag: str }

impl Boom: Drop {
    @drop (self) -> void = {
        print(msg: `boom-{self.tag}`);
        panic(msg: "boom")
    }
}

type Wrap = { m: {str: Boom} }

@main () -> void = {
    let w = [Wrap { m: { "k": Boom { tag: "v" } } }]
}
"#;
    assert_burden_dead_no_use_drop_runs(src, "boom-v", "dead_no_use_list_map_value");
}

#[test]
fn probe_dead_no_use_list_enum_payload_drop_runs() {
    // `[Both(loud:.., quiet:..)]`, dead no-use. The outer-List dec walks to the enum
    // payload: the loud field @drop panics, the sibling quiet field still drops via
    // the landing pad (both prints appear).
    let src = r#"
type Loud = { tag: str }

impl Loud: Drop {
    @drop (self) -> void = {
        print(msg: `loud-{self.tag}`);
        panic(msg: "loud-boom")
    }
}

type Quiet = { tag: str }

impl Quiet: Drop {
    @drop (self) -> void = print(msg: `quiet-{self.tag}`);
}

type Wrapper = Both(loud: Loud, quiet: Quiet);

@main () -> void = {
    let w = [Both(
        loud: Loud { tag: "L" },
        quiet: Quiet { tag: "Q" },
    )]
}
"#;
    let (exit, stdout, stderr) =
        compile_and_run_with_build_env(src, &[("ORI_DISABLE_PREDICATE_STACK_RC", "1")]);
    assert!(
        stdout.contains("loud-L") && stdout.contains("quiet-Q"),
        "[dead_no_use_list_enum_payload] both payload fields must drop (cleanup dec fired); \
         exit {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        exit == 1,
        "[dead_no_use_list_enum_payload] payload field-drop panic must unwind (1); \
         exit {exit}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[dead_no_use_list_enum_payload] leak / double-free reported\nstderr:\n{stderr}"
    );
}

#[test]
fn probe_dead_no_use_list_nested_collection_element_drop_runs() {
    // Dead no-use `[Mixed { items: [Counted, Counted], trigger: Bomb }]`: the outer-
    // List dec walks `elem_dec_fn` -> Mixed field walk -> the inner `[Counted]`
    // element drops (non-panicking) + the Bomb trigger @drop (panics once). One
    // panic unwinds (exit 1); the landing pad still drops the remaining nested
    // elements (all three prints appear). Exercises the recursive elem_dec_fn
    // composition through a nested collection field on the no-use cleanup dec.
    let src = r#"
type Counted = { tag: str }

impl Counted: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

type Bomb = { tag: str }

impl Bomb: Drop {
    @drop (self) -> void = {
        print(msg: `bomb-{self.tag}`);
        panic(msg: "bomb")
    }
}

type Mixed = { items: [Counted], trigger: Bomb }

@main () -> void = {
    let m = [Mixed {
        items: [Counted { tag: "a" }, Counted { tag: "b" }],
        trigger: Bomb { tag: "T" },
    }]
}
"#;
    let (exit, stdout, stderr) =
        compile_and_run_with_build_env(src, &[("ORI_DISABLE_PREDICATE_STACK_RC", "1")]);
    assert!(
        stdout.contains("bomb-T") && stdout.contains("drop-a") && stdout.contains("drop-b"),
        "[dead_no_use_list_nested_collection_element] panicking trigger + remaining nested \
         elements must drop via the landing pad (cleanup dec fired); \
         exit {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        exit == 1,
        "[dead_no_use_list_nested_collection_element] element @drop panic must unwind (1), \
         not abort or silently leak (0); exit {exit}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL") && !stderr.contains("already-freed"),
        "[dead_no_use_list_nested_collection_element] double-free reported\nstderr:\n{stderr}"
    );
}

#[test]
fn probe_dead_no_use_list_int_buffer_freed_at_scope_exit() {
    // A dead no-use `[int]` literal has NO heap-bearing elements with a user @drop,
    // but its buffer IS an owned allocation that must be freed exactly once. Without
    // the no-use scope-exit cleanup the buffer leaks (alloc `+1` unreleased); the dec
    // must fire (no leak) and must NOT double-free (the int buffer has a null
    // elem_dec_fn). The exit-0 leak-free clamp also pins that the cleanup composes
    // with `ori_buffer_rc_dec`'s null-elem_dec_fn path (no spurious element walk).
    let src = r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    0
}
"#;
    assert_burden_path_self_sufficient(src, "dead_no_use_list_int_buffer_freed_at_scope_exit");
}

#[test]
fn probe_dead_no_use_returned_list_no_double_dec_negative() {
    // NEGATIVE / transfer clamp: a fresh owned list that IS RETURNED is an RL-2
    // ownership transfer — the caller inherits the release. The dead-no-use
    // scope-exit cleanup must NOT fire on a returned value (a double-dec aborts).
    let src = r#"
@build () -> [int] = {
    let xs = [10, 20, 30];
    xs
}

@main () -> int = {
    let ys = build();
    if ys.length() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "dead_no_use_returned_list_no_double_dec_negative");
}

#[test]
fn probe_dead_no_use_used_list_no_extra_dec_negative() {
    // NEGATIVE / used-value clamp: an owned list that IS USED (borrowed-read
    // `.length()`) before dying at scope exit is handled by the existing borrowed-
    // read last-use sink, NOT the no-use path. The no-use cleanup must NOT also fire
    // (a double release on a used-then-dead value aborts).
    let src = r#"
@main () -> int = {
    let xs = ["alpha long heap element here exceeds sso", "beta long heap element here exceeds sso"];
    let n = xs.length();
    if n == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "dead_no_use_used_list_no_extra_dec_negative");
}

// Dead-no-use INLINE-AGGREGATE matrix (RL-2 ScopeExit). A bare `let a = Doc {
// field: <heap> }` / `let c = Link(..)` / `let t = (.., ..)` binds an inline
// struct / enum / tuple (`ValueRepr::Aggregate`, lowered `RcStrategy::AggregateFields`
// for struct/tuple and `RcStrategy::InlineEnum` for sum types) whose type
// `burden_carries_rc` (a heap-bearing `owned_fields` / `variant_burdens` field),
// dead with ZERO uses. The oracle emits one scope-exit `RcDec [AggFields]` /
// `[InlineEnum]` that walks the field drop-glue, freeing the heap field(s) and
// running their user `@drop`. Under sole-emitter burden lowering the Phase-5 walk
// emits ZERO burden ops on the no-use aggregate (no duplicating use -> no inc, no
// last-use sink -> no dec), so the heap field is NEVER freed and its `@drop`
// silently does not run (a leak; observable as the missing drop print). Distinct
// from the dead-no-use COLLECTION shapes above (`let r = [Resource {..}]`): those
// wrap the aggregate in an `RcPointer` list buffer; these are the BARE inline
// aggregate. An inline aggregate has NO self-buffer `+1` (it is not heap-allocated);
// the dec balances the HEAP FIELD's implicit `+1` owned by the AggFields / InlineEnum
// drop-glue. The cure is a NEW candidate class `compute_dead_no_use_aggregate_reps`
// gated on `var_repr in {Aggregate}` + `burden_carries_rc`, emitting one scope-exit
// `BurdenDec` on the OUTERMOST dead-no-use lineage (nested constructs are
// owned-consumed into the parent Construct -> excluded). Spec: Annex E §AIMS RL-2.

/// Compile `source` with the predicate-stack RC emitter OFF and assert the dead
/// no-use aggregate's owned field `@drop` runs (its `expect_print` appears in
/// stdout — the scope-exit cleanup dec fired), the program exits 0, and no leak /
/// double-free diagnostic surfaces. For non-panicking field drops.
fn assert_burden_dead_no_use_aggregate_drop_runs(source: &str, expect_print: &str, label: &str) {
    let (exit, stdout, stderr) =
        compile_and_run_with_build_env(source, &[("ORI_DISABLE_PREDICATE_STACK_RC", "1")]);
    assert!(
        stdout.contains(expect_print),
        "[{label}] dead no-use aggregate's field @drop must run (scope-exit cleanup dec must \
         fire); expected `{expect_print}` in stdout\nexit: {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        exit == 0,
        "[{label}] non-panicking field drops must exit 0; exit was {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[{label}] dead no-use aggregate run reported a leak / double-free\nstderr:\n{stderr}"
    );
}

#[test]
fn probe_dead_no_use_struct_str_field_drop_runs() {
    // Bare `let a = Doc { content: Logged {..} }`: a struct (`AggFields`) holding a
    // heap-bearing field, dead no-use. The scope-exit `RcDec [AggFields]` must walk
    // to the `content` field and run its @drop.
    let src = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

type Doc = { content: Logged }

@main () -> void = {
    let a = Doc { content: Logged { tag: "S" } }
}
"#;
    assert_burden_dead_no_use_aggregate_drop_runs(src, "drop-S", "dead_no_use_struct_str_field");
}

#[test]
fn probe_dead_no_use_tuple_str_fields_drop_runs() {
    // Bare `let t = (Logged {..}, Logged {..})`: a tuple (`AggFields`), dead no-use.
    // The scope-exit dec walks both tuple slots in reverse decl order.
    let src = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

@main () -> void = {
    let t = (Logged { tag: "T0" }, Logged { tag: "T1" })
}
"#;
    let (exit, stdout, stderr) =
        compile_and_run_with_build_env(src, &[("ORI_DISABLE_PREDICATE_STACK_RC", "1")]);
    assert!(
        stdout.contains("drop-T0") && stdout.contains("drop-T1"),
        "[dead_no_use_tuple_str_fields] both tuple slots must drop (cleanup dec fired); \
         exit {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        exit == 0,
        "[dead_no_use_tuple_str_fields] non-panicking tuple drops must exit 0; exit {exit}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[dead_no_use_tuple_str_fields] leak / double-free reported\nstderr:\n{stderr}"
    );
}

#[test]
fn probe_dead_no_use_option_struct_drop_runs() {
    // Bare `let a: Option<Logged> = Some(Logged {..})`: a niche/tagged Option
    // (`InlineEnum`), dead no-use. The scope-exit `RcDec [InlineEnum]` walks the
    // Some payload and runs its @drop.
    let src = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

@main () -> void = {
    let a: Option<Logged> = Some(Logged { tag: "O" })
}
"#;
    assert_burden_dead_no_use_aggregate_drop_runs(src, "drop-O", "dead_no_use_option_struct");
}

#[test]
fn probe_dead_no_use_result_struct_drop_runs() {
    // Bare `let a: Result<Logged, int> = Ok(Logged {..})`: a tagged Result
    // (`InlineEnum`), dead no-use. The scope-exit `RcDec [InlineEnum]` walks the Ok
    // payload and runs its @drop.
    let src = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

@main () -> void = {
    let a: Result<Logged, int> = Ok(Logged { tag: "RE" })
}
"#;
    assert_burden_dead_no_use_aggregate_drop_runs(src, "drop-RE", "dead_no_use_result_struct");
}

#[test]
fn probe_dead_no_use_user_enum_payload_drop_runs() {
    // Bare `let c = Link(a:.., b:.., next: Link(.., next: Nil))`: a user sum type
    // (`InlineEnum`) with a heap-bearing recursive payload, dead no-use. The single
    // scope-exit `RcDec [InlineEnum]` on the OUTERMOST `c` lineage walks every node's
    // payload recursively (the nested Link is owned-consumed into the outer Construct,
    // so it gets NO separate dec) and runs every node's field @drop.
    let src = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

type Chain = Nil | Link(a: Logged, b: Logged, next: Chain);

@main () -> void = {
    let c = Link(
        a: Logged { tag: "outer-a" },
        b: Logged { tag: "outer-b" },
        next: Link(
            a: Logged { tag: "inner-a" },
            b: Logged { tag: "inner-b" },
            next: Nil,
        ),
    )
}
"#;
    let (exit, stdout, stderr) =
        compile_and_run_with_build_env(src, &[("ORI_DISABLE_PREDICATE_STACK_RC", "1")]);
    for tag in [
        "drop-outer-a",
        "drop-outer-b",
        "drop-inner-a",
        "drop-inner-b",
    ] {
        assert!(
            stdout.contains(tag),
            "[dead_no_use_user_enum_payload] every node payload field @drop must run via the \
             single outermost cleanup dec; missing {tag}; exit {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }
    assert!(
        exit == 0,
        "[dead_no_use_user_enum_payload] non-panicking node drops must exit 0; exit {exit}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[dead_no_use_user_enum_payload] leak / double-free reported (the outermost lineage dec \
         must NOT double-free the owned-consumed nested node)\nstderr:\n{stderr}"
    );
}

#[test]
fn probe_dead_no_use_nested_struct_field_drop_runs() {
    // Bare `let w = Outer { inner: Inner { payload: Boom {..} } }`: nested structs
    // (`AggFields`) where the innermost field @drop panics. The single outermost
    // scope-exit dec walks Outer -> Inner -> Boom and the panic unwinds (exit 1); the
    // print proves the dec fired and reached the nested field.
    let src = r#"
type Boom = { tag: str }

impl Boom: Drop {
    @drop (self) -> void = {
        print(msg: `boom-{self.tag}`);
        panic(msg: "boom")
    }
}

type Inner = { payload: Boom }

type Outer = { inner: Inner }

@main () -> void = {
    let w = Outer { inner: Inner { payload: Boom { tag: "N" } } }
}
"#;
    let (exit, stdout, stderr) =
        compile_and_run_with_build_env(src, &[("ORI_DISABLE_PREDICATE_STACK_RC", "1")]);
    assert!(
        stdout.contains("boom-N"),
        "[dead_no_use_nested_struct_field] the nested field @drop must run via the outermost \
         cleanup dec walking the field tree; exit {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        exit == 1,
        "[dead_no_use_nested_struct_field] the single nested field-drop panic must unwind (1), \
         not abort or silently leak (0); exit {exit}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL") && !stderr.contains("already-freed"),
        "[dead_no_use_nested_struct_field] double-free reported\nstderr:\n{stderr}"
    );
}

#[test]
fn probe_dead_no_use_heap_str_field_freed_negative() {
    // A bare struct holding a genuinely-heap (>23-byte, non-SSO) str field, dead
    // no-use, with NO user @drop: the str buffer is an owned allocation that must be
    // freed exactly once via the scope-exit `RcDec [AggFields]` walking the field's
    // FatPointer. Without the cleanup the buffer leaks (alloc `+1` unreleased); the
    // exit-0 leak-free clamp pins the cleanup fires and does not double-free.
    let src = r#"
type Doc = { content: str }

@main () -> int = {
    let a = Doc { content: "owned-heap-string-not-sso-xxxxxxxxxxxxxxxx" };
    0
}
"#;
    assert_burden_path_self_sufficient(src, "dead_no_use_heap_str_field_freed");
}

#[test]
fn probe_dead_no_use_returned_aggregate_no_double_free_negative() {
    // NEGATIVE / transfer clamp: a fresh owned aggregate that IS RETURNED is an RL-2
    // ownership transfer — the caller inherits the release. The dead-no-use scope-exit
    // cleanup must NOT fire on a returned aggregate (a double-dec aborts / double-frees
    // the heap field).
    let src = r#"
type Logged = { tag: str }

impl Logged: Drop {
    @drop (self) -> void = print(msg: `drop-{self.tag}`);
}

type Doc = { content: Logged }

@build () -> Doc = {
    let a = Doc { content: Logged { tag: "R" } };
    a
}

@main () -> void = {
    let d = build();
    print(msg: d.content.tag)
}
"#;
    let (exit, stdout, stderr) =
        compile_and_run_with_build_env(src, &[("ORI_DISABLE_PREDICATE_STACK_RC", "1")]);
    // The returned aggregate's field @drop runs exactly once (the caller's transfer
    // release), printing `drop-R` after the `R` field read.
    assert!(
        exit == 0 && stdout.contains("drop-R") && stdout.contains('R'),
        "[dead_no_use_returned_aggregate] returned aggregate must be released once by the caller; \
         exit {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[dead_no_use_returned_aggregate] the no-use cleanup must NOT add a second dec on a \
         returned aggregate (double-free)\nstderr:\n{stderr}"
    );
}

#[test]
fn probe_dead_no_use_scalar_only_struct_no_dec_negative() {
    // NEGATIVE / non-burden-carrying clamp: a scalar-only struct (`{ x: int, y: int }`)
    // has no heap-bearing field, so `burden_carries_rc` is false and the no-use cleanup
    // must NOT fire (a `RcDec [AggFields]` on a struct with a null field drop-glue is a
    // spurious release). Exit-0 leak-free proves the pass skips it.
    let src = r#"
type Point = { x: int, y: int }

@main () -> int = {
    let p = Point { x: 1, y: 2 };
    0
}
"#;
    assert_burden_path_self_sufficient(src, "dead_no_use_scalar_only_struct_no_dec");
}

// Take-project iterator-handle source matrix (RL-2 ScopeExit + bypass-safe
// per-class drop). An `Iterator<int>` payload inside an enum is projected out
// and consumed on one match arm; on every NON-projecting path the source enum
// (holding the iterator handle) is dead-at-scope-exit and must be freed
// (`RcDec [InlineEnum]` -> the InlineEnum drop walks the iterator field ->
// `ori_iter_drop`). Under sole-emitter burden lowering the Phase-5 walk
// mis-models the take-project source: it emits a spurious dec on the consuming
// arm (-> use-after-free, the iterator is freed before `@count` reads it) and
// omits the dec on the bypass / Empty paths (-> leak). The cure mirrors the
// predicate-stack `dead_cleanup` bypass-safe-entry emission via the shared
// `TakeMoveFacts` SSOT.

#[test]
fn probe_take_project_match_consume_no_use_after_free() {
    // Iterator projected out of `Holds` and consumed via `.count()`. The source
    // enum must NOT be decced on the consume arm (the projection transfers the
    // iterator out; `@count` owns + frees it) -> a dec there is a use-after-free.
    let src = r#"
type MaybeIter = Empty | Holds(it: Iterator<int>);

@main () -> int = {
    let x: MaybeIter = Holds(it: [1, 2, 3].iter());
    let n = match x {
        Empty -> 0,
        Holds(it) -> it.count()
    };
    if n == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "take_project_match_consume");
}

#[test]
fn probe_take_project_conditional_consume_no_leak() {
    // Path-sensitive: `if flag then <match consumes> else 0`. On the runtime
    // bypass path (flag false) the iterator is never consumed, so the source
    // enum is dead-at-scope-exit on the else branch and must be freed there
    // (the burden walk omitted that dec -> leak).
    let src = r#"
type MaybeIter = Empty | Holds(it: Iterator<int>);

@main () -> int = {
    let x: MaybeIter = Holds(it: [1, 2, 3].iter());
    let flag = false;
    if flag then
        match x {
            Empty -> 0,
            Holds(it) -> it.count()
        }
    else
        0
}
"#;
    assert_burden_path_self_sufficient(src, "take_project_conditional_consume");
}

#[test]
fn probe_take_project_dynamic_consume_no_double_free() {
    // Dynamic Holds/Empty construction via a helper -> the match diamond is not
    // constant-folded, both arms live. The Empty arm frees the whole enum; the
    // Holds arm transfers the iterator out -> no double-free across the diamond.
    let src = r#"
type MaybeIter = Empty | Holds(it: Iterator<int>);

@build (use_holds: bool) -> MaybeIter =
    if use_holds then Holds(it: [1, 2, 3].iter()) else Empty;

@main () -> int = {
    let x = build(use_holds: true);
    let n = match x {
        Empty -> 0,
        Holds(it) -> it.count()
    };
    if n == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "take_project_dynamic_consume");
}

#[test]
fn probe_take_project_two_unrelated_sources_no_leak() {
    // Two independent take-project sources `a`, `b` on disjoint alias chains:
    // `b` is consumed, `a` is on a bypass path. `a` must drop on the bypass-safe
    // path via its own per-class scope-exit drop (function-global bypass-safe
    // computation would suppress `a`'s drop on every block reachable from `b`).
    let src = r#"
type MaybeIter = Empty | Holds(it: Iterator<int>);

@main () -> int = {
    let a: MaybeIter = Holds(it: [1, 2, 3].iter());
    let b: MaybeIter = Holds(it: [4, 5, 6].iter());
    let flag1 = false;
    let flag2 = true;
    let count_b = if flag1 then
        match a {
            Empty -> 0,
            Holds(it) -> it.count()
        }
    else
        if flag2 then
            match b {
                Empty -> 0,
                Holds(it) -> it.count()
            }
        else
            0;
    count_b - count_b
}
"#;
    assert_burden_path_self_sufficient(src, "take_project_two_unrelated_sources");
}

#[test]
fn probe_take_project_phi_merge_no_leak() {
    // Two take-project sources whose match-arm RESULTS converge at a phi-style
    // merge block param. The phi param is a CFG choice, not shared storage; the
    // per-class bypass-safe set must not falsely conflate the two sources'
    // lineages through the shared merge param.
    let src = r#"
type MaybeIter = Empty | Holds(it: Iterator<int>);

@main () -> int = {
    let a: MaybeIter = Holds(it: [1, 2, 3].iter());
    let b: MaybeIter = Holds(it: [4, 5, 6].iter());
    let pick = false;
    let result = if pick then
        match a {
            Empty -> 0,
            Holds(it) -> it.count()
        }
    else
        match b {
            Empty -> 0,
            Holds(it) -> it.count()
        };
    result - 3
}
"#;
    assert_burden_path_self_sufficient(src, "take_project_phi_merge");
}

#[test]
fn probe_take_project_in_loop_no_leak() {
    // Topology: a take-project source held across an explicit `loop { break }`,
    // consumed conditionally after. The loop body never reaches the projection
    // (bypass path); the source enum must drop on the post-loop bypass-safe path
    // even though the loop header is reached via a back-edge from a bypass-safe
    // latch.
    let src = r#"
type MaybeIter = Empty | Holds(it: Iterator<int>);

@main () -> int = {
    let x: MaybeIter = Holds(it: [1, 2, 3].iter());
    let do_take = false;
    let iters = 0;
    loop {
        iters = iters + 1;
        if iters >= 1 then
            break
    };
    let count = if do_take then
        match x {
            Empty -> 0,
            Holds(it) -> it.count()
        }
    else
        0;
    count + iters - 1
}
"#;
    assert_burden_path_self_sufficient(src, "take_project_in_loop");
}

#[test]
fn probe_take_project_unused_binding_negative_no_double_free() {
    // NEGATIVE / project-then-unused clamp: the iterator is projected into a
    // binding that is NEVER consumed (no `.count()`). The projected iterator
    // binding drops at its OWN scope exit (handled already); the take-project
    // source-dec extension must NOT also fire on the source enum here (a double
    // release of the same iterator payload aborts). This guards the
    // `enum_match_unused_binding` shape from regressing.
    let src = r#"
type MaybeIter = Empty | Holds(it: Iterator<int>);

@main () -> int = {
    let x: MaybeIter = Holds(it: [1, 2, 3].iter());
    match x {
        Empty -> 0,
        Holds(it) -> 0
    }
}
"#;
    assert_burden_path_self_sufficient(src, "take_project_unused_binding_negative");
}

#[test]
fn probe_eager_filter_borrowed_source_freed_after_call() {
    // `nums.filter(p)`: the eager list filter BORROWS `nums` and produces a
    // FRESH non-aliasing `[int]` result (a distinct buffer, never a view into
    // `nums`). The Phase-5 walk misplaces `nums`'s scope-exit dec INLINE before
    // `Invoke @filter(nums [borrow]) normal/unwind` -> the source buffer frees
    // before the callee reads it -> use-after-free + double-free. RL-2
    // (`ApplyToBorrowedParam` emits a caller dec) + RL-4 (release on the dying
    // successor edge): the source survives the borrowed call, dead on each
    // successor -> relocate to BOTH edges. Fresh-result -> safe to relocate.
    let src = r#"
@main () -> int = {
    let nums: [int] = [1, 2, 3, 4, 5];
    let evens: [int] = nums.filter(predicate: x -> x % 2 == 0);
    if evens.len() == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "eager_filter_borrowed_source_freed");
}

#[test]
fn probe_eager_map_borrowed_source_freed_after_call() {
    // `nums.map(f)`: the eager list map BORROWS `nums` and produces a FRESH
    // non-aliasing `[int]` result. Same misplaced-inline-source-dec UAF as
    // filter; same RL-2 + RL-4 successor-edge relocation cure.
    let src = r#"
@main () -> int = {
    let nums: [int] = [1, 2, 3];
    let doubled: [int] = nums.map(transform: x -> x * 2);
    if doubled.len() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "eager_map_borrowed_source_freed");
}

#[test]
fn probe_eager_filter_then_index_borrowed_source_freed() {
    // `nums.filter(p)` then `evens[0]`: the fresh filter result is index-read.
    // The borrowed source `nums` still must relocate its dec to the successor
    // edge (it does not alias the fresh result, which the index reads).
    let src = r#"
@main () -> int = {
    let nums: [int] = [1, 2, 3, 4, 5, 6];
    let evens: [int] = nums.filter(predicate: x -> x % 2 == 0);
    if evens[0] == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "eager_filter_then_index_borrowed_source_freed");
}

#[test]
fn probe_bare_list_len_borrowed_source_unchanged_negative() {
    // NEGATIVE / scalar-result clamp: a bare `nums.len()` (no transform) already
    // relocates its borrowed-source dec to the successor edge via the verdict's
    // scalar-result-builtin branch (`@len` returns `int`). The eager-transform
    // extension must NOT disturb this floor-passing path — the source frees once
    // on the successor edge, leak-free + double-free-free, with or without the
    // new fresh-result set.
    let src = r#"
@main () -> int = {
    let nums: [int] = [1, 2, 3];
    if nums.len() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "bare_list_len_borrowed_source_unchanged");
}

#[test]
fn probe_list_clone_borrowed_source_freed_after_call() {
    // `xs.clone()`: list clone is an rc-INC of the SAME buffer (rc 1 -> 2), NOT a
    // deep copy — the result and source ALIAS one buffer, each holding its own
    // ref. The Phase-5 walk misplaces `xs`'s scope-exit dec INLINE before
    // `Invoke @clone(xs [borrow])` -> the source frees the shared buffer before
    // the callee incs it -> use-after-free + double-free. The borrow-survives
    // relocation moves the source dec to the successor edge (rc 2 -> 1); the
    // result's own dec frees it (1 -> 0). Both refs balanced, single free — the
    // clone-vs-buffer-sharing contract-indistinguishability is resolved by the
    // refined escape-gated Phase-6.65 relocation.
    let src = r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let ys = xs.clone();
    if ys.length() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_clone_borrowed_source_freed");
}

#[test]
fn probe_iter_map_collect_result_freed_int() {
    // `xs.iter().map(f).collect()`: the `@collect` consumer allocates a FRESH owned
    // `[int]` result (`ori_iter_collect` -> `ori_rc_alloc`, distinct from the
    // iterator + source). The iterator (and its source buffer) is freed by
    // `ori_iter_drop`; the collect RESULT is borrowed-read (`.length()`) then dead
    // at scope exit. The Phase-5 walk emitted ZERO ops on the result -> leak under
    // the flag. RL-2 scope-exit dec on the fresh result (lowering to
    // `RcDec [HeapPtr]`) frees it once; the iterator-consumer recognizer +
    // alloc-aware net catch it. Spec: Annex E §AIMS RL-2.
    let src = r#"
@main () -> int = {
    let xs = [1, 2, 3];
    let doubled = xs.iter().map(x -> x * 2).collect();
    if doubled.length() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "iter_map_collect_result_freed_int");
}

#[test]
fn probe_iter_map_collect_result_freed_heap_str() {
    // Same iter-chain shape with HEAP-string elements: the `@collect` result owns
    // its element COPIES (`ori_iter_collect` `elem_inc_fn`s each element into the
    // fresh buffer). The RL-2 scope-exit `RcDec [HeapPtr]` on the result walks the
    // V5 `elem_dec_fn` so the str copies free too (the buffer + element-glue
    // composition). Without it the result buffer + 2 element strings leak.
    let src = r#"
@main () -> int = {
    let words = [
        "this is a very long heap string that exceeds the SSO threshold of twenty three bytes",
        "another long heap string for testing trampoline ABI correctness"
    ];
    let same = words.iter().map(transform: s -> s).collect();
    if same.len() == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "iter_map_collect_result_freed_heap_str");
}

#[test]
fn probe_iter_collect_result_returned_no_double_free_negative() {
    // NEGATIVE: when the collect result is RETURNED (transferred to the caller),
    // the caller inherits the release (RL-2 transfer). The dead-owned-collection
    // pass MUST NOT emit a freeing dec on a returned collect result (the
    // `compute_returned_lineages` exclusion holds) — a dec here would double-free
    // against the caller's release. `@main` consumes the returned list locally.
    let src = r#"
@build () -> [int] = {
    let xs = [1, 2, 3];
    xs.iter().map(x -> x * 2).collect()
}

@main () -> int = {
    let got = build();
    if got.length() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "iter_collect_result_returned_no_double_free");
}

// Loop-carried-reassignment fresh-source matrix (RL-1 duplication-balanced):
// a fresh self-alloc constructed in a loop-init block, transferred into the
// loop via the loop-entry Jump-arg, and CONSUMED at an owned-position mutation
// (`push` / `insert` / `+`) each iteration. The same-site bb0 burden pair must
// lower to a harmless net-0 `[RcInc, RcDec]` pair (the fresh inc is NOT elided)
// because the in-loop owned-consume IS the source's release across the
// Jump-arg → block-param rename. Eliding the fresh inc strands a lone `RcDec`
// at the construct site (Lean `RL1_duplication_balanced`: a duplication is
// `[inc, dec]` OR `[]`, never `[dec]`), freeing the buffer before the loop
// reads it (Spec: Annex E §AIMS RL-1).

#[test]
fn probe_loop_carried_push_list_int_source_no_double_free() {
    let src = r#"
@main () -> int = {
    let xs = [1, 2, 3];
    for i in 0..30 do {
        xs = xs.push(i);
    };
    if xs.len() == 33 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "loop_carried_push_list_int_source");
}

#[test]
fn probe_loop_carried_push_list_str_source_no_double_free() {
    let src = r#"
@main () -> int = {
    let xs = ["a", "b"];
    for i in 0..10 do {
        xs = xs.push("x");
    };
    if xs.len() == 12 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "loop_carried_push_list_str_source");
}

#[test]
fn probe_loop_carried_insert_map_source_no_double_free() {
    let src = r#"
@main () -> int = {
    let m = {"seed": 0};
    let keys = ["a", "b", "c", "d"];
    for k in keys do {
        m = m.insert(key: k, value: 1);
    };
    if m.len() == 5 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "loop_carried_insert_map_source");
}

#[test]
fn probe_loop_carried_concat_str_source_no_double_free() {
    // `s = s + "x"` concat-loop: the old string operand is consumed/COW-read by
    // `ori_str_concat` (an owned-position duplicating use) each iteration.
    let src = r#"
@main () -> int = {
    let s = "ab";
    for i in 0..10 do {
        s = s + "x";
    };
    if s.length() == 12 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "loop_carried_concat_str_source");
}

#[test]
fn probe_loop_carried_push_while_source_no_double_free() {
    let src = r#"
@main () -> int = {
    let xs = [0];
    let n = 0;
    while n < 20 do {
        xs = xs.push(n);
        n = n + 1;
    };
    if xs.len() == 21 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "loop_carried_push_while_source");
}

#[test]
fn probe_loop_carried_push_break_source_no_double_free() {
    let src = r#"
@main () -> int = {
    let xs = [1];
    for i in 0..50 do {
        xs = xs.push(i);
        if xs.len() == 11 then break;
    };
    if xs.len() == 11 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "loop_carried_push_break_source");
}

#[test]
fn probe_loop_invariant_closure_borrow_no_premature_free_negative() {
    // NEGATIVE / loop-invariant-BORROW clamp: a
    // closure-env fresh value (`PartialApply` capturing `multiplier`) threaded
    // UNCHANGED through the loop and passed `[borrow]` to a callee each
    // iteration — NEVER owned-consumed. Its bb0 fresh inc IS elidable (its bb0
    // dec is the genuine scope-exit release). The loop-carried-consume cure MUST
    // NOT flag this lineage cow-mutated (the discriminator is in-loop
    // owned-CONSUME vs BORROW, not the bb0 Jump-transfer shape) — over-flagging
    // keeps a spurious inc and leaks the closure env.
    let src = r#"
@apply (f: (int) -> int, x: int) -> int = f(x);

@main () -> int = {
    let multiplier = 10;
    let scale = (n: int) -> int = n * multiplier;
    let sum = 0;
    for i in 1..=3 do {
        sum = sum + apply(f: scale, x: i);
    };
    if sum == 60 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "loop_invariant_closure_borrow_negative");
}

#[test]
fn probe_loop_invariant_map_index_borrow_no_premature_free_negative() {
    // NEGATIVE / loop-invariant read-only-BORROW clamp: a map read via `m[k]`
    // (`__index [borrow]`) each iteration, never reassigned/consumed. The map's
    // bb0 fresh inc is elidable; the loop-carried-consume cure MUST NOT flag the
    // read-only map lineage (a `[borrow]` index is not an owned-position
    // consume).
    let src = r#"
@main () -> int = {
    let m = {"alpha": 10, "beta": 20, "gamma": 30};
    let keys = ["alpha", "beta", "gamma"];
    let sum = 0;
    for k in keys do {
        let v = m[k];
        if v.is_some() then sum = sum + v.unwrap()
    };
    if sum == 60 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "loop_invariant_map_index_borrow_negative");
}

// --- Transfer-through-return forwarder RESULT freeing (RL-2 ScopeExit) ---
// A fresh-owned collection passed `[own]` into a `transfers_through_return ∧
// ReturnAliasShape::Direct` forwarder (`@id<T>(x: T) -> T = x`) is returned
// unchanged: the caller's result IS the SAME allocation as the transferred owned
// arg, borrowed-read then dead at scope exit, carrying ZERO burden ops. Under
// sole-emitter Phase-7 lowering the allocation's `+1` is never released → leak.
// The per-allocation alloc-aware net threaded through the apply-Direct transfer
// edge fires ONE scope-exit `BurdenDec` on the result's live SSA value (the
// trivial `@id` chain nets +1 = leaked → dec; a multi-borrow-then-return
// forwarder already nets 0 = released → no dec, the negative below).

#[test]
fn probe_forwarder_result_freed_id_list_int() {
    // `@id` over `[int]`: result borrowed-read (`@len`/`@__index`) then dead.
    let src = r#"
@id <T> (x: T) -> T = x;

@main () -> int = {
    let xs = id(x: [1, 2, 3]);
    if xs.len() == 3 && xs[0] == 1 && xs[2] == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "forwarder_result_freed_id_list_int");
}

#[test]
fn probe_forwarder_result_freed_multi_hop_list() {
    // Two-hop chain `@id2(@id1(xs))`: each hop is a Direct transfer; the final
    // result is the same allocation as the original Construct.
    let src = r#"
@id1 <T> (x: T) -> T = x;
@id2 <T> (x: T) -> T = id1(x: x);

@main () -> int = {
    let xs = id2(x: [7, 8, 9]);
    if xs.len() == 3 && xs[1] == 8 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "forwarder_result_freed_multi_hop_list");
}

#[test]
fn probe_forwarder_result_freed_non_generic_list() {
    // Non-generic forwarder — the apply-Direct merge is structural, not
    // generics-keyed. Straight-line result use (single condition): the lineage's
    // single unbalanced allocation `+1` is released at its one borrowed-read dead
    // sink. (Branchy multi-condition result use is the compound-shape next leaf —
    // the result's own per-branch `binc`/`bdec` pairs need joint accounting.)
    let src = r#"
@just_return_it (x: [int]) -> [int] = x;

@main () -> int = {
    let xs = just_return_it(x: [11, 22, 33]);
    if xs.len() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "forwarder_result_freed_non_generic_list");
}

#[test]
fn probe_multi_borrow_then_return_no_double_free_negative() {
    // The over-fire boundary: a forwarder that BORROW-USES its `[own]` param
    // inside the body AND returns it is contract-INDISTINGUISHABLE from the
    // trivial `@id` (both `transfers_through_return ∧ Direct`), but the burden
    // path ALREADY releases the returned-then-borrow-used lineage. The
    // alloc-aware net nets 0 (the existing release balances the alloc), so the
    // result-freeing pass must NOT add a second dec — a contract-level recognizer
    // would over-fire here and double-free.
    let src = r#"
@use_twice (xs: [int]) -> [int] = {
    print(msg: `len: {xs.len()}`);
    print(msg: `first: {xs[0]}`);
    xs
};

@main () -> int = {
    let original = [10, 20, 30];
    let returned = use_twice(xs: original);
    if returned.len() == 3 && returned[0] == 10 && returned[2] == 30 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "multi_borrow_then_return_no_double_free_negative");
}

// Project-borrowed-view aggregate-field-drop attribution matrix (RL-4 borrowed
// view emits no release + RL-2 the aggregate's `[AggFields]`/`[InlineEnum]` drop
// IS the field's single release). A `let v = w.field` borrow-view of a local
// owned aggregate whose `[AggFields]` drop frees the projected heap field gets a
// SPURIOUS scope-exit `BurdenDec` from the Phase-5 walk under sole-emitter
// lowering -> the field is freed by the view dec AND by the aggregate drop ->
// double-free. The alloc-aware net attributes the aggregate field-drop as the
// field's release, so the view's lineage nets +1 surplus (strip the dec); a
// paired-inc collection-field view (the aggregate is shared, the view dec
// releases the extra ref) nets 0 (keep). Spec: Annex E §AIMS RL-2 + RL-4.

#[test]
fn probe_project_borrowed_view_struct_str_field_no_double_free() {
    // The canonical str-field view: `let borrowed = w.s` borrow-view of a local
    // `Wrapper { s: str }`. The struct's `RcDec [AggFields]` frees the field
    // string; the spurious `RcDec %view [FatPtr]` frees it AGAIN -> double-free.
    let src = r#"
type Wrapper = { s: str }

@main () -> int = {
    let w = Wrapper { s: "borrow_then_letvar_chain" };
    let borrowed = w.s;
    let chained = borrowed;
    if chained == "borrow_then_letvar_chain" then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "project_borrowed_view_struct_str_field");
}

#[test]
fn probe_project_borrowed_view_struct_list_str_field_no_double_free() {
    // `[str]`-field view: `c.items` borrow-view of `Container { items: [str] }`.
    // The struct `[AggFields]` drop frees the `[str]` buffer (RcPtr field); the
    // spurious view dec double-frees it.
    let src = r#"
type Container = { items: [str] }

@main () -> int = {
    let c = Container { items: ["hello", "world"] };
    if c.items.length() == 2 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "project_borrowed_view_struct_list_str_field");
}

#[test]
fn probe_project_borrowed_view_struct_list_int_field_no_double_free() {
    // `[int]`-field view: `c.items` borrow-view of `Container { items: [int] }`.
    // Same shape as the `[str]` field — the scalar element type does not change
    // the aggregate-drop-frees-the-buffer accounting. The membership-strip
    // approach mishandled `[int]`-field index-retain shapes; the net does not.
    let src = r#"
type Container = { items: [int] }

@main () -> int = {
    let c = Container { items: [10, 20, 30] };
    if c.items.length() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "project_borrowed_view_struct_list_int_field");
}

#[test]
fn probe_project_borrowed_view_option_struct_str_field_no_double_free() {
    // Option-payload struct-field view: a `Some(Wrapper { s: str })` matched, then
    // the inner struct's `s` field projected and borrow-read. The InlineEnum +
    // AggFields drop walk frees the field; the spurious view dec double-frees.
    let src = r#"
type Wrapper = { s: str }

@main () -> int = {
    let o: Option<Wrapper> = Some(Wrapper { s: "abcdefghijklmnopqrstuvwxyz1234" });
    match o {
        Some(w) -> if w.s.length() == 30 then 0 else 1,
        None -> 2,
    }
}
"#;
    assert_burden_path_self_sufficient(src, "project_borrowed_view_option_struct_str_field");
}

#[test]
fn probe_project_borrowed_view_result_struct_str_field_no_double_free() {
    // Result-payload struct-field view: an `Ok(Wrapper { s: str })` matched, then
    // the inner struct's `s` field projected and borrow-read. Same InlineEnum +
    // AggFields drop walk; the spurious view dec double-frees.
    let src = r#"
type Wrapper = { s: str }

@main () -> int = {
    let r: Result<Wrapper, int> = Ok(Wrapper { s: "abcdefghijklmnopqrstuvwxyz1234" });
    match r {
        Ok(w) -> if w.s.length() == 30 then 0 else 1,
        Err(e) -> e,
    }
}
"#;
    assert_burden_path_self_sufficient(src, "project_borrowed_view_result_struct_str_field");
}

#[test]
fn probe_project_borrowed_view_paired_inc_collection_field_keep_negative() {
    // NEGATIVE / keep clamp: a struct with TWO projected fields (a map field AND a
    // str field) each `.length()`-read. The aggregate is copied (a paired
    // `[AggFields]` inc bumps the fields above rc 1), so each projection dec
    // releases the EXTRA reference, NOT a redundant second release of a single-ref
    // field. The alloc-aware net is 0 (the aggregate inc balances the alloc) ->
    // the dec is the genuine release and MUST be kept. A membership-strip orphans
    // the index-retain inc here and leaks; the net keeps it.
    let src = r#"
type Config = { settings: {str: int}, name: str }

@main () -> int = {
    let c = Config { settings: {"a": 1}, name: "cfg" };
    if c.settings.length() + c.name.length() == 4 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(
        src,
        "project_borrowed_view_paired_inc_collection_field_keep",
    );
}

#[test]
fn probe_project_borrowed_view_sum_str_payload_keep_negative() {
    // NEGATIVE / keep clamp: a `Text(content: str)` sum variant matched, the `str`
    // payload extracted and borrow-read. The match-extract is paired with the
    // enum's keep-alive inc (the payload survives the match into the arm body),
    // so the payload's release is balanced (net 0) and MUST be kept. Stripping it
    // leaks the heap string.
    let src = r#"
type Value = Text(content: str) | Empty;

@main () -> int = {
    let v = Text(content: "abcdefghijklmnopqrstuvwxyz1234");
    match v {
        Text(content) -> if content.length() == 30 then 0 else 1,
        Empty -> 2,
    }
}
"#;
    assert_burden_path_self_sufficient(src, "project_borrowed_view_sum_str_payload_keep");
}

#[test]
fn probe_project_borrowed_view_sum_list_int_payload_keep_negative() {
    // NEGATIVE / keep clamp: a `Numbers(items: [int])` sum variant matched, the
    // `[int]` payload extracted and borrow-read. Same paired-inc keep accounting
    // as the str payload — the RcPtr buffer's release is balanced (net 0). The
    // last-owner sum-payload view is the buffer's genuine release; keep it.
    let src = r#"
type Data = Numbers(items: [int]) | Empty;

@main () -> int = {
    let d = Numbers(items: [10, 20, 30]);
    match d {
        Numbers(items) -> if items.length() == 3 then 0 else 1,
        Empty -> 2,
    }
}
"#;
    assert_burden_path_self_sufficient(src, "project_borrowed_view_sum_list_int_payload_keep");
}

#[test]
fn probe_project_borrowed_view_owned_literal_release_keep_negative() {
    // NEGATIVE / keep clamp: a bare owned heap str literal (NOT a projection) with
    // its own last-use release. The strip discriminator keys on a `Project`-view
    // whose source aggregate drop frees the field; an owned non-view value has no
    // such source, so its release MUST be kept. Stripping it leaks the string.
    let src = r#"
@main () -> int = {
    let a = "abcdefghijklmnopqrstuvwxyz1234";
    if a.length() == 30 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "project_borrowed_view_owned_literal_release_keep");
}

#[test]
fn probe_project_borrowed_view_disjoint_field_no_double_free() {
    // A struct with TWO heap fields where ONE is projected-and-borrow-read and the
    // OTHER is unused. The aggregate `[AggFields]` drop frees BOTH fields; the
    // spurious view dec on the projected field double-frees it (the unused field
    // is freed once by the aggregate drop — no view to double it). Strip the
    // projected-view dec; the aggregate drop owns both releases.
    let src = r#"
type Pair = { a: str, b: str }

@main () -> int = {
    let p = Pair { a: "abcdefghijklmnopqrstuvwxyz1234", b: "0987654321zyxwvutsrqponmlkji" };
    let projected = p.a;
    if projected.length() == 30 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "project_borrowed_view_disjoint_field");
}

#[test]
fn probe_derived_eq_used_struct_str_field_no_leak() {
    // The canonical USED-and-compared derived-`Eq` shape: a `Doc { content: str }`
    // bound to `a`, compared `a == b` then `a != c` on a branch. The multi-use `a`
    // gets ONE keep-alive `BurdenInc`; each comparison move-alias (`%9 = a`, `%12 =
    // a`) is wrongly classified `dup_alias_dst` (use_counts(a) >= 2) -> a SPURIOUS
    // operand keep-alive `BurdenInc`, even though a `==`/`!=` operand is an RL-1
    // borrow-read (`incElidable`, no duplication). The spurious incs net the
    // a-allocation +1 on every path -> the heap `content` string LEAKS.
    // Spec: Annex E §AIMS RL-1 (`RL1_emit_iff_not_elidable`) + RL-2.
    let src = r#"
#derive(Eq)
type Doc = { content: str }

@main () -> int = {
    let a = Doc { content: "abcdefghijklmnopqrstuvwxyz1234" };
    let b = Doc { content: "abcdefghijklmnopqrstuvwxyz1234" };
    let c = Doc { content: "abcdefghijklmnopqrstuvwxyz9999" };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#;
    assert_burden_path_self_sufficient(src, "derived_eq_used_struct_str_field");
}

#[test]
fn probe_derived_eq_used_struct_list_field_no_leak() {
    // The `[int]`-field derived-`Eq` shape: the aggregate holds an `RcPtr` list
    // buffer; the comparison-operand spurious incs leak the buffer.
    let src = r#"
#derive(Eq)
type Bag = { items: [int] }

@main () -> int = {
    let a = Bag { items: [1, 2, 3, 4, 5] };
    let b = Bag { items: [1, 2, 3, 4, 5] };
    let c = Bag { items: [9, 9, 9] };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#;
    assert_burden_path_self_sufficient(src, "derived_eq_used_struct_list_field");
}

#[test]
fn probe_derived_eq_used_struct_map_field_no_leak() {
    // The `{str: int}`-field derived-`Eq` shape: the aggregate holds an `RcPtr` map
    // buffer (with owned key strings via `elem_dec_fn`); the comparison-operand
    // spurious incs leak the whole map + its key strings.
    let src = r#"
#derive(Eq)
type Env = { vars: {str: int} }

@main () -> int = {
    let a = Env { vars: {"alpha": 1, "beta": 2} };
    let b = Env { vars: {"alpha": 1, "beta": 2} };
    let c = Env { vars: {"gamma": 9} };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#;
    assert_burden_path_self_sufficient(src, "derived_eq_used_struct_map_field");
}

#[test]
fn probe_derived_eq_used_option_str_payload_no_leak() {
    // Sum-payload-with-heap-field derived-`Eq`: an `Option<str>` field compared
    // through `a == b` / `a != c`. The `[InlineEnum]` aggregate's heap payload
    // leaks via the comparison-operand spurious incs.
    let src = r#"
#derive(Eq)
type Holder = { maybe: Option<str> }

@main () -> int = {
    let a = Holder { maybe: Some("abcdefghijklmnopqrstuvwxyz1234") };
    let b = Holder { maybe: Some("abcdefghijklmnopqrstuvwxyz1234") };
    let c = Holder { maybe: None };
    if a == b then {
        if a != c then 0 else 1
    } else 2
}
"#;
    assert_burden_path_self_sufficient(src, "derived_eq_used_option_str_payload");
}

#[test]
fn probe_derived_clone_used_struct_str_field_no_leak() {
    // The f20 sibling (mirrors `fm_clone_struct_str_heap`): a `#derive(Eq, Clone)`
    // struct cloned then compared `a == b`, with the clone result re-read on the
    // then-branch. The compared aggregate flows through the same comparison-operand
    // keep-alive divergence as f13; the str field leaks via the spurious operand
    // inc unless M3+M4 fire.
    let src = r#"
#derive(Eq, Clone)
type Doc = { content: str }

@main () -> int = {
    let a = Doc { content: "abcdefghijklmnopqrstuvwxyz1234" };
    let b = a.clone();
    if a == b then {
        if b.content.length() == 30 then 0 else 1
    } else 2
}
"#;
    assert_burden_path_self_sufficient(src, "derived_clone_used_struct_str_field");
}

#[test]
fn probe_config_projected_fields_compared_keep_negative() {
    // NEGATIVE (the inline-struct projected-field boundary): a `Config { settings,
    // name }` whose fields are PROJECTED + independently read (`.settings.length()`
    // + `.name.length()`). The aggregate fields are released by explicit
    // projection-path decs; the per-(field) alloc-aware net is 0. There are NO
    // `==`/`!=` comparison operands, so the comparison-operand strip MUST NOT fire.
    // Passes pre AND post the cure (must-not-regress + must-not-over-strip).
    let src = r#"
type Config = { settings: {str: int}, name: str }

@main () -> int = {
    let c = Config { settings: {"a": 1}, name: "configname_long_enough_to_heap" };
    if c.settings.length() == 1 then {
        if c.name.length() == 30 then 0 else 1
    } else 2
}
"#;
    assert_burden_path_self_sufficient(src, "config_projected_fields_compared");
}

#[test]
fn probe_derived_eq_single_comparison_keep_negative() {
    // NEGATIVE (single-comparison no-over-strip clamp): a derived-`Eq` struct
    // compared EXACTLY ONCE (`a == b`), no branch re-compare. `a` is used once at
    // the comparison, so it is NOT a multi-use dup_alias source -> no spurious
    // keep-alive inc -> nothing to strip. Passes pre AND post; the cure must not
    // touch the single-comparison shape (would double-free if it stripped the
    // genuine release).
    let src = r#"
#derive(Eq)
type Doc = { content: str }

@main () -> int = {
    let a = Doc { content: "abcdefghijklmnopqrstuvwxyz1234" };
    let b = Doc { content: "abcdefghijklmnopqrstuvwxyz1234" };
    if a == b then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "derived_eq_single_comparison");
}

#[test]
fn probe_heap_str_clone_then_double_compare_freed() {
    // POSITIVE (distinct-root comparison-operand widening): a heap `str` cloned
    // then double-compared (`a == b && a == "literal"`). `clone` of a heap str is
    // an rc-INC of the SAME buffer, but the clone RESULT is a DISTINCT
    // `same_alloc` rep (an Invoke result, not a Let-Var alias), so each `==`
    // compares operands of DISTINCT allocations. Each operand is an RL-1
    // borrow-read (`incElidable`) wrongly given a spurious keep-alive inc that
    // nets the compared allocation +1 -> the buffer (and the fresh `==`-literal)
    // LEAK under flag. The comparison-operand strip (M3 inc + M4 dec), widened to
    // FatValue/RcPointer/Literal(String) operands, frees both. FAILED=leak
    // pre-cure -> PASS. Spec: Annex E §AIMS RL-1 + RL-2.
    let src = r#"
@main () -> int = {
    let a = "this is a heap-allocated string!";
    let b = a.clone();
    if a == b && a == "this is a heap-allocated string!" then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "heap_str_clone_then_double_compare");
}

#[test]
fn probe_heap_str_same_root_multi_compare_no_double_free_negative() {
    // CRITICAL NEGATIVE (same-root multi-compare over-strip clamp): `a == b && b
    // == c` where `b`/`c` are Let-Var aliases of `a` -> ALL operands trace to ONE
    // `same_alloc` rep. The comparison-operand strip's net reasoning holds for
    // DISTINCT-root comparisons (two operand decs release two distinct refs); it
    // BREAKS when the two compared operands alias ONE allocation (the two operand
    // decs release the SAME ref, so an added whole-var dec strip over-releases ->
    // double-free). The widened strip MUST exclude a comparison whose two operands
    // share a `same_alloc` rep. Passes pre AND post the cure (must-not-double-free).
    // RL-2 `RL2_release_exactly_once`: one allocation released exactly once per path.
    let src = r#"
@main () -> int = {
    let a = "this is a heap string for alias chain";
    let b = a;
    let c = b;
    if a == b && b == c then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "heap_str_same_root_multi_compare");
}

#[test]
fn probe_heap_str_same_root_three_compare_no_double_free_negative() {
    // NEGATIVE (already-balanced same-root sibling): three separate `==` results
    // (`r1 = a==b; r2 = b==c; r3 = a==c`) where `b`/`c` alias `a` -> one
    // `same_alloc` rep, three same-root comparisons. The widened strip MUST leave
    // every same-root comparison untouched. Passes pre AND post the cure.
    let src = r#"
@main () -> int = {
    let a = "alias chain comparison string";
    let b = a;
    let c = b;
    let $r1 = a == b;
    let $r2 = b == c;
    let $r3 = a == c;
    if r1 && r2 && r3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "heap_str_same_root_three_compare");
}

#[test]
fn probe_heap_str_single_compare_no_double_free_negative() {
    // NEGATIVE (single distinct-root compare balanced): two independent equal heap
    // strings compared exactly once (`a == b`, distinct allocations, returns 0).
    // Each operand is used once -> no spurious keep-alive inc; the per-operand
    // burden dec nets each allocation to 0. The cure must not over-strip a genuine
    // single release. Passes pre AND post the cure.
    let src = r#"
@main () -> int = {
    let a = "equal heap string for single compare!";
    let b = "equal heap string for single compare!";
    if a == b then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "heap_str_single_compare");
}

#[test]
fn probe_sharing_view_list_slice_then_length_no_uaf() {
    // A seamless-slice producer (`slice`) borrows its receiver and rc-incs the
    // SHARED backing buffer (rc 1->2). The Phase-5 walk placed the receiver's
    // last-use dec INLINE BEFORE the slice Apply -> the dec frees the shared
    // buffer before the slice reads+incs it -> UAF/double-free. The cure relocates the
    // receiver dec to after the borrowed read (its true last use), so the buffer
    // is live when the slice reads it. NON-branchy single-read of the result.
    // Spec: Annex E §AIMS RL-2 + RL-4.
    let src = r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    let ys = xs.slice(start: 2, end: 8);
    if ys.length() == 6 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "sharing_view_list_slice_then_length");
}

#[test]
fn probe_sharing_view_list_slice_branchy_multi_read_no_uaf() {
    // Same seamless-slice receiver-before-Apply UAF, but the slice RESULT is read
    // across MULTIPLE `&&`-short-circuit branches (`ys.length()`, `ys.first()`,
    // `ys.last()`). The receiver dies at the slice site (only the result flows
    // onward); its dec belongs at the borrowed read, not split across edges.
    let src = r#"
@main () -> int = {
    let xs = [10, 20, 30, 40, 50];
    let ys = xs.slice(start: 1, end: 4);
    if ys.length() == 3 && ys.first().unwrap() == 20 && ys.last().unwrap() == 40
        then 0
        else 1
}
"#;
    assert_burden_path_self_sufficient(src, "sharing_view_list_slice_branchy_multi_read");
}

#[test]
fn probe_sharing_view_list_take_dead_receiver_no_uaf() {
    // `take` is a seamless-slice producer sharing the receiver buffer. Dead
    // receiver after the take; result read once.
    let src = r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5, 6, 7, 8];
    let ys = xs.take(count: 3);
    if ys.length() == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "sharing_view_list_take_dead_receiver");
}

#[test]
fn probe_sharing_view_list_drop_dead_receiver_no_uaf() {
    // `drop` is a seamless-slice producer sharing the receiver buffer. Dead
    // receiver after the drop; result read once.
    let src = r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5, 6, 7, 8];
    let ys = xs.drop(count: 2);
    if ys.length() == 6 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "sharing_view_list_drop_dead_receiver");
}

#[test]
fn probe_sharing_view_str_substring_then_transform_no_uaf() {
    // `substring` is a seamless-slice producer sharing the str backing. Dead
    // receiver `s` after the substring; the result `sub` flows into a transform
    // (`to_uppercase`) that reads the shared backing.
    let src = r#"
@main () -> int = {
    let s = "the quick brown fox jumps over the lazy dog";
    let sub = s.substring(start: 4, end: 43);
    let upper = sub.to_uppercase();
    if upper == "QUICK BROWN FOX JUMPS OVER THE LAZY DOG" then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "sharing_view_str_substring_then_transform");
}

#[test]
fn probe_sharing_view_non_sharing_borrowed_read_keep_negative() {
    // NEGATIVE / keep clamp: a plain borrowed scalar read (`@length`) that does
    // NOT produce a buffer-sharing view. The sharing-view relocation must NOT fire here —
    // there is no sharing-view callee, so the receiver's release stays where the
    // burden walk placed it. This passes pre-cure AND post-cure.
    let src = r#"
@main () -> int = {
    let xs = [1, 2, 3, 4, 5];
    if xs.length() == 5 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "sharing_view_non_sharing_borrowed_read");
}

#[test]
fn probe_user_call_fresh_list_result_dup_read_freed() {
    // A user function builds and RETURNS a fresh owned `[int]` (`@to_array`'s
    // `[h, ...to_array(t)]` spread). The caller binds it, dup-reads it
    // (`.len()` + index), then it dies. The fresh-site burden inc on the call
    // result is surplus over the alloc-aware net (net +1) -> the result buffer
    // leaks. The result is genuinely fresh (the callee never returns its arg),
    // so the apply-Direct seed does NOT merge it with the source -> the net is
    // cleanly +1 and one freeing dec at the borrowed-read scope-exit sink frees it.
    // Spec: Annex E §AIMS RL-2.
    let src = r#"
type List = Nil | Cons(head: int, tail: List);

@to_array (list: List) -> [int] = match list {
    Nil -> [],
    Cons(h, t) -> [h, ...to_array(list: t)],
};

@main () -> int = {
    let $list = Cons(head: 1, tail: Cons(head: 2, tail: Cons(head: 3, tail: Nil)));
    let $arr = to_array(list: list);
    if arr.len() == 3 && arr[0] == 1 && arr[2] == 3 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "user_call_fresh_list_result_dup_read");
}

#[test]
fn probe_user_call_fresh_list_result_single_read_freed() {
    // Same mechanism, single borrowed read of the fresh user-call result (the
    // matrix scenario where the result is read once then dead). The fresh-owned
    // collection result still leaks pre-cure (alloc +1, no release).
    // Spec: Annex E §AIMS RL-2.
    let src = r#"
@build (n: int) -> [int] = {
    if n <= 0 then [] else [n, ...build(n: n - 1)]
};

@main () -> int = {
    let xs = build(n: 5);
    if xs.len() == 5 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "user_call_fresh_list_result_single_read");
}

#[test]
fn probe_user_call_fresh_map_result_dup_read_freed() {
    // Type-dimension matrix cell: the fresh user-call result is a `{int: int}`
    // map (FatPointer/RcPtr collection), built and returned by a user function,
    // dup-read then dead. Same alloc-aware-net surplus-inc leak.
    // Spec: Annex E §AIMS RL-2.
    let src = r#"
@pair_map (a: int, b: int) -> {int: int} = {
    let m: {int: int} = {};
    let m2 = m.insert(a, a * 10);
    m2.insert(b, b * 10)
};

@main () -> int = {
    let m = pair_map(a: 1, b: 2);
    if m.len() == 2 && m.get(1).unwrap() == 10 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "user_call_fresh_map_result_dup_read");
}

#[test]
fn probe_user_call_returns_borrowed_slice_view_no_double_free_negative() {
    // NEGATIVE clamp: a user function returns a buffer-SHARING seamless slice of
    // its arg (`@head3(xs) = xs.slice(...)`). The result shares the arg's backing
    // buffer (slice cap), distinct fat-pointer but NOT a fresh allocation. The
    // cure MUST NOT treat this as a fresh-owned result and emit a freeing dec
    // (the shared buffer is freed by the source's release; a result dec double-
    // frees the shared backing). Source + slice result freed exactly once total.
    // Spec: Annex E §AIMS RL-2.
    let src = r#"
@head3 (xs: [int]) -> [int] = xs.slice(start: 0, end: 3);

@main () -> int = {
    let src = [10, 20, 30, 40, 50];
    let ys = head3(xs: src);
    if ys.len() == 3 && ys[0] == 10 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "user_call_returns_borrowed_slice_view");
}

#[test]
fn probe_user_call_fresh_recursive_enum_result_dup_read_freed() {
    // A user function builds and RETURNS a fresh owned RECURSIVE AGGREGATE (a boxed
    // `Cons` chain), NOT a collection. The caller dup-reads it (`len` + `sum`) then
    // it dies. The result is `ValueRepr::Aggregate` (RcStrategy `InlineEnum`),
    // heap-allocated per node. Under sole-emitter lowering the dup-alias burden
    // inc/dec pairs net the explicit ops to 0, leaving the allocation `+1`
    // unreleased -> the whole chain leaks (size-24 x N). The result does NOT
    // same-alloc-merge any arg (genuine builder), so it is a fresh-owned aggregate;
    // one freeing dec at the borrowed-read scope-exit sink lowers to
    // `RcDec [InlineEnum]` walking the chain. Spec: Annex E §AIMS RL-2.
    let src = r#"
type List = Nil | Cons(head: int, tail: List);

@build_list (n: int) -> List = {
    if n <= 0 then Nil
    else Cons(head: n, tail: build_list(n: n - 1))
};

@list_len (list: List) -> int = match list {
    Nil -> 0,
    Cons(_, t) -> 1 + list_len(list: t),
};

@list_sum (list: List) -> int = match list {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(list: t),
};

@main () -> int = {
    let $list = build_list(n: 100);
    let $len_ok = list_len(list: list) == 100;
    let $sum_ok = list_sum(list: list) == 5050;
    if len_ok && sum_ok then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "user_call_fresh_recursive_enum_result_dup_read");
}

#[test]
fn probe_inline_construct_recursive_enum_dup_read_freed() {
    // A fresh recursive aggregate built INLINE via `Construct` (`Cons(.., Cons(..))`),
    // dup-read (`len` + `sum`) then dead. Same shape as the user-call result but the
    // head node is produced by a `Construct`, not a call. The inner nodes are owned
    // args of the parent Construct (consumed into the parent's drop-glue) -> only the
    // outermost head fires. The dup-alias burden inc/dec pairs net 0, leaving the
    // chain's head allocation `+1` unreleased -> leak. One `RcDec [InlineEnum]` at
    // the borrowed-read scope-exit sink frees the whole chain. Spec: Annex E §AIMS
    // RL-2.
    let src = r#"
type List = Nil | Cons(head: int, tail: List);

@list_len (list: List) -> int = match list {
    Nil -> 0,
    Cons(_, t) -> 1 + list_len(list: t),
};

@list_sum (list: List) -> int = match list {
    Nil -> 0,
    Cons(h, t) -> h + list_sum(list: t),
};

@main () -> int = {
    let $list = Cons(head: 1, tail: Cons(head: 2, tail: Cons(head: 3, tail: Nil)));
    let $len1 = list_len(list: list);
    let $sum1 = list_sum(list: list);
    if len1 == 3 && sum1 == 6 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "inline_construct_recursive_enum_dup_read");
}

#[test]
fn probe_recursive_struct_payload_enum_dup_read_freed() {
    // Type-dimension matrix cell: a recursive enum whose variant payload is a
    // STRUCT (`Branch(node: TreeNode)` where TreeNode holds the recursive children).
    // The recursive enum self-allocates per node; dup-read then dead. Same boxed
    // single-release accounting as the flat recursive enum. Spec: Annex E §AIMS RL-2.
    let src = r#"
type Tree = Leaf(value: int) | Branch(left: Tree, right: Tree);

@tree_sum (t: Tree) -> int = match t {
    Leaf(v) -> v,
    Branch(l, r) -> tree_sum(t: l) + tree_sum(t: r),
};

@tree_depth (t: Tree) -> int = match t {
    Leaf(_) -> 1,
    Branch(l, r) -> {
        let $ld = tree_depth(t: l);
        let $rd = tree_depth(t: r);
        1 + (if ld > rd then ld else rd)
    },
};

@main () -> int = {
    let $tree = Branch(
        left: Branch(left: Leaf(value: 1), right: Leaf(value: 2)),
        right: Leaf(value: 3),
    );
    let $sum_ok = tree_sum(t: tree) == 6;
    let $depth_ok = tree_depth(t: tree) == 3;
    if sum_ok && depth_ok then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "recursive_struct_payload_enum_dup_read");
}

#[test]
fn probe_inline_struct_multi_heap_field_projected_no_double_free_negative() {
    // NEGATIVE clamp: a NON-recursive inline struct holding TWO heap fields (a map
    // AND a str), each projected + length-read in `@main`. The struct is inline (no
    // self-buffer); the projection reads free the fields via the existing burden
    // ops. The cure MUST NOT recognise it as a self-allocating aggregate (it is not
    // recursive) and emit a spurious `RcDec [AggFields]` -> double-free of the
    // already-released fields. `is_self_allocating_aggregate` gates on recursion, so
    // an inline non-recursive struct is excluded (field-walk / Phase-6.85 domain).
    // Spec: Annex E §AIMS RL-2.
    let src = r#"
type Config = { settings: {str: int}, name: str }

@main () -> int = {
    let c = Config { settings: {"a": 1}, name: "cfg" };
    if c.settings.length() + c.name.length() == 4 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "inline_struct_multi_heap_field_projected");
}

#[test]
fn probe_user_call_returns_recursive_enum_no_double_free_negative() {
    // NEGATIVE clamp: a user function RETURNS a fresh recursive aggregate, and the
    // CALLER returns it onward (the aggregate is an RL-2 transfer, the outer caller
    // inherits the release). The cure MUST NOT emit a freeing dec on a returned
    // aggregate -> double-free. The `compute_returned_lineages` exclusion must hold
    // for the new aggregate candidate class. Built + read once + returned: freed
    // exactly once by main's consumer. Spec: Annex E §AIMS RL-2.
    let src = r#"
type List = Nil | Cons(head: int, tail: List);

@build_list (n: int) -> List = {
    if n <= 0 then Nil
    else Cons(head: n, tail: build_list(n: n - 1))
};

@list_len (list: List) -> int = match list {
    Nil -> 0,
    Cons(_, t) -> 1 + list_len(list: t),
};

@pass_through (list: List) -> List = list;

@main () -> int = {
    let $list = build_list(n: 4);
    let $forwarded = pass_through(list: list);
    if list_len(list: forwarded) == 4 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "user_call_returns_recursive_enum");
}

#[test]
fn probe_scalar_only_struct_dup_read_no_extra_release_negative() {
    // NEGATIVE clamp: a SCALAR-only struct (`{ x: int, y: int }`) holds no heap
    // field -> `classify_triviality == Trivial`, so `is_burden_carrying_aggregate`
    // is false. The cure MUST NOT recognise it as a fresh-owned aggregate and emit
    // a spurious `RcDec` (it has no heap to free; a dec would be an RC op on non-RC
    // memory). Built + dup-read: no RC at all. Spec: Annex E §AIMS RL-2.
    let src = r#"
type P = { x: int, y: int };

@psum (p: P) -> int = p.x + p.y;

@main () -> int = {
    let $p = P { x: 3, y: 4 };
    let $a = psum(p: p);
    let $b = psum(p: p);
    if a == 7 && b == 7 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "scalar_only_struct_dup_read");
}

#[test]
fn probe_user_call_fresh_str_result_dup_read_freed() {
    // A user function builds and RETURNS a fresh owned `str` (a >23-byte heap
    // string). The caller binds it, dup-reads it across an `&&` short-circuit
    // (`.contains()` then `.length()`), then it dies. Under sole-emitter lowering
    // the multi-use keep-alive `RcInc` on the result is surplus over the
    // alloc-aware net (net +1) -> the result fat-pointer buffer leaks. The result
    // is genuinely fresh (the callee never returns its arg), so the apply-Direct
    // seed does NOT merge it with any source -> the net is cleanly +1 and one
    // freeing dec at the borrowed-read scope-exit sink frees it.
    // Spec: Annex E §AIMS RL-2.
    let src = r#"
@make_label (n: int) -> str = {
    if n > 0 then "this is a long positive label!!" else "this is a long negative label!!"
};

@main () -> int = {
    let s = make_label(n: 5);
    let ok = s.contains(substr: "positive") && s.length() > 3;
    if ok then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "user_call_fresh_str_result_dup_read");
}

#[test]
fn probe_user_call_fresh_str_result_single_read_freed() {
    // Matrix clamp (must-not-regress): same fresh-owned-str user-call result, read
    // ONCE then dead. A single use emits NO keep-alive `RcInc`, so the explicit ops
    // net 0 and the result is already freed pre-cure (the single move-alias dec is
    // the sole release). The cure adds the str result to the candidate set; the
    // alloc-aware net stays 0 at the single sink so NO extra dec fires -> no double
    // free. Passes pre AND post cure. Spec: Annex E §AIMS RL-2.
    let src = r#"
@make_label (n: int) -> str = {
    if n > 0 then "this is a long positive label!!" else "this is a long negative label!!"
};

@main () -> int = {
    let s = make_label(n: 5);
    if s.contains(substr: "positive") then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "user_call_fresh_str_result_single_read");
}

#[test]
fn probe_derive_debug_str_result_dup_read_freed() {
    // Type-dimension matrix cell: the fresh user-call str result is a derived
    // `@debug()` return (a non-builtin method synthesising a fresh quoted `str`).
    // The struct has two heap str fields; the debug result `s` is dup-read across
    // `&&` (two `.contains()` checks) then dead -> the result string leaks under
    // the flag pre-cure. Same alloc-aware-net surplus-inc leak as the bare user fn.
    // Spec: Annex E §AIMS RL-2.
    let src = r#"
#[derive(Debug)]
type Pair = { first: str, second: str }

@main () -> int = {
    let p = Pair { first: "aaaaaaaaaaaaaaaaaaaaaaaaa", second: "bbbbbbbbbbbbbbbbbbbbbbbbb" };
    let s = p.debug();
    let ok = s.contains(substr: "aaaaaaaaaaaaaaaaaaaaaaaaa")
        && s.contains(substr: "bbbbbbbbbbbbbbbbbbbbbbbbb");
    if ok then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "derive_debug_str_result_dup_read");
}

#[test]
fn probe_user_call_returns_str_no_double_free_negative() {
    // NEGATIVE clamp: a user function returns a fresh str, and the CALLER returns
    // it onward (the str is an RL-2 transfer; the outer caller inherits the
    // release). The cure MUST NOT emit a freeing dec on a returned str -> double-
    // free. The `compute_returned_lineages` exclusion must hold for the new
    // str-result candidate. Built + read once + returned: freed exactly once by
    // main's consumer. Spec: Annex E §AIMS RL-2.
    let src = r#"
@make_label (n: int) -> str = {
    if n > 0 then "this is a long positive label!!" else "this is a long negative label!!"
};

@relabel (n: int) -> str = make_label(n: n);

@main () -> int = {
    let s = relabel(n: 5);
    if s.contains(substr: "positive") then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "user_call_returns_str");
}

#[test]
fn probe_str_arg_to_user_call_no_double_free_negative() {
    // NEGATIVE clamp: a fresh str is passed as an OWNED arg to a user function (the
    // callee's concern, an RL-2 transfer at the call site). The cure recognises
    // fresh-owned-str RESULTS, not str ARGS; `compute_user_call_arg_lineages`
    // already excludes a str arg (it considers FatValue args). A freeing dec on the
    // arg lineage would double-free the transferred str. Spec: Annex E §AIMS RL-2.
    let src = r#"
@first_char_is_p (label: str) -> bool = label.contains(substr: "positive");

@main () -> int = {
    let s = "this is a long positive label!!";
    if first_char_is_p(label: s) then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "str_arg_to_user_call");
}

#[test]
fn probe_slice_element_into_struct_field_no_double_free() {
    // POSITIVE: a loop element `p` from `text.split(..)` is a seamless-slice
    // FatVal sharing the `text` backing buffer; it is stored as the owned `s`
    // field of an aggregate `Wrapper`. The aggregate's `RcDec [AggFields]` drop
    // walks `.s` and decs the shared backing once per iteration, but the burden
    // path omits the RL-1 keep-alive inc on the slice field (the slice is a
    // Borrowed Project-view, excluded from `owned_vars_needing_rc`). Without the
    // inc, the backing reaches rc 0 early -> FREE -> a later drop double-frees it.
    // The oracle emits `RcInc <slice>` before the `Construct`, balanced by the
    // aggregate field-drop. Spec: Annex E §AIMS RL-1 + RL-2.
    let src = r#"
#derive(Clone)
type Wrapper = { s: str, n: int }

@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let w = Wrapper { s: p, n: p.len() };
        let copy = w.clone();
        total = total + copy.n;
    };
    if total == 89 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "slice_element_into_struct_field");
}

#[test]
fn probe_slice_element_into_option_field_no_double_free() {
    // POSITIVE (Option-wrapped slice field): the slice element is wrapped in
    // `Some(p)` and stored as the `name: Option<str>` field. The aggregate drop
    // walks the Option payload (the slice) and decs the shared backing; the
    // RL-1 keep-alive on the slice element must balance it. Spec: Annex E §AIMS
    // RL-1 + RL-2.
    let src = r#"
#derive(Clone)
type MaybeNamed = { name: Option<str>, id: int }

@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let w = MaybeNamed { name: Some(p), id: p.len() };
        let copy = w.clone();
        total = total + copy.id;
    };
    if total == 89 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "slice_element_into_option_field");
}

#[test]
fn probe_slice_element_into_tuple_field_no_double_free() {
    // POSITIVE (tuple-wrapped slice field): the slice element is the first
    // element of a `(str, int)` tuple stored as the `data` field. The aggregate
    // drop walks the tuple's str element (the slice) and decs the shared
    // backing; the RL-1 keep-alive must balance it. Spec: Annex E §AIMS RL-1 +
    // RL-2.
    let src = r#"
#derive(Clone)
type Pair = { data: (str, int) }

@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let w = Pair { data: (p, p.len()) };
        let copy = w.clone();
        total = total + copy.data.1;
    };
    if total == 89 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "slice_element_into_tuple_field");
}

#[test]
fn probe_slice_element_into_result_field_no_double_free() {
    // POSITIVE (Result-wrapped slice field): the slice element is wrapped in
    // `Ok(p)` / `Err(p)` and stored as a `Result<str, str>` field. Each Result
    // variant's str payload is the shared slice; the aggregate drop decs it and
    // the RL-1 keep-alive must balance. Spec: Annex E §AIMS RL-1 + RL-2.
    let src = r#"
#derive(Clone)
type Holder = { payload: Result<str, str>, id: int }

@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        let ok_holder = Holder { payload: Ok(p), id: p.len() };
        let ok_copy = ok_holder.clone();
        let err_holder = Holder { payload: Err(p), id: p.len() };
        let err_copy = err_holder.clone();
        total = total + ok_copy.id + err_copy.id;
    };
    if total == 178 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "slice_element_into_result_field");
}

#[test]
fn probe_owned_collection_field_into_struct_negative_no_extra_release() {
    // NEGATIVE clamp (the over-fire boundary): a struct holds an OWNED, freshly
    // built `[int]` field (NOT an iter-element-view slice). The field IS in
    // `owned_vars_needing_rc`, so the base burden path ALREADY emits its
    // Construct-arg RL-1 inc, balanced by the aggregate `RcDec [AggFields]`. The
    // slice-element keep-alive pass MUST NOT fire on this owned field (it is not
    // in `collect_iter_element_defs`) — a spurious second inc would orphan a +1
    // and LEAK the list buffer. Spec: Annex E §AIMS RL-1.
    let src = r#"
type Bag = { items: [int], n: int }

@main () -> int = {
    let total = 0;
    for i in 0..3 do {
        let b = Bag { items: [i, i + 1, i + 2], n: i };
        total = total + b.items.length() + b.n;
    };
    if total == 12 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "owned_collection_field_into_struct_negative");
}

#[test]
fn probe_slice_element_scalar_use_only_negative_no_extra_release() {
    // NEGATIVE clamp (the store-into-aggregate boundary): a loop slice element
    // `p` is used ONLY at scalar-returning borrowed reads (`p.len()`) and is NOT
    // stored into any aggregate field. No aggregate `RcDec [AggFields]` ever
    // walks the slice, so the slice-element keep-alive pass MUST NOT fire — a
    // spurious inc with no matching aggregate field-drop would orphan a +1 and
    // leak the `text` backing. Pins that the keep-alive is gated on the
    // Construct/Reuse field-store position, not on every iter-element-view use.
    // Spec: Annex E §AIMS RL-1.
    let src = r#"
@main () -> int = {
    let text = "this is a long string exceeding SSO by a large amount,and this second part also exceeds it";
    let parts = text.split(sep: ",");
    let total = 0;
    for p in parts do {
        total = total + p.len();
    };
    if total == 89 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "slice_element_scalar_use_only_negative");
}

#[test]
fn probe_str_local_dup_read_borrowed_user_call_freed() {
    // POSITIVE: a fresh local `str` LITERAL borrowed-read TWICE at a USER-function
    // call position (`@get_len(s: str)` — param Borrowed), then dead, never
    // returned. The keep-alive `BurdenInc` covers the two borrowed reads but the
    // base burden walk leaves it balanced by only ONE `BurdenDec` (alloc-aware net
    // +1) — the str buffer leaks under the flag. The borrowed-str-arg lineage was
    // EXCLUDED as a "user-call arg" (the callee's concern), but a Borrowed str
    // SURVIVES the call and the caller still owns it: RL-2 mandates one scope-exit
    // release. Spec: Annex E §AIMS RL-2 (`RL2_release_exactly_once`).
    let src = r#"
@get_len (s: str) -> int = s.length();

@main () -> int = {
    let s = "abcdefghijklmnopqrstuvwxyz1234";
    let len1 = get_len(s: s);
    let len2 = get_len(s: s);
    if len1 + len2 == 60 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "str_local_dup_read_borrowed_user_call");
}

#[test]
fn probe_str_local_dup_read_via_higher_order_freed() {
    // POSITIVE (same mechanism, indirect): a fresh local `str` borrowed-read twice
    // through a higher-order forwarder (`@apply(f, s: str)` — `s` Borrowed). The
    // str flows to two user-call Borrowed positions, dead, not returned. Same
    // alloc-aware net +1 leak as the direct shape. Spec: Annex E §AIMS RL-2.
    let src = r#"
@apply (f: (str) -> int, s: str) -> int = f(s);

@get_len (s: str) -> int = s.length();

@main () -> int = {
    let s = "abcdefghijklmnopqrstuvwxyz1234";
    let a = apply(f: get_len, s: s);
    let b = apply(f: get_len, s: s);
    if a + b == 60 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "str_local_dup_read_via_higher_order");
}

#[test]
fn probe_str_single_read_borrowed_user_call_no_double_free_negative() {
    // NEGATIVE clamp (single-use boundary): a fresh local `str` borrowed-read
    // ONCE at a user-call position, then dead. Single-use nets 0 (the lone
    // borrowed read's release already balances the alloc) — the un-exclusion MUST
    // NOT add a second release (double-free). Pins that the alloc-aware net, not a
    // structural "is borrowed str" proxy, gates the release. Spec: Annex E §AIMS RL-2.
    let src = r#"
@get_len (s: str) -> int = s.length();

@main () -> int = {
    let s = "abcdefghijklmnopqrstuvwxyz1234";
    let len = get_len(s: s);
    if len == 30 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "str_single_read_borrowed_user_call_negative");
}

#[test]
fn probe_str_returned_from_user_call_chain_no_double_free_negative() {
    // NEGATIVE clamp (returned boundary): a fresh local `str` borrowed-read once
    // then RETURNED. The `returned` exclusion keeps the lineage out of the
    // dead-owned release set (the caller inherits the release per RL-2 transfer);
    // un-excluding it from the user-call-arg set MUST NOT make it freed here — a
    // scope-exit dec on a returned str double-frees with the caller's release.
    // Pins the `returned`-exclusion still gates the un-excluded str lineage.
    // Spec: Annex E §AIMS RL-2 (`RL2_transfer_kinds_no_dec`).
    let src = r#"
@get_len (s: str) -> int = s.length();

@pick () -> str = {
    let s = "abcdefghijklmnopqrstuvwxyz1234";
    let n = get_len(s: s);
    if n == 30 then s else "fallback_string_value_padding"
}

@main () -> int = {
    let r = pick();
    if r.length() == 30 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "str_returned_from_user_call_chain_negative");
}

#[test]
fn probe_collection_borrowed_to_user_call_single_read_no_double_free_negative() {
    // NEGATIVE clamp (already-balanced single-read boundary): a fresh local `[int]`
    // borrowed-read ONCE at a borrow-read user-call position (`@sum_two` reads
    // `xs[0]`/`xs[1]`, never COW-mutates), dead, not returned. Single-use nets 0
    // (the lone borrowed read's release already balances the alloc). The
    // alloc-aware net — NOT a structural "is borrowed list" proxy — gates the
    // un-exclusion, so the release does NOT fire here and MUST NOT add a second
    // dec (double-free). Pins the net, not membership, decides. Spec: Annex E
    // §AIMS RL-2.
    let src = r#"
@sum_two (xs: [int]) -> int = xs[0] + xs[1];

@main () -> int = {
    let xs = [10, 20, 30];
    let a = sum_two(xs: xs);
    if a == 30 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(
        src,
        "collection_borrowed_to_user_call_single_read_negative",
    );
}

#[test]
fn probe_list_borrowed_to_user_call_dup_read_freed() {
    // POSITIVE: a fresh local `[int]` borrowed-read TWICE at a borrow-read
    // user-call position (`@sum_two` reads `xs[0]`/`xs[1]` — param Borrowed, never
    // COW-mutated, never iter-consumed), then dead, never returned. The keep-alive
    // covers the two borrowed reads but the base burden walk leaves the lineage
    // balanced by only ONE release (alloc-aware net +1) — the list buffer leaks
    // under the flag. The borrowed-list-arg lineage was EXCLUDED as a "user-call
    // arg", but a borrow-read-only Borrowed list SURVIVES the call and the caller
    // still owns it: RL-2 mandates one scope-exit release, exactly as for a
    // borrowed str. The un-exclusion is gated on the callee's per-param
    // `borrowed_read_only` contract fact (the param flows only to borrowed
    // positions — no owned/COW consumer), so a COW-mutating callee stays excluded.
    // Spec: Annex E §AIMS RL-2 (`RL2_borrowed_param_emits_caller_dec` +
    // `RL2_release_exactly_once`).
    let src = r#"
@sum_two (xs: [int]) -> int = xs[0] + xs[1];

@main () -> int = {
    let xs = [10, 20, 30];
    let a = sum_two(xs: xs);
    let b = sum_two(xs: xs);
    if a == 30 && b == 30 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_borrowed_to_user_call_dup_read");
}

#[test]
fn probe_list_borrowed_recursive_borrow_read_dup_read_freed() {
    // POSITIVE (recursive borrow-read forwarder): a fresh local `[int]` passed to a
    // recursive borrow-read callee (`@sum_recursive(xs, idx)` reads `xs.length()` +
    // `xs[idx]` and forwards `xs` to a recursive call where it is ALSO borrow-read).
    // The param flows only to borrowed positions across the recursion — the
    // `borrowed_read_only` contract fact stays true through SCC-propagated
    // forwarding. Mirrors the `fat_matrix::f16_recursion::test_fm_recursion_list_param`
    // corpus shape. Spec: Annex E §AIMS RL-2.
    let src = r#"
@sum_recursive (xs: [int], idx: int) -> int = {
    if idx >= xs.length() then 0
    else xs[idx] + sum_recursive(xs: xs, idx: idx + 1)
}

@main () -> int = {
    if sum_recursive(xs: [10, 20, 30], idx: 0) == 60 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "list_borrowed_recursive_borrow_read_dup_read");
}

#[test]
fn probe_chained_curried_closure_str_capture_freed() {
    // POSITIVE (chained-curried closure last-use dec): `fst("hello")(0)` invokes the
    // outer closure (a PartialApply) producing an inner closure, then invokes the
    // inner. Both closure VALUES are ANONYMOUS chained intermediates (not bound to a
    // `let`), so the base burden walk emits NO scope-exit `BurdenDec` for them — the
    // closure envs leak under the flag. The oracle decs each closure once at its
    // last (invoking) read. RL-2: an owned closure value at its non-transfer last use
    // releases (its env-dec); invoking a closure is a `.LastReadBeforeScopeExit`, not
    // a transfer. Spec: Annex E §AIMS RL-2.
    let src = r#"
@main () -> int = {
    let $fst = a -> b -> a;
    let $s = fst("hello")(0);
    if s == "hello" then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "chained_curried_closure_str_capture");
}

#[test]
fn probe_chained_curried_closure_list_capture_freed() {
    // POSITIVE (heap-list capture in chained-curried closure): same anonymous-chained
    // closure-intermediate leak with a `[int]` capture instead of `str`. The closure
    // env carries the captured list's RC; the missing closure-value last-use dec
    // leaks the env (and the captured list reachable through it). Spec: Annex E §AIMS
    // RL-2.
    let src = r#"
@main () -> int = {
    let $fst = a -> b -> a;
    let $xs = fst([1, 2, 3])(0);
    if xs == [1, 2, 3] then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "chained_curried_closure_list_capture");
}

#[test]
fn probe_call_returned_closure_invoked_freed() {
    // POSITIVE (closure RETURNED from a user fn, anonymous-invoked): `make_adder(5)`
    // returns an owned closure; the call result is invoked once as an anonymous
    // intermediate (`make_adder(5)(10)` style via a `let`-result that is invoked, not
    // re-bound). The closure-returning `Apply` result is a fresh closure allocation
    // the base burden walk does not scope-exit-dec — the env leaks. Distinct from the
    // PartialApply-result shape: this is an `Apply`-result closure. Spec: Annex E
    // §AIMS RL-2.
    let src = r#"
@make_adder (n: int) -> (int) -> int = {
    x -> x + n
}

@main () -> int = {
    let $r = make_adder(n: 5)(10);
    if r == 15 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "call_returned_closure_invoked");
}

#[test]
fn probe_closure_transferred_into_struct_no_double_free_negative() {
    // NEGATIVE (the over-fire boundary — transferred closure): a closure stored as a
    // struct field is TRANSFERRED (the `Construct Struct` owned arg) — the struct's
    // scope-exit `RcDec [AggFields]` walks the closure field and frees its env. The
    // closure-value scope-exit dec MUST NOT also fire here, or the env double-frees.
    // The transferred-out gate (Construct owned-arg = transfer) excludes it. PASS
    // pre AND post cure.
    let src = r#"
type Holder = { f: (int) -> int }

@main () -> int = {
    let n = 5;
    let $h = Holder { f: x -> x + n };
    let $r = (h.f)(10);
    if r == 15 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "closure_transferred_into_struct_no_double_free");
}

#[test]
fn probe_let_bound_closure_single_invoke_no_double_free_negative() {
    // NEGATIVE (the already-balanced boundary — let-bound closure): a closure bound to
    // a `let` and invoked once ALREADY receives its scope-exit `BurdenDec` from the
    // base burden walk (the let-bound lineage carries a dec). The closure-value
    // scope-exit pass MUST NOT add a SECOND dec on a lineage that already has one, or
    // the env double-frees. The existing-dec gate (skip lineages already carrying a
    // BurdenDec) excludes it. PASS pre AND post cure.
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
    assert_burden_path_self_sufficient(src, "let_bound_closure_single_invoke_no_double_free");
}

#[test]
fn probe_fresh_heap_str_dead_on_question_early_exit_freed() {
    // POSITIVE (fresh heap value dead on an early-`?`-return branch): `name` is a
    // fresh heap str defined before `opt?`; on the `?`-None branch the function
    // early-returns the None variant and `name` is dead WITHOUT its release (the
    // base burden walk emits `name`'s single release only on the value-survives
    // branch). RL-4 edge-cleanup: `name` is owned non-scalar, live at the branch
    // block's exit, dead at the early-return successor's entry, not a Jump arg ->
    // one edge `BurdenDec` on that successor. Spec: Annex E §AIMS RL-4.
    let src = r#"
@process (opt: Option<int>) -> Option<int> = {
    let name = "abcdefghijklmnopqrstuvwxyz1234";
    let v = opt?;
    Some(v + name.length())
}

@main () -> int = {
    match process(opt: None) {
        Some(_) -> 1,
        None -> 0,
    }
}
"#;
    assert_burden_path_self_sufficient(src, "fresh_heap_str_dead_on_question_early_exit");
}

#[test]
fn probe_fresh_heap_str_dead_on_explicit_branch_freed() {
    // POSITIVE (fresh heap value dead on one explicit `if` branch): `tag` is a
    // fresh heap str used only inside the `then` branch (`tag.length()`); the
    // `else` branch leaves `tag` dead WITHOUT a release on that edge. Same RL-4
    // edge-cleanup as the `?`-exit shape, via a plain `if/else` split rather than
    // `?`-desugar. Spec: Annex E §AIMS RL-4.
    let src = r#"
@pick (b: bool) -> int = {
    let tag = "a heap string well past the SSO inline threshold of 23!";
    if b then tag.length() else 7
}

@main () -> int = {
    let r = pick(b: false);
    if r == 7 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "fresh_heap_str_dead_on_explicit_branch");
}

#[test]
fn probe_fresh_heap_str_returned_on_early_exit_no_double_free_negative() {
    // NEGATIVE (the over-fire boundary — fresh value TRANSFERRED on the dead-looking
    // branch): `name` is RETURNED on the `b`-true branch (an RL-2 ownership transfer
    // — the caller releases it). The branch-dead-value edge-dec MUST NOT fire on
    // that branch, or `name` double-frees against the caller's release. The
    // transferred-out (Return / Construct-arg) guard excludes it. PASS pre AND post
    // cure. Spec: Annex E §AIMS RL-2 + RL-4.
    let src = r#"
@choose (b: bool) -> str = {
    let name = "a heap string well past the SSO inline threshold of 23!";
    if b then name else "other"
}

@main () -> int = {
    let s = choose(b: true);
    if s.length() == 55 then 0 else 1
}
"#;
    assert_burden_path_self_sufficient(src, "fresh_heap_str_returned_on_early_exit_no_double_free");
}
