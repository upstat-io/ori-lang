---
title: "Compiler Patterns"
description: "Deep dive into run, try, recurse, cache, and with patterns."
order: 19
part: "Advanced Patterns"
---

# Compiler Patterns

Ori provides special patterns that the compiler handles with optimized code generation. These patterns provide powerful abstractions with zero overhead.

## Pattern Categories

Patterns fall into two categories:

**function_seq** — Sequential expressions (order matters):
- `run` — sequential evaluation with bindings
- `try` — error propagation
- `match` — pattern matching

**function_exp** — Named expressions:
- `recurse` — self-referential recursion
- `cache` — cached computation
- `with` — resource management
- `catch` — panic capture

## The run Pattern

Sequential expressions where each step can use previous results:

```ori
run(
    let a = compute_a(),
    let b = compute_b(input: a),
    let c = compute_c(x: a, y: b),
    c,  // Return value
)
```

### Basic Usage

```ori
@process_user (id: int) -> UserProfile = run(
    let user = fetch_user(id: id),
    let orders = fetch_orders(user_id: user.id),
    let stats = calculate_stats(orders: orders),
    UserProfile { user, orders, stats },
)
```

### Scope and Bindings

Each binding is available to subsequent expressions:

```ori
run(
    let x = 10,
    let y = x * 2,      // Can use x
    let z = x + y,      // Can use x and y
    print(msg: `{x} {y} {z}`),
    z,                  // Final value is z
)
```

### Side Effects

`run` is for sequential operations with side effects:

```ori
@save_and_notify (user: User) -> void = run(
    save_to_database(user: user),
    send_email(to: user.email, subject: "Welcome!"),
    log_event(type: "user_created", data: user.id),
)
```

### Contracts with run

Add preconditions and postconditions:

```ori
@sqrt (x: float) -> float = run(
    pre_check: x >= 0.0 | "x must be non-negative",
    compute_sqrt(x: x),
    post_check: result -> result >= 0.0,
)
```

- `pre_check:` — verified before the body runs
- `post_check:` — verified after, receives the result as parameter
- `| "message"` — custom error message (panics with this message if check fails)

### Contract Examples

```ori
@divide (a: int, b: int) -> int = run(
    pre_check: b != 0 | "division by zero",
    a / b,
)

@clamp (value: int, min: int, max: int) -> int = run(
    pre_check: min <= max | "min must not exceed max",
    if value < min then min else if value > max then max else value,
    post_check: result -> result >= min && result <= max,
)

@factorial (n: int) -> int = run(
    pre_check: n >= 0 | "factorial undefined for negative numbers",
    if n <= 1 then 1 else n * factorial(n: n - 1),
    post_check: result -> result > 0 | "factorial must be positive",
)
```

## The try Pattern

Like `run`, but designed for error propagation:

```ori
try(
    let a = fallible_a()?,
    let b = fallible_b(input: a)?,
    let c = fallible_c(x: a, y: b)?,
    Ok(c),
)
```

The `?` operator:
- Extracts `Ok(v)` → `v`
- Propagates `Err(e)` → returns early with `Err(e)`

### Error Traces

`try` automatically collects error traces:

```ori
@load_config () -> Result<Config, Error> = try(
    let data = read_file(path: "config.json")?,  // Trace point
    let config = parse_json(data: data)?,         // Trace point
    Ok(config),
)
```

If parsing fails, the trace shows:
```
Error: invalid JSON
Trace:
  at load_config (config.ori:3:18)
```

### Mixing try and run

Use `try` for fallible code, `run` for infallible:

```ori
@process_batch (items: [int]) -> Result<Summary, Error> = try(
    let results = for item in items yield process_item(id: item)?,

    // Switch to run for non-fallible computation
    let summary = run(
        let total = len(collection: results),
        let sum = results.iter().fold(initial: 0, op: (a, b) -> a + b),
        Summary { total, average: sum / total },
    ),

    Ok(summary),
)
```

## The match Pattern

Pattern matching with exhaustiveness checking:

```ori
match(
    value,
    Pattern1 -> result1,
    Pattern2 -> result2,
    _ -> default,
)
```

### Match Must Return Values

Match is an expression — all arms must return the same type:

```ori
let description = match(
    status,
    Active -> "Currently active",
    Inactive -> "Not active",
    Pending -> "Waiting for approval",
)
```

### Exhaustiveness

The compiler ensures all cases are covered:

```ori
type Color = Red | Green | Blue

// ERROR: non-exhaustive match
let name = match(
    color,
    Red -> "red",
    Green -> "green",
    // Missing Blue!
)

// OK: all cases covered
let name = match(
    color,
    Red -> "red",
    Green -> "green",
    Blue -> "blue",
)
```

## The recurse Pattern

Self-referential recursion with optional memoization:

```ori
@fibonacci (n: int) -> int = recurse(
    condition: n <= 1,
    base: n,
    step: self(n: n - 1) + self(n: n - 2),
    memo: true,
)
```

### Parameters

| Parameter | Purpose |
|-----------|---------|
| `condition` | When to return base case |
| `base` | Value for base case |
| `step` | Recursive computation (use `self()`) |
| `memo` | Enable memoization (default: false) |
| `parallel` | Parallelize for n > threshold (optional) |

### Without Memoization

```ori
@factorial (n: int) -> int = recurse(
    condition: n <= 1,
    base: 1,
    step: n * self(n: n - 1),
)
```

### With Memoization

```ori
@fibonacci (n: int) -> int = recurse(
    condition: n <= 1,
    base: n,
    step: self(n: n - 1) + self(n: n - 2),
    memo: true,
)
```

With `memo: true`, results are cached — the second call to `fibonacci(n: 10)` is instant.

### With Parallelization

```ori
@parallel_fib (n: int) -> int = recurse(
    condition: n <= 1,
    base: n,
    step: self(n: n - 1) + self(n: n - 2),
    memo: true,
    parallel: 20,  // Parallelize for n > 20
)
```

### How self() Works

`self()` is a special reference to the enclosing recursive function:

```ori
@tree_depth<T> (node: TreeNode<T>) -> int = recurse(
    condition: is_leaf(node: node),
    base: 0,
    step: 1 + max(
        left: self(node: node.left),
        right: self(node: node.right),
    ),
)
```

## The cache Pattern

Cache expensive computations:

```ori
@get_user (id: int) -> Result<User, Error> uses Http, Cache =
    cache(
        key: `user:{id}`,
        op: Http.get(url: `/users/{id}`),
        ttl: 5m,
    )
```

### Parameters

| Parameter | Purpose |
|-----------|---------|
| `key` | Cache key (string) |
| `op` | Expression to compute if not cached |
| `ttl` | Time-to-live for cached value |

### Cache Behavior

1. Check if `key` exists in cache
2. If exists and not expired, return cached value
3. If not exists or expired, evaluate `op`
4. Store result with `ttl`
5. Return result

### Requires Cache Capability

```ori
@get_user_cached (id: int) -> Result<User, Error> uses Http, Cache =
    cache(
        key: `user:{id}`,
        op: Http.get(url: `/users/{id}`),
        ttl: 5m,
    )

// Test with mock cache
@test_cache tests @get_user_cached () -> void =
    with Http = MockHttp { responses: { "/users/1": `{"id": 1}` } },
         Cache = MockCache {} in run(
        let first = get_user_cached(id: 1),   // Fetches from Http
        let second = get_user_cached(id: 1),  // Returns from cache
        assert_ok(result: first),
        assert_ok(result: second),
    )
```

## The with Pattern

Resource management with guaranteed cleanup:

```ori
@process_file (path: str) -> Result<str, Error> uses FileSystem =
    with(
        acquire: FileSystem.open(path: path),
        use: file -> FileSystem.read_all(file: file),
        release: file -> FileSystem.close(file: file),
    )
```

### Parameters

| Parameter | Purpose |
|-----------|---------|
| `acquire` | Expression to acquire resource |
| `use` | Function to use resource |
| `release` | Function to release resource (always runs) |

### Guaranteed Cleanup

`release` always runs, even if `use` fails:

```ori
@safe_transaction (db: Database) -> Result<void, Error> uses Database =
    with(
        acquire: db.begin_transaction(),
        use: tx -> run(
            tx.insert(table: "users", data: user_data),
            tx.update(table: "stats", data: stats_data),
            Ok(()),
        ),
        release: tx -> tx.rollback_if_uncommitted(),
    )
```

### Similar to try-finally

The `with` pattern is similar to try-finally or RAII:

```python
# Python equivalent
try:
    resource = acquire()
    return use(resource)
finally:
    release(resource)
```

## The catch Pattern

Capture panics as Results:

```ori
let result = catch(expr: might_panic())
// Result<T, str>

match(
    result,
    Ok(value) -> print(msg: `Got: {value}`),
    Err(msg) -> print(msg: `Panic caught: {msg}`),
)
```

### When to Use catch

Use sparingly — panics indicate bugs, not expected errors:

```ori
// Good: Test frameworks
@test_panics tests @divide () -> void = run(
    let result = catch(expr: divide(a: 1, b: 0)),
    assert_err(result: result),
)

// Good: Plugin systems
@run_plugin (plugin: Plugin) -> Result<void, str> =
    catch(expr: plugin.execute())

// Good: REPL environments
@eval_safely (code: str) -> Result<Value, str> =
    catch(expr: evaluate(code: code))
```

### catch vs Result

| Approach | Use Case |
|----------|----------|
| `Result` | Expected, recoverable errors |
| `catch` | Isolating untrusted code, test frameworks |

## The for Pattern (Advanced)

The `for` pattern has an advanced form:

```ori
for(
    over: items,
    match: pattern,
    default: fallback,
)
```

### With Pattern Matching

```ori
@extract_names (data: [Option<User>]) -> [str] =
    for(
        over: data,
        match: Some(user) -> user.name,
        default: continue,
    )
```

### With Map Function

```ori
@process_all (items: [int]) -> [int] =
    for(
        over: items,
        map: x -> x * 2,
    )
```

## Combining Patterns

### run with recurse

```ori
@tree_sum<T: Addable> (node: TreeNode<T>) -> T = run(
    pre_check: !is_null(node: node),
    recurse(
        condition: is_leaf(node: node),
        base: node.value,
        step: node.value + self(node: node.left) + self(node: node.right),
    ),
)
```

### try with cache

```ori
@fetch_cached (url: str) -> Result<str, Error> uses Http, Cache = try(
    let data = cache(
        key: `fetch:{url}`,
        op: Http.get(url: url)?,
        ttl: 10m,
    ),
    Ok(data),
)
```

### with and try

```ori
@safe_file_op (path: str) -> Result<Data, Error> uses FileSystem = try(
    let result = with(
        acquire: FileSystem.open(path: path)?,
        use: file -> parse_data(content: FileSystem.read(file: file)?),
        release: file -> FileSystem.close(file: file),
    ),
    Ok(result),
)
```

## Complete Example

```ori
type Config = {
    database_url: str,
    cache_ttl: Duration,
    max_retries: int,
}

type User = { id: int, name: str, email: str }

// Load config with validation
@load_config (path: str) -> Result<Config, Error> uses FileSystem = try(
    let content = FileSystem.read(path: path)?,
    let config = parse_config(data: content)?,

    // Validate with contracts
    run(
        pre_check: config.max_retries > 0 | "max_retries must be positive",
        pre_check: config.cache_ttl > 0s | "cache_ttl must be positive",
        Ok(config),
    ),
)

@test_load_config tests @load_config () -> void =
    with FileSystem = MockFileSystem {
        files: {
            "config.json": `{"database_url": "...", "cache_ttl": "5m", "max_retries": 3}`,
        },
    } in run(
        let result = load_config(path: "config.json"),
        assert_ok(result: result),
    )

// Fetch user with caching
@get_user (id: int) -> Result<User, Error> uses Http, Cache =
    cache(
        key: `user:{id}`,
        op: Http.get(url: `/api/users/{id}`),
        ttl: 5m,
    )

// Recursive data processing
@flatten_tree<T> (node: TreeNode<T>) -> [T] = recurse(
    condition: is_leaf(node: node),
    base: [node.value],
    step: [node.value, ...self(node: node.left), ...self(node: node.right)],
)

// Resource-safe database operation
@with_connection<T> (
    url: str,
    op: (Connection) -> Result<T, Error>,
) -> Result<T, Error> uses Database =
    with(
        acquire: Database.connect(url: url),
        use: conn -> op(conn),
        release: conn -> conn.close(),
    )

// Combining multiple patterns
@process_users (ids: [int]) -> Result<[User], Error>
    uses Http, Cache, Logger, Async = try(
    let users = parallel(
        tasks: for id in ids yield () -> run(
            Logger.debug(msg: `Fetching user {id}`),
            get_user(id: id),
        ),
        max_concurrent: 10,
        timeout: 30s,
    ),

    // Extract successful results
    let valid_users = for result in users
        if is_ok(result: result)
        yield match(
            result,
            Ok(user) -> user,
            Err(_) -> continue,
        ),

    Ok(valid_users),
)

@test_process_users tests @process_users () -> void =
    with Http = MockHttp {
        responses: {
            "/api/users/1": `{"id": 1, "name": "Alice", "email": "a@test.com"}`,
            "/api/users/2": `{"id": 2, "name": "Bob", "email": "b@test.com"}`,
        },
    },
    Cache = MockCache {},
    Logger = MockLogger {} in run(
        let result = process_users(ids: [1, 2]),
        assert_ok(result: result),
        match(
            result,
            Ok(users) -> assert_eq(actual: len(collection: users), expected: 2),
            Err(_) -> panic(msg: "Expected Ok"),
        ),
    )
```

## Quick Reference

### run

```ori
run(
    let a = ...,
    let b = ...,
    result,
)

run(
    pre_check: condition | "error message",
    body,
    post_check: result -> condition,
)
```

### try

```ori
try(
    let a = fallible()?,
    let b = fallible()?,
    Ok(result),
)
```

### match

```ori
match(
    value,
    Pattern1 -> result1,
    Pattern2 -> result2,
    _ -> default,
)
```

### recurse

```ori
recurse(
    condition: base_case_condition,
    base: base_case_value,
    step: self(...) + self(...),
    memo: true,
    parallel: threshold,
)
```

### cache

```ori
cache(
    key: "cache_key",
    op: expensive_computation,
    ttl: 5m,
)
```

### with

```ori
with(
    acquire: get_resource,
    use: resource -> use_resource(r: resource),
    release: resource -> cleanup(r: resource),
)
```

### catch

```ori
catch(expr: might_panic()) -> Result<T, str>
```

## What's Next

Now that you understand compiler patterns:

- **[Memory Model](/guide/20-memory-model)** — Understanding ARC
- **[Formatting Rules](/guide/21-formatting)** — Code style guidelines

