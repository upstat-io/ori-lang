# Structured Concurrency

This document covers Ori's structured concurrency model: why there are no detached tasks, how `parallel` ensures all tasks complete, and why fire-and-forget is not allowed.

---

## Overview

Ori enforces **structured concurrency**: all concurrent tasks must be awaited, and parent operations always wait for their children to complete.

```ori
// Structured: parallel waits for all tasks
@fetch_all () -> [Data] uses Http, Async = parallel(
    .tasks: [fetch_a(), fetch_b(), fetch_c()]
)

// NOT allowed: fire-and-forget
@bad () -> void = run(
    // ERROR: detached tasks not allowed
    spawn(background_task()),
    print("done")
)
```

---

## What Is Structured Concurrency?

Structured concurrency means:

1. **Every concurrent task has a parent** that waits for it
2. **No orphan tasks** - tasks cannot outlive their parent scope
3. **Predictable lifetimes** - when a function returns, all its spawned work is done
4. **Hierarchical cancellation** - cancelling a parent cancels its children

Think of it like structured programming for concurrency: just as functions return to their caller, concurrent tasks return to their spawner.

### The Concurrency Hierarchy

```ori
@main () -> void uses Async = run(
    // main waits for process_all
    let result = process_all(),
    print(result),
)

@process_all () -> [Data] uses Async = run(
    // process_all waits for parallel
    let data = parallel(
        .tasks: [task_a(), task_b(), task_c()],
    ),
    // All three tasks complete before we continue
    transform(data),
)
```

When `main` completes, everything it started has completed.

---

## Why No Detached Tasks?

### The Problem with Fire-and-Forget

In many languages, you can spawn background tasks that continue after the spawning function returns:

```python
# Python - dangerous pattern
def handle_request():
    asyncio.create_task(log_analytics())  # fire and forget
    return "done"
    # What if log_analytics() fails?
    # What if the server shuts down?
    # Who handles the error?
```

This causes problems:

1. **Lost errors** - Background task failures go unnoticed
2. **Resource leaks** - Tasks may hold connections, files, memory
3. **Unpredictable shutdown** - How do you wait for all background work?
4. **Race conditions** - Task may access resources after they're freed

### Ori's Solution

Ori simply doesn't allow detached tasks:

```ori
// ERROR: spawn() does not exist
@bad () -> void = spawn(background_task())
```

If you need concurrent work, use `parallel`:

```ori
@good () -> void uses Async = run(
    // Explicitly wait for all concurrent work
    parallel(
        .main_work: handle_request(),
        .analytics: log_analytics(),
    ),
    print("all done"),
)
```

---

## The `parallel` Pattern

`parallel` is Ori's primary concurrency primitive. It runs multiple tasks concurrently and waits for all to complete.

### Basic Usage

Named tasks with struct result:

```ori
@fetch_dashboard (user_id: int) -> Dashboard uses Http, Async = run(
    let data = parallel(
        .user: get_user(user_id),
        .posts: get_posts(user_id),
        .notifications: get_notifications(user_id),
    ),
    Dashboard {
        user: data.user,
        posts: data.posts,
        notifications: data.notifications,
    },
)
```

List of tasks:

```ori
@fetch_all (urls: [str]) -> [Data] uses Http, Async = parallel(
    .tasks: map(
        .over: urls,
        .transform: url -> fetch(
            .url: url,
        ),
    ),
)
```

### Concurrency Limits

Control maximum concurrent tasks:

```ori
@fetch_many (urls: [str]) -> [Data] uses Http, Async = parallel(
    .tasks: map(
        .over: urls,
        .transform: url -> fetch(
            .url: url,
        ),
    ),
    // at most 10 fetches at once
    .max_concurrent: 10,
)
```

Why limit concurrency?
- Prevent overwhelming servers
- Control resource usage (connections, memory)
- Avoid rate limiting

### Error Handling in Parallel

By default, `parallel` fails fast - one failure stops everything:

```ori
@fetch_both () -> Result<(Data, Data), Error> uses Http, Async = parallel(
    // if this fails, the other task is cancelled
    .a: fetch_a(),
    .b: fetch_b(),
)
```

Collect all errors instead:

```ori
@fetch_all (urls: [str]) -> [Result<Data, Error>] uses Http, Async = parallel(
    .tasks: map(
        .over: urls,
        .transform: url -> fetch(
            .url: url,
        ),
    ),
    // collect successes and failures
    .on_error: collect_all,
)
```

### Timeouts

Add a timeout to parallel operations:

```ori
@fetch_with_limit () -> Result<Data, Error> uses Http, Clock, Async = parallel(
    .a: fetch_slow(),
    .b: fetch_fast(),
    // cancel all if not done in 5 seconds
    .timeout: 5s,
)
```

---

## Structured Concurrency Guarantees

### Guarantee 1: No Orphan Tasks

When a function returns, all tasks it started have completed:

```ori
@process () -> Result uses Async = run(
    parallel(
        .task_a: slow_operation(),
        .task_b: fast_operation(),
    ),
    // Both task_a and task_b are DONE here
    Ok(result),
)
// No tasks from process() are still running
```

### Guarantee 2: Hierarchical Cancellation

Cancelling a parent cancels all children:

```ori
@outer () -> void uses Clock, Async = run(
    timeout(
        .operation: inner(),
        .after: 5s,
        .on_timeout: handle_timeout(),
    ),
)

// All tasks are cancelled if outer times out
@inner () -> void uses Async = parallel(
    .a: task_a(),
    .b: task_b(),
    .c: task_c(),
)
```

### Guarantee 3: Error Propagation

Errors bubble up through the hierarchy:

```ori
@main () -> void uses Async = run(
    let result = top_level(),
    match(result,
        Ok(data) -> print(
            .message: "Success: " + str(data),
        ),
        Err(error) -> print(
            .message: "Failed: " + str(error),
        ),
    ),
)

// Errors propagate up the hierarchy
@top_level () -> Result<Data, Error> uses Async = parallel(
    .a: middle_a(),
    .b: middle_b(),
)

// Errors from leaf functions propagate to top_level
@middle_a () -> Result<Data, Error> uses Async = parallel(
    .x: leaf_x(),
    .y: leaf_y(),
)
```

---

## Patterns for Common Scenarios

### Background Work That Must Complete

Instead of fire-and-forget, make background work explicit:

```ori
// BAD: fire-and-forget (not allowed)
// ERROR: spawn is not allowed
@handle_request () -> Response uses Http, Async = run(
    spawn(log_analytics()),
    compute_response(),
)

// GOOD: explicit concurrent work
@handle_request () -> Response uses Http, Async = run(
    let results = parallel(
        .response: compute_response(),
        .analytics: log_analytics(),
    ),
    results.response,
)
```

### Cleanup That Must Run

Use `with` for cleanup instead of spawning cleanup tasks:

```ori
// The release function always runs, even on cancellation
@process_file (path: str) -> Result<Data, Error> uses FileSystem, Async = with(
    .acquire: open_file(
        .path: path,
    ),
    .use: file -> process(
        .file: file,
    ),
    .release: file -> close_file(
        .file: file,
    ),
)
```

### Worker Pools

For processing a queue of work:

```ori
@process_queue (items: [Item]) -> [Result<ProcessedItem, Error>] uses Async = parallel(
    .tasks: map(
        .over: items,
        .transform: item -> process(
            .item: item,
        ),
    ),
    .max_concurrent: $worker_count,
)
```

### Periodic Work

For recurring tasks, use explicit loops:

```ori
@periodic_check (interval: Duration) -> void uses Clock, Async =
    loop(
        perform_check(),
        sleep(interval)
    )
```

Call it within structured concurrency:

```ori
@main () -> void uses Http, Clock, Async = parallel(
    .server: run_server(),
    .health_check: periodic_check(30s),
    .metrics: periodic_metrics(1m)
)
```

---

## What About Long-Running Services?

### Service Lifetime

For services that run "forever," structure them at the top level:

```ori
@main () -> void uses Http, Clock, Async = parallel(
    .http_server: run_http_server(),
    .background_jobs: run_job_processor(),
    .metrics_collector: run_metrics()
    // All three run concurrently
    // If any fails, others are cancelled
)
```

### Graceful Shutdown

Use context-based cancellation for clean shutdown:

```ori
@main () -> void uses Http, Async = run(
    let ctx = Context.with_signal(SIGTERM),
    parallel(
        .server: run_server(ctx),
        .jobs: run_jobs(ctx),
    ),
    // SIGTERM cancels the context, tasks check and exit cleanly
)
```

See [Cancellation](03-cancellation.md) for details.

---

## Comparison with Other Models

### vs. Go Goroutines

Go allows unbounded goroutines:

```go
// Go - goroutine escapes function
func handle() {
    go backgroundTask()  // runs after handle returns
    return
}
```

Ori requires explicit waiting:

```ori
// must complete before return
@handle () -> void uses Async = parallel(
    .main: main_task(),
    .background: background_task(),
)
```

### vs. JavaScript Promises

JavaScript allows unhandled promises:

```javascript
// JavaScript - promise not awaited
async function handle() {
    backgroundTask();  // Promise ignored, no error if it fails
    return "done";
}
```

Ori async functions must declare capabilities:

```ori
@handle () -> str = run(
    // ERROR: calling async function without Async capability
    background_task(),
    "done",
)
```

### vs. Rust async

Rust allows spawning detached tasks:

```rust
// Rust - task can outlive spawner
fn main() {
    tokio::spawn(background_task());  // detached
}
```

Ori has no `spawn`:

```ori
// both must complete
@main () -> void uses Async = parallel(
    .main: main_work(),
    .background: background_task(),
)
```

---

## Design Rationale

### Why This Restriction?

1. **Simpler reasoning** - You always know what concurrent work exists
2. **Predictable resource usage** - No runaway task accumulation
3. **Reliable error handling** - All errors reach a handler
4. **Clean shutdown** - No orphan tasks during shutdown
5. **AI-friendly** - Models generate safer concurrent code

### The Trade-off

Yes, this is more restrictive than other languages. The trade-off:

| Aspect | Detached Tasks | Structured Concurrency |
|--------|----------------|------------------------|
| Flexibility | High | Moderate |
| Safety | Lower | Higher |
| Debugging | Harder | Easier |
| Resource management | Manual | Automatic |
| Error handling | Easy to miss | Guaranteed |

Ori chooses safety and predictability over maximum flexibility.

### Escape Hatches?

Ori intentionally has **no escape hatch** for detached tasks. If you need truly independent background work, model it explicitly in your architecture:

```ori
// Explicit long-running service
@main () -> void uses Http, Async = parallel(
    .api: api_server(),
    .background: background_service()
)
```

---

## Best Practices

### Always Use Parallel Results

```ori
// Good
let data = parallel(
    .a: task_a(),
    .b: task_b(),
)

// Bad: parallel result unused
// WARNING: result unused
parallel(
    .a: task_a(),
    .b: task_b(),
)
```

### Use Named Tasks for Clarity

```ori
// Good: clear what each task does
parallel(
    .user_data: fetch_user(id),
    .preferences: fetch_preferences(id),
    .history: fetch_history(id)
)

// Less clear: anonymous task list
parallel(.tasks: [fetch_user(id), fetch_preferences(id), fetch_history(id)])
```

### Set Appropriate Concurrency Limits

```ori
// Good: bounded concurrency
parallel(
    .tasks: map(
        .over: urls,
        .transform: url -> fetch(
            .url: url,
        ),
    ),
    .max_concurrent: 10,
)

// Risky: unbounded concurrent requests that might open thousands of connections
parallel(
    .tasks: map(
        .over: thousands_of_urls,
        .transform: url -> fetch(
            .url: url,
        ),
    ),
)
```

### Handle Both Success and Failure

```ori
@robust_fetch () -> [Data] uses Http, Async = run(
    let results = parallel(
        .tasks: map(
            .over: urls,
            .transform: url -> fetch(
                .url: url,
            ),
        ),
        .on_error: collect_all,
    ),
    // Handle mixed results
    filter(
        .over: results,
        .predicate: result -> result.is_ok(),
    ).map(
        .transform: result -> result.unwrap(),
    ),
)
```

---

## Error Messages

### Detached Task Attempt

```
error[E0200]: cannot spawn detached task
  --> src/main.ori:5:5
   |
5  |     spawn(background_task())
   |     ^^^^^^^^^^^^^^^^^^^^^^^^ detached tasks not allowed
   |
   = note: use `parallel` to run concurrent tasks
   = help: parallel(
               .main: main_work(),
               .background: background_task(),
           )
```

### Missing Async Capability

```
error[E0201]: missing capability for async operation
  --> src/main.ori:8:5
   |
8  |     parallel(
   |     ^^^^^^^^ requires Async capability
   |
   = note: add `uses Async` to function signature
```

### Capability Not Propagated

```
error[E0202]: capability not propagated
  --> src/main.ori:10:5
   |
10 |     return some_async_fn()
   |            ^^^^^^^^^^^^^^^ calls function with Async capability
   |
   = note: caller must also declare `uses Async` capability
```

---

## See Also

- [Async/Await](01-async-await.md)
- [Cancellation](03-cancellation.md)
- [Channels](04-channels.md)
- [Patterns Reference](../02-syntax/04-patterns-reference.md)
