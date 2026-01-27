# Destructuring

This document covers destructuring in Ori: extracting values from structs, lists, and sum types in bindings, function parameters, and match patterns.

---

## Overview

Destructuring lets you extract values from compound types in a single operation. Instead of accessing fields one by one, you bind multiple values at once while also verifying structure.

```ori
type Point = { x: int, y: int }

// Without destructuring
@distance_verbose (p: Point) -> float = run(
    let x = p.x,
    let y = p.y,
    sqrt(float(x * x + y * y)),
)

// With destructuring
@distance (p: Point) -> float = run(
    let { x, y } = p,
    sqrt(float(x * x + y * y)),
)
```

---

## Struct Destructuring

### Basic Syntax

Extract fields by name:

```ori
type User = { name: str, email: str, age: int }

@greet (u: User) -> str = run(
    let { name, age } = u,
    name + " is " + str(age) + " years old",
)
```

**Key points:**
- `{ field }` binds field to same-named variable
- Only listed fields are bound
- Type is checked at compile time

### Binding to Different Names

Use `: new_name` to bind to a different variable:

```ori
@process (u: User) -> str = run(
    let { name: user_name, email: user_email } = u,
    user_name + " <" + user_email + ">",
)
```

### Ignoring Fields with `..`

Use `..` to ignore unmentioned fields:

```ori
type Record = {
    id: int,
    name: str,
    created_at: Timestamp,
    updated_at: Timestamp,
    metadata: {str: str}
}

// Only need name and id
@summarize (r: Record) -> str = run(
    let { id, name, .. } = r,
    str(id) + ": " + name,
)
```

**Without `..`:**
```ori
// ERROR: struct has fields 'created_at', 'updated_at', 'metadata' not mentioned
@bad (r: Record) -> str = run(
    // missing fields
    let { id, name } = r,
    str(id) + ": " + name,
)
```

### Match-All Pattern

Use `{ .. }` to match a struct without binding any fields:

```ori
@is_user (u: User) -> bool = run(
    // just verify it's a User
    let { .. } = u,
    true,
)
```

This is useful in match arms where you need to match the type but not extract values.

---

## Destructuring in Function Parameters

Destructure directly in parameter position:

```ori
// In parameter
@distance ({ x, y }: Point) -> float =
    sqrt(float(x * x + y * y))

// Multiple fields
@format_user ({ name, email, .. }: User) -> str =
    name + " <" + email + ">"

// With renaming
@greet ({ name: n, age: a, .. }: User) -> str =
    "Hello " + n + ", you are " + str(a)
```

### Nested Destructuring in Parameters

```ori
type Rect = { origin: Point, size: Size }
type Size = { width: int, height: int }

@area ({ size: { width, height, .. }, .. }: Rect) -> int =
    width * height

@top_left ({ origin: { x, y }, .. }: Rect) -> Point =
    Point { x, y }
```

---

## List Destructuring

### Basic Syntax

Extract elements by position:

```ori
@first (items: [int]) -> Option<int> = match(items,
    [] -> None,
    [x, ..] -> Some(x)
)
```

### Head and Tail

The common functional pattern:

```ori
@head_tail (items: [int]) -> Option<(int, [int])> = match(items,
    [] -> None,
    [head, ..tail] -> Some((head, tail))
)
```

**Syntax:**

| Pattern | Meaning |
|---------|---------|
| `[]` | Empty list |
| `[x]` | Exactly one element |
| `[x, y]` | Exactly two elements |
| `[x, ..]` | At least one, ignore rest |
| `[x, ..rest]` | At least one, bind rest |
| `[.., x]` | At least one, x is last |
| `[x, .., y]` | At least two, x is first, y is last |

### Fixed Length Patterns

```ori
@process_pair (items: [int]) -> Option<int> = match(items,
    // exactly two
    [a, b] -> Some(a + b),
    _ -> None
)

@process_triple (items: [int]) -> Option<int> = match(items,
    // exactly three
    [a, b, c] -> Some(a + b + c),
    _ -> None
)
```

### First and Last

```ori
@first_and_last (items: [int]) -> Option<(int, int)> = match(items,
    [] -> None,
    [x] -> Some((x, x)),
    [first, .., last] -> Some((first, last))
)
```

### In Bindings

List destructuring works in bindings when the pattern is irrefutable:

```ori
// Only safe when you know the list has elements
@process_known (items: [int]) -> int = run(
    // This will panic if list is empty
    let [first, second, ..rest] = items,
    first + second + fold(
        .over: rest,
        .initial: 0,
        .op: +,
    ),
)
```

**Note:** For refutable patterns (might fail), use `match`:

```ori
// Safe: handles empty case
@process_safe (items: [int]) -> int = match(items,
    [first, second, ..rest] -> first + second + fold(
        .over: rest,
        .initial: 0,
        .op: +,
    ),
    [single] -> single,
    [] -> 0
)
```

---

## Variant Destructuring

### Single-Value Variants

```ori
type Option<T> = Some(T) | None

@unwrap_or (opt: Option<int>, default: int) -> int = match(opt,
    Some(value) -> value,
    None -> default
)
```

### Named Field Variants

```ori
type Response =
    | Success(data: Data, cached: bool)
    | Failure(code: int, message: str)

@handle (r: Response) -> str = match(r,
    Success(data: d, cached: c) ->
        if c then "cached: " + d.to_string()
        else "fresh: " + d.to_string(),
    Failure(code: c, message: m) ->
        "Error " + str(c) + ": " + m
)

// Can also use positional
@handle_short (r: Response) -> str = match(r,
    Success(d, c) -> d.to_string(),
    Failure(c, m) -> m
)
```

### Nested Variants

```ori
type Tree<T> = Leaf(T) | Node(left: Tree<T>, right: Tree<T>)

@flatten (t: Tree<int>) -> [int] = match(t,
    Leaf(value) -> [value],
    Node(left: l, right: r) -> flatten(l) + flatten(r)
)
```

---

## Nested Destructuring

### Structs within Structs

```ori
type Address = { street: str, city: str }
type Person = { name: str, address: Address }

@get_city ({ address: { city, .. }, .. }: Person) -> str = city

// In match
@describe_location (p: Person) -> str = match(p,
    { address: { city, .. }, .. } -> "Lives in " + city
)
```

### Lists within Structs

```ori
type Group = { name: str, members: [User] }

@first_member (g: Group) -> Option<str> = match(g,
    { members: [first, ..], .. } -> Some(first.name),
    { members: [], .. } -> None
)
```

### Variants within Structs

```ori
type Container = { value: Option<int>, label: str }

@describe (c: Container) -> str = match(c,
    { value: Some(n), label } -> label + ": " + str(n),
    { value: None, label } -> label + ": empty"
)
```

### Deep Nesting

```ori
type Outer = { inner: Inner }
type Inner = { data: Option<[int]> }

@get_first (o: Outer) -> Option<int> = match(o,
    { inner: { data: Some([x, ..]), .. }, .. } -> Some(x),
    _ -> None
)
```

---

## Partial Destructuring

Use `..` to destructure only some fields:

### In Structs

```ori
type Config = {
    host: str,
    port: int,
    timeout: int,
    retries: int,
    debug: bool
}

// Only need host and port
@connect_string ({ host, port, .. }: Config) -> str =
    host + ":" + str(port)
```

### In Nested Structures

```ori
type Response = {
    status: int,
    headers: Headers,
    body: Body
}
type Body = { data: [byte], encoding: str }

// Get just the data, ignore everything else
@extract_data ({ body: { data, .. }, .. }: Response) -> [byte] = data
```

---

## Destructuring with Type Annotations

Add types for clarity:

```ori
@process (u: User) -> str = run(
    let { name, age }: { name: str, age: int, .. } = u,
    name + " is " + str(age),
)

// In parameters
@greet ({ name, age, .. }: User) -> str =
    name + " (" + str(age) + ")"
```

---

## Destructuring in Pattern Contexts

### In `run` Bindings

```ori
@process (users: [User]) -> [str] = run(
    let [first, ..rest] = users,
    let { name, .. } = first,
    let names = map(
        .over: rest,
        .transform: user -> user.name,
    ),
    [name] + names,
)
```

### In `try` Bindings

```ori
@fetch_user_name (id: int) -> Result<str, Error> = try(
    let response = fetch_user(id)?,
    // destructure the body
    let { name, .. } = response.body,
    Ok(name),
)
```

### In `for` Iteration

```ori
@sum_x_coords (points: [Point]) -> int = run(
    let mut total = 0,
    for { x, .. } in points do total = total + x,
    total,
)
```

---

## Refutable vs Irrefutable Patterns

### Irrefutable Patterns

Always succeed, safe in bindings and parameters:

```ori
// Struct: always succeeds (fields always exist)
let { x, y } = point

// Function parameter: always succeeds
@process ({ name, .. }: User) -> str = ...
```

### Refutable Patterns

Might fail, require `match`:

```ori
// List might be empty: use match
@first (items: [int]) -> Option<int> = match(items,
    [x, ..] -> Some(x),
    [] -> None
)

// Variant might not match: use match
@get_value (opt: Option<int>) -> int = match(opt,
    Some(n) -> n,
    None -> 0
)
```

### Error on Refutable in Bindings

```ori
// ERROR: refutable pattern in binding
@bad (items: [int]) -> int = run(
    // what if items is empty?
    let [x, ..rest] = items,
    x,
)

// ERROR: refutable pattern in parameter
// what if None?
@bad2 (Some(x): Option<int>) -> int = x
```

---

## Common Patterns

### Swap Values

```ori
@swap (pair: (int, int)) -> (int, int) = run(
    let (a, b) = pair,
    (b, a),
)
```

### Extract and Transform

```ori
@scale_point (p: Point, factor: int) -> Point = run(
    let { x, y } = p,
    Point { x: x * factor, y: y * factor },
)
```

### Filter by Structure

```ori
@get_values (items: [Option<int>]) -> [int] =
    filter(
        .over: items,
        .predicate: is_some,
    ) |> map(
        .over: _,
        .transform: opt ->
            match(opt,
                Some(n) -> n,
                // unreachable
                None -> 0
            ),
    )

// Cleaner with match in filter
@get_positive_values (items: [Option<int>]) -> [int] =
    fold(items, [], accumulator, item ->
        match(item,
            Some(value).match(value > 0) -> accumulator + [value],
            _ -> accumulator
        )
    )
```

### Recursive Processing

```ori
@sum_list (items: [int]) -> int = match(items,
    [] -> 0,
    [head, ..tail] -> head + sum_list(tail)
)

@reverse (items: [int]) -> [int] = match(items,
    [] -> [],
    [head, ..tail] -> reverse(tail) + [head]
)
```

---

## Destructuring with Defaults

Ori doesn't have default values in destructuring, but you can achieve similar effects:

```ori
type Config = { host: str, port: Option<int> }

@get_port (c: Config) -> int = match(c,
    { port: Some(p), .. } -> p,
    // default
    { port: None, .. } -> 8080
)

// Or with helper
@with_default_port (c: Config) -> Config = match(c.port,
    Some(_) -> c,
    None -> Config { host: c.host, port: Some(8080) }
)
```

---

## Error Messages

### Missing Fields

```
error[E0402]: missing fields in destructuring
  |
5 | { x, y } = point3d
  | ^^^^^^^^ missing field: z
  |
help: use `..` to ignore remaining fields: { x, y, .. }
```

### Wrong Field Name

```
error[E0403]: no field named 'nam' in type User
  |
5 | { nam, email } = user
  |   ^^^ did you mean 'name'?
```

### Refutable Pattern

```
error[E0404]: refutable pattern in irrefutable context
  |
5 | [first, ..rest] = items
  | ^^^^^^^^^^^^^^^ pattern might not match
  |
help: use match to handle the empty case:
  | match(items,
  |     [first, ..rest] -> ...,
  |     [] -> ...
  | )
```

### Type Mismatch

```
error[E0308]: mismatched types
  |
5 | { x, y } = user
  | ^^^^^^^^ expected Point, found User
```

---

## Best Practices

### Use Destructuring for Clarity

```ori
// Preferred: clear what fields are used
@distance ({ x, y }: Point) -> float =
    sqrt(float(x * x + y * y))

// Less clear: accessing through variable
@distance_verbose (p: Point) -> float =
    sqrt(float(p.x * p.x + p.y * p.y))
```

### Destructure Close to Usage

```ori
// Preferred: destructure where values are needed
@process (items: [User]) -> void =
    for user in items do run(
        // destructure in loop
        let { name, email, .. } = user,
        send_email(email, "Hello " + name),
    )

// Avoid: destructuring far from usage
@process_far (items: [User]) -> void = run(
    let names_and_emails = map(
        .over: items,
        .transform: u -> ({ u.name, u.email }),
    ),
    // ... many lines later ...
    for { name, email } in names_and_emails do
        send_email(
            .to: email,
            .message: "Hello " + name,
        ),
)
```

### Use `..` Explicitly

```ori
// Preferred: explicit about ignoring fields
let { name, email, .. } = user

// Avoid: trying to match all fields when you don't need them
let { name, email, age, created_at, settings } = user
```

---

## See Also

- [Match Pattern](01-match-pattern.md) — Basic match syntax
- [Guards and Bindings](03-guards-and-bindings.md) — Guards and @ binding
- [Exhaustiveness](04-exhaustiveness.md) — Complete pattern coverage
- [User-Defined Types](../03-type-system/03-user-defined-types.md) — Struct and sum types
