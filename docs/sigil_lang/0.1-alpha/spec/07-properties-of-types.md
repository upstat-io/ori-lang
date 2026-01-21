# Properties of Types

This section defines type identity, assignability, and type relationships.

## Type Identity

Two types are identical if and only if they have the same definition.

### Primitive Type Identity

Each primitive type is identical only to itself:

- `int` is identical to `int`
- `int` is not identical to `float`
- `str` is not identical to `[byte]`

### Compound Type Identity

Two compound types are identical if:

1. They have the same type constructor
2. Their type arguments (if any) are pairwise identical

```
[int] is identical to [int]
[int] is not identical to [str]
{str: int} is identical to {str: int}
(int, str) is identical to (int, str)
(int, str) is not identical to (str, int)
```

### Nominal Type Identity

User-defined types use nominal identity. Two nominal types are identical only if they refer to the same type definition.

```sigil
type Point2D = { x: int, y: int }
type Vector2D = { x: int, y: int }

// Point2D is NOT identical to Vector2D
// despite having the same structure
```

### Generic Type Identity

Two generic type instances are identical if:

1. They instantiate the same generic type definition
2. Their type arguments are pairwise identical

```
Option<int> is identical to Option<int>
Option<int> is not identical to Option<str>
Result<int, Error> is identical to Result<int, Error>
```

## Assignability

A value of type `S` is assignable to a variable of type `T` if and only if:

1. `S` is identical to `T`, or
2. `S` implements a trait `T` and `T` is used as a trait object (`dyn T`)

### Direct Assignability

Values are directly assignable when types are identical:

```sigil
let x: int = 42        // int is assignable to int
let s: str = "hello"   // str is assignable to str
let p: Point = Point { x: 0, y: 0 }  // Point is assignable to Point
```

### No Implicit Conversions

There are no implicit type conversions in Sigil:

```sigil
let x: float = 42      // ERROR: int is not assignable to float
let y: int = 3.14      // ERROR: float is not assignable to int
```

Explicit conversion is required:

```sigil
let x: float = float(42)   // OK
let y: int = int(3.14)     // OK (truncates to 3)
```

### Trait Object Assignability

A value of type `T` is assignable to `dyn Trait` if `T` implements `Trait`:

```sigil
trait Printable {
    @to_string (self) -> str
}

impl Printable for int { ... }

let x: dyn Printable = 42  // OK: int implements Printable
```

## Type Compatibility

### Function Compatibility

A function type `(A1, ..., An) -> R1` is compatible with `(B1, ..., Bn) -> R2` if:

1. Parameter types are identical: `Ai` identical to `Bi` for all `i`
2. Return types are identical: `R1` identical to `R2`

### Variance

Sigil uses invariant generics. A `Container<T>` is only compatible with `Container<T>`, not `Container<U>` even if `T` and `U` are related.

## Type Relationships

### Subtyping

Sigil does not have structural subtyping. The only subtype relationship is through trait implementation:

- Every type `T` that implements trait `Trait` can be used where `dyn Trait` is expected

### Type Equivalence

Two types are equivalent if and only if they are identical.

## Type Constraints

### Trait Bounds

A type parameter may be constrained by trait bounds:

```
T : Trait
T : Trait1 + Trait2
```

A type argument satisfies a trait bound if it implements all specified traits.

```sigil
@sort<T> (items: [T]) -> [T] where T: Comparable = ...
```

The type argument for `T` must implement `Comparable`.

### Where Clauses

Complex constraints are expressed in `where` clauses:

```
where_clause  = "where" constraint { "," constraint } .
constraint    = identifier ":" bounds .
```

```sigil
@process<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone, U: Default = ...
```

## Default Values

Some types have default values:

| Type | Default Value |
|------|---------------|
| `int` | `0` |
| `float` | `0.0` |
| `bool` | `false` |
| `str` | `""` |
| `byte` | `0` |
| `void` / `()` | `()` |
| `Option<T>` | `None` |
| `[T]` | `[]` |
| `{K: V}` | `{}` |

Types that implement the `Default` trait provide a `default()` method.

## Zero Values

Primitive types have zero values as defined above. Composite types do not have implicit zero values unless they implement `Default`.
