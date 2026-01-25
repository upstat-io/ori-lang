# Error Handling

This section covers Sigil's approach to errors: Result types, the try pattern, custom error types, and panics.

---

## Documents

| Document | Description |
|----------|-------------|
| [Result and Option](01-result-and-option.md) | Core error types |
| [Try Pattern](02-try-pattern.md) | Error propagation with try |
| [Error Types](03-error-types.md) | User-defined errors, conversion |
| [Panics](04-panics.md) | Unrecoverable errors |

---

## Overview

Sigil uses explicit error types instead of exceptions:

```sigil
@divide (a: int, b: int) -> Result<int, str> =
    if b == 0 then Err("division by zero")
    else Ok(a / b)

@process (x: int, y: int) -> Result<int, str> = try(
    let result = divide(x, y)?,
    let doubled = result * 2,
    Ok(doubled),
)
```

### Error Types

| Type | Purpose |
|------|---------|
| `Option<T>` | Value that may not exist (Some/None) |
| `Result<T, E>` | Operation that may fail (Ok/Err) |

### Key Principles

1. **No exceptions** - Errors are values in the type system
2. **Explicit propagation** - `try` pattern makes error flow visible
3. **Exhaustive handling** - Pattern matching forces all cases handled
4. **Panics for bugs** - Use panic only for programmer errors

### Why Not Exceptions?

Exceptions have hidden control flow:

```python
# Python - which calls can throw?
def process():
    data = fetch()      # throws?
    parsed = parse(data) # throws?
    return transform(parsed) # throws?
```

Result types make errors explicit:

```sigil
@process () -> Result<Data, Error> = try(
    let data = fetch()?,      // returns Result, ? propagates error
    let parsed = parse(data)?, // returns Result, ? propagates error
    Ok(transform(parsed)),
)
```

---

## See Also

- [Main Index](../00-index.md)
- [Type System](../03-type-system/index.md)
- [Pattern Matching](../06-pattern-matching/index.md)
