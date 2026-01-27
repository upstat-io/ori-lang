# std.math

Mathematical functions and constants.

```ori
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

```ori
use std.math { pi, e, inf, nan }

pi      // 3.141592653589793
e       // 2.718281828459045
inf     // Positive infinity
nan     // Not a number
```

---

## Basic Functions

### @abs

```ori
@abs (x: int) -> int
@abs (x: float) -> float
```

Absolute value.

```ori
use std.math { abs }

abs(-5)     // 5
abs(-3.14)  // 3.14
```

---

### @min

```ori
@min<T: Comparable> (a: T, b: T) -> T
@min<T: Comparable> (values: [T]) -> Option<T>
```

Minimum value.

```ori
use std.math { min }

min(3, 7)           // 3
min([5, 2, 8, 1])   // Some(1)
```

---

### @max

```ori
@max<T: Comparable> (a: T, b: T) -> T
@max<T: Comparable> (values: [T]) -> Option<T>
```

Maximum value.

```ori
use std.math { max }

max(3, 7)           // 7
max([5, 2, 8, 1])   // Some(8)
```

---

### @clamp

```ori
@clamp<T: Comparable> (value: T, min: T, max: T) -> T
```

Clamps value to range.

```ori
use std.math { clamp }

clamp(5, 0, 10)   // 5
clamp(-5, 0, 10)  // 0
clamp(15, 0, 10)  // 10
```

---

## Rounding

### @floor

```ori
@floor (x: float) -> float
```

Rounds down to nearest integer.

```ori
use std.math { floor }

floor(3.7)   // 3.0
floor(-3.7)  // -4.0
```

---

### @ceil

```ori
@ceil (x: float) -> float
```

Rounds up to nearest integer.

```ori
use std.math { ceil }

ceil(3.2)   // 4.0
ceil(-3.2)  // -3.0
```

---

### @round

```ori
@round (x: float) -> float
@round (x: float, decimals: int) -> float
```

Rounds to nearest integer or decimal places.

```ori
use std.math { round }

round(3.5)       // 4.0
round(3.14159, 2)  // 3.14
```

---

### @trunc

```ori
@trunc (x: float) -> float
```

Truncates toward zero.

```ori
use std.math { trunc }

trunc(3.7)   // 3.0
trunc(-3.7)  // -3.0
```

---

## Powers and Roots

### @pow

```ori
@pow (base: float, exp: float) -> float
```

Raises base to power.

```ori
use std.math { pow }

pow(2.0, 10.0)  // 1024.0
pow(27.0, 1.0/3.0)  // 3.0 (cube root)
```

---

### @sqrt

```ori
@sqrt (x: float) -> float
```

Square root.

```ori
use std.math { sqrt }

sqrt(16.0)  // 4.0
sqrt(2.0)   // 1.4142135623730951
```

---

### @cbrt

```ori
@cbrt (x: float) -> float
```

Cube root.

---

## Exponential and Logarithmic

### @exp

```ori
@exp (x: float) -> float
```

e raised to power x.

---

### @ln

```ori
@ln (x: float) -> float
```

Natural logarithm (base e).

---

### @log

```ori
@log (x: float, base: float) -> float
```

Logarithm with given base.

---

### @log10

```ori
@log10 (x: float) -> float
```

Base-10 logarithm.

---

### @log2

```ori
@log2 (x: float) -> float
```

Base-2 logarithm.

---

## Trigonometric

### @sin / @cos / @tan

```ori
@sin (x: float) -> float  // radians
@cos (x: float) -> float
@tan (x: float) -> float
```

```ori
use std.math { sin, cos, pi }

sin(pi / 2.0)  // 1.0
cos(pi)        // -1.0
```

---

### @asin / @acos / @atan

```ori
@asin (x: float) -> float  // returns radians
@acos (x: float) -> float
@atan (x: float) -> float
```

Inverse trigonometric functions.

---

### @atan2

```ori
@atan2 (y: float, x: float) -> float
```

Two-argument arctangent.

---

### @sinh / @cosh / @tanh

Hyperbolic functions.

---

## Utility

### @is_nan

```ori
@is_nan (x: float) -> bool
```

Checks if value is NaN.

---

### @is_inf

```ori
@is_inf (x: float) -> bool
```

Checks if value is infinite.

---

### @is_finite

```ori
@is_finite (x: float) -> bool
```

Checks if value is finite (not NaN or infinity).

---

### @sign

```ori
@sign (x: int) -> int
@sign (x: float) -> float
```

Returns -1, 0, or 1.

```ori
use std.math { sign }

sign(-5)   // -1
sign(0)    // 0
sign(10)   // 1
```

---

## Examples

### Distance calculation

```ori
use std.math { sqrt, pow }

@distance (x1: float, y1: float, x2: float, y2: float) -> float =
    sqrt(pow(x2 - x1, 2.0) + pow(y2 - y1, 2.0))
```

### Degrees to radians

```ori
use std.math { pi }

@to_radians (degrees: float) -> float = degrees * pi / 180.0
@to_degrees (radians: float) -> float = radians * 180.0 / pi
```

---

## See Also

- [std.math.rand](rand.md) — Random numbers
- [Primitive Types](../prelude.md) — int, float
