# Proposal: Unary and Bitwise Operators

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, expressions, type system

---

## Summary

This proposal formalizes unary and bitwise operator semantics, including type constraints, overflow behavior, and operator traits.

---

## Problem Statement

The spec lists operators but leaves unclear:

1. **Type constraints**: Which types support which operators?
2. **Overflow behavior**: What happens on overflow?
3. **Operator traits**: Can these be overloaded?
4. **Precedence details**: Complete precedence table?
5. **Bit widths**: How do shifts behave with large counts?

---

# Unary Operators

## Logical Not (`!`)

### Semantics

Inverts a boolean value:

```ori
!true   // false
!false  // true
```

### Type Constraint

```ori
! : bool -> bool
```

Only valid on `bool`. Not valid on integers (use `~` for bitwise not).

### Chaining

```ori
!!true   // true (double negation)
!!!x     // !x
```

## Arithmetic Negation (`-`)

### Semantics

Negates a numeric value:

```ori
-42      // -42
-3.14    // -3.14
-(-5)    // 5
```

### Type Constraint

```ori
- : int -> int
- : float -> float
```

### Overflow

Integer negation panics on overflow:

```ori
-(-9223372036854775808)  // panic: integer overflow
// Because +9223372036854775808 doesn't fit in int
```

Float negation never overflows (just flips sign bit).

## Bitwise Not (`~`)

### Semantics

Inverts all bits:

```ori
~0       // -1 (all bits flipped)
~(-1)    // 0
~0b1010  // ...11110101 (signed)
```

### Type Constraint

```ori
~ : int -> int
```

Only valid on `int`. For booleans, use `!`.

### Behavior

`~x` is equivalent to `-(x + 1)` for signed integers:

```ori
~5   // -6
~(-3) // 2
```

---

# Bitwise Operators

## Bitwise AND (`&`)

### Semantics

```ori
0b1100 & 0b1010  // 0b1000 (8)
```

### Type Constraint

```ori
& : (int, int) -> int
```

### Common Uses

```ori
// Check if bit is set
x & (1 << n) != 0

// Clear bits
x & ~mask

// Extract bits
x & 0xFF  // Low byte
```

## Bitwise OR (`|`)

### Semantics

```ori
0b1100 | 0b1010  // 0b1110 (14)
```

### Type Constraint

```ori
| : (int, int) -> int
```

### Common Uses

```ori
// Set bit
x | (1 << n)

// Combine flags
FLAG_A | FLAG_B
```

## Bitwise XOR (`^`)

### Semantics

```ori
0b1100 ^ 0b1010  // 0b0110 (6)
```

### Type Constraint

```ori
^ : (int, int) -> int
```

### Common Uses

```ori
// Toggle bit
x ^ (1 << n)

// Swap without temp
a = a ^ b
b = a ^ b
a = a ^ b
```

## Left Shift (`<<`)

### Semantics

Shifts bits left, filling with zeros:

```ori
1 << 4   // 16 (0b10000)
3 << 2   // 12 (0b1100)
```

### Type Constraint

```ori
<< : (int, int) -> int
```

### Overflow

Shift overflow panics:

```ori
1 << 63   // OK: 9223372036854775808 (but wait, that's i64::MIN)
1 << 64   // panic: shift overflow
```

### Negative Shift Count

Negative shift count panics:

```ori
1 << -1  // panic: negative shift count
```

## Right Shift (`>>`)

### Semantics

Arithmetic right shift (preserves sign):

```ori
16 >> 2     // 4
-16 >> 2    // -4 (sign-extended)
```

### Type Constraint

```ori
>> : (int, int) -> int
```

### Negative Shift Count

```ori
16 >> -1  // panic: negative shift count
```

### Large Shift Count

```ori
16 >> 64  // panic: shift overflow
16 >> 100 // panic: shift overflow
```

---

# Precedence

Complete precedence table (highest to lowest):

| Level | Operators | Associativity | Description |
|-------|-----------|---------------|-------------|
| 1 | `.` `[]` `()` `?` | Left | Postfix |
| 2 | `!` `-` `~` | Right | Unary |
| 3 | `*` `/` `%` `div` | Left | Multiplicative |
| 4 | `+` `-` | Left | Additive |
| 5 | `<<` `>>` | Left | Shift |
| 6 | `..` `..=` `by` | Left | Range |
| 7 | `<` `>` `<=` `>=` | Left | Relational |
| 8 | `==` `!=` | Left | Equality |
| 9 | `&` | Left | Bitwise AND |
| 10 | `^` | Left | Bitwise XOR |
| 11 | `\|` | Left | Bitwise OR |
| 12 | `&&` | Left | Logical AND |
| 13 | `\|\|` | Left | Logical OR |
| 14 | `??` | Left | Coalesce |

### Parentheses

Use parentheses when precedence is unclear:

```ori
(a & b) == 0    // Clear intent
a & b == 0      // May be confusing: a & (b == 0)
```

---

# No Operator Overloading

## Design Decision

Ori does NOT support user-defined operator overloading:

```ori
impl Add for Point {
    @add (self, other: Point) -> Point = ...  // NOT SUPPORTED
}

point1 + point2  // ERROR: + not defined for Point
```

### Rationale

1. **Clarity**: Operators have predictable behavior
2. **Simplicity**: No complex resolution rules
3. **Readability**: Methods are explicit
4. **Consistency**: All user operations use method syntax

### Alternative

Use named methods:

```ori
impl Point {
    @add (self, other: Point) -> Point = Point {
        x: self.x + other.x,
        y: self.y + other.y,
    }
}

point1.add(other: point2)  // Clear intent
```

---

# Error Messages

### Type Mismatch

```
error[E1020]: mismatched types for `~`
  --> src/main.ori:5:5
   |
 5 |     ~true
   |     ^^^^^ expected `int`, found `bool`
   |
   = help: use `!` for boolean negation
```

### Overflow

```
error[E1021]: integer overflow in negation
  --> src/main.ori:5:5
   |
 5 |     let x = -(-9223372036854775808)
   |             ^^^^^^^^^^^^^^^^^^^^^^^ overflow
   |
   = note: the positive value doesn't fit in `int`
```

### Invalid Shift

```
error[E1022]: negative shift count
  --> src/main.ori:5:10
   |
 5 |     x << -1
   |          ^^ negative shift
   |
   = note: shift count must be non-negative
```

---

## Spec Changes Required

### Update `09-expressions.md`

Add comprehensive operator sections with:
1. Complete type constraints
2. Overflow behavior
3. Full precedence table
4. No-overloading rationale

---

## Summary

### Unary Operators

| Operator | Type | Result | Notes |
|----------|------|--------|-------|
| `!` | `bool` | `bool` | Logical not |
| `-` | `int`, `float` | Same | Arithmetic negation |
| `~` | `int` | `int` | Bitwise not |

### Bitwise Operators

| Operator | Types | Result | Notes |
|----------|-------|--------|-------|
| `&` | `(int, int)` | `int` | Bitwise AND |
| `\|` | `(int, int)` | `int` | Bitwise OR |
| `^` | `(int, int)` | `int` | Bitwise XOR |
| `<<` | `(int, int)` | `int` | Left shift |
| `>>` | `(int, int)` | `int` | Arithmetic right shift |

### Key Properties

| Property | Behavior |
|----------|----------|
| Integer overflow | Panic |
| Shift overflow | Panic |
| Negative shift | Panic |
| Operator overloading | Not supported |
