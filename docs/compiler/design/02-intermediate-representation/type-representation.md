---
title: "Type Representation"
description: "Ori Compiler Design — Type Representation"
order: 204
section: "Intermediate Representation"
---

# Type Representation

This document describes how types are represented in the Ori compiler. The design uses a **Pool-based architecture** where every type is a 32-bit index (`Idx`) into a unified storage pool. This enables O(1) equality checks (pointer comparison), cache-friendly iteration, and efficient interning.

The split is:
- **`ori_ir`** contains `TypeId` — a flat `u32` index with pre-interned primitive constants, used by the parser
- **`ori_types`** contains the full type system — `Idx`, `Tag`, `Item`, `Pool`, `TypeFlags`, and all type construction/query operations

## TypeId (in ori_ir)

`TypeId` is a `#[repr(transparent)]` newtype over `u32` used at the parser level. It provides classification predicates for fast dispatch without consulting the type pool:

```rust
impl TypeId {
    pub const PRIMITIVE_COUNT: u32 = 12;  // int through ordering
    pub const FIRST_COMPOUND: u32 = 64;   // First dynamically allocated type

    pub const fn is_primitive(self) -> bool { self.0 < Self::PRIMITIVE_COUNT }
    pub const fn is_infer(self) -> bool { self.0 == Self::INFER.0 }
    pub const fn is_self_type(self) -> bool { self.0 == Self::SELF_TYPE.0 }
    pub const fn is_error(self) -> bool { self.0 == Self::ERROR.0 }
}
```

Primitive constants (matching `Idx` 0-11):

| Index | TypeId | Type |
|-------|--------|------|
| 0 | `INT` | 64-bit signed integer |
| 1 | `FLOAT` | IEEE 754 double |
| 2 | `BOOL` | Boolean |
| 3 | `STR` | UTF-8 string |
| 4 | `CHAR` | Unicode scalar value |
| 5 | `BYTE` | 8-bit unsigned |
| 6 | `UNIT` | Unit type `()` |
| 7 | `NEVER` | Bottom type |
| 8 | `ERROR` | Poison type |
| 9 | `DURATION` | Time duration |
| 10 | `SIZE` | Memory size |
| 11 | `ORDERING` | Comparison result |

Special markers (NOT stored in the type pool):
- `INFER = 12` — placeholder during inference
- `SELF_TYPE = 13` — Self type in trait/impl contexts

## Idx (in ori_types)

`Idx` is the universal type handle used throughout the type checker. Like `TypeId`, it is a `#[repr(transparent)]` newtype over `u32`. Both share the same layout for primitives 0-11 so they can be used interchangeably for those cases.

```rust
pub struct Idx(u32);

impl Idx {
    pub const INT: Self = Self(0);
    pub const FLOAT: Self = Self(1);
    // ... through ORDERING = 11
    pub const FIRST_DYNAMIC: u32 = 64;
    pub const NONE: Self = Self(u32::MAX);  // Sentinel for invalid/no type
}
```

The bridge between the two: `resolve_type_id()` maps `TypeId → Idx` (identity for primitives, pool lookup for compounds).

## Tag (Type Kind Discriminant)

`Tag` is a `#[repr(u8)]` enum that identifies the kind of each type. It organizes types into ranges:

| Range | Category | data field meaning |
|-------|----------|--------------------|
| 0-15 | Primitives | Unused (always 0) |
| 16-31 | Simple containers | Child `Idx` (raw u32) |
| 32-47 | Two-child containers | Index into extra array |
| 48-79 | Complex types | Index into extra array (length-prefixed) |
| 80-95 | Named types | Index into extra array |
| 96-111 | Type variables | Variable ID into var_states |
| 112-127 | Type schemes | Index into extra array |
| 240-255 | Special types | Varies |

**Primitive tags** (0-15):

```
Int=0  Float=1  Bool=2  Str=3  Char=4  Byte=5  Unit=6  Never=7
Error=8  Duration=9  Size=10  Ordering=11
```

**Simple containers** (16-31) — `data` = child Idx:

```
List=16  Option=17  Set=18  Channel=19  Range=20  Iterator=21  DoubleEndedIterator=22
```

**Two-child containers** (32-47) — `data` = extra array index:

```
Map=32  Result=33  Borrowed=34
```

**Complex types** (48-79) — `data` = extra array index with length prefix:

```
Function=48  Tuple=49  Struct=50  Enum=51
```

**Named types** (80-95):

```
Named=80  Applied=81  Alias=82
```

**Type variables** (96-111):

```
Var=96  BoundVar=97  RigidVar=98
```

**Schemes** (112-127):

```
Scheme=112
```

**Special** (240-255):

```
Projection=240  ModuleNs=241  Infer=254  SelfType=255
```

Key predicates on `Tag`:
- `is_primitive()` — tag value < 16
- `is_container()` — tag value in 16..48
- `is_type_variable()` — Var, BoundVar, or RigidVar
- `uses_extra()` — data field points into the extra array (not a direct child or variable ID)

## Item (Compact Type Storage)

Each type in the pool is stored as an `Item`:

```rust
#[repr(C)]
pub struct Item {
    pub tag: Tag,   // 1 byte — type kind
    pub data: u32,  // 4 bytes — meaning depends on tag
}
```

Logically 5 bytes (may be padded to 8 by the compiler due to alignment). The interpretation of `data` depends on the tag category:

| Category | `data` meaning | Example |
|----------|---------------|---------|
| Primitive | Always 0 | `Item { tag: Int, data: 0 }` |
| Simple container | Child Idx as raw u32 | `Item { tag: List, data: elem_idx.raw() }` |
| Two-child | Extra array index | `Item { tag: Map, data: 42 }` → extra[42..44] |
| Complex | Extra array index | `Item { tag: Function, data: 100 }` → extra[100..] |
| Named | Extra array index | `Item { tag: Named, data: 80 }` → extra[80..82] |
| Variable | Variable ID | `Item { tag: Var, data: 3 }` → var_states[3] |
| Scheme | Extra array index | `Item { tag: Scheme, data: 50 }` → extra[50..] |

Constructors:
- `Item::primitive(tag)` — data = 0
- `Item::simple_container(tag, child)` — data = child.raw()
- `Item::with_extra(tag, extra_idx)` — data = index into extra array
- `Item::var(tag, var_id)` — data = variable index

## Pool (Unified Type Storage)

The `Pool` is the central type storage. All types live here as interned, deduplicated entries:

```rust
pub struct Pool {
    items: Vec<Item>,                    // All type items (parallel array)
    flags: Vec<TypeFlags>,               // Pre-computed metadata (parallel)
    hashes: Vec<u64>,                    // Stable hashes (parallel)
    extra: Vec<u32>,                     // Variable-length data for complex types
    intern_map: FxHashMap<u64, Idx>,     // Hash → Idx for deduplication
    resolutions: FxHashMap<Idx, Idx>,    // Named → Struct/Enum resolution
    var_states: Vec<VarState>,           // State for each type variable
    next_var_id: u32,                    // Counter for fresh variable IDs
}
```

### Initialization

`Pool::new()` pre-interns 12 primitives at fixed indices 0-11 and pads to `FIRST_DYNAMIC` (64). This ensures all primitive Idx constants are valid without any construction.

### Extra Array Layouts

Complex types store variable-length data in the `extra` array. The `data` field in `Item` points to the start:

| Tag | Extra layout |
|-----|-------------|
| `Function` | `[param_count, param0, param1, ..., return_type]` |
| `Tuple` | `[elem_count, elem0, elem1, ...]` |
| `Struct` | `[name_lo, name_hi, field_count, f0_name, f0_type, ...]` |
| `Enum` | `[name_lo, name_hi, variant_count, v0_name, v0_field_count, v0_f0, ..., v1_name, ...]` |
| `Named` | `[name_lo, name_hi]` |
| `Applied` | `[name_lo, name_hi, arg_count, arg0, arg1, ...]` |
| `Scheme` | `[var_count, var0, var1, ..., body_type]` |
| `Map`/`Result` | `[child0, child1]` |
| `Borrowed` | `[inner_idx, lifetime_id]` |

Names are stored as two u32 words (`name_lo`, `name_hi`) representing the `Name` interned identifier split across the 64-bit space.

### Type Variable State

Type variables (tags Var, BoundVar, RigidVar) use the `var_states` array:

```rust
pub enum VarState {
    Unbound { id: u32, rank: Rank, name: Option<Name> },
    Link { target: Idx },
    Rigid { name: Name },
    Generalized { id: u32, name: Option<Name> },
}
```

Unification links variables by setting `VarState::Link { target }`. Path compression in `resolve()` follows links to the final target.

### Interning and Deduplication

Every type constructed through the Pool is interned via `intern_map`. The stable hash is computed from the tag and data/extra content. If a hash collision is found, structural equality is checked. This ensures each unique type exists exactly once, enabling O(1) equality via `idx1 == idx2`.

### Resolution

Named types (`Tag::Named`, `Tag::Applied`) reference user-defined type names. During type checking, `set_resolution()` records the mapping from the named reference to the concrete struct/enum type. `resolve()` and `resolve_fully()` follow these resolution chains.

## TypeFlags (Pre-Computed Metadata)

`TypeFlags` is a 32-bit bitfield computed once at interning time. It enables O(1) property queries without type traversal:

**Presence flags** (bits 0-7) — what elements does this type contain:
- `HAS_VAR` — unbound type variables
- `HAS_BOUND_VAR` — bound/quantified variables
- `HAS_RIGID_VAR` — rigid variables (from annotations)
- `HAS_ERROR` — error type
- `HAS_INFER` — inference placeholders
- `HAS_SELF` — Self type
- `HAS_PROJECTION` — type projections

**Category flags** (bits 8-15) — what kind of type:
- `IS_PRIMITIVE`, `IS_CONTAINER`, `IS_FUNCTION`, `IS_COMPOSITE`, `IS_NAMED`, `IS_SCHEME`

**Optimization flags** (bits 16-23) — can we skip operations:
- `NEEDS_SUBST` — has variables needing substitution
- `IS_RESOLVED` — fully resolved
- `IS_MONO` — monomorphic
- `IS_COPYABLE` — known Copy type

**Capability flags** (bits 24-31):
- `HAS_CAPABILITY`, `IS_PURE`, `HAS_IO`, `HAS_ASYNC`

Flags propagate from children to parents via `PROPAGATE_MASK` (bitwise OR). This means checking if a type contains any variables is O(1) regardless of nesting depth.

## Type Construction

The `Pool` provides builder methods for all type kinds:

```rust
impl Pool {
    // Simple containers
    pub fn list(&mut self, elem: Idx) -> Idx { ... }
    pub fn option(&mut self, inner: Idx) -> Idx { ... }
    pub fn set(&mut self, elem: Idx) -> Idx { ... }
    pub fn iterator(&mut self, elem: Idx) -> Idx { ... }

    // Two-child containers
    pub fn map(&mut self, key: Idx, value: Idx) -> Idx { ... }
    pub fn result(&mut self, ok: Idx, err: Idx) -> Idx { ... }

    // Complex types
    pub fn function(&mut self, params: &[Idx], ret: Idx) -> Idx { ... }
    pub fn tuple(&mut self, elems: &[Idx]) -> Idx { ... }
    pub fn struct_type(&mut self, name: Name, fields: &[(Name, Idx)]) -> Idx { ... }
    pub fn enum_type(&mut self, name: Name, variants: &[EnumVariant]) -> Idx { ... }

    // Variables
    pub fn fresh_var(&mut self) -> Idx { ... }
    pub fn rigid_var(&mut self, name: Name) -> Idx { ... }

    // Schemes
    pub fn scheme(&mut self, vars: &[u32], body: Idx) -> Idx { ... }

    // Convenience
    pub fn function1(&mut self, param: Idx, ret: Idx) -> Idx { ... }
    pub fn pair(&mut self, a: Idx, b: Idx) -> Idx { ... }
    pub fn list_str(&mut self) -> Idx { ... }
}
```

All constructors intern the result, returning the deduplicated `Idx`.

## Type Queries

Pool provides O(1) accessors for type properties:

```rust
impl Pool {
    // Core queries
    pub fn tag(&self, idx: Idx) -> Tag { ... }
    pub fn flags(&self, idx: Idx) -> TypeFlags { ... }
    pub fn item(&self, idx: Idx) -> Item { ... }

    // Container accessors
    pub fn list_elem(&self, idx: Idx) -> Idx { ... }
    pub fn option_inner(&self, idx: Idx) -> Idx { ... }
    pub fn map_key(&self, idx: Idx) -> Idx { ... }
    pub fn result_ok(&self, idx: Idx) -> Idx { ... }

    // Complex type accessors
    pub fn function_param_count(&self, idx: Idx) -> usize { ... }
    pub fn function_params(&self, idx: Idx) -> &[u32] { ... }
    pub fn tuple_elem_count(&self, idx: Idx) -> usize { ... }
    pub fn struct_field(&self, idx: Idx, name: Name) -> Option<Idx> { ... }
    pub fn enum_variant(&self, idx: Idx, index: usize) -> (Name, Vec<Idx>) { ... }

    // Variable queries
    pub fn var_state(&self, idx: Idx) -> &VarState { ... }

    // Resolution
    pub fn resolve(&self, idx: Idx) -> Idx { ... }
    pub fn resolve_fully(&self, idx: Idx) -> Idx { ... }
}
```

## Type Display

Types are formatted for error messages via `Pool::format()`:

```rust
// Primitives
pool.format(Idx::INT)    // → "int"
pool.format(Idx::STR)    // → "str"

// Containers
pool.format(list_int)     // → "[int]"
pool.format(map_str_int)  // → "{str: int}"
pool.format(option_str)   // → "Option<str>"

// Functions
pool.format(fn_type)      // → "(int, int) -> int"

// User types
pool.format(point_type)   // → "Point"
```

The `format/` submodule handles all formatting, using `StringLookup` to resolve interned `Name` values to strings.

## Design Rationale

### Why Pool, Not Enum?

A traditional `Type` enum (with `Box<Type>` for recursion) has several drawbacks:
- **O(n) equality**: Structural comparison requires traversing the entire tree
- **Cache-unfriendly**: Box indirection scatters type nodes across the heap
- **Allocation overhead**: Each compound type requires heap allocation
- **No deduplication**: Identical types exist as separate allocations

The Pool design addresses all of these:
- **O(1) equality**: `idx1 == idx2` (both point to the same interned entry)
- **Cache-friendly**: All items in a contiguous `Vec<Item>`
- **Zero allocation for queries**: Constructing an `Idx` is just a u32 copy
- **Automatic deduplication**: Interning ensures each unique type exists once

### Inspiration

The Pool architecture is inspired by:
- **Zig's `InternPool`**: Flat storage with tag-driven dispatch, extra arrays for variable-length data
- **Rust's `rustc_type_ir`**: TypeFlags for pre-computed metadata, bitflag propagation
- **Lean 4's `IRType`**: Compact type classification for ARC analysis

## Related Documents

- [Pool Architecture](../05-type-system/pool-architecture.md) - Detailed Pool internals
- [Type Inference](../05-type-system/type-inference.md) - How types are inferred
- [Unification](../05-type-system/unification.md) - How type variables are resolved
