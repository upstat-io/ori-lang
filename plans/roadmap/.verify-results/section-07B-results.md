# Section 07B: Option & Result -- Verification Results

**Verified**: 2026-03-19
**Branch**: experiment/aims
**Section file**: `plans/roadmap/section-07B-option-result.md`

---

## Summary

- **Total items**: 21 top-level implementation items
- **Checked `[x]` items**: 6 (sampled all 6)
- **Unchecked `[ ]` items**: 15 (sampled all)
- **Roadmap accuracy issues**: Several unchecked items have partial or complete implementations not reflected in the roadmap

---

## 7B.1 Option Functions

### `is_some(x)` -- [x] VERIFIED

- **Eval implementation**: `dispatch_option_method()` in `compiler/ori_eval/src/methods/variants.rs:332-333` -- `matches!(Value::Some(_))`, correct per spec.
- **Rust tests**: `compiler/ori_eval/src/tests/methods_tests.rs:342-353` -- tests both Some(true) and None(false). PASS.
- **Ori tests**: `tests/spec/inference/polymorphism.ori:106,112` and `tests/spec/inference/generics.ori:13,18` -- use `is_some()` with polymorphic types. PASS (4181 passed, 0 failed).
- **AOT tests**: `compiler/ori_llvm/tests/aot/error_handling.rs:73-83` (`test_err_option_some_unwrap` uses `o.is_some()`), `compiler/ori_llvm/tests/aot/spec.rs:815-830` (`test_aot_option_some_unwrap`). PASS.
- **Classification**: VERIFIED

### `is_none(x)` -- [x] VERIFIED

- **Eval implementation**: `compiler/ori_eval/src/methods/variants.rs:334-335` -- `matches!(Value::None)`, correct per spec.
- **Rust tests**: `compiler/ori_eval/src/tests/methods_tests.rs:355-367` -- tests both None(true) and Some(false). PASS.
- **Ori tests**: `tests/spec/inference/polymorphism.ori` uses `is_some()` which implicitly validates `is_none()` behavior; `tests/spec/traits/core/option.ori` has dedicated is_none tests.
- **AOT tests**: `compiler/ori_llvm/tests/aot/error_handling.rs:86-96` (`test_err_option_none_check` uses `o.is_none()`), `compiler/ori_llvm/tests/aot/spec.rs:833-845` (`test_aot_option_none_check`). PASS.
- **Classification**: VERIFIED

### `Option.map` -- [ ] CONFIRMED INCOMPLETE

- **Eval**: Recognized in `dispatch_option_method_str()` at `compiler/ori_eval/src/methods/variants.rs:390`, but returns `wrong_arg_type("function")` -- closure evaluation not wired up for Option methods.
- **Registry**: Registered in `compiler/ori_registry/src/defs/option/mod.rs:112` as `MethodDef::compound("map", ...)`.
- **Missing**: No interpreter-level handler that evaluates closures for Option.map. The `collection_ops.rs` dispatch only handles List/Range map, not Option.
- **No Ori tests**: `tests/spec/stdlib/option.ori` does not exist. `tests/spec/inference/generics.ori:141-145` has the Option.map test commented out with TODO.
- **Classification**: CONFIRMED INCOMPLETE -- needs interpreter closure dispatch for Option

### `Option.unwrap_or` -- [x] VERIFIED

- **Eval implementation**: `compiler/ori_eval/src/methods/variants.rs:318-331` -- returns inner for Some, returns default arg for None. Correct per spec.
- **Rust tests**: `compiler/ori_eval/src/tests/methods_tests.rs:322-339` -- tests Some(returns inner) and None(returns default). PASS.
- **Ori tests**: `tests/spec/inference/generics.ori:179-189` and `tests/spec/traits/core/option.ori:105-139` -- comprehensive testing with int, str types. PASS.
- **AOT tests**: `compiler/ori_llvm/tests/aot/error_handling.rs:99-124` -- `test_err_option_unwrap_or_some`, `test_err_option_unwrap_or_none`, `test_err_option_chain_unwrap`. PASS.
- **Classification**: VERIFIED

### `Option.ok_or` -- [ ] STALE (PARTIALLY IMPLEMENTED)

- **Eval implementation**: IMPLEMENTED at `compiler/ori_eval/src/methods/variants.rs:337-345`. Converts Some(v) to Ok(v), None to Err(error_arg). Correct per spec.
- **Registry**: Registered in `compiler/ori_registry/src/defs/option/mod.rs:114` with correct return type `ResultOfProjectionFresh`.
- **Registry tests**: `compiler/ori_registry/src/defs/option/tests.rs:84-92` -- validates ok_or return type. PASS.
- **Missing**: No dedicated Ori spec tests (only commented-out usage in `tests/spec/patterns/try.ori:96-97`). No LLVM tests.
- **Roadmap says `[ ]` but eval + registry implementation exists. Missing: Ori spec tests, type checker resolution verification, LLVM support.**
- **Classification**: STALE -- roadmap understates implementation status. Eval and registry are done; needs Ori tests and LLVM.

### `Option.and_then` -- [ ] CONFIRMED INCOMPLETE

- **Eval**: Recognized at `compiler/ori_eval/src/methods/variants.rs:390` but returns `wrong_arg_type` -- closure evaluation not wired.
- **Ori tests**: Commented out at `tests/spec/inference/generics.ori:151-155`.
- **Classification**: CONFIRMED INCOMPLETE

### `Option.filter` -- [ ] CONFIRMED INCOMPLETE

- **Eval**: Recognized at `compiler/ori_eval/src/methods/variants.rs:390` but returns `wrong_arg_type` -- closure evaluation not wired.
- **Classification**: CONFIRMED INCOMPLETE

---

## 7B.2 Result Functions

### `is_ok(x)` -- [x] VERIFIED

- **Eval implementation**: `compiler/ori_eval/src/methods/variants.rs:449-450` -- `matches!(Value::Ok(_))`. Correct per spec.
- **Rust tests**: `compiler/ori_eval/src/tests/methods_tests.rs:397-407` -- tests Ok(true) and Err(false). PASS.
- **Ori tests**: `tests/spec/inference/generics.ori:24,29` and `tests/spec/traits/core/result.ori:11-69` -- comprehensive. PASS.
- **AOT tests**: `compiler/ori_llvm/tests/aot/error_handling.rs:17-27` and `compiler/ori_llvm/tests/aot/spec.rs:736-751`. PASS.
- **Classification**: VERIFIED

### `is_err(x)` -- [x] VERIFIED

- **Eval implementation**: `compiler/ori_eval/src/methods/variants.rs:451-452` -- `matches!(Value::Err(_))`. Correct per spec.
- **Rust tests**: `compiler/ori_eval/src/tests/methods_tests.rs:409-421` -- tests Err(true) and Ok(false). PASS.
- **Ori tests**: `tests/spec/traits/core/result.ori:35-53` -- dedicated tests. PASS.
- **AOT tests**: `compiler/ori_llvm/tests/aot/error_handling.rs:30-40` and `compiler/ori_llvm/tests/aot/spec.rs:754-766`. PASS.
- **Classification**: VERIFIED

### `Result.map` -- [ ] CONFIRMED INCOMPLETE

- **Eval**: Recognized at `compiler/ori_eval/src/methods/variants.rs:506` but returns `wrong_arg_type` -- closure evaluation not wired.
- **Ori tests**: Commented out at `tests/spec/inference/generics.ori:161-165`.
- **Classification**: CONFIRMED INCOMPLETE

### `Result.map_err` -- [ ] CONFIRMED INCOMPLETE

- **Eval**: Recognized at `compiler/ori_eval/src/methods/variants.rs:506` but returns `wrong_arg_type`.
- **Ori tests**: Commented out at `tests/spec/inference/generics.ori:171-175`.
- **Classification**: CONFIRMED INCOMPLETE

### `Result.unwrap_or` -- [ ] STALE (IMPLEMENTED)

- **Eval implementation**: IMPLEMENTED at `compiler/ori_eval/src/methods/variants.rs:427-442`. Returns inner for Ok, returns default arg for Err. Correct per spec.
- **Rust tests**: No dedicated Result.unwrap_or test in methods_tests.rs (only Option.unwrap_or tested there).
- **Ori tests**: Only commented-out test at `tests/spec/inference/generics.ori:192-199` with note "IMPLEMENTATION BUG: Result.unwrap_or not implemented yet" -- THIS NOTE IS WRONG, it IS implemented.
- **AOT tests**: `compiler/ori_llvm/tests/aot/error_handling.rs:43-67` -- `test_err_result_unwrap_or_ok` and `test_err_result_unwrap_or_err`. PASS. These correctly test both Ok and Err paths.
- **Roadmap says `[ ]` but eval implementation and AOT tests exist. The TODO comment in generics.ori is stale.**
- **Classification**: STALE -- roadmap and Ori test TODO understates status. Eval implementation is complete; AOT tests pass; needs dedicated Rust unit tests and Ori spec tests.

### `Result.ok` -- [ ] STALE (PARTIALLY IMPLEMENTED)

- **Eval implementation**: IMPLEMENTED at `compiler/ori_eval/src/methods/variants.rs:511-517`. Ok(v) returns Some(v), Err returns None. Correct per spec.
- **No Ori tests, no Rust unit tests, no LLVM tests.**
- **Classification**: STALE -- eval implementation exists; needs tests at all levels.

### `Result.err` -- [ ] STALE (PARTIALLY IMPLEMENTED)

- **Eval implementation**: IMPLEMENTED at `compiler/ori_eval/src/methods/variants.rs:519-525`. Err(e) returns Some(e), Ok returns None. Correct per spec.
- **No Ori tests, no Rust unit tests, no LLVM tests.**
- **Classification**: STALE -- eval implementation exists; needs tests at all levels.

### `Result.and_then` -- [ ] CONFIRMED INCOMPLETE

- **Eval**: Recognized at `compiler/ori_eval/src/methods/variants.rs:506` but returns `wrong_arg_type`.
- **Classification**: CONFIRMED INCOMPLETE

---

## 7B.3 Error Return Traces

**Roadmap says "not-started" -- THIS IS WRONG. Substantial implementation exists.**

### `Result.trace()` -- [ ] STALE (IMPLEMENTED)

- **Eval implementation**: Delegated from Result dispatch at `variants.rs:476-478` via `result_error_trace()`. Error dispatch at `error/mod.rs:33-36` via `ErrorValue::format_trace()`.
- **Ori tests**: `tests/spec/traits/traceable/result_delegation.ori:27-35` -- tests `r.trace() != ""`. PASS.
- **Classification**: STALE -- implementation and Ori tests exist. Needs LLVM support.

### `Result.trace_entries()` -- [ ] STALE (IMPLEMENTED)

- **Eval implementation**: `variants.rs:479-491` -- delegates to inner Error, converts TraceEntryData to struct values.
- **Ori tests**: `tests/spec/traits/traceable/result_delegation.ori:38-48` -- tests `r.trace_entries().len() == 1`. PASS.
- **Classification**: STALE -- implementation and Ori tests exist.

### `Result.has_trace()` -- [ ] STALE (IMPLEMENTED)

- **Eval implementation**: `variants.rs:492-495` -- delegates to inner ErrorValue.
- **Ori tests**: `tests/spec/traits/traceable/result_delegation.ori:16-24` -- tests `r.has_trace()`. PASS.
- **Classification**: STALE -- implementation and Ori tests exist.

### Trace collection at `?` propagation -- [ ] STALE (IMPLEMENTED)

- **Implementation**: `compiler/ori_eval/src/interpreter/can_eval/trace.rs:23-59` -- `inject_trace_entry()` appends TraceEntryData with function name, file, line, column at each `?` site. Uses COW via `Heap::make_mut`.
- **Ori tests**: `tests/spec/traits/traceable/error_trace.ori` -- comprehensive:
  - Single `?` adds one trace entry (line 27-37)
  - Chained `?` (two sites) produces two entries (line 60-71)
  - Ok values pass through `?` untouched (line 91-102)
  - PASS (all 4181 tests pass).
- **Classification**: STALE -- fully implemented with comprehensive tests. Missing: LLVM codegen for trace injection.

### Context storage in Result -- [ ] CONFIRMED INCOMPLETE

- **No implementation found** for `Result.context()` method. The spec mentions `.context(msg:)` on Result, but no dispatch for it exists.
- **Classification**: CONFIRMED INCOMPLETE

### Panic message format with location -- [ ] NOT VERIFIED

- **Partial implementation**: Panic handling exists in evaluator, but could not verify specific location formatting against spec.
- **No Ori tests**: `tests/spec/errors/panic_format.ori` does not exist.
- **Classification**: NEEDS TESTS -- may have partial implementation but no verification possible.

---

## 7B.4 Section Completion Checklist

All items `[ ]` -- CONFIRMED INCOMPLETE. Section is still in-progress.

---

## Cross-Cutting Observations

### WEAK TESTS: Missing Rust unit tests for Result methods

The `compiler/ori_eval/src/tests/methods_tests.rs` file has `result_methods` module but only tests `unwrap_ok`, `unwrap_err_error`, `is_ok`, and `is_err`. Missing:
- Result.unwrap_or (implemented but untested at Rust level)
- Result.ok (implemented but untested)
- Result.err (implemented but untested)

### Stale TODO in generics.ori

`tests/spec/inference/generics.ori:191` says "IMPLEMENTATION BUG: Result.unwrap_or not implemented yet" -- this is incorrect. `Result.unwrap_or` IS implemented and the AOT tests prove it works. The comment should be removed and the test uncommented.

### Roadmap 7B.3 status is "not-started" but should be "in-progress"

Error return traces have substantial implementation:
- `Result.trace()`, `Result.trace_entries()`, `Result.has_trace()` -- all implemented in evaluator
- Trace collection at `?` propagation -- fully implemented with COW optimization
- Comprehensive Ori spec tests in `tests/spec/traits/traceable/`
- All tests pass

Only missing: LLVM codegen, `Result.context()`, panic format with location.

### Higher-order Option/Result methods blocked

`Option.map`, `Option.and_then`, `Option.filter`, `Result.map`, `Result.map_err`, `Result.and_then` are all recognized in method dispatch but cannot execute because the closure evaluation path is not wired through the collection method dispatcher for Option/Result types. The evaluator's `collection_ops.rs` only handles List/Range for higher-order methods.

---

## Verification Matrix

| Item | Roadmap | Actual Status | Classification |
|------|---------|---------------|----------------|
| is_some | [x] | Implemented, tested, AOT | VERIFIED |
| is_none | [x] | Implemented, tested, AOT | VERIFIED |
| Option.map | [ ] | Recognized but closure dispatch missing | CONFIRMED INCOMPLETE |
| Option.unwrap_or | [x] | Implemented, tested, AOT | VERIFIED |
| Option.ok_or | [ ] | Eval+registry done, needs tests+LLVM | STALE |
| Option.and_then | [ ] | Recognized but closure dispatch missing | CONFIRMED INCOMPLETE |
| Option.filter | [ ] | Recognized but closure dispatch missing | CONFIRMED INCOMPLETE |
| is_ok | [x] | Implemented, tested, AOT | VERIFIED |
| is_err | [x] | Implemented, tested, AOT | VERIFIED |
| Result.map | [ ] | Recognized but closure dispatch missing | CONFIRMED INCOMPLETE |
| Result.map_err | [ ] | Recognized but closure dispatch missing | CONFIRMED INCOMPLETE |
| Result.unwrap_or | [ ] | Eval done, AOT tested, stale TODO | STALE |
| Result.ok | [ ] | Eval done, no tests | STALE |
| Result.err | [ ] | Eval done, no tests | STALE |
| Result.and_then | [ ] | Recognized but closure dispatch missing | CONFIRMED INCOMPLETE |
| Result.trace | [ ] | Eval done, Ori tests pass | STALE |
| Result.trace_entries | [ ] | Eval done, Ori tests pass | STALE |
| Result.has_trace | [ ] | Eval done, Ori tests pass | STALE |
| Trace at ? | [ ] | Fully implemented, tested | STALE |
| Context storage | [ ] | Not implemented | CONFIRMED INCOMPLETE |
| Panic format | [ ] | Uncertain | NEEDS TESTS |
