---
section: "10"
title: Testing Improvements
status: not-started
priority: medium
goal: Add missing tests, fix flaky tests, improve test organization
files:
  - compiler/oric/src/typeck.rs
  - compiler/oric/src/eval/module/import.rs
  - compiler/ori_parse/src/grammar/ty.rs
  - compiler/ori_llvm/src/aot/multi_file.rs
  - compiler/ori_llvm/src/aot/linker/wasm.rs
  - compiler/ori_llvm/src/aot/incremental/hash.rs
---

# Section 10: Testing Improvements

**Status:** ✅ Tracked (incremental improvements)
**Priority:** MEDIUM — Technical debt, not blocking features
**Goal:** Comprehensive test coverage, no flaky tests, clear organization

**Assessment:** Test suite is healthy:
- 6,368 tests pass (workspace + LLVM + Ori spec)
- No known flaky tests in CI
- Items below are tracked for incremental improvement

---

## Guidelines

From `.claude/rules/tests.md`:
- Inline tests < 200 lines; longer → `tests/` subdirectory
- Clear naming: `test_parses_nested_generics`, not `test_1`
- AAA structure: Arrange-Act-Assert clearly separated
- No flaky tests (timing, shared state, order-dependent)
- `#[ignore]` must have tracking issue comment

---

## 10.1 Add Missing Tests for Public Functions

### typeck.rs (CRITICAL)

Location: `compiler/oric/src/typeck.rs:87-463`

4 public functions with no tests:

- [ ] Add tests for `type_check_with_context`
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn test_type_check_with_context_simple_function() {
          // Arrange: Create minimal module with one function
          // Act: Call type_check_with_context
          // Assert: No errors, types resolved correctly
      }

      #[test]
      fn test_type_check_with_context_type_error() {
          // Test that type errors are accumulated
      }

      #[test]
      fn test_type_check_with_context_import_resolution() {
          // Test with imports
      }
  }
  ```

- [ ] Add tests for `resolve_imports_for_type_checking`
- [ ] Add tests for `type_check_with_imports`
- [ ] Add tests for `type_check_with_imports_and_source`

### import.rs Edge Cases

Location: `compiler/oric/src/eval/module/import.rs`

- [ ] Add test for empty paths
- [ ] Add test for Unicode filenames
- [ ] Add test for circular imports (error case)
- [ ] Add test for missing permissions (error case)
- [ ] Add test for symlink handling
- [ ] Add test for case sensitivity (platform-dependent)

---

## 10.2 Move Large Inline Test Modules

Files with inline test modules exceeding 200 lines:

### ori_parse

- [ ] `grammar/ty.rs` (357 lines) → `tests/type_parsing.rs`
- [ ] `grammar/attr.rs` (202 lines) → `tests/attribute_parsing.rs`

### ori_llvm

- [ ] `aot/multi_file.rs` (262 lines) → `tests/multi_file_compilation.rs`
- [ ] `aot/linker/wasm.rs` (237 lines) → `tests/wasm_linking.rs`
- [ ] `aot/incremental/hash.rs` (219 lines) → `tests/incremental_hash.rs`
- [ ] `aot/incremental/deps.rs` (203 lines) → `tests/dependency_graph.rs`

### ori_eval

- [ ] `interpreter/scope_guard.rs` (239 lines) → `tests/scope_guard.rs`
- [ ] `module_registration.rs` (233 lines) → `tests/module_registration.rs`

### ori_ir

- [ ] `incremental.rs` (229 lines) → `tests/incremental.rs`
- [ ] `token.rs` (259 lines) → `tests/token.rs`

### oric

- [ ] `edit/tracker.rs` (203 lines) → `tests/edit_tracking.rs`
- [ ] `suggest.rs` (204 lines) → `tests/suggestion.rs`

### Migration Pattern

```rust
// In src/module.rs:
// Remove #[cfg(test)] mod tests { ... }

// Create tests/module_tests.rs:
use crate::module::*;

#[test]
fn test_feature_x() { ... }
```

---

## 10.3 Fix Flaky Tests

### hash.rs: SystemTime for Randomness

Location: `compiler/ori_llvm/src/aot/incremental/hash.rs:314-320`

- [ ] Replace `rand_suffix()` using `SystemTime`
  ```rust
  // Before:
  fn rand_suffix() -> String {
      SystemTime::now()
          .duration_since(UNIX_EPOCH)
          .unwrap()
          .as_nanos()
          .to_string()
  }

  // After: Use atomic counter or tempfile crate
  use std::sync::atomic::{AtomicU64, Ordering};
  static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

  fn unique_suffix() -> String {
      TEST_COUNTER.fetch_add(1, Ordering::SeqCst).to_string()
  }
  ```

### parallel_tests.rs: Timing Dependencies

Location: `compiler/ori_patterns/src/parallel_tests.rs:617-886`

- [ ] Review Duration usage — verify simulated, not actual sleeps
- [ ] Ensure tests don't depend on wall clock timing

---

## 10.4 Improve Test Naming

### Generic Names to Fix

Location: `compiler/ori_llvm/src/tests/collection_tests.rs:207-339`

- [ ] `test_some` → `test_some_variant_wraps_value`
- [ ] `test_none` → `test_none_variant_equals_none`
- [ ] `test_ok` → `test_ok_variant_wraps_success_value`
- [ ] `test_err` → `test_err_variant_wraps_error_value`

Location: `compiler/ori_llvm/src/tests/operator_tests.rs:12-331`

- [ ] `test_subtract` → `test_subtract_integers_returns_difference`
- [ ] `test_multiply` → `test_multiply_integers_returns_product`
- [ ] `test_divide` → `test_divide_integers_returns_quotient`

Location: `compiler/ori_parse/src/context.rs:188`

- [ ] `test_union` → `test_union_combines_context_flags`

Location: `compiler/ori_parse/src/progress.rs:370`

- [ ] `test_map` → `test_map_transforms_parse_result_value`

---

## 10.5 Fix Test Helper Naming

### Rename test_ Prefixed Helpers

Location: `compiler/oric/src/testing/mocks.rs:11-66`

These are factories, not tests:

- [ ] `test_int` → `mock_int_value`
- [ ] `test_str` → `mock_str_value`
- [ ] `test_bool` → `mock_bool_value`
- [ ] `test_list` → `mock_list_value`

Location: `compiler/oric/tests/phases/common/parse.rs:94`

- [ ] `test_interner()` → `create_test_interner()` or `default_interner()`

---

## 10.6 Add Edge Case Tests

### suggest.rs Unicode

- [ ] Test combining characters
- [ ] Test RTL text
- [ ] Test emoji in identifiers (error case)
- [ ] Test zero-width characters
- [ ] Test normalization forms (NFC vs NFD)

### import.rs Paths

- [ ] Test paths with spaces
- [ ] Test very long paths
- [ ] Test relative path edge cases (`./`, `../`, `../../`)
- [ ] Test absolute vs relative resolution

---

## 10.7 Add Compile-Fail Tests

### import.rs

- [ ] Test for non-existent imports (expected error)
- [ ] Test for circular dependencies (expected error)
- [ ] Test for invalid import syntax (expected error)
- [ ] Test for private member access violations (expected error)

### Type Checker

- [ ] Verify error messages match expected output for common type errors
- [ ] Use snapshot testing for complex error output

---

## 10.8 Verification

- [ ] `./test-all.sh` passes
- [ ] No `#[ignore]` without tracking issue
- [ ] All inline test modules < 200 lines
- [ ] Test names describe behavior

---

## 10.N Completion Checklist

- [x] Test suite passes (6,368 tests)
- [x] No known flaky tests in CI
- [ ] Incremental: Move large inline test modules (tracked)
- [ ] Incremental: Add missing tests for public functions (tracked)
- [ ] Incremental: Improve test naming (tracked)
- [ ] Incremental: Add edge case tests (tracked)

**Exit Criteria:** ✅ Test suite healthy; incremental improvements tracked for future work
