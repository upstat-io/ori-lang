# Properties of Types

Type identity, assignability, and constraints.

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

```sigil
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

```sigil
let x: float = 42        // error
let x: float = float(42) // OK
```

## Variance

Generics are invariant. `Container<T>` is only compatible with `Container<T>`.

## Type Constraints

```
constraint   = identifier ":" bounds .
bounds       = type_path { "+" type_path } .
where_clause = "where" constraint { "," constraint } .
```

```sigil
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
