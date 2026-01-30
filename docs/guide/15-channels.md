---
title: "Channels"
description: "Producer-consumer communication, channel types, and patterns."
order: 15
part: "Effects and Concurrency"
---

# Channels

Channels enable communication between concurrent tasks. They provide a safe, typed way to pass data between producers and consumers.

## Channel Basics

A channel is a queue with two ends:
- **Producer** — sends values into the channel
- **Consumer** — receives values from the channel

```ori
// Create a channel
let (producer, consumer) = channel<int>(buffer: 10)

// Producer sends values
producer.send(value: 1)
producer.send(value: 2)
producer.close()

// Consumer receives values
let first = consumer.receive()   // Some(1)
let second = consumer.receive()  // Some(2)
let done = consumer.receive()    // None (channel closed)
```

## Channel Types

Ori provides four channel constructors for different communication patterns:

### channel — One-to-One

Basic channel with one producer and one consumer:

```ori
let (producer, consumer) = channel<int>(buffer: 10)
```

- `Producer<T>` — cannot be cloned
- `Consumer<T>` — cannot be cloned
- Use when: Single producer, single consumer

### channel_in — Fan-In (Many-to-One)

Multiple producers, one consumer:

```ori
let (producer, consumer) = channel_in<int>(buffer: 10)
```

- `CloneableProducer<T>` — can be cloned for multiple senders
- `Consumer<T>` — cannot be cloned
- Use when: Aggregating results from multiple workers

```ori
@aggregate_results (worker_count: int) -> [Result<int, Error>] uses Async = run(
    let (producer, consumer) = channel_in<Result<int, Error>>(buffer: 100),

    nursery(
        body: n -> run(
            // Spawn multiple workers, each with a cloned producer
            for i in 0..worker_count do run(
                let p = producer.clone(),
                n.spawn(task: () -> run(
                    let result = compute_work(worker_id: i),
                    p.send(value: result),
                )),
            ),
            producer.close(),
        ),
        on_error: CollectAll,
        timeout: 60s,
    ),

    // Collect all results
    let results: [Result<int, Error>] = [],
    loop(
        match(consumer.receive()) {
            Some(r) -> run(
                results = [...results, r],
                continue,
            ),
            None -> break results,
        },
    ),
)
```

### channel_out — Fan-Out (One-to-Many)

One producer, multiple consumers:

```ori
let (producer, consumer) = channel_out<int>(buffer: 10)
```

- `Producer<T>` — cannot be cloned
- `CloneableConsumer<T>` — can be cloned for multiple receivers
- Use when: Distributing work to multiple workers

```ori
@distribute_work (items: [Item], worker_count: int) -> void uses Async = run(
    let (producer, consumer) = channel_out<Item>(buffer: 100),

    nursery(
        body: n -> run(
            // Spawn workers with cloned consumers
            for _ in 0..worker_count do run(
                let c = consumer.clone(),
                n.spawn(task: () -> worker(input: c)),
            ),

            // Send work to the channel
            for item in items do
                producer.send(value: item),
            producer.close(),
        ),
        on_error: CollectAll,
        timeout: 300s,
    ),
)

@worker (input: CloneableConsumer<Item>) -> void uses Async = run(
    loop(
        match(input.receive()) {
            Some(item) -> run(
                process_item(item: item),
                continue,
            ),
            None -> break,
        },
    ),
)
```

### channel_all — Many-to-Many

Multiple producers and multiple consumers:

```ori
let (producer, consumer) = channel_all<int>(buffer: 10)
```

- `CloneableProducer<T>` — can be cloned
- `CloneableConsumer<T>` — can be cloned
- Use when: Complex communication patterns

## Channel Operations

### Producer Methods

```ori
// Send a value (blocks if buffer is full)
producer.send(value: 42)

// Close the channel (no more values can be sent)
producer.close()

// Check if channel is closed
if producer.is_closed() then ...
```

### Consumer Methods

```ori
// Receive a value (blocks if buffer is empty)
let value = consumer.receive()  // Option<T>

// Check if channel is closed
if consumer.is_closed() then ...
```

### Consumer as Iterator

Consumers implement `Iterable`, so you can use them in for loops:

```ori
for value in consumer do
    process(value: value)
```

This is equivalent to:

```ori
loop(
    match(consumer.receive()) {
        Some(value) -> process(value: value),
        None -> break,
    },
)
```

## The Buffer

The `buffer` parameter controls how many values can be queued:

```ori
// Small buffer — producers block quickly
let (p, c) = channel<int>(buffer: 1)

// Larger buffer — more values can queue
let (p, c) = channel<int>(buffer: 1000)
```

### Buffer Sizing Guidelines

| Scenario | Buffer Size |
|----------|-------------|
| Tight synchronization | 0 or 1 |
| Producer faster than consumer | Larger buffer |
| Consumer faster than producer | Small buffer is fine |
| Unknown | Start with 10-100, tune based on performance |

### Zero Buffer (Synchronous)

```ori
let (producer, consumer) = channel<int>(buffer: 0)

// Send blocks until receive happens
// Receive blocks until send happens
```

Zero-buffer channels enforce synchronization between producer and consumer.

## The Sendable Trait

Channel values must implement `Sendable`:

```ori
// OK: primitives are Sendable
channel<int>(buffer: 10)
channel<str>(buffer: 10)

// OK: simple structs are Sendable
type Message = { id: int, content: str }
channel<Message>(buffer: 10)

// ERROR: closures capturing mutable state are not Sendable
let counter = 0
channel<() -> int>(buffer: 10)  // Not allowed if closure captures mutable state
```

`Sendable` is automatically implemented for types that are safe to transfer between concurrent tasks:
- All primitive types
- Structs containing only Sendable types
- Immutable closures (capturing only immutable bindings)

## Channel Patterns

### Worker Pool

Process items with a fixed number of workers:

```ori
type WorkItem = { id: int, data: str }
type WorkResult = { id: int, output: str }

@process_with_pool (
    items: [WorkItem],
    worker_count: int,
) -> [WorkResult] uses Async = run(
    let (work_producer, work_consumer) = channel_out<WorkItem>(buffer: 100),
    let (result_producer, result_consumer) = channel_in<WorkResult>(buffer: 100),

    nursery(
        body: n -> run(
            // Spawn workers
            for _ in 0..worker_count do run(
                let wc = work_consumer.clone(),
                let rp = result_producer.clone(),
                n.spawn(task: () -> pool_worker(work: wc, results: rp)),
            ),

            // Send work
            for item in items do
                work_producer.send(value: item),
            work_producer.close(),
        ),
        on_error: CollectAll,
        timeout: 300s,
    ),

    // Collect results
    let results: [WorkResult] = [],
    loop(
        match(result_consumer.receive()) {
            Some(r) -> run(
                results = [...results, r],
                continue,
            ),
            None -> break results,
        },
    ),
)

@pool_worker (
    work: CloneableConsumer<WorkItem>,
    results: CloneableProducer<WorkResult>,
) -> void uses Async = run(
    for item in work do run(
        let output = process_work(item: item),
        results.send(value: WorkResult { id: item.id, output }),
    ),
)

@process_work (item: WorkItem) -> str = `Processed: {item.data}`
```

### Pipeline

Chain processing stages:

```ori
type Stage1Output = { data: str }
type Stage2Output = { data: str, processed: bool }
type FinalOutput = { data: str, processed: bool, validated: bool }

@pipeline (input: [str]) -> [FinalOutput] uses Async = run(
    // Create channels between stages
    let (stage1_out, stage2_in) = channel<Stage1Output>(buffer: 50),
    let (stage2_out, stage3_in) = channel<Stage2Output>(buffer: 50),
    let (stage3_out, collector) = channel<FinalOutput>(buffer: 50),

    nursery(
        body: n -> run(
            // Stage 1: Transform
            n.spawn(task: () -> run(
                for item in input do
                    stage1_out.send(value: Stage1Output { data: item }),
                stage1_out.close(),
            )),

            // Stage 2: Process
            n.spawn(task: () -> run(
                for item in stage2_in do
                    stage2_out.send(value: Stage2Output {
                        data: item.data,
                        processed: true,
                    }),
                stage2_out.close(),
            )),

            // Stage 3: Validate
            n.spawn(task: () -> run(
                for item in stage3_in do
                    stage3_out.send(value: FinalOutput {
                        data: item.data,
                        processed: item.processed,
                        validated: true,
                    }),
                stage3_out.close(),
            )),
        ),
        on_error: FailFast,
        timeout: 60s,
    ),

    // Collect final results
    let results: [FinalOutput] = [],
    for result in collector do
        results = [...results, result],
    results,
)
```

### Fan-Out/Fan-In

Distribute work, then aggregate:

```ori
@fan_out_fan_in (items: [int], worker_count: int) -> int uses Async = run(
    // Fan-out channel
    let (distribute, workers) = channel_out<int>(buffer: 100),

    // Fan-in channel
    let (results, aggregator) = channel_in<int>(buffer: 100),

    nursery(
        body: n -> run(
            // Workers (fan-out -> process -> fan-in)
            for _ in 0..worker_count do run(
                let w = workers.clone(),
                let r = results.clone(),
                n.spawn(task: () -> run(
                    for item in w do
                        r.send(value: item * 2),  // Process: double the value
                )),
            ),

            // Distribute items
            for item in items do
                distribute.send(value: item),
            distribute.close(),
        ),
        on_error: CollectAll,
        timeout: 60s,
    ),

    // Aggregate results
    let sum = 0,
    for value in aggregator do
        sum = sum + value,
    sum,
)
```

### Rate Limiter

Control throughput with a token bucket:

```ori
type Token = {}

@rate_limited_worker (
    work: CloneableConsumer<WorkItem>,
    tokens: Consumer<Token>,
    results: CloneableProducer<WorkResult>,
) -> void uses Async = run(
    for item in work do run(
        // Wait for a token before processing
        let _ = tokens.receive(),
        let output = process_work(item: item),
        results.send(value: WorkResult { id: item.id, output }),
    ),
)

@token_generator (
    tokens: Producer<Token>,
    rate: int,  // tokens per second
) -> void uses Async, Clock = run(
    loop(
        tokens.send(value: Token {}),
        sleep(duration: 1s / rate),
    ),
)
```

## Testing Channels

### Basic Channel Tests

```ori
@test_basic_channel tests _ () -> void = run(
    let (producer, consumer) = channel<int>(buffer: 3),

    producer.send(value: 1),
    producer.send(value: 2),
    producer.send(value: 3),
    producer.close(),

    assert_eq(actual: consumer.receive(), expected: Some(1)),
    assert_eq(actual: consumer.receive(), expected: Some(2)),
    assert_eq(actual: consumer.receive(), expected: Some(3)),
    assert_eq(actual: consumer.receive(), expected: None),
)
```

### Testing Channel Patterns

```ori
@test_worker_pool tests @process_with_pool () -> void = run(
    let items = [
        WorkItem { id: 1, data: "a" },
        WorkItem { id: 2, data: "b" },
        WorkItem { id: 3, data: "c" },
    ],

    let results = process_with_pool(items: items, worker_count: 2),

    assert_eq(actual: len(collection: results), expected: 3),
    // Note: order may vary due to concurrency
)

@test_fan_out_fan_in tests @fan_out_fan_in () -> void = run(
    let items = [1, 2, 3, 4, 5],
    let result = fan_out_fan_in(items: items, worker_count: 3),

    // Sum of doubled values: 2+4+6+8+10 = 30
    assert_eq(actual: result, expected: 30),
)
```

## Error Handling

### Producer Errors

If a producer encounters an error, close the channel and signal:

```ori
@producer_with_errors (
    output: Producer<Result<int, Error>>,
) -> void uses Async = run(
    for i in 0..10 do run(
        let result = might_fail(value: i),
        match(
            result,
            Ok(v) -> output.send(value: Ok(v)),
            Err(e) -> run(
                output.send(value: Err(e)),
                output.close(),
                return (),
            ),
        ),
    ),
    output.close(),
)
```

### Consumer Error Aggregation

```ori
@consume_with_errors (
    input: Consumer<Result<int, Error>>,
) -> (int, [Error]) uses Async = run(
    let sum = 0,
    let errors: [Error] = [],

    for result in input do
        match(
            result,
            Ok(v) -> sum = sum + v,
            Err(e) -> errors = [...errors, e],
        ),

    (sum, errors),
)
```

## Best Practices

### Close Channels Properly

Always close channels when done sending:

```ori
// BAD: Consumer waits forever
producer.send(value: 1)
producer.send(value: 2)
// Missing: producer.close()

// GOOD: Consumer knows when to stop
producer.send(value: 1)
producer.send(value: 2)
producer.close()
```

### Handle Closed Channels

Check for None when receiving:

```ori
loop(
    match(consumer.receive()) {
        Some(value) -> process(value: value),
        None -> break,  // Channel closed
    },
)
```

### Match Buffer Size to Usage

```ori
// Streaming large data: smaller buffer to limit memory
let (p, c) = channel<LargeData>(buffer: 5)

// Many small messages: larger buffer for throughput
let (p, c) = channel<SmallMessage>(buffer: 1000)
```

### Use the Right Channel Type

```ori
// Single producer, single consumer
let (p, c) = channel<T>(buffer: n)

// Multiple producers, single consumer (aggregation)
let (p, c) = channel_in<T>(buffer: n)

// Single producer, multiple consumers (distribution)
let (p, c) = channel_out<T>(buffer: n)

// Multiple of both (complex patterns)
let (p, c) = channel_all<T>(buffer: n)
```

## Quick Reference

### Channel Constructors

```ori
// One-to-one
channel<T>(buffer: n) -> (Producer<T>, Consumer<T>)

// Fan-in (many-to-one)
channel_in<T>(buffer: n) -> (CloneableProducer<T>, Consumer<T>)

// Fan-out (one-to-many)
channel_out<T>(buffer: n) -> (Producer<T>, CloneableConsumer<T>)

// Many-to-many
channel_all<T>(buffer: n) -> (CloneableProducer<T>, CloneableConsumer<T>)
```

### Producer Methods

```ori
producer.send(value: v) -> void
producer.close() -> void
producer.is_closed() -> bool
```

### Consumer Methods

```ori
consumer.receive() -> Option<T>
consumer.is_closed() -> bool
// Also iterable: for item in consumer do ...
```

### Channel Types

| Type | Producer | Consumer | Use Case |
|------|----------|----------|----------|
| `channel` | Single | Single | Simple communication |
| `channel_in` | Multiple | Single | Aggregation |
| `channel_out` | Single | Multiple | Distribution |
| `channel_all` | Multiple | Multiple | Complex patterns |

## What's Next

Now that you understand channels:

- **[Traits](/guide/16-traits)** — Shared behavior definitions
- **[Iterators](/guide/17-iterators)** — Functional data processing

