# Proposal: Sendable Trait and Role-Based Channels

**Status:** Approved
**Approved:** 2026-01-28
**Author:** Eric (with AI assistance)
**Created:** 2026-01-22
**Affects:** Type system, compiler, runtime, standard library

---

## Executive Summary

This proposal introduces compile-time safety for concurrent data transfer in Ori:

1. **Sendable trait** — Auto-implemented marker trait ensuring types can safely cross task boundaries
2. **Role-based channel types** — `Producer<T>` and `Consumer<T>` with compile-time enforcement of roles
3. **Ownership transfer on send** — Values are consumed when sent, preventing data races
4. **Channel variants** — Four channel constructors for different concurrency patterns
5. **nursery pattern** — Structured concurrency with guaranteed task completion

---

## Background: How Other Languages Fail

### The Shared Mutable State Problem

Most concurrency bugs stem from shared mutable state:

| Language | Approach | Failure Mode |
|----------|----------|--------------|
| Java | Locks/synchronized | Deadlocks, forgotten locks |
| C++ | Mutexes, atomics | Data races, undefined behavior |
| Go | Channels | Channels can share pointers |
| JavaScript | Single-threaded | Callback hell |

### Go's Partial Solution

Go's approach: "Don't communicate by sharing memory; share memory by communicating."

**Where Go fails:**
- Channels can send pointers (no ownership transfer)
- Race detector is runtime only, not compile-time
- Nothing prevents sending references to shared memory

```go
// Go allows this dangerous pattern
func bad() {
    data := make([]int, 100)
    ch := make(chan []int)
    go func() { ch <- data }()  // sends reference
    data[0] = 42                 // race condition!
    received := <-ch
}
```

**Citation:** [A Study of Real-World Data Races in Golang](https://arxiv.org/pdf/2204.00764)

### Rust's Compile-Time Approach

Rust's `Send` and `Sync` traits provide compile-time safety:

**What Rust gets right:**
- `Send`: Type can be transferred between threads
- `Sync`: Type can be shared between threads
- Ownership prevents shared mutable state

**Where Rust could be better:**
- Channel types don't enforce producer/consumer roles
- Both ends can be cloned by default

**Citation:** [Fearless Concurrency - The Rust Programming Language](https://doc.rust-lang.org/book/ch16-00-concurrency.html)

### Ori's Position

Ori already has advantages:
- Closures capture by value (no reference cycles)
- No shared mutable references
- `Async` capability makes suspension explicit

This proposal builds on these foundations to provide **compile-time data race freedom** with **role enforcement**.

---

## Part 1: Sendable Trait

### Definition

```ori
// Marker trait — no methods
trait Sendable {}
```

`Sendable` indicates a type can safely cross task boundaries.

### Auto-Implementation

`Sendable` is automatically implemented when ALL conditions are met:

1. All fields are `Sendable`
2. No interior mutability
3. No non-Sendable captured state (for closures)

```ori
// Automatically Sendable
type Point = { x: int, y: int }     // All fields are int (Sendable)
type Config = { host: str, port: int }  // str and int are Sendable

// NOT Sendable (hypothetical examples)
type Handle = { file: FileHandle }  // FileHandle is not Sendable
```

### Standard Sendable Types

| Type | Sendable |
|------|----------|
| `int`, `float`, `bool`, `str`, `char`, `byte` | Yes |
| `Duration`, `Size` | Yes |
| `[T]` where `T: Sendable` | Yes |
| `{K: V}` where `K: Sendable, V: Sendable` | Yes |
| `Set<T>` where `T: Sendable` | Yes |
| `Option<T>` where `T: Sendable` | Yes |
| `Result<T, E>` where `T: Sendable, E: Sendable` | Yes |
| `(T1, T2, ...)` where all `Ti: Sendable` | Yes |
| `(T) -> R` where captures are `Sendable` | Yes |

### Non-Sendable Types

Some types are inherently not `Sendable`:
- Unique resources (file handles, network connections)
- Types with identity semantics
- Types containing non-Sendable fields

---

## Part 2: Role-Based Channel Types

### Motivation

Current `Channel<T>` allows any holder to send or receive, making it easy to accidentally:
- Create multiple consumers in a single-consumer design
- Have a producer accidentally receive
- Violate design invariants at runtime

### Producer and Consumer Types

```ori
// Role-specific types
type Producer<T: Sendable> = { ... }  // Can only send
type Consumer<T: Sendable> = { ... }  // Can only receive

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
impl<T: Sendable> Iterable for Consumer<T> {
    type Item = T
    @iter (self) -> impl Iterator where Item == T
}
```

### Compile-Time Role Enforcement

```ori
@produce (p: Producer<int>) -> void uses Async = run(
    p.send(value: 42),
    // p.receive()  // ERROR: Producer<T> has no method 'receive'
)

@consume (c: Consumer<int>) -> [int] uses Async =
    for item in c yield item
    // c.send(1)  // ERROR: Consumer<T> has no method 'send'
```

---

## Part 3: Channel Constructors

Four constructors for different concurrency patterns:

### channel — One-to-One (Exclusive)

```ori
@channel<T: Sendable> (buffer: int) -> (Producer<T>, Consumer<T>)
```

Default channel with single producer, single consumer. Fastest, neither end is cloneable.

```ori
let (producer, consumer) = channel<int>(buffer: 10)
// producer.clone()  // ERROR: Producer<T> does not implement Clone
// consumer.clone()  // ERROR: Consumer<T> does not implement Clone
```

### channel_in — Fan-In (Many-to-One)

```ori
@channel_in<T: Sendable> (buffer: int) -> (CloneableProducer<T>, Consumer<T>)
```

Multiple producers can send to a single consumer. Producer is cloneable.

```ori
let (producer, consumer) = channel_in<Result>(buffer: 100)

parallel(
    tasks: (0..4).map(i -> run(
        let p = producer.clone(),  // OK: CloneableProducer implements Clone
        worker(p, i),
    )).collect(),
)

// consumer.clone()  // ERROR: Consumer<T> does not implement Clone
```

### channel_out — Fan-Out (One-to-Many)

```ori
@channel_out<T: Sendable> (buffer: int) -> (Producer<T>, CloneableConsumer<T>)
```

Single producer sends to multiple consumers. Consumer is cloneable.

```ori
let (producer, consumer) = channel_out<Task>(buffer: 100)

parallel(
    tasks: (0..4).map(i -> run(
        let c = consumer.clone(),  // OK: CloneableConsumer implements Clone
        worker(c, i),
    )).collect(),
)

// producer.clone()  // ERROR: Producer<T> does not implement Clone
```

### channel_all — Many-to-Many (Broadcast)

```ori
@channel_all<T: Sendable> (buffer: int) -> (CloneableProducer<T>, CloneableConsumer<T>)
```

Multiple producers and multiple consumers. Both ends cloneable.

```ori
let (producer, consumer) = channel_all<Message>(buffer: 100)

let p1 = producer.clone()
let p2 = producer.clone()
let c1 = consumer.clone()
let c2 = consumer.clone()
```

### Type Relationships

```ori
// CloneableProducer is a Producer that implements Clone
type CloneableProducer<T: Sendable> = Producer<T>  // + Clone impl

// CloneableConsumer is a Consumer that implements Clone
type CloneableConsumer<T: Sendable> = Consumer<T>  // + Clone impl

// Clone implementations
impl<T: Sendable> Clone for CloneableProducer<T> { ... }
impl<T: Sendable> Clone for CloneableConsumer<T> { ... }
```

---

## Part 4: Ownership Transfer on Send

### Motivation

Go allows sending pointers while retaining the original, causing data races. Ori prevents this.

### Semantics

Sending a value **consumes** it:

```ori
@producer (p: Producer<Data>) -> void uses Async = run(
    let data = create_data(),
    p.send(value: data),  // Ownership transferred
    // data.field         // ERROR: 'data' moved into channel
)
```

### Explicit Copy

To retain access, explicitly clone:

```ori
@producer (p: Producer<Data>) -> void uses Async = run(
    let data = create_data(),
    p.send(value: data.clone()),  // Send a copy
    print(msg: data.field),       // Original still accessible
)
```

### Why This Works

Ori's memory model ensures:
1. Closures capture by value — no reference sharing through environments
2. No shared mutable references — single ownership of mutable data
3. Types cannot be self-referential — no cycles possible

With ownership transfer on send, data races through channels become impossible.

---

## Part 5: nursery Pattern

### Motivation

Fire-and-forget concurrency (like Go's goroutines) creates orphan tasks. Structured concurrency ensures:
- All spawned tasks complete before the nursery exits
- Errors propagate properly
- No resource leaks from abandoned tasks

### Syntax

```ori
nursery(
    body: n -> expression,
    on_error: ErrorMode,
    timeout: Duration,
)
```

### Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `body` | `Nursery -> T` | Lambda that spawns tasks using the nursery |
| `on_error` | `NurseryErrorMode` | How to handle task failures |
| `timeout` | `Duration` | Maximum time for all tasks (optional) |

### Error Modes

```ori
type NurseryErrorMode = CancelRemaining | CollectAll | FailFast
```

| Mode | Behavior |
|------|----------|
| `CancelRemaining` | On first error, cancel pending tasks, return results so far |
| `CollectAll` | Wait for all tasks regardless of errors |
| `FailFast` | On first error, cancel all and return immediately |

### Return Type

```ori
nursery(...) -> [Result<T, E>]
```

Returns results of all spawned tasks, in spawn order.

### Nursery Methods

```ori
type Nursery = {
    @spawn<T> (self, task: () -> T uses Async) -> void
}
```

### Examples

```ori
// Process batch with error handling
@process_batch (items: [Item]) -> [Result<Output, Error>] uses Async =
    nursery(
        body: n -> for item in items do n.spawn(task: () -> process(item)),
        on_error: CollectAll,
        timeout: 30s,
    )

// Fan-out with early termination
@find_first (queries: [Query]) -> Option<Result> uses Async = run(
    let results = nursery(
        body: n -> for q in queries do n.spawn(task: () -> search(q)),
        on_error: CancelRemaining,
    ),
    results.find(predicate: r -> r.is_ok()).and_then(transform: r -> r.ok()),
)
```

### Guarantees

1. **No orphan tasks** — All spawned tasks complete or cancel before nursery returns
2. **Error propagation** — Task failures are captured and returned
3. **Resource cleanup** — Tasks are cancelled on timeout or error (per mode)
4. **Scoped concurrency** — Cannot escape nursery scope with spawned tasks

---

## Part 6: Examples

### Safe Producer-Consumer

```ori
@main () -> void uses Async = run(
    let (producer, consumer) = channel<Job>(buffer: 100),

    parallel(
        tasks: [
            job_producer(producer),
            job_consumer(consumer),
        ],
    ),

    print(msg: "All jobs processed"),
)

@job_producer (p: Producer<Job>) -> void uses Async = run(
    for job in load_jobs() do p.send(value: job),
    p.close(),
)

@job_consumer (c: Consumer<Job>) -> void uses Async =
    for job in c do process(job)
```

### Worker Pool (Fan-In)

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

### Pipeline with Backpressure

```ori
@data_pipeline (input: Consumer<RawData>) -> [ProcessedData] uses Async = run(
    let (stage1_out, stage1_in) = channel<Parsed>(buffer: 10),
    let (stage2_out, stage2_in) = channel<Validated>(buffer: 10),

    nursery(
        body: n -> run(
            n.spawn(task: () -> pipe(input, stage1_out, parse)),
            n.spawn(task: () -> pipe(stage1_in, stage2_out, validate)),
            n.spawn(task: () -> for item in stage2_in yield transform(item)),
        ),
        on_error: FailFast,
    ),
)

@pipe<A, B> (input: Consumer<A>, output: Producer<B>, f: (A) -> B) -> void uses Async =
    for item in input do output.send(value: f(item))
```

---

## Future Work

This proposal establishes the foundation for safe concurrency. Future proposals may address:

- **Process isolation** — Multi-process parallelism with memory isolation
- **Fine-grained async capabilities** — `Parallel`, `AsyncIO` distinctions
- **Select with priorities** — Channel multiplexing
- **Cancellation tokens** — Cooperative cancellation

These are explicitly deferred to keep this proposal focused.

---

## Implementation

### Phase 1: Type System

1. Add `Sendable` trait to type system
2. Implement auto-derivation rules
3. Add compiler error for non-Sendable in channel context

### Phase 2: Channel Types

4. Implement `Producer<T>`, `Consumer<T>` types
5. Implement `CloneableProducer<T>`, `CloneableConsumer<T>`
6. Add `channel`, `channel_in`, `channel_out`, `channel_all` constructors
7. Deprecate old `Channel<T>` type

### Phase 3: Ownership Transfer

8. Add move semantics on `send`
9. Add compiler error for use-after-send

### Phase 4: nursery Pattern

10. Add `nursery` pattern to compiler
11. Implement `NurseryErrorMode`
12. Add timeout support

---

## Migration

The existing `Channel<T>` type will be deprecated. Migration path:

```ori
// Old
let ch = Channel<int> { buffer: 10 }
ch.send(42)
let v = ch.receive()

// New
let (producer, consumer) = channel<int>(buffer: 10)
producer.send(value: 42)
let v = consumer.receive()
```

The compiler will provide migration suggestions.

---

## References

- [A Study of Real-World Data Races in Golang](https://arxiv.org/pdf/2204.00764)
- [Fearless Concurrency - The Rust Programming Language](https://doc.rust-lang.org/book/ch16-00-concurrency.html)
- [Notes on structured concurrency](https://vorpus.org/blog/notes-on-structured-concurrency-or-go-statement-considered-harmful/) - Nathaniel J. Smith

---

## Changelog

- 2026-01-22: Initial draft
- 2026-01-28: Approved with scope reduction and design decisions
