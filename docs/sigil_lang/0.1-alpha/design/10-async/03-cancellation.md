# Cancellation

This document covers Sigil's cancellation model: context-based cancellation, explicit check points, and how timeouts integrate with the context system.

---

## Overview

Sigil uses **cooperative cancellation** via a `Context` object. Tasks check for cancellation at explicit points and clean up gracefully.

```sigil
@long_operation (ctx: Context) -> Result<Data, Error> uses Async, Http = try(
    let data1 = fetch_part1(ctx)?,
    ctx.check_cancelled()?,  // throws if cancelled
    let data2 = fetch_part2(ctx)?,
    Ok(combine(data1, data2)),
)
```

Key principles:
- Cancellation is cooperative, not preemptive
- Explicit check points make cancellation visible
- Context carries timeout and cancellation signal
- No silent task killing (prevents resource leaks)

---

## The Context Type

### What Is a Context?

A `Context` carries:
- **Cancellation signal** - Whether cancellation was requested
- **Deadline** - Optional time limit
- **Values** - Optional request-scoped data

```sigil
// Create a context with a timeout
ctx = Context.with_timeout(30s)

// Create a context that cancels on signal
ctx = Context.with_signal(SIGTERM)

// Create a child context with shorter timeout
child_ctx = ctx.with_timeout(5s)
```

### Context Hierarchy

Contexts form a hierarchy. Cancelling a parent cancels all children:

```sigil
@main () -> void uses Async, Clock = run(
    let parent_ctx = Context.with_timeout(60s),

    parallel(
        .task_a: operation_a(parent_ctx),
        .task_b: operation_b(parent_ctx),
    ),
    // If parent_ctx times out, both tasks are cancelled
)
```

Child contexts can have shorter (but not longer) timeouts:

```sigil
@operation_a (ctx: Context) -> Result<Data, Error> uses Async = run(
    // Child timeout must be <= parent timeout
    child_ctx = ctx.with_timeout(10s),
    sub_operation(child_ctx)
)
```

---

## Checking for Cancellation

### The `check_cancelled` Method

Use `check_cancelled()` to check if cancellation was requested:

```sigil
@long_computation (ctx: Context) -> Result<Data, Error> uses Async = try(
    let step1 = perform_step1()?,
    ctx.check_cancelled()?,  // returns Err(Cancelled) if cancelled

    let step2 = perform_step2()?,
    ctx.check_cancelled()?,

    let step3 = perform_step3()?,
    Ok(combine(step1, step2, step3)),
)
```

### What `check_cancelled` Returns

```sigil
// Returns Result<void, CancelledError>
ctx.check_cancelled()

// In try block, ? propagates the error
ctx.check_cancelled()?

// Or check explicitly
if ctx.is_cancelled() then Err(CancelledError {})
else continue_work()
```

### Where to Check

Check for cancellation:
- Between expensive operations
- At loop iterations
- Before starting new work
- After I/O operations

```sigil
@process_items (ctx: Context, items: [Item]) -> Result<[Result<ProcessedItem, Error>], Error> uses Async = run(
    let results = [],
    for item in items do run(
        ctx.check_cancelled()?,  // check each iteration
        let result = process(item),
        results = results.append(result),
    ),
    Ok(results),
)
```

---

## Timeouts

### Creating Timeout Contexts

```sigil
// Context that cancels after duration
ctx = Context.with_timeout(30s)

// Context with absolute deadline
ctx = Context.with_deadline(timestamp)

// Derive shorter timeout from existing context
child = ctx.with_timeout(5s)  // 5s or parent deadline, whichever is sooner
```

### Timeout Carries Context

The `timeout` pattern automatically manages context:

```sigil
@bounded_fetch (url: str) -> Result<Data, Error> uses Async, Http, Clock = timeout(
    .op: Http.get(url),
    .after: 30s,
    .on_timeout: Err(TimeoutError { url: url })
)
```

This is equivalent to:

```sigil
@bounded_fetch (url: str) -> Result<Data, Error> uses Async, Http, Clock = run(
    let ctx = Context.with_timeout(30s),
    let result = Http.get_with_context(ctx, url),
    if ctx.is_cancelled() then Err(TimeoutError { url: url })
    else result,
)
```

### Nested Timeouts

Timeouts nest naturally:

```sigil
@outer () -> Result<Data, Error> uses Async, Http, Clock = timeout(
    .op: inner(),
    .after: 60s,
    .on_timeout: Err(OuterTimeout {})
)

@inner () -> Result<Data, Error> uses Async, Http, Clock = timeout(
    .op: fetch_data(),
    .after: 10s,  // effective timeout is min(10s, remaining outer timeout)
    .on_timeout: Err(InnerTimeout {})
)
```

---

## Patterns with Cancellation

### Retry with Cancellation

```sigil
@reliable_fetch (ctx: Context, url: str) -> Result<Data, Error> uses Async, Http, Clock = retry(
    .op: run(
        ctx.check_cancelled()?,
        Http.get(url)
    ),
    .attempts: 3,
    .backoff: exponential(base: 100ms, max: 5s)
)
```

### Parallel with Cancellation

When one task fails, others are cancelled:

```sigil
@fetch_all (ctx: Context) -> Result<(Data, Data, Data), Error> uses Async, Http = run(
    let results = parallel(
        .a: fetch_a(ctx),
        .b: fetch_b(ctx),
        .c: fetch_c(ctx),
    ),
    // If any fails or ctx is cancelled, others are cancelled
    Ok((results.a, results.b, results.c)),
)
```

### Long-Running Operations

Break long operations into cancellable chunks:

```sigil
@process_large_file (ctx: Context, path: str) -> Result<Summary, Error> uses Async, FileSystem = try(
    let file = FileSystem.open(path)?,
    let summary = Summary.empty(),

    for chunk in file.chunks(1000) do run(
        ctx.check_cancelled()?,  // check each chunk
        let partial = process_chunk(chunk),
        summary = summary.merge(partial),
    ),

    Ok(summary),
)
```

---

## Graceful Shutdown

### Signal-Based Cancellation

Create a context that cancels on OS signals:

```sigil
@main () -> void uses Async, Signal = run(
    let ctx = Context.with_signal(SIGTERM, SIGINT),

    parallel(
        .server: run_server(ctx),
        .worker: run_worker(ctx),
    ),

    // SIGTERM or SIGINT will cancel ctx
    // Tasks should check and shut down gracefully
)
```

### Cleanup on Cancellation

Use `with` for cleanup that must run even on cancellation:

```sigil
@process_with_cleanup (ctx: Context) -> Result<Data, Error> uses Async, Network = with(
    .acquire: Network.open_connection(),
    .use: conn ->
        loop(
            ctx.check_cancelled()?,
            process_next(conn)
        ),
    .release: conn -> Network.close_connection(conn)  // always runs
)
```

### Shutdown Timeout

Give tasks time to clean up:

```sigil
@main () -> void uses Async, Signal = run(
    let ctx = Context.with_signal(SIGTERM),

    // Run main work
    let result = parallel(
        .server: run_server(ctx),
        .worker: run_worker(ctx),
    ),

    // On signal, ctx is cancelled
    // Tasks have a grace period to finish cleanup
)
```

---

## Context Values

### Passing Request-Scoped Data

Context can carry values through the call chain:

```sigil
@handle_request (request: Request) -> Response uses Async, Http, Clock = run(
    let ctx = Context.new()
        .with_value("request_id", request.id)
        .with_value("user_id", request.user_id)
        .with_timeout(30s),

    process_request(ctx, request),
)

@process_request (ctx: Context, request: Request) -> Response uses Async, Log = run(
    let request_id = ctx.get_value("request_id"),
    Log.info("Processing request: " + request_id),
    do_work(ctx),
)
```

### Tracing and Logging

Context is ideal for distributed tracing:

```sigil
@traced_operation (ctx: Context) -> Result<Data, Error> uses Async, Log = run(
    let trace_id = ctx.get_value("trace_id"),
    let span_id = generate_span_id(),

    let child_ctx = ctx.with_value("span_id", span_id),

    log_span_start(trace_id, span_id),
    let result = perform_operation(child_ctx),
    log_span_end(trace_id, span_id),

    result,
)
```

---

## Why Cooperative Cancellation?

### The Problem with Preemptive Cancellation

Some languages allow killing tasks immediately:

```python
# Python - can leave resources in bad state
task.cancel()  # task is killed immediately
# What if task was in the middle of writing a file?
# What if task held a lock?
```

### Sigil's Approach

Cooperative cancellation means:
1. Tasks are **notified** of cancellation
2. Tasks **choose** when to stop
3. Tasks can **clean up** before exiting
4. Resources are **properly released**

```sigil
@safe_operation (ctx: Context) -> Result<Data, Error> uses Async = with(
    .acquire: get_resource(),
    .use: resource ->
        loop(
            // Check for cancellation
            if ctx.is_cancelled() then break,
            // Do work
            process(resource)
        ),
    .release: resource -> release_resource(resource)
    // Always released, even on cancellation
)
```

### Trade-offs

| Aspect | Preemptive | Cooperative |
|--------|------------|-------------|
| Speed of cancellation | Immediate | Depends on check frequency |
| Resource safety | Risky | Safe |
| Code complexity | Lower | Slightly higher |
| Predictability | Lower | Higher |

---

## Best Practices

### Check Often in Long Operations

```sigil
// Good: check between operations
@long_task (ctx: Context) -> Result<Data, Error> uses Async = try(
    step1(),
    ctx.check_cancelled()?,
    step2(),
    ctx.check_cancelled()?,
    step3(),
    Ok(result)
)

// Bad: no cancellation checks
@long_task_no_check () -> Result<Data, Error> uses Async = try(
    step1(),
    step2(),
    step3(),  // might run even after cancellation requested
    Ok(result)
)
```

### Pass Context Through Call Chain

```sigil
// Good: context flows through
@top (ctx: Context) -> Result<Data, Error> uses Async =
    middle(ctx)

@middle (ctx: Context) -> Result<Data, Error> uses Async =
    bottom(ctx)

@bottom (ctx: Context) -> Result<Data, Error> uses Async = try(
    ctx.check_cancelled()?,
    do_work()
)
```

### Use Appropriate Timeouts

```sigil
// Good: specific timeouts for different operations
@process (ctx: Context) -> Result<Data, Error> uses Async, Database, Http, Clock = run(
    // Database query: short timeout
    let db_ctx = ctx.with_timeout(5s),
    let data = Database.query(db_ctx),

    // External API: longer timeout
    let api_ctx = ctx.with_timeout(30s),
    let external = Http.call_api(api_ctx, data),

    Ok(combine(data, external)),
)
```

### Handle Cancellation Gracefully

```sigil
@graceful (ctx: Context) -> Result<Data, Error> uses Async, Log = run(
    let result = try(
        work(ctx),
    ),
    match(result,
        Ok(data) -> Ok(data),
        Err(CancelledError) -> run(
            Log.info("Operation cancelled, cleaning up"),
            cleanup(),
            Err(CancelledError {}),
        ),
        Err(e) -> Err(e),
    ),
)
```

---

## Error Messages

### Missing Cancellation Check

```
warning[W0300]: long operation without cancellation check
  --> src/main.si:10:5
   |
10 |     for item in large_list do process(item),
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ loop may run indefinitely
   |
   = note: consider adding `ctx.check_cancelled()?` in loop body
```

### Context Not Passed

```
error[E0301]: function with Async capability called without context
  --> src/main.si:15:10
   |
15 |     result = long_operation()
   |              ^^^^^^^^^^^^^^^^ expected Context parameter
   |
   = note: pass parent context: `long_operation(ctx)`
```

### Timeout Exceeds Parent

```
error[E0302]: child timeout exceeds parent
  --> src/main.si:8:20
   |
8  |     child = ctx.with_timeout(60s)
   |                              ^^^ child timeout 60s exceeds parent remaining 30s
   |
   = note: child timeout will be clamped to parent deadline
```

---

## See Also

- [Async/Await](01-async-await.md)
- [Structured Concurrency](02-structured-concurrency.md)
- [Channels](04-channels.md)
- [Error Handling](../05-error-handling/index.md)
