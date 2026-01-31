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

## Comparable Trait

The `Comparable` trait provides total ordering for values.

```ori
trait Comparable: Eq {
    @compare (self, other: Self) -> Ordering
}
```

`Comparable` extends `Eq` — all comparable types must also be equatable.

### Mathematical Properties

A valid `Comparable` implementation must satisfy:

**Reflexivity**: `a.compare(other: a) == Equal`

**Antisymmetry**: If `a.compare(other: b) == Less`, then `b.compare(other: a) == Greater`

**Transitivity**: If `a.compare(other: b) == Less` and `b.compare(other: c) == Less`, then `a.compare(other: c) == Less`

**Consistency with Eq**: `a.compare(other: b) == Equal` if and only if `a == b`

### Operator Derivation

Types implementing `Comparable` automatically get comparison operators. The builtin `compare` function calls the trait method:

```ori
compare(left: a, right: b)  // → a.compare(other: b)
```

Operators desugar to `Ordering` method calls:

```ori
a < b   // a.compare(other: b).is_less()
a <= b  // a.compare(other: b).is_less_or_equal()
a > b   // a.compare(other: b).is_greater()
a >= b  // a.compare(other: b).is_greater_or_equal()
```

### Standard Implementations

| Type | Ordering |
|------|----------|
| `int` | Numeric order |
| `float` | IEEE 754 total order (NaN handling) |
| `bool` | `false < true` |
| `str` | Lexicographic (Unicode codepoint) |
| `char` | Unicode codepoint |
| `byte` | Numeric order |
| `Duration` | Shorter < longer |
| `Size` | Smaller < larger |
| `[T]` where `T: Comparable` | Lexicographic |
| `(T1, T2, ...)` where all `Ti: Comparable` | Lexicographic |
| `Option<T>` where `T: Comparable` | `None < Some(_)` |
| `Result<T, E>` where `T: Comparable, E: Comparable` | `Ok(_) < Err(_)`, then compare inner |
| `Ordering` | `Less < Equal < Greater` |

Maps and Sets are not Comparable (unordered collections).

### Float Comparison

Floats follow IEEE 754 total ordering:
- `-Inf < negative < -0.0 < +0.0 < positive < +Inf`
- `NaN` compares equal to itself and greater than all other values

Note: For ordering purposes, `NaN == NaN`. This differs from `==` where `NaN != NaN`.

### Derivation

`Comparable` is derivable for user-defined types when all fields implement `Comparable`:

```ori
#derive(Eq, Comparable)
type Point = { x: int, y: int }

// Generated: lexicographic comparison by field declaration order
```

For sum types, variants compare by declaration order (`Low < Medium < High`).

## Hashable Trait

The `Hashable` trait provides hash values for map keys and set elements.

```ori
trait Hashable: Eq {
    @hash (self) -> int
}
```

`Hashable` extends `Eq` — all hashable types must also be equatable.

### Hash Invariant

**Consistency with Eq**: If `a == b`, then `a.hash() == b.hash()`

The converse is NOT required — different values may have the same hash (collisions are expected).

### Standard Implementations

| Type | Hash Method |
|------|-------------|
| `int` | Identity or bit-mixing |
| `float` | Bit representation hash |
| `bool` | `false` → 0, `true` → 1 |
| `str` | FNV-1a or similar |
| `char` | Codepoint value |
| `byte` | Identity |
| `Duration` | Hash of nanoseconds |
| `Size` | Hash of bytes |
| `[T]` where `T: Hashable` | Combined element hashes |
| `{K: V}` where `K: Hashable, V: Hashable` | Combined entry hashes (order-independent) |
| `Set<T>` where `T: Hashable` | Combined element hashes (order-independent) |
| `(T1, T2, ...)` where all `Ti: Hashable` | Combined element hashes |
| `Option<T>` where `T: Hashable` | `None` → 0, `Some(x)` → `x.hash()` with salt |
| `Result<T, E>` where `T: Hashable, E: Hashable` | Combined variant and value hash |

### Float Hashing

Floats hash consistently with equality:
- `+0.0` and `-0.0` hash the same (they're equal)
- `NaN` values hash consistently (all NaN equal for hashing)

### Map Key and Set Element Requirements

To use a type as a map key or set element, it must implement both `Eq` and `Hashable`:

```ori
let map: {Point: str} = {}  // Point must be Eq + Hashable
let set: Set<Point> = Set.new()  // Point must be Eq + Hashable
```

### hash_combine Function

The `hash_combine` function in the prelude mixes hash values:

```ori
@hash_combine (seed: int, value: int) -> int =
    seed ^ (value + 0x9e3779b9 + (seed << 6) + (seed >> 2))
```

This follows the boost hash_combine pattern for good distribution. Users implementing custom `Hashable` can use this function directly.

### Derivation

`Hashable` is derivable for user-defined types when all fields implement `Hashable`:

```ori
#derive(Eq, Hashable)
type Point = { x: int, y: int }

// Generated: combine field hashes using hash_combine
```

Deriving `Hashable` without `Eq` produces a warning.
