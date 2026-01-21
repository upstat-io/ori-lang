# std.async

Async utilities and concurrency primitives.

```sigil
use std.async { spawn, join, timeout, select }
```

**No capability required** (for pure async operations)

---

## Overview

The `std.async` module provides:

- Task spawning and joining
- Timeouts and deadlines
- Select for multiple futures
- Async synchronization

> **Note:** `Channel<T>` is built-in (see [prelude](../prelude.md)). This module provides additional async utilities.

---

## Task Management

### @spawn

```sigil
@spawn<T> (f: () -> async T) -> Task<T>
```

Spawns an async task.

```sigil
use std.async { spawn }

let task = spawn(|| fetch_data(url))
// ... do other work ...
let result = task.await
```

---

### @join

```sigil
@join<T> (tasks: [Task<T>]) -> async [T]
```

Waits for all tasks to complete.

```sigil
use std.async { spawn, join }

let tasks = urls | map(_, url -> spawn(|| fetch(url)))
let results = join(tasks).await
```

---

### @join_any

```sigil
@join_any<T> (tasks: [Task<T>]) -> async (T, [Task<T>])
```

Waits for first task to complete, returns result and remaining tasks.

```sigil
use std.async { spawn, join_any }

let tasks = [spawn(|| slow()), spawn(|| fast())]
let (first_result, remaining) = join_any(tasks).await
```

---

## Task Type

### Task<T>

```sigil
type Task<T>
```

A handle to a spawned async task.

**Methods:**
- `await -> T` — Wait for completion
- `cancel() -> void` — Request cancellation
- `is_done() -> bool` — Check if completed

---

## Timeouts

### @timeout

```sigil
@timeout<T> (future: async T, duration: Duration) -> async Result<T, TimeoutError>
```

Wraps future with a timeout.

```sigil
use std.async { timeout }

let result = timeout(fetch_data(url), 30s).await

match(result,
    Ok(data) -> process(data),
    Err(TimeoutError) -> handle_timeout(),
)
```

---

### @deadline

```sigil
@deadline<T> (future: async T, time: DateTime) -> async Result<T, TimeoutError>
```

Wraps future with an absolute deadline.

```sigil
use std.async { deadline }
use std.time { now }

let must_finish_by = now().add(1h)
let result = deadline(long_operation(), must_finish_by).await
```

---

## Select

### @select

```sigil
@select<T> (futures: [async T]) -> async (int, T)
```

Waits for first future to complete, returns index and result.

```sigil
use std.async { select }

let (index, result) = select([
    fetch_from_primary(),
    fetch_from_backup(),
]).await

print("Got result from source " + str(index))
```

---

### @select_with

```sigil
@select_with (branches: ...) -> async T
```

Select with different future types.

```sigil
use std.async { select_with }

select_with(
    channel.receive() -> msg -> handle_message(msg),
    timer.tick() -> _ -> handle_tick(),
    shutdown.recv() -> _ -> break,
).await
```

---

## Synchronization

### Semaphore

```sigil
type Semaphore
```

Limits concurrent access.

```sigil
use std.async { Semaphore }

let sem = Semaphore.new(10)  // Max 10 concurrent

@limited_fetch (url: str) -> async Result<Data, Error> = run(
    sem.acquire().await,
    let result = fetch(url).await,
    sem.release(),
    result,
)
```

**Methods:**
- `new(permits: int) -> Semaphore` — Create with permit count
- `acquire() -> async void` — Acquire permit (waits if none available)
- `try_acquire() -> bool` — Try to acquire without waiting
- `release() -> void` — Release permit

---

### Barrier

```sigil
type Barrier
```

Synchronization point for multiple tasks.

```sigil
use std.async { Barrier, spawn }

let barrier = Barrier.new(3)

// All three tasks must reach barrier before any proceeds
for i in 0..3 do spawn(|| run(
    prepare(i),
    barrier.wait().await,  // All wait here
    execute(i),
))
```

---

### OnceCell

```sigil
type OnceCell<T>
```

Value initialized once, lazily.

```sigil
use std.async { OnceCell }

let config: OnceCell<Config> = OnceCell.new()

@get_config () -> async Config =
    config.get_or_init(|| load_config()).await
```

---

## Sleep

### @sleep

```sigil
@sleep (duration: Duration) -> async void
```

Pauses for duration.

```sigil
use std.async { sleep }

@retry_with_backoff<T> (f: () -> async Result<T, Error>, attempts: int) -> async Result<T, Error> = run(
    for i in 0..attempts do
        match(f().await,
            Ok(v) -> return Ok(v),
            Err(_) if i < attempts - 1 -> sleep(100ms * pow(2, i)).await,
            Err(e) -> return Err(e),
        ),
)
```

---

## Examples

### Concurrent fetching with limit

```sigil
use std.async { Semaphore, spawn, join }

@fetch_all (urls: [str], max_concurrent: int) -> async [Result<Data, Error>] = run(
    let sem = Semaphore.new(max_concurrent),
    let tasks = urls | map(_, url -> spawn(|| run(
        sem.acquire().await,
        let result = fetch(url).await,
        sem.release(),
        result,
    ))),
    join(tasks).await,
)
```

### Racing with timeout

```sigil
use std.async { select, timeout }

@fetch_with_fallback (primary: str, backup: str) -> async Result<Data, Error> = run(
    let (_, result) = select([
        timeout(fetch(primary), 5s),
        timeout(fetch(backup), 10s),
    ]).await,
    result,
)
```

### Worker pool

```sigil
use std.async { spawn, join }

@process_batch<T, R> (
    items: [T],
    workers: int,
    process: T -> async R
) -> async [R] = run(
    let chunks = items.chunks(items.len() / workers + 1),
    let tasks = chunks | map(_, chunk ->
        spawn(|| map(chunk, process) | join(_))
    ),
    join(tasks).await | flatten(_),
)
```

---

## See Also

- [Channel](../prelude.md) — Built-in channels
- [Design: Async](../../design/10-async/) — Async design
- [Capabilities](../../spec/14-capabilities.md) — Async capabilities
