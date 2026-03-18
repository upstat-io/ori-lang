---
section: "03"
title: "Remove Workarounds & Simplify"
status: not-started
goal: "Remove the phantom __for_coll binding and exit-block dummy reference, simplifying the ARC lowering"
depends_on: ["02"]
reviewed: false
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Remove Phantom Binding from lower_for"
    status: not-started
  - id: "03.2"
    title: "Remove Dummy Reference from lower_for_iterator"
    status: not-started
  - id: "03.2.5"
    title: "Remove Dead elem_dec_fn Parameter from Iterator API"
    status: not-started
  - id: "03.3"
    title: "Verify No Regressions"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Remove Workarounds & Simplify

**Status:** Not Started
**Goal:** With elem_dec_fn stored in the RC header, the ordering workarounds in the ARC lowering are no longer needed. Remove them to simplify the code.

**Depends on:** Section 02 (codegen must use header-based cleanup first).

---

## 03.1 Remove Phantom Binding from lower_for

**File:** `compiler/ori_arc/src/lower/control_flow/loops.rs`

The phantom `__for_coll` mutable binding was added to thread the collection through the loop header, forcing AIMS to add RcInc. With the header storing elem_dec_fn, the ordering no longer matters — whoever reaches zero reads the function from the header.

**Note**: The source code comment at `loops.rs:172-173` says the phantom is needed for "List, Set, Map", but the actual match expression at line 174 only handles `List | Set` — Map is excluded. This is a pre-existing documentation bug in the source code. <!-- reviewed: accuracy fix — noted code/comment discrepancy -->

- [ ] Remove the `needs_phantom` check and `scope.bind_mutable("__for_coll", iter_val)` block (lines ~174-179 in `loops.rs`) <!-- reviewed: accuracy fix — corrected line numbers -->
- [ ] Remove the comments explaining the workaround
- [ ] Also remove/update the `__for_coll` references in `list_builtins.rs` (lines ~118 and ~136) — comments explaining the phantom binding workaround <!-- reviewed: added — these comments reference the workaround too -->
- [ ] Verify: the `__for_coll` name is not referenced anywhere else in the codebase (`grep -r "__for_coll"`)

### Cleanup <!-- reviewed: hygiene fix -->

- [ ] **[WASTE]** `compiler/ori_arc/src/lower/control_flow/loops.rs:173` — Comment says "List, Set, Map" but code at line 174 only matches `List | Set`. After removing the phantom binding, delete both the stale comment and the code. If any comments elsewhere reference `__for_coll`, remove them too.

---

## 03.2 Remove Dummy Reference from lower_for_iterator

**File:** `compiler/ori_arc/src/lower/control_flow/for_loops/for_iterator.rs`

The dummy `Let` after `ori_iter_drop` was added to keep the collection alive past the iterator drop. No longer needed.

- [ ] Remove the `for_coll_name` lookup and `emit_let(coll_ty, ArcValue::Var(exit_param))` block (lines ~195-203 in `for_iterator.rs`) <!-- reviewed: accuracy fix — corrected line numbers -->
- [ ] Remove the comments explaining the ordering guarantee

---

## 03.2.5 Remove Dead `elem_dec_fn` Parameter from Iterator API (Option B Cleanup)

<!-- reviewed: completeness fix — this was deferred as "if time permits" in Section 02.2; made mandatory -->

**Files:** `compiler/ori_rt/src/iterator/sources.rs`, `compiler/ori_rt/src/iterator/state.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs`, `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

With the header storing `elem_dec_fn`, the parameter in `ori_iter_from_list` and the field in `IterState::List` are dead code.

- [ ] Remove `elem_dec_fn` parameter from `ori_iter_from_list` function signature in `sources.rs`
- [ ] Remove `elem_dec_fn` field from `IterState::List` in `state.rs`
- [ ] Update `IterState::List` Drop: `ori_buffer_rc_dec` call no longer needs to pass `elem_dec_fn` — pass NULL (the header provides it)
- [ ] Update `emit_list_iter` in `list_builtins.rs`: remove the `elem_dec_fn_null` argument from the call to `ori_iter_from_list`
- [ ] Update `ori_iter_from_list` declaration in `runtime_functions.rs`: remove the 5th parameter
- [ ] Update any Rust unit tests in `compiler/ori_rt/src/iterator/tests.rs` that call `ori_iter_from_list` with 5 args — update to 4 args
- [ ] Run `timeout 150 cargo test -p ori_rt` and `timeout 150 cargo test -p ori_llvm --test aot` to verify

---

## 03.3 Verify No Regressions

- [ ] Run `timeout 150 cargo test -p ori_llvm --test aot` — all tests pass including unignored fat_ptr_iter tests
- [ ] Run `timeout 150 ./test-all.sh` — all tests pass (Rust + spec tests) <!-- reviewed: accuracy fix — removed stale count -->
- [ ] Run `./clippy-all.sh` — no clippy warnings
- [ ] Run `./fmt-all.sh` — no formatting issues <!-- reviewed: completeness fix -->
- [ ] Verify ARC IR for `[str]` iteration is cleaner (no phantom __for_coll param in loop header) — dump with `ORI_DUMP_AFTER_ARC=1`
- [ ] Verify `ori_iter_from_list` takes 4 parameters (not 5) in the LLVM IR output <!-- reviewed: completeness fix — verify Option B cleanup -->

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] No references to `__for_coll` in the codebase
- [ ] No dummy reference after `ori_iter_drop` in exit block
- [ ] `lower_for` is simpler (no phantom binding logic)
- [ ] `lower_for_iterator` is simpler (no exit-block dummy reference)
- [ ] `ori_iter_from_list` takes 4 parameters (dead `elem_dec_fn` removed) <!-- reviewed: completeness fix -->
- [ ] `IterState::List` has no `elem_dec_fn` field <!-- reviewed: completeness fix -->
- [ ] All tests pass (`timeout 150 ./test-all.sh`)
- [ ] `./clippy-all.sh` — zero warnings <!-- reviewed: completeness fix -->
