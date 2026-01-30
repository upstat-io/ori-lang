# Proposal: std.math API Design

**Status:** Draft
**Created:** 2026-01-30
**Affects:** Standard library
**Depends on:** C FFI proposal

## Summary

A comprehensive mathematics module providing constants, elementary functions, trigonometry, and statistics for numeric computation in Ori. Core mathematical functions are backed by **libm** (the standard C math library) via FFI.

## Motivation

- **Correctness**: libm implementations are battle-tested for IEEE 754 edge cases, special values, and precision
- **Performance**: Hardware-optimized implementations (often using SIMD intrinsics)
- **Consistency**: Behavior matches C/C++/Rust and other languages using libm
- **Coverage**: Full set of mathematical functions without reimplementation effort

## FFI Implementation

### Backend: libm

The standard C math library (`libm`) provides all core mathematical functions. It's available on every platform Ori targets.

### External Declarations

```ori
// std/math/ffi.ori (internal)
extern "c" from "m" {
    // Basic operations
    @_fabs (x: float) -> float as "fabs"
    @_fmod (x: float, y: float) -> float as "fmod"
    @_remainder (x: float, y: float) -> float as "remainder"
    @_fmax (x: float, y: float) -> float as "fmax"
    @_fmin (x: float, y: float) -> float as "fmin"
    @_fdim (x: float, y: float) -> float as "fdim"

    // Rounding
    @_floor (x: float) -> float as "floor"
    @_ceil (x: float) -> float as "ceil"
    @_trunc (x: float) -> float as "trunc"
    @_round (x: float) -> float as "round"
    @_nearbyint (x: float) -> float as "nearbyint"
    @_rint (x: float) -> float as "rint"

    // Powers and roots
    @_sqrt (x: float) -> float as "sqrt"
    @_cbrt (x: float) -> float as "cbrt"
    @_pow (x: float, y: float) -> float as "pow"
    @_hypot (x: float, y: float) -> float as "hypot"

    // Exponential and logarithmic
    @_exp (x: float) -> float as "exp"
    @_exp2 (x: float) -> float as "exp2"
    @_expm1 (x: float) -> float as "expm1"
    @_log (x: float) -> float as "log"
    @_log2 (x: float) -> float as "log2"
    @_log10 (x: float) -> float as "log10"
    @_log1p (x: float) -> float as "log1p"
    @_logb (x: float) -> float as "logb"

    // Trigonometric
    @_sin (x: float) -> float as "sin"
    @_cos (x: float) -> float as "cos"
    @_tan (x: float) -> float as "tan"
    @_asin (x: float) -> float as "asin"
    @_acos (x: float) -> float as "acos"
    @_atan (x: float) -> float as "atan"
    @_atan2 (y: float, x: float) -> float as "atan2"

    // Hyperbolic
    @_sinh (x: float) -> float as "sinh"
    @_cosh (x: float) -> float as "cosh"
    @_tanh (x: float) -> float as "tanh"
    @_asinh (x: float) -> float as "asinh"
    @_acosh (x: float) -> float as "acosh"
    @_atanh (x: float) -> float as "atanh"

    // Error and gamma functions
    @_erf (x: float) -> float as "erf"
    @_erfc (x: float) -> float as "erfc"
    @_tgamma (x: float) -> float as "tgamma"
    @_lgamma (x: float) -> float as "lgamma"

    // Float classification
    @_isnan (x: float) -> int as "isnan"
    @_isinf (x: float) -> int as "isinf"
    @_isfinite (x: float) -> int as "isfinite"
    @_isnormal (x: float) -> int as "isnormal"
    @_signbit (x: float) -> int as "signbit"
    @_copysign (x: float, y: float) -> float as "copysign"

    // Decomposition
    @_frexp (x: float, exp: CPtr) -> float as "frexp"
    @_ldexp (x: float, exp: int) -> float as "ldexp"
    @_modf (x: float, iptr: CPtr) -> float as "modf"
    @_scalbn (x: float, n: int) -> float as "scalbn"
}
```

---

## Mathematical Constants

Pure Ori constants (no FFI needed):

```ori
// std/math/constants.ori
pub let $pi: float = 3.141592653589793
pub let $tau: float = 6.283185307179586
pub let $e: float = 2.718281828459045

pub let $sqrt_2: float = 1.4142135623730951
pub let $sqrt_3: float = 1.7320508075688772
pub let $ln_2: float = 0.6931471805599453
pub let $ln_10: float = 2.302585092994046
pub let $log2_e: float = 1.4426950408889634
pub let $log10_e: float = 0.4342944819032518

pub let $int_max: int = 9223372036854775807
pub let $int_min: int = -9223372036854775808
pub let $float_max: float = 1.7976931348623157e308
pub let $float_min: float = 2.2250738585072014e-308
pub let $float_epsilon: float = 2.220446049250313e-16
pub let $infinity: float = 1.0 / 0.0
pub let $neg_infinity: float = -1.0 / 0.0
pub let $nan: float = 0.0 / 0.0
```

---

## Error Type

```ori
pub type MathError =
    | DomainError(message: str)
    | RangeError(message: str)
    | DivisionByZero
    | Undefined(message: str)
```

---

## Basic Arithmetic Functions

### abs

```ori
// std/math/basic.ori
use "./ffi" { _fabs }

pub @abs (value: int) -> int =
    if value < 0 then -value else value

pub @abs (value: float) -> float =
    _fabs(x: value)
```

### sign

```ori
pub @sign (value: int) -> int =
    if value < 0 then -1
    else if value > 0 then 1
    else 0

pub @sign (value: float) -> int =
    if value < 0.0 then -1
    else if value > 0.0 then 1
    else 0
```

### clamp

```ori
pub @clamp (value: int, min: int, max: int) -> int =
    if value < min then min
    else if value > max then max
    else value

pub @clamp (value: float, min: float, max: float) -> float =
    if value < min then min
    else if value > max then max
    else value
```

### gcd, lcm (Pure Ori)

```ori
pub @gcd (a: int, b: int) -> int =
    run(
        let a = abs(value: a),
        let b = abs(value: b),
        if b == 0 then a else gcd(a: b, b: a % b)
    )

pub @lcm (a: int, b: int) -> int =
    if a == 0 || b == 0 then 0
    else abs(value: a) / gcd(a: a, b: b) * abs(value: b)
```

### factorial (Pure Ori)

```ori
pub @factorial (n: int) -> Result<int, MathError> =
    if n < 0 then
        Err(MathError.DomainError(message: "factorial undefined for negative integers"))
    else if n > 20 then
        Err(MathError.RangeError(message: "factorial overflow for n > 20"))
    else
        Ok(factorial_impl(n: n))

@factorial_impl (n: int) -> int =
    if n <= 1 then 1 else n * factorial_impl(n: n - 1)
```

---

## Rounding Functions (FFI)

```ori
// std/math/rounding.ori
use "./ffi" { _floor, _ceil, _trunc, _round }

pub @floor (value: float) -> float = _floor(x: value)
pub @ceil (value: float) -> float = _ceil(x: value)
pub @trunc (value: float) -> float = _trunc(x: value)
pub @round (value: float) -> float = _round(x: value)

// Pure Ori
pub @round_to (value: float, places: int) -> float =
    run(
        let multiplier = pow(base: 10.0, exp: places as float),
        round(value: value * multiplier) / multiplier
    )

pub @fract (value: float) -> float =
    value - trunc(value: value)
```

---

## Power and Root Functions (FFI)

```ori
// std/math/power.ori
use "./ffi" { _sqrt, _cbrt, _pow, _hypot, _exp, _exp2, _expm1 }

pub @pow (base: int, exp: int) -> Result<int, MathError> =
    if exp < 0 then
        Err(MathError.DomainError(message: "negative exponent for integer pow"))
    else
        Ok(int_pow(base: base, exp: exp))

@int_pow (base: int, exp: int) -> int =
    if exp == 0 then 1
    else if exp == 1 then base
    else run(
        let half = int_pow(base: base, exp: exp / 2),
        if exp % 2 == 0 then half * half
        else half * half * base
    )

pub @pow (base: float, exp: float) -> float =
    _pow(x: base, y: exp)

pub @sqrt (value: float) -> Result<float, MathError> =
    if value < 0.0 then
        Err(MathError.DomainError(message: "sqrt undefined for negative numbers"))
    else
        Ok(_sqrt(x: value))

pub @cbrt (value: float) -> float =
    _cbrt(x: value)

pub @hypot (x: float, y: float) -> float =
    _hypot(x: x, y: y)

pub @exp (value: float) -> float = _exp(x: value)
pub @exp2 (value: float) -> float = _exp2(x: value)
pub @expm1 (value: float) -> float = _expm1(x: value)
```

---

## Logarithmic Functions (FFI)

```ori
// std/math/log.ori
use "./ffi" { _log, _log2, _log10, _log1p }

pub @log (value: float) -> Result<float, MathError> =
    if value <= 0.0 then
        Err(MathError.DomainError(message: "log undefined for non-positive numbers"))
    else
        Ok(_log(x: value))

pub @log2 (value: float) -> Result<float, MathError> =
    if value <= 0.0 then
        Err(MathError.DomainError(message: "log2 undefined for non-positive numbers"))
    else
        Ok(_log2(x: value))

pub @log10 (value: float) -> Result<float, MathError> =
    if value <= 0.0 then
        Err(MathError.DomainError(message: "log10 undefined for non-positive numbers"))
    else
        Ok(_log10(x: value))

pub @log_base (value: float, base: float) -> Result<float, MathError> =
    if value <= 0.0 then
        Err(MathError.DomainError(message: "log undefined for non-positive numbers"))
    else if base <= 0.0 || base == 1.0 then
        Err(MathError.DomainError(message: "log base must be positive and not 1"))
    else
        Ok(_log(x: value) / _log(x: base))

pub @log1p (value: float) -> Result<float, MathError> =
    if value <= -1.0 then
        Err(MathError.DomainError(message: "log1p undefined for value <= -1"))
    else
        Ok(_log1p(x: value))
```

---

## Trigonometric Functions (FFI)

```ori
// std/math/trig.ori
use "./ffi" { _sin, _cos, _tan, _asin, _acos, _atan, _atan2 }
use "./constants" { $pi }

pub @sin (angle: float) -> float = _sin(x: angle)
pub @cos (angle: float) -> float = _cos(x: angle)
pub @tan (angle: float) -> float = _tan(x: angle)

pub @asin (value: float) -> Result<float, MathError> =
    if value < -1.0 || value > 1.0 then
        Err(MathError.DomainError(message: "asin domain is [-1, 1]"))
    else
        Ok(_asin(x: value))

pub @acos (value: float) -> Result<float, MathError> =
    if value < -1.0 || value > 1.0 then
        Err(MathError.DomainError(message: "acos domain is [-1, 1]"))
    else
        Ok(_acos(x: value))

pub @atan (value: float) -> float = _atan(x: value)
pub @atan2 (y: float, x: float) -> float = _atan2(y: y, x: x)

pub @sincos (angle: float) -> (float, float) =
    (_sin(x: angle), _cos(x: angle))

pub @to_radians (degrees: float) -> float =
    degrees * $pi / 180.0

pub @to_degrees (radians: float) -> float =
    radians * 180.0 / $pi
```

---

## Hyperbolic Functions (FFI)

```ori
// std/math/hyperbolic.ori
use "./ffi" { _sinh, _cosh, _tanh, _asinh, _acosh, _atanh }

pub @sinh (value: float) -> float = _sinh(x: value)
pub @cosh (value: float) -> float = _cosh(x: value)
pub @tanh (value: float) -> float = _tanh(x: value)

pub @asinh (value: float) -> float = _asinh(x: value)

pub @acosh (value: float) -> Result<float, MathError> =
    if value < 1.0 then
        Err(MathError.DomainError(message: "acosh domain is [1, ∞)"))
    else
        Ok(_acosh(x: value))

pub @atanh (value: float) -> Result<float, MathError> =
    if value <= -1.0 || value >= 1.0 then
        Err(MathError.DomainError(message: "atanh domain is (-1, 1)"))
    else
        Ok(_atanh(x: value))
```

---

## Float Inspection Functions (FFI)

```ori
// std/math/float.ori
use "./ffi" { _isnan, _isinf, _isfinite, _isnormal, _copysign, _signbit }

pub @is_nan (value: float) -> bool = _isnan(x: value) != 0
pub @is_infinite (value: float) -> bool = _isinf(x: value) != 0
pub @is_finite (value: float) -> bool = _isfinite(x: value) != 0
pub @is_normal (value: float) -> bool = _isnormal(x: value) != 0
pub @copysign (value: float, sign_source: float) -> float = _copysign(x: value, y: sign_source)
```

---

## Statistical Functions (Pure Ori)

These are implemented in pure Ori as they don't have libm equivalents:

```ori
// std/math/stats.ori

pub @sum (values: [int]) -> int =
    values.fold(init: 0, f: (acc, x) -> acc + x)

pub @sum (values: [float]) -> float =
    values.fold(init: 0.0, f: (acc, x) -> acc + x)

pub @product (values: [int]) -> int =
    values.fold(init: 1, f: (acc, x) -> acc * x)

pub @product (values: [float]) -> float =
    values.fold(init: 1.0, f: (acc, x) -> acc * x)

pub @mean (values: [float]) -> Option<float> =
    if is_empty(collection: values) then None
    else Some(sum(values: values) / len(collection: values) as float)

pub @median (values: [float]) -> Option<float> =
    if is_empty(collection: values) then None
    else run(
        let sorted = values.sorted(),
        let n = len(collection: sorted),
        if n % 2 == 1 then
            Some(sorted[n / 2])
        else
            Some((sorted[n / 2 - 1] + sorted[n / 2]) / 2.0)
    )

pub @variance (values: [float]) -> Option<float> =
    run(
        let avg = mean(values: values)?,
        let n = len(collection: values) as float,
        let sum_sq = values.fold(init: 0.0, f: (acc, x) -> acc + (x - avg) * (x - avg)),
        Some(sum_sq / n)
    )

pub @sample_variance (values: [float]) -> Option<float> =
    if len(collection: values) < 2 then None
    else run(
        let avg = mean(values: values)?,
        let n = len(collection: values) as float,
        let sum_sq = values.fold(init: 0.0, f: (acc, x) -> acc + (x - avg) * (x - avg)),
        Some(sum_sq / (n - 1.0))
    )

pub @std_dev (values: [float]) -> Option<float> =
    variance(values: values).map(v -> sqrt(value: v).unwrap_or(default: 0.0))

pub @sample_std_dev (values: [float]) -> Option<float> =
    sample_variance(values: values).map(v -> sqrt(value: v).unwrap_or(default: 0.0))

pub @min_max (values: [int]) -> Option<(int, int)> =
    if is_empty(collection: values) then None
    else run(
        let min = values[0],
        let max = values[0],
        for v in values do run(
            if v < min then min = v,
            if v > max then max = v
        ),
        Some((min, max))
    )

pub @min_max (values: [float]) -> Option<(float, float)> =
    if is_empty(collection: values) then None
    else run(
        let min = values[0],
        let max = values[0],
        for v in values do run(
            if v < min then min = v,
            if v > max then max = v
        ),
        Some((min, max))
    )
```

---

## Linear Interpolation (Pure Ori)

```ori
// std/math/interp.ori

pub @lerp (start: float, end: float, t: float) -> float =
    start + (end - start) * t

pub @inverse_lerp (start: float, end: float, value: float) -> float =
    (value - start) / (end - start)

pub @remap (
    value: float,
    from_start: float,
    from_end: float,
    to_start: float,
    to_end: float
) -> float =
    lerp(
        start: to_start,
        end: to_end,
        t: inverse_lerp(start: from_start, end: from_end, value: value)
    )
```

---

## Float Comparison (Pure Ori)

```ori
// std/math/compare.ori

pub @approx_eq (a: float, b: float) -> bool =
    approx_eq(a: a, b: b, epsilon: $float_epsilon * 100.0)

pub @approx_eq (a: float, b: float, epsilon: float) -> bool =
    abs(value: a - b) <= epsilon

pub @approx_cmp (a: float, b: float, epsilon: float) -> Ordering =
    if approx_eq(a: a, b: b, epsilon: epsilon) then Ordering.Equal
    else if a < b then Ordering.Less
    else Ordering.Greater
```

---

## Combinatorics (Pure Ori)

```ori
// std/math/combinatorics.ori

pub @permutations (n: int, k: int) -> Result<int, MathError> =
    if n < 0 || k < 0 then
        Err(MathError.DomainError(message: "n and k must be non-negative"))
    else if k > n then
        Ok(0)
    else
        run(
            let result = 1,
            for i in 0..k do
                result = result * (n - i),
            Ok(result)
        )

pub @combinations (n: int, k: int) -> Result<int, MathError> =
    if n < 0 || k < 0 then
        Err(MathError.DomainError(message: "n and k must be non-negative"))
    else if k > n then
        Ok(0)
    else
        run(
            // Use the smaller of k and n-k to minimize multiplications
            let k = if k > n - k then n - k else k,
            let result = 1,
            for i in 0..k do
                run(
                    result = result * (n - i),
                    result = result / (i + 1)
                ),
            Ok(result)
        )
```

---

## Random Number Generation

Random number generation uses the `Random` capability or a seeded PRNG:

```ori
// std/math/random.ori (for seeded, deterministic RNG)
// See std.crypto for cryptographically secure random

pub type Rng = {
    state: int,
    increment: int
}

impl Rng {
    // PCG-XSH-RR algorithm (pure Ori)
    pub @new (seed: int) -> Rng =
        run(
            let increment = seed * 2 + 1,
            let state = 0,
            let rng = Rng { state: state, increment: increment },
            let (_, rng) = rng.next_u64(),  // Advance once
            rng
        )

    @next_u64 (self) -> (int, Rng) =
        run(
            let old_state = self.state,
            let new_state = old_state * 6364136223846793005 + self.increment,
            let xorshifted = ((old_state >> 18) ^ old_state) >> 27,
            let rot = old_state >> 59,
            let result = (xorshifted >> rot) | (xorshifted << ((-rot) & 31)),
            (result, Rng { state: new_state, increment: self.increment })
        )

    pub @random (self) -> (float, Rng) =
        run(
            let (bits, rng) = self.next_u64(),
            let value = (bits & 0x1FFFFFFFFFFFFF) as float / 9007199254740992.0,
            (value, rng)
        )

    pub @random_range (self, min: int, max: int) -> (int, Rng) =
        run(
            let (bits, rng) = self.next_u64(),
            let range = max - min,
            let value = min + (abs(value: bits) % range),
            (value, rng)
        )

    pub @random_bool (self) -> (bool, Rng) =
        run(
            let (bits, rng) = self.next_u64(),
            ((bits & 1) == 1, rng)
        )

    pub @shuffle<T> (self, items: [T]) -> ([T], Rng) =
        run(
            let result = items.clone(),
            let rng = self,
            for i in (len(collection: result) - 1)..0 by -1 do
                run(
                    let (j, new_rng) = rng.random_range(min: 0, max: i + 1),
                    rng = new_rng,
                    let temp = result[i],
                    result = [...result[0..i], result[j], ...result[(i + 1)..]],
                    result = [...result[0..j], temp, ...result[(j + 1)..]]
                ),
            (result, rng)
        )

    pub @choice<T> (self, items: [T]) -> (Option<T>, Rng) =
        if is_empty(collection: items) then
            (None, self)
        else
            run(
                let (i, rng) = self.random_range(min: 0, max: len(collection: items)),
                (Some(items[i]), rng)
            )
}
```

---

## Build Configuration

```toml
# ori.toml
[native]
libraries = ["m"]  # libm

# Note: On most systems, libm is automatically linked with libc
# Windows uses UCRT which includes math functions
```

---

## Module Structure

```ori
// std/math/mod.ori
pub use "./constants" {
    $pi, $tau, $e,
    $sqrt_2, $sqrt_3,
    $ln_2, $ln_10, $log2_e, $log10_e,
    $int_max, $int_min,
    $float_max, $float_min, $float_epsilon,
    $infinity, $neg_infinity, $nan
}

pub use "./error" { MathError }

pub use "./basic" { abs, sign, clamp, gcd, lcm, factorial }
pub use "./rounding" { floor, ceil, round, round_to, trunc, fract }
pub use "./power" { pow, sqrt, cbrt, hypot, exp, exp2, expm1 }
pub use "./log" { log, log2, log10, log_base, log1p }
pub use "./trig" { sin, cos, tan, asin, acos, atan, atan2, sincos, to_radians, to_degrees }
pub use "./hyperbolic" { sinh, cosh, tanh, asinh, acosh, atanh }
pub use "./float" { is_nan, is_infinite, is_finite, is_normal, copysign }
pub use "./stats" { sum, product, mean, median, variance, sample_variance, std_dev, sample_std_dev, min_max }
pub use "./interp" { lerp, inverse_lerp, remap }
pub use "./compare" { approx_eq, approx_cmp }
pub use "./combinatorics" { permutations, combinations }
pub use "./random" { Rng }
```

---

## Summary

### FFI-Backed Functions (libm)

| Category | Functions |
|----------|-----------|
| Basic | `abs` (float) |
| Rounding | `floor`, `ceil`, `trunc`, `round` |
| Power | `pow` (float), `sqrt`, `cbrt`, `hypot`, `exp`, `exp2`, `expm1` |
| Log | `log`, `log2`, `log10`, `log1p` |
| Trig | `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2` |
| Hyperbolic | `sinh`, `cosh`, `tanh`, `asinh`, `acosh`, `atanh` |
| Float | `is_nan`, `is_infinite`, `is_finite`, `is_normal`, `copysign` |

### Pure Ori Functions

| Category | Functions |
|----------|-----------|
| Basic | `abs` (int), `sign`, `clamp`, `gcd`, `lcm`, `factorial` |
| Rounding | `round_to`, `fract` |
| Power | `pow` (int) |
| Log | `log_base` |
| Trig | `sincos`, `to_radians`, `to_degrees` |
| Stats | `sum`, `product`, `mean`, `median`, `variance`, `std_dev`, `min_max` |
| Interp | `lerp`, `inverse_lerp`, `remap` |
| Compare | `approx_eq`, `approx_cmp` |
| Combin | `permutations`, `combinations` |
| Random | `Rng` (PCG-based PRNG) |
| Constants | All `$` prefixed constants |

### Design Decisions

1. **FFI for IEEE 754 operations**: Functions that require precise floating-point behavior use libm
2. **Pure Ori for integer/collection operations**: Statistics, combinatorics, and integer math are pure Ori
3. **Result types for domain errors**: Functions that can fail return `Result<T, MathError>`
4. **Immutable RNG**: Random state is threaded through for reproducibility
5. **No cryptographic random here**: Use `std.crypto` for secure random generation
