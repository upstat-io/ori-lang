# Phase 17: Concurrency Extended

**Goal**: Complete concurrency support with Sendable trait, role-based channels, nursery pattern, and structured concurrency

> **PROPOSALS**:
> - `docs/ori_lang/proposals/approved/sendable-channels-proposal.md`
> - `docs/ori_lang/proposals/approved/sendable-interior-mutability-proposal.md`
> - `docs/ori_lang/proposals/approved/task-async-context-proposal.md`
> - `docs/ori_lang/proposals/approved/closure-capture-semantics-proposal.md`
> - `docs/ori_lang/proposals/approved/parallel-execution-guarantees-proposal.md`
> - `docs/ori_lang/proposals/approved/nursery-cancellation-proposal.md`

**Dependencies**: Phase 16 (Async Support)

---

## 17.0 Task and Async Context Definitions

**Proposal**: `proposals/approved/task-async-context-proposal.md`

Foundational definitions for tasks, async contexts, and suspension points that the rest of Phase 17 depends on.

### Implementation

- [ ] **Implement**: Task definition and isolation model
  - [ ] **Rust Tests**: `oric/src/typeck/concurrency/task.rs` — task isolation checks
  - [ ] **Ori Tests**: `tests/spec/concurrency/task_isolation.ori`
  - [ ] **LLVM Support**: LLVM task representation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/task_tests.rs` — task codegen

- [ ] **Implement**: Async context tracking
  - [ ] **Rust Tests**: `oric/src/typeck/concurrency/async_context.rs` — async context validation
  - [ ] **Ori Tests**: `tests/spec/concurrency/async_context.ori`
  - [ ] **LLVM Support**: LLVM async runtime integration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — async context codegen

- [ ] **Implement**: Suspension point tracking
  - [ ] **Rust Tests**: `oric/src/typeck/concurrency/suspension.rs` — suspension point analysis
  - [ ] **Ori Tests**: `tests/spec/concurrency/suspension_points.ori`
  - [ ] **LLVM Support**: LLVM suspension codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/async_tests.rs` — suspension codegen

- [ ] **Implement**: @main uses Async requirement for concurrency patterns
  - [ ] **Rust Tests**: `oric/src/typeck/concurrency/main_async.rs` — main async check
  - [ ] **Ori Tests**: `tests/compile-fail/main_without_async.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

- [ ] **Implement**: Async propagation checking
  - [ ] **Rust Tests**: `oric/src/typeck/concurrency/propagation.rs` — async propagation
  - [ ] **Ori Tests**: `tests/compile-fail/sync_calls_async.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

- [ ] **Implement**: Closure capture-by-value semantics — spec/17-blocks-and-scope.md § Lambda Capture
  - [ ] **Rust Tests**: `oric/src/typeck/closure/capture.rs` — capture-by-value verification
  - [ ] **Ori Tests**: `tests/spec/closures/capture_by_value.ori`
  - [ ] **Ori Tests**: `tests/spec/closures/capture_timing.ori`
  - [ ] **LLVM Support**: LLVM closure capture codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/closure_tests.rs` — capture codegen

- [ ] **Implement**: Closure type inference and coercion — proposals/approved/closure-capture-semantics-proposal.md
  - [ ] **Rust Tests**: `oric/src/typeck/closure/types.rs` — closure type tests
  - [ ] **Ori Tests**: `tests/spec/closures/closure_types.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

- [ ] **Implement**: Captured binding immutability check — spec/17-blocks-and-scope.md § Capture Semantics
  - [ ] **Rust Tests**: `oric/src/typeck/closure/mutability.rs` — capture mutability check
  - [ ] **Ori Tests**: `tests/compile-fail/mutate_captured_binding.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

- [ ] **Implement**: Task capture ownership transfer
  - [ ] **Rust Tests**: `oric/src/typeck/concurrency/capture.rs` — capture analysis
  - [ ] **Ori Tests**: `tests/spec/concurrency/task_capture.ori`
  - [ ] **Ori Tests**: `tests/compile-fail/use_after_capture.ori`
  - [ ] **LLVM Support**: LLVM capture codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/task_tests.rs` — capture codegen

- [ ] **Implement**: Atomic reference counting for cross-task values
  - [ ] **Rust Tests**: `oric/src/runtime/refcount.rs` — atomic refcount
  - [ ] **LLVM Support**: LLVM atomic refcount intrinsics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/memory_tests.rs` — atomic refcount codegen

---

## 17.1 Sendable Trait

**Proposal**: `proposals/approved/sendable-interior-mutability-proposal.md`

Auto-implemented marker trait for types that can safely cross task boundaries.

```ori
trait Sendable {}

// Automatically Sendable when ALL conditions met:
// 1. All fields are Sendable
// 2. No interior mutability (only exists in runtime resources)
// 3. No non-Sendable captured state (for closures)

type Point = { x: int, y: int }  // Sendable: all fields are int
type Handle = { file: FileHandle }  // NOT Sendable

// Manual implementation is forbidden
impl Sendable for MyType { }  // ERROR: cannot implement Sendable manually
```

Interior mutability does not exist in user-defined Ori types. Only runtime-provided resources (FileHandle, Socket, etc.) have interior mutability because they wrap OS state. Channel endpoints (Producer, Consumer) ARE Sendable.

### Implementation

- [ ] **Implement**: Add `Sendable` marker trait to type system
  - [ ] **Rust Tests**: `oric/src/typeck/traits/sendable.rs` — sendable trait
  - [ ] **Ori Tests**: `tests/spec/concurrency/sendable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Sendable marker trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sendable_tests.rs` — Sendable trait codegen

- [ ] **Implement**: Auto-implementation for primitives
  - [ ] **Ori Tests**: `tests/spec/concurrency/sendable_primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for Sendable auto-impl primitives
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sendable_tests.rs` — Sendable primitives codegen

- [ ] **Implement**: Auto-implementation for compound types
  - [ ] **Ori Tests**: `tests/spec/concurrency/sendable_compound.ori`
  - [ ] **LLVM Support**: LLVM codegen for Sendable auto-impl compound types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sendable_tests.rs` — Sendable compound codegen

- [ ] **Implement**: Closure capture analysis for Sendable
  - [ ] **Rust Tests**: `oric/src/typeck/sendable/closures.rs` — closure capture analysis
  - [ ] **Ori Tests**: `tests/spec/concurrency/sendable_closures.ori`
  - [ ] **LLVM Support**: LLVM codegen for closure Sendable analysis
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sendable_tests.rs` — closure Sendable codegen

- [ ] **Implement**: Compiler error for non-Sendable in channel context
  - [ ] **Rust Tests**: `oric/src/typeck/sendable/errors.rs` — non-Sendable errors
  - [ ] **Ori Tests**: `tests/compile-fail/sendable_channel.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

---

## 17.2 Role-Based Channel Types

Split channels into Producer and Consumer roles with compile-time enforcement.

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
  - [ ] **Rust Tests**: `oric/src/types/channel.rs` — Producer type
  - [ ] **Ori Tests**: `tests/spec/concurrency/producer.ori`
  - [ ] **LLVM Support**: LLVM codegen for Producer type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — Producer codegen

- [ ] **Implement**: `Consumer<T>` type
  - [ ] **Rust Tests**: `oric/src/types/channel.rs` — Consumer type
  - [ ] **Ori Tests**: `tests/spec/concurrency/consumer.ori`
  - [ ] **LLVM Support**: LLVM codegen for Consumer type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — Consumer codegen

- [ ] **Implement**: `CloneableProducer<T>` type (implements Clone)
  - [ ] **Rust Tests**: `oric/src/types/channel.rs` — CloneableProducer type
  - [ ] **Ori Tests**: `tests/spec/concurrency/cloneable_producer.ori`
  - [ ] **LLVM Support**: LLVM codegen for CloneableProducer
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — CloneableProducer codegen

- [ ] **Implement**: `CloneableConsumer<T>` type (implements Clone)
  - [ ] **Rust Tests**: `oric/src/types/channel.rs` — CloneableConsumer type
  - [ ] **Ori Tests**: `tests/spec/concurrency/cloneable_consumer.ori`
  - [ ] **LLVM Support**: LLVM codegen for CloneableConsumer
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — CloneableConsumer codegen

- [ ] **Implement**: Consumer implements Iterable
  - [ ] **Rust Tests**: `oric/src/types/channel.rs` — Consumer Iterable impl
  - [ ] **Ori Tests**: `tests/spec/concurrency/consumer_iterable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Consumer iteration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — Consumer iteration codegen

---

## 17.3 Channel Constructors

Four constructors for different concurrency patterns.

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
  - [ ] **Rust Tests**: `oric/src/eval/channel.rs` — exclusive channel
  - [ ] **Ori Tests**: `tests/spec/concurrency/channel_exclusive.ori`
  - [ ] **LLVM Support**: LLVM codegen for exclusive channel
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — exclusive channel codegen

- [ ] **Implement**: `channel_in<T>()` — fan-in channel
  - [ ] **Rust Tests**: `oric/src/eval/channel.rs` — fan-in channel
  - [ ] **Ori Tests**: `tests/spec/concurrency/channel_fan_in.ori`
  - [ ] **LLVM Support**: LLVM codegen for fan-in channel
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — fan-in channel codegen

- [ ] **Implement**: `channel_out<T>()` — fan-out channel
  - [ ] **Rust Tests**: `oric/src/eval/channel.rs` — fan-out channel
  - [ ] **Ori Tests**: `tests/spec/concurrency/channel_fan_out.ori`
  - [ ] **LLVM Support**: LLVM codegen for fan-out channel
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — fan-out channel codegen

- [ ] **Implement**: `channel_all<T>()` — broadcast channel
  - [ ] **Rust Tests**: `oric/src/eval/channel.rs` — broadcast channel
  - [ ] **Ori Tests**: `tests/spec/concurrency/channel_broadcast.ori`
  - [ ] **LLVM Support**: LLVM codegen for broadcast channel
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — broadcast channel codegen

- [ ] **Implement**: Deprecate old `Channel<T>` type
  - [ ] **Rust Tests**: `oric/src/typeck/deprecated.rs` — Channel deprecation warning
  - [ ] **Ori Tests**: `tests/spec/concurrency/channel_migration.ori`

---

## 17.4 Ownership Transfer on Send

Values are consumed when sent, preventing data races.

```ori
@producer (p: Producer<Data>) -> void uses Async = run(
    let data = create_data(),
    p.send(value: data),  // Ownership transferred
    // data.field         // ERROR: 'data' moved into channel
)
```

### Implementation

- [ ] **Implement**: Move semantics on `send`
  - [ ] **Rust Tests**: `oric/src/typeck/ownership.rs` — move on send
  - [ ] **Ori Tests**: `tests/spec/concurrency/ownership_transfer.ori`
  - [ ] **LLVM Support**: LLVM codegen for ownership transfer
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — ownership codegen

- [ ] **Implement**: Compiler error for use-after-send
  - [ ] **Rust Tests**: `oric/src/typeck/ownership.rs` — use-after-send error
  - [ ] **Ori Tests**: `tests/compile-fail/use_after_send.ori`
  - [ ] **LLVM Support**: N/A (compile-time only)

- [ ] **Implement**: Explicit clone for retained access
  - [ ] **Ori Tests**: `tests/spec/concurrency/send_clone.ori`
  - [ ] **LLVM Support**: LLVM codegen for clone before send
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — send clone codegen

---

## 17.5 nursery Pattern

Structured concurrency with guaranteed task completion.

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

- [ ] **Implement**: `Nursery` type with `spawn` method
  - [ ] **Rust Tests**: `oric/src/types/nursery.rs` — Nursery type
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_spawn.ori`
  - [ ] **LLVM Support**: LLVM codegen for Nursery type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — Nursery type codegen

- [ ] **Implement**: `NurseryErrorMode` sum type
  - [ ] **Rust Tests**: `oric/src/types/nursery.rs` — NurseryErrorMode type
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_error_modes.ori`
  - [ ] **LLVM Support**: LLVM codegen for NurseryErrorMode
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — NurseryErrorMode codegen

- [ ] **Implement**: `CancelRemaining` error handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_cancel_remaining.ori`
  - [ ] **LLVM Support**: LLVM codegen for CancelRemaining
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — CancelRemaining codegen

- [ ] **Implement**: `CollectAll` error handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_collect_all.ori`
  - [ ] **LLVM Support**: LLVM codegen for CollectAll
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — CollectAll codegen

- [ ] **Implement**: `FailFast` error handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_fail_fast.ori`
  - [ ] **LLVM Support**: LLVM codegen for FailFast
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — FailFast codegen

- [ ] **Implement**: Timeout support
  - [ ] **Rust Tests**: `oric/src/eval/nursery.rs` — timeout handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_timeout.ori`
  - [ ] **LLVM Support**: LLVM codegen for nursery timeout
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — timeout codegen

- [ ] **Implement**: Return type `[Result<T, E>]`
  - [ ] **Ori Tests**: `tests/spec/concurrency/nursery_results.ori`
  - [ ] **LLVM Support**: LLVM codegen for nursery results
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — results codegen

---

## 17.6 Parallel Execution Guarantees

**Proposal**: `proposals/approved/parallel-execution-guarantees-proposal.md`

Specifies execution guarantees for the `parallel` pattern: task ordering, concurrency limits, resource exhaustion, and timeout behavior.

### Implementation

- [ ] **Implement**: Start order guarantee (tasks start in list order)
  - [ ] **Rust Tests**: `oric/src/eval/parallel.rs` — start order verification
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_start_order.ori`
  - [ ] **LLVM Support**: LLVM codegen for ordered start
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — start order codegen

- [ ] **Implement**: Result order guarantee (results match input order)
  - [ ] **Rust Tests**: `oric/src/eval/parallel.rs` — result order verification
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_result_order.ori`
  - [ ] **LLVM Support**: LLVM codegen for result ordering
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — result order codegen

- [ ] **Implement**: `max_concurrent: Option<int>` parameter
  - [ ] **Rust Tests**: `oric/src/eval/parallel.rs` — concurrency limit
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_max_concurrent.ori`
  - [ ] **LLVM Support**: LLVM codegen for concurrency limit
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — max_concurrent codegen

- [ ] **Implement**: `timeout: Option<Duration>` parameter
  - [ ] **Rust Tests**: `oric/src/eval/parallel.rs` — timeout handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_timeout.ori`
  - [ ] **LLVM Support**: LLVM codegen for parallel timeout
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — timeout codegen

- [ ] **Implement**: Resource exhaustion error handling
  - [ ] **Rust Tests**: `oric/src/eval/parallel.rs` — resource exhaustion
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_resource_exhaustion.ori`
  - [ ] **LLVM Support**: LLVM codegen for resource exhaustion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — resource exhaustion codegen

- [ ] **Implement**: Empty task list handling (returns `[]` immediately)
  - [ ] **Rust Tests**: `oric/src/eval/parallel.rs` — empty list
  - [ ] **Ori Tests**: `tests/spec/concurrency/parallel_empty.ori`
  - [ ] **LLVM Support**: LLVM codegen for empty parallel
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/parallel_tests.rs` — empty codegen

---

## 17.7 Nursery Cancellation Semantics

**Proposal**: `proposals/approved/nursery-cancellation-proposal.md`

Specifies cooperative cancellation model, checkpoints, error mode behaviors, and cleanup guarantees.

### Implementation

- [ ] **Implement**: Cooperative cancellation model
  - [ ] **Rust Tests**: `oric/src/eval/cancellation.rs` — cooperative cancellation
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellation_cooperative.ori`
  - [ ] **LLVM Support**: LLVM codegen for cancellation state
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — cancellation codegen

- [ ] **Implement**: Cancellation checkpoints (suspension, loop iteration, pattern entry)
  - [ ] **Rust Tests**: `oric/src/eval/cancellation.rs` — checkpoint detection
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellation_checkpoints.ori`
  - [ ] **LLVM Support**: LLVM codegen for checkpoint insertion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — checkpoint codegen

- [ ] **Implement**: `CancellationError` type
  - [ ] **Rust Tests**: `oric/src/types/cancellation.rs` — CancellationError type
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellation_error.ori`
  - [ ] **LLVM Support**: LLVM codegen for CancellationError
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — CancellationError codegen

- [ ] **Implement**: `CancellationReason` sum type (Timeout, SiblingFailed, NurseryExited, ExplicitCancel, ResourceExhausted)
  - [ ] **Rust Tests**: `oric/src/types/cancellation.rs` — CancellationReason type
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellation_reason.ori`
  - [ ] **LLVM Support**: LLVM codegen for CancellationReason
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — CancellationReason codegen

- [ ] **Implement**: `is_cancelled()` built-in function
  - [ ] **Rust Tests**: `oric/src/eval/builtins.rs` — is_cancelled
  - [ ] **Ori Tests**: `tests/spec/concurrency/is_cancelled.ori`
  - [ ] **LLVM Support**: LLVM codegen for is_cancelled
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — is_cancelled codegen

- [ ] **Implement**: Automatic loop cancellation checking in async contexts
  - [ ] **Rust Tests**: `oric/src/eval/loops.rs` — automatic cancellation check
  - [ ] **Ori Tests**: `tests/spec/concurrency/loop_cancellation.ori`
  - [ ] **LLVM Support**: LLVM codegen for loop cancellation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — loop cancellation codegen

- [ ] **Implement**: Destructor execution guarantee during cancellation
  - [ ] **Rust Tests**: `oric/src/eval/cancellation.rs` — destructor guarantee
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellation_cleanup.ori`
  - [ ] **LLVM Support**: LLVM codegen for cancellation unwinding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — cleanup codegen

- [ ] **Implement**: Nested nursery cancellation propagation
  - [ ] **Rust Tests**: `oric/src/eval/nursery.rs` — nested cancellation
  - [ ] **Ori Tests**: `tests/spec/concurrency/nested_nursery_cancellation.ori`
  - [ ] **LLVM Support**: LLVM codegen for nested cancellation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/nursery_tests.rs` — nested cancellation codegen

---

## 17.8 Phase Completion Checklist

- [ ] All items in 17.1-17.7 have checkboxes marked `[x]`
- [ ] Spec updated: `spec/06-types.md` — Sendable, Producer, Consumer, CloneableProducer, CloneableConsumer, CancellationError, CancellationReason
- [ ] Spec updated: `spec/10-patterns.md` — nursery pattern, parallel execution guarantees
- [ ] Spec updated: `spec/23-concurrency-model.md` — cancellation model
- [ ] CLAUDE.md updated with channel constructors, nursery syntax, is_cancelled()
- [ ] Sendable trait working (auto-implemented)
- [ ] Role-based channels working (Producer/Consumer)
- [ ] Channel constructors working (channel, channel_in, channel_out, channel_all)
- [ ] Ownership transfer on send working
- [ ] nursery pattern working
- [ ] Parallel execution guarantees working (ordering, max_concurrent, timeout)
- [ ] Cancellation semantics working (cooperative, checkpoints, is_cancelled)
- [ ] All tests pass: `./test-all`

**Exit Criteria**: Can write producer/consumer pipeline with ownership safety and proper cancellation

---

## Example: Worker Pool with Fan-In

```ori
@worker_pool (jobs: [Job]) -> [Result<Output, Error>] uses Async = run(
    let (sender, receiver) = channel_in<Result<Output, Error>>(buffer: 100),

    nursery(
        body: n -> run(
            // Spawn workers with cloned senders
            for i in 0..4 do
                n.spawn(task: () -> worker(sender.clone(), i)),
            // Spawn job feeder
            n.spawn(task: () -> run(
                for job in jobs do sender.send(value: Ok(job)),
                sender.close(),
            )),
        ),
        on_error: CollectAll,
    ),

    // Collect results
    for result in receiver yield result,
)
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
