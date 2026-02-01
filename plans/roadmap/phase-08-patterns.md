---
phase: 8
title: Pattern Evaluation
status: in-progress
tier: 3
goal: All patterns evaluate correctly
spec:
  - spec/10-patterns.md
sections:
  - id: "8.1"
    title: run (Sequential Execution)
    status: complete
  - id: "8.2"
    title: try (Error Propagation)
    status: complete
  - id: "8.3"
    title: recurse (Recursive Functions)
    status: in-progress
  - id: "8.4"
    title: parallel (All-Settled Concurrent Execution)
    status: complete
  - id: "8.5"
    title: spawn (Fire and Forget)
    status: complete
  - id: "8.6"
    title: timeout (Time-Bounded)
    status: complete
  - id: "8.7"
    title: cache (Memoization with TTL)
    status: in-progress
  - id: "8.8"
    title: with (Resource Management)
    status: in-progress
  - id: "8.9"
    title: for (Iteration with Early Exit)
    status: complete
  - id: "8.10"
    title: Data Transformation — MOVED TO STDLIB
    status: complete
  - id: "8.11"
    title: Resilience Patterns — MOVED TO STDLIB
    status: complete
  - id: "8.12"
    title: Phase Completion Checklist
    status: complete
---

# Phase 8: Pattern Evaluation

**Goal**: All patterns evaluate correctly

> **SPEC**: `spec/10-patterns.md`
> **DESIGN**: `design/02-syntax/04-patterns-reference.md`

---

## Pattern Categories (per spec/10-patterns.md)

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

> **Future Enhancement**: Approved proposal `proposals/approved/checks-proposal.md` adds `.pre_check:` and `.post_check:` properties to `run`. See Phase 15.5.

- [x] **Implement**: Grammar `run_expr = "run" "(" { binding "," } expression ")"` — spec/10-patterns.md § run
  - [x] **Rust Tests**: `oric/src/patterns/run.rs` — run pattern execution tests
  - [x] **Ori Tests**: `tests/spec/patterns/run.ori` — 7 tests pass

- [x] **Implement**: Binding `let [ "mut" ] identifier [ ":" type ] "=" expression` — spec/10-patterns.md § run
- [x] **Implement**: Evaluate each binding in order — spec/10-patterns.md § run
- [x] **Implement**: Each binding introduces variable into scope — spec/10-patterns.md § run
- [x] **Implement**: Final expression is the result — spec/10-patterns.md § run

---

## 8.2 try (Error Propagation)

- [x] **Implement**: Grammar `try_expr = "try" "(" { binding "," } expression ")"` — spec/10-patterns.md § try
  - [x] **Rust Tests**: `oric/src/patterns/try.rs` — try pattern execution tests
  - [x] **Ori Tests**: `tests/spec/patterns/try.ori` — 5 tests pass

- [x] **Implement**: Binding with `Result<T, E>` gives variable type `T` — spec/10-patterns.md § try
- [x] **Implement**: If `Err(e)`, return immediately — spec/10-patterns.md § try
- [x] **Implement**: Final expression is result — spec/10-patterns.md § try

---

## 8.3 recurse (Recursive Functions)

**Proposal**: `proposals/approved/recurse-pattern-proposal.md`

### Basic Implementation (complete)

- [x] **Implement**: `.condition:` property type `bool` — spec/10-patterns.md § recurse
  - [x] **Rust Tests**: `oric/src/patterns/recurse.rs` — recurse pattern execution tests
  - [x] **Ori Tests**: `tests/spec/patterns/recurse.ori` — 5 tests pass

- [x] **Implement**: `.base:` property type `T` — spec/10-patterns.md § recurse
- [x] **Implement**: `.step:` property uses `self()` — spec/10-patterns.md § recurse
- [x] **Implement**: Optional `.memo:` default false — spec/10-patterns.md § recurse
- [x] **Implement**: Optional `.parallel:` threshold — spec/10-patterns.md § recurse (stub: executes sequentially)
- [x] **Implement**: When `.condition` true, return `.base` — spec/10-patterns.md § recurse
- [x] **Implement**: Otherwise evaluate `.step` — spec/10-patterns.md § recurse
- [x] **Implement**: `self(...)` refers to recursive function — spec/10-patterns.md § recurse
- [x] **Implement**: Memoization caches during top-level call — spec/10-patterns.md § recurse

### Self Scoping (from approved proposal)

- [ ] **Implement**: `self(...)` inside `step` is recursive call — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/patterns/recurse.rs` — self keyword tests
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_self.ori`

- [ ] **Implement**: `self` (receiver) coexists with `self(...)` in trait methods — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/patterns/recurse.rs` — self scoping in traits
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_trait_self.ori`

- [ ] **Implement**: Error E1001 — `self(...)` outside `step` is compile error — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/recurse.rs` — self location error
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_errors.ori`

- [ ] **Implement**: Error E1002 — `self(...)` arity mismatch — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/recurse.rs` — arity error
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_errors.ori`

### Memoization (from approved proposal)

- [ ] **Implement**: Memo key constraint `Hashable + Eq` — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/recurse.rs` — memo key constraint
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_memo.ori`
  - [ ] **LLVM Support**: LLVM codegen for memo key hashing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — memo codegen

- [ ] **Implement**: Return type constraint `Clone` with memo — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/recurse.rs` — memo return constraint
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_memo.ori`

- [ ] **Implement**: Error E1000 — non-Hashable params with memo — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/recurse.rs` — hashable error
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_errors.ori`

### Parallel Recursion (from approved proposal)

- [ ] **Implement**: `parallel: true` requires `uses Suspend` — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/capabilities.rs` — suspend capability
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_parallel.ori`
  - [ ] **LLVM Support**: LLVM codegen for parallel recursion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — parallel codegen

- [ ] **Implement**: Captured values must be `Sendable` with parallel — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/recurse.rs` — sendable captures
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_parallel.ori`

- [ ] **Implement**: Return type must be `Sendable` with parallel — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/recurse.rs` — sendable return
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_parallel.ori`

- [ ] **Implement**: Error E1003 — parallel without Suspend capability — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/recurse.rs` — capability error
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_errors.ori`

### Parallel + Memo Thread Safety (from approved proposal)

- [ ] **Implement**: Thread-safe memo cache for parallel recursion — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/patterns/recurse.rs` — thread-safe memo
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_parallel_memo.ori`

- [ ] **Implement**: Concurrent memo access — one computes, others wait — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/patterns/recurse.rs` — memo stampede
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_parallel_memo.ori`

### Tail Call Optimization (from approved proposal)

- [ ] **Implement**: TCO when `self(...)` is in tail position — recurse-pattern-proposal.md
  - [ ] Compile to loop, O(1) stack space
  - [ ] **Rust Tests**: `oric/src/codegen/tco.rs` — tail call optimization
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_tco.ori`
  - [ ] **LLVM Support**: LLVM codegen for TCO
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — TCO codegen

### Stack Limits (from approved proposal)

- [ ] **Implement**: Recursion depth limit of 1000 — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/patterns/recurse.rs` — depth limit tests
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_depth.ori`

- [ ] **Implement**: Panic on depth exceeded — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/patterns/recurse.rs` — depth panic
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_depth.ori`

- [ ] **Implement**: TCO-compiled recursion bypasses depth limit — recurse-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/patterns/recurse.rs` — TCO depth bypass
  - [ ] **Ori Tests**: `tests/spec/patterns/recurse_tco.ori`

---

## 8.4 parallel (All-Settled Concurrent Execution)

> **REDESIGNED**: parallel now uses list-only form and all-settled semantics.
> All tasks run to completion. Errors captured as Err values in result list.
> Pattern itself never fails.

- [x] **Implement**: `.tasks:` property (required) — spec/10-patterns.md § parallel
  - [x] **Rust Tests**: `oric/src/patterns/parallel.rs` — parallel pattern execution tests
  - [x] **Ori Tests**: `tests/spec/patterns/concurrency.ori` — 6 tests pass

- [x] **Implement**: Returns `[Result<T, E>]` — spec/10-patterns.md § parallel
- [x] **Implement**: Optional `.timeout:` (per-task) — spec/10-patterns.md § parallel
- [x] **Implement**: Optional `.max_concurrent:` — spec/10-patterns.md § parallel
- [x] **Implement**: Stub — Execute sequentially, wrap each result in Ok/Err

---

## 8.5 spawn (Fire and Forget)

> **NEW**: spawn executes tasks without waiting. Returns void immediately. Errors discarded.

- [x] **Implement**: `.tasks:` property (required) — spec/10-patterns.md § spawn
  - [x] **Rust Tests**: `oric/src/patterns/spawn.rs` — spawn pattern execution tests
  - [x] **Ori Tests**: `tests/spec/patterns/concurrency.ori` — 3 tests pass

- [x] **Implement**: Returns `void` — spec/10-patterns.md § spawn
- [x] **Implement**: Optional `.max_concurrent:` — spec/10-patterns.md § spawn

---

## 8.6 timeout (Time-Bounded)

> **NOTE**: Stub implementation - timeout not enforced in interpreter, always returns Ok(result).

- [x] **Implement**: `.operation:` property — spec/10-patterns.md § timeout
  - [x] **Rust Tests**: `oric/src/patterns/timeout.rs` — timeout pattern execution tests
  - [x] **Ori Tests**: `tests/spec/patterns/concurrency.ori` — 4 tests pass

- [x] **Implement**: `.after:` property — spec/10-patterns.md § timeout
- [x] **Implement**: Return `Result<T, TimeoutError>` — spec/10-patterns.md § timeout
- [x] **Implement**: Stub — Execute `.operation`, wrap in `Ok()`

---

## 8.7 cache (Memoization with TTL)

**Proposal**: `proposals/approved/cache-pattern-proposal.md`

> **SPEC**: `cache(key: url, op: fetch(url), ttl: 5m)` — Requires `Cache` capability

### Basic Semantics (complete)

- [x] **Implement**: `.key:` property — spec/10-patterns.md § cache
  - [x] **Rust Tests**: `oric/src/patterns/cache.rs` — cache pattern execution tests
  - [x] **Ori Tests**: `tests/spec/patterns/concurrency.ori` — 2 tests pass

- [x] **Implement**: `.op:` property — spec/10-patterns.md § cache
- [x] **Implement**: Stub — Execute `.op` without caching

### Key Requirements (from approved proposal)

- [ ] **Implement**: Key type constraint `Hashable + Eq` — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/cache.rs` — key constraint tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_keys.ori`
  - [ ] **LLVM Support**: LLVM codegen for key hashing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — cache key codegen

### Value Requirements (from approved proposal)

- [ ] **Implement**: Value type constraint `Clone` — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/cache.rs` — value constraint tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_values.ori`

### TTL Semantics (from approved proposal)

- [ ] **Implement**: `.ttl:` with Duration — spec/10-patterns.md § cache
  - [ ] **Rust Tests**: `oric/src/patterns/cache.rs` — TTL tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_ttl.ori`
  - [ ] **LLVM Support**: LLVM codegen for cache TTL
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — cache TTL codegen

- [ ] **Implement**: TTL = 0 means no caching (always recompute) — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/patterns/cache.rs` — zero TTL tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_ttl.ori`

- [ ] **Implement**: Negative TTL is compile error — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/cache.rs` — negative TTL error
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_ttl.ori`

### Capability Requirement (from approved proposal)

- [ ] **Implement**: Requires `Cache` capability — spec/10-patterns.md § cache
  - [ ] **Rust Tests**: `oric/src/typeck/checker/capabilities.rs` — cache capability tests
  - [ ] **Ori Tests**: `tests/spec/capabilities/cache.ori`
  - [ ] **LLVM Support**: LLVM codegen for cache capability
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — cache capability codegen

### Concurrent Access (from approved proposal)

- [ ] **Implement**: Stampede prevention — cache-pattern-proposal.md
  - [ ] First request computes, others wait
  - [ ] All receive same result
  - [ ] **Rust Tests**: `oric/src/patterns/cache.rs` — stampede tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_concurrent.ori`

- [ ] **Implement**: Error during stampede propagates to waiting requests — cache-pattern-proposal.md
  - [ ] Entry NOT cached on error
  - [ ] **Rust Tests**: `oric/src/patterns/cache.rs` — stampede error tests

### Error Handling (from approved proposal)

- [ ] **Implement**: `Err` and panic results NOT cached — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/patterns/cache.rs` — error caching tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_errors.ori`

### Invalidation (from approved proposal)

- [ ] **Implement**: `Cache.invalidate(key:)` method — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/patterns/cache.rs` — invalidation tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_invalidation.ori`

- [ ] **Implement**: `Cache.clear()` method — cache-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/patterns/cache.rs` — clear tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_invalidation.ori`

### Error Messages (from approved proposal)

- [ ] **Implement**: E0990 — cache key must be `Hashable` — cache-pattern-proposal.md
- [ ] **Implement**: E0991 — `cache` requires `Cache` capability — cache-pattern-proposal.md
- [ ] **Implement**: E0992 — TTL must be non-negative — cache-pattern-proposal.md

---

## 8.8 with (Resource Management)

**Proposal**: `proposals/approved/with-pattern-proposal.md`

> **NOTE**: Uses `.action:` instead of spec's `.use:` (`use` is reserved keyword).

### Basic Implementation (complete)

- [x] **Implement**: Parse `with` pattern in parser
  - [x] **Rust Tests**: `oric/src/patterns/with.rs` — with pattern execution tests
  - [x] **Ori Tests**: `tests/spec/patterns/with.ori` — 4 tests pass

- [x] **Implement**: `.acquire:` property — spec/10-patterns.md § with
- [x] **Implement**: `.action:` property (spec uses `.use:`) — spec/10-patterns.md § with
- [x] **Implement**: `.release:` property — spec/10-patterns.md § with
- [x] **Implement**: Acquire resource, call `.action`, always call `.release` even on error

### Release Guarantee (from approved proposal)

- [ ] **Implement**: Release runs if acquire succeeds — with-pattern-proposal.md
  - Release runs on normal completion of use
  - Release runs on panic during use
  - Release runs on error propagation (`?`) in use
  - Release runs on `break`/`continue` in use
  - [ ] **Rust Tests**: `oric/src/patterns/with.rs` — release guarantee tests
  - [ ] **Ori Tests**: `tests/spec/patterns/with_guarantee.ori`
  - [ ] **LLVM Support**: LLVM codegen for unwinding with release
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — with unwinding codegen

### Type Constraints (from approved proposal)

- [ ] **Implement**: `release` must return `void` — with-pattern-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/checker/with.rs` — release type constraint
  - [ ] **Ori Tests**: `tests/spec/patterns/with_types.ori`

### Double Fault Abort (from approved proposal)

- [ ] **Implement**: If release panics during unwind, abort immediately — with-pattern-proposal.md
  - `@panic` handler NOT called
  - Both panic messages shown
  - [ ] **Rust Tests**: `oric/src/patterns/with.rs` — double fault tests
  - [ ] **Ori Tests**: `tests/spec/patterns/with_double_fault.ori`

### Error Messages (from approved proposal)

- [ ] **Implement**: E0860 — `with` pattern missing required parameter — with-pattern-proposal.md
- [ ] **Implement**: E0861 — `release` must return `void` — with-pattern-proposal.md

---

## 8.9 for (Iteration with Early Exit) — function_exp Pattern

> **STATUS**: IMPLEMENTED. Uses FunctionSeq::ForPattern with match arm syntax.
>
> **NOTE**: This is the `for(over:, match:, default:)` **pattern** with named arguments.
> The `for x in items do/yield expr` **expression** syntax is a separate construct in Phase 10 (Control Flow).

- [x] **Implement**: `.over:` property — spec/10-patterns.md § for
  - [x] **Rust Tests**: `oric/src/patterns/for.rs` — for pattern execution tests
  - [x] **Ori Tests**: `tests/spec/patterns/for.ori` — 8 tests pass

- [x] **Implement**: Optional `.map:` property — spec/10-patterns.md § for
- [x] **Implement**: `.match:` property — spec/10-patterns.md § for
- [x] **Implement**: `.default:` property — spec/10-patterns.md § for
- [x] **Implement**: Return first match or `.default` — spec/10-patterns.md § for

---

## 8.10 Data Transformation — MOVED TO STDLIB

> **MOVED**: Per "Lean Core, Rich Libraries", these are now stdlib functions (Phase 7 - Stdlib).

| Pattern | Stdlib Location | Notes |
|---------|-----------------|-------|
| `map` | `std.iter` | `items.map(transform: fn)` |
| `filter` | `std.iter` | `items.filter(predicate: fn)` |
| `fold` | `std.iter` | `items.fold(initial: val, operation: fn)` |
| `find` | `std.iter` | `items.find(where: fn)` |
| `collect` | `std.iter` | `range.collect(transform: fn)` |

---

## 8.11 Resilience Patterns — MOVED TO STDLIB

> **MOVED**: Per "Lean Core, Rich Libraries", these are now stdlib functions (Phase 7 - Stdlib).

| Pattern | Stdlib Location | Notes |
|---------|-----------------|-------|
| `retry` | `std.resilience` | `retry(operation: fn, attempts: n, backoff: strategy)` |
| `exponential` | `std.resilience` | `exponential(base: 100ms)` backoff strategy |
| `linear` | `std.resilience` | `linear(delay: 100ms)` backoff strategy |

---

## 8.12 Phase Completion Checklist

- [x] All compiler patterns implemented
- [x] Data transformation patterns moved to stdlib
- [x] Resilience patterns moved to stdlib
- [x] Run full test suite: `./test-all`

**Exit Criteria**: All compiler patterns evaluate correctly
