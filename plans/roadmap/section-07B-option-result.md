---
section: 7B
title: Option & Result
status: in-progress
last_verified: "2026-03-29"
reviewed: true
tier: 2
goal: Option and Result type methods
spec:
  - spec/annex-c-built-in-functions.md
  - spec/17-errors-and-panics.md
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

> **SPEC**: `spec/annex-c-built-in-functions.md` (7B.1, 7B.2), `spec/17-errors-and-panics.md` (7B.3)

---

## 7B.1 Option Functions

- [x] **Implement**: `is_some(x)` — spec/annex-c-built-in-functions.md § is_some [done] (2026-02-10) (verified 2026-03-29) WEAK TESTS — no negative pin, no complex element types
  - [x] **Rust Tests**: Evaluator builtin — is_some tests
  - [x] **Ori Tests**: `tests/spec/traits/core/option.ori` — test_some_is_some, test_none_is_some, test_nested_some_is_some, test_is_some_builtin; also `tests/spec/inference/polymorphism.ori`, `tests/spec/inference/generics.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_some
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — is_some for Some variant (test_err_option_some_unwrap); `ori_llvm/tests/aot/spec.rs` — is_some with Option constructor (test_aot_option_some_unwrap, test_aot_option_is_some_true, test_aot_option_is_some_false)
  - [ ] **Negative Tests**: Needs `#compile_fail` tests for wrong type

- [x] **Implement**: `is_none(x)` — spec/annex-c-built-in-functions.md § is_none [done] (2026-02-10) (verified 2026-03-29) WEAK TESTS — no negative pin
  - [x] **Rust Tests**: Evaluator builtin — is_none tests
  - [x] **Ori Tests**: `tests/spec/traits/core/option.ori` — test_none_is_none, test_some_is_none, test_is_none_builtin; also `tests/spec/inference/polymorphism.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_none
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — is_none for None variant (test_err_option_none_check); `ori_llvm/tests/aot/spec.rs` — is_none with Option constructor (test_aot_option_none_check)
  - [ ] **Negative Tests**: Needs `#compile_fail` tests for wrong type

- [ ] **Implement**: `Option.map` — spec/annex-c-built-in-functions.md § Option.map — BLOCKED: CollectionMethodResolver does not route Option types for closure-taking methods
  - [ ] **Rust Tests**: `ori_eval/src/methods/variants.rs` — Option.map tests
  - [ ] **Ori Tests**: `tests/spec/traits/core/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.map
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Option.unwrap_or` — spec/annex-c-built-in-functions.md § Option.unwrap_or [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator method dispatch — Option.unwrap_or tests
  - [x] **Ori Tests**: `tests/spec/traits/core/option.ori` — test_unwrap_or_some, test_unwrap_or_none, test_unwrap_or_string; also `tests/spec/inference/generics.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.unwrap_or
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — Option unwrap_or for Some and None variants (test_err_option_unwrap_or_some, test_err_option_unwrap_or_none, test_err_option_chain_unwrap)

- [x] **Implement**: `Option.ok_or` — spec/annex-c-built-in-functions.md § Option.ok_or [done] (verified 2026-03-28) (verified 2026-03-29) NEEDS TESTS — no committed spec tests
  - [x] **Evaluator**: `dispatch_option_method` handles `ok_or` in `variants.rs` — Some.ok_or -> Ok, None.ok_or -> Err
  - [ ] **Ori Tests**: `tests/spec/traits/core/option.ori` — needs dedicated spec tests
  - [ ] **LLVM Support**: LLVM codegen for Option.ok_or
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Option.and_then` — spec/annex-c-built-in-functions.md § Option.and_then — BLOCKED: same CollectionMethodResolver gap as Option.map
  - [ ] **Rust Tests**: `ori_eval/src/methods/variants.rs` — Option.and_then tests
  - [ ] **Ori Tests**: `tests/spec/traits/core/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.and_then
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Option.filter` — spec/annex-c-built-in-functions.md § Option.filter — BLOCKED: same CollectionMethodResolver gap as Option.map
  - [ ] **Rust Tests**: `ori_eval/src/methods/variants.rs` — Option.filter tests
  - [ ] **Ori Tests**: `tests/spec/traits/core/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.filter
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Option.or` — spec/annex-c-built-in-functions.md § Option.or [done] (verified 2026-03-29) NEEDS TESTS — evaluator works, no dedicated spec tests
  - [x] **Evaluator**: `dispatch_option_method_str` handles "or" in `variants.rs`
  - [x] **Registry**: Defined in `ori_registry/src/defs/option/mod.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/core/option.ori` — needs dedicated spec tests
  - [ ] **LLVM Support**: LLVM codegen for Option.or
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Option.or_else` — spec/annex-c-built-in-functions.md § Option.or_else — BLOCKED: same CollectionMethodResolver gap as Option.map
  - [ ] **Evaluator**: Recognizes method but cannot dispatch closure
  - [ ] **Ori Tests**: `tests/spec/traits/core/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option.or_else
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Option.expect` — spec/annex-c-built-in-functions.md § Option.expect [done] (verified 2026-03-29) NEEDS TESTS — evaluator works, no dedicated spec tests
  - [x] **Evaluator**: `dispatch_option_method` handles "expect" in `variants.rs`
  - [x] **Registry**: Defined in `ori_registry/src/defs/option/mod.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/core/option.ori` — needs dedicated spec tests
  - [ ] **LLVM Support**: LLVM codegen for Option.expect
  - [ ] **AOT Tests**: No AOT coverage yet

> **Architecture blocker for higher-order Option methods**: All closure-taking Option methods (map, and_then, filter, or_else) share the same gap -- `CollectionMethodResolver` / `eval_collection_method` does not route Option types. Fix requires adding Option variant handling to `eval_collection_method` in `collection_ops.rs`, similar to how List.map/filter/fold are handled.

---

## 7B.2 Result Functions

- [x] **Implement**: `is_ok(x)` — spec/annex-c-built-in-functions.md § is_ok [done] (2026-02-10) (verified 2026-03-29) WEAK TESTS — no negative pin
  - [x] **Rust Tests**: Evaluator builtin — is_ok tests
  - [x] **Ori Tests**: `tests/spec/traits/core/result.ori` — test_ok_is_ok, test_err_is_ok, test_is_ok_builtin; also `tests/spec/inference/generics.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_ok
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — is_ok for Ok variant with unwrap (test_err_result_ok_unwrap); `ori_llvm/tests/aot/spec.rs` — is_ok with Result constructor (test_aot_result_ok_unwrap, test_aot_result_is_ok_true, test_aot_result_is_ok_false)
  - [ ] **Negative Tests**: Needs `#compile_fail` tests for wrong type

- [x] **Implement**: `is_err(x)` — spec/annex-c-built-in-functions.md § is_err [done] (2026-02-10) (verified 2026-03-29) WEAK TESTS — no negative pin
  - [x] **Rust Tests**: Evaluator builtin — is_err tests
  - [x] **Ori Tests**: `tests/spec/traits/core/result.ori` — test_ok_is_err, test_err_is_err, test_is_err_builtin; also `tests/spec/inference/generics.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_err
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — is_err for Err variant (test_err_result_err_check); `ori_llvm/tests/aot/spec.rs` — is_err with Result constructor (test_aot_result_err_check, test_aot_result_is_err_true, test_aot_result_is_err_false)
  - [ ] **Negative Tests**: Needs `#compile_fail` tests for wrong type

- [ ] **Implement**: `Result.map` — spec/annex-c-built-in-functions.md § Result.map — BLOCKED: CollectionMethodResolver does not route Result types for closure-taking methods
  - [ ] **Rust Tests**: `ori_eval/src/methods/variants.rs` — Result.map tests
  - [ ] **Ori Tests**: `tests/spec/traits/core/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.map
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Result.map_err` — spec/annex-c-built-in-functions.md § Result.map_err — BLOCKED: same CollectionMethodResolver gap as Result.map
  - [ ] **Rust Tests**: `ori_eval/src/methods/variants.rs` — Result.map_err tests
  - [ ] **Ori Tests**: `tests/spec/traits/core/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.map_err
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Result.unwrap_or` — spec/annex-c-built-in-functions.md § Result.unwrap_or [done] (verified 2026-03-28) (verified 2026-03-29) NEEDS TESTS — no dedicated spec tests; STALE TODO at `tests/spec/inference/generics.ori` line 191 says "not implemented" but method IS implemented
  - [x] **Evaluator**: `dispatch_result_method` handles `unwrap_or` in `variants.rs` — works for both Ok and Err
  - [ ] **Ori Tests**: `tests/spec/traits/core/result.ori` — needs dedicated spec tests
  - [ ] **LLVM Support**: LLVM codegen for Result.unwrap_or
  - [x] **AOT Tests**: `ori_llvm/tests/aot/error_handling.rs` — Result unwrap_or for Ok and Err variants (test_err_result_unwrap_or_ok, test_err_result_unwrap_or_err)

- [x] **Implement**: `Result.ok` — spec/annex-c-built-in-functions.md § Result.ok [done] (verified 2026-03-28) (verified 2026-03-29) NEEDS TESTS — no committed spec tests
  - [x] **Evaluator**: `dispatch_result_method_str` handles "ok" in `variants.rs` — Ok -> Some(value), Err -> None
  - [ ] **Ori Tests**: `tests/spec/traits/core/result.ori` — needs dedicated spec tests
  - [ ] **LLVM Support**: LLVM codegen for Result.ok
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Result.err` — spec/annex-c-built-in-functions.md § Result.err [done] (verified 2026-03-28) (verified 2026-03-29) NEEDS TESTS — no committed spec tests
  - [x] **Evaluator**: `dispatch_result_method_str` handles "err" in `variants.rs` — Err -> Some(error), Ok -> None
  - [ ] **Ori Tests**: `tests/spec/traits/core/result.ori` — needs dedicated spec tests
  - [ ] **LLVM Support**: LLVM codegen for Result.err
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Result.and_then` — spec/annex-c-built-in-functions.md § Result.and_then — BLOCKED: same CollectionMethodResolver gap as Result.map
  - [ ] **Rust Tests**: `ori_eval/src/methods/variants.rs` — Result.and_then tests
  - [ ] **Ori Tests**: `tests/spec/traits/core/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result.and_then
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Result.expect` — spec/annex-c-built-in-functions.md § Result.expect [done] (verified 2026-03-29) NEEDS TESTS — evaluator works, no dedicated spec tests
  - [x] **Evaluator**: `dispatch_result_method` handles "expect" in `variants.rs`
  - [x] **Registry**: Defined in `ori_registry/src/defs/result/mod.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/core/result.ori` — needs dedicated spec tests
  - [ ] **LLVM Support**: LLVM codegen for Result.expect
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Result.expect_err` — spec/annex-c-built-in-functions.md § Result.expect_err [done] (verified 2026-03-29) NEEDS TESTS — evaluator works, no dedicated spec tests
  - [x] **Evaluator**: `dispatch_result_method` handles "expect_err" in `variants.rs`
  - [x] **Registry**: Defined in `ori_registry/src/defs/result/mod.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/core/result.ori` — needs dedicated spec tests
  - [ ] **LLVM Support**: LLVM codegen for Result.expect_err
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Result.unwrap_err` — spec/annex-c-built-in-functions.md § Result.unwrap_err [done] (verified 2026-03-29) NEEDS TESTS — evaluator works, no dedicated spec tests
  - [x] **Evaluator**: `dispatch_result_method` handles "unwrap_err" in `variants.rs` (lines 443-446)
  - [x] **Registry**: Defined in `ori_registry/src/defs/result/mod.rs`
  - [ ] **Ori Tests**: `tests/spec/traits/core/result.ori` — needs dedicated spec tests
  - [ ] **LLVM Support**: LLVM codegen for Result.unwrap_err
  - [ ] **AOT Tests**: No AOT coverage yet

> **Architecture blocker for higher-order Result methods**: All closure-taking Result methods (map, map_err, and_then) share the same gap as Option -- `CollectionMethodResolver` / `eval_collection_method` does not route Result types. Fix requires adding Result variant handling to `eval_collection_method` in `collection_ops.rs`.

---

## 7B.3 Error Return Traces

**Proposal**: `proposals/approved/error-trace-async-semantics-proposal.md`

Implements Result trace methods and context storage for error propagation debugging.

- [x] **Implement**: `Result.trace()` — spec/17-errors-and-panics.md § Result Trace Methods [partial] (verified 2026-03-29) — evaluator works, LLVM not implemented
  - [x] **Evaluator**: `dispatch_result_method` handles trace in `variants.rs` (lines 476-478) — delegates to `result_error_trace()` -> `ErrorValue::format_trace`
  - [x] **Ori Tests**: `tests/spec/traits/traceable/definition.ori` — test_trace verifies `Err(Error("test")).trace() == ""`
  - [ ] **LLVM Support**: LLVM codegen for Result.trace
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Result.trace_entries()` — spec/17-errors-and-panics.md § Result Trace Methods [partial] (verified 2026-03-29) — evaluator works, LLVM not implemented
  - [x] **Evaluator**: `dispatch_result_method` handles trace_entries in `variants.rs` (lines 479-491) — maps trace entries to Value structs
  - [x] **Type Checker**: `computed_returns.rs` handles trace_entries return type as `[TraceEntry]`
  - [x] **Ori Tests**: `tests/spec/traits/traceable/definition.ori` — test_entries verifies `len() == 0` on fresh error
  - [ ] **LLVM Support**: LLVM codegen for Result.trace_entries
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Result.has_trace()` — spec/17-errors-and-panics.md § Result Trace Methods [partial] (verified 2026-03-29) — evaluator works, LLVM not implemented
  - [x] **Evaluator**: `dispatch_result_method` handles has_trace in `variants.rs` (lines 492-495) — checks `result_inner_error(&receiver).is_some_and(ErrorValue::has_trace)`
  - [x] **Ori Tests**: `tests/spec/traits/traceable/definition.ori` — test_has and test_ok
  - [ ] **LLVM Support**: LLVM codegen for Result.has_trace
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Trace collection at `?` propagation — spec/17-errors-and-panics.md § Automatic Collection
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/propagation.rs` — trace collection tests
  - [ ] **Ori Tests**: needs spec tests (no file exists yet)
  - [ ] **LLVM Support**: LLVM codegen for trace collection
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Result.context(msg:)` — spec/17-errors-and-panics.md § 17.7.8 Context Storage — NOT IMPLEMENTED: method not in evaluator dispatch tables, not in registry. Spec-defined but missing from implementation.
  - [ ] **Evaluator**: Not implemented — `Result.context(msg: "yo")` fails with E6024 "no method 'context' on type Result"
  - [ ] **Registry**: Not defined in `ori_registry/src/defs/result/mod.rs`
  - [ ] **Ori Tests**: needs spec tests (no file exists yet)
  - [ ] **LLVM Support**: LLVM codegen for context storage
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Panic message format with location — spec/17-errors-and-panics.md § Panic Message Format
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/panic.rs` — panic format tests
  - [ ] **Ori Tests**: needs spec tests (no file exists yet)
  - [ ] **LLVM Support**: LLVM codegen for panic format
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 7B.4 Section Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`
- [ ] **LLVM Support**: All LLVM codegen tests pass
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.
- [ ] **Hygiene**: Fix STALE TODO at `tests/spec/inference/generics.ori` line 191 — comment says "Result.unwrap_or not implemented yet" but method IS implemented; uncomment test at lines 192-197
- [ ] **Hygiene**: Fix bare TODOs at `tests/spec/inference/generics.ori` lines 139, 149, 159, 169 — add plan references (format: `// TODO(eval): section-07B -- Option.map not implemented yet`)
- [ ] **Hygiene**: Remove decorative banners (`// =============`) from `tests/spec/traits/core/option.ori` and `tests/spec/traits/core/result.ori`
- [ ] **Test gaps**: Add `#compile_fail` negative tests for type errors on Option/Result method misuse
- [ ] **Test gaps**: Add interaction tests (Option.unwrap_or inside for loop, Result.ok inside match, etc.)
- [ ] **Test gaps**: Add dedicated spec tests for: Option.ok_or, Option.or, Option.expect, Result.unwrap_or, Result.ok, Result.err, Result.expect, Result.expect_err, Result.unwrap_err

**Exit Criteria**: Option and Result methods working correctly
