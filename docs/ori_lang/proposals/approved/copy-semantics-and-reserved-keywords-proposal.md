# Proposal: Copy Semantics and Additional Reserved Keywords

**Status:** Approved
**Author:** Claude (with Eric)
**Created:** 2026-02-02
**Approved:** 2026-02-02
**Depends On:** low-level-future-proofing-proposal.md

---

## Summary

Extend the low-level future-proofing proposal with:

1. **Copy trait slot** — Reserve architectural space for distinguishing copy vs move semantics
2. **Additional reserved keywords** — `union`, `static`, `asm`

This ensures Ori can later add:
- Inline types with move-only semantics (e.g., unique handles)
- Untagged unions for FFI
- Mutable static state for embedded systems
- Inline assembly for performance-critical code

---

## Motivation

### The Copy vs Move Gap

The low-level-future-proofing proposal reserves `ValueCategory` to distinguish inline (stack) vs boxed (heap) types:

```rust
pub enum ValueCategory {
    Boxed,   // Heap-allocated with ARC
    Inline,  // Stack-allocated
    View,    // Borrowed
}
```

However, this doesn't address **assignment semantics**:

```ori
inline type Vec3 = { x: float, y: float, z: float }
inline type UniqueHandle = { ptr: CPtr }

let a = Vec3 { x: 1.0, y: 2.0, z: 3.0 }
let b = a  // Copy? a still valid

let h1 = UniqueHandle { ptr: acquire_resource() }
let h2 = h1  // Move? h1 invalidated?
```

Without a Copy/Move distinction:
- All inline types would have uniform semantics (either all copy or all move)
- Cannot express unique ownership for inline types
- Resource handles would need to be boxed unnecessarily

### Missing Keywords

Several low-level features need syntax that could conflict with user identifiers:

| Feature | Keyword | Risk if Not Reserved |
|---------|---------|---------------------|
| Untagged unions | `union` | Breaking change when FFI unions added |
| Mutable statics | `static` | Breaking change for embedded support |
| Inline assembly | `asm` | Breaking change for systems programming |

---

## Design

### 1. Copy Trait Slot

Add a marker trait slot to the type system:

```rust
/// Marker indicating a type can be implicitly copied on assignment.
/// Types without Copy have move semantics (original invalidated after use).
///
/// Currently unused - all types behave as if Copy.
/// Reserved for future inline types with move-only semantics.
#[doc(hidden)]
pub trait Copy: Clone {}
```

**Semantics (Future):**

| Type | Copy | Assignment `let b = a` |
|------|------|------------------------|
| Primitives (`int`, `float`, etc.) | Yes | `a` remains valid |
| `inline type Vec3 = {...}` | Yes (default) | `a` remains valid |
| `inline type UniqueHandle = {...}` | No (opt-out) | `a` invalidated |
| Boxed types | N/A | ARC increment |

**Opt-out Syntax (Future):**

```ori
// Default: inline types are Copy
inline type Vec3 = { x: float, y: float, z: float }

// Explicit non-Copy for unique ownership
#no_copy
inline type UniqueHandle = { ptr: CPtr }
```

**Current Behavior:** All types behave as Copy (or use ARC). The trait slot exists but is unused.

### 2. Reserved Keywords

Add to the "Reserved (Future)" category:

```
inline   view     // Existing (from low-level-future-proofing)
union    static   asm   // New
```

#### `union`

For untagged unions (FFI interop):

```ori
// Future: C-compatible untagged union
#repr("c")
union CValue = { as_int: c_int, as_float: c_float, as_ptr: CPtr }

// Access requires unsafe (no tag to check)
let v = CValue { as_int: 42 }
let f = unsafe(v.as_float)  // Reinterpret bits
```

#### `static`

For mutable global state (embedded systems):

```ori
// Current: immutable module-level constants
let $BUFFER_SIZE = 1024

// Future: mutable static (requires unsafe to access)
static COUNTER: int = 0

@increment () -> int uses Intrinsics =
    unsafe {
        COUNTER = COUNTER + 1
        COUNTER
    }
```

Note: The exact syntax for static declarations (standalone `static` vs `let static`) will be determined in the implementation proposal.

#### `asm`

For inline assembly:

```ori
// Future: inline assembly for performance-critical code
@fast_sqrt (x: float) -> float uses Intrinsics =
    unsafe(asm("sqrtsd %xmm0, %xmm0"))
```

### 3. Compiler Behavior

Reserved future keywords produce informative errors:

```ori
let union = 5
// Error: `union` is reserved for a future version of Ori
// Hint: Use a different variable name

type static = int
// Error: `static` is reserved for a future version of Ori
```

---

## Implementation

### Phase 1: Trait Slot (ori_types)

```rust
// In ori_types/src/traits.rs or similar

/// Marker trait for types that can be implicitly copied.
///
/// # Future Behavior
///
/// When inline types are implemented:
/// - Primitives: always Copy
/// - Inline structs: Copy by default, opt-out with #no_copy
/// - Boxed types: not Copy (use Clone explicitly, or ARC handles it)
///
/// # Current Behavior
///
/// All types behave as if Copy. This trait exists for future compatibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CopyTrait;

impl CopyTrait {
    /// Check if a type implements Copy.
    /// Currently always returns true.
    pub fn is_copy(_ty: TypeId) -> bool {
        true  // Future: check type's Copy impl
    }
}
```

**Estimated scope:** ~20 lines in `ori_types`

### Phase 2: Reserved Keywords (ori_lexer)

Add to keyword recognition:

```rust
// In ori_lexer/src/keywords.rs

pub enum ReservedFuture {
    Inline,  // Existing
    View,    // Existing
    Union,   // New
    Static,  // New
    Asm,     // New
}

impl ReservedFuture {
    pub fn error_message(&self) -> &'static str {
        match self {
            Self::Inline => "`inline` types are reserved for a future version of Ori",
            Self::View => "`view` types are reserved for a future version of Ori",
            Self::Union => "`union` types are reserved for a future version of Ori",
            Self::Static => "`static` declarations are reserved for a future version of Ori",
            Self::Asm => "`asm` blocks are reserved for a future version of Ori",
        }
    }
}
```

**Estimated scope:** ~25 lines in `ori_lexer`, ~10 lines in parser

### Phase 3: Documentation

Update:
- `docs/ori_lang/0.1-alpha/spec/03-lexical-elements.md` — add keywords
- `docs/ori_lang/0.1-alpha/spec/grammar.ebnf` — update comment
- `/CLAUDE.md` — update keywords section

---

## Design Rationale

### Why Copy as a Trait?

Alternatives considered:

| Approach | Pros | Cons |
|----------|------|------|
| **Trait (chosen)** | Matches Rust, flexible | Requires trait system |
| Attribute (`#copy`) | Simple | Not composable, can't bound on it |
| ValueCategory variant | Simple | Conflates storage with semantics |

A trait allows:
- Generic bounds: `@swap<T: Copy>(a: T, b: T) -> (T, T)`
- Derived implementations
- Explicit opt-out

### Why Not Reserve More?

Considered but not reserved:

| Keyword | Reason Not Reserved |
|---------|---------------------|
| `volatile` | Can be a capability or intrinsic function |
| `pin` | Can be a type (`Pin<T>`) not a keyword |
| `async`/`await` | Ori uses `uses Suspend` and implicit resolution |
| `const` | Ori uses `$` prefix instead |
| `mut` | Ori's `let`/`let $` design is intentional |

---

## Alternatives Considered

### Reserve Nothing

**Risk:** Breaking changes when adding low-level features.

**Example:** Go didn't reserve `generics`/`type` properly, leading to years of design constraints.

### Reserve Everything Conceivable

**Risk:** Over-reservation limits user naming freedom unnecessarily.

**Balance:** Reserve only keywords with high probability of future use and significant breaking-change risk.

### Use Attributes Instead of Keywords

```ori
#union
type CValue = ...

#static
let COUNTER: int = 0
```

**Problem:** These concepts are fundamental enough to warrant keyword status. Attributes suggest optional metadata, not core semantics.

---

## Comparison to Other Languages

| Language | Copy Mechanism | Union Keyword | Static Keyword |
|----------|----------------|---------------|----------------|
| **Rust** | `Copy` trait | `union` | `static` |
| **C++** | Implicit (copy ctor) | `union` | `static` |
| **Swift** | Value types copy | N/A | `static` |
| **Go** | Value types copy | N/A | N/A (package-level) |
| **Zig** | `@memcpy` explicit | `union` | `var` at file scope |
| **Ori** | Reserved trait slot | Reserved keyword | Reserved keyword |

---

## Summary

| Reservation | Purpose | Cost |
|-------------|---------|------|
| `Copy` trait slot | Distinguish copy vs move semantics | ~20 lines, unused |
| `union` keyword | Untagged unions for FFI | Reserved keyword |
| `static` keyword | Mutable global state | Reserved keyword |
| `asm` keyword | Inline assembly | Reserved keyword |

**Total cost:** ~35 lines of code, 3 additional reserved keywords

**Benefit:** Ori can add move semantics, unions, statics, and inline assembly without breaking changes.

---

## Open Questions

1. **Copy default:** Should inline types be Copy by default (opt-out via `#no_copy`) or non-Copy by default (opt-in via `#derive(Copy)`)?

2. **Union safety:** Should union field access require `unsafe(...)`, or use a safer pattern-matching approach?

3. **Static initialization:** How do mutable statics get initialized? Compile-time only, or runtime `init` blocks?

4. **Static syntax:** Should it be standalone `static NAME` or Ori-style `let static NAME`?

These questions don't need answers now. The reserved slots support any direction.
