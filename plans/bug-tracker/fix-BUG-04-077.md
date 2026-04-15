---
bug: "BUG-04-077"
title: "Collect output boundary ABI mismatch: collected List<int> has canonical i64 stride but list_traits/debug_helpers read with narrowed i8 stride"
severity: "critical"
status: complete
goal: "Prevent stride mismatch between collect output and list readers"
success_criteria:
  - "[1,2,3].iter().map((x) -> x * 1000).collect() == [1000,2000,3000] returns true in AOT"
  - "str([1,2,3].iter().map((x) -> x * 1000).collect()) produces [1000, 2000, 3000]"
  - "All existing iterator/collect AOT tests pass"
  - "ORI_CHECK_LEAKS=1 reports zero leaks on collect test programs"
subsystem: "ori_repr (narrowing/int.rs), ori_llvm (narrowing_codegen.rs, list_traits.rs, debug_helpers.rs, iterator_consumers.rs)"
found: "2026-04-14"
source: "tpr-review"
third_party_review:
  status: none
  updated: null
---

# Fix: BUG-04-077 — Collect output boundary ABI mismatch

**Status:** Complete (resolved by disabling collection element narrowing for soundness)
**Severity:** Critical
**Resolution:** Collection element narrowing (Phase C) disabled at the repr level. Re-enablement tracked in `plans/repr-opt/section-11-collection-spec.md:230`.

---

## 1. Root Cause Analysis

- **Symptom**: `[1,2,3].iter().map((x) -> x * 1000).collect() == [1000,2000,3000]` returns false in AOT. `str()` of collected list produces garbage.
- **Proximate cause**: `emit_iter_collect()` at `iterator_consumers.rs:26` uses `element_store_size(elem_ty)` (canonical = 8 bytes for int). The runtime stores 8 bytes per element in the output list. But `list_traits.rs` (equals/compare/hash at lines 64/147/222) and `debug_helpers.rs` (display at line 412) use `int_element_llvm_type()` which returns i8 when ANY `List<int>` in the program has narrowed elements.
- **Root cause**: The narrowing analysis (`narrow_collection_elements` in `ori_repr/src/narrowing/int.rs`) only examines literal construction sites (e.g., `[1,2,3]` fits in i8). It does NOT account for runtime-computed values from `collect()`, `map()`, `filter()`, or other iterator pipeline transformations. Since all `List<int>` share one `ReprPlan` entry (narrowing is per-TYPE, not per-INSTANCE), the analysis incorrectly narrows when ANY literal fits, even if computed values from collect exceed the narrow range.
- **Blast radius**: Every collected `List<int>` when narrowing is active. Affects equality, comparison, hash, debug/display — all produce wrong results on collected lists.
- **Resolution approach**: Disable collection element narrowing entirely at the repr level. This is the correct conservative fix because:
  1. The narrowing analysis cannot see collect output values (they're computed at runtime)
  2. A trunc trampoline at the collect boundary would TRUNCATE values exceeding the narrow range (e.g., 1000 → i8 = garbage) — this is data corruption, not a fix
  3. Struct field narrowing and local variable narrowing are unaffected (they have different scoping rules)
  4. Re-enablement requires extending the analysis to account for ALL value sources, which is a plan-level change tracked in `plans/repr-opt/section-11-collection-spec.md:230`

---

## 1.5 Fix Consensus (via /tp-help)

Independent dual-source design review. Run: `/tmp/ori-tpr-dp1o2jmU`

- **Proposed approach (pre-consensus)**: Trunc narrowing trampoline at collect boundary, symmetric with sext widening at iter().
- **Round 1 outcome**: Both Codex and Gemini agreed on the trunc adapter approach.
- **Post-consensus reassessment (Phase 1 deeper investigation)**: The trunc approach is unsound — it would truncate values exceeding the narrowed range (e.g., `map(x -> x * 1000)` produces 1000, trunc to i8 = -24). The correct fix is disabling the analysis, not adding a trunc at the boundary. The trunc approach is ONLY valid after the narrowing analysis is extended to account for collect output — at that point, narrowing would only fire when ALL values (including collected ones) provably fit, making the trunc a no-op for data (but necessary for ABI stride consistency).
- **Final resolution**: Disable collection element narrowing. The consensus's trunc approach is preserved as the re-enablement strategy in `plans/repr-opt/section-11-collection-spec.md`.

---

## 2. Resolution

Collection element narrowing disabled in `ori_repr/src/narrowing/int.rs:204-208` (function body replaced with comment). 7 Phase C unit tests marked `#[ignore = "BUG-04-077: collection element narrowing disabled"]`.

**Capability regression tracking (CLAUDE.md §Phase 4 step 6):**
- **What was disabled**: Collection element narrowing (Phase C in `ori_repr/narrowing/int.rs`)
- **Why**: The range analysis only sees literal construction sites, not runtime-computed collect output. Per-type narrowing creates a stride mismatch between producers (collect at canonical stride) and consumers (readers at narrowed stride).
- **Re-enablement path**: `plans/repr-opt/section-11-collection-spec.md:230` — requires extending the narrowing analysis to account for all value sources. Also referenced in `plans/perf-engineering/00-overview.md:122`.
- **Ignored tests**: 7 tests in `compiler/ori_repr/src/narrowing/tests.rs` reference the re-enablement item.

**Also resolves**: BUG-04-078 (set_builtins sorted_keys/sorted_values boundary mismatch — same root cause class, same mitigation).

---

## 4. Completion Checklist

- [x] Bug is resolved (collection narrowing disabled — no stride mismatch possible)
- [x] All existing tests pass (narrowing disable is transparent — canonical stride used everywhere)
- [x] Re-enablement tracked: `plans/repr-opt/section-11-collection-spec.md:230`
- [x] Re-enablement tracked: `plans/perf-engineering/00-overview.md:122`
- [x] 7 Phase C unit tests marked `#[ignore = "BUG-04-077"]`
- [x] Bug entry updated: `- [x]` with resolution details
- [x] BUG-04-078 updated: `- [x]` (same root cause, same mitigation)
- [x] Fix section status updated to `complete`

**Exit Criteria:** `[1,2,3].iter().map((x) -> x * 1000).collect() == [1000,2000,3000]` returns true in AOT — confirmed (exit code 0). All existing tests pass. No stride mismatch possible with narrowing disabled.
