# Section 08: Pattern Evaluation — Verification Results

**Date**: 2026-03-28
**Verifier**: Claude Opus 4.6 (1M context)
**Status**: IN-PROGRESS (tier 3)

## Files Loaded Before Verification

1. `/home/eric/projects/ori_lang/CLAUDE.md` — full read (183 lines)
2. All 19 rules files in `.claude/rules/` — full read:
   - `types.md`, `typeck.md`, `eval.md`, `patterns.md`, `roadmap.md`, `ori-lang.md`, `spec.md`,
     `aot.md`, `llvm.md`, `diagnostic.md`, `parse.md`, `ir.md`, `compiler.md`, `cargo.md`,
     `registry.md`, `runtime.md`, `ori-syntax.md`, `arc.md`, `impl-hygiene.md`, `tests.md`
3. Spec: `docs/ori_lang/v2026/spec/15-patterns.md` — full read (1153 lines)
4. Section file: `plans/roadmap/section-08-patterns.md` — full read (482 lines)

## Summary

| Metric | Count |
|--------|-------|
| Top-level `[x]` items | 12 |
| Top-level `[ ]` items | 67 |
| Total `[x]` (incl. sub-items) | 18 |
| Total `[ ]` (incl. sub-items) | 188 |
| VERIFIED | 9 |
| WEAK | 3 |
| NEEDS TESTS | 67 |
| STALE | 0 |
| BUG FOUND | 0 |
| REGRESSION | 0 |

**Overall Assessment**: Section 8 is very early stage. Only sections 8.1 (run/blocks) and 8.3 (recurse basic) have meaningful implementations. Section 8.4 (parallel) has a working stub. The vast majority of items (sections 8.2, 8.5-8.9, plus all memoization, parallelism, TCO, self-scoping, error codes) are completely unimplemented with no tests. The `[x]` items that exist are generally verified but have weak LLVM/AOT coverage for the pattern-specific features (recurse pattern, parallel pattern).

---

## 8.1 run (Sequential Execution) [function_seq]

### Item: Grammar `run_expr` [x]

**Tests found**:
- `tests/spec/patterns/run.ori` — 12 test functions (simple, expressions, nested, function calls, shadowing, mutable, void, if, match, deeply nested, return value)
- Rust tests: no dedicated `run` pattern Rust tests (run is now block expressions, not a separate pattern)
- AOT: block expression tests are covered by many AOT tests across the codebase (scoping, codegen, etc.)

**Test audit**: The 12 Ori tests cover sequential binding, nesting, shadowing, mutable bindings, if/match within blocks, and deeply nested blocks. Good behavioral coverage. No compile-fail tests for invalid run syntax.

**Run result**: `cargo st tests/spec/patterns/run.ori` — 4181 passed, 0 failed, 42 skipped (full suite runs)

**Matrix coverage**:
- Types: int, void tested. Missing: str, float, bool, struct, Option, Result, list, tuple, closures
- Patterns: sequential, nested, shadowing, mutable, control flow within blocks tested. Missing: error propagation in blocks, break/continue interaction
- Backend: interpreter only for these specific tests; block expressions broadly tested in AOT

**Classification**: VERIFIED — basic implementation is solid and well-tested for integer/void types. Type matrix is narrow but block expressions are a foundational feature tested extensively elsewhere.

### Sub-items: Binding, ordering, scope, final expression [x]

**Classification**: VERIFIED — covered by the 12 tests in `run.ori` which exercise all four properties.

### Sub-items: LLVM Support, LLVM Rust Tests, AOT Tests [ ]

**Assessment**: Block expressions ARE supported in LLVM (they're fundamental to all AOT tests), but there are no dedicated "run pattern" LLVM tests. The roadmap's framing is slightly misleading — `run` was replaced by block expressions, which are thoroughly tested in AOT. However, the specific `run()` pattern syntax as a `function_seq` is not in the LLVM path.

**Classification**: NEEDS TESTS — but low priority since block expressions work in AOT. The `run()` as a distinct pattern may not need separate LLVM support.

---

## 8.2 try (Error Propagation)

### All items [ ]

**Tests found**:
- `tests/spec/patterns/try.ori` — ENTIRELY COMMENTED OUT. Zero active tests. 208 lines of commented-out code with TODO noting type checker needs `try` pattern, `?` operator, Result polymorphism, etc.
- No Rust tests for try pattern
- No AOT tests for try pattern

**Assessment**: `try` as a block expression (`try { ... }`) is partially implemented (the `?` operator works in regular blocks per other test files). But the `try` pattern as `function_seq` is NOT implemented.

**Classification**: NEEDS TESTS — zero tests active. The entire file is commented out.

---

## 8.3 recurse (Recursive Functions)

### Basic Implementation: condition, base, step, self(), evaluate [x]

**Tests found**:
- `tests/spec/patterns/recurse.ori` — 18 test functions covering:
  - `factorial`: condition/base/step, single self() call
  - `fibonacci`: double self() call, memo: true
  - `sum_to_n`: condition with <=0
  - `power`: multi-parameter self() call with argument punning
  - `fib_parallel`: parallel: 5 (stub, runs sequentially)
  - `fib_memo`/`fib_large`: memoization with large values (n=20, n=30)
  - `gcd`: multi-parameter tail-recursive self() call
  - `ackermann`: nested self() calls with memo
  - `sum_tail`/`factorial_tail`: tail-recursive patterns
  - `list_sum`/`list_max`: recursion with list parameters
  - `count_char`: recursion with string parameters
  - `binary_search`: complex condition with nested if/else
  - `range_list`: workaround using for/yield (list spread in recurse not supported)
  - `is_even`: boolean return with `!self(n-1)`
  - `countdown`: string concatenation in recursion

- Rust tests (`compiler/ori_patterns/src/recurse/tests.rs`) — 6 tests:
  - `recurse_returns_base_when_condition_true`: mock condition=true, verify base returned
  - `recurse_returns_step_when_condition_false`: mock condition=false, verify step returned
  - `recurse_pattern_name`: "recurse"
  - `recurse_required_props`: ["condition", "base", "step"]
  - `recurse_optional_props`: ["memo"]
  - `recurse_has_scoped_bindings_for_self`: self binding for step prop

- AOT tests (`compiler/ori_llvm/tests/aot/recursion.rs`) — 22 tests covering:
  - Direct recursion: factorial, fibonacci, sum_to, power, gcd
  - Tail-recursive with accumulator: fact_acc, sum_acc, count_digits
  - Mutual recursion: is_even/is_odd, count_a/count_b
  - Recursion with Result: safe_divide with `?`
  - Recursion with match: countdown, collatz
  - Depth tests: 100 levels, 1000 levels
  - Struct parameters: move_towards_origin
  - Binary search, Ackermann, Tower of Hanoi
  - TCO stress tests: countdown_deep (200K), collatz_deep, both_branches, fact_deep
  - `recurse()` pattern in AOT: `test_tail_rec_recurse_pattern`, `test_tail_rec_recurse_deep`
  - RC-managed args: list_param (100K iterations), string_param
  - Mixed tail/non-tail: one branch tail, one branch non-tail

**Run results**:
- `cargo st tests/spec/patterns/recurse.ori` — 4181 passed, 0 failed, 42 skipped
- `cargo test -p ori_llvm -- recursion` — 49 passed, 0 failed
- `cargo test -p ori_patterns` — 372 passed, 0 failed

**Matrix coverage**:
- Types: int, str, bool, [int], list, struct (via AOT tests). Missing: float, Option, Result in recurse pattern specifically (though Result recursion tested via direct recursion in AOT)
- Patterns: single self(), double self(), multi-param self(), nested self(), tail recursion, non-tail, memo, parallel stub. Good coverage.
- Backend: Both interpreter and LLVM/AOT. Two dedicated `recurse()` pattern AOT tests exist.
- Semantic pins: `fib_large` (n=30 = 832040) would fail without memo, `test_tail_rec_recurse_deep` (200K depth) would stack overflow without TCO

**Assessment**: The basic recurse pattern (condition/base/step/self) is well-tested across both backends with good type and pattern coverage. Memoization works in the interpreter. AOT has `recurse()` pattern support including TCO.

**Classification**:
- `condition: bool` [x]: VERIFIED
- `base: T` [x]: VERIFIED
- `step: self()` [x]: VERIFIED
- `memo: false default` [ ]: Marked unchecked but memo IS implemented and tested — STALE CHECKBOX (memo works, tests pass with `memo: true`)
- `parallel: threshold` [ ]: Stub implemented, test passes (runs sequentially)
- `When condition true, return base` [x]: VERIFIED
- `Otherwise evaluate step` [x]: VERIFIED
- `self(...) refers to recursive function` [x]: VERIFIED
- `Memoization caches during top-level call` [ ]: Marked unchecked but IS working — tests use `memo: true` successfully

**NOTE**: The `memo:` optional prop is listed as unchecked `[ ]` in the roadmap but IS implemented and tested. `fib_memo(20)=6765` and `fib_memo(30)=832040` both pass. The Rust test confirms `optional_props() == &["memo"]`. The evaluator code in `eval_can_recurse()` handles memo by wrapping `self` in a `MemoizedFunctionValue`. **These two checkbox items should be `[x]`**.

### Self Scoping [ ]

**Tests found**: None. No `tests/spec/patterns/recurse_self.ori` or `recurse_trait_self.ori` exist. No `self` scoping tests in trait context.

**Classification**: NEEDS TESTS — no implementation evidence found for self-scoping in trait methods.

### Error Codes E1001, E1002 [ ]

**Tests found**: No compile-fail tests for `self()` outside step or arity mismatch. No grep hits for E1001/E1002 in test files.

**Classification**: NEEDS TESTS

### Memoization key constraints (Hashable + Eq), return constraint (Clone), E1000 [ ]

**Tests found**: No type constraint tests exist. Memoization works but no tests verify that non-Hashable keys are rejected.

**Classification**: NEEDS TESTS — memoization works but constraint checking is untested.

### Parallel Recursion (parallel: true, Suspend, Sendable, E1003) [ ]

**Tests found**: `fib_parallel` test exists with `parallel: 5` but it runs sequentially (stub). No Suspend capability tests, no Sendable constraint tests, no E1003 tests.

**Classification**: NEEDS TESTS — stub works but real parallel recursion is not implemented.

### Parallel + Memo Thread Safety [ ]

**Classification**: NEEDS TESTS — not implemented.

### Tail Call Optimization [ ]

**Tests found**: AOT tests cover TCO for direct recursion (`tail_rec_countdown_deep`, 200K depth) and for `recurse()` pattern (`test_tail_rec_recurse_deep`, 200K depth). These demonstrate TCO IS working in the LLVM backend.

**Assessment**: TCO appears to be working in AOT. The roadmap marks this unchecked, but `test_tail_rec_recurse_deep` proves the `recurse()` pattern gets TCO in LLVM. However, the interpreter does NOT have TCO (it relies on the runtime stack). The depth limit item is also not tested.

**Classification**: WEAK — TCO works in LLVM (proven by 200K depth test), but the roadmap item is about the `recurse` pattern specifically, and there's no interpreter TCO or explicit depth limit enforcement.

### Stack Limits (depth 1000, panic on exceed, TCO bypasses) [ ]

**Tests found**: No depth limit tests for the `recurse()` pattern specifically. AOT tests test direct recursion depth (100, 1000 levels).

**Classification**: NEEDS TESTS

---

## 8.4 parallel (All-Settled Concurrent Execution)

### Item: `.tasks:` property [x]

**Tests found**:
- `tests/spec/patterns/parallel.ori` — MOSTLY COMMENTED OUT. Only 5 active tests, and these are WORKAROUND tests using `for...yield` instead of the actual `parallel()` pattern:
  - `test_compute_tasks`: uses `for t in tasks yield compute_square(n: t)` — NOT testing parallel
  - `test_task_list`: uses `for f in fns yield f()` — NOT testing parallel
  - `test_task_results`: uses `for n in items yield process_item(n:)` — NOT testing parallel
  - `test_accumulate`: uses `for...yield` — NOT testing parallel
  - `test_collect_all_semantics`: uses `for...yield` — NOT testing parallel

- `tests/spec/patterns/parallel_threads.ori` — tests `thread_id()` and actual `parallel()` invocation with thread-based execution

- Rust tests (`compiler/ori_patterns/src/parallel/mod.rs`, `parallel_tests.rs`) — parallel pattern unit tests exist with mock executors

- Evaluator stub: `eval_can_function_exp()` has a working sequential stub for `FunctionExpKind::Parallel` that wraps results in `Ok`/`Err`

**Assessment**: The 5 tests in `parallel.ori` are MISLEADING — they do NOT test the `parallel()` pattern at all. They test `for...yield` which is a completely different construct. The actual `parallel()` pattern tests are all commented out. However, `parallel_threads.ori` does test the actual `parallel()` pattern. The evaluator has a working sequential stub.

**Run results**: All pass (4181 passed), but the "parallel pattern tests" in `parallel.ori` are testing `for...yield`, not `parallel()`.

**Classification**: WEAK — the roadmap claims "5 tests pass" but these tests do not exercise the `parallel()` pattern. The evaluator stub exists and works (sequential execution). `parallel_threads.ori` provides actual coverage. The tests in `parallel.ori` should be relabeled.

### Sub-items: Returns `[Result<T, E>]`, timeout, max_concurrent, stub [ ]

**Classification**: NEEDS TESTS — all commented out in `concurrency.ori` and `parallel.ori`.

---

## 8.5 spawn (Fire and Forget) [ ]

**Tests found**:
- `tests/spec/patterns/concurrency.ori` — all spawn tests COMMENTED OUT
- Rust tests (`compiler/ori_patterns/src/spawn/tests.rs`) — 5 unit tests:
  - `spawn_empty_list_returns_void`: verifies empty tasks returns Void
  - `spawn_pattern_name`: "spawn"
  - `spawn_required_props`: ["tasks"]
  - `spawn_does_not_allow_arbitrary_props`
  - `spawn_requires_list_for_tasks`: error on non-list input

- Evaluator stub exists: `FunctionExpKind::Spawn` in `eval_can_function_exp` — synchronous execution + `tracing::warn!`

**Classification**: NEEDS TESTS — Rust unit tests exist for the pattern struct but no Ori spec tests are active. No AOT tests.

---

## 8.6 timeout (Time-Bounded) [ ]

**Tests found**:
- `tests/spec/patterns/concurrency.ori` — all timeout tests COMMENTED OUT
- Rust tests (`compiler/ori_patterns/src/timeout/tests.rs`) — 4 unit tests:
  - `timeout_success_wraps_in_ok`: verify Ok wrapping
  - `timeout_error_wraps_in_err`: verify Err wrapping on operation failure
  - `timeout_pattern_name`: "timeout"
  - `timeout_required_props`: ["operation", "after"]

- Evaluator stub exists: `FunctionExpKind::Timeout` — wraps result in `Ok()`, no timeout enforcement

**Classification**: NEEDS TESTS — Rust unit tests exist for pattern struct but no Ori spec tests active. No AOT tests.

---

## 8.7 cache (Memoization with TTL) [ ]

**Tests found**:
- `tests/spec/patterns/concurrency.ori` — all cache tests COMMENTED OUT
- Rust tests (`compiler/ori_patterns/src/cache/tests.rs`) — 4 unit tests:
  - `cache_non_function_returns_value_directly`
  - `cache_pattern_name`: "cache"
  - `cache_required_props`: ["operation"]
  - `cache_optional_props`: ["key", "ttl"]

- Evaluator stub exists: `FunctionExpKind::Cache` — calls operation without memoization

**All sub-items** (key constraints, value constraints, TTL semantics, capability, concurrent access, error handling, invalidation, error codes E0990-E0992): NEEDS TESTS — nothing implemented or tested.

**Classification**: NEEDS TESTS for all items in 8.7.

---

## 8.8 with (Resource Management) [ ]

**Tests found**:
- `tests/spec/patterns/with.ori` — ENTIRELY COMMENTED OUT (99 lines)
- Rust tests (`compiler/ori_patterns/src/with_pattern/tests.rs`) — 4 unit tests:
  - `with_pattern_name`: "with"
  - `with_required_props`: ["acquire", "action"]
  - `with_optional_props`: ["release"]
  - `with_returns_action_result`: mock acquire + action, verify result

- Evaluator stub exists: `FunctionExpKind::With` — RAII acquire/action/release with release guarantee

**Assessment**: The evaluator stub is more than a stub — it actually implements RAII semantics (always calls release even on error). But no Ori tests exercise this.

**All sub-items** (release guarantee, type constraints, double fault, error codes E0860-E0861): NEEDS TESTS

**Classification**: NEEDS TESTS for all items in 8.8.

---

## 8.9 for (Iteration with Early Exit) — function_exp Pattern [ ]

**Tests found**:
- `tests/spec/patterns/for.ori` — ENTIRELY COMMENTED OUT (462 lines). Contains commented-out tests for:
  - `for(over:, match:, default:)` pattern (8 tests)
  - `for/do` loops (4 tests)
  - `for/yield` comprehensions (4 tests)
  - `for/if/yield` filtering (4 tests)
  - Nested loops, break/continue, labels, map collection, range by step

**NOTE**: The `for(over:, match:, default:)` pattern (function_exp) is distinct from `for x in items do/yield` (expression syntax). Both are commented out in this file. However, `for x in items do/yield` IS extensively tested in other spec test files (Section 10 Control Flow tests).

**Classification**: NEEDS TESTS for the `for(over:, match:, default:)` function_exp pattern specifically.

---

## 8.10 Data Transformation — MOVED TO STDLIB

**Assessment**: Correctly moved to stdlib. `map`, `filter`, `fold`, `find`, `collect` are tested in `tests/spec/patterns/data.ori` as collection methods, and extensively in `tests/spec/traits/iterator/` tests.

**Classification**: N/A — correctly moved, no action needed.

---

## 8.11 Resilience Patterns — MOVED TO STDLIB

**Assessment**: Correctly moved. `retry` not yet implemented.

**Classification**: N/A — correctly moved, no action needed.

---

## 8.12 Section Completion Checklist [ ]

All 4 items unchecked:
- [ ] All compiler patterns implemented — INCOMPLETE (try, cache, with, for pattern, spawn, timeout all stub/unimplemented)
- [ ] Data transformation moved to stdlib — DONE
- [ ] Resilience patterns moved to stdlib — DONE (retry not implemented)
- [ ] Run full test suite — not done as section is incomplete

---

## catch (Not in Roadmap but Implemented)

**NOTE**: The `catch(expr: ...)` pattern is implemented and tested but NOT listed as a roadmap item in Section 8. It is mentioned in the spec (Section 15.6.1) and has:

- `tests/spec/patterns/catch.ori` — 7 active tests:
  - `test_catch_success`: Ok wrapping on success
  - `test_catch_panic`: Err wrapping on panic
  - `test_catch_message`: panic message capture
  - `test_catch_div_zero`: division by zero capture
  - `test_catch_ok_value`: value computation in Ok
  - `test_catch_string`: string expression in catch
  - `test_catch_nested`: nested catch (inner catches inner, outer remains Ok)

- Evaluator: `eval_can_catch()` implemented in `function_exp.rs`

**Run result**: All 7 tests pass.

**Classification**: VERIFIED — `catch` is fully working but missing from the roadmap. Should be added as a completed item.

---

## Cross-Cutting Concerns

### LLVM/AOT Coverage for Patterns

The AOT test file `compiler/ori_llvm/tests/aot/patterns.rs` (23 tests) covers:
- Or-patterns (int, char, bool, in-loop)
- Guard clauses (basic, with binding, complex conditions, in-loop)
- Tuple patterns (basic, 3-element, wildcards, from function)
- Binding patterns (capture, mixed with literals)
- Combined: guard + tuple, Result dispatch, nested match, fizzbuzz

These test MATCH patterns in AOT, NOT function_exp patterns (recurse, parallel, cache, etc.).

For `recurse()` specifically in AOT: 2 tests exist (`test_tail_rec_recurse_pattern`, `test_tail_rec_recurse_deep`). No AOT tests for parallel, spawn, timeout, cache, with, for pattern, or catch.

### Negative Testing

- `tests/spec/patterns/exhaustiveness_fail.ori` — 10 compile_fail tests for non-exhaustive and redundant patterns
- No compile_fail tests for any pattern-specific error codes (E0860, E0861, E0990-E0992, E1000-E1003)
- No negative tests for pattern misuse (cache without Cache capability, recurse self() outside step, etc.)

### Spec Alignment

The spec (Clause 15) defines patterns with specific error codes and constraints. The roadmap accurately captures these, but none of the constraint checking (type constraints, capability requirements, error codes) is implemented or tested.

---

## Stale Checkboxes (Items Marked `[ ]` But Working)

1. **`memo: false` default** (line 122): Marked `[ ]` but memo IS implemented. The evaluator's `eval_can_recurse()` handles `memo: true` by wrapping self in `MemoizedFunctionValue`. Tests `fib_memo(20)=6765` and `fib_memo(30)=832040` pass. Should be `[x]`.

2. **`Memoization caches during top-level call`** (line 127): Marked `[ ]` but IS working. Same evidence as above. Should be `[x]`.

---

## Detailed Item-by-Item Classification

### 8.1 run

| Line | Item | Mark | Classification |
|------|------|------|----------------|
| 81 | Grammar run_expr | [x] | VERIFIED — 12 Ori tests |
| 82 | Rust Tests | [x] | VERIFIED — pattern struct tests |
| 83 | Ori Tests: 12 tests | [x] | VERIFIED |
| 84 | LLVM Support | [ ] | NEEDS TESTS (blocks work in LLVM but no dedicated tests) |
| 85 | LLVM Rust Tests | [ ] | NEEDS TESTS |
| 86 | AOT Tests | [ ] | NEEDS TESTS |
| 88 | Binding syntax | [x] | VERIFIED |
| 89 | Evaluate each binding in order | [x] | VERIFIED |
| 90 | Each binding introduces scope | [x] | VERIFIED |
| 91 | Final expression is result | [x] | VERIFIED |

### 8.2 try

| Line | Item | Mark | Classification |
|------|------|------|----------------|
| 97 | Grammar try_expr | [ ] | NEEDS TESTS — all commented out |
| 104 | Binding with Result<T,E> | [ ] | NEEDS TESTS |
| 105 | If Err(e), return immediately | [ ] | NEEDS TESTS |
| 106 | Final expression is result | [ ] | NEEDS TESTS |

### 8.3 recurse

| Line | Item | Mark | Classification |
|------|------|------|----------------|
| 116 | condition: bool | [x] | VERIFIED |
| 120 | base: T | [x] | VERIFIED |
| 121 | step: self() | [x] | VERIFIED |
| 122 | memo: false default | [ ] | STALE CHECKBOX — memo works, should be [x] |
| 123 | parallel: threshold | [ ] | NEEDS TESTS (stub, not real parallelism) |
| 124 | When condition true, return base | [x] | VERIFIED |
| 125 | Otherwise evaluate step | [x] | VERIFIED |
| 126 | self(...) refers to recursive function | [x] | VERIFIED |
| 127 | Memoization caches during top-level call | [ ] | STALE CHECKBOX — memo works, should be [x] |
| 131 | self(...) inside step is recursive call | [ ] | NEEDS TESTS (self-scoping in trait context) |
| 135 | self receiver coexists with self(...) | [ ] | NEEDS TESTS |
| 139 | Error E1001 self outside step | [ ] | NEEDS TESTS |
| 143 | Error E1002 arity mismatch | [ ] | NEEDS TESTS |
| 149 | Memo key constraint Hashable + Eq | [ ] | NEEDS TESTS |
| 156 | Return type constraint Clone | [ ] | NEEDS TESTS |
| 160 | Error E1000 non-Hashable with memo | [ ] | NEEDS TESTS |
| 166 | parallel: true requires Suspend | [ ] | NEEDS TESTS |
| 173 | Captured values must be Sendable | [ ] | NEEDS TESTS |
| 177 | Return type must be Sendable | [ ] | NEEDS TESTS |
| 181 | Error E1003 parallel without Suspend | [ ] | NEEDS TESTS |
| 187 | Thread-safe memo cache | [ ] | NEEDS TESTS |
| 191 | Concurrent memo access | [ ] | NEEDS TESTS |
| 197 | TCO when self() in tail position | [ ] | WEAK — works in LLVM but not interpreter, no dedicated test |
| 207 | Recursion depth limit 1000 | [ ] | NEEDS TESTS |
| 211 | Panic on depth exceeded | [ ] | NEEDS TESTS |
| 215 | TCO-compiled bypasses depth limit | [ ] | NEEDS TESTS |

### 8.4 parallel

| Line | Item | Mark | Classification |
|------|------|------|----------------|
| 229 | .tasks: property | [x] | WEAK — "5 tests pass" but tests use for/yield not parallel() |
| 230 | Rust Tests | [x] | VERIFIED — pattern struct tests exist |
| 231 | Ori Tests: 5 tests | [x] | WEAK — misleading, tests don't use parallel() |
| 232 | LLVM Support | [ ] | NEEDS TESTS |
| 236 | Returns [Result<T,E>] | [ ] | NEEDS TESTS |
| 237 | Optional .timeout: | [ ] | NEEDS TESTS |
| 238 | Optional .max_concurrent: | [ ] | NEEDS TESTS |
| 239 | Stub — sequential execution | [ ] | Stub IS implemented in evaluator, should be [x] |

### 8.5 spawn

All items [ ] — NEEDS TESTS (Rust unit tests exist, no Ori spec tests active)

### 8.6 timeout

All items [ ] — NEEDS TESTS (Rust unit tests exist, no Ori spec tests active)

### 8.7 cache

All items [ ] — NEEDS TESTS (Rust unit tests exist, no Ori spec tests active)

### 8.8 with

All items [ ] — NEEDS TESTS (Rust unit tests exist, evaluator stub has RAII semantics, no Ori spec tests active)

### 8.9 for pattern

All items [ ] — NEEDS TESTS (all commented out)

---

## Findings

### STALE CHECKBOX items (should be updated in roadmap):
1. Line 122: `memo: false default` — memo IS implemented and tested, should be `[x]`
2. Line 127: `Memoization caches during top-level call` — IS working, should be `[x]`
3. Line 239: `Stub — Execute sequentially, wrap each result in Ok/Err` — stub IS implemented in evaluator for parallel, spawn, timeout, cache, with

### Missing from roadmap:
1. `catch(expr: ...)` pattern is implemented and tested (7 passing tests) but not listed in Section 8. The spec covers it in Section 15.6.1. Should be added as a completed `[x]` item.

### Test quality concerns:
1. `parallel.ori` claims "5 tests pass" but those 5 tests test `for...yield`, NOT the `parallel()` pattern. The tests are misleading.
2. All concurrency pattern tests in `concurrency.ori` are commented out — zero active coverage for `parallel`, `spawn`, `timeout`, `cache` as Ori spec tests.
3. `try.ori` is entirely commented out — zero active coverage.
4. `with.ori` is entirely commented out — zero active coverage.
5. `for.ori` is entirely commented out — zero active coverage for the `for(over:, match:, default:)` pattern.

### What IS working well:
1. `recurse()` pattern: excellent coverage across both backends with 18 Ori tests + 6 Rust unit tests + 2 AOT-specific recurse tests + 22 general recursion AOT tests
2. `catch()` pattern: 7 well-structured Ori tests covering success, panic capture, message capture, div-by-zero, nested catch
3. Block expressions (formerly `run`): 12 solid tests
4. Match patterns in AOT: 23 tests covering or-patterns, guards, tuples, bindings, combined patterns

### Priorities for next implementation work:
1. Uncomment and fix `try.ori` tests (try blocks with `?` are partially working)
2. Write actual `parallel()` spec tests (replace misleading for/yield tests)
3. Add `catch` to the roadmap as a completed item
4. Fix stale checkboxes for memo
5. Implement and test error codes (E0860, E0861, E0990-E0992, E1000-E1003)
