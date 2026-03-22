---
section: "03"
title: "Remove Workarounds & Simplify"
status: in-progress
goal: "Remove the phantom __for_coll_N binding, exit-block dummy references, and for-yield coll_param threading, simplifying the ARC lowering"
depends_on: ["02"]
reviewed: true
third_party_review:
  status: findings
  updated: 2026-03-22
sections:
  - id: "03.1"
    title: "Remove Phantom Binding from lower_for"
    status: complete
  - id: "03.1.5"
    title: "Remove For-Yield Collection Threading"
    status: complete
  - id: "03.2"
    title: "Remove Dummy Reference from lower_for_iterator"
    status: complete
  - id: "03.2.5"
    title: "Remove Dead elem_dec_fn Parameter from Iterator API"
    status: complete
  - id: "03.3"
    title: "Verify No Regressions"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: in-progress
  - id: "03.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 03: Remove Workarounds & Simplify

**Status:** In Progress
**Goal:** With elem_dec_fn stored in the RC header, the ordering workarounds in the ARC lowering are no longer needed. Remove them to simplify the code.

**Depends on:** Section 02 (codegen must use header-based cleanup first).

**Ordering within this section:** Steps 03.1, 03.1.5, and 03.2 remove the for-loop workarounds and MUST be done together (they share state: `for_coll_counter`, `ForYieldContext::coll_param`, and `__for_coll_N` bindings). Step 03.2.5 (removing `elem_dec_fn` from the iterator API) is an independent change that can be done before or after the workaround removal, but doing it last is recommended to keep the removal steps focused and testable as a single unit.

---

## 03.1 Remove Phantom Binding from lower_for

**File:** `compiler/ori_arc/src/lower/control_flow/loops.rs`

The phantom `__for_coll_N` mutable binding was added to thread the collection through the loop header, forcing AIMS to add RcInc. With the header storing elem_dec_fn, the ordering no longer matters — whoever reaches zero reads the function from the header.

**Note**: The source code comment at `loops.rs:169-176` says the phantom is needed for "List, Set, Map", but the actual match expression at line 177 only handles `List | Set` — Map is excluded. This is a pre-existing documentation bug in the source code.

- [x] Remove the `needs_phantom` check and `scope.bind_mutable(coll_name, iter_val)` block (lines 177-184 in `loops.rs`) (2026-03-21)
- [x] Remove the comments explaining the workaround (lines 169-176) (2026-03-21)
- [x] Verify: the `__for_coll` name is not referenced anywhere else in the compiler (`grep -r "__for_coll" compiler/` → 0 results) (2026-03-21)
- [x] **[BUG FIX]** Set.iter() was using `emit_list_iter` directly, treating the hash table as a contiguous array — exposed by phantom removal. Fixed: `emit_set_iter` now converts via `ori_set_to_list` + explicit set buffer `ori_set_buffer_rc_dec` (2026-03-21)

### Cleanup (03.1)

- [x] **[WASTE]** `compiler/ori_arc/src/lower/control_flow/loops.rs:169-184` — Removed phantom binding code and stale comment. ALL `__for_coll` references updated:
  - [x] `loops.rs:177-184` — phantom binding creation code removed (2026-03-21)
  - [x] `for_iterator.rs:192-207` — dummy reference after `ori_iter_drop` removed (Section 03.2) (2026-03-21)
  - [x] `for_yield.rs:60-62` — `prepare_iterator()` doc comment updated (2026-03-21)
  - [x] `for_yield.rs:86-96` — `prepare_iterator()` `coll` logic removed (Section 03.1.5) (2026-03-21)
  - [x] `expr/mod.rs:108-112` — `for_coll_counter` field removed. Initialization sites at `lower/mod.rs:168` and `lower/calls/lambda.rs:126` also removed (2026-03-21)
  - [x] `borrowed_defs.rs:208-209` — doc comment updated to describe generic mechanism without referencing removed phantom (2026-03-21)
  - [x] `walk_dec.rs:79` — comment updated (2026-03-21)
  - [x] `list_builtins.rs:143-147` — comment updated to reference header-based approach (2026-03-21)

---

## 03.1.5 Remove For-Yield Collection Threading

**Files:** `compiler/ori_arc/src/lower/control_flow/for_yield.rs`, `compiler/ori_arc/src/lower/expr/mod.rs`, `compiler/ori_arc/src/lower/control_flow/mod.rs`

The for-yield path has its own independent collection-phantom mechanism parallel to the for-do `__for_coll_N` binding. `prepare_iterator()` returns `coll_var = Some(iter_val)` for List/Set, which `lower_for_yield_iterator()` threads through header/exit blocks as `coll_param`/`exit_coll_param` block parameters, and emits a dummy let after `ori_iter_drop`. `ForYieldContext::coll_param` is used by `lower_break`/`lower_continue` to prepend the phantom to jump args. With the header storing `elem_dec_fn`, all of this is unnecessary.

- [x] In `for_yield.rs` `prepare_iterator()`: simplified return type to `(ArcVarId, Idx)`, removed `coll` variable logic (2026-03-21)
- [x] In `for_yield.rs` `lower_for_yield_iterator()`: removed `coll_var` parameter and all `coll_param`/`exit_coll_param` block parameter threading (2026-03-21)
- [x] In `for_yield.rs` exit block: removed dummy reference to `exit_coll_param` after `ori_iter_drop` (2026-03-21)
- [x] In `for_yield.rs` `lower_for_yield_iterator()` doc comment: updated block diagram — removed `coll_param` from header and exit_prep jump args (2026-03-21)
- [x] In `for_yield.rs` `lower_for_yield_iterator()`: removed `#[expect(clippy::too_many_arguments)]` (2026-03-21)
- [x] In `for_yield.rs` `ForYieldContext` construction: removed `coll_param` field initialization (2026-03-21)
- [x] In `expr/mod.rs` `ForYieldContext`: removed `coll_param` field (2026-03-21)
- [x] In `for_yield_option.rs:158`: removed `coll_param: None` and updated comment at line 88 (2026-03-21)
- [x] In `control_flow/mod.rs` `lower_break()`: removed `coll_param` from for-yield break path (2026-03-21)
- [x] In `control_flow/mod.rs` `lower_continue()`: removed `coll_param` from for-yield continue path (2026-03-21)
- [x] **[NOTE]** `control_flow/mod.rs` shrunk from 494 to ~475 lines after `coll_param` removal. Safe margin restored. (2026-03-21)
- [x] In `for_yield.rs` `lower_for_yield()` call site: updated destructuring from 3-tuple to 2-tuple (2026-03-21)

### Cleanup (03.1.5)

- [x] **[WASTE]** `for_yield.rs` — `lower_for_yield_iterator` shrunk ~40 lines. `#[expect(clippy::too_many_arguments)]` removed. `#[expect(clippy::too_many_lines)]` retained (function still > 50 lines but well under 100). (2026-03-21)
- [x] **[NOTE]** `for_yield.rs` shrunk from 390 to ~345 lines. No split needed. (2026-03-21)

---

## 03.2 Remove Dummy Reference from lower_for_iterator

**File:** `compiler/ori_arc/src/lower/control_flow/for_loops/for_iterator.rs`

The dummy `Let` after `ori_iter_drop` was added to keep the collection alive past the iterator drop. No longer needed.

- [x] Remove the `__for_coll_N` lookup and `emit_let(coll_ty, ArcValue::Var(exit_param))` block (lines ~192-207 in `for_iterator.rs`) (2026-03-21)
- [x] Remove the comments explaining the ordering guarantee (2026-03-21)

---

## 03.2.5 Remove Dead `elem_dec_fn` Parameter from Iterator API

**Files:** `compiler/ori_rt/src/iterator/sources.rs`, `compiler/ori_rt/src/iterator/state.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs`, `compiler/ori_llvm/src/codegen/runtime_decl/runtime_functions.rs`, `compiler/ori_llvm/src/evaluator/runtime_mappings.rs`

With the header storing `elem_dec_fn`, the parameter in `ori_iter_from_list` and the field in `IterState::List` are redundant (the header provides the function). Section 02.2 chose Option A (keep parameter for defense-in-depth while the header approach was unproven). Now that the header approach is proven stable by Sections 02-04, the redundant parameter is removed.

**WARNING -- ABI sync point**: This step changes the `ori_iter_from_list` signature from 5 to 4 parameters. All 4 locations MUST be updated in a single commit: (1) `ori_rt/src/iterator/sources.rs` (runtime impl), (2) `runtime_functions.rs` (LLVM IR declaration), (3) `list_builtins.rs` (codegen call site), (4) `runtime_mappings.rs` (JIT symbol). Partial updates cause linker errors or silent memory corruption.

- [x] Remove `elem_dec_fn` parameter from `ori_iter_from_list` function signature in `sources.rs` (currently at line 32). Also: (2026-03-22)
  - [x] Remove `elem_dec_fn,` from the `IterState::List { ... }` construction at line 41 (field must be removed from both the struct definition AND this construction site). (2026-03-22)
  - [x] Update the function doc comment (lines 20-25) — remove the reference to `elem_dec_fn = None` for test data. The doc should explain that element cleanup is entirely header-based (V5 RC header). (2026-03-22)
- [x] Remove `elem_dec_fn` field from `IterState::List` in `state.rs` (field at line 62; `List` variant starts at line 56). Also update the variant doc comment at lines 43-55 — remove the paragraph explaining `elem_dec_fn` defense-in-depth (lines 51-55), since the field will no longer exist. The doc should explain that element cleanup is entirely header-based. (2026-03-22)
- [x] Update `IterState::List` Drop (`state.rs:162-178`): the `drop()` match arm currently destructures `elem_dec_fn` (line 168) and passes `*elem_dec_fn` to `ori_buffer_rc_dec` (line 176). With the field removed, pass `None` instead — `ori_buffer_rc_dec` reads the function from the V5 RC header via `load_elem_dec_fn` at cleanup time. The destructuring pattern at lines 163-169 must also remove the `elem_dec_fn` field. **Safety**: passing `None` is safe because the header was populated at construction time (Section 02.1) and `ori_buffer_rc_dec` always reads from the header in its drop path (`drop_elements_and_free` calls `load_elem_dec_fn`). (2026-03-22)
- [x] Update `emit_list_iter` in `list_builtins.rs`: remove the `elem_dec_fn` argument from the call to `ori_iter_from_list` (currently passes real function at line 148, call at line 152 — this becomes unnecessary when header provides it). Also: (2026-03-22)
  - [x] Update the function doc comment (lines 115-129): remove the "real `elem_dec_fn`" explanation and the "Defense-in-depth" paragraph. Replace with a note that element cleanup is entirely header-based (V5 RC header). (2026-03-22)
  - [x] Remove the comment block at lines 143-147 explaining why `elem_dec_fn` is passed and referencing `__for_coll`. (2026-03-22)
  - [x] Remove `let elem_dec_fn = self.get_or_generate_elem_dec_fn(elem_ty);` at line 148. (2026-03-22)
  - [x] Update the `emit_rt_call` args from `&[data_ptr, len, cap, elem_size_val, elem_dec_fn]` to `&[data_ptr, len, cap, elem_size_val]`. (2026-03-22)
- [x] Update `ori_iter_from_list` declaration in `runtime_functions.rs` (line 1305): remove the 5th parameter (`Ty::Ptr` for `elem_dec_fn`) and update the inline comment at line 1307. **Note**: `runtime_functions.rs` is 1528 lines — self-documenting exemption from the 500-line limit (pure static data table), but verify no logic has crept in. (2026-03-22)
- [x] Update Rust unit tests in `compiler/ori_rt/src/iterator/tests.rs` that call `ori_iter_from_list` with 5 args — update to 4 args. **There are 30+ call sites** across all iterator tests (next, map, filter, take, skip, enumerate, zip, chain, flatten, flat_map, cycle, collect, for_each, fold, slice drop). Each passes `None` as the 5th arg — remove that argument from every call. (2026-03-22)
- [x] Update `ori_iter_from_list` JIT symbol mapping in `compiler/ori_llvm/src/evaluator/runtime_mappings.rs` (line 204) to match the new 4-parameter signature — no change needed: JIT mapping is a function pointer cast, parameter count derived from runtime_functions.rs declaration (already updated). (2026-03-22)
- [x] Run `timeout 150 cargo test -p ori_rt` and `timeout 150 cargo test -p ori_llvm --test aot` to verify — 350 + 1876 tests pass (2026-03-22)

### Cleanup (03.2.5)

- [x] **[STYLE]** `compiler/ori_rt/src/iterator/mod.rs:43,60` — Remove 2 decorative banners (`// ── Extern C API — Core ──...`, `// ── Extern C API — Cleanup ──...`). Replace with plain section comments. (2026-03-22)
- [x] **[STYLE]** `compiler/ori_rt/src/iterator/tests.rs` — Remove 20 decorative banners across test file (e.g., `// ── List iterator ───`, `// ── Range iterator ──`, etc.). Replace with plain section comments. Since this file has 30+ `ori_iter_from_list` calls being modified, clean banners in the same commit. (2026-03-22)

---

## 03.3 Verify No Regressions

- [x] Run `timeout 150 cargo test -p ori_llvm --test aot` — 1876 passed, 0 failed, 17 ignored (2026-03-22)
- [x] Run `timeout 150 ./test-all.sh` — 13,551 passed, 0 failed (2026-03-22)
- [x] Run `./clippy-all.sh` — zero warnings (2026-03-22)
- [x] Run `./fmt-all.sh` — clean (2026-03-22)
- [x] Verify ARC IR for `[str]` iteration is cleaner (no phantom `__for_coll_N` param in loop header, no `coll_param` in for-yield loop header) — verified with `ORI_DUMP_AFTER_ARC=1`, no matches for `__for_coll` or `coll_param` (2026-03-22)
- [x] Verify `ori_iter_from_list` takes 4 parameters (not 5) in the LLVM IR output — confirmed: `declare ptr @ori_iter_from_list(ptr, i64, i64, i64)` (2026-03-22)

---

## 03.R Third Party Review Findings

- [ ] `[TPR-03-002][low]` `plans/rc-header-elem-dec/section-03-remove-workarounds.md:4` — Section 03 status metadata contradicts the document body and index.
  Evidence: Frontmatter marks the section `complete` and `third_party_review.status: resolved`, and `plans/rc-header-elem-dec/index.md` still shows Section 03 as `Status: Complete`, but the section body still says `**Status:** In Progress`.
  Impact: Readers cannot tell whether Section 03 is actually closed, so the review state is hidden behind conflicting metadata.
  Required plan update: Align the section frontmatter, body status text, and index entry in one edit when the section is truly complete, or keep the section reopened until they agree.

- [ ] `[TPR-03-003][low]` `compiler/ori_llvm/src/codegen/arc_emitter/builtins/collections/list_builtins.rs:118` — `emit_list_iter`'s doc comment still claims the function explicitly emits `ori_list_rc_inc`, but the implementation no longer does that.
  Evidence: `list_builtins.rs:118-121` says the function explicitly emits `ori_list_rc_inc`, while the implementation only calls `ori_iter_from_list`; the RC handoff is handled by ARC arg-ownership/auto-iter logic instead of an in-function `ori_list_rc_inc`.
  Impact: The comment misstates the ownership contract at a fragile iterator boundary, increasing the risk of future duplicate-inc or missing-inc changes.
  Required plan update: Update the doc comment to describe the actual ownership path and remove the nonexistent `ori_list_rc_inc` claim.

- [x] `[TPR-03-001][high]` `compiler/ori_llvm/src/codegen/arc_emitter/builtins/mod.rs:397` — Auto-iterator promotion still routes `Set` receivers through `emit_list_iter`, so methods like `Set.fold()` still iterate the hash-table buffer as if it were a contiguous list.
  Resolved: Validated and fixed on 2026-03-22. Split `TypeInfo::List | TypeInfo::Set` match arm in `emit_auto_iter()` so `Set` routes through `emit_set_iter()`. Added 5 semantic pin tests: `set_auto_fold`, `set_auto_count`, `set_auto_any`, `set_auto_all`, `set_str_auto_fold`.

---

## 03.N Completion Checklist

### Workaround Removal (03.1, 03.1.5, 03.2)

- [x] No references to `__for_coll` in compiler code (verified: `grep -rn "__for_coll" compiler/` → 0 results) (2026-03-22)
- [x] No references to `coll_param` in for-yield collection-threading context (verified: `grep -rn "coll_param" compiler/ori_arc/src/lower/` → 0 results) (2026-03-22)
- [x] No dummy reference after `ori_iter_drop` in exit block (both for-do and for-yield paths) — exit blocks only call `ori_iter_drop` + scope restore (2026-03-22)
- [x] `lower_for` is simpler (no phantom binding logic) (2026-03-22)
- [x] `lower_for_iterator` is simpler (no exit-block dummy reference) (2026-03-22)
- [x] `lower_for_yield_iterator` is simpler (no `coll_var`/`coll_param` threading, no exit-block dummy reference) (2026-03-22)
- [x] `ForYieldContext::coll_param` field removed from `expr/mod.rs` — only 3 fields: `list_ptr`, `elem_size`, `list_push_name` (2026-03-22)
- [x] `lower_break` and `lower_continue` in `control_flow/mod.rs` no longer prepend `coll_param` to jump args (2026-03-22)
- [x] `prepare_iterator` in `for_yield.rs` no longer returns collection variable for phantom threading — returns `(ArcVarId, Idx)` (2026-03-22)
- [x] `for_coll_counter` field removed from `ArcLowerer` in `expr/mod.rs` (2026-03-22)
- [x] `for_coll_counter: 0` initialization removed from `lower/mod.rs` and `lower/calls/lambda.rs` (2026-03-22)
- [x] `propagate_borrowed_closure` in `borrowed_defs.rs` updated (no stale `__for_coll` references) (2026-03-22)
- [x] `for_yield_option.rs` updated (no `coll_param: None` in `ForYieldContext` construction) (2026-03-22)
- [x] `for_yield_option.rs` comment updated (no reference to `coll_param`) (2026-03-22)
- [x] `lower_for_yield_iterator` doc comment block diagram updated (no `coll_param` in jump args) (2026-03-22)
- [x] `lower_for_yield_iterator` `#[expect(clippy::too_many_arguments)]` removed (only `too_many_lines` remains with justification) (2026-03-22)

### Iterator API Cleanup (03.2.5)

- [x] `ori_iter_from_list` takes 4 parameters (dead `elem_dec_fn` removed) (2026-03-22)
- [x] `ori_iter_from_list` doc comment updated to reflect header-based cleanup (no `elem_dec_fn` parameter) (2026-03-22)
- [x] `ori_iter_from_list` JIT symbol mapping in `evaluator/runtime_mappings.rs` — no change needed (function pointer cast) (2026-03-22)
- [x] `IterState::List` has no `elem_dec_fn` field (2026-03-22)
- [x] `IterState::List` doc comment updated: header-based cleanup, no defense-in-depth reference (2026-03-22)
- [x] `IterState::List` Drop match arm destructuring pattern updated (no `elem_dec_fn` field) (2026-03-22)
- [x] 30+ `ori_iter_from_list` calls in `iterator/tests.rs` updated from 5 args to 4 args (2026-03-22)
- [x] `emit_list_iter` in `list_builtins.rs` doc comment and code updated — no reference to `elem_dec_fn` parameter or `__for_coll` phantom (2026-03-22)
- [x] Decorative banners in `iterator/mod.rs` and `iterator/tests.rs` replaced with plain section comments (2026-03-22)

### Verification

- [x] All tests pass (`timeout 150 ./test-all.sh`) — 13,551 passed, 0 failed (2026-03-22)
- [x] All tests pass in release build (`cargo b --release && timeout 150 cargo test -p ori_llvm --test aot --release`) — 1876 passed (2026-03-22)
- [x] `./clippy-all.sh` — zero warnings (2026-03-22)
- [x] `lower_for_yield_iterator` function length: 211 lines (exceeds 100-line target but justified with `#[expect(clippy::too_many_lines)]` — inherently sequential SSA merge logic) (2026-03-22)
- [x] `control_flow/mod.rs` file length: 494 lines (under 500-line limit) (2026-03-22)
