---
section: "02"
title: "Fix All Pre-Existing Leaks"
status: not-started
goal: "All 1317 AOT tests pass with ORI_CHECK_LEAKS=1 — zero leaked allocations in any test"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Categorize Failing Tests by Root Cause"
    status: not-started
  - id: "02.2"
    title: "ARC Pipeline — FatValue & Aggregate Drop Fixes"
    status: not-started
  - id: "02.3"
    title: "ARC Pipeline — Slice RC Cleanup"
    status: not-started
  - id: "02.4"
    title: "ARC Pipeline — Edge Cases (Catch, ForIter, AIMS)"
    status: not-started
  - id: "02.S"
    title: "Sync Points — Files That Must Stay Consistent"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Fix All Pre-Existing Leaks

**Status:** Not Started
**Goal:** All 1317 AOT tests pass with `ORI_CHECK_LEAKS=1` enabled — zero leaked RC allocations in any test program.

**Context:** Fixing `ORI_CHECK_LEAKS` for AOT (Section 01) exposed 23 pre-existing leaks across diverse patterns. The FatValue PrimOp bug (`is_consuming_primop` checking `!= Scalar` instead of `== RcPointer`) was already fixed, resolving 7 of the original 30 failures. The remaining 23 fall into distinct categories that need separate fixes.

**Depends on:** Section 01 (leak detection must work to verify fixes).

---

## 02.1 Categorize Failing Tests by Root Cause

For each of the 23 failing tests, determine the root cause by:
1. Dumping ARC IR (`ORI_DUMP_AFTER_ARC=1`)
2. Running with `ORI_TRACE_RC=1` to see alloc/inc/dec patterns
3. Classifying the leak pattern

**Failing tests to categorize:**

- [ ] **Slices (7 tests):** Run `ORI_TRACE_RC=1` on `test_list_slice_basic`, `test_list_slice_empty`, `test_list_slice_from_start`, `test_list_slice_full`, `test_list_slice_preserves_original`, `test_list_slice_single_element`, `test_list_slice_then_length`. Record which allocation leaks (original buffer vs slice) and whether the fix belongs in `ori_rt` or the ARC pipeline.
  - Hypothesis: slice drop function doesn't decrement the original buffer's RC
- [ ] **String SSO (4 tests):** Run `ORI_DUMP_AFTER_ARC=1` and `ORI_TRACE_RC=1` on `test_catch_returns_heap_string`, `test_format_heap_result`, `test_heap_in_struct`, `test_heap_iteration`. Identify the missing RcDec site for each.
  - Hypothesis: heap strings in certain contexts (catch, format, struct field, iteration) not getting RcDec at scope exit
- [ ] **Structs with heap fields (3 tests):** Run `ORI_DUMP_AFTER_ARC=1` on `test_aot_struct_with_list_field`, `test_aot_struct_with_list_and_string`, `test_struct_list_field`. Confirm whether `DropKind::Fields` is generated and whether `drop_gen.rs` emits recursive child drops.
  - Hypothesis: Aggregate drop doesn't recurse into RC-typed fields
- [ ] **List traits (4 tests):** Run `ORI_TRACE_RC=1` on `test_aot_list_equals`, `test_aot_list_equals_empty`, `test_aot_list_compare`, `test_aot_list_compare_empty`. Determine whether the leak is in the derived trait method or the call site.
  - Hypothesis: derived trait methods (equals, compare) borrow operands but nobody drops them
- [ ] **Misc (5 tests):** Run `ORI_TRACE_RC=1` on `test_h6_callee_returns_unique_for_caller_reuse`, `test_rc_catch_heap_alias_scalar_project`, `test_coll_list_pop`, `test_for_iter_break_with_mutation`, `test_sso_repeated_concat_loop`. Classify each by root cause (may be fixed by FatValue fix -- verify first).

**Additional patterns to investigate (may not have failing tests yet but are high-risk):**

- [ ] **While loops:** `while` desugars to `loop { if !cond then break; body }` — verify ARC pipeline emits drops correctly for the implicit break path (dead variables on the break edge)
- [ ] **Closures with RC captures:** Closure environments (`DropKind::ClosureEnv`) must drop captured RC variables when the closure itself is dropped — verify the ARC pipeline emits closure env drops
- [ ] **Match arms:** Variables live in one match arm but dead in another must be dropped on the dead arm's edge — verify `emit_edge_cleanup` handles this

---

## 02.2 ARC Pipeline — FatValue & Aggregate Drop Fixes

**File(s):** `compiler/ori_arc/src/aims/emit_rc/helpers.rs`, `compiler/ori_arc/src/aims/realize/walk.rs`, `compiler/ori_arc/src/aims/emit_rc/edge_cleanup.rs`

Fix missing RcDec emissions for FatValue variables and Aggregate types containing RC fields.

- [ ] Verify `is_consuming_primop` fix (DONE: `== RcPointer` instead of `!= Scalar`, at `emit_rc/helpers.rs:288`) resolves string concat loop leaks
- [ ] Inspect `emit_last_use_decs` in `realize/walk.rs`: trace a struct-with-list-field test and confirm whether a drop call is emitted for the struct local at end of scope. If missing, add Aggregate handling.
- [ ] If Aggregate drops are missing: add logic in `emit_last_use_decs` or `emit_edge_cleanup` to emit struct-level drops that recursively free RC children via `DropKind::Fields`
- [ ] Verify the `DropInfo`/`DropKind` system correctly identifies structs needing drops (`DropKind::Fields` with RC-typed fields)
- [ ] Check that `emit_defined_dead` handles Aggregates — a struct created but never used should still be dropped if it contains RC fields
- [ ] Check that `emit_defined_dead` and `emit_edge_cleanup` handle `FatValue` variables (str, closure) — a FatValue local that goes dead must have its pointer component RcDec'd
- [ ] Verify `is_ownership_transfer()` in `emit_rc/helpers.rs` correctly classifies all four `ValueRepr` variants (`Scalar`, `RcPointer`, `Aggregate`, `FatValue`) — an incorrect classification causes either double-free or leak
- [ ] Verify that function return paths emit drops for all live non-returned RC variables (Aggregate, FatValue, RcPointer) — not just scope-exit drops
- [ ] Verify `ori_str_concat` runtime function (`compiler/ori_rt/src/string/ops.rs:146`) actually borrows (not consumes) both inputs — if it consumes, the `is_consuming_primop` fix is wrong for strings
- [ ] Add unit tests for each fixed pattern in `compiler/ori_arc/src/aims/emit_rc/` test modules
- [ ] Run `ORI_TRACE_RC=1` on fixed binaries to verify alloc/free balance

---

## 02.3 ARC Pipeline — Slice RC Cleanup

**File(s):** `compiler/ori_rt/src/list/slice.rs`, `compiler/ori_rt/src/rc/list_rc.rs`, `compiler/ori_rt/src/slice_encoding/mod.rs`

Fix slice RC management — slices share the original buffer's data and must properly decrement the original's RC when dropped.

> **Warning:** `slice_buffer_rc_dec` in `list_rc.rs:143` has a documented limitation: when a slice is the last reference and its range doesn't cover all elements of the original buffer, elements outside the slice's range will have their child RCs leaked. The plan must determine whether this limitation is acceptable for the leaking slice tests or whether it is the root cause. Investigate before assuming the fix is in the ARC pipeline.

- [ ] Trace a slice test with `ORI_TRACE_RC=1` to identify which allocation leaks (the original list or the slice)
- [ ] Check `ori_buffer_rc_dec` handles slice caps correctly (bit 63 flag)
- [ ] Verify the ARC pipeline emits RcDec for the original list after creating a slice (or that the slice's drop function handles the original)
- [ ] If the issue is in the runtime: fix `ori_buffer_rc_dec` to properly handle slice cleanup
- [ ] If the issue is in the ARC pipeline: fix missing RcDec for the original list when it goes out of scope alongside the slice
- [ ] Add Rust unit tests for slice RC lifecycle in `compiler/ori_rt/src/list/slice/tests.rs`

### Cleanup

- [ ] **[WASTE]** `compiler/ori_rt/src/rc/list_rc.rs:70-127,157-216` — The `#[cfg(not(feature = "single-threaded"))]` and `#[cfg(feature = "single-threaded")]` blocks in both `ori_buffer_rc_dec` and `slice_buffer_rc_dec` duplicate the element cleanup and free logic (~30 lines each, repeated 4 times total). Extract a shared `fn drop_buffer_elements_and_free(data, slice_data, n, es, elem_dec_fn, data_size_or_cap)` helper that both cfg paths call after the atomic/non-atomic RC check. This reduces ~120 duplicated lines to ~30.

---

## 02.4 ARC Pipeline — Edge Cases (Catch, ForIter, AIMS)

**File(s):** Various

Fix remaining edge-case leaks that don't fall into the above categories.

- [ ] `test_catch_returns_heap_string`: Check if `catch` expression cleanup frees the heap string
- [ ] `test_for_iter_break_with_mutation`: Check if breaking from a for loop with a mutated binding leaks
- [ ] `test_h6_callee_returns_unique_for_caller_reuse`: Check AIMS interaction where callee returns unique value
- [ ] `test_rc_catch_heap_alias_scalar_project`: Check catch + heap alias + scalar projection pattern
- [ ] `test_coll_list_pop`: Check if list pop operation leaks the popped element or the original list
- [ ] `test_sso_repeated_concat_loop`: This is the original motivating crash pattern (string concat in loop promoting from SSO to heap). Verify the `is_consuming_primop` fix resolves it completely. If it still leaks, the issue is in `emit_last_use_decs` not emitting RcDec for the old string value before reassignment.
- [ ] For each fix: add a TDD-style test — write failing test, verify failure, fix, verify pass

---

## 02.S Sync Points — Files That Must Stay Consistent

Any fix to RC emission logic must verify consistency across all of these locations:

| Location | Purpose |
|----------|---------|
| `compiler/ori_arc/src/aims/emit_rc/helpers.rs` | `is_consuming_primop()`, `is_ownership_transfer()`, `has_rc_projection()` — classify which operations consume/transfer RC |
| `compiler/ori_arc/src/aims/emit_rc/mod.rs` | `emit_block_rc_ops()`, `precompute_block_uses()` — per-block RC emission orchestration |
| `compiler/ori_arc/src/aims/realize/walk.rs` | `emit_last_use_decs()`, `emit_defined_dead()` — last-use drop insertion |
| `compiler/ori_arc/src/aims/emit_rc/edge_cleanup.rs` | `emit_edge_cleanup()` — inter-block drop insertion for variables dead across edges |
| `compiler/ori_arc/src/drop/mod.rs` | `DropKind`, `compute_drop_kind()` — determines what cleanup is needed per type |
| `compiler/ori_llvm/src/codegen/arc_emitter/drop_gen.rs` | LLVM IR generation for each `DropKind` variant — must match `ori_arc` drop descriptors |
| `compiler/ori_rt/src/rc/allocate.rs` | Runtime RC alloc/inc/dec — must agree with codegen expectations |
| `compiler/ori_rt/src/string/ops.rs` | `ori_str_concat` — borrow vs consume semantics must match `is_consuming_primop` |
| `compiler/ori_rt/src/list/cow.rs` | COW runtime functions — consume semantics must match `is_consuming_primop` |

**Rule:** A fix that changes behavior in one of these files must be verified against all others. A change to `is_consuming_primop` that says "string concat does not consume" is only correct if `ori_str_concat` actually borrows (not frees) its inputs.

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] All 23 originally failing tests pass with `ORI_CHECK_LEAKS=1`
- [ ] `ORI_TRACE_RC=1` shows balanced alloc/free for each fixed pattern
- [ ] No new leaks introduced (full AOT test suite passes: 1317 tests)
- [ ] `timeout 150 ./test-all.sh` green — 12,908+ tests, 0 failures
- [ ] `./clippy-all.sh` green
- [ ] All 13 code journeys still score 10/10
- [ ] Valgrind clean on heap-allocating journeys (J5, J9, J10, J13)
- [ ] All sync points in Section 02.S verified consistent (no partial fix that shifts leak to a different pattern)
- [ ] While-loop heap reassignment tested and leak-free (even if no pre-existing test was failing)
- [ ] Closure capturing RC variable tested and leak-free
- [ ] Match arm with dead RC variable tested and leak-free (variable live in one arm, dead in another)

**Exit Criteria:** `cargo test -p ori_llvm --test aot` passes with 1317 tests, 0 failures, 0 leaks. Every AOT test binary exits cleanly with `ORI_CHECK_LEAKS=1`.
