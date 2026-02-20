# Proposal: Repr Extensions

**Status:** Approved
**Author:** Eric
**Created:** 2026-02-02
**Approved:** 2026-02-02

---

## Summary

Extend the `#repr` attribute to support additional memory layout controls beyond `#repr("c")`. Add `#repr("packed")`, `#repr("transparent")`, and `#repr("aligned", N)` for fine-grained control over struct layout.

```ori
#repr("packed")
type PackedData = { flags: byte, value: int }  // No padding

#repr("transparent")
type UserId = { inner: int }  // Same layout as int

#repr("aligned", 64)
type CacheLine = { data: [byte, max 64] }  // 64-byte aligned
```

---

## Motivation

### Current State

Ori has `#repr("c")` for C-compatible struct layout, used for FFI:

```ori
#repr("c")
type CStruct = { x: int, y: float }
```

This works well for interop, but doesn't cover all layout needs.

### Missing Capabilities

| Need | Current Solution | Problem |
|------|------------------|---------|
| Minimize struct size | None | Padding wastes memory |
| Newtype with same ABI | None | Extra indirection possible |
| Cache-aligned data | None | False sharing in concurrent code |
| SIMD-aligned vectors | None | Unaligned loads are slower |

### Use Cases

**1. Binary Protocol Parsing**
```ori
// Network packet header - must match wire format exactly
#repr("packed")
type TcpHeader = {
    source_port: c_short,  // 16-bit, no padding between fields
    dest_port: c_short,    // 16-bit
    sequence: c_int,       // 32-bit
    ack: c_int,            // 32-bit
    flags: byte
}
```

**2. Zero-Cost Newtypes**
```ori
// UserId should have identical ABI to int
#repr("transparent")
type UserId = { inner: int }

// Can pass to C functions expecting int
extern "c" { @process_user (id: int) -> void }
process_user(id: user_id.inner)  // Guaranteed same representation
```

**3. Cache-Optimized Concurrent Data**
```ori
// Prevent false sharing between threads
#repr("aligned", 64)
type Counter = { value: int }

// Each counter on its own cache line
let counters: [Counter, max 8] = ...
```

**4. SIMD Alignment**
```ori
// Ensure 16-byte alignment for SSE operations
#repr("aligned", 16)
type Vec4f = { x: float, y: float, z: float, w: float }
```

---

## Design

### Applicability

`#repr` attributes apply only to struct types. Sum types use Ori's standard tagged representation and cannot have custom layout attributes.

### Syntax

The `#repr` attribute takes a string or string + integer argument:

```ori
#repr("c")              // Existing - C-compatible layout
#repr("packed")         // New - no padding between fields
#repr("transparent")    // New - same layout as single field
#repr("aligned", N)     // New - minimum N-byte alignment
```

### `#repr("packed")`

Removes all padding between struct fields. Fields are laid out consecutively in declaration order.

```ori
#repr("packed")
type Packed = { a: byte, b: int, c: byte }
// Layout: [a:1][b:8][c:1] = 10 bytes total
// Without packed: [a:1][pad:7][b:8][c:1][pad:7] = 24 bytes
```

**Constraints:**
- Applies to struct types only
- All field accesses may be unaligned (compiler handles this)
- Cannot combine with `#repr("aligned", N)`

**Warning:** Unaligned access is slower on most architectures. Use only when size matters more than speed.

### `#repr("transparent")`

The struct has the same memory layout and ABI as its single non-zero-sized field.

```ori
#repr("transparent")
type Wrapper = { inner: SomeType }
// Wrapper has identical layout to SomeType
```

**Constraints:**
- Struct must have exactly one non-zero-sized field
- May have additional zero-sized fields (if Ori adds them in future)
- The struct's alignment equals the field's alignment
- The struct's size equals the field's size

**Relationship to newtypes:**

Newtypes (`type UserId = int`) are implicitly transparent — they have identical layout to their underlying type without requiring an explicit attribute. Use `#repr("transparent")` for struct types with a single field that need guaranteed ABI compatibility:

```ori
// Newtype - implicitly transparent
type UserId = int

// Struct with explicit transparent repr - for FFI wrappers
#repr("transparent")
type FileHandle = { fd: c_int }
```

**Use cases:**
- FFI wrapper structs that must match C ABI
- Single-field structs passed to external functions
- Type-safe handles with guaranteed layout

### `#repr("aligned", N)`

Sets minimum alignment to N bytes. N must be a power of two.

```ori
#repr("aligned", 16)
type Aligned16 = { x: int, y: int }
// Alignment: 16 bytes (instead of default 8)
// Size: 16 bytes (padded to alignment)
```

**Constraints:**
- N must be a power of two: 1, 2, 4, 8, 16, 32, 64, 128, ...
- N must be ≤ platform maximum (typically 4096)
- Alignment is at least N, but may be higher if fields require it
- Cannot combine with `#repr("packed")`

**Common values:**
| N | Use Case |
|---|----------|
| 16 | SSE/NEON SIMD |
| 32 | AVX SIMD |
| 64 | Cache line (most CPUs) |
| 128 | Cache line (some ARM) |

### Combining with `#repr("c")`

`#repr("c")` can combine with `#repr("aligned", N)`:

```ori
#repr("c")
#repr("aligned", 16)
type CAligned = { x: int, y: int }
// C-compatible field order AND 16-byte aligned
```

Cannot combine:
- `#repr("c")` with `#repr("packed")` - use `#repr("c")` only, add explicit padding
- `#repr("packed")` with `#repr("aligned", N)` - contradictory

### Compile-Time Validation

The compiler validates repr attributes:

```ori
#repr("packed")
type NotAStruct = int  // ERROR: #repr("packed") only applies to struct types

#repr("transparent")
type TwoFields = { a: int, b: int }  // ERROR: #repr("transparent") requires exactly one field

#repr("aligned", 7)
type BadAlign = { x: int }  // ERROR: alignment must be a power of two

#repr("packed")
#repr("aligned", 16)
type Conflict = { x: int }  // ERROR: cannot combine packed and aligned
```

---

## Examples

### Binary File Format

```ori
#repr("packed")
type BmpHeader = {
    magic: [byte, max 2],      // "BM"
    file_size: c_int,          // 32-bit
    reserved: c_int,           // 32-bit
    data_offset: c_int         // 32-bit
}

@read_bmp_header (data: [byte]) -> Result<BmpHeader, Error> = {
    pre_check: len(collection: data) >= 14
    // ... parse bytes into header
}
```

### Type-Safe Handles

```ori
#repr("transparent")
type FileHandle = { fd: c_int }

#repr("transparent")
type SocketHandle = { fd: c_int }

// These are distinct types but have c_int's ABI
// Compiler prevents mixing them up
@close_file (handle: FileHandle) -> void = ...
@close_socket (handle: SocketHandle) -> void = ...
```

### SIMD-Ready Types

```ori
#repr("aligned", 16)
type Vec4f = { x: float, y: float, z: float, w: float }

@dot (a: Vec4f, b: Vec4f) -> float uses Intrinsics =
    // Compiler can use aligned SIMD loads
    a.x * b.x + a.y * b.y + a.z * b.z + a.w * b.w
```

### Cache-Conscious Data Structures

```ori
#repr("aligned", 64)
type PaddedCounter = {
    value: int,
    _pad: [byte, max 56]  // Fill to 64 bytes
}

// Array of counters, each on own cache line
type CounterArray = {
    counters: [PaddedCounter, max 8]
}
```

---

## Implementation Notes

### LLVM Lowering

Each repr maps directly to LLVM concepts:

| Repr | LLVM Representation |
|------|---------------------|
| `#repr("c")` | Default struct, no `packed` |
| `#repr("packed")` | `packed` struct type |
| `#repr("transparent")` | Same as inner type |
| `#repr("aligned", N)` | `align N` on allocations |

### Compiler Changes

1. **Parser**: Accept `#repr("packed")`, `#repr("transparent")`, `#repr("aligned", N)`
2. **Type checker**: Validate constraints (single field for transparent, power-of-two for aligned)
3. **IR**: Add `ReprKind` enum to struct type definitions
4. **LLVM codegen**: Apply appropriate LLVM attributes

```rust
// In ori_ir or ori_types
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReprKind {
    Default,           // Ori's default layout
    C,                 // #repr("c")
    Packed,            // #repr("packed")
    Transparent,       // #repr("transparent")
    Aligned(u32),      // #repr("aligned", N)
    CAligned(u32),     // #repr("c") + #repr("aligned", N)
}
```

### Size and Alignment Queries

With these repr options, expose layout information:

```ori
// Future: compile-time size/alignment queries
$size_of<Vec4f>       // 16
$align_of<Vec4f>      // 16
$size_of<PackedData>  // 9 (no padding)
```

This is a separate proposal but naturally follows from repr control.

---

## Design Rationale

### Why Strings Instead of Identifiers?

```ori
#repr("packed")   // Chosen
#repr(packed)     // Alternative
```

Strings match existing `#repr("c")` syntax and clearly indicate these are not Ori identifiers. This prevents confusion if `packed` or `aligned` become keywords.

### Why Not `#packed` / `#aligned`?

```ori
#packed           // Alternative
#aligned(16)      // Alternative
```

Grouping under `#repr` signals these are all about memory representation. It's clearer that `#repr("packed")` and `#repr("aligned", 16)` are related and mutually exclusive than if they were separate attributes.

### Why Power-of-Two Alignment Only?

All hardware architectures align to power-of-two boundaries. Non-power-of-two alignment would:
- Have no hardware support
- Complicate offset calculations
- Provide no practical benefit

### Why Prohibit `packed` + `aligned`?

These are contradictory:
- `packed`: minimize size, ignore alignment
- `aligned`: enforce minimum alignment, add padding if needed

If you need packed data at an aligned address, align the containing allocation, not the type itself.

---

## Comparison to Other Languages

| Language | Packed | Transparent | Aligned |
|----------|--------|-------------|---------|
| **C** | `__attribute__((packed))` | N/A | `__attribute__((aligned(N)))` |
| **C++** | `[[gnu::packed]]` | N/A | `alignas(N)` |
| **Rust** | `#[repr(packed)]` | `#[repr(transparent)]` | `#[repr(align(N))]` |
| **Zig** | `packed struct` | N/A | `align(N)` |
| **Ori** | `#repr("packed")` | `#repr("transparent")` | `#repr("aligned", N)` |

Ori's syntax is closest to Rust's, using an attribute with a string argument.

---

## Future Extensions

### `#repr("simd")`

Automatic SIMD-friendly layout for homogeneous structs:

```ori
#repr("simd")
type Vec4f = { x: float, y: float, z: float, w: float }
// Automatically aligned to 16, laid out for vector ops
```

### Size/Alignment Intrinsics

```ori
$size_of<T>    // Compile-time size in bytes
$align_of<T>   // Compile-time alignment in bytes
$offset_of<T, field>  // Compile-time field offset
```

### Union Types

If Ori adds untagged unions (for FFI), repr would apply:

```ori
#repr("c")
union CUnion = { as_int: int, as_float: float }
```

---

## Summary

This proposal extends `#repr` with three new options:

| Repr | Purpose | Constraint |
|------|---------|------------|
| `#repr("packed")` | No padding between fields | Struct only, no aligned |
| `#repr("transparent")` | Same layout as single field | Exactly one field |
| `#repr("aligned", N)` | Minimum N-byte alignment | N is power of two |

These provide fine-grained control over memory layout for:
- Binary protocol parsing
- FFI compatibility
- Cache optimization
- SIMD performance

All map directly to LLVM concepts, making implementation straightforward.
