# Patterns

Compiler-level control flow and concurrency constructs.

## Categories

| Category | Patterns | Purpose |
|----------|----------|---------|
| `function_seq` | `run`, `try`, `match` | Sequential expressions |
| `function_exp` | `recurse`, `parallel`, `spawn`, `timeout`, `cache`, `with`, `for` | Concurrency, recursion, resources |
| `function_val` | `int`, `float`, `str`, `byte` | Type conversion |

> **Note:** Data transformation (`map`, `filter`, `fold`, `find`, `collect`) and resilience (`retry`, `validate`) are stdlib methods, not compiler patterns. See [Built-in Functions](11-built-in-functions.md).

## Grammar

```
pattern_expr   = function_seq | function_exp | function_val .
function_seq   = run_expr | try_expr | match_expr | for_pattern .
function_exp   = pattern_name "(" named_arg { "," named_arg } ")" .
function_val   = ( "int" | "float" | "str" | "byte" ) "(" expression ")" .
named_arg      = identifier ":" expression .
pattern_name   = "recurse" | "parallel" | "spawn" | "timeout" | "cache" | "with" .
```

## Sequential (function_seq)

### run

```
run_expr = "run" "(" { binding "," } expression ")" .
```

```sigil
run(
    let x = compute(),
    let y = transform(x),
    x + y,
)
```

### try

Error-propagating sequence. Returns early on `Err`.

```sigil
try(
    let content = read_file(path),
    let parsed = parse(content),
    Ok(transform(parsed)),
)
```

### match

```
match_expr = "match" "(" expression "," match_arm { "," match_arm } ")" .
match_arm  = pattern [ guard ] "->" expression .
guard      = ".match" "(" expression ")" .
```

```sigil
match(status,
    Pending -> "waiting",
    Running(p) -> str(p) + "%",
    x.match(x > 0) -> "positive",
    _ -> "other",
)
```

Match patterns:

```
pattern = literal | identifier | "_"
        | type_path [ "(" pattern { "," pattern } ")" ]
        | "{" [ field_pattern { "," field_pattern } ] [ ".." ] "}"
        | "[" [ pattern { "," pattern } [ ".." identifier ] ] "]"
        | pattern "|" pattern
        | identifier "@" pattern
        | literal ".." [ "=" ] literal .
```

Match must be exhaustive.

## Recursion (function_exp)

### recurse

```sigil
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

```sigil
parallel(
    tasks: [get_user(id), get_posts(id)],
    max_concurrent: 10,
    timeout: 5s,
)
```

Returns `[Result<T, E>]`. Never fails; errors captured in results.

### spawn

Fire and forget.

```sigil
spawn(tasks: [send_email(u) for u in users])
```

Returns `void`. Errors discarded.

### timeout

```sigil
timeout(op: fetch(url), after: 5s)
```

Returns `Result<T, TimeoutError>`.

## Resource Management (function_exp)

### cache

```sigil
cache(key: url, op: fetch(url), ttl: 5m)
```

Requires `Cache` capability.

### with

Resource management.

```sigil
with(
    acquire: open_file(path),
    use: f -> read_all(f),
    release: f -> close(f),
)
```

`release` always runs.

## for Pattern

```sigil
for(over: items, match: Some(x) -> x, default: 0)
for(over: items, map: parse, match: Ok(v) -> v, default: fallback)
```

Returns first match or default.
