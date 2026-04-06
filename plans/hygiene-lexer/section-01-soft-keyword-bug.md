---
section: "01"
title: "Bug Fix — Soft Keyword Cache Contamination"
status: in-progress
reviewed: true
goal: "Fix BUG-01-001: IdentCache bypasses soft keyword resolution for previously-seen identifiers"
success_criteria:
  - "Soft keywords resolve correctly even when the same text appeared earlier as an identifier"
  - "Regression test covers all 6 soft keywords (cache, catch, parallel, recurse, spawn, timeout)"
  - "No performance regression in lexer benchmarks (cache still works for hard keywords and identifiers)"
inspired_by:
  - "rustc_lexer — no identifier cache, re-resolves every token (correctness over speed)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Fix IdentCache to exclude soft keyword candidates"
    status: complete
  - id: "01.2"
    title: "Regression tests"
    status: complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 01: Bug Fix — Soft Keyword Cache Contamination

**Status:** Not Started
**Goal:** Fix BUG-01-001 so that soft keywords (context-sensitive pattern keywords) resolve correctly regardless of whether their text appeared earlier as a regular identifier.

**Success Criteria:**

- [x] `let cache = 42; cache(key: "x", op: () -> 1)` lexes second `cache` as `TokenKind::Cache` (not `Ident`)
- [x] All 6 soft keywords tested: cache, catch, parallel, recurse, spawn, timeout
- [x] Hard keyword caching still works (no performance regression for `let`, `if`, `type`, etc.)
- [x] Identifier caching still works for non-soft-keyword identifiers
- [x] Satisfies mission criterion: "BUG-01-001 fixed with regression tests"

**Context:** The `IdentCache` in `compiler/ori_lexer/src/cooker/identifier.rs` is a direct-mapped 256-entry array that caches `cook_ident()` results. The cache correctly excludes soft keywords from being *inserted* (line 332-337 in `cooker/mod.rs` returns without caching). However, when soft keyword text (e.g., "cache") first appears as a regular identifier (not followed by `(`), it IS cached as `TokenKind::Ident(Name)` at line 364. On subsequent occurrences — even in keyword context — the cache hit at line 320 returns `Ident` immediately, bypassing `soft_keyword_lookup()` entirely. This is a correctness bug: valid programs are tokenized incorrectly.

**Depends on:** None (this is the first section).

---

## 01.1 Fix IdentCache to exclude soft keyword candidates

**File(s):** `compiler/ori_lexer/src/cooker/mod.rs`

The fix is surgical: before caching an identifier at line 364, check whether the text could be a soft keyword. If it could, skip caching — the same way the code already skips caching for soft keyword *hits* (line 332).

- [x] **Write regression tests FIRST** (TDD — see 01.2 below). Verify they fail before the fix.

- [x] In `cook_ident()` (`compiler/ori_lexer/src/cooker/mod.rs:362-364`), guard the cache insert:
  ```rust
  // Before (line 362-364):
  let kind = TokenKind::Ident(self.interner.intern(text));
  self.ident_cache.insert(text, kind.clone());

  // After:
  let kind = TokenKind::Ident(self.interner.intern(text));
  // Don't cache identifiers that could be soft keywords — they are
  // context-sensitive and must be re-evaluated on every occurrence.
  if !keywords::could_be_soft_keyword(text) {
      self.ident_cache.insert(text, kind.clone());
  }
  ```

- [x] Verify: the `could_be_soft_keyword()` pre-filter (`keywords/mod.rs:161-163`) already checks length (5, 7, 8) and first byte (`c`, `p`, `r`, `s`, `t`) — this is a fast O(1) check that rejects >99% of identifiers. No meaningful perf impact.

---

## 01.2 Regression tests

**File(s):** `compiler/ori_lexer/src/cooker/tests.rs` (or `compiler/ori_lexer/src/tests.rs`)

Write tests that exercise the specific failure mode: soft keyword text appearing first as identifier, then in keyword context.

- [x] Test matrix — all 6 soft keywords, each in both orderings:
  - [x] **Identifier-then-keyword**: `let cache = 42; cache(key: "x", op: () -> 1)` — second occurrence must be `TokenKind::Cache`
  - [x] **Keyword-then-identifier**: `cache(key: "x", op: () -> 1); let cache = 42` — second occurrence must be `TokenKind::Ident`
  - [x] **Keyword-only**: `cache(key: "x", op: () -> 1)` — must be `TokenKind::Cache`
  - [x] **Identifier-only**: `let cache = 42` — must be `TokenKind::Ident`

- [x] Semantic pin: at least one test that ONLY passes with the fix (the identifier-then-keyword case)

- [x] Negative pin: verify that caching still works for hard keywords — `let x = 1; let y = 2` should still hit the cache for the second `let`

- [x] Run `timeout 150 cargo test -p ori_lexer` — all tests pass in debug
- [x] Run `timeout 150 cargo test -p ori_lexer --release` — all tests pass in release

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [x] Regression tests exist and pass for all 6 soft keywords in identifier-then-keyword ordering
- [x] Semantic pin test exists that only passes with the fix
- [x] Hard keyword and regular identifier caching still works (no perf regression)
- [x] `timeout 150 cargo test -p ori_lexer` green (debug)
- [x] `timeout 150 cargo test -p ori_lexer --release` green (release)
- [x] `timeout 150 ./test-all.sh` green — no regressions
- [x] BUG-01-001 marked resolved in `plans/bug-tracker/section-01-parser-lexer.md`
- [x] Plan annotation cleanup: no stale annotations added
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** BUG-01-001 is fixed. The test `test_soft_keyword_after_identifier_cache` (or equivalent) passes, proving that `cache(...)` is tokenized as the `cache` keyword even when `cache` was previously seen as an identifier. No existing test regressions. No lexer benchmark regression.
