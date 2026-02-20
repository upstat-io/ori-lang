---
title: "Error Propagation"
description: "The ? operator, try pattern, and error traces."
order: 8
part: "Error Handling & Safety"
---

# Error Propagation

Real programs often have chains of operations that can fail. The `?` operator and `try` pattern make error handling elegant without sacrificing safety.

## The Problem with Nested Errors

Without special syntax, error handling gets deeply nested:

```ori
@load_user_config (user_id: int) -> Result<Config, Error> = {
    let user_result = find_user(id: user_id)
    match user_result {
        Err(e) -> Err(e)
        Ok(user) -> {
            let file_result = read_file(path: user.config_path)
            match file_result {
                Err(e) -> Err(e)
                Ok(content) -> {
                    let config_result = parse_config(data: content)
                    match config_result {
                        Err(e) -> Err(e)
                        Ok(config) -> Ok(config)
                    }
                }
            }
        }
    }
}
```

This is hard to read and write.

## The `?` Operator

The `?` operator does two things:
1. If the value is `Ok(v)`, extract `v`
2. If the value is `Err(e)`, return early with that error

```ori
@load_user_config (user_id: int) -> Result<Config, Error> = {
    let user = find_user(id: user_id)?
    let content = read_file(path: user.config_path)?
    let config = parse_config(data: content)?
    Ok(config)
}
```

Much cleaner! Each `?` either continues with the success value or returns early with the error.

### How `?` Works

When you write `expression?`:

```ori
let value = fallible_operation()?
```

It expands to:

```ori
let value = match fallible_operation() {
    Ok(v) -> v
    Err(e) -> return Err(e)
}
```

The key insight is that `?` causes an early return on error.

### `?` with Option

The `?` operator also works with `Option`:

```ori
@get_user_city (id: int) -> Option<str> = {
    let user = find_user(id: id)?,          // Returns None if not found
    let address = user.address?,             // Returns None if no address
    let city = address.city?,                // Returns None if no city
    Some(city)
}
```

If any `?` encounters `None`, the function returns `None` immediately.

## The `try` Pattern

For a sequence of fallible operations, `try` is cleaner than `run`:

```ori
@load_user_config (user_id: int) -> Result<Config, Error> = try {
    let user = find_user(id: user_id)?
    let content = read_file(path: user.config_path)?
    let config = parse_config(data: content)?
    Ok(config)
}
```

`try` is designed for error-propagating code and provides better error traces.

### Mixing `try` and `run`

Use `try` when you're mainly propagating errors. Use `run` when you're not:

```ori
@process_batch (items: [int]) -> Result<Summary, Error> = try {
    let results = for item in items yield process_item(id: item)?

    // Switch to run for non-fallible computation
    let summary = {
        let total = len(collection: results)
        let sum = results.fold(initial: 0, op: (a, b) -> a + b)
        Summary { total, average: sum / total }
    }

    Ok(summary)
}
```

## Error Traces

When errors propagate with `?`, Ori records each propagation point.

### TraceEntry Type

```ori
type TraceEntry = {
    function: str,
    file: str,
    line: int,
    column: int,
}
```

### Accessing Traces

```ori
match result {
    Ok(v) -> print(msg: `Success: {v}`)
    Err(e) -> {
        print(msg: `Error: {e.message}`)
        if e.has_trace() then
            print(msg: `Trace:\n{e.trace()}`)
    }
}
```

### Trace Example

When an error propagates through multiple functions:

```ori
@inner () -> Result<int, Error> =
    Err(Error { message: "something went wrong" })

@middle () -> Result<int, Error> = try {
    let x = inner()?,    // Trace point 1
    Ok(x)
}

@outer () -> Result<int, Error> = try {
    let x = middle()?,   // Trace point 2
    Ok(x)
}
```

The error trace shows the propagation path:

```
Error: something went wrong
Trace:
  at middle (file.ori:5:13)
  at outer (file.ori:10:13)
```

## Adding Context

When propagating errors, add context to help with debugging:

```ori
@load_settings () -> Result<Settings, Error> = try {
    let data = read_file(path: "settings.json")
        .context(msg: "Failed to read settings file")?
    let settings = parse_settings(data: data)
        .context(msg: "Invalid settings format")?
    Ok(settings)
}
```

### How `.context()` Works

The `.context()` method:
1. Preserves the original error and its trace
2. Adds additional context information
3. Returns a new error with the combined information

```ori
let result = read_file(path: "missing.txt")
    .context(msg: "While loading configuration")

// If read_file returns Err(FileNotFound { path: "missing.txt" })
// The context wraps it with additional information
```

### Context vs map_err

| Method | Use Case |
|--------|----------|
| `.context()` | Add context while preserving trace |
| `.map_err()` | Transform error type completely |

```ori
// context: adds information, keeps trace
let result = fallible().context(msg: "extra info")?

// map_err: converts error type
let result = fallible().map_err(transform: e -> MyError { cause: e })?
```

## Custom Error Types

For complex error handling, define custom error types:

```ori
type ApiError =
    | NetworkError(message: str)
    | ParseError(message: str, line: int)
    | ValidationError(errors: [str])
    | NotFound(resource: str)

impl Printable for ApiError {
    @to_str (self) -> str = match self {
        NetworkError(msg) -> `Network error: {msg}`
        ParseError(msg, line) -> `Parse error at line {line}: {msg}`
        ValidationError(errors) -> `Validation errors: {errors.join(sep: ", ")}`
        NotFound(resource) -> `Not found: {resource}`
    }
}
```

### The Traceable Trait

For custom errors to work with traces:

```ori
trait Traceable {
    @with_trace (self, trace: [TraceEntry]) -> Self
    @trace (self) -> [TraceEntry]
}
```

Implement this to make your error types support traces:

```ori
type MyError = { message: str, trace: [TraceEntry] }

impl Traceable for MyError {
    @with_trace (self, trace: [TraceEntry]) -> Self =
        MyError { ...self, trace }

    @trace (self) -> [TraceEntry] = self.trace
}
```

For non-Traceable error types, traces attach to the `Result` wrapper during propagation.

## Error Conversion

### Converting Error Types

When functions return different error types:

```ori
@read_config () -> Result<Config, ConfigError> = try {
    // read_file returns IoError, but we need ConfigError
    let data = read_file(path: "config.json")
        .map_err(transform: e -> ConfigError.IoError(msg: e.message))?

    // parse returns ParseError
    let config = parse(data: data)
        .map_err(transform: e -> ConfigError.ParseError(msg: e.message))?

    Ok(config)
}
```

### The Into Trait

Types implementing `Into<Error>` convert automatically:

```ori
// str implements Into<Error>
@example () -> Result<int, Error> = try {
    let value = parse_int(s: "abc")
        .ok_or(error: "invalid number")?,  // str -> Error
    Ok(value)
}
```

## Combining Results

### Collecting Results

When you have a list of Results:

```ori
@process_all (ids: [int]) -> Result<[User], Error> = {
    let results = for id in ids yield fetch_user(id: id)

    // Check if any failed
    let first_error = results.iter()
        .find(predicate: r -> is_err(result: r))

    match first_error {
        Some(Err(e)) -> Err(e)
        _ -> Ok(for r in results yield match r { Ok(u) -> u, Err(_) -> continue})
    }
}
```

### Fail Fast

Stop on first error:

```ori
@process_all_fast (ids: [int]) -> Result<[User], Error> = try {
    let users = for id in ids yield fetch_user(id: id)?
    Ok(users)
}
```

### Collect All Errors

Gather all errors:

```ori
@process_all_errors (ids: [int]) -> Result<[User], [Error]> = {
    let results = for id in ids yield fetch_user(id: id)

    let errors = for r in results if is_err(result: r) yield match r {
        Err(e) -> e
        Ok(_) -> continue
    }

    if is_empty(collection: errors) then
        Ok(for r in results yield match r { Ok(u) -> u, Err(_) -> continue})
    else
        Err(errors)
}
```

## Complete Example

```ori
type User = { id: int, name: str, config_path: str }
type Config = { theme: str, language: str }
type AppError =
    | UserNotFound(id: int)
    | FileError(path: str, message: str)
    | ParseError(message: str)

impl Printable for AppError {
    @to_str (self) -> str = match self {
        UserNotFound(id) -> `User {id} not found`
        FileError(path, msg) -> `File error ({path}): {msg}`
        ParseError(msg) -> `Parse error: {msg}`
    }
}

// Simulated operations
@find_user (id: int) -> Result<User, AppError> =
    if id > 0 then
        Ok(User { id, name: "Test", config_path: "/config.json" })
    else
        Err(UserNotFound(id: id))

@test_find_user tests @find_user () -> void = {
    assert_ok(result: find_user(id: 1))
    assert_err(result: find_user(id: -1))
}

@read_config_file (path: str) -> Result<str, AppError> =
    if path == "/config.json" then
        Ok(`{"theme": "dark", "language": "en"}`)
    else
        Err(FileError(path: path, message: "not found"))

@test_read_config tests @read_config_file () -> void = {
    assert_ok(result: read_config_file(path: "/config.json"))
    assert_err(result: read_config_file(path: "/missing.json"))
}

@parse_config (data: str) -> Result<Config, AppError> =
    if data.contains(substring: "theme") then
        Ok(Config { theme: "dark", language: "en" })
    else
        Err(ParseError(message: "missing theme"))

@test_parse tests @parse_config () -> void = {
    assert_ok(result: parse_config(data: `{"theme": "dark"}`))
    assert_err(result: parse_config(data: `{}`))
}

// The main function using error propagation
@load_user_config (user_id: int) -> Result<Config, AppError> = try {
    let user = find_user(id: user_id)?
    let data = read_config_file(path: user.config_path)?
    let config = parse_config(data: data)?
    Ok(config)
}

@test_load_user_config tests @load_user_config () -> void = {
    assert_ok(result: load_user_config(user_id: 1))
    assert_err(result: load_user_config(user_id: -1))
}

// With context
@load_user_config_verbose (user_id: int) -> Result<Config, Error> = try {
    let user = find_user(id: user_id)
        .map_err(transform: e -> Error { message: e.to_str() })
        .context(msg: `Failed to find user {user_id}`)?

    let data = read_config_file(path: user.config_path)
        .map_err(transform: e -> Error { message: e.to_str() })
        .context(msg: `Failed to read config for {user.name}`)?

    let config = parse_config(data: data)
        .map_err(transform: e -> Error { message: e.to_str() })
        .context(msg: "Failed to parse config")?

    Ok(config)
}

@test_verbose tests @load_user_config_verbose () -> void = {
    assert_ok(result: load_user_config_verbose(user_id: 1))
}
```

## Quick Reference

### The `?` Operator

```ori
// With Result
let value = fallible()? // Returns early on Err

// With Option
let value = optional()? // Returns early on None
```

### The `try` Pattern

```ori
try {
    let a = step1()?
    let b = step2(input: a)?
    Ok(result)
}
```

### Context and Transformation

```ori
// Add context (preserves trace)
result.context(msg: "context string")?

// Transform error type
result.map_err(transform: e -> new_error)?
```

### TraceEntry

```ori
type TraceEntry = {
    function: str,
    file: str,
    line: int,
    column: int,
}

error.trace()          // Get trace as string
error.trace_entries()  // Get [TraceEntry]
error.has_trace()      // Check if trace exists
```

## What's Next

Now that you understand error propagation:

- **[Panic and Recovery](/guide/09-panic-recovery)** — Unrecoverable errors and contracts
- **[Modules and Imports](/guide/10-modules-imports)** — Organize your code
