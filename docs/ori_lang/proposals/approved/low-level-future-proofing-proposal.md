# Proposal: Low-Level Future-Proofing

**Status:** Approved
**Author:** Eric
**Created:** 2026-02-02
**Approved:** 2026-02-02

---

## Summary

Reserve design space in Ori's type system and IR to enable future low-level programming features (stack-allocated types, borrowed views, arenas) without breaking ARC semantics or the high-level programming model.

This proposal does **not** implement any low-level features. It identifies architectural decisions that should be made now to avoid boxing ourselves out of these capabilities later.

---

## Motivation

### The Problem

Ori is designed as a high-level language with ARC memory management. However, looking at successful languages that added low-level features post-design reveals a pattern:

| Language | Low-Level Addition | Difficulty |
|----------|-------------------|------------|
| Java | Value types (Valhalla) | 20+ years, still incomplete |
| Go | Generics | 10+ years, significant redesign |
| Python | Type hints | Bolted-on, two parallel worlds |
| **.NET** | `Span<T>`, `stackalloc` | Relatively smooth |
| **Rust** | (Had low-level from start) | N/A |

The difference: .NET and Rust had **architectural slots** reserved from the beginning, even for features they didn't ship initially.

### Why This Matters for Ori

Potential future use cases that require low-level features:

1. **Game development**: Frame-allocated temporary data, SIMD math types
2. **Embedded systems**: No heap, fixed memory budgets
3. **High-performance networking**: Zero-copy buffer views
4. **Scientific computing**: Unboxed numeric arrays
5. **Interop**: C libraries that expect stack-allocated structs

Without reserved design space, adding these later would require:
- Breaking changes to the type system
- IR redesign
- Compiler rewrites

### Current State Analysis

An audit of Ori's current architecture reveals:

| Aspect | Status | Risk Level |
|--------|--------|------------|
| Generics | Monomorphization | ✅ Safe |
| Type interning | Sharded TypeId | ✅ Safe |
| LLVM codegen | Uses `alloca`, `build_struct` | ✅ Safe |
| `#repr("c")` | Exists | ✅ Safe |
| Trait objects | Not yet implemented | ✅ Can design fresh |
| **Lifetime concept** | None | ⚠️ Needs slot |
| **Value category** | Uniform | ⚠️ Needs slot |
| **Copy trait** | None | ⚠️ Needs slot |

---

## Design

This proposal reserves three architectural slots without implementing their features.

### 1. Lifetime Slot in Type Representation

**Current** (`ori_types/src/core.rs`):
```rust
pub enum Type {
    Int,
    List(Box<Type>),
    // ...
}
```

**Proposed**:
```rust
/// Lifetime identifier for future borrowed reference types.
/// Currently unused - all values are `'static` (owned/ARC'd).
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct LifetimeId(u32);

impl LifetimeId {
    /// The static lifetime - owned values, no borrowing.
    /// All current Ori types implicitly have this lifetime.
    pub const STATIC: LifetimeId = LifetimeId(0);

    /// Reserved for future: lifetime bound to current scope.
    pub const SCOPED: LifetimeId = LifetimeId(1);
}

pub enum Type {
    Int,
    List(Box<Type>),
    // ... existing variants unchanged ...

    /// Future: borrowed view type with lifetime constraint.
    /// Not yet implemented - reserved for `Slice<T>`, `Ref<T>`.
    #[doc(hidden)]
    Borrowed {
        inner: Box<Type>,
        lifetime: LifetimeId,
    },
}
```

**Why**: This allows future addition of borrowed views (`Slice<T>`) without restructuring the type enum. The `Borrowed` variant is hidden and unused, but its presence reserves the concept.

**Cost**: One additional enum variant (not constructed), one new type (4 bytes). Zero runtime impact.

### 2. Value Category in TypeData

**Current** (`ori_types/src/data.rs`):
```rust
pub enum TypeData {
    Int,
    List(TypeId),
    // ...
}
```

**Proposed**:
```rust
/// Value category for a type - determines memory representation and semantics.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug, Default)]
pub enum ValueCategory {
    /// Heap-allocated with ARC (current default for all compound types).
    #[default]
    Boxed,

    /// Stack-allocated, copied on assignment (future: `inline type`).
    /// Reserved - not yet implemented.
    Inline,

    /// Borrowed view, cannot outlive source (future: `Slice<T>`).
    /// Reserved - not yet implemented.
    View,
}

pub enum TypeData {
    Int,
    List(TypeId),
    // ...

    // New: compound types can carry category metadata
    Struct {
        name: Name,
        category: ValueCategory,  // Always Boxed for now
    },
}
```

**Why**: When we add `inline type Vec3 = { ... }`, the type system needs to know this struct has different semantics (copy, no ARC). Reserving the category now means `inline` is an annotation change, not a structural change.

**Cost**: One additional field in struct variants. `ValueCategory` is 1 byte (3-variant enum).

### 3. Syntax Reservation

Reserve these syntax patterns in the grammar (parse but reject with clear error):

```ori
// Reserved for stack-allocated value types
inline type Vec3 = { x: float, y: float, z: float }
// Error: "inline types are reserved for a future version of Ori"

// Reserved for borrowed views
@process (data: &[byte]) -> int = ...
// Error: "borrowed references (&T) are reserved for a future version"

// Alternative view syntax (if & is problematic)
@process (data: view [byte]) -> int = ...
// Error: "view types are reserved for a future version of Ori"
```

**Why**: If someone writes `inline type` today, they get a helpful message instead of a confusing parse error. It also prevents us from accidentally using these keywords for something else.

**Cost**: Parser recognizes keywords but immediately errors. No codegen impact.

---

## What This Enables (Future)

With these slots reserved, future proposals could add:

### Stack-Allocated Types (Future Proposal)

```ori
// inline types: copied, no ARC, stored in registers/stack
inline type Vec3 = { x: float, y: float, z: float }

let a = Vec3 { x: 1.0, y: 2.0, z: 3.0 }
let b = a  // Copy, not refcount - a is still valid

// Perfect for: SIMD types, small math types, embedded
```

### Borrowed Views (Future Proposal)

```ori
// Slice: view into list, cannot outlive source
@sum (data: Slice<int>) -> int = data.fold(initial: 0, f: (a, b) -> a + b)

let list = [1, 2, 3, 4, 5]
let chunk = list[1..4].as_slice()  // No allocation
sum(data: chunk)  // Zero-copy access

// chunk cannot escape this scope - compiler enforced
```

### Arena Allocation (Future Proposal)

```ori
// Bulk allocation, bulk deallocation
with Arena.scoped() as arena in
    let nodes = arena.alloc_many<Node>(count: 1000)
    build_tree(nodes: nodes)
    // All nodes freed at end of block - no per-object ARC
```

---

## Design Rationale

### Why Not Implement Now?

1. **Focus**: Ori's core semantics are still being refined
2. **Learning**: Real usage patterns will inform the right design
3. **Compatibility**: Reserved slots don't constrain; premature implementation does
4. **Cost**: These reservations have near-zero implementation cost

### Why Reserve Now?

1. **IR stability**: Adding enum variants later is a breaking change
2. **Keyword protection**: Prevents accidental use of `inline`, `view`, etc.
3. **Mental model**: Contributors know these directions exist
4. **Design constraint**: Forces us to not accidentally close doors

### Why Lifetime Slots Instead of Full Lifetimes?

Rust-style lifetime annotations (`'a`, `'b`) are complex:
- Require explicit annotation in most cases
- Lifetime subtyping is subtle
- Variance rules are confusing

Ori's approach would likely be **simpler**:
- Most types: `'static` (owned, ARC'd) - no annotation needed
- View types: `'scoped` (cannot escape current scope) - implicit
- Advanced: Named lifetimes only when absolutely necessary

The `LifetimeId` slot supports all these models without committing to one.

### Why Not Just Use Attributes?

```ori
// Could we just use attributes instead?
#inline
type Vec3 = { ... }
```

Attributes work for **annotations**, but value categories affect:
- Type equality (is `Vec3 == Vec3` across modules?)
- ABI (how is it passed to functions?)
- Memory layout (inline in structs?)

These need representation in the type system, not just metadata.

---

## Implementation Notes

### Phase 1: Type System Slots (This Proposal)

1. Add `LifetimeId` type to `ori_types`
2. Add `ValueCategory` enum to `ori_types`
3. Add `#[doc(hidden)]` `Borrowed` variant to `Type` enum
4. Add `category` field to struct-related `TypeData` variants
5. All new fields default to "current behavior" values

**Estimated scope**: ~50-100 lines across `ori_types`

### Phase 2: Syntax Reservation

1. Add `inline` and `view` as reserved keywords in lexer
2. Parser recognizes but rejects with informative error
3. Grammar docs note reservation

**Estimated scope**: ~20 lines in lexer, ~10 lines in parser

### Phase 3: Future Work (Separate Proposals)

Each low-level feature would be its own proposal:
- `inline-types-proposal.md`
- `borrowed-views-proposal.md`
- `arena-allocation-proposal.md`

These can be designed when there's concrete demand.

---

## Alternatives Considered

### Do Nothing

**Risk**: Future low-level features require breaking changes or awkward workarounds.

**Example**: Java's Valhalla project has taken 10+ years because value types were an afterthought.

### Full Lifetime System Now

**Risk**: Over-engineering. We don't know the right design yet.

**Example**: Premature lifetime syntax might conflict with future Ori idioms.

### Attribute-Based Categories

```rust
#[inline]
type Vec3 = { ... }
```

**Problem**: Attributes are metadata, not type identity. Two `Vec3` types with different attributes would be confusingly "equal" in the type system but have different semantics.

---

## Comparison to Other Languages

| Language | Low-Level Strategy | Outcome |
|----------|-------------------|---------|
| **Java** | Afterthought (Valhalla) | Decades of work, still incomplete |
| **Go** | Afterthought (generics) | Major redesign after v1.0 |
| **.NET** | Reserved space (`struct` vs `class`) | `Span<T>` added smoothly in C# 7.2 |
| **Rust** | Built-in from start | N/A (no retrofit needed) |
| **Swift** | Reserved space (`struct` vs `class`) | Value types work well |
| **Ori** | This proposal | Reserve space now, implement later |

---

## Summary

This proposal reserves architectural space for future low-level features:

| Reservation | Purpose | Cost |
|-------------|---------|------|
| `LifetimeId` | Enable borrowed views | 4-byte type, unused |
| `ValueCategory` | Enable inline types | 1-byte field, always `Boxed` |
| `Borrowed` variant | Type system slot | Unused enum variant |
| `inline`/`view` keywords | Syntax protection | Reserved keywords |

**Total cost**: ~100 lines of code, no runtime impact, no user-visible changes.

**Benefit**: When we want low-level features, we add them incrementally rather than redesigning the compiler.

---

## Open Questions

1. **Syntax for views**: Should it be `&T`, `view T`, `Slice<T>`, or something else?
2. **Inline trait**: Should inline types have a marker trait, or is it purely a type modifier?
3. **Lifetime syntax**: If we ever need explicit lifetimes, what syntax? (`'a`? `@a`? `#a`?)
4. **Arena integration**: How do arenas interact with capabilities?

These questions don't need answers now. The reserved slots support any of these directions.
