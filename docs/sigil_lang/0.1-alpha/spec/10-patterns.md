# Patterns

This section defines built-in patterns. Patterns are language constructs distinct from function calls. There are two categories:

- **function_seq** — Contains a sequence of expressions evaluated in order
- **function_exp** — Contains named expressions (`.name: expr`)

## Pattern Categories

### function_seq

A function_seq contains a sequence of expressions. Order is significant; expressions are evaluated serially. These are control flow constructs.

```
function_seq       = run_expr | try_expr | match_expr .
```

| Pattern | Purpose |
|---------|---------|
| `run` | Sequential execution with bindings |
| `try` | Sequential execution with error propagation |
| `match` | Pattern matching with ordered arms |

### function_exp

A function_exp contains named expressions. Each argument is a `.name: expr` pair. These are data transformation constructs and built-in functions.

```
function_exp       = ( pattern_name | builtin_name ) "(" named_exp { "," named_exp } [ "," ] ")" .
pattern_name       = "map" | "filter" | "fold" | "recurse" | "collect" | "find"
                   | "parallel" | "retry" | "cache" | "validate" | "timeout" | "with" .
builtin_name       = "len" | "is_empty"
                   | "is_some" | "is_none" | "is_ok" | "is_err"
                   | "assert" | "assert_eq" | "assert_ne"
                   | "assert_some" | "assert_none" | "assert_ok" | "assert_err"
                   | "assert_panics" | "assert_panics_with"
                   | "print" | "compare" | "min" | "max" | "panic" .
named_exp          = "." identifier ":" expression .
```

**Exception:** Type conversion functions (`int`, `float`, `str`, `byte`) allow positional argument syntax:

```
conversion_call    = ( "int" | "float" | "str" | "byte" ) "(" expression ")" .
```

**Patterns:**

| Pattern | Purpose |
|---------|---------|
| `map` | Transform each element |
| `filter` | Select matching elements |
| `fold` | Reduce to single value |
| `collect` | Build list from range |
| `find` | Find first matching element |
| `recurse` | Recursive computation |
| `parallel` | Concurrent execution |
| `timeout` | Time-bounded operation |
| `retry` | Retry with backoff |
| `cache` | Memoization with TTL |
| `validate` | Input validation |
| `with` | Resource management |

**Built-in Functions:**

| Builtin | Purpose |
|---------|---------|
| `int`, `float`, `str`, `byte` | Type conversion (positional allowed) |
| `len`, `is_empty` | Collection inspection |
| `is_some`, `is_none` | Option inspection |
| `is_ok`, `is_err` | Result inspection |
| `assert`, `assert_eq`, `assert_ne` | Assertion |
| `assert_some`, `assert_none`, `assert_ok`, `assert_err` | Type-specific assertion |
| `assert_panics`, `assert_panics_with` | Panic assertion |
| `print` | Output |
| `compare`, `min`, `max` | Comparison |
| `panic` | Termination |

See [Built-in Functions](11-built-in-functions.md) for complete signatures and semantics.

## Combined Grammar

```
pattern_expr       = function_seq | function_exp | conversion_call .
function_seq       = run_expr | try_expr | match_expr .
function_exp       = ( pattern_name | builtin_name ) "(" named_exp { "," named_exp } [ "," ] ")" .
conversion_call    = ( "int" | "float" | "str" | "byte" ) "(" expression ")" .
pattern_name       = "map" | "filter" | "fold" | "recurse" | "collect" | "find"
                   | "parallel" | "retry" | "cache" | "validate" | "timeout" | "with" .
builtin_name       = "len" | "is_empty"
                   | "is_some" | "is_none" | "is_ok" | "is_err"
                   | "assert" | "assert_eq" | "assert_ne"
                   | "assert_some" | "assert_none" | "assert_ok" | "assert_err"
                   | "assert_panics" | "assert_panics_with"
                   | "print" | "compare" | "min" | "max" | "panic" .
named_exp          = "." identifier ":" expression .
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
2. If any binding expression returns a `Result<T, E>`, the binding variable has type `T`
3. If any binding expression evaluates to `Err(e)`, return `Err(e)` immediately
4. The final expression is the result (typically wrapped in `Ok`)

```sigil
try(
    let content = read_file(path),
    let parsed = parse(content),
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

Execute tasks concurrently and wait for all to settle.

```sigil
parallel(
    .tasks: task_list,
    [ .max_concurrent: int, ]
    [ .timeout: duration, ]
)
```

**Semantics:** All tasks run to completion (success or failure). The pattern always returns a list of results—it never fails itself. Errors are captured as `Err` values in the result list.

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.tasks` | `[() -> T]` | List of task expressions |
| `.max_concurrent` | `int` | Maximum concurrent tasks (optional) |
| `.timeout` | `Duration` | Per-task timeout (optional) |

**Result type:** `[Result<T, E>]`

Each slot contains:
- `Ok(value)` if the task succeeded
- `Err(e)` if the task failed
- `Err(TimeoutError)` if the task timed out

**Examples:**

```sigil
// Fetch multiple users concurrently
let results = parallel(
    .tasks: map(.over: ids, .transform: id -> get_user(id)),
    .max_concurrent: 10,
)
// results: [Result<User, Error>]

// Handle results explicitly
let users = filter(.over: results, .predicate: r -> is_ok(r))
    |> map(.over: _, .transform: r -> r.unwrap())
```

```sigil
// Fixed tasks with timeout
let results = parallel(
    .tasks: [get_user(id), get_posts(id), get_notifs(id)],
    .timeout: 5s,
)
let user = results[0]?           // propagate error
let posts = results[1] ?? []     // default on error
let notifs = results[2] ?? []    // default on error
```

### spawn

Execute tasks concurrently without waiting (fire and forget).

```sigil
spawn(
    .tasks: task_list,
    [ .max_concurrent: int, ]
)
```

**Semantics:** Tasks are started but not awaited. Errors are silently discarded. Use for side effects where results are not needed.

**Properties:**

| Property | Type | Description |
|----------|------|-------------|
| `.tasks` | `[() -> T]` | List of task expressions |
| `.max_concurrent` | `int` | Maximum concurrent tasks (optional) |

**Result type:** `void`

**Examples:**

```sigil
// Fire and forget - send notifications, don't wait
spawn(
    .tasks: map(.over: users, .transform: u -> send_notification(u)),
)

// Log analytics events
spawn(
    .tasks: [log_event(event), send_to_analytics(event)],
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
