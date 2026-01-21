# std.math

Mathematical functions and constants.

```sigil
use std.math { sqrt, sin, cos, pi, abs, min, max }
use std.math.rand { random, random_int }
```

**No capability required** (except `std.math.rand`)

---

## Overview

The `std.math` module provides:

- Trigonometric functions
- Exponential and logarithmic functions
- Rounding and absolute value
- Constants (pi, e)
- Random numbers (in `std.math.rand`)

---

## Submodules

| Module | Description |
|--------|-------------|
| [std.math.rand](rand.md) | Random number generation |

---

## Constants

```sigil
use std.math { pi, e, inf, nan }

pi      // 3.141592653589793
e       // 2.718281828459045
inf     // Positive infinity
nan     // Not a number
```

---

## Basic Functions

### @abs

```sigil
@abs (x: int) -> int
@abs (x: float) -> float
```

Absolute value.

```sigil
use std.math { abs }

abs(-5)     // 5
abs(-3.14)  // 3.14
```

---

### @min

```sigil
@min<T: Comparable> (a: T, b: T) -> T
@min<T: Comparable> (values: [T]) -> Option<T>
```

Minimum value.

```sigil
use std.math { min }

min(3, 7)           // 3
min([5, 2, 8, 1])   // Some(1)
```

---

### @max

```sigil
@max<T: Comparable> (a: T, b: T) -> T
@max<T: Comparable> (values: [T]) -> Option<T>
```

Maximum value.

```sigil
use std.math { max }

max(3, 7)           // 7
max([5, 2, 8, 1])   // Some(8)
```

---

### @clamp

```sigil
@clamp<T: Comparable> (value: T, min: T, max: T) -> T
```

Clamps value to range.

```sigil
use std.math { clamp }

clamp(5, 0, 10)   // 5
clamp(-5, 0, 10)  // 0
clamp(15, 0, 10)  // 10
```

---

## Rounding

### @floor

```sigil
@floor (x: float) -> float
```

Rounds down to nearest integer.

```sigil
use std.math { floor }

floor(3.7)   // 3.0
floor(-3.7)  // -4.0
```

---

### @ceil

```sigil
@ceil (x: float) -> float
```

Rounds up to nearest integer.

```sigil
use std.math { ceil }

ceil(3.2)   // 4.0
ceil(-3.2)  // -3.0
```

---

### @round

```sigil
@round (x: float) -> float
@round (x: float, decimals: int) -> float
```

Rounds to nearest integer or decimal places.

```sigil
use std.math { round }

round(3.5)       // 4.0
round(3.14159, 2)  // 3.14
```

---

### @trunc

```sigil
@trunc (x: float) -> float
```

Truncates toward zero.

```sigil
use std.math { trunc }

trunc(3.7)   // 3.0
trunc(-3.7)  // -3.0
```

---

## Powers and Roots

### @pow

```sigil
@pow (base: float, exp: float) -> float
```

Raises base to power.

```sigil
use std.math { pow }

pow(2.0, 10.0)  // 1024.0
pow(27.0, 1.0/3.0)  // 3.0 (cube root)
```

---

### @sqrt

```sigil
@sqrt (x: float) -> float
```

Square root.

```sigil
use std.math { sqrt }

sqrt(16.0)  // 4.0
sqrt(2.0)   // 1.4142135623730951
```

---

### @cbrt

```sigil
@cbrt (x: float) -> float
```

Cube root.

---

## Exponential and Logarithmic

### @exp

```sigil
@exp (x: float) -> float
```

e raised to power x.

---

### @ln

```sigil
@ln (x: float) -> float
```

Natural logarithm (base e).

---

### @log

```sigil
@log (x: float, base: float) -> float
```

Logarithm with given base.

---

### @log10

```sigil
@log10 (x: float) -> float
```

Base-10 logarithm.

---

### @log2

```sigil
@log2 (x: float) -> float
```

Base-2 logarithm.

---

## Trigonometric

### @sin / @cos / @tan

```sigil
@sin (x: float) -> float  // radians
@cos (x: float) -> float
@tan (x: float) -> float
```

```sigil
use std.math { sin, cos, pi }

sin(pi / 2.0)  // 1.0
cos(pi)        // -1.0
```

---

### @asin / @acos / @atan

```sigil
@asin (x: float) -> float  // returns radians
@acos (x: float) -> float
@atan (x: float) -> float
```

Inverse trigonometric functions.

---

### @atan2

```sigil
@atan2 (y: float, x: float) -> float
```

Two-argument arctangent.

---

### @sinh / @cosh / @tanh

Hyperbolic functions.

---

## Utility

### @is_nan

```sigil
@is_nan (x: float) -> bool
```

Checks if value is NaN.

---

### @is_inf

```sigil
@is_inf (x: float) -> bool
```

Checks if value is infinite.

---

### @is_finite

```sigil
@is_finite (x: float) -> bool
```

Checks if value is finite (not NaN or infinity).

---

### @sign

```sigil
@sign (x: int) -> int
@sign (x: float) -> float
```

Returns -1, 0, or 1.

```sigil
use std.math { sign }

sign(-5)   // -1
sign(0)    // 0
sign(10)   // 1
```

---

## Examples

### Distance calculation

```sigil
use std.math { sqrt, pow }

@distance (x1: float, y1: float, x2: float, y2: float) -> float =
    sqrt(pow(x2 - x1, 2.0) + pow(y2 - y1, 2.0))
```

### Degrees to radians

```sigil
use std.math { pi }

@to_radians (degrees: float) -> float = degrees * pi / 180.0
@to_degrees (radians: float) -> float = radians * 180.0 / pi
```

---

## See Also

- [std.math.rand](rand.md) — Random numbers
- [Primitive Types](../prelude.md) — int, float
