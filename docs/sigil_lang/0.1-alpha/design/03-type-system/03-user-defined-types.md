# User-Defined Types

This document covers defining custom types in Sigil: structs, newtypes, and sum types (enums).

---

## Type Definition Syntax

All type definitions use the `type` keyword:

```sigil
type Name = definition
```

---

## Structs (Product Types)

Structs combine multiple fields into a single type.

### Definition

```sigil
type Point = { x: int, y: int }

type User = {
    id: int,
    name: str,
    email: str,
    active: bool
}
```

### Construction

```sigil
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

```sigil
let x_coord = origin.x
let user_name = alice.name
```

### Field Shorthand

When variable name matches field name:

```sigil
@create_point (x: int, y: int) -> Point = Point { x, y }
// Equivalent to: Point { x: x, y: y }
```

### Destructuring

```sigil
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

```sigil
type Point2D = { x: int, y: int }
type Vector2D = { x: int, y: int }

// Point2D != Vector2D even though fields match
```

---

## Newtypes (Opaque Aliases)

Newtypes create distinct types from existing ones.

### Definition

```sigil
type UserId = str
type Email = str
type Timestamp = int
type Hash = str
```

### Why Newtypes?

Prevent mixing up values of the same underlying type:

```sigil
type UserId = str
type Email = str

@find_user (id: UserId) -> Option<User> = ...

// Type error: Email != UserId
let email = Email("alice@example.com")
find_user(email)  // ERROR
```

### Construction

```sigil
let user_id = UserId("user-123")
let email = Email("alice@example.com")
```

### Unwrapping

```sigil
// Get underlying value
let raw_id: str = user_id.unwrap()
```

### With Traits

Derive traits for newtypes:

```sigil
#[derive(Eq, Hashable, Printable)]
type UserId = str
```

---

## Sum Types (Enums)

Sum types represent values that can be one of several variants.

### Simple Variants

```sigil
type Status = Pending | Running | Completed | Failed
```

### Variants with Data

```sigil
type Status =
    | Pending
    | Running(progress: float)
    | Completed
    | Failed(error: str)

type Option<T> = Some(T) | None

type Result<T, E> = Ok(T) | Err(E)
```

### Construction

```sigil
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

```sigil
@describe (s: Status) -> str = match(s,
    Pending -> "waiting",
    Running(p) -> "at " + str(p * 100) + "%",
    Completed -> "done",
    Failed(e) -> "error: " + e
)
```

### Variant Namespacing

When variants might conflict:

```sigil
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

```sigil
type Pair<T> = { first: T, second: T }

type KeyValue<K, V> = { key: K, value: V }
```

### Generic Sum Types

```sigil
type Option<T> = Some(T) | None

type Result<T, E> = Ok(T) | Err(E)

type Tree<T> =
    | Leaf(value: T)
    | Node(left: Tree<T>, right: Tree<T>)
```

### Usage

```sigil
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

```sigil
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

```sigil
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

```sigil
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

```sigil
// Good: distinct types prevent mistakes
type UserId = str
type Email = str
type PasswordHash = str

// Bad: easy to mix up strings
@find_user (id: str, email: str) -> ...
```

### Use Sum Types for State

```sigil
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

```sigil
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

```sigil
// Good: these sets ARE closed
type Option<T> = Some(T) | None
type Result<T, E> = Ok(T) | Err(E)
type Ordering = Less | Equal | Greater

// Good: internal state machine, closed by design
type ConnectionState = Connecting | Connected | Disconnecting | Closed
```

**Use traits when users need to add their own types:**

```sigil
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

## See Also

- [Generics](04-generics.md)
- [Traits](../04-traits/index.md)
- [Pattern Matching](../06-pattern-matching/index.md)
