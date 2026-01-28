# Phase 8: Pattern Evaluation

**Goal**: All patterns evaluate correctly

> **SPEC**: `spec/10-patterns.md`
> **DESIGN**: `design/02-syntax/04-patterns-reference.md`

---

## Phase Status: 🔶 ~95% Complete (cache TTL and capability blocked on Phase 6)

**Design Change**: Per "Lean Core, Rich Libraries" principle, data transformation
patterns (`map`, `filter`, `fold`, `find`, `collect`) and resilience patterns
(`retry`, `backoff`) have been moved to stdlib. Only patterns requiring special
syntax or static analysis remain in the compiler.

**Compiler patterns** (8 total):
- Control flow: `run`, `try`, `match`, `for`
- Recursion: `recurse`
- Concurrency: `parallel`, `spawn`, `timeout`
- Resource management: `cache`, `with`

**Moved to stdlib (Phase 7 - Stdlib)**:
- Data transformation: `map`, `filter`, `fold`, `find`, `collect`
- Resilience: `retry`, `exponential`, `linear`
- Validation: `validate`

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

> **SPEC**: `cache(key: url, operation: fetch(url), ttl: 5m)` — Requires `Cache` capability

- [x] **Implement**: `.key:` property — spec/10-patterns.md § cache
  - [x] **Rust Tests**: `oric/src/patterns/cache.rs` — cache pattern execution tests
  - [x] **Ori Tests**: `tests/spec/patterns/concurrency.ori` — 2 tests pass

- [x] **Implement**: `.operation:` property — spec/10-patterns.md § cache
- [ ] **Implement**: Optional `.ttl:` with Duration — spec/10-patterns.md § cache
  - [ ] **Rust Tests**: `oric/src/patterns/cache.rs` — TTL tests
  - [ ] **Ori Tests**: `tests/spec/patterns/cache_ttl.ori`
  - [ ] **LLVM Support**: LLVM codegen for cache TTL
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — cache TTL codegen
- [ ] **Implement**: Requires `Cache` capability — spec/10-patterns.md § cache — **Blocked on Phase 6 (Capabilities)**
  - [ ] **Rust Tests**: `oric/src/typeck/checker/capabilities.rs` — cache capability tests
  - [ ] **Ori Tests**: `tests/spec/capabilities/cache.ori`
  - [ ] **LLVM Support**: LLVM codegen for cache capability
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — cache capability codegen
- [x] **Implement**: Stub — Execute `.operation` without caching

---

## 8.8 with (Resource Management)

> **NOTE**: Uses `.action:` instead of spec's `.use:` (`use` is reserved keyword).

- [x] **Implement**: Parse `with` pattern in parser
  - [x] **Rust Tests**: `oric/src/patterns/with.rs` — with pattern execution tests
  - [x] **Ori Tests**: `tests/spec/patterns/with.ori` — 4 tests pass

- [x] **Implement**: `.acquire:` property — spec/10-patterns.md § with
- [x] **Implement**: `.action:` property (spec uses `.use:`) — spec/10-patterns.md § with
- [x] **Implement**: `.release:` property — spec/10-patterns.md § with
- [x] **Implement**: Acquire resource, call `.action`, always call `.release` even on error

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
