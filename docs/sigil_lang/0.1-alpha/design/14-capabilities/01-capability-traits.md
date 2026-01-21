# Capability Traits

This document covers defining capability traits - interfaces that represent side effects, external dependencies, and async behavior.

---

## What Are Capability Traits?

Capability traits are regular traits that represent access to external resources or side effects:

```sigil
trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

trait FileSystem {
    @read (path: str) -> Result<str, Error>
    @write (path: str, content: str) -> Result<void, Error>
}

trait Clock {
    @now () -> Timestamp
}
```

They use the same `trait` syntax as any other trait, but by convention represent effects that may include:
- I/O operations (network, file system)
- Non-determinism (random, time)
- External state (cache, database)
- Suspension (any async I/O)

---

## The Async Capability

A key feature: **the `Async` capability explicitly tracks suspension**.

When you want a function to be non-blocking (may suspend), you declare `uses Async` alongside your I/O capabilities:

```sigil
trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

// With Async: non-blocking, may suspend
@fetch_user (id: str) -> Result<User, Error> uses Http, Async =
    Http.get("/users/" + id)?.parse()

// Without Async: blocking, runs to completion
@fetch_user_sync (id: str) -> Result<User, Error> uses Http =
    Http.get("/users/" + id)?.parse()
```

This is cleaner than traditional async/await:

```sigil
// Traditional (hypothetical): async pollutes types
@fetch_user (id: str) -> async Result<User, Error> =
    http_get("/users/" + id).await?.parse()

// Sigil: clean types, Async capability is explicit
@fetch_user (id: str) -> Result<User, Error> uses Http, Async =
    Http.get("/users/" + id)?.parse()
```

See [Async via Capabilities](../10-async/01-async-await.md) for the full explanation.

---

## Defining Capabilities

### Basic Capability

```sigil
trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
    @put (url: str, body: str) -> Result<Response, Error>
    @delete (url: str) -> Result<void, Error>
}
```

### Capability with Configuration

```sigil
trait Database {
    @query (sql: str) -> Result<[Row], Error>
    @execute (sql: str) -> Result<int, Error>
    @transaction<T> (f: () -> Result<T, Error>) -> Result<T, Error>
}
```

### When to Use Async

The `Async` capability is typically used with I/O capabilities:

```sigil
// I/O capabilities - typically paired with Async for non-blocking behavior
trait Http {
    @get (url: str) -> Result<Response, Error>
}

trait FileSystem {
    @read (path: str) -> Result<str, Error>
}

// Non-blocking HTTP
@fetch (url: str) -> Result<Data, Error> uses Http, Async = Http.get(url)?

// Non-blocking file read
@read_config () -> Result<str, Error> uses FileSystem, Async = FileSystem.read("/config.json")
```

Some capabilities don't need `Async` because they're inherently fast:

```sigil
// Clock - reading time is instant, no need for Async
trait Clock {
    @now () -> Timestamp
}

// Random - computation only, no I/O
trait Random {
    @int (min: int, max: int) -> int
}

// These don't need Async
@get_time () -> Timestamp uses Clock = Clock.now()
@roll_dice () -> int uses Random = Random.int(1, 6)
```

---

## Implementing Capabilities

### Production Implementation

```sigil
type RealHttp = {
    base_url: str,
    timeout: Duration
}

impl Http for RealHttp {
    @get (url: str) -> Result<Response, Error> =
        // Actual HTTP call implementation
        native_http_get(self.base_url + url, self.timeout)

    @post (url: str, body: str) -> Result<Response, Error> =
        native_http_post(self.base_url + url, body, self.timeout)

    @put (url: str, body: str) -> Result<Response, Error> =
        native_http_put(self.base_url + url, body, self.timeout)

    @delete (url: str) -> Result<void, Error> =
        native_http_delete(self.base_url + url, self.timeout)
}
```

### Mock Implementation

```sigil
type MockHttp = {
    responses: {str: Response},  // url -> response
    errors: {str: Error}         // url -> error to return
}

impl Http for MockHttp {
    @get (url: str) -> Result<Response, Error> =
        match(self.errors.get(url),
            Some(err) -> Err(err),
            None -> match(self.responses.get(url),
                Some(resp) -> Ok(resp),
                None -> Err(Error { message: "Not found: " + url })
            )
        )

    @post (url: str, body: str) -> Result<Response, Error> =
        self.get(url)  // Simplified mock

    @put (url: str, body: str) -> Result<Response, Error> =
        self.get(url)

    @delete (url: str) -> Result<void, Error> =
        match(self.get(url),
            Ok(_) -> Ok(void),
            Err(e) -> Err(e)
        )
}
```

---

## Common Capability Patterns

### HTTP Client

```sigil
trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
    @put (url: str, body: str) -> Result<Response, Error>
    @delete (url: str) -> Result<void, Error>
}

type Response = {
    status: int,
    body: str,
    headers: {str: str}
}
```

### File System

```sigil
trait FileSystem {
    @read (path: str) -> Result<str, Error>
    @write (path: str, content: str) -> Result<void, Error>
    @exists (path: str) -> bool
    @delete (path: str) -> Result<void, Error>
    @list (dir: str) -> Result<[str], Error>
}
```

### Clock / Time

```sigil
trait Clock {
    @now () -> Timestamp
    @today () -> Date
}
```

### Random

```sigil
trait Random {
    @int (min: int, max: int) -> int
    @float () -> float
    @choice<T> (items: [T]) -> T
    @shuffle<T> (items: [T]) -> [T]
}
```

### Environment Variables

```sigil
trait Env {
    @get (name: str) -> Option<str>
    @require (name: str) -> Result<str, Error>
}
```

### Logger

```sigil
trait Logger {
    @debug (message: str) -> void
    @info (message: str) -> void
    @warn (message: str) -> void
    @error (message: str) -> void
}
```

### Cache

```sigil
trait Cache {
    @get (key: str) -> Option<str>
    @set (key: str, value: str) -> void
    @set_with_ttl (key: str, value: str, ttl: Duration) -> void
    @delete (key: str) -> void
}
```

---

## Composing Capabilities

### Capability That Uses Another

```sigil
// A capability can be built on other capabilities
type LoggingHttp = {
    inner: dyn Http,
    logger: dyn Logger
}

impl Http for LoggingHttp {
    @get (url: str) -> Result<Response, Error> = run(
        self.logger.info("GET " + url),
        let result = self.inner.get(url),
        match(result,
            Ok(_) -> self.logger.debug("GET " + url + " succeeded"),
            Err(e) -> self.logger.error("GET " + url + " failed: " + e.message),
        ),
        result,
    )

    // ... other methods
}
```

### Using in Application

```sigil
@main () -> void =
    with Logger = StdoutLogger {} in
    with Http = LoggingHttp {
        inner: RealHttp { base_url: $api_url, timeout: 30s },
        logger: Logger  // Uses the Logger from scope
    } in
    run_app()
```

---

## Capability vs Regular Trait

Not every trait is a capability. The distinction:

| Aspect | Regular Trait | Capability Trait |
|--------|---------------|------------------|
| Purpose | Abstraction, polymorphism | Side effects, external resources |
| State | Usually stateless computation | Often involves I/O or state |
| Testing | No special handling | Needs mocking |
| Examples | `Eq`, `Hash`, `Display` | `Http`, `FileSystem`, `Clock` |

```sigil
// Regular trait: pure computation
trait Eq {
    @equals (self, other: Self) -> bool
}

// Capability trait: side effect
trait Http {
    @get (url: str) -> Result<Response, Error>  // I/O!
}
```

**Rule of thumb:** If a trait method could fail due to external factors (network, disk, time) or produces non-deterministic results, it's likely a capability.

---

## Best Practices

### Keep Capabilities Focused

```sigil
// Good: focused capability
trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

// Avoid: capability that does too much
trait Everything {
    @http_get (url: str) -> Result<Response, Error>
    @read_file (path: str) -> Result<str, Error>
    @get_time () -> Timestamp
    @random () -> int
}
```

### Use Result for Fallible Operations

```sigil
// Good: explicit failure
trait FileSystem {
    @read (path: str) -> Result<str, Error>
}

// Avoid: hiding failures
trait FileSystem {
    @read (path: str) -> str  // What if file doesn't exist?
}
```

### Provide Useful Mocks

```sigil
// Good: configurable mock
type MockClock = {
    fixed_time: Timestamp
}

impl Clock for MockClock {
    @now () -> Timestamp = self.fixed_time
}

// Usage in tests
@test_expiry tests @is_expired () -> void =
    with Clock = MockClock { fixed_time: Timestamp { seconds: 1000 } } in
    run(
        assert(is_expired(Token { expires_at: Timestamp { seconds: 500 } })),
        assert(not(is_expired(Token { expires_at: Timestamp { seconds: 2000 } }))),
    )
```

---

## Error Messages

### Missing Implementation

```
error[E0500]: trait `Http` is not implemented for type `MyType`
  --> src/main.si:10:5
   |
10 |     with Http = MyType {} in
   |                 ^^^^^^ `MyType` does not implement `Http`
   |
   = help: add `impl Http for MyType { ... }`
```

### Incomplete Implementation

```
error[E0501]: missing method in impl
  --> src/main.si:15:1
   |
15 | impl Http for MockHttp {
   | ^^^^^^^^^^^^^^^^^^^^^^ missing method `delete`
   |
   = note: trait `Http` requires method `@delete (url: str) -> Result<void, Error>`
```

---

## See Also

- [Async via Capabilities](../10-async/01-async-await.md) - How capabilities track async
- [Uses Clause](02-uses-clause.md) - Declaring capability dependencies
- [Testing Effectful Code](03-testing-effectful-code.md) - Mocking patterns
- [Trait Definitions](../04-traits/01-trait-definitions.md) - General trait syntax
