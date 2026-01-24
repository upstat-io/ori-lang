# Result and Option

This document covers Sigil's core error handling types: `Result<T, E>` and `Option<T>`. These types replace exceptions with explicit, type-safe error handling.

---

## Philosophy

Sigil rejects exceptions in favor of explicit error types:

| Approach | Control Flow | Type Signature | AI Compatibility |
|----------|--------------|----------------|------------------|
| Exceptions | Hidden | Lies (doesn't show errors) | Poor |
| Error codes | Explicit | Partial (easy to ignore) | Medium |
| Result types | Explicit | Complete (errors in types) | Excellent |

Result types make errors **visible in function signatures** and **impossible to ignore** without explicit handling.

---

## `Option<T>` -- Maybe a Value

`Option<T>` represents a value that may or may not exist.

### Definition

```sigil
type Option<T> = Some(T) | None
```

### When to Use

Use `Option<T>` when:
- A value might not exist (lookup, search)
- A function has no meaningful result for some inputs
- A field is optional

```sigil
@find_user (id: str) -> Option<User>
@first (items: [T]) -> Option<T>
@parse_int (text: str) -> Option<int>
```

### Creating Option Values

```sigil
// Some value exists
found = Some(42)
user = Some(User { name: "Alice", id: 1 })

// No value
not_found: Option<int> = None
missing: Option<User> = None

// Type inference from context
@find (items: [int], target: int) -> Option<int> = run(
    let index = find_index(
        .items: items,
        .target: target,
    ),
    if index >= 0 then Some(items[index])
    else None,
)
```

### Checking Options

```sigil
// Boolean checks
// true
found.is_some()
// false
found.is_none()
// false
missing.is_some()
// true
missing.is_none()

// In conditions
if result.is_some() then "found" else "not found"
```

### Extracting Values

#### Pattern Matching (Preferred)

Always handle both cases explicitly:

```sigil
@describe (opt: Option<int>) -> str = match(opt,
    Some(value) -> "value is " + str(.value: value),
    None -> "no value",
)

@double_or_zero (opt: Option<int>) -> int = match(opt,
    Some(value) -> value * 2,
    None -> 0,
)
```

#### Default Value Operator `??`

Provide a fallback for `None` (also works on `Result` for `Err`):

```sigil
// Option: fallback on None
value = some_option ?? 0
name = find_name(id) ?? "unknown"

// Result: fallback on Err
config = load_config() ?? default_config

// Equivalent to:
value = match(some_option,
    Some(inner) -> inner,
    None -> 0
)
```

#### Unwrap (Use Sparingly)

`.unwrap()` extracts the value but **panics if None**:

```sigil
// 42
value = Some(42).unwrap()
// PANIC!
value = None.unwrap()
```

Use only when you are certain the value exists:

```sigil
// OK: we just checked
if opt.is_some() then run(
    let value = opt.unwrap(),
    use(.value: value),
)

// Better: use pattern matching instead
match(opt,
    Some(value) -> use(.value: value),
    None -> handle_missing(),
)
```

### Common Operations

#### `map` -- Transform the Value

Apply a function to `Some`, pass through `None`:

```sigil
// Some(10)
Some(5).map(.transform: value -> value * 2)
// None
None.map(.transform: value -> value * 2)

// Type: Option<T>.map(.transform: T -> U) -> Option<U>
```

#### `and_then` -- Chain Operations

Flatten nested options:

```sigil
@lookup_then_process (id: str) -> Option<Result<ProcessedUser, Error>> = run(
    let user = find_user(.id: id),
    // Option<Result<...>>, not Option<Option<...>>
    user.and_then(.f: found_user -> process(.user: found_user)),
)

// Type: Option<T>.and_then(.f: T -> Option<U>) -> Option<U>
```

#### `or` -- Alternative Value

Return an alternative if `None`:

```sigil
// primary if Some, otherwise fallback
primary.or(.fallback: fallback)

find_user(.id: id).or(.fallback: default_user)
```

#### `ok_or` -- Convert to Result

Transform `Option<T>` to `Result<T, E>`:

```sigil
@require_user (id: str) -> Result<User, str> =
    find_user(.id: id).ok_or(.error: "user not found: " + id)

// Some(value) -> Ok(value)
// None -> Err(error_value)
```

#### `filter` -- Conditional Some

Keep `Some` only if predicate passes:

```sigil
// Some(5)
Some(5).filter(.predicate: value -> value > 3)
// None
Some(2).filter(.predicate: value -> value > 3)
// None
None.filter(.predicate: value -> value > 3)
```

---

## `Result<T, E>` -- Success or Failure

`Result<T, E>` represents an operation that can succeed with value `T` or fail with error `E`.

### Definition

```sigil
type Result<T, E> = Ok(T) | Err(E)
```

### When to Use

Use `Result<T, E>` when:
- An operation can fail in expected ways
- You want to propagate errors to callers
- Error handling is required, not optional

```sigil
@read_file (path: str) -> Result<str, FileError>
@parse_json (input: str) -> Result<Json, ParseError>
@connect (url: str) -> Result<Connection, NetworkError>
```

### Creating Result Values

```sigil
// Success
success = Ok(42)
data = Ok(parse_result)

// Failure
failure: Result<int, str> = Err("something went wrong")
file_error = Err(FileError.NotFound(path: path))

// In functions
@divide (
    .numerator: numerator,
    .denominator: denominator,
) -> Result<int, str> =
    if denominator == 0 then Err("division by zero")
    else Ok(numerator / denominator)
```

### Checking Results

```sigil
// Boolean checks
// true
is_ok(.result: success)
// false
is_err(.result: success)
// false
is_ok(.result: failure)
// true
is_err(.result: failure)

// In conditions
if is_ok(.result: result) then "success" else "failure"
```

### Extracting Values

#### Pattern Matching (Preferred)

Always handle both cases:

```sigil
@describe (result: Result<int, str>) -> str = match(result,
    Ok(value) -> "success: " + str(.value: value),
    Err(error) -> "error: " + error,
)

@process_result (result: Result<Data, Error>) -> Output = match(result,
    Ok(data) -> transform(.data: data),
    Err(error) -> run(
        log_error(.error: error),
        default_output(),
    ),
)
```

#### Unwrap (Use Sparingly)

```sigil
// 42
value = Ok(42).unwrap()
// PANIC!
value = Err("oops").unwrap()

// "oops"
error = Err("oops").unwrap_err()
// PANIC!
error = Ok(42).unwrap_err()
```

#### Expect (Better Panic Messages)

```sigil
// Panics with custom message if Err
value = result.expect(
    .msg: "config file must exist",
)
```

### Common Operations

#### `map` -- Transform Success

```sigil
// Ok(10)
Ok(5).map(.transform: value -> value * 2)
// Err("error")
Err("error").map(.transform: value -> value * 2)

// Type: Result<T, E>.map(.transform: T -> U) -> Result<U, E>
```

#### `map_err` -- Transform Error

```sigil
// Ok(5)
Ok(5).map_err(.transform: error -> "Error: " + error)
// Err("Error: original")
Err("original").map_err(.transform: error -> "Error: " + error)

// Type: Result<T, E>.map_err(.transform: E -> F) -> Result<T, F>
```

#### `and_then` -- Chain Operations

```sigil
@parse_and_validate (input: str) -> Result<int, str> =
    parse_int(.s: input).and_then(.f: value ->
        if value > 0 then Ok(value)
        else Err("must be positive")
    )

// Type: Result<T, E>.and_then(.f: T -> Result<U, E>) -> Result<U, E>
```

#### `or_else` -- Error Recovery

```sigil
primary_source().or_else(.f: error -> fallback_source())

// Try primary, if it fails, try fallback
```

#### `ok` -- Convert to Option

```sigil
// Some(42)
Ok(42).ok()
// None
Err("error").ok()

// Discards error information
```

#### `err` -- Extract Error as Option

```sigil
// None
Ok(42).err()
// Some("error")
Err("error").err()
```

---

## Built-In Status

Both `Option<T>` and `Result<T, E>` are built-in types:

- **No import required** -- Always available
- **Special compiler support** -- Exhaustiveness checking in `match`
- **Pattern integration** -- Work seamlessly with `try` pattern

```sigil
// Just works, no imports needed
@find (items: [int], target: int) -> Option<int> = ...
@read (path: str) -> Result<str, Error> = ...
```

---

## Option vs Result

| Situation | Use |
|-----------|-----|
| Value might not exist | `Option<T>` |
| Operation can fail | `Result<T, E>` |
| Lookup/search | `Option<T>` |
| I/O, parsing, validation | `Result<T, E>` |
| No error information needed | `Option<T>` |
| Need to know why it failed | `Result<T, E>` |

### Converting Between Them

```sigil
// Option to Result
opt.ok_or(.error: default_error)
opt.ok_or_else(.f: compute_error)

// Result to Option
// Discards error
result.ok()
// Discards success
result.err()
```

---

## Combining Multiple Options/Results

### All Must Succeed

Use the `try` pattern (see [Try Pattern](02-try-pattern.md)):

```sigil
@process (first: str, second: str) -> Result<int, Error> = try(
    let left = parse_int(.s: first),
    let right = parse_int(.s: second),
    Ok(left + right),
)
```

### Collect Results

```sigil
// Parse all strings, fail if any fails
@parse_all (strings: [str]) -> Result<[int], Error> =
    strings.traverse(.f: item -> parse_int(.s: item))

// Parse all strings, keep only successes
@parse_valid (strings: [str]) -> [int] =
    strings.filter_map(.f: item -> parse_int(.s: item).ok())
```

---

## Best Practices

### 1. Prefer Pattern Matching

```sigil
// Good: explicit handling
match(result,
    Ok(value) -> use(.value: value),
    Err(error) -> handle(.error: error),
)

// Avoid: unwrap hides potential panics
use(.value: result.unwrap())
```

### 2. Use Type-Specific Errors

```sigil
// Good: precise error type
@read_config (path: str) -> Result<Config, ConfigError>

// Acceptable for simple cases
@parse_int (text: str) -> Option<int>

// Avoid: stringly-typed errors in libraries
@read_config (path: str) -> Result<Config, str>
```

### 3. Document None/Err Conditions

```sigil
// #Find user by ID
// @returns None if user does not exist
@find_user (id: str) -> Option<User> = ...

// #Read configuration file
// @returns Err(NotFound) if file doesn't exist
// @returns Err(ParseError) if file is malformed
@read_config (path: str) -> Result<Config, ConfigError> = ...
```

### 4. Use ?? for Simple Defaults

```sigil
// Good: concise default handling
name = find_name(.id: id) ?? "anonymous"
count = parse_int(.s: input) ?? 0

// Overkill for simple defaults
let name = match(find_name(.id: id),
    Some(found) -> found,
    None -> "anonymous",
)
```

### 5. Chain Operations with map/and_then

```sigil
// Good: fluent style
result = find_user(.id: id)
    .and_then(.f: user -> load_profile(.id: user.id))
    .map(.transform: profile -> profile.display_name)
    .ok()
    ?? "unknown"

// Verbose alternative
let result = match(find_user(.id: id),
    Some(user) -> match(load_profile(.id: user.id),
        Some(profile) -> profile.display_name,
        None -> "unknown",
    ),
    None -> "unknown",
)
```

---

## See Also

- [Try Pattern](02-try-pattern.md) -- Error propagation with try
- [Error Types](03-error-types.md) -- User-defined error types
- [Panics](04-panics.md) -- Unrecoverable errors
- [Pattern Matching](../06-pattern-matching/index.md) -- match pattern details
- [Compound Types](../03-type-system/02-compound-types.md) -- Type system context
