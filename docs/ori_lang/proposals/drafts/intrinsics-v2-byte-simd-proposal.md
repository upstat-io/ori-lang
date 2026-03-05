# Proposal: Intrinsics v2 — Byte-Level SIMD & Systematic Redesign

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-03-05
**Revises:** `proposals/approved/intrinsics-capability-proposal.md`
**Affects:** Spec (Clause 20.8.4), Compiler, stdlib (`std.bytes`)

---

## Motivation

The approved Intrinsics proposal (2026-01-30) covers float SIMD and 64-bit integer SIMD. It has **no byte-level SIMD operations** — no way to load, compare, search, or classify bytes in parallel.

Byte-level SIMD is the foundation of high-performance string processing, parsing, lexing, JSON decoding, UTF-8 validation, and network protocol handling. Without it:

- `memchr` (find byte in buffer) cannot be implemented natively
- `memchr3` (find any of 3 bytes) cannot be implemented natively
- Byte classification tables (is_digit, is_alpha) cannot be vectorized
- The `std.bytes` stdlib module has no fast path

This revision adds byte-level SIMD, simplifies the naming scheme, and defines the `std.bytes` stdlib module that makes SIMD accessible without `uses Intrinsics`.

---

## Design Principles

### 1. Two-level architecture

```
User code               std.bytes.find_byte(bytes:, target:)     // no capability needed
                                    |
stdlib internals        uses Intrinsics → byte SIMD + scalar tail // capability-gated
```

Most programmers use `std.bytes`. Only stdlib authors and performance specialists touch `Intrinsics` directly.

### 2. Byte vectors as first-class SIMD type

`[byte, max 16]`, `[byte, max 32]`, `[byte, max 64]` map directly to SIMD registers. Unlike `[int, max 2]` (which packs 2 i64s into 128 bits), byte vectors pack 16/32/64 bytes — the natural width for string scanning.

### 3. Systematic naming

Current: `simd_add_f32x4`, `simd_add_f32x8`, `simd_add_i64x2` — verbose, every width spelled out.

Proposed: operations are generic over lane type and width. The compiler monomorphizes based on the fixed-capacity list type at the call site.

---

## Changes to Intrinsics Capability

### New: Byte SIMD operations

```ori
trait Intrinsics {
    // -- Byte vector operations (NEW) --

    // Load bytes from a byte slice at the given offset.
    // Panics if offset + width > bytes.len().
    @simd_load_u8x16 (bytes: [byte], offset: int) -> [byte, max 16];
    @simd_load_u8x32 (bytes: [byte], offset: int) -> [byte, max 32];

    // Compare each lane against a scalar byte.
    // Returns a mask: 0xFF where equal, 0x00 where not.
    @simd_cmpeq_u8x16 (v: [byte, max 16], scalar: byte) -> [byte, max 16];
    @simd_cmpeq_u8x32 (v: [byte, max 32], scalar: byte) -> [byte, max 32];

    // Compare each lane: less-than (unsigned)
    @simd_cmplt_u8x16 (a: [byte, max 16], b: [byte, max 16]) -> [byte, max 16];
    @simd_cmplt_u8x32 (a: [byte, max 32], b: [byte, max 32]) -> [byte, max 32];

    // Bitwise OR/AND of two byte vectors (for combining masks)
    @simd_or_u8x16 (a: [byte, max 16], b: [byte, max 16]) -> [byte, max 16];
    @simd_or_u8x32 (a: [byte, max 32], b: [byte, max 32]) -> [byte, max 32];
    @simd_and_u8x16 (a: [byte, max 16], b: [byte, max 16]) -> [byte, max 16];
    @simd_and_u8x32 (a: [byte, max 32], b: [byte, max 32]) -> [byte, max 32];
    @simd_andnot_u8x16 (a: [byte, max 16], b: [byte, max 16]) -> [byte, max 16];
    @simd_andnot_u8x32 (a: [byte, max 32], b: [byte, max 32]) -> [byte, max 32];

    // Broadcast: fill all lanes with a single byte value
    @simd_splat_u8x16 (value: byte) -> [byte, max 16];
    @simd_splat_u8x32 (value: byte) -> [byte, max 32];

    // Extract high bit of each byte lane into an integer bitmask.
    // Bit 0 = lane 0's high bit, bit 1 = lane 1's high bit, etc.
    @simd_movemask_u8x16 (v: [byte, max 16]) -> int;
    @simd_movemask_u8x32 (v: [byte, max 32]) -> int;

    // Test if any lane in the mask is non-zero
    @simd_any_u8x16 (v: [byte, max 16]) -> bool;
    @simd_any_u8x32 (v: [byte, max 32]) -> bool;

    // Shuffle: rearrange bytes according to index vector.
    // Each lane of `idx` selects a byte from `v` (idx[i] & 0x0F).
    // If high bit of idx[i] is set, result lane is 0x00.
    @simd_shuffle_u8x16 (v: [byte, max 16], idx: [byte, max 16]) -> [byte, max 16];
    @simd_shuffle_u8x32 (v: [byte, max 32], idx: [byte, max 32]) -> [byte, max 32];

    // -- Existing operations (unchanged) --

    // Float SIMD: simd_add_f32x4, simd_mul_f32x4, etc.
    // Int SIMD: simd_add_i64x2, simd_add_i64x4, etc.
    // Bit ops: count_ones, count_leading_zeros, count_trailing_zeros, etc.
    // Hardware: cpu_has_feature
}
```

### Platform mapping

| Intrinsic | x86_64 SSE2 | x86_64 AVX2 | aarch64 NEON | wasm SIMD128 |
|-----------|-------------|-------------|--------------|--------------|
| `simd_load_u8x16` | `_mm_loadu_si128` | `_mm_loadu_si128` | `vld1q_u8` | `v128.load` |
| `simd_load_u8x32` | emulated (2x) | `_mm256_loadu_si256` | emulated (2x) | emulated (2x) |
| `simd_cmpeq_u8x16` | `_mm_cmpeq_epi8` | `_mm_cmpeq_epi8` | `vceqq_u8` | `i8x16.eq` |
| `simd_cmpeq_u8x32` | emulated (2x) | `_mm256_cmpeq_epi8` | emulated (2x) | emulated (2x) |
| `simd_movemask_u8x16` | `_mm_movemask_epi8` | `_mm_movemask_epi8` | polyfill* | `i8x16.bitmask` |
| `simd_movemask_u8x32` | emulated (2x) | `_mm256_movemask_epi8` | polyfill* | emulated (2x) |
| `simd_shuffle_u8x16` | `_mm_shuffle_epi8` (SSSE3) | `_mm_shuffle_epi8` | `vqtbl1q_u8` | `i8x16.swizzle` |
| `simd_splat_u8x16` | `_mm_set1_epi8` | `_mm_set1_epi8` | `vdupq_n_u8` | `i8x16.splat` |

\* NEON lacks native `movemask`. Polyfill: `vshrn` + `vget_lane_u64` (~4 instructions).

### Byte vector types

| Width | Type | Register | Platforms (native) |
|-------|------|----------|-------------------|
| 128-bit | `[byte, max 16]` | XMM / NEON Q / v128 | SSE2, NEON, SIMD128 |
| 256-bit | `[byte, max 32]` | YMM | AVX2 |
| 512-bit | `[byte, max 64]` | ZMM | AVX-512BW |

128-bit is the portable baseline — native on all SIMD-capable platforms.

---

## New: `std.bytes` stdlib module

High-level byte search functions that use SIMD internally. **No `uses Intrinsics` needed by callers.**

```ori
use std.bytes { find_byte, find_any, find_not }

// Find the first occurrence of `target` in `bytes` starting from `from`.
// Returns None if not found.
@find_byte (bytes: [byte], target: byte, from: int = 0) -> Option<int>;

// Find the first occurrence of any byte in `targets`.
// Equivalent to memchr2/memchr3 for 2-3 targets.
@find_any (bytes: [byte], targets: [byte], from: int = 0) -> Option<int>;

// Find the first byte NOT in `accept` set.
// Useful for "eat while whitespace" patterns.
@find_not (bytes: [byte], accept: [byte], from: int = 0) -> Option<int>;

// Count occurrences of `target` in a byte range.
@count_byte (bytes: [byte], target: byte, from: int = 0) -> int;

// Check if a byte range contains `target`.
@contains_byte (bytes: [byte], target: byte, from: int = 0) -> bool;
```

### Implementation strategy

```ori
// Inside std.bytes — uses Intrinsics internally
@find_byte (bytes: [byte], target: byte, from: int = 0) -> Option<int>
    uses Intrinsics
= {
    let $len = bytes.len();
    let pos = from;

    // SIMD path: 16 bytes at a time
    let $needle = Intrinsics.simd_splat_u8x16(value: target);
    while pos + 16 <= len do {
        let chunk = Intrinsics.simd_load_u8x16(bytes: bytes, offset: pos);
        let mask = Intrinsics.simd_cmpeq_u8x16(v: chunk, scalar: target);
        let bits = Intrinsics.simd_movemask_u8x16(v: mask);
        if bits != 0 then {
            break Some(pos + Intrinsics.count_trailing_zeros(value: bits))
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

### Lexer usage example

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
| `Intrinsics.simd_load_u8x16(...)` | XMM/NEON register (128-bit) |
| `Intrinsics.simd_load_u8x32(...)` | YMM register (256-bit) |
| `let v: [byte, max 16] = [...]` outside SIMD | Stack-allocated inline storage |
| Passed to non-intrinsic function | Stack spill + reload |

The compiler shall recognize Intrinsics call patterns and keep intermediate byte vectors in registers without spilling to memory. This is an LLVM codegen optimization — the ARC pipeline classifies `[byte, max 16]` as Scalar when used in SIMD context (no heap allocation, no RC).

### Performance expectations

| Operation | Expected throughput | Baseline |
|-----------|-------------------|----------|
| `find_byte` (16-byte SIMD) | ~8-12 GiB/s | C `memchr` SSE2: ~12 GiB/s |
| `find_any` 3 targets (16-byte) | ~4-8 GiB/s | C `memchr3` SSE2: ~8 GiB/s |
| `find_byte` scalar fallback | ~1-2 GiB/s | C `strchr`: ~1.5 GiB/s |

SIMD path should achieve 60-80% of hand-tuned C `memchr` (which uses AVX2 + clever alignment tricks). The scalar fallback matches naive C performance.

---

## Changes to Spec (Clause 20.8.4)

### Additions

1. Add byte vector types table: `[byte, max 16]`, `[byte, max 32]`, `[byte, max 64]`
2. Add byte SIMD operations table: `load`, `cmpeq`, `cmplt`, `or`, `and`, `andnot`, `splat`, `movemask`, `any`, `shuffle`
3. Add platform mapping table for byte operations
4. Add cost model note: byte vectors in Intrinsics context are register-allocated
5. Add `std.bytes` module reference

### Modifications

1. Expand the SIMD operations table to include byte category alongside float and int
2. Add NEON `movemask` polyfill note (NEON lacks native movemask)

### No changes

- Float SIMD operations (unchanged)
- Int SIMD operations (unchanged)
- Bit operations (unchanged)
- Hardware feature detection (unchanged)
- Safety guarantees (unchanged — byte ops panic on OOB, not UB)

---

## Prior Art

| Language | Byte SIMD approach |
|----------|--------------------|
| **Rust** | `std::arch` raw intrinsics + `memchr` crate as stdlib-level wrapper |
| **Zig** | `@Vector(16, u8)` first-class SIMD type + `std.mem.indexOfScalar` |
| **Go** | `bytes.IndexByte` in stdlib, assembly implementations per platform |
| **Swift** | No direct SIMD for bytes; uses C `memchr` via bridge |
| **C** | `<immintrin.h>` raw intrinsics, `memchr` in libc |

Ori's approach is closest to **Zig** (SIMD types in the language) combined with **Go** (high-level stdlib functions). The two-level architecture (stdlib for users, Intrinsics for implementers) is unique to Ori's capability model.

---

## Open Questions

1. **Generic SIMD operations?** Should `simd_add` be generic over lane type and width (one function, compiler picks the right instruction), or should each width remain explicit? Generic reduces API surface but complicates type checking.

2. **Aligned loads?** The current proposal only has unaligned loads (`loadu`). Should we expose aligned loads (`loada`) for performance-critical code that controls alignment? Or should the compiler auto-select?

3. **256-bit as default?** AVX2 is ubiquitous on modern x86_64. Should `std.bytes` default to 32-byte chunks on x86_64, falling back to 16-byte on other platforms? Or always use 16-byte for portability?

4. **Mask type?** `movemask` returns `int` (i64). Should there be a dedicated `Mask16`/`Mask32` type for clarity, or is `int` sufficient?

5. **`std.bytes` in prelude?** Should `find_byte`/`find_any` be promoted to the prelude for common use, or always require `use std.bytes`?

---

## Summary of Changes

| Area | Current (v1) | Proposed (v2) |
|------|-------------|---------------|
| Float SIMD | 12 ops x 3 widths | Unchanged |
| Int SIMD | 9 ops x 2 widths | Unchanged |
| **Byte SIMD** | **None** | **12 ops x 2 widths** |
| Bit ops | 5 ops | Unchanged |
| Stdlib | None | **`std.bytes` module (5 functions)** |
| Cost model | Unspecified | **Register allocation guarantee** |
