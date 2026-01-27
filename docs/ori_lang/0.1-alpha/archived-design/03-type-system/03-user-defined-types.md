# User-Defined Types

This document covers defining custom types in Ori: structs, newtypes, and sum types (enums).

---

## Type Definition Syntax

All type definitions use the `type` keyword:

```ori
type Name = definition
```

---

## Structs (Product Types)

Structs combine multiple fields into a single type.

### Definition

```ori
type Point = { x: int, y: int }

type User = {
    id: int,
    name: str,
    email: str,
    active: bool
}
```

### Construction

```ori
// All fields required
let origin = Point { x: 0, y: 0 }

let alice = User {
    id: 1,
    name: "Alice",
    email: "alice@example.com",
    active: true,
}
```

### Field Access

```ori
let x_coord = origin.x
let user_name = alice.name
```

### Field Shorthand

When variable name matches field name:

```ori
@create_point (x: int, y: int) -> Point = Point { x, y }
// Equivalent to: Point { x: x, y: y }
```

### Destructuring

```ori
@distance (p: Point) -> float = run(
    let { x, y } = p,
    sqrt(float(x * x + y * y)),
)

// In function parameters
@distance ({ x, y }: Point) -> float =
    sqrt(float(x * x + y * y))
```

### Nominal Typing

Structs are nominally typed—structure alone doesn't make types equal:

```ori
type Point2D = { x: int, y: int }
type Vector2D = { x: int, y: int }

// Point2D != Vector2D even though fields match
```

---

## Newtypes (Opaque Aliases)

Newtypes create distinct types from existing ones.

### Definition

```ori
type UserId = str
type Email = str
type Timestamp = int
type Hash = str
```

### Why Newtypes?

Prevent mixing up values of the same underlying type:

```ori
type UserId = str
type Email = str

@find_user (id: UserId) -> Option<User> = ...

// Type error: Email != UserId
let email = Email("alice@example.com")
// ERROR
find_user(email)
```

### Construction

```ori
let user_id = UserId("user-123")
let email = Email("alice@example.com")
```

### Unwrapping

```ori
// Get underlying value
let raw_id: str = user_id.unwrap()
```

### With Traits

Derive traits for newtypes:

```ori
#[derive(Eq, Hashable, Printable)]
type UserId = str
```

---

## Sum Types (Enums)

Sum types represent values that can be one of several variants.

### Simple Variants

```ori
type Status = Pending | Running | Completed | Failed
```

### Variants with Data

```ori
type Status =
    | Pending
    | Running(progress: float)
    | Completed
    | Failed(error: str)

type Option<T> = Some(T) | None

type Result<T, E> = Ok(T) | Err(E)
```

### Construction

```ori
let status1 = Pending
let status2 = Running(progress: 0.5)
let status3 = Failed(error: "connection lost")

let maybe_value = Some(42)
let no_value: Option<int> = None

let success = Ok(result)
let failure = Err(error)
```

### Pattern Matching

Sum types are typically used with match:

```ori
@describe (s: Status) -> str = match(s,
    Pending -> "waiting",
    Running(p) -> "at " + str(p * 100) + "%",
    Completed -> "done",
    Failed(e) -> "error: " + e
)
```

### Variant Namespacing

When variants might conflict:

```ori
type Color = Red | Green | Blue
type Priority = Low | Medium | High

// Unambiguous
let color = Red

// If ambiguous, qualify
let color = Color.Red
let priority = Priority.High
```

---

## Generic Types

Types can have type parameters.

### Generic Structs

```ori
type Pair<T> = { first: T, second: T }

type KeyValue<K, V> = { key: K, value: V }
```

### Generic Sum Types

```ori
type Option<T> = Some(T) | None

type Result<T, E> = Ok(T) | Err(E)

type Tree<T> =
    | Leaf(value: T)
    | Node(left: Tree<T>, right: Tree<T>)
```

### Usage

```ori
let pair_of_ints = Pair { first: 1, second: 2 }
let pair_of_strings = Pair { first: "a", second: "b" }

let tree: Tree<int> = Node(
    left: Leaf(value: 1),
    right: Node(
        left: Leaf(value: 2),
        right: Leaf(value: 3),
    ),
)
```

---

## Public Types

Export types with `pub`:

```ori
pub type User = {
    id: int,
    name: str,
    email: str
}

pub type AuthError =
    | NotFound
    | Unauthorized
    | Expired
```

---

## Type Documentation

```ori
// #User account with authentication credentials
// @field email must be valid email format
// @field active whether account is enabled
type User = {
    id: int,
    name: str,
    email: str,
    active: bool
}
```

---

## Deriving Traits

Auto-implement common traits:

```ori
#[derive(Eq, Hashable, Printable, Clone)]
type User = {
    id: int,
    name: str,
    email: str
}

// Now User has:
// - Equality comparison (==, !=)
// - Hash computation
// - String conversion
// - Cloning
```

### Derivable Traits

| Trait | Behavior |
|-------|----------|
| `Eq` | Field-by-field equality |
| `Hashable` | Combine field hashes |
| `Comparable` | Lexicographic by field order |
| `Printable` | Debug-style output |
| `Clone` | Field-by-field copy |
| `Default` | Default values for all fields |
| `Serialize` | JSON serialization |
| `Deserialize` | JSON deserialization |

---

## Best Practices

### Use Newtypes for Domain Concepts

```ori
// Good: distinct types prevent mistakes
type UserId = str
type Email = str
type PasswordHash = str

// Bad: easy to mix up strings
@find_user (id: str, email: str) -> ...
```

### Use Sum Types for State

```ori
// Good: states are explicit
type ConnectionState =
    | Disconnected
    | Connecting
    | Connected(session: Session)
    | Error(message: str)

// Bad: multiple booleans
type Connection = {
    is_connected: bool,
    is_connecting: bool,
    has_error: bool,
    ...
}
```

### Keep Types Small

```ori
// Good: focused types
type Address = { street: str, city: str, country: str }
type Contact = { email: str, phone: str }
type User = { id: UserId, name: str, address: Address, contact: Contact }

// Bad: god object
type User = { id: int, name: str, street: str, city: str, ... }
```

### Sum Types vs Traits: The Expression Problem

Sum types are **closed** — all variants are known at definition time. This enables exhaustive matching but prevents extension.

**Use sum types when the set is truly fixed:**

```ori
// Good: these sets ARE closed
type Option<T> = Some(T) | None
type Result<T, E> = Ok(T) | Err(E)
type Ordering = Less | Equal | Greater

// Good: internal state machine, closed by design
type ConnectionState = Connecting | Connected | Disconnecting | Closed
```

**Use traits when users need to add their own types:**

```ori
// Library wants users to add widgets
trait Widget {
    @render (self, ctx: Context) -> void
    @handle_event (self, event: Event) -> void
}

// Library provides some implementations
type Button = { label: str, on_click: () -> void }
impl Widget for Button { ... }

type Slider = { value: float, range: (float, float) }
impl Widget for Slider { ... }

// Users can add their own
type MyCustomGraph = { data: [Point] }
impl Widget for MyCustomGraph { ... }
```

**Decision guide:**

| Scenario | Use |
|----------|-----|
| Fixed, known set (Result, Option, Ordering) | Sum type |
| Internal state machines | Sum type |
| Error types with known variants | Sum type |
| User-extensible abstractions | Trait |
| Plugin/extension systems | Trait |
| Library APIs meant for extension | Trait |

See [Traits](../04-traits/index.md) for defining extensible abstractions.

---

## Real-World Example: HTTP API

This example demonstrates sum types, structs, newtypes, and pattern matching working together in a realistic HTTP routing scenario.

### Defining the Types

```ori
// Sum type for HTTP methods - a closed, known set
type HttpMethod = Get | Post | Put | Delete | Patch

// Sum type for response status - variants with associated data
type HttpStatus =
    // 200
    | Ok
    // 201
    | Created
    // 204
    | NoContent
    // 400 + message
    | BadRequest(str)
    // 404
    | NotFound
    // 500 + message
    | InternalError(str)

// Struct for route configuration
type Route = {
    method: HttpMethod,
    path: str,
    handler: (Request) -> Response
}

// Struct for theming with known fields
type Theme = {
    primary: str,
    secondary: str,
    error: str
}
```

### Using the Types

```ori
// Route definitions are fully typed
let routes: [Route] = [
    { method: Get, path: "/users", handler: list_users },
    { method: Post, path: "/users", handler: create_user },
    { method: Put, path: "/users/{id}", handler: update_user },
    { method: Delete, path: "/users/{id}", handler: delete_user },
]

// Theme with compile-time field checking
let theme = Theme {
    primary: "#3b82f6",
    secondary: "#64748b",
    error: "#ef4444",
}

// OK
theme.primary
// theme.success
// Compile error: 'success' not a field of Theme
```

### Exhaustive Matching

```ori
// Convert method to string - compiler ensures all variants handled
@method_to_str (m: HttpMethod) -> str = match(m,
    Get -> "GET",
    Post -> "POST",
    Put -> "PUT",
    Delete -> "DELETE",
    Patch -> "PATCH"
)

// Get status code - pattern match with associated data
@status_code (s: HttpStatus) -> int = match(s,
    Ok -> 200,
    Created -> 201,
    NoContent -> 204,
    BadRequest(_) -> 400,
    NotFound -> 404,
    InternalError(_) -> 500
)
```

### Boundary Parsing

At external boundaries (JSON input, HTTP headers), parse strings into proper types:

```ori
@parse_method (input: str) -> Result<HttpMethod, ParseError> = match(input,
    "GET" -> Ok(Get),
    "POST" -> Ok(Post),
    "PUT" -> Ok(Put),
    "DELETE" -> Ok(Delete),
    "PATCH" -> Ok(Patch),
    _ -> Err(ParseError { message: "Invalid HTTP method: " + input })
)
```

### Why Sum Types Over String Literals

Some languages use string literal types (e.g., `"GET" | "POST"`). Ori uses sum types because:

1. **Distinct from strings** — `Get` is not a `str`, preventing accidental mixing
2. **Exhaustive matching** — compiler verifies all variants are handled
3. **Explicit boundaries** — parsing happens once at the edge, not throughout code
4. **Self-documenting** — type definition shows all valid values

---

## See Also

- [Generics](04-generics.md)
- [Traits](../04-traits/index.md)
- [Pattern Matching](../06-pattern-matching/index.md)
