# Trait Implementations

This document covers implementing traits for types.

---

## Basic Implementation

Use `impl Trait for Type` to implement a trait:

```sigil
trait Printable {
    @to_string (self) -> str
}

type User = { name: str, email: str }

impl Printable for User {
    @to_string (self) -> str = self.name + " <" + self.email + ">"
}
```

---

## Implementation Syntax

```sigil
impl TraitName for TypeName {
    @method_name (self, params...) -> ReturnType = expression
    @another_method (self) -> AnotherType = expression
}
```

### All Required Methods

You must implement all required methods (those without defaults):

```sigil
trait Serialize {
    @to_json (self) -> str
}

trait Deserialize {
    @from_json (json: str) -> Result<Self, Error>
}

impl Serialize for User {
    @to_json (self) -> str = ...
}

impl Deserialize for User {
    @from_json (json: str) -> Result<Self, Error> = ...
}
```

### Skip Default Methods

Methods with defaults don't need implementation:

```sigil
trait Eq {
    @equals (self, other: Self) -> bool
    @not_equals (self, other: Self) -> bool = !self.equals(other)  // default
}

impl Eq for Point {
    // Only equals is required
    @equals (self, other: Self) -> bool =
        self.x == other.x && self.y == other.y
    // not_equals comes from default
}
```

---

## Generic Implementations

### For Generic Types

```sigil
impl<T> Printable for Option<T> where T: Printable {
    @to_string (self) -> str = match(self,
        Some(x) -> "Some(" + x.to_string() + ")",
        None -> "None"
    )
}
```

### Blanket Implementations

Implement for all types meeting a constraint:

```sigil
// Any Printable type gets Debug for free
impl<T> Debug for T where T: Printable {
    @debug (self) -> str = "Debug: " + self.to_string()
}
```

---

## Implementing for Primitives

You can implement your traits for primitive types:

```sigil
trait Doubled {
    @doubled (self) -> Self
}

impl Doubled for int {
    @doubled (self) -> Self = self * 2
}

impl Doubled for str {
    @doubled (self) -> Self = self + self
}
```

Usage:

```sigil
x = 5.doubled()        // 10
s = "hi".doubled()     // "hihi"
```

---

## The Orphan Rule

At least one of (trait, type) must be defined in your module:

```sigil
// OK: your trait for external type
trait MyTrait { ... }
impl MyTrait for int { ... }

// OK: external trait for your type
type MyType = { ... }
impl Printable for MyType { ... }

// ERROR: external trait for external type
impl Printable for int { ... }  // Neither Printable nor int is yours
```

### Why?

Prevents conflicting implementations from different libraries. If two libraries could both implement `Printable` for `int`, which one wins?

---

## Multiple Traits

A type can implement multiple traits:

```sigil
type User = { id: int, name: str, email: str }

impl Eq for User {
    @equals (self, other: Self) -> bool = self.id == other.id
}

impl Hashable for User {
    @hash (self) -> int = hash(self.id)
}

impl Printable for User {
    @to_string (self) -> str = self.name
}

impl Comparable for User {
    @compare (self, other: Self) -> Ordering = compare(self.id, other.id)
}
```

---

## Implementation Organization

### One impl Block Per Trait

```sigil
// Good: organized by trait
impl Eq for User { ... }
impl Hashable for User { ... }
impl Printable for User { ... }
```

### Separate from Type Definition

Implementations are separate from type definitions:

```sigil
// types.si
type User = { id: int, name: str }

// traits.si (or same file, separate section)
impl Printable for User { ... }
impl Serialize for User { ... }
```

This allows implementing traits defined elsewhere.

---

## Implementing Associated Types

```sigil
trait Iterator {
    type Item
    @next (self) -> Option<Self.Item>
}

impl Iterator for Range {
    type Item = int  // specify the associated type
    @next (self) -> Option<int> = ...
}
```

---

## Method Resolution

When multiple traits have methods with the same name:

```sigil
trait A { @process (self) -> int }
trait B { @process (self) -> str }

type MyType = { ... }
impl A for MyType { @process (self) -> int = 42 }
impl B for MyType { @process (self) -> str = "hello" }

// Ambiguous call
x = my_value.process()  // ERROR: ambiguous

// Qualify the trait
x = A.process(my_value)  // OK: calls A's process
y = B.process(my_value)  // OK: calls B's process
```

---

## Coherence

There can only be one implementation of a trait for a type:

```sigil
impl Printable for User {
    @to_string (self) -> str = self.name
}

impl Printable for User {  // ERROR: duplicate implementation
    @to_string (self) -> str = self.email
}
```

---

## Visibility

Implementations follow the type's visibility:

```sigil
// If User is public, implementations are usable where User is visible
pub type User = { ... }
impl Printable for User { ... }  // accessible where User is

// Private types have private implementations
type Internal = { ... }
impl Printable for Internal { ... }  // only accessible in this module
```

---

## See Also

- [Trait Definitions](01-trait-definitions.md)
- [Bounds and Constraints](03-bounds-and-constraints.md)
- [Derive](04-derive.md)
