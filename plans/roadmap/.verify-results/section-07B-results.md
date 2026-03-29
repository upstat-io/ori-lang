# Section 7B Verification Results: Option & Result

**Verified by**: Claude Opus 4.6 (1M context)
**Date**: 2026-03-28
**Section file**: `plans/roadmap/section-07B-option-result.md`

## Files Loaded Before Verification

- `/home/eric/projects/ori_lang/CLAUDE.md` (full, 183 lines)
- All 20 rules files in `.claude/rules/`: aot.md, arc.md, cargo.md, compiler.md, diagnostic.md, eval.md, impl-hygiene.md, ir.md, llvm.md, ori-lang.md, ori-syntax.md, parse.md, patterns.md, registry.md, roadmap.md, runtime.md, spec.md, tests.md, typeck.md, types.md
- `docs/ori_lang/v2026/spec/annex-c-built-in-functions.md` (Option/Result spec)
- `docs/ori_lang/v2026/spec/17-errors-and-panics.md` (error traces, panic format spec)

## Summary

| Status | Count |
|--------|-------|
| VERIFIED | 6 |
| WEAK | 2 |
| INCOMPLETE MATRIX | 2 |
| NEEDS TESTS | 12 |
| STALE | 4 |

**Overall section status**: Partially implemented. The core functions (`is_some`, `is_none`, `is_ok`, `is_err`, `Option.unwrap_or`) are solid with working eval + AOT. The higher-order methods (`Option.map`, `Option.and_then`, `Option.filter`, `Result.map`, `Result.map_err`, `Result.and_then`) are NOT implemented -- they error with "expects a function argument" because the `CollectionMethodResolver` does not route Option/Result closure-taking methods. `Result.unwrap_or`, `Result.ok`, `Result.err`, `Option.ok_or` ARE implemented but the roadmap incorrectly marks them `[ ]`. Section 7B.3 (Error Return Traces) is partially implemented -- the evaluator handles `Result.trace()`, `Result.trace_entries()`, `Result.has_trace()` but no spec tests exist in the planned paths; the traceable tests live elsewhere.

---

## 7B.1 Option Functions

### 7B.1.1 `is_some(x)` -- marked `[x]`

**Status**: VERIFIED

**Tests found and run**:
- `tests/spec/traits/core/option.ori` -- `test_is_some_builtin`, `test_some_is_some`, `test_none_is_some`, `test_nested_some_is_some` (4 tests exercising Some(int), None, nested Some)
- `tests/spec/inference/generics.ori` -- `test_infer_option_type` uses `is_some`
- `tests/spec/inference/polymorphism.ori` -- `test_poly_option` uses `is_some`
- `compiler/ori_eval/src/tests/methods_tests.rs` -- `option_methods::is_some` (Rust unit test, both Some and None)
- `compiler/ori_llvm/tests/aot/error_handling.rs` -- `test_err_option_some_unwrap` uses `is_some`
- `compiler/ori_llvm/tests/aot/spec.rs` -- `test_aot_option_some_unwrap` uses `is_some`

**Test execution**: All pass (spec tests: 4181 passed; AOT error_handling: 28 passed; eval unit: 5 passed).

**Matrix coverage**: int, str, nested Option, bool types tested. Missing: collections as element type. Method vs free function both tested.

**LLVM sub-items**: Roadmap marks `[ ] LLVM Support` and `[ ] LLVM Rust Tests`. AOT tests DO use `is_some` via LLVM compilation (`assert_aot_success`). The `is_some` method works through LLVM -- the checkbox labels are misleading (there is no separate "LLVM codegen" needed; it goes through method dispatch). STALE labels.

**Assessment**: Core implementation VERIFIED. LLVM sub-items are STALE -- AOT tests already exercise is_some through LLVM.

---

### 7B.1.2 `is_none(x)` -- marked `[x]`

**Status**: VERIFIED

**Tests found and run**:
- `tests/spec/traits/core/option.ori` -- `test_none_is_none`, `test_some_is_none`, `test_is_none_builtin` (3 tests, str and int types)
- `tests/spec/inference/polymorphism.ori` -- uses `is_some` (not `is_none` directly)
- `compiler/ori_eval/src/tests/methods_tests.rs` -- `option_methods::is_none`
- `compiler/ori_llvm/tests/aot/error_handling.rs` -- `test_err_option_none_check` uses `is_none`
- `compiler/ori_llvm/tests/aot/spec.rs` -- `test_aot_option_none_check` uses `is_none`

**Test execution**: All pass.

**LLVM sub-items**: Same as is_some -- AOT tests already exercise this. STALE labels.

**Assessment**: VERIFIED. LLVM sub-items STALE.

---

### 7B.1.3 `Option.map` -- marked `[ ]`

**Status**: NEEDS TESTS (correctly marked `[ ]` in roadmap)

**Implementation check**: Method is recognized by evaluator (`dispatch_option_method_str` returns `wrong_arg_type("function")` for map). The `CollectionMethodResolver` does NOT route `Option.map` -- it only handles List, Range, Map, Iterator. Calling `Some(21).map(transform: x -> x * 2)` fails with "map expects a function argument".

**Verified via ad-hoc test**: `/tmp/test_option_map.ori` -- FAILED: "map expects a function argument"

**Tests**: No `tests/spec/stdlib/option.ori` file exists. The generics.ori file has `Option.map` commented out with `// TODO: IMPLEMENTATION BUG`.

**Assessment**: NOT IMPLEMENTED. Needs: CollectionMethodResolver routing for Option variants, interpreter closure dispatch, type checker method typing, spec tests, AOT tests.

---

### 7B.1.4 `Option.unwrap_or` -- marked `[x]`

**Status**: VERIFIED

**Tests found and run**:
- `tests/spec/traits/core/option.ori` -- `test_unwrap_or_some`, `test_unwrap_or_none`, `test_unwrap_or_string` (3 tests: int Some, int None, str None)
- `tests/spec/inference/generics.ori` -- `test_infer_option_unwrap_or`
- `compiler/ori_eval/src/tests/methods_tests.rs` -- `option_methods::unwrap_or` (Some(42) + None)
- `compiler/ori_llvm/tests/aot/error_handling.rs` -- `test_err_option_unwrap_or_some`, `test_err_option_unwrap_or_none`, `test_err_option_chain_unwrap`

**Test execution**: All pass.

**Matrix coverage**: int, str types tested across Some/None variants. AOT tests exercise LLVM path.

**LLVM sub-items**: AOT tests exist and pass. STALE labels for `[ ] LLVM Support` and `[ ] LLVM Rust Tests`.

**Assessment**: VERIFIED. LLVM sub-items STALE.

---

### 7B.1.5 `Option.ok_or` -- marked `[ ]`

**Status**: STALE (implementation exists, roadmap says not implemented)

**Implementation check**: Evaluator `dispatch_option_method` handles `ok_or` at line 337-345 of `variants.rs`. Registry declares it. Type checker resolves it.

**Verified via ad-hoc test**: `/tmp/test_option_ok_or.ori` -- 2 tests passed (Some.ok_or -> Ok, None.ok_or -> Err).

**Tests**: No `tests/spec/stdlib/option.ori`. No dedicated spec tests for ok_or.

**Assessment**: IMPLEMENTATION EXISTS AND WORKS in evaluator. Roadmap incorrectly marks `[ ]` for implementation. However, NO spec tests or LLVM tests exist. Should be marked `[x]` for eval implementation with `[ ]` for tests and LLVM. STALE roadmap status.

---

### 7B.1.6 `Option.and_then` -- marked `[ ]`

**Status**: NEEDS TESTS (correctly marked `[ ]`)

**Implementation check**: Same as `Option.map` -- evaluator recognizes it but returns `wrong_arg_type("function")`. NOT implemented for closure dispatch.

**Assessment**: NOT IMPLEMENTED. Same fix needed as Option.map.

---

### 7B.1.7 `Option.filter` -- marked `[ ]`

**Status**: NEEDS TESTS (correctly marked `[ ]`)

**Implementation check**: Same as `Option.map` -- evaluator recognizes it but returns `wrong_arg_type("function")`. NOT implemented for closure dispatch.

**Assessment**: NOT IMPLEMENTED. Same fix needed as Option.map.

---

## 7B.2 Result Functions

### 7B.2.1 `is_ok(x)` -- marked `[x]`

**Status**: VERIFIED

**Tests found and run**:
- `tests/spec/traits/core/result.ori` -- `test_ok_is_ok`, `test_err_is_ok`, `test_is_ok_builtin` (3 tests)
- `tests/spec/inference/generics.ori` -- `test_infer_result_type` uses `is_ok`
- `compiler/ori_eval/src/tests/methods_tests.rs` -- `result_methods::is_ok`
- `compiler/ori_llvm/tests/aot/error_handling.rs` -- `test_err_result_ok_unwrap` uses `is_ok`
- `compiler/ori_llvm/tests/aot/spec.rs` -- `test_aot_result_ok_unwrap` uses `is_ok`

**Test execution**: All pass.

**LLVM sub-items**: AOT tests pass. STALE labels.

**Assessment**: VERIFIED. LLVM sub-items STALE.

---

### 7B.2.2 `is_err(x)` -- marked `[x]`

**Status**: VERIFIED

**Tests found and run**:
- `tests/spec/traits/core/result.ori` -- `test_err_is_err`, `test_ok_is_err`, `test_is_err_builtin`
- `tests/spec/inference/generics.ori` -- uses `is_err` indirectly
- `compiler/ori_eval/src/tests/methods_tests.rs` -- `result_methods::is_err`
- `compiler/ori_llvm/tests/aot/error_handling.rs` -- `test_err_result_err_check` uses `is_err`
- `compiler/ori_llvm/tests/aot/spec.rs` -- `test_aot_result_err_check` uses `is_err`

**Test execution**: All pass.

**Assessment**: VERIFIED. LLVM sub-items STALE.

---

### 7B.2.3 `Result.map` -- marked `[ ]`

**Status**: NEEDS TESTS (correctly marked `[ ]`)

**Implementation check**: Evaluator recognizes "map" on Result but returns `wrong_arg_type("function")`. NOT implemented for closure dispatch.

**Verified via ad-hoc test**: `/tmp/test_result_map.ori` -- FAILED: "map expects a function argument"

**Assessment**: NOT IMPLEMENTED.

---

### 7B.2.4 `Result.map_err` -- marked `[ ]`

**Status**: NEEDS TESTS (correctly marked `[ ]`)

**Implementation check**: Same as Result.map -- recognized but returns `wrong_arg_type("function")`. NOT implemented.

**Assessment**: NOT IMPLEMENTED.

---

### 7B.2.5 `Result.unwrap_or` -- marked `[ ]`

**Status**: STALE (implementation exists, roadmap says not implemented)

**Implementation check**: Evaluator `dispatch_result_method` handles `unwrap_or` at lines 427-440 of `variants.rs`. Works for both Ok and Err.

**Verified via ad-hoc test**: `/tmp/test_result_unwrap_or.ori` -- 2 tests passed.

**AOT Tests**: `compiler/ori_llvm/tests/aot/error_handling.rs` -- `test_err_result_unwrap_or_ok` and `test_err_result_unwrap_or_err` both pass. Roadmap correctly marks `[x]` for AOT Tests.

**Assessment**: IMPLEMENTATION EXISTS AND WORKS. Roadmap incorrectly marks `[ ]` for implementation. The eval sub-items should be `[x]`. No dedicated spec tests in `tests/spec/stdlib/result.ori` (file doesn't exist). STALE.

---

### 7B.2.6 `Result.ok` -- marked `[ ]`

**Status**: STALE (implementation exists, roadmap says not implemented)

**Implementation check**: Evaluator `dispatch_result_method_str` handles "ok" at line 511-516 of `variants.rs`. Returns `Some(value)` for Ok, `None` for Err.

**Verified via ad-hoc test**: `/tmp/test_result_ok_err.ori` -- 4 tests passed (ok() and err() on both Ok and Err variants).

**Assessment**: IMPLEMENTATION EXISTS AND WORKS. Roadmap incorrectly marks `[ ]`. No spec tests exist. STALE.

---

### 7B.2.7 `Result.err` -- marked `[ ]`

**Status**: STALE (implementation exists, roadmap says not implemented)

**Implementation check**: Evaluator `dispatch_result_method_str` handles "err" at line 519-525 of `variants.rs`. Returns `Some(error)` for Err, `None` for Ok.

**Verified via ad-hoc test**: Same as Result.ok test above -- passes.

**Assessment**: IMPLEMENTATION EXISTS AND WORKS. Roadmap incorrectly marks `[ ]`. No spec tests exist. STALE.

---

### 7B.2.8 `Result.and_then` -- marked `[ ]`

**Status**: NEEDS TESTS (correctly marked `[ ]`)

**Implementation check**: Same as Result.map -- recognized but returns `wrong_arg_type("function")`. NOT implemented for closure dispatch.

**Assessment**: NOT IMPLEMENTED.

---

## 7B.3 Error Return Traces

### 7B.3.1 `Result.trace()` -- marked `[ ]`

**Status**: INCOMPLETE MATRIX (implementation exists but roadmap marks `[ ]`)

**Implementation check**: Evaluator `dispatch_result_method` handles `trace` at line 476-478 of `variants.rs`. Delegates to `result_error_trace()`.

**Tests found**: `tests/spec/traits/traceable/result_delegation.ori` -- `test_trace`, `test_ok_trace` (tests trace on Ok and Err). Tests exist but NOT at the path claimed by roadmap (`tests/spec/stdlib/result_traces.ori` does not exist).

**Test execution**: All traceable tests pass (4181 passed).

**Assessment**: IMPLEMENTATION EXISTS AND WORKS in evaluator. Tests exist at different path than roadmap claims. Roadmap is STALE on implementation status. INCOMPLETE MATRIX: only tests Error type, not other error types or deeper propagation chains.

---

### 7B.3.2 `Result.trace_entries()` -- marked `[ ]`

**Status**: INCOMPLETE MATRIX

**Implementation check**: Evaluator handles at line 479-491. Converts trace to struct values.

**Tests found**: `tests/spec/traits/traceable/result_delegation.ori` -- `test_entries`, `test_ok_entries`.

**Assessment**: Same as Result.trace(). IMPLEMENTATION EXISTS. Tests at different path than planned. INCOMPLETE MATRIX.

---

### 7B.3.3 `Result.has_trace()` -- marked `[ ]`

**Status**: WEAK

**Implementation check**: Evaluator handles at line 492-494.

**Tests found**: `tests/spec/traits/traceable/result_delegation.ori` -- `test_has`, `test_ok_has`, `test_fresh`.

**Assessment**: IMPLEMENTATION EXISTS. Tests at different path. WEAK -- no type variety in tests, no LLVM coverage.

---

### 7B.3.4 Trace collection at `?` propagation -- marked `[ ]`

**Status**: WEAK

**Implementation check**: The `?` operator in the evaluator does collect traces (evidenced by `result_delegation.ori` test `traced()` function successfully creating a trace via `?`).

**Tests found**: `tests/spec/traits/traceable/error_trace.ori` and `result_delegation.ori` exercise this indirectly.

**Assessment**: WORKS in evaluator. No dedicated spec test at `tests/spec/errors/trace_collection.ori` (file does not exist). WEAK -- only tested via traceable tests, not dedicated propagation tests.

---

### 7B.3.5 Context storage in Result -- marked `[ ]`

**Status**: NEEDS TESTS

**Implementation check**: The `.context()` method is mentioned in the spec but I found no evaluator implementation for it. The `dispatch_result_method_str` does not handle "context".

**Tests found**: No `tests/spec/errors/context_storage.ori` file exists.

**Assessment**: NOT IMPLEMENTED. No tests exist.

---

### 7B.3.6 Panic message format with location -- marked `[ ]`

**Status**: NEEDS TESTS

**Implementation check**: Panic messages include location info (seen in AOT catch tests). But the specific format `<message> at <file>:<line>:<column>` per spec 17.2.4 has no dedicated test.

**Tests found**: No `tests/spec/errors/panic_format.ori` file exists. The AOT catch tests verify panics are caught but don't verify message format.

**Assessment**: Partial implementation (panics include location in some paths). No dedicated format verification tests.

---

## 7B.4 Section Completion Checklist

All 5 items remain `[ ]`. This is correct -- the section is far from complete.

---

## Findings Summary

### Roadmap Accuracy Issues

1. **STALE: Result.unwrap_or** -- roadmap marks `[ ]` for implementation but it works in eval and AOT
2. **STALE: Result.ok** -- roadmap marks `[ ]` for implementation but it works in eval
3. **STALE: Result.err** -- roadmap marks `[ ]` for implementation but it works in eval
4. **STALE: Option.ok_or** -- roadmap marks `[ ]` for implementation but it works in eval
5. **STALE: Result.trace/trace_entries/has_trace** -- roadmap marks `[ ]` but implementation exists and tests exist at different paths
6. **STALE LLVM sub-items**: For is_some, is_none, is_ok, is_err, Option.unwrap_or -- the `[ ] LLVM Support` and `[ ] LLVM Rust Tests` are misleading. AOT tests already exercise these through LLVM. The methods go through standard method dispatch which the LLVM backend handles.

### True Not-Implemented Items

These methods are genuinely not implemented (the roadmap correctly marks them `[ ]`):

1. **Option.map** -- CollectionMethodResolver doesn't route Option closure-taking methods
2. **Option.and_then** -- same issue
3. **Option.filter** -- same issue
4. **Result.map** -- same issue
5. **Result.map_err** -- same issue
6. **Result.and_then** -- same issue
7. **Result.context()** -- no evaluator implementation found
8. **Panic message format** -- no dedicated test

### Architecture Note

The fundamental blocker for Option.map/and_then/filter and Result.map/map_err/and_then is that the `CollectionMethodResolver` (which handles closure-taking methods with evaluator access) only routes for Iterator, List, Range, Map, and Ordering types. Option and Result higher-order methods need to be added to this resolver, or a new resolver needs to be introduced. The evaluator's `dispatch_option_method_str` / `dispatch_result_method_str` correctly recognize these method names but cannot execute them because they lack the interpreter context needed to evaluate closures. The fix requires wiring Option/Result higher-order methods through the resolver chain, similar to how List.map/filter/fold work.

### Missing Test Files

- `tests/spec/stdlib/option.ori` -- does not exist (referenced by multiple roadmap items)
- `tests/spec/stdlib/result.ori` -- does not exist (referenced by multiple roadmap items)
- `tests/spec/stdlib/result_traces.ori` -- does not exist
- `tests/spec/errors/trace_collection.ori` -- does not exist
- `tests/spec/errors/context_storage.ori` -- does not exist
- `tests/spec/errors/panic_format.ori` -- does not exist
- `compiler/ori_llvm/tests/option_tests.rs` -- does not exist
- `compiler/ori_llvm/tests/result_tests.rs` -- does not exist
