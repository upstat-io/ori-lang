---
section: "02"
title: Pre-Computed Metadata
status: in-progress
goal: Cache type properties at interning time for O(1) queries
sections:
  - id: "02.1"
    title: TypeFlags Definition
    status: complete
  - id: "02.2"
    title: Presence Flags
    status: complete
  - id: "02.3"
    title: Category Flags
    status: complete
  - id: "02.4"
    title: Optimization Flags
    status: complete
  - id: "02.5"
    title: Capability Flags
    status: in-progress
  - id: "02.6"
    title: Flag Computation
    status: complete
  - id: "02.7"
    title: Stable Hash Caching
    status: complete
  - id: "02.8"
    title: Completion Checklist
    status: in-progress
---

# Section 02: Pre-Computed Metadata

**Status:** In Progress (~90% complete)
**Goal:** Cache type properties at interning time for O(1) queries
**Source:** Rust (`rustc_type_ir/src/flags.rs`), TypeScript (`types.ts`)

---

## Naming Clarification

> **Note:** This section's "metadata" (`TypeFlags`) is unrelated to Parser V2's "metadata" (`ModuleExtra`).
>
> | This Section | Parser V2 Section 6 |
> |--------------|---------------------|
> | `TypeFlags` — type properties | `ModuleExtra` — formatting trivia |
> | `ori_types/src/flags.rs` | `ori_ir/src/metadata.rs` |
> | Optimization gates | Formatter/IDE support |
>
> The naming overlap is coincidental. These are orthogonal systems.

---

## Background

### Current Problems

1. **No pre-computed metadata** — Every query requires traversal
2. **Repeated computation** — Same checks done multiple times
3. **No optimization gates** — Can't skip unnecessary work

### Solution from Rust

Rust's `WithCachedTypeInfo` pattern caches:
- TypeFlags (26 bits of properties)
- Stable hash (for incremental compilation)
- Outer exclusive binder (De Bruijn depth)

This enables O(1) checks like `has_type_flags(HAS_VAR)` instead of O(n) traversal.

---

## 02.1 TypeFlags Definition

**Goal:** Define the bitflags type with all categories

### Design

```rust
bitflags::bitflags! {
    /// Pre-computed type properties for O(1) queries.
    /// Computed once at interning time, never recomputed.
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
    pub struct TypeFlags: u32 {
        // === Presence Flags (bits 0-7) ===
        const HAS_VAR        = 1 << 0;
        const HAS_BOUND_VAR  = 1 << 1;
        const HAS_RIGID_VAR  = 1 << 2;
        const HAS_ERROR      = 1 << 3;
        const HAS_INFER      = 1 << 4;
        const HAS_SELF       = 1 << 5;
        const HAS_PROJECTION = 1 << 6;

        // === Category Flags (bits 8-15) ===
        const IS_PRIMITIVE   = 1 << 8;
        const IS_CONTAINER   = 1 << 9;
        const IS_FUNCTION    = 1 << 10;
        const IS_COMPOSITE   = 1 << 11;
        const IS_NAMED       = 1 << 12;
        const IS_SCHEME      = 1 << 13;

        // === Optimization Flags (bits 16-23) ===
        const NEEDS_SUBST    = 1 << 16;
        const IS_RESOLVED    = 1 << 17;
        const IS_MONO        = 1 << 18;
        const IS_COPYABLE    = 1 << 19;

        // === Capability Flags (bits 24-31) ===
        const HAS_CAPABILITY = 1 << 24;
        const IS_PURE        = 1 << 25;
        const HAS_IO         = 1 << 26;
        const HAS_ASYNC      = 1 << 27;
    }
}
```

### Tasks

- [x] Create `ori_types/src/flags.rs` ✅
- [x] Define `TypeFlags` using `bitflags!` macro ✅
- [x] Ensure derives: `Copy, Clone, Eq, PartialEq, Hash, Debug` ✅
- [x] Document each flag's meaning ✅
- [x] Add helper methods for common flag combinations ✅

---

## 02.2 Presence Flags

**Goal:** Track what elements a type contains

### Flag Definitions

| Flag | Meaning | Use Case |
|------|---------|----------|
| `HAS_VAR` | Contains unbound type variables | Skip substitution if false |
| `HAS_BOUND_VAR` | Contains bound/quantified variables | Skip instantiation if false |
| `HAS_RIGID_VAR` | Contains rigid (annotation) variables | Error checking |
| `HAS_ERROR` | Contains Error type | Error propagation |
| `HAS_INFER` | Contains Infer placeholder | Incomplete inference |
| `HAS_SELF` | Contains SelfType | Trait context |
| `HAS_PROJECTION` | Contains type projection | Associated types |

### Tasks

- [x] Define all presence flags ✅
- [x] Add `has_vars(&self) -> bool` helper ✅
- [x] Add `has_any_var(&self) -> bool` (VAR | BOUND_VAR | RIGID_VAR) ✅
- [x] Add `has_errors(&self) -> bool` ✅

---

## 02.3 Category Flags

**Goal:** Classify types for fast dispatch

### Flag Definitions

| Flag | Meaning | Types Included |
|------|---------|----------------|
| `IS_PRIMITIVE` | Built-in primitive | Int, Float, Bool, Str, etc. |
| `IS_CONTAINER` | Generic container | List, Option, Set, Map, Result |
| `IS_FUNCTION` | Function type | Function |
| `IS_COMPOSITE` | User-defined composite | Struct, Enum, Tuple |
| `IS_NAMED` | Named type reference | Named, Applied, Alias |
| `IS_SCHEME` | Type scheme (quantified) | Scheme |

### Tasks

- [x] Define all category flags ✅
- [x] Add `category(&self) -> TypeCategory` helper ✅ (2026-02-04)
- [x] Ensure categories are mutually exclusive where appropriate ✅ (tested)

---

## 02.4 Optimization Flags

**Goal:** Enable optimization shortcuts

### Flag Definitions

| Flag | Meaning | Optimization |
|------|---------|--------------|
| `NEEDS_SUBST` | Has vars needing substitution | Skip subst pass if false |
| `IS_RESOLVED` | Fully resolved, no holes | Skip resolution pass |
| `IS_MONO` | Monomorphic, no generics | Skip instantiation |
| `IS_COPYABLE` | Known to be Copy type | Memory optimization |

### Tasks

- [x] Define all optimization flags ✅
- [x] Add `needs_work(&self) -> bool` helper ✅
- [x] Document when each optimization applies ✅

---

## 02.5 Capability Flags

**Goal:** Track Ori's capability/effect information

### Flag Definitions

| Flag | Meaning | Use Case |
|------|---------|----------|
| `HAS_CAPABILITY` | Uses capabilities | Effect checking |
| `IS_PURE` | Guaranteed pure | Optimization |
| `HAS_IO` | Has IO effects | Effect checking |
| `HAS_ASYNC` | Has async effects | Effect checking |

### Tasks

- [x] Define capability flags ✅
- [x] Integrate with Ori's capability system ✅ (2026-02-05, InferEngine capability tracking + propagation checking)
- [ ] Add `effects(&self) -> EffectFlags` helper — deferred to full effect system

---

## 02.6 Flag Computation

**Goal:** Compute flags during interning

### Design

```rust
impl Pool {
    fn compute_flags(&self, tag: Tag, data: u32) -> TypeFlags {
        match tag {
            // Primitives
            Tag::Int | Tag::Float | Tag::Bool | Tag::Str |
            Tag::Char | Tag::Byte | Tag::Unit | Tag::Duration |
            Tag::Size | Tag::Ordering => {
                TypeFlags::IS_PRIMITIVE | TypeFlags::IS_RESOLVED | TypeFlags::IS_MONO
            }

            Tag::Never => {
                TypeFlags::IS_PRIMITIVE | TypeFlags::IS_RESOLVED | TypeFlags::IS_MONO
            }

            Tag::Error => {
                TypeFlags::IS_PRIMITIVE | TypeFlags::HAS_ERROR | TypeFlags::IS_RESOLVED
            }

            // Containers: inherit from children
            Tag::List | Tag::Option | Tag::Set | Tag::Channel | Tag::Range => {
                let child_flags = self.flags[data as usize];
                TypeFlags::IS_CONTAINER | self.propagate_presence(child_flags)
            }

            Tag::Map | Tag::Result => {
                let extra_idx = data as usize;
                let child1_flags = self.flags[self.extra[extra_idx] as usize];
                let child2_flags = self.flags[self.extra[extra_idx + 1] as usize];
                TypeFlags::IS_CONTAINER
                    | self.propagate_presence(child1_flags)
                    | self.propagate_presence(child2_flags)
            }

            // Variables
            Tag::Var => {
                TypeFlags::HAS_VAR | TypeFlags::NEEDS_SUBST
            }

            Tag::BoundVar => {
                TypeFlags::HAS_BOUND_VAR | TypeFlags::NEEDS_SUBST
            }

            Tag::RigidVar => {
                TypeFlags::HAS_RIGID_VAR | TypeFlags::NEEDS_SUBST
            }

            // Functions
            Tag::Function => {
                self.compute_function_flags(data)
            }

            // Tuples
            Tag::Tuple => {
                self.compute_tuple_flags(data)
            }

            // ... other tags
            _ => TypeFlags::empty()
        }
    }

    fn propagate_presence(&self, child: TypeFlags) -> TypeFlags {
        child & (TypeFlags::HAS_VAR | TypeFlags::HAS_BOUND_VAR |
                 TypeFlags::HAS_RIGID_VAR | TypeFlags::HAS_ERROR |
                 TypeFlags::HAS_INFER | TypeFlags::HAS_SELF |
                 TypeFlags::HAS_PROJECTION | TypeFlags::NEEDS_SUBST)
    }
}
```

### Tasks

- [x] Implement `compute_flags()` for all tags ✅ (in pool/mod.rs)
- [x] Implement `propagate_presence()` for child flag inheritance ✅ (TypeFlags::propagate_from)
- [x] Implement specialized flag computation for complex types ✅
- [x] Add tests verifying flag correctness ✅

---

## 02.7 Stable Hash Caching

**Goal:** Cache stable hashes for Salsa compatibility

### Design

```rust
impl Pool {
    fn compute_hash(&self, tag: Tag, data: u32) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();

        (tag as u8).hash(&mut hasher);
        data.hash(&mut hasher);

        // For complex types, also hash the extra data
        if tag.uses_extra() {
            let extra_slice = self.get_extra_slice(data);
            extra_slice.hash(&mut hasher);
        }

        hasher.finish()
    }
}
```

### Tasks

- [x] Implement `compute_hash()` for all tag types ✅
- [x] Ensure hashes are stable across runs (no pointer-based hashing) ✅
- [x] Add `Pool::hash(idx: Idx) -> u64` accessor ✅
- [ ] Verify Salsa compatibility with hash values

---

## 02.8 Completion Checklist

- [x] `flags.rs` complete with all flag definitions ✅
- [x] `compute_flags()` implemented for all tags ✅
- [x] `compute_hash()` implemented and stable ✅
- [x] All flags arrays parallel to items array ✅
- [x] O(1) flag queries working ✅
- [ ] Optimization gates tested (e.g., skip subst when !NEEDS_SUBST) — needs Sections 03-06
- [ ] Salsa compatibility verified — needs Sections 03-06
- [x] `category() -> TypeCategory` helper ✅ (2026-02-04)
- [ ] Capability system integration (`effects()` helper) — blocked on Section 06/07

**Section 02 Status:** In Progress (~90%)

**Remaining:**
1. `effects()` helper — deferred to full effect system
2. Salsa compatibility verification — verify hash stability
3. Optimization gate tests — verify flags are checked to skip work

**Exit Criteria:** Every type has pre-computed flags accessible via `pool.flags(idx)`. Flag checks are O(1) and enable optimization shortcuts throughout the type checker.
