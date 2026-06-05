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
