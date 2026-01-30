---
title: "Capabilities"
description: "Ori Language Specification — Capabilities"
order: 14
section: "Verification"
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

Capabilities are traits with default implementations:

```ori
pub trait Http {
    @get (url: str) -> Result<Response, Error>
    @post (url: str, body: str) -> Result<Response, Error>
}

pub def impl Http {
    @get (url: str) -> Result<Response, Error> = ...
    @post (url: str, body: str) -> Result<Response, Error> = ...
}
```

Import the trait to use the default:

```ori
use std.net.http { Http }

@fetch () -> Result<str, Error> uses Http =
    Http.get(url: "https://api.example.com/data")
```

Other standard capability traits:

```ori
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

`Async` is a marker capability indicating a function may suspend. A function with `uses Async` requires an async context to execute.

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

See [Concurrency Model](23-concurrency-model.md) for task definitions, async context semantics, and suspension points.

## Providing Capabilities

Default implementations are automatically bound when importing a trait. Override with `with...in` when custom configuration or mocking is needed:

```ori
with Http = ConfiguredHttp { timeout: 5s } in
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

`Print` has a default implementation via `def impl`. Programs may use `print` without declaring `uses Print`:

```ori
@main () -> void = print(msg: "Hello, World!")
```

The default is `StdoutPrint` for native execution, `BufferPrint` for WASM.

### Name Resolution

When resolving a capability name:

1. Check for `with...in` binding (innermost first)
2. Check for imported default (`def impl` from source module)
3. Check for module-local `def impl`
4. Error: capability not provided

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
