---
title: "Pool Architecture"
description: "Ori Compiler Design — Type Pool Architecture"
order: 505
section: "Type System"
---

# Pool Architecture

The type pool is the central data structure of Ori's type system. It stores all types using a Structure-of-Arrays (SoA) layout for cache efficiency, with automatic interning to ensure each unique type is stored exactly once.

## Location

```
compiler/ori_types/src/
├── idx.rs           # Idx handle type
├── tag.rs           # Tag discriminant
├── item.rs          # Item storage record
├── flags.rs         # TypeFlags bitfield
└── pool/
    ├── mod.rs       # Pool struct, queries, variable state
    ├── construct.rs # Type construction (interning + dedup)
    └── format.rs    # Human-readable formatting
```

## Design Rationale

Traditional type system designs use recursive `enum` types with heap allocation:

```rust
// V1 approach (replaced)
enum Type {
    List(Box<Type>),
    Function { params: Vec<Type>, ret: Box<Type> },
    // ...
}
```

This has poor cache locality, high allocation overhead, and expensive equality checks (deep comparison). The pool architecture solves all three:

- **Cache locality**: Parallel arrays enable linear scans with minimal cache misses
- **Zero allocation**: Types are constructed once in the pool; references are 4-byte `Idx` handles
- **O(1) equality**: Same `Idx` means same type (interning guarantee)
- **Cheap metadata**: Pre-computed flags avoid recursive traversals for common queries

This design is inspired by Zig's `InternPool` and Roc's type storage.

## Pool Structure

```rust
pub struct Pool {
    items: Vec<Item>,              // Parallel array: tag + data
    flags: Vec<TypeFlags>,         // Parallel array: cached metadata
    hashes: Vec<u64>,              // Parallel array: for dedup verification
    extra: Vec<u32>,               // Variable-length data (func params, tuple elems)
    intern_map: FxHashMap<u64, Idx>,  // Hash → Idx for deduplication
    var_states: Vec<VarState>,     // Type variable state (separate from items)
    next_var_id: u32,              // Counter for fresh variable generation
}
```

### Parallel Arrays

The `items`, `flags`, and `hashes` vectors are indexed by the same `Idx`. For a given type at index `i`:

- `items[i]` — The `Item` containing the type's `Tag` and data field
- `flags[i]` — Pre-computed `TypeFlags` for O(1) property queries
- `hashes[i]` — The hash used for interning (for dedup verification)

### Item Layout

```rust
#[repr(C)]
pub struct Item {
    pub tag: Tag,     // 1 byte — type kind
    pub data: u32,    // 4 bytes — interpretation depends on tag
}
```

The `data` field is interpreted based on the tag:

| Tag Category | Data Interpretation |
|-------------|-------------------|
| Primitives | Unused (tag is sufficient) |
| Simple containers (`List`, `Option`, etc.) | Child `Idx` as `u32` |
| Two-child containers (`Map`, `Result`) | Index into `extra` array |
| Complex types (`Function`, `Tuple`) | Index into `extra` array |
| Named types (`Named`, `Applied`) | Index into `extra` array or `Name` |
| Type variables (`Var`) | Variable ID for `var_states` lookup |

### Extra Array

Variable-length type data (function parameters, tuple elements, applied type arguments) is stored in the `extra: Vec<u32>` array. The layout for a complex type at extra index `i`:

```
extra[i]     = count (number of children)
extra[i+1]   = child_0 (as u32)
extra[i+2]   = child_1 (as u32)
...
extra[i+n]   = child_{n-1}
extra[i+n+1] = return_type (for functions)
```

For example, a function `(int, str) -> bool`:
```
data = extra_offset
extra[offset]   = 2        (param count)
extra[offset+1] = 0        (Idx::INT)
extra[offset+2] = 3        (Idx::STR)
extra[offset+3] = 2        (Idx::BOOL, return type)
```

## Initialization

`Pool::new()` pre-interns the 12 primitive types at fixed indices 0–11, then pads to index 64. This guarantees that primitive `Idx` values are stable constants:

```rust
impl Pool {
    pub fn new() -> Self {
        let mut pool = Pool { /* empty vecs */ };
        // Primitives at indices 0-11
        pool.push(Tag::Int, 0);      // Idx::INT = 0
        pool.push(Tag::Float, 0);    // Idx::FLOAT = 1
        // ... through Ordering at index 11
        // Pad indices 12-63 (reserved)
        pool
    }
}
```

## Type Construction

Type construction methods live in `pool/construct.rs`. Every method:

1. Computes the hash for the type
2. Checks the intern map for an existing entry
3. If found, returns the existing `Idx` (deduplication)
4. If not found, appends to the parallel arrays and returns the new `Idx`

### Simple Containers

Single-child types store the child `Idx` directly in the data field:

```rust
pub fn list(&mut self, elem: Idx) -> Idx    // [T]
pub fn option(&mut self, inner: Idx) -> Idx  // T?
pub fn set(&mut self, elem: Idx) -> Idx      // set<T>
pub fn channel(&mut self, elem: Idx) -> Idx  // channel<T>
pub fn range(&mut self, elem: Idx) -> Idx    // range<T>
```

### Two-Child Containers

Two-child types store both children in the extra array:

```rust
pub fn map(&mut self, key: Idx, value: Idx) -> Idx     // {K: V}
pub fn result(&mut self, ok: Idx, err: Idx) -> Idx      // result<T, E>
```

### Complex Types

Functions and tuples use the extra array with a length prefix:

```rust
pub fn function(&mut self, params: &[Idx], ret: Idx) -> Idx
pub fn function0(&mut self, ret: Idx) -> Idx              // () -> T
pub fn function1(&mut self, param: Idx, ret: Idx) -> Idx  // (A) -> T
pub fn function2(&mut self, p1: Idx, p2: Idx, ret: Idx) -> Idx
pub fn tuple(&mut self, elems: &[Idx]) -> Idx  // empty tuple → Idx::UNIT
pub fn pair(&mut self, a: Idx, b: Idx) -> Idx
pub fn triple(&mut self, a: Idx, b: Idx, c: Idx) -> Idx
```

### Type Variables

Variables are tracked separately in `var_states`, not in the main `items` array's extra data:

```rust
pub fn fresh_var(&mut self) -> Idx
pub fn fresh_var_with_rank(&mut self, rank: Rank) -> Idx
pub fn fresh_named_var(&mut self, name: Name) -> Idx
pub fn rigid_var(&mut self, name: Name) -> Idx  // From type annotation
```

### Schemes

```rust
pub fn scheme(&mut self, vars: &[u32], body: Idx) -> Idx
// Returns body directly if vars is empty (monomorphic optimization)
```

### Convenience Shortcuts

Common type patterns have dedicated constructors:

```rust
pub fn list_str(&mut self) -> Idx       // [str]
pub fn list_int(&mut self) -> Idx       // [int]
pub fn option_int(&mut self) -> Idx     // int?
pub fn option_str(&mut self) -> Idx     // str?
pub fn result_str_err(&mut self, ok: Idx) -> Idx  // result<T, str>
pub fn map_str_key(&mut self, value: Idx) -> Idx   // {str: V}
```

## Query Methods

All queries on `Pool` are O(1) since they index directly into the parallel arrays:

```rust
pub fn tag(&self, idx: Idx) -> Tag
pub fn data(&self, idx: Idx) -> u32
pub fn item(&self, idx: Idx) -> &Item
pub fn flags(&self, idx: Idx) -> TypeFlags
pub fn hash(&self, idx: Idx) -> u64
```

### Complex Type Accessors

```rust
// Functions
pub fn function_param_count(&self, idx: Idx) -> usize
pub fn function_param(&self, idx: Idx, i: usize) -> Idx
pub fn function_params(&self, idx: Idx) -> &[u32]
pub fn function_return(&self, idx: Idx) -> Idx

// Tuples
pub fn tuple_elem_count(&self, idx: Idx) -> usize
pub fn tuple_elem(&self, idx: Idx, i: usize) -> Idx
pub fn tuple_elems(&self, idx: Idx) -> &[u32]

// Containers
pub fn list_elem(&self, idx: Idx) -> Idx
pub fn option_inner(&self, idx: Idx) -> Idx
pub fn map_key(&self, idx: Idx) -> Idx
pub fn map_value(&self, idx: Idx) -> Idx
pub fn result_ok(&self, idx: Idx) -> Idx
pub fn result_err(&self, idx: Idx) -> Idx

// Schemes
pub fn scheme_vars(&self, idx: Idx) -> &[u32]
pub fn scheme_body(&self, idx: Idx) -> Idx

// Applied types
pub fn applied_name(&self, idx: Idx) -> Name
pub fn applied_arg_count(&self, idx: Idx) -> usize
pub fn applied_args(&self, idx: Idx) -> &[u32]
```

## Type Variable State

Type variables are managed through a separate `var_states` vector. Each variable has a state:

```rust
pub enum VarState {
    Unbound { id: u32, rank: Rank, name: Option<Name> },
    Link { target: Idx },           // Unified with target (path compression)
    Rigid { name: Name },           // From annotation, cannot be unified
    Generalized { id: u32, name: Option<Name> },  // Captured in type scheme
}
```

The `Link` variant is the key to the union-find approach — see [Unification](unification.md) for details on how path compression works through these links.

## Type Formatting

`pool/format.rs` converts types to human-readable strings for error messages:

```rust
pub fn format_type(&self, idx: Idx) -> String
pub fn format_type_into(&self, idx: Idx, buf: &mut String)
pub fn format_type_resolved(&self, idx: Idx, interner: &StringInterner) -> String
```

Output examples:

| Type | Formatted |
|------|-----------|
| `List(Int)` | `[int]` |
| `Option(Str)` | `str?` |
| `Function([Int, Str], Bool)` | `(int, str) -> bool` |
| `Tuple([Int, Str, Bool])` | `(int, str, bool)` |
| `Map(Str, Int)` | `{str: int}` |
| `Result(Int, Str)` | `result<int, str>` |

The `format_type_resolved` variant uses a `StringInterner` to resolve `Name` values in named types, producing output like `MyStruct` instead of `<named:42>`.

## TypeFlags Design

TypeFlags use `bitflags!` with a `u32` backing type, organized into four 8-bit regions:

```
Bits 0-7:   Presence flags (HAS_VAR, HAS_ERROR, HAS_INFER, ...)
Bits 8-15:  Category flags (IS_PRIMITIVE, IS_CONTAINER, IS_FUNCTION, ...)
Bits 16-23: Optimization flags (NEEDS_SUBST, IS_RESOLVED, IS_MONO, ...)
Bits 24-31: Capability flags (HAS_CAPABILITY, IS_PURE, HAS_IO, ...)
```

A `PROPAGATE_MASK` determines which flags percolate from children to parents during construction. For example, if any child `HAS_VAR`, the parent also gets `HAS_VAR`. This enables powerful O(1) optimizations:

```rust
// Skip occurs check if type has no variables
if !pool.flags(idx).contains(TypeFlags::HAS_VAR) {
    return false; // No variables, no need to check
}
```

`TypeCategory` provides a coarse-grained classification derived from flags:

```rust
pub enum TypeCategory {
    Primitive, Function, Container, Composite,
    Scheme, Named, Variable, Unknown,
}
```
