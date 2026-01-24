# Match Pattern

This document covers Sigil's `match` pattern for pattern matching: basic syntax, structure, and using match as an expression.

---

## Overview

The `match` pattern provides type-safe dispatch over sum types and values. It combines testing and extraction in a single construct, ensuring all cases are handled at compile time.

```sigil
type Status = Pending | Running | Completed | Failed(str)

@describe (status: Status) -> str = match(status,
    Pending -> "waiting",
    Running -> "in progress",
    Completed -> "done",
    Failed(message) -> "failed: " + message
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

@to_hex (color: Color) -> str = match(color,
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
@process (number: int) -> str = match(number,
    0 -> "zero",
    // value binds the matched number
    value -> "got: " + str(value)
)
```

---

## Match with Destructuring

### Variant Destructuring

Extract data from variants:

```sigil
type Result<T, E> = Ok(T) | Err(E)

@process (result: Result<int, str>) -> str = match(result,
    Ok(value) -> "success: " + str(value),
    Err(message) -> "error: " + message
)
```

### Nested Variants

Match nested structures:

```sigil
@get_nested (option: Option<Option<int>>) -> int = match(option,
    Some(Some(value)) -> value,
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

@describe (status: Status) -> str = match(status,
    Pending -> "waiting",
    Running(progress: percent) -> "at " + str(percent * 100) + "%",
    Failed(error: message, code: code_num) -> "error " + str(code_num) + ": " + message
)
```

---

## Match as Expression

Match is an expression that returns a value. Use it anywhere an expression is valid.

### In Variable Binding

```sigil
@process (opt: Option<int>) -> int = run(
    let extracted = match(opt,
        Some(value) -> value,
        None -> 0,
    ),
    extracted * 2,
)
```

### In Function Calls

```sigil
@log_result (result: Result<Data, Error>) -> void = print(
    .message: match(result,
        Ok(data) -> "success: " + data.to_string(),
        Err(error) -> "error: " + error.message,
    ),
)
```

### In Return Position

```sigil
@classify (number: int) -> str = match(number,
    0 -> "zero",
    value if value > 0 -> "positive",
    _ -> "negative"
)
```

The match result is the function's return value directly.

### Nested in Patterns

```sigil
@process (items: [Result<int, str>]) -> [int] = map(items, item ->
    match(item,
        Ok(value) -> value,
        Err(_) -> 0
    )
)
```

---

## Multiple Match Subjects

Match on multiple values simultaneously:

```sigil
type Response = { status: Status, data: Option<Data> }

@describe (response: Response) -> str = match((response.status, response.data),
    (Completed, Some(data)) -> "done with: " + data.to_string(),
    (Completed, None) -> "done, no data",
    (Failed(message), _) -> "failed: " + message,
    (_, _) -> "in progress"
)
```

**Alternative syntax:**

```sigil
@describe (response: Response) -> str = match(response.status, response.data,
    (Completed, Some(data)) -> "done with: " + data.to_string(),
    (Completed, None) -> "done, no data",
    (Failed(message), _) -> "failed: " + message,
    (_, _) -> "in progress"
)
```

---

## Common Match Idioms

### Option Handling

```sigil
@get_name (user: Option<User>) -> str = match(user,
    Some(found_user) -> found_user.name,
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
@handle_result (result: Result<int, str>) -> int = match(result,
    Ok(value) -> value,
    Err(message) -> run(
        print(.message: "Error: " + message),
        0,
    ),
)
```

### Enum Dispatch

```sigil
type Command = Add(int) | Remove(int) | Clear | Reset(int)

@execute (command: Command, current: int) -> int = match(command,
    Add(amount) -> current + amount,
    Remove(amount) -> current - amount,
    Clear -> 0,
    Reset(value) -> value
)
```

### Recursive Data Structures

```sigil
type Tree<T> = Leaf(T) | Node(Tree<T>, Tree<T>)

@sum_tree (tree: Tree<int>) -> int = match(tree,
    Leaf(value) -> value,
    Node(left, right) -> sum_tree(left) + sum_tree(right)
)

@depth (tree: Tree<int>) -> int = match(tree,
    Leaf(_) -> 1,
    Node(left, right) -> 1 + max(depth(left), depth(right))
)
```

---

## Match Arm Syntax

### Single Expression Arms

```sigil
@describe (status: Status) -> str = match(status,
    Pending -> "waiting",
    Running -> "active"
)
```

### Multi-Expression Arms

Use `run` for multiple steps:

```sigil
@process (result: Result<int, str>) -> int = match(result,
    Ok(value) -> run(
        print(.message: "Got value: " + str(value)),
        let validated = validate(value),
        validated * 2,
    ),
    Err(error) -> run(
        log_error(error),
        0,
    ),
)
```

### Finding First Match

Use the `find` pattern for searching with match logic:

```sigil
@find_value (items: [Option<int>]) -> Option<int> = find(
    .over: items,
    .where: item -> match(item,
        Some(value).match(value > 0) -> true,
        _ -> false
    ),
)

// With default value
@find_value_or_default (items: [Option<int>]) -> int = find(
    .over: items,
    .where: item -> match(item,
        Some(value).match(value > 0) -> true,
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
@classify (number: int) -> str =
    if number >= 0 then "non-negative"
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
@describe (number: int) -> str = match(number,
    0 -> "zero",
    1 -> "one",
    _ -> "many"
)

// Invalid: mismatched types
@bad (number: int) -> str = match(number,
    0 -> "zero",
    // ERROR: expected str, found int
    1 -> 1,
    _ -> "many"
)
```

### Type Inference with Generics

```sigil
@unwrap<T> (opt: Option<T>) -> T = match(opt,
    Some(value) -> value,
    None -> panic(.message: "unwrap on None"),
)
```

---

## Match Pattern Ordering

### Order Matters

Patterns are checked top-to-bottom. Put specific patterns before general:

```sigil
// Correct: specific first
@classify (number: int) -> str = match(number,
    0 -> "zero",
    1 -> "one",
    _ -> "other"
)

// Wrong: unreachable code
@bad (number: int) -> str = match(number,
    // matches everything
    _ -> "other",
    // ERROR: unreachable pattern
    0 -> "zero",
    // ERROR: unreachable pattern
    1 -> "one"
)
```

### Overlapping Patterns

Guards can create overlapping patterns:

```sigil
@grade (score: int) -> str = match(score,
    value.match(value >= 90) -> "A",
    value.match(value >= 80) -> "B",
    value.match(value >= 70) -> "C",
    value.match(value >= 60) -> "D",
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
@describe (status: Status) -> str = match(status,
    Pending -> "waiting",
    Running -> "active",
    Completed -> "done",
    Failed(message) -> "error: " + message
)

// Acceptable when intentional, but compiler warns
@is_active (status: Status) -> bool = match(status,
    Running -> true,
    // warning: wildcard hides variants
    _ -> false
)
```

### Keep Arms Simple

Complex logic should go in helper functions:

```sigil
// Preferred: delegate to helpers
@process (command: Command) -> State = match(command,
    Add(amount) -> handle_add(amount),
    Remove(amount) -> handle_remove(amount),
    Clear -> handle_clear(),
    Reset(value) -> handle_reset(value)
)

// Avoid: complex inline logic
@process_bad (command: Command) -> State = match(command,
    Add(amount) -> run(
        let validated = validate(amount),
        let checked = check_bounds(validated),
        apply_add(checked),
        log_action("add", amount),
        get_state(),
    ),
    // ... more complex arms
)
```

### Use Meaningful Names

```sigil
// Good: meaningful binding names
@process (result: Result<User, AuthError>) -> str = match(result,
    Ok(user) -> welcome(user),
    Err(auth_error) -> show_error(auth_error)
)

// Avoid: single letter names except for trivial cases
@process (result: Result<User, AuthError>) -> str = match(result,
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
    let extracted = match(opt,
        Some(value) -> value,
        None -> 0,
    ),
    let doubled = extracted * 2,
    doubled + 1,
)
```

### With `try`

```sigil
@fetch_user (id: int) -> Result<User, Error> = try(
    let response = fetch("/users/" + str(id))?,
    match(response.status,
        200 -> Ok(parse_user(response.body)),
        404 -> Err(NotFound { id: id }),
        _ -> Err(Unknown { status: response.status }),
    ),
)
```

### With `map`

```sigil
@extract_values (items: [Option<int>]) -> [int] =
    map(
        .over: filter(
            .over: items,
            .predicate: is_some,
        ),
        .transform: opt ->
            match(opt,
                Some(value) -> value,
                // unreachable, but required
                None -> 0,
            ),
    )
```

---

## Error Messages

### Missing Case

```
error[E0400]: non-exhaustive match
  |
5 | @describe (status: Status) -> str = match(status,
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
5 | @describe (number: int) -> str = match(number,
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
5 | @classify (number: int) -> str = match(number,
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
