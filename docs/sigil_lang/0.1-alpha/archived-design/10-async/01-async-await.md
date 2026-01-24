# Async via Capabilities

This document covers Sigil's approach to asynchronous programming: the explicit `Async` capability instead of async/await syntax.

---

## Overview

Sigil takes a different approach to async than most languages. Instead of an `async` type modifier and `.await` syntax, **async behavior is tracked through an explicit `Async` capability**.

```sigil
// Async capability explicitly declares "this function may suspend"
@fetch_user (id: int) -> Result<User, Error> uses Http, Async =
    let response = Http.get(
        .url: "/users/" + str(id),
    )?,
    Ok(parse(
        .input: response,
    ))
```

Key insight: **You're already paying the propagation cost with async/await. Capabilities give you more value for the same cost.**

---

## The Problem with Traditional Async/Await

Consider async in Rust, JavaScript, or Python. The `async` keyword propagates up the call stack:

```rust
// Rust: async propagates up
async fn fetch_user(id: i32) -> Result<User, Error> { ... }
async fn get_dashboard(id: i32) -> Result<Dashboard, Error> {
    let user = fetch_user(id).await?;  // Caller must be async too
    ...
}
async fn main() {
    let dashboard = get_dashboard(1).await;  // And main must be async
}
```

This propagation tells you something true: somewhere in the call chain, there's an operation that may suspend. But you get **nothing else** from this propagation:

- No help with mocking
- No explicit dependency tracking
- Tests still need runtime hacks to stub network calls

---

## Sigil's Approach: The Async Capability

In Sigil, the `uses` clause already propagates up the call stack. Instead of an `async` keyword, we have an explicit `Async` capability:

1. **Same propagation** you'd have with `async` anyway
2. **Plus** explicit dependencies
3. **Plus** easy mocking for tests
4. **Plus** cleaner return types
5. **Plus** clear sync vs async distinction

```sigil
// Async capability explicitly declares suspension
@fetch_user (id: int) -> Result<User, Error> uses Http, Async =
    let response = Http.get(
        .url: "/users/" + str(id),
    )?,
    Ok(parse(
        .input: response,
    ))

// Caller must declare both Http and Async (same as async propagation)
@get_dashboard (id: int) -> Result<Dashboard, Error> uses Http, Async =
    let user = fetch_user(
        .id: id,
    )?,
    Dashboard { user: user, ... }

// Tests provide mock - sync, no Async needed!
@test_fetch_user tests @fetch_user () -> void =
    with Http = MockHttp { responses: {"/users/1": user_json} } in
    run(
        let result = fetch_user(
            .id: 1,
        ),
        assert_eq(
            .actual: result,
            .expected: Ok(expected_user),
        ),
    )
```

Note: Tests don't need `Async` because MockHttp is synchronous - it returns immediately without suspending.

---

## Why This Works

### The Propagation Tax

Any effect system has a "propagation tax" - callers must know about effects their callees have. This is true whether you use:

- `async/await` (async propagates)
- Checked exceptions (throws propagates)
- Capabilities (uses propagates)

**You're paying this tax anyway.** The question is: what do you get in return?

| Approach | Propagates | Testing | Dependencies | Clean Types |
|----------|------------|---------|--------------|-------------|
| `async/await` | Yes | Hard | Hidden | No (`async T`) |
| Capabilities | Yes | Easy | Explicit | Yes (`T`) |

### Same Cost, More Value

With traditional async:
```sigil
// Hypothetical async/await syntax
@fetch_user (id: int) -> async Result<User, Error> =
    http_get(
        .url: "/users/" + str(id),
    // Returns async Result
    ).await

// Awkward: .await returns Result, need ? for error, .body for field
@fetch_json (url: str) -> async Result<Json, Error> =
    http_get(
        .url: url,
    // .await?.body is clunky
    ).await?.body.parse_json()
```

With the Async capability:
```sigil
// Clean syntax - Http.get returns Result directly, Async declares suspension
@fetch_user (id: int) -> Result<User, Error> uses Http, Async =
    Http.get(
        .url: "/users/" + str(id),
    )?.parse()

// Natural chaining - no .await interrupting the flow
@fetch_json (url: str) -> Result<Json, Error> uses Http, Async =
    Http.get(
        .url: url,
    )?.body.parse_json()
```

---

## How It Works

### The Async Capability

`Async` is a capability that represents the ability to suspend execution:

```sigil
// Async is a marker capability - it has no methods
trait Async {}
```

When you declare `uses Async`, you're explicitly stating: "this function may suspend execution."

### Sync vs Async Operations

The same I/O trait can be used synchronously or asynchronously:

```sigil
trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

// With Async: Http.get may suspend (non-blocking)
@fetch_async (url: str) -> Result<Data, Error> uses Http, Async =
    Http.get(
        .url: url,
    )?.body

// Without Async: Http.get blocks until complete (synchronous)
@fetch_blocking (url: str) -> Result<Data, Error> uses Http =
    Http.get(
        .url: url,
    )?.body
```

The presence or absence of `Async` tells you exactly what to expect:
- `uses Http, Async` → Non-blocking, may suspend
- `uses Http` → Blocking, waits for completion

### Suspension is Explicit

Suspension is NOT implicit - it's explicitly declared via the `Async` capability:

```sigil
@fetch_data (url: str) -> Result<Data, Error> uses Http, Async = run(
    // Http.get may suspend because we declared 'uses Async'
    let response = Http.get(
        .url: url,
    )?,

    // Execution continues after response is ready
    let data = parse(
        .input: response.body,
    )?,

    Ok(data),
)
```

No `.await` needed - the `Async` capability declaration makes suspension explicit at the function level rather than at every call site.

### Propagation Rules

The same rules apply as for any capability:

1. **Functions must declare capabilities they use**
   ```sigil
   @fetch (url: str) -> Result<str, Error> uses Http = Http.get(
       .url: url,
   )?.body
   ```

2. **Callers must have capabilities their callees need**
   ```sigil
   // Must declare Http because fetch uses it
   @process (url: str) -> Result<Data, Error> uses Http =
       let raw = fetch(
           .url: url,
       )?,
       Ok(parse(
           .input: raw,
       ))
   ```

3. **`with` provides capabilities for a scope**
   ```sigil
   @main () -> void =
       with Http = RealHttp { timeout: 30s } in
       run_app()
   ```

---

## Concurrency with `parallel`

Without `.await`, how do you run operations concurrently? Use the `parallel` pattern:

### Sequential (Default)

```sigil
@fetch_both (id: int) -> Result<(User, Posts), Error> uses Http, Async = run(
    // These run sequentially - first user, then posts
    let user = fetch_user(
        .id: id,
    )?,
    let posts = fetch_posts(
        .id: id,
    )?,
    Ok((user, posts)),
)
```

### Concurrent with `parallel`

```sigil
@fetch_both (id: int) -> Result<{ user: User, posts: Posts }, Error> uses Http, Async =
    // These run concurrently
    parallel(
        .user: fetch_user(
            .id: id,
        ),
        .posts: fetch_posts(
            .id: id,
        ),
    )
```

The `parallel` pattern:
- Takes named operations as lambdas/thunks
- Runs them concurrently (requires `Async`)
- Returns a struct with the results
- Inherits capability requirements from its operations

### Concurrent with Limits

```sigil
@fetch_all (urls: [str]) -> [Result<Data, Error>] uses Http, Async = parallel(
    .tasks: map(
        .over: urls,
        .transform: url -> fetch(
            .url: url,
        ),
    ),
    .max_concurrent: 10,
)
```

---

## Comparison to Other Languages

### Rust

```rust
// Rust: async is a type modifier
async fn fetch() -> Result<Data, Error> { ... }
let data = fetch().await?;  // .await extracts value
```

```sigil
// Sigil: Async capability is explicit
@fetch () -> Result<Data, Error> uses Http, Async = ...
// No .await needed
let data = fetch()?
```

### JavaScript

```javascript
// JavaScript: async/await keywords
async function fetch() { return await httpGet(url); }
const data = await fetch();
```

```sigil
// Sigil: Async capability declaration
@fetch () -> Result<Data, Error> uses Http, Async = Http.get(
    .url: url,
)?
let data = fetch()?
```

### Go

Go is actually similar in spirit - goroutines make code look synchronous:

```go
// Go: synchronous-looking code, but blocks
func fetch() (Data, error) { return httpGet(url) }
data, err := fetch()
```

```sigil
// Sigil without Async: same blocking behavior
@fetch () -> Result<Data, Error> uses Http = Http.get(
    .url: url,
)?
let data = fetch()?

// Sigil with Async: non-blocking, may suspend
@fetch () -> Result<Data, Error> uses Http, Async = Http.get(
    .url: url,
)?
let data = fetch()?
```

The difference: Sigil makes the sync/async distinction **explicit** via the `Async` capability.

---

## Testing Async Code

The biggest benefit of capability-based async: **testing is trivial**.

### The Problem with Traditional Async Testing

```javascript
// JavaScript: need to mock at module level or use DI frameworks
jest.mock('./http', () => ({ get: jest.fn() }));
test('fetch user', async () => {
    http.get.mockResolvedValue({ body: '{"name": "Alice"}' });
    const user = await fetchUser(1);
    expect(user.name).toBe('Alice');
});
```

### Sigil: Sync Mocks, No Async Needed

```sigil
// Production code uses Http + Async (non-blocking)
@fetch_user (id: int) -> Result<User, Error> uses Http, Async =
    Http.get(
        .url: "/users/" + str(id),
    )?.parse()

// Test uses Http only (blocking, sync) - MockHttp returns immediately
@test_fetch_user tests @fetch_user () -> void =
    with Http = MockHttp {
        responses: { "/users/1": "{\"name\": \"Alice\"}" }
    } in
    run(
        let result = fetch_user(
            .id: 1,
        ),
        assert_eq(
            .actual: result,
            .expected: Ok(User { name: "Alice" }),
        ),
    )
```

Notice: the test doesn't declare `Async` because MockHttp is synchronous. It returns the mocked response immediately without any suspension. This is natural - mocks don't need to simulate network delays.

No mocking frameworks. No runtime hacks. No special test configuration. Just provide a sync implementation.

---

## Patterns That Work with Async

All patterns work naturally with the Async capability:

### `retry` - Retry Failed Operations

```sigil
@reliable_fetch (url: str) -> Result<Data, Error> uses Http, Async = retry(
    .operation: Http.get(
        .url: url,
    ),
    .attempts: 3,
    .backoff: exponential(
        .base: 100ms,
        .max: 5s,
    ),
)
```

### `timeout` - Bound Operation Time

```sigil
@bounded_fetch (url: str) -> Result<Data, Error> uses Http, Async = timeout(
    .operation: Http.get(
        .url: url,
    ),
    .after: 30s,
    .on_timeout: Err(TimeoutError { url: url }),
)
```

### `map` with Concurrent Execution

```sigil
// Sequential - still async (may suspend between iterations)
@fetch_all_seq (urls: [str]) -> [Result<Data, Error>] uses Http, Async =
    map(
        .over: urls,
        .transform: url -> Http.get(
            .url: url,
        ),
    )

// Concurrent - async with parallelism
@fetch_all_par (urls: [str]) -> [Result<Data, Error>] uses Http, Async = parallel(
    .tasks: map(
        .over: urls,
        .transform: url -> Http.get(
            .url: url,
        ),
    ),
    .max_concurrent: 10,
)
```

### `try` for Error Handling

```sigil
@complex_operation (id: int) -> Result<Output, Error> uses Http, Async = try(
    let user = fetch_user(
        .id: id,
    )?,
    let perms = fetch_permissions(
        .role: user.role,
    )?,
    let data = fetch_data_if_allowed(
        .user: user,
        .perms: perms,
    )?,
    Ok(Output { user: user, data: data }),
)
```

---

## The Trade-Off

The capability approach has one trade-off: **implementation details "leak" into the type signature**.

If `fetch_user` uses Http and Async, and `get_dashboard` calls `fetch_user`, then `get_dashboard` must also declare `uses Http, Async`. Callers know that somewhere in the call chain, HTTP is involved and the function may suspend.

**But this is the same trade-off as async/await.** In Rust, if you call an async function, your function must be async. In JavaScript, if you await something, your function must be async. The "leaking" happens either way.

The question is: **if you're paying this cost anyway, why not get more value?**

```sigil
// With async/await (hypothetical): leaks that it's async, but nothing else
@get_dashboard (user_id: int) -> async Result<Dashboard, Error>

// With capabilities: leaks what it uses, AND you can mock it, AND sync/async is explicit
@get_dashboard (user_id: int) -> Result<Dashboard, Error> uses Http, Async
```

---

## Summary

| Aspect | Traditional Async | Sigil Async Capability |
|--------|------------------|------------------------|
| Syntax | `async fn`, `.await` | `uses Http, Async` |
| Return type | `async T` | `T` |
| Propagation | `async` bubbles up | `uses` bubbles up |
| Testing | Hard (need mocks) | Easy (sync mocks) |
| Suspension | Per-call (`.await`) | Per-function (`Async`) |
| Sync vs Async | Different APIs | Same API, different capabilities |
| Concurrency | Manual with `.await` | `parallel` pattern |

The `Async` capability approach gives you:
- **Cleaner types** - no `async` wrapper
- **Natural chaining** - no `.await` interrupting flow
- **Easy testing** - sync mocks don't need Async
- **Explicit dependencies** - know what effects your code has
- **Clear sync/async distinction** - `uses Http` vs `uses Http, Async`

All for the same propagation cost you'd pay with async/await anyway.

---

## See Also

- [Structured Concurrency](02-structured-concurrency.md)
- [Cancellation](03-cancellation.md)
- [Channels](04-channels.md)
- [Capabilities](../14-capabilities/index.md)
- [Testing Effectful Code](../14-capabilities/03-testing-effectful-code.md)
