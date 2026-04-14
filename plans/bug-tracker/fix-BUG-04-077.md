---
bug: "BUG-04-077"
title: "Collect output boundary ABI mismatch: collected List<int> has canonical i64 stride but list_traits/debug_helpers read with narrowed i8 stride"
severity: "critical"
status: in-progress
goal: "Collected lists store elements at the same narrowed stride as list literals, so list_traits (equals/compare/hash) and debug_helpers (display) read correct data"
success_criteria:
  - "[1,2,3].iter().map((x) -> x * 1000).collect() == [1000,2000,3000] returns true in AOT"
  - "str([1,2,3].iter().map((x) -> x * 1000).collect()) produces [1000, 2000, 3000]"
  - "All existing iterator/collect AOT tests pass"
  - "ORI_CHECK_LEAKS=1 reports zero leaks on collect test programs"
subsystem: "ori_llvm (iterator_consumers.rs, trampolines.rs)"
found: "2026-04-14"
source: "tpr-review"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-04-077 — Collect output boundary ABI mismatch

**Status:** In Progress
**Severity:** Critical
**Goal:** Collected lists store elements at the same narrowed stride as list literals, so equality, comparison, hash, and display all read correct data from collected lists.

**Success Criteria:**
- [ ] `[1,2,3].iter().map((x) -> x * 1000).collect() == [1000,2000,3000]` returns true in AOT
- [ ] `str(collected_list)` produces correct output
- [ ] All existing iterator/collect AOT tests pass
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks

**Context:** The BUG-04-071 fix canonicalized the iterator pipeline — `emit_list_iter` injects a sext widening trampoline so all iterator values are canonical i64. But the symmetric narrowing at the `collect()` storage boundary was not implemented. `collect()` stores elements at canonical stride (8 bytes), while list literal construction and all list readers use narrowed stride (e.g., 1 byte for i8-narrowed int). This causes silent data corruption: equality comparisons, hashing, and display on collected lists read from wrong memory offsets. Found by TPR — both Codex and Gemini independently confirmed with live AOT reproducers.

---

## 1. Root Cause Analysis

- **Symptom**: `[1,2,3].iter().map((x) -> x * 1000).collect() == [1000,2000,3000]` returns false in AOT. `str()` of collected list produces garbage.
- **Proximate cause**: `emit_iter_collect()` at `iterator_consumers.rs:26` uses `element_store_size(elem_ty)` (canonical = 8 bytes for int) for the collect runtime call's `elem_size`. The runtime stores 8 bytes per element in the output list.
- **Root cause**: The BUG-04-071 fix implemented the READ-side storage boundary (sext widening at `iter()`) but not the WRITE-side storage boundary (trunc narrowing at `collect()`). The iter→collect pipeline is: narrowed buffer → sext widen → canonical pipeline → collect → output list. The output list should be narrowed (matching literal construction and readers), but collect writes canonical.
- **Blast radius**: Every collected `List<int>` when narrowing is active. Affects: equality (`list_traits.rs:64`), comparison (`list_traits.rs:147`), hash (`list_traits.rs:222`), debug/display (`debug_helpers.rs:412`). All produce wrong results on collected lists.
- **Affected files**:
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/iterator_consumers.rs` — `emit_iter_collect()` must inject trunc adapter and pass narrowed elem_size
  - `compiler/ori_llvm/src/codegen/arc_emitter/builtins/trampolines.rs` — add `generate_trunc_narrowing_trampoline()` (symmetric to existing sext widening trampoline)

**Key observation**: List literal construction (`construction.rs:169`) uses `collection_elem_size()` (narrowed). List readers (`list_traits.rs`, `debug_helpers.rs`) use `int_element_llvm_type()` (narrowed). These are CORRECT and consistent. Only `collect()` uses canonical stride — it's the outlier.

---

## 1.5 Fix Consensus (via /tp-help)

Independent dual-source design review. Run: `/tmp/ori-tpr-dp1o2jmU`

- **Proposed approach (pre-consensus)**: Trunc narrowing trampoline at collect boundary, symmetric with sext widening at iter().

### Round 1
- **Codex summary**: Agrees trunc adapter is correct per CG:NR-1/NR-3/NR-4/RN-1. MUST gate on `elem_ty == int`. Found GAP in `set_builtins.rs:284` (builds List<T> with canonical stride). Found LEAK in `map_builtins.rs` (derives list layout from map repr). Found DRIFT (stale comment at `iterator_consumers.rs:87`).
- **Gemini summary**: Agrees structurally. Flags that `narrowed_int_collection_element_width()` is a global heuristic — use `collection_elem_size(result_list_ty, elem_ty)` with specific collection Idx. Notes NR-3 text contradicts RN-1 (needs amendment). Found same DRIFT (stale comment).
- **Agreement points**: Trunc adapter correct. sext in readers unchanged. No other list producers affected. elem_inc/dec null for int.
- **Independent code verification**: Confirmed per-collection functions already exist at `narrowing_codegen.rs:25-105` (`narrowed_collection_element_width`, `collection_elem_size`, `collection_elem_llvm_type`, `trunc_for_narrowed_collection_element`, `sext_narrowed_collection_element`). Global heuristic at line 117 has explicit LIMITATION comment. `pool.list(elem)` at `pool/construct/mod.rs:26` available to compute collection_idx.
- **Outcome**: Agreement — both endorse trunc adapter. Refined: use per-collection `collection_elem_size()` not global heuristic.

### Final agreed approach
1. In `emit_iter_collect`: compute `collection_idx` via `pool.list(elem_ty)`, use `collection_elem_size()`. If narrowed, generate trunc trampoline + wrap iterator + pass narrowed size.
2. Add `generate_trunc_narrowing_trampoline()` in trampolines.rs.
3. File /add-bug for: set_builtins.rs GAP, reader global-heuristic LEAK.
4. Fix stale comment DRIFT at iterator_consumers.rs.

---

## 2. TDD — Test Matrix

### Exact failing case
- [ ] `collect_map_equals_literal` — `[1,2,3].iter().map((x) -> x * 1000).collect() == [1000,2000,3000]` returns true
- [ ] `collect_map_str_representation` — `str([1,2,3].iter().map((x) -> x * 1000).collect())` produces `[1000, 2000, 3000]`

### Edge cases
- [ ] `collect_identity_equals_literal` — `[1,2,3].iter().collect() == [1,2,3]` (no map, just collect)
- [ ] `collect_empty_equals_empty` — `[].iter().collect() == []` (empty list edge case)
- [ ] `collect_single_element` — `[42].iter().collect() == [42]`
- [ ] `collect_negative_values` — `[-1,-128,127].iter().collect()` — tests signed narrowing boundary
- [ ] `collect_large_values` — `[1000,2000,3000].iter().collect()` — values outside i8 range

### Cross-feature interactions
- [ ] `collect_filter_then_equals` — `[1,2,3,4,5].iter().filter(x -> x > 2).collect() == [3,4,5]`
- [ ] `collect_chain_then_equals` — chained adapters before collect
- [ ] `collect_enumerate_then_equals` — enumerate + collect

### Semantic pin
- [ ] `collect_stride_semantic_pin` — test that ONLY passes when collect stores at narrowed stride (the regression guard)

### Negative pin
- [ ] `collect_wrong_stride_negative` — verifies the broken behavior is rejected

### Verify tests fail before fix
- [ ] All new tests fail against current code

---

## 2.5 Fix Plan TPR Findings

**Gate:** Mandatory — severity is critical AND complexity-elevated subsystem (LLVM codegen)

Pending — will run after /tp-help consensus.

---

## 3. Implementation

- [ ] **Add `generate_trunc_narrowing_trampoline()`** in `trampolines.rs`
  - Symmetric to existing `generate_sext_widening_trampoline()`
  - Signature: `(env: ptr, in_ptr: ptr, out_ptr: ptr) -> void` (standard Map trampoline)
  - Body: load canonical i64 from `in_ptr`, trunc to narrowed width, store to `out_ptr`

- [ ] **Modify `emit_iter_collect()`** in `iterator_consumers.rs`
  - When narrowing is active for int elements: wrap iterator with `ori_iter_map(iter, trunc_trampoline, null_env, canonical_size=8)`
  - Pass narrowed elem_size (from `narrowed_int_collection_element_width() / 8`) instead of `element_store_size()`
  - When narrowing is NOT active: no change (existing canonical path)

---

## R. Third Party Review Findings

{Initially empty — populated during Phase 5 completion checklist.}

---

## 4. Completion Checklist

- [ ] All new tests pass unchanged after fix
- [ ] Matrix completeness verified
- [ ] Debug AND release builds pass
- [ ] Interpreter and LLVM produce identical results for all new tests
- [ ] `ORI_CHECK_LEAKS=1` reports zero leaks
- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] `cargo test -p ori_llvm` green
- [ ] `/commit-push` — commit all changes before review
- [ ] Plan TPR (Phase 2.5) — pending
- [ ] `/tpr-review` (Phase 5 — code review) passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed
- [ ] Bug entry updated: `- [x]` with resolution details
- [ ] Fix section status updated to `complete`
- [ ] Bug-tracker overview open bug count updated
- [ ] Final `/commit-push`

**Exit Criteria:** `[1,2,3].iter().map((x) -> x * 1000).collect() == [1000,2000,3000]` returns true in AOT (currently false). All existing iterator/collect tests pass. `str()` of collected int lists produces correct output. Zero regressions in `./test-all.sh`.
