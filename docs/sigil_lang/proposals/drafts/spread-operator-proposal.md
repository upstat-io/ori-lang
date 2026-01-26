# Proposal: Spread Operator

**Status:** Draft
**Author:** Eric
**Created:** 2026-01-25

---

## Summary

Add a spread operator `...` for expanding collections and structs in literal contexts.

```sigil
let combined = [...list1, ...list2]
let merged = {...defaults, ...overrides}
let updated = Point { ...original, x: 10 }
```

---

## Motivation

### The Problem

Combining collections or creating modified copies of structs currently requires verbose code:

**Lists:**
```sigil
// Current: concatenation
let combined = list1 + list2 + list3
let with_extra = items + [new_item]
let with_prefix = [first] + rest

// Awkward for interleaving
let interleaved = [a] + middle + [b]
```

**Maps:**
```sigil
// Current: no clean way to merge
let merged = map1  // How to add map2's entries?

// Must iterate manually or use stdlib
let merged = run(
    let result = map1,
    for (k, v) in map2.entries() do
        result = result.insert(key: k, value: v),
    result,
)
```

**Structs:**
```sigil
// Current: must repeat all fields
let updated = Point {
    x: 10,           // changed
    y: original.y,   // copied
    z: original.z,   // copied
}
```

### Prior Art

| Language | List Spread | Map/Object Spread | Struct Update |
|----------|-------------|-------------------|---------------|
| JavaScript | `[...a, ...b]` | `{...a, ...b}` | `{...obj, x: 1}` |
| Python | `[*a, *b]` | `{**a, **b}` | N/A |
| Kotlin | `listOf(*a, *b)` | N/A | `.copy(x = 1)` |
| Rust | N/A | N/A | `Point { x: 1, ..original }` |
| TypeScript | `[...a, ...b]` | `{...a, ...b}` | `{...obj, x: 1}` |

### The Sigil Way

Use `...` consistently across lists, maps, and structs:
- `[...list]` — spread list elements
- `{...map}` — spread map entries
- `Type { ...struct }` — spread struct fields

---

## Design

### List Spread

Expand list elements in a list literal:

```sigil
let a = [1, 2, 3]
let b = [4, 5, 6]

[...a, ...b]           // [1, 2, 3, 4, 5, 6]
[0, ...a, 10]          // [0, 1, 2, 3, 10]
[...a, ...a]           // [1, 2, 3, 1, 2, 3]
[...[1, 2], ...[3, 4]] // [1, 2, 3, 4]
```

**Order matters:**
```sigil
[...a, ...b]  // a's elements, then b's elements
[...b, ...a]  // b's elements, then a's elements
```

**Mixed with regular elements:**
```sigil
[first, ...middle, last]
[...prefix, separator, ...suffix]
```

### Map Spread

Expand map entries in a map literal:

```sigil
let defaults = {"timeout": 30, "retries": 3}
let custom = {"retries": 5, "verbose": true}

{...defaults, ...custom}
// {"timeout": 30, "retries": 5, "verbose": true}
```

**Later spreads override earlier ones:**
```sigil
{...defaults, ...overrides}  // overrides win on conflicts
{...overrides, ...defaults}  // defaults win on conflicts
```

**Mixed with regular entries:**
```sigil
{...defaults, "timeout": 60}           // override one key
{"extra": true, ...base}               // add before spreading
{...a, "middle": 1, ...b}              // interleave
```

### Struct Spread

Create a new struct copying fields from an existing one:

```sigil
type Point = { x: int, y: int, z: int }

let original = Point { x: 1, y: 2, z: 3 }

Point { ...original, x: 10 }       // Point { x: 10, y: 2, z: 3 }
Point { x: 10, ...original }       // Point { x: 1, y: 2, z: 3 } (original.x wins)
Point { ...original }              // Copy of original
```

**Order determines precedence:**
```sigil
// Explicit fields after spread = override
Point { ...original, x: 10 }  // x is 10

// Explicit fields before spread = original wins
Point { x: 10, ...original }  // x is 1 (original's value)
```

**Multiple spreads:**
```sigil
type Config = { a: int, b: int, c: int }

let base = Config { a: 1, b: 2, c: 3 }
let patch = Config { a: 10, b: 20, c: 30 }

Config { ...base, ...patch }  // patch wins: { a: 10, b: 20, c: 30 }
Config { ...base, b: 100 }    // { a: 1, b: 100, c: 3 }
```

### Type Constraints

**Lists:** All spread elements must be lists of the same element type:
```sigil
let ints = [1, 2, 3]
let strs = ["a", "b"]

[...ints, ...strs]  // Error: cannot spread [str] into [int]
```

**Maps:** All spread elements must be maps with compatible key/value types:
```sigil
let a = {"x": 1}
let b = {"y": "two"}

{...a, ...b}  // Error: incompatible value types int and str
```

**Structs:** Can only spread the same struct type:
```sigil
type Point2D = { x: int, y: int }
type Point3D = { x: int, y: int, z: int }

let p2 = Point2D { x: 1, y: 2 }

Point3D { ...p2, z: 3 }  // Error: cannot spread Point2D into Point3D
```

---

## Examples

### Configuration Merging

```sigil
let $defaults = {
    "timeout": 30s,
    "retries": 3,
    "verbose": false,
}

@create_client (overrides: {str: any}) -> Client = run(
    let config = {...$defaults, ...overrides},
    Client.new(config: config),
)

// Usage
create_client(overrides: {"timeout": 60s})
// Config: {"timeout": 60s, "retries": 3, "verbose": false}
```

### Immutable Updates

```sigil
type User = { id: int, name: str, email: str, active: bool }

@deactivate (user: User) -> User =
    User { ...user, active: false }

@update_email (user: User, new_email: str) -> User =
    User { ...user, email: new_email }

@update_profile (user: User, name: str, email: str) -> User =
    User { ...user, name: name, email: email }
```

### Building Lists

```sigil
@surround<T> (items: [T], before: T, after: T) -> [T] =
    [before, ...items, after]

@interleave<T> (a: [T], b: [T], separator: [T]) -> [T] =
    [...a, ...separator, ...b]

@flatten<T> (lists: [[T]]) -> [T] = run(
    let result = [],
    for list in lists do
        result = [...result, ...list],
    result,
)
```

### Request Building

```sigil
type Request = {
    method: str,
    url: str,
    headers: {str: str},
    body: Option<str>,
}

let $base_headers = {
    "Content-Type": "application/json",
    "Accept": "application/json",
}

@post (url: str, body: str, extra_headers: {str: str} = {}) -> Request =
    Request {
        method: "POST",
        url: url,
        headers: {...$base_headers, ...extra_headers},
        body: Some(body),
    }
```

### State Updates (Redux-style)

```sigil
type AppState = {
    user: Option<User>,
    items: [Item],
    loading: bool,
    error: Option<str>,
}

@reduce (state: AppState, action: Action) -> AppState = match(action,
    LoadStart -> AppState { ...state, loading: true, error: None },
    LoadSuccess(items) -> AppState { ...state, loading: false, items: items },
    LoadError(msg) -> AppState { ...state, loading: false, error: Some(msg) },
    AddItem(item) -> AppState { ...state, items: [...state.items, item] },
    ClearItems -> AppState { ...state, items: [] },
)
```

### Variadic-like Patterns

```sigil
@log (level: str, messages: [str]) -> void =
    print(msg: `[{level}] {messages |> join(separator: " ", items: _)}`)

// Can spread arguments
let context = ["user=123", "action=login"]
log(level: "INFO", messages: ["Request received", ...context])
```

---

## Design Rationale

### Why `...` Syntax?

| Syntax | Precedent | Notes |
|--------|-----------|-------|
| `...x` | JavaScript, TypeScript | Most widely known |
| `*x` / `**x` | Python | Conflicts with multiplication |
| `..x` | Rust (struct update) | Conflicts with range `..` |
| `@x` | None | `@` used for functions |

`...` is familiar from JavaScript/TypeScript, unambiguous, and visually clear.

### Why Order-Based Precedence?

When spreading maps or structs, later entries override earlier ones. This matches:
- JavaScript object spread behavior
- Intuitive "last write wins" semantics
- Left-to-right evaluation order

```sigil
{...defaults, ...overrides}  // overrides win — intuitive
```

### Why Not a Method?

```sigil
// Alternative: method-based
list1.concat(list2)
map1.merge(map2)
original.with(x: 10)
```

Methods work but:
1. Less visual — spread shows structure at a glance
2. Multiple spreads require chaining
3. Struct field update is awkward as a method

### Why Allow Spread Anywhere in Literal?

Some languages restrict spread position. Sigil allows it anywhere:

```sigil
[first, ...middle, last]           // Valid
{...base, key: val, ...more}       // Valid
Point { x: 1, ...rest, z: 3 }      // Valid
```

This provides maximum flexibility for composition.

---

## Edge Cases

### Empty Spread

Spreading empty collections is valid and produces nothing:

```sigil
let empty = []
[1, ...empty, 2]  // [1, 2]

let empty_map = {}
{...empty_map, "a": 1}  // {"a": 1}
```

### Nested Spread

Spread is shallow — only one level:

```sigil
let nested = [[1, 2], [3, 4]]
[...nested]  // [[1, 2], [3, 4]] — NOT [1, 2, 3, 4]

// For deep flatten, use explicit logic
nested |> flatten(lists: _)  // [1, 2, 3, 4]
```

### Spread in Function Arguments

Spread only works in literal contexts, not function calls:

```sigil
@sum (a: int, b: int, c: int) -> int = a + b + c

let args = [1, 2, 3]
sum(...args)  // Error: spread not allowed in function calls

// Use explicit arguments
sum(a: args[0], b: args[1], c: args[2])
```

This maintains Sigil's explicit named-argument philosophy.

---

## Implementation Notes

### Parser Changes

Add `...` as a prefix operator in list, map, and struct literal contexts:

```
list_element = "..." expression | expression .
map_entry = "..." expression | expression ":" expression .
struct_field = "..." expression | identifier ":" expression | identifier .
```

### Type Checking

- Verify spread expression type matches container type
- For structs, verify all required fields are provided (via spread or explicit)
- Track field coverage to detect missing fields

### Desugaring

**List spread:**
```sigil
[a, ...b, c]
// Desugars to:
[a] + b + [c]
```

**Map spread:**
```sigil
{...a, "key": val, ...b}
// Desugars to:
a.merge({"key": val}).merge(b)
```

**Struct spread:**
```sigil
Point { ...original, x: 10 }
// Desugars to:
Point { x: 10, y: original.y, z: original.z }
```

---

## Summary

| Context | Syntax | Result |
|---------|--------|--------|
| List | `[...a, ...b]` | Concatenated list |
| Map | `{...a, ...b}` | Merged map (later wins) |
| Struct | `T { ...s, x: v }` | Updated struct copy |

The spread operator `...` provides concise, readable syntax for composing collections and creating modified copies of structs, following the familiar JavaScript/TypeScript pattern while maintaining Sigil's explicit philosophy.
