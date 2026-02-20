# Proposal: Representation Optimization

**Status:** Approved
**Author:** Eric
**Created:** 2026-02-19
**Approved:** 2026-02-19

---

## Summary

Formalize the compiler's freedom to optimize the machine representation of types during lowering. The programmer writes `int`, `float`, and other high-level types; the compiler selects the most efficient machine representation that preserves the type's semantic guarantees.

```ori
// The programmer writes:
let count = 0
let flags: [bool] = [true, false, true]
let status = Less

// The compiler may lower to:
// count  -> i32 (if provably within i32 range)
// [bool] -> packed i1 array
// status -> i8 (only 3 possible values)
```

This proposal does **not** add new types to the language. It codifies behavior the compiler already performs and establishes the principle that representation is a compiler concern, not a language concern.

---

## Motivation

### The Problem

Ori's type spec currently states:

> `int` — 64-bit signed integer
> `float` — 64-bit IEEE 754

This conflates two distinct things:

1. **Semantic contract** — the range of values a type can hold, its overflow behavior, its arithmetic properties
2. **Machine representation** — the number of bits used in registers, memory, and generated code

If the spec says `int` _is_ 64 bits, then every conforming implementation must use 64 bits everywhere — even for a loop counter that never exceeds 100, or an enum discriminant with only 3 values. This forecloses optimization opportunities that every major compiler exploits.

### What the Compiler Already Does

The Ori compiler already optimizes representations during LLVM lowering. These optimizations are scattered across the codegen with no spec backing:

| Ori Type | Semantic Width | LLVM Representation | Savings | Location |
|----------|---------------|---------------------|---------|----------|
| `bool` | 1 value bit | `i1` | 63 bits vs i64 | `type_info/mod.rs` |
| `byte` | 8 bits | `i8` | 56 bits vs i64 | `type_info/mod.rs` |
| `char` | 21 bits (Unicode) | `i32` | 32 bits vs i64 | `type_info/mod.rs` |
| `Ordering` | 2 bits (3 values) | `i8` | 56 bits vs i64 | `type_info/mod.rs` |
| Enum tags | log₂(variants) bits | `i8` | 56 bits vs i64 | `type_info/mod.rs` |
| All-unit enums | tag only | `i8` (no payload) | Entire payload eliminated | `type_info/mod.rs` |
| `Result<T, E>` | max(T, E) | Shared payload slot | Smaller variant padded | `lower_error_handling.rs` |
| Trivial ARC types | N/A | No retain/release | Entire ARC overhead eliminated | `type_info/mod.rs` |
| Range inclusive flag | 1 bit | `i1` | 63 bits vs i64 | `type_info/mod.rs` |

These are _ad hoc_ implementation decisions. Without a spec, there is no way to know:
- Which optimizations are intentional vs accidental
- What invariants they must preserve
- Whether they can change between compiler versions
- How they interact with each other

### The Ori Way

Ori's design philosophy favors:

1. **Simple mental model** — Programmers think in `int`, `float`, `bool`. No `i8`/`i16`/`i32`/`i64`/`u8`/`u16`/`u32`/`u64`/`f32`/`f64` matrix.
2. **Compiler does the work** — The compiler is smart so the programmer doesn't have to be. Type inference, ARC, tail call optimization — all are compiler responsibilities.
3. **No low-level escape hatches** — Ori is not a systems language. The programmer should not need to reason about bit widths.

Representation optimization follows this pattern: the programmer declares _intent_ (an integer, a boolean, a three-valued ordering), and the compiler selects the _implementation_.

### Prior Art

| Language | Approach | Documentation Level |
|----------|----------|-------------------|
| **JavaScript (V8)** | Single `number` type (f64 semantically); engine uses Smis (i31) internally for small integers | Implementation detail; no spec |
| **Java (HotSpot)** | `int` is always 32 bits in language, but JIT may use registers of any width | JVM spec allows; no observable guarantees |
| **Haskell (GHC)** | Unboxing optimization — `Int` in a strict field may be stored as raw machine int | Documented as compiler optimization |
| **Swift** | Formal "type lowering" from AST types to SIL types; documented in SIL spec; resilient types have runtime-determined layout | Part of SIL specification |
| **Rust** | `repr(Rust)` allows field reordering and niche filling; `repr(C)` locks C-compatible layout; niche optimization encodes enum discriminants in unused bit patterns | Partially documented via `repr` attributes |
| **Lean 4** | Explicit boxing/unboxing IR pass; scalars stay unboxed, heap values get boxed; RC insertion follows boxing decisions | Documented as IR transformation pipeline |
| **Koka** | Explicit `value struct` vs `reference struct` declarations; compiler reorders fields for alignment; warns on large value types | Programmer-controlled with compiler guidance |
| **Go** | Layout is part of the language spec; no field reordering; string/slice layouts guaranteed | Fully specified; no optimization freedom |
| **LLVM** | `InstCombine` and `TruncInstCombine` passes narrow operations when upper bits are provably unused | Pass documentation |

Ori's approach is closest to **Swift's type lowering concept**: the language has a clean surface type system, and the compiler's lowering phase transforms it into an efficient machine representation. The key difference is that Ori makes this transformation fully opaque to the programmer (no `@box`, no `indirect`, no `resilient`), while Swift exposes some control.

---

## Design

### Principle: Semantic Equivalence (The As-If Rule)

The compiler may use any machine representation that is **semantically equivalent** to the specified type. Two representations are semantically equivalent if and only if all three conditions hold:

1. **Range preservation** — Every value representable in the semantic type is representable in the machine type. The machine type may represent _more_ values (e.g., `i32` can hold values outside `byte`'s [0, 255] range), but every semantic value must round-trip exactly.

2. **Operation preservation** — All operations produce bit-identical results:
   - Arithmetic operations produce the same value or the same panic
   - Comparison operations produce the same `Ordering`
   - Equality operations produce the same `bool`
   - Hashing operations produce the same `int`
   - Conversion operations (`as`, `as?`, `.into()`) produce the same result
   - String formatting produces the same output
   - Pattern matching takes the same branch

3. **No observable distinction** — No conforming Ori program can distinguish the optimized representation from the canonical one through any language-level operation. This explicitly excludes:
   - FFI (which operates outside the language)
   - Debugger memory inspection
   - Compiler diagnostics (which may mention representation details)

This is Ori's version of the **as-if rule**: the compiler may transform the representation as long as observable behavior is identical.

### Canonical Representations

Each type has a **canonical representation** that defines its semantic contract. The canonical representation is the _maximum_ representation — it defines the full range of values and the precision of operations:

| Type | Canonical Representation | Semantic Contract |
|------|------------------------|-------------------|
| `int` | 64-bit signed two's complement | Range: [-2⁶³, 2⁶³ - 1]. Overflow panics. Bitwise operations treat as 64 unsigned bits. |
| `float` | 64-bit IEEE 754 binary64 | ~15-17 significant digits. IEEE 754 semantics including ±0.0, ±inf, NaN. |
| `bool` | 1-bit boolean | Exactly two values: `true` (1), `false` (0). |
| `byte` | 8-bit unsigned | Range: [0, 255]. Overflow panics. |
| `char` | 32-bit Unicode scalar value | Range: U+0000–U+10FFFF, excluding surrogates U+D800–U+DFFF. |
| `Duration` | 64-bit signed (nanoseconds) | Range: [-2⁶³, 2⁶³ - 1] nanoseconds (~±292 years). |
| `Size` | 64-bit signed, non-negative (bytes) | Range: [0, 2⁶³ - 1] bytes (~8 exabytes). Overflow panics. |
| `Ordering` | Tri-state enumeration | Exactly three values: `Less` (0), `Equal` (1), `Greater` (2). |

The canonical representation establishes the **semantic ceiling**. The compiler must never:
- Narrow a type such that a valid semantic value becomes unrepresentable
- Change overflow boundaries (overflow is defined by the semantic range, not the machine range)
- Alter comparison or equality results
- Change the output of `debug()`, `print()`, or string interpolation

---

## Optimization Tiers

### Tier 1: Type-Intrinsic Narrowing

Type-intrinsic narrowing derives the machine representation from the type's definition alone — no usage analysis required. These optimizations are **always valid** because the semantic range of the type fits entirely within the narrowed representation.

| Type | Unoptimized Width | Narrowed | Rationale |
|------|-------------------|----------|-----------|
| `bool` | 64-bit | `i1` | Only 2 values; `i1` is LLVM's native boolean |
| `byte` | 64-bit | `i8` | Range [0, 255] fits in 8 bits by definition |
| `char` | 64-bit | `i32` | Unicode scalar range [0, 0x10FFFF] fits in 21 bits; `i32` is the natural machine word |
| `Ordering` | 64-bit | `i8` | Only 3 values (0, 1, 2); `i8` is the minimum addressable unit |
| `void` / `()` | N/A | Zero-sized or `i64(0)` | No information content; LLVM requires non-void for phi nodes |

**Alignment implications:**

| Narrowed Type | Natural Alignment | Storage Alignment |
|--------------|-------------------|-------------------|
| `i1` (bool) | 1 byte | 1 byte |
| `i8` (byte, Ordering) | 1 byte | 1 byte |
| `i32` (char) | 4 bytes | 4 bytes |
| `i64` (int, float, Duration, Size) | 8 bytes | 8 bytes |

**Status:** All Tier 1 optimizations are currently implemented in `ori_llvm/src/codegen/type_info/mod.rs`.

### Tier 2: Layout Optimizations

Layout optimizations change the memory structure of composite types. They require analysis of the type's fields but not of how values are used at runtime.

#### 2a. Enum Discriminant Narrowing

The discriminant (tag) of an enum type is narrowed to the minimum integer width that can represent all variant indices:

| Variant Count | Minimum Tag Width | Tag Type |
|--------------|-------------------|----------|
| 1–256 | 8 bits | `i8` |
| 257–65536 | 16 bits | `i16` |
| 65537–2³² | 32 bits | `i32` |

**Current implementation:** All enum tags use `i8`. This is correct for all current Ori enums (which have far fewer than 256 variants) but must be generalized if const-generic enum generation is added.

**Layout formula:**

```
Enum layout = { tag: i8, payload: [ceil(max_variant_bytes / 8) × i64] }
```

If `max_variant_bytes == 0` (all-unit enum), the payload array is omitted entirely.

#### 2b. All-Unit Enum Elimination

Enums where every variant carries no payload are represented as the tag alone:

```ori
type Color = Red | Green | Blue
// Layout: { tag: i8 }  (1 byte, no payload array)

type Status = Pending | Running | Done | Failed(reason: str)
// Layout: { tag: i8, payload: [2 × i64] }  (str = 16 bytes → 2 i64s)
```

**Correctness constraint:** Pattern matching on the tag byte must use the correct discriminant values. The variant-to-index mapping is declaration order (0, 1, 2, ...).

#### 2c. Sum Type Payload Sharing

For sum types with multiple payload variants, the payload slot uses the **maximum** payload size across all variants:

```ori
type Result<T, E> = Ok(T) | Err(E)

// For Result<int, str>:
//   Ok(int)  → 8 bytes payload
//   Err(str) → 16 bytes payload
//   Shared payload: 16 bytes (max)
//   Layout: { tag: i8, payload: [2 × i64] }
```

**Value coercion at boundaries:** When storing a smaller variant into the shared payload slot, the compiler:
1. Allocates a stack temporary of the payload type
2. Zero-initializes the temporary (preventing garbage bits)
3. Stores the smaller value at the beginning
4. Loads back as the payload type

This is implemented in `lower_error_handling.rs` using the alloca+store+load pattern.

**Correctness constraint:** Zero-initialization prevents reading uninitialized memory when the smaller variant's padding bytes are accessed (e.g., through a union-style reinterpretation in LLVM IR).

#### 2d. ARC Elision (Transitive Triviality)

Types with no reference-counted fields — checked transitively through all nested types — skip all retain/release operations.

**Triviality classification:**

| Category | Trivial? | Examples |
|----------|----------|---------|
| Scalar primitives | Always | `int`, `float`, `bool`, `char`, `byte`, `Duration`, `Size`, `Ordering` |
| `void`, `Never` | Always | Unit and bottom types have no runtime data |
| `Range` | Always | Contains only scalars (`start: int`, `end: int`, `inclusive: bool`) |
| `str` | Never | Heap-allocated UTF-8 buffer |
| `[T]`, `Set<T>`, `{K: V}` | Never | Heap-allocated collections |
| `(T) -> R` (closures) | Never | May capture heap-allocated values |
| `Channel` types | Never | Runtime-managed concurrent state |
| `Option<T>` | If `T` trivial | `Option<int>` trivial; `Option<str>` non-trivial |
| `Result<T, E>` | If both trivial | `Result<int, int>` trivial; `Result<int, str>` non-trivial |
| `(T₁, T₂, ...)` | If all trivial | `(int, float)` trivial; `(int, str)` non-trivial |
| User structs | If all fields trivial | `Point { x: int, y: int }` trivial; `User { name: str }` non-trivial |
| User enums | If all variant payloads trivial | `Color` trivial; `Status` with `Failed(str)` non-trivial |
| Recursive types | Never | Cycle detection terminates conservatively |

**Implementation:** Two-level check:
1. `TypeInfo::is_trivial()` — per-type conservative check (scalars yes, compounds conservatively no)
2. `TypeInfoStore::is_trivial()` — transitive check that walks children with cycle detection

**Correctness constraint:** Conservative over-classification (marking a trivial type as non-trivial) wastes retain/release cycles but is safe. Aggressive under-classification (marking a non-trivial type as trivial) causes use-after-free. The cycle detection guard ensures recursive types default to non-trivial.

#### 2e. Newtype Erasure

Newtypes have zero runtime overhead. The compiler erases the wrapper:

```ori
type UserId = int
// UserId has identical layout to int
// No wrapper struct, no indirection
```

This is already specified in [Types § Newtype](06-types.md#newtype) ("Newtypes have zero runtime overhead. They share the same memory layout as their underlying type; the compiler erases the wrapper"). This proposal merely notes it as part of the representation optimization framework.

#### 2f. Struct Field Reordering

The compiler may reorder struct fields to minimize alignment padding:

```ori
type Mixed = { flag: bool, value: int, tag: byte }
// Declaration order: bool (1 byte), int (8 bytes), byte (1 byte)
// → 1 + 7 padding + 8 + 1 + 7 padding = 24 bytes

// Optimized order: int (8 bytes), bool (1 byte), byte (1 byte)
// → 8 + 1 + 1 + 6 padding = 16 bytes
```

**Constraint on reordering:**
- Initialization must still use declaration order (field names resolve this — Ori uses named fields, not positional)
- Destruction must still follow reverse declaration order (the compiler maps destruction order to the reordered layout)
- Pattern matching destructuring must still use declaration names
- `debug()` output must still show declaration order
- Derived trait implementations (`Eq`, `Comparable`, `Hashable`, `Debug`) iterate fields in declaration order — the codegen must maintain a mapping from declaration order to memory order for derived method dispatch (see [Derived Traits Proposal](../approved/derived-traits-proposal.md))

**Status:** Not currently implemented. This optimization is permitted but not required.

### Tier 3: Value Range Narrowing (Future)

These optimizations narrow `int` or `float` based on provable value ranges. They are the most aggressive and require the most careful implementation.

#### 3a. Integer Narrowing

If the compiler can prove that an `int` value always falls within a smaller range, it may use a narrower machine type:

| Provable Range | Permitted Narrowing |
|---------------|-------------------|
| [0, 1] | `i1` (boolean-like) |
| [-128, 127] | `i8` |
| [0, 255] | `i8` (unsigned context) |
| [-32768, 32767] | `i16` |
| [-2³¹, 2³¹ - 1] | `i32` |
| Anything wider | `i64` (canonical) |

**Sources of range information** (informative — illustrates what LLVM can deduce):
- Literal constants (e.g., `42` is provably in i8 range)
- Range iteration bounds (e.g., `for i in 0..100`)
- Pattern match guards (e.g., `match x { 0 -> ..., 1 -> ..., 2 -> ... }`)
- Bitwise masks (e.g., `x & 0xFF` is provably in [0, 255])
- Return types of known functions (e.g., `len()` returns non-negative)

**Critical constraint — overflow boundary:** If LLVM narrows `int` to `i32`, it must ensure that:
- Overflow detection fires at the `int` boundary (2⁶³ - 1), not the `i32` boundary (2³¹ - 1)
- OR the value has been proven to stay within the narrowed range

In practice, this means narrowing is only valid when there is a **complete range proof**, not merely a heuristic. If the value _might_ exceed the narrowed range, the canonical representation must be used.

**Implementation approach:** Ori does not perform its own value range analysis. Instead, Ori emits LLVM IR annotated with metadata that enables LLVM's `TruncInstCombine` and `InstCombine` passes to perform conservative narrowing:
- `nsw` (no signed wrap) and `nuw` (no unsigned wrap) flags on arithmetic where overflow has been checked
- `range` metadata on loads to annotate known value ranges
- `dereferenceable` and `nonnull` attributes on pointers

This division of responsibility avoids duplicating LLVM's sophisticated data flow analysis while still enabling aggressive optimization. Ori's overflow checks naturally provide the `nsw`/`nuw` information that LLVM needs.

#### 3b. Float Narrowing

The compiler may use `f32` when it can prove no precision loss:

| Condition | Narrowing Permitted? |
|-----------|---------------------|
| Float literal that is exactly representable as f32 | Yes |
| Result of int-to-float conversion where int ≤ 2²⁴ | Yes (f32 has 24-bit mantissa) |
| Arbitrary float arithmetic | Generally no — precision loss accumulates |

**Critical constraint:** IEEE 754 double-precision (f64) and single-precision (f32) produce different results for the same arithmetic operations due to different rounding boundaries. Float narrowing is only safe for values that are exactly representable in both formats.

**Status:** Not currently implemented. Float narrowing is lower priority than integer narrowing due to the precision complexity.

**Implementation approach:** Like integer narrowing, float narrowing is delegated to LLVM passes. Ori's role is to emit IR that preserves IEEE 754 semantics (no `fast-math` flags by default), which constrains LLVM to only perform safe float optimizations.

#### 3c. Array Element Packing

For homogeneous collections where all elements have a narrowed representation:

```ori
let bytes: [byte] = [1, 2, 3, 4]
// Elements stored as i8, not i64 → 4× memory reduction

let flags: [bool] = [true, false, true, false]
// Elements stored as i1 → 64× memory reduction (before LLVM padding)
```

**Already partially implemented:** `[byte]` uses `i8` elements. `[bool]` uses `i1` elements in LLVM IR (LLVM may further pack or pad based on target).

#### 3d. Bit Packing

Multiple sub-byte values may be packed into a single machine word:

```ori
type Flags = { read: bool, write: bool, execute: bool }
// Canonical: 3 × 8 bytes = 24 bytes
// Packed: 3 bits in 1 byte (or 1 i8)
```

**Status:** Not implemented. This overlaps with struct field reordering (Tier 2f) and requires alignment-aware layout computation.

---

## Cross-Cutting Concerns

Representation optimization touches nearly every compiler phase. This section documents each interaction point, its current behavior, and invariants that must hold.

### Interaction 1: Overflow Checking

**Spec reference:** [Overflow Behavior Proposal](../approved/overflow-behavior-proposal.md)

Ori panics on integer overflow in both debug and release builds. Representation narrowing must not change when overflow fires.

**Rule:** Overflow is defined by the **semantic type's range**, not the machine representation.

```ori
let x: int = int.max  // 9223372036854775807
let y = x + 1         // MUST panic: int overflow
```

If the compiler had narrowed `x` to `i32`, it would overflow at 2³¹ - 1 (2147483647) instead of 2⁶³ - 1. This changes observable behavior and violates semantic equivalence.

**Implementation strategy:**
- Tier 1/2 optimizations do not affect `int` — they narrow types that already have smaller canonical ranges (`bool`, `byte`, `char`, `Ordering`)
- Tier 3 integer narrowing is only valid when the compiler proves the value stays within the narrowed range. If the proof fails, the value stays at i64.
- LLVM's `TruncInstCombine` respects `nsw`/`nuw` flags and only narrows when safe

**Invariant:** `int` arithmetic must produce the same result regardless of machine representation. A narrowed `i32` that silently wraps at 2³¹ when the semantic value should have panicked at 2⁶³ is a compiler bug.

### Interaction 2: ARC (Reference Counting)

**Spec reference:** [Memory Model](../../0.1-alpha/spec/15-memory-model.md)

ARC operations (retain/release) are elided for **transitively trivial** types — types whose entire closure of nested types contains no heap-allocated data.

**Rule:** ARC classification is determined by **semantic type structure**, not by machine representation.

Narrowing `int` from i64 to i32 does not change its triviality (it's always trivial). Narrowing an enum discriminant from i64 to i8 does not change the enum's triviality (which depends on variant payloads, not the tag).

**Invariant:** The set of types classified as trivial must be identical regardless of representation optimization level. A type that is trivial with i64 fields must remain trivial with i32 fields, and vice versa.

**Implementation:** `TypeInfoStore::is_trivial()` walks child `TypeInfo` nodes, checking each against a known-trivial list. The walk is representation-agnostic because it checks the `TypeInfo` variant (e.g., `TypeInfo::Int`), not the underlying LLVM type.

### Interaction 3: ABI Passing Convention

**Spec reference:** [System Considerations § Platform Support](../../0.1-alpha/spec/22-system-considerations.md)

The compiler decides whether to pass function arguments and return values **directly** (in registers) or **indirectly** (via pointer):

| Canonical Size | Passing Mode |
|---------------|-------------|
| ≤ 16 bytes | Direct (registers) |
| > 16 bytes | Indirect (sret pointer) |

**Rule:** ABI classification uses the **actual machine size** after representation optimization, not the canonical size.

This is intentional: the whole point of narrowing is to make values smaller, and passing smaller values in registers is a direct benefit. A struct with two `char` fields (2 × 4 bytes = 8 bytes after narrowing) should pass in a single register, not via pointer.

**Invariant:** The ABI classification must be computed from the lowered layout, not the semantic layout. Two semantically identical types with different representation optimizations may use different passing conventions. This is not observable within the language (only via FFI, which has its own rules).

**Known gap — struct padding:** The current ABI size computation (`abi/mod.rs`) sums field sizes without accounting for alignment padding between fields. For a struct `{ flag: bool, value: int }`:
- Sum: 1 + 8 = 9 bytes → Direct
- Actual layout with padding: 1 + 7 padding + 8 = 16 bytes → Direct (correct by coincidence)
- But for `{ a: bool, b: int, c: bool }`: sum = 10, actual = 24 → **misclassified**

This gap exists independently of this proposal but becomes more impactful when fields have varied widths due to narrowing. The ABI size computation should use actual layout with padding, not a naive sum.

### Interaction 4: Alignment

Each type has a natural alignment determined by its machine representation:

| Machine Type | Alignment |
|-------------|-----------|
| `i1` (bool) | 1 byte |
| `i8` (byte, Ordering) | 1 byte |
| `i32` (char) | 4 bytes |
| `i64` (int, float, Duration, Size) | 8 bytes |
| `f64` (float) | 8 bytes |
| pointers | 8 bytes (on 64-bit targets) |

**Rule:** Alignment is determined by the **machine representation**, not the canonical type.

**Impact on struct layout:** If `int` fields are narrowed to `i32`, their alignment drops from 8 to 4 bytes, reducing padding in structs:

```
// Canonical layout of { flag: bool, x: int, y: int }:
//   flag: 1 byte at offset 0
//   padding: 7 bytes
//   x: 8 bytes at offset 8
//   y: 8 bytes at offset 16
//   Total: 24 bytes, alignment 8

// Narrowed layout (if x, y proven ≤ i32 range):
//   flag: 1 byte at offset 0
//   padding: 3 bytes
//   x: 4 bytes at offset 4
//   y: 4 bytes at offset 8
//   Total: 12 bytes, alignment 4
```

**Invariant:** Alignment must be consistent within a single compilation unit. If a type is narrowed in one function, it must be narrowed identically in all functions within the same module (otherwise, struct field offsets would disagree).

**Known gap:** The current `TypeInfo::alignment()` method returns hardcoded values based on the `TypeInfo` variant, not the actual machine width. If Tier 3 narrowing is implemented, alignment must become representation-aware.

### Interaction 5: Value vs. Reference Classification

**Spec reference:** [Memory Model § Value vs Reference Types](../../0.1-alpha/spec/15-memory-model.md#value-vs-reference-types)

The memory model classifies types as:

| Classification | Criteria | Semantics |
|---------------|----------|-----------|
| Value | ≤32 bytes AND primitives only | Copied on assignment |
| Reference | >32 bytes OR contains references | Shared, reference counted |

**Rule:** Value/reference classification uses the **canonical representation**, not the narrowed representation.

This prevents a type from changing classification based on optimization level:

```ori
type BigStruct = { a: int, b: int, c: int, d: int, e: int }
// Canonical: 5 × 8 = 40 bytes → Reference type
// If narrowed: 5 × 4 = 20 bytes → Would become Value type
// → Classification MUST stay Reference (based on canonical)
```

**Invariant:** A type's value/reference classification is a semantic property determined at type-checking time, not a codegen-time decision. Representation optimization must not change it.

### Interaction 6: Equality, Comparison, and Hashing

**Spec reference:** [Types § Comparable, Hashable](../../0.1-alpha/spec/06-types.md)

Equality, comparison, and hashing must produce identical results regardless of representation.

**For primitive types:**

| Operation | Implementation | Narrowing-Safe? |
|-----------|---------------|-----------------|
| `int == int` | Bitwise comparison of `i64` values | Yes — narrowed values zero/sign-extend to same bits |
| `float == float` | IEEE 754 comparison (NaN ≠ NaN) | Yes — narrowing only when exact |
| `Ordering == Ordering` | Compare `i8` tag values | Yes — tag values are canonical |

**For compound types:**

Equality and hashing recurse through fields. Each field is compared/hashed using its own type's operations. Representation narrowing of a field does not affect the comparison because:
1. Comparisons operate on semantic values, not bit patterns
2. Hash functions operate on semantic values (e.g., `hash_value()` calls `.raw()` to get the i64)

**Invariant:** `hash(x) == hash(y)` whenever `x == y`, regardless of how `x` and `y` are represented in memory. The hash function must operate on the **semantic value**, not the **machine representation**.

**Current implementation:** `ScalarInt` stores the canonical `i64` value. Hash uses `.raw()` which returns `i64`. This is correct — even if the LLVM representation is `i32`, the evaluator and hash function see the full `i64`.

### Interaction 7: Type Conversions

**Spec reference:** [Types § Conversion Traits](../../0.1-alpha/spec/06-types.md#conversion-traits)

Conversions (`as`, `as?`, `.into()`) must produce the same result regardless of representation.

**Conversion matrix and narrowing impact:**

| Conversion | Canonical Codegen | Narrowed Codegen | Notes |
|-----------|------------------|-----------------|-------|
| `int` → `float` | `si_to_fp(i64, f64)` | `si_to_fp(i32, f64)` | Both produce same f64 value |
| `float` → `int` (via `truncate()`) | `fp_to_si(f64, i64)` | `fp_to_si(f64, i32)` | **Dangerous** — `i32` truncation may lose high bits |
| `byte` → `int` | `sext(i8, i64)` | `sext(i8, i32)` | Both produce same semantic value |
| `int` → `byte` | `trunc(i64, i8)` | `trunc(i32, i8)` | Both produce same `i8` |
| `char` → `int` | `sext(i32, i64)` | `sext(i32, i32)` (no-op) | Narrowed int = same width as char |
| `int` → `char` | `trunc(i64, i32)` | identity (i32 → i32) | No truncation needed |
| `bool` → `int` | `zext(i1, i64)` | `zext(i1, i32)` | Both produce 0 or 1 |

**Invariant:** `float` → `int` conversion (via `truncate()`, `round()`, `floor()`, `ceil()`) must produce the full `int`-range result. If `int` is narrowed to `i32`, the conversion must widen back to `i64` first OR the compiler must prove the float value fits in `i32`.

**Known gap:** The current `emit_cast()` in `lower_operators.rs` dispatches on `Idx` (semantic type) and emits for fixed widths. If Tier 3 narrowing is implemented, casts must account for the actual machine width of the source and target.

### Interaction 8: Pattern Matching

Pattern matching on narrowed types must produce identical branch decisions.

**For integer patterns:**

```ori
match x {
    0 -> "zero",
    1 -> "one",
    _ -> "other"
}
```

Pattern constants (0, 1) are emitted at the same width as the scrutinee. If `x` is narrowed to `i32`, the constants become `i32 0` and `i32 1`. The comparison result is identical.

**For enum patterns:**

```ori
match color {
    Red -> ...,
    Green -> ...,
    Blue -> ...,
}
```

The discriminant is loaded as `i8` (Tier 2a) and compared against variant indices. This is already representation-optimized and works correctly.

**Invariant:** Pattern matching is semantics-driven (compare against constant values), not representation-driven. Branch decisions must be identical regardless of the machine width of the scrutinee.

### Interaction 9: Constant Evaluation

**Spec reference:** [Constant Expressions](../../0.1-alpha/spec/21-constant-expressions.md)

Constants are evaluated at compile time using the canonical representation:

```ori
$MAX = int.max            // 9223372036854775807
$SHIFT = 1 << 40          // 1099511627776
$COMBINED = $MAX - $SHIFT // 9223372035755264031
```

**Rule:** Constant evaluation always uses the canonical (full-width) representation. Narrowing is a codegen concern, not a semantic concern.

**Invariant:** A constant expression must produce the same value whether evaluated by the interpreter (using `ScalarInt(i64)`) or by the codegen (using potentially narrowed LLVM IR). The codegen must widen constants to the canonical width for evaluation, then narrow for storage.

### Interaction 10: Debug and Print Output

`debug()` and `print()` must display the **semantic value**, not the machine representation.

```ori
let x: int = 42
print(x)       // "42" — not "42i32" or "0x0000002A"
x.debug()      // "42" — not "Int32(42)"
```

**Implementation:** The evaluator formats `Value::Int(ScalarInt)` using `.raw()` → `i64` → decimal string. The LLVM codegen calls runtime formatting functions that accept the semantic type. Neither path is affected by representation narrowing because:
1. In the evaluator: `ScalarInt` always stores `i64`
2. In the codegen: formatting functions accept the canonical width (the narrowed value is widened before formatting)

**Invariant:** The string output of `debug()`, `print()`, and string interpolation must be identical for the same semantic value, regardless of machine representation.

### Interaction 11: FFI Boundaries

**Spec reference:** [FFI](../../0.1-alpha/spec/24-ffi.md)

FFI operates outside Ori's semantic equivalence guarantee. At FFI boundaries:

**Rule:** Types must use their **canonical representation** unless the FFI declaration explicitly specifies a different representation.

```ori
// FFI function expects C's int32_t
@extern "C" @add_i32 (a: int, b: int) -> int
// Compiler must pass i64 (canonical) unless the FFI declaration
// specifies a width override
```

This ensures interoperability with C and other languages. FFI is the **one place** where representation is observable, and it's governed by its own rules, not by this proposal.

**Invariant:** Representation optimization must never change the ABI visible at FFI boundaries. FFI calling conventions use the canonical type width (or an explicitly specified width), regardless of internal optimization.

### Interaction 12: WASM Target

WebAssembly has different representation constraints than native targets:

| Concern | x86-64 | WASM |
|---------|--------|------|
| Integer narrowing | i32 is a natural fit | i32 is WASM's default integer |
| Float narrowing | f32 is hardware-supported | f32 is a separate WASM type |
| Alignment | Hardware-enforced | Mostly software-enforced |
| Pointer size | 8 bytes | 4 bytes (wasm32) or 8 bytes (wasm64) |

**Rule:** Representation optimization decisions may differ between targets. A `char` stored as `i32` on both x86-64 and WASM has different alignment implications (x86-64: 4-byte aligned in 8-byte-aligned structs; WASM32: naturally 4-byte aligned).

**Invariant:** The semantic behavior of a program must be identical across targets. Only the machine representation (and therefore performance characteristics) may differ.

### Interaction 13: Repr Attributes

**Spec reference:** [Repr Extensions Proposal](../approved/repr-extensions-proposal.md)

The `#repr` attribute gives the programmer explicit control over memory layout. When a type has a `#repr` attribute, representation optimization is **constrained** by the attribute's requirements.

**Rule:** `#repr` declarations take precedence over representation optimization. The compiler must not perform any optimization that violates the layout guarantees of a `#repr` attribute.

| Repr Attribute | Effect on Optimization |
|----------------|----------------------|
| `#repr("c")` | Fields locked to declaration order. Alignment follows C ABI rules. Tier 2f (field reordering) is prohibited. Tier 1 narrowing of individual fields still applies. |
| `#repr("packed")` | Fields locked to declaration order with no padding. Tier 2f prohibited. Tier 1 narrowing applies (a `bool` field in a packed struct is still `i1`). |
| `#repr("transparent")` | The struct has identical layout and ABI to its single field. Optimization of the inner type applies normally — the wrapper is erased. |
| `#repr("aligned", N)` | Minimum alignment is N. Tier 2f (field reordering) is permitted within the alignment constraint. All other optimizations apply. |

**Invariant:** A type's `#repr` attribute is a programmer contract. Representation optimization operates *within* the constraints of this contract, never in violation of it. Types without `#repr` attributes use Ori's default layout, which the compiler is free to optimize.

---

## Guarantees and Non-Guarantees

### Guarantees (Normative)

Programs may rely on these properties:

1. **Semantic range preservation** — `int` always supports [-2⁶³, 2⁶³ - 1]. `byte` always supports [0, 255]. `char` always supports all Unicode scalar values. No operation within the semantic range produces a different result due to representation optimization.

2. **Overflow boundary stability** — Overflow panics fire at the semantic boundary, not the machine boundary. `int.max + 1` always panics, even if the compiler uses `i32` internally for that particular variable.

3. **Equality reflexivity** — `x == x` is always `true` (except for `float` NaN, per IEEE 754). Representation narrowing does not break reflexivity.

4. **Hash consistency** — `x == y` implies `hash(x) == hash(y)`, regardless of how `x` and `y` are represented.

5. **Round-trip fidelity** — Storing a value and reading it back always produces the original value. `let y = x; assert_eq(left: x, right: y)` never fails due to representation.

6. **Format stability** — `debug(x)`, `print(x)`, and `"{x}"` produce the same string output for the same semantic value, regardless of representation.

7. **Deterministic behavior** — The same program with the same inputs produces the same outputs. Representation optimization does not introduce non-determinism.

8. **Value/reference stability** — A type's value/reference classification (per the memory model) does not change based on representation optimization.

### Non-Guarantees (Informative)

Programs must not rely on these properties:

1. **Specific machine type** — The compiler may use `i1`, `i8`, `i16`, `i32`, or `i64` for any type at its discretion.

2. **Representation stability across versions** — A future compiler version may narrow (or widen) a representation differently.

3. **Representation consistency across targets** — The same type may have different machine representations on x86-64 vs WASM vs ARM64.

4. **Struct field layout** — Fields may be reordered, padded, or packed differently across compiler versions and targets.

5. **Size of types** — The size in bytes of any Ori type is unspecified (and is not exposed to programs). If a future `sizeof` operator is added, it will reflect the machine size, not the canonical size, and will be explicitly documented as target-dependent.

6. **Memory layout at FFI boundaries** — Ori types passed through FFI use canonical representations by default, but this may change with explicit FFI width annotations.

---

## Relationship to LLVM Optimization Passes

Ori's representation optimization and LLVM's optimization passes are complementary, not competing:

| Concern | Ori Lowering (Pre-LLVM) | LLVM Passes (Post-Lowering) |
|---------|------------------------|---------------------------|
| `bool` → `i1` | Yes (Tier 1) | — |
| `byte` → `i8` | Yes (Tier 1) | — |
| `Ordering` → `i8` | Yes (Tier 1) | — |
| `char` → `i32` | Yes (Tier 1) | — |
| Enum tag → `i8` | Yes (Tier 2a) | — |
| All-unit enum → tag only | Yes (Tier 2b) | — |
| Sum type payload sharing | Yes (Tier 2c) | — |
| ARC elision | Yes (Tier 2d) | — |
| Newtype erasure | Yes (Tier 2e) | — |
| `int` range narrowing | Future (Tier 3a) | `TruncInstCombine` |
| Dead field elimination | — | SROA |
| Constant folding | — | `ConstantFolding` |
| Dead code elimination | — | `SimplifyCFG`, `ADCE` |

**Division of responsibility:**

- **Ori lowering** handles _semantic_ narrowing: optimizations that require knowledge of Ori's type system (e.g., "Ordering has exactly 3 values," "this enum has no payloads," "this type has no heap references")
- **LLVM passes** handle _opportunistic_ narrowing: optimizations based on data flow analysis (e.g., "the upper 32 bits of this i64 are never read," "this branch is never taken")

**Principle:** Ori should emit LLVM IR that is _friendly_ to LLVM's optimization passes. This means:
- Use `nsw` (no signed wrap) and `nuw` (no unsigned wrap) flags on arithmetic where overflow has been checked
- Use LLVM's `range` metadata to annotate known value ranges
- Use `dereferenceable` attributes on pointers known to be valid
- Use `nonnull` attributes where applicable
- Prefer `getelementptr inbounds` for array access

These annotations give LLVM more information to work with, enabling its passes to perform more aggressive (but still correct) optimizations — including integer narrowing that Ori itself doesn't implement.

---

## Examples

### Example 1: Enum Discriminant Narrowing

```ori
type Color = Red | Green | Blue
// 3 variants → discriminant fits in i8
// Layout: { tag: i8 } (1 byte total, no payload)
```

**Machine code impact:** Pattern matching on `Color` generates `i8` comparisons instead of `i64`, reducing instruction count and enabling byte-level branch optimizations.

### Example 2: Transitive ARC Elision

```ori
type Point = { x: int, y: int }
// All fields are primitives → trivial type
// No retain/release emitted for Point values

type Line = { start: Point, end: Point }
// Transitively trivial (Point is trivial) → no ARC overhead

type Named = { name: str, value: int }
// str is reference-counted → non-trivial
// ARC retain/release emitted for Named values

type MaybeNamed = Option<Named>
// Named is non-trivial → Option<Named> is non-trivial
// ARC operations emitted

type MaybePoint = Option<Point>
// Point is trivial → Option<Point> is trivial
// NO ARC operations — the entire Option is stack-allocated
```

### Example 3: Result Payload Sharing

```ori
@parse (input: str) -> Result<int, str>
// sizeof(int) = 8 bytes, sizeof(str) = 16 bytes
// Compiler uses 16-byte payload slot (max of both):
//   Layout: { tag: i8, payload: [2 × i64] }
// Ok(int): stores 8 bytes, remaining 8 zero-initialized
// Err(str): uses full 16 bytes
```

### Example 4: Bool as i1

```ori
let flag = true
// Stored as i1 in LLVM IR
// Comparison: icmp eq i1 %flag, true

let flags: [bool] = [true, false, true, false]
// Array elements stored as i1
// LLVM may pack into bytes for memory storage
```

### Example 5: Future Integer Narrowing (Tier 3)

```ori
@count_matches (items: [str], target: str) -> int = {
    let count = 0
    // If compiler proves count stays within [0, len(items)] and len(items) < 2³¹
    // it MAY narrow count to i32 for the loop body.
    // Overflow checking still fires at int.max, not i32.max.
    for item in items do
        if item == target then count + 1 else count
    count
}
```

---

## Spec Changes

### 06-types.md (Types)

Amend the primitive types table to separate semantic range from representation.

**Before:**
```markdown
| `int` | 64-bit signed integer | `0` |
| `float` | 64-bit IEEE 754 | `0.0` |
```

**After:**
```markdown
| `int` | Signed integer (range: -2⁶³ to 2⁶³ - 1) | `0` |
| `float` | IEEE 754 double-precision (range: ±1.8 × 10³⁰⁸, ~15-17 digits) | `0.0` |
```

Add a note after the table:

> **Note:** The ranges above define the _semantic contract_ — the set of values a type can hold and the precision of its operations. The compiler may use a narrower machine representation when it can prove semantic equivalence. See [System Considerations § Representation Optimization](22-system-considerations.md#representation-optimization).

### 22-system-considerations.md (System Considerations)

Replace the "Numeric Types" section header content for Integers and Floats. The current text says "The `int` type is a 64-bit signed integer." Change to:

**Before:**
```markdown
### Integers

The `int` type is a 64-bit signed integer.

| Property | Value |
|----------|-------|
| Size | 64 bits |
```

**After:**
```markdown
### Integers

The `int` type is a signed integer with the following semantic range:

| Property | Value |
|----------|-------|
| Canonical size | 64 bits |
| Minimum | -9,223,372,036,854,775,808 (-2⁶³) |
| Maximum | 9,223,372,036,854,775,807 (2⁶³ - 1) |
| Overflow | Panics |

The canonical size defines the semantic range. The compiler may use a narrower machine representation (see § Representation Optimization).
```

Apply the same pattern to the Floats section:

**After:**
```markdown
### Floats

The `float` type is an IEEE 754 double-precision floating-point number:

| Property | Value |
|----------|-------|
| Canonical size | 64 bits |
| Precision | ~15-17 significant decimal digits |
| Range | ±1.7976931348623157 × 10³⁰⁸ |

The canonical size defines the semantic precision. The compiler may use a narrower machine representation when it can prove no precision loss (see § Representation Optimization).
```

Add a new section **Representation Optimization** after Numeric Types:

```markdown
## Representation Optimization

The compiler may optimize the machine representation of any type, provided the optimization preserves _semantic equivalence_. An optimization is semantically equivalent if no conforming program can distinguish the optimized representation from the canonical one through any language-level operation.

### Canonical Representations

| Type | Canonical | Semantic Range |
|------|-----------|----------------|
| `int` | 64-bit signed two's complement | [-2⁶³, 2⁶³ - 1] |
| `float` | 64-bit IEEE 754 binary64 | ±1.8 × 10³⁰⁸, ~15-17 digits |
| `bool` | 1-bit | `true` or `false` |
| `byte` | 8-bit unsigned | [0, 255] |
| `char` | 32-bit Unicode scalar | U+0000–U+10FFFF excluding surrogates |
| `Ordering` | Tri-state | `Less`, `Equal`, `Greater` |

### Permitted Optimizations

Permitted optimizations include but are not limited to:

- Narrowing primitive machine types (`bool` → `i1`, `byte` → `i8`, `char` → `i32`, `Ordering` → `i8`)
- Enum discriminant narrowing (`i8` for ≤256 variants)
- All-unit enum payload elimination
- Sum type shared payload slots (`Result<T, E>` uses `max(sizeof(T), sizeof(E))`)
- ARC operation elision for transitively trivial types
- Newtype representation erasure
- Struct field reordering for alignment
- Integer narrowing based on value range analysis
- Float narrowing when precision loss is provably zero

### Guarantees

1. The semantic range of every type is always preserved
2. Overflow behavior is determined by the semantic type, not the machine representation
3. Values stored and retrieved through any language operation are identical
4. `debug()` and `print()` display semantic values
5. `x == y` and `hash(x) == hash(y)` relationships are representation-independent
6. Value/reference type classification is determined by canonical size, not machine size

### Non-Guarantees

1. The exact machine representation of any type is unspecified
2. Memory layout may differ between compiler versions and target platforms
3. Struct field order in memory may differ from declaration order
```

### 15-memory-model.md (Memory Model)

Amend the "Value vs Reference Types" section. Add after the existing criteria table:

> **Note:** The size thresholds above (≤32 bytes) refer to the _canonical representation_. The compiler's representation optimization may reduce the actual machine size, but value/reference classification is determined by the canonical layout. A type that is canonically 40 bytes (reference) remains a reference type even if representation optimization reduces it to 20 bytes.

---

## Design Rationale

### Why Not Expose Widths to Programmers?

Several alternatives were considered:

| Approach | Example | Rejection Reason |
|----------|---------|------------------|
| Multiple integer types | `i8`, `i16`, `i32`, `i64` | Multiplies type system complexity: operator resolution, literal inference, implicit widening rules. Not justified for a non-systems language. Gleam, Elm, and Roc all avoid this. |
| Type suffixes on literals | `42i32`, `3.14f32` | Same complexity as above, plus grammar changes. Creates Rust's "what type is `42`?" inference problem. |
| Width annotations | `int<32>` | Creates false control — the compiler may ignore or override the hint. Misleads programmers about what they can control. |
| Programmer hints | `#compact` attribute | Adds cognitive load for marginal gain. The programmer shouldn't need to think about bit widths. |
| Zig-style explicit control | `packed struct`, `@bitCast` | Appropriate for systems language; wrong for Ori's abstraction level. |
| Koka-style declarations | `value struct` vs `reference struct` | Reasonable but still pushes representation concerns to programmers. Ori prefers the compiler to handle this. |

The chosen approach (compiler-directed optimization, invisible to programmers) is the simplest: one integer type, one float type, compiler handles the rest.

### Why Document Implementation Details?

If it's invisible to programmers, why spec it?

1. **Implementer guidance** — Compiler contributors need to know what optimizations are legal and what invariants must hold. Without a spec, a well-meaning contributor might narrow `int` overflow to `i32` range, thinking it's "just an optimization."
2. **Correctness boundary** — The as-if rule needs precise definition. "Observable behavior" must be enumerated (equality, hashing, formatting, overflow).
3. **FFI contract** — FFI is the one place representation matters. The spec must define what representation FFI uses.
4. **Cross-phase consistency** — Representation decisions in the codegen must be consistent with assumptions in the type checker, evaluator, and ARC system. The spec documents these consistency requirements.
5. **Target portability** — Different backends (LLVM, WASM, future JIT) need a shared understanding of what they may and may not optimize.

### Why Three Tiers?

The tier system reflects confidence and complexity:

| Tier | Confidence | Analysis Required | Risk |
|------|-----------|-------------------|------|
| 1 (Type-Intrinsic) | Maximum | None (type definition) | Near zero |
| 2 (Layout) | High | Composite type structure | Low (well-understood algorithms) |
| 3 (Range Narrowing) | Medium | Data flow / value range | Medium (overflow boundary correctness) |

Tier 1 and 2 optimizations are already implemented and well-tested. Tier 3 is explicitly future work and may rely on LLVM passes rather than Ori's own analysis.

### Relationship to Swift's Type Lowering

Swift's SIL (Swift Intermediate Language) has the closest model:
- **Formal types** (what the user writes) are distinct from **lowered types** (how the compiler implements them)
- The lowering process is documented in the SIL specification
- Resilient types have runtime-determined layout for ABI stability

Ori differs from Swift in that:
- Ori has no `resilient` concept (ABI stability is not a current goal)
- Ori's lowering is fully opaque (Swift exposes `indirect`, `@box`, etc.)
- Ori's lowering is compile-time only (no runtime layout computation)

### Relationship to Rust's Layout Optimization

Rust's enum niche optimization is the most aggressive prior art:
- **Niche filling:** `Option<&T>` uses the null pointer as the `None` discriminant, making `Option<&T>` the same size as `&T`
- **Field reordering:** `repr(Rust)` permits field reordering for padding reduction

Ori could adopt niche optimization in the future (e.g., `Option<byte>` could use 256 as the `None` discriminant, fitting in a single byte). This is explicitly permitted by the as-if rule and would be a Tier 2 optimization.

---

## Summary

1. **Representation is a compiler concern** — Programmers write `int`, `float`, `bool`. The compiler chooses the machine representation.
2. **Semantic equivalence is the contract** — Any optimization that preserves observable behavior is legal. Observable behavior includes: equality, hashing, formatting, overflow boundaries, comparison ordering, and conversion results.
3. **Existing optimizations are codified** — `bool` → `i1`, enum tags → `i8`, ARC elision, payload sharing — all become spec-backed with documented invariants.
4. **Future optimizations are enabled** — Value range narrowing, niche optimization, bit packing, and struct field reordering have a principled framework.
5. **Cross-cutting invariants are explicit** — Every interaction point (ARC, ABI, alignment, equality, hashing, conversions, pattern matching, const eval, debug output, FFI, WASM) has documented rules.
6. **No new language features** — This proposal adds zero syntax, zero types, and zero programmer-facing complexity.
