# Uses Clause

This document covers the `uses` clause for declaring capability dependencies in function signatures.

---

## Overview

The `uses` clause declares what capabilities a function requires:

```sigil
@get_user (id: str) -> Result<User, Error> uses Http = try(
    let json = Http.get(
        .url: "/users/" + id,
    )?,
    Ok(parse(
        .input: json,
    )),
)
```

The function signature says: "This function takes a string, returns a Result, and requires the `Http` capability."

---

## Basic Syntax

### Single Capability

```sigil
@function_name (params) -> ReturnType uses Capability = body
```

Example:

```sigil
@fetch_data (url: str) -> Result<str, Error> uses Http =
    Http.get(
        .url: url,
    )

@read_config (path: str) -> Result<Config, Error> uses FileSystem = try(
    let content = FileSystem.read(
        .path: path,
    )?,
    Ok(parse_config(
        .content: content,
    )),
)

@get_timestamp () -> Timestamp uses Clock =
    Clock.now()
```

### Multiple Capabilities

Separate multiple capabilities with commas:

```sigil
@function_name (params) -> ReturnType uses Cap1, Cap2, Cap3 = body
```

Example:

```sigil
@fetch_and_cache (key: str) -> Result<Data, Error> uses Http, Cache = try(
    let cached = Cache.get(
        .key: key,
    ),
    match(cached,
        Some(data) -> Ok(parse(
            .input: data,
        )),
        None -> run(
            let response = Http.get(
                .url: "/data/" + key,
            )?,
            Cache.set(
                .key: key,
                .value: response,
            ),
            Ok(parse(
                .input: response,
            )),
        ),
    ),
)

@fetch_with_logging (url: str) -> Result<str, Error> uses Http, Logger = run(
    Logger.info(
        .message: "Fetching: " + url,
    ),
    let result = Http.get(
        .url: url,
    ),
    match(result,
        Ok(data) -> Logger.debug(
            .message: "Success: " + str(len(
                .collection: data,
            )) + " bytes",
        ),
        Err(err) -> Logger.error(
            .message: "Failed: " + err.message,
        ),
    ),
    result,
)
```

---

## Using Capabilities in Function Body

When a function declares `uses Capability`, the capability is available as a value in the body:

```sigil
// Logger is available in the function body
@example () -> void uses Logger = run(
    Logger.info(
        .message: "Hello",
    ),
    Logger.debug(
        .message: "World",
    ),
)
```

The capability name refers to the trait, and method calls go to the provided implementation:

```sigil
// Calls Http.get on the provided implementation
@get_user (id: str) -> Result<User, Error> uses Http = try(
    let json = Http.get(
        .url: "/users/" + id,
    )?,
    Ok(parse(
        .input: json,
    )),
)

// When called with:
// Http.get calls RealHttp.get
with Http = RealHttp { base_url: "https://api.com" } in
    get_user(
        .id: "123",
    )
```

---

## Providing Capabilities

### The `with`...`in` Expression

Capabilities are provided using `with Capability = implementation in expression`:

```sigil
with Http = RealHttp { base_url: "https://api.com" } in
    get_user(
        .id: "123",
    )
```

### Multiple Capabilities

Chain `with` expressions:

```sigil
with Http = RealHttp { base_url: $api_url } in
with Cache = RedisCache { host: $redis_host } in
with Logger = StdoutLogger {} in
    fetch_and_cache_logged(
        .key: "key",
    )
```

### In Main

```sigil
@main () -> void =
    with Http = RealHttp { base_url: $api_url } in
    with FileSystem = RealFileSystem {} in
    with Clock = SystemClock {} in
    run_app()
```

### Nested Scopes

Inner `with` shadows outer:

```sigil
with Logger = ProductionLogger {} in
run(
    // Uses ProductionLogger
    log_something(),

    // Override for a specific section
    with Logger = VerboseLogger {} in
    run(
        // Uses VerboseLogger
        debug_operation(),
    ),

    // Back to ProductionLogger
    log_something_else(),
)
```

---

## Capability Propagation

### Automatic Propagation

If function `f` calls function `g` which `uses Http`, then `f` must either:

1. **Declare `uses Http`** — propagate the requirement up
2. **Provide `Http` with `with`** — satisfy the requirement locally

```sigil
@get_user (id: str) -> Result<User, Error> uses Http = ...

// Option 1: Propagate
@get_all_users (ids: [str]) -> Result<[User], Error> uses Http =
    traverse(
        .over: ids,
        .transform: id -> get_user(
            .id: id,
        ),
    )

// Option 2: Provide locally
@get_all_users_with_http (ids: [str], http: Http) -> Result<[User], Error> =
    with Http = http in
    traverse(
        .over: ids,
        .transform: id -> get_user(
            .id: id,
        ),
    )
```

### Compile Error Without

```sigil
// ERROR: get_user uses Http, but it's not provided
@broken (ids: [str]) -> Result<[User], Error> =
    traverse(
        .over: ids,
        .transform: id -> get_user(
            .id: id,
        ),
    )
```

```
error[E0510]: capability `Http` not provided
  --> src/main.si:5:25
   |
5  |     traverse(ids, id -> get_user(id))
   |                         ^^^^^^^^^^^^ `get_user` requires `Http` capability
   |
   = help: add `uses Http` to function signature
   = help: or provide with `with Http = ... in`
```

### Transitive Propagation

Capabilities propagate through call chains:

```sigil
@level3 () -> str uses Http = Http.get(
    .url: "/",
)

@level2 () -> str uses Http = level3()

@level1 () -> str uses Http = level2()

@main () -> void =
    with Http = RealHttp {} in
    print(
        .message: level1(),
    )
```

---

## Capabilities and Higher-Order Functions

### Passing Functions That Use Capabilities

When passing a function that uses a capability, the capability must be available:

```sigil
@fetch_item (id: str) -> Result<Item, Error> uses Http = ...

// OK: Http is available
@process_all (ids: [str]) -> Result<[Item], Error> uses Http =
    traverse(
        .over: ids,
        .transform: id -> fetch_item(
            .id: id,
        ),
    )
```

### Lambdas Inherit Capabilities

Lambdas created in a `uses` context can use those capabilities:

```sigil
// Lambda uses Http from enclosing scope
@process (ids: [str]) -> Result<[str], Error> uses Http =
    traverse(
        .over: ids,
        .transform: id -> run(
            let data = Http.get(
                .url: "/items/" + id,
            )?,
            Ok(data),
        ),
    )
```

---

## Capabilities and Async

Capabilities work with async functions. The `Async` capability explicitly marks functions that may suspend:

```sigil
trait Http {
    @get (url: str) -> Result<str, Error>
}

@fetch_user (id: str) -> Result<User, Error> uses Http, Async = try(
    let json = Http.get(
        .url: "/users/" + id,
    )?,
    Ok(parse(
        .input: json,
    )),
)

@main () -> void =
    with Http = RealHttp {} in
    run(
        let user = fetch_user(
            .id: "123",
        ),
        print(
            .message: user.name,
        ),
    )
```

See [Async via Capabilities](../10-async/01-async-await.md) for details on how `uses Async` tracks suspension.

---

## Common Patterns

### Dependency Injection at Entry Point

```sigil
@main () -> void =
    with Http = RealHttp { base_url: $api_url } in
    with Database = PostgresDb { conn_str: $db_url } in
    with Logger = FileLogger { path: $log_path } in
    with Clock = SystemClock {} in
    run_application()
```

### Environment-Based Configuration

```sigil
@main () -> void =
    with Env = SystemEnv {} in
    run(
        // load_config uses Env
        let config = load_config(),
        with Http = RealHttp { base_url: config.api_url } in
        with Database = PostgresDb { conn_str: config.db_url } in
        run_application(),
    )
```

### Capability Wrappers

```sigil
@with_retry<T> (
    attempts: int,
    action: () -> Result<T, Error>
) -> Result<T, Error> uses Logger = run(
    let result = action(),
    match(result,
        Ok(value) -> Ok(value),
        Err(err) -> if attempts > 1 then run(
            Logger.warn(
                .message: "Retrying after error: " + err.message,
            ),
            with_retry(
                .attempts: attempts - 1,
                .action: action,
            ),
        ) else Err(err),
    ),
)
```

---

## Error Messages

### Missing Capability

```
error[E0510]: capability `Http` not provided
  --> src/main.si:10:5
   |
10 |     get_user("123")
   |     ^^^^^^^^^^^^^^^ `get_user` requires `Http` capability
   |
   = help: add `uses Http` to function signature
   = help: or provide with `with Http = impl in`
```

### Wrong Type for Capability

```
error[E0511]: type mismatch for capability `Http`
  --> src/main.si:10:17
   |
10 |     with Http = "not an http" in
   |                 ^^^^^^^^^^^^^ expected type implementing `Http`, found `str`
```

### Undeclared Capability Usage

```
error[E0512]: use of capability `Logger` not declared in function signature
  --> src/main.si:5:5
   |
3  | @process (x: int) -> int uses Http =
   |                          --------- capabilities declared here
   |
5  |     Logger.info("processing")
   |     ^^^^^^ `Logger` not in `uses` clause
   |
   = help: add `Logger` to uses clause: `uses Http, Logger`
```

---

## See Also

- [Capability Traits](01-capability-traits.md) — Defining capabilities
- [Testing Effectful Code](03-testing-effectful-code.md) — Mocking in tests
- [Function Definitions](../07-functions/01-function-definitions.md) — General function syntax
