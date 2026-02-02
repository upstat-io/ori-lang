# std.async

Capability-based async utilities and concurrency primitives.

```ori
use std.async { spawn, join, timeout, select }
```

**Requires:** `Async` capability for async operations

---

## Overview

The `std.async` module provides capability-based asynchronous programming:

- Task spawning and joining
- Timeouts and deadlines
- Select for multiple futures
- Async synchronization

Ori uses **capability-based async** rather than async/await syntax. Functions that perform async operations declare the `Async` capability in their signature with `uses Async`. The runtime automatically manages suspension and resumption — no explicit `.await` calls are needed.

> **Note:** `Channel<T>` is built-in (see [prelude](../prelude.md)). This module provides additional async utilities.

---

## Task Management

### @spawn

```ori
@spawn<T> (f: () -> T uses Async) -> Task<T>
```

Spawns an async task.

```ori
use std.async { spawn }

let task = spawn(|| fetch_data(url))
// ... do other work ...
let result = task.result()
```

---

### @join

```ori
@join<T> (tasks: [Task<T>]) -> [T] uses Async
```

Waits for all tasks to complete.

```ori
use std.async { spawn, join }

let tasks = urls | map(_, url -> spawn(|| fetch(url)))
let results = join(tasks)
```

---

### @join_any

```ori
@join_any<T> (tasks: [Task<T>]) -> (T, [Task<T>]) uses Async
```

Waits for first task to complete, returns result and remaining tasks.

```ori
use std.async { spawn, join_any }

let tasks = [spawn(|| slow()), spawn(|| fast())]
let (first_result, remaining) = join_any(tasks)
```

---

## Task Type

### Task<T>

```ori
type Task<T>
```

A handle to a spawned async task.

**Methods:**
- `result() -> T uses Async` — Wait for completion and get result
- `cancel() -> void` — Request cancellation
- `is_done() -> bool` — Check if completed

---

## Timeouts

### @timeout

```ori
@timeout<T> (f: () -> T uses Async, duration: Duration) -> Result<T, TimeoutError> uses Async
```

Wraps an async operation with a timeout.

```ori
use std.async { timeout }

let result = timeout(|| fetch_data(url), 30s)

match(result,
    Ok(data) -> process(data),
    Err(TimeoutError) -> handle_timeout(),
)
```

---

### @deadline

```ori
@deadline<T> (f: () -> T uses Async, time: DateTime) -> Result<T, TimeoutError> uses Async
```

Wraps an async operation with an absolute deadline.

```ori
use std.async { deadline }
use std.time { now }

let must_finish_by = now().add(1h)
let result = deadline(|| long_operation(), must_finish_by)
```

---

## Select

### @select

```ori
@select<T> (tasks: [() -> T uses Async]) -> (int, T) uses Async
```

Waits for first operation to complete, returns index and result.

```ori
use std.async { select }

let (index, result) = select([
    || fetch_from_primary(),
    || fetch_from_backup(),
])

print("Got result from source " + str(index))
```

---

### @select_with

```ori
@select_with (branches: ...) -> T uses Async
```

Select with different operation types.

```ori
use std.async { select_with }

select_with(
    || channel.receive() -> msg -> handle_message(msg),
    || timer.tick() -> _ -> handle_tick(),
    || shutdown.recv() -> _ -> break,
)
```

---

## Synchronization

### Semaphore

```ori
type Semaphore
```

Limits concurrent access.

```ori
use std.async { Semaphore }

let sem = Semaphore.new(10)  // Max 10 concurrent

@limited_fetch (url: str) -> Result<Data, Error> uses Async = run(
    sem.acquire(),
    let result = fetch(url),
    sem.release(),
    result,
)
```

**Methods:**
- `new(permits: int) -> Semaphore` — Create with permit count
- `acquire() -> void uses Async` — Acquire permit (waits if none available)
- `try_acquire() -> bool` — Try to acquire without waiting
- `release() -> void` — Release permit

---

### Barrier

```ori
type Barrier
```

Synchronization point for multiple tasks.

```ori
use std.async { Barrier, spawn }

let barrier = Barrier.new(3)

// All three tasks must reach barrier before any proceeds
for i in 0..3 do spawn(|| run(
    prepare(i),
    barrier.wait(),  // All wait here
    execute(i),
))
```

---

### OnceCell

```ori
type OnceCell<T>
```

Value initialized once, lazily.

```ori
use std.async { OnceCell }

let config: OnceCell<Config> = OnceCell.new()

@get_config () -> Config uses Async =
    config.get_or_init(|| load_config())
```

---

## Sleep

### @sleep

```ori
@sleep (duration: Duration) -> void uses Async
```

Pauses for duration.

```ori
use std.async { sleep }

@retry_with_backoff<T> (f: () -> Result<T, Error> uses Async, attempts: int) -> Result<T, Error> uses Async =
    // Fold through attempts, keeping last error or first success
    (0..attempts).fold(
        initial: Err(Error.from("no attempts")),
        f: (acc, i) -> match(acc,
            Ok(v) -> Ok(v),  // Already succeeded, keep it
            Err(_) -> match(f(),
                Ok(v) -> Ok(v),
                Err(e) if i < attempts - 1 -> run(
                    sleep(100ms * pow(2, i)),
                    Err(e),
                ),
                Err(e) -> Err(e),
            ),
        ),
    )
```

---

## Examples

### Concurrent fetching with limit

```ori
use std.async { Semaphore, spawn, join }

@fetch_all (urls: [str], max_concurrent: int) -> [Result<Data, Error>] uses Async = run(
    let sem = Semaphore.new(max_concurrent),
    let tasks = urls | map(_, url -> spawn(|| run(
        sem.acquire(),
        let result = fetch(url),
        sem.release(),
        result,
    ))),
    join(tasks),
)
```

### Racing with timeout

```ori
use std.async { select, timeout }

@fetch_with_fallback (primary: str, backup: str) -> Result<Data, Error> uses Async = run(
    let (_, result) = select([
        || timeout(|| fetch(primary), 5s),
        || timeout(|| fetch(backup), 10s),
    ]),
    result,
)
```

### Worker pool

```ori
use std.async { spawn, join }

@process_batch<T, R> (
    items: [T],
    workers: int,
    process: T -> R uses Async
) -> [R] uses Async = run(
    let chunks = items.chunks(items.len() / workers + 1),
    let tasks = chunks | map(_, chunk ->
        spawn(|| map(chunk, process) | join(_))
    ),
    join(tasks) | flatten(_),
)
```

---

## See Also

- [Channel](../prelude.md) — Built-in channels
- [Design: Async](../../design/10-async/) — Async design
- [Capabilities](../../spec/14-capabilities.md) — Async capabilities
