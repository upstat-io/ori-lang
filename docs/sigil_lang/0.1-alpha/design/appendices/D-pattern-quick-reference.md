# Appendix D: Pattern Quick Reference

A quick reference for all Sigil patterns with syntax and examples.

---

## Pattern Summary Table

All patterns use **named properties exclusively**.

| Pattern | Purpose | Required Properties |
|---------|---------|---------------------|
| `run` | Sequential execution | bindings, final expr |
| `try` | Error propagation | bindings, final expr |
| `match` | Pattern matching | value, arms |
| `map` | Transform elements | `.over`, `.transform` |
| `filter` | Select elements | `.over`, `.predicate` |
| `fold` | Aggregate/reduce | `.over`, `.init`, `.op` |
| `recurse` | Recursion | `.cond`, `.base`, `.step` |
| `collect` | Build list | `.range`, `.transform` |
| `parallel` | Concurrent execution | named task exprs |
| `retry` | Retry on failure | `.op`, `.attempts` |
| `cache` | Memoize result | `.key`, `.op` |
| `validate` | Input validation | `.rules`, `.then` |
| `timeout` | Time limit | `.op`, `.after` |
| `with` | Resource management | `.acquire`, `.use`, `.release` |

---

## run — Sequential Execution

**Purpose:** Execute expressions in order, return last value.

```sigil
run(
    let x = compute_a(),
    let y = compute_b(x),
    x + y,
)
```

**Bindings:** Variables bound with `let` are available in subsequent expressions. Use `let mut` for mutable bindings.

**Returns:** Value of last expression.

---

## try — Error Propagation

**Purpose:** Execute until first error, return early on `Err`.

```sigil
try(
    let user = get_user(id)?,
    let data = fetch_data(user.id)?,
    process(data),
)
```

**Binding Syntax:**
- `let name = expr?` — unwraps `Ok`, returns early on `Err`
- `let name = expr` — regular binding (no unwrap)

**Returns:** `Result<T, E>` — success value or first error.

---

## match — Pattern Matching

**Purpose:** Destructure value and branch on patterns.

```sigil
match(value,
    Pattern1 -> result1,
    Pattern2 -> result2,
    _ -> default,
)
```

**Pattern Types:**
- Literal: `5`, `"hello"`, `true`
- Binding: `x` (binds value to name)
- Wildcard: `_` (matches anything)
- Variant: `Some(x)`, `None`
- Struct: `{ x, y }`, `{ x, .. }`
- List: `[first, ..rest]`
- Or: `A | B`
- Guard: `x if x > 0`

**Returns:** Result of matching arm.

---

## map — Transform Elements

**Purpose:** Apply function to each element.

```sigil
map(
    .over: list,
    .transform: x -> x * 2,
)
```

**Properties:**
- `.over` — Collection to transform
- `.transform` — Function to apply to each element

**Returns:** New list with transformed elements.

---

## filter — Select Elements

**Purpose:** Keep elements matching predicate.

```sigil
filter(
    .over: list,
    .predicate: x -> x > 0,
)
```

**Properties:**
- `.over` — Collection to filter
- `.predicate` — Function returning true for elements to keep

**Returns:** List of elements where predicate is true.

---

## fold — Aggregate/Reduce

**Purpose:** Combine elements into single value.

```sigil
fold(
    .over: list,
    .init: 0,
    .op: (acc, x) -> acc + x,
)

// Using operator shorthand
fold(
    .over: list,
    .init: 0,
    .op: +,
)
```

**Properties:**
- `.over` — Collection to reduce
- `.init` — Initial accumulator value
- `.op` — Binary operation `(acc, item) -> new_acc`

**Returns:** Final accumulated value.

---

## recurse — Recursion

**Purpose:** Express recursive computation.

```sigil
recurse(
    .cond: n <= 1,
    .base: 1,
    .step: n * self(n - 1),
    .memo: true,  // Optional memoization
)
```

**Properties:**
- `.cond` — When to return base case
- `.base` — Value to return when cond is true
- `.step` — Recursive computation (use `self` for recursive call)
- `.memo` — Enable memoization (optional, default false)
- `.parallel` — Parallelize when n > threshold (optional)

**Returns:** Result of recursion.

---

## collect — Build List

**Purpose:** Generate list from range.

```sigil
collect(
    .range: 1..=10,
    .transform: x -> x * x,
)  // [1, 4, 9, 16, ..., 100]
```

**Properties:**
- `.range` — Range to iterate over
- `.transform` — Function to apply to each value

**Returns:** List of generated elements.

---

## parallel — Concurrent Execution

**Purpose:** Run tasks concurrently.

```sigil
parallel(
    .user: fetch_user(id),
    .orders: fetch_orders(id),
    .products: fetch_products(),
)
// Returns struct: { user: User, orders: [Order], products: [Product] }
```

**Properties:**
- Named task expressions (`.name: async_expr`)
- `.timeout` — Cancel all after duration (optional)
- `.on_error` — `fail_fast` or `collect_all` (optional)
- `.max_concurrent` — Limit concurrent tasks (optional, with `.tasks`)

**Returns:** Struct with named results. Waits for all tasks.

**Errors:** If any task fails with `fail_fast`, returns first error (cancels remaining).

---

## retry — Retry on Failure

**Purpose:** Retry failed operation.

```sigil
retry(
    .op: fetch_data(),
    .attempts: 5,
    .backoff: exponential(base: 100ms, max: 5s),
    .on: [Timeout, ConnectionError],
)
```

**Properties:**
- `.op` — Operation to retry
- `.attempts` — Maximum retry attempts
- `.backoff` — `constant(dur)`, `linear(start, inc)`, or `exponential(base, max)`
- `.on` — Error types that trigger retry (optional)
- `.jitter` — Add randomness (optional, default true)

**Returns:** `Result<T, E>` — success or last error.

---

## cache — Memoize Result

**Purpose:** Cache expensive computation across calls (requires `Cache` capability).

```sigil
cache(
    .key: "user:" + str(id),
    .op: fetch_user(id),
    .ttl: 5m,
)
```

**Properties:**
- `.key` — Cache key
- `.op` — Computation to cache
- `.ttl` — Time-to-live (optional)

**Behavior:** Returns cached value if present and not expired; otherwise computes and caches.

**Returns:** The cached or computed value.

---

## validate — Input Validation

**Purpose:** Validate value against rules with error accumulation.

```sigil
validate(
    .rules: [
        input.name.len() >= 1 | "name required",
        input.age >= 0 | "age must be non-negative",
        input.email.contains("@") | "invalid email",
    ],
    .then: User { name: input.name, age: input.age, email: input.email },
)
```

**Properties:**
- `.rules` — List of `condition | "error message"` expressions
- `.then` — Value to return if all rules pass

**Returns:** `Result<T, [str]>` — Ok with value, or Err with all failures.

---

## timeout — Time Limit

**Purpose:** Limit execution time.

```sigil
timeout(
    .op: slow_operation(),
    .after: 5s,
)
```

**Properties:**
- `.op` — Operation to time-limit
- `.after` — Maximum duration

**Returns:** `Result<T, TimeoutError>` — `Ok(value)` if completes in time, `Err(TimeoutError)` otherwise.

---

## with — Resource Management

**Purpose:** Ensure resource cleanup.

```sigil
with(
    .acquire: open_file("data.txt"),
    .use: file -> read_all(file),
    .release: file -> close_file(file),
)
```

**Properties:**
- `.acquire` — Expression to obtain resource
- `.use` — Function using the resource
- `.release` — Cleanup function (always runs, even on error)

**Returns:** Result of use function.

---

## Common Combinations

### Map + Filter
```sigil
filter(
    .over: map(
        .over: users,
        .transform: u -> u.age,
    ),
    .predicate: age -> age >= 18,
)
```

### Try + Map
```sigil
try(
    let users = fetch_users(),
    map(
        .over: users,
        .transform: process_user,
    ),
)
```

### Parallel + Retry
```sigil
parallel(
    .a: retry(
        .op: fetch_a(),
        .attempts: 3,
    ),
    .b: retry(
        .op: fetch_b(),
        .attempts: 3,
    ),
)
```

### Cache + Timeout
```sigil
cache(
    .key: key,
    .op: timeout(
        .op: expensive_op(),
        .after: 10s,
    ),
)
```

### With + Try
```sigil
with(
    .acquire: db.connect(),
    .use: conn -> try(
        let data = conn.query(sql),
        process(data),
    ),
    .release: conn -> conn.close(),
)
```

---

## Pattern Nesting

Patterns can be nested:

```sigil
@process_all (ids: [int]) -> Result<[Data], Error> =
    try(
        let results = map(
            .over: ids,
            .transform: id -> retry(
                .op: timeout(
                    .op: fetch_data(id),
                    .after: 5s,
                ),
                .attempts: 3,
            ),
        ),
        Ok(results),
    )
```

---

## Quick Syntax Reference

### Ranges
```sigil
0..10     // 0 to 9 (exclusive end)
0..=10    // 0 to 10 (inclusive end)
```

### Lambdas
```sigil
x -> x + 1           // Single parameter
(x, y) -> x + y      // Multiple parameters
```

### Match Arms
```sigil
pattern -> expression
pattern if guard -> expression
```

### Named Properties
```sigil
.name: value
```

---

## See Also

- [Patterns Overview](../02-syntax/03-patterns-overview.md)
- [Patterns Reference](../02-syntax/04-patterns-reference.md)
- [Error Handling](../05-error-handling/index.md)
