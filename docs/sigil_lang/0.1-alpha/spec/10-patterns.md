# Patterns

Built-in control flow and data transformation constructs.

## Categories

| Category | Syntax | Purpose |
|----------|--------|---------|
| `function_seq` | `run`, `try`, `match` | Sequential expressions |
| `function_exp` | Named args (`name: expr`) | Data transformation |
| `function_val` | `int`, `float`, `str`, `byte` | Type conversion |

## Grammar

```
pattern_expr   = function_seq | function_exp | function_val .
function_seq   = run_expr | try_expr | match_expr .
function_exp   = pattern_name "(" named_arg { "," named_arg } ")" .
function_val   = ( "int" | "float" | "str" | "byte" ) "(" expression ")" .
named_arg      = identifier ":" expression .
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

## Data Transformation (function_exp)

### map

```sigil
map(over: items, transform: x -> x * 2)
```

`[T]` × `(T -> U)` → `[U]`

### filter

```sigil
filter(over: items, predicate: x -> x > 0)
```

`[T]` × `(T -> bool)` → `[T]`

### fold

```sigil
fold(over: items, initial: 0, operation: (acc, x) -> acc + x)
```

`[T]` × `U` × `((U, T) -> U)` → `U`

### collect

```sigil
collect(range: 1..=10, transform: n -> n * n)
```

`Range` × `(int -> T)` → `[T]`

### find

```sigil
find(over: items, where: x -> x > 0)
find(over: items, where: predicate, default: fallback)
find(over: items, map: x -> parse(x))  // find_map variant
```

Returns `Option<T>` without default, `T` with default.

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

## Resilience

### retry

```sigil
retry(op: fetch(url), attempts: 3, backoff: exponential(base: 100ms))
```

### cache

```sigil
cache(key: url, op: fetch(url), ttl: 5m)
```

Requires `Cache` capability.

### validate

```sigil
validate(
    rules: [
        age >= 0 | "age must be non-negative",
        name != "" | "name required",
    ],
    then: User { name, age },
)
```

Returns `Result<T, [str]>`.

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
