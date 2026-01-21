# Capabilities

This section defines the capability system for effect tracking and async behavior.

## Overview

Capabilities are traits that represent access to external resources, side effects, or operations that may suspend execution. Functions that perform effects must declare their capability requirements.

Capabilities serve two purposes:
1. **Effect tracking** - Making side effects explicit and injectable
2. **Async tracking** - Identifying functions that may suspend execution

## Capability Declaration

### uses Clause

A function declares required capabilities with the `uses` clause:

```ebnf
function      = ... [ uses_clause ] "=" expression .
uses_clause   = "uses" identifier { "," identifier } .
```

```sigil
@fetch (url: str) -> Result<Response, Error> uses Http = Http.get(url)

@save (data: str) -> Result<void, Error> uses FileSystem =
    FileSystem.write("/data.txt", data)
```

### Multiple Capabilities

```sigil
@fetch_and_save (url: str) -> Result<void, Error> uses Http, FileSystem = try(
    let content = Http.get(url)?,
    FileSystem.write("/data.txt", content.body),
)
```

## Capability Traits

A capability is defined as a regular trait:

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

> **Note:** Capability methods do not use `async` return types. Suspension behavior is tracked implicitly through capability usage.

## The Async Capability

### Explicit Suspension Declaration

`Async` is a capability that represents the ability to suspend execution. Functions that may suspend must explicitly declare `uses Async`:

```sigil
// Async is a marker capability
trait Async {}

// Function explicitly declares it may suspend
@fetch_user (id: str) -> Result<User, Error> uses Http, Async =
    Http.get("/users/" + id)?.parse()
```

### Sync vs Async Behavior

The presence or absence of `Async` determines whether I/O operations block or suspend:

```sigil
// With Async: Http.get may suspend (non-blocking)
@fetch_async (url: str) -> Result<Data, Error> uses Http, Async =
    Http.get(url)?.body

// Without Async: Http.get blocks until complete (synchronous)
@fetch_blocking (url: str) -> Result<Data, Error> uses Http =
    Http.get(url)?.body
```

### No async Type Modifier

Unlike languages with `async/await`, Sigil does not have an `async T` type. The return type is the final value type:

```sigil
// Return type is Result<User, Error>, not async Result<User, Error>
@fetch_user (id: str) -> Result<User, Error> uses Http, Async = ...
```

### No await Expression

Sigil does not have an `.await` expression. Suspension is declared at the function level via `uses Async`, not at each call site:

```sigil
@fetch_data (url: str) -> Result<Data, Error> uses Http, Async = run(
    // No .await needed - Async capability declares suspension
    let response = Http.get(url)?,
    Ok(parse(response.body)),
)
```

### Concurrency with parallel

Concurrent execution is achieved through the `parallel` pattern:

```sigil
@fetch_both (id: int) -> Result<{ user: User, posts: Posts }, Error> uses Http, Async =
    parallel(
        .user: fetch_user(id),
        .posts: fetch_posts(id),
    )
```

## Providing Capabilities

### with...in Expression

Capabilities are provided using `with...in`:

```ebnf
with_expr     = "with" identifier "=" expression "in" expression .
```

```sigil
with Http = RealHttp { base_url: "https://api.example.com" } in
with FileSystem = LocalFileSystem {} in
    fetch_and_save("/data")
```

### Scoping

A capability binding is in scope for the `in` expression:

```sigil
@main () -> void =
    with Http = RealHttp {} in
    with Cache = RedisCache {} in
    run_application()
```

## Capability Propagation

### Transitive Requirements

If function A calls function B, and B requires capability C, then A must also require C (or provide it):

```sigil
@helper () -> Result<str, Error> uses Http = Http.get("/data")?.body

// Must declare Http because helper uses it
@process () -> Result<str, Error> uses Http = run(
    let data = helper()?,
    Ok(transform(data)),
)
```

### Providing vs Requiring

A function may provide a capability rather than requiring it:

```sigil
@run_with_mock () -> Result<str, Error> =
    with Http = MockHttp {} in
    helper()  // helper uses Http, we provide it
```

## Standard Capabilities

### Http

HTTP client operations (async capability - may suspend):

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

### FileSystem

File system operations (async capability - may suspend):

```sigil
trait FileSystem {
    @read (path: str) -> Result<str, Error>
    @write (path: str, content: str) -> Result<void, Error>
    @exists (path: str) -> bool
    @delete (path: str) -> Result<void, Error>
}
```

### Clock

Time operations (sync capability - does not suspend):

```sigil
trait Clock {
    @now () -> Timestamp
    @today () -> Date
}
```

### Random

Random number generation (sync capability - does not suspend):

```sigil
trait Random {
    @int (min: int, max: int) -> int
    @float () -> float
}
```

### Cache

Caching operations (async capability - may suspend):

```sigil
trait Cache {
    @get (key: str) -> Option<str>
    @set (key: str, value: str) -> void
    @delete (key: str) -> void
}
```

### Logger

Logging operations (sync capability - does not suspend):

```sigil
trait Logger {
    @debug (message: str) -> void
    @info (message: str) -> void
    @warn (message: str) -> void
    @error (message: str) -> void
}
```

### Env

Environment variable access (sync capability - does not suspend):

```sigil
trait Env {
    @get (name: str) -> Option<str>
}
```

## Testing with Capabilities

### Mock Implementations

Tests provide mock implementations of capabilities:

```sigil
type MockHttp = {
    responses: {str: Response},
}

impl Http for MockHttp {
    @get (url: str) -> Result<Response, Error> =
        match(self.responses.get(url),
            Some(resp) -> Ok(resp),
            None -> Err(Error { message: "Not found" }),
        )
    // ... other methods
}
```

### Test Example

```sigil
// Production code uses Http + Async
@get_user (id: str) -> Result<User, Error> uses Http, Async = try(
    let response = Http.get("/users/" + id)?,
    Ok(parse(response.body)),
)

// Tests use sync mock - no Async needed
@test_get_user tests @get_user () -> void =
    with Http = MockHttp {
        responses: {"/users/1": Response { status: 200, body: "{\"name\": \"Alice\"}", headers: {} }}
    } in
    run(
        let result = get_user("1"),
        assert(is_ok(result)),
        assert_eq(result.unwrap().name, "Alice"),
    )
```

Note: The test does not declare `Async` because `MockHttp` is synchronous - it returns immediately without suspending.

## Capability Constraints

### Compile-Time Enforcement

It is a compile-time error if:

1. A function uses a capability without declaring it
2. A capability is used but not provided in scope
3. A capability is declared but not used (warning)

```
error[E0600]: function uses `Http` capability without declaring it
  --> api.si:5:10
   |
 5 |     Http.get(url)
   |          ^^^ uses Http
   |
   = help: add `uses Http` to function signature
```

### No Implicit Capabilities

Capabilities must be explicitly declared. There are no implicit effects.

## Capability vs Regular Trait

| Aspect | Regular Trait | Capability |
|--------|---------------|------------|
| Purpose | Abstraction | Effects |
| State | Stateless | Typically stateful/I/O |
| Declaration | `impl Trait for Type` | `impl Trait for Type` |
| Usage | Method call | `uses` clause + method |
| Testing | Direct | Requires mock |

The distinction is semantic, not syntactic. A trait becomes a capability when it represents external effects.

### The Async Capability

`Async` is a special capability that indicates a function may suspend:

| With `uses Async` | Without `uses Async` |
|-------------------|----------------------|
| Non-blocking | Blocking |
| May suspend | Never suspends |
| Requires async runtime | Runs synchronously |

## Effect Purity

A function without a `uses` clause is pure: it produces no side effects beyond its return value. Pure functions:

1. Always return the same result for the same arguments
2. Have no observable effects
3. Cannot suspend execution (no `uses Async`)
4. Can be safely memoized, parallelized, or reordered

A function with `uses` but without `Async` may have effects but will not suspend - it runs synchronously to completion.

The capability system makes effect boundaries explicit and statically verified.

## Rationale

> **Note:** The following is informative.

Sigil uses an explicit `Async` capability instead of `async/await` syntax because:

1. **Same propagation cost** - Both `async` and `uses Async` propagate up the call stack
2. **Additional benefits** - Capabilities provide easy testing via sync mock implementations
3. **Cleaner types** - Return types are `T` instead of `async T`
4. **Clear sync/async distinction** - Omitting `Async` means synchronous blocking behavior
5. **Familiar keyword** - Developers recognize `Async` immediately
6. **Unified model** - All effects (I/O, suspension, non-determinism) tracked uniformly

See [Design: Async via Capabilities](../design/10-async/01-async-await.md) for detailed rationale.
