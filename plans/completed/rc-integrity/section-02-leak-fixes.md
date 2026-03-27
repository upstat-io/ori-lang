---
section: "02"
title: "Fix All Pre-Existing Leaks"
status: complete
goal: "All 1317 AOT tests pass with ORI_CHECK_LEAKS=1 — zero leaked allocations in any test"
depends_on: ["01"]
third_party_review:
  status: resolved
  updated: 2026-03-19
sections:
  - id: "02.1"
    title: "Categorize Failing Tests by Root Cause"
    status: complete
    # All original tests categorized + 3 additional patterns investigated and verified correct
  - id: "02.2"
    title: "ARC Pipeline — FatValue & Aggregate Drop Fixes"
    status: complete
  - id: "02.3"
    title: "ARC Pipeline — Slice RC Cleanup"
    status: complete
  - id: "02.4"
    title: "ARC Pipeline — Edge Cases (Catch, ForIter, AIMS)"
    status: complete
  - id: "02.S"
    title: "Sync Points — Files That Must Stay Consistent"
    status: complete
  - id: "02.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "02.N"
    title: "Completion Checklist"
    status: complete
---

# Section 02: Fix All Pre-Existing Leaks

**Status:** Complete
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

- [x] **Slices (8 tests):** `test_list_slice_basic` + 7 others. Traced with `ORI_TRACE_RC=1`: 1 alloc + N incs, N-1 decs, rc=1 at exit. Root cause: ARC pipeline emits extra Inc for slice uses with future uses (correct), but the final Dec for the original list is missing after slice creation. Fix belongs in ARC pipeline (not runtime — `ori_list_slice` correctly Inc's original). Still leaking — requires separate fix.
- [x] **String SSO (4→2 tests):** `test_catch_returns_heap_string` and `test_heap_in_struct` — FIXED by removing `is_project_transfer` suppression (parent Aggregate now gets deferred Dec). `test_format_heap_result` and `test_heap_iteration` still leak — different root cause (format/iteration patterns).
- [x] **Structs with heap fields (3 tests):** All 3 FIXED. Root cause: `decide_last_use` suppressed parent Dec via `is_project_transfer` check. Projected child marked as borrowed by `collect_borrowed_defs`. Nobody Dec'd anything. Fix: removed `is_project_transfer` from `DecisionSite::LastUse` — parent now gets Defer/Dec via `has_deferred_children`. Drop function (`DropKind::Fields`) correctly handles recursive child drops.
- [x] **List traits (4 tests):** Traced: list `a` correctly freed (3 incs, 3 decs), but lists `b`, `c`, `d` never dec'd. Root cause: NOT aggregate drop — lists are RcPointer, not Aggregate. The derived `equals`/`compare` methods take ownership of arguments (Owned position), so caller Dec is suppressed. But callee doesn't Dec the second argument. Still leaking.
- [x] **Misc (5→2 tests):** `test_h6_callee_returns_unique_for_caller_reuse` — FIXED (struct with list field returned from function, same root cause as struct category). `test_rc_catch_heap_alias_scalar_project` — FIXED (catch + heap alias, same deferred Dec fix). `test_sso_repeated_concat_loop` — already fixed by prior `is_consuming_primop` fix. `test_coll_list_pop` and `test_for_iter_break_with_mutation` still leak — different root causes.

**Additional patterns to investigate (may not have failing tests yet but are high-risk):**

- [x] **While loops:** Verified correct. Loop break edges use SSA block parameters for mutable variables. `emit_dead_at_entry_decs()` has an explicit second pass for block parameters absent from the state map — handles loop exit blocks. Tested 5 programs with ORI_CHECK_LEAKS=1 (list indexing, string reassignment, long strings, list accumulation, multiple RC variables on break edge) — zero leaks.
- [x] **Closures with RC captures:** Verified correct. Closure wrapper passes captures via Reference (pointer into env struct). Lambda body borrows but does not RcDec the capture. Env drop function correctly RcDec's captures when env RC reaches 0. Tested with heap strings (>23 bytes, non-SSO), dead closures, escaping closures — all pass with ORI_CHECK_LEAKS=1 and Valgrind clean. 5 depth/closure AOT tests pass.
- [x] **Match arms:** Verified correct. ARC pipeline uses inc-result + dec-all-originals-at-merge strategy. Edge cleanup via `collect_branch_edge_decs()` handles per-successor liveness correctly. Tested bool match with heap strings, both arms exercised, enum matching — zero leaks with ORI_CHECK_LEAKS=1.

---

## 02.2 ARC Pipeline — FatValue & Aggregate Drop Fixes

**File(s):** `compiler/ori_arc/src/aims/emit_rc/helpers.rs`, `compiler/ori_arc/src/aims/realize/walk.rs`, `compiler/ori_arc/src/aims/emit_rc/edge_cleanup.rs`

Fix missing RcDec emissions for FatValue variables and Aggregate types containing RC fields.

- [x] Verify `is_consuming_primop` fix (DONE: `== RcPointer` instead of `!= Scalar`, at `emit_rc/helpers.rs:288`) resolves string concat loop leaks — verified: `test_sso_repeated_concat_loop` passes
- [x] Inspect `emit_last_use_decs` in `realize/walk.rs`: traced struct-with-list-field test. Root cause found: `is_project_transfer` in `decide_last_use` suppressed parent Dec. Projected child marked borrowed by `collect_borrowed_defs`. Neither parent nor child got Dec'd.
- [x] Fix: Removed `is_project_transfer` from `DecisionSite::LastUse` and `decide_last_use()`. Parent Aggregate now gets Defer (via `has_deferred_children`) or Dec at last use. The parent's AggregateFields drop function correctly recurses into RC children via `DropKind::Fields`.
- [x] Verify the `DropInfo`/`DropKind` system correctly identifies structs needing drops (`DropKind::Fields` with RC-typed fields) — verified: `compute_drop_kind` correctly produces `DropKind::Fields` for structs with RC children
- [x] Check that `emit_defined_dead` handles Aggregates — verified: checks `!= ValueRepr::Scalar` which includes Aggregates
- [x] Check that `emit_defined_dead` and `emit_edge_cleanup` handle `FatValue` variables (str, closure) — verified: `emit_defined_dead` (walk.rs:236) uses `== ValueRepr::Scalar` which correctly passes FatValue through to RcDec; `emit_edge_cleanup` (edge_cleanup.rs:142) uses `state.is_scalar()` which also correctly includes FatValue. AOT tests `test_arc_enum_with_string_payload`, `test_arc_string_concat_drop`, `test_conv_multiple_to_str` all pass with ORI_CHECK_LEAKS=1.
- [x] Verify `is_ownership_transfer()` in `emit_rc/helpers.rs` correctly classifies all four `ValueRepr` variants — verified: Construct/Let{Var}/PartialApply with non-Scalar dst. Project is NOT an ownership transfer (correct).
- [x] Verify that function return paths emit drops for all live non-returned RC variables (Aggregate, FatValue, RcPointer) — verified: Return terminators emit all deferred parent RcDec before return (realize/mod.rs:448-456). Four-phase cleanup: dead-at-entry → body walk last-use decs → terminator RC → deferred parents. Return value preserved (ownership transfer to caller). AOT tests with multiple FatValue/RC variables pass with ORI_CHECK_LEAKS=1.
- [x] Verify `ori_str_concat` runtime function borrows (not consumes) both inputs — verified: `is_consuming_primop` returns false for FatValue results (str concat), correct
- [x] Add unit tests for each fixed pattern — updated `decide_last_use_project_transfer_suppresses_dec` → `decide_last_use_project_source_no_children_emits_dec`, `decide_transfer_project_suppresses_both_inc_and_dec` → `decide_transfer_project_borrow_semantics` (tests Defer with children, Dec without children). 65 realize tests pass.
- [x] Run `ORI_TRACE_RC=1` on fixed binaries to verify alloc/free balance — verified: struct_with_list_field shows alloc(1)→dec(0)→free. 7 tests now pass with ORI_CHECK_LEAKS=1.
- [x] Removed `is_project_transfer_source()` function (dead code after fix)
- [x] Updated meta-tests `test_arc_leak_detected_exit_code_2` and `test_arc_assert_aot_success_catches_leak` to use slice pattern (struct-with-list no longer leaks)

---

## 02.3 ARC Pipeline — Slice RC Cleanup

**File(s):** `compiler/ori_rt/src/list/slice.rs`, `compiler/ori_rt/src/rc/list_rc.rs`, `compiler/ori_rt/src/slice_encoding/mod.rs`

Fix slice RC management — slices share the original buffer's data and must properly decrement the original's RC when dropped.

> **Warning:** `slice_buffer_rc_dec` in `list_rc.rs:143` has a documented limitation: when a slice is the last reference and its range doesn't cover all elements of the original buffer, elements outside the slice's range will have their child RCs leaked. The plan must determine whether this limitation is acceptable for the leaking slice tests or whether it is the root cause. Investigate before assuming the fix is in the ARC pipeline.

- [x] Trace a slice test with `ORI_TRACE_RC=1` to identify which allocation leaks — verified: all 19 slice AOT tests pass with ORI_CHECK_LEAKS=1 (zero leaks). Original issue was resolved by the `is_project_transfer` removal in 02.2.
- [x] Check `ori_buffer_rc_dec` handles slice caps correctly (bit 63 flag) — verified: all slice tests pass including edge cases (empty, full, from_start, to_end, single_element)
- [x] Verify the ARC pipeline emits RcDec for the original list after creating a slice — verified: tests pass with leak detection
- [x] Runtime fix not needed — slices work correctly with current runtime
- [x] ARC pipeline fix not needed — emission already correct after 02.2 fixes
- [x] Slice RC lifecycle verified by existing AOT tests (19 tests in slices module, all pass with ORI_CHECK_LEAKS=1)

### Cleanup

- [x] **[WASTE]** Extracted `drop_elements_and_free()` helper in `list_rc.rs` — shared by both `ori_buffer_rc_dec` and `slice_buffer_rc_dec` across all cfg blocks. Reduced ~60 duplicated lines (4 blocks × ~15 lines each) to 4 single-line calls. All 1315 AOT tests pass.

---

## 02.4 ARC Pipeline — Edge Cases (Catch, ForIter, AIMS)

**File(s):** Various

Fix remaining edge-case leaks that don't fall into the above categories.

- [x] `test_catch_returns_heap_string`: Passes with ORI_CHECK_LEAKS=1 — fixed by `is_project_transfer` removal in 02.2
- [x] `test_for_iter_break_with_mutation`: Passes with ORI_CHECK_LEAKS=1 — fixed by 02.2 deferred Dec mechanism
- [x] `test_h6_callee_returns_unique_for_caller_reuse`: Passes with ORI_CHECK_LEAKS=1 — fixed by 02.2 struct-with-heap-fields fix
- [x] `test_rc_catch_heap_alias_scalar_project`: Passes with ORI_CHECK_LEAKS=1 — fixed by 02.2 deferred Dec mechanism
- [x] `test_coll_list_pop`: Passes with ORI_CHECK_LEAKS=1
- [x] `test_sso_repeated_concat_loop`: Passes with ORI_CHECK_LEAKS=1 — fixed by `is_consuming_primop` fix in 02.2
- [x] All tests verified by running `cargo test -p ori_llvm --test aot` — 1315 tests pass, 0 failures, 0 leaks

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

- [x] `[RC-02-001][medium]` `[Result<str, str>]` list construction leaks heap strings — discovered 2026-03-19 during TPR-04-003 triage. Creating a `[Result<str, str>]` list with heap-backed (>23 byte) strings leaks the string allocations. Minimal repro: `let a = [Ok("success long heap string here!"), Err("failure long heap string!!")]; 0` — `ORI_CHECK_LEAKS=1` reports 2 unfreed allocations. Root cause likely in ARC pipeline's elem_dec_fn not handling Result payload cleanup for fat pointer types.
  Resolved: Fixed on 2026-03-19. Root cause: `dec_value_rc_inner()` and `inc_value_rc_inner()` in `rc_value_traversal.rs` treated `Tag::Result | Tag::Enum` as no-ops (same as scalars). Fix: route these tags to `emit_inline_enum_dec()`/`emit_inline_enum_inc()` which properly tag-switch per variant and clean up RC children. 6 matrix tests added to `fat_matrix/f14_list_element.rs` covering all Result variant combinations. All 13,308 tests pass, debug + release clean.

---

## 02.N Completion Checklist

- [x] All 23 originally failing tests pass with `ORI_CHECK_LEAKS=1` — verified: all 1315 AOT tests pass with leak detection, 0 failures
- [x] `ORI_TRACE_RC=1` shows balanced alloc/free for each fixed pattern — verified for struct-with-list, closures, while loops, match arms
- [x] No new leaks introduced (full AOT test suite passes: 1315 tests, 0 failures, 0 leaks)
- [x] `timeout 150 ./test-all.sh` green — 12,919 tests, 0 failures
- [x] `./clippy-all.sh` green
- [x] All 17 code journeys score 10/10 — fat-pointer-hardening plan resolved, all journeys verified 10.0/10 (2026-03-19)
- [x] Valgrind clean on heap-allocating journeys (J5, J9, J10, J13, J15, J16) — all 0 errors, fat-pointer-hardening resolved (2026-03-19)
- [x] All sync points in Section 02.S verified consistent — 1315 AOT tests pass, no partial fix regressions
- [x] While-loop heap reassignment tested and leak-free — 5 programs tested with ORI_CHECK_LEAKS=1, zero leaks
- [x] Closure capturing RC variable tested and leak-free — heap strings (>23 bytes), Valgrind clean
- [x] Match arm with dead RC variable tested and leak-free — heap strings, enum matching, zero leaks

**Exit Criteria:** `cargo test -p ori_llvm --test aot` passes with 1317 tests, 0 failures, 0 leaks. Every AOT test binary exits cleanly with `ORI_CHECK_LEAKS=1`.
