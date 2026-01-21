# Uses Clause

This document covers the `uses` clause for declaring capability dependencies in function signatures.

---

## Overview

The `uses` clause declares what capabilities a function requires:

```sigil
@get_user (id: str) -> Result<User, Error> uses Http = try(
    json = Http.get("/users/" + id),
    Ok(parse(json))
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
    Http.get(url)

@read_config (path: str) -> Result<Config, Error> uses FileSystem = try(
    content = FileSystem.read(path),
    Ok(parse_config(content))
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
    cached = Cache.get(key),
    match(cached,
        Some(data) -> Ok(parse(data)),
        None -> run(
            response = Http.get("/data/" + key),
            Cache.set(key, response),
            Ok(parse(response))
        )
    )
)

@fetch_with_logging (url: str) -> Result<str, Error> uses Http, Logger = run(
    Logger.info("Fetching: " + url),
    result = Http.get(url),
    match(result,
        Ok(data) -> Logger.debug("Success: " + str(len(data)) + " bytes"),
        Err(e) -> Logger.error("Failed: " + e.message)
    ),
    result
)
```

---

## Using Capabilities in Function Body

When a function declares `uses Capability`, the capability is available as a value in the body:

```sigil
@example () -> void uses Logger = run(
    Logger.info("Hello"),     // Logger is available
    Logger.debug("World")
)
```

The capability name refers to the trait, and method calls go to the provided implementation:

```sigil
@get_user (id: str) -> Result<User, Error> uses Http = try(
    json = Http.get("/users/" + id),  // Calls Http.get on provided impl
    Ok(parse(json))
)

// When called with:
with Http = RealHttp { base_url: "https://api.com" } in
    get_user("123")
// Http.get calls RealHttp.get
```

---

## Providing Capabilities

### The `with`...`in` Expression

Capabilities are provided using `with Capability = implementation in expression`:

```sigil
with Http = RealHttp { base_url: "https://api.com" } in
    get_user("123")
```

### Multiple Capabilities

Chain `with` expressions:

```sigil
with Http = RealHttp { base_url: $api_url } in
with Cache = RedisCache { host: $redis_host } in
with Logger = StdoutLogger {} in
    fetch_and_cache_logged("key")
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
    log_something(),  // Uses ProductionLogger

    // Override for a specific section
    with Logger = VerboseLogger {} in
    run(
        debug_operation()  // Uses VerboseLogger
    ),

    log_something_else()  // Back to ProductionLogger
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
    traverse(ids, id -> get_user(id))

// Option 2: Provide locally
@get_all_users_with_http (ids: [str], http: Http) -> Result<[User], Error> =
    with Http = http in
    traverse(ids, id -> get_user(id))
```

### Compile Error Without

```sigil
// ERROR: get_user uses Http, but it's not provided
@broken (ids: [str]) -> Result<[User], Error> =
    traverse(ids, id -> get_user(id))
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
@level3 () -> str uses Http = Http.get("/")

@level2 () -> str uses Http = level3()

@level1 () -> str uses Http = level2()

@main () -> void =
    with Http = RealHttp {} in
    print(level1())
```

---

## Capabilities and Higher-Order Functions

### Passing Functions That Use Capabilities

When passing a function that uses a capability, the capability must be available:

```sigil
@fetch_item (id: str) -> Result<Item, Error> uses Http = ...

@process_all (ids: [str]) -> Result<[Item], Error> uses Http =
    traverse(ids, id -> fetch_item(id))  // OK: Http is available
```

### Lambdas Inherit Capabilities

Lambdas created in a `uses` context can use those capabilities:

```sigil
@process (ids: [str]) -> Result<[str], Error> uses Http =
    traverse(ids, id -> run(
        data = Http.get("/items/" + id),  // Lambda uses Http from enclosing scope
        Ok(data)
    ))
```

---

## Capabilities and Async

Capabilities work with async functions:

```sigil
trait AsyncHttp {
    @get (url: str) -> async Result<str, Error>
}

@fetch_user (id: str) -> async Result<User, Error> uses AsyncHttp = try(
    json = AsyncHttp.get("/users/" + id).await,
    Ok(parse(json))
)

@main () -> async void =
    with AsyncHttp = RealAsyncHttp {} in
    run(
        user = fetch_user("123").await,
        print(user.name)
    )
```

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
        config = load_config(),  // uses Env
        with Http = RealHttp { base_url: config.api_url } in
        with Database = PostgresDb { conn_str: config.db_url } in
        run_application()
    )
```

### Capability Wrappers

```sigil
@with_retry<T> (
    attempts: int,
    f: () -> Result<T, Error>
) -> Result<T, Error> uses Logger = run(
    result = f(),
    match(result,
        Ok(v) -> Ok(v),
        Err(e) -> if attempts > 1 then run(
            Logger.warn("Retrying after error: " + e.message),
            with_retry(attempts - 1, f)
        ) else Err(e)
    )
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
