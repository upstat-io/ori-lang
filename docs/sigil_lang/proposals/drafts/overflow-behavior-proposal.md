# Proposal: Integer Overflow Behavior

**Status:** Draft
**Author:** Eric
**Created:** 2026-01-22

---

## Summary

Define integer overflow behavior in Sigil: panic by default for safety, with explicit stdlib functions for alternative behaviors (saturation, wrapping).

```sigil
// Default: overflow panics
let x: int = int.max + 1  // PANIC: integer overflow

// Explicit alternatives via std.math
use std.math { saturating_add, wrapping_add }

saturating_add(int.max, 1)  // Returns int.max (clamped)
wrapping_add(int.max, 1)    // Returns int.min (wrapped)
```

---

## Motivation

### The Problem

Integer overflow is a common source of bugs:

- **Security vulnerabilities**: Buffer overflows, integer overflow exploits
- **Silent corruption**: Wrong values propagate through calculations
- **Difficult debugging**: Bugs appear far from the actual overflow

Sigil has fixed-width integer types:

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
| C++26 | — | `std::saturate_add()`, etc. |

### The Sigil Way

Sigil's core principles:

1. **Explicit over implicit** — No silent behavior changes
2. **Safe by default** — Bugs should be caught, not hidden
3. **No undefined behavior** — Every operation has defined semantics

This leads to: **Panic on overflow by default**, with explicit functions when other behavior is needed.

---

## Design

### Default Behavior: Panic

All arithmetic operations on `int` and `byte` panic on overflow:

```sigil
// Overflow panics
int.max + 1        // PANIC: integer overflow
int.min - 1        // PANIC: integer overflow
int.max * 2        // PANIC: integer overflow

byte.max + byte(1) // PANIC: integer overflow
```

**Panic message format:**

```
PANIC at src/calc.si:42:15
  integer overflow in addition
    left:  9223372036854775807 (int.max)
    right: 1
    operation: +
```

### Explicit Alternatives: std.math Functions

For cases where overflow behavior is intentional, use explicit functions:

```sigil
use std.math {
    saturating_add, saturating_sub, saturating_mul,
    wrapping_add, wrapping_sub, wrapping_mul,
    checked_add, checked_sub, checked_mul
}
```

#### Saturating Arithmetic

Clamps the result to the type's bounds:

```sigil
saturating_add(int.max, 1)   // int.max
saturating_add(int.max, 100) // int.max
saturating_sub(int.min, 1)   // int.min
saturating_mul(int.max, 2)   // int.max

saturating_add(byte.max, byte(1))  // byte.max (255)
```

#### Wrapping Arithmetic

Wraps around on overflow (modular arithmetic):

```sigil
wrapping_add(int.max, 1)     // int.min
wrapping_sub(int.min, 1)     // int.max
wrapping_mul(int.max, 2)     // -2

wrapping_add(byte.max, byte(1))  // byte(0)
```

#### Checked Arithmetic

Returns `Option<T>` — `None` on overflow:

```sigil
checked_add(int.max, 1)      // None
checked_add(100, 200)        // Some(300)
checked_sub(int.min, 1)      // None
checked_mul(int.max, 2)      // None
```

### Type Signatures

```sigil
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

```sigil
int.min div -1  // PANIC: integer overflow
int.min % -1    // PANIC: integer overflow (implementation may vary)
```

Division by zero is a separate error:

```sigil
10 div 0  // PANIC: division by zero
```

---

## Examples

### Safe Counter with Saturation

```sigil
type Counter = { value: int }

use std.math { saturating_add }

@increment (c: Counter) -> Counter = run(
    Counter { value: saturating_add(c.value, 1) }
)

// Counter.value stays at int.max instead of overflowing
```

### Hash Function with Wrapping

```sigil
use std.math { wrapping_add, wrapping_mul }

@hash (bytes: [byte]) -> int = run(
    fold(
        .over: bytes,
        .init: 0,
        .op: (acc, b) -> wrapping_add(
            wrapping_mul(acc, 31),
            int(b)
        )
    )
)
```

### Checked Arithmetic for User Input

```sigil
use std.math { checked_add }

@add_scores (a: int, b: int) -> Result<int, str> = run(
    match(checked_add(a, b),
        Some(result) -> Ok(result),
        None -> Err("score overflow")
    )
)
```

### Compile-Time Constants

Overflow in compile-time constant expressions is a compilation error:

```sigil
$big = int.max + 1  // ERROR: constant overflow
```

---

## Design Rationale

### Why Panic by Default?

1. **Catches bugs early**: Most overflow is unintentional
2. **No silent corruption**: Bad values don't propagate
3. **Consistent behavior**: Same in debug and release builds
4. **Matches Sigil philosophy**: Safe by default

### Why Not Wrap by Default?

- Wrapping is almost never the intended behavior
- Silent wrapping hides bugs
- When wrapping is needed, it should be explicit

### Why Not Arbitrary Precision?

- Performance cost (heap allocation, variable-size math)
- Most code works fine with 64-bit integers
- `BigInt` can be a stdlib type for when it's needed

### Why Functions Instead of Operators?

Alternatives considered:

| Approach | Example | Problem |
|----------|---------|---------|
| Operators | `a +% b` (wrapping) | Adds cryptic syntax |
| Methods | `a.wrapping_add(b)` | Integers aren't struct types |
| **Functions** | `wrapping_add(a, b)` | Clear, explicit, no new syntax |

Functions are consistent with Sigil's pattern-based approach.

### Why Include Checked?

Checked arithmetic returns `Option`, integrating with Sigil's error handling:

```sigil
let result = checked_add(a, b) ?? default_value
```

This is more composable than panics for cases where overflow is expected.

---

## Type Bounds

The following constants are available:

```sigil
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

Sigil's approach is similar to Rust's, but simpler:

| Aspect | Rust | Sigil |
|--------|------|-------|
| Debug behavior | Panic | Panic |
| Release behavior | Wrap | Panic |
| Explicit wrapping | `wrapping_add()` | `wrapping_add()` |
| Explicit saturation | `saturating_add()` | `saturating_add()` |
| Explicit checked | `checked_add()` → `Option` | `checked_add()` → `Option` |

Sigil doesn't change behavior between debug and release builds — consistency is valued over performance optimization.

---

## Summary

Integer overflow in Sigil:

1. **Panics by default** — Safe, catches bugs, no silent corruption
2. **Explicit alternatives** — `saturating_*`, `wrapping_*`, `checked_*` in `std.math`
3. **No undefined behavior** — Every operation has defined semantics
4. **Consistent** — Same behavior in debug and release builds

This aligns with Sigil's philosophy of being explicit, safe, and predictable.
