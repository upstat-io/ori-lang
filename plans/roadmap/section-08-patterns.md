---
section: 8
title: Pattern Evaluation
status: in-progress
reviewed: true
last_verified: "2026-03-29"
tier: 3
goal: All patterns evaluate correctly
spec:
  - spec/15-patterns.md
verification_notes: |
  Verified 2026-03-29. 9 items verified, 3 stale checkboxes fixed, 3 weak-test
  annotations added. Added missing catch (8.10, done) and nursery (8.11,
  not-started) sections. 5 commented-out test files provide zero CI protection
  (try.ori, with.ori, for.ori, concurrency.ori, parallel.ori actual tests).
  Hygiene leak: eval_can_function_exp bypasses PatternRegistry for most patterns
  — inline dispatch could drift from ori_patterns PatternDefinition impls.
  No compile_fail tests for any pattern error code.
sections:
  - id: "8.1"
    title: run (Sequential Execution)
    status: in-progress
    verified: "2026-03-29"
  - id: "8.2"
    title: try (Error Propagation)
    status: not-started
  - id: "8.3"
    title: recurse (Recursive Functions)
    status: in-progress
    verified: "2026-03-29"
  - id: "8.4"
    title: parallel (All-Settled Concurrent Execution)
    status: in-progress
    verified: "2026-03-29"
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
    title: catch (Error Capture)
    status: done
    verified: "2026-03-29"
  - id: "8.11"
    title: nursery (Structured Concurrency)
    status: not-started
  - id: "8.12"
    title: CancellationError Type
    status: not-started
  - id: "8.13"
    title: Data Transformation — MOVED TO STDLIB
    status: done
    verified: "2026-03-29"
  - id: "8.14"
    title: Resilience Patterns — MOVED TO STDLIB
    status: done
    verified: "2026-03-29"
  - id: "8.15"
    title: Hygiene Notes
    status: not-started
  - id: "8.16"
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
- `parallel`, `spawn`, `timeout`, `nursery`, `cache` — Concurrency/resilience
- `with` — Resource management
- `catch` — Error capture

> **NOTE**: `map`, `filter`, `fold`, `find`, `collect`, `retry` are now stdlib functions.

---

## 8.1 run (Sequential Execution) [function_seq]

> **Future Enhancement**: Approved proposal `proposals/approved/checks-proposal.md` adds function-level `pre()` and `post()` contract declarations. See Section 15.5.
>
> **NOTE**: The roadmap refers to "run" pattern but the implementation uses block expressions (`{ }` syntax). The `run()` function syntax was removed. Implementation is correct — block expressions are the canonical form.

- [x] **Implement**: Grammar `run_expr = "run" "(" { binding "," } expression ")"` — spec/15-patterns.md § run [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator pattern execution — run pattern tests (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/run.ori` — 12 tests pass (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for run pattern — NEEDS TESTS (low priority: block expressions fundamental to all AOT codegen, broadly tested)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — run pattern codegen
  - [ ] **AOT Tests**: No dedicated AOT coverage yet — blocks broadly tested in AOT

- [x] **Implement**: Binding `let [ "mut" ] identifier [ ":" type ] "=" expression` — spec/15-patterns.md § run [done] (2026-02-10) (verified 2026-03-29)
- [x] **Implement**: Evaluate each binding in order — spec/15-patterns.md § run [done] (2026-02-10) (verified 2026-03-29)
- [x] **Implement**: Each binding introduces variable into scope — spec/15-patterns.md § run [done] (2026-02-10) (verified 2026-03-29)
- [x] **Implement**: Final expression is the result — spec/15-patterns.md § run [done] (2026-02-10) (verified 2026-03-29)
- [ ] **Test Gap**: Missing `#compile_fail` negative tests for run pattern — NEEDS TESTS
- [ ] **Test Gap**: Type coverage narrow (only int, void tested) — missing str, float, bool, struct, Option, Result, list, closures — NEEDS TESTS

- [ ] **Subsection close-out (8.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.2 try (Error Propagation)

> **Commented-out tests**: `tests/spec/patterns/try.ori` is entirely commented out (208 lines, zero active tests). Blocked on type checker features (`try` pattern, `?` operator, Result polymorphism).

- [ ] **Implement**: Grammar `try_expr = "try" "(" { binding "," } expression ")"` — spec/15-patterns.md § try
  - [ ] **Rust Tests**: `ori_patterns/src/try.rs` — try pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/try.ori` — 5 tests pass
  - [ ] **LLVM Support**: LLVM codegen for try pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — try pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Binding with `Result<T, E>` gives variable type `T` — spec/15-patterns.md § try
- [ ] **Implement**: If `Err(e)`, return immediately — spec/15-patterns.md § try
- [ ] **Implement**: Final expression is result — spec/15-patterns.md § try

- [ ] **Subsection close-out (8.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.3 recurse (Recursive Functions)

**Proposal**: `proposals/approved/recurse-pattern-proposal.md`

### Basic Implementation (complete)

- [x] **Implement**: `.condition:` property type `bool` — spec/15-patterns.md § recurse [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator pattern execution — 6 Rust tests pass (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/recurse.ori` — 18 tests pass (verified 2026-03-29)
  - [x] **AOT Tests**: 2 dedicated recurse pattern AOT tests (`test_tail_rec_recurse_pattern`, `test_tail_rec_recurse_deep` at 200K depth) (verified 2026-03-29)

- [x] **Implement**: `.base:` property type `T` — spec/15-patterns.md § recurse [done] (2026-02-10) (verified 2026-03-29)
- [x] **Implement**: `.step:` property uses `self()` — spec/15-patterns.md § recurse [done] (2026-02-10) (verified 2026-03-29)
- [x] **Implement**: Optional `.memo:` default false — spec/15-patterns.md § recurse [done] (2026-02-10) (verified 2026-03-29) — STALE CHECKBOX fixed: memo IS implemented and tested (`fib_memo`, `fib_large`, `ackermann` tests pass with `memo: true`)
- [ ] **Implement**: Optional `.parallel:` threshold — spec/15-patterns.md § recurse (stub: executes sequentially, `fib_parallel` test runs sequentially)
- [x] **Implement**: When `.condition` true, return `.base` — spec/15-patterns.md § recurse [done] (2026-02-10) (verified 2026-03-29)
- [x] **Implement**: Otherwise evaluate `.step` — spec/15-patterns.md § recurse [done] (2026-02-10) (verified 2026-03-29)
- [x] **Implement**: `self(...)` refers to recursive function — spec/15-patterns.md § recurse [done] (2026-02-10) (verified 2026-03-29)
- [x] **Implement**: Memoization caches during top-level call — spec/15-patterns.md § recurse [done] (2026-02-10) (verified 2026-03-29) — STALE CHECKBOX fixed: memoization IS working, `MemoizedFunctionValue` wraps self when `memo: true`

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

- [ ] **Subsection close-out (8.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.4 parallel (All-Settled Concurrent Execution)

> **REDESIGNED**: parallel now uses list-only form and all-settled semantics.
> All tasks run to completion. Errors captured as Err values in result list.
> Pattern itself never fails.
>
> **STUB**: Evaluator has a loud stub in `can_eval.rs:eval_can_function_exp` (sequential execution + `tracing::warn!`). When implementing for real, replace the stub there.

- [x] **Implement**: `.tasks:` property (required) — spec/15-patterns.md § parallel [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator pattern execution — parallel pattern tests (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/parallel.ori` — 5 tests pass (verified 2026-03-29) — WEAK TESTS: all 5 active tests use `for...yield` comprehensions, NOT `parallel()`. Actual `parallel(tasks: [...])` calls are commented out. Tests do not exercise the parallel pattern.
  - [ ] **LLVM Support**: LLVM codegen for parallel pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — parallel pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Returns `[Result<T, E>]` — spec/15-patterns.md § parallel
- [ ] **Implement**: Optional `.timeout:` (per-task) — spec/15-patterns.md § parallel
- [ ] **Implement**: Optional `.max_concurrent:` — spec/15-patterns.md § parallel
- [x] **Implement**: Stub — Execute sequentially, wrap each result in Ok/Err [done] (2026-02-10) (verified 2026-03-29) — STALE CHECKBOX fixed: stub IS implemented in evaluator (`eval_can_function_exp`, sequential execution + `tracing::warn!`)

- [ ] **Subsection close-out (8.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.5 spawn (Fire and Forget)

> **NEW**: spawn executes tasks without waiting. Returns void immediately. Errors discarded.
>
> **STUB**: Evaluator has a loud stub in `can_eval.rs:eval_can_function_exp` (synchronous execution + `tracing::warn!`). When implementing for real, replace the stub there.
>
> **Commented-out tests**: `tests/spec/patterns/concurrency.ori` spawn section entirely commented out. 5 Rust unit tests for pattern struct exist.

- [ ] **Implement**: `.tasks:` property (required) — spec/15-patterns.md § spawn
  - [ ] **Rust Tests**: `ori_patterns/src/spawn.rs` — spawn pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/concurrency.ori` — all spawn tests commented out
  - [ ] **LLVM Support**: LLVM codegen for spawn pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — spawn pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Returns `void` — spec/15-patterns.md § spawn
- [ ] **Implement**: Optional `.max_concurrent:` — spec/15-patterns.md § spawn

- [ ] **Subsection close-out (8.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.6 timeout (Time-Bounded)

> **NOTE**: Stub implementation — timeout not enforced in evaluator, always returns Ok(result).
>
> **STUB**: Evaluator has a loud stub in `can_eval.rs:eval_can_function_exp` (no timeout enforcement + `tracing::warn!`). When implementing for real, replace the stub there.
>
> **Commented-out tests**: `tests/spec/patterns/concurrency.ori` timeout section entirely commented out. 4 Rust unit tests for pattern struct exist.

- [ ] **Implement**: `.operation:` property — spec/15-patterns.md § timeout
  - [ ] **Rust Tests**: `ori_patterns/src/timeout.rs` — timeout pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/concurrency.ori` — all timeout tests commented out
  - [ ] **LLVM Support**: LLVM codegen for timeout pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — timeout pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `.after:` property — spec/15-patterns.md § timeout
- [ ] **Implement**: Return `Result<T, TimeoutError>` — spec/15-patterns.md § timeout
- [ ] **Implement**: Stub — Execute `.operation`, wrap in `Ok()`

- [ ] **Subsection close-out (8.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.7 cache (Memoization with TTL)

**Proposal**: `proposals/approved/cache-pattern-proposal.md`

> **SPEC**: `cache(key: url, op: fetch(url), ttl: 5m)` — Requires `Cache` capability
>
> **STUB**: Evaluator has a loud stub in `can_eval.rs:eval_can_function_exp` (calls op directly, no memoization + `tracing::warn!`). When implementing for real, replace the stub there.

### Basic Semantics (complete)

- [ ] **Implement**: `.key:` property — spec/15-patterns.md § cache
  - [ ] **Rust Tests**: `ori_patterns/src/cache.rs` — cache pattern execution tests (4 Rust unit tests for pattern struct exist)
  - [ ] **Ori Tests**: `tests/spec/patterns/concurrency.ori` — all cache tests commented out

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

- [ ] **Subsection close-out (8.7)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.7 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.7: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.8 with (Resource Management)

**Proposal**: `proposals/approved/with-pattern-proposal.md`

> **NOTE**: Uses `.action:` instead of spec's `.use:` (`use` is reserved keyword).
>
> **STUB**: Evaluator stub actually implements basic RAII semantics (acquire/action/release with release guarantee on error). More functional than "not-started" implies. Blocked on type checker lambda inference for Ori test cases.
>
> **Commented-out tests**: `tests/spec/patterns/with.ori` is entirely commented out (99 lines, zero active tests). 4 Rust unit tests for pattern struct exist.

### Basic Implementation (complete)

- [ ] **Implement**: Parse `with` pattern in parser
  - [ ] **Rust Tests**: `ori_patterns/src/with.rs` — with pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/with.ori` — all tests commented out (blocked on type checker lambda inference)

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

- [ ] **Subsection close-out (8.8)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.8 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.8: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.9 for (Iteration with Early Exit) — function_exp Pattern

> **STATUS**: IMPLEMENTED. Uses FunctionSeq::ForPattern with match arm syntax.
>
> **NOTE**: This is the `for(over:, match:, default:)` **pattern** with named arguments.
> The `for x in items do/yield expr` **expression** syntax is a separate construct in Section 10 (Control Flow).
>
> **Commented-out tests**: `tests/spec/patterns/for.ori` is entirely commented out (462 lines, zero active tests). `FunctionSeq::ForPattern` exists in IR but pattern form is untested.

- [ ] **Implement**: `.over:` property — spec/15-patterns.md § for
  - [ ] **Rust Tests**: `ori_patterns/src/for.rs` — for pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/for.ori` — all tests commented out
  - [ ] **LLVM Support**: LLVM codegen for for pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — for pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Optional `.map:` property — spec/15-patterns.md § for
- [ ] **Implement**: `.match:` property — spec/15-patterns.md § for
- [ ] **Implement**: `.default:` property — spec/15-patterns.md § for
- [ ] **Implement**: Return first match or `.default` — spec/15-patterns.md § for

- [ ] **Subsection close-out (8.9)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.9 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.9: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.10 catch (Error Capture)

> **IMPLEMENTED**: `catch(expr:)` captures panics and returns `Result<T, str>`. Has `FunctionExpKind::Catch`, `CatchPattern` in `ori_patterns`, and dedicated `eval_can_catch()` method.
>
> **Spec**: spec/15-patterns.md section 15.6.1

- [x] **Implement**: `catch(expr:)` pattern — spec/15-patterns.md § catch [done] (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/patterns/catch.ori` — 7 tests pass: success, panic, message, div-by-zero, value, string, nested (verified 2026-03-29)
  - [x] **Rust Tests**: `CatchPattern` in `ori_patterns` + `eval_can_catch()` in evaluator (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for catch pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/pattern_tests.rs` — catch pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet
- [ ] **Test Gap**: Missing `#compile_fail` negative tests for catch pattern

---

## 8.11 nursery (Structured Concurrency)

> **MISSING FROM ROADMAP**: Spec 15.4.4 defines nursery as a core structured concurrency pattern with guaranteed task completion. No `FunctionExpKind::Nursery` variant exists. No pattern struct. No evaluator stub.
>
> **Spec**: `nursery(body:, on_error:, timeout:)` — `Nursery` type, `n.spawn()` API, error modes (`CancelRemaining`/`CollectAll`/`FailFast`)

- [ ] **Implement**: `nursery(body:, on_error:, timeout:)` pattern — spec/15-patterns.md § nursery
  - [ ] Add `FunctionExpKind::Nursery` variant
  - [ ] Add `NurseryPattern` in `ori_patterns`
  - [ ] Add evaluator stub in `eval_can_function_exp`
  - [ ] **Rust Tests**: `ori_patterns/src/nursery.rs` — nursery pattern execution tests
  - [ ] **Ori Tests**: `tests/spec/patterns/nursery.ori`
  - [ ] **LLVM Support**: LLVM codegen for nursery pattern
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Nursery` type with `n.spawn()` API — spec/15-patterns.md § nursery
- [ ] **Implement**: `NurseryErrorMode` (`CancelRemaining | CollectAll | FailFast`) — spec/15-patterns.md § nursery
- [ ] **Implement**: Guaranteed task completion semantics — spec/15-patterns.md § nursery
- [ ] **Implement**: Requires `uses Suspend` capability — spec/15-patterns.md § nursery

- [ ] **Subsection close-out (8.11)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.11 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.11: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.12 CancellationError Type

> **MISSING FROM ROADMAP**: Required by nursery, parallel, and timeout patterns. `CancellationError` and `CancellationReason` are prelude types per spec.

- [ ] **Implement**: `CancellationError` type — prelude type for structured concurrency
- [ ] **Implement**: `CancellationReason` enum — prelude type for cancellation semantics
- [ ] **Implement**: `is_cancelled()` built-in — prelude function
- [ ] **Ori Tests**: `tests/spec/patterns/cancellation.ori`

- [ ] **Subsection close-out (8.12)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.12 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.12: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.13 Data Transformation — MOVED TO STDLIB

> **MOVED**: Per "Lean Core, Rich Libraries", these are now stdlib functions (Section 7 - Stdlib). (verified 2026-03-29)

| Pattern | Stdlib Location | Notes |
|---------|-----------------|-------|
| `map` | `std.iter` | `items.map(transform: fn)` |
| `filter` | `std.iter` | `items.filter(predicate: fn)` |
| `fold` | `std.iter` | `items.fold(initial: val, operation: fn)` |
| `find` | `std.iter` | `items.find(where: fn)` |
| `collect` | `std.iter` | `range.collect(transform: fn)` |

---

## 8.14 Resilience Patterns — MOVED TO STDLIB

> **MOVED**: Per "Lean Core, Rich Libraries", these are now stdlib functions (Section 7 - Stdlib). (verified 2026-03-29)

| Pattern | Stdlib Location | Notes |
|---------|-----------------|-------|
| `retry` | `std.resilience` | `retry(operation: fn, attempts: n, backoff: strategy)` |
| `exponential` | `std.resilience` | `exponential(base: 100ms)` backoff strategy |
| `linear` | `std.resilience` | `linear(delay: 100ms)` backoff strategy |

---

## 8.15 Hygiene Notes

> **LEAK: scattered-knowledge** (Minor): The canonical evaluator `eval_can_function_exp` bypasses the `PatternRegistry` for most patterns -- it pre-evaluates props and dispatches inline. Only `Recurse` and `Catch` use dedicated lazy-evaluation methods. This means `ori_patterns` crate's `PatternDefinition::evaluate()` implementations are largely dead code for canonical IR evaluation. The two code paths (pattern registry vs. inline evaluator) could drift.

> **Commented-out test files** (5 files, zero CI protection): `try.ori` (208 lines), `with.ori` (99 lines), `for.ori` (462 lines), `concurrency.ori` (parallel/spawn/timeout/cache sections), `parallel.ori` (actual `parallel()` calls). All are blocked on type checker features.

> **No compile_fail tests** for any pattern-specific error code: E0860, E0861, E0990-E0992, E1000-E1003.

- [ ] **Subsection close-out (8.15)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.15 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.15: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 8.16 Section Completion Checklist

- [ ] All compiler patterns implemented
- [ ] Data transformation patterns moved to stdlib
- [ ] Resilience patterns moved to stdlib
- [ ] catch pattern tracked (added 2026-03-29)
- [ ] nursery pattern implemented (added 2026-03-29)
- [ ] CancellationError type implemented (added 2026-03-29)
- [ ] Hygiene leak resolved: unify PatternRegistry and evaluator dispatch
- [ ] All 5 commented-out test files activated
- [ ] compile_fail tests for all pattern error codes
- [ ] Run full test suite: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: All compiler patterns evaluate correctly

- [ ] **Subsection close-out (8.16)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-8.16 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 8.16: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

