---
section: "01"
title: Unified Pool Architecture
status: complete
goal: Replace dual Type/TypeData with single unified pool using 32-bit indices
sections:
  - id: "01.1"
    title: Idx Universal Handle
    status: complete
  - id: "01.2"
    title: Tag Enum
    status: complete
  - id: "01.3"
    title: Item Compact Storage
    status: complete
  - id: "01.4"
    title: Pool Implementation
    status: complete
  - id: "01.5"
    title: Pre-interned Primitives
    status: complete
  - id: "01.6"
    title: Extra Arrays (SoA)
    status: complete
  - id: "01.7"
    title: Type Construction Helpers
    status: complete
  - id: "01.8"
    title: Completion Checklist
    status: complete
---

# Section 01: Unified Pool Architecture

**Status:** Complete
**Goal:** Replace dual Type/TypeData representation with single unified pool
**Source:** Zig (`InternPool.zig`), Roc (`types/src/subs.rs`)

---

## Background

### Current Problems

1. **Dual representation** — `Type` (external) and `TypeData` (internal) require conversion at boundaries
2. **Box<Type>** for recursive types causes heap fragmentation
3. **Type equality** requires deep comparison or interning lookup
4. **No pre-computed metadata** for fast queries

### Solution from Zig/Roc

1. Everything is an `Idx(u32)` — no conversion needed
2. Flat storage in vectors — cache-friendly
3. Equality is `idx1 == idx2` — O(1)
4. Pre-computed flags and hashes at interning time

---

## 01.1 Idx Universal Handle

**Goal:** Define the canonical type handle used everywhere

### Design

```rust
/// A 32-bit index into the type pool.
/// This is THE canonical representation - no other type representation exists.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct Idx(u32);

impl Idx {
    // Pre-interned primitives (compile-time constants, no lookup)
    pub const INT: Self = Self(0);
    pub const FLOAT: Self = Self(1);
    pub const BOOL: Self = Self(2);
    pub const STR: Self = Self(3);
    pub const CHAR: Self = Self(4);
    pub const BYTE: Self = Self(5);
    pub const UNIT: Self = Self(6);
    pub const NEVER: Self = Self(7);
    pub const ERROR: Self = Self(8);
    pub const DURATION: Self = Self(9);
    pub const SIZE: Self = Self(10);
    pub const ORDERING: Self = Self(11);

    // Sentinel
    pub const NONE: Self = Self(u32::MAX);

    // First user-defined type index
    pub const FIRST_DYNAMIC: u32 = 64;

    #[inline]
    pub fn is_primitive(self) -> bool {
        self.0 < Self::FIRST_DYNAMIC
    }

    #[inline]
    pub fn is_none(self) -> bool {
        self.0 == u32::MAX
    }

    #[inline]
    pub fn raw(self) -> u32 {
        self.0
    }
}
```

### Tasks

- [x] Create `ori_types/src/idx.rs` ✅
- [x] Define `Idx` struct with `Copy, Clone, Eq, PartialEq, Hash, Debug` ✅
- [x] Define primitive constants (INT, FLOAT, BOOL, etc.) ✅
- [x] Define NONE sentinel and FIRST_DYNAMIC boundary ✅
- [x] Add helper methods (is_primitive, is_none, raw) ✅
- [x] Add compile-time size assertion: `assert!(size_of::<Idx>() == 4)` ✅

---

## 01.2 Tag Enum

**Goal:** Define type kind discriminant for tag-driven dispatch

### Design

```rust
/// Type kind tag (u8 = 256 possible kinds)
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[repr(u8)]
pub enum Tag {
    // === Primitives (0-15) ===
    Int = 0, Float = 1, Bool = 2, Str = 3, Char = 4, Byte = 5,
    Unit = 6, Never = 7, Error = 8, Duration = 9, Size = 10, Ordering = 11,

    // === Simple Containers (16-31) ===
    // data = child Idx
    List = 16, Option = 17, Set = 18, Channel = 19, Range = 20,

    // === Two-Child Containers (32-47) ===
    // data = index into extra array (two consecutive Idx values)
    Map = 32, Result = 33,

    // === Complex Types (48-79) ===
    // data = index into extra array with length prefix
    Function = 48, Tuple = 49, Struct = 50, Enum = 51,

    // === Named Types (80-95) ===
    Named = 80, Applied = 81, Alias = 82,

    // === Type Variables (96-111) ===
    Var = 96, BoundVar = 97, RigidVar = 98,

    // === Type Schemes (112-127) ===
    Scheme = 112,

    // === Special (240-255) ===
    Projection = 240, ModuleNs = 241, Infer = 254, SelfType = 255,
}
```

### Tasks

- [x] Create `ori_types/src/tag.rs` ✅
- [x] Define `Tag` enum with `#[repr(u8)]` ✅
- [x] Organize tags by category with reserved ranges ✅
- [x] Document data field interpretation for each tag ✅
- [x] Add `Tag::uses_extra(&self) -> bool` helper ✅
- [x] Add compile-time assertion: `assert!(size_of::<Tag>() == 1)` ✅

---

## 01.3 Item Compact Storage

**Goal:** Define the compact storage unit for each type

### Design

```rust
/// A single type item in the pool.
/// 5 bytes: 1 byte tag + 4 bytes data
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct Item {
    pub tag: Tag,
    pub data: u32,
}

// Compile-time size assertion (from Roc)
const _: () = assert!(std::mem::size_of::<Item>() == 5);
```

### Data Interpretation by Tag

| Tag Category | `data` Meaning |
|--------------|----------------|
| Primitives | Unused (0) |
| Simple containers | Child `Idx.raw()` |
| Two-child containers | Index into `extra[]` (2 consecutive u32s) |
| Complex types | Index into `extra[]` (length prefix + data) |
| Type variables | Variable ID |
| Schemes | Index into `extra[]` |

### Tasks

- [x] Create `ori_types/src/item.rs` ✅
- [x] Define `Item` struct with `#[repr(C)]` ✅
- [x] Add compile-time size assertion ✅
- [x] Document data interpretation for each tag category ✅

---

## 01.4 Pool Implementation

**Goal:** Implement the unified type pool with interning

### Design

```rust
/// The unified type pool - single source of truth for all types.
pub struct Pool {
    // === Core Storage ===
    items: Vec<Item>,          // All type items
    extra: Vec<u32>,           // Extra data for complex types

    // === Pre-Computed Metadata (parallel to items) ===
    flags: Vec<TypeFlags>,     // flags[i] = flags for items[i]
    hashes: Vec<u64>,          // hashes[i] = stable hash for items[i]

    // === Deduplication ===
    intern_map: FxHashMap<u64, Idx>,

    // === Type Variable State ===
    var_states: Vec<VarState>, // State for each type variable
    next_var_id: u32,          // Counter for fresh variables

    // === String Interner Reference ===
    strings: SharedInterner,
}
```

### Key Methods

```rust
impl Pool {
    /// Create a new pool with pre-interned primitives.
    pub fn new(strings: SharedInterner) -> Self;

    /// Intern a type, returning its canonical index.
    pub fn intern(&mut self, tag: Tag, data: u32) -> Idx;

    /// Intern a complex type with extra data.
    pub fn intern_complex(&mut self, tag: Tag, extra_data: &[u32]) -> Idx;

    /// Get the tag for an index.
    pub fn tag(&self, idx: Idx) -> Tag;

    /// Get the data for an index.
    pub fn data(&self, idx: Idx) -> u32;

    /// Get the flags for an index.
    pub fn flags(&self, idx: Idx) -> TypeFlags;
}
```

### Tasks

- [x] Create `ori_types/src/pool/mod.rs` ✅
- [x] Define `Pool` struct with all fields ✅
- [x] Implement `Pool::new()` with primitive pre-interning ✅
- [x] Implement `Pool::intern()` with hash-based deduplication ✅
- [x] Implement `Pool::intern_complex()` for complex types ✅
- [x] Implement query methods (tag, data, flags, hash) ✅
- [x] Add `Pool::format_type(idx: Idx) -> String` for debugging ✅ (in pool/format.rs)

---

## 01.5 Pre-interned Primitives

**Goal:** Pre-allocate primitive types at fixed indices

### Design

```rust
impl Pool {
    fn new(strings: SharedInterner) -> Self {
        let mut pool = Self { /* ... */ };

        // Pre-intern primitives at fixed indices
        pool.intern_primitive(Tag::Int);      // index 0
        pool.intern_primitive(Tag::Float);    // index 1
        pool.intern_primitive(Tag::Bool);     // index 2
        pool.intern_primitive(Tag::Str);      // index 3
        pool.intern_primitive(Tag::Char);     // index 4
        pool.intern_primitive(Tag::Byte);     // index 5
        pool.intern_primitive(Tag::Unit);     // index 6
        pool.intern_primitive(Tag::Never);    // index 7
        pool.intern_primitive(Tag::Error);    // index 8
        pool.intern_primitive(Tag::Duration); // index 9
        pool.intern_primitive(Tag::Size);     // index 10
        pool.intern_primitive(Tag::Ordering); // index 11

        // Pad to FIRST_DYNAMIC
        while pool.items.len() < Idx::FIRST_DYNAMIC as usize {
            pool.items.push(Item { tag: Tag::Error, data: 0 });
            pool.flags.push(TypeFlags::HAS_ERROR);
            pool.hashes.push(0);
        }

        pool
    }
}
```

### Tasks

- [x] Implement `intern_primitive()` helper ✅
- [x] Pre-intern all 12 primitive types ✅
- [x] Pad to FIRST_DYNAMIC with error placeholders ✅
- [x] Verify `Idx::INT.raw() == 0`, `Idx::FLOAT.raw() == 1`, etc. ✅
- [x] Add tests verifying primitive indices ✅

---

## 01.6 Extra Arrays (SoA)

**Goal:** Structure-of-Arrays layout for complex types

### Design

For complex types, store variable-length data in separate arrays:

```rust
impl Pool {
    /// Get function parameter types.
    pub fn function_params(&self, idx: Idx) -> &[Idx] {
        debug_assert_eq!(self.tag(idx), Tag::Function);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        let start = extra_idx + 1;
        let end = start + count;
        // Safety: Idx and u32 have same repr
        unsafe { std::slice::from_raw_parts(
            self.extra[start..end].as_ptr() as *const Idx,
            count
        )}
    }

    /// Get function return type.
    pub fn function_return(&self, idx: Idx) -> Idx {
        debug_assert_eq!(self.tag(idx), Tag::Function);
        let extra_idx = self.data(idx) as usize;
        let count = self.extra[extra_idx] as usize;
        Idx(self.extra[extra_idx + 1 + count])
    }
}
```

### Extra Array Layout by Type

| Type | Extra Layout |
|------|--------------|
| Function | `[param_count, param0, param1, ..., return_type]` |
| Tuple | `[elem_count, elem0, elem1, ...]` |
| Map | `[key_type, value_type]` |
| Result | `[ok_type, err_type]` |
| Applied | `[name_idx, arg_count, arg0, arg1, ...]` |
| Scheme | `[var_count, var0, var1, ..., body_type]` |
| Struct | `[field_count, (name0, type0), (name1, type1), ...]` |

### Tasks

- [x] Implement `function_params()`, `function_return()` ✅
- [x] Implement `tuple_elems()` ✅
- [x] Implement `map_key()`, `map_value()` ✅
- [x] Implement `result_ok()`, `result_err()` ✅
- [x] Implement `applied_name()`, `applied_args()` ✅ (2026-02-04)
- [x] Implement `scheme_vars()`, `scheme_body()` ✅
- [x] Add comprehensive tests for each accessor ✅

---

## 01.7 Type Construction Helpers

**Goal:** Convenient methods to create types

### Design

```rust
impl Pool {
    pub fn list(&mut self, elem: Idx) -> Idx {
        self.intern(Tag::List, elem.0)
    }

    pub fn option(&mut self, inner: Idx) -> Idx {
        self.intern(Tag::Option, inner.0)
    }

    pub fn result(&mut self, ok: Idx, err: Idx) -> Idx {
        self.intern_complex(Tag::Result, &[ok.0, err.0])
    }

    pub fn map(&mut self, key: Idx, value: Idx) -> Idx {
        self.intern_complex(Tag::Map, &[key.0, value.0])
    }

    pub fn function(&mut self, params: &[Idx], ret: Idx) -> Idx {
        let mut extra = Vec::with_capacity(params.len() + 2);
        extra.push(params.len() as u32);
        for &p in params {
            extra.push(p.0);
        }
        extra.push(ret.0);
        self.intern_complex(Tag::Function, &extra)
    }

    pub fn tuple(&mut self, elems: &[Idx]) -> Idx {
        if elems.is_empty() {
            return Idx::UNIT;
        }
        let mut extra = Vec::with_capacity(elems.len() + 1);
        extra.push(elems.len() as u32);
        for &e in elems {
            extra.push(e.0);
        }
        self.intern_complex(Tag::Tuple, &extra)
    }

    pub fn fresh_var(&mut self, rank: Rank) -> Idx {
        let id = self.next_var_id;
        self.next_var_id += 1;
        self.var_states.push(VarState::Unbound { id, rank, name: None });
        self.intern(Tag::Var, id)
    }
}
```

### Tasks

- [x] Create `ori_types/src/pool/construct.rs` ✅
- [x] Implement all construction helpers ✅
- [x] Add `fresh_var()` and `fresh_named_var()` ✅
- [x] Add tests for each constructor ✅
- [x] Verify deduplication (same inputs = same Idx) ✅

---

## 01.8 Completion Checklist

- [x] `idx.rs` complete with all constants and helpers ✅
- [x] `tag.rs` complete with all tag variants organized ✅
- [x] `item.rs` complete with size assertion ✅
- [x] `pool/mod.rs` complete with interning ✅
- [x] `pool/construct.rs` complete with all builders ✅
- [x] All primitives pre-interned at correct indices ✅
- [x] Extra array accessors working for all complex types ✅ (2026-02-04)
- [x] Comprehensive test suite passing ✅
- [x] No remnants of old `Type` or `TypeData` ✅ (2026-02-05) — `TypeId` in `ori_ir` is intentional (parser-level type representation, not legacy)

**Section 01 Status:** Complete

**Note:** `TypeId` (in `ori_ir`) coexists with `Idx` (in `ori_types`) by design. `TypeId` is the parser-level type index; `Idx` is the type checker's pool-based handle. Both share the same primitive index layout (0-11) for zero-cost bridging via `resolve_type_id()`.

**Exit Criteria:** `Idx` is the only type handle used throughout the codebase. Pool provides all type construction and query operations.
