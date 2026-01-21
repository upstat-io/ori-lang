# The Try Pattern

This document covers Sigil's `try` pattern for error propagation. The `try` pattern provides explicit, scoped error handling that makes error flow visible and predictable.

---

## Philosophy

Many languages use a single-character operator (`?` in Rust, `?` in Swift) for error propagation. Sigil takes a different approach:

| Approach | Visibility | Scope | AI Compatibility |
|----------|------------|-------|------------------|
| Exceptions | Hidden | Implicit | Poor |
| `?` operator | Low (single char) | Per-expression | Medium |
| `try` pattern | High (explicit block) | Explicit block | Excellent |

The `try` pattern makes error propagation **visible** and **scoped**, following Sigil's principle of explicit control flow.

---

## Basic Syntax

The `try` pattern evaluates a sequence of expressions, propagating any `Err` values:

```sigil
@function_name (params) -> Result<T, E> = try(
    let binding1 = expr1,
    let binding2 = expr2,
    ...
    final_expression,
)
```

### Behavior

1. Evaluate expressions in sequence
2. If any expression returns `Err(e)`, immediately return `Err(e)`
3. If all succeed, return the final expression

### Example

```sigil
@process (path: str) -> Result<Data, Error> = try(
    let content = read_file(path),     // If Err, return immediately
    let parsed = parse(content),       // If Err, return immediately
    let validated = validate(parsed),  // If Err, return immediately
    Ok(transform(validated)),          // Final result if all succeed
)
```

---

## How Try Works

### Expression Types in Try

Each expression in a `try` block can be:

#### 1. Result-Returning Expression with Binding

```sigil
try(
    let content = read_file(path),  // read_file returns Result<str, Error>
    ...                             // content has type str (unwrapped)
)
```

The binding receives the **unwrapped Ok value**. If the expression returns `Err`, the entire `try` block returns that `Err`.

#### 2. Plain Expression (No Propagation)

```sigil
try(
    let content = read_file(path),
    let length = len(content),      // len doesn't return Result, no propagation
    ...
)
```

Expressions that don't return `Result` are evaluated normally.

#### 3. Final Expression

```sigil
try(
    let x = get_x(),
    let y = get_y(),
    Ok(x + y),  // Final expression determines return type
)
```

The final expression must match the function's return type.

### Type Flow

```sigil
@example (path: str) -> Result<int, FileError> = try(
    // content: str (unwrapped from Result<str, FileError>)
    let content = read_file(path),

    // lines: [str] (plain value, no unwrapping)
    let lines = split(content, "\n"),

    // count: int (unwrapped from Result<int, FileError>)
    let count = parse_int(lines[0]),

    // Return type: Result<int, FileError>
    Ok(count * 2),
)
```

---

## Error Conversion

When combining operations that return different error types, you must convert errors explicitly.

### The `| e -> ...` Syntax

Use the pipe operator to convert errors inline:

```sigil
@load (path: str) -> Result<Data, AppError> = try(
    // read_file returns Result<str, FileError>
    // Convert FileError to AppError
    let content = read_file(path) | e -> AppError.Io(e),

    // parse returns Result<Data, ParseError>
    // Convert ParseError to AppError
    let data = parse(content) | e -> AppError.Parse(e),

    Ok(data),
)
```

### Conversion Syntax

```sigil
expression | e -> converted_error
```

This syntax:
1. Evaluates `expression`
2. If `Ok(value)`, returns `value` (unwrapped)
3. If `Err(e)`, returns `Err(converted_error)`

### Example: Multiple Error Types

```sigil
type AppError =
    | FileError(FileError)
    | ParseError(ParseError)
    | ValidationError(str)

@process_file (path: str) -> Result<Output, AppError> = try(
    // Convert each error type
    let content = read_file(path) | e -> AppError.FileError(e),
    let config = parse_json(content) | e -> AppError.ParseError(e),

    // Inline validation with custom error
    if !is_valid(config) then
        return Err(AppError.ValidationError("invalid config")),

    Ok(build_output(config)),
)
```

---

## Comparison with Other Approaches

### vs. Exception-Based Languages

**Python with exceptions:**
```python
def process(path):
    content = read_file(path)      # Can throw - invisible
    parsed = parse(content)        # Can throw - invisible
    return transform(parsed)       # Can throw - invisible
```

**Sigil with try:**
```sigil
@process (path: str) -> Result<Data, Error> = try(
    let content = read_file(path),     // Error handling visible
    let parsed = parse(content),       // Error handling visible
    Ok(transform(parsed)),
)
```

In Sigil, the `try` block explicitly shows where errors are handled.

### vs. `?` Operator (Rust)

**Rust with `?`:**
```rust
fn process(path: &str) -> Result<Data, Error> {
    let content = read_file(path)?;  // Easy to miss the ?
    let parsed = parse(&content)?;   // Must remember on each line
    Ok(transform(parsed))
}
```

**Sigil with try:**
```sigil
@process (path: str) -> Result<Data, Error> = try(
    let content = read_file(path),
    let parsed = parse(content),
    Ok(transform(parsed)),
)
```

| Aspect | `?` Operator | `try` Pattern |
|--------|--------------|---------------|
| Visibility | Easy to miss single char | Explicit block |
| Scope | Per-expression | Entire block |
| Forgetting | Compile error per site | One pattern covers all |
| Error conversion | `.map_err()?` | `\| e -> ...` |

### vs. Do-Notation (Haskell)

Sigil's `try` is similar to Haskell's do-notation for the `Either` monad, but with explicit syntax and error conversion.

---

## Advanced Patterns

### Early Return

Return early from within a `try` block:

```sigil
@process (data: Data) -> Result<Output, Error> = try(
    let validated = validate(data),

    // Early return on condition
    if validated.is_empty then return Ok(Output.empty()),

    let transformed = transform(validated),
    Ok(transformed),
)
```

### Conditional Error Propagation

Only propagate errors under certain conditions:

```sigil
@flexible_read (path: str, required: bool) -> Result<str, Error> = try(
    let result = read_file(path),

    let content = if required then
        result | e -> e  // Propagate error
    else
        result ?? "",    // Use default

    Ok(process(content)),
)
```

### Nested Try Blocks

Use nested `try` for sub-operations with different error handling:

```sigil
@outer (path: str) -> Result<int, OuterError> = try(
    // Inner operation with its own error handling
    let data = try(
        let raw = read(path),
        let parsed = parse(raw),
        Ok(validate(parsed)),
    ) | e -> OuterError.from_inner(e),

    Ok(compute(data)),
)
```

### Combining Try with Match

Handle specific errors differently:

```sigil
@resilient_read (path: str) -> Result<str, Error> = try(
    let result = read_file(path),

    let content = match(result,
        Ok(c) -> c,
        Err(FileError.NotFound) -> "",  // Default for missing
        Err(e) -> return Err(Error.from(e)),  // Propagate others
    ),

    Ok(content),
)
```

---

## Try vs Run

Both `try` and `run` execute sequences of expressions. The difference is error handling:

| Pattern | Purpose | On Error |
|---------|---------|----------|
| `run` | Sequential execution | Does not propagate |
| `try` | Error propagation | Returns immediately |

### Use Run for Non-Failable Operations

```sigil
@process (items: [int]) -> int = run(
    let doubled = map(items, x -> x * 2),
    let filtered = filter(doubled, x -> x > 10),
    fold(filtered, 0, +),
)
```

### Use Try for Failable Operations

```sigil
@process (path: str) -> Result<int, Error> = try(
    let content = read_file(path),
    let items = parse_items(content),
    Ok(sum(items)),
)
```

### Mixing Run and Try

```sigil
@process (path: str) -> Result<Summary, Error> = try(
    let data = load_data(path),

    // Non-failable processing in run
    let summary = run(
        let filtered = filter(data, is_valid),
        let grouped = group_by(filtered, category),
        Summary.from(grouped),
    ),

    Ok(summary),
)
```

---

## Error Accumulation

By default, `try` returns on the first error. For collecting multiple errors, use different approaches:

### Validate All

```sigil
@validate_all (items: [Item]) -> Result<[Item], [Error]> = run(
    let errors = filter_map(items, i -> validate(i).err()),
    if is_empty(errors) then Ok(items)
    else Err(errors),
)
```

### Partition Results

```sigil
@process_all (items: [str]) -> { successes: [int], failures: [Error] } = run(
    let results = map(items, parse_int),
    {
        successes: filter_map(results, r -> r.ok()),
        failures: filter_map(results, r -> r.err()),
    },
)
```

---

## Best Practices

### 1. Use Try for Chained Failable Operations

```sigil
// Good: clear error flow
@process (input: str) -> Result<Output, Error> = try(
    let step1 = do_step1(input)?,
    let step2 = do_step2(step1)?,
    let step3 = do_step3(step2)?,
    Ok(step3),
)

// Avoid: deeply nested matches
@process (input: str) -> Result<Output, Error> = match(do_step1(input),
    Ok(step1) -> match(do_step2(step1),
        Ok(step2) -> match(do_step3(step2),
            Ok(step3) -> Ok(step3),
            Err(e) -> Err(e)
        ),
        Err(e) -> Err(e)
    ),
    Err(e) -> Err(e)
)
```

### 2. Convert Errors Explicitly

```sigil
// Good: explicit conversion
content = read_file(path) | e -> AppError.Io(e)

// Avoid: relying on implicit conversion (Sigil doesn't have this)
content = read_file(path)  // Type error if errors don't match
```

### 3. Keep Try Blocks Focused

```sigil
// Good: focused try block
@load_config (path: str) -> Result<Config, Error> = try(
    let content = read_file(path),
    let config = parse_config(content),
    Ok(config),
)

// Avoid: mixing failable and non-failable operations unnecessarily
@load_config (path: str) -> Result<Config, Error> = try(
    let content = read_file(path),
    let lines = split(content, "\n"),     // Not failable
    let header = lines[0],                 // Not failable
    let config = parse_config(content),
    Ok(config),
)
```

### 4. Document Error Conditions

```sigil
// #Load and validate configuration
// @returns Err(NotFound) if file doesn't exist
// @returns Err(ParseError) if file format is invalid
// @returns Err(ValidationError) if config values are invalid
@load_config (path: str) -> Result<Config, ConfigError> = try(
    let content = read_file(path) | _ -> ConfigError.NotFound(path),
    let config = parse(content) | e -> ConfigError.ParseError(e),
    validate(config) | e -> ConfigError.ValidationError(e),
    Ok(config),
)
```

### 5. Prefer Try Over Manual Match Chains

The `try` pattern exists specifically to avoid the "pyramid of doom" from nested matches:

```sigil
// Pyramid of doom - avoid
match(a(),
    Ok(x) -> match(b(x),
        Ok(y) -> match(c(y),
            Ok(z) -> Ok(z),
            Err(e) -> Err(e)),
        Err(e) -> Err(e)),
    Err(e) -> Err(e))

// Clean try block - prefer
try(
    let x = a(),
    let y = b(x),
    let z = c(y),
    Ok(z),
)
```

---

## See Also

- [Result and Option](01-result-and-option.md) -- Core error types
- [Error Types](03-error-types.md) -- User-defined error types
- [Panics](04-panics.md) -- Unrecoverable errors
- [Patterns Overview](../02-syntax/03-patterns-overview.md) -- Pattern system context
- [Expressions](../02-syntax/02-expressions.md) -- Expression evaluation
