---
title: "Pattern Matching"
description: "Match expressions, pattern types, guards, and exhaustiveness."
order: 6
part: "Data"
---

# Pattern Matching

Pattern matching is how you inspect and decompose values in Ori. It's the primary way to work with sum types, and it's more powerful than switch statements in other languages.

## The `match` Expression

The `match` expression compares a value against patterns:

```ori
@color_code (c: Color) -> str = match c {
    Red -> "#FF0000"
    Green -> "#00FF00"
    Blue -> "#0000FF"
}
```

Structure:
- First argument is the value to match
- Following arguments are pattern-result pairs
- `->` separates pattern from result
- First matching pattern wins

### Match Returns a Value

`match` is an expression, so it returns a value:

```ori
let description = match status {
    Active -> "Running"
    Paused -> "On hold"
    Stopped -> "Finished"
}
```

All branches must return the same type.

## Pattern Types

### Literal Patterns

Match exact values:

```ori
@describe_number (n: int) -> str = match n {
    0 -> "zero"
    1 -> "one"
    2 -> "two"
    _ -> "many"
}
```

Works with strings, characters, and booleans too:

```ori
@is_yes (s: str) -> bool = match s {
    "yes" | "y" | "Y" -> true
    _ -> false
}
```

### Binding Patterns

Capture the value in a variable:

```ori
@double (n: int) -> int = match n {
    0 -> 0
    x -> x * 2,    // x binds to n's value
}
```

### Wildcard Pattern

`_` matches anything and discards it:

```ori
@is_zero (n: int) -> bool = match n {
    0 -> true
    _ -> false,    // Don't care what it is
}
```

### Variant Patterns

Match sum type variants:

```ori
type Shape =
    | Circle(radius: float)
    | Rectangle(width: float, height: float)

@area (s: Shape) -> float = match s {
    Circle(radius) -> 3.14159 * radius * radius
    Rectangle(width, height) -> width * height
}
```

### Struct Patterns

Match on struct fields:

```ori
type Point = { x: int, y: int }

@describe_point (p: Point) -> str = match p {
    Point { x: 0, y: 0 } -> "origin"
    Point { x: 0, y } -> `on y-axis at {y}`
    Point { x, y: 0 } -> `on x-axis at {x}`
    Point { x, y } -> `at ({x}, {y})`
}
```

Use `..` to ignore remaining fields:

```ori
type User = { id: int, name: str, email: str, active: bool }

@user_name (u: User) -> str = match u {
    User { name, .. } -> name
}
```

### Tuple Patterns

Match on tuple elements:

```ori
@describe_pair (p: (int, int)) -> str = match p {
    (0, 0) -> "origin"
    (0, y) -> `y-axis at {y}`
    (x, 0) -> `x-axis at {x}`
    (x, y) -> `({x}, {y})`
}
```

### List Patterns

Match on list structure:

```ori
@describe_list (items: [int]) -> str = match items {
    [] -> "empty"
    [x] -> `single element: {x}`
    [x, y] -> `two elements: {x} and {y}`
    [first, ..rest] -> `starts with {first}, {len(collection: rest)} more`
}
```

List pattern syntax:
- `[]` — empty list
- `[x]` — exactly one element
- `[x, y]` — exactly two elements
- `[first, ..rest]` — first element and remaining list
- `[..init, last]` — all but last, and last element

### Range Patterns

Match value in a range:

```ori
@grade (score: int) -> str = match score {
    90..=100 -> "A"
    80..90 -> "B"
    70..80 -> "C"
    60..70 -> "D"
    _ -> "F"
}
```

## Advanced Patterns

### Or Patterns

Match multiple patterns with `|`:

```ori
@is_primary (c: Color) -> bool = match c {
    Red | Green | Blue -> true
    _ -> false
}

@is_weekend (day: str) -> bool = match day {
    "Saturday" | "Sunday" -> true
    _ -> false
}
```

### At Patterns

Bind a name while also matching a pattern:

```ori
@process (s: Status) -> str = match s {
    status @ Failed(_) -> {
        log_failure(status: status),   // Use the full value
        "failed"
    }
    _ -> "ok"
}
```

### Guards

Add conditions with `.match()`:

```ori
@classify (n: int) -> str = match n {
    x.match(x < 0) -> "negative"
    0 -> "zero"
    x.match(x < 10) -> "small"
    x.match(x < 100) -> "medium"
    _ -> "large"
}
```

Guards are evaluated after the pattern matches:

```ori
@describe_age (age: int) -> str = match age {
    a.match(a < 0) -> "invalid"
    a.match(a < 13) -> "child"
    a.match(a < 20) -> "teenager"
    a.match(a < 65) -> "adult"
    _ -> "senior"
}
```

### Combining Patterns

Combine different pattern types:

```ori
type Request =
    | Get(path: str)
    | Post(path: str, body: str)
    | Delete(path: str)

@handle (r: Request) -> str = match r {
    Get(path).match(path.starts_with("/api")) -> `API GET: {path}`
    Post(path, body).match(body.len() > 1000) -> "Body too large"
    Delete("/admin") -> "Cannot delete admin"
    Get(path) | Delete(path) -> `Reading: {path}`
    Post(path, _) -> `Writing: {path}`
}
```

## Exhaustiveness

The compiler ensures you handle all cases.

### Complete Coverage

```ori
type Direction = North | South | East | West

// ERROR: non-exhaustive match
@describe (d: Direction) -> str = match d {
    North -> "up"
    South -> "down"
    // Missing East and West!
}
```

The compiler tells you what's missing:

```
error: non-exhaustive match
  --> file.ori:5:1
   |
   | missing patterns: East, West
```

### Catching Everything

Use `_` as a catch-all:

```ori
@is_north (d: Direction) -> bool = match d {
    North -> true
    _ -> false,    // Handles South, East, West
}
```

### Unreachable Patterns

The compiler warns about patterns that can never match:

```ori
// WARNING: unreachable pattern
@example (n: int) -> int = match n {
    _ -> 0,        // This matches everything
    42 -> 42,      // Never reached!
}
```

## Pattern Matching in Functions

### Function Clauses

Define functions with pattern-matched parameters:

```ori
@factorial (0: int) -> int = 1
@factorial (n) -> int = n * factorial(n: n - 1)
```

### Guards in Functions

```ori
@abs (n: int) -> int if n < 0 = -n
@abs (n: int) -> int = n
```

### Combining Clauses and Guards

```ori
@classify (0: int) -> str = "zero"
@classify (n) -> str if n < 0 = "negative"
@classify (n) -> str if n < 10 = "small"
@classify (_: int) -> str = "large"
```

## Common Patterns

### Handling Option

```ori
@display_name (name: Option<str>) -> str = match name {
    Some(n) -> n
    None -> "Anonymous"
}
```

### Handling Result

```ori
@process_result (r: Result<int, str>) -> str = match r {
    Ok(value) -> `Success: {value}`
    Err(error) -> `Error: {error}`
}
```

### Extracting Nested Data

```ori
type Response = {
    status: int,
    data: Option<{
        user: Option<User>,
        items: [Item],
    }>,
}

@get_user_name (r: Response) -> Option<str> = match r {
    Response { data: Some({ user: Some(u), .. }), .. } -> Some(u.name)
    _ -> None
}
```

### Matching Multiple Values

Use tuples to match multiple values at once:

```ori
@compare_sizes (a: int, b: int) -> str = match (a, b) {
    (0, 0) -> "both zero"
    (0, _) -> "first is zero"
    (_, 0) -> "second is zero"
    (x, y).match(x == y) -> "equal"
    (x, y).match(x < y) -> "first is smaller"
    _ -> "first is larger"
}
```

## Refutability

Patterns can be:

### Irrefutable Patterns

Always match — used in `let` bindings:

```ori
let x = 42                    // Always matches
let (a, b) = tuple            // Always matches (tuple has two elements)
let Point { x, y } = point    // Always matches
```

### Refutable Patterns

Might not match — used in `match`:

```ori
match option {
    Some(x) -> use(x),        // Only matches Some
    None -> handle_none(),     // Only matches None
}
```

### Lists are Refutable

List patterns in `let` can panic:

```ori
let [first, second] = items   // PANIC if items doesn't have exactly 2 elements
```

Use `match` for safe list patterns:

```ori
let first_two = match items {
    [a, b, ..] -> Some((a, b))
    _ -> None
}
```

## Complete Example

```ori
type Json =
    | Null
    | Bool(value: bool)
    | Number(value: float)
    | String(value: str)
    | Array(items: [Json])
    | Object(fields: {str: Json})

@json_type (j: Json) -> str = match j {
    Null -> "null"
    Bool(_) -> "boolean"
    Number(_) -> "number"
    String(_) -> "string"
    Array(_) -> "array"
    Object(_) -> "object"
}

@test_json_type tests @json_type () -> void = {
    assert_eq(actual: json_type(j: Null), expected: "null")
    assert_eq(actual: json_type(j: Bool(value: true)), expected: "boolean")
    assert_eq(actual: json_type(j: Array(items: [])), expected: "array")
}

@json_to_string (j: Json) -> str = match j {
    Null -> "null"
    Bool(true) -> "true"
    Bool(false) -> "false"
    Number(n) -> `{n}`
    String(s) -> `"{s}"`
    Array(items) -> {
        let parts = for item in items yield json_to_string(j: item)
        `[{parts.join(sep: ", ")}]`
    }
    Object(fields) -> {
        let parts = for (key, value) in fields.entries()
            yield `"{key}": {json_to_string(j: value)}`
        `\{{parts.join(sep: ", ")}\}`
    }
}

@test_json_to_string tests @json_to_string () -> void = {
    assert_eq(actual: json_to_string(j: Null), expected: "null")
    assert_eq(actual: json_to_string(j: Number(value: 42.0)), expected: "42")
    assert_eq(
        actual: json_to_string(j: Array(items: [Number(value: 1.0), Number(value: 2.0)]))
        expected: "[1, 2]"
    )
}

@get_string_field (obj: Json, key: str) -> Option<str> = match obj {
    Object(fields) -> match fields[key] {
        Some(String(s)) -> Some(s)
        _ -> None
    }
    _ -> None
}

@test_get_string_field tests @get_string_field () -> void = {
    let obj = Object(fields: {"name": String(value: "Alice")})
    assert_eq(actual: get_string_field(obj: obj, key: "name"), expected: Some("Alice"))
    assert_eq(actual: get_string_field(obj: obj, key: "age"), expected: None)
    assert_eq(actual: get_string_field(obj: Null, key: "name"), expected: None)
}
```

## Quick Reference

### Pattern Types

```ori
42, "hello", true        // Literal
x                        // Binding
_                        // Wildcard
Variant(x)               // Sum type variant
{ field, .. }            // Struct
(a, b)                   // Tuple
[]                       // Empty list
[x, y]                   // Exact list
[first, ..rest]          // List with rest
10..20                   // Range
A | B                    // Or pattern
x @ Pattern              // At pattern
x.match(condition)       // Guard
```

### Match Expression

```ori
match value {
    Pattern1 -> result1
    Pattern2 -> result2
    _ -> default
}
```

### Function Clauses

```ori
@fn (0: int) -> int = 0
@fn (n) -> int if n < 0 = -n
@fn (n) -> int = n
```

## What's Next

Now that you understand pattern matching:

- **[Option and Result](/guide/07-option-result)** — Handle missing values and errors
- **[Error Propagation](/guide/08-error-propagation)** — The `?` operator and error traces
