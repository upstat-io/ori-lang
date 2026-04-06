---
section: "04"
title: "Drift, Gap & Polish"
status: not-started
reviewed: false
goal: "Fix the remaining DRIFT, GAP, and WASTE findings from the hygiene review"
success_criteria:
  - "cook() match is exhaustive — no catch-all arm for non-trivial tags"
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

- [ ] Adding a new non-trivial `RawTag` variant causes a compile error in `cook()` (not silent fallthrough)
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

The `cook()` method (line 164) has a `_ =>` catch-all at line 252 that delegates to `try_trivial()`. This is defensive — it handles operator/delimiter tags that normally take the fast path in the driver. However, it means adding a new non-trivial `RawTag` variant (e.g., a new literal type) won't cause a compile error in `cook()`.

- [ ] Replace the `_ =>` catch-all with explicit arms for every trivial tag that might reach `cook()`:
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

- [ ] **Alternative** (simpler): Keep the `_ =>` catch-all but add a compile-time exhaustiveness test that iterates `RawTag::ALL` and asserts every variant is explicitly handled in either `cook()`'s named arms or `try_trivial()`. This is the test-time enforcement pattern from `impl-hygiene.md`.

- [ ] Choose the approach that gives the strongest guarantee without excessive code. The exhaustiveness test approach is preferred if listing all 49 trivial tags in `cook()` is too verbose.

- [ ] Verify: `timeout 150 cargo test -p ori_lexer` green

---

## 04.2 Soft keyword sync guard

**File(s):** `compiler/ori_lexer/src/keywords/tests.rs`

Add a test that validates consistency between the `SOFT_KEYWORDS` table and the `could_be_soft_keyword()` pre-filter.

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

- [ ] Note: `SOFT_KEYWORDS` is `pub(crate)` — accessible from tests. If not, make it `pub(crate)` or expose via a test helper.

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

- [ ] `cook()` either has exhaustive match or exhaustiveness enforcement test
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

**Exit Criteria:** All three remaining findings resolved. Adding a new `RawTag` variant produces either a compile error or a test failure in the cooker. The `SOFT_KEYWORDS` table is guaranteed consistent with its pre-filter. No duplicate helper functions remain.
