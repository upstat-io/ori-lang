# Patterns Reference

Complete documentation for all Ori patterns.

---

## Data Transformation Patterns

### `fold` — Reduce/Aggregate

Reduce a collection to a single value.

**Syntax:**
```ori
fold(
    .over: collection,
    .initial: initial_value,
    .operation: operation,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to fold over |
| `.initial` | `U` | Initial accumulator value |
| `.operation` | `(U, T) -> U` | Binary operation |

**Examples:**
```ori
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

@concat (strings: [str]) -> str = fold(
    .over: strings,
    .initial: "",
    .operation: +,
)

@maximum (numbers: [int]) -> int = fold(
    .over: numbers,
    .initial: numbers[0],
    .operation: (left, right) -> if left > right then left else right,
)
```

---

### `map` — Transform

Transform each element in a collection.

**Syntax:**
```ori
map(
    .over: collection,
    .transform: function,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to map over |
| `.transform` | `T -> U` | Function to apply |

**Examples:**
```ori
@double_all (numbers: [int]) -> [int] = map(
    .over: numbers,
    .transform: number -> number * 2,
)

@names (users: [User]) -> [str] = map(
    .over: users,
    .transform: user -> user.name,
)

@lengths (strings: [str]) -> [int] = map(
    .over: strings,
    .transform: string -> len(
        .collection: string,
    ),
)
```

---

### `filter` — Select

Select elements matching a predicate.

**Syntax:**
```ori
filter(
    .over: collection,
    .predicate: function,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to filter |
| `.predicate` | `T -> bool` | Selection condition |

**Examples:**
```ori
@evens (numbers: [int]) -> [int] = filter(
    .over: numbers,
    .predicate: number -> number % 2 == 0,
)

@adults (users: [User]) -> [User] = filter(
    .over: users,
    .predicate: user -> user.age >= 18,
)

@non_empty (strings: [str]) -> [str] = filter(
    .over: strings,
    .predicate: string -> len(
        .collection: string,
    ) > 0,
)
```

---

### `find` — First Match

Find the first element matching a predicate.

**Syntax:**
```ori
find(
    .over: collection,
    .where: predicate,
    // optional
    .default: fallback,
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
```ori
@first_positive (numbers: [int]) -> Option<int> = find(
    .over: numbers,
    .where: number -> number > 0,
)

@first_positive_or_zero (numbers: [int]) -> int = find(
    .over: numbers,
    .where: number -> number > 0,
    .default: 0,
)

@find_admin (users: [User]) -> Option<User> = find(
    .over: users,
    .where: user -> user.role == Admin,
)
```

**With Transformation (find_map):**

For cases where you want to find and transform in one pass:

```ori
find(
    .over: collection,
    // T -> Option<U>
    .map: transform,
)
```

```ori
@first_valid_number (strings: [str]) -> Option<int> = find(
    .over: strings,
    .map: string -> parse_int(
        .value: string,
    ).ok(),
)
```

---

### `collect` — Build from Range

Build a list by applying a function over a range.

**Syntax:**
```ori
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
```ori
@squares (count: int) -> [int] = collect(
    .range: 1..count,
    .transform: number -> number * number,
)
@fib_sequence (count: int) -> [int] = collect(
    .range: 0..count,
    .transform: fibonacci,
)
```

---

## Control Flow Patterns

### `recurse` — Recursive Functions

Define recursive functions with optional memoization and parallelism.

**Syntax:**
```ori
recurse(
    .condition: condition,
    .base: value,
    .step: expression,
    // optional
    .memo: bool,
    // optional
    .parallel: int,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.condition` | `bool` | Base case condition |
| `.base` | `T` | Value when condition true |
| `.step` | `T` | Recursive expression (use `self()`) |
| `.memo` | `bool` | Enable memoization (default: false) |
| `.parallel` | `int` | Parallelize when input > threshold |

**Examples:**
```ori
@factorial (number: int) -> int = recurse(
    .condition: number <= 1,
    .base: 1,
    .step: number * self(number - 1),
)

@fibonacci (term: int) -> int = recurse(
    .condition: term <= 1,
    .base: term,
    .step: self(term - 1) + self(term - 2),
    .memo: true,
)

@fibonacci_parallel (term: int) -> int = recurse(
    .condition: term <= 1,
    .base: term,
    .step: self(term - 1) + self(term - 2),
    .parallel: 20,
)
```

**Memoization Semantics:**

The `.memo: true` option enables function-scoped memoization:

- Cache is created when the top-level call begins
- All recursive calls within that invocation share the cache
- Cache is discarded when the top-level call returns
- Next call starts with a fresh cache

```ori
@fibonacci (term: int) -> int = recurse(
    .condition: term <= 1,
    .base: term,
    .step: self(term - 1) + self(term - 2),
    .memo: true,
)

// fibonacci(100) creates cache, computes in O(n), discards cache
// fibonacci(100) again creates fresh cache, recomputes in O(n)
```

This design:
- Has no hidden global state
- Is deterministic (same input → same behavior)
- Solves exponential blowup within recursive calls
- Requires no capability declaration

**For cross-call caching**, use the `Cache` capability explicitly:
```ori
@fib_persistent (number: int) -> int uses Cache =
    match(Cache.get("fib:" + str(number)),
        Some(cached) -> parse_int(
            .value: cached,
        ),
        None -> run(
            let result = compute_fib(
                .value: number,
            ),
            Cache.set("fib:" + str(number), str(result)),
            result,
        ),
    )
```

See [Capabilities](../14-capabilities/index.md) for persistent caching patterns.

**When to Use `recurse` vs Direct Recursion:**

Ori supports both the `recurse` pattern and direct recursion (calling the function by name):

```ori
// Direct recursion - function calls itself by name
@factorial_direct (number: int) -> int =
    if number <= 1 then 1
    else number * factorial_direct(number - 1)

// recurse pattern - uses self() for recursive calls
@factorial_pattern (number: int) -> int = recurse(
    .condition: number <= 1,
    .base: 1,
    .step: number * self(number - 1),
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
```ori
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
```ori
// Find first valid parsed integer in range
@find_valid (items: [str]) -> Result<int, void> = for(
    .over: items,
    .map: item -> parse_int(
        .value: item,
    ),
    .match: Ok(value).match(value > 0 && value < 100),
    .default: Err(void),
)

// Find first Some value
@find_some (items: [Option<int>]) -> Option<int> = for(
    .over: items,
    .map: item -> item,
    .match: Some(value) -> true,
    .default: None,
)
```

**Semantics:**
- Iterates through `.over` collection
- Applies `.map` transformation to each item
- If `.match` pattern succeeds, returns that mapped value
- If no match found, returns `.default`

**Note:** For simple iteration without early exit, use the imperative `for` form:
```ori
for item in items do process(
    .item: item,
)
for item in items yield item * 2
```

See [Expressions](02-expressions.md) for the imperative `for` syntax.

---

### `find` — Find First Match

Find the first element matching a predicate.

**Syntax:**
```ori
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
```ori
@first_positive (numbers: [int]) -> Option<int> = find(
    .over: numbers,
    .where: number -> number > 0,
)

@find_user (users: [User], name: str) -> Option<User> = find(
    .over: users,
    .where: user -> user.name == name,
)
```

**Returns:** `Option<T>` — `Some(item)` if found, `None` otherwise.

---

### `match` — Pattern Matching

Dispatch based on value patterns.

**Syntax:**
```ori
match(value,
    pattern1 -> result1,
    pattern2 -> result2,
    ...
)
```

**Examples:**
```ori
@describe (status: Status) -> str = match(status,
    Pending -> "waiting",
    Running(progress) -> "at " + str(progress) + "%",
    Done -> "complete",
    Failed(error) -> "error: " + error,
)

@fizzbuzz (number: int) -> str = match(number,
    number.match(number % 15 == 0) -> "FizzBuzz",
    number.match(number % 3 == 0) -> "Fizz",
    number.match(number % 5 == 0) -> "Buzz",
    number -> str(number),
)
```

See [Pattern Matching](../06-pattern-matching/index.md) for full documentation.

---

### `run` — Sequential Execution

Execute expressions in sequence, with bindings.

**Syntax:**
```ori
run(
    let binding1 = expr1,
    let binding2 = expr2,
    final_expression,
)
```

**Examples:**
```ori
@process (items: [int]) -> int = run(
    let doubled = map(
        .over: items,
        .transform: number -> number * 2,
    ),
    let filtered = filter(
        .over: doubled,
        .predicate: number -> number > 10,
    ),
    fold(
        .over: filtered,
        .initial: 0,
        .operation: +,
    ),
)

@main () -> void = run(
    print(
        .message: "Starting",
    ),
    let result = compute(),
    print(
        .message: "Result: " + str(result),
    ),
)
```

---

### `try` — Error Propagation

Execute expressions, propagating errors automatically.

**Syntax:**
```ori
try(
    // must return Result
    let binding1 = result_expr1,
    let binding2 = result_expr2,
    Ok(final_value),
)
```

**Behavior:**
1. Evaluates expressions in sequence
2. If any returns `Err(e)`, immediately returns `Err(e)`
3. If all succeed, returns the final expression

**Examples:**
```ori
@process (path: str) -> Result<Data, Error> = try(
    let content = read_file(
        .path: path,
    )?,
    let parsed = parse(
        .content: content,
    )?,
    Ok(transform(
        .data: parsed,
    )),
)

@load (path: str) -> Result<Data, AppError> = try(
    let content = read_file(
        .path: path,
    ).map_err(
        .transform: error -> AppError.Io(error),
    )?,
    Ok(parse(
        .content: content,
    )?),
)
```

---

## Concurrency Patterns

### `parallel` — Concurrent Execution (All-Settled)

Execute tasks concurrently and wait for all to settle. The pattern always returns—errors are captured as values, never causing `parallel` itself to fail.

**Syntax:**
```ori
parallel(
    .tasks: task_list,
    // optional
    .max_concurrent: int,
    // optional, per-task
    .timeout: duration,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.tasks` | `[() -> T]` | List of task expressions |
| `.max_concurrent` | `int` | Maximum concurrent tasks (optional) |
| `.timeout` | `Duration` | Per-task timeout (optional) |

**Return Type:** `[Result<T, E>]`

Each slot in the result list contains:
- `Ok(value)` — task succeeded
- `Err(e)` — task failed with error `e`
- `Err(TimeoutError)` — task exceeded timeout

**Why All-Settled?**

The all-settled approach was chosen because:
1. **No hidden control flow** — You always know what happened to every task
2. **Explicit error handling** — Forces conscious decisions about each result
3. **Predictable behavior** — `parallel` always completes, always returns same-length list
4. **Composable** — Filter successes, collect errors, or handle each individually

**Examples:**

```ori
// Fetch multiple users concurrently
@fetch_users (ids: [int]) -> [Result<User, Error>] = parallel(
    .tasks: map(
        .over: ids,
        .transform: user_id -> () -> get_user(
            .id: user_id,
        ),
    ),
    .max_concurrent: 10,
)

// Handle results explicitly
@fetch_users_safe (ids: [int]) -> [User] = run(
    let results = parallel(
        .tasks: map(
            .over: ids,
            .transform: user_id -> () -> get_user(
                .id: user_id,
            ),
        ),
    ),
    // Extract successful results only
    filter(
        .over: results,
        .predicate: result -> is_ok(
            .result: result,
        ),
    ) |> map(
        .over: _,
        .transform: result -> result.unwrap(),
    ),
)
```

```ori
// Fixed tasks with timeout
@fetch_dashboard (user_id: str) -> Dashboard uses Http = run(
    let results = parallel(
        .tasks: [
            () -> get_user(
                .id: user_id,
            ),
            () -> get_posts(
                .user_id: user_id,
            ),
            () -> get_notifications(
                .user_id: user_id,
            ),
        ],
        .timeout: 5s,
    ),
    // Index access with explicit error handling
    // Propagate error
    let user = results[0]?,
    // Default on error
    let posts = results[1] ?? [],
    // Default on error
    let notifications = results[2] ?? [],
    Dashboard { user, posts, notifications },
)
```

**Error Handling Patterns:**

```ori
// Check if all succeeded
@all_succeeded (results: [Result<T, E>]) -> bool =
    fold(
        .over: results,
        .initial: true,
        .operation: (accumulator, result) -> accumulator && is_ok(
            .result: result,
        ),
    )

// Collect all errors
@collect_errors (results: [Result<T, E>]) -> [E] =
    filter(
        .over: results,
        .predicate: result -> is_err(
            .result: result,
        ),
    ) |> map(
        .over: _,
        .transform: result -> result.err().unwrap(),
    )

// First error or all successes
@all_or_first_error (results: [Result<T, E>]) -> Result<[T], E> = run(
    let first_error = find(
        .over: results,
        .where: result -> is_err(
            .result: result,
        ),
    ),
    match(first_error,
        Some(Err(error)) -> Err(error),
        None -> Ok(map(
            .over: results,
            .transform: result -> result.unwrap(),
        )),
    ),
)
```

---

### `spawn` — Fire and Forget

Execute tasks concurrently without waiting for results. Use when you don't need results and errors can be silently discarded.

**Syntax:**
```ori
spawn(
    .tasks: task_list,
    // optional
    .max_concurrent: int,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.tasks` | `[() -> T]` | List of task expressions |
| `.max_concurrent` | `int` | Maximum concurrent tasks (optional) |

**Return Type:** `void`

**Semantics:**
- Tasks are started but not awaited
- Errors are silently discarded
- Execution continues immediately

**When to Use:**
- Fire-and-forget side effects (logging, analytics, notifications)
- Background tasks where failures are acceptable
- When you explicitly don't need results

**Examples:**

```ori
// Send notifications without waiting
@notify_all (users: [User], message: str) -> void = spawn(
    .tasks: map(
        .over: users,
        .transform: user -> () -> send_notification(
            .user: user,
            .message: message,
        ),
    ),
)

// Log analytics events in background
@log_event (event: Event) -> void = spawn(
    .tasks: [
        () -> log_to_console(
            .event: event,
        ),
        () -> send_to_analytics(
            .event: event,
        ),
    ],
)
```

**Warning:** Since errors are discarded, use `spawn` only when you genuinely don't care about failures. For most cases, prefer `parallel` and explicitly handle results.

---

### `timeout` — Time-Bounded Operations

Limit operation execution time.

**Syntax:**
```ori
timeout(
    .operation: expression,
    .after: duration,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.operation` | `async T` | Operation to timeout |
| `.after` | `Duration` | Timeout duration |

**Returns:** `Result<T, TimeoutError>`

**Examples:**
```ori
@fetch_with_timeout (url: str) -> Result<Data, Error> =
    timeout(
        .operation: http_get(
            .url: url,
        ),
        .after: 30s,
    ).map_err(
        .transform: error -> Error { message: "request timed out", cause: None },
    )

// With fallback using ??
@fetch_or_default (url: str) -> Data =
    timeout(
        .operation: http_get(
            .url: url,
        ),
        .after: 30s,
    ) ?? default_data()
```

---

## Resilience Patterns

### `retry` — Retry with Backoff

Retry operations that may transiently fail.

**Syntax:**
```ori
retry(
    .operation: expression,
    .attempts: int,
    // optional
    .backoff: strategy,
    // optional
    .on: [ErrorType, ...],
    // optional
    .jitter: bool,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.operation` | `Result<T, E>` | Operation to retry |
| `.attempts` | `int` | Maximum attempts |
| `.backoff` | Backoff | Backoff strategy |
| `.on` | `[ErrorType]` | Errors that trigger retry |
| `.jitter` | `bool` | Add randomness (default: true) |

**Backoff Strategies:**
- `constant(duration)` — Same wait each retry
- `linear(start, increment)` — Increases linearly
- `exponential(base, max)` — Doubles (capped)

**Example:**
```ori
@fetch_data (url: str) -> Result<Data, Error> = retry(
    .operation: http_get(
        .url: url,
    ),
    .attempts: 3,
    .backoff: exponential(
        .base: 100ms,
        .max: 5s,
    ),
    .on: [Timeout, ConnectionError],
)
```

---

### `cache` — Memoization with TTL

Cache results with optional time-to-live. Requires the `Cache` capability since it persists state across calls.

**Syntax:**
```ori
cache(
    .key: expression,
    .operation: expression,
    // optional
    .ttl: duration,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.key` | `K` | Cache key |
| `.operation` | `T` | Computation to cache |
| `.ttl` | `Duration` | Time-to-live (optional) |

**Example:**
```ori
@get_user (id: int) -> Result<User, Error> uses Cache = cache(
    .key: "user:" + str(id),
    .operation: fetch_user_from_db(
        .id: id,
    ),
    .ttl: 5m,
)
```

**Note:** Unlike `recurse(.memo: true)` which is function-scoped and automatic, the `cache` pattern persists across calls and requires the `Cache` capability. This makes it explicit and testable.

| Caching Type | Scope | Capability Required |
|--------------|-------|---------------------|
| `recurse(.memo: true)` | Within single call | No |
| `cache(.key: ..., .operation: ...)` | Across calls | Yes (`uses Cache`) |

See [Capabilities](../14-capabilities/index.md) for testing cached functions.

---

### `validate` — Input Validation

Validate input with error accumulation.

**Syntax:**
```ori
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
```ori
@validate_user (input: UserInput) -> Result<User, [str]> = validate(
    .rules: [
        len(
            .collection: input.name,
        ) >= 1 | "name required",
        len(
            .collection: input.name,
        ) <= 100 | "name too long",
        input.age >= 0 | "age must be non-negative",
        input.email.contains(
            .pattern: "@",
        ) | "invalid email",
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
```ori
with(
    .acquire: expression,
    .use: resource -> expression,
    .release: resource -> expression,
)
```

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.acquire` | `R` | Get resource |
| `.use` | `R -> T` | Use resource |
| `.release` | `R -> void` | Cleanup (always runs) |

**Example:**
```ori
@read_config (path: str) -> Result<Config, Error> = with(
    .acquire: open_file(
        .path: path,
    ),
    .use: file -> parse_config(
        .file: file,
    ),
    .release: file -> close_file(
        .file: file,
    ),
)
```

---

## See Also

- [Patterns Overview](03-patterns-overview.md)
- [Error Handling](../05-error-handling/index.md)
- [Async](../10-async/index.md)
