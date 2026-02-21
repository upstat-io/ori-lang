# Proposal: Intrinsics Capability

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Approved:** 2026-01-30
**Affects:** Compiler, capabilities, low-level operations

---

## Summary

This proposal formalizes the `Intrinsics` capability for low-level, platform-specific operations including SIMD, bit manipulation, and hardware feature detection.

---

## Scope

This proposal covers:
- SIMD operations (arithmetic, comparisons, reductions)
- Bit manipulation operations
- CPU feature detection

**Deferred to separate proposal:**
- Atomic operations (require integration with memory model)
- Memory operations (`prefetch`, `memory_fence`)

---

## Capability Definition

`Intrinsics` is a capability trait providing low-level hardware operations:

```ori
trait Intrinsics {
    // SIMD float operations (128-bit / 4-wide)
    @simd_add_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
    @simd_sub_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
    @simd_mul_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
    @simd_div_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
    @simd_min_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
    @simd_max_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
    @simd_sqrt_f32x4 (a: [float, max 4]) -> [float, max 4]
    @simd_abs_f32x4 (a: [float, max 4]) -> [float, max 4]
    @simd_eq_f32x4 (a: [float, max 4], b: [float, max 4]) -> [bool, max 4]
    @simd_lt_f32x4 (a: [float, max 4], b: [float, max 4]) -> [bool, max 4]
    @simd_gt_f32x4 (a: [float, max 4], b: [float, max 4]) -> [bool, max 4]
    @simd_sum_f32x4 (a: [float, max 4]) -> float  // Horizontal sum

    // SIMD float operations (256-bit / 8-wide, AVX)
    @simd_add_f32x8 (a: [float, max 8], b: [float, max 8]) -> [float, max 8]
    @simd_sub_f32x8 (a: [float, max 8], b: [float, max 8]) -> [float, max 8]
    @simd_mul_f32x8 (a: [float, max 8], b: [float, max 8]) -> [float, max 8]
    @simd_div_f32x8 (a: [float, max 8], b: [float, max 8]) -> [float, max 8]
    @simd_min_f32x8 (a: [float, max 8], b: [float, max 8]) -> [float, max 8]
    @simd_max_f32x8 (a: [float, max 8], b: [float, max 8]) -> [float, max 8]
    @simd_sqrt_f32x8 (a: [float, max 8]) -> [float, max 8]
    @simd_abs_f32x8 (a: [float, max 8]) -> [float, max 8]
    @simd_eq_f32x8 (a: [float, max 8], b: [float, max 8]) -> [bool, max 8]
    @simd_lt_f32x8 (a: [float, max 8], b: [float, max 8]) -> [bool, max 8]
    @simd_gt_f32x8 (a: [float, max 8], b: [float, max 8]) -> [bool, max 8]
    @simd_sum_f32x8 (a: [float, max 8]) -> float

    // SIMD float operations (512-bit / 16-wide, AVX-512)
    @simd_add_f32x16 (a: [float, max 16], b: [float, max 16]) -> [float, max 16]
    @simd_sub_f32x16 (a: [float, max 16], b: [float, max 16]) -> [float, max 16]
    @simd_mul_f32x16 (a: [float, max 16], b: [float, max 16]) -> [float, max 16]
    @simd_div_f32x16 (a: [float, max 16], b: [float, max 16]) -> [float, max 16]
    @simd_min_f32x16 (a: [float, max 16], b: [float, max 16]) -> [float, max 16]
    @simd_max_f32x16 (a: [float, max 16], b: [float, max 16]) -> [float, max 16]
    @simd_sqrt_f32x16 (a: [float, max 16]) -> [float, max 16]
    @simd_abs_f32x16 (a: [float, max 16]) -> [float, max 16]
    @simd_eq_f32x16 (a: [float, max 16], b: [float, max 16]) -> [bool, max 16]
    @simd_lt_f32x16 (a: [float, max 16], b: [float, max 16]) -> [bool, max 16]
    @simd_gt_f32x16 (a: [float, max 16], b: [float, max 16]) -> [bool, max 16]
    @simd_sum_f32x16 (a: [float, max 16]) -> float

    // SIMD 64-bit integer operations (128-bit / 2-wide)
    @simd_add_i64x2 (a: [int, max 2], b: [int, max 2]) -> [int, max 2]
    @simd_sub_i64x2 (a: [int, max 2], b: [int, max 2]) -> [int, max 2]
    @simd_mul_i64x2 (a: [int, max 2], b: [int, max 2]) -> [int, max 2]
    @simd_min_i64x2 (a: [int, max 2], b: [int, max 2]) -> [int, max 2]
    @simd_max_i64x2 (a: [int, max 2], b: [int, max 2]) -> [int, max 2]
    @simd_eq_i64x2 (a: [int, max 2], b: [int, max 2]) -> [bool, max 2]
    @simd_lt_i64x2 (a: [int, max 2], b: [int, max 2]) -> [bool, max 2]
    @simd_gt_i64x2 (a: [int, max 2], b: [int, max 2]) -> [bool, max 2]
    @simd_sum_i64x2 (a: [int, max 2]) -> int

    // SIMD 64-bit integer operations (256-bit / 4-wide, AVX2)
    @simd_add_i64x4 (a: [int, max 4], b: [int, max 4]) -> [int, max 4]
    @simd_sub_i64x4 (a: [int, max 4], b: [int, max 4]) -> [int, max 4]
    @simd_mul_i64x4 (a: [int, max 4], b: [int, max 4]) -> [int, max 4]
    @simd_min_i64x4 (a: [int, max 4], b: [int, max 4]) -> [int, max 4]
    @simd_max_i64x4 (a: [int, max 4], b: [int, max 4]) -> [int, max 4]
    @simd_eq_i64x4 (a: [int, max 4], b: [int, max 4]) -> [bool, max 4]
    @simd_lt_i64x4 (a: [int, max 4], b: [int, max 4]) -> [bool, max 4]
    @simd_gt_i64x4 (a: [int, max 4], b: [int, max 4]) -> [bool, max 4]
    @simd_sum_i64x4 (a: [int, max 4]) -> int

    // Bit operations
    @count_leading_zeros (value: int) -> int
    @count_trailing_zeros (value: int) -> int
    @count_ones (value: int) -> int
    @rotate_left (value: int, amount: int) -> int
    @rotate_right (value: int, amount: int) -> int

    // Hardware queries
    @cpu_has_feature (feature: str) -> bool
}
```

---

## Usage

### Function Declaration

```ori
@fast_dot_product (a: [float], b: [float]) -> float uses Intrinsics =
    // Use SIMD intrinsics for vectorized computation
    ...
```

### Capability Provision

The default `def impl Intrinsics` uses native instructions when available, falling back to scalar emulation when not:

```ori
// Uses default (NativeWithFallback)
@compute () -> float uses Intrinsics =
    Intrinsics.simd_add_f32x4(a: vec1, b: vec2)
```

Override for testing or explicit control:

```ori
with Intrinsics = EmulatedIntrinsics {} in
    fast_operation()  // Always uses scalar fallback
```

### Conditional Use with Feature Detection

```ori
@dot_product (a: [float], b: [float]) -> float uses Intrinsics =
    if Intrinsics.cpu_has_feature(feature: "avx2") then
        avx2_dot_product(a, b)
    else if Intrinsics.cpu_has_feature(feature: "sse4.1") then
        sse4_dot_product(a, b)
    else
        scalar_dot_product(a, b)
```

### Compile-Time Platform Targeting

```ori
#target(arch: "x86_64")
@fast_checksum (data: [byte]) -> int uses Intrinsics =
    Intrinsics.crc32(data: data)

#target(not_arch: "x86_64")
@fast_checksum (data: [byte]) -> int =
    data.fold(initial: 0, op: (acc, b) -> acc ^ (b as int))
```

---

## SIMD Operations

### Vector Types

SIMD operations work on fixed-capacity lists:

- `[float, max 4]` — 128-bit (SSE, NEON)
- `[float, max 8]` — 256-bit (AVX, AVX2)
- `[float, max 16]` — 512-bit (AVX-512)
- `[int, max 2]` — 128-bit i64 (SSE2)
- `[int, max 4]` — 256-bit i64 (AVX2)

The `int` type is 64-bit in Ori, so integer SIMD uses i64 lanes.

### Core Operations

| Category | Float Operations | Int Operations |
|----------|-----------------|----------------|
| Arithmetic | `add`, `sub`, `mul`, `div` | `add`, `sub`, `mul` |
| Comparison | `eq`, `lt`, `gt` | `eq`, `lt`, `gt` |
| Min/Max | `min`, `max` | `min`, `max` |
| Math | `sqrt`, `abs` | — |
| Reduction | `sum` | `sum` |

### Example: SIMD Dot Product

```ori
@simd_dot_4 (a: [float, max 4], b: [float, max 4]) -> float uses Intrinsics = {
    let products = Intrinsics.simd_mul_f32x4(a: a, b: b)
    Intrinsics.simd_sum_f32x4(a: products)
}
```

### Platform Availability

| Target | 128-bit (x4) | 256-bit (x8) | 512-bit (x16) |
|--------|--------------|--------------|---------------|
| x86_64 | SSE (baseline) | AVX/AVX2 | AVX-512 |
| aarch64 | NEON | — | — |
| wasm32 | SIMD128 | — | — |

---

## Bit Manipulation

### Operations

```ori
// Number of set bits (population count)
@count_ones (value: int) -> int uses Intrinsics

// Number of leading zero bits
@count_leading_zeros (value: int) -> int uses Intrinsics

// Number of trailing zero bits
@count_trailing_zeros (value: int) -> int uses Intrinsics

// Bitwise rotation
@rotate_left (value: int, amount: int) -> int uses Intrinsics
@rotate_right (value: int, amount: int) -> int uses Intrinsics
```

### Example

```ori
@is_power_of_two (n: int) -> bool uses Intrinsics =
    n > 0 && Intrinsics.count_ones(value: n) == 1
```

---

## Hardware Feature Detection

### Runtime Detection

```ori
@cpu_has_feature (feature: str) -> bool uses Intrinsics
```

Valid feature strings:

| Platform | Features |
|----------|----------|
| x86_64 | `"sse"`, `"sse2"`, `"sse3"`, `"sse4.1"`, `"sse4.2"`, `"avx"`, `"avx2"`, `"avx512f"` |
| aarch64 | `"neon"` |
| wasm32 | `"simd128"` |

Unknown feature strings cause a panic.

### Example

```ori
@optimized_compute (data: [float]) -> float uses Intrinsics =
    if Intrinsics.cpu_has_feature(feature: "avx2") then
        avx2_compute(data)
    else if Intrinsics.cpu_has_feature(feature: "sse4.1") then
        sse4_compute(data)
    else
        scalar_compute(data)
```

---

## Platform-Specific Behavior

### Auto-Fallback (Default)

The default `def impl Intrinsics` provides `NativeWithFallback`:
- Uses native SIMD instructions when the operation is supported on the current platform
- Falls back to scalar emulation when not supported
- Always works, but may be slower on emulated paths

```ori
// This always works, even on platforms without AVX
Intrinsics.simd_add_f32x8(a, b)  // Uses AVX if available, emulates otherwise
```

### Explicit Control

For performance-critical code, use feature detection to select optimal paths:

```ori
@fast_path (data: [float]) -> float uses Intrinsics =
    if Intrinsics.cpu_has_feature(feature: "avx2") then
        // Known to use native AVX2
        avx2_implementation(data)
    else
        // Known to use scalar
        scalar_implementation(data)
```

### Implementation Providers

| Provider | Behavior |
|----------|----------|
| `NativeWithFallback` | Native when available, scalar fallback (default) |
| `EmulatedIntrinsics` | Always uses scalar operations (for testing) |

---

## Safety Guarantees

### No Undefined Behavior

Unlike C intrinsics, Ori intrinsics:
- Check input sizes at runtime
- Panic on invalid inputs (not UB)
- Don't allow arbitrary memory access

### SIMD Safety

SIMD operations require correctly sized inputs:

```ori
Intrinsics.simd_add_f32x4(a: [1.0, 2.0], b: [...])  // panic: expected 4 elements
```

### Bit Operation Safety

Rotation amounts are taken modulo 64:

```ori
Intrinsics.rotate_left(value: 1, amount: 65)  // Same as amount: 1
```

---

## Error Messages

### Missing Capability

```
error[E1060]: `simd_add_f32x4` requires `Intrinsics` capability
  --> src/main.ori:5:5
   |
 5 |     Intrinsics.simd_add_f32x4(a, b)
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ requires `uses Intrinsics`
   |
   = help: add `uses Intrinsics` to function signature
```

### Unknown Feature

```
error[E1062]: unknown CPU feature `"avx3"`
  --> src/main.ori:5:45
   |
 5 |     if Intrinsics.cpu_has_feature(feature: "avx3") then
   |                                            ^^^^^^ unknown feature
   |
   = note: valid features for x86_64: "sse", "sse2", "sse3", "sse4.1", "sse4.2", "avx", "avx2", "avx512f"
```

### Wrong Vector Size

```
error[E1063]: SIMD operation requires exactly 4 elements
  --> src/main.ori:5:5
   |
 5 |     Intrinsics.simd_add_f32x4(a: [1.0, 2.0], b: b)
   |                                  ^^^^^^^^^^ has 2 elements
```

---

## Spec Changes Required

### Update `14-capabilities.md`

Add `Intrinsics` to standard capabilities table:

```markdown
| Capability | Purpose | Suspends |
|------------|---------|----------|
| `Intrinsics` | Low-level SIMD and bit operations | No |
```

Add section describing available operations and platform behavior.

---

## Implementation

### Phase 6.14: Intrinsics Capability

1. Add `Intrinsics` trait to prelude
2. Implement `def impl Intrinsics` with NativeWithFallback
3. Add `EmulatedIntrinsics` provider
4. Implement SIMD codegen for LLVM backend
5. Add feature detection for x86_64, aarch64, wasm32
6. Add comprehensive tests

### LLVM Backend

SIMD operations map to LLVM vector intrinsics:
- `simd_add_f32x4` → `fadd <4 x float>`
- `count_ones` → `llvm.ctpop.i64`
- `cpu_has_feature` → Runtime CPUID check

---

## Summary

| Category | Operations |
|----------|------------|
| SIMD Float | `add`, `sub`, `mul`, `div`, `min`, `max`, `sqrt`, `abs`, `eq`, `lt`, `gt`, `sum` |
| SIMD Int | `add`, `sub`, `mul`, `min`, `max`, `eq`, `lt`, `gt`, `sum` |
| Widths | 4-wide (128-bit), 8-wide (256-bit), 16-wide (512-bit) |
| Bit | `count_ones`, `count_leading_zeros`, `count_trailing_zeros`, `rotate_left`, `rotate_right` |
| Query | `cpu_has_feature` |

| Aspect | Behavior |
|--------|----------|
| Safety | Bounds-checked, panic on invalid |
| Fallback | Auto-emulation (default), explicit via EmulatedIntrinsics |
| Detection | `cpu_has_feature` for runtime, `#target` for compile-time |
| Provider | Capability trait with `def impl` |

## Design Decisions

1. **Atomics deferred** — Atomic operations require integration with Ori's memory model and proper pointer types. A separate proposal will address these.
2. **Auto-fallback default** — The default `def impl` uses native instructions when available and emulates otherwise, ensuring code always works across platforms.
3. **64-bit integers only** — Integer SIMD uses Ori's native `int` (i64) to avoid truncation complexity.
4. **String-based feature detection** — Simple `cpu_has_feature("avx2")` pattern with documented valid strings and panic on unknown features.
5. **Core operation set** — Includes arithmetic, comparisons, min/max, math (sqrt/abs), and horizontal sum. More exotic operations (shuffle, blend, FMA) can be added in future proposals.
