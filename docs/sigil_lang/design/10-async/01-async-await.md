# Async/Await

This document covers Sigil's async/await model: function declarations, the `.await` syntax, and how async integrates with patterns.

---

## Overview

Sigil uses async/await with explicit suspension points. Every async operation is marked in the type system, and every suspension point is visible in the code.

```sigil
@fetch_user (id: int) -> async Result<User, Error> =
    http_get($api_url + "/users/" + str(id)).await
```

Key principles:
- `async` in the return type marks functions that can suspend
- `.await` marks exact suspension points
- Async is transparent to patterns
- No hidden control flow

---

## Async Function Declaration

### The `async` Keyword

Place `async` before the return type to declare an async function:

```sigil
@fetch_data (url: str) -> async Result<Data, Error> = ...
@sync_compute (n: int) -> int = ...  // clearly not async
```

### Why `async` in Return Type?

The signature tells you everything:

```sigil
// Signature alone tells you this suspends
@get_user (id: int) -> async Result<User, Error>

// Signature alone tells you this doesn't
@compute_hash (data: str) -> int
```

Benefits:
1. **Call site visibility** - You know an operation may suspend before calling
2. **Type system integration** - `async` is part of the type, not hidden metadata
3. **AI readability** - Models generate correct call patterns based on signatures
4. **No surprise blocking** - Sync functions never secretly perform I/O

### Async vs Sync Signatures

| Declaration | Meaning |
|-------------|---------|
| `-> T` | Synchronous, returns `T` immediately |
| `-> async T` | Asynchronous, returns `T` after possible suspension |
| `-> async Result<T, E>` | Async operation that may fail |

---

## The `.await` Syntax

### Postfix Await

Sigil uses postfix `.await` syntax:

```sigil
@process () -> async Result<Data, Error> = try(
    response = http_get(url).await,
    parsed = parse(response.body).await,
    Ok(transform(parsed))
)
```

### Why Postfix?

Postfix `.await` chains naturally with data flow:

```sigil
// Reads left-to-right
fetch(url).await.body

// Compare to prefix (hypothetical) - breaks flow
(await fetch(url)).body
```

Benefits:
1. **Natural chaining** - Data flows left to right
2. **Explicit suspension** - Every `.await` is a visible suspension point
3. **Familiar syntax** - Matches Rust (large training corpus for AI)
4. **No precedence confusion** - Always clear what is being awaited

### Suspension Points

Each `.await` is a point where:
1. The function may pause execution
2. Control returns to the runtime
3. Other tasks may run
4. Execution resumes when the operation completes

```sigil
@sequential_fetches () -> async [Data] = run(
    // First fetch - may suspend
    data1 = fetch(url1).await,

    // Second fetch - may suspend
    data2 = fetch(url2).await,

    // Third fetch - may suspend
    data3 = fetch(url3).await,

    [data1, data2, data3]
)
```

### Chaining Awaits

Multiple operations can be chained:

```sigil
// Chain operations, await at the end
@get_user_name (id: int) -> async str =
    fetch_user(id).await.name

// Multiple awaits in expression
@get_combined () -> async str =
    fetch_first().await + " " + fetch_second().await
```

### Await and Error Handling

`.await` works naturally with `try`:

```sigil
@fetch_and_parse (url: str) -> async Result<Data, Error> = try(
    // Each await can fail, try propagates errors
    response = http_get(url).await,
    validated = validate(response).await,
    parsed = parse(validated.body).await,
    Ok(transform(parsed))
)
```

---

## Async in Patterns

Sigil patterns work transparently with async operations. No special "async versions" needed.

### With `parallel`

Run multiple async operations concurrently:

```sigil
@fetch_dashboard (user_id: int) -> async Dashboard = run(
    data = parallel(
        .user: get_user(user_id),
        .posts: get_posts(user_id),
        .notifications: get_notifications(user_id)
    ).await,
    Dashboard {
        user: data.user,
        posts: data.posts,
        notifications: data.notifications
    }
)
```

With concurrency limits:

```sigil
@fetch_all (ids: [int]) -> async [User] = parallel(
    .tasks: map(ids, id -> fetch_user(id)),
    .max_concurrent: 10
).await
```

### With `retry`

Retry failed async operations:

```sigil
@reliable_fetch (url: str) -> async Result<Data, Error> = retry(
    .op: http_get(url).await,
    .attempts: 3,
    .backoff: exponential(base: 100ms, max: 5s)
)
```

The pattern handles:
- Waiting between retries
- Tracking attempt count
- Applying backoff delays

### With `timeout`

Bound async operation time:

```sigil
@bounded_fetch (url: str) -> async Result<Data, Error> = timeout(
    .op: http_get(url).await,
    .after: 30s,
    .on_timeout: Err(TimeoutError { url: url })
)
```

### With `map`

Transform collections with async operations:

```sigil
// Sequential - one at a time
@fetch_all_sequential (urls: [str]) -> async [Data] =
    map(urls, url -> fetch(url).await)

// Parallel - concurrent with limit
@fetch_all_parallel (urls: [str]) -> async [Data] = parallel(
    .tasks: map(urls, url -> fetch(url)),
    .max_concurrent: 5
).await
```

### With `try`

Compose async operations with error handling:

```sigil
@complex_operation (id: int) -> async Result<Output, Error> = try(
    user = fetch_user(id).await,
    permissions = fetch_permissions(user.role).await,
    data = fetch_data_if_allowed(user, permissions).await,
    processed = process(data).await,
    Ok(Output { user: user, data: processed })
)
```

---

## Async Types

### The `async T` Type

`async T` represents a computation that will eventually produce a `T`:

```sigil
// Type is: async Result<User, Error>
@fetch_user (id: int) -> async Result<User, Error> = ...

// Calling without await gives you the async value
pending = fetch_user(42)  // type: async Result<User, Error>

// Awaiting extracts the value
user = pending.await      // type: Result<User, Error>
```

### Type Inference with Async

Return types are inferred in lambdas:

```sigil
// Lambda return type inferred as async Result<Data, Error>
urls.map(url -> fetch(url))
```

### Async in Generic Bounds

Functions can accept async operations:

```sigil
@with_timeout<T> (op: async T, limit: Duration) -> async Result<T, TimeoutError> =
    timeout(
        .op: op.await,
        .after: limit,
        .on_timeout: Err(TimeoutError {})
    )
```

---

## Async Function Bodies

### Expression Bodies

Simple async functions use expression bodies:

```sigil
@fetch_json (url: str) -> async Result<Json, Error> =
    http_get(url).await.body.parse_json()
```

### Sequential Operations with `run`

Use `run` for multiple sequential async operations:

```sigil
@multi_step () -> async Result<Output, Error> = run(
    step1 = perform_first().await,
    step2 = perform_second(step1).await,
    step3 = perform_third(step2).await,
    Ok(step3)
)
```

### Conditional Async

Conditionals work naturally with async:

```sigil
@fetch_if_needed (cached: Option<Data>, url: str) -> async Data =
    if cached.is_some() then cached.unwrap()
    else fetch(url).await
```

### Async in Loops

Use patterns instead of explicit loops:

```sigil
// Process items sequentially
@process_all (items: [Item]) -> async [Result<ProcessedItem, Error>] =
    map(items, item -> process(item).await)

// Process items in parallel
@process_all_parallel (items: [Item]) -> async [Result<ProcessedItem, Error>] = parallel(
    .tasks: map(items, item -> process(item)),
    .max_concurrent: 10
).await
```

---

## Calling Async Functions

### From Async Context

Async functions can only be awaited from async contexts:

```sigil
@outer () -> async void = run(
    // Can await because outer is async
    data = fetch_data().await,
    print(data)
)
```

### From Sync Context

Sync functions cannot await. Use `parallel` at the top level:

```sigil
@main () -> void = run(
    // Start async operations from main
    result = runtime.block_on(async_main()),
    print(result)
)

@async_main () -> async Result<str, Error> = try(
    data = fetch_data().await,
    Ok(process(data))
)
```

### The `@main` Function

For async programs, make `main` async:

```sigil
@main () -> async void = run(
    result = fetch_and_process().await,
    print("Result: " + str(result))
)
```

---

## Best Practices

### Mark Suspension Points Clearly

```sigil
// Good: suspension points visible
@process () -> async Data = run(
    a = fetch_a().await,
    b = fetch_b().await,
    combine(a, b)
)

// Avoid: hiding await deep in expressions
@process () -> async Data =
    combine(deep(nested(fetch_a().await).value).await, fetch_b().await)
```

### Prefer Parallel When Independent

```sigil
// Good: independent fetches run concurrently
@fetch_both () -> async (Data, Data) = run(
    results = parallel(
        .a: fetch_a(),
        .b: fetch_b()
    ).await,
    (results.a, results.b)
)

// Less efficient: sequential when could be parallel
@fetch_both_slow () -> async (Data, Data) = run(
    a = fetch_a().await,
    b = fetch_b().await,
    (a, b)
)
```

### Handle Errors at Appropriate Levels

```sigil
// Good: error handling at logical boundaries
@fetch_user_data (id: int) -> async Result<UserData, AppError> = try(
    user = fetch_user(id).await | e -> AppError.Network(e),
    profile = fetch_profile(user.id).await | e -> AppError.Network(e),
    Ok(UserData { user: user, profile: profile })
)
```

### Use Patterns for Common Async Tasks

```sigil
// Good: use retry for transient failures
@reliable_fetch (url: str) -> async Result<Data, Error> = retry(
    .op: http_get(url).await,
    .attempts: 3,
    .backoff: exponential(base: 100ms, max: 5s)
)

// Good: use timeout for bounded operations
@bounded_operation () -> async Result<Data, Error> = timeout(
    .op: slow_operation().await,
    .after: 30s,
    .on_timeout: Err(TimeoutError {})
)
```

---

## Common Patterns

### Fetch with Retry and Timeout

```sigil
@robust_fetch (url: str) -> async Result<Data, Error> = timeout(
    .op: retry(
        .op: http_get(url).await,
        .attempts: 3,
        .backoff: exponential(base: 100ms, max: 2s)
    ),
    .after: 30s,
    .on_timeout: Err(TimeoutError { url: url })
)
```

### Fan-Out/Fan-In

```sigil
@aggregate_data (sources: [str]) -> async Summary = run(
    // Fan out: fetch all sources concurrently
    results = parallel(
        .tasks: map(sources, src -> fetch(src)),
        .max_concurrent: 20
    ).await,

    // Fan in: combine results
    fold(results, Summary.empty(), Summary.merge)
)
```

### Dependent Async Operations

```sigil
@cascade_fetch (user_id: int) -> async FullProfile = run(
    // Sequential because each step depends on previous
    user = fetch_user(user_id).await,
    posts = fetch_posts(user.id).await,
    comments = parallel(
        .tasks: map(posts, p -> fetch_comments(p.id))
    ).await,

    FullProfile {
        user: user,
        posts: posts,
        comments: flatten(comments)
    }
)
```

### Polling with Backoff

```sigil
@poll_until_complete (job_id: str) -> async Result<JobResult, Error> = recurse(
    .cond: check_status(job_id).await == Complete,
    .base: fetch_result(job_id).await,
    .step: run(
        sleep(1s).await,
        self(job_id)
    )
)
```

---

## Error Messages

### Missing Await

```
error[E0123]: async value not awaited
  --> src/main.si:10:5
   |
10 |     fetch_user(id)
   |     ^^^^^^^^^^^^^^ this async operation is never awaited
   |
   = note: add `.await` to get the result: `fetch_user(id).await`
```

### Await in Sync Function

```
error[E0124]: cannot await in sync function
  --> src/main.si:5:10
   |
5  | @compute (n: int) -> int = fetch(n).await
   |                                    ^^^^^^ await requires async context
   |
   = note: make the function async: `-> async int`
```

### Type Mismatch with Async

```
error[E0125]: type mismatch
  --> src/main.si:8:5
   |
8  |     let x: int = fetch_number()
   |         ^ expected `int`, found `async int`
   |
   = note: add `.await` to get the value: `fetch_number().await`
```

---

## See Also

- [Structured Concurrency](02-structured-concurrency.md)
- [Cancellation](03-cancellation.md)
- [Channels](04-channels.md)
- [Patterns Reference](../02-syntax/04-patterns-reference.md)
- [Error Handling](../05-error-handling/index.md)
