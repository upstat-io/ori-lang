# Patterns

This section defines built-in patterns and pattern matching.

## Pattern Expression Syntax

```
pattern_expr       = run_expr | try_expr | match_expr | data_pattern .
data_pattern       = pattern_name "(" named_args ")" .
pattern_name       = "map" | "filter" | "fold" | "recurse" | "collect" | "find"
                   | "parallel" | "retry" | "cache" | "validate" | "timeout" | "with" .
named_args         = named_arg { "," named_arg } [ "," ] .
named_arg          = "." identifier ":" expression .
```

## Sequential Execution

### run

The `run` pattern executes expressions sequentially with bindings.

```
run_expr           = "run" "(" { binding "," } expression ")" .
binding            = "let" [ "mut" ] identifier [ ":" type ] "=" expression .
```

**Semantics:**

1. Evaluate each binding in order
2. Each binding introduces a variable into scope for subsequent expressions
3. The final expression is the result

```sigil
run(
    let x = compute(),
    let y = transform(x),
    x + y,
)
```

### try

The `try` pattern executes expressions, propagating errors.

```
try_expr           = "try" "(" { binding "," } expression ")" .
```

**Semantics:**

1. Evaluate each binding in order
2. If any binding evaluates to `Err(e)`, return `Err(e)` immediately
3. Use `?` suffix to unwrap `Result` values within bindings
4. The final expression is the result (typically wrapped in `Ok`)

```sigil
try(
    let content = read_file(path)?,
    let parsed = parse(content)?,
    Ok(transform(parsed)),
)
```

## Pattern Matching

### match

The `match` pattern dispatches based on value patterns.

```
match_expr         = "match" "(" expression "," match_arms ")" .
match_arms         = match_arm { "," match_arm } [ "," ] .
match_arm          = pattern [ guard ] "->" expression .
guard              = "." "match" "(" expression ")" .
```

**Semantics:**

1. Evaluate the scrutinee expression
2. Test each arm's pattern in order
3. If pattern matches (and guard passes), evaluate that arm's expression
4. Return the result

```sigil
match(status,
    Pending -> "waiting",
    Running(p) -> "at " + str(p) + "%",
    Done -> "complete",
    Failed(e) -> "error: " + e,
)
```

### Match Patterns

```
pattern            = literal_pattern
                   | binding_pattern
                   | wildcard_pattern
                   | variant_pattern
                   | struct_pattern
                   | list_pattern
                   | range_pattern
                   | or_pattern
                   | at_pattern .

literal_pattern    = literal .
binding_pattern    = identifier .
wildcard_pattern   = "_" .
variant_pattern    = type_path [ "(" [ pattern { "," pattern } ] ")" ] .
struct_pattern     = "{" [ field_pattern { "," field_pattern } ] [ ".." ] "}" .
field_pattern      = identifier [ ":" pattern ] .
list_pattern       = "[" [ list_elem { "," list_elem } ] "]" .
list_elem          = pattern | ".." [ identifier ] .
range_pattern      = [ literal ] ".." [ literal ] | [ literal ] "..=" literal .
or_pattern         = pattern "|" pattern .
at_pattern         = identifier "@" pattern .
```

### Pattern Guards

```sigil
match(n,
    x.match(x > 0 && x < 100) -> "in range",
    _ -> "out of range",
)
```

The guard expression must evaluate to `bool`. Variables bound by the pattern are in scope.

### Exhaustiveness

Match expressions must be exhaustive. It is an error if any possible value of the scrutinee type is not covered by some arm.

## Data Transformation Patterns

### map

Transform each element in a collection.

```sigil
map(
    .over: collection,
    .transform: function,
)
```

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to transform |
| `.transform` | `T -> U` | Transformation function |

**Result type:** `[U]`

### filter

Select elements matching a predicate.

```sigil
filter(
    .over: collection,
    .predicate: function,
)
```

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to filter |
| `.predicate` | `T -> bool` | Selection predicate |

**Result type:** `[T]`

### fold

Reduce a collection to a single value.

```sigil
fold(
    .over: collection,
    .init: initial,
    .op: operation,
)
```

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to fold |
| `.init` | `U` | Initial accumulator |
| `.op` | `(U, T) -> U` | Combining operation |

**Result type:** `U`

### collect

Build a list from a range.

```sigil
collect(
    .range: range,
    .transform: function,
)
```

| Property | Type | Description |
|----------|------|-------------|
| `.range` | Range | Range to iterate |
| `.transform` | `int -> T` | Transformation |

**Result type:** `[T]`

### find

Find the first element matching a predicate.

```sigil
find(
    .over: collection,
    .where: predicate,
    [ .default: fallback, ]
)
```

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to search |
| `.where` | `T -> bool` | Predicate |
| `.default` | `T` | Fallback (optional) |

**Result type:** `Option<T>` without default, `T` with default.

**Variant: find with transformation (find_map)**

```sigil
find(
    .over: collection,
    .map: transform,
)
```

| Property | Type | Description |
|----------|------|-------------|
| `.over` | `[T]` | Collection to search |
| `.map` | `T -> Option<U>` | Transformation returning optional |

Returns the first `Some(value)` produced by the transformation, or `None` if all return `None`.

## Recursive Patterns

### recurse

Define a recursive function.

```sigil
recurse(
    .cond: condition,
    .base: base_value,
    .step: recursive_expression,
    [ .memo: bool, ]
    [ .parallel: threshold, ]
)
```

| Property | Type | Description |
|----------|------|-------------|
| `.cond` | `bool` | Base case condition |
| `.base` | `T` | Value when condition true |
| `.step` | `T` | Recursive expression (uses `self()`) |
| `.memo` | `bool` | Enable memoization (default: false) |
| `.parallel` | `int` | Parallelize above threshold (optional) |

**Semantics:**

Within `.step`, `self(...)` refers to the recursive function.

Memoization (`.memo: true`) caches results for the duration of the top-level call. The cache is discarded when the call returns.

```sigil
@fibonacci (n: int) -> int = recurse(
    .cond: n <= 1,
    .base: n,
    .step: self(n - 1) + self(n - 2),
    .memo: true,
)
```

## Concurrency Patterns

### parallel

Execute tasks concurrently.

```sigil
parallel(
    .name1: expr1,
    .name2: expr2,
    [ .timeout: duration, ]
    [ .on_error: strategy, ]
)
```

**Named form:** Each `.name: expr` is a named task. Returns a struct with fields matching the names.

**List form:**

```sigil
parallel(
    .tasks: list_of_tasks,
    [ .max_concurrent: int, ]
)
```

### timeout

Limit operation execution time.

```sigil
timeout(
    .op: expression,
    .after: duration,
)
```

**Result type:** `Result<T, TimeoutError>`

## Resilience Patterns

### retry

Retry operations with backoff.

```sigil
retry(
    .op: expression,
    .attempts: count,
    [ .backoff: strategy, ]
    [ .on: error_types, ]
)
```

| Property | Type | Description |
|----------|------|-------------|
| `.op` | `Result<T, E>` | Operation to retry |
| `.attempts` | `int` | Maximum attempts |
| `.backoff` | Backoff | Backoff strategy |
| `.on` | `[ErrorType]` | Errors triggering retry |

### cache

Cache results with optional TTL. Requires `Cache` capability.

```sigil
cache(
    .key: key_expression,
    .op: expression,
    [ .ttl: duration, ]
)
```

### validate

Validate input with error accumulation.

```sigil
validate(
    .rules: [ condition | "error", ... ],
    .then: success_value,
)
```

**Result type:** `Result<T, [str]>`

### with

Resource management with cleanup.

```sigil
with(
    .acquire: expression,
    .use: resource -> expression,
    .release: resource -> expression,
)
```

The `.release` expression is always executed, even on error.

## for Pattern

The `for` pattern enables iteration with early exit.

```sigil
for(
    .over: collection,
    [ .map: transform, ]
    .match: pattern,
    .default: fallback,
)
```

**Semantics:**

1. Iterate over `.over`
2. Apply `.map` transformation (if provided)
3. Test `.match` pattern
4. Return first match, or `.default` if none
