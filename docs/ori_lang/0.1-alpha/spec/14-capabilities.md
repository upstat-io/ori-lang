---
title: "Capabilities"
description: "Ori Language Specification — Capabilities"
order: 14
---

# Capabilities

Capabilities are traits representing effects or suspension.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § DECLARATIONS (uses_clause), EXPRESSIONS (with_expr)

## Declaration

```ori
@fetch (url: str) -> Result<Response, Error> uses Http = Http.get(url)

@save (data: str) -> Result<void, Error> uses FileSystem, Async =
    FileSystem.write(path: "/data.txt", content: data)
```

## Capability Traits

```ori
trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

trait FileSystem {
    @read (path: str) -> Result<str, Error>
    @write (path: str, content: str) -> Result<void, Error>
}

trait Print {
    @print (msg: str) -> void
    @println (msg: str) -> void
    @output () -> str
    @clear () -> void
}
```

## Async Capability

`Async` indicates a function may suspend.

| With `uses Async` | Without |
|-------------------|---------|
| Non-blocking, may suspend | Blocking, synchronous |

```ori
@fetch_async (url: str) -> Result<Data, Error> uses Http, Async = ...  // may suspend
@fetch_sync (url: str) -> Result<Data, Error> uses Http = ...          // blocks
```

No `async` type modifier. No `.await` expression. Return type is final value type.

Concurrency via `parallel` pattern:

```ori
parallel(tasks: [fetch(a), fetch(b)], max_concurrent: 10)
```

## Providing Capabilities

```ori
with Http = RealHttp { base_url: "https://api.example.com" } in
    fetch("/data")
```

## Propagation

Capabilities propagate: if A calls B with capability C, A must declare or provide C.

```ori
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
| `Print` | Standard output | No |
| `Logger` | Structured logging | No |
| `Env` | Environment | No |
| `Async` | Suspension marker | Yes |

## Default Capabilities

`Print` has a default implementation. Programs may use `print` without declaring `uses Print`:

```ori
@main () -> void = print(msg: "Hello, World!")
```

The default is `StdoutPrint` for native execution, `BufferPrint` for WASM.

## Testing

```ori
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
