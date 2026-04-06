---
section: "04"
title: "Drift, Gap & Polish"
status: in-progress
reviewed: true
goal: "Fix the remaining DRIFT, GAP, and WASTE findings from the hygiene review"
success_criteria:
  - "Non-trivial RawTag additions trip an explicit cook()/trivial-routing guard — by compile error or targeted drift-test failure"
  - "Sync guard test exists for SOFT_KEYWORDS ↔ could_be_soft_keyword consistency"
  - "Duplicate span()/make_span() unified to single shared function"
inspired_by: []
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Exhaustive match in cook()"
    status: complete
  - id: "04.2"
    title: "Soft keyword sync guard"
    status: complete
  - id: "04.3"
    title: "Unify span helper"
    status: complete
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 04: Drift, Gap & Polish

**Status:** In Progress
**Goal:** Fix the remaining DRIFT, GAP, and WASTE findings from the hygiene review — three targeted fixes that improve correctness guarantees and eliminate minor duplication.

**Success Criteria:**

- [x] Adding a new non-trivial `RawTag` variant produces an explicit guard failure in lexer verification (compile error or targeted drift-test failure), not silent fallthrough
- [x] A test validates that `SOFT_KEYWORDS` and `could_be_soft_keyword()` are consistent
- [x] Only one `span(offset, len)` helper exists, shared by cooker and driver
- [x] Satisfies mission criteria for exhaustive match, sync guard, and span dedup

**Context:** Three findings from the hygiene review that don't fit into the algorithmic DRY sections:
1. **DRIFT**: `cook()`'s `_ =>` catch-all silently absorbs new `RawTag` variants
2. **GAP**: No test guards consistency between `SOFT_KEYWORDS` array and `could_be_soft_keyword()` pre-filter
3. **WASTE**: Two identical span helper functions in cooker and driver

**Depends on:** Section 01 (bug fix must land first).

---

## 04.1 Exhaustive match in cook()

**File(s):** `compiler/ori_lexer/src/cooker/mod.rs`

The `cook()` method (line 164) has a `_ =>` catch-all at line 252 that delegates to `try_trivial()`. This is defensive — it handles operator/delimiter tags that normally take the fast path in the driver. However, it means adding a new non-trivial `RawTag` variant (e.g., a new literal type) won't automatically trip `cook()` itself. The repo already has an adjacent drift guard for fixed-lexeme trivial routing in `compiler/ori_lexer/src/trivial/tests.rs`, so this subsection should strengthen the non-trivial side without duplicating 54 trivial arms unless the result stays small and clear.

- [x] Preferred approach: keep trivial routing centralized in `try_trivial()`, but add a targeted exhaustiveness guard that derives from `RawTag::ALL` and fails when a variant is neither:
  - explicitly handled in `cook()`, nor
  - intentionally routed through `try_trivial()`, nor
  - intentionally excluded with a documented reason

- [x] One concrete implementation is a test in `compiler/ori_lexer/src/trivial/tests.rs` (`every_raw_tag_has_explicit_routing`) that enumerates `RawTag::ALL` and asserts each variant belongs to exactly one bucket (`cook` explicit arm / `try_trivial`). 26 cooked + 54 trivial = 80 = `RawTag::ALL.len()`.

- [x] Alternative rejected: explicit arms would push `cooker/mod.rs` (417 lines) toward the 500-line limit. The `RawTag::ALL` drift-test gives identical protection with zero production code change.

- [x] Verify: `timeout 150 cargo test -p ori_lexer` green — 296 passed (2026-04-06)

---

## 04.2 Soft keyword sync guard

**File(s):** `compiler/ori_lexer/src/keywords/tests.rs`

Add a test that validates consistency between the `SOFT_KEYWORDS` table and the `could_be_soft_keyword()` pre-filter. This complements the existing literal-list prefilter tests in `compiler/ori_lexer/src/keywords/tests.rs`; the new guard should derive from the table itself so future keyword additions only need one source of truth.

- [x] Added test `soft_keyword_prefilter_consistency` in `compiler/ori_lexer/src/keywords/tests.rs` — iterates `SOFT_KEYWORDS` table directly and asserts each entry passes `could_be_soft_keyword()`.

- [x] This test catches the scenario where a new soft keyword is added to `SOFT_KEYWORDS` with a length or first byte not covered by `could_be_soft_keyword()`'s pre-filter.

- [x] Note: `SOFT_KEYWORDS` is private; the test accesses it via `use super::*` as a child module of `keywords`.

- [x] Verify: `timeout 150 cargo test -p ori_lexer` green — 296 passed (2026-04-06)

---

## 04.3 Unify span helper

**File(s):** `compiler/ori_lexer/src/cooker/mod.rs`, `compiler/ori_lexer/src/driver.rs`

Two identical functions exist:
- `cooker::span(offset, len)` at `cooker/mod.rs:406-410` — `pub(super)`, used by cooker + submodules
- `driver::make_span(offset, len)` at `driver.rs:237-239` — private, used by driver

- [x] Widened `cooker::span()` from `pub(super)` to `pub(crate)` in `cooker/mod.rs:408`
- [x] Updated `driver.rs:81` to use `crate::cooker::span(offset, raw.len)` instead of local `make_span()`
- [x] Removed duplicate `make_span()` function from `driver.rs` (was at lines 235-239)

- [x] Verify: `timeout 150 cargo test -p ori_lexer` green — 296 passed (2026-04-06)

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [x] `cook()` / `try_trivial()` routing has an explicit exhaustiveness guard derived from `RawTag::ALL`
- [x] Soft keyword sync guard test exists and passes
- [x] Single span helper function shared by cooker and driver
- [x] `timeout 150 cargo test -p ori_lexer` green (debug) — 296 passed (2026-04-06)
- [x] `timeout 150 cargo test -p ori_lexer --release` green (release) — 296 passed (2026-04-06)
- [x] `timeout 150 ./test-all.sh` — 0 failures, exits 0; LLVM backend crash is known BUG-04-030 (2026-04-06)
- [x] Plan annotation cleanup: no stale annotations
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** All three remaining findings resolved. Adding a new `RawTag` variant produces an explicit guard failure (compile error or targeted drift-test failure) instead of silent fallthrough. The `SOFT_KEYWORDS` table is guaranteed consistent with its pre-filter. No duplicate helper functions remain.
