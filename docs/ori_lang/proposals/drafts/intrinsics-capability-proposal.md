# Proposal: Intrinsics Capability

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-01-30
**Affects:** Compiler, capabilities, low-level operations

---

## Summary

This proposal formalizes the `Intrinsics` capability for low-level, platform-specific operations including SIMD, atomic operations, and hardware-specific features.

---

## Problem Statement

The spec mentions `uses Intrinsics` in conditional compilation examples but doesn't define:

1. **Purpose**: What operations require Intrinsics?
2. **Available operations**: What intrinsics are provided?
3. **Platform behavior**: How do intrinsics vary by target?
4. **Safety**: What guarantees do intrinsics provide?
5. **Fallbacks**: How to handle missing intrinsics?

---

## Capability Definition

```ori
trait Intrinsics {
    // SIMD operations
    @simd_add_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
    @simd_mul_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
    // ... more SIMD ops

    // Atomic operations
    @atomic_load (ptr: int) -> int
    @atomic_store (ptr: int, value: int) -> void
    @atomic_add (ptr: int, value: int) -> int
    @atomic_compare_exchange (ptr: int, expected: int, desired: int) -> (bool, int)

    // Memory operations
    @prefetch (ptr: int) -> void
    @memory_fence () -> void

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
    // Use SIMD intrinsics
    ...
```

### Capability Provision

```ori
with Intrinsics = NativeIntrinsics {} in
    fast_dot_product(a, b)
```

### Conditional Use

```ori
@dot_product (a: [float], b: [float]) -> float =
    #target(arch: "x86_64")
    with Intrinsics = NativeIntrinsics {} in fast_dot_product(a, b)

    #target(not_arch: "x86_64")
    scalar_dot_product(a, b)
```

---

## SIMD Operations

### Vector Types

SIMD operations work on fixed-size arrays:

```ori
// 4-wide float operations
@simd_add_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
@simd_mul_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
@simd_sub_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]
@simd_div_f32x4 (a: [float, max 4], b: [float, max 4]) -> [float, max 4]

// 4-wide integer operations
@simd_add_i32x4 (a: [int, max 4], b: [int, max 4]) -> [int, max 4]
// ... etc
```

### Example: SIMD Dot Product

```ori
@simd_dot_4 (a: [float, max 4], b: [float, max 4]) -> float uses Intrinsics = run(
    let products = Intrinsics.simd_mul_f32x4(a, b),
    products[0] + products[1] + products[2] + products[3],
)
```

### Platform Availability

| Target | Available SIMD |
|--------|----------------|
| x86_64 | SSE, AVX, AVX2, AVX-512 (depending on CPU) |
| aarch64 | NEON |
| wasm32 | SIMD128 |

---

## Atomic Operations

### Memory Ordering

Atomic operations have implicit sequential consistency:

```ori
@atomic_load (ptr: int) -> int uses Intrinsics
@atomic_store (ptr: int, value: int) -> void uses Intrinsics
@atomic_add (ptr: int, value: int) -> int uses Intrinsics  // Returns old value
@atomic_sub (ptr: int, value: int) -> int uses Intrinsics
@atomic_and (ptr: int, value: int) -> int uses Intrinsics
@atomic_or (ptr: int, value: int) -> int uses Intrinsics
@atomic_xor (ptr: int, value: int) -> int uses Intrinsics
```

### Compare-and-Exchange

```ori
@atomic_compare_exchange (ptr: int, expected: int, desired: int) -> (bool, int) uses Intrinsics
// Returns (success, actual_value)
```

### Usage Pattern

```ori
@atomic_increment (counter_ptr: int) -> int uses Intrinsics = run(
    let old = Intrinsics.atomic_add(ptr: counter_ptr, value: 1),
    old + 1,  // Return new value
)
```

---

## Bit Manipulation

### Population Count and Leading/Trailing Zeros

```ori
@count_ones (value: int) -> int uses Intrinsics
// Number of set bits (popcount)

@count_leading_zeros (value: int) -> int uses Intrinsics
// Number of leading zero bits

@count_trailing_zeros (value: int) -> int uses Intrinsics
// Number of trailing zero bits
```

### Rotation

```ori
@rotate_left (value: int, amount: int) -> int uses Intrinsics
@rotate_right (value: int, amount: int) -> int uses Intrinsics
```

### Example

```ori
@is_power_of_two (n: int) -> bool uses Intrinsics =
    n > 0 && Intrinsics.count_ones(value: n) == 1
```

---

## Memory Operations

### Prefetch

Hint to prefetch memory:

```ori
@prefetch (ptr: int) -> void uses Intrinsics
// No-op if not supported
```

### Memory Fence

```ori
@memory_fence () -> void uses Intrinsics
// Full memory barrier
```

---

## Hardware Feature Detection

### Runtime Detection

```ori
@cpu_has_feature (feature: str) -> bool uses Intrinsics
```

Features:
- `"sse"`, `"sse2"`, `"sse3"`, `"sse4.1"`, `"sse4.2"`
- `"avx"`, `"avx2"`, `"avx512f"`
- `"neon"`
- `"simd128"` (WASM)

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

### Unsupported Operations

Calling an unsupported intrinsic panics:

```ori
// On platform without AVX-512:
Intrinsics.simd_add_f32x16(a, b)  // panic: intrinsic not available
```

### Compile-Time Detection

Use conditional compilation for unavailable intrinsics:

```ori
#target(arch: "x86_64")
@fast_impl (data: [float]) -> float uses Intrinsics = ...

#target(not_arch: "x86_64")
@fast_impl (data: [float]) -> float = compile_error(msg: "x86_64 only")
```

---

## Safety Guarantees

### No Undefined Behavior

Unlike C intrinsics, Ori intrinsics:
- Check bounds where applicable
- Panic on invalid inputs (not UB)
- Don't allow arbitrary memory access

### Atomic Safety

Atomic operations work on properly aligned addresses:

```ori
Intrinsics.atomic_load(ptr: unaligned_ptr)  // panic: unaligned atomic
```

### SIMD Safety

SIMD operations require properly sized inputs:

```ori
Intrinsics.simd_add_f32x4(a: [1.0, 2.0], b: [...])  // panic: expected 4 elements
```

---

## Implementation Providers

### NativeIntrinsics

Default implementation using platform instructions:

```ori
with Intrinsics = NativeIntrinsics {} in
    fast_operation()
```

### EmulatedIntrinsics

Fallback using scalar operations:

```ori
with Intrinsics = EmulatedIntrinsics {} in
    fast_operation()  // Works but slower
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
```

### Unsupported Intrinsic

```
error[E1061]: intrinsic not available on target platform
  --> src/main.ori:5:5
   |
 5 |     Intrinsics.simd_add_f32x16(a, b)
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ AVX-512 not available
   |
   = note: target `aarch64-apple-darwin` does not support AVX-512
   = help: use conditional compilation to provide fallback
```

---

## Spec Changes Required

### Update `14-capabilities.md`

Add Intrinsics capability section with:
1. Purpose and scope
2. Available operations
3. Platform availability
4. Safety guarantees

### Create `spec/24-intrinsics.md`

New spec file for detailed intrinsic documentation.

---

## Summary

| Category | Operations |
|----------|------------|
| SIMD | `simd_add_*`, `simd_mul_*`, `simd_sub_*`, `simd_div_*` |
| Atomic | `atomic_load`, `atomic_store`, `atomic_add`, `atomic_compare_exchange` |
| Bit | `count_ones`, `count_leading_zeros`, `count_trailing_zeros`, `rotate_*` |
| Memory | `prefetch`, `memory_fence` |
| Query | `cpu_has_feature` |

| Aspect | Behavior |
|--------|----------|
| Safety | Bounds-checked, panic on invalid |
| Unavailable | Panic at runtime |
| Detection | `cpu_has_feature` for runtime |
| Fallback | Use conditional compilation |
