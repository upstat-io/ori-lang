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
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/capabilities.rs` — async capability
  - [ ] **Sigil Tests**: `tests/spec/async/declaration.si`

- [ ] **Implement**: Sync vs async distinction — spec/14-capabilities.md § Async Capability, design/10-async/index.md § Sync vs Async
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/capabilities.rs` — sync/async distinction
  - [ ] **Sigil Tests**: `tests/spec/async/sync_async.si`

---

## 16.2 Structured Concurrency

- [ ] **Implement**: Structured concurrency — design/10-async/index.md § Structured Concurrency
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/async.rs` — structured concurrency
  - [ ] **Sigil Tests**: `tests/spec/async/structured.si`

- [ ] **Implement**: No shared mutable state — design/10-async/index.md § No Shared Mutable State
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/mutability.rs` — shared state detection
  - [ ] **Sigil Tests**: `tests/spec/async/no_shared_state.si`

---

## 16.3 Concurrency Patterns

- [ ] **Implement**: `parallel` pattern — spec/10-patterns.md § parallel, design/02-syntax/04-patterns-reference.md § parallel
  - [ ] **Rust Tests**: `sigilc/src/patterns/parallel.rs` — parallel pattern async
  - [ ] **Sigil Tests**: `tests/spec/async/parallel.si`

- [ ] **Implement**: `timeout` pattern — spec/10-patterns.md § timeout, design/02-syntax/04-patterns-reference.md § timeout
  - [ ] **Rust Tests**: `sigilc/src/patterns/timeout.rs` — timeout pattern async
  - [ ] **Sigil Tests**: `tests/spec/async/timeout.si`

- [ ] **Implement**: Channels — spec/06-types.md § Channel
  - [ ] **Rust Tests**: `sigilc/src/eval/channel.rs` — channel implementation
  - [ ] **Sigil Tests**: `tests/spec/async/channels.si`

---

## 16.4 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] Spec updated: `spec/14-capabilities.md` async section, `spec/10-patterns.md` concurrency patterns
- [ ] CLAUDE.md updated with async/concurrency syntax
- [ ] Re-evaluate against docs/compiler-design/v2/02-design-principles.md
- [ ] 80+% test coverage, tests against spec/design
- [ ] Run full test suite: `cargo test && sigil test tests/spec/`

**Exit Criteria**: Async code compiles and runs
