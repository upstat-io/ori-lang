# Proposal: Enhanced Parallel and Concurrency Support in Sigil

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-22
**Affects:** Language design, compiler, runtime, standard library

---

## Executive Summary

This proposal outlines enhancements to Sigil's concurrency model based on analysis of how other languages fail at parallelism and concurrency. The goal is to make Sigil's concurrency support not just safe, but **impossible to misuse** through type-level enforcement of correctness.

Key proposals:
1. **Role-based channel types** - Enforce producer/consumer separation at compile time
2. **Process isolation primitives** - Support multi-process parallelism with memory isolation
3. **Deny capabilities** - Prevent data races through type-level aliasing restrictions
4. **Structured concurrency enhancements** - Nursery-style task groups with scope guarantees
5. **Effect-based concurrency tracking** - Fine-grained capability distinctions

---

## Part 1: How Other Languages Fail

### 1.1 The Shared Mutable State Problem

Most concurrency bugs stem from shared mutable state. Languages attempt to address this differently, and most fail:

| Language | Approach | Failure Mode |
|----------|----------|--------------|
| Java | Locks/synchronized | Deadlocks, forgotten locks, lock ordering bugs |
| C++ | Mutexes, atomics | Data races, undefined behavior, false sharing |
| Python | GIL + threading | GIL doesn't protect against logical races |
| JavaScript | Single-threaded + callbacks | Callback hell, unhandled promise rejections |

**Sigil's existing advantage:** No `Mutex` type, message-passing only. But we can do better.

### 1.2 Go's Partial Solution

Go's approach: "Don't communicate by sharing memory; share memory by communicating."

**What Go gets right:**
- Goroutines are lightweight
- Channels are first-class
- `select` for multiplexing

**Where Go fails:**
- Channels can still share pointers (no ownership transfer)
- `-race` detector is runtime only, not compile-time
- Nothing prevents sending references to shared memory
- No enforcement of producer/consumer roles
- Fire-and-forget goroutines (no structured concurrency)

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

**Research citation:** [A Study of Real-World Data Races in Golang](https://arxiv.org/pdf/2204.00764) found that "transparent capture-by-reference of free variables in goroutines is a recipe for data races."

### 1.3 Rust's Compile-Time Approach

Rust's `Send` and `Sync` traits provide compile-time data race freedom:
- `Send`: Type can be transferred between threads
- `Sync`: Type can be shared between threads (via `&T`)

**What Rust gets right:**
- Compile-time guarantees
- Ownership prevents shared mutable state
- Types encode thread-safety

**Where Rust could be better:**
- `async` function coloring creates viral propagation
- `Arc<Mutex<T>>` is verbose and still allows deadlocks
- No built-in structured concurrency (Tokio `spawn` allows orphan tasks)
- Channel types don't enforce producer/consumer roles

**Citation:** [Fearless Concurrency - The Rust Programming Language](https://doc.rust-lang.org/book/ch16-00-concurrency.html)

### 1.4 Erlang/BEAM's Process Isolation

Erlang processes are completely isolated with no shared memory:

**What Erlang gets right:**
- True process isolation (no shared memory at all)
- Preemptive scheduling (no cooperative yield required)
- Fault tolerance via supervision trees
- "Let it crash" philosophy

**Where Erlang differs:**
- Dynamic typing loses compile-time guarantees
- Message copying overhead for large data
- Not suitable for shared-memory parallelism

**Citation:** [The BEAM Book](https://blog.stenmans.org/theBeamBook/), [Erlang Process Isolation](https://www.erlang-solutions.com/blog/the-beam-erlangs-virtual-machine/)

### 1.5 The Function Coloring Problem

Bob Nystrom's famous essay ["What Color is Your Function?"](https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function/) describes how `async` creates a viral divide:

- "Red" (async) functions can only be called from other red functions
- This "colors" your entire codebase
- Higher-order functions become complex (need async and sync variants)

**Languages with this problem:** JavaScript, Python, Rust, C#, Kotlin, C++

**Languages without it:** Go (goroutines), Ruby (fibers), Erlang (processes)

**Sigil's position:** We have the Async capability, which colors functions but provides more value (explicit dependencies, easy mocking) for the same propagation cost.

### 1.6 Pony's Deny Capabilities

Pony introduced **reference capabilities** that restrict aliasing at the type level:

| Capability | Can Read | Can Write | Aliases Can Read | Aliases Can Write |
|------------|----------|-----------|------------------|-------------------|
| `iso` | Yes | Yes | No | No |
| `val` | Yes | No | Yes | No |
| `ref` | Yes | Yes | Yes | Yes |
| `box` | Yes | No | Yes | Maybe |
| `tag` | No | No | No | No |

**Key insight:** "Deny capabilities" work by denying what aliases can do, not by granting permissions.

**What Pony gets right:**
- Data-race freedom at compile time
- No locks, no atomics
- Sendable types have the same guarantees locally and globally

**Citation:** [Deny Capabilities for Safe, Fast Actors](https://www.ponylang.io/media/papers/fast-cheap.pdf)

### 1.7 Swift's Evolving Concurrency

Swift 6 introduced:
- `Sendable` protocol for safe cross-actor transfer
- Actor isolation
- `@MainActor` for UI safety

**Lessons from Swift 6.2's "Approachable Concurrency":**
- Progressive disclosure: start simple, add complexity only when needed
- `@concurrent` attribute opts into parallelism explicitly
- Default to main actor isolation reduces accidental races

**Citation:** [Approachable Concurrency in Swift 6.2](https://www.avanderlee.com/concurrency/approachable-concurrency-in-swift-6-2-a-clear-guide/)

### 1.8 Java Virtual Threads (Project Loom)

Virtual threads enable massive concurrency for I/O-bound work:

**Limitations:**
- No benefit for CPU-intensive work
- Doesn't solve data race problems
- Library compatibility issues
- Debugging millions of threads is hard

**Citation:** [Project Loom Virtual Threads Guide](https://dev.to/elsie-rainee/project-loom-virtual-threads-in-java-complete-2026-guide-d35)

### 1.9 The CppCon 2025 IPC Talk Insights

Jody Hagins' talk on inter-process queues highlighted:

1. **C++ atomics don't work across processes** - The standard assumes single-process
2. **Role separation is essential** - API should enforce producer/consumer roles
3. **Single-producer designs need compile-time enforcement**

**Key quote:** "A properly designed inter-process queue API must enforce role separation, ensuring that a process can only perform operations appropriate to its designated role."

---

## Part 2: Proposed Enhancements

### 2.1 Role-Based Channel Types

**Problem:** Current `Channel<T>` allows any task to send or receive, making it easy to accidentally create multiple consumers in a single-consumer design.

**Proposal:** Split channels into role-specific types with readable named parameters:

```sigil
// Sharing options (what can be cloned)
type Sharing = Exclusive | Producers | Consumers | Both

// Return types vary based on sharing
type ChannelPair<T> = { producer: Producer<T>, consumer: Consumer<T> }

// Single function with named parameters - no cryptic acronyms
@channel<T: Sendable> (
    .buffer: int,
    .share: Sharing = Exclusive,  // default: neither end is cloneable
) -> ChannelPair<T>
```

**Usage:**

```sigil
@main () -> void uses Async = run(
    // Exclusive channel (default) - most efficient, neither end cloneable
    let { producer, consumer } = channel<int>(
        .buffer: 10,
    ),

    parallel(
        .producer: produce(producer),
        .consumer: consume(consumer),
    ),
)

@produce (p: Producer<int>) -> void uses Async =
    for i in 0..100 do p.send(i)
    // p.receive()  // ERROR: Producer<T> has no method 'receive'

@consume (c: Consumer<int>) -> [int] uses Async = run(
    let result = [],
    loop(
        match(c.receive(),
            Some(v) -> result = result.append(v),
            None -> break,
        ),
    ),
    result,
)
```

**Multi-producer scenario:**

```sigil
@main () -> void uses Async = run(
    // Share the producer end - multiple tasks can send
    let { producer, consumer } = channel<int>(
        .buffer: 100,
        .share: Producers,
    ),

    parallel(
        // Clone producer for each worker
        .workers: map(0..4, i -> worker(producer.clone(), i)),
        .aggregator: aggregate(consumer),
    ),
)
```

**All sharing modes:**

```sigil
// Exclusive (default): one producer, one consumer - fastest
let { producer, consumer } = channel<int>(
    .buffer: 10,
)

// Share producers: many producers, one consumer (fan-in)
let { producer, consumer } = channel<int>(
    .buffer: 10,
    .share: Producers,
)

// Share consumers: one producer, many consumers (fan-out)
let { producer, consumer } = channel<int>(
    .buffer: 10,
    .share: Consumers,
)

// Share both: many producers, many consumers (broadcast/work-stealing)
let { producer, consumer } = channel<int>(
    .buffer: 10,
    .share: Both,
)
```

**Compile-time enforcement:**

```sigil
// ERROR: cannot clone consumer from exclusive channel
let { producer, consumer } = channel<int>(
    .buffer: 10,
)
let consumer2 = consumer.clone()  // ERROR: Consumer<T> is not Clone (sharing: Exclusive)

// OK: consumer can be cloned when sharing allows it
let { producer, consumer } = channel<int>(
    .buffer: 10,
    .share: Consumers,
)
let consumer2 = consumer.clone()  // OK
```

### 2.2 Sendable Type Constraint

**Problem:** We need to ensure only safe types cross task boundaries.

**Proposal:** Introduce a `Sendable` marker trait (similar to Rust/Swift):

```sigil
// Marker trait - automatically derived for safe types
trait Sendable {}

// Automatically Sendable:
// - Primitives (int, float, bool, str, char)
// - Immutable collections of Sendable types
// - Structs/enums where all fields are Sendable
// - Functions without captured mutable state

// NOT Sendable:
// - Mutable references (&mut T)
// - Types with interior mutability
// - Raw pointers (if we ever have them)
```

**Channel constraint:**

```sigil
// Channel only accepts Sendable types (already shown in signature above)
@channel<T: Sendable> (.buffer: int, .share: Sharing = Exclusive) -> ChannelPair<T>

// Compile error:
type BadType = { mutable_ref: &mut int }
let ch = channel<BadType>(
    .buffer: 10,
)  // ERROR: BadType is not Sendable
```

### 2.3 Ownership Transfer for Channels

**Problem:** Go allows sending pointers over channels while retaining access.

**Proposal:** Channels consume ownership on send (like Rust):

```sigil
@producer (p: Producer<Data>) -> void uses Async = run(
    let data = create_data(),
    p.send(data),          // Ownership transferred
    // data.field          // ERROR: 'data' moved into channel
)
```

**For types that need sharing, use explicit copy:**

```sigil
@producer (p: Producer<Data>) -> void uses Async = run(
    let data = create_data(),
    p.send(data.clone()),  // Send a copy
    print(data.field),     // Original still accessible
)
```

### 2.4 Process Isolation Primitives

**Problem:** Thread-level parallelism shares memory; some workloads need true isolation.

**Proposal:** Add process-level parallelism with explicit IPC:

```sigil
// Process isolation capability
trait ProcessIsolation {}

// Spawn isolated process
@spawn_process<T: Sendable, R: Sendable> (
    task: T -> R,
    input: T,
) -> ProcessHandle<R> uses ProcessIsolation

// Process handle for communication
type ProcessHandle<R> = {
    @wait () -> Result<R, ProcessError>,
    @kill () -> void,
    @is_alive () -> bool,
}

// Usage
@compute_isolated (data: LargeData) -> Result<int, Error> uses ProcessIsolation = run(
    let handle = spawn_process(
        task: heavy_computation,
        input: data,
    ),
    handle.wait(),
)
```

**Inter-process channels:**

```sigil
// IPC channel - data is serialized, not shared
@ipc_channel<T: Sendable + Serializable> (buffer: int) -> IpcChannelPair<T>

type IpcChannelPair<T> = {
    producer: IpcProducer<T>,
    consumer: IpcConsumer<T>,
}
```

**Use cases:**
- Fault isolation (crash in child doesn't kill parent)
- Security isolation (sandboxing)
- Resource isolation (memory limits per process)
- Multi-core CPU-bound work without GC pauses

### 2.5 Structured Concurrency Enhancements

**Current state:** Sigil has structured concurrency via `parallel`. Enhance with explicit nursery/task group concept:

```sigil
// Task group with explicit scope
@process_batch (items: [Item]) -> [Result<Output, Error>] uses Async =
    task_group(
        .spawn: group -> for item in items do group.spawn(process(item)),
        .on_error: cancel_remaining,  // or: collect_all
        .timeout: 30s,
    )
```

**Nursery pattern (inspired by Trio):**

```sigil
@complex_workflow () -> Result<Data, Error> uses Async =
    nursery(
        .tasks: n -> run(
            // Spawn tasks dynamically
            n.spawn(fetch_config()),

            let config = n.wait_for_first(),

            // Spawn more based on config
            for endpoint in config.endpoints do
                n.spawn(fetch_data(endpoint)),
        ),
        .on_error: cancel_all,
    )
```

**Guarantees:**
- All spawned tasks complete before nursery exits
- Errors propagate and cancel siblings (configurable)
- No orphan tasks possible

### 2.6 Fine-Grained Async Capabilities

**Current:** Single `Async` capability for all async operations.

**Proposal:** Distinguish different kinds of concurrent effects:

```sigil
// Base async capability (can suspend)
trait Async {}

// Parallel execution (CPU-bound)
trait Parallel: Async {}

// I/O operations (may block on external resources)
trait AsyncIO: Async {}

// Process spawning
trait ProcessIsolation {}

// Function signatures become more precise
@cpu_bound_work (data: [int]) -> int uses Parallel =
    parallel(
        .tasks: map(chunk(data, 1000), sum),
    ).fold(0, +)

@io_bound_work (urls: [str]) -> [Data] uses Http, AsyncIO =
    parallel(
        .tasks: map(urls, Http.get),
        .max_concurrent: 10,
    )
```

**Benefits:**
- Clearer documentation of what a function does
- Potential for different scheduling strategies
- Tests can mock at appropriate granularity

### 2.7 Deny Capabilities (Pony-Inspired)

**Proposal:** Reference capabilities that restrict aliasing:

```sigil
// Reference capabilities as type modifiers
type IsoRef<T> = iso T      // Isolated: no other references exist
type ValRef<T> = val T      // Value: immutable, shareable
type RefRef<T> = ref T      // Reference: mutable, not shareable (current default)

// Channels require iso or val
trait Sendable {
    // Type can be safely sent across task boundaries
    // Automatically satisfied by: iso T, val T, and types composed of them
}

// Send transfers ownership (iso) or shares immutable (val)
@send_isolated (p: Producer<iso Data>) -> void uses Async = run(
    let data: iso Data = create_data(),
    p.send(data),  // data is consumed (iso transferred)
)

@send_immutable (p: Producer<val Config>) -> void uses Async = run(
    let config: val Config = load_config(),
    p.send(config),  // config can still be read (val is shareable)
    print(config.name),  // OK: val allows read aliases
)
```

### 2.8 Select with Priorities and Fairness

**Proposal:** Enhanced select for channel multiplexing:

```sigil
@multiplexer (high: Consumer<Urgent>, normal: Consumer<Regular>) -> void uses Async =
    loop(
        select(
            // Priority order: urgent messages first
            .priority: [
                high.receive() -> msg -> handle_urgent(msg),
                normal.receive() -> msg -> handle_normal(msg),
            ],
            // Or fair scheduling (round-robin)
            // .fair: [...],
            .timeout: 1s -> check_health(),
            .closed: break,
        ),
    )
```

### 2.9 Cancellation Tokens (Enhanced)

**Proposal:** First-class cancellation with scoped tokens:

```sigil
type CancellationToken = {
    @is_cancelled () -> bool,
    @check () -> Result<void, Cancelled>,  // Returns Err if cancelled
    @child () -> CancellationToken,         // Create linked child token
}

@cancellable_work (token: CancellationToken) -> Result<Data, Error> uses Async = run(
    for chunk in data_chunks do run(
        token.check()?,  // Early exit if cancelled
        process(chunk),
    ),
    Ok(result),
)
```

---

## Part 3: Implementation Roadmap

### Phase 1: Foundation (Immediate)

1. **Implement `Sendable` trait** in type system
2. **Add ownership transfer semantics** for channel send
3. **Implement role-based channel types** (Producer/Consumer split)

### Phase 2: Enhanced Patterns (Short-term)

4. **Add task_group/nursery pattern**
5. **Implement enhanced select** with priorities
6. **Add fine-grained async capabilities** (Parallel, AsyncIO)

### Phase 3: Process Isolation (Medium-term)

7. **Design IPC primitives** (serialization format, protocols)
8. **Implement spawn_process** with isolation guarantees
9. **Add IPC channels** with proper serialization

### Phase 4: Advanced Features (Long-term)

10. **Explore deny capabilities** (Pony-style reference capabilities)
11. **Add supervision trees** (Erlang-inspired fault tolerance)
12. **Consider distributed channels** (cross-machine communication)

---

## Part 4: Comparison Matrix

| Feature | Go | Rust | Erlang | Swift | Sigil (Current) | Sigil (Proposed) |
|---------|-----|------|--------|-------|-----------------|------------------|
| Data race freedom | Runtime | Compile | Runtime | Compile | Compile | Compile |
| Role-based channels | No | No | N/A | No | No | **Yes** |
| Structured concurrency | No | Optional | Yes | Yes | Yes | **Enhanced** |
| Process isolation | No | No | Yes | No | No | **Yes** |
| Ownership transfer | No | Yes | Copy | Partial | No | **Yes** |
| Sendable constraint | No | Yes | N/A | Yes | No | **Yes** |
| Fire-and-forget | Yes | Yes | Yes | No | No | No |
| Function coloring | No | Yes | No | Yes | Yes (valuable) | Yes (valuable) |

---

## Part 5: Open Questions

### 5.1 Reference Capabilities Complexity

Pony's reference capabilities are powerful but complex. Should Sigil:
- A) Adopt full deny capabilities (maximum safety, steep learning curve)
- B) Use simplified model (iso/val/ref only, easier to learn)
- C) Start without, add later based on demand

**Recommendation:** Option B - start with simplified model.

### 5.2 Process Isolation Overhead

IPC requires serialization. Should Sigil:
- A) Always serialize (safe, slower for large data)
- B) Allow shared memory regions for specific types (faster, more complex)
- C) Provide both, let users choose

**Recommendation:** Option A initially, Option C for performance-critical use cases later.

### 5.3 Async Capability Granularity

How fine should capability distinctions be?
- A) Single `Async` (current - simple)
- B) `Async` + `Parallel` + `AsyncIO` (proposed - moderate)
- C) Many fine-grained capabilities (maximum precision, verbose)

**Recommendation:** Option B - meaningful distinctions without verbosity.

### 5.4 Backward Compatibility

Role-based channels change the Channel API. Migration strategy:
- A) Breaking change in next major version
- B) Deprecation period with both APIs
- C) New types alongside existing (parallel APIs)

**Recommendation:** Option A - Sigil is pre-1.0, clean breaks are acceptable.

---

## Part 6: Examples

### 6.1 Safe Producer-Consumer

```sigil
@main () -> void uses Async = run(
    // Exclusive channel - type system enforces single producer, single consumer
    let { producer, consumer } = channel<Job>(
        .buffer: 100,
    ),

    parallel(
        .producer: job_producer(producer),
        .consumer: job_consumer(consumer),
    ),

    print("All jobs processed"),
)

@job_producer (p: Producer<Job>) -> void uses Async = run(
    for job in load_jobs() do p.send(job),
    p.close(),
)

@job_consumer (c: Consumer<Job>) -> void uses Async =
    for job in c do process(job)
```

### 6.2 Fault-Tolerant Worker Pool

```sigil
@fault_tolerant_pool (jobs: [Job]) -> [Result<Output, Error>] uses Async, ProcessIsolation = run(
    // Share producers - multiple workers can send results
    let { producer, consumer } = channel<Result<Output, Error>>(
        .buffer: 100,
        .share: Producers,
    ),

    // Each worker in isolated process - crash doesn't affect others
    let workers = map(0..$worker_count, i ->
        spawn_process(
            task: worker_main,
            input: { id: i, producer: producer.clone() },
        )
    ),

    // Feed jobs
    let job_producer = producer.clone(),
    for job in jobs do job_producer.send(Ok(job)),
    job_producer.close(),

    // Collect results, handle worker crashes
    let results = [],
    for _ in 0..len(jobs) do
        match(consumer.receive(),
            Some(result) -> results = results.append(result),
            None -> break,  // All producers closed
        ),

    // Wait for workers (may have crashed)
    for w in workers do
        match(w.wait(),
            Ok(_) -> (),
            Err(e) -> print("Worker crashed: " + str(e)),
        ),

    results,
)
```

### 6.3 Pipeline with Backpressure

```sigil
@data_pipeline (input: Consumer<RawData>) -> Producer<ProcessedData> uses Async = run(
    // Exclusive channels for each stage - maximum efficiency
    let { producer: stage1_out, consumer: stage1_in } = channel<Parsed>(
        .buffer: 10,
    ),
    let { producer: stage2_out, consumer: stage2_in } = channel<Validated>(
        .buffer: 10,
    ),
    let { producer: final_out, consumer: _ } = channel<ProcessedData>(
        .buffer: 10,
    ),

    parallel(
        .parse: pipe(input, stage1_out, parse),
        .validate: pipe(stage1_in, stage2_out, validate),
        .transform: pipe(stage2_in, final_out, transform),
    ),

    final_out,
)

@pipe<A, B> (input: Consumer<A>, output: Producer<B>, f: A -> B) -> void uses Async =
    for item in input do output.send(f(item))
```

---

## References

### Academic Papers
- [Deny Capabilities for Safe, Fast Actors](https://www.ponylang.io/media/papers/fast-cheap.pdf) - Clebsch et al.
- [A Study of Real-World Data Races in Golang](https://arxiv.org/pdf/2204.00764)
- [Concurrency bugs in open source software](https://jisajournal.springeropen.com/articles/10.1186/s13174-017-0055-2)

### Language Documentation
- [Rust Fearless Concurrency](https://doc.rust-lang.org/book/ch16-00-concurrency.html)
- [Swift Concurrency](https://developer.apple.com/videos/play/wwdc2025/268/)
- [Go Data Race Detector](https://go.dev/doc/articles/race_detector)
- [Erlang/BEAM](https://blog.stenmans.org/theBeamBook/)
- [Pony Reference Capabilities](https://tutorial.ponylang.io/reference-capabilities/)

### Blog Posts and Essays
- [What Color is Your Function?](https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function/) - Bob Nystrom
- [Notes on structured concurrency](https://vorpus.org/blog/notes-on-structured-concurrency-or-go-statement-considered-harmful/) - Nathaniel J. Smith
- [Red & blue functions are actually a good thing](https://blainehansen.me/post/red-blue-functions-are-actually-good/)
- [Effect Systems Discussion](https://typesanitizer.com/blog/effects-convo.html)

### Conference Talks
- CppCon 2025: "How To Build Robust C++ Inter-Process Queues" - Jody Hagins

---

## Changelog

- 2026-01-22: Initial draft
