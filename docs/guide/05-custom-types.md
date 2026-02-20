---
title: "Custom Types"
description: "Structs, sum types, generics, and methods."
order: 5
part: "Data"
---

# Custom Types

Ori lets you define your own types to model your domain. This guide covers structs for grouping related data, sum types for representing alternatives, and how to add methods to your types.

## Struct Types

Structs group related data with named fields.

### Defining Structs

```ori
type Point = { x: int, y: int }

type User = {
    id: int,
    name: str,
    email: str,
    active: bool,
}

type Config = {
    host: str,
    port: int,
    timeout: Duration,
}
```

### Creating Struct Instances

```ori
let origin = Point { x: 0, y: 0 }

let alice = User {
    id: 1,
    name: "Alice",
    email: "alice@example.com",
    active: true,
}
```

**Field shorthand** — when variable names match field names:

```ori
let x = 10
let y = 20
let point = Point { x, y }    // Same as Point { x: x, y: y }

let name = "Bob"
let email = "bob@example.com"
let bob = User {
    id: 2,
    name,           // Shorthand
    email,          // Shorthand
    active: true,
}
```

### Accessing Fields

Use dot notation:

```ori
alice.name      // "Alice"
alice.email     // "alice@example.com"
origin.x        // 0
```

### Updating Structs

Structs are immutable. Create new ones with spread:

```ori
let moved_point = Point { ...origin, x: 5 }
// Point { x: 5, y: 0 }

let deactivated = User { ...alice, active: false }
// Same user but inactive
```

Spread copies all fields, then specific fields override:

```ori
let updated = Config {
    ...defaults,
    timeout: 60s,    // Override just timeout
}
```

### Nested Structs

Structs can contain other structs:

```ori
type Address = { street: str, city: str, zip: str }

type Person = {
    name: str,
    home: Address,
    work: Address,
}

let person = Person {
    name: "Alice",
    home: Address { street: "123 Main", city: "Boston", zip: "02101" },
    work: Address { street: "456 Office", city: "Cambridge", zip: "02142" },
}

person.home.city    // "Boston"
person.work.zip     // "02142"
```

## Sum Types

Sum types (also called enums or tagged unions) represent values that can be one of several variants.

### Simple Variants

```ori
type Color = Red | Green | Blue

type Direction = North | South | East | West

type Ordering = Less | Equal | Greater
```

Creating values:

```ori
let color = Red
let direction = North
```

### Variants with Data

Variants can carry data:

```ori
type Shape =
    | Circle(radius: float)
    | Rectangle(width: float, height: float)
    | Triangle(a: float, b: float, c: float)
```

Creating values:

```ori
let circle = Circle(radius: 5.0)
let rect = Rectangle(width: 10.0, height: 20.0)
```

### Why Sum Types Matter

Sum types make illegal states unrepresentable:

```ori
// BAD: boolean flags create impossible states
type Connection = {
    connected: bool,
    authenticated: bool,
    error_message: Option<str>,
}
// What does connected=false, authenticated=true mean?

// GOOD: sum type enforces valid states
type Connection =
    | Disconnected
    | Connected
    | Authenticated(user: User)
    | Error(message: str)
// Each state is distinct and meaningful
```

### Built-in Sum Types

Ori provides important sum types in the prelude:

**`Option<T>`** — value that might not exist:

```ori
type Option<T> = Some(T) | None

let found = Some(42)
let not_found: Option<int> = None
```

**`Result<T, E>`** — operation that might fail:

```ori
type Result<T, E> = Ok(T) | Err(E)

let success = Ok(42)
let failure = Err("something went wrong")
```

**`Ordering`** — comparison result:

```ori
type Ordering = Less | Equal | Greater
```

### Working with Sum Types

You **must** use pattern matching to work with sum types:

```ori
type Status = Pending | Running | Done | Failed(reason: str)

@describe (s: Status) -> str = match s {
    Pending -> "Waiting to start"
    Running -> "In progress"
    Done -> "Complete"
    Failed(reason) -> `Failed: {reason}`
}
```

The compiler ensures you handle every case.

## Generic Types

Make types work with any inner type.

### Generic Structs

```ori
type Pair<T> = { first: T, second: T }
type Box<T> = { value: T }
type Entry<K, V> = { key: K, value: V }
```

Use with concrete types:

```ori
let int_pair: Pair<int> = Pair { first: 1, second: 2 }
let str_pair: Pair<str> = Pair { first: "a", second: "b" }
let entry: Entry<str, int> = Entry { key: "count", value: 42 }
```

### Generic Sum Types

```ori
type Tree<T> =
    | Leaf(value: T)
    | Node(left: Tree<T>, right: Tree<T>)

let tree = Node(
    left: Leaf(value: 1),
    right: Node(
        left: Leaf(value: 2),
        right: Leaf(value: 3),
    ),
)
```

## Newtypes

Create distinct types from existing ones:

```ori
type UserId = int
type Email = str
type Meters = float
type Seconds = float
```

This provides type safety:

```ori
@send_email (to: Email, from: Email) -> void = ...

let user_id: UserId = 42
let email: Email = "alice@example.com"

send_email(to: email, from: email)      // OK
send_email(to: user_id, from: email)    // ERROR: type mismatch
```

## Deriving Traits

Automatically implement common behavior:

```ori
#derive(Eq, Clone, Debug, Printable)
type Point = { x: int, y: int }
```

| Trait | What You Get |
|-------|--------------|
| `Eq` | `==` and `!=` comparison |
| `Clone` | `.clone()` method |
| `Debug` | `.debug()` for developer output |
| `Printable` | `.to_str()` for user output |
| `Comparable` | `<`, `>`, `<=`, `>=` |
| `Hashable` | Can be used as map keys |
| `Default` | `Type.default()` constructor |

```ori
let a = Point { x: 1, y: 2 }
let b = Point { x: 1, y: 2 }
let c = a.clone()

a == b       // true (Eq)
a.debug()    // "Point { x: 1, y: 2 }" (Debug)
```

## Adding Methods with `impl`

Add methods to your custom types:

```ori
type Point = { x: int, y: int }

impl Point {
    // Static method (no self) - constructor
    @new (x: int, y: int) -> Point = Point { x, y }

    @origin () -> Point = Point { x: 0, y: 0 }

    // Instance methods (take self)
    @magnitude (self) -> float =
        sqrt(x: float(self.x * self.x + self.y * self.y))

    @distance_to (self, other: Point) -> float = {
        let dx = self.x - other.x
        let dy = self.y - other.y
        sqrt(x: float(dx * dx + dy * dy))
    }

    @translate (self, dx: int, dy: int) -> Point =
        Point { x: self.x + dx, y: self.y + dy }
}
```

### Using Methods

```ori
// Static methods called on type
let a = Point.new(x: 0, y: 0)
let b = Point.new(x: 3, y: 4)

// Instance methods called on value
b.magnitude()              // 5.0
a.distance_to(other: b)    // 5.0
b.translate(dx: 1, dy: 1)  // Point { x: 4, y: 5 }
```

### The `self` and `Self` Keywords

- `self` — the instance the method is called on
- `Self` — the implementing type itself

```ori
impl Point {
    @clone (self) -> Self = Point { x: self.x, y: self.y }
}
```

### Methods on Sum Types

```ori
type Shape =
    | Circle(radius: float)
    | Rectangle(width: float, height: float)

impl Shape {
    @area (self) -> float = match self {
        Circle(radius) -> 3.14159 * radius * radius
        Rectangle(width, height) -> width * height
    }

    @perimeter (self) -> float = match self {
        Circle(radius) -> 2.0 * 3.14159 * radius
        Rectangle(width, height) -> 2.0 * (width + height)
    }
}

let circle = Circle(radius: 5.0)
circle.area()        // 78.54
circle.perimeter()   // 31.42
```

## Destructuring

Extract fields with pattern matching.

### Struct Destructuring

```ori
let Point { x, y } = origin
// x = 0, y = 0

let User { name, email, .. } = alice
// name = "Alice", email = "alice@example.com"
// .. ignores remaining fields
```

**Rename during destructuring:**

```ori
let Point { x: px, y: py } = origin
// px = 0, py = 0
```

**Nested destructuring:**

```ori
let Person { name, home: Address { city, .. } } = person
// name = "Alice", city = "Boston"
```

### Tuple Destructuring

```ori
let (a, b) = (10, 20)
let (first, _, third) = (1, 2, 3)  // Ignore second
```

### List Destructuring

```ori
let [$head, ..tail] = items     // head immutable
let [first, second, ..rest] = items
let [only] = single_item_list    // Panics if not exactly one element
```

### Immutability in Destructuring

Control mutability per binding:

```ori
let { $x, y } = point    // x immutable, y mutable
let ($a, b) = tuple      // a immutable, b mutable
```

## Visibility

Control access to types and their fields:

```ori
// Public type with public fields
pub type Config = {
    pub host: str,
    pub port: int,
}

// Public type with private fields (use methods to access)
pub type User = {
    id: int,           // Private
    name: str,         // Private
}

impl User {
    pub @name (self) -> str = self.name    // Public accessor
}
```

## Complete Example

```ori
// Priority levels
type Priority = Low | Medium | High | Urgent

// Task status with data
type TaskStatus =
    | Todo
    | InProgress(started: Duration)
    | Done(completed: Duration)
    | Blocked(reason: str)

// A task
#derive(Eq, Clone, Debug)
type Task = {
    id: int,
    title: str,
    priority: Priority,
    status: TaskStatus,
    tags: [str],
}

impl Task {
    @new (id: int, title: str, priority: Priority) -> Task =
        Task {
            id,
            title,
            priority,
            status: Todo,
            tags: [],
        }

    @with_tags (self, tags: [str]) -> Task =
        Task { ...self, tags }

    @start (self, at: Duration) -> Task =
        Task { ...self, status: InProgress(started: at) }

    @complete (self, at: Duration) -> Task =
        Task { ...self, status: Done(completed: at) }

    @block (self, reason: str) -> Task =
        Task { ...self, status: Blocked(reason: reason) }

    @is_actionable (self) -> bool = match self.status {
        Todo | InProgress(_) -> true
        Done(_) | Blocked(_) -> false
    }
}

// Convert priority to number for sorting
@priority_score (p: Priority) -> int = match p {
    Low -> 1
    Medium -> 2
    High -> 3
    Urgent -> 4
}

@test_priority_score tests @priority_score () -> void = {
    assert_eq(actual: priority_score(p: Low), expected: 1)
    assert_eq(actual: priority_score(p: Urgent), expected: 4)
}

// Describe task status
@status_description (s: TaskStatus) -> str = match s {
    Todo -> "not started"
    InProgress(started) -> `in progress since {started}`
    Done(completed) -> `completed at {completed}`
    Blocked(reason) -> `blocked: {reason}`
}

@test_status tests @status_description () -> void = {
    assert_eq(actual: status_description(s: Todo), expected: "not started")
    assert_eq(actual: status_description(s: Blocked(reason: "waiting")), expected: "blocked: waiting")
}

// Test task workflow
@test_task_workflow tests @Task.new tests @Task.start tests @Task.is_actionable () -> void = {
    let task = Task.new(id: 1, title: "Fix bug", priority: High)
    assert(condition: task.is_actionable())

    let started = task.start(at: 0s)
    assert(condition: started.is_actionable())

    let done = started.complete(at: 30m)
    assert(condition: !done.is_actionable())
}
```

## Quick Reference

### Structs

```ori
type Name = { field: Type }
let x = Name { field: value }
let y = Name { field }           // Shorthand
let z = Name { ...x, field: new_value }   // Spread
x.field
let { field } = x                // Destructure
```

### Sum Types

```ori
type Name = Variant1 | Variant2(data: Type)
let x = Variant1
let y = Variant2(data: value)
```

### Generics

```ori
type Container<T> = { value: T }
type Either<L, R> = Left(L) | Right(R)
```

### Methods

```ori
impl Type {
    @static_method () -> Return = ...
    @instance_method (self) -> Return = ...
    @method_returning_self (self) -> Self = ...
}
```

### Deriving

```ori
#derive(Eq, Clone, Debug, Printable, Comparable, Hashable, Default)
type Name = ...
```

## What's Next

Now that you can create custom types:

- **[Pattern Matching](/guide/06-pattern-matching)** — Deep dive into matching patterns
- **[Option and Result](/guide/07-option-result)** — Handle missing values and errors
