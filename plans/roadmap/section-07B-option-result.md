---
section: 7B
title: Option & Result
status: in-progress
reviewed: false
tier: 2
goal: Option and Result type methods
spec:
  - spec/annex-c-built-in-functions.md
sections:
  - id: "7B.1"
    title: Option Functions
    status: in-progress
  - id: "7B.2"
    title: Result Functions
    status: in-progress
  - id: "7B.3"
    title: Error Return Traces
    status: not-started
  - id: "7B.4"
    title: Section Completion Checklist
    status: not-started
---

# Section 7B: Option & Result

**Goal**: Option and Result type methods

> **SPEC**: `spec/annex-c-built-in-functions.md`

---

## 7B.1 Option Functions

- [x] **Implement**: `is_some(x)` — spec/annex-c-built-in-functions.md § is_some [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator builtin — is_some tests
  - [x] **Ori Tests**: Used in `tests/spec/inference/polymorphism.ori`, `tests/spec/inference/generics.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_some
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — is_some codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — is_some for Some variant (test_err_option_some_unwrap); `ori_llvm/tests/aot/spec.rs` — is_some with Option constructor (test_aot_option_some_unwrap)

- [x] **Implement**: `is_none(x)` — spec/annex-c-built-in-functions.md § is_none [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator builtin — is_none tests
  - [x] **Ori Tests**: Used in `tests/spec/inference/polymorphism.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_none
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — is_none codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — is_none for None variant (test_err_option_none_check); `ori_llvm/tests/aot/spec.rs` — is_none with Option constructor (test_aot_option_none_check)

- [ ] **Implement**: `Option.map` — spec/annex-c-built-in-functions.md § Option.map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.map
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.map codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Option.unwrap_or` — spec/annex-c-built-in-functions.md § Option.unwrap_or [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator method dispatch — Option.unwrap_or tests
  - [x] **Ori Tests**: `tests/spec/inference/generics.ori` — `opt.unwrap_or(default: 42)`
  - [ ] **LLVM Support**: LLVM codegen for Option.unwrap_or
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.unwrap_or codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — Option unwrap_or for Some and None variants (test_err_option_unwrap_or_some, test_err_option_unwrap_or_none, test_err_option_chain_unwrap)

- [ ] **Implement**: `Option.ok_or` — spec/annex-c-built-in-functions.md § Option.ok_or
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.ok_or tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.ok_or
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.ok_or codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Option.and_then` — spec/annex-c-built-in-functions.md § Option.and_then
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.and_then tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.and_then
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.and_then codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Option.filter` — spec/annex-c-built-in-functions.md § Option.filter
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Option.filter tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.filter
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_tests.rs` — Option.filter codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 7B.2 Result Functions

- [x] **Implement**: `is_ok(x)` — spec/annex-c-built-in-functions.md § is_ok [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator builtin — is_ok tests
  - [x] **Ori Tests**: `tests/spec/inference/generics.ori` — `is_ok(r: res)`
  - [ ] **LLVM Support**: LLVM codegen for is_ok
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — is_ok codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — is_ok for Ok variant with unwrap (test_err_result_ok_unwrap); `ori_llvm/tests/aot/spec.rs` — is_ok with Result constructor (test_aot_result_ok_unwrap)

- [x] **Implement**: `is_err(x)` — spec/annex-c-built-in-functions.md § is_err [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator builtin — is_err tests
  - [x] **Ori Tests**: `tests/spec/inference/generics.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — is_err codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — is_err for Err variant (test_err_result_err_check); `ori_llvm/tests/aot/spec.rs` — is_err with Result constructor (test_aot_result_err_check)

- [ ] **Implement**: `Result.map` — spec/annex-c-built-in-functions.md § Result.map
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.map tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.map
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.map codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Result.map_err` — spec/annex-c-built-in-functions.md § Result.map_err
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.map_err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.map_err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.map_err codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Result.unwrap_or` — spec/annex-c-built-in-functions.md § Result.unwrap_or
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.unwrap_or tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.unwrap_or
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.unwrap_or codegen
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — Result unwrap_or for Ok and Err variants (test_err_result_unwrap_or_ok, test_err_result_unwrap_or_err)

- [ ] **Implement**: `Result.ok` — spec/annex-c-built-in-functions.md § Result.ok
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.ok tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.ok
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.ok codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Result.err` — spec/annex-c-built-in-functions.md § Result.err
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.err tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.err
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.err codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Result.and_then` — spec/annex-c-built-in-functions.md § Result.and_then
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.and_then tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.and_then
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.and_then codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 7B.3 Error Return Traces

**Proposal**: `proposals/approved/error-trace-async-semantics-proposal.md`

Implements Result trace methods and context storage for error propagation debugging.

- [ ] **Implement**: `Result.trace()` — spec/17-errors-and-panics.md § Result Trace Methods
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.trace tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.trace
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.trace codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Result.trace_entries()` — spec/17-errors-and-panics.md § Result Trace Methods
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.trace_entries tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.trace_entries
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.trace_entries codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Result.has_trace()` — spec/17-errors-and-panics.md § Result Trace Methods
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — Result.has_trace tests
  - [ ] **Ori Tests**: `tests/spec/stdlib/result_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.has_trace
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/result_tests.rs` — Result.has_trace codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Trace collection at `?` propagation — spec/17-errors-and-panics.md § Automatic Collection
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/propagation.rs` — trace collection tests
  - [ ] **Ori Tests**: `tests/spec/errors/trace_collection.ori`
  - [ ] **LLVM Support**: LLVM codegen for trace collection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_tests.rs` — trace collection codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Context storage in Result — spec/17-errors-and-panics.md § Context Storage
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — context storage tests
  - [ ] **Ori Tests**: `tests/spec/errors/context_storage.ori`
  - [ ] **LLVM Support**: LLVM codegen for context storage
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_tests.rs` — context storage codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Panic message format with location — spec/17-errors-and-panics.md § Panic Message Format
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/panic.rs` — panic format tests
  - [ ] **Ori Tests**: `tests/spec/errors/panic_format.ori`
  - [ ] **LLVM Support**: LLVM codegen for panic format
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/panic_tests.rs` — panic format codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 7B.4 Section Completion Checklist

- [ ] All items above have all checkboxes marked `[ ]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`
- [ ] **LLVM Support**: All LLVM codegen tests pass

**Exit Criteria**: Option and Result methods working correctly
