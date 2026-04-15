# Proposal: Limbs Trait — Compiler-Driven Multi-Word Arithmetic

**Status:** Draft
**Author:** Eric (with AI assistance)
**Created:** 2026-04-14
**Affects:** Compiler (type checker, AIMS pipeline, LLVM codegen), type system, standard library, spec
**Depends On:** `value-trait-proposal.md`, `overflow-behavior-proposal.md`, `intrinsics-v2-byte-simd-proposal.md`, `const-generics-proposal.md`

---

## Summary

Introduce a `Limbs` trait that marks a type as a fixed-width multi-word integer. Types conforming to `Limbs` declare their structure — limb type, limb count — and the compiler derives optimal arithmetic, comparison, hashing, and serialization automatically. The programmer describes **what the type is**; the compiler generates **how to operate on it**.

```ori
type U256: Value, Limbs, Eq, Comparable, Hashable, Add, Mul = {
    digits: [int, max 4],
}

// That's it. The compiler derives:
// - Carry-chained addition/subtraction (unrolled for WIDTH <= 8, SIMD-vectorized otherwise)
// - Schoolbook/Karatsuba/Toom-Cook multiplication (selected by WIDTH)
// - MSB-first comparison, limb-wise equality, FNV-1a hashing
// - All operations are ARC-free (Value), register-allocated when possible
```

This is the Ori approach to high-performance numerics: **traits as compiler-consumable meta-information**, not hand-written optimized code.

---

## Motivation

### The GMP Problem

High-performance arbitrary-precision arithmetic (GMP, libtommath, OpenSSL BIGNUM) is implemented by hand-tuning algorithms and assembly for each target architecture. This approach:

1. **Requires expert manual optimization** — GMP has 30+ years of hand-written assembly for 13 CPU families
2. **Duplicates algorithm logic per representation** — the same Karatsuba is reimplemented for each limb size and type
3. **Cannot benefit from compiler analysis** — the optimizer sees opaque pointer arithmetic, not semantic intent
4. **Creates a C dependency** — every language binds to GMP via FFI rather than expressing the problem natively

### The Ori Opportunity

Ori already has the building blocks to solve this differently:

- **`Value` trait** — guarantees no ARC, inline storage, bitwise copy. The compiler *knows* there is no reference counting overhead, not merely *optimizes it away*.
- **Const generics (`$N: int`)** — compile-time parameterization over limb count. Enables monomorphization per size.
- **Compile-time reflection (`$for`, `$if`)** — generic code that expands to specialized code per instantiation. Loop unrolling is a compiler consequence of knowing `$WIDTH`, not a manual optimization.
- **SIMD intrinsics generic over `[T, max N]`** — hardware instruction selection driven by type, not by programmer-written dispatch tables.
- **AIMS lattice** — functional-style code (creating new values) compiles to imperative code (in-place mutation) when uniqueness is proven. The programmer writes clean algorithms; AIMS makes them fast.
- **`checked_add` / `wrapping_add` / `wrapping_mul`** — safe overflow control for limb arithmetic without dropping to unsafe.

What's missing is the **trait that ties these together** — the declaration that says "this type is a multi-word integer" and unlocks the compiler's ability to derive everything from that fact.

### The Philosophy

In C, the programmer optimizes. In Ori, the programmer **informs** and the compiler optimizes. The `Limbs` trait is the information channel: it tells the compiler "these are limbs, here is their structure" and the compiler generates carry chains, selects algorithms, chooses SIMD widths, and proves in-place mutation safety — all from the type declaration.

This is not syntactic sugar. It is a categorically different approach to high-performance numerics.

---

## Design

### The `Limbs` Trait

```ori
trait Limbs: Value {
    /// The type of each individual limb.
    /// Must be Value (inline, no ARC). Typically `int` (64-bit) or `byte` (8-bit).
    type Limb: Value

    /// Number of limbs, known at compile time.
    /// Drives algorithm selection, SIMD width, unrolling decisions.
    let $WIDTH: int

    /// Access the limb at the given index (0 = least significant).
    @limb (self, index: int) -> Self.Limb

    /// Return a copy with one limb replaced.
    @with_limb (self, index: int, value: Self.Limb) -> Self

    /// Number of significant limbs (may be less than $WIDTH).
    @limb_count (self) -> int
}
```

**Supertrait rationale**: `Value` is required because multi-word integers must be ARC-free and bitwise-copyable for arithmetic to be efficient. Heap-allocated, reference-counted limb arrays would defeat the purpose. The `Value` supertrait is a hard constraint, not a convenience.

### Declaration Syntax

```ori
// 256-bit unsigned integer: 4 × 64-bit limbs
type U256: Value, Limbs, Eq, Comparable, Hashable = {
    digits: [int, max 4],
}

// 512-bit unsigned integer: 8 × 64-bit limbs
type U512: Value, Limbs, Eq, Comparable, Hashable = {
    digits: [int, max 8],
}

// 2048-bit for RSA key operations: 32 × 64-bit limbs
type U2048: Value, Limbs, Eq, Comparable, Hashable = {
    digits: [int, max 32],
}

// Byte-oriented 256-bit for constant-time crypto
type CryptoU256: Value, Limbs, Eq = {
    bytes: [byte, max 32],
}
```

Each declaration tells the compiler: "this is a multi-word integer with these dimensions." The compiler derives everything else.

### Compiler-Derived Trait Implementations

When a type declares `Limbs`, the compiler can derive implementations for arithmetic and comparison traits based on the limb structure. These derived implementations use the `$WIDTH` and `Limb` associated type to select optimal strategies.

#### Arithmetic: `Add`, `Sub`

```ori
// Compiler-derived addition for any T: Limbs
// The compiler generates this — the user does NOT write it.
impl<T: Limbs> T: Add {
    @add (self, other: Self) -> Self uses Intrinsics = {
        $if T.$WIDTH <= 8 then {
            // Small: fully unrolled at compile time via $for
            let $result = Self.zero()
            let $carry = 0
            $for i in 0..T.$WIDTH do {
                let $a = self.limb(index: i)
                let $b = other.limb(index: i)
                let $sum_with_carry = wrapping_add(a: wrapping_add(a: a, b: b), b: carry)
                carry = $if is_type(T.Limb, int) then {
                    // 64-bit limbs: carry when sum < either operand
                    let $sum_no_carry = wrapping_add(a: a, b: b)
                    if sum_no_carry < a || sum_with_carry < carry then 1 else 0
                } else {
                    // byte limbs: carry via checked_add
                    match checked_add(a: a, b: b) {
                        None -> 1
                        Some(_) -> if sum_with_carry < carry then 1 else 0
                    }
                }
                result = result.with_limb(index: i, value: sum_with_carry)
            }
            result
        } else {
            // Large: SIMD-vectorized, auto-selected width
            add_vectorized<T>(a: self, b: other)
        }
    }
}
```

For `U256` ($WIDTH=4), the `$for` expands to four straight-line addition sequences — no loop overhead, fully register-allocated. For `U2048` ($WIDTH=32), the compiler emits a SIMD-vectorized loop using `simd_add` at the widest available lane width for the target.

#### Multiplication: `Mul`

```ori
// Compiler-derived multiplication — algorithm selected by $WIDTH
impl<T: Limbs> T: Mul {
    @multiply (self, other: Self) -> Self uses Intrinsics = {
        $if T.$WIDTH <= 4 then {
            schoolbook<T>(a: self, b: other)
        } else $if T.$WIDTH <= 32 then {
            karatsuba<T>(a: self, b: other)
        } else {
            toom_cook_3<T>(a: self, b: other)
        }
    }
}
```

The `$if` on `T.$WIDTH` is compile-time dead-code elimination. Each monomorphized instance contains only the algorithm that applies to its size. `U256` gets schoolbook (optimal for 4 limbs). `U2048` gets Karatsuba. No runtime dispatch.

#### Comparison: `Eq`, `Comparable`

```ori
// Derived Eq: limb-by-limb equality
// For Value types, the compiler may further optimize to memcmp
impl<T: Limbs> T: Eq {
    @equals (self, other: Self) -> bool = {
        $for i in 0..T.$WIDTH do {
            if self.limb(index: i) != other.limb(index: i) then return false
        }
        true
    }
}

// Derived Comparable: MSB-first lexicographic ordering
impl<T: Limbs> T: Comparable {
    @compare (self, other: Self) -> Ordering = {
        // Most significant limb first
        $for i in (0..T.$WIDTH).rev() do {
            let $a = self.limb(index: i)
            let $b = other.limb(index: i)
            if a < b then return Ordering.Less
            if a > b then return Ordering.Greater
        }
        Ordering.Equal
    }
}
```

#### Hashing: `Hashable`

```ori
// Derived Hashable: FNV-1a across limbs
impl<T: Limbs> T: Hashable {
    @hash (self) -> int = {
        let $h = 0
        $for i in 0..T.$WIDTH do {
            h = hash_combine(seed: h, value: self.limb(index: i))
        }
        h
    }
}
```

### AIMS Interaction

AIMS sees `Limbs` types as `Value` — the entire AIMS pipeline already handles this optimally:

- **Uniqueness = Unique** for local values → in-place mutation of limbs
- **No RC operations** anywhere — `Value` supertrait guarantees this
- **FIP certification** possible for pure arithmetic functions — the compiler can certify that `add`, `mul`, etc. perform zero net allocations
- **COW is irrelevant** — no shared references to copy-on-write from

Functional-style code:
```ori
let $a = U256.from_int(42)
let $b = a + U256.from_int(58)  // 'a' not used after this line
```

AIMS proves `a` is consumed. The addition mutates `a`'s storage in place. The programmer wrote pure functional code. The emitted code is imperative.

### SIMD Integration

The `Limbs` trait's `$WIDTH` and `Limb` type directly inform SIMD instruction selection:

| `Limb` type | `$WIDTH` | Target | SIMD strategy |
|---|---|---|---|
| `int` (64-bit) | 2 | SSE2 | 1 × `paddq` (128-bit) |
| `int` (64-bit) | 4 | AVX2 | 1 × `vpaddq` (256-bit) |
| `int` (64-bit) | 8 | AVX-512 | 1 × `vpaddq` (512-bit) |
| `int` (64-bit) | 32 | AVX2 | 8 × `vpaddq` loop |
| `byte` (8-bit) | 32 | AVX2 | 1 × `vpaddb` (256-bit) |

The compiler maps `simd_add<T.Limb, N>` to the right instruction. The programmer never names an instruction — the type carries the information.

Runtime feature detection via `Intrinsics.cpu_has_feature(feature:)` allows the compiler to generate multi-versioned functions that select the widest available SIMD path at runtime, driven by the same `$WIDTH` information.

### Standard Library Surface

The `Limbs` trait enables a family of fixed-width integer types in `std.math`:

```ori
// std.math.fixed_int

pub type U128: Value, Limbs, Eq, Comparable, Hashable, Add, Sub, Mul, Div, Rem = {
    digits: [int, max 2],
}

pub type U256: Value, Limbs, Eq, Comparable, Hashable, Add, Sub, Mul, Div, Rem = {
    digits: [int, max 4],
}

pub type U512: Value, Limbs, Eq, Comparable, Hashable, Add, Sub, Mul, Div, Rem = {
    digits: [int, max 8],
}

pub type U1024: Value, Limbs, Eq, Comparable, Hashable, Add, Sub, Mul, Div, Rem = {
    digits: [int, max 16],
}

pub type U2048: Value, Limbs, Eq, Comparable, Hashable, Add, Sub, Mul, Div, Rem = {
    digits: [int, max 32],
}

pub type U4096: Value, Limbs, Eq, Comparable, Hashable, Add, Sub, Mul, Div, Rem = {
    digits: [int, max 64],
}
```

Each type is a one-line declaration. The compiler generates the entire optimized implementation from the trait declarations. No hand-written arithmetic, no platform-specific code, no FFI.

Users can also define their own:

```ori
// Domain-specific: 384-bit for P-384 elliptic curve
type P384Int: Value, Limbs, Eq, Comparable = {
    digits: [int, max 6],
}

// Game engine: 96-bit fixed-point with 32-bit fractional part
type FixedPoint96: Value, Limbs = {
    digits: [int, max 2],  // 1 integer limb + 1 fractional limb (custom interpretation)
}
```

### Common Operations

Beyond arithmetic traits, `Limbs` types support standard operations:

```ori
// Construction
let $a = U256.zero()                       // All limbs zero
let $b = U256.from_int(42)                 // From native int
let $c = U256.from_str("0xFF00FF00FF")     // From hex/decimal string

// Bitwise (derived from Limbs structure)
let $d = a & b                             // Limb-wise AND
let $e = a | b                             // Limb-wise OR
let $f = a ^ b                             // Limb-wise XOR
let $g = ~a                                // Limb-wise complement
let $h = a << 64                           // Cross-limb shift (limb shuffle + intra-limb shift)
let $i = a >> 1                            // Cross-limb shift right

// Inspection
let $bits = a.leading_zeros()              // CLZ across limbs
let $is_zero = a == U256.zero()
let $top = a.limb(index: 3)               // Direct limb access
```

Cross-limb shifts are derived from `$WIDTH` — shifting by a multiple of the limb bit-width becomes a limb shuffle (register rename), and the residual shift is intra-limb. The compiler generates this decomposition automatically.

### Widening Multiply (Future Extension)

The one operation that would significantly benefit `Limbs` arithmetic but is not yet available is widening multiply — 64×64→128-bit. This proposal recommends a companion intrinsic:

```ori
trait Intrinsics {
    // ... existing intrinsics ...

    /// Widening multiply: returns (low, high) halves of a × b.
    /// For int (64-bit): 64×64→128, returned as two 64-bit values.
    @widening_mul (a: int, b: int) -> (int, int)
}
```

Without this intrinsic, `Limbs` multiplication uses the standard split-into-halves approach (as mini-gmp and libtommath do). With it, the inner loop of schoolbook and Karatsuba multiplication operates at full hardware speed. This is a single intrinsic addition, not a language redesign.

---

## Semantic Rules

### Rule 1: Value Supertrait Required

A type implementing `Limbs` must also implement `Value`. Attempting to declare `Limbs` without `Value` is a compile error:

```ori
// Error E2050: Limbs requires Value supertrait
type Bad: Limbs = { digits: [int, max 4] }

// Correct
type Good: Value, Limbs = { digits: [int, max 4] }
```

### Rule 2: $WIDTH Must Be Positive

The associated constant `$WIDTH` must be a positive integer known at compile time:

```ori
// Error E2051: Limbs $WIDTH must be a positive compile-time integer
type Bad: Value, Limbs = { ... }  // where $WIDTH = 0
```

### Rule 3: Limb Type Must Be Value

The associated type `Limb` must satisfy `Value`. Non-value limb types are rejected:

```ori
// Error E2052: Limbs.Limb must satisfy Value
type Bad: Value, Limbs = { digits: [str, max 4] }  // str is not Value
```

### Rule 4: Derived Arithmetic Is Total

Arithmetic operations on `Limbs` types are modular (wrapping) by default — overflow wraps to zero, not panics. This matches the behavior of fixed-width hardware integers and GMP's `mpn_` layer. For checked variants:

```ori
let $a = U256.max_value()
let $b = a + U256.from_int(1)       // Wraps to zero (modular)
let $c = a.checked_add(U256.from_int(1))  // Returns None (overflow)
let $d = a.saturating_add(U256.from_int(1))  // Returns U256.max_value()
```

### Rule 5: Limb Ordering Is Little-Endian

Limb index 0 is the least significant. This is the universal convention in multi-word arithmetic (GMP, libtommath, Go math/big) and enables carry propagation from low to high.

### Rule 6: Size Limit Follows Value Trait

`Limbs` types inherit `Value`'s size limits: warning above 256 bytes, error above 512 bytes. This constrains `$WIDTH`:

| Limb type | Max $WIDTH (256B warning) | Max $WIDTH (512B hard limit) |
|---|---|---|
| `int` (8 bytes) | 32 (2048-bit) | 64 (4096-bit) |
| `byte` (1 byte) | 256 (2048-bit) | 512 (4096-bit) |

For larger multi-word integers, use heap-allocated `[int]` without the `Value`/`Limbs` traits.

---

## Prior Art

| Language/Library | Approach | Limb representation | Algorithm selection | Compiler integration |
|---|---|---|---|---|
| **GMP (C)** | Hand-tuned asm per arch | `mp_limb_t` (machine word) | Manual dispatch by size | None — opaque to compiler |
| **Rust `num-bigint`** | Portable Rust + unsafe | `Vec<u64>` (heap) | Runtime size check | None — generic library |
| **Go `math/big`** | Go + asm kernels | `[]Word` (slice) | Runtime size check | Minimal — escape analysis |
| **Haskell `Integer`** | GMP FFI | GMP's internal repr | GMP's internal dispatch | None — FFI boundary |
| **Zig `std.math.big`** | Comptime + SIMD | Fixed arrays | Comptime dispatch | Partial — comptime evaluation |
| **Ori `Limbs`** | **Trait-driven codegen** | **`[T, max N]` (inline)** | **Compile-time via `$WIDTH`** | **Full — AIMS + SIMD + unrolling** |

Ori's approach is unique: no other language uses a trait declaration to drive algorithm selection, SIMD width mapping, and memory model optimization simultaneously. Zig's comptime comes closest but lacks the unified memory model (AIMS) and trait-driven derivation.

---

## Examples

### Cryptography: 256-bit Arithmetic

```ori
use std.math.fixed_int { U256 }

@mod_exp (base: U256, exp: U256, modulus: U256) -> U256 = {
    let $result = U256.from_int(1)
    let $b = base % modulus
    let $e = exp
    while e > U256.zero() do {
        if e.limb(index: 0) & 1 == 1 then {
            result = (result * b) % modulus
        }
        e = e >> 1
        b = (b * b) % modulus
    }
    result
}
```

The entire function operates on stack-allocated `Value` types. No heap allocation, no ARC, no GC. AIMS certifies this as FIP — zero net allocations. The compiler selects SIMD-accelerated multiplication based on the `U256.$WIDTH` of 4.

### Blockchain: Hash Computation

```ori
use std.math.fixed_int { U256 }

type BlockHeader: Value, Eq = {
    prev_hash: U256,
    merkle_root: U256,
    timestamp: int,
    difficulty: U256,
    nonce: int,
}

@meets_difficulty (hash: U256, target: U256) -> bool = {
    hash <= target
}
```

`U256` comparison is MSB-first, derived automatically from `Limbs + Comparable`. No manual implementation needed.

### Scientific Computing: Extended Precision

```ori
use std.math.fixed_int { U128 }

// Exact 128-bit accumulator for summing many floats
@kahan_sum_exact (values: [float]) -> U128 = {
    let $sum = U128.zero()
    for v in values do {
        sum = sum + U128.from_float_bits(v)
    }
    sum
}
```

### Custom Domain: 384-bit for P-384 Elliptic Curve

```ori
type P384: Value, Limbs, Eq, Comparable, Add, Sub, Mul = {
    digits: [int, max 6],
}

@p384_field_mul (a: P384, b: P384, modulus: P384) -> P384 = {
    (a * b) % modulus
}
```

Six limbs. The compiler selects schoolbook multiplication (optimal for $WIDTH=6), unrolls the carry chain via `$for`, and SIMD-accelerates the reduction. One type declaration, zero hand-tuning.

---

## Implementation

### Phase 1: Trait Definition and Validation

- Define `Limbs` trait in prelude with `Limb`, `$WIDTH`, `limb`, `with_limb`, `limb_count`
- Type checker validates `Value` supertrait, positive `$WIDTH`, `Limb: Value`
- Error codes E2050–E2052
- **Tests**: validation of trait conformance, rejection of invalid declarations

### Phase 2: Derived Arithmetic

- Compiler-generated `Add`, `Sub` implementations with carry chains
- `$for` unrolling for small `$WIDTH`, loop emission for large
- `checked_add`/`wrapping_add` integration for carry detection
- **Tests**: correctness across all standard sizes (U128–U4096), edge cases (max + 1, zero, carry propagation across all limbs)

### Phase 3: Derived Comparison and Hashing

- Compiler-generated `Eq` (limb-wise), `Comparable` (MSB-first), `Hashable` (FNV-1a across limbs)
- **Tests**: ordering correctness, hash consistency with equality

### Phase 4: SIMD Integration

- Map `simd_add`/`simd_cmplt` to carry-chained vectorized arithmetic
- Automatic SIMD width selection based on `$WIDTH` and target capabilities
- Runtime multi-versioning via `cpu_has_feature`
- **Tests**: correctness parity between scalar and SIMD paths, cross-platform (x86-64, aarch64, wasm32)

### Phase 5: Multiplication Algorithms

- Schoolbook for `$WIDTH <= 4`
- Karatsuba for `$WIDTH <= 32`
- Toom-Cook-3 for larger
- **Tests**: multiplication correctness across algorithm boundaries, cross-size verification

### Phase 6: Standard Library Types

- `std.math.fixed_int` module with U128, U256, U512, U1024, U2048, U4096
- Bitwise operations, shifts, string conversion
- **Tests**: full arithmetic test matrix, string round-trip, bitwise operations

### Phase 7: Widening Multiply Intrinsic (Optional, Companion)

- `Intrinsics.widening_mul(a: int, b: int) -> (int, int)`
- LLVM codegen to target-specific widening multiply instructions
- Integration into `Limbs` schoolbook/Karatsuba inner loops
- **Tests**: widening multiply correctness, performance comparison with split-half approach

---

## Spec & Grammar Impact

- **Spec Clause 8 (Types)**: Add `Limbs` to marker trait inventory alongside `Value` and `Sendable`
- **Spec Clause 12 (Traits)**: Document `Limbs` trait definition, associated type and constant requirements
- **Spec Clause 20 (Capabilities)**: Add `widening_mul` to `Intrinsics` trait (Phase 7)
- **No grammar changes** — `Limbs` uses existing trait syntax and `$` const generic syntax

---

## Design Decisions

### Why a trait, not a built-in type?

A built-in `BigInt` type would be one specific implementation baked into the compiler. A trait lets users define their own sizes, their own limb types, their own representations — all getting the same compiler-driven optimization. This follows Ori's purity principle: the compiler provides the analysis machinery, the library provides the types.

### Why `Value` supertrait, not optional?

Multi-word arithmetic with ARC overhead is pointless — you'd spend more time on reference counting than on the actual computation. Making `Value` a hard requirement ensures the compiler can always generate optimal code. Users who need heap-allocated arbitrary-precision integers should use a separate `BigInt` type backed by `[int]`, which does not conform to `Limbs`.

### Why little-endian limb ordering?

Every major multi-word arithmetic library uses least-significant-limb-first ordering (GMP, libtommath, Go math/big, Rust num-bigint). Carry propagation naturally flows from low to high indices. Deviating would create gratuitous incompatibility with existing algorithms and FFI interop.

### Why modular (wrapping) arithmetic by default?

Fixed-width integers wrap on overflow in hardware (x86 `add`, ARM `add`). Ori's default `int` panics on overflow for safety, but multi-word integers are used in domains (cryptography, hashing, checksums) where wrapping is the correct semantics. Checked and saturating variants are available for domains that need them.

### Why compile-time algorithm selection, not runtime?

The `$WIDTH` is a compile-time constant. There is no reason to dispatch at runtime when the compiler knows the optimal algorithm at compile time. Runtime dispatch would add branch prediction overhead to every arithmetic operation for zero benefit. The `$if` compile-time branching ensures each monomorphized instance contains only the code it needs.

---

## Future Extensions

### Signed Multi-Word Integers

The `Limbs` trait as designed is unsigned. Signed variants (`I256`, `I512`, etc.) can be built on top with a sign field and two's-complement or sign-magnitude representation — both representable as a `Limbs` type plus a `bool` or a convention on the high limb.

### Arbitrary-Precision BigInt

A heap-allocated `BigInt` backed by `[int]` would not conform to `Limbs` (violates `Value` supertrait) but could use the same algorithm implementations internally. The compiler-generated algorithms for `Limbs` types could be extracted as generic functions usable by `BigInt`.

### Montgomery Multiplication

Cryptographic applications need modular multiplication in Montgomery form. This could be a trait extension:

```ori
trait MontgomeryLimbs: Limbs {
    let $MODULUS: Self
    let $MONT_R: Self
    @to_montgomery (self) -> Self
    @from_montgomery (self) -> Self
    @mont_mul (self, other: Self) -> Self
}
```

### Constant-Time Operations

Cryptographic code requires constant-time arithmetic (no data-dependent branches). A `ConstantTime` marker trait could compose with `Limbs` to instruct the compiler to use branchless implementations:

```ori
type CryptoU256: Value, Limbs, ConstantTime = { digits: [int, max 4] }
// Compiler generates cmov-based carry chains, no branches
```

---

## Summary Table

| Feature | Mechanism | Compiler action |
|---|---|---|
| No ARC | `Value` supertrait | AIMS skips all RC analysis |
| Inline storage | `[T, max N]` limbs | Register allocation, no heap |
| Algorithm selection | `$if T.$WIDTH` | Dead-code elimination per size |
| Loop unrolling | `$for i in 0..T.$WIDTH` | Compile-time expansion |
| SIMD acceleration | `simd_add<T.Limb, N>` | Target-specific instruction |
| In-place mutation | AIMS uniqueness proof | Functional code → imperative codegen |
| Carry detection | `checked_add` / `wrapping_add` | Safe, no unsafe needed |
| Comparison | Derived from `Limbs + Comparable` | MSB-first unrolled comparison |
| Hashing | Derived from `Limbs + Hashable` | FNV-1a across limbs |
