# Operator Traits Proposal

**Status:** Approved
**Approved:** 2026-01-31
**Author:** Claude
**Created:** 2026-01-31
**Depends On:** default-type-parameters-proposal.md, default-associated-types-proposal.md
**Enables:** duration-size-to-stdlib.md

## Summary

Define traits for arithmetic, bitwise, and unary operators that user-defined types can implement to support operator syntax. The compiler desugars operators to trait method calls.

## Motivation

Currently, operators (`+`, `-`, `*`, `/`, `%`, `-x`, `~x`, `!x`) are hardcoded in the compiler for built-in types only. User-defined types cannot use operator syntax:

```ori
type Vector2 = { x: float, y: float }

// Today: verbose method calls
let sum = v1.add(other: v2)

// Goal: natural operator syntax
let sum = v1 + v2
```

This limitation prevents:
1. Mathematical types (vectors, matrices, complex numbers)
2. Unit types (Duration, Size, Currency, Temperature)
3. Wrapper types that should behave like their inner type
4. Domain-specific numeric types

## Design

### Trait Definitions

Operator traits are defined in the prelude:

```ori
// Binary arithmetic operators
trait Add<Rhs = Self> {
    type Output = Self
    @add (self, rhs: Rhs) -> Self.Output
}

trait Sub<Rhs = Self> {
    type Output = Self
    @sub (self, rhs: Rhs) -> Self.Output
}

trait Mul<Rhs = Self> {
    type Output = Self
    @mul (self, rhs: Rhs) -> Self.Output
}

trait Div<Rhs = Self> {
    type Output = Self
    @div (self, rhs: Rhs) -> Self.Output
}

trait FloorDiv<Rhs = Self> {
    type Output = Self
    @floor_div (self, rhs: Rhs) -> Self.Output
}

trait Rem<Rhs = Self> {
    type Output = Self
    @rem (self, rhs: Rhs) -> Self.Output
}

// Unary operators
trait Neg {
    type Output = Self
    @neg (self) -> Self.Output
}

trait Not {
    type Output = Self
    @not (self) -> Self.Output
}

trait BitNot {
    type Output = Self
    @bit_not (self) -> Self.Output
}

// Bitwise operators
trait BitAnd<Rhs = Self> {
    type Output = Self
    @bit_and (self, rhs: Rhs) -> Self.Output
}

trait BitOr<Rhs = Self> {
    type Output = Self
    @bit_or (self, rhs: Rhs) -> Self.Output
}

trait BitXor<Rhs = Self> {
    type Output = Self
    @bit_xor (self, rhs: Rhs) -> Self.Output
}

trait Shl<Rhs = int> {
    type Output = Self
    @shl (self, rhs: Rhs) -> Self.Output
}

trait Shr<Rhs = int> {
    type Output = Self
    @shr (self, rhs: Rhs) -> Self.Output
}
```

### Operator Desugaring

The compiler desugars operators to trait method calls:

| Operator | Desugars To |
|----------|-------------|
| `a + b` | `a.add(rhs: b)` |
| `a - b` | `a.sub(rhs: b)` |
| `a * b` | `a.mul(rhs: b)` |
| `a / b` | `a.div(rhs: b)` |
| `a div b` | `a.floor_div(rhs: b)` |
| `a % b` | `a.rem(rhs: b)` |
| `-a` | `a.neg()` |
| `!a` | `a.not()` |
| `~a` | `a.bit_not()` |
| `a & b` | `a.bit_and(rhs: b)` |
| `a \| b` | `a.bit_or(rhs: b)` |
| `a ^ b` | `a.bit_xor(rhs: b)` |
| `a << b` | `a.shl(rhs: b)` |
| `a >> b` | `a.shr(rhs: b)` |

### Existing Comparison Operators

Comparison operators already use traits:

| Operator | Trait | Method |
|----------|-------|--------|
| `a == b` | `Eq` | `a.equals(other: b)` |
| `a != b` | `Eq` | `!a.equals(other: b)` |
| `a < b` | `Comparable` | `a.compare(other: b).is_less()` |
| `a <= b` | `Comparable` | `a.compare(other: b).is_less_or_equal()` |
| `a > b` | `Comparable` | `a.compare(other: b).is_greater()` |
| `a >= b` | `Comparable` | `a.compare(other: b).is_greater_or_equal()` |

These remain unchanged.

### Built-in Implementations

Primitives have built-in implementations:

```ori
impl Add for int {
    type Output = int
    @add (self, rhs: int) -> int = /* intrinsic */
}

impl Add for float {
    type Output = float
    @add (self, rhs: float) -> float = /* intrinsic */
}

impl Add for str {
    type Output = str
    @add (self, rhs: str) -> str = /* intrinsic: concatenation */
}

impl Add for Duration {
    type Output = Duration
    @add (self, rhs: Duration) -> Duration = /* intrinsic */
}

// ... etc for all primitives
```

### Mixed-Type Operations

Traits support different right-hand-side types:

```ori
impl Mul<int> for Duration {
    type Output = Duration
    @mul (self, n: int) -> Duration = Duration.from_nanoseconds(ns: self.nanoseconds() * n)
}

impl Div<int> for Duration {
    type Output = Duration
    @div (self, n: int) -> Duration = Duration.from_nanoseconds(ns: self.nanoseconds() / n)
}

// Usage
let doubled = 5s * 2      // Duration * int -> Duration
let halved = 10s / 2      // Duration / int -> Duration
```

### Commutative Mixed-Type Operations

For operations where both orderings should be valid (e.g., `int * Duration` and `Duration * int`), implement both directions explicitly:

```ori
// Duration * int
impl Mul<int> for Duration {
    type Output = Duration
    @mul (self, n: int) -> Duration = Duration.from_nanoseconds(ns: self.nanoseconds() * n)
}

// int * Duration
impl Mul<Duration> for int {
    type Output = Duration
    @mul (self, d: Duration) -> Duration = d * self  // Delegate to Duration * int
}

// Usage
let a = 5s * 2      // Duration * int -> Duration
let b = 2 * 5s      // int * Duration -> Duration (same result)
```

The compiler does not automatically commute operands. Each ordering requires an explicit implementation.

### User-Defined Example

```ori
type Vector2 = { x: float, y: float }

impl Add for Vector2 {
    @add (self, rhs: Vector2) -> Self = Vector2 {
        x: self.x + rhs.x,
        y: self.y + rhs.y,
    }
}

impl Sub for Vector2 {
    @sub (self, rhs: Vector2) -> Self = Vector2 {
        x: self.x - rhs.x,
        y: self.y - rhs.y,
    }
}

impl Mul<float> for Vector2 {
    @mul (self, scalar: float) -> Self = Vector2 {
        x: self.x * scalar,
        y: self.y * scalar,
    }
}

impl Neg for Vector2 {
    @neg (self) -> Self = Vector2 { x: -self.x, y: -self.y }
}

// Usage
let a = Vector2 { x: 1.0, y: 2.0 }
let b = Vector2 { x: 3.0, y: 4.0 }
let sum = a + b           // Vector2 { x: 4.0, y: 6.0 }
let diff = a - b          // Vector2 { x: -2.0, y: -2.0 }
let scaled = a * 2.0      // Vector2 { x: 2.0, y: 4.0 }
let negated = -a          // Vector2 { x: -1.0, y: -2.0 }
```

### Chaining

Operators chain naturally with method calls:

```ori
let result = Vector2.zero()
    .add(rhs: offset)
    .mul(scalar: 2.0)
    .normalize()

// Or with operators
let result = ((Vector2.zero() + offset) * 2.0).normalize()
```

### Derivation

Common cases can use `#derive`:

```ori
// For newtypes wrapping numeric types
#derive(Add, Sub, Mul, Div)
type Meters = { value: float }

// Generates:
impl Add for Meters {
    @add (self, rhs: Meters) -> Self = Meters { value: self.value + rhs.value }
}
// ... etc
```

## Language Features Required

### 1. Default Type Parameters on Traits (REQUIRED)

The syntax `trait Add<Rhs = Self>` requires default type parameters:

```ori
trait Add<Rhs = Self> {  // Rhs defaults to Self if not specified
    type Output = Self
    @add (self, rhs: Rhs) -> Self.Output
}

// These are equivalent:
impl Add for Point { ... }
impl Add<Point> for Point { ... }
```

**Status:** APPROVED — See `default-type-parameters-proposal.md`

### 2. Default Associated Types (REQUIRED)

The syntax `type Output = Self` requires default associated types:

```ori
trait Add<Rhs = Self> {
    type Output = Self  // Defaults to Self if not specified
    @add (self, rhs: Rhs) -> Self.Output
}

// Can omit Output if it's Self:
impl Add for Point {
    @add (self, rhs: Point) -> Self = ...  // Output inferred as Self = Point
}
```

**Status:** APPROVED — See `default-associated-types-proposal.md`

### 3. Self in Associated Type Defaults (REQUIRED)

`Self` must be usable in associated type default values:

```ori
trait Add<Rhs = Self> {
    type Output = Self  // Self refers to implementing type
}
```

**Status:** IMPLEMENTED — Covered by default-type-parameters and default-associated-types proposals.

### 4. Derive Macros for Operator Traits (NICE TO HAVE)

`#derive(Add, Sub, ...)` for newtypes:

```ori
#derive(Add, Sub, Mul, Div)
type Celsius = { value: float }
```

**Status:** NOT IMPLEMENTED — derive system exists but not for operators. Defer to future proposal.

## Implementation Plan

### Phase 1: Language Prerequisites

1. Implement default type parameters on traits ✅ (approved)
2. Implement default associated types ✅ (approved)
3. Verify `Self` works in associated type defaults ✅ (covered by above)

### Phase 2: Operator Traits

1. Define operator traits in prelude (`Add`, `Sub`, `Mul`, `Div`, `FloorDiv`, `Rem`, `Neg`, `Not`, `BitNot`, etc.)
2. Modify type checker to desugar operators to trait method calls
3. Modify evaluator to dispatch operators via trait impls
4. Add built-in impls for primitives (int, float, str, Duration, Size, etc.)

### Phase 3: Testing

1. User-defined types with operators
2. Mixed-type operations (`Duration * int`, `int * Duration`)
3. Chaining operators and methods
4. Error messages for missing impls

### Phase 4: Derive Support (Optional)

1. Add `#derive(Add)`, `#derive(Sub)`, etc.
2. Generate appropriate impls for newtypes

## Error Messages

Good error messages are critical:

```ori
let x = Point { x: 1, y: 2 } + 5

// Error: cannot add `Point` and `int`
//   --> file.ori:3:9
//   |
// 3 | let x = Point { x: 1, y: 2 } + 5
//   |         ^^^^^^^^^^^^^^^^^^^^^^^^
//   |
//   = note: `Point` implements `Add<Point>` but not `Add<int>`
//   = help: consider implementing `Add<int>` for `Point`
```

## Alternatives Considered

### Method-Only Approach

Require explicit method calls instead of operators:

```ori
let sum = v1.add(other: v2)
```

**Rejected:** Too verbose for mathematical code, doesn't match user expectations.

### Operator Functions

Define operators as standalone functions:

```ori
@(+) (a: Vector2, b: Vector2) -> Vector2 = ...
```

**Rejected:** Doesn't integrate with trait system, can't have multiple impls.

### Compiler Intrinsics Only

Keep operators for built-in types only.

**Rejected:** Prevents Duration/Size from moving to stdlib, limits user types.

## References

- Rust: `std::ops` module
- Haskell: Numeric type classes
- Swift: Operator declarations
- Archived design: `docs/ori_lang/0.1-alpha/archived-design/appendices/C-builtin-traits.md`
