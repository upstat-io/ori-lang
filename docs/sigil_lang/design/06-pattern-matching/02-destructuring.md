# Destructuring

This document covers destructuring in Sigil: extracting values from structs, lists, and sum types in bindings, function parameters, and match patterns.

---

## Overview

Destructuring lets you extract values from compound types in a single operation. Instead of accessing fields one by one, you bind multiple values at once while also verifying structure.

```sigil
type Point = { x: int, y: int }

// Without destructuring
@distance_verbose (p: Point) -> float = run(
    x = p.x,
    y = p.y,
    sqrt(float(x * x + y * y))
)

// With destructuring
@distance (p: Point) -> float = run(
    { x, y } = p,
    sqrt(float(x * x + y * y))
)
```

---

## Struct Destructuring

### Basic Syntax

Extract fields by name:

```sigil
type User = { name: str, email: str, age: int }

@greet (u: User) -> str = run(
    { name, age } = u,
    name + " is " + str(age) + " years old"
)
```

**Key points:**
- `{ field }` binds field to same-named variable
- Only listed fields are bound
- Type is checked at compile time

### Binding to Different Names

Use `: new_name` to bind to a different variable:

```sigil
@process (u: User) -> str = run(
    { name: user_name, email: user_email } = u,
    user_name + " <" + user_email + ">"
)
```

### Ignoring Fields with `..`

Use `..` to ignore unmentioned fields:

```sigil
type Record = {
    id: int,
    name: str,
    created_at: Timestamp,
    updated_at: Timestamp,
    metadata: {str: str}
}

// Only need name and id
@summarize (r: Record) -> str = run(
    { id, name, .. } = r,
    str(id) + ": " + name
)
```

**Without `..`:**
```sigil
// ERROR: struct has fields 'created_at', 'updated_at', 'metadata' not mentioned
@bad (r: Record) -> str = run(
    { id, name } = r,  // missing fields
    str(id) + ": " + name
)
```

### Match-All Pattern

Use `{ .. }` to match a struct without binding any fields:

```sigil
@is_user (u: User) -> bool = run(
    { .. } = u,  // just verify it's a User
    true
)
```

This is useful in match arms where you need to match the type but not extract values.

---

## Destructuring in Function Parameters

Destructure directly in parameter position:

```sigil
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

```sigil
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

```sigil
@first (items: [int]) -> Option<int> = match(items,
    [] -> None,
    [x, ..] -> Some(x)
)
```

### Head and Tail

The common functional pattern:

```sigil
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

```sigil
@process_pair (items: [int]) -> Option<int> = match(items,
    [a, b] -> Some(a + b),  // exactly two
    _ -> None
)

@process_triple (items: [int]) -> Option<int> = match(items,
    [a, b, c] -> Some(a + b + c),  // exactly three
    _ -> None
)
```

### First and Last

```sigil
@first_and_last (items: [int]) -> Option<(int, int)> = match(items,
    [] -> None,
    [x] -> Some((x, x)),
    [first, .., last] -> Some((first, last))
)
```

### In Bindings

List destructuring works in bindings when the pattern is irrefutable:

```sigil
// Only safe when you know the list has elements
@process_known (items: [int]) -> int = run(
    // This will panic if list is empty
    [first, second, ..rest] = items,
    first + second + fold(rest, 0, +)
)
```

**Note:** For refutable patterns (might fail), use `match`:

```sigil
// Safe: handles empty case
@process_safe (items: [int]) -> int = match(items,
    [first, second, ..rest] -> first + second + fold(rest, 0, +),
    [single] -> single,
    [] -> 0
)
```

---

## Variant Destructuring

### Single-Value Variants

```sigil
type Option<T> = Some(T) | None

@unwrap_or (opt: Option<int>, default: int) -> int = match(opt,
    Some(value) -> value,
    None -> default
)
```

### Named Field Variants

```sigil
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

```sigil
type Tree<T> = Leaf(T) | Node(left: Tree<T>, right: Tree<T>)

@flatten (t: Tree<int>) -> [int] = match(t,
    Leaf(value) -> [value],
    Node(left: l, right: r) -> flatten(l) + flatten(r)
)
```

---

## Nested Destructuring

### Structs within Structs

```sigil
type Address = { street: str, city: str }
type Person = { name: str, address: Address }

@get_city ({ address: { city, .. }, .. }: Person) -> str = city

// In match
@describe_location (p: Person) -> str = match(p,
    { address: { city, .. }, .. } -> "Lives in " + city
)
```

### Lists within Structs

```sigil
type Group = { name: str, members: [User] }

@first_member (g: Group) -> Option<str> = match(g,
    { members: [first, ..], .. } -> Some(first.name),
    { members: [], .. } -> None
)
```

### Variants within Structs

```sigil
type Container = { value: Option<int>, label: str }

@describe (c: Container) -> str = match(c,
    { value: Some(n), label } -> label + ": " + str(n),
    { value: None, label } -> label + ": empty"
)
```

### Deep Nesting

```sigil
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

```sigil
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

```sigil
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

```sigil
@process (u: User) -> str = run(
    { name, age }: { name: str, age: int, .. } = u,
    name + " is " + str(age)
)

// In parameters
@greet ({ name, age, .. }: User) -> str =
    name + " (" + str(age) + ")"
```

---

## Destructuring in Pattern Contexts

### In `run` Bindings

```sigil
@process (users: [User]) -> [str] = run(
    [first, ..rest] = users,
    { name, .. } = first,
    names = map(rest, u -> u.name),
    [name] + names
)
```

### In `try` Bindings

```sigil
@fetch_user_name (id: int) -> Result<str, Error> = try(
    response = fetch_user(id),
    { name, .. } = response.body,  // destructure the body
    Ok(name)
)
```

### In `for` Iteration

```sigil
@sum_x_coords (points: [Point]) -> int = run(
    total = 0,
    for { x, .. } in points do total = total + x,
    total
)
```

---

## Refutable vs Irrefutable Patterns

### Irrefutable Patterns

Always succeed, safe in bindings and parameters:

```sigil
// Struct: always succeeds (fields always exist)
{ x, y } = point

// Function parameter: always succeeds
@process ({ name, .. }: User) -> str = ...
```

### Refutable Patterns

Might fail, require `match`:

```sigil
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

```sigil
// ERROR: refutable pattern in binding
@bad (items: [int]) -> int = run(
    [x, ..rest] = items,  // what if items is empty?
    x
)

// ERROR: refutable pattern in parameter
@bad2 (Some(x): Option<int>) -> int = x  // what if None?
```

---

## Common Patterns

### Swap Values

```sigil
@swap (pair: (int, int)) -> (int, int) = run(
    (a, b) = pair,
    (b, a)
)
```

### Extract and Transform

```sigil
@scale_point (p: Point, factor: int) -> Point = run(
    { x, y } = p,
    Point { x: x * factor, y: y * factor }
)
```

### Filter by Structure

```sigil
@get_values (items: [Option<int>]) -> [int] =
    filter(items, is_some) |> map(opt ->
        match(opt,
            Some(n) -> n,
            None -> 0  // unreachable
        )
    )

// Cleaner with match in filter
@get_positive_values (items: [Option<int>]) -> [int] =
    fold(items, [], acc, item ->
        match(item,
            Some(n).match(n > 0) -> acc + [n],
            _ -> acc
        )
    )
```

### Recursive Processing

```sigil
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

Sigil doesn't have default values in destructuring, but you can achieve similar effects:

```sigil
type Config = { host: str, port: Option<int> }

@get_port (c: Config) -> int = match(c,
    { port: Some(p), .. } -> p,
    { port: None, .. } -> 8080  // default
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

```sigil
// Preferred: clear what fields are used
@distance ({ x, y }: Point) -> float =
    sqrt(float(x * x + y * y))

// Less clear: accessing through variable
@distance_verbose (p: Point) -> float =
    sqrt(float(p.x * p.x + p.y * p.y))
```

### Destructure Close to Usage

```sigil
// Preferred: destructure where values are needed
@process (items: [User]) -> void =
    for user in items do run(
        { name, email, .. } = user,  // destructure in loop
        send_email(email, "Hello " + name)
    )

// Avoid: destructuring far from usage
@process_far (items: [User]) -> void = run(
    names_and_emails = map(items, u -> ({ u.name, u.email })),
    // ... many lines later ...
    for { name, email } in names_and_emails do
        send_email(email, "Hello " + name)
)
```

### Use `..` Explicitly

```sigil
// Preferred: explicit about ignoring fields
{ name, email, .. } = user

// Avoid: trying to match all fields when you don't need them
{ name, email, age, created_at, settings } = user
```

---

## See Also

- [Match Pattern](01-match-pattern.md) — Basic match syntax
- [Guards and Bindings](03-guards-and-bindings.md) — Guards and @ binding
- [Exhaustiveness](04-exhaustiveness.md) — Complete pattern coverage
- [User-Defined Types](../03-type-system/03-user-defined-types.md) — Struct and sum types
