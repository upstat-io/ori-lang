# Proposal: Intrinsics v2 — Generic SIMD API & Byte Operations

**Status:** Approved
**Author:** Eric (with AI assistance)
**Created:** 2026-03-05
**Approved:** 2026-03-05
**Revises:** `proposals/approved/intrinsics-capability-proposal.md`
**Affects:** Spec (Clause 20.8.4), Compiler (type checker, evaluator, LLVM codegen), stdlib (`std.bytes`)

---

## Summary

This proposal:
1. Redesigns the Intrinsics SIMD API from explicit-width functions to generic operations that monomorphize based on lane type and width
2. Adds byte-level SIMD operations — the foundation of high-performance string processing, parsing, and lexing
3. Introduces `Mask<$N>` — a dedicated type for SIMD comparison results
4. Fixes the v1 float lane width inconsistency (v1 used `f32` naming but Ori's `float` is f64)
5. Defines `std.bytes` — a stdlib module providing high-level byte search functions backed by SIMD

---

## Motivation

### Byte-Level SIMD Gap

The approved Intrinsics proposal (2026-01-30) covers float and 64-bit integer SIMD but has **no byte-level operations**. Without them:

- `memchr` (find byte in buffer) cannot be implemented natively
- `memchr3` (find any of 3 bytes) cannot be implemented natively
- Byte classification tables (is_digit, is_alpha) cannot be vectorized
- The `std.bytes` stdlib module has no fast path

Byte SIMD is the foundation of high-performance string processing, parsing, lexing, JSON decoding, UTF-8 validation, and network protocol handling.

### Naming Explosion

The v1 API defines ~70 explicit-width functions: `simd_add_f32x4`, `simd_add_f32x8`, `simd_add_f32x16`, `simd_add_i64x2`, `simd_add_i64x4` — each width spelled out. Adding byte operations at three widths would add another ~36 functions. A generic API reduces this to ~20 operations that the compiler monomorphizes.

### Float Lane Width Bug

The v1 proposal uses `f32` in names (`simd_add_f32x4`) but operates on `[float, max 4]` where Ori's `float` is f64. This means `[float, max 4]` = 4 × 64 = 256 bits, not the 128-bit claimed by v1. The generic API fixes this: `simd_add(a: [float, max 2], b: [float, max 2])` correctly represents 128-bit f64 operations.

---

## Design Principles

### 1. Two-level architecture

```
User code               std.bytes.find_byte(bytes:, target:)     // no capability needed
                                    |
stdlib internals        uses Intrinsics -> byte SIMD + scalar tail // capability-gated
```

Most programmers use `std.bytes`. Only stdlib authors and performance specialists touch `Intrinsics` directly.

### 2. Generic operations

Operations are generic over lane type `T` and width `$N`. The compiler monomorphizes based on the fixed-capacity list type at the call site and validates that the `T × N` combination maps to a real SIMD register.

### 3. Dedicated mask type

Comparison operations return `Mask<$N>` instead of byte vectors. This cleanly separates boolean lane masks from data vectors and provides type-safe methods for position extraction.

---

## Generic SIMD API

### Trait Definition

```ori
trait Intrinsics {
    // -- Load --
    @simd_load<T, $N: int> (data: [T], offset: int) -> [T, max N]
    @simd_load_aligned<T, $N: int> (data: [T], offset: int) -> [T, max N]

    // -- Arithmetic --
    @simd_add<T, $N: int> (a: [T, max N], b: [T, max N]) -> [T, max N]
    @simd_sub<T, $N: int> (a: [T, max N], b: [T, max N]) -> [T, max N]
    @simd_mul<T, $N: int> (a: [T, max N], b: [T, max N]) -> [T, max N]
    @simd_div<T, $N: int> (a: [T, max N], b: [T, max N]) -> [T, max N]   // float only
    @simd_sqrt<T, $N: int> (a: [T, max N]) -> [T, max N]                 // float only
    @simd_abs<T, $N: int> (a: [T, max N]) -> [T, max N]                  // float only

    // -- Comparison -> Mask --
    @simd_cmpeq<T, $N: int> (a: [T, max N], b: [T, max N]) -> Mask<N>
    @simd_cmplt<T, $N: int> (a: [T, max N], b: [T, max N]) -> Mask<N>
    @simd_cmpgt<T, $N: int> (a: [T, max N], b: [T, max N]) -> Mask<N>

    // -- Min/Max --
    @simd_min<T, $N: int> (a: [T, max N], b: [T, max N]) -> [T, max N]
    @simd_max<T, $N: int> (a: [T, max N], b: [T, max N]) -> [T, max N]

    // -- Reduction --
    @simd_sum<T, $N: int> (a: [T, max N]) -> T

    // -- Broadcast --
    @simd_splat<T, $N: int> (value: T) -> [T, max N]

    // -- Bitwise (data vectors) --
    @simd_and<T, $N: int> (a: [T, max N], b: [T, max N]) -> [T, max N]
    @simd_or<T, $N: int> (a: [T, max N], b: [T, max N]) -> [T, max N]
    @simd_xor<T, $N: int> (a: [T, max N], b: [T, max N]) -> [T, max N]
    @simd_andnot<T, $N: int> (a: [T, max N], b: [T, max N]) -> [T, max N]

    // -- Select (mask-driven) --
    @simd_select<T, $N: int> (mask: Mask<N>, a: [T, max N], b: [T, max N]) -> [T, max N]

    // -- Byte-specific --
    @simd_shuffle<$N: int> (v: [byte, max N], idx: [byte, max N]) -> [byte, max N]

    // -- Bit operations (unchanged from v1) --
    @count_ones (value: int) -> int
    @count_leading_zeros (value: int) -> int
    @count_trailing_zeros (value: int) -> int
    @rotate_left (value: int, amount: int) -> int
    @rotate_right (value: int, amount: int) -> int

    // -- Hardware queries (unchanged from v1) --
    @cpu_has_feature (feature: str) -> bool
}
```

### Valid Type x Width Combinations

The compiler validates `T x N` against this table. Invalid combinations are compile-time errors (E1063).

| Type | Lane width | 128-bit | 256-bit | 512-bit |
|------|-----------|---------|---------|---------|
| `byte` | 8-bit | `max 16` | `max 32` | `max 64` |
| `int` | 64-bit | `max 2` | `max 4` | `max 8` |
| `float` | 64-bit | `max 2` | `max 4` | `max 8` |

### Operation Availability by Lane Type

Not all operations are valid for all lane types. The compiler enforces:

| Operation | `byte` | `int` | `float` |
|-----------|--------|-------|---------|
| `simd_load`, `simd_load_aligned` | Yes | Yes | Yes |
| `simd_add`, `simd_sub`, `simd_mul` | Yes | Yes | Yes |
| `simd_div`, `simd_sqrt`, `simd_abs` | No | No | Yes |
| `simd_cmpeq`, `simd_cmplt`, `simd_cmpgt` | Yes | Yes | Yes |
| `simd_min`, `simd_max` | Yes | Yes | Yes |
| `simd_sum` | Yes | Yes | Yes |
| `simd_splat` | Yes | Yes | Yes |
| `simd_and`, `simd_or`, `simd_xor`, `simd_andnot` | Yes | Yes | No |
| `simd_select` | Yes | Yes | Yes |
| `simd_shuffle` | Yes | No | No |

Using `simd_div` with `byte` or `int` vectors, or `simd_shuffle` with non-byte vectors, is a compile error.

### Aligned Loads

`simd_load` performs unaligned loads (safe on all platforms). `simd_load_aligned` requires the data offset to be aligned to the vector width boundary. Misalignment causes a panic at runtime.

| Width | Alignment requirement |
|-------|-----------------------|
| 128-bit | 16-byte aligned offset |
| 256-bit | 32-byte aligned offset |
| 512-bit | 64-byte aligned offset |

### Platform Mapping

| Width | x86_64 SSE2 | x86_64 AVX2 | x86_64 AVX-512 | aarch64 NEON | wasm SIMD128 |
|-------|------------|-------------|----------------|--------------|--------------|
| 128-bit | Native | Native | Native | Native | Native |
| 256-bit | Emulated (2x) | Native | Native | Emulated (2x) | Emulated (2x) |
| 512-bit | Emulated (4x) | Emulated (2x) | Native | Emulated (4x) | Emulated (4x) |

128-bit is the portable baseline — native on all SIMD-capable platforms.

### Byte-Specific Platform Mapping

| Generic Operation | x86_64 SSE2 | x86_64 AVX2 | aarch64 NEON | wasm SIMD128 |
|-------------------|-------------|-------------|--------------|--------------|
| `simd_load<byte, 16>` | `_mm_loadu_si128` | `_mm_loadu_si128` | `vld1q_u8` | `v128.load` |
| `simd_cmpeq<byte, 16>` | `_mm_cmpeq_epi8` | `_mm_cmpeq_epi8` | `vceqq_u8` | `i8x16.eq` |
| `simd_shuffle<16>` | `_mm_shuffle_epi8` (SSSE3) | `_mm_shuffle_epi8` | `vqtbl1q_u8` | `i8x16.swizzle` |
| `Mask<16>.bits()` | `_mm_movemask_epi8` | `_mm_movemask_epi8` | polyfill* | `i8x16.bitmask` |
| `simd_splat<byte, 16>` | `_mm_set1_epi8` | `_mm_set1_epi8` | `vdupq_n_u8` | `i8x16.splat` |

\* NEON lacks native `movemask`. Polyfill: `vshrn` + `vget_lane_u64` (~4 instructions).

---

## `Mask<$N>` Type

Comparison operations return `Mask<$N>` — an opaque type representing N boolean lanes.

### Definition

```ori
type Mask<$N: int>
```

`Mask<$N>` is a compiler-known type. It cannot be constructed directly — only SIMD comparison intrinsics produce masks.

### Methods

```ori
impl<$N: int> Mask<N> {
    // Convert to bitmask integer. Bit i = 1 if lane i is true.
    @bits (self) -> int

    // Test if any lane is true.
    @any (self) -> bool

    // Test if all lanes are true.
    @all (self) -> bool

    // Count the number of true lanes.
    @count (self) -> int

    // Index of the first true lane, or None if all false.
    @first_set (self) -> Option<int>
}
```

### Operators

`Mask<$N>` implements bitwise operators for combining masks:

```ori
impl<$N: int> Mask<N>: BitAnd { type Output = Mask<N> }  // mask & mask
impl<$N: int> Mask<N>: BitOr  { type Output = Mask<N> }  // mask | mask
impl<$N: int> Mask<N>: BitNot { type Output = Mask<N> }  // ~mask
```

### Hardware Representation

| Platform | 128-bit Mask<16> | 256-bit Mask<32> | 512-bit Mask<64> |
|----------|-----------------|-----------------|-----------------|
| x86_64 SSE2 | `__m128i` (0xFF/0x00 lanes) | 2x `__m128i` | 4x `__m128i` |
| x86_64 AVX2 | `__m128i` | `__m256i` | 2x `__m256i` |
| x86_64 AVX-512 | `__mmask16` (k-register) | `__mmask32` | `__mmask64` |
| aarch64 NEON | `uint8x16_t` (0xFF/0x00) | 2x `uint8x16_t` | 4x `uint8x16_t` |
| wasm SIMD128 | `v128` (0xFF/0x00) | 2x `v128` | 4x `v128` |

### Valid Mask Widths

`Mask<$N>` is valid for any N that appears as a lane count in the SIMD type table:

| N | Produced by | Register width |
|---|-------------|---------------|
| 2 | `simd_cmpeq<float, 2>`, `simd_cmpeq<int, 2>` | 128-bit |
| 4 | `simd_cmpeq<float, 4>`, `simd_cmpeq<int, 4>` | 256-bit |
| 8 | `simd_cmpeq<float, 8>`, `simd_cmpeq<int, 8>` | 512-bit |
| 16 | `simd_cmpeq<byte, 16>` | 128-bit |
| 32 | `simd_cmpeq<byte, 32>` | 256-bit |
| 64 | `simd_cmpeq<byte, 64>` | 512-bit |

---

## V1 Migration

The v1 explicit-width names become deprecated aliases. They remain valid but emit a deprecation warning:

| V1 Name | Generic Equivalent | Note |
|---------|-------------------|------|
| `simd_add_f32x4` | `simd_add<float, 2>` | Width fix: was 4×f32(128-bit), now 2×f64(128-bit) |
| `simd_add_f32x8` | `simd_add<float, 4>` | Width fix: was 8×f32(256-bit), now 4×f64(256-bit) |
| `simd_add_f32x16` | `simd_add<float, 8>` | Width fix: was 16×f32(512-bit), now 8×f64(512-bit) |
| `simd_add_i64x2` | `simd_add<int, 2>` | Unchanged semantics |
| `simd_add_i64x4` | `simd_add<int, 4>` | Unchanged semantics |
| `simd_eq_f32x4` | `simd_cmpeq<float, 2>` | Now returns `Mask<2>` not `[bool, max 2]` |
| `simd_sum_f32x4` | `simd_sum<float, 2>` | Unchanged semantics |

The `[bool, max N]` return type from v1 comparison operations is replaced by `Mask<N>`.

---

## `std.bytes` Stdlib Module

High-level byte search functions that use SIMD internally. **No `uses Intrinsics` needed by callers.**

```ori
use std.bytes { find_byte, find_any, find_not, count_byte, contains_byte }
```

### API

```ori
// Find the first occurrence of `target` in `bytes` starting from `from`.
// Returns None if not found.
@find_byte (bytes: [byte], target: byte, from: int = 0) -> Option<int>

// Find the first occurrence of any byte in `targets`.
// Equivalent to memchr2/memchr3 for 2-3 targets.
@find_any (bytes: [byte], targets: [byte], from: int = 0) -> Option<int>

// Find the first byte NOT in `accept` set.
// Useful for "eat while whitespace" patterns.
@find_not (bytes: [byte], accept: [byte], from: int = 0) -> Option<int>

// Count occurrences of `target` in a byte range.
@count_byte (bytes: [byte], target: byte, from: int = 0) -> int

// Check if a byte range contains `target`.
@contains_byte (bytes: [byte], target: byte, from: int = 0) -> bool
```

### Default Width

`std.bytes` uses 128-bit (16-byte) SIMD chunks as the portable baseline. This is native on SSE2, NEON, and SIMD128. Callers who need AVX2 throughput can use Intrinsics directly with 256-bit widths via `cpu_has_feature` detection.

### Implementation Strategy

```ori
// Inside std.bytes — uses Intrinsics internally
@find_byte (bytes: [byte], target: byte, from: int = 0) -> Option<int>
    uses Intrinsics
= {
    let $len = bytes.len();
    let pos = from;

    // SIMD path: 16 bytes at a time
    let $needle: [byte, max 16] = Intrinsics.simd_splat(value: target);
    while pos + 16 <= len do {
        let chunk: [byte, max 16] = Intrinsics.simd_load(data: bytes, offset: pos);
        let mask = Intrinsics.simd_cmpeq(a: chunk, b: needle);  // Mask<16>
        if mask.any() then {
            break Some(pos + Intrinsics.count_trailing_zeros(value: mask.bits()))
        };
        pos += 16;
    }

    // Scalar tail
    while pos < len do {
        if bytes[pos] == target then break Some(pos);
        pos += 1;
    }

    None
}
```

### Lexer Usage Example

```ori
use std.bytes { find_byte, find_any }

impl Scanner {
    @eat_until_newline_or_eof (self) -> void = {
        let remaining = self.buf.slice(start: self.pos);
        match find_byte(bytes: remaining, target: b'\n') {
            Some(offset) -> { self.pos += offset; }
            None -> { self.pos = self.buf.len(); }
        }
    }

    @skip_to_string_delim (self) -> byte = {
        let remaining = self.buf.slice(start: self.pos);
        match find_any(bytes: remaining, targets: [b'"', b'\\', b'\n', b'\r']) {
            Some(offset) -> {
                self.pos += offset;
                self.buf[self.pos]
            }
            None -> {
                self.pos = self.buf.len();
                b'\0'
            }
        }
    }
}
```

---

## Cost Model

### Zero-cost abstraction guarantee

`[byte, max N]` in SIMD context shall compile to register operations, not heap-allocated lists:

| Context | Representation |
|---------|---------------|
| Intrinsic argument/return | SIMD register (XMM/YMM/ZMM/NEON Q) |
| `let v: [byte, max 16] = [...]` outside SIMD | Stack-allocated inline storage |
| Passed to non-intrinsic function | Stack spill + reload |

The compiler shall recognize Intrinsics call patterns and keep intermediate SIMD vectors in registers without spilling to memory. This is an LLVM codegen optimization — the ARC pipeline classifies `[T, max N]` as Scalar when used in SIMD context (no heap allocation, no RC).

`Mask<$N>` values are similarly register-allocated — never heap-allocated.

### Performance Expectations

| Operation | Expected throughput | Baseline |
|-----------|-------------------|----------|
| `find_byte` (16-byte SIMD) | ~8-12 GiB/s | C `memchr` SSE2: ~12 GiB/s |
| `find_any` 3 targets (16-byte) | ~4-8 GiB/s | C `memchr3` SSE2: ~8 GiB/s |
| `find_byte` scalar fallback | ~1-2 GiB/s | C `strchr`: ~1.5 GiB/s |

SIMD path should achieve 60-80% of hand-tuned C `memchr`. The scalar fallback matches naive C performance.

---

## Changes to Spec (Clause 20.8.4)

### Additions

1. Replace explicit-width operation listing with generic SIMD API table
2. Add `Mask<$N>` type definition and methods
3. Add byte vector types and byte-specific operations (`shuffle`)
4. Add aligned load specification
5. Add valid T x N combinations table
6. Add operation availability by lane type table
7. Add `std.bytes` module reference
8. Add cost model note: SIMD vectors and masks are register-allocated

### Modifications

1. Fix float lane width: `[float, max 4]` (128-bit) becomes `[float, max 2]` (128-bit, f64 lanes)
2. Comparison return type: `[bool, max N]` becomes `Mask<N>`
3. V1 explicit names noted as deprecated aliases

### Additions to Feature Detection

Add to `cpu_has_feature` valid strings:

| Platform | New Features |
|----------|-------------|
| x86_64 | `"ssse3"` (required for byte shuffle), `"avx512bw"` (byte-width AVX-512) |
| aarch64 | (none — NEON is baseline) |
| wasm32 | (none — SIMD128 is sufficient) |

---

## Prior Art

| Language | SIMD approach |
|----------|--------------|
| **Rust** | `std::arch` raw intrinsics + `memchr` crate. Explicit per-platform, no generic layer. |
| **Zig** | `@Vector(16, u8)` first-class SIMD type + generic operations. Closest to our design. |
| **Go** | `bytes.IndexByte` in stdlib, assembly implementations per platform. No user-facing SIMD. |
| **Swift** | No direct SIMD for bytes; uses C `memchr` via bridge. |
| **C** | `<immintrin.h>` raw intrinsics, `memchr` in libc. |
| **LLVM IR** | `<N x T>` vector types with generic operations + `<N x i1>` masks. Our `Mask<$N>` mirrors this. |

Ori's approach is closest to **Zig** (generic SIMD operations, compiler picks instructions) combined with **Go** (high-level stdlib functions). The two-level architecture (stdlib for users, Intrinsics for implementers) is unique to Ori's capability model. The `Mask<$N>` type mirrors LLVM IR's `<N x i1>` separation of masks from data vectors.

---

## Design Decisions

1. **Generic over explicit** — ~20 generic operations replace ~70 explicit-width functions. The compiler monomorphizes based on type arguments and validates against a validity table.
2. **`Mask<$N>` type** — Cleanly separates boolean lane masks from data vectors. Methods (`bits`, `any`, `first_set`) provide type-safe position extraction. Bitwise operators on masks (`&`, `|`, `~`) are separate from bitwise operations on data vectors.
3. **Float lane width fix** — Ori's `float` is f64. `[float, max 2]` = 128-bit is correct. V1's `f32x4` naming was wrong.
4. **Aligned loads included** — `simd_load_aligned` enables maximum throughput for code that controls alignment. Panics on misalignment (not UB).
5. **128-bit default for `std.bytes`** — Portable baseline. Native on all SIMD platforms. AVX2 users can call Intrinsics directly.
6. **`std.bytes` not in prelude** — Requires `use std.bytes`. Lean prelude principle.
7. **`shuffle` is byte-only** — Byte shuffle (`pshufb`/`tbl`) is the most commonly needed form. Float/int permutations can be added later.
8. **Atomics still deferred** — Require integration with memory model. Separate proposal.

---

## Summary of Changes from V1

| Area | V1 (Current) | V2 (This Proposal) |
|------|-------------|---------------------|
| API style | Explicit width (`simd_add_f32x4`) | Generic (`simd_add<T, $N>`) |
| Float lanes | Wrong (`f32` naming, `float`=f64) | Correct (`[float, max 2]` = 128-bit f64) |
| Comparison result | `[bool, max N]` | `Mask<$N>` with methods |
| Byte SIMD | None | All generic ops + `shuffle` |
| Mask operations | N/A | `Mask<$N>` operators (`&`, `\|`, `~`) + methods |
| Aligned loads | None | `simd_load_aligned` |
| Stdlib | None | `std.bytes` (5 functions) |
| Cost model | Unspecified | Register allocation guarantee |
| Total API surface | ~70 explicit functions | ~20 generic operations + `Mask<$N>` |
