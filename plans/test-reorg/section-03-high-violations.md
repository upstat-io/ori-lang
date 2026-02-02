---
section: 3
title: High Violations
status: completed
goal: Extract 500-800 line inline test modules
sections:
  - id: "3.1"
    title: Extract scalar_int.rs Tests
    status: completed
  - id: "3.2"
    title: Extract passes.rs Tests
    status: completed
  - id: "3.3"
    title: Extract errors.rs Tests
    status: completed
  - id: "3.4"
    title: Extract object.rs Tests
    status: completed
  - id: "3.5"
    title: Extract lexer Tests
    status: completed
  - id: "3.6"
    title: Completion Checklist
    status: completed
---

# Section 3: High Violations (500-800 lines)

**Status:** ✅ Completed
**Goal:** Extract 500-800 line inline test modules

---

## 3.1 Extract scalar_int.rs Tests

**Source:** `ori_patterns/src/value/scalar_int.rs` (786 lines)
**Target:** `tests/phases/eval/scalar_int.rs`

- [ ] Analyze test content:
  - [ ] Integer pattern matching tests
  - [ ] Range pattern tests
  - [ ] Edge case tests (overflow, boundaries)

- [ ] Create `tests/phases/eval/scalar_int.rs`

- [ ] Move tests:
  - [ ] Copy tests to target file
  - [ ] Add necessary imports
  - [ ] Update `eval/mod.rs`

- [ ] Verify tests pass:
  ```bash
  cargo test --test phases scalar_int
  cargo test -p ori_patterns scalar_int
  ```

- [ ] Delete extracted tests from source

- [ ] Verify source is < 200 lines of test code

---

## 3.2 Extract passes.rs Tests

**Source:** `ori_llvm/src/aot/passes.rs` (636 lines)
**Target:** `tests/phases/codegen/optimization.rs`

- [ ] Analyze test content:
  - [ ] Optimization pass configuration
  - [ ] Pass ordering tests
  - [ ] Optimization level tests

- [ ] Create `tests/phases/codegen/optimization.rs`

- [ ] Move tests:
  - [ ] Copy tests to target file
  - [ ] Add necessary imports
  - [ ] Update `codegen/mod.rs`

- [ ] Verify tests pass:
  ```bash
  cargo test --test phases optimization
  cargo test -p ori_llvm passes
  ```

- [ ] Delete extracted tests from source

- [ ] Verify source is < 200 lines of test code

---

## 3.3 Extract errors.rs Tests

**Source:** `ori_patterns/src/errors.rs` (512 lines)
**Target:** `tests/phases/eval/pattern_errors.rs`

- [ ] Analyze test content:
  - [ ] Pattern match failure tests
  - [ ] Error message quality tests
  - [ ] Exhaustiveness error tests

- [ ] Create `tests/phases/eval/pattern_errors.rs`

- [ ] Move tests:
  - [ ] Copy tests to target file
  - [ ] Add necessary imports
  - [ ] Update `eval/mod.rs`

- [ ] Verify tests pass:
  ```bash
  cargo test --test phases pattern_errors
  cargo test -p ori_patterns errors
  ```

- [ ] Delete extracted tests from source

- [ ] Verify source is < 200 lines of test code

---

## 3.4 Extract object.rs Tests

**Source:** `ori_llvm/src/aot/object.rs` (466 lines)
**Target:** `tests/phases/codegen/object_emit.rs`

- [ ] Analyze test content:
  - [ ] Object file emission tests
  - [ ] Symbol table tests
  - [ ] Section tests

- [ ] Create `tests/phases/codegen/object_emit.rs`

- [ ] Move tests:
  - [ ] Copy tests to target file
  - [ ] Add necessary imports
  - [ ] Update `codegen/mod.rs`

- [ ] Verify tests pass:
  ```bash
  cargo test --test phases object_emit
  cargo test -p ori_llvm object
  ```

- [ ] Delete extracted tests from source

- [ ] Verify source is < 200 lines of test code

---

## 3.5 Extract lexer Tests

**Source:** `ori_lexer/src/lib.rs` (461 lines)
**Target:** `tests/phases/parse/lexer.rs`

- [ ] Analyze test content:
  - [ ] Token recognition tests
  - [ ] Span correctness tests
  - [ ] Edge case tests (unicode, escapes)

- [ ] Create `tests/phases/parse/lexer.rs`

- [ ] Move tests:
  - [ ] Copy tests to target file
  - [ ] Add necessary imports
  - [ ] Update `parse/mod.rs`

- [ ] Verify tests pass:
  ```bash
  cargo test --test phases lexer
  cargo test -p ori_lexer
  ```

- [ ] Delete extracted tests from source

- [ ] Verify source is < 200 lines of test code

---

## 3.6 Completion Checklist

- [x] `scalar_int.rs` inline tests < 200 lines (0 lines - all extracted)
- [x] `passes.rs` inline tests < 200 lines (0 lines - all extracted)
- [x] `errors.rs` inline tests < 200 lines (0 lines - all extracted)
- [x] `object.rs` inline tests < 200 lines (0 lines - all extracted)
- [x] `ori_lexer/src/lib.rs` inline tests < 200 lines (0 lines - all extracted)
- [x] All tests pass in phase tests (392 tests passing)
- [ ] No test regressions in CI

**Exit Criteria:** All 5 high-violation modules extracted; all tests passing; no module > 200 lines of inline tests.
