---
title: "Patterns"
description: "Ori Language Specification — Patterns"
order: 10
section: "Expressions"
---

# Patterns

Compiler-level control flow and concurrency constructs.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § PATTERNS

## Categories

| Category | Patterns | Purpose |
|----------|----------|---------|
| `function_seq` | `run`, `try`, `match` | Sequential expressions |
| `function_exp` | `recurse`, `parallel`, `spawn`, `timeout`, `cache`, `with`, `for`, `catch` | Concurrency, recursion, resources, error recovery |
| `function_val` | `int`, `float`, `str`, `byte` | Type conversion |

> **Note:** Data transformation (`map`, `filter`, `fold`, `find`, `collect`) and resilience (`retry`, `validate`) are stdlib methods, not compiler patterns. See [Built-in Functions](11-built-in-functions.md).

## Sequential (function_seq)

### run

Sequential expressions with optional pre/post checks.

```ori
run(
    let x = compute(),
    let y = transform(x),
    x + y,
)
```

#### Pre/Post Checks

The `run` pattern supports `pre_check:` and `post_check:` properties for contract-style defensive programming:

```ori
@divide (a: int, b: int) -> int = run(
    pre_check: b != 0,
    a div b,
    post_check: r -> r * b <= a
)

// Multiple conditions via multiple properties
@transfer (from: Account, to: Account, amount: int) -> (Account, Account) = run(
    pre_check: amount > 0 | "amount must be positive",
    pre_check: from.balance >= amount | "insufficient funds",
    let new_from = Account { balance: from.balance - amount, ..from },
    let new_to = Account { balance: to.balance + amount, ..to },
    (new_from, new_to),
    post_check: (f, t) -> f.balance + t.balance == from.balance + to.balance,
)
```

**Positional constraints** (parser-enforced):
- `pre_check:` must appear before any body bindings or expressions
- `post_check:` must appear after the final body expression

**Semantics**:
1. Evaluate all `pre_check:` conditions in order; panic on failure
2. Execute body statements and final expression
3. Bind result to each `post_check:` lambda parameter
4. Evaluate all `post_check:` conditions in order; panic on failure
5. Return result

**Scope constraints**:
- `pre_check:` may only reference bindings visible in the enclosing scope
- `post_check:` may reference the result, enclosing scope bindings, and body bindings

**Type constraints**:
- `pre_check:` condition must have type `bool`
- `post_check:` must be a lambda from result type to `bool`
- It is a compile-time error to use `post_check:` when the body evaluates to `void`

**Custom messages**: Use `condition | "message"` to provide a custom panic message. Without a message, the compiler embeds the condition's source text.

### try

Error-propagating sequence. Returns early on `Err`.

```ori
try(
    let content = read_file(path),
    let parsed = parse(content),
    Ok(transform(parsed)),
)
```

### match

```ori
match(status,
    Pending -> "waiting",
    Running(p) -> str(p) + "%",
    x.match(x > 0) -> "positive",
    _ -> "other",
)
```

Match patterns include: literals, identifiers, wildcards (`_`), variant patterns, struct patterns, list patterns with rest (`..`), or-patterns (`|`), at-patterns (`@`), and range patterns.

Match must be exhaustive.

#### Exhaustiveness Checking

A match expression is _exhaustive_ if every possible value of the scrutinee type matches at least one pattern arm. The compiler uses pattern matrix decomposition to verify exhaustiveness.

For each type, the compiler knows its constructors:
- `bool`: `true`, `false`
- `Option<T>`: `Some(_)`, `None`
- `Result<T, E>`: `Ok(_)`, `Err(_)`
- Sum types: all declared variants
- Integers: infinite (requires wildcard)
- Strings: infinite (requires wildcard)

**Never variants:** Variants containing `Never` are uninhabited and need not be matched:

```ori
type MaybeNever = Value(int) | Impossible(Never)

match(maybe,
    Value(v) -> v,
    // Impossible case can be omitted — it can never occur
)
```

Matching `Never` explicitly is permitted but the arm is unreachable.

Non-exhaustiveness is a compile-time error. There is no partial match construct.

| Context | Non-Exhaustive | Rationale |
|---------|---------------|-----------|
| `match` expression | Error | Must handle all cases to return a value |
| `let` binding destructure | Error | Must match to bind |
| Function clause patterns | Error | All clauses together must be exhaustive |

#### Pattern Refutability

An _irrefutable pattern_ always matches. A _refutable pattern_ may fail to match.

Irrefutable patterns:
- Wildcard (`_`)
- Variable binding (`x`)
- Struct with all irrefutable fields (`Point { x, y }`)
- Tuple with all irrefutable elements (`(a, b)`)

Refutable patterns:
- Literals (`42`, `"hello"`)
- Variants (`Some(x)`, `None`)
- Ranges (`0..10`)
- Lists with length (`[a, b]`)
- Guards (`x.match(x > 0)`)

| Context | Requirement |
|---------|-------------|
| `match` arm | Any pattern (refutable OK) |
| `let` binding | Must be irrefutable |
| Function parameter | Must be irrefutable |
| `for` loop variable | Must be irrefutable |

#### Guards and Exhaustiveness

Guards are not considered for exhaustiveness checking. The compiler cannot statically verify guard conditions. A match with guards must include a catch-all pattern:

```ori
// ERROR: guards require catch-all
match(n,
    x.match(x > 0) -> "positive",
    x.match(x < 0) -> "negative",
    // Error: patterns not exhaustive due to guards
)

// OK: catch-all ensures exhaustiveness
match(n,
    x.match(x > 0) -> "positive",
    x.match(x < 0) -> "negative",
    _ -> "zero",
)
```

#### Or-Pattern Exhaustiveness

Or-patterns contribute their combined coverage:

```ori
type Light = Red | Yellow | Green

// Exhaustive via or-pattern
match(light,
    Red | Yellow -> "stop",
    Green -> "go",
)
```

Bindings in or-patterns must appear in all alternatives with the same type.

#### At-Pattern Exhaustiveness

At-patterns contribute the same coverage as their inner pattern:

```ori
match(opt,
    whole @ Some(x) -> use_both(whole, x),
    None -> default,  // Required for exhaustiveness
)
```

#### List Pattern Exhaustiveness

List patterns match by length:

| Pattern | Matches |
|---------|---------|
| `[]` | Empty list only |
| `[x]` | Exactly one element |
| `[x, y]` | Exactly two elements |
| `[x, ..rest]` | One or more elements |
| `[..rest]` | Any list (including empty) |

To be exhaustive, patterns must cover all lengths.

#### Range Pattern Exhaustiveness

Integer ranges cannot be exhaustive without a wildcard (infinite domain). The compiler warns about overlapping ranges.

#### Unreachable Patterns

The compiler warns about patterns that can never match due to earlier patterns covering all their cases.

## Recursion (function_exp)

### recurse

The `recurse` pattern evaluates recursive computations with optional memoization and parallelism.

```ori
recurse(
    condition: bool_expr,
    base: expr,
    step: expr_with_self,
    memo: bool = false,
    parallel: bool = false,
)
```

#### Evaluation

1. Evaluate `condition`
2. If true: return `base` expression
3. If false: evaluate `step` expression (which may contain `self(...)` calls)

```ori
@factorial (n: int) -> int = recurse(
    condition: n <= 1,
    base: 1,
    step: n * self(n - 1),
)
```

#### Self Keyword

`self(...)` within `step` represents a recursive invocation:

```ori
@fibonacci (n: int) -> int = recurse(
    condition: n <= 1,
    base: n,
    step: self(n - 1) + self(n - 2),
)
```

Arguments to `self(...)` must match the enclosing function's parameter arity.

#### Self Scoping

Within a `recurse` expression:
- `self` (without parentheses) — trait method receiver (if applicable)
- `self(...)` (with arguments) — recursive call

These coexist when `recurse` appears in a trait method:

```ori
impl Tree for TreeOps {
    @depth (self) -> int = recurse(
        condition: self.is_leaf(),  // Receiver
        base: 1,
        step: 1 + max(left: self(self.left()), right: self(self.right())),  // Recursive calls
    )
}
```

It is a compile-time error to use `self(...)` outside of a `recurse` step expression.

#### Memoization

With `memo: true`, results are cached for the duration of the top-level call:

```ori
@fib (n: int) -> int = recurse(
    condition: n <= 1,
    base: n,
    step: self(n - 1) + self(n - 2),
    memo: true,  // O(n) instead of O(2^n)
)
```

Memo requirements:
- All parameters must be `Hashable + Eq`
- Return type must be `Clone`

The cache is created at top-level entry, shared across recursive calls, and discarded when the top-level call returns.

#### Parallel Recursion

With `parallel: true`, independent `self(...)` calls execute concurrently:

```ori
@parallel_fib (n: int) -> int uses Suspend = recurse(
    condition: n <= 1,
    base: n,
    step: self(n - 1) + self(n - 2),
    parallel: true,
)
```

Parallel requirements:
- Requires `uses Suspend` capability
- Captured values must be `Sendable`
- Return type must be `Sendable`

When `memo: true` and `parallel: true` are combined, the memo cache is thread-safe. If multiple tasks request the same key simultaneously, one computes while others wait.

#### Tail Call Optimization

When `self(...)` is in tail position, the compiler optimizes to a loop with O(1) stack space:

```ori
@sum_to (n: int, acc: int = 0) -> int = recurse(
    condition: n == 0,
    base: acc,
    step: self(n - 1, acc + n),  // Tail position: compiled to loop
)
```

#### Stack Limits

Non-tail recursive calls are limited to a depth of 1000. Exceeding this limit causes a panic. Tail-optimized recursion bypasses this limit.

## Concurrency

Concurrency patterns create tasks. See [Concurrency Model](23-concurrency-model.md) for task definitions, async context semantics, and capture rules.

### parallel

Execute tasks, wait for all to settle. Creates one task per list element.

```ori
parallel(
    tasks: [() -> T uses Async],
    max_concurrent: Option<int> = None,
    timeout: Option<Duration> = None,
) -> [Result<T, E>]
```

Returns `[Result<T, E>]`. Never fails; errors captured in results.

#### Execution Order Guarantees

| Aspect | Guarantee |
|--------|-----------|
| Start order | Tasks start in list order |
| Completion order | Any order (concurrent execution) |
| Result order | Same as task list order |

```ori
let results = parallel(tasks: [slow, fast, medium])
// results[0] = result of slow  (first task)
// results[1] = result of fast  (second task)
// results[2] = result of medium (third task)
// Even if fast completed first
```

#### Concurrency Limits

When `max_concurrent` is `Some(n)`:
- At most `n` tasks run simultaneously
- Tasks are queued in list order
- When one completes, the next queued task starts

When `max_concurrent` is `None` (default), all tasks may run simultaneously.

#### Timeout Behavior

When `timeout` expires:
1. Incomplete tasks are marked for cancellation
2. Tasks reach cancellation checkpoints and terminate
3. Cancelled tasks return `Err(CancellationError { reason: Timeout, task_id: n })`
4. Completed results are preserved

Tasks can cooperatively check for cancellation using `is_cancelled()`.

#### Resource Exhaustion

If the runtime cannot allocate resources for a task:
- The task returns `Err(CancellationError { reason: ResourceExhausted, task_id: n })`
- Other tasks continue executing
- The pattern does NOT panic

#### Error Handling

Errors do not stop other tasks. All tasks run to completion (equivalent to `CollectAll` behavior).

For early termination on error, use `nursery` with `on_error: FailFast`.

#### Empty Task List

`parallel(tasks: [])` returns `[]` immediately.

See [nursery](#nursery) for cancellation semantics and `CancellationError` type.

### spawn

Fire-and-forget task execution. Creates one task per list element.

```ori
spawn(
    tasks: [() -> T uses Suspend],
    max_concurrent: Option<int> = None,
)
```

Returns `void` immediately. Task results and errors are discarded.

#### Fire-and-Forget Semantics

Tasks start and the pattern returns without waiting:

```ori
spawn(tasks: [send_email(u) for u in users])
// Returns void immediately
// Emails sent in background
```

Errors in spawned tasks are silently discarded. To log errors, handle them within the task:

```ori
spawn(tasks: [
    () -> match(risky_operation(),
        Ok(_) -> log(msg: "success"),
        Err(e) -> log(msg: `failed: {e}`),
    ),
])
```

#### Task Lifetime

Spawned tasks:
- Run independently of the spawning scope
- May outlive the spawning function (true fire-and-forget)
- Complete naturally, are cancelled on program exit, or terminate on panic

> **Note:** `spawn` is the ONLY concurrency pattern that allows tasks to escape their spawning scope. Unlike `parallel` and `nursery`, which guarantee all tasks complete before the pattern returns, `spawn` tasks are managed by the runtime. For structured concurrency with guaranteed completion, use `nursery`.

```ori
@setup () -> void uses Suspend = run(
    spawn(tasks: [background_monitor()]),
    // Function returns, but monitor continues
)
```

#### Concurrency Control

When `max_concurrent` is `Some(n)`, at most `n` tasks run simultaneously. When `None` (default), all tasks may start simultaneously.

#### Resource Exhaustion

If the runtime cannot allocate resources for a task:
- The task is dropped
- No error is surfaced (fire-and-forget semantics)
- Other tasks continue

### timeout

Bounded execution time for an operation.

```ori
timeout(
    op: expression,
    after: Duration,
) -> Result<T, CancellationError>
```

#### Basic Behavior

1. Start executing `op`
2. If `op` completes before `after`: return `Ok(result)`
3. If `after` elapses first: cancel `op`, return `Err(CancellationError { reason: Timeout, task_id: 0 })`

```ori
let result = timeout(op: fetch(url), after: 5s)
// result: Result<Response, CancellationError>
```

#### Cancellation

When timeout expires, the operation is cooperatively cancelled using the same cancellation model as `nursery`:

1. Operation is marked for cancellation
2. At the next cancellation checkpoint, operation terminates
3. Destructors run during unwinding
4. `Err(CancellationError { reason: Timeout, task_id: 0 })` is returned

Cancellation checkpoints:
- Suspending function calls (functions with `uses Suspend`)
- Loop iterations
- Pattern entry (`run`, `try`, `match`, etc.)

CPU-bound operations without checkpoints cannot be cancelled until they reach one.

#### Nested Timeout

Inner timeouts can be shorter than outer:

```ori
timeout(
    op: run(
        let a = timeout(op: step1(), after: 2s)?,
        let b = timeout(op: step2(), after: 2s)?,
        (a, b),
    ),
    after: 5s,  // Overall timeout
)
```

### nursery

Structured concurrency with guaranteed task completion. Creates tasks via `n.spawn()`.

```ori
nursery(
    body: n -> for item in items do n.spawn(task: () -> process(item)),
    on_error: CollectAll,
    timeout: 30s,
)
```

| Parameter | Type | Description |
|-----------|------|-------------|
| `body` | `Nursery -> T` | Lambda that spawns tasks |
| `on_error` | `NurseryErrorMode` | Error handling mode |
| `timeout` | `Duration` | Maximum time (optional) |

Returns `[Result<T, E>]`. All spawned tasks complete before nursery exits.

The `Nursery` type provides a single method:

```ori
type Nursery = {
    @spawn<T> (self, task: () -> T uses Suspend) -> void
}
```

Error modes:

```ori
type NurseryErrorMode = CancelRemaining | CollectAll | FailFast
```

| Mode | Behavior |
|------|----------|
| `CancelRemaining` | On first error, cancel pending tasks; running tasks continue |
| `CollectAll` | Wait for all tasks regardless of errors (no cancellation) |
| `FailFast` | On first error, cancel all tasks immediately |

Guarantees:
- No orphan tasks — all spawned tasks complete or cancel
- Error propagation — task failures captured in results
- Scoped concurrency — tasks cannot escape nursery scope

#### Cancellation Model

Ori uses **cooperative cancellation**. A cancelled task:
1. Is marked for cancellation
2. Continues executing until it reaches a cancellation checkpoint
3. At the checkpoint, terminates with `CancellationError`
4. Runs cleanup/destructors during termination

Cancellation checkpoints:
- Suspension points (async calls, channel operations)
- Loop iterations (start of each `for` or `loop` iteration)
- Pattern entry (`run`, `try`, `match`, `parallel`, `nursery`)

#### Cancellation Types

```ori
type CancellationError = {
    reason: CancellationReason,
    task_id: int,
}

type CancellationReason =
    | Timeout
    | SiblingFailed
    | NurseryExited
    | ExplicitCancel
    | ResourceExhausted
```

#### Cancellation API

The `is_cancelled()` built-in function returns `bool`, available in async contexts:

```ori
@long_task () -> Result<Data, Error> uses Async = run(
    for item in items do run(
        if is_cancelled() then break Err(CancellationError { ... }),
        process(item),
    ),
    Ok(result),
)
```

The `for` loop automatically checks cancellation at each iteration when inside an async context.

#### Cleanup Guarantees

When a task is cancelled:
1. Stack unwinding occurs from the cancellation checkpoint
2. Destructors run for all values in scope
3. Cleanup is guaranteed to complete before task terminates

## Resource Management (function_exp)

### cache

Memoization with TTL-based expiration. Requires `Cache` capability.

```ori
cache(
    key: expression,
    op: expression,
    ttl: Duration,
)
```

#### Semantics

1. Compute `key` expression
2. Check cache for existing unexpired entry
3. If hit: return cached value (clone)
4. If miss: evaluate `op`, store result, return it

```ori
@fetch_user (id: int) -> User uses Cache =
    cache(
        key: `user-{id}`,
        op: db.query(id: id),
        ttl: 5m,
    )
```

The `cache` pattern returns the same type as `op`.

#### Key Requirements

Keys must implement `Hashable` and `Eq`:

```ori
cache(key: "string-key", op: ..., ttl: 1m)  // OK: str is Hashable + Eq
cache(key: 42, op: ..., ttl: 1m)            // OK: int is Hashable + Eq
cache(key: (user_id, "profile"), op: ..., ttl: 1m)  // OK: tuple of hashables
```

#### Value Requirements

Cached values must implement `Clone`. The cache returns a clone of the stored value.

#### TTL Behavior

| TTL | Behavior |
|-----|----------|
| Positive | Entry expires after TTL from creation |
| Zero | No caching (always recompute) |
| Negative | Compile error (E0992) |

#### Concurrent Access

When multiple tasks request the same key simultaneously (stampede prevention):

1. First request computes the value
2. Other requests wait for computation
3. All receive the same result

If `op` fails during stampede, waiting requests also receive the error. Failed results are NOT cached.

#### Error Handling

If `op` returns `Err` or panics, the result is NOT cached. To cache error results, wrap in a non-error type:

```ori
cache(
    key: url,
    op: match(fetch(url), r -> r),  // Cache the Result itself
    ttl: 5m,
)
```

#### Invalidation

Time-based expiration is automatic. Manual invalidation uses `Cache` capability methods:

```ori
@invalidate_user (id: int) -> void uses Cache =
    Cache.invalidate(key: `user-{id}`)

@clear_all_cache () -> void uses Cache =
    Cache.clear()
```

#### Cache vs Memoization

The `cache` pattern and `recurse(..., memo: true)` serve different purposes:

| Aspect | `cache(...)` | `recurse(..., memo: true)` |
|--------|--------------|---------------------------|
| Persistence | TTL-based, may persist across calls | Call-duration only |
| Capability | Requires `Cache` | Pure, no capability |
| Scope | Shared across function calls | Private to single recurse |
| Use case | API responses, config, expensive I/O | Pure recursive algorithms |

#### Error Codes

| Code | Description |
|------|-------------|
| E0990 | Cache key must be `Hashable` |
| E0991 | `cache` requires `Cache` capability |
| E0992 | TTL must be non-negative |

### with

Resource management with guaranteed cleanup.

```ori
with(
    acquire: open_file(path),
    action: f -> read_all(f),
    release: f -> close(f),
)
```

> **Note:** The property is named `action:` because `use` is a reserved keyword.

#### Semantics

1. Evaluate `acquire:` to obtain resource
2. If `acquire` fails (returns `Err` or panics), stop—no cleanup needed
3. If `acquire` succeeds, bind result to `action:` parameter
4. Evaluate `action:` expression
5. Always evaluate `release:` with the resource, regardless of how `action:` completed

The pattern returns the result of `action:`.

#### Release Guarantee

If `acquire:` succeeds, `release:` runs under all exit conditions:

| Exit Condition | Release Runs |
|----------------|--------------|
| Normal completion | Yes |
| Panic during action | Yes |
| Error propagation (`?`) | Yes |
| `break` in action | Yes |
| `continue` in action | Yes |

```ori
with(
    acquire: open_file(path),
    action: f -> run(
        if bad_condition then panic("abort"),
        if err_condition then Err("failed")?,
        read_all(f),
    ),
    release: f -> close(f),  // Always called
)
```

#### Type Constraints

The `release:` expression must return `void`:

```ori
// OK: release returns void
with(acquire: lock(), action: l -> work(), release: l -> l.unlock())

// Error E0861: release must return void
with(acquire: lock(), action: l -> work(), release: l -> l.count())
```

#### Result Types

When `acquire:` returns `Result<R, E>`:

```ori
@open_file (path: str) -> Result<File, IoError>

// Using `?` for fallible acquire:
@read_config (path: str) -> Result<Config, IoError> =
    with(
        acquire: open_file(path)?,  // Propagates Err, no cleanup needed
        action: f -> parse(f.read_all()),
        release: f -> f.close(),
    )
```

When `action:` may fail:

```ori
@process_file (path: str) -> Result<Data, Error> uses FileSystem =
    with(
        acquire: open_file(path)?,
        action: f -> run(
            let content = f.read_all()?,  // May propagate Err
            parse(content)?,              // May propagate Err
        ),
        release: f -> f.close(),  // Still runs on any Err
    )
```

#### Double Fault Abort

If `release:` panics while already unwinding (e.g., after `action:` panicked), the program aborts immediately:

- The `@panic` handler is NOT called
- Both panic messages are shown
- Exit code is non-zero

This prevents cascading failures during cleanup.

#### Suspending Context

In a suspending context (`uses Suspend`), both `action:` and `release:` may suspend:

```ori
@with_connection (url: str) -> Data uses Suspend =
    with(
        acquire: connect(url),
        action: conn -> fetch_data(conn),  // May suspend
        release: conn -> conn.close(),     // May suspend
    )
```

#### Error Codes

| Code | Description |
|------|-------------|
| E0860 | `with` pattern missing required parameter (`acquire:`, `action:`, or `release:`) |
| E0861 | `release` must return `void` |

## Error Recovery (function_exp)

### catch

Captures panics and converts them to `Result<T, str>`.

```ori
catch(expr: may_panic())
```

If the expression evaluates successfully, returns `Ok(value)`. If the expression panics, returns `Err(message)` where `message` is the panic message string.

See [Errors and Panics § Catching Panics](20-errors-and-panics.md#catching-panics).

## for Pattern

```ori
for(over: items, match: Some(x) -> x, default: 0)
for(over: items, map: parse, match: Ok(v) -> v, default: fallback)
```

Returns first match or default.

## For Loop Desugaring

The `for` loop desugars to use the `Iterable` and `Iterator` traits:

```ori
// This:
for x in items do
    process(x: x)

// Desugars to:
run(
    let iter = items.iter(),
    loop(
        match(
            iter.next(),
            (Some(x), next_iter) -> run(
                process(x: x),
                iter = next_iter,
                continue,
            ),
            (None, _) -> break,
        ),
    ),
)
```

## For-Yield Comprehensions

The `for...yield` expression builds collections from iteration.

### Basic Syntax

```ori
for x in items yield expression
```

Desugars to:

```ori
items.iter().map(transform: x -> x * 2).collect()
```

### Type Inference

The result type is inferred from context:

```ori
let numbers: [int] = for x in items yield x.id  // [int]
let set: Set<str> = for x in items yield x.name  // Set<str>
```

Without context, defaults to list:

```ori
let result = for x in 0..5 yield x * 2  // [int]
```

### Filtering

A single `if` clause filters elements. Use `&&` for multiple conditions:

```ori
for x in items if x > 0 yield x
for x in items if x > 0 && x < 100 yield x
```

Desugars to:

```ori
items.iter().filter(predicate: x -> x > 0).map(transform: x -> x).collect()
```

### Nested Comprehensions

Multiple `for` clauses produce a flat result:

```ori
for x in xs for y in ys yield (x, y)
```

Desugars to:

```ori
xs.iter().flat_map(transform: x -> ys.iter().map(transform: y -> (x, y))).collect()
```

Each clause can have its own filter:

```ori
for x in xs if x > 0 for y in ys if y > 0 yield x * y
```

### Break and Continue

In yield context, `break` and `continue` control collection building:

| Statement | Effect |
|-----------|--------|
| `continue` | Skip element, add nothing |
| `continue value` | Add `value` instead of yield expression |
| `break` | Stop iteration, return results so far |
| `break value` | Add `value` and stop |

```ori
for x in items yield
    if skip(x) then continue,
    if done(x) then break x,
    transform(x),
```

### Map Collection

Maps implement `Collect<(K, V)>`. Yielding 2-tuples collects into a map:

```ori
let by_id: {int: User} = for user in users yield (user.id, user)
```

If duplicate keys are yielded, later values overwrite earlier ones.

### Empty Results

Empty source or all elements filtered produces an empty collection:

```ori
for x in [] yield x * 2  // []
for x in [1, 2, 3] if x > 10 yield x  // []
```

See [Types § Iterator Traits](06-types.md#iterator-traits) for trait definitions.
