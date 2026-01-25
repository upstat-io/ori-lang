# Capabilities

This section covers Sigil's capability system for managing side effects, enabling testable code, and tracking async behavior.

---

## Overview

Capabilities are Sigil's solution to three related tensions:
- **Mandatory testing** - Every function must have tests
- **No magic DI** - Dependencies should be explicit
- **Side effects** - Real programs need I/O, network, time, etc.
- **Async tracking** - Know which functions may suspend

The capability system makes effects explicit in function signatures while providing clean mechanisms for testing and async tracking.

```sigil
// Declare a capability trait
trait Http {
    @get (url: str) -> Result<Response, Error>
}

// Function declares what capabilities it uses
// No 'async' keyword - Http capability implies suspension
@get_user (id: str) -> Result<User, Error> uses Http = try(
    let response = Http.get("/users/" + id)?,
    Ok(parse(response.body)),
)

// Tests provide mock implementations
@test_get_user tests @get_user () -> void =
    with Http = MockHttp { responses: {"/users/1": "{\"name\": \"Alice\"}"} } in
    run(
        let result = get_user("1"),
        assert_eq(result, Ok(User { name: "Alice" })),
    )
```

---

## Key Concepts

| Concept | Purpose |
|---------|---------|
| [Capability Traits](01-capability-traits.md) | Define interfaces for effects |
| [Uses Clause](02-uses-clause.md) | Declare function dependencies |
| [Testing Effectful Code](03-testing-effectful-code.md) | Mock capabilities in tests |

---

## The Async Capability

A key insight: **the `Async` capability explicitly tracks suspension**.

Traditional languages use `async/await`:
```rust
// Rust: async bubbles up the call stack
async fn fetch() -> Result<Data, Error> { ... }
async fn process() -> Result<Output, Error> {
    let data = fetch().await?;
    ...
}
```

Sigil uses an explicit `Async` capability:
```sigil
// Sigil: Async capability explicitly declares suspension
@fetch () -> Result<Data, Error> uses Http, Async = ...
@process () -> Result<Output, Error> uses Http, Async =
    let data = fetch()?,
    ...
```

Both approaches have the same "propagation tax" - callers must know about effects. But the `Async` capability gives you more:

| What You Get | async/await | uses Async |
|--------------|-------------|------------|
| Propagation info | Yes | Yes |
| Easy testing | No | Yes (sync mocks) |
| Explicit deps | No | Yes |
| Clean types | No | Yes |
| Sync/Async clear | Yes | Yes |

### Sync vs Async

The presence or absence of `Async` is explicit:
```sigil
// With Async: non-blocking, may suspend
@fetch () -> Result<Data, Error> uses Http, Async = Http.get(url)?

// Without Async: blocking, runs to completion
@fetch_sync () -> Result<Data, Error> uses Http = Http.get(url)?
```

See [Async via Capabilities](../10-async/01-async-await.md) for the full explanation.

---

## Why Capabilities?

### The Problem

Consider testing a function with side effects:

```sigil
@get_user (id: str) -> Result<User, Error> = try(
    let json = http.get("https://api.com/users/" + id)?,  // Side effect!
    Ok(parse(json)),
)
```

Without capabilities:
- Tests hit the real network
- Builds fail if network is down
- Tests are slow and non-reproducible
- Cannot test error cases easily

### The Solution

Capabilities make effects explicit and injectable:

```sigil
// Effect is declared
@get_user (id: str) -> Result<User, Error> uses Http = ...

// Production: provide real implementation
@main () -> void =
    with Http = RealHttp { base_url: $api_url } in
    run_app()

// Tests: provide mock
@test_get_user tests @get_user () -> void =
    with Http = MockHttp { ... } in
    run(...)
```

---

## Design Principles

### Explicit Dependencies

Functions declare exactly what effects they need:

```sigil
// Clear: this function needs Http and Cache
@fetch_cached (key: str) -> Result<Data, Error> uses Http, Cache = ...
```

### Compile-Time Safety

Forgetting to provide a capability is a compile error:

```sigil
@main () -> void = run(
    get_user("1")  // ERROR: Http capability not provided
)
```

### No Hidden State

Capabilities are passed explicitly through `with`...`in`, not stored in globals:

```sigil
// Good: explicit
with Http = RealHttp {} in
    get_user("1")

// Not possible: no implicit global
get_user("1")  // ERROR
```

### Propagation

If function `f` calls function `g` which `uses Http`, then `f` must either:
1. Declare `uses Http` itself, or
2. Provide `Http` with `with`...`in`

```sigil
@get_user (id: str) -> Result<User, Error> uses Http = ...

// Option 1: propagate the requirement
@get_all_users (ids: [str]) -> Result<[User], Error> uses Http =
    traverse(ids, id -> get_user(id))

// Option 2: provide it locally
@get_all_users_v2 (ids: [str], http: Http) -> Result<[User], Error> =
    with Http = http in
    traverse(ids, id -> get_user(id))
```

---

## The Trade-Off

Capabilities have one trade-off: **implementation details propagate to callers**.

If `fetch_user` uses `Http`, callers must either declare `uses Http` or provide it. This "leaks" the fact that HTTP is used somewhere in the call chain.

**But this is the same trade-off as async/await.** In Rust/JS/Python, if you call an async function, your function must be async too. The "leaking" happens either way.

The question is: **if you're paying this cost anyway, why not get testing benefits too?**

---

## Documents in This Section

1. **[Capability Traits](01-capability-traits.md)** - Defining capability interfaces
2. **[Uses Clause](02-uses-clause.md)** - Declaring and propagating dependencies
3. **[Testing Effectful Code](03-testing-effectful-code.md)** - Mocking and testing patterns

---

## See Also

- [Async](../10-async/index.md) - How capabilities track async
- [Traits](../04-traits/index.md) - Capabilities are traits
- [Testing](../11-testing/index.md) - Mandatory testing requirements
- [Error Handling](../05-error-handling/index.md) - Result types with capabilities
