# Panics

This document covers Sigil's panic mechanism: what panics are, when to use them, and how to test panic conditions.

---

## Philosophy

Sigil distinguishes between two kinds of failures:

| Type | Meaning | Handling | Example |
|------|---------|----------|---------|
| Recoverable | Expected failure | `Result<T, E>` | File not found |
| Unrecoverable | Bug in code | `panic` | Index out of bounds |

**Panics indicate bugs**, not expected error conditions. They should never occur in correct programs.

---

## What is a Panic?

A panic is an immediate, unrecoverable termination of the current operation.

### Behavior

1. Execution stops at the panic point
2. A structured error message is produced
3. The program terminates (unless caught in tests)

### Built-in Panic Function

```sigil
@panic (message: str) -> Never
```

The `Never` return type indicates this function never returns normally.

---

## When Panics Occur

### Explicit Panic Calls

```sigil
@get (items: [T], index: int) -> T =
    if index < 0 || index >= len(items) then
        panic("index out of bounds: " + str(index) + " for length " + str(len(items)))
    else
        items[index]
```

### Built-in Operations

Some operations panic on invalid inputs:

| Operation | Panic Condition |
|-----------|-----------------|
| `items[index]` | Index out of bounds |
| `a / b` | Division by zero (for integers) |
| `unwrap(None)` | Unwrapping None |
| `unwrap(Err(e))` | Unwrapping Err |
| `expect(None, msg)` | Expecting Some |

---

## Panic vs Result

### Use Panic When

The error represents a **bug in the calling code**:

```sigil
// Panic: caller should ensure valid index
@get (items: [T], index: int) -> T =
    if index < 0 || index >= len(items) then
        panic("index out of bounds")
    else items[index]

// Panic: impossible state indicates bug
@process_valid (v: ValidatedData) -> Result<ComputedData, Error> =
    if !v.is_valid then panic("ValidatedData must be valid")
    else compute(v)
```

### Use Result When

The error represents an **expected failure condition**:

```sigil
// Result: file might not exist
@read_file (path: str) -> Result<str, FileError>

// Result: input might be malformed
@parse_int (s: str) -> Option<int>

// Result: network might fail
@fetch (url: str) -> Result<Response, NetworkError>
```

### Decision Guide

| Question | Yes | No |
|----------|-----|-----|
| Can caller prevent this? | Panic | Result |
| Is this an expected case? | Result | Panic |
| Should caller handle this? | Result | Panic |
| Is this a programming error? | Panic | Result |

---

## Panic Messages

### Best Practices

Include information that helps identify the bug:

```sigil
// Good: includes relevant values
panic("index out of bounds: got " + str(index) + ", length is " + str(len(items)))

// Good: includes context
panic("invariant violated: balance cannot be negative (got " + str(balance) + ")")

// Bad: no useful information
panic("error")

// Bad: generic message
panic("invalid input")
```

### Structured Panic Output

Panics produce structured output for debugging:

```
PANIC at src/main.si:42:5
  in function: @get
  message: index out of bounds: got 10, length is 5
  stack trace:
    @get (src/main.si:42)
    @process (src/main.si:78)
    @main (src/main.si:95)
```

This structured format helps AI tools identify and fix bugs.

---

## Testing Panic Conditions

Sigil provides `assert_panics` to verify that code panics when it should:

### Basic Usage

```sigil
@test_bounds tests @get () -> void = run(
    assert_panics(get([], 0)),
    assert_panics(get([1, 2, 3], -1)),
    assert_panics(get([1, 2, 3], 10))
)
```

### Syntax

```sigil
assert_panics(expression)
```

The test:
- **Passes** if the expression panics
- **Fails** if the expression completes normally

### Testing Multiple Panic Cases

```sigil
@test_division_panics tests @divide () -> void = run(
    // These should all panic
    assert_panics(divide(1, 0)),
    assert_panics(divide(-5, 0)),
    assert_panics(divide(0, 0))
)
```

### Combining Normal and Panic Tests

```sigil
@test_get tests @get () -> void = run(
    // Normal cases
    assert_eq(get([1, 2, 3], 0), 1),
    assert_eq(get([1, 2, 3], 1), 2),
    assert_eq(get([1, 2, 3], 2), 3),

    // Panic cases
    assert_panics(get([], 0)),
    assert_panics(get([1], 5))
)
```

---

## Panic-Free Alternatives

For operations that might panic, provide safe alternatives:

### Pattern: Checked Operations

```sigil
// May panic
@get (items: [T], index: int) -> T

// Never panics, returns Option
@try_get (items: [T], index: int) -> Option<T> =
    if index >= 0 && index < len(items) then Some(items[index])
    else None

// Never panics, returns default
@get_or (items: [T], index: int, default: T) -> T =
    if index >= 0 && index < len(items) then items[index]
    else default
```

### Pattern: Validated Types

Use types that guarantee validity:

```sigil
type NonEmpty<T> = { items: [T] }  // Invariant: items.len() > 0

@first (ne: NonEmpty<T>) -> T = ne.items[0]  // Cannot panic

@to_non_empty (items: [T]) -> Option<NonEmpty<T>> =
    if len(items) > 0 then Some(NonEmpty { items: items })
    else None
```

### Pattern: Bounds-Checked Index

```sigil
type BoundedIndex = { value: int, max: int }  // Invariant: 0 <= value < max

@bounded (index: int, len: int) -> Option<BoundedIndex> =
    if index >= 0 && index < len then Some(BoundedIndex { value: index, max: len })
    else None

@get_bounded (items: [T], index: BoundedIndex) -> T =
    items[index.value]  // Cannot panic
```

---

## Debugging Panics

### Panic Stack Traces

When a panic occurs, Sigil provides a full stack trace:

```
PANIC at src/processor.si:156:9
  in function: @validate_input
  message: assertion failed: input.length > 0

  stack trace:
    @validate_input (src/processor.si:156)
    @process (src/processor.si:89)
    @handle_request (src/server.si:234)
    @main (src/main.si:12)

  context:
    input = ""
    config.strict = true
```

### Environment Variables

Control panic behavior:

```bash
# Show full stack traces (default in development)
SIGIL_BACKTRACE=1 sigil run program.si

# Minimal output (default in release)
SIGIL_BACKTRACE=0 sigil run program.si
```

---

## Common Panic Patterns

### Assert for Invariants

```sigil
@process (data: Data) -> Result<ProcessedData, Error> = run(
    // Assert invariant at function entry
    if data.items.is_empty() then panic("data.items must not be empty"),

    // Process assuming invariant holds
    result = compute(data),

    // Assert invariant at exit
    if result.count != len(data.items) then
        panic("count mismatch: expected " + str(len(data.items)) + ", got " + str(result.count)),

    Ok(result)
)
```

### Unreachable Code

```sigil
@process (status: Status) -> str = match(status,
    Pending -> "waiting",
    Running -> "active",
    Done -> "complete",
    Failed -> "failed"
)

// If you're certain a variant can't occur:
@process_active (status: Status) -> str = match(status,
    Running -> "active",
    _ -> panic("unreachable: expected Running status")
)
```

### Todo Marker

Mark unimplemented code:

```sigil
@todo (msg: str) -> Never = panic("not yet implemented: " + msg)

@future_feature (x: int) -> str = todo("implement future_feature")
```

---

## Panic Safety

### What Happens on Panic

1. **No resource cleanup** - Panics don't run destructors
2. **No recovery** - Cannot catch panics in normal code
3. **Process termination** - Program exits with error code

### Implications

```sigil
// Bad: resource may leak on panic
@process () -> void = run(
    handle = open_resource(),
    do_work(handle),      // If this panics, handle leaks
    close_resource(handle)
)

// Better: use with pattern for resource safety
@process () -> void = with(
    .acquire: open_resource(),
    .use: handle -> do_work(handle),
    .release: handle -> close_resource(handle)
)
```

---

## Best Practices

### 1. Panic Only for Bugs

```sigil
// Good: panic for programming errors
if index < 0 then panic("index must be non-negative")

// Bad: panic for user input
if user_input.is_empty() then panic("input required")  // Use Result instead
```

### 2. Provide Helpful Messages

```sigil
// Good: message helps identify the bug
panic("array index " + str(i) + " out of bounds for length " + str(len(arr)))

// Bad: message doesn't help
panic("invalid")
```

### 3. Test Panic Conditions

```sigil
// Every panic should have a corresponding test
@test_invariants tests @process () -> void = run(
    assert_panics(process(empty_data)),
    assert_panics(process(invalid_data))
)
```

### 4. Document Panic Conditions

```sigil
// #Get element at index
// @panics if index < 0 or index >= len(items)
@get (items: [T], index: int) -> T = ...
```

### 5. Consider Safe Alternatives

```sigil
// Provide both panicking and non-panicking versions
@get (items: [T], index: int) -> T           // Panics on bad index
@try_get (items: [T], index: int) -> Option<T>  // Returns None on bad index
@get_or (items: [T], index: int, default: T) -> T  // Returns default on bad index
```

### 6. Never Panic in Library Code on User Input

```sigil
// Library code should return Result for user-facing operations
@lib_parse (s: str) -> Result<Data, ParseError>  // Good
@lib_parse (s: str) -> Data  // Bad if it can panic on invalid input
```

---

## Summary

| Aspect | Guidance |
|--------|----------|
| When to panic | Programming errors, violated invariants |
| When not to panic | User input, expected failures |
| Message content | Include values, context, what went wrong |
| Testing | Use `assert_panics` for all panic conditions |
| Alternatives | Provide safe versions (`try_get`, `get_or`) |
| Documentation | Document all panic conditions in function docs |

---

## See Also

- [Result and Option](01-result-and-option.md) -- For recoverable errors
- [Try Pattern](02-try-pattern.md) -- Error propagation
- [Error Types](03-error-types.md) -- User-defined errors
- [Testing](../11-testing/index.md) -- Test syntax and assertions
- [Mandatory Tests](../11-testing/01-mandatory-tests.md) -- Test requirements
