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
@fibonacci (term: int) -> int = recurse(
    .condition: term <= 1,
    .base: term,
    .step: self(term - 1) + self(term - 2),
    .memo: true,
)
```

The AI declares WHAT (fibonacci with memoization), and the language handles HOW.

---

## Pattern Categories

Patterns are distinct from function calls. They fall into three categories based on their internal structure:

### function_seq — Sequential Expressions

A **function_seq** contains a sequence of expressions evaluated in order. These are control flow constructs where order is the meaning.

| Pattern | Purpose |
|---------|---------|
| `run` | Sequential execution with bindings |
| `try` | Sequential execution with error propagation |
| `match` | Pattern matching with ordered arms |

```sigil
// function_seq: expressions flow in sequence
run(
    let first = step1(),
    let second = step2(
        .input: first,
    ),
    first + second,
)
```

### function_exp — Named Expressions

A **function_exp** contains named expressions (`.name: expr`). These are configuration-based constructs where names provide meaning. Both patterns and built-in functions are function_exp constructs.

**Patterns:**

| Pattern | Purpose |
|---------|---------|
| `map` | Transform each element |
| `filter` | Select elements matching predicate |
| `fold` | Reduce/aggregate to single value |
| `collect` | Build list from range |
| `find` | Find first matching element |
| `recurse` | Recursive functions with memoization |
| `parallel` | Concurrent execution |
| `timeout` | Time-bounded operations |
| `retry` | Retry with backoff |
| `cache` | Memoization with TTL |
| `validate` | Input validation |
| `with` | Resource management |

**Core Functions (function_exp):**

| Function | Purpose |
|----------|---------|
| `len`, `is_empty` | Collection inspection |
| `is_some`, `is_none` | Option inspection |
| `is_ok`, `is_err` | Result inspection |
| `assert`, `assert_eq`, `assert_ne` | Assertion |
| `assert_some`, `assert_none`, `assert_ok`, `assert_err` | Type-specific assertion |
| `assert_panics`, `assert_panics_with` | Panic assertion |
| `print` | Output |
| `compare`, `min`, `max` | Comparison |
| `panic` | Termination |

### function_val — Type Conversion Functions

A **function_val** is a type conversion function that allows positional argument syntax for brevity. This is the only category where positional syntax is permitted.

| Function | Purpose |
|----------|---------|
| `int`, `float`, `str`, `byte` | Type conversion |

```sigil
// function_exp pattern: named expressions, each on its own line
fold(
    .over: items,
    .initial: 0,
    .operation: +,
)

// function_exp core function: same named expression syntax
assert_eq(
    .actual: result,
    .expected: 42,
)

// function_val: positional syntax allowed
int(3.14)
str(42)
```

### Why Three Categories?

The distinction reflects fundamentally different semantics:

| Category | Contents | Order | Argument Style |
|----------|----------|-------|----------------|
| **function_seq** | Sequence of expressions | Matters (serial evaluation) | Positional |
| **function_exp** | Named expressions | Doesn't matter | Named (`.name:`) |
| **function_val** | Type conversion | N/A (single argument) | Positional |

This is not about "positional vs named arguments." These are different constructs:
- function_seq doesn't have "parameters" — it has a sequence
- function_exp doesn't have "parameters" — it has named expressions
- function_val is a special case for brevity in type conversions

---

## Pattern Syntax

Patterns use **named properties exclusively**, with each argument on its own line. This is a deliberate design choice with significant benefits for both AI and human readers.

### Syntax

```sigil
pattern_name(
    .property: value,
    .property: value,
)
```

The leading dot (`.property:`) distinguishes pattern arguments from regular function calls and struct fields.

### Why Named-Only, One Per Line?

#### For AI-Assisted Development

1. **Line-oriented edits** — AI can add or remove a single line without modifying a range. No risk of breaking syntax by miscounting commas or parentheses in a dense inline expression.

2. **Self-documenting** — AI doesn't need to trace callers to discover parameter order or meaning. The property name (`.over:`, `.transform:`, `.predicate:`) immediately conveys intent.

3. **Reduced context usage** — While more verbose in tokens, the structured format reduces the context needed to understand code. AI can scan property names without parsing complex nested expressions.

#### For Humans

1. **Whitespace aids comprehension** — Research shows whitespace significantly improves human understanding. Each argument gets visual separation.

2. **Narrow column, fast scanning** — Vertical layout creates a narrow column that humans scan substantially faster than wide horizontal code.

3. **Zero ambiguity** — No question about argument order or meaning. `.predicate:` is obviously the filter condition; `.initial:` is obviously the initial value.

4. **Self-documenting** — Code explains itself without requiring jumps to function signatures or documentation.

#### The Tradeoff

Yes, this is more verbose:

```
// Compact but ambiguous (not valid Sigil)
fold(items, 0, +)
```

```sigil
// Verbose but clear (valid Sigil)
fold(
    .over: items,
    .initial: 0,
    .operation: +,
)
```

The verbosity cost is offset by:
- Faster reading and understanding
- Fewer bugs from argument order mistakes
- Easier code review and maintenance
- Better AI-assisted editing

### Example

```sigil
@fibonacci (term: int) -> int = recurse(
    .condition: term <= 1,
    .base: term,
    .step: self(term - 1) + self(term - 2),
    .memo: true,
)

@sum (numbers: [int]) -> int = fold(
    .over: numbers,
    .initial: 0,
    .operation: +,
)

@doubled (numbers: [int]) -> [int] = map(
    .over: numbers,
    .transform: number -> number * 2,
)
```

---

## Core Patterns

### `recurse` — Recursive Functions

```sigil
// .condition: base case condition
// .base: value when condition true
// .step: recursive step
@factorial (number: int) -> int = recurse(
    .condition: number <= 1,
    .base: 1,
    .step: number * self(number - 1),
)

// With memoization
@fibonacci (term: int) -> int = recurse(
    .condition: term <= 1,
    .base: term,
    .step: self(term - 1) + self(term - 2),
    .memo: true,
)

// With parallelism
// .parallel: parallelize when term > threshold
@fibonacci_parallel (term: int) -> int = recurse(
    .condition: term <= 1,
    .base: term,
    .step: self(term - 1) + self(term - 2),
    .parallel: 20,
)
```

### `fold` — Reduce/Aggregate

```sigil
@sum (numbers: [int]) -> int = fold(
    .over: numbers,
    .initial: 0,
    .operation: +,
)

@product (numbers: [int]) -> int = fold(
    .over: numbers,
    .initial: 1,
    .operation: *,
)
```

### `map` — Transform

```sigil
@double_all (numbers: [int]) -> [int] = map(
    .over: numbers,
    .transform: number -> number * 2,
)
```

### `filter` — Select

```sigil
@evens (numbers: [int]) -> [int] = filter(
    .over: numbers,
    .predicate: number -> number % 2 == 0,
)
```

### `match` — Pattern Matching

```sigil
@describe (status: Status) -> str = match(
    status,
    Pending -> "waiting",
    Running -> "active",
    Done -> "complete",
)
```

### `run` — Sequential Execution

```sigil
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: item -> item * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: item -> item > 10,
    ),
    fold(
        .over: filtered,
        .initial: 0,
        .operation: +,
    ),
)
```

### `try` — Error Propagation

```sigil
@process (path: str) -> Result<Data, Error> = try(
    let content = read_file(
        .path: path,
    )?,
    let parsed = parse(
        .input: content,
    )?,
    Ok(
        transform(
            .data: parsed,
        ),
    ),
)
```

### `parallel` — Concurrent Execution

```sigil
@fetch_dashboard (user_id: str) -> Dashboard = run(
    let data = parallel(
        .user: get_user(
            .id: user_id,
        ),
        .posts: get_posts(
            .user_id: user_id,
        ),
        .notifications: get_notifs(
            .user_id: user_id,
        ),
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
    .operation: fetch_data(),
    .attempts: 3,
    .backoff: exponential(
        .base: 100ms,
        .max: 10s,
    ),
)

// Language handles: timing, jitter, error handling
```

### 3. Consistent Structure

All patterns follow the same structure:

```sigil
pattern_name(
    .required_property: value,
    .optional_property: value,
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
| `recurse` | `.condition`, `.base`, `.step` | `.memo`, `.parallel` |
| `fold` | `.over`, `.initial`, `.operation` | — |
| `map` | `.over`, `.transform` | — |
| `filter` | `.over`, `.predicate` | — |
| `collect` | `.range`, `.transform` | — |
| `parallel` | Named expr pairs or `.tasks` | `.timeout`, `.on_error`, `.max_concurrent` |
| `retry` | `.operation`, `.attempts` | `.backoff`, `.on`, `.jitter` |
| `cache` | `.key`, `.operation` | `.ttl` |
| `validate` | `.rules`, `.then` | — |
| `timeout` | `.operation`, `.after`, `.on_timeout` | — |

---

## See Also

- [Patterns Reference](04-patterns-reference.md) — Complete pattern documentation
- [Basic Syntax](01-basic-syntax.md)
- [Error Handling](../05-error-handling/index.md)
