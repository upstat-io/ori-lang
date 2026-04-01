---
section: 16
title: Async Support
status: not-started
reviewed: true
last_verified: "2026-03-29"
tier: 6
goal: Suspend-based concurrency via capabilities
spec:
  - spec/20-capabilities.md
sections:
  - id: "16.1"
    title: Suspend Capability
    status: not-started
  - id: "16.2"
    title: Structured Concurrency
    status: not-started
  - id: "16.3"
    title: Concurrency Patterns
    status: not-started
  - id: "16.4"
    title: Async Error Traces
    status: not-started
  - id: "16.5"
    title: Section Completion Checklist
    status: not-started
---

# Section 16: Async Support

**Goal**: Suspend-based concurrency via capabilities (verified 2026-03-29)

> **SPEC**: `spec/20-capabilities.md § Suspend Capability`
> **DESIGN**: `archived-design/10-async/index.md` (archived — runtime architecture TBD)

**Dependencies**: Section 6 (Capabilities — `uses Suspend` capability declarations and propagation)

> **PREREQUISITE FOR**: Section 17 (Concurrency Extended) — select, cancellation, enhanced channels.

> **NOTE**: This section is thin for an entire concurrency support system. Before implementation, expand with: async runtime architecture (stackless coroutines vs green threads vs OS threads), task scheduling model (work-stealing, cooperative, event-loop), ARC interaction with task boundaries, Suspend context propagation in typeck, channel buffer semantics (bounded/unbounded, backpressure, close propagation), memory ownership transfer on channel send.

> **Future Enhancements**: Approved proposal `parallel-execution-guarantees-proposal.md` adds:
> - `Sendable` trait for safe cross-task transfer
> - Role-based channels (`Producer<T>`, `Consumer<T>`)
> - Ownership transfer semantics for channel send
> - Process isolation primitives
> See Section 17 for implementation details.

---

## 16.1 Suspend Capability

> **Naming**: The approved `rename-async-to-suspend-proposal.md` renamed `Async` to `Suspend`. The spec, lexer (`TokenKind::Suspend`), and syntax reference all use `Suspend`. (verified 2026-03-29)
>
> **Existing scaffolding**: `uses` clause parsing works for `Suspend` via the general capabilities infrastructure (Section 6). The lexer reserves `suspend` as a keyword. No async-specific semantic enforcement exists. DRIFT: `library/std/prelude.ori` still declares `pub trait Async {}` — must be renamed to `Suspend` before this section begins.

- [ ] **Implement**: `uses Suspend` declaration — spec/20-capabilities.md § Suspend Capability
  - [ ] **Rust Tests**: `ori_types/src/check/capabilities.rs` — suspend capability enforcement
  - [ ] **Ori Tests**: `tests/spec/async/declaration.ori`
  - [ ] **LLVM Support**: LLVM codegen for suspend capability declaration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — suspend capability declaration codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Sync vs suspend distinction — spec/20-capabilities.md § Suspend Capability
  - [ ] Type checker must enforce that non-Suspend functions cannot call Suspend functions without propagating the capability (E1200)
  - [ ] **Rust Tests**: `ori_types/src/check/capabilities.rs` — sync/suspend distinction
  - [ ] **Ori Tests**: `tests/spec/async/sync_suspend.ori`
  - [ ] **LLVM Support**: LLVM codegen for sync/suspend distinction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — sync/suspend distinction codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Fix**: Rename `pub trait Async {}` to `pub trait Suspend {}` in `library/std/prelude.ori` per approved rename-async-to-suspend proposal (DRIFT found 2026-03-29)

---

## 16.2 Structured Concurrency

> **Existing scaffolding**: `FunctionExpKind::Nursery` IR variant exists. Parser handles `nursery(body:, on_error:, timeout:)`. Evaluator has stub (unimplemented). (verified 2026-03-29)

- [ ] **Implement**: Structured concurrency — archived-design/10-async/index.md § Structured Concurrency
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/async.rs` — structured concurrency
  - [ ] **Ori Tests**: `tests/spec/async/structured.ori`
  - [ ] **LLVM Support**: LLVM codegen for structured concurrency
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — structured concurrency codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `nursery` pattern — spec/15-patterns.md § nursery (verified 2026-03-29 — missing from original plan)
  - [ ] Nursery runtime: task spawning, scoped lifetime, error collection
  - [ ] `NurseryErrorMode` (`CancelRemaining | CollectAll | FailFast`) per spec
  - [ ] `on_error:` callback support
  - [ ] `timeout:` enforcement within nursery scope
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/nursery.rs` — nursery pattern
  - [ ] **Ori Tests**: `tests/spec/async/nursery.ori`
  - [ ] **LLVM Support**: LLVM codegen for nursery pattern
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: No shared mutable state — archived-design/10-async/index.md § No Shared Mutable State
  - [ ] Ori's value semantics (capture by value, no shared mutable refs) provide the foundation, but explicit compiler enforcement is needed for async-specific guarantees
  - [ ] **Rust Tests**: `ori_types/src/check/mutability.rs` — shared state detection
  - [ ] **Ori Tests**: `tests/spec/async/no_shared_state.ori`
  - [ ] **LLVM Support**: LLVM codegen for no shared mutable state enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — no shared mutable state codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Sendable` trait enforcement — spec/08-types.md § Channel Types (verified 2026-03-29 — missing from original plan)
  - [ ] Auto-derived marker trait: all fields must be `Sendable`, no interior mutability, no non-Sendable captures
  - [ ] Required for channel types `T: Sendable`
  - [ ] Cannot be implemented manually
  - [ ] **Rust Tests**: `ori_types/src/check/sendable.rs` — sendable enforcement
  - [ ] **Ori Tests**: `tests/spec/async/sendable.ori`
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `CancellationError` / `CancellationReason` types — prelude types (verified 2026-03-29 — missing from original plan)
  - [ ] Required by parallel/nursery cancellation semantics
  - [ ] `CancellationReason` enum for nursery `on_error:` callback
  - [ ] **Ori Tests**: `tests/spec/async/cancellation_types.ori`

- [ ] **Implement**: `is_cancelled()` builtin — spec prelude (verified 2026-03-29 — missing from original plan)
  - [ ] Returns `bool` indicating whether the current task has been cancelled
  - [ ] **Ori Tests**: `tests/spec/async/is_cancelled.ori`

---

## 16.3 Concurrency Patterns

> **Existing scaffolding**: All four patterns (`parallel`, `spawn`, `timeout`, `channel`) have IR variants, parser support, and evaluator stubs that execute sequentially / return void. `tests/spec/patterns/parallel.ori` and `tests/spec/patterns/channels.ori` exist but only test synchronous fallback; all async-specific tests are commented out. (verified 2026-03-29)

- [ ] **Implement**: `parallel` pattern — spec/15-patterns.md § parallel
  - [ ] [partial] IR (`FunctionExpKind::Parallel`), parser, typeck partial, evaluator stub (sequential) exist (verified 2026-03-29)
  - [ ] Actual concurrent execution (currently runs tasks sequentially)
  - [ ] `max_concurrent:` enforcement
  - [ ] `timeout:` enforcement
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/parallel.rs` — parallel pattern async
  - [ ] **Ori Tests**: `tests/spec/async/parallel.ori`
  - [ ] **LLVM Support**: LLVM codegen for parallel pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — parallel pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `spawn` pattern — spec/15-patterns.md § spawn (verified 2026-03-29 — missing from original plan)
  - [ ] [partial] IR (`FunctionExpKind::Spawn`), parser, evaluator stub exist (verified 2026-03-29)
  - [ ] Fire-and-forget task spawning with `max_concurrent:` enforcement
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/spawn.rs` — spawn pattern
  - [ ] **Ori Tests**: `tests/spec/async/spawn.ori`
  - [ ] **LLVM Support**: LLVM codegen for spawn pattern
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `timeout` pattern — spec/15-patterns.md § timeout
  - [ ] [partial] IR (`FunctionExpKind::Timeout`), parser, evaluator stub exist (verified 2026-03-29)
  - [ ] Actual timeout enforcement with timer/cancellation infrastructure
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/timeout.rs` — timeout pattern async
  - [ ] **Ori Tests**: `tests/spec/async/timeout.ori`
  - [ ] **LLVM Support**: LLVM codegen for timeout pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — timeout pattern codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Channels — spec/08-types.md § Channel Types (section 8.5)
  - [ ] [partial] IR (`FunctionExpKind::Channel/ChannelIn/ChannelOut/ChannelAll`), parser, evaluator stub exist (verified 2026-03-29)
  - [ ] `Producer<T>`, `Consumer<T>`, `CloneableProducer<T>`, `CloneableConsumer<T>` types in typeck
  - [ ] Channel runtime: buffer, send/receive, close semantics
  - [ ] `Sendable` constraint enforcement on `T`
  - [ ] Ownership transfer on send (value consumed)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/channel.rs` — channel implementation
  - [ ] **Ori Tests**: `tests/spec/async/channels.ori`
  - [ ] **LLVM Support**: LLVM codegen for channels
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — channels codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 16.4 Async Error Traces

**Proposal**: `proposals/approved/error-trace-async-semantics-proposal.md` (verified exists 2026-03-29)

Implements error trace preservation across task boundaries in async code. Spec coverage in `spec/17-errors-and-panics.md` section 17.8. (verified 2026-03-29)

- [ ] **Implement**: Task boundary marker in traces — spec/17-errors-and-panics.md § 17.8.1 Task Boundary Marker
  - [ ] `TraceEntry { function: "<task boundary>", file: "", line: 0, column: 0 }` per spec
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/async.rs` — task boundary marker tests
  - [ ] **Ori Tests**: `tests/spec/async/trace_boundary.ori`
  - [ ] **LLVM Support**: LLVM codegen for task boundary marker
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — task boundary marker codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Trace preservation across parallel tasks — spec/17-errors-and-panics.md § Trace from Parallel Tasks
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/async.rs` — parallel trace tests
  - [ ] **Ori Tests**: `tests/spec/async/parallel_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for parallel trace preservation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — parallel trace codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Trace preservation across nursery tasks — spec/17-errors-and-panics.md § 17.8 Async Error Traces
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/nursery.rs` — nursery trace tests
  - [ ] **Ori Tests**: `tests/spec/async/nursery_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for nursery trace preservation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — nursery trace codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Catch and panic trace interaction — spec/17-errors-and-panics.md § 17.8.3 Catch and Panic Traces
  - [ ] Note: synchronous `catch` pattern (`FunctionExpKind::Catch`) works; async-specific trace interaction is not implemented (verified 2026-03-29)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/catch.rs` — catch trace tests
  - [ ] **Ori Tests**: `tests/spec/errors/catch_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for catch trace interaction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_tests.rs` — catch trace codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 16.5 Section Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] DRIFT resolved: `pub trait Async {}` renamed to `pub trait Suspend {}` in prelude
- [ ] Async runtime architecture decision documented (stackless coroutines vs green threads vs OS threads)
- [ ] Spec updated: `spec/20-capabilities.md` suspend section, `spec/15-patterns.md` concurrency patterns
- [ ] CLAUDE.md updated with async/concurrency syntax
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review last commit` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.

**Exit Criteria**: Suspend-based concurrent code compiles and runs; parallel/spawn/nursery/timeout/channels all functional with actual concurrency (not sequential stubs)
