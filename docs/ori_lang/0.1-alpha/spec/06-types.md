---
title: "Types"
description: "Ori Language Specification — Types"
order: 6
---

# Types

Every value has a type determined at compile time.

## Type Syntax

```ebnf
type          = type_path [ type_args ]
              | list_type | map_type | tuple_type | function_type
              | "dyn" type .
type_path     = identifier { "." identifier } .
type_args     = "<" type { "," type } ">" .
list_type     = "[" type "]" .
map_type      = "{" type ":" type "}" .
tuple_type    = "(" type { "," type } ")" | "()" .
function_type = "(" [ type { "," type } ] ")" "->" type .
```

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

```ebnf
type_def      = [ "pub" ] [ derive ] "type" identifier [ generics ] [ where ] "=" type_body .
derive        = "#[derive(" identifier { "," identifier } ")]" .
generics      = "<" generic_param { "," generic_param } ">" .
generic_param = identifier [ ":" bounds ] .
bounds        = type_path { "+" type_path } .
type_body     = struct_type | sum_type | type .
struct_type   = "{" [ field { "," field } ] "}" .
sum_type      = variant { "|" variant } .
variant       = identifier [ "(" [ field { "," field } ] ")" ] .
field         = identifier ":" type .
```

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

## Type Inference

Types inferred where possible. Required annotations:
- Function parameters
- Function return types
- Type definitions
