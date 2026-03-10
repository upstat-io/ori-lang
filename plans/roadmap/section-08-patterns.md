---
section: 8
title: Pattern Evaluation
status: in-progress
reviewed: false
tier: 3
goal: All patterns evaluate correctly
spec:
  - spec/15-patterns.md
sections:
  - id: "8.1"
    title: run (Sequential Execution)
    status: in-progress
  - id: "8.2"
    title: try (Error Propagation)
    status: not-started
  - id: "8.3"
    title: recurse (Recursive Functions)
    status: in-progress
  - id: "8.4"
    title: parallel (All-Settled Concurrent Execution)
    status: in-progress
  - id: "8.5"
    title: spawn (Fire and Forget)
    status: not-started
  - id: "8.6"
    title: timeout (Time-Bounded)
    status: not-started
  - id: "8.7"
    title: cache (Memoization with TTL)
    status: not-started
  - id: "8.8"
    title: with (Resource Management)
    status: not-started
  - id: "8.9"
    title: for (Iteration with Early Exit)
    status: not-started
  - id: "8.10"
    title: Data Transformation — MOVED TO STDLIB
    status: not-started
  - id: "8.11"
    title: Resilience Patterns — MOVED TO STDLIB
    status: not-started
  - id: "8.12"
    title: Section Completion Checklist
    status: not-started
---

# Section 8: Pattern Evaluation

**Goal**: All patterns evaluate correctly

> **SPEC**: `spec/15-patterns.md`
> **DESIGN**: `design/02-syntax/04-patterns-reference.md`

---

## Pattern Categories (per spec/15-patterns.md)

The spec formalizes two distinct pattern categories:

### function_seq (Sequential Expressions)
- `run` — Sequential execution with bindings
- `try` — Sequential execution with error propagation
- `match` — Pattern matching with ordered arms
- `for` — Iteration with early exit

### function_exp (Named Expressions)
- `recurse` — Recursive computation
- `parallel`, `spawn`, `timeout`, `cache` — Concurrency/resilience
- `with` — Resource management

> **NOTE**: `map`, `filter`, `fold`, `find`, `collect`, `retry` are now stdlib functions.

---

## 8.1 run (Sequential Execution) [function_seq]

> **Future Enhancement**: Approved proposal `proposals/approved/checks-proposal.md` adds function-level `pre()` and `post()` contract declarations. See Section 15.5.

- [x] **Implement**: Grammar `run_expr = "run" "(" { binding "," } expression ")"` — spec/15-patterns.md § run [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator pattern execution — run pattern tests
  - [x] **Ori Tests**: `tests/spec/patterns/run.ori` — 12 tests pass
  - [ ] **LLVM Support**: LLVM codegen for run pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — run pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Binding `let [ "mut" ] identifier [ ":" type ] "=" expression` — spec/15-patterns.md § run [done] (2026-02-10)
- [x] **Implement**: Evaluate each binding in order — spec/15-patterns.md § run [done] (2026-02-10)
- [x] **Implement**: Each binding introduces variable into scope — spec/15-patterns.md § run [done] (2026-02-10)
- [x] **Implement**: Final expression is the result — spec/15-patterns.md § run [done] (2026-02-10)

---

## 8.2 try (Error Propagation)

- [ ] **Implement**: Grammar `try_expr = "try" "(" { binding "," } expression ")"` — spec/15-patterns.md § try
  - [ ] **Rust Tests**: `ori_patterns/src/try.rs` — try pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/try.ori` — 5 tests pass
  - [ ] **LLVM Support**: LLVM codegen for try pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — try pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Binding with `Result<T, E>` gives variable type `T` — spec/15-patterns.md § try
- [ ] **Implement**: If `Err(e)`, return immediately — spec/15-patterns.md § try
- [ ] **Implement**: Final expression is result — spec/15-patterns.md § try

---

## 8.3 recurse (Recursive Functions)

**Proposal**: `proposals/approved/recurse-pattern-proposal.md`

### Basic Implementation (complete)

- [x] **Implement**: `.condition:` property type `bool` — spec/15-patterns.md § recurse [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator pattern execution — recurse pattern tests
  - [x] **Ori Tests**: `tests/spec/patterns/recurse.ori` — 18 tests pass

- [x] **Implement**: `.base:` property type `T` — spec/15-patterns.md § recurse [done] (2026-02-10)
- [x] **Implement**: `.step:` property uses `self()` — spec/15-patterns.md § recurse [done] (2026-02-10)
- [ ] **Implement**: Optional `.memo:` default false — spec/15-patterns.md § recurse
- [ ] **Implement**: Optional `.parallel:` threshold — spec/15-patterns.md § recurse (stub: executes sequentially)
- [x] **Implement**: When `.condition` true, return `.base` — spec/15-patterns.md § recurse [done] (2026-02-10)
- [x] **Implement**: Otherwise evaluate `.step` — spec/15-patterns.md § recurse [done] (2026-02-10)
- [x] **Implement**: `self(...)` refers to recursive function — spec/15-patterns.md § recurse [done] (2026-02-10)
- [ ] **Implement**: Memoization caches during top-level call — spec/15-patterns.md § recurse

### Self Scoping (from approved proposal)

- [ ] **Implement**: `self(...)` inside `step` is recursive call — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_patterns/src/recurse.rs` — self keyword tests
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_self.ori`

- [ ] **Implement**: `self` (receiver) coexists with `self(...)` in trait methods — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_patterns/src/recurse.rs` — self scoping in traits
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_trait_self.ori`

- [ ] **Implement**: Error E1001 — `self(...)` outside `step` is compile error — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/recurse.rs` — self location error
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_errors.ori`

- [ ] **Implement**: Error E1002 — `self(...)` arity mismatch — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/recurse.rs` — arity error
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_errors.ori`

### Memoization (from approved proposal)

- [ ] **Implement**: Memo key constraint `Hashable + Eq` — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/recurse.rs` — memo key constraint
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_memo.ori`
  - [ ] **LLVM Support**: LLVM codegen for memo key hashing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — memo codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Return type constraint `Clone` with memo — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/recurse.rs` — memo return constraint
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_memo.ori`

- [ ] **Implement**: Error E1000 — non-Hashable params with memo — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/recurse.rs` — hashable error
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_errors.ori`

### Parallel Recursion (from approved proposal)

- [ ] **Implement**: `parallel: true` requires `uses Suspend` — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/capabilities.rs` — suspend capability
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_parallel.ori`
  - [ ] **LLVM Support**: LLVM codegen for parallel recursion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — parallel codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Captured values must be `Sendable` with parallel — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/recurse.rs` — sendable captures
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_parallel.ori`

- [ ] **Implement**: Return type must be `Sendable` with parallel — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/recurse.rs` — sendable return
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_parallel.ori`

- [ ] **Implement**: Error E1003 — parallel without Suspend capability — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/recurse.rs` — capability error
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_errors.ori`

### Parallel + Memo Thread Safety (from approved proposal)

- [ ] **Implement**: Thread-safe memo cache for parallel recursion — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_patterns/src/recurse.rs` — thread-safe memo
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_parallel_memo.ori`

- [ ] **Implement**: Concurrent memo access — one computes, others wait — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_patterns/src/recurse.rs` — memo stampede
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_parallel_memo.ori`

### Tail Call Optimization (from approved proposal)

- [ ] **Implement**: TCO when `self(...)` is in tail position — recurse-pattern-proposal.md
  - [ ] Compile to loop, O(1) stack space
  - [ ] **Rust Tests**: `ori_llvm/src/codegen/tco.rs` — tail call optimization
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_tco.ori`
  - [ ] **LLVM Support**: LLVM codegen for TCO
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — TCO codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Stack Limits (from approved proposal)

- [ ] **Implement**: Recursion depth limit of 1000 — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_patterns/src/recurse.rs` — depth limit tests
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_depth.ori`

- [ ] **Implement**: Panic on depth exceeded — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_patterns/src/recurse.rs` — depth panic
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_depth.ori`

- [ ] **Implement**: TCO-compiled recursion bypasses depth limit — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_patterns/src/recurse.rs` — TCO depth bypass
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_tco.ori`

---

## 8.4 parallel (All-Settled Concurrent Execution)

> **REDESIGNED**: parallel now uses list-only form and all-settled semantics.
> All tasks run to completion. Errors captured as Err values in result list.
> Pattern itself never fails.
>
> **STUB**: Evaluator has a loud stub in `can_eval.rs:eval_can_function_exp` (sequential execution + `tracing::warn!`). When implementing for real, replace the stub there.

- [x] **Implement**: `.tasks:` property (required) — spec/15-patterns.md § parallel [done] (2026-02-10)
  - [x] **Rust Tests**: Evaluator pattern execution — parallel pattern tests
  - [x] **Ori Tests**: `tests/spec/patterns/parallel.ori` — 5 tests pass
  - [ ] **LLVM Support**: LLVM codegen for parallel pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — parallel pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Returns `[Result<T, E>]` — spec/15-patterns.md § parallel
- [ ] **Implement**: Optional `.timeout:` (per-task) — spec/15-patterns.md § parallel
- [ ] **Implement**: Optional `.max_concurrent:` — spec/15-patterns.md § parallel
- [ ] **Implement**: Stub — Execute sequentially, wrap each result in Ok/Err

---

## 8.5 spawn (Fire and Forget)

> **NEW**: spawn executes tasks without waiting. Returns void immediately. Errors discarded.
>
> **STUB**: Evaluator has a loud stub in `can_eval.rs:eval_can_function_exp` (synchronous execution + `tracing::warn!`). When implementing for real, replace the stub there.

- [ ] **Implement**: `.tasks:` property (required) — spec/15-patterns.md § spawn
  - [ ] **Rust Tests**: `ori_patterns/src/spawn.rs` — spawn pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/concurrency.ori` — 3 tests pass
  - [ ] **LLVM Support**: LLVM codegen for spawn pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — spawn pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Returns `void` — spec/15-patterns.md § spawn
- [ ] **Implement**: Optional `.max_concurrent:` — spec/15-patterns.md § spawn

---

## 8.6 timeout (Time-Bounded)

> **NOTE**: Stub implementation — timeout not enforced in evaluator, always returns Ok(result).
>
> **STUB**: Evaluator has a loud stub in `can_eval.rs:eval_can_function_exp` (no timeout enforcement + `tracing::warn!`). When implementing for real, replace the stub there.

- [ ] **Implement**: `.operation:` property — spec/15-patterns.md § timeout
  - [ ] **Rust Tests**: `ori_patterns/src/timeout.rs` — timeout pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/concurrency.ori` — 4 tests pass
  - [ ] **LLVM Support**: LLVM codegen for timeout pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — timeout pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `.after:` property — spec/15-patterns.md § timeout
- [ ] **Implement**: Return `Result<T, TimeoutError>` — spec/15-patterns.md § timeout
- [ ] **Implement**: Stub — Execute `.operation`, wrap in `Ok()`

---

## 8.7 cache (Memoization with TTL)

**Proposal**: `proposals/approved/cache-pattern-proposal.md`

> **SPEC**: `cache(key: url, op: fetch(url), ttl: 5m)` — Requires `Cache` capability
>
> **STUB**: Evaluator has a loud stub in `can_eval.rs:eval_can_function_exp` (calls op directly, no memoization + `tracing::warn!`). When implementing for real, replace the stub there.

### Basic Semantics (complete)

- [ ] **Implement**: `.key:` property — spec/15-patterns.md § cache
  - [ ] **Rust Tests**: `ori_patterns/src/cache.rs` — cache pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/concurrency.ori` — 2 tests pass

- [ ] **Implement**: `.op:` property — spec/15-patterns.md § cache
- [ ] **Implement**: Stub — Execute `.op` without caching

### Key Requirements (from approved proposal)

- [ ] **Implement**: Key type constraint `Hashable + Eq` — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/cache.rs` — key constraint tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_keys.ori`
  - [ ] **LLVM Support**: LLVM codegen for key hashing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — cache key codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Value Requirements (from approved proposal)

- [ ] **Implement**: Value type constraint `Clone` — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/cache.rs` — value constraint tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_values.ori`

### TTL Semantics (from approved proposal)

- [ ] **Implement**: `.ttl:` with Duration — spec/15-patterns.md § cache
  - [ ] **Rust Tests**: `ori_patterns/src/cache.rs` — TTL tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_ttl.ori`
  - [ ] **LLVM Support**: LLVM codegen for cache TTL
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — cache TTL codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: TTL = 0 means no caching (always recompute) — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_patterns/src/cache.rs` — zero TTL tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_ttl.ori`

- [ ] **Implement**: Negative TTL is compile error — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/cache.rs` — negative TTL error
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_ttl.ori`

### Capability Requirement (from approved proposal)

- [ ] **Implement**: Requires `Cache` capability — spec/15-patterns.md § cache
  - [ ] **Rust Tests**: `ori_types/src/check/capabilities.rs` — cache capability tests
  - [ ] **Ori Tests**: `tests/spec/capabilities/cache.ori`
  - [ ] **LLVM Support**: LLVM codegen for cache capability
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — cache capability codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Concurrent Access (from approved proposal)

- [ ] **Implement**: Stampede prevention — cache-pattern-proposal.md
  - [ ] First request computes, others wait
  - [ ] All receive same result
  - [ ] **Rust Tests**: `ori_patterns/src/cache.rs` — stampede tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_concurrent.ori`

- [ ] **Implement**: Error during stampede propagates to waiting requests — cache-pattern-proposal.md
  - [ ] Entry NOT cached on error
  - [ ] **Rust Tests**: `ori_patterns/src/cache.rs` — stampede error tests

### Error Handling (from approved proposal)

- [ ] **Implement**: `Err` and panic results NOT cached — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_patterns/src/cache.rs` — error caching tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_errors.ori`

### Invalidation (from approved proposal)

- [ ] **Implement**: `Cache.invalidate(key:)` method — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_patterns/src/cache.rs` — invalidation tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_invalidation.ori`

- [ ] **Implement**: `Cache.clear()` method — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_patterns/src/cache.rs` — clear tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_invalidation.ori`

### Error Messages (from approved proposal)

- [ ] **Implement**: E0990 — cache key must be `Hashable` — cache-pattern-proposal.md
- [ ] **Implement**: E0991 — `cache` requires `Cache` capability — cache-pattern-proposal.md
- [ ] **Implement**: E0992 — TTL must be non-negative — cache-pattern-proposal.md

---

## 8.8 with (Resource Management)

**Proposal**: `proposals/approved/with-pattern-proposal.md`

> **NOTE**: Uses `.action:` instead of spec's `.use:` (`use` is reserved keyword).
>
> **STUB**: Evaluator has a loud stub in `can_eval.rs:eval_can_function_exp` (RAII acquire/action/release + `tracing::warn!`). The RAII semantics are real but blocked on type checker for lambda inference. When implementing fully, replace the stub there.

### Basic Implementation (complete)

- [ ] **Implement**: Parse `with` pattern in parser
  - [ ] **Rust Tests**: `ori_patterns/src/with.rs` — with pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/with.ori` — 4 tests pass

- [ ] **Implement**: `.acquire:` property — spec/15-patterns.md § with
- [ ] **Implement**: `.action:` property (spec uses `.use:`) — spec/15-patterns.md § with
- [ ] **Implement**: `.release:` property — spec/15-patterns.md § with
- [ ] **Implement**: Acquire resource, call `.action`, always call `.release` even on error

### Release Guarantee (from approved proposal)

- [ ] **Implement**: Release runs if acquire succeeds — with-pattern-proposal.md
  - Release runs on normal completion of use
  - Release runs on panic during use
  - Release runs on error propagation (`?`) in use
  - Release runs on `break`/`continue` in use
  - [ ] **Rust Tests**: `ori_patterns/src/with.rs` — release guarantee tests
  - [ ] **Ori Tests**: `tests/spec/patterns/with_guarantee.ori`
  - [ ] **LLVM Support**: LLVM codegen for unwinding with release
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — with unwinding codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Type Constraints (from approved proposal)

- [ ] **Implement**: `release` must return `void` — with-pattern-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/with.rs` — release type constraint
  - [ ] **Ori Tests**: `tests/spec/patterns/with_types.ori`

### Double Fault Abort (from approved proposal)

- [ ] **Implement**: If release panics during unwind, abort immediately — with-pattern-proposal.md
  - `@panic` handler NOT called
  - Both panic messages shown
  - [ ] **Rust Tests**: `ori_patterns/src/with.rs` — double fault tests
  - [ ] **Ori Tests**: `tests/spec/patterns/with_double_fault.ori`

### Error Messages (from approved proposal)

- [ ] **Implement**: E0860 — `with` pattern missing required parameter — with-pattern-proposal.md
- [ ] **Implement**: E0861 — `release` must return `void` — with-pattern-proposal.md

---

## 8.9 for (Iteration with Early Exit) — function_exp Pattern

> **STATUS**: IMPLEMENTED. Uses FunctionSeq::ForPattern with match arm syntax.
>
> **NOTE**: This is the `for(over:, match:, default:)` **pattern** with named arguments.
> The `for x in items do/yield expr` **expression** syntax is a separate construct in Section 10 (Control Flow).

- [ ] **Implement**: `.over:` property — spec/15-patterns.md § for
  - [ ] **Rust Tests**: `ori_patterns/src/for.rs` — for pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/for.ori` — 8 tests pass
  - [ ] **LLVM Support**: LLVM codegen for for pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — for pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Optional `.map:` property — spec/15-patterns.md § for
- [ ] **Implement**: `.match:` property — spec/15-patterns.md § for
- [ ] **Implement**: `.default:` property — spec/15-patterns.md § for
- [ ] **Implement**: Return first match or `.default` — spec/15-patterns.md § for

---

## 8.10 Data Transformation — MOVED TO STDLIB

> **MOVED**: Per "Lean Core, Rich Libraries", these are now stdlib functions (Section 7 - Stdlib).

| Pattern | Stdlib Location | Notes |
|---------|-----------------|-------|
| `map` | `std.iter` | `items.map(transform: fn)` |
| `filter` | `std.iter` | `items.filter(predicate: fn)` |
| `fold` | `std.iter` | `items.fold(initial: val, operation: fn)` |
| `find` | `std.iter` | `items.find(where: fn)` |
| `collect` | `std.iter` | `range.collect(transform: fn)` |

---

## 8.11 Resilience Patterns — MOVED TO STDLIB

> **MOVED**: Per "Lean Core, Rich Libraries", these are now stdlib functions (Section 7 - Stdlib).

| Pattern | Stdlib Location | Notes |
|---------|-----------------|-------|
| `retry` | `std.resilience` | `retry(operation: fn, attempts: n, backoff: strategy)` |
| `exponential` | `std.resilience` | `exponential(base: 100ms)` backoff strategy |
| `linear` | `std.resilience` | `linear(delay: 100ms)` backoff strategy |

---

## 8.12 Section Completion Checklist

- [ ] All compiler patterns implemented
- [ ] Data transformation patterns moved to stdlib
- [ ] Resilience patterns moved to stdlib
- [ ] Run full test suite: `./test-all.sh`

**Exit Criteria**: All compiler patterns evaluate correctly
