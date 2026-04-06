---
section: "04"
title: "Drift, Gap & Polish"
status: not-started
reviewed: false
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
    status: not-started
  - id: "04.2"
    title: "Soft keyword sync guard"
    status: not-started
  - id: "04.3"
    title: "Unify span helper"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Drift, Gap & Polish

**Status:** Not Started
**Goal:** Fix the remaining DRIFT, GAP, and WASTE findings from the hygiene review — three targeted fixes that improve correctness guarantees and eliminate minor duplication.

**Success Criteria:**

- [ ] Adding a new non-trivial `RawTag` variant produces an explicit guard failure in lexer verification (compile error or targeted drift-test failure), not silent fallthrough
- [ ] A test validates that `SOFT_KEYWORDS` and `could_be_soft_keyword()` are consistent
- [ ] Only one `span(offset, len)` helper exists, shared by cooker and driver
- [ ] Satisfies mission criteria for exhaustive match, sync guard, and span dedup

**Context:** Three findings from the hygiene review that don't fit into the algorithmic DRY sections:
1. **DRIFT**: `cook()`'s `_ =>` catch-all silently absorbs new `RawTag` variants
2. **GAP**: No test guards consistency between `SOFT_KEYWORDS` array and `could_be_soft_keyword()` pre-filter
3. **WASTE**: Two identical span helper functions in cooker and driver

**Depends on:** Section 01 (bug fix must land first).

---

## 04.1 Exhaustive match in cook()

**File(s):** `compiler/ori_lexer/src/cooker/mod.rs`

The `cook()` method (line 164) has a `_ =>` catch-all at line 252 that delegates to `try_trivial()`. This is defensive — it handles operator/delimiter tags that normally take the fast path in the driver. However, it means adding a new non-trivial `RawTag` variant (e.g., a new literal type) won't automatically trip `cook()` itself. The repo already has an adjacent drift guard for fixed-lexeme trivial routing in `compiler/ori_lexer/src/trivial/tests.rs`, so this subsection should strengthen the non-trivial side without duplicating 54 trivial arms unless the result stays small and clear.

- [ ] Preferred approach: keep trivial routing centralized in `try_trivial()`, but add a targeted exhaustiveness guard that derives from `RawTag::ALL` and fails when a variant is neither:
  - explicitly handled in `cook()`, nor
  - intentionally routed through `try_trivial()`, nor
  - intentionally excluded with a documented reason

- [ ] One concrete implementation is a test in `compiler/ori_lexer/src/cooker/tests.rs` or `compiler/ori_lexer/src/trivial/tests.rs` that enumerates `RawTag::ALL` and asserts each variant belongs to exactly one bucket (`cook` explicit arm / `try_trivial` / explicit exclusion). This gives the same drift protection while avoiding a giant repeated tag list in `cook()`.

- [ ] Alternative: replace the `_ =>` catch-all with explicit arms for every trivial tag that might reach `cook()`, but only take this route if the resulting `compiler/ori_lexer/src/cooker/mod.rs` remains comfortably under the 500-line hygiene limit (current size: 417 lines).
  ```rust
  // Replace the catch-all with explicit arms for trivial tags.
  // This list is maintained in sync with try_trivial() — if a new
  // trivial tag is added, it must appear in BOTH places.
  // Using | patterns to keep it compact.
  RawTag::Plus | RawTag::Minus | RawTag::Star | RawTag::Slash
  | RawTag::Percent | RawTag::Caret | RawTag::Ampersand | RawTag::Pipe
  | RawTag::Tilde | RawTag::Bang | RawTag::Equal | RawTag::Less
  | RawTag::Greater | RawTag::Dot | RawTag::Question
  | RawTag::EqualEqual | RawTag::BangEqual | RawTag::LessEqual
  // ... (all trivial tags from try_trivial())
  => {
      if let Some((kind, tag_byte)) = crate::trivial::try_trivial(tag) {
          CookResult { kind, tag: tag_byte, had_error: false, contextual_kw: false }
      } else {
          debug_assert!(false, "tag listed as trivial in cook() but not in try_trivial(): {tag:?}");
          CookResult::new(TokenKind::Error)
      }
  }
  ```

- [ ] Choose the approach that gives the strongest guarantee without excessive code. The `RawTag::ALL` drift-test approach is preferred because `compiler/ori_lexer/src/cooker/mod.rs` is already 417 lines and the explicit-arm variant is likely to push the file toward the 500-line limit.

- [ ] Verify: `timeout 150 cargo test -p ori_lexer` green

---

## 04.2 Soft keyword sync guard

**File(s):** `compiler/ori_lexer/src/keywords/tests.rs`

Add a test that validates consistency between the `SOFT_KEYWORDS` table and the `could_be_soft_keyword()` pre-filter. This complements the existing literal-list prefilter tests in `compiler/ori_lexer/src/keywords/tests.rs`; the new guard should derive from the table itself so future keyword additions only need one source of truth.

- [ ] Add test `test_soft_keyword_prefilter_consistency`:
  ```rust
  #[test]
  fn test_soft_keyword_prefilter_consistency() {
      // Every soft keyword must pass the pre-filter
      for (kw, _) in &keywords::SOFT_KEYWORDS {
          assert!(
              keywords::could_be_soft_keyword(kw),
              "SOFT_KEYWORDS entry '{kw}' not accepted by could_be_soft_keyword()"
          );
      }
  }
  ```

- [ ] This test catches the scenario where a new soft keyword is added to `SOFT_KEYWORDS` with a length or first byte not covered by `could_be_soft_keyword()`'s pre-filter.

- [ ] Note: `SOFT_KEYWORDS` is currently private, but `compiler/ori_lexer/src/keywords/tests.rs` is a child module of `keywords`, so it can access the table directly without widening visibility.

- [ ] Verify: `timeout 150 cargo test -p ori_lexer` green

---

## 04.3 Unify span helper

**File(s):** `compiler/ori_lexer/src/cooker/mod.rs`, `compiler/ori_lexer/src/driver.rs`

Two identical functions exist:
- `cooker::span(offset, len)` at `cooker/mod.rs:404-407` — `pub(super)`, used by cooker + submodules
- `driver::make_span(offset, len)` at `driver.rs:237-239` — private, used by driver

- [ ] Move the function to a shared location (e.g., a `util` helper in `lib.rs` or keep in `cooker/mod.rs` with `pub(crate)` visibility)
- [ ] Update `driver.rs` to use the shared function
- [ ] Remove the duplicate

- [ ] Verify: `timeout 150 cargo test -p ori_lexer` green

---

## 04.R Third Party Review Findings

- None.

---

## 04.N Completion Checklist

- [ ] `cook()` / `try_trivial()` routing has an explicit exhaustiveness guard derived from `RawTag::ALL`
- [ ] Soft keyword sync guard test exists and passes
- [ ] Single span helper function shared by cooker and driver
- [ ] `timeout 150 cargo test -p ori_lexer` green (debug)
- [ ] `timeout 150 cargo test -p ori_lexer --release` green (release)
- [ ] `timeout 150 ./test-all.sh` green — no regressions
- [ ] Plan annotation cleanup: no stale annotations
- [ ] **Plan sync** — update plan metadata:
  - [ ] This section's frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference table updated
  - [ ] `index.md` section status updated
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** All three remaining findings resolved. Adding a new `RawTag` variant produces an explicit guard failure (compile error or targeted drift-test failure) instead of silent fallthrough. The `SOFT_KEYWORDS` table is guaranteed consistent with its pre-filter. No duplicate helper functions remain.
