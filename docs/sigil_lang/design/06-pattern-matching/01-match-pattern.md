# Match Pattern

This document covers Sigil's `match` pattern for pattern matching: basic syntax, structure, and using match as an expression.

---

## Overview

The `match` pattern provides type-safe dispatch over sum types and values. It combines testing and extraction in a single construct, ensuring all cases are handled at compile time.

```sigil
type Status = Pending | Running | Completed | Failed(str)

@describe (s: Status) -> str = match(s,
    Pending -> "waiting",
    Running -> "in progress",
    Completed -> "done",
    Failed(msg) -> "failed: " + msg
)
```

---

## Basic Syntax

### Match Structure

```sigil
match(expression,
    pattern1 -> result1,
    pattern2 -> result2,
    ...
)
```

**Components:**

| Part | Description |
|------|-------------|
| `match(...)` | The match pattern |
| `expression` | Value to match against |
| `pattern -> result` | Arms mapping patterns to results |

### Simple Example

```sigil
type Color = Red | Green | Blue

@to_hex (c: Color) -> str = match(c,
    Red -> "#FF0000",
    Green -> "#00FF00",
    Blue -> "#0000FF"
)
```

---

## Pattern Types

### Variant Patterns

Match sum type variants:

```sigil
type Option<T> = Some(T) | None

@unwrap_or (opt: Option<int>, default: int) -> int = match(opt,
    Some(value) -> value,
    None -> default
)
```

### Literal Patterns

Match specific values:

```sigil
@describe_code (code: int) -> str = match(code,
    200 -> "ok",
    201 -> "created",
    400 -> "bad request",
    404 -> "not found",
    500 -> "server error",
    _ -> "unknown"
)
```

**Supported literals:**

| Type | Examples |
|------|----------|
| `int` | `0`, `42`, `-1` |
| `float` | `3.14`, `-0.5` |
| `str` | `"hello"`, `""` |
| `bool` | `true`, `false` |

### Wildcard Pattern

Match anything with `_`:

```sigil
@is_success (s: Status) -> bool = match(s,
    Completed -> true,
    _ -> false
)
```

**Rules:**
- `_` matches anything, binds nothing
- Must be the last arm
- Compiler warns if wildcard hides unhandled variants

### Variable Patterns

Bind the matched value to a name:

```sigil
@process (n: int) -> str = match(n,
    0 -> "zero",
    x -> "got: " + str(x)  // x binds the value
)
```

---

## Match with Destructuring

### Variant Destructuring

Extract data from variants:

```sigil
type Result<T, E> = Ok(T) | Err(E)

@process (r: Result<int, str>) -> str = match(r,
    Ok(value) -> "success: " + str(value),
    Err(msg) -> "error: " + msg
)
```

### Nested Variants

Match nested structures:

```sigil
@get_nested (o: Option<Option<int>>) -> int = match(o,
    Some(Some(n)) -> n,
    Some(None) -> 0,
    None -> -1
)
```

### Named Fields in Variants

When variants have named fields:

```sigil
type Status =
    | Pending
    | Running(progress: float)
    | Failed(error: str, code: int)

@describe (s: Status) -> str = match(s,
    Pending -> "waiting",
    Running(progress: p) -> "at " + str(p * 100) + "%",
    Failed(error: e, code: c) -> "error " + str(c) + ": " + e
)
```

---

## Match as Expression

Match is an expression that returns a value. Use it anywhere an expression is valid.

### In Variable Binding

```sigil
@process (opt: Option<int>) -> int = run(
    value = match(opt,
        Some(n) -> n,
        None -> 0
    ),
    value * 2
)
```

### In Function Calls

```sigil
@log_result (r: Result<Data, Error>) -> void = print(match(r,
    Ok(data) -> "success: " + data.to_string(),
    Err(e) -> "error: " + e.message
))
```

### In Return Position

```sigil
@classify (n: int) -> str = match(n,
    0 -> "zero",
    n if n > 0 -> "positive",
    _ -> "negative"
)
```

The match result is the function's return value directly.

### Nested in Patterns

```sigil
@process (items: [Result<int, str>]) -> [int] = map(items, item ->
    match(item,
        Ok(n) -> n,
        Err(_) -> 0
    )
)
```

---

## Multiple Match Subjects

Match on multiple values simultaneously:

```sigil
type Response = { status: Status, data: Option<Data> }

@describe (r: Response) -> str = match((r.status, r.data),
    (Completed, Some(d)) -> "done with: " + d.to_string(),
    (Completed, None) -> "done, no data",
    (Failed(msg), _) -> "failed: " + msg,
    (_, _) -> "in progress"
)
```

**Alternative syntax:**

```sigil
@describe (r: Response) -> str = match(r.status, r.data,
    (Completed, Some(d)) -> "done with: " + d.to_string(),
    (Completed, None) -> "done, no data",
    (Failed(msg), _) -> "failed: " + msg,
    (_, _) -> "in progress"
)
```

---

## Common Match Idioms

### Option Handling

```sigil
@get_name (user: Option<User>) -> str = match(user,
    Some(u) -> u.name,
    None -> "anonymous"
)

// With default value
@get_or_default<T> (opt: Option<T>, default: T) -> T = match(opt,
    Some(value) -> value,
    None -> default
)
```

### Result Handling

```sigil
@handle_result (r: Result<int, str>) -> int = match(r,
    Ok(n) -> n,
    Err(msg) -> run(
        print("Error: " + msg),
        0
    )
)
```

### Enum Dispatch

```sigil
type Command = Add(int) | Remove(int) | Clear | Reset(int)

@execute (cmd: Command, current: int) -> int = match(cmd,
    Add(n) -> current + n,
    Remove(n) -> current - n,
    Clear -> 0,
    Reset(n) -> n
)
```

### Recursive Data Structures

```sigil
type Tree<T> = Leaf(T) | Node(Tree<T>, Tree<T>)

@sum_tree (t: Tree<int>) -> int = match(t,
    Leaf(n) -> n,
    Node(left, right) -> sum_tree(left) + sum_tree(right)
)

@depth (t: Tree<int>) -> int = match(t,
    Leaf(_) -> 1,
    Node(left, right) -> 1 + max(depth(left), depth(right))
)
```

---

## Match Arm Syntax

### Single Expression Arms

```sigil
@describe (s: Status) -> str = match(s,
    Pending -> "waiting",
    Running -> "active"
)
```

### Multi-Expression Arms

Use `run` for multiple steps:

```sigil
@process (r: Result<int, str>) -> int = match(r,
    Ok(n) -> run(
        print("Got value: " + str(n)),
        validated = validate(n),
        validated * 2
    ),
    Err(e) -> run(
        log_error(e),
        0
    )
)
```

### Finding First Match

Use the `find` pattern for searching with match logic:

```sigil
@find_value (items: [Option<int>]) -> Option<int> = find(
    .over: items,
    .where: item -> match(item,
        Some(n).match(n > 0) -> true,
        _ -> false
    ),
)

// With default value
@find_value_or_default (items: [Option<int>]) -> int = find(
    .over: items,
    .where: item -> match(item,
        Some(n).match(n > 0) -> true,
        _ -> false
    ),
    .default: Some(0),
) ?? 0
```

---

## Match vs If-Else

### When to Use Match

- Sum type dispatch
- Multiple specific cases
- Destructuring needed
- Exhaustiveness checking desired

```sigil
// Good: clear dispatch over variants
@handle (r: Result<Data, Error>) -> str = match(r,
    Ok(data) -> process(data),
    Err(e) -> handle_error(e)
)
```

### When to Use If-Else

- Boolean conditions
- Range checks without patterns
- Simple two-way branching

```sigil
// Good: simple boolean check
@classify (n: int) -> str =
    if n >= 0 then "non-negative"
    else "negative"
```

### Converting Between Them

```sigil
// If-else (verbose)
@is_some (opt: Option<int>) -> bool =
    if is_variant(opt, Some) then true
    else false

// Match (cleaner)
@is_some (opt: Option<int>) -> bool = match(opt,
    Some(_) -> true,
    None -> false
)
```

---

## Return Type Inference

All match arms must have the same type:

```sigil
// Valid: all arms return str
@describe (n: int) -> str = match(n,
    0 -> "zero",
    1 -> "one",
    _ -> "many"
)

// Invalid: mismatched types
@bad (n: int) -> str = match(n,
    0 -> "zero",
    1 -> 1,      // ERROR: expected str, found int
    _ -> "many"
)
```

### Type Inference with Generics

```sigil
@unwrap<T> (opt: Option<T>) -> T = match(opt,
    Some(value) -> value,
    None -> panic("unwrap on None")
)
```

---

## Match Pattern Ordering

### Order Matters

Patterns are checked top-to-bottom. Put specific patterns before general:

```sigil
// Correct: specific first
@classify (n: int) -> str = match(n,
    0 -> "zero",
    1 -> "one",
    _ -> "other"
)

// Wrong: unreachable code
@bad (n: int) -> str = match(n,
    _ -> "other",  // matches everything
    0 -> "zero",   // ERROR: unreachable pattern
    1 -> "one"     // ERROR: unreachable pattern
)
```

### Overlapping Patterns

Guards can create overlapping patterns:

```sigil
@grade (score: int) -> str = match(score,
    s.match(s >= 90) -> "A",
    s.match(s >= 80) -> "B",
    s.match(s >= 70) -> "C",
    s.match(s >= 60) -> "D",
    _ -> "F"
)
```

Order determines which matches first.

---

## Best Practices

### Handle All Cases Explicitly

Prefer explicit handling over wildcards:

```sigil
// Preferred: all cases explicit
@describe (s: Status) -> str = match(s,
    Pending -> "waiting",
    Running -> "active",
    Completed -> "done",
    Failed(msg) -> "error: " + msg
)

// Acceptable when intentional, but compiler warns
@is_active (s: Status) -> bool = match(s,
    Running -> true,
    _ -> false  // warning: wildcard hides variants
)
```

### Keep Arms Simple

Complex logic should go in helper functions:

```sigil
// Preferred: delegate to helpers
@process (cmd: Command) -> State = match(cmd,
    Add(n) -> handle_add(n),
    Remove(n) -> handle_remove(n),
    Clear -> handle_clear(),
    Reset(n) -> handle_reset(n)
)

// Avoid: complex inline logic
@process_bad (cmd: Command) -> State = match(cmd,
    Add(n) -> run(
        validated = validate(n),
        checked = check_bounds(validated),
        apply_add(checked),
        log_action("add", n),
        get_state()
    ),
    // ... more complex arms
)
```

### Use Meaningful Names

```sigil
// Good: meaningful binding names
@process (r: Result<User, AuthError>) -> str = match(r,
    Ok(user) -> welcome(user),
    Err(auth_error) -> show_error(auth_error)
)

// Avoid: single letter names except for trivial cases
@process (r: Result<User, AuthError>) -> str = match(r,
    Ok(u) -> welcome(u),
    Err(e) -> show_error(e)
)
```

---

## Integration with Patterns

Match works seamlessly with other Sigil patterns:

### With `run`

```sigil
@process (opt: Option<int>) -> int = run(
    value = match(opt,
        Some(n) -> n,
        None -> 0
    ),
    doubled = value * 2,
    doubled + 1
)
```

### With `try`

```sigil
@fetch_user (id: int) -> Result<User, Error> = try(
    response = fetch("/users/" + str(id)),
    match(response.status,
        200 -> Ok(parse_user(response.body)),
        404 -> Err(NotFound { id: id }),
        _ -> Err(Unknown { status: response.status })
    )
)
```

### With `map`

```sigil
@extract_values (items: [Option<int>]) -> [int] =
    map(filter(items, is_some), opt ->
        match(opt,
            Some(n) -> n,
            None -> 0  // unreachable, but required
        )
    )
```

---

## Error Messages

### Missing Case

```
error[E0400]: non-exhaustive match
  |
5 | @describe (s: Status) -> str = match(s,
6 |     Pending -> "waiting",
7 |     Running -> "active"
8 | )
  | ^ missing: Completed, Failed

help: add missing patterns or use wildcard `_`
```

### Type Mismatch

```
error[E0308]: mismatched types in match arms
  |
5 | @describe (n: int) -> str = match(n,
6 |     0 -> "zero",
7 |     1 -> 1,
  |          ^ expected str, found int
8 |     _ -> "many"
  | )
```

### Unreachable Pattern

```
error[E0401]: unreachable pattern
  |
5 | @classify (n: int) -> str = match(n,
6 |     _ -> "other",
7 |     0 -> "zero",
  |     ^ this pattern is never reached
```

---

## See Also

- [Destructuring](02-destructuring.md) — Struct and list destructuring
- [Guards and Bindings](03-guards-and-bindings.md) — Guards, or patterns, @ binding
- [Exhaustiveness](04-exhaustiveness.md) — Compiler enforcement
- [Type Narrowing](05-type-narrowing.md) — Flow-sensitive typing
- [Type System](../03-type-system/index.md) — Sum types and generics
