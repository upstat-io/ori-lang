---
title: "Patterns"
description: "Ori Language Specification — Patterns"
order: 10
---

# Patterns

Compiler-level control flow and concurrency constructs.

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § PATTERNS

## Categories

| Category | Patterns | Purpose |
|----------|----------|---------|
| `function_seq` | `run`, `try`, `match` | Sequential expressions |
| `function_exp` | `recurse`, `parallel`, `spawn`, `timeout`, `cache`, `with`, `for`, `catch` | Concurrency, recursion, resources, error recovery |
| `function_val` | `int`, `float`, `str`, `byte` | Type conversion |

> **Note:** Data transformation (`map`, `filter`, `fold`, `find`, `collect`) and resilience (`retry`, `validate`) are stdlib methods, not compiler patterns. See [Built-in Functions](11-built-in-functions.md).

## Sequential (function_seq)

### run

```ori
run(
    let x = compute(),
    let y = transform(x),
    x + y,
)
```

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

### parallel

Execute tasks, wait for all to settle.

```ori
parallel(
    tasks: [get_user(id), get_posts(id)],
    max_concurrent: 10,
    timeout: 5s,
)
```

Returns `[Result<T, E>]`. Never fails; errors captured in results.

### spawn

Fire and forget.

```ori
spawn(tasks: [send_email(u) for u in users])
```

Returns `void`. Errors discarded.

### timeout

```ori
timeout(op: fetch(url), after: 5s)
```

Returns `Result<T, TimeoutError>`.

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
