# Phase 17: Concurrency Extended

**Goal**: Complete concurrency support with select, cancellation, and enhanced channels

> **PROPOSAL**: `docs/ori_lang/proposals/approved/parallel-concurrency-proposal.md`

**Dependencies**: Phase 16 (Async Support)

---

## 17.1 Sendable Trait

Types that can safely cross task boundaries.

```ori
trait Sendable {
    // Marker trait - no methods
}

// Auto-derived for primitives and immutable collections
// Channel constraint: T: Sendable
```

### Implementation

- [ ] **Implement**: Add `Sendable` marker trait
  - [ ] **Rust Tests**: `oric/src/typeck/traits/sendable.rs` — sendable trait
  - [ ] **Ori Tests**: `tests/spec/concurrency/sendable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Sendable marker trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sendable_tests.rs` — Sendable trait codegen

- [ ] **Implement**: Auto-derive for primitives and immutable collections
  - [ ] **LLVM Support**: LLVM codegen for Sendable auto-derive
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sendable_tests.rs` — Sendable auto-derive codegen

- [ ] **Implement**: Channel constraint: `T: Sendable`
  - [ ] **LLVM Support**: LLVM codegen for channel Sendable constraint
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sendable_tests.rs` — channel Sendable constraint codegen

---

## 17.2 Role-Based Channel Types

Split channels into producer/consumer roles.

```ori
type ChannelPair<T> = {
    producer: Producer<T>,
    consumer: Consumer<T>,
}

let (tx, rx) = channel<int>(capacity: 10)
send(tx, 42)
let value = recv(rx)
```

### Implementation

- [ ] **Implement**: Split `Channel<T>` into `Producer<T>` and `Consumer<T>`
  - [ ] **Rust Tests**: `oric/src/eval/channel.rs` — role-based channels
  - [ ] **Ori Tests**: `tests/spec/concurrency/channels.ori`
  - [ ] **LLVM Support**: LLVM codegen for Producer/Consumer channel types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — role-based channels codegen

- [ ] **Implement**: Add `Sharing` enum: `Exclusive | Producers | Consumers | Both`
  - [ ] **LLVM Support**: LLVM codegen for Sharing enum
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — Sharing enum codegen

- [ ] **Implement**: Update `channel()` to return `ChannelPair<T>`
  - [ ] **LLVM Support**: LLVM codegen for channel() returning ChannelPair
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — ChannelPair codegen

- [ ] **Implement**: Enforce clonability based on sharing mode
  - [ ] **LLVM Support**: LLVM codegen for clonability based on sharing mode
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — sharing mode clonability codegen

- [ ] **Implement**: Ownership transfer on send (value consumed)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/ownership.rs` — channel ownership
  - [ ] **Ori Tests**: `tests/compile-fail/channel_ownership.ori`
  - [ ] **LLVM Support**: LLVM codegen for ownership transfer on channel send
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — ownership transfer codegen

---

## 17.3 Channel Select

Multiplex over multiple channel operations.

```ori
let result = select(
    recv(ch1) -> value: handle_ch1(value: value),
    recv(ch2) -> value: handle_ch2(value: value),
    send(ch3, message) -> void: handle_sent(),
)
```

### Basic Select

- [ ] **Implement**: `select` expression parsing
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — select parsing
  - [ ] **Ori Tests**: `tests/spec/concurrency/select_basic.ori`
  - [ ] **LLVM Support**: LLVM codegen for select expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/select_tests.rs` — select expression codegen

- [ ] **Implement**: `recv(channel) -> pattern: expr` arm
  - [ ] **LLVM Support**: LLVM codegen for recv arm in select
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/select_tests.rs` — recv arm codegen

- [ ] **Implement**: `send(channel, value) -> void: expr` arm
  - [ ] **LLVM Support**: LLVM codegen for send arm in select
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/select_tests.rs` — send arm codegen

- [ ] **Implement**: Type checking (all arms same return type)
  - [ ] **LLVM Support**: LLVM codegen for select type unification
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/select_tests.rs` — select type unification codegen

- [ ] **Implement**: Runtime select with fairness (pseudo-random when multiple ready)
  - [ ] **LLVM Support**: LLVM codegen for runtime select with fairness
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/select_tests.rs` — runtime select fairness codegen

### Default Case

- [ ] **Implement**: `else: expr` for non-blocking select
  - [ ] **Rust Tests**: `oric/src/eval/exec/select.rs` — default case
  - [ ] **Ori Tests**: `tests/spec/concurrency/select_default.ori`
  - [ ] **LLVM Support**: LLVM codegen for non-blocking select default case
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/select_tests.rs` — non-blocking select codegen

### Timeout Integration

- [ ] **Implement**: `after(duration): expr` for timed select
  - [ ] **Rust Tests**: `oric/src/eval/exec/select.rs` — timeout arm
  - [ ] **Ori Tests**: `tests/spec/concurrency/select_timeout.ori`
  - [ ] **LLVM Support**: LLVM codegen for timed select with after
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/select_tests.rs` — timed select codegen

### Closed Channel Handling

- [ ] **Implement**: `recv` returns `Option<T>` (None when closed)
  - [ ] **Rust Tests**: `oric/src/eval/channel.rs` — closed channel handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/select_closed.ori`
  - [ ] **LLVM Support**: LLVM codegen for recv returning Option on closed channel
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — closed channel recv codegen

---

## 17.4 Cancellation Tokens

Structured cancellation for async operations.

```ori
let token = CancellationToken.new()
let child = token.child()

// Cancel all
token.cancel()

// Check
if token.is_cancelled() then ...

// Wait
token.wait()

// In select
select(
    recv(token.channel()) -> _: cancelled(),
    recv(data_ch) -> data: process(data: data),
)
```

### Implementation

- [ ] **Implement**: `CancellationToken` type in stdlib
  - [ ] **Rust Tests**: `library/std/cancellation.rs` — cancellation token
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellation_tokens.ori`
  - [ ] **LLVM Support**: LLVM codegen for CancellationToken type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — CancellationToken codegen

- [ ] **Implement**: `new()`, `child()`, `cancel()`, `is_cancelled()`
  - [ ] **LLVM Support**: LLVM codegen for CancellationToken methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — CancellationToken methods codegen

- [ ] **Implement**: `wait()` - blocks until cancelled
  - [ ] **LLVM Support**: LLVM codegen for CancellationToken wait()
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — wait() codegen

- [ ] **Implement**: `channel()` - returns channel for select
  - [ ] **LLVM Support**: LLVM codegen for CancellationToken channel()
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — channel() for select codegen

- [ ] **Implement**: Parent-child propagation (cancel parent → cancel children)
  - [ ] **LLVM Support**: LLVM codegen for parent-child cancellation propagation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — propagation codegen

---

## 17.5 Cancellable Operations

Cooperative cancellation in async code.

```ori
@fetch_data (url: str, cancel: CancellationToken) -> Result<Data, Error> uses Http, Async = run(
    if cancel.is_cancelled() then
        return Err(CancelledError {})

    let response = http_get(url: url)

    if cancel.is_cancelled() then
        return Err(CancelledError {})

    parse_response(response: response)
)
```

### Implementation

- [ ] **Implement**: `CancelledError` type in stdlib
  - [ ] **Rust Tests**: `library/std/error.rs` — cancelled error type
  - [ ] **Ori Tests**: `tests/spec/concurrency/cancellable_ops.ori`
  - [ ] **LLVM Support**: LLVM codegen for CancelledError type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — CancelledError codegen

- [ ] **Implement**: Document cooperative checking patterns
  - [ ] **LLVM Support**: LLVM codegen for cooperative cancellation checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — cooperative checking codegen

---

## 17.6 Automatic Propagation

Integration with parallel/spawn patterns.

```ori
parallel(
    tasks: [task1, task2, task3],
    cancel: token,  // If cancelled, all tasks receive cancellation
)
```

### Implementation

- [ ] **Implement**: Add `cancel:` parameter to `parallel` pattern
  - [ ] **Rust Tests**: `oric/src/patterns/parallel.rs` — cancel parameter
  - [ ] **Ori Tests**: `tests/spec/concurrency/propagation.ori`
  - [ ] **LLVM Support**: LLVM codegen for parallel cancel parameter
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — parallel cancel codegen

- [ ] **Implement**: Add `cancel:` parameter to `spawn` pattern
  - [ ] **LLVM Support**: LLVM codegen for spawn cancel parameter
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — spawn cancel codegen

- [ ] **Implement**: Propagate cancellation to child tasks
  - [ ] **LLVM Support**: LLVM codegen for child task cancellation propagation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — child task propagation codegen

---

## 17.7 Cleanup on Cancellation

Guaranteed cleanup even when cancelled.

```ori
with(
    acquire: open_file(path: path),
    use: file -> process(file: file, cancel: cancel),
    release: file -> close_file(file: file),  // Always runs
)
```

### Implementation

- [ ] **Implement**: `with` release runs on cancellation
  - [ ] **Rust Tests**: `oric/src/patterns/with.rs` — cancellation cleanup
  - [ ] **Ori Tests**: `tests/spec/concurrency/cleanup.ori`
  - [ ] **LLVM Support**: LLVM codegen for with pattern cancellation cleanup
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — with cleanup codegen

- [ ] **Implement**: Document cleanup patterns
  - [ ] **LLVM Support**: LLVM codegen for cleanup pattern documentation examples
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — cleanup patterns codegen

---

## 17.8 Timeout as Cancellation

```ori
let token = CancellationToken.with_timeout(duration: 30s)
fetch_data(url: url, cancel: token)
```

### Implementation

- [ ] **Implement**: `CancellationToken.with_timeout(duration:)`
  - [ ] **Rust Tests**: `library/std/cancellation.rs` — timeout-based cancel
  - [ ] **Ori Tests**: `tests/spec/concurrency/timeout_cancel.ori`
  - [ ] **LLVM Support**: LLVM codegen for CancellationToken.with_timeout
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — with_timeout codegen

- [ ] **Implement**: Auto-cancel on timeout expiry
  - [ ] **LLVM Support**: LLVM codegen for auto-cancel on timeout
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/cancellation_tests.rs` — auto-cancel timeout codegen

---

## 17.9 Graceful Shutdown Pattern

```ori
@main () -> void uses Async = run(
    let shutdown = CancellationToken.new()

    on_signal(signal: SIGINT, handler: () -> shutdown.cancel())

    run_server(cancel: shutdown)
)
```

### Implementation

- [ ] **Implement**: `on_signal` in stdlib
  - [ ] **Rust Tests**: `library/std/signal.rs` — signal handling
  - [ ] **Ori Tests**: `tests/spec/concurrency/graceful_shutdown.ori`
  - [ ] **LLVM Support**: LLVM codegen for on_signal
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — on_signal codegen

- [ ] **Implement**: SIGINT, SIGTERM support
  - [ ] **LLVM Support**: LLVM codegen for SIGINT/SIGTERM handling
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/concurrency_tests.rs` — signal handling codegen

---

## 17.10 Phase Completion Checklist

- [ ] All items above have checkboxes marked `[x]`
- [ ] Spec updated: `spec/06-types.md` channel types, concurrency patterns
- [ ] CLAUDE.md updated with select/cancellation syntax
- [ ] Sendable trait working
- [ ] Role-based channels working
- [ ] Select expression working
- [ ] Cancellation tokens working
- [ ] Propagation working
- [ ] All tests pass: `./test-all`

**Exit Criteria**: Can write a server with graceful shutdown on SIGINT

---

## Example: Chat Server with Graceful Shutdown

```ori
type Message = { from: str, content: str }

@chat_server (
    messages: Consumer<Message>,
    shutdown: CancellationToken,
) -> void uses Async = run(
    loop(
        select(
            recv(shutdown.channel()) -> _: break void,

            recv(messages) -> msg_opt: match(msg_opt,
                Some(msg) -> broadcast(msg: msg),
                None -> break void,
            ),

            after(30s): health_check(),
        )
    )
)

@main () -> void uses Async = run(
    let shutdown = CancellationToken.new()
    on_signal(signal: SIGINT, handler: () -> shutdown.cancel())

    let (tx, rx) = channel<Message>(capacity: 100)

    parallel(
        tasks: [
            chat_server(messages: rx, shutdown: shutdown),
            message_producer(output: tx, shutdown: shutdown),
        ],
        cancel: shutdown,
    )

    print("Server stopped gracefully")
)
```
