# Capabilities

Capabilities are traits representing effects or suspension.

## Declaration

```
uses_clause = "uses" identifier { "," identifier } .
```

```sigil
@fetch (url: str) -> Result<Response, Error> uses Http = Http.get(url)

@save (data: str) -> Result<void, Error> uses FileSystem, Async =
    FileSystem.write(path: "/data.txt", content: data)
```

## Capability Traits

```sigil
trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

trait FileSystem {
    @read (path: str) -> Result<str, Error>
    @write (path: str, content: str) -> Result<void, Error>
}
```

## Async Capability

`Async` indicates a function may suspend.

| With `uses Async` | Without |
|-------------------|---------|
| Non-blocking, may suspend | Blocking, synchronous |

```sigil
@fetch_async (url: str) -> Result<Data, Error> uses Http, Async = ...  // may suspend
@fetch_sync (url: str) -> Result<Data, Error> uses Http = ...          // blocks
```

No `async` type modifier. No `.await` expression. Return type is final value type.

Concurrency via `parallel` pattern:

```sigil
parallel(tasks: [fetch(a), fetch(b)], max_concurrent: 10)
```

## Providing Capabilities

```
with_expr = "with" identifier "=" expression "in" expression .
```

```sigil
with Http = RealHttp { base_url: "https://api.example.com" } in
    fetch("/data")
```

## Propagation

Capabilities propagate: if A calls B with capability C, A must declare or provide C.

```sigil
@helper () -> str uses Http = Http.get("/").body

// Must declare Http
@caller () -> str uses Http = helper()

// Or provide it
@caller () -> str = with Http = MockHttp {} in helper()
```

## Standard Capabilities

| Capability | Purpose | Suspends |
|------------|---------|----------|
| `Http` | HTTP client | May |
| `FileSystem` | File I/O | May |
| `Cache` | Caching | May |
| `Clock` | Time | No |
| `Random` | RNG | No |
| `Logger` | Logging | No |
| `Env` | Environment | No |
| `Async` | Suspension marker | Yes |

## Testing

```sigil
@test_fetch tests @fetch () -> void =
    with Http = MockHttp { responses: {"/users/1": "{...}"} } in
    run(
        assert_ok(result: fetch(url: "/users/1")),
    )
```

Mock implementations are synchronous; test does not need `Async`.

## Purity

Functions without `uses` are pure: no side effects, cannot suspend, safely parallelizable.

## Errors

```
error[E0600]: function uses `Http` without declaring it
```

Capabilities must be explicitly declared or provided.
