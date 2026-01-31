# Operator Trait Method Naming

**Status:** Approved
**Approved:** 2026-01-31
**Author:** Claude
**Created:** 2026-01-31
**Depends On:** operator-traits-proposal.md

## Summary

Rename operator trait methods to use full words instead of abbreviations for consistency and readability.

## Motivation

Currently, operator trait method names are inconsistent:

| Trait | Current Method | Issue |
|-------|----------------|-------|
| `Add` | `add` | Fine (already full word) |
| `Sub` | `sub` | Abbreviation |
| `Mul` | `mul` | Abbreviation |
| `Div` | `divide` | Full word (due to `div` keyword) |
| `FloorDiv` | `floor_div` | Abbreviation |
| `Rem` | `rem` | Abbreviation |
| `Neg` | `neg` | Abbreviation |
| `Not` | `not` | Fine (already full word) |
| `BitNot` | `bit_not` | Fine |
| `BitAnd` | `bit_and` | Fine |
| `BitOr` | `bit_or` | Fine |
| `BitXor` | `bit_xor` | Fine |
| `Shl` | `shl` | Abbreviation |
| `Shr` | `shr` | Abbreviation |

The `Div` trait uses `divide` because `div` is a keyword (floor division operator). This creates an inconsistency where some methods use abbreviations and one uses the full word.

### Problems

1. **Inconsistency**: `divide` stands out among `add`, `sub`, `mul`, `rem`
2. **Readability**: Full words are more readable in method chains and impl blocks
3. **Discoverability**: New users may not recognize `rem` as "remainder" or `shl` as "shift left"

## Design

### Method Renames

| Trait | Current | Proposed |
|-------|---------|----------|
| `Sub` | `sub` | `subtract` |
| `Mul` | `mul` | `multiply` |
| `Div` | `divide` | `divide` (unchanged) |
| `FloorDiv` | `floor_div` | `floor_divide` |
| `Rem` | `rem` | `remainder` |
| `Neg` | `neg` | `negate` |
| `Shl` | `shl` | `shift_left` |
| `Shr` | `shr` | `shift_right` |

### Unchanged Methods

These methods are already full words or conventional:

| Trait | Method | Reason |
|-------|--------|--------|
| `Add` | `add` | Already full word |
| `Not` | `not` | Already full word |
| `BitNot` | `bit_not` | Consistent with bitwise family |
| `BitAnd` | `bit_and` | Consistent with bitwise family |
| `BitOr` | `bit_or` | Consistent with bitwise family |
| `BitXor` | `bit_xor` | Consistent with bitwise family |

### Updated Trait Definitions

```ori
trait Sub<Rhs = Self> {
    type Output = Self
    @subtract (self, rhs: Rhs) -> Self.Output
}

trait Mul<Rhs = Self> {
    type Output = Self
    @multiply (self, rhs: Rhs) -> Self.Output
}

trait Div<Rhs = Self> {
    type Output = Self
    @divide (self, rhs: Rhs) -> Self.Output
}

trait FloorDiv<Rhs = Self> {
    type Output = Self
    @floor_divide (self, rhs: Rhs) -> Self.Output
}

trait Rem<Rhs = Self> {
    type Output = Self
    @remainder (self, rhs: Rhs) -> Self.Output
}

trait Neg {
    type Output = Self
    @negate (self) -> Self.Output
}

trait Shl<Rhs = int> {
    type Output = Self
    @shift_left (self, rhs: Rhs) -> Self.Output
}

trait Shr<Rhs = int> {
    type Output = Self
    @shift_right (self, rhs: Rhs) -> Self.Output
}
```

### Updated Desugaring Table

| Operator | Desugars To |
|----------|-------------|
| `a + b` | `a.add(rhs: b)` |
| `a - b` | `a.subtract(rhs: b)` |
| `a * b` | `a.multiply(rhs: b)` |
| `a / b` | `a.divide(rhs: b)` |
| `a div b` | `a.floor_divide(rhs: b)` |
| `a % b` | `a.remainder(rhs: b)` |
| `-a` | `a.negate()` |
| `!a` | `a.not()` |
| `~a` | `a.bit_not()` |
| `a & b` | `a.bit_and(rhs: b)` |
| `a \| b` | `a.bit_or(rhs: b)` |
| `a ^ b` | `a.bit_xor(rhs: b)` |
| `a << b` | `a.shift_left(rhs: b)` |
| `a >> b` | `a.shift_right(rhs: b)` |

## Implementation

### Files to Update

1. **library/std/prelude.ori** — Rename methods in trait definitions
2. **compiler/ori_typeck/src/infer/expressions/operators.rs** — Update `binary_op_to_trait()` and `unary_op_to_trait()`
3. **compiler/ori_eval/src/interpreter/mod.rs** — Update `binary_op_to_method()`
4. **docs/ori_lang/0.1-alpha/spec/09-expressions.md** — Update operator traits table
5. **docs/ori_lang/proposals/approved/operator-traits-proposal.md** — Update method names
6. **docs/ori_lang/proposals/approved/duration-size-to-stdlib-proposal.md** — Update method names
7. **tests/spec/traits/operators/user_defined.ori** — Update test implementations

### Migration

This is a breaking change for any code that:
1. Implements operator traits with the old method names
2. Calls operator methods directly (rare)

Since operator traits were just implemented and not yet released, there is no migration burden.

## Alternatives Considered

### Keep Abbreviations

Continue using `sub`, `mul`, `rem`, etc.

**Rejected**: Creates permanent inconsistency with `divide`.

### Abbreviate `divide` to `div`

Change `divide` back to `div` and accept the keyword conflict by using a different parsing strategy.

**Rejected**: Introduces parser complexity and potential confusion. The keyword `div` for floor division is well-established.

### Use Rust-style Names

Use Rust's exact names: `sub`, `mul`, `div`, `rem`, `neg`, `shl`, `shr`.

**Rejected**: Ori already diverged from Rust by using `divide`. Full words are more readable and Ori values clarity over terseness.

## References

- operator-traits-proposal.md — Original operator traits design
- Rust std::ops — Uses abbreviated names (sub, mul, div, rem)
- Python — Uses full names (__sub__, __mul__, __truediv__, __mod__)
- Swift — Uses full names (subtract, multiply, divide)
