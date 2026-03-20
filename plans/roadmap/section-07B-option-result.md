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
    status: in-progress
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

- [x] **Implement**: `Option.ok_or` — spec/annex-c-built-in-functions.md § Option.ok_or [done — eval+registry]
  - Eval: `variants.rs` — Some(v) -> Ok(v), None -> Err(error_arg). Registry: `option/mod.rs` with `ResultOfProjectionFresh` return type.
  - [x] **Registry Tests**: `ori_registry/src/defs/option/tests.rs` — validates ok_or return type
  - [ ] **Ori Tests**: NEEDS TESTS — no dedicated spec tests
  - [ ] **LLVM Support**: LLVM codegen for Option.ok_or
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

- [x] **Implement**: `Result.unwrap_or` — spec/annex-c-built-in-functions.md § Result.unwrap_or [done — eval+AOT]
  - Eval: `variants.rs` — Ok returns inner, Err returns default arg
  - [ ] **Rust Tests**: No dedicated Rust unit test for Result.unwrap_or
  - [ ] **Ori Tests**: NEEDS TESTS — stale TODO in `generics.ori:191` incorrectly says "not implemented yet"
  - [ ] **LLVM Support**: LLVM codegen for Result.unwrap_or
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — Result unwrap_or for Ok and Err variants (test_err_result_unwrap_or_ok, test_err_result_unwrap_or_err)

- [x] **Implement**: `Result.ok` — spec/annex-c-built-in-functions.md § Result.ok [done — eval only]
  - Eval: `variants.rs` — Ok(v) -> Some(v), Err -> None
  - [ ] **Rust Tests**: No dedicated Rust unit test
  - [ ] **Ori Tests**: NEEDS TESTS — no spec tests
  - [ ] **LLVM Support**: LLVM codegen for Result.ok
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Result.err` — spec/annex-c-built-in-functions.md § Result.err [done — eval only]
  - Eval: `variants.rs` — Err(e) -> Some(e), Ok -> None
  - [ ] **Rust Tests**: No dedicated Rust unit test
  - [ ] **Ori Tests**: NEEDS TESTS — no spec tests
  - [ ] **LLVM Support**: LLVM codegen for Result.err
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

- [x] **Implement**: `Result.trace()` — spec/17-errors-and-panics.md § Result Trace Methods [done — eval only]
  - Eval: `variants.rs` delegates via `result_error_trace()` to `ErrorValue::format_trace()`
  - [x] **Ori Tests**: `tests/spec/traits/traceable/result_delegation.ori` — tests `r.trace() != ""`
  - [ ] **LLVM Support**: LLVM codegen for Result.trace
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Result.trace_entries()` — spec/17-errors-and-panics.md § Result Trace Methods [done — eval only]
  - Eval: `variants.rs` delegates to inner Error, converts TraceEntryData to struct values
  - [x] **Ori Tests**: `tests/spec/traits/traceable/result_delegation.ori` — tests `r.trace_entries().len() == 1`
  - [ ] **LLVM Support**: LLVM codegen for Result.trace_entries
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Result.has_trace()` — spec/17-errors-and-panics.md § Result Trace Methods [done — eval only]
  - Eval: `variants.rs` delegates to inner ErrorValue
  - [x] **Ori Tests**: `tests/spec/traits/traceable/result_delegation.ori` — tests `r.has_trace()`
  - [ ] **LLVM Support**: LLVM codegen for Result.has_trace
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Trace collection at `?` propagation — spec/17-errors-and-panics.md § Automatic Collection [done — eval only]
  - Eval: `interpreter/can_eval/trace.rs` — `inject_trace_entry()` appends TraceEntryData with function name, file, line, column at each `?` site. Uses COW via `Heap::make_mut`.
  - [x] **Ori Tests**: `tests/spec/traits/traceable/error_trace.ori` — single `?` trace entry, chained `?` two entries, Ok passthrough
  - [ ] **LLVM Support**: LLVM codegen for trace collection
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
