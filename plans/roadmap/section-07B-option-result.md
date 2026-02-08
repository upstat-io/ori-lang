---
section: 7B
title: Option & Result
status: not-started
tier: 2
goal: Option and Result type methods
spec:
  - spec/11-built-in-functions.md
sections:
  - id: "7B.1"
    title: Option Functions
    status: not-started
  - id: "7B.2"
    title: Result Functions
    status: not-started
  - id: "7B.3"
    title: Error Return Traces
    status: not-started
  - id: "7B.4"
    title: Section Completion Checklist
    status: not-started
---

# Section 7B: Option & Result

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

## 7B.3 Error Return Traces

**Proposal**: `proposals/approved/error-trace-async-semantics-proposal.md`

Implements Result trace methods and context storage for error propagation debugging.

- [ ] **Implement**: `Result.trace()` — spec/20-errors-and-panics.md § Result Trace Methods
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.trace tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.trace
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.trace codegen

- [ ] **Implement**: `Result.trace_entries()` — spec/20-errors-and-panics.md § Result Trace Methods
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.trace_entries tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.trace_entries
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.trace_entries codegen

- [ ] **Implement**: `Result.has_trace()` — spec/20-errors-and-panics.md § Result Trace Methods
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.has_trace tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.has_trace
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.has_trace codegen

- [ ] **Implement**: Trace collection at `?` propagation — spec/20-errors-and-panics.md § Automatic Collection
  - [ ] **Rust Tests**: `oric/src/eval/propagation.rs` — trace collection tests
  - [ ] **Ori Tests**: `tests/spec/errors/trace_collection.ori`
  - [ ] **LLVM Support**: LLVM codegen for trace collection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_tests.rs` — trace collection codegen

- [ ] **Implement**: Context storage in Result — spec/20-errors-and-panics.md § Context Storage
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — context storage tests
  - [ ] **Ori Tests**: `tests/spec/errors/context_storage.ori`
  - [ ] **LLVM Support**: LLVM codegen for context storage
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_tests.rs` — context storage codegen

- [ ] **Implement**: Panic message format with location — spec/20-errors-and-panics.md § Panic Message Format
  - [ ] **Rust Tests**: `oric/src/eval/panic.rs` — panic format tests
  - [ ] **Ori Tests**: `tests/spec/errors/panic_format.ori`
  - [ ] **LLVM Support**: LLVM codegen for panic format
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — panic format codegen

---

## 7B.4 Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`
- [ ] **LLVM Support**: All LLVM codegen tests pass

**Exit Criteria**: Option and Result methods working correctly
