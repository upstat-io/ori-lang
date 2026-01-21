# Patterns Reference

Complete documentation for all Sigil patterns.

---

## Data Transformation Patterns

### `fold` — Reduce/Aggregate

Reduce a collection to a single value.

**Syntax:**
```sigil
fold(
    .over: collection,
    .init: initial_value,
    .op: operation
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to fold over |
| `.init` | `U` | Initial accumulator value |
| `.op` | `(U, T) -> U` | Binary operation |

**Examples:**
```sigil
@sum (arr: [int]) -> int = fold(
    .over: arr,
    .init: 0,
    .op: +,
)

@product (arr: [int]) -> int = fold(
    .over: arr,
    .init: 1,
    .op: *,
)

@concat (strs: [str]) -> str = fold(
    .over: strs,
    .init: "",
    .op: +,
)

@max (arr: [int]) -> int = fold(
    .over: arr,
    .init: arr[0],
    .op: (a, b) -> if a > b then a else b,
)
```

---

### `map` — Transform

Transform each element in a collection.

**Syntax:**
```sigil
map(
    .over: collection,
    .transform: function
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to map over |
| `.transform` | `T -> U` | Function to apply |

**Examples:**
```sigil
@double_all (arr: [int]) -> [int] = map(
    .over: arr,
    .transform: x -> x * 2,
)

@names (users: [User]) -> [str] = map(
    .over: users,
    .transform: u -> u.name,
)

@lengths (strs: [str]) -> [int] = map(
    .over: strs,
    .transform: s -> s.len(),
)
```

---

### `filter` — Select

Select elements matching a predicate.

**Syntax:**
```sigil
filter(
    .over: collection,
    .predicate: function
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to filter |
| `.predicate` | `T -> bool` | Selection condition |

**Examples:**
```sigil
@evens (arr: [int]) -> [int] = filter(
    .over: arr,
    .predicate: x -> x % 2 == 0,
)

@adults (users: [User]) -> [User] = filter(
    .over: users,
    .predicate: u -> u.age >= 18,
)

@non_empty (strs: [str]) -> [str] = filter(
    .over: strs,
    .predicate: s -> s.len() > 0,
)
```

---

### `find` — First Match

Find the first element matching a predicate.

**Syntax:**
```sigil
find(
    .over: collection,
    .where: predicate,
    .default: fallback,    // optional
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to search |
| `.where` | `T -> bool` | Selection predicate |
| `.default` | `T` | Fallback if not found (optional) |

**Returns:**
- `Option<T>` if no `.default` provided
- `T` if `.default` provided

**Examples:**
```sigil
@first_positive (numbers: [int]) -> Option<int> = find(
    .over: numbers,
    .where: n -> n > 0,
)

@first_positive_or_zero (numbers: [int]) -> int = find(
    .over: numbers,
    .where: n -> n > 0,
    .default: 0,
)

@find_admin (users: [User]) -> Option<User> = find(
    .over: users,
    .where: u -> u.role == Admin,
)
```

**With Transformation (find_map):**

For cases where you want to find and transform in one pass:

```sigil
find(
    .over: collection,
    .map: transform,       // T -> Option<U>
)
```

```sigil
@first_valid_number (strings: [str]) -> Option<int> = find(
    .over: strings,
    .map: s -> parse_int(s).ok(),
)
```

---

### `collect` — Build from Range

Build a list by applying a function over a range.

**Syntax:**
```sigil
collect(
    .range: range,
    .transform: function,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.range` | `Range` | Range to iterate |
| `.transform` | `int -> T` | Function to apply |

**Examples:**
```sigil
@squares (n: int) -> [int] = collect(
    .range: 1..n,
    .transform: x -> x * x,
)
@fib_sequence (n: int) -> [int] = collect(
    .range: 0..n,
    .transform: fibonacci,
)
```

---

## Control Flow Patterns

### `recurse` — Recursive Functions

Define recursive functions with optional memoization and parallelism.

**Syntax:**
```sigil
recurse(
    .cond: condition,
    .base: value,
    .step: expression,
    .memo: bool,        // optional
    .parallel: int,     // optional
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.cond` | `bool` | Base case condition |
| `.base` | `T` | Value when condition true |
| `.step` | `T` | Recursive expression (use `self()`) |
| `.memo` | `bool` | Enable memoization (default: false) |
| `.parallel` | `int` | Parallelize when n > threshold |

**Examples:**
```sigil
@factorial (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: 1,
    .step: n * self(n - 1),
)

@fibonacci (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true,
)

@fib_parallel (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .parallel: 20,
)
```

**Memoization Semantics:**

The `.memo: true` option enables function-scoped memoization:

- Cache is created when the top-level call begins
- All recursive calls within that invocation share the cache
- Cache is discarded when the top-level call returns
- Next call starts with a fresh cache

```sigil
@fib (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true,
)

// fib(100) creates cache, computes in O(n), discards cache
// fib(100) again creates fresh cache, recomputes in O(n)
```

This design:
- Has no hidden global state
- Is deterministic (same input → same behavior)
- Solves exponential blowup within recursive calls
- Requires no capability declaration

**For cross-call caching**, use the `Cache` capability explicitly:
```sigil
@fib_persistent (n: int) -> int uses Cache =
    match(Cache.get("fib:" + str(n)),
        Some(v) -> parse_int(v),
        None -> run(
            let result = compute_fib(n),
            Cache.set("fib:" + str(n), str(result)),
            result,
        ),
    )
```

See [Capabilities](../14-capabilities/index.md) for persistent caching patterns.

**When to Use `recurse` vs Direct Recursion:**

Sigil supports both the `recurse` pattern and direct recursion (calling the function by name):

```sigil
// Direct recursion - function calls itself by name
@factorial_direct (n: int) -> int =
    if n <= 1 then 1
    else n * factorial_direct(n - 1)

// recurse pattern - uses self() for recursive calls
@factorial_pattern (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: 1,
    .step: n * self(n - 1),
)
```

| Use `recurse` when | Use direct recursion when |
|--------------------|---------------------------|
| You need memoization (`.memo: true`) | Simple tail recursion |
| You want parallelization (`.parallel: n`) | Complex branching logic |
| You want guaranteed structure | Multiple recursive paths |
| AI-generated code (safer defaults) | Manual optimization needed |

The `recurse` pattern provides:
- **Structural guarantees** — Base case and step are explicit
- **Automatic optimizations** — Memoization, parallelization
- **AI-friendliness** — Harder to get wrong

Direct recursion provides:
- **Flexibility** — Any control flow is possible
- **Familiarity** — Standard recursive style
- **Complex patterns** — Mutual recursion, multiple base cases

**Recommendation:** Prefer `recurse` for standard recursive algorithms (factorial, fibonacci, tree traversal). Use direct recursion when the algorithm doesn't fit the single-base-case, single-step pattern.

---

### `for` — Iteration with Early Exit

The `for` pattern enables iteration over collections with early exit via `Ok`/`Err` semantics.

**Syntax:**
```sigil
for(
    .over: collection,
    .map: item -> expression,
    .match: pattern -> condition,
    .default: value,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to iterate over |
| `.map` | `(T) -> U` | Transform each item |
| `.match` | `pattern -> bool` | Match condition for early exit |
| `.default` | `V` | Value if no match found |

**Examples:**
```sigil
// Find first valid parsed integer in range
@find_valid (items: [str]) -> Result<int, void> = for(
    .over: items,
    .map: item -> parse_int(item),
    .match: Ok(n).match(n > 0 && n < 100),
    .default: Err(void),
)

// Find first Some value
@find_some (items: [Option<int>]) -> Option<int> = for(
    .over: items,
    .map: item -> item,
    .match: Some(n) -> true,
    .default: None,
)
```

**Semantics:**
- Iterates through `.over` collection
- Applies `.map` transformation to each item
- If `.match` pattern succeeds, returns that mapped value
- If no match found, returns `.default`

**Note:** For simple iteration without early exit, use the imperative `for` form:
```sigil
for item in items do process(item)
for x in xs yield x * 2
```

See [Expressions](02-expressions.md) for the imperative `for` syntax.

---

### `find` — Find First Match

Find the first element matching a predicate.

**Syntax:**
```sigil
find(
    .over: collection,
    .where: predicate,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to search |
| `.where` | `(T) -> bool` | Predicate function |

**Examples:**
```sigil
@first_positive (numbers: [int]) -> Option<int> = find(
    .over: numbers,
    .where: n -> n > 0,
)

@find_user (users: [User], name: str) -> Option<User> = find(
    .over: users,
    .where: u -> u.name == name,
)
```

**Returns:** `Option<T>` — `Some(item)` if found, `None` otherwise.

---

### `match` — Pattern Matching

Dispatch based on value patterns.

**Syntax:**
```sigil
match(value,
    pattern1 -> result1,
    pattern2 -> result2,
    ...
)
```

**Examples:**
```sigil
@describe (status: Status) -> str = match(status,
    Pending -> "waiting",
    Running(p) -> "at " + str(p) + "%",
    Done -> "complete",
    Failed(e) -> "error: " + e
)

@fizzbuzz (n: int) -> str = match(n,
    n.match(n % 15 == 0) -> "FizzBuzz",
    n.match(n % 3 == 0) -> "Fizz",
    n.match(n % 5 == 0) -> "Buzz",
    n -> str(n),
)
```

See [Pattern Matching](../06-pattern-matching/index.md) for full documentation.

---

### `run` — Sequential Execution

Execute expressions in sequence, with bindings.

**Syntax:**
```sigil
run(
    let binding1 = expr1,
    let binding2 = expr2,
    final_expression,
)
```

**Examples:**
```sigil
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: x -> x * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: x -> x > 10,
    ),
    fold(
        .over: filtered,
        .init: 0,
        .op: +,
    ),
)

@main () -> void = run(
    print("Starting"),
    let result = compute(),
    print("Result: " + str(result)),
)
```

---

### `try` — Error Propagation

Execute expressions, propagating errors automatically.

**Syntax:**
```sigil
try(
    let binding1 = result_expr1,  // must return Result
    let binding2 = result_expr2,
    Ok(final_value),
)
```

**Behavior:**
1. Evaluates expressions in sequence
2. If any returns `Err(e)`, immediately returns `Err(e)`
3. If all succeed, returns the final expression

**Examples:**
```sigil
@process (path: str) -> Result<Data, Error> = try(
    let content = read_file(path)?,
    let parsed = parse(content)?,
    Ok(transform(parsed)),
)

@load (path: str) -> Result<Data, AppError> = try(
    let content = read_file(path).map_err(e -> AppError.Io(e))?,
    Ok(parse(content)?),
)
```

---

## Concurrency Patterns

### `parallel` — Concurrent Execution

Execute multiple tasks concurrently, wait for all.

**Syntax:**
```sigil
parallel(
    .name1: expr1,
    .name2: expr2,
    ...
    .timeout: duration,    // optional
    .on_error: strategy    // optional
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.name` | `T` | Named task expressions |
| `.timeout` | `Duration` | Cancel after timeout (optional) |
| `.on_error` | `fail_fast \| collect_all` | Error strategy (default: fail_fast) |

**Examples:**
```sigil
@fetch_dashboard (id: str) -> Dashboard = run(
    let data = parallel(
        .user: get_user(id),
        .posts: get_posts(id),
        .notifications: get_notifs(id),
    ),
    Dashboard {
        user: data.user,
        posts: data.posts,
        notifications: data.notifications,
    },
)

@fetch_with_timeout () -> Result<Data, Error> = parallel(
    .a: fetch_slow(),
    .b: fetch_fast(),
    .timeout: 5s,
    .on_error: fail_fast,
)
```

**List-based form with `.tasks`:**

For dynamic task lists, use `.tasks` with `.max_concurrent`:

```sigil
@fetch_all (ids: [int]) -> [User] = parallel(
    .tasks: map(ids, id -> fetch_user(id)),
    .max_concurrent: 10,
)
```

| Property | Type | Description |
|----------|------|-------------|
| `.tasks` | `[async T]` | List of async tasks |
| `.max_concurrent` | `int` | Max parallel tasks (optional) |

---

### `timeout` — Time-Bounded Operations

Limit operation execution time.

**Syntax:**
```sigil
timeout(
    .op: expression,
    .after: duration
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.op` | `async T` | Operation to timeout |
| `.after` | `Duration` | Timeout duration |

**Returns:** `Result<T, TimeoutError>`

**Examples:**
```sigil
@fetch_with_timeout (url: str) -> Result<Data, Error> =
    timeout(
        .op: http_get(url),
        .after: 30s
    ).map_err(e -> Error { message: "request timed out", cause: None })

// With fallback using ??
@fetch_or_default (url: str) -> Data =
    timeout(
        .op: http_get(url),
        .after: 30s,
    ) ?? default_data()
```

---

## Resilience Patterns

### `retry` — Retry with Backoff

Retry operations that may transiently fail.

**Syntax:**
```sigil
retry(
    .op: expression,
    .attempts: int,
    .backoff: strategy,      // optional
    .on: [ErrorType, ...],   // optional
    .jitter: bool            // optional
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.op` | `Result<T, E>` | Operation to retry |
| `.attempts` | `int` | Maximum attempts |
| `.backoff` | Backoff | Backoff strategy |
| `.on` | `[ErrorType]` | Errors that trigger retry |
| `.jitter` | `bool` | Add randomness (default: true) |

**Backoff Strategies:**
- `constant(duration)` — Same wait each retry
- `linear(start, increment)` — Increases linearly
- `exponential(base, max)` — Doubles (capped)

**Example:**
```sigil
@fetch_data (url: str) -> Result<Data, Error> = retry(
    .op: http_get(url),
    .attempts: 3,
    .backoff: exponential(base: 100ms, max: 5s),
    .on: [Timeout, ConnectionError],
)
```

---

### `cache` — Memoization with TTL

Cache results with optional time-to-live. Requires the `Cache` capability since it persists state across calls.

**Syntax:**
```sigil
cache(
    .key: expression,
    .op: expression,
    .ttl: duration,    // optional
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.key` | `K` | Cache key |
| `.op` | `T` | Computation to cache |
| `.ttl` | `Duration` | Time-to-live (optional) |

**Example:**
```sigil
@get_user (id: int) -> Result<User, Error> uses Cache = cache(
    .key: "user:" + str(id),
    .op: fetch_user_from_db(id),
    .ttl: 5m,
)
```

**Note:** Unlike `recurse(.memo: true)` which is function-scoped and automatic, the `cache` pattern persists across calls and requires the `Cache` capability. This makes it explicit and testable.

| Caching Type | Scope | Capability Required |
|--------------|-------|---------------------|
| `recurse(.memo: true)` | Within single call | No |
| `cache(.key: ..., .op: ...)` | Across calls | Yes (`uses Cache`) |

See [Capabilities](../14-capabilities/index.md) for testing cached functions.

---

### `validate` — Input Validation

Validate input with error accumulation.

**Syntax:**
```sigil
validate(
    .rules: [
        condition | "error message",
        ...
    ],
    .then: success_value
)
```

**Returns:** `Result<T, [str]>` — Ok with value, or Err with all failures.

**Example:**
```sigil
@validate_user (input: UserInput) -> Result<User, [str]> = validate(
    .rules: [
        input.name.len() >= 1 | "name required",
        input.name.len() <= 100 | "name too long",
        input.age >= 0 | "age must be non-negative",
        input.email.contains("@") | "invalid email",
    ],
    .then: User {
        name: input.name,
        age: input.age,
        email: input.email,
    },
)
```

---

### `with` — Resource Management

Ensure resource cleanup even on errors.

**Syntax:**
```sigil
with(
    .acquire: expression,
    .use: resource -> expression,
    .release: resource -> expression
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.acquire` | `R` | Get resource |
| `.use` | `R -> T` | Use resource |
| `.release` | `R -> void` | Cleanup (always runs) |

**Example:**
```sigil
@read_config (path: str) -> Result<Config, Error> = with(
    .acquire: open_file(path),
    .use: file -> parse_config(file),
    .release: file -> close_file(file),
)
```

---

## See Also

- [Patterns Overview](03-patterns-overview.md)
- [Error Handling](../05-error-handling/index.md)
- [Async](../10-async/index.md)
