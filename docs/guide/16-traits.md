---
title: "Traits"
description: "Defining shared behavior, implementing traits, and standard traits."
order: 16
part: "Abstraction"
---

# Traits

Traits define behavior that types can share. If you've used interfaces in other languages, traits are similar — but more powerful.

## Your First Trait

Let's create a trait for things that can be displayed:

```ori
trait Displayable {
    @display (self) -> str
}
```

This says: "Any type implementing `Displayable` must have a `display` method that returns a string."

## Implementing Traits

Now let's implement this trait for a type:

```ori
type Point = { x: int, y: int }

impl Displayable for Point {
    @display (self) -> str = `({self.x}, {self.y})`
}

// Now we can call display on any Point
let p = Point { x: 10, y: 20 }
let s = p.display()  // "(10, 20)"
```

## Why Use Traits?

Traits enable **polymorphism** — writing code that works with any type that implements a behavior:

```ori
@print_all<T: Displayable> (items: [T]) -> void =
    for item in items do
        print(msg: item.display())
```

This function works with Points, Users, or any type that implements `Displayable`:

```ori
type User = { name: str, age: int }

impl Displayable for User {
    @display (self) -> str = `{self.name} (age {self.age})`
}

// Both work!
print_all(items: [Point { x: 1, y: 2 }, Point { x: 3, y: 4 }])
print_all(items: [User { name: "Alice", age: 30 }])
```

## Trait Methods: self and Self

Two important concepts in traits:

- `self` — the instance the method is called on
- `Self` — the implementing type itself

```ori
trait Clonable {
    @clone (self) -> Self  // Returns the same type as the implementer
}

impl Clonable for Point {
    @clone (self) -> Self = Point { x: self.x, y: self.y }
}

let p = Point { x: 1, y: 2 }
let p2: Point = p.clone()  // Type is Point, not some generic type
```

## Default Implementations

Traits can provide default method implementations:

```ori
trait Describable {
    @name (self) -> str  // Required — implementers must provide

    @describe (self) -> str = `This is a {self.name()}`  // Default — can be overridden
}

type Car = { model: str }

impl Describable for Car {
    @name (self) -> str = self.model
    // describe uses the default implementation
}

let car = Car { model: "Tesla" }
car.describe()  // "This is a Tesla"
```

Implementers can override defaults if needed:

```ori
impl Describable for Point {
    @name (self) -> str = "Point"

    @describe (self) -> str = `Point at ({self.x}, {self.y})`  // Override default
}
```

## Associated Types

Traits can define types that implementers specify:

```ori
trait Container {
    type Item  // Associated type — implementer decides

    @get (self, index: int) -> Option<Self.Item>
    @len (self) -> int
}

impl Container for [int] {
    type Item = int  // For [int], Item is int

    @get (self, index: int) -> Option<int> =
        if index >= 0 && index < self.len() then Some(self[index]) else None

    @len (self) -> int = len(collection: self)
}
```

Associated types let you write generic code that works with any container:

```ori
@first<C: Container> (container: C) -> Option<C.Item> =
    container.get(index: 0)
```

## Trait Inheritance

Traits can require other traits:

```ori
trait Comparable: Eq {  // Comparable requires Eq
    @compare (self, other: Self) -> Ordering
}
```

To implement `Comparable`, you must also implement `Eq`:

```ori
impl Eq for Point {
    @eq (self, other: Self) -> bool = self.x == other.x && self.y == other.y
}

impl Comparable for Point {
    @compare (self, other: Self) -> Ordering = run(
        let by_x = compare(left: self.x, right: other.x),
        if by_x != Equal then by_x else compare(left: self.y, right: other.y),
    )
}
```

## Multiple Trait Bounds

Require multiple traits with `+`:

```ori
@sort_and_display<T: Comparable + Displayable> (items: [T]) -> void = run(
    let sorted = items.sort(),
    for item in sorted do print(msg: item.display()),
)
```

## Where Clauses

For complex bounds, use `where`:

```ori
@process<T, U> (input: T, transformer: (T) -> U) -> [U]
    where T: Clone,
          U: Displayable + Default = run(
    let items = [input.clone(), input.clone(), input.clone()],
    for item in items yield transformer(item),
)
```

## Generic Trait Implementations

Implement traits for generic types:

```ori
// Implement Displayable for any list of Displayable items
impl<T: Displayable> Displayable for [T] {
    @display (self) -> str = run(
        let items = for item in self yield item.display(),
        `[{items.join(sep: ", ")}]`,
    )
}

// Now [Point] is Displayable
let points = [Point { x: 1, y: 2 }, Point { x: 3, y: 4 }]
points.display()  // "[(1, 2), (3, 4)]"
```

## Deriving Traits

Many common traits can be automatically derived:

```ori
#derive(Eq, Clone, Debug, Printable)
type Point = { x: int, y: int }
```

| Trait | What It Provides |
|-------|------------------|
| `Eq` | `==`, `!=` operators |
| `Hashable` | Hash value for use as map keys |
| `Comparable` | `<`, `>`, `<=`, `>=` operators |
| `Clone` | `.clone()` method |
| `Debug` | `.debug()` for developer output |
| `Printable` | `.to_str()` for user output |
| `Default` | `Type.default()` constructor |

Deriving generates sensible implementations based on your type's fields.

## Standard Traits

Ori provides these commonly-used traits:

### Eq — Equality

```ori
trait Eq {
    @eq (self, other: Self) -> bool
}

// Enables == and != operators
let a = Point { x: 1, y: 2 }
let b = Point { x: 1, y: 2 }
a == b  // true
a != b  // false
```

### Comparable — Ordering

```ori
trait Comparable: Eq {
    @compare (self, other: Self) -> Ordering
}

// Enables <, >, <=, >= operators and sorting
let points = [Point { x: 3, y: 0 }, Point { x: 1, y: 0 }]
points.sort()  // [Point { x: 1, y: 0 }, Point { x: 3, y: 0 }]
```

### Clone — Copying

```ori
trait Clone {
    @clone (self) -> Self
}

let original = [1, 2, 3]
let copy = original.clone()  // Independent copy
```

### Debug and Printable — String Representations

```ori
trait Debug {
    @debug (self) -> str  // Developer-facing, shows structure
}

trait Printable {
    @to_str (self) -> str  // User-facing, shows content
}

#derive(Debug, Printable)
type User = { name: str, email: str }

let user = User { name: "Alice", email: "alice@example.com" }
user.debug()   // "User { name: \"Alice\", email: \"alice@example.com\" }"
user.to_str()  // "Alice (alice@example.com)" (if you customize to_str)
```

### Default — Default Values

```ori
trait Default {
    @default () -> Self
}

#derive(Default)
type Config = { timeout: int, retries: int }

let config = Config.default()  // Config { timeout: 0, retries: 0 }
```

### Hashable — Hash Values

```ori
trait Hashable: Eq {
    @hash (self) -> int
}

// Required for use as map keys
#derive(Eq, Hashable)
type UserId = { value: int }

let user_scores: {UserId: int} = {}
```

## Trait Objects

For heterogeneous collections, use trait objects:

```ori
trait Animal {
    @speak (self) -> str
}

type Dog = { name: str }
type Cat = { name: str }

impl Animal for Dog {
    @speak (self) -> str = "Woof!"
}

impl Animal for Cat {
    @speak (self) -> str = "Meow!"
}

// Trait object: any Animal
let animals: [Animal] = [
    Dog { name: "Rex" } as Animal,
    Cat { name: "Whiskers" } as Animal,
]

for animal in animals do
    print(msg: animal.speak())
```

### Object Safety

Not all traits can be used as trait objects. A trait is object-safe if:
- It doesn't use `Self` in return types (except for `self`)
- All methods have `self` as a parameter
- No generic methods

```ori
// Object-safe
trait Drawable {
    @draw (self) -> void
}

// NOT object-safe (returns Self)
trait Clonable {
    @clone (self) -> Self
}
```

## Type Conversions

Ori uses the `as` and `as?` operators for type conversions.

### Infallible Conversions with `as`

```ori
let x: int = 42
let y: float = x as float  // 42.0

let n: int = 100
let s: str = n as str  // "100"
```

### Fallible Conversions with `as?`

```ori
let s: str = "42"
let maybe_n: Option<int> = s as? int  // Some(42)

let bad: str = "not a number"
let none: Option<int> = bad as? int  // None
```

### The As and TryAs Traits

Conversions are backed by traits:

```ori
trait As<T> {
    @as (self) -> T
}

trait TryAs<T> {
    @try_as (self) -> Option<T>
}
```

Implementing these traits enables your types to use `as`/`as?`:

```ori
type Celsius = { value: float }
type Fahrenheit = { value: float }

impl As<Fahrenheit> for Celsius {
    @as (self) -> Fahrenheit =
        Fahrenheit { value: self.value * 9.0 / 5.0 + 32.0 }
}

let c = Celsius { value: 100.0 }
let f = c as Fahrenheit  // Fahrenheit { value: 212.0 }
```

### The Into Trait

For converting into a target type:

```ori
trait Into<T> {
    @into (self) -> T
}
```

Commonly used with error contexts:

```ori
// str implements Into<Error>
let result = fallible_op().context(msg: "operation failed")
// "operation failed" converts to Error via Into<Error>
```

## Complete Example

```ori
// Define a trait for scoring
trait Scorable {
    @score (self) -> int
}

// Define a trait for ranking
trait Rankable: Scorable + Comparable {}

// Player type
#derive(Clone, Debug)
type Player = {
    name: str,
    kills: int,
    deaths: int,
    assists: int,
}

impl Scorable for Player {
    @score (self) -> int = self.kills * 3 + self.assists - self.deaths
}

impl Eq for Player {
    @eq (self, other: Self) -> bool = self.name == other.name
}

impl Comparable for Player {
    @compare (self, other: Self) -> Ordering =
        compare(left: self.score(), right: other.score())
}

impl Rankable for Player {}

impl Printable for Player {
    @to_str (self) -> str = `{self.name}: {self.score()} points`
}

@test_player_score tests @Scorable.score () -> void = run(
    let player = Player { name: "Alice", kills: 10, deaths: 3, assists: 5 },
    assert_eq(actual: player.score(), expected: 32),
)

// Generic leaderboard for any Rankable
@leaderboard<T: Rankable + Printable + Clone> (
    items: [T],
    top_n: int,
) -> [str] = run(
    let sorted = items.clone(),
    sorted.sort_by(key: item -> -item.score()),  // Descending

    sorted.iter()
        .take(count: top_n)
        .enumerate()
        .map(transform: (rank, item) -> `#{rank + 1} {item.to_str()}`)
        .collect(),
)

@test_leaderboard tests @leaderboard () -> void = run(
    let players = [
        Player { name: "Alice", kills: 10, deaths: 3, assists: 5 },
        Player { name: "Bob", kills: 5, deaths: 5, assists: 10 },
        Player { name: "Charlie", kills: 15, deaths: 10, assists: 2 },
    ],

    let top_2 = leaderboard(items: players, top_n: 2),
    assert_eq(actual: len(collection: top_2), expected: 2),
)

// Team type also implementing Scorable
type Team = { name: str, players: [Player] }

impl Scorable for Team {
    @score (self) -> int =
        self.players.iter()
            .map(transform: p -> p.score())
            .fold(initial: 0, op: (a, b) -> a + b)
}

@test_team_score tests @Scorable.score () -> void = run(
    let team = Team {
        name: "Red Team",
        players: [
            Player { name: "Alice", kills: 10, deaths: 3, assists: 5 },
            Player { name: "Bob", kills: 5, deaths: 5, assists: 10 },
        ],
    },
    // Alice: 32, Bob: 20
    assert_eq(actual: team.score(), expected: 52),
)
```

## Quick Reference

### Define a Trait

```ori
trait Name {
    @method (self) -> Type
    @with_default (self) -> Type = default_impl
    type AssocType
}
```

### Trait Inheritance

```ori
trait Child: Parent { ... }
```

### Implement a Trait

```ori
impl Trait for Type { ... }
impl<T: Bound> Trait for Container<T> { ... }
```

### Derive Traits

```ori
#derive(Eq, Clone, Debug)
type Name = ...
```

### Multiple Bounds

```ori
@fn<T: A + B> (x: T) -> T = ...
```

### Where Clause

```ori
@fn<T> (x: T) -> T where T: Clone, T: Debug = ...
```

### Type Conversions

```ori
value as Type      // Infallible
value as? Type     // Fallible (returns Option<T>)
```

### Standard Traits

| Trait | Purpose |
|-------|---------|
| `Eq` | Equality (`==`, `!=`) |
| `Comparable` | Ordering (`<`, `>`, etc.) |
| `Hashable` | Hash value for maps |
| `Clone` | Deep copy |
| `Debug` | Developer string |
| `Printable` | User string |
| `Default` | Default value |

## What's Next

Now that you understand traits:

- **[Iterators](/guide/17-iterators)** — Functional data processing
- **[Extensions](/guide/18-extensions)** — Adding methods to existing traits

