# Channels

This document covers Sigil's channel system: typed channels for communication, send/receive operations, bounded buffers, and why Sigil has no shared mutable state.

---

## Overview

Channels are Sigil's mechanism for communication between concurrent tasks. They are typed, bounded, and the only way to share data between tasks.

```sigil
@producer (channel: Channel<int>) -> void uses Async =
    for index in 0..10 do channel.send(
        .value: index,
    )

@consumer (channel: Channel<int>) -> [int] uses Async = collect(
    .source: channel.receive(),
    .until: channel.closed,
)

@main () -> void uses Async = run(
    let channel = Channel<int>.new(
        .buffer: 5,
    ),
    parallel(
        .producer: producer(
            .channel: channel,
        ),
        .consumer: consumer(
            .channel: channel,
        ),
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
channel = Channel<int>.new(
    .buffer: 10,
)

// Channel with buffer size 1 (synchronous-ish)
channel = Channel<str>.new(
    .buffer: 1,
)

// Unbuffered channel (rendezvous)
channel = Channel<Data>.new(
    .buffer: 0,
)
```

### Channel Types

The type parameter determines what can be sent:

```sigil
// Channel of integers
int_channel = Channel<int>.new(
    .buffer: 10,
)

// Channel of structs
user_channel = Channel<User>.new(
    .buffer: 5,
)

// Channel of results
result_channel = Channel<Result<Data, Error>>.new(
    .buffer: 10,
)
```

### Why Bounded Buffers?

Unbounded buffers can cause:
- Memory exhaustion if producer is faster than consumer
- Hidden backpressure problems
- Unpredictable latency

Sigil requires explicit buffer sizes:

```sigil
// Good: explicit buffer size
channel = Channel<int>.new(
    .buffer: 100,
)

// Not allowed: unbounded channel
// ERROR: buffer size required
channel = Channel<int>.new()
```

---

## Sending and Receiving

### The `send` Operation

```sigil
// Send a value - blocks if buffer is full
channel.send(
    .value: value,
)

// Returns void, cannot fail (unless channel is closed)
```

### The `receive` Operation

```sigil
// Receive a value - blocks if buffer is empty
value = channel.receive()

// Returns Option<T> - None if channel is closed
```

### Example: Producer-Consumer

```sigil
@producer (channel: Channel<int>) -> void uses Async = run(
    for index in 1..100 do channel.send(
        .value: index,
    ),
    // signal no more values
    channel.close(),
)

@consumer (channel: Channel<int>) -> int uses Async = run(
    let sum = 0,
    loop(
        match(channel.receive(),
            // channel closed
            Some(value) -> sum = sum + value,
            None -> break,
        ),
    ),
    sum,
)
```

### Closing Channels

```sigil
// Close the channel - no more sends allowed
channel.close()

// After close:
// ERROR: channel closed
channel.send(
    .value: value,
)
// Returns None
channel.receive()
```

---

## Channel Patterns

### Fan-Out: One Producer, Multiple Consumers

```sigil
@distribute_work (items: [Item]) -> [Result<ProcessedItem, Error>] uses Async = run(
    let work_channel = Channel<Item>.new(
        .buffer: 100,
    ),
    let result_channel = Channel<Result<ProcessedItem, Error>>.new(
        .buffer: 100,
    ),

    // Start workers
    let workers = parallel(
        .tasks: map(
            .over: 0..$worker_count,
            .transform: _ -> worker(
                .work: work_channel,
                .results: result_channel,
            ),
        ),
    ),

    // Send work
    for item in items do work_channel.send(
        .value: item,
    ),
    work_channel.close(),

    // Collect results
    workers,
    result_channel.close(),
    // drain remaining items into list
    result_channel.collect(),
)

@worker (work: Channel<Item>, results: Channel<Result<ProcessedItem, Error>>) -> void uses Async = run(
    loop(
        match(work.receive(),
            Some(item) -> results.send(
                .value: process(
                    .item: item,
                ),
            ),
            None -> break,
        ),
    ),
)
```

### Fan-In: Multiple Producers, One Consumer

```sigil
@aggregate_sources (sources: [Source]) -> [Data] uses Async = run(
    let data_channel = Channel<Data>.new(
        .buffer: 100,
    ),

    // Start producers
    let producers = parallel(
        .tasks: map(
            .over: sources,
            .transform: source -> produce_from(
                .source: source,
                .channel: data_channel,
            ),
        ),
    ),

    // Collect until all producers done
    let result = [],
    let producers_done = 0,
    loop(
        match(data_channel.receive(),
            Some(data) -> result = result.append(
                .element: data,
            ),
            None -> break,
        ),
    ),
    result,
)
```

### Pipeline: Chained Processing

```sigil
@pipeline (input: [int]) -> [int] uses Async = run(
    let channel1 = Channel<int>.new(
        .buffer: 10,
    ),
    let channel2 = Channel<int>.new(
        .buffer: 10,
    ),
    let channel3 = Channel<int>.new(
        .buffer: 10,
    ),

    parallel(
        .source: feed_channel(
            .input: input,
            .channel: channel1,
        ),
        .stage1: transform(
            .input: channel1,
            .output: channel2,
            .function: number -> number * 2,
        ),
        .stage2: transform(
            .input: channel2,
            .output: channel3,
            .function: number -> number + 1,
        ),
        .sink: collect_channel(
            .channel: channel3,
        ),
    ),
)

@transform<T> (input: Channel<T>, output: Channel<T>, function: T -> T) -> void uses Async = run(
    loop(
        match(input.receive(),
            Some(value) -> output.send(
                .value: function(value),
            ),
            None -> run(output.close(), break),
        ),
    ),
)
```

### Select: Multiple Channels

Wait on multiple channels:

```sigil
@multiplex (channel1: Channel<int>, channel2: Channel<str>) -> void uses Async = run(
    loop(
        select(
            channel1.receive() -> value -> print(
                .message: "int: " + str(value),
            ),
            channel2.receive() -> value -> print(
                .message: "str: " + value,
            ),
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
    let count_channel = Channel<int>.new(
        .buffer: 100,
    ),

    // Workers send counts to channel
    parallel(
        .tasks: map(
            .over: chunk(
                .collection: items,
                .size: 100,
            ),
            .transform: item_chunk ->
                count_chunk_and_send(
                    .items: item_chunk,
                    .channel: count_channel,
                ),
        ),
    ),

    // Aggregate counts
    count_channel.close(),
    fold(
        .over: count_channel.collect(),
        .initial: 0,
        .operation: +,
    ),
)

@count_chunk_and_send (items: [Item], channel: Channel<int>) -> void uses Async = run(
    let count = filter(
        .over: items,
        .predicate: item -> item.is_valid(),
    ).len(),
    channel.send(
        .value: count,
    ),
)
```

### Example: Parallel Processing Without Locks

```sigil
@process_documents (documents: [str]) -> [ProcessedDoc] uses Async = run(
    // Process documents in parallel
    let results = parallel(
        .tasks: map(
            .over: documents,
            .transform: document -> process_doc(
                .document: document,
            ),
        ),
        .max_concurrent: 10,
    ),

    // Combine results (functional, no locks needed)
    flatten(
        .collection: results,
    ),
)
```

---

## Channel Properties

### Type Safety

Channels enforce type safety at compile time:

```sigil
int_channel = Channel<int>.new(
    .buffer: 10,
)

// OK
int_channel.send(
    .value: 42,
)
// ERROR: expected int, got str
int_channel.send(
    .value: "hello",
)
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
// Blocks when buffer full, slowing producer
@fast_producer (channel: Channel<int>) -> void uses Async =
    for index in 0..1000000 do channel.send(
        .value: index,
    )

// Consumer pace limits producer
@slow_consumer (channel: Channel<int>) -> void uses Async = run(
    loop(
        let value = channel.receive(),
        process_slowly(
            .value: value,
        ),
    ),
)
```

---

## Channels with Cancellation

### Cancellable Send/Receive

Use context with channel operations:

```sigil
@send_with_timeout (context: Context, channel: Channel<Data>, value: Data) -> Result<void, Error> uses Async =
    timeout(
        .operation: channel.send(
            .value: value,
        ),
        .after: 5s,
        .on_timeout: Err(SendTimeout {}),
    )

@receive_with_timeout (context: Context, channel: Channel<Data>) -> Result<Data, Error> uses Async =
    timeout(
        .operation: channel.receive(),
        .after: 5s,
        .on_timeout: Err(ReceiveTimeout {}),
    )
```

### Graceful Shutdown with Channels

```sigil
@worker (context: Context, work: Channel<Job>, results: Channel<Result<JobResult, Error>>) -> void uses Async = run(
    loop(
        // Check for cancellation
        if context.is_cancelled() then break,

        // Try to receive with timeout
        match(timeout(
            .operation: work.receive(),
            .after: 100ms,
        ),
            Ok(job) -> results.send(
                .value: process(
                    .job: job,
                ),
            ),
            // timeout, check cancellation again
            Err(_) -> continue,
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
channel = Channel<Job>.new(
    .buffer: $worker_count * 2,
)

// Bad: arbitrary large buffer (wastes memory)
channel = Channel<Job>.new(
    .buffer: 1000000,
)

// Bad: too small buffer (excessive blocking) unless intentional
channel = Channel<Job>.new(
    .buffer: 1,
)
```

### Always Close Channels

```sigil
// Good: explicit close signals completion
@producer (channel: Channel<int>) -> void uses Async = run(
    for index in items do channel.send(
        .value: index,
    ),
    // signal no more data
    channel.close(),
)

// Bad: consumers wait forever because channel is never closed
@producer_bad (channel: Channel<int>) -> void uses Async =
    for index in items do channel.send(
        .value: index,
    )
```

### Use Typed Channels

```sigil
// Good: specific message types
type WorkerMessage = Start | Stop | Process(Data)
channel = Channel<WorkerMessage>.new(
    .buffer: 10,
)

// Less clear: generic channel (avoid if possible)
channel = Channel<any>.new(
    .buffer: 10,
)
```

### Handle Channel Closure

```sigil
// Good: handle None from closed channel
// When channel is closed, exit gracefully
loop(
    match(channel.receive(),
        Some(value) -> process(
            .value: value,
        ),
        None -> break,
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

- [Async via Capabilities](01-async-await.md)
- [Structured Concurrency](02-structured-concurrency.md)
- [Cancellation](03-cancellation.md)
- [Patterns Reference](../02-syntax/04-patterns-reference.md)
