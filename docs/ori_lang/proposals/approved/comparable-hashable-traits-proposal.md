# Proposal: Comparable and Hashable Traits

**Status:** Approved
**Approved:** 2026-01-30
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, type system, traits

---

## Summary

This proposal formalizes the `Comparable` and `Hashable` traits, including their definitions, invariants, standard implementations, and derivation rules.

---

## Problem Statement

The spec lists these traits in the prelude but leaves unclear:

1. **Definitions**: What are the exact trait signatures?
2. **Invariants**: What mathematical properties must hold?
3. **Implementations**: Which types implement these traits?
4. **Relationship**: How do Eq, Comparable, and Hashable relate?
5. **Derivation**: How are these traits derived?

---

## Comparable Trait

### Definition

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
| `float` | IEEE 754 order (NaN handling) |
| `bool` | `false < true` |
| `str` | Lexicographic (Unicode codepoint) |
| `char` | Unicode codepoint |
| `byte` | Numeric order |
| `Duration` | Shorter < longer |
| `Size` | Smaller < larger |
| `[T]` where `T: Comparable` | Lexicographic |
| `(T1, T2, ...)` where all `Ti: Comparable` | Lexicographic |
| `Option<T>` where `T: Comparable` | `None < Some(_)` |
| `Result<T, E>` where `T: Comparable, E: Comparable` | `Ok(_) < Err(_)`, then compare inner values |
| `Ordering` | `Less < Equal < Greater` |

### Non-Comparable Collections

Maps and Sets are not Comparable because they are unordered collections:

| Type | Hashable | Comparable |
|------|----------|------------|
| `{K: V}` | Yes (when K, V: Hashable) | No (unordered) |
| `Set<T>` | Yes (when T: Hashable) | No (unordered) |

### Float Comparison

Floats follow IEEE 754 total ordering:
- `-Inf < negative < -0.0 < +0.0 < positive < +Inf`
- `NaN` compares equal to itself and greater than all other values

```ori
let a = 0.0 / 0.0  // NaN
a.compare(other: a)  // Equal (NaN == NaN for ordering purposes)
```

Note: This differs from `==` where `NaN != NaN`.

### Derivation

For complete `#derive` semantics including field constraints, generic types, and error handling, see the derived-traits-proposal. This proposal defines the specific derivation logic for `Comparable` and `Hashable`.

```ori
#derive(Eq, Comparable)
type Point = { x: int, y: int }

// Generated: lexicographic comparison by field declaration order
impl Comparable for Point {
    @compare (self, other: Point) -> Ordering =
        compare(left: self.x, right: other.x)
            .then(other: compare(left: self.y, right: other.y))
}
```

For sum types, variants compare by declaration order:

```ori
#derive(Eq, Comparable)
type Priority = Low | Medium | High

// Low < Medium < High
```

---

## Hashable Trait

### Definition

```ori
trait Hashable: Eq {
    @hash (self) -> int
}
```

`Hashable` extends `Eq` — all hashable types must also be equatable.

### Hash Invariant

**Consistency with Eq**: If `a == b`, then `a.hash() == b.hash()`

The converse is NOT required — different values may have the same hash (collisions are expected).

### Hash Quality

While not required, implementations should aim for:
- Good distribution across the int range
- Avalanche effect (small input changes cause large hash changes)
- Fast computation

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

Floats must hash consistently with equality:
- `+0.0` and `-0.0` must hash the same (they're equal)
- `NaN` values must hash consistently (all NaN equal for hashing)

```ori
(0.0).hash() == (-0.0).hash()  // true
```

### Map Key Requirements

To use a type as a map key, it must implement both `Eq` and `Hashable`:

```ori
let map: {Point: str} = {}  // Point must be Eq + Hashable
```

### Set Element Requirements

Similarly for set elements:

```ori
let set: Set<Point> = Set.new()  // Point must be Eq + Hashable
```

### Derivation

```ori
#derive(Eq, Hashable)
type Point = { x: int, y: int }

// Generated: combine field hashes
impl Hashable for Point {
    @hash (self) -> int = {
        let h = 0
        h = hash_combine(seed: h, value: self.x.hash())
        h = hash_combine(seed: h, value: self.y.hash())
        h
    }
}
```

### hash_combine Function

The `hash_combine` function is available in the prelude and mixes hash values:

```ori
@hash_combine (seed: int, value: int) -> int =
    seed ^ (value + 0x9e3779b9 + (seed << 6) + (seed >> 2))
```

This follows the boost hash_combine pattern for good distribution. Users implementing custom `Hashable` can use this function directly.

---

## Relationship Diagram

```
       Eq
      /  \
     /    \
Comparable  Hashable
```

- `Eq` is the base trait for equality
- `Comparable` extends `Eq` for ordering
- `Hashable` extends `Eq` for hashing

A type may implement:
- Only `Eq` (equality without ordering or hashing)
- `Eq + Hashable` (can be map key)
- `Eq + Comparable` (can be sorted)
- `Eq + Comparable + Hashable` (full functionality)

---

## Cannot Implement

### Types Without Eq

Types that don't have meaningful equality cannot implement these traits:

```ori
type Closure = (int) -> int
// Closures cannot implement Eq, Comparable, or Hashable
```

### Types With Interior State

Types with hidden mutable state should not implement these traits:

```ori
type Counter = { value: int }  // If value mutates, hash could change
```

---

## Error Messages

### Missing Eq

```
error[E0940]: cannot derive `Hashable` without `Eq`
  --> src/types.ori:1:10
   |
 1 | #derive(Hashable)
   |         ^^^^^^^^ requires `Eq`
   |
   = help: add `Eq`: `#derive(Eq, Hashable)`
```

### Hash Invariant Violation

```
error[E0941]: `Hashable` implementation violates hash invariant
  --> src/types.ori:5:5
   |
   = note: equal values must have equal hashes
   = note: a == b but a.hash() != b.hash()
```

### Non-Hashable Map Key

```
error[E0942]: `MyType` cannot be used as map key
  --> src/main.ori:5:10
   |
 5 | let map: {MyType: int} = {}
   |           ^^^^^^ does not implement `Hashable`
   |
   = help: derive or implement `Hashable` for `MyType`
```

---

## Spec Changes Required

### Update `07-properties-of-types.md`

Add comprehensive Comparable and Hashable sections with:
1. Trait definitions
2. Mathematical invariants
3. Standard implementations
4. Derivation rules

---

## Summary

| Trait | Extends | Purpose | Key Invariant |
|-------|---------|---------|---------------|
| `Eq` | — | Equality | Reflexive, symmetric, transitive |
| `Comparable` | `Eq` | Ordering | Consistent with Eq, total order |
| `Hashable` | `Eq` | Hashing | `a == b` implies `a.hash() == b.hash()` |

| Capability | Required Traits |
|------------|-----------------|
| `==`, `!=` | `Eq` |
| `<`, `<=`, `>`, `>=` | `Comparable` |
| Map key | `Eq + Hashable` |
| Set element | `Eq + Hashable` |
| Sorting | `Comparable` |
