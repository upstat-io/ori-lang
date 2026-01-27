# Cancellation

This document covers Ori's cancellation model: context-based cancellation, explicit check points, and how timeouts integrate with the context system.

---

## Overview

Ori uses **cooperative cancellation** via a `Context` object. Tasks check for cancellation at explicit points and clean up gracefully.

```ori
// The check_cancelled call throws if cancelled
@long_operation (context: Context) -> Result<Data, Error> uses Async, Http = try(
    let data1 = fetch_part1(
        .context: context,
    )?,
    context.check_cancelled()?,
    let data2 = fetch_part2(
        .context: context,
    )?,
    Ok(combine(
        .first: data1,
        .second: data2,
    )),
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

```ori
// Create a context with a timeout
context = Context.with_timeout(30s)

// Create a context that cancels on signal
context = Context.with_signal(SIGTERM)

// Create a child context with shorter timeout
child_context = context.with_timeout(5s)
```

### Context Hierarchy

Contexts form a hierarchy. Cancelling a parent cancels all children:

```ori
// If parent_context times out, both tasks are cancelled
@main () -> void uses Async, Clock = run(
    let parent_context = Context.with_timeout(60s),

    parallel(
        .task_a: operation_a(
            .context: parent_context,
        ),
        .task_b: operation_b(
            .context: parent_context,
        ),
    ),
)
```

Child contexts can have shorter (but not longer) timeouts:

```ori
// Child timeout must be <= parent timeout
@operation_a (context: Context) -> Result<Data, Error> uses Async = run(
    child_context = context.with_timeout(10s),
    sub_operation(
        .context: child_context,
    ),
)
```

---

## Checking for Cancellation

### The `check_cancelled` Method

Use `check_cancelled()` to check if cancellation was requested:

```ori
// check_cancelled returns Err(Cancelled) if cancelled
@long_computation (context: Context) -> Result<Data, Error> uses Async = try(
    let step1 = perform_step1()?,
    context.check_cancelled()?,

    let step2 = perform_step2()?,
    context.check_cancelled()?,

    let step3 = perform_step3()?,
    Ok(combine(
        .first: step1,
        .second: step2,
        .third: step3,
    )),
)
```

### What `check_cancelled` Returns

```ori
// Returns Result<void, CancelledError>
context.check_cancelled()

// In try block, ? propagates the error
context.check_cancelled()?

// Or check explicitly
if context.is_cancelled() then Err(CancelledError {})
else continue_work()
```

### Where to Check

Check for cancellation:
- Between expensive operations
- At loop iterations
- Before starting new work
- After I/O operations

```ori
// Check for cancellation each iteration
@process_items (context: Context, items: [Item]) -> Result<[Result<ProcessedItem, Error>], Error> uses Async = run(
    let results = [],
    for item in items do run(
        context.check_cancelled()?,
        let result = process(
            .item: item,
        ),
        results = results.append(
            .element: result,
        ),
    ),
    Ok(results),
)
```

---

## Timeouts

### Creating Timeout Contexts

```ori
// Context that cancels after duration
context = Context.with_timeout(30s)

// Context with absolute deadline
context = Context.with_deadline(timestamp)

// Derive shorter timeout from existing context
// Uses 5s or parent deadline, whichever is sooner
child_context = context.with_timeout(5s)
```

### Timeout Carries Context

The `timeout` pattern automatically manages context:

```ori
@bounded_fetch (url: str) -> Result<Data, Error> uses Async, Http, Clock = timeout(
    .operation: Http.get(url),
    .after: 30s,
    .on_timeout: Err(TimeoutError { url: url })
)
```

This is equivalent to:

```ori
@bounded_fetch (url: str) -> Result<Data, Error> uses Async, Http, Clock = run(
    let context = Context.with_timeout(30s),
    let result = Http.get_with_context(
        .context: context,
        .url: url,
    ),
    if context.is_cancelled() then Err(TimeoutError { url: url })
    else result,
)
```

### Nested Timeouts

Timeouts nest naturally:

```ori
@outer () -> Result<Data, Error> uses Async, Http, Clock = timeout(
    .operation: inner(),
    .after: 60s,
    .on_timeout: Err(OuterTimeout {})
)

// Effective timeout is min(10s, remaining outer timeout)
@inner () -> Result<Data, Error> uses Async, Http, Clock = timeout(
    .operation: fetch_data(),
    .after: 10s,
    .on_timeout: Err(InnerTimeout {}),
)
```

---

## Patterns with Cancellation

### Retry with Cancellation

```ori
@reliable_fetch (context: Context, url: str) -> Result<Data, Error> uses Async, Http, Clock = retry(
    .operation: run(
        context.check_cancelled()?,
        Http.get(
            .url: url,
        ),
    ),
    .attempts: 3,
    .backoff: exponential(
        .base: 100ms,
        .max: 5s,
    ),
)
```

### Parallel with Cancellation

When one task fails, others are cancelled:

```ori
// If any task fails or context is cancelled, other tasks are cancelled
@fetch_all (context: Context) -> Result<(Data, Data, Data), Error> uses Async, Http = run(
    let results = parallel(
        .a: fetch_a(
            .context: context,
        ),
        .b: fetch_b(
            .context: context,
        ),
        .c: fetch_c(
            .context: context,
        ),
    ),
    Ok((results.a, results.b, results.c)),
)
```

### Long-Running Operations

Break long operations into cancellable chunks:

```ori
// Check for cancellation after each chunk
@process_large_file (context: Context, path: str) -> Result<Summary, Error> uses Async, FileSystem = try(
    let file = FileSystem.open(
        .path: path,
    )?,
    let summary = Summary.empty(),

    for chunk in file.chunks(1000) do run(
        context.check_cancelled()?,
        let partial = process_chunk(
            .chunk: chunk,
        ),
        summary = summary.merge(
            .other: partial,
        ),
    ),

    Ok(summary),
)
```

---

## Graceful Shutdown

### Signal-Based Cancellation

Create a context that cancels on OS signals:

```ori
// SIGTERM or SIGINT will cancel the context
// Tasks should check and shut down gracefully
@main () -> void uses Async, Signal = run(
    let context = Context.with_signal(SIGTERM, SIGINT),

    parallel(
        .server: run_server(
            .context: context,
        ),
        .worker: run_worker(
            .context: context,
        ),
    ),
)
```

### Cleanup on Cancellation

Use `with` for cleanup that must run even on cancellation:

```ori
// The release function always runs
@process_with_cleanup (context: Context) -> Result<Data, Error> uses Async, Network = with(
    .acquire: Network.open_connection(),
    .use: connection ->
        loop(
            context.check_cancelled()?,
            process_next(
                .connection: connection,
            ),
        ),
    .release: connection -> Network.close_connection(
        .connection: connection,
    ),
)
```

### Shutdown Timeout

Give tasks time to clean up:

```ori
// On signal, context is cancelled
// Tasks have a grace period to finish cleanup
@main () -> void uses Async, Signal = run(
    let context = Context.with_signal(SIGTERM),

    // Run main work
    let result = parallel(
        .server: run_server(
            .context: context,
        ),
        .worker: run_worker(
            .context: context,
        ),
    ),
)
```

---

## Context Values

### Passing Request-Scoped Data

Context can carry values through the call chain:

```ori
@handle_request (request: Request) -> Response uses Async, Http, Clock = run(
    let context = Context.new()
        .with_value("request_id", request.id)
        .with_value("user_id", request.user_id)
        .with_timeout(30s),

    process_request(
        .context: context,
        .request: request,
    ),
)

@process_request (context: Context, request: Request) -> Response uses Async, Log = run(
    let request_id = context.get_value("request_id"),
    Log.info(
        .message: "Processing request: " + request_id,
    ),
    do_work(
        .context: context,
    ),
)
```

### Tracing and Logging

Context is ideal for distributed tracing:

```ori
@traced_operation (context: Context) -> Result<Data, Error> uses Async, Log = run(
    let trace_id = context.get_value("trace_id"),
    let span_id = generate_span_id(),

    let child_context = context.with_value("span_id", span_id),

    log_span_start(
        .trace_id: trace_id,
        .span_id: span_id,
    ),
    let result = perform_operation(
        .context: child_context,
    ),
    log_span_end(
        .trace_id: trace_id,
        .span_id: span_id,
    ),

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

### Ori's Approach

Cooperative cancellation means:
1. Tasks are **notified** of cancellation
2. Tasks **choose** when to stop
3. Tasks can **clean up** before exiting
4. Resources are **properly released**

```ori
// Resource is always released, even on cancellation
@safe_operation (context: Context) -> Result<Data, Error> uses Async = with(
    .acquire: get_resource(),
    .use: resource ->
        loop(
            // Check for cancellation
            if context.is_cancelled() then break,
            // Do work
            process(
                .resource: resource,
            ),
        ),
    .release: resource -> release_resource(
        .resource: resource,
    ),
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

```ori
// Good: check between operations
@long_task (context: Context) -> Result<Data, Error> uses Async = try(
    step1(),
    context.check_cancelled()?,
    step2(),
    context.check_cancelled()?,
    step3(),
    Ok(result),
)

// Bad: no cancellation checks, might run even after cancellation requested
@long_task_no_check () -> Result<Data, Error> uses Async = try(
    step1(),
    step2(),
    step3(),
    Ok(result),
)
```

### Pass Context Through Call Chain

```ori
// Good: context flows through
@top (context: Context) -> Result<Data, Error> uses Async =
    middle(
        .context: context,
    )

@middle (context: Context) -> Result<Data, Error> uses Async =
    bottom(
        .context: context,
    )

@bottom (context: Context) -> Result<Data, Error> uses Async = try(
    context.check_cancelled()?,
    do_work(),
)
```

### Use Appropriate Timeouts

```ori
// Good: specific timeouts for different operations
@process (context: Context) -> Result<Data, Error> uses Async, Database, Http, Clock = run(
    // Database query: short timeout
    let db_context = context.with_timeout(5s),
    let data = Database.query(
        .context: db_context,
    ),

    // External API: longer timeout
    let api_context = context.with_timeout(30s),
    let external = Http.call_api(
        .context: api_context,
        .data: data,
    ),

    Ok(combine(
        .first: data,
        .second: external,
    )),
)
```

### Handle Cancellation Gracefully

```ori
@graceful (context: Context) -> Result<Data, Error> uses Async, Log = run(
    let result = try(
        work(
            .context: context,
        ),
    ),
    match(result,
        Ok(data) -> Ok(data),
        Err(CancelledError) -> run(
            Log.info(
                .message: "Operation cancelled, cleaning up",
            ),
            cleanup(),
            Err(CancelledError {}),
        ),
        Err(error) -> Err(error),
    ),
)
```

---

## Error Messages

### Missing Cancellation Check

```
warning[W0300]: long operation without cancellation check
  --> src/main.ori:10:5
   |
10 |     for item in large_list do process(item),
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ loop may run indefinitely
   |
   = note: consider adding `ctx.check_cancelled()?` in loop body
```

### Context Not Passed

```
error[E0301]: function with Async capability called without context
  --> src/main.ori:15:10
   |
15 |     result = long_operation()
   |              ^^^^^^^^^^^^^^^^ expected Context parameter
   |
   = note: pass parent context: `long_operation(ctx)`
```

### Timeout Exceeds Parent

```
error[E0302]: child timeout exceeds parent
  --> src/main.ori:8:20
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
