---
title: "Properties of Types"
description: "Clause 9: Ori Language Specification — Properties of Types"
order: 9
section: "Language"
---

# 9 Properties of Types

Type identity, assignability, and constraints.

> **Grammar:** See [grammar.ebnf](grammar.md) § DECLARATIONS (generics, where_clause)

## 9.1 Type Identity

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

## 9.2 Assignability

A value of type `S` is assignable to type `T` if:
- `S` is identical to `T`, or
- `S` implements trait `T` and target is `dyn T`

No implicit conversions:

```ori
let x: float = 42;        // error
let x: float = float(42); // OK
```

## 9.3 Variance

Generics are invariant. `Container<T>` is only compatible with `Container<T>`.

## 9.4 Type Constraints

```ori
@sort<T: Comparable> (items: [T]) -> [T] = ...;

@process<T, U> (items: [T], f: (T) -> U) -> [U]
    where T: Clone, U: Default = ...;
```

## 9.5 Default Values

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

## 9.6 Printable Trait

The `Printable` trait provides human-readable string conversion.

```ori
trait Printable {
    @to_str (self) -> str;
}
```

`Printable` is required for string interpolation without format specifiers:

```ori
let x = 42;
`value: {x}`  // Calls x.to_str()
```

### 9.6.1 Standard Implementations

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

### 9.6.2 Derivation

`Printable` is derivable for user-defined types when all fields implement `Printable`:

```ori
#derive(Printable)
type Point = { x: int, y: int }

Point { x: 1, y: 2 }.to_str()  // "Point(1, 2)"
```

Derived implementation creates human-readable format with type name and field values in order.

## 9.7 Formattable Trait

The `Formattable` trait provides formatted string conversion with format specifications.

```ori
trait Formattable {
    @format (self, spec: FormatSpec) -> str;
}
```

`Formattable` is required for string interpolation with format specifiers:

```ori
let n = 42;
`hex: {n:x}`     // Calls n.format(spec: ...) with Hex format type
`padded: {n:08}` // Calls n.format(spec: ...) with width 8, zero-pad
```

### 9.7.1 FormatSpec Type

```ori
type FormatSpec = {
    fill: Option<char>,
    align: Option<Alignment>,
    sign: Option<Sign>,
    width: Option<int>,
    precision: Option<int>,
    format_type: Option<FormatType>,
}

type Alignment = Left | Center | Right;

type Sign = Plus | Minus | Space;

type FormatType = Binary | Octal | Hex | HexUpper | Exp | ExpUpper | Fixed | Percent;
```

These types are in the prelude.

### 9.7.2 Format Spec Syntax

Format specifications in template strings use the syntax:

```
[[fill]align][sign][#][0][width][.precision][type]
```

| Component | Syntax | Description |
|-----------|--------|-------------|
| Fill | Any character | Padding character (default: space) |
| Align | `<` `>` `^` | Left, right, center alignment |
| Sign | `+` `-` ` ` | Sign display for numbers |
| `#` | `#` | Alternate form (prefix for hex, etc.) |
| `0` | `0` | Zero-pad (implies right-align) |
| Width | Integer | Minimum field width |
| Precision | `.N` | Decimal places or max string length |
| Type | Letter | Format type (b, o, x, X, e, E, f, %) |

### 9.7.3 Format Types

**Integer Types:**

| Type | Description | Example |
|------|-------------|---------|
| `b` | Binary | `42` → `"101010"` |
| `o` | Octal | `42` → `"52"` |
| `x` | Hex (lowercase) | `255` → `"ff"` |
| `X` | Hex (uppercase) | `255` → `"FF"` |

**Float Types:**

| Type | Description | Example |
|------|-------------|---------|
| `e` | Scientific (lowercase) | `1234.5` → `"1.2345e+03"` |
| `E` | Scientific (uppercase) | `1234.5` → `"1.2345E+03"` |
| `f` | Fixed-point (6 decimals) | `1234.5` → `"1234.500000"` |
| `%` | Percentage | `0.75` → `"75%"` |

**Alternate Form (`#`):**

| Type | Without `#` | With `#` |
|------|-------------|----------|
| `b` | `"101010"` | `"0b101010"` |
| `o` | `"52"` | `"0o52"` |
| `x` | `"ff"` | `"0xff"` |
| `X` | `"FF"` | `"0xFF"` |

### 9.7.4 Standard Implementations

| Type | Behavior |
|------|----------|
| `int` | Supports b, o, x, X format types; sign and alternate form |
| `float` | Supports e, E, f, % format types; precision and sign |
| `str` | Width, alignment, fill; precision truncates |
| `bool` | Width and alignment only |
| `char` | Width and alignment only |

### 9.7.5 Blanket Implementation

All `Printable` types have a blanket `Formattable` implementation:

```ori
impl<T: Printable> T: Formattable {
    @format (self, spec: FormatSpec) -> str = {
        let base = self.to_str();
        apply_format(s: base, spec: spec)
    }
}
```

This applies width, alignment, and fill. Type-specific formatting (binary, hex, etc.) is only available for types that implement `Formattable` directly.

### 9.7.6 Custom Implementation

User types may implement `Formattable` for custom formatting:

```ori
type Money = { cents: int }

impl Money: Formattable {
    @format (self, spec: FormatSpec) -> str = {
        let dollars = self.cents / 100;
        let cents = self.cents % 100;
        let base = `{dollars}.{cents:02}`;
        apply_alignment(s: base, spec: spec)
    }
}
```

Newtypes can delegate to their inner value:

```ori
type UserId = int;

impl UserId: Formattable {
    @format (self, spec: FormatSpec) -> str = self.inner.format(spec: spec);
}
```

### 9.7.7 Error Codes

| Code | Description |
|------|-------------|
| E0970 | Invalid format specification syntax |
| E0971 | Format type not supported for this type |
| E0972 | Type does not implement `Formattable` |

## 9.8 Default Trait

The `Default` trait provides zero/empty values.

```ori
trait Default {
    @default () -> Self;
}
```

### 9.8.1 Standard Implementations

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

### 9.8.2 Derivation

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
type Status = Pending | Running | Done;
```

## 9.9 Traceable Trait

The `Traceable` trait enables error trace propagation.

```ori
trait Traceable {
    @with_trace (self, entry: TraceEntry) -> Self;
    @trace (self) -> str;
    @trace_entries (self) -> [TraceEntry];
    @has_trace (self) -> bool;
}
```

The `?` operator automatically adds trace entries at propagation points:

```ori
@outer () -> Result<int, Error> = {
    let x = inner()?;  // Adds trace entry for this location
    Ok(x * 2)
}
```

### 9.9.1 TraceEntry Type

```ori
type TraceEntry = {
    function: str,   // Function name with @ prefix
    file: str,       // Source file path
    line: int,       // Line number
    column: int,     // Column number
}
```

### 9.9.2 Standard Implementations

| Type | Implements |
|------|------------|
| `Error` | Yes |
| `Result<T, E>` where `E: Traceable` | Yes (delegates to E) |

## 9.10 Len Trait

The `Len` trait provides length information for collections and sequences.

```ori
trait Len {
    @len (self) -> int;
}
```

### 9.10.1 Semantic Requirements

Implementations _shall_ satisfy:

- **Non-negative**: `x.len() >= 0` for all `x`
- **Deterministic**: `x.len()` returns the same value for unchanged `x`

### 9.10.2 String Length

For `str`, `.len()` returns the **byte count**, not the codepoint or grapheme count:

```ori
"hello".len()  // 5
"café".len()   // 5 (é is 2 bytes in UTF-8)
"日本".len()   // 6 (each character is 3 bytes)
```

### 9.10.3 Standard Implementations

| Type | Implements `Len` | Returns |
|------|-------------------|---------|
| `[T]` | Yes | Number of elements |
| `str` | Yes | Number of bytes |
| `{K: V}` | Yes | Number of entries |
| `Set<T>` | Yes | Number of elements |
| `Range<int>` | Yes | Number of values in range |
| `(T₁, T₂, ...)` | Yes | Number of elements (statically known) |

### 9.10.4 Derivation

`Len` cannot be derived. Types _shall_ implement it explicitly or be built-in.

### 9.10.5 Distinction from Iterator.count()

The `Len` trait is distinct from `Iterator.count()`:

| | `Len.len()` | `Iterator.count()` |
|--|------------|-------------------|
| **Complexity** | O(1) for built-in types | O(n) — consumes the iterator |
| **Side effects** | None — non-consuming | Consuming — iterator is exhausted |
| **Semantics** | Current size of collection | Number of remaining elements |

Iterators do _not_ implement `Len`. To count iterator elements, use `.count()`.

## 9.11 IsEmpty Trait

The `IsEmpty` trait provides emptiness checking for collections and sequences.

```ori
trait IsEmpty {
    @is_empty (self) -> bool
}
```

### 9.11.1 Semantic Requirements

Implementations _shall_ satisfy:

- **Consistency**: `is_empty()` returns `true` if and only if the collection contains no elements
- **Deterministic**: `x.is_empty()` returns the same value for unchanged `x`
- **Relationship to Len**: If a type implements both `Len` and `IsEmpty`, then `x.is_empty() == (x.len() == 0)` _shall_ hold

### 9.11.2 Standard Implementations

All built-in _Iterable_ types implement `IsEmpty`:

| Type | Implements `IsEmpty` | Condition |
|------|---------------------|-----------|
| `[T]` | Yes | `true` if no elements |
| `str` | Yes | `true` if zero bytes |
| `{K: V}` | Yes | `true` if no entries |
| `Set<T>` | Yes | `true` if no elements |
| `[T, max N]` | Yes | `true` if length is 0 |
| `Range<int>` | Yes | `true` if `start >= end` |

### 9.11.3 Types That Do Not Implement IsEmpty

| Type | Reason |
|------|--------|
| Tuples `(T₁, T₂, ...)` | Fixed-size at compile time; never empty |
| `Option<T>` | Not a collection; use `is_none()` or pattern matching |
| `Range<float>` | Not iterable |

### 9.11.4 Relationship to Iterable

`IsEmpty` is independent of `Iterable`. While all built-in `Iterable` types implement `IsEmpty`, a type _may_ implement `IsEmpty` without implementing `Iterable` (e.g., types where emptiness can be checked without iteration).

### 9.11.5 Derivation

`IsEmpty` cannot be derived. Types _shall_ implement it explicitly or be built-in.

## 9.12 Comparable Trait

The `Comparable` trait provides total ordering for values.

```ori
trait Comparable: Eq {
    @compare (self, other: Self) -> Ordering;
}
```

`Comparable` extends `Eq` — all comparable types shall also be equatable.

### 9.12.1 Mathematical Properties

A valid `Comparable` implementation shall satisfy:

**Reflexivity**: `a.compare(other: a) == Equal`

**Antisymmetry**: If `a.compare(other: b) == Less`, then `b.compare(other: a) == Greater`

**Transitivity**: If `a.compare(other: b) == Less` and `b.compare(other: c) == Less`, then `a.compare(other: c) == Less`

**Consistency with Eq**: `a.compare(other: b) == Equal` if and only if `a == b`

### 9.12.2 Operator Derivation

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

### 9.12.3 Standard Implementations

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

### 9.12.4 Float Comparison

Floats follow IEEE 754 total ordering:
- `-Inf < negative < -0.0 < +0.0 < positive < +Inf`
- `NaN` compares equal to itself and greater than all other values

Note: For ordering purposes, `NaN == NaN`. This differs from `==` where `NaN != NaN`.

### 9.12.5 Derivation

`Comparable` is derivable for user-defined types when all fields implement `Comparable`:

```ori
#derive(Eq, Comparable)
type Point = { x: int, y: int }

// Generated: lexicographic comparison by field declaration order
```

For sum types, variants compare by declaration order (`Low < Medium < High`).

## 9.13 Hashable Trait

The `Hashable` trait provides hash values for map keys and set elements.

```ori
trait Hashable: Eq {
    @hash (self) -> int;
}
```

`Hashable` extends `Eq` — all hashable types shall also be equatable.

### 9.13.1 Hash Invariant

**Consistency with Eq**: If `a == b`, then `a.hash() == b.hash()`

The converse is NOT required — different values may have the same hash (collisions are expected).

### 9.13.2 Standard Implementations

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

### 9.13.3 Float Hashing

Floats hash consistently with equality:
- `+0.0` and `-0.0` hash the same (they're equal)
- `NaN` values hash consistently (all NaN equal for hashing)

### 9.13.4 Map Key and Set Element Requirements

To use a type as a map key or set element, it shall implement both `Eq` and `Hashable`.
Using a type that does not implement `Hashable` as a map key is an error (E2031):

```ori
let map: {Point: str} = {};  // Point must be Eq + Hashable
let set: Set<Point> = Set.new();  // Point must be Eq + Hashable
```

### 9.13.5 hash_combine Function

The `hash_combine` function in the prelude mixes hash values:

```ori
@hash_combine (seed: int, value: int) -> int =
    seed ^ (value + 0x9e3779b9 + (seed << 6) + (seed >> 2));
```

This follows the boost hash_combine pattern for good distribution. Users implementing custom `Hashable` can use this function directly.

### 9.13.6 Derivation

`Hashable` is derivable for user-defined types when all fields implement `Hashable`:

```ori
#derive(Eq, Hashable)
type Point = { x: int, y: int }

// Generated: combine field hashes using hash_combine
```

Deriving `Hashable` without `Eq` is an error (E2029). The hash invariant requires that equal values produce equal hashes, which cannot be guaranteed without an `Eq` implementation.

## 9.14 Into Trait

The `Into` trait provides semantic, lossless type conversions.

```ori
trait Into<T> {
    @into (self) -> T;
}
```

`Into<T>` represents a conversion from `Self` to `T`. Unlike `as` conversions (which are built-in and handle representation changes), `Into` is user-extensible and represents semantic conversions between types.

### 9.14.1 Usage

Conversions are always explicit. The caller shall invoke `.into()`:

```ori
let error: Error = "something went wrong".into();
```

When a function accepts `impl Into<T>`, the caller shall still call `.into()` explicitly:

```ori
@fail (err: impl Into<Error>) -> Never = panic(msg: err.into().message);

fail(err: "simple message".into());  // Explicit .into() required
fail(err: Error { message: "detailed" });  // No conversion needed
```

No implicit conversion occurs at call sites. This maintains Ori's "no implicit conversions" principle.

### 9.14.2 Standard Implementations

| Source | Target | Notes |
|--------|--------|-------|
| `str` | `Error` | Creates Error with message |
| `int` | `float` | Lossless numeric widening |
| `Set<T>` | `[T]` | Requires `T: Eq + Hashable` |

NOTE  `Into` is for lossless conversions only. Lossy conversions (like `float` to `int` truncation) require explicit `as` syntax.

### 9.14.3 Relationship to `as`

| Mechanism | Fallible | Implicit | Extensible | Use Case |
|-----------|----------|----------|------------|----------|
| `as` | No | No | No | Primitive representation changes |
| `as?` | Yes | No | No | Parsing, checked conversions |
| `Into` | No | No | Yes | Semantic type conversions |

### 9.14.4 Custom Implementations

User types may implement `Into` for meaningful conversions:

```ori
type UserId = int;

impl UserId: Into<str> {
    @into (self) -> str = `user-{self.inner}`;
}

let id = UserId(42);
let s: str = id.into();  // "user-42"
```

### 9.14.5 No Blanket Identity

There is no blanket `impl<T> T: Into<T>`. Each conversion shall be explicitly implemented. This ensures `impl Into<T>` parameters remain meaningful — they indicate types that can be converted to `T`, not any type.

### 9.14.6 No Automatic Chaining

Conversions do not chain automatically:

```ori
// Given: A implements Into<B>, B implements Into<C>
let a: A = ...;
let c: C = a.into();         // ERROR: A does not implement Into<C>
let c: C = a.into().into();  // OK: explicit A → B → C
```

### 9.14.7 Orphan Rules

`Into` implementations follow standard orphan rules:
- Implement in the module defining the source type, OR
- Implement in the module defining the target type

### 9.14.8 Error Codes

| Code | Description |
|------|-------------|
| E2036 | Type does not implement `Into<T>` |
| E2037 | Multiple `Into` implementations apply (ambiguous) |
