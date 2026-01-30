# Phase 7B: Option & Result

**Goal**: Option and Result type methods

> **SPEC**: `spec/11-built-in-functions.md`

---

## 7B.1 Option Functions

- [ ] **Implement**: `is_some(x)` — spec/11-built-in-functions.md § is_some
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_some tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_some
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — is_some codegen

- [ ] **Implement**: `is_none(x)` — spec/11-built-in-functions.md § is_none
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_none tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_none
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — is_none codegen

- [ ] **Implement**: `Option.map` — spec/11-built-in-functions.md § Option.map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.map
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.map codegen

- [ ] **Implement**: `Option.unwrap_or` — spec/11-built-in-functions.md § Option.unwrap_or
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.unwrap_or tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.unwrap_or
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.unwrap_or codegen

- [ ] **Implement**: `Option.ok_or` — spec/11-built-in-functions.md § Option.ok_or
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.ok_or tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.ok_or
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.ok_or codegen

- [ ] **Implement**: `Option.and_then` — spec/11-built-in-functions.md § Option.and_then
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.and_then tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.and_then
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.and_then codegen

- [ ] **Implement**: `Option.filter` — spec/11-built-in-functions.md § Option.filter
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.filter tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.filter
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.filter codegen

---

## 7B.2 Result Functions

- [ ] **Implement**: `is_ok(x)` — spec/11-built-in-functions.md § is_ok
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_ok
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — is_ok codegen

- [ ] **Implement**: `is_err(x)` — spec/11-built-in-functions.md § is_err
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — is_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — is_err codegen

- [ ] **Implement**: `Result.map` — spec/11-built-in-functions.md § Result.map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.map
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.map codegen

- [ ] **Implement**: `Result.map_err` — spec/11-built-in-functions.md § Result.map_err
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.map_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.map_err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.map_err codegen

- [ ] **Implement**: `Result.unwrap_or` — spec/11-built-in-functions.md § Result.unwrap_or
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.unwrap_or tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.unwrap_or
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.unwrap_or codegen

- [ ] **Implement**: `Result.ok` — spec/11-built-in-functions.md § Result.ok
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.ok
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.ok codegen

- [ ] **Implement**: `Result.err` — spec/11-built-in-functions.md § Result.err
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.err codegen

- [ ] **Implement**: `Result.and_then` — spec/11-built-in-functions.md § Result.and_then
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.and_then tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.and_then
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.and_then codegen

---

## 7B.3 Phase Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all`
- [ ] **LLVM Support**: All LLVM codegen tests pass

**Exit Criteria**: Option and Result methods working correctly
