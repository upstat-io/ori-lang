# Capabilities

This section covers Sigil's capability system for managing side effects and enabling testable code.

---

## Overview

Capabilities are Sigil's solution to the tension between:
- **Mandatory testing** — Every function must have tests
- **No magic DI** — Dependencies should be explicit
- **Side effects** — Real programs need I/O, network, time, etc.

The capability system makes effects explicit in function signatures while providing a clean mechanism for testing.

```sigil
// Declare a capability trait
trait Http {
    @get (url: str) -> Result<str, Error>
}

// Function declares what capabilities it uses
@get_user (id: str) -> Result<User, Error> uses Http = try(
    json = Http.get("/users/" + id),
    Ok(parse(json))
)

// Tests provide mock implementations
@test_get_user tests @get_user () -> void =
    with Http = MockHttp { responses: {"/users/1": "{\"name\": \"Alice\"}"} } in
    run(
        result = get_user("1"),
        assert_eq(result, Ok(User { name: "Alice" }))
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

## Why Capabilities?

### The Problem

Consider testing a function with side effects:

```sigil
@get_user (id: str) -> Result<User, Error> = try(
    json = http.get("https://api.com/users/" + id),  // Side effect!
    Ok(parse(json))
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

## Documents in This Section

1. **[Capability Traits](01-capability-traits.md)** — Defining capability interfaces
2. **[Uses Clause](02-uses-clause.md)** — Declaring and propagating dependencies
3. **[Testing Effectful Code](03-testing-effectful-code.md)** — Mocking and testing patterns

---

## See Also

- [Traits](../04-traits/index.md) — Capabilities are traits
- [Testing](../11-testing/index.md) — Mandatory testing requirements
- [Error Handling](../05-error-handling/index.md) — Result types with capabilities
