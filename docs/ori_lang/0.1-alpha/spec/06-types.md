---
title: "Types"
description: "Ori Language Specification — Types"
order: 6
---

# Types

Every value has a type determined at compile time.

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § TYPES

## Primitive Types

| Type | Description | Default |
|------|-------------|---------|
| `int` | 64-bit signed integer | `0` |
| `float` | 64-bit IEEE 754 | `0.0` |
| `bool` | `true` or `false` | `false` |
| `str` | UTF-8 string | `""` |
| `byte` | 8-bit unsigned | `0` |
| `char` | Unicode scalar value (U+0000–U+10FFFF, excluding surrogates) | — |
| `void` | Unit type, alias for `()` | `()` |
| `Never` | Bottom type, uninhabited | — |
| `Duration` | Time span (nanoseconds) | — |
| `Size` | Byte count | — |

`Never` is the return type for functions that never return (panic, infinite loop). Coerces to any type.

## Compound Types

### List

```
[T]
```

Ordered, homogeneous collection.

### Map

```
{K: V}
```

Key-value pairs. Keys must implement `Eq` and `Hashable`.

### Set

```
Set<T>
```

Unordered unique elements. Elements must implement `Eq` and `Hashable`.

### Tuple

```
(T1, T2, ...)
()
```

Fixed-size heterogeneous collection. `()` is the unit value.

### Function

```
(T1, T2) -> R
```

### Range

```
Range<T>
```

Produced by `..` (exclusive) and `..=` (inclusive). Bounds must be `Comparable`.

```ori
0..10       // 0 to 9
0..=10      // 0 to 10
```

## Generic Types

Type parameters in angle brackets:

```ori
Option<int>
Result<User, Error>
type Pair<T> = { first: T, second: T }
```

## Built-in Types

```
type Option<T> = Some(T) | None
type Result<T, E> = Ok(T) | Err(E)
type Ordering = Less | Equal | Greater
type Error = { message: str, source: Option<Error> }
type Channel<T>   // bounded async channel
```

## User-Defined Types

> **Grammar:** See [grammar.ebnf](grammar.ebnf) § DECLARATIONS (type_def)

### Struct

```ori
type Point = { x: int, y: int }
```

### Sum Type

```ori
type Status = Pending | Running | Done | Failed(reason: str)
```

### Newtype

```ori
type UserId = int
```

Creates distinct nominal type.

### Derive

```ori
#[derive(Eq, Hashable, Clone)]
type Point = { x: int, y: int }
```

Derivable: `Eq`, `Hashable`, `Comparable`, `Printable`, `Clone`, `Default`, `Serialize`, `Deserialize`.

## Nominal Typing

User-defined types are nominally typed. Identical structure does not imply same type.

## Trait Objects

A trait name used as a type represents "any value implementing this trait":

```ori
@display (item: Printable) -> void = print(item.to_str())

let items: [Printable] = [point, user, "hello"]
```

The compiler determines the dispatch mechanism. Users specify *what* (any Printable), not *how* (vtable vs monomorphization).

### Trait Object vs Generic Bound

| Syntax | Meaning |
|--------|---------|
| `item: Printable` | Any Printable value (trait object) |
| `<T: Printable> item: T` | Generic over Printable types |

Use trait objects for heterogeneous collections. Use generics when all elements share a concrete type.

### Object Safety

Not all traits can be used as types. Traits with methods that return `Self` or have generic parameters may not be object-safe. The compiler enforces these constraints with clear error messages.

## Clone Trait

The `Clone` trait enables explicit value duplication:

```ori
trait Clone {
    @clone (self) -> Self
}
```

`Clone` creates an independent copy of a value. The clone operation:
- For value types: returns a copy of the value
- For reference types: allocates new memory with refcount 1
- Element-wise recursive: cloning a container clones each element via `.clone()`

After cloning, original and clone have independent reference counts. Modifying the clone does not affect the original.

### Standard Implementations

All primitive types implement `Clone`:

| Type | Implementation |
|------|----------------|
| `int`, `float`, `bool`, `str`, `char`, `byte` | Returns copy of self |
| `Duration`, `Size` | Returns copy of self |

Collections implement `Clone` when their element types implement `Clone`:

| Type | Constraint |
|------|------------|
| `[T]` | `T: Clone` |
| `{K: V}` | `K: Clone, V: Clone` |
| `Set<T>` | `T: Clone` |
| `Option<T>` | `T: Clone` |
| `Result<T, E>` | `T: Clone, E: Clone` |
| `(A, B, ...)` | All element types: Clone |

### Derivable

`Clone` is derivable for user-defined types when all fields implement `Clone`:

```ori
#[derive(Clone)]
type Point = { x: int, y: int }
```

Derived implementation clones each field.

### Non-Cloneable Types

Some types do not implement `Clone`:
- Unique resources (file handles, network connections)
- Types with identity where duplicates would be semantically wrong

## Iterator Traits

Four traits formalize iteration:

```ori
trait Iterator {
    type Item
    @next (self) -> (Option<Self.Item>, Self)
}

trait DoubleEndedIterator: Iterator {
    @next_back (self) -> (Option<Self.Item>, Self)
}

trait Iterable {
    type Item
    @iter (self) -> impl Iterator where Item == Self.Item
}

trait Collect<T> {
    @from_iter (iter: impl Iterator where Item == T) -> Self
}
```

`Iterator.next()` returns a tuple of the optional value and the updated iterator. This functional approach fits Ori's immutable parameter semantics.

**Fused Guarantee:** Once `next()` returns `(None, iter)`, all subsequent calls must return `(None, _)`.

`Range<float>` does not implement `Iterable` due to floating-point precision ambiguity.

### Standard Implementations

| Type | Implements |
|------|------------|
| `[T]` | `Iterable`, `DoubleEndedIterator`, `Collect` |
| `{K: V}` | `Iterable` (not double-ended) |
| `Set<T>` | `Iterable`, `Collect` (not double-ended) |
| `str` | `Iterable`, `DoubleEndedIterator` |
| `Range<int>` | `Iterable`, `DoubleEndedIterator` |
| `Option<T>` | `Iterable` |

## Type Inference

Types inferred where possible. Required annotations:
- Function parameters
- Function return types
- Type definitions
