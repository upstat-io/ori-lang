# Pattern Matching

This section covers Sigil's pattern matching: the match pattern, destructuring, guards, and exhaustiveness checking.

---

## Documents

| Document | Description |
|----------|-------------|
| [Match Pattern](01-match-pattern.md) | Basic match syntax and usage |
| [Destructuring](02-destructuring.md) | Struct and list destructuring |
| [Guards and Bindings](03-guards-and-bindings.md) | Guards, or patterns, @ binding |
| [Exhaustiveness](04-exhaustiveness.md) | Compiler enforcement of complete matching |
| [Type Narrowing](05-type-narrowing.md) | Flow-sensitive typing after checks |

---

## Overview

Pattern matching combines testing and extraction:

```sigil
type Result<T, E> = Ok(T) | Err(E)

@process (r: Result<int, str>) -> str = match(r,
    Ok(value) -> "got: " + str(value),
    Err(msg) -> "error: " + msg
)
```

### Pattern Types

| Pattern | Example |
|---------|---------|
| Literal | `0 -> "zero"` |
| Variable | `x -> use(x)` |
| Wildcard | `_ -> "default"` |
| Variant | `Some(x) -> x` |
| Struct | `{ name, age } -> ...` |
| List | `[head, ..tail] -> ...` |
| Range | `1..10 -> "small"` |
| Or | `Sat \| Sun -> "weekend"` |
| Guard | `x if x > 0 -> "positive"` |

### Key Features

1. **Exhaustiveness** - Compiler enforces all cases handled
2. **Destructuring** - Bind variables while matching
3. **Guards** - Additional conditions with `if`
4. **Type narrowing** - Compiler tracks narrowed types in branches

---

## See Also

- [Main Index](../00-index.md)
- [Type System](../03-type-system/index.md)
- [Error Handling](../05-error-handling/index.md)
