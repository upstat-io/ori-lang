---
title: "Properties of Types"
description: "Ori Language Specification — Properties of Types"
order: 7
section: "Types & Values"
---

# Properties of Types

Type identity, assignability, and constraints.

> **Grammar:** See [grammar.ebnf](https://ori-lang.com/docs/compiler-design/04-parser#grammar) § DECLARATIONS (generics, where_clause)

## Type Identity

Two types are identical if they have the same definition.

**Primitives**: Each primitive is identical only to itself.

**Compounds**: Same constructor and pairwise identical type arguments.

```
[int] ≡ [int]
[int] ≢ [str]
(int, str) ≢ (str, int)
```

**Nominal**: Same type definition, not structural equivalence.

```ori
type Point2D = { x: int, y: int }
type Vector2D = { x: int, y: int }
// Point2D ≢ Vector2D
```

**Generics**: Same definition and pairwise identical arguments.

```
Option<int> ≡ Option<int>
Option<int> ≢ Option<str>
```

## Assignability

A value of type `S` is assignable to type `T` if:
- `S` is identical to `T`, or
- `S` implements trait `T` and target is `dyn T`

No implicit conversions:

```ori
let x: float = 42        // error
let x: float = float(42) // OK
```

## Variance

Generics are invariant. `Container<T>` is only compatible with `Container<T>`.

## Type Constraints

```ori
@sort<T: Comparable> (items: [T]) -> [T] = ...

@process<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone, U: Default = ...
```

## Default Values

| Type | Default |
|------|---------|
| `int` | `0` |
| `float` | `0.0` |
| `bool` | `false` |
| `str` | `""` |
| `byte` | `0` |
| `void` | `()` |
| `Option<T>` | `None` |
| `[T]` | `[]` |
| `{K: V}` | `{}` |

Types implementing `Default` provide `default()` method.

## Printable Trait

The `Printable` trait provides human-readable string conversion.

```ori
trait Printable {
    @to_str (self) -> str
}
```

`Printable` is required for string interpolation without format specifiers:

```ori
let x = 42
`value: {x}`  // Calls x.to_str()
```

### Standard Implementations

| Type | Output |
|------|--------|
| `int` | `"42"` |
| `float` | `"3.14"` |
| `bool` | `"true"` or `"false"` |
| `str` | Identity |
| `char` | Single character string |
| `byte` | Numeric string |
| `[T]` where `T: Printable` | `"[1, 2, 3]"` |
| `Option<T>` where `T: Printable` | `"Some(42)"` or `"None"` |
| `Result<T, E>` where both Printable | `"Ok(42)"` or `"Err(msg)"` |

### Derivation

`Printable` is derivable for user-defined types when all fields implement `Printable`:

```ori
#derive(Printable)
type Point = { x: int, y: int }

Point { x: 1, y: 2 }.to_str()  // "Point(1, 2)"
```

Derived implementation creates human-readable format with type name and field values in order.

## Default Trait

The `Default` trait provides zero/empty values.

```ori
trait Default {
    @default () -> Self
}
```

### Standard Implementations

| Type | Default Value |
|------|---------------|
| `int` | `0` |
| `float` | `0.0` |
| `bool` | `false` |
| `str` | `""` |
| `byte` | `0` |
| `char` | `'\0'` |
| `void` | `()` |
| `[T]` | `[]` |
| `{K: V}` | `{}` |
| `Set<T>` | `Set.new()` |
| `Option<T>` | `None` |
| `Duration` | `0ns` |
| `Size` | `0b` |

### Derivation

`Default` is derivable for struct types when all fields implement `Default`:

```ori
#derive(Default)
type Config = {
    host: str,    // ""
    port: int,    // 0
    debug: bool,  // false
}
```

Sum types cannot derive `Default` (ambiguous variant):

```ori
#derive(Default)  // error: cannot derive Default for sum type
type Status = Pending | Running | Done
```

## Traceable Trait

The `Traceable` trait enables error trace propagation.

```ori
trait Traceable {
    @with_trace (self, entry: TraceEntry) -> Self
    @trace (self) -> str
    @trace_entries (self) -> [TraceEntry]
    @has_trace (self) -> bool
}
```

The `?` operator automatically adds trace entries at propagation points:

```ori
@outer () -> Result<int, Error> = run(
    let x = inner()?,  // Adds trace entry for this location
    Ok(x * 2),
)
```

### TraceEntry Type

```ori
type TraceEntry = {
    function: str,   // Function name with @ prefix
    file: str,       // Source file path
    line: int,       // Line number
    column: int,     // Column number
}
```

### Standard Implementations

| Type | Implements |
|------|------------|
| `Error` | Yes |
| `Result<T, E>` where `E: Traceable` | Yes (delegates to E) |
