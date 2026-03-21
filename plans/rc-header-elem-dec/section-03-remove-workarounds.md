---
section: "03"
title: "Remove Workarounds & Simplify"
status: not-started
goal: "Remove the phantom __for_coll_N binding and exit-block dummy reference, simplifying the ARC lowering"
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

The phantom `__for_coll_N` mutable binding was added to thread the collection through the loop header, forcing AIMS to add RcInc. With the header storing elem_dec_fn, the ordering no longer matters — whoever reaches zero reads the function from the header.

**Note**: The source code comment at `loops.rs:169-176` says the phantom is needed for "List, Set, Map", but the actual match expression at line 177 only handles `List | Set` — Map is excluded. This is a pre-existing documentation bug in the source code.

- [ ] Remove the `needs_phantom` check and `scope.bind_mutable(coll_name, iter_val)` block (lines ~177-184 in `loops.rs`)
- [ ] Remove the comments explaining the workaround (lines 169-176)
- [ ] Also remove/update the `__for_coll` references in `list_builtins.rs` (lines ~118 and ~142) — comments referencing the phantom binding workaround
- [ ] Verify: the `__for_coll` name is not referenced anywhere else in the codebase (`grep -r "__for_coll"`)

### Cleanup

- [ ] **[WASTE]** `compiler/ori_arc/src/lower/control_flow/loops.rs:169-184` -- Comment says "List, Set, Map" but code at line 177 only matches `List | Set`. After removing the phantom binding, delete both the stale comment and the code. ALL locations referencing `__for_coll` must be updated:
  - [ ] `loops.rs:180-184` — phantom binding creation code (primary removal target)
  - [ ] `for_iterator.rs:192-207` — dummy reference after `ori_iter_drop` (Section 03.2 handles this)
  - [ ] `for_yield.rs:62` — doc comment referencing `__for_coll` phantom (update or remove)
  - [ ] `expr/mod.rs:108-111` — `for_coll_counter` field and its doc comment (remove field entirely)
  - [ ] `borrowed_defs.rs:208-209` — doc comment on `propagate_borrowed_closure` references `__for_coll` as the motivating use case. The function itself is generic (propagates borrowed-ness through Let aliases and Jump arg-to-param flows) and does NOT depend on `__for_coll` — only the comment mentions it. Update comment to describe the general mechanism without referencing the removed phantom.
  - [ ] `walk_dec.rs:79` — comment referencing `__for_coll` phantom threading (update)
  - [ ] `list_builtins.rs:142` — comment referencing `__for_coll` phantom mechanism (update to reference header-based approach)

---

## 03.2 Remove Dummy Reference from lower_for_iterator

**File:** `compiler/ori_arc/src/lower/control_flow/for_loops/for_iterator.rs`

The dummy `Let` after `ori_iter_drop` was added to keep the collection alive past the iterator drop. No longer needed.

- [ ] Remove the `__for_coll_N` lookup and `emit_let(coll_ty, ArcValue::Var(exit_param))` block (lines ~192-207 in `for_iterator.rs`)
- [ ] Remove the comments explaining the ordering guarantee

---

## 03.2.5 Remove Dead `elem_dec_fn` Parameter from Iterator API (Option B Cleanup)

**Files:** `compiler/ori_rt/src/iterator/sources.rs`, `compiler/ori_rt/src/iterator/state.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs`, `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`

With the header storing `elem_dec_fn`, the parameter in `ori_iter_from_list` and the field in `IterState::List` are redundant (the header provides the function).

- [ ] Remove `elem_dec_fn` parameter from `ori_iter_from_list` function signature in `sources.rs` (currently at line 32)
- [ ] Remove `elem_dec_fn` field from `IterState::List` in `state.rs` (currently at line 56)
- [ ] Update `IterState::List` Drop: `ori_buffer_rc_dec` call no longer needs to pass `elem_dec_fn` — pass NULL (the header provides it)
- [ ] Update `emit_list_iter` in `list_builtins.rs`: remove the `elem_dec_fn` argument from the call to `ori_iter_from_list` (currently passes real function at line 144 — this becomes unnecessary when header provides it)
- [ ] Update `ori_iter_from_list` declaration in `runtime_functions.rs`: remove the 5th parameter
- [ ] Update any Rust unit tests in `compiler/ori_rt/src/iterator/tests.rs` that call `ori_iter_from_list` with 5 args — update to 4 args
- [ ] Remove `ori_list_push_new` declaration from `runtime_functions.rs` and JIT symbol mapping from `runtime_mappings.rs` — zero codegen callers confirmed in Section 02; dead declaration creates confusion
- [ ] Update `ori_iter_from_list` JIT symbol mapping in `runtime_mappings.rs` to match the new 4-parameter signature
- [ ] Run `timeout 150 cargo test -p ori_rt` and `timeout 150 cargo test -p ori_llvm --test aot` to verify

---

## 03.3 Verify No Regressions

- [ ] Run `timeout 150 cargo test -p ori_llvm --test aot` — all tests pass including unignored fat_ptr_iter tests
- [ ] Run `timeout 150 ./test-all.sh` — all tests pass (Rust + spec tests)
- [ ] Run `./clippy-all.sh` — no clippy warnings
- [ ] Run `./fmt-all.sh` — no formatting issues
- [ ] Verify ARC IR for `[str]` iteration is cleaner (no phantom __for_coll_N param in loop header) — dump with `ORI_DUMP_AFTER_ARC=1`
- [ ] Verify `ori_iter_from_list` takes 4 parameters (not 5) in the LLVM IR output

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] No references to `__for_coll` in the codebase (verify with `grep -rn "__for_coll" compiler/`)
- [ ] No dummy reference after `ori_iter_drop` in exit block
- [ ] `lower_for` is simpler (no phantom binding logic)
- [ ] `lower_for_iterator` is simpler (no exit-block dummy reference)
- [ ] `for_coll_counter` field removed from `ExprLowerer` in `expr/mod.rs`
- [ ] `propagate_borrowed_closure` in `borrowed_defs.rs` updated (no stale `__for_coll` references)
- [ ] `ori_iter_from_list` takes 4 parameters (dead `elem_dec_fn` removed)
- [ ] `ori_iter_from_list` JIT symbol mapping updated for 4-parameter signature
- [ ] `IterState::List` has no `elem_dec_fn` field
- [ ] `ori_list_push_new` declaration removed from `runtime_functions.rs` and `runtime_mappings.rs`
- [ ] All tests pass (`timeout 150 ./test-all.sh`)
- [ ] All tests pass in release build (`cargo b --release && timeout 150 cargo test -p ori_llvm --test aot`)
- [ ] `./clippy-all.sh` -- zero warnings
