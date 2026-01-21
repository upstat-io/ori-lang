# Cancellation

This document covers Sigil's cancellation model: context-based cancellation, explicit check points, and how timeouts integrate with the context system.

---

## Overview

Sigil uses **cooperative cancellation** via a `Context` object. Tasks check for cancellation at explicit points and clean up gracefully.

```sigil
@long_operation (ctx: Context) -> async Result<Data, Error> = try(
    let data1 = fetch_part1(ctx).await?,
    ctx.check_cancelled()?,  // throws if cancelled
    let data2 = fetch_part2(ctx).await?,
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
@main () -> async void = run(
    let parent_ctx = Context.with_timeout(60s),

    parallel(
        .task_a: operation_a(parent_ctx),
        .task_b: operation_b(parent_ctx),
    ).await,
    // If parent_ctx times out, both tasks are cancelled
)
```

Child contexts can have shorter (but not longer) timeouts:

```sigil
@operation_a (ctx: Context) -> async Result<Data, Error> = run(
    // Child timeout must be <= parent timeout
    child_ctx = ctx.with_timeout(10s),
    sub_operation(child_ctx).await
)
```

---

## Checking for Cancellation

### The `check_cancelled` Method

Use `check_cancelled()` to check if cancellation was requested:

```sigil
@long_computation (ctx: Context) -> async Result<Data, Error> = try(
    let step1 = perform_step1().await?,
    ctx.check_cancelled()?,  // returns Err(Cancelled) if cancelled

    let step2 = perform_step2().await?,
    ctx.check_cancelled()?,

    let step3 = perform_step3().await?,
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
@process_items (ctx: Context, items: [Item]) -> async Result<[Result<ProcessedItem, Error>], Error> = run(
    let results = [],
    for item in items do run(
        ctx.check_cancelled()?,  // check each iteration
        let result = process(item).await,
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
@bounded_fetch (url: str) -> async Result<Data, Error> = timeout(
    .op: http_get(url).await,
    .after: 30s,
    .on_timeout: Err(TimeoutError { url: url })
)
```

This is equivalent to:

```sigil
@bounded_fetch (url: str) -> async Result<Data, Error> = run(
    let ctx = Context.with_timeout(30s),
    let result = http_get_with_context(ctx, url).await,
    if ctx.is_cancelled() then Err(TimeoutError { url: url })
    else result,
)
```

### Nested Timeouts

Timeouts nest naturally:

```sigil
@outer () -> async Result<Data, Error> = timeout(
    .op: inner().await,
    .after: 60s,
    .on_timeout: Err(OuterTimeout {})
)

@inner () -> async Result<Data, Error> = timeout(
    .op: fetch_data().await,
    .after: 10s,  // effective timeout is min(10s, remaining outer timeout)
    .on_timeout: Err(InnerTimeout {})
)
```

---

## Patterns with Cancellation

### Retry with Cancellation

```sigil
@reliable_fetch (ctx: Context, url: str) -> async Result<Data, Error> = retry(
    .op: run(
        ctx.check_cancelled()?,
        http_get(url).await
    ),
    .attempts: 3,
    .backoff: exponential(base: 100ms, max: 5s)
)
```

### Parallel with Cancellation

When one task fails, others are cancelled:

```sigil
@fetch_all (ctx: Context) -> async Result<(Data, Data, Data), Error> = run(
    let results = parallel(
        .a: fetch_a(ctx),
        .b: fetch_b(ctx),
        .c: fetch_c(ctx),
    ).await,
    // If any fails or ctx is cancelled, others are cancelled
    Ok((results.a, results.b, results.c)),
)
```

### Long-Running Operations

Break long operations into cancellable chunks:

```sigil
@process_large_file (ctx: Context, path: str) -> async Result<Summary, Error> = try(
    let file = open_file(path).await?,
    let summary = Summary.empty(),

    for chunk in file.chunks(1000) do run(
        ctx.check_cancelled()?,  // check each chunk
        let partial = process_chunk(chunk).await,
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
@main () -> async void = run(
    let ctx = Context.with_signal(SIGTERM, SIGINT),

    parallel(
        .server: run_server(ctx),
        .worker: run_worker(ctx),
    ).await,

    // SIGTERM or SIGINT will cancel ctx
    // Tasks should check and shut down gracefully
)
```

### Cleanup on Cancellation

Use `with` for cleanup that must run even on cancellation:

```sigil
@process_with_cleanup (ctx: Context) -> async Result<Data, Error> = with(
    .acquire: open_connection().await,
    .use: conn ->
        loop(
            ctx.check_cancelled()?,
            process_next(conn).await
        ),
    .release: conn -> close_connection(conn).await  // always runs
)
```

### Shutdown Timeout

Give tasks time to clean up:

```sigil
@main () -> async void = run(
    let ctx = Context.with_signal(SIGTERM),

    // Run main work
    let result = parallel(
        .server: run_server(ctx),
        .worker: run_worker(ctx),
    ).await,

    // On signal, ctx is cancelled
    // Tasks have a grace period to finish cleanup
)
```

---

## Context Values

### Passing Request-Scoped Data

Context can carry values through the call chain:

```sigil
@handle_request (request: Request) -> async Response = run(
    let ctx = Context.new()
        .with_value("request_id", request.id)
        .with_value("user_id", request.user_id)
        .with_timeout(30s),

    process_request(ctx, request).await,
)

@process_request (ctx: Context, request: Request) -> async Response = run(
    let request_id = ctx.get_value("request_id"),
    log("Processing request: " + request_id),
    do_work(ctx).await,
)
```

### Tracing and Logging

Context is ideal for distributed tracing:

```sigil
@traced_operation (ctx: Context) -> async Result<Data, Error> = run(
    let trace_id = ctx.get_value("trace_id"),
    let span_id = generate_span_id(),

    let child_ctx = ctx.with_value("span_id", span_id),

    log_span_start(trace_id, span_id),
    let result = perform_operation(child_ctx).await,
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
@safe_operation (ctx: Context) -> async Result<Data, Error> = with(
    .acquire: get_resource().await,
    .use: resource ->
        loop(
            // Check for cancellation
            if ctx.is_cancelled() then break,
            // Do work
            process(resource).await
        ),
    .release: resource -> release_resource(resource).await
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
@long_task (ctx: Context) -> async Result<Data, Error> = try(
    step1().await,
    ctx.check_cancelled()?,
    step2().await,
    ctx.check_cancelled()?,
    step3().await,
    Ok(result)
)

// Bad: no cancellation checks
@long_task_no_check () -> async Result<Data, Error> = try(
    step1().await,
    step2().await,
    step3().await,  // might run even after cancellation requested
    Ok(result)
)
```

### Pass Context Through Call Chain

```sigil
// Good: context flows through
@top (ctx: Context) -> async Result<Data, Error> =
    middle(ctx).await

@middle (ctx: Context) -> async Result<Data, Error> =
    bottom(ctx).await

@bottom (ctx: Context) -> async Result<Data, Error> = try(
    ctx.check_cancelled()?,
    do_work().await
)
```

### Use Appropriate Timeouts

```sigil
// Good: specific timeouts for different operations
@process (ctx: Context) -> async Result<Data, Error> = run(
    // Database query: short timeout
    let db_ctx = ctx.with_timeout(5s),
    let data = query_db(db_ctx).await,

    // External API: longer timeout
    let api_ctx = ctx.with_timeout(30s),
    let external = call_api(api_ctx, data).await,

    Ok(combine(data, external)),
)
```

### Handle Cancellation Gracefully

```sigil
@graceful (ctx: Context) -> async Result<Data, Error> = run(
    let result = try(
        work(ctx).await,
    ),
    match(result,
        Ok(data) -> Ok(data),
        Err(CancelledError) -> run(
            log("Operation cancelled, cleaning up"),
            cleanup().await,
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
error[E0301]: async function called without context
  --> src/main.si:15:10
   |
15 |     result = long_operation().await
   |              ^^^^^^^^^^^^^^^^ expected Context parameter
   |
   = note: pass parent context: `long_operation(ctx).await`
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
