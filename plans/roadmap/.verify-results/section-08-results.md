# Section 08: Pattern Evaluation -- Verification Results

**Verified**: 2026-03-19
**Section**: `plans/roadmap/section-08-patterns.md`
**Status**: in-progress (18/205 items = 8%)
**Spec**: `docs/ori_lang/v2026/spec/15-patterns.md`

---

## Summary

Section 08 covers `function_exp` patterns (recurse, parallel, spawn, timeout, cache, with, for) and block expressions (run/blocks, try, match). The section has 18 checked `[x]` items and 187 unchecked `[ ]` items. Most subsections are not-started; only 8.1 (run), 8.3 (recurse basic), and 8.4 (parallel basic) have checked items.

**Overall assessment**: Checked items are largely VERIFIED at the interpreter level. Block expressions (formerly "run") and recurse basic semantics work correctly. The parallel pattern has real threaded execution in `ori_patterns` but the evaluator uses a sequential stub for the canonical IR path. All LLVM/AOT items are unchecked and genuinely not-started. Several unchecked subsections (8.2 try, 8.5-8.9) are genuinely not implemented -- test files exist but are entirely commented out.

---

## 8.1 run (Sequential Execution)

### [x] Grammar `run_expr` -- VERIFIED
- Block expressions (replacement for `run()`) parse and evaluate correctly
- `tests/spec/patterns/run.ori` has 12 active `@test_` tests (roadmap claims 12 -- accurate)
- All tests pass via `cargo st tests/spec/patterns/run.ori`
- Tests cover: simple bindings, nested blocks, function calls, shadowing, mutable bindings, return values, void blocks, if/match in blocks, deeply nested blocks

### [x] Rust Tests -- WEAK TESTS
- No dedicated Rust-side unit tests for "run" pattern evaluation found
- The `ori_patterns` crate has no `run` pattern module (blocks are handled directly by the interpreter, not via the pattern registry)
- This is architecturally correct (blocks are not `function_exp` patterns) but the roadmap item references "Evaluator pattern execution -- run pattern tests" which is misleading

### [x] Ori Tests -- VERIFIED
- 12 tests confirmed active and passing

### [x] Binding `let [mut] identifier [: type] = expression` -- VERIFIED
- Tests cover both `let x = v` and `let x = v; x = x + 1` (mutable reassignment)
- Shadowing tested (`let x = 1; let x = x + 1`)

### [x] Evaluate each binding in order -- VERIFIED
- `run_with_expressions` tests sequential binding evaluation (`a = 10; b = a * 2; c = b + 5`)

### [x] Each binding introduces variable into scope -- VERIFIED
- Tested implicitly via all binding tests

### [x] Final expression is the result -- VERIFIED
- `run_return_value`, `run_with_if`, `run_with_match`, `run_deeply_nested` all verify last-expression-as-value

### [ ] LLVM Support -- CONFIRMED NOT STARTED
- No `ori_llvm` tests for block expression patterns
- AOT recursion tests (`compiler/ori_llvm/tests/aot/recursion.rs`) test direct recursion, not `recurse()` pattern

---

## 8.2 try (Error Propagation) -- CONFIRMED NOT STARTED

- `tests/spec/patterns/try.ori` exists (208 lines) but is **entirely commented out**
- Comment explains: "Type checker needs various features -- try pattern, ? operator, Result type polymorphism"
- All 4 `[ ]` items genuinely not started

---

## 8.3 recurse (Recursive Functions)

### Basic Implementation

#### [x] `.condition:` property type `bool` -- VERIFIED
- `RecursePattern.required_props()` returns `["condition", "base", "step"]`
- `infer_recurse()` in `ori_types/src/infer/expr/concurrency.rs` unifies condition with `Idx::BOOL`
- `evaluate()` checks `cond_val.is_truthy()`
- Tested by all recurse tests (factorial, fibonacci, etc.)

#### [x] Rust Tests -- VERIFIED
- `compiler/ori_patterns/src/recurse/tests.rs` has 6 tests:
  - `recurse_returns_base_when_condition_true` -- condition=true returns base value
  - `recurse_returns_step_when_condition_false` -- condition=false returns step value
  - `recurse_pattern_name` -- name is "recurse"
  - `recurse_required_props` -- ["condition", "base", "step"]
  - `recurse_optional_props` -- ["memo"]
  - `recurse_has_scoped_bindings_for_self` -- self binding for step prop

#### [x] Ori Tests -- VERIFIED
- `tests/spec/patterns/recurse.ori` has 18 active `@test_` tests (roadmap claims 18 -- accurate)
- All pass via `cargo st tests/spec/patterns/recurse.ori`
- Tests cover: factorial, fibonacci, sum_to_n, power, parallel stub, memo, large fib, GCD, Ackermann, tail recursion, list processing, string processing, binary search, range_list (uses for/yield), is_even, countdown

#### [x] `.base:` property type `T` -- VERIFIED
- Type inference unifies base and step types in `infer_recurse()`

#### [x] `.step:` property uses `self()` -- VERIFIED
- `scoped_bindings()` returns a `ScopedBinding { name: "self", for_props: ["step"] }`
- Tests verify `self(...)` calls work in factorial, fibonacci, GCD, binary_search etc.

#### [ ] Optional `.memo:` -- WEAK TESTS (partially implemented)
- `RecursePattern.optional_props()` returns `["memo"]` -- registered
- `eval_can_recurse()` in `function_exp.rs` handles `memo: true` by wrapping self in `MemoizedFunction`
- Ori tests `fib_memo` and `fib_large` exercise memoization and pass
- BUT: no Rust unit test verifies memoization behavior (only that `optional_props` includes "memo")
- No test verifies `Hashable + Eq` constraint on params or `Clone` on return type (spec requirement)
- **Status**: Functionally working but constraints not enforced

#### [ ] Optional `.parallel:` threshold -- WEAK TESTS (partially implemented)
- BUG FOUND: spec says `parallel: bool = false` but `recurse.ori` test uses `parallel: 5` (int threshold)
- The `RecursePattern.optional_props()` only returns `["memo"]` -- `parallel` is NOT registered as optional
- `infer_recurse()` processes props positionally (first 3 only), silently ignoring extra props like `parallel: 5`
- The `fib_parallel` test passes because `parallel: 5` is silently ignored -- the recursion runs sequentially
- Error doc E1010 and E3002 document `parallel` as optional for recurse, but the pattern definition does not include it
- **Status**: Accepted by parser/typechecker due to prop processing, but NOT implemented

#### [x] When `.condition` true, return `.base` -- VERIFIED
- Rust test `recurse_returns_base_when_condition_true` + all factorial/fibonacci base cases

#### [x] Otherwise evaluate `.step` -- VERIFIED
- Rust test `recurse_returns_step_when_condition_false` + all recursive step tests

#### [x] `self(...)` refers to recursive function -- VERIFIED
- Tested by all recursive tests: `self(n - 1)`, `self(a: b, b: a % b)` etc.

#### [ ] Memoization caches during top-level call -- NOT VERIFIED
- Implementation exists in `eval_can_recurse()` but no test specifically verifies cache lifetime
- No test verifies cache is discarded after top-level call returns
- **Status**: Likely working but untested at this granularity

### Self Scoping, Memoization constraints, Parallel Recursion, TCO, Stack Limits -- CONFIRMED NOT STARTED
- All `[ ]` items in sections 8.3 Self Scoping through Stack Limits are genuinely not started
- No error codes E1000-E1003 implemented
- No `Hashable + Eq` constraint enforcement
- No `Sendable` checks
- No TCO (tail call optimization to loop)
- No recursion depth limit of 1000
- Referenced test files (`recurse_self.ori`, `recurse_trait_self.ori`, etc.) do not exist

---

## 8.4 parallel (All-Settled Concurrent Execution)

### [x] `.tasks:` property -- VERIFIED (with caveats)

**Ori Tests**: `tests/spec/patterns/parallel.ori` has 5 active tests (roadmap claims 5 -- accurate). However, these tests do NOT actually use the `parallel()` pattern -- they use `for...yield` as a workaround:
- `test_compute_tasks` -- uses `for t in tasks yield compute_square(n: t)`
- `test_task_list` -- uses `for f in fns yield f()`
- `test_task_results` -- uses `for n in items yield process_item(n:)`
- `test_accumulate` -- uses `for item in items yield item * 2`
- `test_collect_all_semantics` -- uses `for r in results if is_ok(r:) yield ...`

All actual `parallel()` pattern tests are commented out (lines 16-133).

**Evaluator Implementation**: Two paths exist:
1. `ori_patterns/src/parallel/mod.rs` -- Full `ParallelPattern` with real threading (Semaphore, `thread::scope`), timeout, max_concurrent
2. `ori_eval/src/interpreter/can_eval/function_exp.rs` -- Sequential stub via canonical IR (`FunctionExpKind::Parallel` arm, line 219)

The canonical IR path (used by the actual evaluator) is a sequential stub that wraps results in Ok/Err.

**Rust Tests**: `ori_patterns` has 372 tests passing (includes parallel tests). The `parallel_tests::stress::rapid_spawn_and_complete` test exercises the threaded implementation.

### [ ] Returns `[Result<T, E>]` -- PARTIALLY IMPLEMENTED
- The canonical stub wraps results in `Value::ok()` / `Value::err()`
- The `ori_patterns` `ParallelPattern` returns properly wrapped results

### [ ] Optional `.timeout:` -- PARTIALLY IMPLEMENTED
- `ParallelPattern` in `ori_patterns` handles timeout via `Duration::from_millis` and `mpsc::recv_timeout`
- Canonical stub ignores timeout entirely

### [ ] Optional `.max_concurrent:` -- PARTIALLY IMPLEMENTED
- `ParallelPattern` in `ori_patterns` implements `Semaphore` for concurrency limiting
- Canonical stub ignores max_concurrent

### [ ] Stub execution -- VERIFIED
- Canonical stub in `function_exp.rs` executes tasks sequentially and emits `tracing::warn!`

---

## 8.5 spawn -- CONFIRMED NOT STARTED
- `concurrency.ori` entirely commented out
- Evaluator has a sequential stub in `function_exp.rs` (line 238)
- No active tests

## 8.6 timeout -- CONFIRMED NOT STARTED
- `concurrency.ori` entirely commented out
- Evaluator stub wraps in `Value::ok()` without timeout enforcement (line 249)
- No active tests

## 8.7 cache -- CONFIRMED NOT STARTED
- `concurrency.ori` entirely commented out
- Evaluator stub calls operation directly without caching (line 207)
- Proposal exists: `proposals/approved/cache-pattern-proposal.md`
- No active tests

## 8.8 with -- CONFIRMED NOT STARTED
- `with.ori` entirely commented out (blocked on type checker lambda inference)
- Evaluator has a partial RAII stub in `function_exp.rs` (line 254)
- No active tests

## 8.9 for pattern -- CONFIRMED NOT STARTED
- `for.ori` entirely commented out (blocked on type checker features)
- Note: `for x in items do/yield` expression syntax works (Section 10 control flow), but the `for(over:, match:, default:)` function_exp pattern does not
- No active tests

## 8.10-8.11 Data Transformation / Resilience -- MOVED TO STDLIB
- Correctly marked as moved. Not applicable to this section.

---

## Bugs Found

### BUG-08-01: `parallel` property silently ignored on `recurse`
- **Spec**: `recurse` has `parallel: bool = false` (spec 15.3.1)
- **Code**: `RecursePattern.optional_props()` returns `["memo"]` only -- no `parallel`
- **Test**: `recurse.ori` line 132 uses `parallel: 5` (int, not bool) and passes silently
- **Impact**: Users may think parallel recursion works when it does not
- **Root cause**: `infer_recurse()` processes props positionally and ignores extras; `optional_props` does not include "parallel"
- **Fix needed**: Either add "parallel" to `optional_props` (for stub) or validate and error on unknown props

### BUG-08-02: `infer_recurse()` uses positional prop processing
- **Location**: `ori_types/src/infer/expr/concurrency.rs:44-53`
- **Issue**: Props are processed as first=condition, second=base, third=step regardless of names
- **Impact**: `recurse(base: 1, condition: true, step: x)` would mis-assign types
- **Spec**: Props are named (`condition:`, `base:`, `step:`) and should be matched by name
- **Note**: The evaluator (`eval_can_recurse`) correctly uses name-based lookup via `find_prop_can_id`

---

## Verification Statistics

| Category | Count | Details |
|----------|-------|---------|
| VERIFIED | 13 | Core run/recurse semantics work correctly |
| WEAK TESTS | 3 | memo (no constraint tests), parallel.ori (workaround tests), Rust run tests (none exist) |
| BUG FOUND | 2 | parallel prop ignored, positional prop inference |
| CONFIRMED NOT STARTED | 7 subsections | try, spawn, timeout, cache, with, for pattern, all LLVM/AOT |
| NEEDS TESTS | 2 | Memo cache lifetime, parallel actual pattern usage |

## Test Commands Used

```bash
timeout 150 cargo st tests/spec/patterns/run.ori      # 4181 passed, 0 failed, 42 skipped
timeout 150 cargo st tests/spec/patterns/recurse.ori   # 4181 passed, 0 failed, 42 skipped
timeout 150 cargo st tests/spec/patterns/parallel.ori  # 4181 passed, 0 failed, 42 skipped
timeout 150 cargo test -p ori_patterns                 # 372 passed, 0 failed
```
