# Appendix D: Pattern Quick Reference

A quick reference for all Sigil patterns with syntax and examples.

---

## Pattern Summary Table

| Pattern | Purpose | Positional | Named |
|---------|---------|------------|-------|
| `run` | Sequential execution | `run(a, b, c)` | N/A |
| `try` | Error propagation | `try(a, b, c)` | N/A |
| `match` | Pattern matching | `match(val, arms...)` | N/A |
| `map` | Transform elements | `map(list, fn)` | N/A |
| `filter` | Select elements | `filter(list, pred)` | N/A |
| `fold` | Aggregate/reduce | `fold(list, init, fn)` | N/A |
| `recurse` | Recursion | `recurse(cond, base, step)` | `.memo: true` |
| `collect` | Build list | `collect(range, fn)` | N/A |
| `parallel` | Concurrent execution | `parallel(tasks...)` | N/A |
| `retry` | Retry on failure | `retry(expr)` | `.times`, `.delay`, `.backoff` |
| `cache` | Memoize result | `cache(key, expr)` | `.ttl` |
| `validate` | Input validation | `validate(val, rules...)` | N/A |
| `timeout` | Time limit | `timeout(expr, dur)` | N/A |
| `with` | Resource management | `with(resource, fn)` | N/A |

---

## run — Sequential Execution

**Purpose:** Execute expressions in order, return last value.

```sigil
run(
    x = compute_a(),
    y = compute_b(x),
    x + y
)
```

**Bindings:** Variables bound with `=` are available in subsequent expressions.

**Returns:** Value of last expression.

---

## try — Error Propagation

**Purpose:** Execute until first error, return early on `Err`.

```sigil
try(
    user = get_user(id)?,
    data = fetch_data(user.id)?,
    process(data)
)
```

**Binding Syntax:**
- `name = expr?` — unwraps `Ok`, returns early on `Err`
- `name = expr` — regular binding (no unwrap)

**Returns:** `Result<T, E>` — success value or first error.

---

## match — Pattern Matching

**Purpose:** Destructure value and branch on patterns.

```sigil
match(value,
    Pattern1 -> result1,
    Pattern2 -> result2,
    _ -> default
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
map(list, x -> x * 2)
map(list, transform_fn)
```

**Type:** `([T], (T) -> U) -> [U]`

**Returns:** New list with transformed elements.

---

## filter — Select Elements

**Purpose:** Keep elements matching predicate.

```sigil
filter(list, x -> x > 0)
filter(list, is_valid)
```

**Type:** `([T], (T) -> bool) -> [T]`

**Returns:** List of elements where predicate is true.

---

## fold — Aggregate/Reduce

**Purpose:** Combine elements into single value.

```sigil
fold(list, initial, (acc, x) -> acc + x)
fold(list, 0, +)  // Shorthand for addition
```

**Type:** `([T], U, (U, T) -> U) -> U`

**Returns:** Final accumulated value.

---

## recurse — Recursion

**Purpose:** Express recursive computation.

### Positional
```sigil
recurse(condition, base_case, recursive_step)
```

### Named
```sigil
recurse(
    .cond: n <= 1,
    .base: 1,
    .step: n * self(n - 1),
    .memo: true  // Optional memoization
)
```

**Keywords:**
- `.cond` — When to return base case
- `.base` — Value to return when cond is true
- `.step` — Recursive computation (use `self` for recursive call)
- `.memo` — Enable memoization (optional, default false)

**Returns:** Result of recursion.

---

## collect — Build List

**Purpose:** Generate list from range.

```sigil
collect(1..=10, x -> x * x)  // [1, 4, 9, 16, ..., 100]
collect(0..n, identity)       // [0, 1, 2, ..., n-1]
```

**Type:** `(Range, (int) -> T) -> [T]`

**Returns:** List of generated elements.

---

## parallel — Concurrent Execution

**Purpose:** Run tasks concurrently.

```sigil
parallel(
    a = fetch_users(),
    b = fetch_orders(),
    c = fetch_products()
)
// Returns struct: { a: [User], b: [Order], c: [Product] }
```

**Execution:** All tasks start immediately, run concurrently.

**Returns:** Struct with named results. Waits for all tasks.

**Errors:** If any task fails, returns first error (cancels remaining).

---

## retry — Retry on Failure

**Purpose:** Retry failed operation.

### Basic
```sigil
retry(fetch_data())  // Default: 3 retries, 1s delay
```

### Configured
```sigil
retry(
    fetch_data(),
    .times: 5,
    .delay: 500ms,
    .backoff: exponential
)
```

**Keywords:**
- `.times` — Maximum retry attempts (default 3)
- `.delay` — Initial delay between retries (default 1s)
- `.backoff` — `constant`, `linear`, or `exponential` (default constant)

**Returns:** `Result<T, E>` — success or last error.

---

## cache — Memoize Result

**Purpose:** Cache expensive computation.

### Basic
```sigil
cache("user:" + id, fetch_user(id))
```

### With TTL
```sigil
cache("user:" + id, fetch_user(id), .ttl: 5m)
```

**Keywords:**
- `.ttl` — Time-to-live for cached value

**Behavior:** Returns cached value if present and not expired; otherwise computes and caches.

**Returns:** The cached or computed value.

---

## validate — Input Validation

**Purpose:** Validate value against rules.

```sigil
validate(input,
    .min_length: 8,
    .max_length: 100,
    .matches: email_regex
)
```

**Built-in Rules:**
- `.min_length` — Minimum string length
- `.max_length` — Maximum string length
- `.min` — Minimum numeric value
- `.max` — Maximum numeric value
- `.matches` — Regex pattern
- `.predicate` — Custom validation function

**Returns:** `Result<T, ValidationError>`

---

## timeout — Time Limit

**Purpose:** Limit execution time.

```sigil
timeout(slow_operation(), 5s)
```

**Type:** `(T, Duration) -> Result<T, TimeoutError>`

**Returns:** `Ok(value)` if completes in time, `Err(TimeoutError)` otherwise.

---

## with — Resource Management

**Purpose:** Ensure resource cleanup.

```sigil
with(open_file("data.txt"), file ->
    read_all(file)
)
// File automatically closed after block
```

**Behavior:**
1. Acquire resource
2. Execute function with resource
3. Release resource (even on error)

**Returns:** Result of function.

---

## Common Combinations

### Map + Filter
```sigil
filter(map(users, u -> u.age), age -> age >= 18)
```

### Try + Map
```sigil
try(
    users = fetch_users()?,
    map(users, process_user)
)
```

### Parallel + Retry
```sigil
parallel(
    a = retry(fetch_a(), .times: 3),
    b = retry(fetch_b(), .times: 3)
)
```

### Cache + Timeout
```sigil
cache(key, timeout(expensive_op(), 10s))
```

### With + Try
```sigil
with(db.connect(), conn ->
    try(
        data = conn.query(sql)?,
        process(data)
    )
)
```

---

## Pattern Nesting

Patterns can be nested:

```sigil
@process_all (ids: [int]) -> Result<[Data], Error> =
    try(
        map(ids, id ->
            retry(
                timeout(
                    fetch_data(id),
                    5s
                ),
                .times: 3
            )
        )
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
