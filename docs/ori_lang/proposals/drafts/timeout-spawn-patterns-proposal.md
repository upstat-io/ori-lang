# Proposal: Timeout and Spawn Patterns

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, patterns, concurrency

---

## Summary

This proposal formalizes the `timeout` and `spawn` pattern semantics, including cancellation behavior, error handling, and relationship to other concurrency patterns.

---

## Problem Statement

The spec shows `timeout` and `spawn` patterns but leaves unclear:

1. **Timeout cancellation**: How is the operation cancelled?
2. **Timeout error**: What error type is returned?
3. **Spawn fire-and-forget**: What happens to errors?
4. **Spawn limits**: Can spawn exhaust resources?
5. **Relationship**: How do these relate to `parallel` and `nursery`?

---

# Timeout Pattern

## Syntax

```ori
timeout(
    op: expression,
    after: Duration,
)
```

## Semantics

### Basic Behavior

1. Start executing `op`
2. If `op` completes before `after`: return `Ok(result)`
3. If `after` elapses first: cancel `op`, return `Err(TimeoutError)`

```ori
let result = timeout(op: fetch(url), after: 5s)
// result: Result<Response, TimeoutError>
```

### Return Type

```ori
timeout(op: T, after: Duration) -> Result<T, TimeoutError>
```

Where `T` is the type of `op`.

### TimeoutError Type

```ori
type TimeoutError = {
    duration: Duration,  // The timeout duration
    message: str,        // Human-readable message
}

impl Printable for TimeoutError {
    @to_str (self) -> str = `operation timed out after {self.duration}`
}
```

## Cancellation

### Cooperative Cancellation

When timeout expires:

1. Operation is marked for cancellation
2. At next cancellation checkpoint, operation terminates
3. Destructors run during unwinding
4. `Err(TimeoutError)` is returned

### Cancellation Checkpoints

Same as nursery cancellation:
- Async calls
- Loop iterations
- Pattern entry (`run`, `try`, `match`, etc.)

### Uncancellable Operations

CPU-bound operations without checkpoints cannot be cancelled until they reach one:

```ori
timeout(
    op: tight_cpu_loop(),  // No checkpoints inside
    after: 1s,
)
// May take longer than 1s if no checkpoints
```

## Async Requirement

`timeout` requires async context:

```ori
@fetch_with_timeout (url: str) -> Result<Data, Error> uses Async =
    timeout(op: fetch(url), after: 10s)
        .map_err(transform: e -> Error { message: e.to_str() })
```

## Nested Timeout

Inner timeouts can be shorter than outer:

```ori
timeout(
    op: run(
        let a = timeout(op: step1(), after: 2s)?,
        let b = timeout(op: step2(), after: 2s)?,
        (a, b),
    ),
    after: 5s,  // Overall timeout
)
```

---

# Spawn Pattern

## Syntax

```ori
spawn(tasks: [expression])
spawn(tasks: [...], max_concurrent: int)
```

## Semantics

### Fire and Forget

`spawn` starts tasks and returns immediately:

```ori
spawn(tasks: [send_email(u) for u in users])
// Returns void immediately
// Emails sent in background
```

### Return Type

```ori
spawn(tasks: [() -> T uses Async]) -> void
```

Always returns `void`. Results are discarded.

### Error Handling

Errors in spawned tasks are silently discarded:

```ori
spawn(tasks: [
    () -> run(
        let result = risky_operation(),  // Might fail
        log(msg: "done"),
    ),
])
// If risky_operation() fails, error is silently dropped
```

### Logging Errors

To handle errors, log explicitly within the task:

```ori
spawn(tasks: [
    () -> match(risky_operation(),
        Ok(_) -> log(msg: "success"),
        Err(e) -> log(msg: `failed: {e}`),
    ),
])
```

## Concurrency Control

### max_concurrent

Limit simultaneous tasks:

```ori
spawn(
    tasks: [send_email(u) for u in users],
    max_concurrent: 10,
)
// At most 10 emails sending at once
```

### Default Behavior

Without `max_concurrent`, all tasks may start simultaneously.

### Resource Exhaustion

If runtime cannot allocate resources:
- Task is dropped
- No error surfaced (fire-and-forget semantics)
- Other tasks continue

## No Wait Mechanism

`spawn` provides no way to wait for completion:

```ori
spawn(tasks: tasks)
// Cannot wait here
```

For waiting, use `parallel` or `nursery`:

```ori
let results = parallel(tasks: tasks)
// Wait for all to complete
```

## Task Lifetime

Spawned tasks:
- Run independently of spawning scope
- May outlive the spawning function
- Are cancelled on program exit

```ori
@setup () -> void uses Async = run(
    spawn(tasks: [background_monitor()]),
    // Function returns, but monitor continues
)
```

---

# Comparison with Other Patterns

| Pattern | Returns | Waits | Errors | Use Case |
|---------|---------|-------|--------|----------|
| `timeout` | `Result<T, TimeoutError>` | Yes | Surfaced | Bounded wait |
| `spawn` | `void` | No | Dropped | Fire-and-forget |
| `parallel` | `[Result<T, E>]` | Yes | Collected | Batch operations |
| `nursery` | `[Result<T, E>]` | Yes | Configurable | Structured concurrency |

---

## Examples

### Timeout with Fallback

```ori
@fetch_with_fallback (url: str, fallback: Data) -> Data uses Async =
    match(timeout(op: fetch(url), after: 5s),
        Ok(data) -> data,
        Err(_) -> fallback,
    )
```

### Spawn Background Tasks

```ori
@on_user_signup (user: User) -> void uses Async = run(
    save_user(user),  // Synchronous, must complete
    spawn(tasks: [
        () -> send_welcome_email(user),
        () -> notify_admin(user),
        () -> update_analytics(user),
    ]),  // Fire and forget
)
```

### Timeout in Loop

```ori
@fetch_all (urls: [str]) -> [Option<Data>] uses Async =
    for url in urls yield
        match(timeout(op: fetch(url), after: 5s),
            Ok(data) -> Some(data),
            Err(_) -> None,
        )
```

### Spawn with Rate Limiting

```ori
@notify_all_users (users: [User]) -> void uses Async =
    spawn(
        tasks: [() -> send_notification(u) for u in users],
        max_concurrent: 50,  // Avoid overwhelming notification service
    )
```

---

## Error Messages

### Timeout Missing Async

```
error[E1010]: `timeout` requires `Async` capability
  --> src/main.ori:5:5
   |
 5 |     timeout(op: fetch(url), after: 5s)
   |     ^^^^^^^ requires `uses Async`
   |
   = help: add `uses Async` to the function signature
```

### Spawn Task Not Async

```
error[E1011]: `spawn` tasks must use `Async`
  --> src/main.ori:5:18
   |
 5 |     spawn(tasks: [() -> sync_function()])
   |                  ^^^^^^^^^^^^^^^^^^^^^^^ missing `uses Async`
   |
   = note: spawn requires async tasks for concurrent execution
```

---

## Spec Changes Required

### Update `10-patterns.md`

Add comprehensive sections for:
1. Timeout semantics and cancellation
2. Spawn fire-and-forget behavior
3. Comparison table with other patterns

---

## Summary

### Timeout

| Aspect | Details |
|--------|---------|
| Syntax | `timeout(op:, after:)` |
| Returns | `Result<T, TimeoutError>` |
| Cancellation | Cooperative at checkpoints |
| Requires | `uses Async` |
| Use case | Bounded waiting for operations |

### Spawn

| Aspect | Details |
|--------|---------|
| Syntax | `spawn(tasks:, max_concurrent:)` |
| Returns | `void` |
| Errors | Silently dropped |
| Requires | `uses Async` |
| Use case | Fire-and-forget background work |
