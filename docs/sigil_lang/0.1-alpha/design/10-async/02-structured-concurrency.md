# Structured Concurrency

This document covers Sigil's structured concurrency model: why there are no detached tasks, how `parallel` ensures all tasks complete, and why fire-and-forget is not allowed.

---

## Overview

Sigil enforces **structured concurrency**: all concurrent tasks must be awaited, and parent operations always wait for their children to complete.

```sigil
// Structured: parallel waits for all tasks
@fetch_all () -> async [Data] = parallel(
    .tasks: [fetch_a(), fetch_b(), fetch_c()]
).await

// NOT allowed: fire-and-forget
@bad () -> void = run(
    spawn(background_task()),  // ERROR: detached tasks not allowed
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

```sigil
@main () -> async void = run(
    // main waits for process_all
    let result = process_all().await,
    print(result),
)

@process_all () -> async [Data] = run(
    // process_all waits for parallel
    let data = parallel(
        .tasks: [task_a(), task_b(), task_c()],
    ).await,
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

### Sigil's Solution

Sigil simply doesn't allow detached tasks:

```sigil
// ERROR: spawn() does not exist
@bad () -> void = spawn(background_task())
```

If you need concurrent work, use `parallel`:

```sigil
@good () -> async void = run(
    // Explicitly wait for all concurrent work
    parallel(
        .main_work: handle_request(),
        .analytics: log_analytics(),
    ).await,
    print("all done"),
)
```

---

## The `parallel` Pattern

`parallel` is Sigil's primary concurrency primitive. It runs multiple tasks concurrently and waits for all to complete.

### Basic Usage

Named tasks with struct result:

```sigil
@fetch_dashboard (user_id: int) -> async Dashboard = run(
    let data = parallel(
        .user: get_user(user_id),
        .posts: get_posts(user_id),
        .notifications: get_notifications(user_id),
    ).await,
    Dashboard {
        user: data.user,
        posts: data.posts,
        notifications: data.notifications,
    },
)
```

List of tasks:

```sigil
@fetch_all (urls: [str]) -> async [Data] = parallel(
    .tasks: map(urls, url -> fetch(url))
).await
```

### Concurrency Limits

Control maximum concurrent tasks:

```sigil
@fetch_many (urls: [str]) -> async [Data] = parallel(
    .tasks: map(urls, url -> fetch(url)),
    .max_concurrent: 10  // at most 10 fetches at once
).await
```

Why limit concurrency?
- Prevent overwhelming servers
- Control resource usage (connections, memory)
- Avoid rate limiting

### Error Handling in Parallel

By default, `parallel` fails fast - one failure stops everything:

```sigil
@fetch_both () -> async Result<(Data, Data), Error> = parallel(
    .a: fetch_a(),  // if this fails...
    .b: fetch_b()   // ...this is cancelled
).await
```

Collect all errors instead:

```sigil
@fetch_all (urls: [str]) -> async [Result<Data, Error>] = parallel(
    .tasks: map(urls, url -> fetch(url)),
    .on_error: collect_all  // collect successes and failures
).await
```

### Timeouts

Add a timeout to parallel operations:

```sigil
@fetch_with_limit () -> async Result<Data, Error> = parallel(
    .a: fetch_slow(),
    .b: fetch_fast(),
    .timeout: 5s  // cancel all if not done in 5 seconds
).await
```

---

## Structured Concurrency Guarantees

### Guarantee 1: No Orphan Tasks

When a function returns, all tasks it started have completed:

```sigil
@process () -> async Result = run(
    parallel(
        .task_a: slow_operation(),
        .task_b: fast_operation(),
    ).await,
    // Both task_a and task_b are DONE here
    Ok(result),
)
// No tasks from process() are still running
```

### Guarantee 2: Hierarchical Cancellation

Cancelling a parent cancels all children:

```sigil
@outer () -> async void = run(
    timeout(
        .op: inner().await,
        .after: 5s,
        .on_timeout: handle_timeout()
    )
)

@inner () -> async void = parallel(
    .a: task_a(),  // cancelled if outer times out
    .b: task_b(),  // cancelled if outer times out
    .c: task_c()   // cancelled if outer times out
).await
```

### Guarantee 3: Error Propagation

Errors bubble up through the hierarchy:

```sigil
@main () -> async void = run(
    let result = top_level().await,
    match(result,
        Ok(data) -> print("Success: " + str(data)),
        Err(e) -> print("Failed: " + str(e)),
    ),
)

@top_level () -> async Result<Data, Error> = parallel(
    .a: middle_a(),  // error here...
    .b: middle_b()
).await  // ...propagates here

@middle_a () -> async Result<Data, Error> = parallel(
    .x: leaf_x(),  // error here...
    .y: leaf_y()
).await  // ...propagates to top_level
```

---

## Patterns for Common Scenarios

### Background Work That Must Complete

Instead of fire-and-forget, make background work explicit:

```sigil
// BAD: fire-and-forget (not allowed)
@handle_request () -> async Response = run(
    spawn(log_analytics()),  // ERROR
    compute_response()
)

// GOOD: explicit concurrent work
@handle_request () -> async Response = run(
    let results = parallel(
        .response: compute_response(),
        .analytics: log_analytics(),
    ).await,
    results.response,
)
```

### Cleanup That Must Run

Use `with` for cleanup instead of spawning cleanup tasks:

```sigil
@process_file (path: str) -> async Result<Data, Error> = with(
    .acquire: open_file(path).await,
    .use: file -> process(file).await,
    .release: file -> close_file(file).await  // always runs
)
```

### Worker Pools

For processing a queue of work:

```sigil
@process_queue (items: [Item]) -> async [Result<ProcessedItem, Error>] = parallel(
    .tasks: map(items, item -> process(item)),
    .max_concurrent: $worker_count
).await
```

### Periodic Work

For recurring tasks, use explicit loops with await:

```sigil
@periodic_check (interval: Duration) -> async void =
    loop(
        perform_check().await,
        sleep(interval).await
    )
```

Call it within structured concurrency:

```sigil
@main () -> async void = parallel(
    .server: run_server(),
    .health_check: periodic_check(30s),
    .metrics: periodic_metrics(1m)
).await
```

---

## What About Long-Running Services?

### Service Lifetime

For services that run "forever," structure them at the top level:

```sigil
@main () -> async void = parallel(
    .http_server: run_http_server(),
    .background_jobs: run_job_processor(),
    .metrics_collector: run_metrics()
    // All three run concurrently
    // If any fails, others are cancelled
).await
```

### Graceful Shutdown

Use context-based cancellation for clean shutdown:

```sigil
@main () -> async void = run(
    let ctx = Context.with_signal(SIGTERM),
    parallel(
        .server: run_server(ctx),
        .jobs: run_jobs(ctx),
    ).await,
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

Sigil requires explicit waiting:

```sigil
@handle () -> async void = parallel(
    .main: main_task(),
    .background: background_task()
).await  // must complete before return
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

Sigil async values must be awaited:

```sigil
@handle () -> async str = run(
    background_task(),  // ERROR: async value not awaited
    "done"
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

Sigil has no `spawn`:

```sigil
@main () -> async void = parallel(
    .main: main_work(),
    .background: background_task()
).await  // both must complete
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

Sigil chooses safety and predictability over maximum flexibility.

### Escape Hatches?

Sigil intentionally has **no escape hatch** for detached tasks. If you need truly independent background work, model it explicitly in your architecture:

```sigil
// Explicit long-running service
@main () -> async void = parallel(
    .api: api_server(),
    .background: background_service()
).await
```

---

## Best Practices

### Always Await Parallel Results

```sigil
// Good
let data = parallel(
    .a: task_a(),
    .b: task_b(),
).await

// Bad: parallel without await is useless
parallel(
    .a: task_a(),
    .b: task_b(),
)  // ERROR: async not awaited
```

### Use Named Tasks for Clarity

```sigil
// Good: clear what each task does
parallel(
    .user_data: fetch_user(id),
    .preferences: fetch_preferences(id),
    .history: fetch_history(id)
).await

// Less clear: anonymous task list
parallel(.tasks: [fetch_user(id), fetch_preferences(id), fetch_history(id)]).await
```

### Set Appropriate Concurrency Limits

```sigil
// Good: bounded concurrency
parallel(
    .tasks: map(urls, url -> fetch(url)),
    .max_concurrent: 10
).await

// Risky: unbounded concurrent requests
parallel(
    .tasks: map(thousands_of_urls, url -> fetch(url))
).await  // might open thousands of connections
```

### Handle Both Success and Failure

```sigil
@robust_fetch () -> async [Data] = run(
    let results = parallel(
        .tasks: map(urls, url -> fetch(url)),
        .on_error: collect_all,
    ).await,
    // Handle mixed results
    filter(results, r -> r.is_ok()).map(r -> r.unwrap()),
)
```

---

## Error Messages

### Detached Task Attempt

```
error[E0200]: cannot spawn detached task
  --> src/main.si:5:5
   |
5  |     spawn(background_task())
   |     ^^^^^^^^^^^^^^^^^^^^^^^^ detached tasks not allowed
   |
   = note: use `parallel` to run concurrent tasks
   = help: parallel(
               .main: main_work(),
               .background: background_task(),
           ).await
```

### Async Value Not Awaited

```
error[E0201]: async value not awaited
  --> src/main.si:8:5
   |
8  |     parallel(
   |     ^^^^^^^^ this parallel is never awaited
   |
   = note: add `.await` to wait for tasks
```

### Task Escaped Scope

```
error[E0202]: task would escape scope
  --> src/main.si:10:5
   |
10 |     return some_async_value
   |            ^^^^^^^^^^^^^^^^ async value escapes without being awaited
   |
   = note: all async operations must complete before function returns
```

---

## See Also

- [Async/Await](01-async-await.md)
- [Cancellation](03-cancellation.md)
- [Channels](04-channels.md)
- [Patterns Reference](../02-syntax/04-patterns-reference.md)
