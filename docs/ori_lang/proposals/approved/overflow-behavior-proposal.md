# Proposal: Integer Overflow Behavior

**Status:** Approved
**Approved:** 2026-01-28
**Author:** Eric
**Created:** 2026-01-22
**Draft:** 2026-01-25

---

## Summary

Define integer overflow behavior in Ori: panic by default for safety, with explicit stdlib functions for alternative behaviors (saturation, wrapping).

```ori
// Default: overflow panics
let x: int = int.max + 1  // PANIC: integer overflow

// Explicit alternatives via std.math
use std.math { saturating_add, wrapping_add }

saturating_add(a: int.max, b: 1)  // Returns int.max (clamped)
wrapping_add(a: int.max, b: 1)    // Returns int.min (wrapped)
```

---

## Motivation

### The Problem

Integer overflow is a common source of bugs:

- **Security vulnerabilities**: Buffer overflows, integer overflow exploits
- **Silent corruption**: Wrong values propagate through calculations
- **Difficult debugging**: Bugs appear far from the actual overflow

Ori has fixed-width integer types:

- `int` — 64-bit signed integer
- `byte` — 8-bit unsigned integer

Both types can overflow. The question is: what should happen when they do?

### Prior Art

| Language | Default Behavior | Alternatives |
|----------|-----------------|--------------|
| C/C++ | Undefined behavior (signed), wrap (unsigned) | — |
| Java | Wrap silently | `Math.addExact()` throws |
| Python | Arbitrary precision (no overflow) | — |
| Rust | Panic in debug, wrap in release | `saturating_add()`, `wrapping_add()`, `checked_add()` |
| Swift | Trap (crash) | `&+`, `&-` operators for wrapping |

### The Ori Way

Ori's core principles:

1. **Explicit over implicit** — No silent behavior changes
2. **Safe by default** — Bugs should be caught, not hidden
3. **No undefined behavior** — Every operation has defined semantics

This leads to: **Panic on overflow by default**, with explicit functions when other behavior is needed.

---

## Design

### Default Behavior: Panic

All arithmetic operations on `int` and `byte` panic on overflow:

```ori
// Overflow panics
int.max + 1        // PANIC: integer overflow
int.min - 1        // PANIC: integer overflow
int.max * 2        // PANIC: integer overflow

byte.max + byte(1) // PANIC: integer overflow
```

**Panic message format:**

```
PANIC at src/calc.ori:42:15
  integer overflow in addition
    left:  9223372036854775807 (int.max)
    right: 1
    operation: +
```

### Explicit Alternatives: std.math Functions

For cases where overflow behavior is intentional, use explicit functions:

```ori
use std.math {
    saturating_add, saturating_sub, saturating_mul,
    wrapping_add, wrapping_sub, wrapping_mul,
    checked_add, checked_sub, checked_mul
}
```

#### Saturating Arithmetic

Clamps the result to the type's bounds:

```ori
saturating_add(a: int.max, b: 1)     // int.max
saturating_add(a: int.max, b: 100)   // int.max
saturating_sub(a: int.min, b: 1)     // int.min
saturating_mul(a: int.max, b: 2)     // int.max

saturating_add(a: byte.max, b: byte(1))  // byte.max (255)
```

#### Wrapping Arithmetic

Wraps around on overflow (modular arithmetic):

```ori
wrapping_add(a: int.max, b: 1)     // int.min
wrapping_sub(a: int.min, b: 1)     // int.max
wrapping_mul(a: int.max, b: 2)     // -2

wrapping_add(a: byte.max, b: byte(1))  // byte(0)
```

#### Checked Arithmetic

Returns `Option<T>` — `None` on overflow:

```ori
checked_add(a: int.max, b: 1)    // None
checked_add(a: 100, b: 200)      // Some(300)
checked_sub(a: int.min, b: 1)    // None
checked_mul(a: int.max, b: 2)    // None
```

### Type Signatures

```ori
// Saturating
@saturating_add (a: int, b: int) -> int
@saturating_sub (a: int, b: int) -> int
@saturating_mul (a: int, b: int) -> int

// Wrapping
@wrapping_add (a: int, b: int) -> int
@wrapping_sub (a: int, b: int) -> int
@wrapping_mul (a: int, b: int) -> int

// Checked
@checked_add (a: int, b: int) -> Option<int>
@checked_sub (a: int, b: int) -> Option<int>
@checked_mul (a: int, b: int) -> Option<int>

// Byte versions
@saturating_add (a: byte, b: byte) -> byte
// ... etc for all operations
```

### Division and Modulo

Division has a special overflow case: `int.min / -1` overflows because the result would be `int.max + 1`.

```ori
int.min div -1  // PANIC: integer overflow
int.min % -1    // PANIC: integer overflow
```

Division by zero is a separate error:

```ori
10 div 0  // PANIC: division by zero
```

---

## Examples

### Safe Counter with Saturation

```ori
type Counter = { value: int }

use std.math { saturating_add }

@increment (c: Counter) -> Counter = run(
    Counter { value: saturating_add(a: c.value, b: 1) }
)

// Counter.value stays at int.max instead of overflowing
```

### Hash Function with Wrapping

```ori
use std.math { wrapping_add, wrapping_mul }

@hash (bytes: [byte]) -> int = run(
    fold(
        over: bytes,
        init: 0,
        op: (acc, b) -> wrapping_add(
            a: wrapping_mul(a: acc, b: 31),
            b: int(b)
        )
    )
)
```

### Checked Arithmetic for User Input

```ori
use std.math { checked_add }

@add_scores (a: int, b: int) -> Result<int, str> = run(
    match(checked_add(a: a, b: b),
        Some(result) -> Ok(result),
        None -> Err("score overflow")
    )
)
```

### Compile-Time Constants

Overflow in compile-time constant expressions is a compilation error:

```ori
$big = int.max + 1  // ERROR: constant overflow
```

---

## Design Rationale

### Why Panic by Default?

1. **Catches bugs early**: Most overflow is unintentional
2. **No silent corruption**: Bad values don't propagate
3. **Consistent behavior**: Same in debug and release builds
4. **Matches Ori philosophy**: Safe by default

### Why Not Wrap by Default?

- Wrapping is almost never the intended behavior
- Silent wrapping hides bugs
- When wrapping is needed, it should be explicit

### Why Functions Instead of Operators?

Alternatives considered:

| Approach | Example | Problem |
|----------|---------|---------|
| Operators | `a +% b` (wrapping) | Adds cryptic syntax |
| Methods | `a.wrapping_add(b)` | Integers aren't struct types |
| **Functions** | `wrapping_add(a: x, b: y)` | Clear, explicit, no new syntax |

Functions are consistent with Ori's pattern-based approach.

---

## Type Bounds

The following constants are available:

```ori
int.min   // -9223372036854775808
int.max   //  9223372036854775807

byte.min  // 0
byte.max  // 255
```

---

## Implementation Notes

### Compiler Changes

1. All arithmetic operations emit overflow-checking instructions
2. Native overflow detection where available (most architectures have this)
3. Constant folding must check for overflow at compile time

### Standard Library

Add to `std.math`:

- 9 functions for `int`: `{saturating,wrapping,checked}_{add,sub,mul}`
- 9 functions for `byte`: same set
- Potentially: `div` variants (though `div` by zero is the main concern)

### Performance

Overflow checking has minimal overhead on modern CPUs:

- Addition/subtraction: check overflow flag (1 instruction)
- Multiplication: slightly more expensive but still fast

For hot loops where this matters, use explicit `wrapping_*` functions.

---

## Comparison to Rust

Ori's approach is similar to Rust's, but simpler:

| Aspect | Rust | Ori |
|--------|------|-------|
| Debug behavior | Panic | Panic |
| Release behavior | Wrap | Panic |
| Explicit wrapping | `wrapping_add()` | `wrapping_add()` |
| Explicit saturation | `saturating_add()` | `saturating_add()` |
| Explicit checked | `checked_add()` → `Option` | `checked_add()` → `Option` |

Ori doesn't change behavior between debug and release builds — consistency is valued over performance optimization.

---

## Summary

Integer overflow in Ori:

1. **Panics by default** — Safe, catches bugs, no silent corruption
2. **Explicit alternatives** — `saturating_*`, `wrapping_*`, `checked_*` in `std.math`
3. **No undefined behavior** — Every operation has defined semantics
4. **Consistent** — Same behavior in debug and release builds

This aligns with Ori's philosophy of being explicit, safe, and predictable.
