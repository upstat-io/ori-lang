# Patterns Overview

This document introduces Sigil's pattern system—declarative constructs that replace common boilerplate with guaranteed-correct implementations.

---

## What Are Patterns?

Patterns are built-in language constructs that capture common computational patterns. Instead of writing imperative code, you declare what you want and let the language handle how.

### Traditional Approach

```python
# Python - AI must implement memoization correctly
def fib(n, memo={}):
    if n in memo: return memo[n]
    if n <= 1: return n
    memo[n] = fib(n-1, memo) + fib(n-2, memo)
    return memo[n]
```

### Sigil Approach

```sigil
@fibonacci (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true
)
```

The AI declares WHAT (fibonacci with memoization), and the language handles HOW.

---

## Pattern Categories

### Data Transformation

| Pattern | Purpose |
|---------|---------|
| `map` | Transform each element |
| `filter` | Select elements matching predicate |
| `fold` | Reduce/aggregate to single value |
| `collect` | Build list from range |

### Control Flow

| Pattern | Purpose |
|---------|---------|
| `recurse` | Recursive functions with memoization |
| `match` | Pattern matching/dispatch |
| `run` | Sequential execution |
| `try` | Error propagation |

### Concurrency

| Pattern | Purpose |
|---------|---------|
| `parallel` | Concurrent execution |
| `timeout` | Time-bounded operations |

### Resilience

| Pattern | Purpose |
|---------|---------|
| `retry` | Retry with backoff |
| `cache` | Memoization with TTL |
| `validate` | Input validation |
| `with` | Resource management |

---

## Pattern Syntax

Patterns use **named properties exclusively**. This ensures:
- Self-documenting code
- Clear distinction from function calls
- Consistent structure across all patterns
- No positional argument ambiguity

### Syntax

```sigil
pattern_name(
    .property: value,
    .property: value
)
```

The leading dot (`.property:`) distinguishes pattern arguments from regular function calls and struct fields.

### Example

```sigil
@fibonacci (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true
)

@sum (arr: [int]) -> int = fold(
    .over: arr,
    .init: 0,
    .op: +
)

@doubled (arr: [int]) -> [int] = map(
    .over: arr,
    .transform: x -> x * 2
)
```

---

## Core Patterns

### `recurse` — Recursive Functions

```sigil
@factorial (n: int) -> int = recurse(
    .cond: n <= 1,    // base case condition
    .base: 1,         // value when condition true
    .step: n * self(n - 1)  // recursive step
)

// With memoization
@fib (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true
)

// With parallelism
@fib_parallel (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .parallel: 20  // parallelize when n > 20
)
```

### `fold` — Reduce/Aggregate

```sigil
@sum (arr: [int]) -> int = fold(
    .over: arr,
    .init: 0,
    .op: +
)

@product (arr: [int]) -> int = fold(
    .over: arr,
    .init: 1,
    .op: *
)
```

### `map` — Transform

```sigil
@double_all (arr: [int]) -> [int] = map(
    .over: arr,
    .transform: x -> x * 2
)
```

### `filter` — Select

```sigil
@evens (arr: [int]) -> [int] = filter(
    .over: arr,
    .predicate: x -> x % 2 == 0
)
```

### `match` — Pattern Matching

```sigil
@describe (status: Status) -> str = match(status,
    Pending -> "waiting",
    Running -> "active",
    Done -> "complete"
)
```

### `run` — Sequential Execution

```sigil
@process (items: [int]) -> int = run(
    let doubled = map(items, x -> x * 2),
    let filtered = filter(doubled, x -> x > 10),
    fold(filtered, 0, +),
)
```

### `try` — Error Propagation

```sigil
@process (path: str) -> Result<Data, Error> = try(
    let content = read_file(path)?,
    let parsed = parse(content)?,
    Ok(transform(parsed)),
)
```

### `parallel` — Concurrent Execution

```sigil
@fetch_dashboard (id: str) -> Dashboard = run(
    let data = parallel(
        .user: get_user(id),
        .posts: get_posts(id),
        .notifications: get_notifs(id),
    ),
    Dashboard { user: data.user, posts: data.posts, ... },
)
```

---

## Why Patterns?

### 1. Correctness by Construction

Patterns guarantee correct implementation of common operations:

| Pattern | What You Avoid |
|---------|---------------|
| `recurse` | Stack overflow, wrong base case |
| `fold` | Off-by-one, wrong initial value |
| `retry` | Wrong backoff, missing jitter |
| `parallel` | Race conditions, deadlocks |

### 2. Declarative Intent

AI declares what, not how:

```sigil
// Intent: "retry 3 times with exponential backoff"
retry(
    .op: fetch(),
    .attempts: 3,
    .backoff: exponential(...),
)

// Language handles: timing, jitter, error handling
```

### 3. Consistent Structure

All patterns follow the same structure:

```sigil
pattern_name(
    .required_property: value,
    .optional_property: value
)
```

### 4. Semantic Addressing

Pattern properties are addressable:

```
@fibonacci.memo       // access memoization flag
@fetch.attempts      // access retry count
@fetch.backoff.base  // access backoff base time
```

---

## Pattern Properties Summary

| Pattern | Required | Optional |
|---------|----------|----------|
| `recurse` | `.cond`, `.base`, `.step` | `.memo`, `.parallel` |
| `fold` | `.over`, `.init`, `.op` | — |
| `map` | `.over`, `.transform` | — |
| `filter` | `.over`, `.predicate` | — |
| `collect` | `.range`, `.transform` | — |
| `parallel` | Named expr pairs or `.tasks` | `.timeout`, `.on_error`, `.max_concurrent` |
| `retry` | `.op`, `.attempts` | `.backoff`, `.on`, `.jitter` |
| `cache` | `.key`, `.op` | `.ttl` |
| `validate` | `.rules`, `.then` | — |
| `timeout` | `.op`, `.after`, `.on_timeout` | — |

---

## See Also

- [Patterns Reference](04-patterns-reference.md) — Complete pattern documentation
- [Basic Syntax](01-basic-syntax.md)
- [Error Handling](../05-error-handling/index.md)
