# Channels

This document covers Sigil's channel system: typed channels for communication, send/receive operations, bounded buffers, and why Sigil has no shared mutable state.

---

## Overview

Channels are Sigil's mechanism for communication between concurrent tasks. They are typed, bounded, and the only way to share data between tasks.

```sigil
@producer (ch: Channel<int>) -> void uses Async =
    for i in 0..10 do ch.send(i)

@consumer (ch: Channel<int>) -> [int] uses Async = collect(
    ch.receive(),
    .until: ch.closed
)

@main () -> void uses Async = run(
    let ch = Channel<int>.new(buffer: 5),
    parallel(
        .producer: producer(ch),
        .consumer: consumer(ch),
    ),
)
```

Key principles:
- Channels are typed: `Channel<T>`
- All channels are bounded (have a buffer limit)
- Communication is explicit via send/receive
- No shared mutable state (no locks, no mutexes)

---

## Creating Channels

### Basic Channel Creation

```sigil
// Channel with buffer size 10
ch = Channel<int>.new(buffer: 10)

// Channel with buffer size 1 (synchronous-ish)
ch = Channel<str>.new(buffer: 1)

// Unbuffered channel (rendezvous)
ch = Channel<Data>.new(buffer: 0)
```

### Channel Types

The type parameter determines what can be sent:

```sigil
// Channel of integers
int_ch = Channel<int>.new(buffer: 10)

// Channel of structs
user_ch = Channel<User>.new(buffer: 5)

// Channel of results
result_ch = Channel<Result<Data, Error>>.new(buffer: 10)
```

### Why Bounded Buffers?

Unbounded buffers can cause:
- Memory exhaustion if producer is faster than consumer
- Hidden backpressure problems
- Unpredictable latency

Sigil requires explicit buffer sizes:

```sigil
// Good: explicit buffer size
ch = Channel<int>.new(buffer: 100)

// Not allowed: unbounded channel
ch = Channel<int>.new()  // ERROR: buffer size required
```

---

## Sending and Receiving

### The `send` Operation

```sigil
// Send a value - blocks if buffer is full
ch.send(value)

// Returns void, cannot fail (unless channel is closed)
```

### The `receive` Operation

```sigil
// Receive a value - blocks if buffer is empty
value = ch.receive()

// Returns Option<T> - None if channel is closed
```

### Example: Producer-Consumer

```sigil
@producer (ch: Channel<int>) -> void uses Async = run(
    for i in 1..100 do ch.send(i),
    ch.close(),  // signal no more values
)

@consumer (ch: Channel<int>) -> int uses Async = run(
    let sum = 0,
    loop(
        match(ch.receive(),
            Some(value) -> sum = sum + value,
            None -> break,  // channel closed
        ),
    ),
    sum,
)
```

### Closing Channels

```sigil
// Close the channel - no more sends allowed
ch.close()

// After close:
ch.send(value)  // ERROR: channel closed
ch.receive()    // Returns None
```

---

## Channel Patterns

### Fan-Out: One Producer, Multiple Consumers

```sigil
@distribute_work (items: [Item]) -> [Result<ProcessedItem, Error>] uses Async = run(
    let work_ch = Channel<Item>.new(buffer: 100),
    let result_ch = Channel<Result<ProcessedItem, Error>>.new(buffer: 100),

    // Start workers
    let workers = parallel(
        .tasks: map(0..$worker_count, _ -> worker(work_ch, result_ch)),
    ),

    // Send work
    for item in items do work_ch.send(item),
    work_ch.close(),

    // Collect results
    workers,
    result_ch.close(),
    result_ch.collect(),  // drain remaining items into list
)

@worker (work: Channel<Item>, results: Channel<Result<ProcessedItem, Error>>) -> void uses Async = run(
    loop(
        match(work.receive(),
            Some(item) -> results.send(process(item)),
            None -> break,
        ),
    ),
)
```

### Fan-In: Multiple Producers, One Consumer

```sigil
@aggregate_sources (sources: [Source]) -> [Data] uses Async = run(
    let data_ch = Channel<Data>.new(buffer: 100),

    // Start producers
    let producers = parallel(
        .tasks: map(sources, src -> produce_from(src, data_ch)),
    ),

    // Collect until all producers done
    let result = [],
    let producers_done = 0,
    loop(
        match(data_ch.receive(),
            Some(data) -> result = result.append(data),
            None -> break,
        ),
    ),
    result,
)
```

### Pipeline: Chained Processing

```sigil
@pipeline (input: [int]) -> [int] uses Async = run(
    let ch1 = Channel<int>.new(buffer: 10),
    let ch2 = Channel<int>.new(buffer: 10),
    let ch3 = Channel<int>.new(buffer: 10),

    parallel(
        .source: feed_channel(input, ch1),
        .stage1: transform(ch1, ch2, x -> x * 2),
        .stage2: transform(ch2, ch3, x -> x + 1),
        .sink: collect_channel(ch3),
    ),
)

@transform<T> (input: Channel<T>, output: Channel<T>, f: T -> T) -> void uses Async = run(
    loop(
        match(input.receive(),
            Some(value) -> output.send(f(value)),
            None -> run(output.close(), break),
        ),
    ),
)
```

### Select: Multiple Channels

Wait on multiple channels:

```sigil
@multiplex (ch1: Channel<int>, ch2: Channel<str>) -> void uses Async = run(
    loop(
        select(
            ch1.receive() -> value -> print("int: " + str(value)),
            ch2.receive() -> value -> print("str: " + value),
            .closed: break,
        ),
    ),
)
```

---

## No Shared Mutable State

### The Problem with Shared State

Traditional concurrent programming uses locks:

```python
# Python - shared mutable state
counter = 0
lock = Lock()

def increment():
    with lock:
        counter += 1  # protected by lock
```

Problems:
- **Deadlocks** - Two tasks waiting for each other's locks
- **Race conditions** - Forgetting to acquire lock
- **Performance** - Lock contention limits parallelism
- **Complexity** - Hard to reason about correct ordering

### Sigil's Solution

Sigil has no `Mutex`, `Lock`, or shared mutable variables:

```sigil
// ERROR: Mutex type doesn't exist
shared_counter = Mutex<int>.new(0)

// ERROR: cannot share mutable reference
@bad (shared: &mut int) -> void uses Async = ...
```

### How to Share State

Instead of shared mutable state, use:

1. **Message passing** via channels
2. **Functional accumulation** with fold
3. **Immutable sharing** with copy

### Example: Counter Without Locks

```sigil
// Instead of shared counter, use channel
@count_items (items: [Item]) -> int uses Async = run(
    let count_ch = Channel<int>.new(buffer: 100),

    // Workers send counts to channel
    parallel(
        .tasks: map(chunk(items, 100), chunk ->
            count_chunk_and_send(chunk, count_ch)
        ),
    ),

    // Aggregate counts
    count_ch.close(),
    fold(count_ch.collect(), 0, +),
)

@count_chunk_and_send (items: [Item], ch: Channel<int>) -> void uses Async = run(
    let count = filter(items, item -> item.is_valid()).len(),
    ch.send(count),
)
```

### Example: Parallel Processing Without Locks

```sigil
@process_documents (documents: [str]) -> [ProcessedDoc] uses Async = run(
    // Process documents in parallel
    let results = parallel(
        .tasks: map(documents, doc -> process_doc(doc)),
        .max_concurrent: 10,
    ),

    // Combine results (functional, no locks needed)
    flatten(results),
)
```

---

## Channel Properties

### Type Safety

Channels enforce type safety at compile time:

```sigil
int_ch = Channel<int>.new(buffer: 10)

int_ch.send(42)     // OK
int_ch.send("hello") // ERROR: expected int, got str
```

### Buffer Semantics

| Buffer Size | Behavior |
|-------------|----------|
| 0 | Rendezvous: send blocks until receive |
| 1 | Minimal buffering |
| N > 1 | Buffered: send blocks when full |

### Backpressure

Bounded buffers provide natural backpressure:

```sigil
@fast_producer (ch: Channel<int>) -> void uses Async =
    for i in 0..1000000 do ch.send(i)  // blocks when buffer full, slowing producer

@slow_consumer (ch: Channel<int>) -> void uses Async = run(
    loop(
        let value = ch.receive(),
        process_slowly(value),  // consumer pace limits producer
    ),
)
```

---

## Channels with Cancellation

### Cancellable Send/Receive

Use context with channel operations:

```sigil
@send_with_timeout (ctx: Context, ch: Channel<Data>, value: Data) -> Result<void, Error> uses Async =
    timeout(
        .op: ch.send(value),
        .after: 5s,
        .on_timeout: Err(SendTimeout {})
    )

@receive_with_timeout (ctx: Context, ch: Channel<Data>) -> Result<Data, Error> uses Async =
    timeout(
        .op: ch.receive(),
        .after: 5s,
        .on_timeout: Err(ReceiveTimeout {})
    )
```

### Graceful Shutdown with Channels

```sigil
@worker (ctx: Context, work: Channel<Job>, results: Channel<Result<JobResult, Error>>) -> void uses Async = run(
    loop(
        // Check for cancellation
        if ctx.is_cancelled() then break,

        // Try to receive with timeout
        match(timeout(
            .op: work.receive(),
            .after: 100ms,
        ),
            Ok(job) -> results.send(process(job)),
            Err(_) -> continue,  // timeout, check cancellation again
        ),
    ),
)
```

---

## When to Use Channels

### Use Channels For

| Scenario | Example |
|----------|---------|
| Streaming data | Log lines, events |
| Work distribution | Job queues |
| Result collection | Aggregating worker outputs |
| Pipeline processing | Multi-stage transformations |

### Use `parallel` Instead For

| Scenario | Example |
|----------|---------|
| Fixed concurrent tasks | Fetching 3 resources |
| Map over collection | Processing each item |
| Fan-out/fan-in | Parallel processing then combine |

### Decision Guide

```
Need streaming/continuous data flow? -> Channel
Fixed set of concurrent operations? -> parallel
Workers processing a queue? -> Channel
Parallel map over collection? -> parallel with max_concurrent
```

---

## Best Practices

### Choose Appropriate Buffer Sizes

```sigil
// Good: buffer based on expected load
ch = Channel<Job>.new(buffer: $worker_count * 2)

// Bad: arbitrary large buffer (wastes memory)
ch = Channel<Job>.new(buffer: 1000000)

// Bad: too small buffer (excessive blocking)
ch = Channel<Job>.new(buffer: 1)  // unless intentional
```

### Always Close Channels

```sigil
// Good: explicit close signals completion
@producer (ch: Channel<int>) -> void uses Async = run(
    for i in items do ch.send(i),
    ch.close(),  // signal no more data
)

// Bad: consumers wait forever
@producer_bad (ch: Channel<int>) -> void uses Async =
    for i in items do ch.send(i)
    // forgot to close!
```

### Use Typed Channels

```sigil
// Good: specific message types
type WorkerMessage = Start | Stop | Process(Data)
ch = Channel<WorkerMessage>.new(buffer: 10)

// Less clear: generic channel
ch = Channel<any>.new(buffer: 10)  // avoid if possible
```

### Handle Channel Closure

```sigil
// Good: handle None from closed channel
loop(
    match(ch.receive(),
        Some(value) -> process(value),
        None -> break,  // channel closed, exit gracefully
    ),
)
```

---

## Error Messages

### Type Mismatch

```
error[E0400]: type mismatch in channel send
  --> src/main.si:10:14
   |
10 |     ch.send("hello")
   |             ^^^^^^^ expected `int`, found `str`
   |
   = note: channel type is `Channel<int>`
```

### Missing Buffer Size

```
error[E0401]: channel buffer size required
  --> src/main.si:5:10
   |
5  |     ch = Channel<int>.new()
   |          ^^^^^^^^^^^^^^^^^^ missing buffer parameter
   |
   = note: specify buffer size: `Channel<int>.new(buffer: 10)`
```

### Send on Closed Channel

```
error[E0402]: send on closed channel
  --> src/main.si:15:5
   |
15 |     ch.send(value)
   |     ^^^^^^^^^^^^^^ channel was closed
   |
   = note: check if channel is open before sending
```

### Shared Mutable State Attempt

```
error[E0403]: shared mutable state not allowed
  --> src/main.si:8:10
   |
8  |     shared = Mutex<int>.new(0)
   |              ^^^^^ type `Mutex` does not exist
   |
   = note: use channels for communication between tasks
   = help: see "Channels" documentation for patterns
```

---

## See Also

- [Capability-Based Async](01-capability-based-async.md)
- [Structured Concurrency](02-structured-concurrency.md)
- [Cancellation](03-cancellation.md)
- [Patterns Reference](../02-syntax/04-patterns-reference.md)
