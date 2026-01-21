# Syntax

This section covers Sigil's syntax for functions, configuration, expressions, and the pattern system.

---

## Documents

| Document | Description |
|----------|-------------|
| [Basic Syntax](01-basic-syntax.md) | Functions, config variables, comments, types |
| [Expressions](02-expressions.md) | Operators, conditionals, line continuation |
| [Patterns Overview](03-patterns-overview.md) | Introduction to Sigil's pattern system |
| [Patterns Reference](04-patterns-reference.md) | Complete pattern documentation |

---

## Quick Reference

### Function Syntax

```sigil
@function_name (param: type, ...) -> return_type = expression
```

### Config Variables

```sigil
$config_name = value
$timeout = 30s
$max_retries = 3
```

### Patterns

```sigil
// All patterns support positional and named syntax
@sum (arr: [int]) -> int = fold(arr, 0, +)

@fibonacci (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true
)
```

### Available Patterns

| Pattern | Purpose |
|---------|---------|
| `recurse` | Recursive functions with memoization |
| `fold` | Reduce/aggregate operations |
| `map` | Transform each element |
| `filter` | Select elements matching predicate |
| `collect` | Build list from range |
| `match` | Pattern matching |
| `run` | Sequential execution |
| `parallel` | Concurrent execution |
| `try` | Error propagation |
| `retry` | Retry with backoff |
| `cache` | Memoization with TTL |
| `validate` | Input validation |
| `timeout` | Time-bounded operations |
| `with` | Resource management |

---

## See Also

- [Main Index](../00-index.md)
- [Type System](../03-type-system/index.md)
