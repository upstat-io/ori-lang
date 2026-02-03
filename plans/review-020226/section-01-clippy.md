---
section: "01"
title: Clippy Errors & CI
status: completed
priority: critical
goal: Fix all clippy errors to unblock CI
files:
  - compiler/ori_parse/src/incremental.rs
---

# Section 01: Clippy Errors & CI

**Status:** ✅ Completed
**Priority:** CRITICAL — Blocks all other work
**Goal:** Fix 11 clippy errors in `ori_parse/src/incremental.rs` to restore CI

---

## 01.1 Fix Clippy Errors in incremental.rs

All errors were in `/home/eric/ori_lang/compiler/ori_parse/src/incremental.rs`:

### Logic Errors

- [x] **Line 195**: `if_not_else` — Flipped condition logic
  ```rust
  // Fixed: Swapped branches to use positive condition
  if self.marker.intersects(decl.span) { None } else { Some(decl) }
  ```

### Cast Precision Errors

- [x] **Line 231**: `cast_precision_loss` — Added `#[allow]` with justification
  ```rust
  #[allow(clippy::cast_precision_loss)] // Acceptable for percentage display - counts won't approach 2^52
  ```

- [x] **Line 430**: `cast_possible_truncation` — Added `#[allow]` with justification
  ```rust
  #[allow(clippy::cast_possible_truncation)] // Statement indices won't exceed u32::MAX in practice
  ```

### Documentation Errors

- [x] **Line 547**: `doc_markdown` — Added backticks around `ExprList`
- [x] **Line 812**: `doc_markdown` — Added backticks around `FunctionSeq`
- [x] **Line 908**: `doc_markdown` — Added backticks around `FunctionExp`

### Self-Recursion Warning

- [x] **Line 681**: `self_only_used_in_recursion` — Added targeted allow
  ```rust
  #[allow(clippy::self_only_used_in_recursion)] // Recursive copy pattern requires &self for consistency
  ```

---

## 01.2 Fix Test-Only Clippy Errors

- [x] **Line 1431-1432**: `unwrap_used`/`expect_used` — Refactored to use `let Some(...) = ... else { panic!() }`
- [x] **Line 1438**: `float_cmp` — Added scoped `#[allow(clippy::float_cmp)]` for exact zero comparison

---

## 01.3 Verification

- [x] Run `./clippy-all` — passes with no errors
- [x] Run `./test-all` — passes (6,367 tests, 0 failures)
- [x] Verify CI is green

---

## 01.N Completion Checklist

- [x] All 11 clippy errors fixed
- [x] `./clippy-all` passes
- [x] `./test-all` passes
- [x] No new warnings introduced

**Exit Criteria:** ✅ CI is green, `./clippy-all` produces no errors
