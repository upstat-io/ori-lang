# Trait Definitions

This document covers how to define traits in Ori.

---

## What Are Traits?

Traits define shared behavior that types can implement. They're Ori's mechanism for polymorphism without inheritance.

```ori
trait Printable {
    @to_string (self) -> str
}
```

This says: "Any type that is Printable must provide a `to_string` method."

---

## Basic Syntax

```ori
trait TraitName {
    @method_name (self, params...) -> ReturnType
    @another_method (self) -> AnotherType
}
```

### Components

- `trait` — keyword
- `TraitName` — the trait's name (PascalCase)
- `self` — the implementing type's instance
- Method signatures — what implementors must provide

---

## The `self` Parameter

`self` is the instance of the implementing type:

```ori
trait Printable {
    @to_string (self) -> str
}

impl Printable for User {
    // self refers to the User instance
    @to_string (self) -> str = self.name + " <" + self.email + ">"
}
```

### `Self` Type

`Self` (capitalized) refers to the implementing type:

```ori
trait Clone {
    // returns same type as self
    @clone (self) -> Self
}

impl Clone for Point {
    // Self is Point when implementing for Point
    @clone (self) -> Self = Point { x: self.x, y: self.y }
}
```

---

## Method Signatures

### With Parameters

```ori
trait Comparable {
    @compare (self, other: Self) -> Ordering
}

trait Displayable {
    @display (self) -> str
}
```

### With Generics

```ori
trait Container<T> {
    @get (self, index: int) -> Option<T>
    @set (self, index: int, value: T) -> Self
}
```

### No Self Parameter (Static Methods)

```ori
trait Default {
    // no self, creates new instance
    @default () -> Self
}

impl Default for Point {
    @default () -> Self = Point { x: 0, y: 0 }
}
```

---

## Default Implementations

Traits can provide default method implementations:

```ori
trait Eq {
    @equals (self, other: Self) -> bool

    // Default implementation using equals
    @not_equals (self, other: Self) -> bool = !self.equals(other)
}
```

Implementors get `not_equals` for free:

```ori
impl Eq for Point {
    @equals (self, other: Self) -> bool =
        self.x == other.x && self.y == other.y
    // not_equals is automatically available
}
```

### Override Defaults

```ori
impl Eq for SpecialType {
    @equals (self, other: Self) -> bool = ...

    // Can override the default if needed
    @not_equals (self, other: Self) -> bool =
        custom_not_equals_logic(self, other)
}
```

### Derived Defaults

```ori
trait Comparable: Eq {
    @compare (self, other: Self) -> Ordering

    // All derived from compare
    @less_than (self, other: Self) -> bool =
        self.compare(other) == Less

    @greater_than (self, other: Self) -> bool =
        self.compare(other) == Greater

    @less_or_equal (self, other: Self) -> bool =
        self.compare(other) != Greater

    @greater_or_equal (self, other: Self) -> bool =
        self.compare(other) != Less
}
```

Implement `compare`, get five methods.

---

## Trait Inheritance

Traits can require other traits:

```ori
trait Eq {
    @equals (self, other: Self) -> bool
}

// Comparable requires Eq
trait Comparable: Eq {
    @compare (self, other: Self) -> Ordering
}
```

To implement `Comparable`, you must also implement `Eq`:

```ori
impl Eq for User {
    @equals (self, other: Self) -> bool = self.id == other.id
}

impl Comparable for User {
    @compare (self, other: Self) -> Ordering = compare(self.id, other.id)
}
```

### Multiple Requirements

```ori
trait Sortable: Eq + Comparable + Clone {
    // Requires all three traits
}
```

---

## Associated Types

Traits can have associated types:

```ori
trait Iterator {
    // associated type
    type Item

    @next (self) -> Option<Self.Item>
    @has_next (self) -> bool
}
```

### Implementing with Associated Types

```ori
impl Iterator for Range {
    type Item = int

    @next (self) -> Option<int> = ...
    @has_next (self) -> bool = ...
}

impl Iterator for StringChars {
    type Item = str

    @next (self) -> Option<str> = ...
    @has_next (self) -> bool = ...
}
```

### Using Associated Types in Bounds

```ori
@collect<I> (iter: I) -> [I.Item] where I: Iterator = ...
```

---

## Documentation

Document traits like other definitions:

```ori
// #Types that can be converted to a string representation
trait Printable {
    // #Returns a human-readable string representation
    @to_string (self) -> str
}

// #Types that can be compared for ordering
// !Implementors must ensure consistent ordering
trait Comparable: Eq {
    // #Compare self with other
    // >compare(1, 2) -> Less
    @compare (self, other: Self) -> Ordering
}
```

---

## Best Practices

### Keep Traits Focused

```ori
// Good: single responsibility
trait Hashable { @hash (self) -> int }
trait Printable { @to_string (self) -> str }
trait Clonable { @clone (self) -> Self }

// Bad: too many unrelated methods
trait Everything {
    @hash (self) -> int
    @to_string (self) -> str
    @clone (self) -> Self
    @serialize (self) -> str
}
```

### Use Semantic Names

```ori
// Good: describes the capability
trait Serialize { ... }
trait Comparable { ... }
trait Iterator { ... }

// Bad: too generic
trait Doable { ... }
trait Thing { ... }
```

### Provide Useful Defaults

```ori
// Good: implementors only need equals()
trait Eq {
    @equals (self, other: Self) -> bool
    @not_equals (self, other: Self) -> bool = !self.equals(other)
}
```

---

## See Also

- [Implementations](02-implementations.md)
- [Bounds and Constraints](03-bounds-and-constraints.md)
- [Dynamic Dispatch](05-dynamic-dispatch.md)
