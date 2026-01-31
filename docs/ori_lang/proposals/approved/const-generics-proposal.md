# Proposal: Basic Const Generics

**Status:** Approved
**Approved:** 2026-01-31
**Author:** Eric (with AI assistance)
**Created:** 2026-01-31
**Affects:** Compiler, type system, generics, type inference

---

## Summary

This proposal formalizes const generic parameters (`$N: int`), enabling compile-time values as type and function parameters. Const generics allow types like fixed-capacity lists to encode their capacity in the type system, enabling compile-time safety guarantees without runtime overhead.

```ori
@zeros<$N: int> () -> [int, max N]
    where N > 0
= for _ in 0..N yield 0
```

---

## Problem Statement

Without const generics, Ori lacks the ability to:

1. **Encode compile-time constants in types**: Fixed-capacity lists `[T, max N]` require `N` to be a const generic
2. **Create generic functions over sizes**: Functions like `zeros<N>()` cannot be written
3. **Provide compile-time guarantees**: Array bounds, buffer sizes, and similar constraints cannot be verified statically
4. **Avoid runtime overhead**: Without const generics, size information must be passed at runtime

### Current Workarounds

Without const generics, users would need:

```ori
// Separate functions for each size (not scalable)
@zeros_10 () -> [int, max 10] = ...
@zeros_20 () -> [int, max 20] = ...

// Runtime-sized collections (loses compile-time guarantees)
@zeros (n: int) -> [int] = for _ in 0..n yield 0
```

### Goals

1. Define the syntax for const generic parameters
2. Specify allowed const types (`int`, `bool`)
3. Define declaration forms for functions, types, traits, and impls
4. Specify instantiation and monomorphization semantics
5. Define interaction with type parameters and inference

---

## Terminology

| Term | Definition |
|------|------------|
| **Type parameter** | Generic parameter representing a type: `<T>` |
| **Const generic parameter** | Generic parameter representing a compile-time value: `<$N: int>` |
| **Type bound** | Constraint on a type parameter: `where T: Clone` |
| **Const bound** | Constraint on a const generic: `where N > 0` |
| **Monomorphization** | Creating concrete code for each unique combination of generic arguments |

---

## Design

### Const Generic Sigil

Const generic parameters use the `$` sigil, consistent with const bindings:

```ori
let $MAX_SIZE = 1024      // Const binding uses $
@buffer<$N: int> () ...   // Const generic uses $
```

This maintains a clear visual distinction from type parameters and aligns with Ori's "$ means compile-time" convention.

### Allowed Const Types

Only `int` and `bool` are valid as const generic types:

| Type | Syntax | Use Cases |
|------|--------|-----------|
| `int` | `$N: int` | Sizes, counts, indices, capacities |
| `bool` | `$B: bool` | Feature flags, conditional compilation |

Rationale for this restriction:
- `int` and `bool` cover the vast majority of use cases
- These types have clear equality semantics needed for type identity
- Extending to `str` or custom types would complicate type equality and monomorphization

### Declaration Syntax

#### Functions

```ori
// Const generic only
@zeros<$N: int> () -> [int, max N] = ...

// Mixed type and const generics
@replicate<T, $N: int> (value: T) -> [T, max N] = ...

// Multiple const generics
@matrix<$R: int, $C: int> () -> [[float, max C], max R] = ...

// With bounds
@non_empty<T, $N: int> (items: [T, max N]) -> T
    where N > 0
= items[0]
```

#### Types

```ori
// Struct with const generic
type FixedBuffer<T, $N: int> = {
    data: [T, max N],
    len: int,
}

// Sum type with const generic
type SmallOrLarge<T, $N: int> =
    | Small([T, max N])
    | Large([T])

// Newtype with const generic
type Capacity<$N: int> = int
```

#### Traits

```ori
// Trait with const generic parameter
trait FixedSize<$N: int> {
    @capacity (self) -> int = N
    @elements (self) -> [Self.Element, max N]
    type Element
}

```

#### Impls

```ori
// Impl with const generic
impl<T, $N: int> FixedSize<N> for [T, max N] {
    type Element = T
    @elements (self) -> [T, max N] = self
}

// Conditional impl based on const bound
impl<T, $N: int> Default for [T, max N]
    where T: Default
    where N >= 0 && N <= 16
{
    @default () -> [T, max N] = for _ in 0..N yield T.default()
}
```

#### Extensions

```ori
extend<T, $N: int> [T, max N] {
    @is_full (self) -> bool = len(self) == N
    @remaining (self) -> int = N - len(self)
}
```

### Parameter Ordering

When mixing type and const generic parameters, either ordering is allowed:

```ori
@example<T, $N: int> (...)      // Type first (preferred)
@example<$N: int, T> (...)      // Const first (allowed)
@example<T, $N: int, U> (...)   // Interleaved (allowed)
```

**Style recommendation**: Place type parameters before const generics for consistency, unless a specific ordering improves readability.

### Default Values

Const generics can have default values:

```ori
@buffer<$N: int = 64> () -> [byte, max N] = ...

buffer()          // Uses N = 64
buffer<128>()     // Overrides to N = 128

type Vector<T, $N: int = 3> = [T, max N]

Vector<float>         // 3D vector
Vector<float, 4>      // 4D vector
```

Default const values must be:
1. Compile-time constant expressions
2. Valid for any bounds on the parameter

```ori
@sized<$N: int = 10> ()
    where N > 0       // OK: 10 > 0
= ...

@bad<$N: int = 0> ()
    where N > 0       // ERROR: default value 0 violates bound
= ...
```

### Instantiation

#### Explicit Instantiation

Const generics are instantiated with concrete values:

```ori
zeros<10>()                      // [int, max 10]
replicate<str, 5>(value: "hi")   // [str, max 5]
matrix<3, 4>()                   // [[float, max 4], max 3]
```

#### Inferred Instantiation

When possible, const generics are inferred from context:

```ori
let buffer: [int, max 100] = zeros()  // N = 100 inferred from type annotation

@accepts_ten (items: [int, max 10]) = ...
accepts_ten(zeros())                   // N = 10 inferred from parameter type
```

#### Partial Inference

Type parameters can be inferred while const generics are explicit:

```ori
let items = replicate<_, 5>(value: "hello")  // T = str inferred, N = 5 explicit
```

### Monomorphization

Each unique combination of const generic values produces a distinct monomorphized function or type:

```ori
zeros<5>()   // Generates zeros_5
zeros<10>()  // Generates zeros_10

// These are distinct types, not compatible:
let a: [int, max 5] = ...
let b: [int, max 10] = a  // ERROR: type mismatch
```

#### Monomorphization Explosion

Large numbers of distinct const values can cause compile time and binary size increases. The compiler may warn for excessive instantiations:

```
warning: 1000+ instantiations of `buffer<$N>` may impact compile time
  --> src/main.ori:42:1
   |
   = help: consider using runtime-sized `[T]` if sizes vary widely
```

### Const Expressions in Types

Const generics enable arithmetic in type positions:

```ori
// Using const generic in expressions
@double_capacity<$N: int> (items: [T, max N]) -> [T, max N * 2] = ...

// Computed sizes
type Pair<T, $N: int> = [[T, max N], max 2]

// Division (truncating)
@halve<$N: int> (items: [T, max N]) -> [T, max N / 2]
    where N > 0
    where N % 2 == 0
= ...
```

Allowed arithmetic operations in type positions:
- Addition: `N + M`, `N + 1`
- Subtraction: `N - M`, `N - 1`
- Multiplication: `N * M`, `N * 2`
- Division: `N / M`, `N / 2` (truncating)
- Modulo: `N % M`
- Bitwise: `N & M`, `N | M`, `N ^ M`, `N << M`, `N >> M`

### Interaction with Type Parameters

Const and type parameters are independent:

```ori
@generic<T, $N: int> (items: [T, max N]) -> [T, max N] = items
```

A type parameter cannot be constrained based on a const generic except through trait bounds:

```ori
// This is allowed (trait bound):
@with_default<T: Default, $N: int> () -> [T, max N] = ...

// This is NOT allowed (const doesn't constrain type):
@invalid<T, $N: int> () where T depends on N = ...  // Not valid Ori
```

### Interaction with Trait Bounds

Const bounds and type bounds can be combined:

```ori
@fill<T: Clone + Default, $N: int> () -> [T, max N]
    where N > 0
    where N <= 1000
= for _ in 0..N yield T.default()
```

Multiple `where` clauses are combined with logical AND:

```ori
// These are equivalent:
@f<$N: int> () where N > 0 && N < 100 = ...
@f<$N: int> () where N > 0 where N < 100 = ...
```

### Interaction with Type Inference

The type inference algorithm extends to const generics:

1. **Constraint generation**: Collect equations like `N = 10` from context
2. **Unification**: Unify const expressions when they must be equal
3. **Default propagation**: Apply default values when no constraint exists
4. **Error on ambiguity**: Report error if const cannot be determined

```ori
let x = zeros()  // ERROR: cannot infer const generic `N`
                 // help: add type annotation: `let x: [int, max N] = zeros()`
```

### Visibility

Const generic parameters follow normal visibility rules:

```ori
pub @public_fn<$N: int> () = ...        // N is visible at call sites
@private_fn<$N: int> () = ...           // N is visible within module

pub type PublicType<$N: int> = ...      // N is part of public API
type PrivateType<$N: int> = ...         // N is internal
```

When a const generic appears in a public API, changing its default value or bounds is a breaking change.

---

## Design Rationale

### Why `$` Sigil?

Alternatives considered:

| Option | Example | Assessment |
|--------|---------|------------|
| `$N: int` | `@f<$N: int>` | **Chosen**: Consistent with const bindings |
| `const N: int` | `@f<const N: int>` | Verbose, keyword overload |
| `N: int` (inferred) | `@f<N: int>` | Ambiguous with type parameters |
| `#N: int` | `@f<#N: int>` | Conflicts with attribute syntax |

The `$` sigil clearly communicates "this is a compile-time value" and matches `$` in const function names and const bindings.

### Why Only `int` and `bool`?

| Type | Consideration | Decision |
|------|--------------|----------|
| `int` | Covers sizes, counts, indices | Included |
| `bool` | Covers feature flags, conditionals | Included |
| `str` | Type equality complex (interning?) | Excluded |
| `char` | Limited use cases | Excluded |
| Enums | Would require closed set of values | Future consideration |
| Structs | Complex equality semantics | Excluded |

Limiting to `int` and `bool` keeps the implementation tractable while covering the primary use cases.

### Why Allow Default Values?

Default const values enable ergonomic APIs:

```ori
// Without defaults: always specify capacity
let buf = Buffer<byte, 1024>.new()

// With defaults: sensible default, override when needed
let buf = Buffer<byte>.new()        // Uses default capacity
let big = Buffer<byte, 8192>.new()  // Override for special case
```

### Monomorphization vs. Type Erasure

Ori uses monomorphization (generating code per instantiation) rather than type erasure because:

1. **Performance**: No runtime dispatch or size parameters
2. **Type safety**: `[T, max 5]` and `[T, max 10]` are truly distinct types
3. **Optimization**: Compiler can optimize for specific sizes (loop unrolling, etc.)

The tradeoff is potential code size increase, which is acceptable for Ori's target use cases.

---

## Interaction with Other Features

### Fixed-Capacity Lists

Const generics are the foundation for fixed-capacity lists:

```ori
type FixedList<T, $N: int> = [T, max N]

@empty<T, $N: int> () -> [T, max N] = []
@full<T: Default, $N: int> () -> [T, max N] = for _ in 0..N yield T.default()
```

See the spec section on fixed-capacity lists for the full API.

### Const Bounds

Const generic parameters can be constrained with bounds:

```ori
@positive<$N: int> () where N > 0 = ...
```

For full details on bound syntax and semantics, see the **Const Generic Bounds** proposal.

### Const Evaluation

The values used in const generics and their bounds are evaluated at compile time. Limits and behaviors are specified in the **Const Evaluation Termination** proposal.

### Conditional Compilation

Const generics interact with conditional compilation:

```ori
@optimized<$N: int> (data: [int, max N]) -> int =
    if N <= 8 then
        // Use simple loop for small arrays
        for x in data.iter() yield x
    else
        // Use SIMD for larger arrays
        simd_sum(data:)
```

The `if` condition is evaluated at compile time when `N` is known.

---

## Error Messages

### Unknown Const Type

```
error[E1040]: invalid const generic type
  --> src/main.ori:1:15
   |
 1 | @f<$N: float> () = ...
   |        ^^^^^ `float` is not allowed as a const generic type
   |
   = note: only `int` and `bool` are allowed as const generic types
```

### Cannot Infer Const Generic

```
error[E1041]: cannot infer const generic parameter
  --> src/main.ori:5:9
   |
 5 |     let x = zeros()
   |             ^^^^^^ cannot infer `N`
   |
   = help: add a type annotation: `let x: [int, max N] = zeros()`
   = help: or specify explicitly: `zeros<10>()`
```

### Type Mismatch with Const Generics

```
error[E1042]: mismatched types
  --> src/main.ori:3:9
   |
 3 |     let a: [int, max 5] = zeros<10>()
   |            ^^^^^^^^^^^   ^^^^^^^^^^ expected `[int, max 5]`, found `[int, max 10]`
   |
   = note: `[int, max 5]` and `[int, max 10]` are distinct types
```

### Default Value Violates Bound

```
error[E1043]: default value violates const bound
  --> src/main.ori:1:18
   |
 1 | @f<$N: int = 0> () where N > 0 = ...
   |              ^         ^^^^^^^ this bound requires `N > 0`
   |              |
   |              default value `0` does not satisfy `0 > 0`
```

### Const Expression Overflow in Type

```
error[E1044]: const expression overflow in type
  --> src/main.ori:2:30
   |
 2 | @huge<$N: int> () -> [byte, max N * N * N] where N > 1000 = ...
   |                              ^^^^^^^^^^^ `N * N * N` overflows for large N
   |
   = note: const expressions in types use 64-bit signed integers
```

---

## Spec Changes Required

### Update `06-types.md`

Expand the "Const Generic Parameters" section with:
1. Sigil and syntax (`$N: int`)
2. Allowed types (`int`, `bool`)
3. Declaration forms (functions, types, traits, impls, extensions)
4. Default values
5. Instantiation and inference
6. Monomorphization semantics
7. Const expressions in type positions

### Update `08-declarations.md`

Add const generic parameter syntax to:
1. Function declarations
2. Type declarations
3. Trait declarations
4. Impl blocks
5. Extension declarations

### Update `grammar.ebnf`

Add grammar rules for:
```ebnf
generic_params      = "<" generic_param_list ">" .
generic_param_list  = generic_param { "," generic_param } .
generic_param       = type_param | const_param .
type_param          = identifier [ ":" bounds ] [ "=" type ] .
const_param         = "$" identifier ":" const_type [ "=" const_expr ] .
const_type          = "int" | "bool" .
```

### Update `21-constant-expressions.md`

Reference const generics as a context where const expressions are required.

---

## Summary

| Aspect | Details |
|--------|---------|
| Sigil | `$` prefix (e.g., `$N: int`) |
| Allowed types | `int`, `bool` |
| Declaration forms | Functions, types, traits, impls, extensions |
| Default values | Allowed with `= value` syntax |
| Instantiation | Explicit (`f<10>`) or inferred from context |
| Monomorphization | Each unique value generates distinct code |
| Type expressions | Arithmetic allowed (`N + 1`, `N * 2`, etc.) |
| Inference | Unified with type inference algorithm |
| Visibility | Follows normal visibility rules |

---

## Related Proposals

- **Const Generic Bounds** (`const-generic-bounds-proposal.md`): Constraint syntax for const generics
- **Const Evaluation Termination** (`const-evaluation-termination-proposal.md`): Compile-time evaluation limits
