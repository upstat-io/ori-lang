---
section: 17
title: Concurrency Extended
status: not-started
reviewed: true
last_verified: "2026-03-29"
tier: 6
goal: Complete concurrency support with Sendable trait, role-based channels, nursery pattern, and structured concurrency
sections:
  - id: "17.0"
    title: Task and Async Context Definitions
    status: not-started
  - id: "17.1"
    title: Sendable Trait
    status: not-started
  - id: "17.2"
    title: Role-Based Channel Types
    status: not-started
  - id: "17.3"
    title: Channel Constructors
    status: not-started
  - id: "17.4"
    title: Ownership Transfer on Send
    status: not-started
  - id: "17.5"
    title: nursery Pattern
    status: not-started
  - id: "17.6"
    title: Parallel Execution Guarantees
    status: not-started
  - id: "17.7"
    title: Nursery Cancellation Semantics
    status: not-started
  - id: "17.8"
    title: Timeout and Spawn Pattern Semantics
    status: not-started
  - id: "17.9"
    title: Section Completion Checklist
    status: not-started
---

# Section 17: Concurrency Extended

**Goal**: Complete concurrency support with Sendable trait, role-based channels, nursery pattern, and structured concurrency

> **PROPOSALS**:
> - `docs/ori_lang/proposals/approved/sendable-channels-proposal.md`
> - `docs/ori_lang/proposals/approved/sendable-interior-mutability-proposal.md`
> - `docs/ori_lang/proposals/approved/task-async-context-proposal.md`
> - `docs/ori_lang/proposals/approved/closure-capture-semantics-proposal.md`
> - `docs/ori_lang/proposals/approved/parallel-execution-guarantees-proposal.md`
> - `docs/ori_lang/proposals/approved/nursery-cancellation-proposal.md`
> - `docs/ori_lang/proposals/approved/timeout-spawn-patterns-proposal.md`

**Dependencies**: Section 16 (Async Support), Section 3 (Traits — Sendable trait definition), Section 5 (Type Declarations — sum types for NurseryErrorMode, CancellationReason), Section 6 (Capabilities — `uses Suspend`/`uses Async`)

> **Verification (2026-03-29)**: Status `not-started` CONFIRMED. No substantive implementation exists.
> Existing scaffolding (FunctionExpKind variants for channel/parallel/spawn/timeout, Tag::Channel,
> Sendable registered in well-known traits, sequential stubs in evaluator) does not constitute
> meaningful progress. Section 16 (Async Support) is also not-started -- a prerequisite blocker.
>
> **10 gaps identified** (see .verify-results/section-17-results.md):
> - GAP-17-001: `nursery` not parseable -- no FunctionExpKind::Nursery variant
> - GAP-17-002: Channel constructor evaluator gap -- parsed but no eval dispatch
> - GAP-17-003: Spec clause numbering drift -- plan says 23, actual is 22 (FIXED in this update)
> - GAP-17-004: No Sendable auto-derivation logic (expected for not-started)
> - GAP-17-005: Channel type representation incomplete -- single Tag::Channel vs 4 role-based types
> - GAP-17-006: No stdlib concurrency types -- CancellationError/CancellationReason/NurseryErrorMode missing from prelude
> - GAP-17-007: is_cancelled() not implemented in any compiler crate
> - GAP-17-008: Section 16 dependency not started -- prerequisite blocker
> - GAP-17-009: 30+ planned Rust test file paths reference non-existent modules/directories
> - GAP-17-010: All concurrency spec tests commented out (3 files with extensive inactive test code)

### Sync Points: New Types (multi-crate sync required)


Adding the following types requires updates across **all** these crates:

**Channel types** (`Producer<T>`, `Consumer<T>`, `CloneableProducer<T>`, `CloneableConsumer<T>`):
1. **`ori_ir`** — Add type variants or generic type representations
2. **`ori_types`** — Register types, validate `T: Sendable` bounds, method signatures
3. **`ori_eval`** — Runtime value representations, method dispatch (send, receive, close, is_closed)
4. **`ori_llvm`** — LLVM type layouts, codegen for channel operations
5. **`library/std/prelude.ori`** — Type definitions and trait impls (Iterable for Consumer)

**Concurrency types** (`Nursery`, `NurseryErrorMode`, `CancellationError`, `CancellationReason`):
1. **`ori_ir`** — Sum type variants for NurseryErrorMode (CancelRemaining | CollectAll | FailFast) and CancellationReason (Timeout | SiblingFailed | NurseryExited | ExplicitCancel | ResourceExhausted)
2. **`ori_types`** — Register types, pattern exhaustiveness for sum types
3. **`ori_eval`** — Runtime value representations, nursery execution, cancellation state machine
4. **`ori_llvm`** — LLVM type layouts, codegen for nursery pattern and cancellation
5. **`library/std/prelude.ori`** — Type definitions in prelude

**Built-in function** (`is_cancelled()`):
1. **`ori_types`** — Type signature `() -> bool` in `infer_ident()`
2. **`ori_eval`** — Runtime implementation checking task cancellation state
3. **`ori_llvm`** — Codegen for `is_cancelled` intrinsic
4. **`library/std/prelude.ori`** — Prelude registration

---

## 17.0 Task and Async Context Definitions

**Proposal**: `proposals/approved/task-async-context-proposal.md`

Foundational definitions for tasks, async contexts, and suspension points that the rest of Section 17 depends on.

> **Verification (2026-03-29)**: 0/10 done, 1 partial (closure capture-by-value exists in eval
> via `environment/mod.rs` `capture()` but has no dedicated test/verification infrastructure).
> No task abstraction, async context, or suspension analysis exists in any crate.
> `ori_rt` uses non-atomic refcounting throughout. No `tests/spec/concurrency/` directory exists.

### Implementation

- [ ] **Implement**: Task definition and isolation model
  - [ ] **Rust Tests**: `ori_types/src/check/concurrency/task.rs` — task isolation checks
  - [ ] **Ori Tests**: `tests/spec/concurrency/task_isolation.ori`
  - [ ] **LLVM Support**: LLVM task representation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/task_tests.rs` — task codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Async context tracking
  - [ ] **Rust Tests**: `ori_types/src/check/concurrency/async_context.rs` — async context validation
  - [ ] **Ori Tests**: `tests/spec/concurrency/async_context.ori`
  - [ ] **LLVM Support**: LLVM async runtime integration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — async context codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Suspension point tracking
  - [ ] **Rust Tests**: `ori_types/src/check/concurrency/suspension.rs` — suspension point analysis
  - [ ] **Ori Tests**: `tests/spec/concurrency/suspension_points.ori`
  - [ ] **LLVM Support**: LLVM suspension codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — suspension codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: @main uses Async requirement for concurrency patterns
  - [ ] **Rust Tests**: `ori_types/src/check/concurrency/main_async.rs` — main async check
  - [ ] **Ori Tests**: `tests/compile-fail/main_without_async.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

- [ ] **Implement**: Async propagation checking
  - [ ] **Rust Tests**: `ori_types/src/check/concurrency/propagation.rs` — async propagation
  - [ ] **Ori Tests**: `tests/compile-fail/sync_calls_async.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

- [ ] **Implement**: Closure capture-by-value semantics — spec/11-blocks-and-scope.md § Lambda Capture
  - [ ] **Rust Tests**: `ori_types/src/check/closure/capture.rs` — capture-by-value verification
  - [ ] **Ori Tests**: `tests/spec/closures/capture_by_value.ori`
  - [ ] **Ori Tests**: `tests/spec/closures/capture_timing.ori`
  - [ ] **LLVM Support**: LLVM closure capture codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/closure_tests.rs` — capture codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Closure type inference and coercion — proposals/approved/closure-capture-semantics-proposal.md
  - [ ] **Rust Tests**: `ori_types/src/check/closure/types.rs` — closure type tests
  - [ ] **Ori Tests**: `tests/spec/closures/closure_types.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

- [ ] **Implement**: Captured binding immutability check — spec/11-blocks-and-scope.md § Capture Semantics
  - [ ] **Rust Tests**: `ori_types/src/check/closure/mutability.rs` — capture mutability check
  - [ ] **Ori Tests**: `tests/compile-fail/mutate_captured_binding.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

- [ ] **Implement**: Task capture ownership transfer
  - [ ] **Rust Tests**: `ori_types/src/check/concurrency/capture.rs` — capture analysis
  - [ ] **Ori Tests**: `tests/spec/concurrency/task_capture.ori`
  - [ ] **Ori Tests**: `tests/compile-fail/use_after_capture.ori`
  - [ ] **LLVM Support**: LLVM capture codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/task_tests.rs` — capture codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Atomic reference counting for cross-task values
  - [ ] **Rust Tests**: `ori_rt/src/rc/mod.rs` — atomic refcount
  - [ ] **LLVM Support**: LLVM atomic refcount intrinsics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/memory_tests.rs` — atomic refcount codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 17.1 Sendable Trait

**Proposal**: `proposals/approved/sendable-interior-mutability-proposal.md`

Auto-implemented marker trait for types that can safely cross task boundaries.

> **Verification (2026-03-29)**: 0/5 done, 1 partial. `Sendable` IS registered in well-known
> traits (`ori_types/src/check/well_known/mod.rs:108`, Duration/Size have it in trait sets).
> However: no auto-implementation logic, no field-recursive analysis, no enforcement at
> channel/spawn boundaries. Registered as a trait name only -- NOT semantically implemented.
> Commented-out `#compile_fail("not Sendable")` test exists in `channels.ori` line 162-166.

```ori
trait Sendable {}

// Automatically Sendable when ALL conditions met:
// 1. All fields are Sendable
// 2. No interior mutability (only exists in runtime resources)
// 3. No non-Sendable captured state (for closures)

type Point = { x: int, y: int }  // Sendable: all fields are int
type Handle = { file: FileHandle }  // NOT Sendable

// Manual implementation is forbidden
impl MyType: Sendable { }  // ERROR: cannot implement Sendable manually
```

Interior mutability does not exist in user-defined Ori types. Only runtime-provided resources (FileHandle, Socket, etc.) have interior mutability because they wrap OS state. Channel endpoints (Producer, Consumer) ARE Sendable.

### Implementation

- [ ] **Implement**: Add `Sendable` marker trait — auto-derive in `ori_types`, check at channel/spawn boundaries
  - [ ] **Rust Tests**: `ori_types/src/check/traits/sendable.rs` — sendable trait
  - [ ] **Ori Tests**: `tests/spec/concurrency/sendable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Sendable marker trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sendable_tests.rs` — Sendable trait codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Auto-implementation for primitives
  - [ ] **Ori Tests**: `tests/spec/concurrency/sendable_primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for Sendable auto-impl primitives
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sendable_tests.rs` — Sendable primitives codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Auto-implementation for compound types
  - [ ] **Ori Tests**: `tests/spec/concurrency/sendable_compound.ori`
  - [ ] **LLVM Support**: LLVM codegen for Sendable auto-impl compound types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sendable_tests.rs` — Sendable compound codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Closure capture analysis for Sendable
  - [ ] **Rust Tests**: `ori_types/src/check/sendable/closures.rs` — closure capture analysis
  - [ ] **Ori Tests**: `tests/spec/concurrency/sendable_closures.ori`
  - [ ] **LLVM Support**: LLVM codegen for closure Sendable analysis
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sendable_tests.rs` — closure Sendable codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Compiler error for non-Sendable in channel context
  - [ ] **Rust Tests**: `ori_types/src/check/sendable/errors.rs` — non-Sendable errors
  - [ ] **Ori Tests**: `tests/compile-fail/sendable_channel.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

---

## 17.2 Role-Based Channel Types

Split channels into Producer and Consumer roles with compile-time enforcement.

> **Verification (2026-03-29)**: 0/5 done. No `Producer<T>`, `Consumer<T>`, `CloneableProducer<T>`,
> `CloneableConsumer<T>` types exist. `Tag::Channel` (value 19) in `ori_types/src/tag/mod.rs` is
> a single generic `chan<T>` -- the old undifferentiated channel, not the role-based split.
> All channel type tests in `channels.ori` are commented out.

```ori
// Role-specific types
type Producer<T: Sendable>  // Can only send
type Consumer<T: Sendable>  // Can only receive

// Producer methods
impl<T: Sendable> Producer<T> {
    @send (self, value: T) -> void uses Async
    @close (self) -> void
    @is_closed (self) -> bool
}

// Consumer methods
impl<T: Sendable> Consumer<T> {
    @receive (self) -> Option<T> uses Async
    @is_closed (self) -> bool
}

// Consumer is Iterable
impl<T: Sendable> Iterable for Consumer<T> { ... }
```

### Implementation

- [ ] **Implement**: `Producer<T>` type
  - [ ] **Rust Tests**: `ori_ir/src/types/channel.rs` — Producer type
  - [ ] **Ori Tests**: `tests/spec/concurrency/producer.ori`
  - [ ] **LLVM Support**: LLVM codegen for Producer type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — Producer codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Consumer<T>` type
  - [ ] **Rust Tests**: `ori_ir/src/types/channel.rs` — Consumer type
  - [ ] **Ori Tests**: `tests/spec/concurrency/consumer.ori`
  - [ ] **LLVM Support**: LLVM codegen for Consumer type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — Consumer codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `CloneableProducer<T>` type (implements Clone)
  - [ ] **Rust Tests**: `ori_ir/src/types/channel.rs` — CloneableProducer type
  - [ ] **Ori Tests**: `tests/spec/concurrency/cloneable_producer.ori`
  - [ ] **LLVM Support**: LLVM codegen for CloneableProducer
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — CloneableProducer codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `CloneableConsumer<T>` type (implements Clone)
  - [ ] **Rust Tests**: `ori_ir/src/types/channel.rs` — CloneableConsumer type
  - [ ] **Ori Tests**: `tests/spec/concurrency/cloneable_consumer.ori`
  - [ ] **LLVM Support**: LLVM codegen for CloneableConsumer
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — CloneableConsumer codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Consumer implements Iterable
  - [ ] **Rust Tests**: `ori_ir/src/types/channel.rs` — Consumer Iterable impl
  - [ ] **Ori Tests**: `tests/spec/concurrency/consumer_iterable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Consumer iteration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — Consumer iteration codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 17.3 Channel Constructors

Four constructors for different concurrency patterns.

> **Verification (2026-03-29)**: 0/5 done, 1 partial. `FunctionExpKind` has `Channel`/`ChannelIn`/
> `ChannelOut`/`ChannelAll` variants (parser can parse them), but the evaluator
> (`function_exp.rs`) has NO match arms for these -- any Ori program using `channel<T>(buffer: N)`
> would hit a missing match arm at runtime. GAP-17-002.

```ori
// One-to-one (exclusive, fastest)
@channel<T: Sendable> (buffer: int) -> (Producer<T>, Consumer<T>)

// Fan-in (many-to-one)
@channel_in<T: Sendable> (buffer: int) -> (CloneableProducer<T>, Consumer<T>)

// Fan-out (one-to-many)
@channel_out<T: Sendable> (buffer: int) -> (Producer<T>, CloneableConsumer<T>)

// Many-to-many (broadcast)
@channel_all<T: Sendable> (buffer: int) -> (CloneableProducer<T>, CloneableConsumer<T>)
```

### Implementation

- [ ] **Implement**: `channel<T>()` — exclusive channel
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/channel.rs` — exclusive channel
  - [ ] **Ori Tests**: `tests/spec/concurrency/channel_exclusive.ori`
  - [ ] **LLVM Support**: LLVM codegen for exclusive channel
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — exclusive channel codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `channel_in<T>()` — fan-in channel
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/channel.rs` — fan-in channel
  - [ ] **Ori Tests**: `tests/spec/concurrency/channel_fan_in.ori`
  - [ ] **LLVM Support**: LLVM codegen for fan-in channel
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — fan-in channel codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `channel_out<T>()` — fan-out channel
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/channel.rs` — fan-out channel
  - [ ] **Ori Tests**: `tests/spec/concurrency/channel_fan_out.ori`
  - [ ] **LLVM Support**: LLVM codegen for fan-out channel
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — fan-out channel codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `channel_all<T>()` — broadcast channel
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/channel.rs` — broadcast channel
  - [ ] **Ori Tests**: `tests/spec/concurrency/channel_broadcast.ori`
  - [ ] **LLVM Support**: LLVM codegen for broadcast channel
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — broadcast channel codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Deprecate old `Channel<T>` type
  - [ ] **Rust Tests**: `ori_types/src/check/deprecated.rs` — Channel deprecation warning
  - [ ] **Ori Tests**: `tests/spec/concurrency/channel_migration.ori`

---

## 17.4 Ownership Transfer on Send

Values are consumed when sent, preventing data races.

> **Verification (2026-03-29)**: 0/3 done. No move semantics on channel send. No use-after-send
> analysis. `tests/compile-fail/use_after_send.ori` does not exist.

```ori
@producer (p: Producer<Data>) -> void uses Async = {
    let data = create_data()
    p.send(value: data),  // Ownership transferred
    // data.field         // ERROR: 'data' moved into channel
}
```

### Implementation

- [ ] **Implement**: Move semantics on `send`
  - [ ] **Rust Tests**: `ori_types/src/check/ownership.rs` — move on send
  - [ ] **Ori Tests**: `tests/spec/concurrency/ownership_transfer.ori`
  - [ ] **LLVM Support**: LLVM codegen for ownership transfer
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — ownership codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Compiler error for use-after-send
  - [ ] **Rust Tests**: `ori_types/src/check/ownership.rs` — use-after-send error
  - [ ] **Ori Tests**: `tests/compile-fail/use_after_send.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

- [ ] **Implement**: Explicit clone for retained access
  - [ ] **Ori Tests**: `tests/spec/concurrency/send_clone.ori`
  - [ ] **LLVM Support**: LLVM codegen for clone before send
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — send clone codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 17.5 nursery Pattern

Structured concurrency with guaranteed task completion.

> **Verification (2026-03-29)**: 0/8 done. PREREQUISITE GAP: `FunctionExpKind` has NO `Nursery`
> variant -- the parser CANNOT parse `nursery(...)`. GAP-17-001. The formatter
> (`ori_fmt/src/packing/construct.rs`) recognizes `ConstructKind::Nursery` for formatting
> scaffolding only. No `Nursery` type, no `NurseryErrorMode` sum type, no nursery evaluation.
> All nursery tests in `channels.ori` (lines 172-246) are commented out.

```ori
nursery(
    body: n -> for item in items do n.spawn(task: () -> process(item)),
    on_error: CollectAll,
    timeout: 30s,
)

type NurseryErrorMode = CancelRemaining | CollectAll | FailFast
```

### Implementation

- [ ] **Implement**: `nursery` pattern parsing
  - [ ] **Rust Tests**: `ori_parse/src/grammar/patterns.rs` — nursery parsing
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_basic.ori`
  - [ ] **LLVM Support**: LLVM codegen for nursery pattern
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — nursery codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Nursery` type with `spawn` method
  - [ ] **Rust Tests**: `ori_ir/src/types/nursery.rs` — Nursery type
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_spawn.ori`
  - [ ] **LLVM Support**: LLVM codegen for Nursery type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — Nursery type codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `NurseryErrorMode` sum type
  - [ ] **Rust Tests**: `ori_ir/src/types/nursery.rs` — NurseryErrorMode type
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_error_modes.ori`
  - [ ] **LLVM Support**: LLVM codegen for NurseryErrorMode
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — NurseryErrorMode codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `CancelRemaining` error handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_cancel_remaining.ori`
  - [ ] **LLVM Support**: LLVM codegen for CancelRemaining
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — CancelRemaining codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `CollectAll` error handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_collect_all.ori`
  - [ ] **LLVM Support**: LLVM codegen for CollectAll
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — CollectAll codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `FailFast` error handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_fail_fast.ori`
  - [ ] **LLVM Support**: LLVM codegen for FailFast
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — FailFast codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Nursery timeout support — `timeout:` parameter with cancellation propagation
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/nursery.rs` — timeout handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_timeout.ori`
  - [ ] **LLVM Support**: LLVM codegen for nursery timeout
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — timeout codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Return type `[Result<T, E>]`
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_results.ori`
  - [ ] **LLVM Support**: LLVM codegen for nursery results
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — results codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 17.6 Parallel Execution Guarantees

**Proposal**: `proposals/approved/parallel-execution-guarantees-proposal.md`

Specifies execution guarantees for the `parallel` pattern: task ordering, concurrency limits, resource exhaustion, and timeout behavior.

> **Verification (2026-03-29)**: 0/6 done, 1 partial. The stub in `function_exp.rs:219-237`
> iterates tasks in list order (sequential) -- trivially satisfies start order but is NOT
> concurrent execution. `max_concurrent`, `timeout`, resource exhaustion, and empty list
> handling are all unimplemented. All parallel tests in `parallel.ori` are commented out;
> active tests use `for...yield` as sequential simulation.

### Implementation

- [ ] **Implement**: Start order guarantee (tasks start in list order)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/parallel.rs` — start order verification
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_start_order.ori`
  - [ ] **LLVM Support**: LLVM codegen for ordered start
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — start order codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Result order guarantee (results match input order)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/parallel.rs` — result order verification
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_result_order.ori`
  - [ ] **LLVM Support**: LLVM codegen for result ordering
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — result order codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `max_concurrent: Option<int>` parameter
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/parallel.rs` — concurrency limit
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_max_concurrent.ori`
  - [ ] **LLVM Support**: LLVM codegen for concurrency limit
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — max_concurrent codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `timeout: Option<Duration>` parameter
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/parallel.rs` — timeout handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_timeout.ori`
  - [ ] **LLVM Support**: LLVM codegen for parallel timeout
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — timeout codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Resource exhaustion error handling
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/parallel.rs` — resource exhaustion
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_resource_exhaustion.ori`
  - [ ] **LLVM Support**: LLVM codegen for resource exhaustion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — resource exhaustion codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Empty task list handling (returns `[]` immediately)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/parallel.rs` — empty list
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_empty.ori`
  - [ ] **LLVM Support**: LLVM codegen for empty parallel
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — empty codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 17.7 Nursery Cancellation Semantics

**Proposal**: `proposals/approved/nursery-cancellation-proposal.md`

**Spec**: `spec/22-concurrency-model.md` § Cancellation (added cooperative model, checkpoints, CancellationError, is_cancelled)

Specifies cooperative cancellation model, checkpoints, error mode behaviors, and cleanup guarantees.

> **Verification (2026-03-29)**: 0/8 done. Spec (Clause 22.7) is well-written with checkpoints,
> CancellationError/CancellationReason types, error mode semantics, timeout cancellation, and
> cleanup guarantees -- most complete part of the concurrency story. But: `is_cancelled()` is
> NOT implemented in any compiler crate despite being documented in CLAUDE.md and ori-syntax.md.
> No `CancellationError` or `CancellationReason` types in `library/std/prelude.ori` or compiler.
> Single commented-out `is_cancelled` test in `channels.ori` (lines 252-255).
> DRIFT FIXED: was referencing `spec/23-concurrency-model.md`, actual file is `spec/22-concurrency-model.md`.

### Implementation

- [ ] **Spec**: Cancellation semantics in `spec/22-concurrency-model.md` DONE

- [ ] **Implement**: Cooperative cancellation model
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/cancellation.rs` — cooperative cancellation
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellation_cooperative.ori`
  - [ ] **LLVM Support**: LLVM codegen for cancellation state
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — cancellation codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Cancellation checkpoints (suspension, loop iteration, pattern entry)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/cancellation.rs` — checkpoint detection
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellation_checkpoints.ori`
  - [ ] **LLVM Support**: LLVM codegen for checkpoint insertion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — checkpoint codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `CancellationError` type
  - [ ] **Rust Tests**: `ori_ir/src/types/cancellation.rs` — CancellationError type
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellation_error.ori`
  - [ ] **LLVM Support**: LLVM codegen for CancellationError
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — CancellationError codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `CancellationReason` sum type (Timeout, SiblingFailed, NurseryExited, ExplicitCancel, ResourceExhausted)
  - [ ] **Rust Tests**: `ori_ir/src/types/cancellation.rs` — CancellationReason type
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellation_reason.ori`
  - [ ] **LLVM Support**: LLVM codegen for CancellationReason
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — CancellationReason codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `is_cancelled()` built-in function
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/builtins.rs` — is_cancelled
  - [ ] **Ori Tests**: `tests/spec/concurrency/is_cancelled.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_cancelled
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — is_cancelled codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Automatic loop cancellation checking in async contexts
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/loops.rs` — automatic cancellation check
  - [ ] **Ori Tests**: `tests/spec/concurrency/loop_cancellation.ori`
  - [ ] **LLVM Support**: LLVM codegen for loop cancellation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — loop cancellation codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Destructor execution guarantee during cancellation
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/cancellation.rs` — destructor guarantee
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellation_cleanup.ori`
  - [ ] **LLVM Support**: LLVM codegen for cancellation unwinding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — cleanup codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Nested nursery cancellation propagation
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/nursery.rs` — nested cancellation
  - [ ] **Ori Tests**: `tests/spec/concurrency/nested_nursery_cancellation.ori`
  - [ ] **LLVM Support**: LLVM codegen for nested cancellation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — nested cancellation codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 17.8 Timeout and Spawn Pattern Semantics

**Proposal**: `proposals/approved/timeout-spawn-patterns-proposal.md`

Formalizes semantics for `timeout` and `spawn` patterns including cancellation behavior, error handling, and task lifetime.

> **Verification (2026-03-29)**: 0/12 done, 2 partial. `timeout` stub executes operation, wraps
> in `Ok()`, ignores timeout duration -- no `CancellationError` return. `spawn` stub executes
> tasks sequentially, ignores errors, returns `Void`. Neither implements actual concurrency.
> All timeout and spawn tests in `concurrency.ori` and `parallel.ori` are commented out.

### Timeout Pattern Implementation

- [ ] **Implement**: `timeout(op:, after:)` pattern returns `Result<T, CancellationError>`
  - [ ] **Rust Tests**: `ori_patterns/src/timeout.rs` — timeout return type tests
  - [ ] **Ori Tests**: `tests/spec/concurrency/timeout_semantics.ori`
  - [ ] **LLVM Support**: LLVM codegen for timeout with cancellation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/timeout_tests.rs` — timeout codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Cooperative cancellation on timeout expiry
  - [ ] **Rust Tests**: `ori_patterns/src/timeout.rs` — cancellation tests
  - [ ] **Ori Tests**: `tests/spec/concurrency/timeout_cancellation.ori`
  - [ ] **LLVM Support**: LLVM timeout cancellation codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/timeout_tests.rs` — cancellation codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Cancellation checkpoints (suspending calls, loops, pattern entry)
  - [ ] **Rust Tests**: `ori_patterns/src/timeout.rs` — checkpoint tests
  - [ ] **Ori Tests**: `tests/spec/concurrency/timeout_checkpoints.ori`
  - [ ] **LLVM Support**: LLVM checkpoint insertion for timeout
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/timeout_tests.rs` — checkpoint codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Nested timeout support (inner can be shorter than outer)
  - [ ] **Rust Tests**: `ori_patterns/src/timeout.rs` — nested timeout tests
  - [ ] **Ori Tests**: `tests/spec/concurrency/timeout_nested.ori`
  - [ ] **LLVM Support**: LLVM nested timeout codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/timeout_tests.rs` — nested codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Error E1010 — `timeout` requires `Suspend` capability
  - [ ] **Rust Tests**: `ori_types/src/check/capabilities/timeout.rs` — capability error
  - [ ] **Ori Tests**: `tests/compile-fail/timeout_no_suspend.ori`

### Spawn Pattern Implementation

- [ ] **Implement**: `spawn(tasks:, max_concurrent:)` pattern returns `void`
  - [ ] **Rust Tests**: `ori_patterns/src/spawn.rs` — spawn return type tests
  - [ ] **Ori Tests**: `tests/spec/concurrency/spawn_semantics.ori`
  - [ ] **LLVM Support**: LLVM codegen for spawn
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spawn_tests.rs` — spawn codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Fire-and-forget semantics (errors silently discarded)
  - [ ] **Rust Tests**: `ori_patterns/src/spawn.rs` — error discard tests
  - [ ] **Ori Tests**: `tests/spec/concurrency/spawn_fire_forget.ori`
  - [ ] **LLVM Support**: LLVM spawn error handling codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spawn_tests.rs` — fire-forget codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Task escapes spawning scope (outlive spawning function)
  - [ ] **Rust Tests**: `ori_patterns/src/spawn.rs` — task lifetime tests
  - [ ] **Ori Tests**: `tests/spec/concurrency/spawn_lifetime.ori`
  - [ ] **LLVM Support**: LLVM unscoped task codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spawn_tests.rs` — lifetime codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `max_concurrent: Option<int>` parameter (default None = unlimited)
  - [ ] **Rust Tests**: `ori_patterns/src/spawn.rs` — concurrency limit tests
  - [ ] **Ori Tests**: `tests/spec/concurrency/spawn_max_concurrent.ori`
  - [ ] **LLVM Support**: LLVM spawn concurrency limit codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spawn_tests.rs` — max_concurrent codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Resource exhaustion handling (task dropped, no error)
  - [ ] **Rust Tests**: `ori_patterns/src/spawn.rs` — resource exhaustion tests
  - [ ] **Ori Tests**: `tests/spec/concurrency/spawn_resource_exhaustion.ori`
  - [ ] **LLVM Support**: LLVM resource exhaustion codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spawn_tests.rs` — exhaustion codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Tasks cancelled on program exit
  - [ ] **Rust Tests**: `ori_patterns/src/spawn.rs` — exit cancellation tests
  - [ ] **Ori Tests**: `tests/spec/concurrency/spawn_exit.ori`

- [ ] **Implement**: Error E1011 — `spawn` tasks must use `Suspend`
  - [ ] **Rust Tests**: `ori_types/src/check/capabilities/spawn.rs` — capability error
  - [ ] **Ori Tests**: `tests/compile-fail/spawn_no_suspend.ori`

---

## 17.9 Section Completion Checklist

> **Verification (2026-03-29)**: 0/16 done. Consistent with overall not-started status.

- [ ] All items in 17.1-17.8 have checkboxes marked `[ ]`
- [ ] Spec updated: `spec/08-types.md` — Sendable, Producer, Consumer, CloneableProducer, CloneableConsumer, CancellationError, CancellationReason
- [ ] Spec updated: `spec/15-patterns.md` — nursery pattern, parallel execution guarantees
- [ ] Spec updated: `spec/22-concurrency-model.md` — cancellation model
- [ ] CLAUDE.md updated with channel constructors, nursery syntax, is_cancelled()
- [ ] Sendable trait working (auto-implemented)
- [ ] Role-based channels working (Producer/Consumer)
- [ ] Channel constructors working (channel, channel_in, channel_out, channel_all)
- [ ] Ownership transfer on send working
- [ ] nursery pattern working
- [ ] Parallel execution guarantees working (ordering, max_concurrent, timeout)
- [ ] Cancellation semantics working (cooperative, checkpoints, is_cancelled)
- [ ] Timeout pattern working (returns CancellationError, cooperative cancellation)
- [ ] Spawn pattern working (fire-and-forget, task escapes scope, errors discarded)
- [ ] All tests pass: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria**: Can write producer/consumer pipeline with ownership safety and proper cancellation

---

## Example: Worker Pool with Fan-In

```ori
@worker_pool (jobs: [Job]) -> [Result<Output, Error>] uses Async = {
    let (sender, receiver) = channel_in<Result<Output, Error>>(buffer: 100)

    nursery(
        body: n -> {
            // Spawn workers with cloned senders
            for i in 0..4 do
                n.spawn(task: () -> worker(sender.clone(), i))
            // Spawn job feeder
            n.spawn(task: () -> {
                for job in jobs do sender.send(value: Ok(job))
                sender.close()
            })
        }
        on_error: CollectAll
    )

    // Collect results
    for result in receiver yield result
}
```

---

## Future Work (Deferred to Separate Proposals)

The following features are deferred to separate proposals:

### Channel Select (Future Proposal)

```ori
select(
    recv(ch1) -> value: handle_ch1(value),
    recv(ch2) -> value: handle_ch2(value),
    after(5s): handle_timeout(),
)
```

### Cancellation Tokens (Future Proposal)

```ori
let token = CancellationToken.new()
let child = token.child()
token.cancel()
```

### Graceful Shutdown (Future Proposal)

```ori
on_signal(signal: SIGINT, handler: () -> shutdown.cancel())
```

See `docs/ori_lang/proposals/approved/sendable-channels-proposal.md` § Future Work for details.
