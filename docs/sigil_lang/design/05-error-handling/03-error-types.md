# Error Types

This document covers defining and working with error types in Sigil: user-defined errors, the standard `Error` type, error conversion, and best practices.

---

## Philosophy

Sigil encourages **precise, matchable error types**:

| Approach | Expressiveness | Handling | AI Compatibility |
|----------|----------------|----------|------------------|
| String errors | Low | Pattern match impossible | Poor |
| Error codes | Medium | Magic numbers | Medium |
| Sum type errors | High | Exhaustive matching | Excellent |

Sum type errors let you:
- Know exactly what can go wrong
- Handle each case explicitly
- Get compiler errors when new cases are added

---

## Defining Error Types

Error types are sum types (enums) that describe failure cases:

```sigil
type FileError =
    | NotFound(path: str)
    | PermissionDenied(path: str)
    | IsDirectory(path: str)
    | Corrupted(path: str, details: str)

type ParseError =
    | InvalidSyntax(line: int, column: int, message: str)
    | UnexpectedEof
    | UnexpectedToken(expected: str, found: str)

type NetworkError =
    | ConnectionFailed(host: str)
    | Timeout(after_ms: int)
    | InvalidResponse(status: int)
```

### Naming Conventions

- Error type names end with `Error`: `FileError`, `ParseError`
- Variant names describe the error condition: `NotFound`, `Timeout`
- Include relevant context as variant fields: `NotFound(path: str)`

---

## Using Error Types

### In Function Signatures

```sigil
@read_file (path: str) -> Result<str, FileError> = ...

@parse_config (content: str) -> Result<Config, ParseError> = ...

@connect (url: str) -> Result<Connection, NetworkError> = ...
```

### Creating Errors

```sigil
// Simple variant
Err(ParseError.UnexpectedEof)

// Variant with data
Err(FileError.NotFound(path: "/etc/config.json"))

// In function body
@read_file (path: str) -> Result<str, FileError> =
    if !exists(path) then Err(FileError.NotFound(path: path))
    else if !is_readable(path) then Err(FileError.PermissionDenied(path: path))
    else Ok(read_contents(path))
```

### Matching Errors

Use `match` to handle specific error cases:

```sigil
@handle_file_error (e: FileError) -> str = match(e,
    NotFound(path) -> "File not found: " + path,
    PermissionDenied(path) -> "Cannot access: " + path,
    IsDirectory(path) -> "Expected file, got directory: " + path,
    Corrupted(path, details) -> "File corrupted: " + path + " (" + details + ")"
)

@try_read (path: str) -> str = match(read_file(path),
    Ok(content) -> content,
    Err(FileError.NotFound(_)) -> "",  // Default for missing
    Err(e) -> run(
        log_error(e),
        panic("Unexpected file error")
    )
)
```

---

## The Standard Error Type

Sigil provides a built-in `Error` type for when you don't need precise error information:

```sigil
// Built-in definition
type Error = {
    message: str,
    source: Option<Error>
}
```

### When to Use Error

- Application-level code where error details don't affect handling
- Prototyping before defining precise error types
- Aggregating errors from multiple sources

```sigil
// Simple application code
@main () -> Result<void, Error> = try(
    config = load_config("app.json"),
    data = fetch_data(config.url),
    process(data),
    Ok(())
)
```

### Creating Error Values

```sigil
// Simple error
error = Error { message: "something went wrong", source: None }

// With source (error chaining)
error = Error {
    message: "failed to load config",
    source: Some(original_error)
}

// Helper function
@error (msg: str) -> Error = Error { message: msg, source: None }

// With cause
@error_from (msg: str, cause: Error) -> Error =
    Error { message: msg, source: Some(cause) }
```

---

## Error Conversion

When combining operations with different error types, you must convert errors explicitly.

### Manual Conversion

```sigil
type AppError =
    | ConfigError(str)
    | NetworkError(NetworkError)
    | ParseError(ParseError)

@load_app (config_path: str) -> Result<App, AppError> = try(
    // Convert FileError to AppError
    content = read_file(config_path) | e -> match(e,
        FileError.NotFound(p) -> AppError.ConfigError("Config not found: " + p),
        e -> AppError.ConfigError("Config error: " + str(e))
    ),

    // Convert ParseError to AppError
    config = parse_config(content) | e -> AppError.ParseError(e),

    // Convert NetworkError to AppError
    data = fetch(config.url) | e -> AppError.NetworkError(e),

    Ok(build_app(config, data))
)
```

### Conversion Functions

Define reusable conversion functions:

```sigil
@file_to_app_error (e: FileError) -> AppError = match(e,
    FileError.NotFound(p) -> AppError.ConfigError("Not found: " + p),
    FileError.PermissionDenied(p) -> AppError.ConfigError("Permission denied: " + p),
    e -> AppError.ConfigError(str(e))
)

@parse_to_app_error (e: ParseError) -> AppError = AppError.ParseError(e)

// Usage in try
@load_app (path: str) -> Result<App, AppError> = try(
    content = read_file(path) | file_to_app_error,
    config = parse(content) | parse_to_app_error,
    Ok(build(config))
)
```

### Wrapping Errors

A common pattern is wrapping lower-level errors:

```sigil
type HighLevelError =
    | IoError(FileError)
    | ParseError(ParseError)
    | LogicError(str)

// Wrap preserves original error for inspection
@process (path: str) -> Result<Output, HighLevelError> = try(
    content = read_file(path) | e -> HighLevelError.IoError(e),
    data = parse(content) | e -> HighLevelError.ParseError(e),
    Ok(transform(data))
)
```

---

## Converting to Standard Error

Convert domain errors to the standard `Error` type:

```sigil
@to_error (e: FileError) -> Error = Error {
    message: match(e,
        NotFound(p) -> "File not found: " + p,
        PermissionDenied(p) -> "Permission denied: " + p,
        IsDirectory(p) -> "Is a directory: " + p,
        Corrupted(p, d) -> "Corrupted: " + p + " - " + d
    ),
    source: None
}

// Or implement Printable trait
impl Printable for FileError {
    @to_str (self) -> str = match(self,
        NotFound(p) -> "File not found: " + p,
        ...
    )
}
```

---

## Error Hierarchies

For complex applications, organize errors hierarchically:

```sigil
// Low-level errors
type IoError =
    | FileError(FileError)
    | NetworkError(NetworkError)

type DataError =
    | ParseError(ParseError)
    | ValidationError(str)

// High-level application error
type AppError =
    | Io(IoError)
    | Data(DataError)
    | Internal(str)
```

### Flattening Nested Errors

```sigil
@describe_app_error (e: AppError) -> str = match(e,
    Io(IoError.FileError(FileError.NotFound(p))) -> "File not found: " + p,
    Io(IoError.NetworkError(NetworkError.Timeout(ms))) -> "Network timeout after " + str(ms) + "ms",
    Data(DataError.ParseError(pe)) -> "Parse error: " + str(pe),
    Data(DataError.ValidationError(msg)) -> "Validation failed: " + msg,
    Internal(msg) -> "Internal error: " + msg
)
```

---

## Error Context

Add context to errors when propagating:

```sigil
type ContextError = {
    context: str,
    inner: Error
}

@with_context (result: Result<T, Error>, ctx: str) -> Result<T, ContextError> =
    result.map_err(e -> ContextError { context: ctx, inner: e })

// Usage
@process_user (id: str) -> Result<User, ContextError> = try(
    data = fetch_user_data(id).with_context("fetching user " + id),
    validated = validate(data).with_context("validating user " + id),
    Ok(User.from(validated))
)
```

---

## Best Practices

### 1. Be Specific in Libraries, General in Applications

```sigil
// Library: precise errors
@lib_parse (s: str) -> Result<Data, ParseError> = ...

// Application: can use general Error
@app_main () -> Result<void, Error> = ...
```

### 2. Include Actionable Information

```sigil
// Good: includes information needed to fix the problem
type ValidationError =
    | TooLong(field: str, max: int, actual: int)
    | Required(field: str)
    | InvalidFormat(field: str, expected: str, got: str)

// Bad: no actionable information
type ValidationError =
    | Invalid
    | Error
```

### 3. Make Errors Exhaustively Matchable

```sigil
// Good: finite set of known cases
type FileError =
    | NotFound(path: str)
    | PermissionDenied(path: str)
    | IoError(code: int)

// Avoid: catch-all hides new cases
type FileError =
    | NotFound(path: str)
    | PermissionDenied(path: str)
    | Other(str)  // What errors go here?
```

### 4. Document Error Conditions

```sigil
// #Read file contents as string
// @returns Err(NotFound) if path doesn't exist
// @returns Err(PermissionDenied) if file isn't readable
// @returns Err(IsDirectory) if path is a directory
@read_file (path: str) -> Result<str, FileError> = ...
```

### 5. Use Error Types for Control Flow

```sigil
type LookupResult =
    | Found(User)
    | NotFound
    | RateLimited(retry_after: int)

@lookup (id: str) -> LookupResult = ...

@handle_lookup (id: str) -> Response = match(lookup(id),
    Found(user) -> respond_ok(user),
    NotFound -> respond_not_found(),
    RateLimited(seconds) -> respond_retry(seconds)
)
```

### 6. Keep Error Types Close to Their Source

```sigil
// Good: error type defined with the operations that produce it
// in file: database.si

type DatabaseError =
    | ConnectionFailed
    | QueryFailed(query: str, details: str)
    | NotFound(table: str, id: str)

@connect (url: str) -> Result<Connection, DatabaseError> = ...
@query (conn: Connection, q: str) -> Result<[Row], DatabaseError> = ...
```

### 7. Avoid Stringly-Typed Errors

```sigil
// Good: structured error
type ConfigError =
    | MissingField(name: str)
    | InvalidValue(field: str, expected: str, got: str)

// Bad: string error loses structure
@load_config (path: str) -> Result<Config, str>
// Error message: "missing field: timeout"
// Cannot pattern match on error type!
```

---

## Error Type Patterns

### The Infallible Pattern

For operations that logically cannot fail with the given inputs, use the built-in `Never` type:

```sigil
// Never is the built-in bottom type (uninhabited)
// Function that cannot fail
@safe_operation (x: ValidatedInput) -> Result<Output, Never> = Ok(compute(x))
```

### The Retry Pattern

Include retry information in errors:

```sigil
type RetryableError =
    | Retryable(inner: Error, after_ms: int)
    | Fatal(inner: Error)

@should_retry (e: RetryableError) -> bool = match(e,
    Retryable(_, _) -> true,
    Fatal(_) -> false
)
```

### The Aggregate Pattern

For operations that can have multiple errors:

```sigil
type ValidationErrors = {
    errors: [ValidationError]
}

@validate (form: Form) -> Result<ValidForm, ValidationErrors> = run(
    errors = collect_errors(form),
    if is_empty(errors) then Ok(ValidForm.from(form))
    else Err(ValidationErrors { errors: errors })
)
```

---

## See Also

- [Result and Option](01-result-and-option.md) -- Core error types
- [Try Pattern](02-try-pattern.md) -- Error propagation
- [Panics](04-panics.md) -- Unrecoverable errors
- [User-Defined Types](../03-type-system/03-user-defined-types.md) -- Sum type syntax
- [Pattern Matching](../06-pattern-matching/index.md) -- Matching on errors
