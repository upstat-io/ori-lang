# Capabilities

This section defines the capability system for effect tracking.

## Overview

Capabilities are traits that represent access to external resources or side effects. Functions that perform effects must declare their capability requirements.

## Capability Declaration

### uses Clause

A function declares required capabilities with the `uses` clause:

```
function      = ... [ uses_clause ] "=" expression .
uses_clause   = "uses" identifier { "," identifier } .
```

```sigil
@fetch (url: str) -> Result<str, Error> uses Http = Http.get(url)

@save (data: str) -> Result<void, Error> uses FileSystem =
    FileSystem.write("/data.txt", data)
```

### Multiple Capabilities

```sigil
@fetch_and_save (url: str) -> Result<void, Error> uses Http, FileSystem = try(
    let content = Http.get(url)?,
    FileSystem.write("/data.txt", content),
)
```

## Capability Traits

A capability is defined as a regular trait:

```sigil
trait Http {
    @get (url: str) -> Result<str, Error>
    @post (url: str, body: str) -> Result<str, Error>
}

trait FileSystem {
    @read (path: str) -> Result<str, Error>
    @write (path: str, content: str) -> Result<void, Error>
}

trait Clock {
    @now () -> Timestamp
}
```

## Providing Capabilities

### with...in Expression

Capabilities are provided using `with...in`:

```
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
@helper () -> Result<str, Error> uses Http = Http.get("/data")

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

HTTP client operations:

```sigil
trait Http {
    @get (url: str) -> Result<str, Error>
    @post (url: str, body: str) -> Result<str, Error>
    @put (url: str, body: str) -> Result<str, Error>
    @delete (url: str) -> Result<void, Error>
}
```

### FileSystem

File system operations:

```sigil
trait FileSystem {
    @read (path: str) -> Result<str, Error>
    @write (path: str, content: str) -> Result<void, Error>
    @exists (path: str) -> bool
    @delete (path: str) -> Result<void, Error>
}
```

### Clock

Time operations:

```sigil
trait Clock {
    @now () -> Timestamp
    @today () -> Date
}
```

### Random

Random number generation:

```sigil
trait Random {
    @int (min: int, max: int) -> int
    @float () -> float
}
```

### Cache

Caching operations:

```sigil
trait Cache {
    @get (key: str) -> Option<str>
    @set (key: str, value: str) -> void
    @delete (key: str) -> void
}
```

### Logger

Logging operations:

```sigil
trait Logger {
    @debug (message: str) -> void
    @info (message: str) -> void
    @warn (message: str) -> void
    @error (message: str) -> void
}
```

### Env

Environment variable access:

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
    responses: {str: str},
}

impl Http for MockHttp {
    @get (url: str) -> Result<str, Error> =
        match(self.responses[url],
            Some(body) -> Ok(body),
            None -> Err(Error { message: "Not found" }),
        )
    // ... other methods
}
```

### Test Example

```sigil
@get_user (id: str) -> Result<User, Error> uses Http = try(
    let json = Http.get("/users/" + id)?,
    Ok(parse(json)),
)

@test_get_user tests @get_user () -> void =
    with Http = MockHttp {
        responses: {"/users/1": "{\"name\": \"Alice\"}"}
    } in
    run(
        let result = get_user("1"),
        assert(is_ok(result)),
        assert_eq(result.unwrap().name, "Alice"),
    )
```

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

## Effect Purity

A function without a `uses` clause is pure: it produces no side effects beyond its return value. Pure functions:

1. Always return the same result for the same arguments
2. Have no observable effects
3. Can be safely memoized, parallelized, or reordered

The capability system makes effect boundaries explicit and statically verified.
