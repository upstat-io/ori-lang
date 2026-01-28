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

## Type Inference

Types inferred where possible. Required annotations:
- Function parameters
- Function return types
- Type definitions
