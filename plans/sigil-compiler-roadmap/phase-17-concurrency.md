# Phase 17: Concurrency Extended

**Goal**: Complete concurrency support with select, cancellation, and enhanced channels

> **PROPOSAL**: `docs/sigil_lang/proposals/approved/parallel-concurrency-proposal.md`

**Dependencies**: Phase 16 (Async Support)

---

## 17.1 Sendable Trait

Types that can safely cross task boundaries.

```sigil
trait Sendable {
    // Marker trait - no methods
}

// Auto-derived for primitives and immutable collections
// Channel constraint: T: Sendable
```

### Implementation

- [ ] **Implement**: Add `Sendable` marker trait
  - [ ] **Rust Tests**: `sigilc/src/typeck/traits/sendable.rs` — sendable trait
  - [ ] **Sigil Tests**: `tests/spec/concurrency/sendable.si`

- [ ] **Implement**: Auto-derive for primitives and immutable collections

- [ ] **Implement**: Channel constraint: `T: Sendable`

---

## 17.2 Role-Based Channel Types

Split channels into producer/consumer roles.

```sigil
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
  - [ ] **Rust Tests**: `sigilc/src/eval/channel.rs` — role-based channels
  - [ ] **Sigil Tests**: `tests/spec/concurrency/channels.si`

- [ ] **Implement**: Add `Sharing` enum: `Exclusive | Producers | Consumers | Both`

- [ ] **Implement**: Update `channel()` to return `ChannelPair<T>`

- [ ] **Implement**: Enforce clonability based on sharing mode

- [ ] **Implement**: Ownership transfer on send (value consumed)
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/ownership.rs` — channel ownership
  - [ ] **Sigil Tests**: `tests/compile-fail/channel_ownership.si`

---

## 17.3 Channel Select

Multiplex over multiple channel operations.

```sigil
let result = select(
    recv(ch1) -> value: handle_ch1(value: value),
    recv(ch2) -> value: handle_ch2(value: value),
    send(ch3, message) -> void: handle_sent(),
)
```

### Basic Select

- [ ] **Implement**: `select` expression parsing
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — select parsing
  - [ ] **Sigil Tests**: `tests/spec/concurrency/select_basic.si`

- [ ] **Implement**: `recv(channel) -> pattern: expr` arm

- [ ] **Implement**: `send(channel, value) -> void: expr` arm

- [ ] **Implement**: Type checking (all arms same return type)

- [ ] **Implement**: Runtime select with fairness (pseudo-random when multiple ready)

### Default Case

- [ ] **Implement**: `else: expr` for non-blocking select
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/select.rs` — default case
  - [ ] **Sigil Tests**: `tests/spec/concurrency/select_default.si`

### Timeout Integration

- [ ] **Implement**: `after(duration): expr` for timed select
  - [ ] **Rust Tests**: `sigilc/src/eval/exec/select.rs` — timeout arm
  - [ ] **Sigil Tests**: `tests/spec/concurrency/select_timeout.si`

### Closed Channel Handling

- [ ] **Implement**: `recv` returns `Option<T>` (None when closed)
  - [ ] **Rust Tests**: `sigilc/src/eval/channel.rs` — closed channel handling
  - [ ] **Sigil Tests**: `tests/spec/concurrency/select_closed.si`

---

## 17.4 Cancellation Tokens

Structured cancellation for async operations.

```sigil
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
  - [ ] **Sigil Tests**: `tests/spec/concurrency/cancellation_tokens.si`

- [ ] **Implement**: `new()`, `child()`, `cancel()`, `is_cancelled()`

- [ ] **Implement**: `wait()` - blocks until cancelled

- [ ] **Implement**: `channel()` - returns channel for select

- [ ] **Implement**: Parent-child propagation (cancel parent → cancel children)

---

## 17.5 Cancellable Operations

Cooperative cancellation in async code.

```sigil
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
  - [ ] **Sigil Tests**: `tests/spec/concurrency/cancellable_ops.si`

- [ ] **Implement**: Document cooperative checking patterns

---

## 17.6 Automatic Propagation

Integration with parallel/spawn patterns.

```sigil
parallel(
    tasks: [task1, task2, task3],
    cancel: token,  // If cancelled, all tasks receive cancellation
)
```

### Implementation

- [ ] **Implement**: Add `cancel:` parameter to `parallel` pattern
  - [ ] **Rust Tests**: `sigilc/src/patterns/parallel.rs` — cancel parameter
  - [ ] **Sigil Tests**: `tests/spec/concurrency/propagation.si`

- [ ] **Implement**: Add `cancel:` parameter to `spawn` pattern

- [ ] **Implement**: Propagate cancellation to child tasks

---

## 17.7 Cleanup on Cancellation

Guaranteed cleanup even when cancelled.

```sigil
with(
    acquire: open_file(path: path),
    use: file -> process(file: file, cancel: cancel),
    release: file -> close_file(file: file),  // Always runs
)
```

### Implementation

- [ ] **Implement**: `with` release runs on cancellation
  - [ ] **Rust Tests**: `sigilc/src/patterns/with.rs` — cancellation cleanup
  - [ ] **Sigil Tests**: `tests/spec/concurrency/cleanup.si`

- [ ] **Implement**: Document cleanup patterns

---

## 17.8 Timeout as Cancellation

```sigil
let token = CancellationToken.with_timeout(duration: 30s)
fetch_data(url: url, cancel: token)
```

### Implementation

- [ ] **Implement**: `CancellationToken.with_timeout(duration:)`
  - [ ] **Rust Tests**: `library/std/cancellation.rs` — timeout-based cancel
  - [ ] **Sigil Tests**: `tests/spec/concurrency/timeout_cancel.si`

- [ ] **Implement**: Auto-cancel on timeout expiry

---

## 17.9 Graceful Shutdown Pattern

```sigil
@main () -> void uses Async = run(
    let shutdown = CancellationToken.new()

    on_signal(signal: SIGINT, handler: () -> shutdown.cancel())

    run_server(cancel: shutdown)
)
```

### Implementation

- [ ] **Implement**: `on_signal` in stdlib
  - [ ] **Rust Tests**: `library/std/signal.rs` — signal handling
  - [ ] **Sigil Tests**: `tests/spec/concurrency/graceful_shutdown.si`

- [ ] **Implement**: SIGINT, SIGTERM support

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
- [ ] All tests pass: `cargo test && sigil test tests/spec/concurrency/`

**Exit Criteria**: Can write a server with graceful shutdown on SIGINT

---

## Example: Chat Server with Graceful Shutdown

```sigil
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
