---
phase: 16
title: Async Support
status: not-started
tier: 6
goal: Async/await semantics via capabilities
spec:
  - spec/14-capabilities.md
sections:
  - id: "16.1"
    title: Async via Capability
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
    title: Phase Completion Checklist
    status: not-started
---

# Phase 16: Async Support

**Goal**: Async/await semantics via capabilities

> **SPEC**: `spec/14-capabilities.md § Async Capability`
> **DESIGN**: `design/10-async/index.md`

> **PREREQUISITE FOR**: Phase 17 (Concurrency Extended) — select, cancellation, enhanced channels.

> **Future Enhancements**: Approved proposal `parallel-concurrency-proposal.md` adds:
> - `Sendable` trait for safe cross-task transfer
> - Role-based channels (`Producer<T>`, `Consumer<T>`)
> - Ownership transfer semantics for channel send
> - Process isolation primitives
> See Phase 17 for implementation details.

---

## 16.1 Async via Capability

- [ ] **Implement**: `uses Async` declaration — spec/14-capabilities.md § Async Capability, design/10-async/index.md § Async via Capability
  - [ ] **Rust Tests**: `oric/src/typeck/checker/capabilities.rs` — async capability
  - [ ] **Ori Tests**: `tests/spec/async/declaration.ori`
  - [ ] **LLVM Support**: LLVM codegen for async capability declaration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — async capability declaration codegen

- [ ] **Implement**: Sync vs async distinction — spec/14-capabilities.md § Async Capability, design/10-async/index.md § Sync vs Async
  - [ ] **Rust Tests**: `oric/src/typeck/checker/capabilities.rs` — sync/async distinction
  - [ ] **Ori Tests**: `tests/spec/async/sync_async.ori`
  - [ ] **LLVM Support**: LLVM codegen for sync/async distinction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — sync/async distinction codegen

---

## 16.2 Structured Concurrency

- [ ] **Implement**: Structured concurrency — design/10-async/index.md § Structured Concurrency
  - [ ] **Rust Tests**: `oric/src/eval/exec/async.rs` — structured concurrency
  - [ ] **Ori Tests**: `tests/spec/async/structured.ori`
  - [ ] **LLVM Support**: LLVM codegen for structured concurrency
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — structured concurrency codegen

- [ ] **Implement**: No shared mutable state — design/10-async/index.md § No Shared Mutable State
  - [ ] **Rust Tests**: `oric/src/typeck/checker/mutability.rs` — shared state detection
  - [ ] **Ori Tests**: `tests/spec/async/no_shared_state.ori`
  - [ ] **LLVM Support**: LLVM codegen for no shared mutable state enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — no shared mutable state codegen

---

## 16.3 Concurrency Patterns

- [ ] **Implement**: `parallel` pattern — spec/10-patterns.md § parallel, design/02-syntax/04-patterns-reference.md § parallel
  - [ ] **Rust Tests**: `oric/src/patterns/parallel.rs` — parallel pattern async
  - [ ] **Ori Tests**: `tests/spec/async/parallel.ori`
  - [ ] **LLVM Support**: LLVM codegen for parallel pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — parallel pattern codegen

- [ ] **Implement**: `timeout` pattern — spec/10-patterns.md § timeout, design/02-syntax/04-patterns-reference.md § timeout
  - [ ] **Rust Tests**: `oric/src/patterns/timeout.rs` — timeout pattern async
  - [ ] **Ori Tests**: `tests/spec/async/timeout.ori`
  - [ ] **LLVM Support**: LLVM codegen for timeout pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — timeout pattern codegen

- [ ] **Implement**: Channels — spec/06-types.md § Channel
  - [ ] **Rust Tests**: `oric/src/eval/channel.rs` — channel implementation
  - [ ] **Ori Tests**: `tests/spec/async/channels.ori`
  - [ ] **LLVM Support**: LLVM codegen for channels
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — channels codegen

---

## 16.4 Async Error Traces

**Proposal**: `proposals/approved/error-trace-async-semantics-proposal.md`

Implements error trace preservation across task boundaries in async code.

- [ ] **Implement**: Task boundary marker in traces — spec/20-errors-and-panics.md § Task Boundary Marker
  - [ ] **Rust Tests**: `oric/src/eval/exec/async.rs` — task boundary marker tests
  - [ ] **Ori Tests**: `tests/spec/async/trace_boundary.ori`
  - [ ] **LLVM Support**: LLVM codegen for task boundary marker
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — task boundary marker codegen

- [ ] **Implement**: Trace preservation across parallel tasks — spec/20-errors-and-panics.md § Trace from Parallel Tasks
  - [ ] **Rust Tests**: `oric/src/eval/exec/async.rs` — parallel trace tests
  - [ ] **Ori Tests**: `tests/spec/async/parallel_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for parallel trace preservation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — parallel trace codegen

- [ ] **Implement**: Trace preservation across nursery tasks — spec/20-errors-and-panics.md § Async Error Traces
  - [ ] **Rust Tests**: `oric/src/eval/exec/nursery.rs` — nursery trace tests
  - [ ] **Ori Tests**: `tests/spec/async/nursery_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for nursery trace preservation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — nursery trace codegen

- [ ] **Implement**: Catch and panic trace interaction — spec/20-errors-and-panics.md § Catch and Panic Traces
  - [ ] **Rust Tests**: `oric/src/eval/exec/catch.rs` — catch trace tests
  - [ ] **Ori Tests**: `tests/spec/errors/catch_traces.ori`
  - [ ] **LLVM Support**: LLVM codegen for catch trace interaction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_tests.rs` — catch trace codegen

---

## 16.5 Phase Completion Checklist

- [ ] All items above have all checkboxes marked `[x]`
- [ ] Spec updated: `spec/14-capabilities.md` async section, `spec/10-patterns.md` concurrency patterns
- [ ] CLAUDE.md updated with async/concurrency syntax
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `./test-all`

**Exit Criteria**: Async code compiles and runs
