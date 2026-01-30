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

## Recursion (function_exp)

### recurse

```ori
recurse(
    condition: n <= 1,
    base: n,
    step: self(n - 1) + self(n - 2),
    memo: true,
)
```

`self(...)` calls recursively. `memo: true` caches results for call duration.

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

Fire and forget. Creates one task per list element.

```ori
spawn(tasks: [send_email(u) for u in users])
```

Returns `void`. Errors discarded.

### timeout

```ori
timeout(op: fetch(url), after: 5s)
```

Returns `Result<T, TimeoutError>`.

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
    @spawn<T> (self, task: () -> T uses Async) -> void
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

```ori
cache(key: url, op: fetch(url), ttl: 5m)
```

Requires `Cache` capability.

### with

Resource management.

```ori
with(
    acquire: open_file(path),
    use: f -> read_all(f),
    release: f -> close(f),
)
```

`release` always runs.

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

For `for...yield`:

```ori
// This:
for x in items yield x * 2

// Desugars to:
items.iter().map(transform: x -> x * 2).collect()
```

See [Types § Iterator Traits](06-types.md#iterator-traits) for trait definitions.
