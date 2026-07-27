//! Memory-lifecycle probe for the compiled-counter adapter.
//!
//! Compiles real Ori programs and runs each under `ORI_CHECK_LEAKS=1`. The
//! class-ledger path is the sole logical ownership-event placement authority
//! and its `BurdenInc → RcInc` / `BurdenDec → RcDec` lowering is unconditional.
//! A pass proves only that this adapter produces a VF-1-balanced, leak-free,
//! double-free-free binary for the covered shape; it is not an AIMS-wide or
//! cross-executor verdict.
//!
//! Matrix dimensions (burden-lowering completeness shapes): move-alias chain,
//! duplication-alias with live source, collection-buffer last-use
//! (list / map / set), borrow-chain (project of a projection), closure-capture
//! last-use.
//!
//! Build-step env (compile-time flag) via `compile_and_run_with_build_env`;
//! run-step `ORI_CHECK_LEAKS=1` always-on. Subprocess-isolated — parallel-safe.

use crate::util::compile_and_run_with_build_env;

/// Compile and run `source` under leak checking. Asserts the program exits 0
/// with no FATAL double-free or leak diagnostic on stderr.
fn assert_runs_clean_no_leak_or_double_free(source: &str, label: &str) {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(source, &[]);
    assert!(
        exit == 0,
        "[{label}] run exited {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    // ORI_CHECK_LEAKS=1 emits a leak report on stderr; the RC double-free guard
    // emits `FATAL — ori_rc_dec called on already-freed`. Either is a failure.
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[{label}] run reported a leak / double-free\nstderr:\n{stderr}"
    );
}

#[test]
fn probe_move_alias_chain_str() {
    // Move-alias chain: a heap str moved through Let-Var hops (FatVal lineage
    // %0 → %2 → %4) then returned — burden RL-2 move-alias transfer suppression
    // must keep the net at 0 with no orphan dec.
    let src = include_str!("fixtures/memory_lifecycle_probe/probe_move_alias_chain_str.ori");
    assert_runs_clean_no_leak_or_double_free(src, "move_alias_chain_str");
}

#[test]
fn probe_dup_alias_live_source_str() {
    // Duplication: a Let-Var alias whose SOURCE stays live afterward — RL-1
    // duplication inc on the alias, balanced by its own last-use dec.
    let src = include_str!("fixtures/memory_lifecycle_probe/probe_dup_alias_live_source_str.ori");
    assert_runs_clean_no_leak_or_double_free(src, "dup_alias_live_source_str");
}

// Memory-lifecycle coverage for collection types: the AOT + JIT compile
// paths reconstruct the `TypeRegistry` from the `TypedModule` exports and
// thread it into `run_arc_pipeline`, so the burden walker's
// `type_registry.burden(idx)` lookup for `[T]` / `{K:V}` / `Set<T>` resolves
// the composed `UserBurdenSpec`; collection buffers receive `BurdenInc` /
// `BurdenDec`. Closure capture resolves through the same lookup.
#[test]
fn probe_collection_buffer_last_use_list() {
    // Collection-buffer last-use: a heap list built, consumed, dropped — the
    // burden CollectionBuffer dec at last use must release the buffer exactly
    // once.
    let src =
        include_str!("fixtures/memory_lifecycle_probe/probe_collection_buffer_last_use_list.ori");
    assert_runs_clean_no_leak_or_double_free(src, "collection_buffer_last_use_list");
}

#[test]
fn probe_collection_buffer_last_use_map() {
    let src =
        include_str!("fixtures/memory_lifecycle_probe/probe_collection_buffer_last_use_map.ori");
    assert_runs_clean_no_leak_or_double_free(src, "collection_buffer_last_use_map");
}

#[test]
fn probe_collection_buffer_last_use_set() {
    let src =
        include_str!("fixtures/memory_lifecycle_probe/probe_collection_buffer_last_use_set.ori");
    assert_runs_clean_no_leak_or_double_free(src, "collection_buffer_last_use_set");
}

#[test]
fn probe_borrow_chain_project_of_projection() {
    // Borrow-chain: a Project of a projection (nested field borrow). TF-4
    // Borrowed propagation must keep the nested borrow-view from emitting a
    // last-use dec (a borrow owns no allocation).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_borrow_chain_project_of_projection.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "borrow_chain_project_of_projection");
}

#[test]
fn probe_closure_capture_last_use_str() {
    // Closure-capture last-use: a heap str captured by a closure; the closure's
    // env carries the capture's RC, released when the closure dies. PartialApply
    // FRESH + last-use dec must net 0.
    let src =
        include_str!("fixtures/memory_lifecycle_probe/probe_closure_capture_last_use_str.ori");
    assert_runs_clean_no_leak_or_double_free(src, "closure_capture_last_use_str");
}

/// With `ORI_DISABLE_BURDEN_OPS=1`, class-ledger Step-4b emission is disabled.
/// Because the class ledger is the sole emitter, realization must fail loud
/// instead of synthesizing a fallback or producing an under-released
/// executable. Spec: Annex E §AIMS RL-2.
#[test]
fn probe_closure_capture_last_use_str_burden_ops_disabled_fails_loud() {
    use crate::util::compile_and_run_with_build_env;
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_closure_capture_last_use_str_burden_ops_disabled_fails_loud.ori"
    );
    let probe: &[(&str, &str)] = &[("ORI_VERIFY_ARC", "1"), ("ORI_VERIFY_EACH", "1")];

    // Control: probe, Phase-5 emission intact.
    let (control_exit, _stdout, control_stderr) = compile_and_run_with_build_env(src, probe);
    assert_eq!(
        control_exit, 0,
        "probe with Phase-5 burden-op emission intact must run \
         clean (no leak, no double-free)\nstderr:\n{control_stderr}"
    );

    // Forced: burden-op emission disabled — sole-emitter realization must
    // reject the unreplaced function before codegen.
    let mut forced: Vec<(&str, &str)> = probe.to_vec();
    forced.push(("ORI_DISABLE_BURDEN_OPS", "1"));
    let (forced_exit, _stdout, forced_stderr) = compile_and_run_with_build_env(src, &forced);
    assert_eq!(
        forced_exit, -1,
        "disabling the sole class-ledger emitter must fail during compilation\n\
         stderr:\n{forced_stderr}"
    );
    assert!(
        forced_stderr.contains("realize reached a non-class-ledger function")
            && forced_stderr.contains("class-ledger plan admits only replaced functions"),
        "the forced leg must reach the intentional sole-emitter fail-loud gate, \
         not fail for an unrelated reason\nstderr:\n{forced_stderr}"
    );
}

#[test]
fn probe_result_str_partial_move_via_try_codegen_clean() {
    // `?` on a `Result<int, str>` projects the heap Err payload into the
    // propagated value. The
    // burden walk records that move and emits `burden_dec_partial %r skip=[1]`
    // for the Result var — a DropKind::Enum partial-move drop shape. That op
    // lowers to a real per-variant `RcDec` walk. `DropKind::Enum` must dispatch through
    // `emit_variant_burden_walk` and skip the moved-out source variant by
    // ordinal (RL-2); SSO payloads keep the assertion allocation-independent.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_result_str_partial_move_via_try_codegen_clean.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "result_str_partial_move_via_try");
}

#[test]
fn probe_result_scalar_only_no_partial_move_codegen_clean() {
    // Negative companion (the moved_fields cluster's scalar-projection filter):
    // `??` on a `Result<int, str>` where the Ok payload is a SCALAR int and the
    // Err is taken projects only the scalar int slot. A scalar projection
    // transfers NO RC ownership (L-9 / TF-4), so it must NOT seed `skip_fields`
    // — the surviving Err payload owes a FULL `burden_dec`, never a partial-skip
    // that strands it. Pre-filter the scalar projection wrongly marked field 1
    // moved, suppressing the Err drop; the filter keeps the full dec. SSO Err
    // string keeps this pin heap-free (codegen-clean clamp; the separately
    // tracked heap-payload discard leak is out of scope for this pin).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_result_scalar_only_no_partial_move_codegen_clean.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "result_scalar_only_no_partial_move");
}

#[test]
fn probe_coalesce_discards_heap_err_payload() {
    // RL-4 / RL-5: `a ?? default` on a `Result<int, str>` whose `a` is the
    // heap-Err variant discards the Result on the Err-taken edge. The Result's
    // heap str payload is live at the coalesce branch but dead in the default
    // successor — its release belongs on that dying CFG edge. The class-ledger
    // per-edge placement emits the dying-edge `BurdenDec`, lowered to the real
    // `RcDec` under the probe. The Err str is >23 bytes (defeats SSO) so the
    // leak is observable. Fails before the class-ledger edge-release fix:
    // `ORI_CHECK_LEAKS=1` reports `1 RC allocation not freed`.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_coalesce_discards_heap_err_payload.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "coalesce_discards_heap_err_payload");
}

#[test]
fn probe_default_path_unaffected_str() {
    // Symmetry pin: the SAME program runs leak-free — guards that the
    // per-shape probes are not vacuous (separate per-shape probes pin that the burden
    // path actually fires).
    let src = include_str!("fixtures/memory_lifecycle_probe/probe_default_path_unaffected_str.ori");
    let (exit, _stdout, stderr) = compile_and_run_with_build_env(src, &[]);
    assert!(
        exit == 0,
        "default-path run exited {exit}\nstderr:\n{stderr}"
    );
}

// A populated `TypeRegistry` must not perturb default-path emission while burden
// operations are disabled; every resolvable collection and closure shape stays
// leak-free.

/// Compile `source` and run under leak checking.
/// Asserts exit 0 with no leak / double-free — pins that the reconstructed
/// populated registry leaves default-path emission unaffected for the covered
/// residual-risk collection / closure / non-collection-heap shape.
fn assert_default_path_leak_free(source: &str, label: &str) {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(source, &[]);
    assert!(
        exit == 0,
        "[{label}] default-path run exited {exit}\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stderr.contains("FATAL")
            && !stderr.contains("already-freed")
            && !stderr.to_lowercase().contains("leak"),
        "[{label}] default-path run reported a leak / double-free\nstderr:\n{stderr}"
    );
}

#[test]
fn probe_default_path_unaffected_list_int() {
    // `[int]` collection buffer — burden lookup resolves the CollectionBuffer
    // spec; default-path emission must stay leak-free.
    let src =
        include_str!("fixtures/memory_lifecycle_probe/probe_default_path_unaffected_list_int.ori");
    assert_default_path_leak_free(src, "default_path_unaffected_list_int");
}

#[test]
fn probe_default_path_unaffected_map_str_int() {
    // `{str: int}` collection buffer — burden lookup resolves the map spec
    // (heap-str keys + scalar values); default-path emission must stay leak-free.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_default_path_unaffected_map_str_int.ori"
    );
    assert_default_path_leak_free(src, "default_path_unaffected_map_str_int");
}

#[test]
fn probe_default_path_unaffected_set_int() {
    // `Set<int>` collection buffer — burden lookup resolves the set spec;
    // default-path emission must stay leak-free.
    let src =
        include_str!("fixtures/memory_lifecycle_probe/probe_default_path_unaffected_set_int.ori");
    assert_default_path_leak_free(src, "default_path_unaffected_set_int");
}

#[test]
fn probe_default_path_unaffected_closure_env() {
    // Closure-env captured heap str — burden lookup resolves the capture spec via
    // the same registry path; default-path emission must stay leak-free.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_default_path_unaffected_closure_env.ori"
    );
    assert_default_path_leak_free(src, "default_path_unaffected_closure_env");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_map_keys_str_source_freed_with_elements.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "map_keys_str_source_freed_with_elements");
}

#[test]
fn probe_set_to_list_str_source_freed_no_double_free() {
    // `set.to_list()`: the set source is borrowed, the list iterated. The dead
    // set at the loop exit must be freed exactly once (a second dec aborts) — its
    // element strings are slice/heap-aware via `elem_dec_fn`.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_to_list_str_source_freed_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "set_to_list_str_source_freed_no_double_free");
}

#[test]
fn probe_str_split_source_freed() {
    // `s.split()`: the str source is borrowed, the parts (slice-views into `s`)
    // iterated. The dead source string at the loop exit must be freed (slice
    // provenance handled by `ori_rc_dec` on the FatPointer data).
    let src = include_str!("fixtures/memory_lifecycle_probe/probe_str_split_source_freed.ori");
    assert_runs_clean_no_leak_or_double_free(src, "str_split_source_freed");
}

#[test]
fn probe_map_keys_str_loop_managed_not_double_freed() {
    // Negative pin (the for-loop-cluster guard): the keys RESULT is iterator-
    // managed (freed by `ori_iter_drop`); the dead-collection-source pass must
    // free ONLY the borrowed map source, never the iter-consumed keys list — a
    // dec there would double-free. Covered by the positive pin's leak-free exit,
    // but pinned separately for the double-free shape (a second dec aborts).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_map_keys_str_loop_managed_not_double_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "map_keys_str_loop_managed_not_double_freed");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_sort_result_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "list_sort_result_freed_at_scope_exit");
}

#[test]
fn probe_list_set_result_freed_at_scope_exit() {
    // `xs.set(i, v)` mutation-result owned-collection dead at scope exit.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_set_result_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "list_set_result_freed_at_scope_exit");
}

#[test]
fn probe_list_insert_result_freed_at_scope_exit() {
    // `xs.insert(i, v)` mutation-result owned-collection dead at scope exit.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_insert_result_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "list_insert_result_freed_at_scope_exit");
}

#[test]
fn probe_list_remove_result_freed_at_scope_exit() {
    // `xs.remove(i)` mutation-result owned-collection dead at scope exit.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_remove_result_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "list_remove_result_freed_at_scope_exit");
}

#[test]
fn probe_map_read_only_owned_source_freed_at_scope_exit() {
    // Read-only `m.contains_key(..)`: the owned map is NEVER mutated, borrowed at
    // every use, dead at scope exit — the simplest whole-buffer leak shape (alloc
    // `+1` unreleased). Int keys keep this a pure buffer-freeing case (the
    // heap-str-element-arg layer is the separate residual leaf).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_map_read_only_owned_source_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "map_read_only_owned_source_freed_at_scope_exit");
}

#[test]
fn probe_map_int_index_result_freed_at_scope_exit() {
    // `m[k]` on an int-keyed int-value map: the owned map is borrowed by the index
    // read, dead at scope exit — the whole-buffer leak shape.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_map_int_index_result_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "map_int_index_result_freed_at_scope_exit");
}

#[test]
fn probe_list_int_sort_negative_no_extra_release() {
    // Negative pin: a sort result that IS subsequently returned (ownership
    // transfer, RL-2 transfer kind) must NOT receive a scope-exit release — the
    // caller inherits the obligation. A double-release here aborts.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_int_sort_negative_no_extra_release.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "list_int_sort_negative_no_extra_release");
}

// CHAINED COW-mutation results: `xs.push(a).push(b)` / `xs.concat(..).reverse()`
// build a single fresh-local allocation transformed in place by each COW op. The
// receiver of the SECOND mutation is itself a mutation RESULT (not a direct
// `Construct`), so the dead-owned-collection candidate set must admit the chain
// tail or leak the buffer. The fresh-local-equivalence transitive closure over a
// COW-mutator chain rooted at a fresh local Construct makes the chain tail
// freeable at its borrowed-read scope-exit sink (RL-2 ApplyToBorrowedParam).

#[test]
fn probe_list_push_chain_result_freed_at_scope_exit() {
    // `[1].push(2).push(3)`: the second push's receiver is the first push RESULT,
    // not a Construct. The chain tail is borrowed-read by `@length`/`@first`/`@last`,
    // dead at scope exit. Without the transitive closure the realloc'd buffer leaks.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_push_chain_result_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "list_push_chain_result_freed_at_scope_exit");
}

#[test]
fn probe_list_concat_reverse_chain_result_freed_at_scope_exit() {
    // `([1,2] + [3]).reverse()`: reverse's receiver is the concat result. The
    // chain tail is borrowed-read at scope exit.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_concat_reverse_chain_result_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "list_concat_reverse_chain_result_freed_at_scope_exit",
    );
}

#[test]
fn probe_list_reverse_reverse_chain_result_freed_at_scope_exit() {
    // `xs.reverse().reverse()`: a two-COW chain whose tail is borrowed-read, dead
    // at scope exit.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_reverse_reverse_chain_result_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "list_reverse_reverse_chain_result_freed_at_scope_exit",
    );
}

#[test]
fn probe_list_push_chain_negative_returned_no_extra_release() {
    // Negative pin: a push-chain result that IS returned (ownership transfer, RL-2
    // transfer kind) must NOT receive a scope-exit release — the caller inherits
    // the obligation. A double-release here aborts.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_push_chain_negative_returned_no_extra_release.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "list_push_chain_negative_returned_no_extra_release",
    );
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_union_result_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "set_union_result_freed_at_scope_exit");
}

#[test]
fn probe_set_difference_result_freed_at_scope_exit() {
    // `a.difference(b)` fresh owned `{int}` result, borrowed-read then dead.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_difference_result_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "set_difference_result_freed_at_scope_exit");
}

#[test]
fn probe_set_intersection_result_freed_at_scope_exit() {
    // `a.intersection(b)` fresh owned `{int}` result, borrowed-read then dead.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_intersection_result_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "set_intersection_result_freed_at_scope_exit");
}

#[test]
fn probe_set_union_result_returned_negative_no_extra_release() {
    // Negative pin: a set-algebra result that IS returned (ownership transfer,
    // RL-2 transfer kind) must NOT receive a scope-exit release — the caller
    // inherits the obligation. The `returned` exclusion must hold; a
    // double-release here aborts.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_union_result_returned_negative_no_extra_release.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "set_union_result_returned_negative_no_extra_release",
    );
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_map_insert_heap_str_key_local_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "map_insert_heap_str_key_local_freed");
}

#[test]
fn probe_map_insert_heap_str_value_local_freed() {
    // The inserted VALUE str is copied into the map (val_inc); the local leaks.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_map_insert_heap_str_value_local_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "map_insert_heap_str_value_local_freed");
}

#[test]
fn probe_map_remove_str_key_lookup_local_freed() {
    // The `remove` lookup KEY str is borrowed for the search (never stored); the
    // local is the only reference, dead after the borrowed call — it leaks without
    // a last-use dec.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_map_remove_str_key_lookup_local_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "map_remove_str_key_lookup_local_freed");
}

#[test]
fn probe_map_construct_heap_str_keys_negative_no_double_free() {
    // Negative pin: str keys MOVED into a `Construct Map` literal (an OWNED position)
    // are the map's only reference — the map's `elem_dec_fn` frees them. A per-element
    // local dec here double-frees. The map is read-only (`contains_key`), dead at
    // scope exit; the buffer-freeing pass frees the buffer + elements via the V5
    // walk, and the per-element pass MUST NOT also free the moved keys.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_map_construct_heap_str_keys_negative_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "map_construct_heap_str_keys_negative_no_double_free",
    );
}

#[test]
fn probe_set_to_list_conversion_result_freed() {
    // A collection-CONVERSION result (`set.to_list()` / `m.keys()` / `m.values()`)
    // is a FRESH owned collection the runtime allocates from the receiver; bound to
    // a local, borrowed-read by `@length`, dead at scope exit — it leaks the result
    // buffer under sole-emitter lowering without a scope-exit release dec.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_to_list_conversion_result_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "set_to_list_conversion_result_freed");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_map_values_heap_str_source_borrowed_no_loop.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "map_values_heap_str_source_borrowed_no_loop");
}

#[test]
fn probe_set_str_union_owned_consumed() {
    // `a.union(b)`: a set union consumes its operand sets; the heap-str elements'
    // ownership is handled by the union/result lineage. A caller-side freeing dec
    // on a union operand double-frees against the union's own consume. Pins that
    // the single-borrow conversion-source relocation does not over-fire on a
    // union-operand shape (the receiver is owned-consumed, not a borrowed
    // conversion source, so the relocation leaves it untouched).
    let src =
        include_str!("fixtures/memory_lifecycle_probe/probe_set_str_union_owned_consumed.ori");
    assert_runs_clean_no_leak_or_double_free(src, "set_str_union_owned_consumed");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_map_str_passed_to_iter_consuming_fn_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "map_str_passed_to_iter_consuming_fn");
}

#[test]
fn probe_set_str_passed_to_iter_consuming_fn_no_double_free() {
    // `count_items(s)` iter-consumes the set via `for x in s`. Same inward-transfer
    // as the map case; the caller dec is suppressed.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_str_passed_to_iter_consuming_fn_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "set_str_passed_to_iter_consuming_fn");
}

// Set iteration decrements only owned receivers; borrowed generic parameters
// remain caller-owned. The matrix spans scalar/heap elements, named/automatic
// iteration, and borrowed/owned receivers.

/// A generic borrowed `Set<int>` parameter survives named `.iter()`.
#[test]
fn probe_generic_set_int_param_named_iter_count_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_generic_set_int_param_named_iter_count_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "generic_set_int_param_named_iter_count");
}

/// Regression: generic by-value `Set<str>` param (heap elements) + named
/// `.iter()`. Heap-elem variant: `elem_inc_fn`/`elem_dec_fn` element
/// refcounts must net zero while the source set buffer is freed exactly once.
#[test]
fn probe_generic_set_str_param_named_iter_count_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_generic_set_str_param_named_iter_count_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "generic_set_str_param_named_iter_count");
}

/// Regression: generic by-value `Set<int>` param + named `.iter()` feeding a
/// DISTINCT iterator consumer (`.fold`, not `.count`). The cure gates
/// the dec at the iter SOURCE (`emit_set_iter`), so it is consumer-agnostic; this
/// cell pins that — a fix special-cased to the `.count` codepath would leave this
/// genuine `.fold` consumer double-freeing the borrowed set buffer. (Generic `T`
/// forbids summing `x: T`, so the fold counts one per element; the deliverable is
/// the distinct fold consumer, not the accumulated value.)
#[test]
fn probe_generic_set_int_param_named_iter_fold_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_generic_set_int_param_named_iter_fold_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "generic_set_int_param_named_iter_fold");
}

/// Automatic iteration balances its owned `Set<int>` receiver before the
/// set-buffer decrement (`receiver_owned = true`).
#[test]
fn probe_generic_set_int_param_auto_iter_balanced() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_generic_set_int_param_auto_iter_balanced.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "generic_set_int_param_auto_iter");
}

/// Named iteration frees a non-generic owned `Set<int>` exactly once.
#[test]
fn probe_owned_set_int_local_named_iter_count_freed_once() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_owned_set_int_local_named_iter_count_freed_once.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "owned_set_int_local_named_iter_count");
}

/// Named iteration frees an owned `Set<str>` buffer and every heap element once.
#[test]
fn probe_owned_set_str_local_named_iter_count_freed_once() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_owned_set_str_local_named_iter_count_freed_once.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "owned_set_str_local_named_iter_count");
}

/// An owned `Set<int>` returned after named iteration retains one caller reference.
/// The receiver is `[own]` (transfers through Return),
/// so AIMS freezes an additional owner credit across the iterator; the
/// counter adapter realizes that credit as a keep-alive inc (RC 2). The cure's
/// gate fires the set-buffer dec, leaving RC 1 for the caller's surviving ref —
/// no leak, no double-free. Covers the owned-Set-param iterator-consumption and
/// escaping-return overlap, governed by
/// `RL2_iter_consume_return_overlap_balanced`. If the ownership gate
/// skipped the dec on this owned-surviving path, the set buffer would LEAK (the
/// `ORI_CHECK_LEAKS=1` harness detects that).
#[test]
fn probe_owned_set_int_returned_after_iter_freed_once() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_owned_set_int_returned_after_iter_freed_once.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "owned_set_int_returned_after_iter");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_iter_consume_call_inside_catch_then_normal_call_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "iter_consume_call_inside_catch_then_normal_call",
    );
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_borrow_read_fold_call_keeps_caller_dec_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "borrow_read_fold_call_keeps_caller_dec");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_inline_for_loop_str_list_two_call_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "inline_for_loop_str_list_two_call");
}

#[test]
fn probe_inline_for_loop_map_two_call_no_double_free() {
    // `{str: int}` source iter-consumed by TWO inline `for entry in m do` loops —
    // the keep-alive composes with the map buffer's `elem_dec_fn` key-string walk.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_inline_for_loop_map_two_call_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "inline_for_loop_map_two_call");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_inline_for_loop_single_loop_negative_no_extra_release.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "inline_for_loop_single_loop_negative");
}

#[test]
fn probe_bare_unused_iterator_handle_freed_at_scope_exit() {
    // In-function iterator-handle (RL-2): `[..].iter()` produces a FRESH owned
    // `DoubleEndedIterator` handle (the buffer moved INTO the iterator state). It
    // is never consumed by a for-loop / `iter_next` / `ori_iter_drop`, so it must
    // be freed by a scope-exit `RcDec [Iterator]` (= `ori_iter_drop`). The default
    // path emits it; the burden path must emit a standalone `BurdenDec` on the
    // handle lineage that lowers (via `RcStrategy::from_repr` Iterator) to the same.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_bare_unused_iterator_handle_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "bare_unused_iterator_handle");
}

#[test]
fn probe_iterator_handle_in_tuple_freed_at_scope_exit() {
    // Iterator handle MOVED into a tuple field: the handle transfers ownership
    // into the `Construct Tuple`, so the freeing burden is on the AGGREGATE — the
    // tuple's scope-exit `RcDec [AggFields]` walks to the iterator field and
    // `ori_iter_drop`s it (freeing the iterator-owned buffer). The burden path
    // must emit a `BurdenDec` on the fresh iterator-bearing tuple lineage.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_iterator_handle_in_tuple_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "iterator_handle_in_tuple");
}

#[test]
fn probe_iterator_handle_in_struct_freed_at_scope_exit() {
    // Iterator handle MOVED into a struct field — same AGGREGATE-drop mechanism
    // as the tuple shape, exercising `Tag::Struct` `RcStrategy::AggregateFields`.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_iterator_handle_in_struct_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "iterator_handle_in_struct");
}

#[test]
fn probe_for_loop_iterator_handle_negative_no_double_free() {
    // A `for x in coll` lowering emits an explicit `@ori_iter_drop` Apply on
    // every loop-exit path. Iterator-handle cleanup must not also emit a dec on
    // that for-loop-managed handle — doing so double-frees the iterator-owned
    // buffer. The handle's lineage is in `compute_iter_drop_handle_lineages`
    // (it is an `ori_iter_drop` arg) and must be excluded.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_loop_iterator_handle_negative_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_loop_iterator_handle_negative");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_int_result_dup_indexed_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_yield_int_result_dup_indexed");
}

#[test]
fn probe_for_yield_int_result_triple_indexed_freed() {
    // `for i in nums yield i * i` builds an `[int]` result indexed THREE times.
    // The `ori_list_take` fresh-result over-count nets +1 regardless of index
    // multiplicity (the dup-index incs net 0 among themselves); the net-keyed
    // elision removes the single surplus fresh inc.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_int_result_triple_indexed_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_yield_int_result_triple_indexed");
}

#[test]
fn probe_for_yield_int_result_single_use_negative_no_double_free() {
    // NEGATIVE pin (the alloc-aware-net boundary): a SINGLE-use for_yield result
    // (`.length()` only, no dup-index) has a net != 1 once the move-alias dec is
    // counted — the fresh inc is load-bearing there. The net-keyed elision MUST
    // NOT elide it (eliding a net-0 lineage's inc would net −1 = a double-free).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_int_result_single_use_negative_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_yield_int_result_single_use_negative");
}

// JUMP-THREADED `ori_list_take` result (the `for_yield_*_two_call` shape). TWO
// for_yields over the same source build two `[int]` results; the FIRST result's
// `ori_list_take` value flows through a Jump-arg → block-param POSITIONAL rename
// (the 2nd loop's scratch-init block carries it forward) before its lone TRUE
// release fires on the threaded block-param's `Let` alias. The fresh-site
// `BurdenInc` + paired premature `BurdenDec` net 0 at the alloc site; the threaded
// subsequent dec is the genuine single release. `compute_same_alloc_reps` excludes
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_int_two_call_jump_threaded_result_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_yield_int_two_call_jump_threaded");
}

#[test]
fn probe_for_yield_int_three_call_jump_threaded_result_no_double_free() {
    // THREE for_yields over the same source → the first TWO results are each
    // jump-threaded forward through the later loops' init blocks. Both threaded
    // results must keep their fresh incs (phi-aware net == 0 each), so neither
    // double-frees.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_int_three_call_jump_threaded_result_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_yield_int_three_call_jump_threaded");
}

#[test]
fn probe_for_yield_int_single_call_not_threaded_negative_no_double_free() {
    // NEGATIVE pin (the phi-threading lower boundary): a SINGLE for_yield result
    // dup-indexed (NOT jump-threaded — straight-line flow, no Jump-arg → param
    // rename of the result). The phi-aware net MUST collapse to the unthreaded net
    // here (no phi edge to thread), keeping the alloc-aware-net fresh-inc elision
    // intact (net +1 → elide the surplus fresh inc, no leak). The phi-aware
    // extension must not perturb the non-threaded single-result case.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_int_single_call_not_threaded_negative_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_yield_int_single_call_not_threaded");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_str_identity_indexed_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_yield_str_identity_indexed");
}

#[test]
fn probe_for_yield_break_str_no_double_free() {
    // `for w in words yield { if ..break; w }` — early-exit yield of the heap str
    // element. The result is length-checked (index-equivalent consumption: the
    // result owns its yielded copies; the source retains the un-yielded ones).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_break_str_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_yield_break_str");
}

#[test]
fn probe_for_yield_str_identity_iter_consumed_negative_no_double_free() {
    // NEGATIVE pin (the move-vs-borrow discriminator boundary): `for w in words
    // yield w` then a SECOND for-loop consumes the result (`for w in copy do ...`).
    // The result is ITER-consumed (`@iter [own]` → `ori_iter_drop` frees its
    // elements), so the yield-element inc + per-view dec MUST NOT fire (adding the
    // inc would double-free against the iterator drop).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_str_identity_iter_consumed_negative_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_yield_str_identity_iter_consumed");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_str_list_two_iter_consuming_calls_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "str_list_two_iter_consuming_calls");
}

#[test]
fn probe_int_list_two_iter_consuming_calls_no_double_free() {
    // Type-dimension cell: `[int]` source (scalar elements, but the buffer is
    // still RcPtr) borrowed by two iter-consuming calls.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_int_list_two_iter_consuming_calls_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "int_list_two_iter_consuming_calls");
}

#[test]
fn probe_two_distinct_iter_consumed_sources_no_double_free() {
    // Pattern cell: two DISTINCT sources, one borrowed twice + one borrowed once,
    // interleaved — each source's keep-alive accounting is independent.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_two_distinct_iter_consumed_sources_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "two_distinct_iter_consumed_sources");
}

#[test]
fn probe_chained_iter_consuming_callee_no_double_free() {
    // Pattern cell: the iter-consume is one call deep (a `wrapper` forwards the
    // borrowed source to the iter-consuming `iterate_words`); `wrapper` is called
    // twice on the same source.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_chained_iter_consuming_callee_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "chained_iter_consuming_callee");
}

#[test]
fn probe_single_iter_consuming_call_negative_still_freed() {
    // NEGATIVE pin (the multi-borrow lower boundary): a SINGLE iter-consuming call
    // where the source DIES after the call (the single-borrow `Suppress` shape).
    // The multi-borrow suppression must NOT change this — the callee's iter-drop
    // is still the sole release; emitting a keep-alive inc here would LEAK.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_single_iter_consuming_call_negative_still_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "single_iter_consuming_call_negative");
}

#[test]
fn probe_iter_consumed_twice_inside_borrowed_callee_no_double_free() {
    // Pattern cell: the two iter-consuming calls happen INSIDE a borrowed-param
    // callee (`call_twice` borrows `words`, calls iter-consuming `sum_lens` twice)
    // — the multi-borrow keep-alive accounting must hold one call-frame deep, on
    // the borrowed param's lineage.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_iter_consumed_twice_inside_borrowed_callee_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "iter_consumed_twice_inside_borrowed_callee");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_option_str_match_projected_interior_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_yield_option_str_match_projected_interior");
}

#[test]
fn probe_for_yield_struct_field_projected_interior_no_double_free() {
    // `item.name.length()` projects the `name: str` field out of the `Item`
    // iter-element-view via `Project (struct).0`. The field view is a borrow.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_struct_field_projected_interior_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "for_yield_struct_field_projected_interior");
}

// Nested-loop iter-element-view keep-alive: the inner loop's source is a
// `Project @__iter_next.1` of the OUTER source (an iter-element-view) consumed
// `[own]` by the inner `@iter`. The inner element view owns no allocation (the
// outer `elem_dec_fn` frees it), yet the inner `@iter [own]` -> `ori_iter_drop`
// ALSO frees it -> double-free WITHOUT a keep-alive inc on the inner view.

#[test]
fn probe_nested_for_do_str_inner_list_keepalive_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_nested_for_do_str_inner_list_keepalive_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "nested_for_do_str_inner_list_keepalive");
}

#[test]
fn probe_nested_for_do_three_level_int_list_keepalive_no_double_free() {
    // Three nesting levels: each level's `Project @__iter_next.1` inner-list view
    // is iter-consumed by the next `@iter [own]` and needs its own keep-alive.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_nested_for_do_three_level_int_list_keepalive_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "nested_for_do_three_level_int_list_keepalive");
}

#[test]
fn probe_for_yield_inner_list_user_callee_iter_consume_keepalive_no_double_free() {
    // `for l in lists yield sum_list(l)` — the inner `[int]` element view `l` is
    // passed to a USER callee `sum_list` whose `ParamContract.iter_consumes` is
    // true (its body `for x in xs` -> `@iter [own]` -> `ori_iter_drop` frees the
    // arg). The view needs a keep-alive inc before the iter-consuming call.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_for_yield_inner_list_user_callee_iter_consume_keepalive_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_flat_str_yield_not_keepalive_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "flat_str_yield_not_keepalive_negative");
}

// Accessor-result payload retention (Spec: Annex E §AIMS RL-2 / RL-4)
//
// `@unwrap` / `@unwrap_err` / `@first` / `@last` / `@get` extract an OWNED heap
// payload out of a wrapper and RETAIN it (codegen `inc_value_rc` on the extracted
// element/payload). The wrapper/source is passed at a BORROWED `Invoke` terminator
// arg position; per RL-2/RL-4 it SURVIVES the accessor call and is released on the
// normal+unwind successor EDGES — never inline before the borrowed call. Emitting
// the source dec inline frees the payload BEFORE the accessor's retain runs ->
// use-after-free. RL-2/RL-4 place the source dec on BOTH successor edges; the
// emitted path must match that placement.

#[test]
fn probe_option_unwrap_heap_str_payload_retained() {
    // `o.unwrap()` extracts an owned heap str out of `Option<str>`. The wrapper
    // `o` is borrowed by `@unwrap`; its dec must land on the successor edge AFTER
    // `@unwrap` retains the payload, not inline before -> else the payload is
    // freed under the still-aliasing result (UAF; masked at the floor only by
    // same-size allocator slot reuse).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_option_unwrap_heap_str_payload_retained.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "option_unwrap_heap_str_payload_retained");
}

#[test]
fn probe_option_unwrap_heap_str_different_size_literal_no_double_free() {
    // The different-size-literal variant: the `==` comparison literal is a
    // DIFFERENT length than the unwrapped payload, so the freed payload slot is
    // NOT reused by the literal alloc -> the latent UAF surfaces as a hard
    // double-free at the clean floor (the same-size variant only passes by
    // allocator slot reuse). The intentionally false comparison makes a clean
    // exit independent of allocator coincidence.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_option_unwrap_heap_str_different_size_literal_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "option_unwrap_heap_str_different_size_literal_no_double_free",
    );
}

#[test]
fn probe_result_unwrap_heap_str_payload_retained() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_result_unwrap_heap_str_payload_retained.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "result_unwrap_heap_str_payload_retained");
}

#[test]
fn probe_result_unwrap_err_list_payload_retained() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_result_unwrap_err_list_payload_retained.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "result_unwrap_err_list_payload_retained");
}

#[test]
fn probe_list_first_heap_str_payload_retained() {
    // `items.first()` returns `Option<str>` whose Some payload is a RETAINED copy
    // of the first element. The list `items` is borrowed by `@first`; its dec
    // belongs on the successor edge AFTER `@first` retains the element copy.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_first_heap_str_payload_retained.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "list_first_heap_str_payload_retained");
}

#[test]
fn probe_list_last_list_payload_retained() {
    let src =
        include_str!("fixtures/memory_lifecycle_probe/probe_list_last_list_payload_retained.ori");
    assert_runs_clean_no_leak_or_double_free(src, "list_last_list_payload_retained");
}

#[test]
fn probe_eq_comparison_literal_stays_elidable_negative() {
    // NEGATIVE / over-fire guard: a heap str compared by `==` against a heap
    // literal, with NO accessor in play. The comparison literal is a borrow-read
    // operand (RL-1 `!incElidable`) — relocating accessor source decs to edges
    // must NOT disturb the comparison-literal balance, and the `==`-literal must
    // stay leak-free. Clamps the cure to accessor-source decs only.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_eq_comparison_literal_stays_elidable_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "eq_comparison_literal_stays_elidable_negative");
}

#[test]
fn probe_list_contains_borrowed_read_no_payload_negative() {
    // NEGATIVE / clamp: `.contains(value:)` is a borrowed-read returning a SCALAR
    // (`bool`) — it extracts NO heap payload. The source list dec must keep its
    // existing balance and the heap-str literal arg must stay leak-free; the
    // accessor-source relocation must NOT over-fire on a no-payload borrowed read.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_contains_borrowed_read_no_payload_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "list_contains_borrowed_read_no_payload_negative",
    );
}

/// Retain-aliasing closure growth (gate d) DECLINES when an accessor-retain
/// member (`val.unwrap()`) feeds a FURTHER borrowed read whose result is
/// NEITHER a tracked closure member NOR provably scalar — here
/// `.substring(..)`, a sharing-view producer returning a co-owning view of
/// the SAME str backing buffer. An unvetted closure would place the
/// unwrap()-result's release without accounting for the untracked substring
/// view's own read (use-after-free / double-free). The decline is correct;
/// fallback placement is outside this vet.
#[test]
fn probe_retain_aliasing_untracked_sharing_view_declines_no_uaf() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_retain_aliasing_untracked_sharing_view_declines_no_uaf.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "retain_aliasing_untracked_sharing_view_declines",
    );
}

/// Same scalar-decline gate as
/// [`probe_retain_aliasing_untracked_sharing_view_declines_no_uaf`], on a
/// list-`.first()` root instead of a map `__index` root. Map roots take a
/// distinct fallback-placement path, so this list-root variant isolates the
/// vet's decline behavior cleanly. `f.unwrap()` is a
/// tracked accessor-retain member; `.substring(..)` on it is neither a
/// member nor provably scalar, so gate (d) must decline retain-aliasing
/// admission and let the base walk place the releases.
#[test]
fn probe_retain_aliasing_untracked_sharing_view_declines_list_root_no_uaf() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_retain_aliasing_untracked_sharing_view_declines_list_root_no_uaf.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "retain_aliasing_untracked_sharing_view_declines_list_root",
    );
}

// No-use dead-owned-collection scope-exit cleanup dec (RL-2 unused-owned).
//
// `let x = [Outer { .. }]` constructs an owned `[T]` that is DEAD at scope exit
// with ZERO uses. RL-2 mandates an immediate scope-exit cleanup dec on an unused
// owned non-scalar definition ("unused owned non-scalar (Dead/Absent) -> immediate
// RcDec at definition", Spec: Annex E §AIMS RL-2), emitted as a straight-line
// `RcDec %x [HeapPtr]` at scope exit. A sink keyed on a borrowed-read LAST use
// cannot reach this value, because a never-used value has no such use. The
// omission leaks the value AND silently elides the
// element's user `@drop` side-effects. When the element type has a panicking
// `@drop`, the missing dec is observable: exit 0 + no drop print, instead of the
// drop running (its print appears) + unwind exit 1. One whole-collection dec walks
// `elem_dec_fn` recursively through nested struct / map / enum payloads and the
// runtime drop-glue handles the panic-during-drop continuation.

/// Compile `source` on the default RC emission path and assert the dead
/// no-use owned value's user `@drop` actually runs (its `expect_print` appears in
/// stdout — the cleanup dec fired), the program unwinds (exit 1, not abort 134 or
/// silent leak exit 0), and no leak / double-free diagnostic surfaces.
fn assert_burden_dead_no_use_drop_runs(source: &str, expect_print: &str, label: &str) {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(source, &[]);
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_list_struct_drop_runs_at_scope_exit.ori"
    );
    assert_burden_dead_no_use_drop_runs(src, "drop-a", "dead_no_use_list_struct");
}

#[test]
fn probe_dead_no_use_list_struct_user_panic_drop_runs() {
    // `[Holder { payload: <heap-str> }]`, dead no-use. The user @drop body prints
    // then panics — the print proves the scope-exit dec fired and ran the drop glue.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_list_struct_user_panic_drop_runs.ori"
    );
    assert_burden_dead_no_use_drop_runs(src, "drop-user", "dead_no_use_list_struct_user_panic");
}

#[test]
fn probe_dead_no_use_list_map_value_drop_runs() {
    // `[Wrap { m: {"k": Boom{..}} }]`, dead no-use. The single outer-List dec walks
    // `elem_dec_fn` -> Wrap field walk -> Map two-channel teardown -> Boom @drop.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_list_map_value_drop_runs.ori"
    );
    assert_burden_dead_no_use_drop_runs(src, "boom-v", "dead_no_use_list_map_value");
}

#[test]
fn probe_dead_no_use_list_enum_payload_drop_runs() {
    // `[Both(loud:.., quiet:..)]`, dead no-use. The outer-List dec walks to the enum
    // payload: the loud field @drop panics, and the quiet peer field still drops via
    // the landing pad (both prints appear).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_list_enum_payload_drop_runs.ori"
    );
    let (exit, stdout, stderr) = compile_and_run_with_build_env(src, &[]);
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_list_nested_collection_element_drop_runs.ori"
    );
    let (exit, stdout, stderr) = compile_and_run_with_build_env(src, &[]);
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_list_int_buffer_freed_at_scope_exit.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "dead_no_use_list_int_buffer_freed_at_scope_exit",
    );
}

#[test]
fn probe_dead_no_use_returned_list_no_double_dec_negative() {
    // NEGATIVE / transfer clamp: a fresh owned list that IS RETURNED is an RL-2
    // ownership transfer — the caller inherits the release. The dead-no-use
    // scope-exit cleanup must NOT fire on a returned value (a double-dec aborts).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_returned_list_no_double_dec_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "dead_no_use_returned_list_no_double_dec_negative",
    );
}

#[test]
fn probe_dead_no_use_used_list_no_extra_dec_negative() {
    // NEGATIVE / used-value clamp: an owned list that IS USED (borrowed-read
    // `.length()`) before dying at scope exit is handled by the existing borrowed-
    // read last-use sink, NOT the no-use path. The no-use cleanup must NOT also fire
    // (a double release on a used-then-dead value aborts).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_used_list_no_extra_dec_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "dead_no_use_used_list_no_extra_dec_negative");
}

// Dead-no-use INLINE-AGGREGATE matrix (RL-2 ScopeExit). A bare `let a = Doc {
// field: <heap> }` / `let c = Link(..)` / `let t = (.., ..)` binds an inline
// struct / enum / tuple (`ValueRepr::Aggregate`, lowered `RcStrategy::AggregateFields`
// for struct/tuple and `RcStrategy::InlineEnum` for sum types) whose type
// `burden_carries_rc` (a heap-bearing `owned_fields` / `variant_burdens` field),
// dead with ZERO uses. The oracle emits one scope-exit `RcDec [AggFields]` /
// `[InlineEnum]` that walks the field drop-glue, freeing the heap field(s) and
// running their user `@drop`. Under sole-emitter burden lowering the AIMS walk
// emits ZERO burden ops on the no-use aggregate (no duplicating use -> no inc, no
// last-use sink -> no dec), so the heap field is NEVER freed and its `@drop`
// silently does not run (a leak; observable as the missing drop print). Distinct
// from dead-no-use `RcPointer` collection shapes (`let r = [Resource {..}]`): those
// wrap the aggregate in an `RcPointer` list buffer; these are the BARE inline
// aggregate. An inline aggregate has NO self-buffer `+1` (it is not heap-allocated);
// the dec balances the HEAP FIELD's implicit `+1` owned by the AggFields / InlineEnum
// drop-glue. `compute_dead_no_use_aggregate_reps` selects
// `var_repr in {Aggregate}` + `burden_carries_rc`, emitting one scope-exit
// `BurdenDec` on the OUTERMOST dead-no-use lineage (nested constructs are
// owned-consumed into the parent Construct -> excluded). Spec: Annex E §AIMS RL-2.

/// Compile `source` on the default RC emission path and assert the dead
/// no-use aggregate's owned field `@drop` runs (its `expect_print` appears in
/// stdout — the scope-exit cleanup dec fired), the program exits 0, and no leak /
/// double-free diagnostic surfaces. For non-panicking field drops.
fn assert_burden_dead_no_use_aggregate_drop_runs(source: &str, expect_print: &str, label: &str) {
    let (exit, stdout, stderr) = compile_and_run_with_build_env(source, &[]);
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_struct_str_field_drop_runs.ori"
    );
    assert_burden_dead_no_use_aggregate_drop_runs(src, "drop-S", "dead_no_use_struct_str_field");
}

#[test]
fn probe_dead_no_use_tuple_str_fields_drop_runs() {
    // Bare `let t = (Logged {..}, Logged {..})`: a tuple (`AggFields`), dead no-use.
    // The scope-exit dec walks both tuple slots in reverse decl order.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_tuple_str_fields_drop_runs.ori"
    );
    let (exit, stdout, stderr) = compile_and_run_with_build_env(src, &[]);
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_option_struct_drop_runs.ori"
    );
    assert_burden_dead_no_use_aggregate_drop_runs(src, "drop-O", "dead_no_use_option_struct");
}

#[test]
fn probe_dead_no_use_result_struct_drop_runs() {
    // Bare `let a: Result<Logged, int> = Ok(Logged {..})`: a tagged Result
    // (`InlineEnum`), dead no-use. The scope-exit `RcDec [InlineEnum]` walks the Ok
    // payload and runs its @drop.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_result_struct_drop_runs.ori"
    );
    assert_burden_dead_no_use_aggregate_drop_runs(src, "drop-RE", "dead_no_use_result_struct");
}

#[test]
fn probe_dead_no_use_user_enum_payload_drop_runs() {
    // Bare `let c = Link(a:.., b:.., next: Link(.., next: Nil))`: a user sum type
    // (`InlineEnum`) with a heap-bearing recursive payload, dead no-use. The single
    // scope-exit `RcDec [InlineEnum]` on the OUTERMOST `c` lineage walks every node's
    // payload recursively (the nested Link is owned-consumed into the outer Construct,
    // so it gets NO separate dec) and runs every node's field @drop.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_user_enum_payload_drop_runs.ori"
    );
    let (exit, stdout, stderr) = compile_and_run_with_build_env(src, &[]);
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_nested_struct_field_drop_runs.ori"
    );
    let (exit, stdout, stderr) = compile_and_run_with_build_env(src, &[]);
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_heap_str_field_freed_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "dead_no_use_heap_str_field_freed");
}

#[test]
fn probe_dead_no_use_returned_aggregate_no_double_free_negative() {
    // NEGATIVE / transfer clamp: a fresh owned aggregate that IS RETURNED is an RL-2
    // ownership transfer — the caller inherits the release. The dead-no-use scope-exit
    // cleanup must NOT fire on a returned aggregate (a double-dec aborts / double-frees
    // the heap field).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_returned_aggregate_no_double_free_negative.ori"
    );
    let (exit, stdout, stderr) = compile_and_run_with_build_env(src, &[]);
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_dead_no_use_scalar_only_struct_no_dec_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "dead_no_use_scalar_only_struct_no_dec");
}

// Take-project iterator-handle source matrix (RL-2 ScopeExit + bypass-safe
// per-class drop). An `Iterator<int>` payload inside an enum is projected out
// and consumed on one match arm; on every NON-projecting path the source enum
// (holding the iterator handle) is dead-at-scope-exit and must be freed
// (`RcDec [InlineEnum]` -> the InlineEnum drop walks the iterator field ->
// `ori_iter_drop`). Under sole-emitter burden lowering the Phase-5 walk
// mis-models the take-project source: it emits a spurious dec on the consuming
// arm (-> use-after-free, the iterator is freed before `@count` reads it) and
// omits the dec on the bypass / Empty paths (-> leak). The cure classifies the
// take-project source's release per arm: consumed (no dec) on the projecting
// arm, dead-at-scope-exit (decced) on every non-projecting arm.

#[test]
fn probe_take_project_match_consume_no_use_after_free() {
    // Iterator projected out of `Holds` and consumed via `.count()`. The source
    // enum must NOT be decced on the consume arm (the projection transfers the
    // iterator out; `@count` owns + frees it) -> a dec there is a use-after-free.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_take_project_match_consume_no_use_after_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "take_project_match_consume");
}

#[test]
fn probe_take_project_conditional_consume_no_leak() {
    // Path-sensitive: `if flag then <match consumes> else 0`. On the runtime
    // bypass path (flag false) the iterator is never consumed, so the source
    // enum is dead-at-scope-exit on the else branch and must be freed there
    // (the burden walk omitted that dec -> leak).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_take_project_conditional_consume_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "take_project_conditional_consume");
}

#[test]
fn probe_take_project_dynamic_consume_no_double_free() {
    // Dynamic Holds/Empty construction via a helper -> the match diamond is not
    // constant-folded, both arms live. The Empty arm frees the whole enum; the
    // Holds arm transfers the iterator out -> no double-free across the diamond.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_take_project_dynamic_consume_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "take_project_dynamic_consume");
}

#[test]
fn probe_take_project_two_unrelated_sources_no_leak() {
    // Two independent take-project sources `a`, `b` on disjoint alias chains:
    // `b` is consumed, `a` is on a bypass path. `a` must drop on the bypass-safe
    // path via its own per-class scope-exit drop (function-global bypass-safe
    // computation would suppress `a`'s drop on every block reachable from `b`).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_take_project_two_unrelated_sources_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "take_project_two_unrelated_sources");
}

#[test]
fn probe_take_project_phi_merge_no_leak() {
    // Two take-project sources whose match-arm RESULTS converge at a phi-style
    // merge block param. The phi param is a CFG choice, not shared storage; the
    // per-class bypass-safe set must not falsely conflate the two sources'
    // lineages through the shared merge param.
    let src =
        include_str!("fixtures/memory_lifecycle_probe/probe_take_project_phi_merge_no_leak.ori");
    assert_runs_clean_no_leak_or_double_free(src, "take_project_phi_merge");
}

#[test]
fn probe_take_project_in_loop_no_leak() {
    // Topology: a take-project source held across an explicit `loop { break }`,
    // consumed conditionally after. The loop body never reaches the projection
    // (bypass path); the source enum must drop on the post-loop bypass-safe path
    // even though the loop header is reached via a back-edge from a bypass-safe
    // latch.
    let src =
        include_str!("fixtures/memory_lifecycle_probe/probe_take_project_in_loop_no_leak.ori");
    assert_runs_clean_no_leak_or_double_free(src, "take_project_in_loop");
}

#[test]
fn probe_take_project_unused_binding_negative_no_double_free() {
    // NEGATIVE / project-then-unused clamp: the iterator is projected into a
    // binding that is NEVER consumed (no `.count()`). The projected iterator
    // binding drops at its OWN scope exit (handled already); the take-project
    // source-dec extension must NOT also fire on the source enum here (a double
    // release of the same iterator payload aborts). This guards the
    // `enum_match_unused_binding` shape from regressing.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_take_project_unused_binding_negative_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "take_project_unused_binding_negative");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_eager_filter_borrowed_source_freed_after_call.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "eager_filter_borrowed_source_freed");
}

#[test]
fn probe_eager_map_borrowed_source_freed_after_call() {
    // `nums.map(f)`: the eager list map BORROWS `nums` and produces a FRESH
    // non-aliasing `[int]` result. Same misplaced-inline-source-dec UAF as
    // filter; same RL-2 + RL-4 successor-edge relocation cure.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_eager_map_borrowed_source_freed_after_call.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "eager_map_borrowed_source_freed");
}

#[test]
fn probe_eager_filter_then_index_borrowed_source_freed() {
    // `nums.filter(p)` then `evens[0]`: the fresh filter result is index-read.
    // The borrowed source `nums` still must relocate its dec to the successor
    // edge (it does not alias the fresh result, which the index reads).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_eager_filter_then_index_borrowed_source_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "eager_filter_then_index_borrowed_source_freed");
}

#[test]
fn probe_bare_list_len_borrowed_source_unchanged_negative() {
    // NEGATIVE / scalar-result clamp: a bare `nums.len()` (no transform) already
    // relocates its borrowed-source dec to the successor edge via the verdict's
    // scalar-result-builtin branch (`@len` returns `int`). The eager-transform
    // extension must NOT disturb this floor-passing path — the source frees once
    // on the successor edge, leak-free + double-free-free, with or without the
    // new fresh-result set.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_bare_list_len_borrowed_source_unchanged_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "bare_list_len_borrowed_source_unchanged");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_clone_borrowed_source_freed_after_call.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "list_clone_borrowed_source_freed");
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
    let src =
        include_str!("fixtures/memory_lifecycle_probe/probe_iter_map_collect_result_freed_int.ori");
    assert_runs_clean_no_leak_or_double_free(src, "iter_map_collect_result_freed_int");
}

#[test]
fn probe_iter_map_collect_result_freed_heap_str() {
    // Same iter-chain shape with HEAP-string elements: the `@collect` result owns
    // its element COPIES (`ori_iter_collect` `elem_inc_fn`s each element into the
    // fresh buffer). The RL-2 scope-exit `RcDec [HeapPtr]` on the result walks the
    // V5 `elem_dec_fn` so the str copies free too (the buffer + element-glue
    // composition). Without it the result buffer + 2 element strings leak.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_iter_map_collect_result_freed_heap_str.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "iter_map_collect_result_freed_heap_str");
}

#[test]
fn probe_iter_collect_result_returned_no_double_free_negative() {
    // NEGATIVE: when the collect result is RETURNED (transferred to the caller),
    // the caller inherits the release (RL-2 transfer). The dead-owned-collection
    // pass MUST NOT emit a freeing dec on a returned collect result (the
    // `compute_returned_lineages` exclusion holds) — a dec here would double-free
    // against the caller's release. `@main` consumes the returned list locally.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_iter_collect_result_returned_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "iter_collect_result_returned_no_double_free");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_loop_carried_push_list_int_source_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "loop_carried_push_list_int_source");
}

#[test]
fn probe_loop_carried_push_list_str_source_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_loop_carried_push_list_str_source_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "loop_carried_push_list_str_source");
}

#[test]
fn probe_loop_carried_insert_map_source_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_loop_carried_insert_map_source_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "loop_carried_insert_map_source");
}

#[test]
fn probe_loop_carried_concat_str_source_no_double_free() {
    // `s = s + "x"` concat-loop: the old string operand is consumed/COW-read by
    // `ori_str_concat` (an owned-position duplicating use) each iteration.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_loop_carried_concat_str_source_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "loop_carried_concat_str_source");
}

#[test]
fn probe_loop_carried_push_while_source_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_loop_carried_push_while_source_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "loop_carried_push_while_source");
}

#[test]
fn probe_loop_carried_push_break_source_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_loop_carried_push_break_source_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "loop_carried_push_break_source");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_loop_invariant_closure_borrow_no_premature_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "loop_invariant_closure_borrow_negative");
}

#[test]
fn probe_loop_invariant_map_index_borrow_no_premature_free_negative() {
    // NEGATIVE / loop-invariant read-only-BORROW clamp: a map read via `m[k]`
    // (`__index [borrow]`) each iteration, never reassigned/consumed. The map's
    // bb0 fresh inc is elidable; the loop-carried-consume cure MUST NOT flag the
    // read-only map lineage (a `[borrow]` index is not an owned-position
    // consume).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_loop_invariant_map_index_borrow_no_premature_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "loop_invariant_map_index_borrow_negative");
}

#[test]
fn probe_loop_reassigned_list_borrow_read_no_double_free_negative() {
    // NEGATIVE / loop-invariant-BORROW closure clamp: a list local REASSIGNED
    // each iteration (`xs = [i]; xs = xs.push(i)`) is then borrow-READ
    // (`@__index [borrow]`). The reassignment feeds a FRESH per-iteration
    // allocation into the loop-carried slot via the back-edge, so the slot is
    // NOT loop-invariant — the borrow-only-read RL-5 release MUST NOT fire (the
    // base walk already releases each fresh value). Over-firing a single
    // scope-exit release on the reassigned slot double-frees the last
    // iteration's buffer (`-134`). The discriminator is the lineage-closure gate:
    // a member block-param fed by a non-member (the fresh reassignment) declines.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_loop_reassigned_list_borrow_read_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "loop_reassigned_list_borrow_read_negative");
}

// Transfer-through-return forwarder RESULT freeing (RL-2 ScopeExit)
// A fresh-owned collection passed `[own]` into a `transfers_through_return ∧
// ReturnAliasShape::Direct` forwarder (`@id<T>(x: T) -> T = x`) is returned
// unchanged: the caller's result IS the SAME allocation as the transferred owned
// arg, borrowed-read then dead at scope exit, carrying ZERO burden ops. Under
// sole-emitter lowering the allocation's `+1` is never released → leak.
// The per-allocation alloc-aware net threaded through the apply-Direct transfer
// edge fires ONE scope-exit `BurdenDec` on the result's live SSA value (the
// trivial `@id` chain nets +1 = leaked → dec; a multi-borrow-then-return
// multi-borrow-return forwarder already nets 0 = released → no dec).

#[test]
fn probe_forwarder_result_freed_id_list_int() {
    // `@id` over `[int]`: result borrowed-read (`@len`/`@__index`) then dead.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_forwarder_result_freed_id_list_int.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "forwarder_result_freed_id_list_int");
}

#[test]
fn probe_forwarder_result_freed_multi_hop_list() {
    // Two-hop chain `@id2(@id1(xs))`: each hop is a Direct transfer; the final
    // result is the same allocation as the original Construct.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_forwarder_result_freed_multi_hop_list.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "forwarder_result_freed_multi_hop_list");
}

#[test]
fn probe_forwarder_result_freed_non_generic_list() {
    // Non-generic forwarder — the apply-Direct merge is structural, not
    // generics-keyed. Straight-line result use (single condition): the lineage's
    // single unbalanced allocation `+1` is released at its one borrowed-read dead
    // sink. (Branchy multi-condition result use is the compound-shape next leaf —
    // the result's own per-branch `binc`/`bdec` pairs need joint accounting.)
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_forwarder_result_freed_non_generic_list.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "forwarder_result_freed_non_generic_list");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_multi_borrow_then_return_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "multi_borrow_then_return_no_double_free_negative",
    );
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_project_borrowed_view_struct_str_field_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "project_borrowed_view_struct_str_field");
}

#[test]
fn probe_project_borrowed_view_struct_list_str_field_no_double_free() {
    // `[str]`-field view: `c.items` borrow-view of `Container { items: [str] }`.
    // The struct `[AggFields]` drop frees the `[str]` buffer (RcPtr field); the
    // spurious view dec double-frees it.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_project_borrowed_view_struct_list_str_field_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "project_borrowed_view_struct_list_str_field");
}

#[test]
fn probe_project_borrowed_view_struct_list_int_field_no_double_free() {
    // `[int]`-field view: `c.items` borrow-view of `Container { items: [int] }`.
    // Same shape as the `[str]` field — the scalar element type does not change
    // the aggregate-drop-frees-the-buffer accounting. The membership-strip
    // approach mishandled `[int]`-field index-retain shapes; the net does not.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_project_borrowed_view_struct_list_int_field_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "project_borrowed_view_struct_list_int_field");
}

#[test]
fn probe_project_borrowed_view_option_struct_str_field_no_double_free() {
    // Option-payload struct-field view: a `Some(Wrapper { s: str })` matched, then
    // the inner struct's `s` field projected and borrow-read. The InlineEnum +
    // AggFields drop walk frees the field; the spurious view dec double-frees.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_project_borrowed_view_option_struct_str_field_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "project_borrowed_view_option_struct_str_field");
}

#[test]
fn probe_project_borrowed_view_result_struct_str_field_no_double_free() {
    // Result-payload struct-field view: an `Ok(Wrapper { s: str })` matched, then
    // the inner struct's `s` field projected and borrow-read. Same InlineEnum +
    // AggFields drop walk; the spurious view dec double-frees.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_project_borrowed_view_result_struct_str_field_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "project_borrowed_view_result_struct_str_field");
}

#[test]
fn probe_project_borrowed_view_paired_inc_collection_field_keep_negative() {
    // NEGATIVE / keep clamp: a struct with TWO projected fields (a map field AND a
    // str field) each `.length()`-read. The aggregate is copied (a paired
    // `[AggFields]` inc raises the fields past rc 1), so each projection dec
    // releases the EXTRA reference, NOT a redundant second release of a single-ref
    // field. The alloc-aware net is 0 (the aggregate inc balances the alloc) ->
    // the dec is the genuine release and MUST be kept. A membership-strip orphans
    // the index-retain inc here and leaks; the net keeps it.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_project_borrowed_view_paired_inc_collection_field_keep_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_project_borrowed_view_sum_str_payload_keep_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "project_borrowed_view_sum_str_payload_keep");
}

#[test]
fn probe_project_borrowed_view_sum_list_int_payload_keep_negative() {
    // NEGATIVE / keep clamp: a `Numbers(items: [int])` sum variant matched, the
    // `[int]` payload extracted and borrow-read. Same paired-inc keep accounting
    // as the str payload — the RcPtr buffer's release is balanced (net 0). The
    // last-owner sum-payload view is the buffer's genuine release; keep it.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_project_borrowed_view_sum_list_int_payload_keep_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "project_borrowed_view_sum_list_int_payload_keep",
    );
}

#[test]
fn probe_project_borrowed_view_owned_literal_release_keep_negative() {
    // NEGATIVE / keep clamp: a bare owned heap str literal (NOT a projection) with
    // its own last-use release. The strip discriminator keys on a `Project`-view
    // whose source aggregate drop frees the field; an owned non-view value has no
    // such source, so its release MUST be kept. Stripping it leaks the string.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_project_borrowed_view_owned_literal_release_keep_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "project_borrowed_view_owned_literal_release_keep",
    );
}

#[test]
fn probe_project_borrowed_view_disjoint_field_no_double_free() {
    // A struct with TWO heap fields where ONE is projected-and-borrow-read and the
    // OTHER is unused. The aggregate `[AggFields]` drop frees BOTH fields; the
    // spurious view dec on the projected field double-frees it (the unused field
    // is freed once by the aggregate drop — no view to double it). Strip the
    // projected-view dec; the aggregate drop owns both releases.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_project_borrowed_view_disjoint_field_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "project_borrowed_view_disjoint_field");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_derived_eq_used_struct_str_field_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "derived_eq_used_struct_str_field");
}

#[test]
fn probe_derived_eq_used_struct_list_field_no_leak() {
    // The `[int]`-field derived-`Eq` shape: the aggregate holds an `RcPtr` list
    // buffer; the comparison-operand spurious incs leak the buffer.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_derived_eq_used_struct_list_field_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "derived_eq_used_struct_list_field");
}

#[test]
fn probe_derived_eq_used_struct_map_field_no_leak() {
    // The `{str: int}`-field derived-`Eq` shape: the aggregate holds an `RcPtr` map
    // buffer (with owned key strings via `elem_dec_fn`); the comparison-operand
    // spurious incs leak the whole map + its key strings.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_derived_eq_used_struct_map_field_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "derived_eq_used_struct_map_field");
}

#[test]
fn probe_derived_eq_used_option_str_payload_no_leak() {
    // Sum-payload-with-heap-field derived-`Eq`: an `Option<str>` field compared
    // through `a == b` / `a != c`. The `[InlineEnum]` aggregate's heap payload
    // leaks via the comparison-operand spurious incs.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_derived_eq_used_option_str_payload_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "derived_eq_used_option_str_payload");
}

#[test]
fn probe_derived_clone_used_struct_str_field_no_leak() {
    // A `#derive(Eq, Clone)` struct is cloned and compared `a == b`, with the result read on the
    // then-branch. The compared aggregate flows through the same comparison-operand
    // keep-alive divergence as f13; the str field leaks via the spurious operand
    // inc unless M3+M4 fire.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_derived_clone_used_struct_str_field_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "derived_clone_used_struct_str_field");
}

#[test]
fn probe_derived_clone_subset_move_releases_unread_heap_field() {
    // A call-produced derived-Clone result is a constructless positional
    // aggregate. Moving one heap field out must skip that field at the
    // container release while still releasing the unread heap sibling.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_derived_clone_subset_move_releases_unread_heap_field.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "derived_clone_subset_move");
}

#[test]
fn probe_config_projected_fields_compared_keep_negative() {
    // POSITIVE PIN (the multi-borrow-view-alias surplus cure): a `Config {
    // settings, name }` whose fields are borrow-read through DISTINCT whole-var
    // aliases (`%6 = c` -> `Project %6.settings` -> `.length()`; `%11 = c` ->
    // `Project %11.name` -> `.length()`). The aggregate `%4`, its Let-Var aliases
    // `%6`/`%11`, are the SAME allocation; the base walk emits a surplus whole-var
    // `BurdenDec` at EACH alias's borrow-project AND a spurious keep-alive FRESH
    // inc — N+1 releases of ONE allocation (cleanup-on leaks both heap fields;
    // cleanup-off double-frees). The Phase-5 multi-borrow-view-alias arm suppresses
    // each alias's surplus dec + the keep-alive inc, leaving the owner's single
    // edge-cleanup release (RL-2 `RL2_release_exactly_once`). There are NO
    // `==`/`!=` comparison operands on the aggregate, so the comparison-operand
    // strip MUST NOT fire. Spec: Annex E §AIMS RL-2 + TF-4 + DP-3.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_config_projected_fields_compared_keep_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "config_projected_fields_compared");
}

#[test]
fn probe_derived_eq_single_comparison_keep_negative() {
    // NEGATIVE (single-comparison no-over-strip clamp): a derived-`Eq` struct
    // compared EXACTLY ONCE (`a == b`), no branch re-compare. `a` is used once at
    // the comparison, so it is NOT a multi-use dup_alias source -> no spurious
    // keep-alive inc -> nothing to strip. Passes pre AND post; the cure must not
    // touch the single-comparison shape (would double-free if it stripped the
    // genuine release).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_derived_eq_single_comparison_keep_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "derived_eq_single_comparison");
}

#[test]
fn probe_heap_str_clone_then_double_compare_freed() {
    // A heap `str` cloned and double-compared (`a == b && a == "literal"`)
    // increments the same buffer, while its result has a distinct
    // `same_alloc` rep (an Invoke result, not a Let-Var alias), so each `==`
    // compares operands of DISTINCT allocations. Each operand is an RL-1
    // borrow-read (`incElidable`). Comparison-operand stripping must leave both
    // the buffer and fresh literal balanced (Spec: Annex E §AIMS RL-1 + RL-2).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_heap_str_clone_then_double_compare_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "heap_str_clone_then_double_compare");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_heap_str_same_root_multi_compare_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "heap_str_same_root_multi_compare");
}

#[test]
fn probe_heap_str_same_root_three_compare_no_double_free_negative() {
    // Three already-balanced same-root `==` results
    // (`r1 = a==b; r2 = b==c; r3 = a==c`) where `b`/`c` alias `a` -> one
    // `same_alloc` rep, three same-root comparisons. The widened strip MUST leave
    // every same-root comparison untouched.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_heap_str_same_root_three_compare_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "heap_str_same_root_three_compare");
}

#[test]
fn probe_heap_str_single_compare_no_double_free_negative() {
    // NEGATIVE (single distinct-root compare balanced): two independent equal heap
    // strings compared exactly once (`a == b`, distinct allocations, returns 0).
    // Each operand is used once -> no spurious keep-alive inc; the per-operand
    // burden dec nets each allocation to 0. The cure must not over-strip a genuine
    // single release. Passes pre AND post the cure.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_heap_str_single_compare_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "heap_str_single_compare");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_sharing_view_list_slice_then_length_no_uaf.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "sharing_view_list_slice_then_length");
}

#[test]
fn probe_sharing_view_list_slice_branchy_multi_read_no_uaf() {
    // Same seamless-slice receiver-before-Apply UAF, but the slice RESULT is read
    // across MULTIPLE `&&`-short-circuit branches (`ys.length()`, `ys.first()`,
    // `ys.last()`). The receiver dies at the slice site (only the result flows
    // onward); its dec belongs at the borrowed read, not split across edges.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_sharing_view_list_slice_branchy_multi_read_no_uaf.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "sharing_view_list_slice_branchy_multi_read");
}

#[test]
fn probe_sharing_view_list_take_dead_receiver_no_uaf() {
    // `take` is a seamless-slice producer sharing the receiver buffer. Dead
    // receiver after the take; result read once.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_sharing_view_list_take_dead_receiver_no_uaf.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "sharing_view_list_take_dead_receiver");
}

#[test]
fn probe_sharing_view_list_drop_dead_receiver_no_uaf() {
    // `drop` is a seamless-slice producer sharing the receiver buffer. Dead
    // receiver after the drop; result read once.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_sharing_view_list_drop_dead_receiver_no_uaf.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "sharing_view_list_drop_dead_receiver");
}

#[test]
fn probe_sharing_view_str_substring_then_transform_no_uaf() {
    // `substring` is a seamless-slice producer sharing the str backing. Dead
    // receiver `s` after the substring; the result `sub` flows into a transform
    // (`to_uppercase`) that reads the shared backing.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_sharing_view_str_substring_then_transform_no_uaf.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "sharing_view_str_substring_then_transform");
}

#[test]
fn probe_sharing_view_non_sharing_borrowed_read_keep_negative() {
    // A plain borrowed scalar read (`@length`) creates no sharing view, so its
    // receiver release remains at the burden-walk position.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_sharing_view_non_sharing_borrowed_read_keep_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "sharing_view_non_sharing_borrowed_read");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_user_call_fresh_list_result_dup_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "user_call_fresh_list_result_dup_read");
}

#[test]
fn probe_user_call_fresh_list_result_single_read_freed() {
    // A fresh collection result read once and then dead requires one release
    // (Spec: Annex E §AIMS RL-2).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_user_call_fresh_list_result_single_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "user_call_fresh_list_result_single_read");
}

#[test]
fn probe_user_call_fresh_map_result_dup_read_freed() {
    // Type-dimension matrix cell: the fresh user-call result is a `{int: int}`
    // map (FatPointer/RcPtr collection), built and returned by a user function,
    // dup-read then dead. Same alloc-aware-net surplus-inc leak.
    // Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_user_call_fresh_map_result_dup_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "user_call_fresh_map_result_dup_read");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_user_call_returns_borrowed_slice_view_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "user_call_returns_borrowed_slice_view");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_user_call_fresh_recursive_enum_result_dup_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "user_call_fresh_recursive_enum_result_dup_read");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_inline_construct_recursive_enum_dup_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "inline_construct_recursive_enum_dup_read");
}

#[test]
fn probe_recursive_struct_payload_enum_dup_read_freed() {
    // Type-dimension matrix cell: a recursive enum whose variant payload is a
    // STRUCT (`Branch(node: TreeNode)` where TreeNode holds the recursive children).
    // The recursive enum self-allocates per node; dup-read then dead. Same boxed
    // single-release accounting as the flat recursive enum. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_recursive_struct_payload_enum_dup_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "recursive_struct_payload_enum_dup_read");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_inline_struct_multi_heap_field_projected_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "inline_struct_multi_heap_field_projected");
}

#[test]
fn probe_user_call_returns_recursive_enum_no_double_free_negative() {
    // NEGATIVE clamp: a user function RETURNS a fresh recursive aggregate, and the
    // CALLER returns it onward (the aggregate is an RL-2 transfer, the outer caller
    // inherits the release). The cure MUST NOT emit a freeing dec on a returned
    // aggregate -> double-free. The `compute_returned_lineages` exclusion must hold
    // for the new aggregate candidate class. Built + read once + returned: freed
    // exactly once by main's consumer. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_user_call_returns_recursive_enum_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "user_call_returns_recursive_enum");
}

#[test]
fn probe_scalar_only_struct_dup_read_no_extra_release_negative() {
    // NEGATIVE clamp: a SCALAR-only struct (`{ x: int, y: int }`) holds no heap
    // field -> `classify_triviality == Trivial`, so `is_burden_carrying_aggregate`
    // is false. The cure MUST NOT recognise it as a fresh-owned aggregate and emit
    // a spurious `RcDec` (it has no heap to free; a dec would be an RC op on non-RC
    // memory). Built + dup-read: no RC at all. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_scalar_only_struct_dup_read_no_extra_release_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "scalar_only_struct_dup_read");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_user_call_fresh_str_result_dup_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "user_call_fresh_str_result_dup_read");
}

#[test]
fn probe_user_call_fresh_str_result_single_read_freed() {
    // A fresh owned string result read once needs no keep-alive `RcInc`; its
    // move-alias decrement is the sole release, and allocation-aware accounting
    // must not add another (Spec: Annex E §AIMS RL-2).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_user_call_fresh_str_result_single_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "user_call_fresh_str_result_single_read");
}

#[test]
fn probe_derive_debug_str_result_dup_read_freed() {
    // Type-dimension matrix cell: the fresh user-call str result is a derived
    // `@debug()` return (a non-builtin method synthesising a fresh quoted `str`).
    // Two `.contains()` reads must leave the dead result string balanced
    // (Spec: Annex E §AIMS RL-2).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_derive_debug_str_result_dup_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "derive_debug_str_result_dup_read");
}

#[test]
fn probe_user_call_returns_str_no_double_free_negative() {
    // NEGATIVE clamp: a user function returns a fresh str, and the CALLER returns
    // it onward (the str is an RL-2 transfer; the outer caller inherits the
    // release). The cure MUST NOT emit a freeing dec on a returned str -> double-
    // free. The `compute_returned_lineages` exclusion must hold for the new
    // str-result candidate. Built + read once + returned: freed exactly once by
    // main's consumer. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_user_call_returns_str_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "user_call_returns_str");
}

#[test]
fn probe_str_arg_to_user_call_no_double_free_negative() {
    // NEGATIVE clamp: a fresh str is passed as an OWNED arg to a user function (the
    // callee's concern, an RL-2 transfer at the call site). The cure recognises
    // fresh-owned-str RESULTS, not str ARGS; `compute_user_call_arg_lineages`
    // already excludes a str arg (it considers FatValue args). A freeing dec on the
    // arg lineage would double-free the transferred str. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_str_arg_to_user_call_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "str_arg_to_user_call");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_slice_element_into_struct_field_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "slice_element_into_struct_field");
}

#[test]
fn probe_slice_element_into_option_field_no_double_free() {
    // POSITIVE (Option-wrapped slice field): the slice element is wrapped in
    // `Some(p)` and stored as the `name: Option<str>` field. The aggregate drop
    // walks the Option payload (the slice) and decs the shared backing; the
    // RL-1 keep-alive on the slice element must balance it. Spec: Annex E §AIMS
    // RL-1 + RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_slice_element_into_option_field_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "slice_element_into_option_field");
}

#[test]
fn probe_slice_element_into_tuple_field_no_double_free() {
    // POSITIVE (tuple-wrapped slice field): the slice element is the first
    // element of a `(str, int)` tuple stored as the `data` field. The aggregate
    // drop walks the tuple's str element (the slice) and decs the shared
    // backing; the RL-1 keep-alive must balance it. Spec: Annex E §AIMS RL-1 +
    // RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_slice_element_into_tuple_field_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "slice_element_into_tuple_field");
}

#[test]
fn probe_slice_element_into_result_field_no_double_free() {
    // POSITIVE (Result-wrapped slice field): the slice element is wrapped in
    // `Ok(p)` / `Err(p)` and stored as a `Result<str, str>` field. Each Result
    // variant's str payload is the shared slice; the aggregate drop decs it and
    // the RL-1 keep-alive must balance. Spec: Annex E §AIMS RL-1 + RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_slice_element_into_result_field_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "slice_element_into_result_field");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_owned_collection_field_into_struct_negative_no_extra_release.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "owned_collection_field_into_struct_negative");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_slice_element_scalar_use_only_negative_no_extra_release.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "slice_element_scalar_use_only_negative");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_str_local_dup_read_borrowed_user_call_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "str_local_dup_read_borrowed_user_call");
}

#[test]
fn probe_str_local_dup_read_via_higher_order_freed() {
    // POSITIVE (same mechanism, indirect): a fresh local `str` borrowed-read twice
    // through a higher-order forwarder (`@apply(f, s: str)` — `s` Borrowed). The
    // str flows to two user-call Borrowed positions, dead, not returned. Same
    // alloc-aware net +1 leak as the direct shape. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_str_local_dup_read_via_higher_order_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "str_local_dup_read_via_higher_order");
}

#[test]
fn probe_str_single_read_borrowed_user_call_no_double_free_negative() {
    // NEGATIVE clamp (single-use boundary): a fresh local `str` borrowed-read
    // ONCE at a user-call position, then dead. Single-use nets 0 (the lone
    // borrowed read's release already balances the alloc) — the un-exclusion MUST
    // NOT add a second release (double-free). Pins that the alloc-aware net, not a
    // structural "is borrowed str" proxy, gates the release. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_str_single_read_borrowed_user_call_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "str_single_read_borrowed_user_call_negative");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_str_returned_from_user_call_chain_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "str_returned_from_user_call_chain_negative");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_collection_borrowed_to_user_call_single_read_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_borrowed_to_user_call_dup_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "list_borrowed_to_user_call_dup_read");
}

#[test]
fn probe_list_borrowed_recursive_borrow_read_dup_read_freed() {
    // POSITIVE (recursive borrow-read forwarder): a fresh local `[int]` passed to a
    // recursive borrow-read callee (`@sum_recursive(xs, idx)` reads `xs.length()` +
    // `xs[idx]` and forwards `xs` to a recursive call where it is ALSO borrow-read).
    // The param flows only to borrowed positions across the recursion — the
    // `borrowed_read_only` contract fact stays true through SCC-propagated
    // forwarding. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_borrowed_recursive_borrow_read_dup_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "list_borrowed_recursive_borrow_read_dup_read");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_chained_curried_closure_str_capture_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "chained_curried_closure_str_capture");
}

#[test]
fn probe_chained_curried_closure_list_capture_freed() {
    // POSITIVE (heap-list capture in chained-curried closure): same anonymous-chained
    // closure-intermediate leak with a `[int]` capture instead of `str`. The closure
    // env carries the captured list's RC; the missing closure-value last-use dec
    // leaks the env (and the captured list reachable through it). Spec: Annex E §AIMS
    // RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_chained_curried_closure_list_capture_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "chained_curried_closure_list_capture");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_call_returned_closure_invoked_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "call_returned_closure_invoked");
}

#[test]
fn probe_closure_transferred_into_struct_no_double_free_negative() {
    // NEGATIVE (the over-fire boundary — transferred closure): a closure stored as a
    // struct field is TRANSFERRED (the `Construct Struct` owned arg) — the struct's
    // scope-exit `RcDec [AggFields]` walks the closure field and frees its env. The
    // closure-value scope-exit dec MUST NOT also fire here, or the env double-frees.
    // The transferred-out gate (Construct owned-arg = transfer) excludes it. PASS
    // pre AND post cure.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_closure_transferred_into_struct_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "closure_transferred_into_struct_no_double_free");
}

#[test]
fn probe_let_bound_closure_single_invoke_no_double_free_negative() {
    // NEGATIVE (the already-balanced boundary — let-bound closure): a closure bound to
    // a `let` and invoked once ALREADY receives its scope-exit `BurdenDec` from the
    // base burden walk (the let-bound lineage carries a dec). The closure-value
    // scope-exit pass MUST NOT add a SECOND dec on a lineage that already has one, or
    // the env double-frees. The existing-dec gate (skip lineages already carrying a
    // BurdenDec) excludes it. PASS pre AND post cure.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_let_bound_closure_single_invoke_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "let_bound_closure_single_invoke_no_double_free");
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
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_fresh_heap_str_dead_on_question_early_exit_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "fresh_heap_str_dead_on_question_early_exit");
}

#[test]
fn probe_fresh_heap_str_dead_on_explicit_branch_freed() {
    // POSITIVE (fresh heap value dead on one explicit `if` branch): `tag` is a
    // fresh heap str used only inside the `then` branch (`tag.length()`); the
    // `else` branch leaves `tag` dead WITHOUT a release on that edge. Same RL-4
    // edge-cleanup as the `?`-exit shape, via a plain `if/else` split rather than
    // `?`-desugar. Spec: Annex E §AIMS RL-4.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_fresh_heap_str_dead_on_explicit_branch_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "fresh_heap_str_dead_on_explicit_branch");
}

#[test]
fn probe_fresh_heap_str_returned_on_early_exit_no_double_free_negative() {
    // NEGATIVE (the over-fire boundary — fresh value TRANSFERRED on the dead-looking
    // branch): `name` is RETURNED on the `b`-true branch (an RL-2 ownership transfer
    // — the caller releases it). The branch-dead-value edge-dec MUST NOT fire on
    // that branch, or `name` double-frees against the caller's release. The
    // transferred-out (Return / Construct-arg) guard excludes it. PASS pre AND post
    // cure. Spec: Annex E §AIMS RL-2 + RL-4.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_fresh_heap_str_returned_on_early_exit_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "fresh_heap_str_returned_on_early_exit_no_double_free",
    );
}

// A heap value (str / list) moved into an INLINE SUM VARIANT, where the fresh
// variant is then passed BORROWED to a user callee that borrow-reads it and is
// DEAD afterward, leaks the moved-in heap field under sole-emitter lowering. The
// Phase-5 walk emits a matched `BurdenInc v; BurdenDec v` pair on the variant
// BEFORE the borrowed call; the coalesce peephole cancels the adjacent pair to
// net-0, so no scope-exit `RcDec [InlineEnum]` survives — the variant's
// drop-glue (which would walk the heap field) never runs. RL-2 mandates the
// single scope-exit release of a fresh owned aggregate whose last use is a
// borrowed call (`RL2_release_exactly_once` + the borrowed-call last use is a
// non-transfer kind, `rl2_emits_dec(.LastReadBeforeScopeExit)`); the moved-in
// field is an RL-2 `ConstructArg` transfer INTO the variant, so the variant's
// own drop is the field's sole release. Spec: Annex E §AIMS RL-2 + RL-4.

#[test]
fn probe_heap_str_into_sum_variant_borrow_read_freed() {
    // A heap `str` (> 23-byte SSO threshold) moved into
    // `Named(description: heap, count: int)`, the variant borrowed-read by
    // `desc_len` and dead afterward. The moved-in str field must be freed at the
    // variant's scope-exit drop.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_heap_str_into_sum_variant_borrow_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "heap_str_into_sum_variant_borrow_read_freed");
}

#[test]
fn probe_heap_list_into_sum_variant_borrow_read_freed() {
    // A heap `[int]` list moved into `Items(list)`, the
    // variant borrowed-read by `get_size` and dead afterward. The moved-in list
    // backing buffer must be freed at the variant's scope-exit drop.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_heap_list_into_sum_variant_borrow_read_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "heap_list_into_sum_variant_borrow_read_freed");
}

#[test]
fn probe_call_result_variant_borrow_read_no_double_free_negative() {
    // CRITICAL NEGATIVE (the over-fire boundary — variant from a CALL RESULT,
    // already balanced): the variant is the RESULT of `make()` (an owned transfer
    // to the caller via RL-2 Return), then borrowed-read by `desc_len` and dead.
    // The burden path ALREADY frees this lineage at scope exit under flag — the
    // sum-variant edge-release MUST NOT also fire, or the str double-frees. The
    // alloc-aware net (already-balanced, net 0) excludes it. PASS pre AND post
    // cure. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_call_result_variant_borrow_read_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "call_result_variant_borrow_read_no_double_free");
}

#[test]
fn probe_value_variant_into_sum_no_spurious_dec_negative() {
    // NEGATIVE (the Value-variant boundary): a variant carrying ONLY scalar fields
    // (`Pair(a: int, b: int)`) is NOT burden-carrying (triviality Trivial — no heap
    // field). The scope-exit edge-release MUST NOT fire (there is no field to free;
    // a spurious `RcDec [InlineEnum]` on a scalar-only inline enum is unsound). The
    // `is_burden_carrying_aggregate` gate excludes it. PASS pre AND post cure.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_value_variant_into_sum_no_spurious_dec_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "value_variant_into_sum_no_spurious_dec");
}

#[test]
fn probe_heap_str_into_sum_variant_already_balanced_sibling() {
    // A heap string bound directly, borrowed, and then dead is already balanced.
    // Sum-variant edge release must ignore a lineage with no variant wrapper.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_heap_str_into_sum_variant_already_balanced_sibling.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "heap_str_into_sum_variant_already_balanced_sibling",
    );
}

// A self-allocating collection-SOURCE Apply result (`.collect()` / `.collect()`
// into a Set / set-algebra `.union()`) borrowed-read across MULTIPLE branches and
// dead after leaks the result buffer under sole-emitter lowering: Phase-5
// `fresh_site_burden_inc_dst` emits a fresh-site `BurdenInc` on EVERY `Apply`
// result with a Unique/MaybeShared return contract, treating it as a
// caller-acquires-owned-reference. But `@collect`/`@collect_set`/`@union` are
// SELF-allocating (the runtime allocates a fresh rc=1 buffer distinct from any
// operand), so under Phase-7 lowering that fresh inc is the M1 over-count:
// `alloc(+1) + RcInc − RcDec = +1` -> LEAK. RL-1 (`RL1_emit_iff_not_elidable`):
// a non-duplicated FRESH value's single use is move-once-linear -> inc elidable
// -> NO fresh inc; the alloc IS the +1, one dec frees it. The M1 alloc-aware-net
// elision drops the spurious fresh inc once the collection-source result is
// recognized as a fresh self-alloc. Spec: Annex E §AIMS RL-1 + RL-2.

#[test]
fn probe_collect_set_result_multibranch_dead_freed() {
    // POSITIVE (the rc_matrix/narrowing `test_set_int_operations_canonical` shape):
    // `[..].iter().collect()` into a `Set<int>`, borrow-read by `.contains()` across
    // a chain of `if/else if` branches, dead after the last read. The fresh-site
    // `BurdenInc` on the `@collect_set` result is the M1 over-count -> the result
    // buffer leaks. The alloc-aware-net elision must drop it. Spec: Annex E §AIMS RL-1.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_collect_set_result_multibranch_dead_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "collect_set_result_multibranch_dead");
}

#[test]
fn probe_collect_list_result_multibranch_dead_freed() {
    // POSITIVE (the `narrowing::test_iter_map_on_narrowed_int_list` shape): a
    // `.iter().map(..).collect()` list result borrow-read across `if/else if`
    // branches and dead after. Same M1 fresh-inc over-count on the `@collect`
    // result. Spec: Annex E §AIMS RL-1.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_collect_list_result_multibranch_dead_freed.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "collect_list_result_multibranch_dead");
}

#[test]
fn probe_collect_result_duplicated_not_double_freed_negative() {
    // CRITICAL NEGATIVE (the over-fire boundary): a `.collect()` result that is
    // DUPLICATED (`let b = a; both read`) has a LOAD-BEARING fresh inc — the second
    // alias needs the +1 so each alias's dec balances. The alloc-aware net of the
    // duplicated lineage is NOT +1 (the dup-alias dec balances the fresh inc), so
    // the elision MUST NOT fire; eliding here would net -1 -> DOUBLE-FREE. The
    // `net == 1` gate excludes it. PASS pre AND post cure. Spec: Annex E §AIMS RL-1.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_collect_result_duplicated_not_double_freed_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "collect_result_duplicated_not_double_freed");
}

#[test]
fn probe_collect_result_straightline_dead_already_balanced_sibling() {
    // A `.collect()` result read once on a straight-line path is already net
    // zero; fresh-inc elision must preserve its single release (Spec: Annex E
    // §AIMS RL-2).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_collect_result_straightline_dead_already_balanced_sibling.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "collect_result_straightline_dead_already_balanced_sibling",
    );
}

// M-a callee transfer-source-dec strip — a param that flows to a `Return`
// terminator while ALSO used multiple times across blocks. Per AIMS RL-2 the
// `Return` terminal use transfers ownership back to the caller, so the callee
// MUST NOT emit a scope-exit `BurdenDec` on the param (it would double-release
// the allocation handed back through the return). The structural move-alias
// scan conservatively keeps the dec for a multi-block-used param; the function's
// own `MemoryContract.transfers_through_return` carries the proven Return-flow
// fact precisely. Matrix: element-type axis x multi-block-use scenario. Spec:
// Annex E §AIMS RL-2 (`RL2_transfer_kinds_no_dec` for `Return`).

#[test]
fn probe_transfer_through_return_param_list_int_multi_use() {
    // [int] param: two borrow uses across CFG (a print + an index) then return.
    // The interpolation splits the body across unwind-edge blocks, so the param's
    // terminal move to the Return is multi-block — the contract strip is the cure.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_transfer_through_return_param_list_int_multi_use.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "transfer_through_return_param_list_int_multi_use",
    );
}

#[test]
fn probe_transfer_through_return_param_str_multi_use() {
    // str param: borrow uses across CFG then return — the str fat-value variant
    // of the same multi-block transfer-through-return shape.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_transfer_through_return_param_str_multi_use.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "transfer_through_return_param_str_multi_use");
}

#[test]
fn probe_transfer_through_return_param_list_str_multi_use() {
    // [str] param (heap elements): exercises elem_dec_fn on the returned buffer —
    // a spurious callee dec would double-free the element buffer, not just the
    // outer fat pointer.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_transfer_through_return_param_list_str_multi_use.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "transfer_through_return_param_list_str_multi_use",
    );
}

#[test]
fn probe_transfer_through_return_param_match_arm_multi_use() {
    // Pattern-dispatch path: the param is used in a `match` arm then returned —
    // the terminal move crosses Maranget decision-tree blocks, so the param is
    // multi-block-used and the contract strip is required.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_transfer_through_return_param_match_arm_multi_use.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "transfer_through_return_param_match_arm_multi_use",
    );
}

#[test]
fn probe_transfer_through_return_param_borrowed_does_not_leak_negative() {
    // NEGATIVE PIN (the over-strip boundary): a param that is borrow-READ but NOT
    // returned must KEEP its callee scope-exit release — `transfers_through_return`
    // is FALSE, so the contract strip MUST NOT fire. Stripping the dec here would
    // LEAK the param's allocation (the callee owns the only reference at scope
    // exit). The function returns a scalar derived from the param, never the param
    // itself. Spec: Annex E §AIMS RL-2 (non-transfer terminal use -> dec).
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_transfer_through_return_param_borrowed_does_not_leak_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "transfer_through_return_param_borrowed_does_not_leak_negative",
    );
}

#[test]
fn probe_aggregate_transfer_forwarder_box_no_double_free() {
    // POSITIVE PIN: Generic forwarders preserve transfer-through-return for aggregate results.
    // an owned-transfer-through-return forwarder (`@id<T>(x: T) -> T = x`) over an
    // AGGREGATE result (`Box<[int]>`, a heap struct wrapping a heap [int]) — the
    // Phase-5 walk KEEPS the spurious apply-result inc for an Aggregate result
    // (`compute_transfer_through_return_results` repr-gates suppression to
    // RcPtr/FatVal), and the Aggregate `Construct` is NOT `fresh_rc_alloc_dst`-
    // recognized, so the lineage carries an unbalanced spurious inc + orphan
    // alias decs. The Phase-6 lineage re-balance anchors the `+1` at the forwarder
    // transfer point (where the caller acquires the transferred-in allocation at
    // the Invoke result), elides ALL incs + keeps exactly one POST-transfer
    // release (RL-1 spurious-inc + RL-2 release-exactly-once + RL-34 transfer).
    // Spec: Annex E §AIMS RL-1 + RL-2 + RL-34.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_aggregate_transfer_forwarder_box_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "aggregate_transfer_forwarder_box");
}

#[test]
fn probe_aggregate_transfer_forwarder_option_no_double_free() {
    // POSITIVE PIN (the forwarder dead-block-param leak class, TWO layered cures):
    // an owned-transfer forwarder (`@id<T>(x: T) -> T = x`) over a SUM-type
    // Aggregate (`Option<[int]>`) whose payload carries the heap [int]. On the
    // UNPRUNED shape the forwarder identity (`%4` source = `%7` result) reaches the
    // post-match merge/return block as TWO DEAD block-params (`Cardinality =
    // Absent`) via `Jump bb(.., %4, %7)`; the Jump-arg -> Owned-param handoff (RL-4
    // exemption) defers the source's release to those dead params -> the [int]
    // buffer leaks unless the RL-5 dead-at-entry release
    // (`RL5_dead_at_entry_cleanup`) fires, deduped by forwarder identity (one dec
    // per allocation, two would double-free). At DEFAULT, match-merge mutable-param
    // pruning removes the never-reassigned bindings from the merge signature
    // first, dissolving the dead-param shape before the RL-5 scan runs — the four
    // four assertions pin every cell of that pruning x RL-5 matrix.
    // Spec: Annex E §AIMS RL-5 + RL-4 + RL-34.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_aggregate_transfer_forwarder_option_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "aggregate_transfer_forwarder_option");
}

#[test]
fn probe_borrowed_forwarder_rcptr_not_rebalanced_no_corruption() {
    // An owned-through-return forwarder with an RcPtr result has no apply-result
    // increment. Aggregate-only rebalancing must exclude this lineage or it may
    // retain the wrong release and corrupt the returned list
    // (the value read back is wrong → exit 1). Verifies the RcPtr forwarder still
    // runs correctly (exit 0, no corruption) under the compiled-counter path. Spec:
    // Annex E §AIMS RL-1 + RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_borrowed_forwarder_rcptr_not_rebalanced_no_corruption.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "borrowed_forwarder_rcptr_not_rebalanced");
}

#[test]
fn probe_construct_fed_dead_param_for_yield_option_str_no_leak() {
    // POSITIVE PIN (the construct-fed dead-param lineage cure): a `for x in
    // Some(str) yield { break }` — the `Option<str>` aggregate (`%1 = Construct
    // Variant(Option.0)(%0)`, threaded via the Let-Var alias `%3 = %1`) reaches the
    // `ori_list_take` exit block as a DEAD block-param (`%7: str?`) via `Jump`. The
    // base walk OVER-emits: a FRESH-site `BurdenInc` on the Construct + a dup-alias
    // `BurdenInc` on `%3` (the `use_counts >= 2` cardinality proxy mis-classes the
    // same-alloc alias as a duplication — `%1` is "live" only because it ALSO feeds
    // `Jump bb3(%1)`) + a misplaced alias release, netting +1 (the str backing
    // leaks). The cure suppresses the whole lineage (both incs + the misplaced dec)
    // + its heap-element borrow-views, and emits EXACTLY ONE RL-5 dead-at-entry
    // release at `%7` (no-op on None, frees the heap str on Some). RL-2
    // release-exactly-once + RL-5 dead-at-entry + RL-4 Jump-arg exemption. Spec:
    // Annex E §AIMS RL-5 + RL-4 + RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_construct_fed_dead_param_for_yield_option_str_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "construct_fed_dead_param_for_yield_option_str");
}

#[test]
fn probe_construct_fed_dead_param_conditional_body_no_uaf() {
    // POSITIVE PIN (the UAF-avoidance clamp): a `for x in Some(str) yield { if
    // x.len() > 10 then break; x.len() }` — the body BORROWS the str element view
    // (`%11 = Project %3.1`, `x.len()`) on a path that MAY break early. The cure's
    // single release at the dead exit-block param `%7` runs AFTER every borrow-view
    // use (the param is dead at the merge, all body uses precede it), so the str
    // backing is released exactly once with no use-after-free. Stripping the
    // lineage incs WITHOUT relocating the release to the post-body dead param would
    // surface a live UAF (exit 139) — this pin proves the relocation avoids it
    // (exit 0, no double-free). Spec: Annex E §AIMS RL-2 + RL-5.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_construct_fed_dead_param_conditional_body_no_uaf.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "construct_fed_dead_param_conditional_body");
}

#[test]
fn probe_construct_fed_forwarder_option_disjoint_no_double_free() {
    // NEGATIVE / over-fire boundary PIN (the disjointness gate): a SUM-aggregate
    // `Construct` (`Some([int])`) that is ALSO forwarded through `@id<T>(x) -> T = x`
    // is owned by the FORWARDER dead-param pass (`compute_dead_forwarder_block_param_releases`,
    // which KEEPS the keep-alive inc + adds the dead-param dec — net 0 for a
    // transferred-in allocation whose `+1` came from the forwarded arg). The
    // construct-fed pass MUST NOT also fire here: its Part-B suppression would strip
    // that keep-alive inc and double-free. Verifies the `is_forwarder_rep` exclusion
    // keeps the forwarder lineage correct (exit 0, no double-free) on BOTH paths —
    // the construct-fed pass and the forwarder pass target DISJOINT reps. Spec:
    // Annex E §AIMS RL-5 + RL-34.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_construct_fed_forwarder_option_disjoint_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "construct_fed_forwarder_option_disjoint");
}

#[test]
fn probe_nested_construct_fed_dead_param_option_recursive_node_no_leak() {
    // POSITIVE PIN (burden self-sufficiency for the recursive-Node match-scalar
    // shape): a recursive `Node { value: int, next: Option<Node> }` matched out of
    // an `Option<Node>` reading only the scalar `.value`. The `match ... node.value`
    // lowers to `Switch %14` on the scalar tag projection plus an inline
    // `Project %12.1` payload extract — NOT a Jump-arg-to-dead-block-param shape, so
    // NO dead merge-block param exists and the construct-fed dead-param scan
    // (`compute_construct_fed_dead_param_lineage`) is INERT here. The base burden
    // walk balances the two-node tree on its own: `transfer_via_move_alias`
    // suppresses the per-Construct-link source decs and emits exactly one inline
    // `burden_dec` for the outer aggregate (`%12`, which transitively frees the
    // whole owning Construct chain) plus one for the projected inner Node (`%16`) —
    // RL-2 release-exactly-once, verified leak-free / double-free-free under the
    // verification env (2 allocs → 2 frees, live=0). Spec: Annex E §AIMS
    // RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_nested_construct_fed_dead_param_option_recursive_node_no_leak.ori"
    );
    // No mutation-verify against `ORI_DISABLE_CONSTRUCT_FED_DEAD_PARAM_RELEASE`: that
    // scan never fires for this `Switch`+inline-`Project` lowering, so toggling it
    // off changes nothing — the base walk owns the balance. The genuine
    // construct-fed dead-param mutation-verify lives on
    // `probe_construct_fed_dead_param_for_yield_option_str` (a Jump-arg-to-dead-param
    // lowering the scan actually cures).
    assert_runs_clean_no_leak_or_double_free(
        src,
        "nested_construct_fed_dead_param_option_recursive_node",
    );
}

#[test]
fn probe_nested_construct_fed_dead_param_result_recursive_node_no_leak() {
    // The recursive-`Node` match-scalar shape is matched out of a
    // `Result<Node, str>` instead of an
    // `Option<Node>`. The `Ok(node) -> node.value` arm lowers to the identical
    // `Switch`+inline-`Project` form (no dead block-param), so the construct-fed
    // scan is INERT and the base burden walk owns the two-node-tree balance —
    // RL-2 release-exactly-once, verified under the verification env. No
    // mutation-verify (the scan never fires for this shape). Spec: Annex E §AIMS
    // RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_nested_construct_fed_dead_param_result_recursive_node_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "nested_construct_fed_dead_param_result_recursive_node",
    );
}

#[test]
fn probe_nested_construct_payload_extracted_live_no_double_free_negative() {
    // NEGATIVE / over-fire boundary PIN (the `released_payload_escapes_live`
    // precondition): the recursive `Node` is EXTRACTED and RETURNED (`Some(node) ->
    // node`) rather than scalar-read (`node.value`). The extracted heap Node holds a
    // LIVE reference to the same allocation as the nested Construct, with its OWN
    // release. The nested-lineage suppression MUST NOT fire here: stripping the
    // nested Construct's keep-alive inc while BOTH the parent's dead-param dec AND
    // the live extract's release run would double-free (exit 134). The precondition
    // detects the heap payload reaching a live owned block-param via its `Project`
    // view closure and aborts the whole nested suppression. Verifies exit 0, no
    // double-free, on the compiled-counter path. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_nested_construct_payload_extracted_live_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "nested_construct_payload_extracted_live_no_double_free",
    );
}

#[test]
fn probe_double_wrap_niche_sum_cross_block_extract_no_double_free() {
    // POSITIVE PIN (cross-block enum-cascade dominating-extract, RL-2): a
    // `catch(catch(panic(..)))` double-wrap lowers to a chain of TRANSPARENT
    // niche-family sum wrappers over ONE leaf `str` allocation
    // (`Result<Result<never, str>, str>`). The outer-Ok match arm `Project`s the
    // inner `Result` (itself a niche-family sum, NOT a leaf) and the base walk
    // places the outer sum's CASCADE dec at that site — freeing the leaf `str`
    // in a block that STRICTLY DOMINATES the inner-Err successor that re-extracts
    // + retains the same leaf. The cure admits the DEPTH-≥2 niche-of-niche
    // projection web (the live-extract merge places ONE release for the whole
    // transparent nest; `RL2_release_exactly_once`). Verifies exit 0, no
    // double-free, no leak, on the compiled-counter path. Spec: Annex E §AIMS RL-2 +
    // TF-4.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_double_wrap_niche_sum_cross_block_extract_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "double_wrap_niche_sum_cross_block_extract");
}

#[test]
fn probe_single_wrap_niche_sum_extract_no_regression_negative() {
    // NEGATIVE / over-fire boundary PIN (the FLAT single-wrap shape the
    // depth-≥2 merge gate MUST NOT touch): a single `catch(panic(..))` lowers to
    // ONE niche-family sum wrapper (`Result<never, str>`) `Project`ed DIRECTLY to
    // a LEAF `str` (no nested niche projection). The base walk handles the flat
    // shape correctly (same-block extract + keep-alive); the depth-≥2 merge gate
    // (`has_nested_niche_projection`) is FALSE here, so the overlapping Ok/Err
    // candidate web keeps the gate-(g) decline (status quo). Firing the merge
    // here double-frees the leaf (the `match_alias::test_match_arm_alias_result_str`
    // over-fire boundary). Verifies exit 0, no double-free, on the compiled-counter
    // path. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_single_wrap_niche_sum_extract_no_regression_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "single_wrap_niche_sum_extract_no_regression");
}

#[test]
fn probe_owner_drop_deferred_past_borrow_view_intlist_field_no_uaf() {
    // TF-14 owner-drop liveness applies to a sole-owned-RC-field struct
    // (`Container { items: [int] }`) whose `[int]` field is projected
    // as a borrow-view (`let xs = c.items`) and read at a borrowed-`Invoke`
    // terminator arg (`xs.fold(..)`) AFTER the container's own syntactic last use.
    // The container's whole-var `RcDec [AggFields]` cascade-frees the field; placed
    // at the container's last use it frees the buffer before `fold` reads it.
    // The container release belongs at the borrowed call's normal-successor entry.
    // Spec: Annex E §AIMS TF-14 + RL-2 + RL-4.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_owner_drop_deferred_past_borrow_view_intlist_field_no_uaf.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "owner_drop_deferred_past_borrow_view_intlist_field",
    );
}

#[test]
fn probe_owner_drop_deferred_past_borrow_view_str_field_no_uaf() {
    // Matrix (str field): the same owner-drop-past-borrow-view shape with a `str`
    // owned field instead of `[int]` — the borrow-view `s` is read via a borrowed
    // call after the holder's syntactic last use. Spec: Annex E §AIMS TF-14 + RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_owner_drop_deferred_past_borrow_view_str_field_no_uaf.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "owner_drop_deferred_past_borrow_view_str_field");
}

#[test]
fn probe_terminal_concat_str_operand_no_leak() {
    // A fresh str literal consumed EXACTLY ONCE as the LHS operand of a `+`
    // concat in a match arm: `let $s = match x { Some(v) -> "lit_" + str(v), ..}`.
    // The concat helper BORROWS the operand; the caller's single dec frees it, so
    // the operand is move-once-linear (DP-3 `incElidable`) and its keep-alive
    // FRESH-site inc is surplus — without suppression the literal leaks (alloc
    // rc=1 + inc rc=2 - one dec rc=1). Spec: Annex E §AIMS RL-1 + RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_terminal_concat_str_operand_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "terminal_concat_str_operand");
}

#[test]
fn probe_terminal_concat_list_operand_no_leak() {
    // Matrix (list `+`): a fresh `[int]` literal consumed EXACTLY ONCE as a `+`
    // concat operand (`[1,2,3] + [4,5]`). The same move-once-linear surplus-inc
    // shape as the str variant (`ori_list_concat_cow` borrows the operand).
    // Spec: Annex E §AIMS RL-1 + RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_terminal_concat_list_operand_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "terminal_concat_list_operand");
}

#[test]
fn probe_concat_lhs_reread_after_concat_no_double_free() {
    // Negative clamp (the over-fire boundary the single-use gate protects): when
    // the concat LHS is RE-READ after the concat (`let $s = a + "x"; a.starts_with`),
    // the keep-alive inc is LOAD-BEARING — it raises rc >= 2 so `ori_str_concat`
    // COPIES instead of mutating `a` in place, preserving the later read. The
    // multi-use count excludes this shape from suppression; if the cure
    // over-fired here, `a` would be mutated-in-place and the later read would see
    // the concatenated value (wrong) or double-free. Spec: Annex E §AIMS RL-1.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_concat_lhs_reread_after_concat_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "concat_lhs_reread_after_concat");
}

#[test]
fn probe_heap_str_literal_slice_split_borrow_read_dead_no_leak() {
    // POSITIVE / heap str-literal receiver-lineage clamp: a HEAP string literal
    // (`base`, > SSO_MAX_LEN) borrow-read through a chain of borrowed-`Invoke`
    // args (`base.substring(..)` -> `.split(..)`: every long part is a seamless
    // slice co-owning `base`'s buffer) then DEAD at the normal-exit `Return`. The
    // base walk places `base`'s whole-var release on the dying unwind edges only,
    // NOT on the normal `Return` -> `base` leaks on the normal path
    // (`RL2_release_exactly_once`: every concrete path nets to 0; the normal path
    // nets +1). The borrowed-`Invoke` lineage scan admits the heap str-literal
    // root (gated on `owned_vars_needing_rc` membership = heap, not SSO) and
    // places EXACTLY ONE release after the closure's final borrow-read on the
    // normal exit. Spec: Annex E §AIMS RL-2 + RL-4.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_heap_str_literal_slice_split_borrow_read_dead_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "heap_str_literal_slice_split_borrow_read_dead");
}

#[test]
fn probe_heap_str_literal_returned_no_double_free_negative() {
    // NEGATIVE / heap str-literal OWNED-CONSUME clamp: a HEAP string literal
    // (`s`, > SSO_MAX_LEN) read once via a borrowed `@len` then RETURNED through a
    // branch. The `Return` is an OWNED-position consume (RL-2 transfer) — the
    // same-alloc vetting gate (d) of the borrowed-`Invoke` lineage scan MUST
    // DECLINE the str-literal root (any owned-position consume / `Return` declines).
    // Over-firing a normal-path dead-param release on a value the `Return`
    // transfers would double-free the returned allocation (`-134`). The
    // discriminator is the vetting gate: a member at an owned terminator operand
    // declines. Spec: Annex E §AIMS RL-2.
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_heap_str_literal_returned_no_double_free_negative.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "heap_str_literal_returned_negative");
}

// Set-collect fresh-allocation matrix: a `.iter().collect()` Set/List result
// flowing into an iter-consuming generic callee at a borrowed body-call arg.
// Spec: Annex E §AIMS RL-1 + RL-2; each matrix case pins one boundary.

/// A heap-element `Set` remains live across an iteration-consuming generic call.
/// The caller-owned release balances the surviving lineage without a UAF or
/// double free.
#[test]
fn probe_set_str_collect_read_after_iter_consuming_call_no_uaf() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_str_collect_read_after_iter_consuming_call_no_uaf.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "set_str_collect_read_after_iter_consuming_call");
}

/// Two iteration-consuming calls over a heap-element Set use multi-borrow
/// accounting; single-use suppression must decline.
#[test]
fn probe_set_str_collect_two_iter_consuming_calls_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_str_collect_two_iter_consuming_calls_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "set_str_collect_two_iter_consuming_calls");
}

/// Regression: iter-consuming callee that ALSO returns its param — ownership
/// transfers through the return, so the caller-side suppression must NOT fire
/// (the returned lineage carries the release).
#[test]
fn probe_set_str_collect_iter_consume_then_return_param_no_leak() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_str_collect_iter_consume_then_return_param_no_leak.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "set_str_collect_iter_consume_then_return_param");
}

/// Regression: heap-elem Set collect result DEAD at scope exit (no consuming
/// call) — the fresh-collect admission gives it exactly one scope-exit release
/// (no leak, no double-free).
#[test]
fn probe_set_str_collect_dead_at_scope_exit_freed_once() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_str_collect_dead_at_scope_exit_freed_once.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "set_str_collect_dead_at_scope_exit");
}

/// Regression: heap-elem Set collect result RETURNED from a helper — the
/// transferred lineage keeps its accounting (fresh-collect admission must not
/// strip a returned value's ops).
#[test]
fn probe_set_str_collect_returned_from_helper_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_str_collect_returned_from_helper_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "set_str_collect_returned_from_helper");
}

/// A heap-element `List` passes through an iteration-consuming generic callee;
/// its admission and caller-release accounting match the set path.
#[test]
fn probe_list_str_collect_iter_consuming_call_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_list_str_collect_iter_consuming_call_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(src, "list_str_collect_iter_consuming_call");
}

/// Regression: heap-elem Set collect borrowed at a MAY-UNWIND call to the
/// iter-consuming generic callee (`catch` wraps the call so it lowers to an
/// `Invoke` terminator arg) — pins the Invoke-terminator admission arm.
#[test]
fn probe_set_str_collect_invoke_terminator_iter_consuming_call_no_double_free() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_set_str_collect_invoke_terminator_iter_consuming_call_no_double_free.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "set_str_collect_invoke_terminator_iter_consuming_call",
    );
}

/// Regression: heap-key map built then borrowed-read through the `keys()`
/// conversion — clamps the narrowed `__collect_set` admission against touching
/// map-producing names (the conversion-result accounting must be unchanged).
#[test]
fn probe_map_str_keys_conversion_unchanged_by_collect_admission() {
    let src = include_str!(
        "fixtures/memory_lifecycle_probe/probe_map_str_keys_conversion_unchanged_by_collect_admission.ori"
    );
    assert_runs_clean_no_leak_or_double_free(
        src,
        "map_str_keys_conversion_unchanged_by_collect_admission",
    );
}
