---
title: "Pool Architecture"
description: "Ori Compiler Design — Type Pool Architecture"
order: 505
section: "Type System"
---

# Pool Architecture

## What Is Type Interning?

Every type in a program is a structured value — `int` is a primitive, `[int]` is a list containing integers, `(int, str) -> bool` is a function taking two parameters and returning a boolean. During compilation, the type checker creates, compares, and manipulates thousands of these type values. The way a compiler *represents* types in memory has profound consequences for performance, correctness, and implementation complexity.

The most natural representation is a recursive tree:

```rust
// The obvious approach
enum Type {
    Int,
    Str,
    List(Box<Type>),
    Function { params: Vec<Type>, ret: Box<Type> },
    // ...
}
```

This is simple and intuitive. `[int]` is `List(Box::new(Int))`. `(int) -> str` is `Function { params: vec![Int], ret: Box::new(Str) }`. But it has three serious problems:

1. **Equality is expensive.** Comparing two types requires recursive structural comparison. For deeply nested types like `Result<[Option<int>], Error>`, this walks the entire tree. In a type checker that performs thousands of unifications, this cost compounds.

2. **Allocation is scattered.** Every `Box` and `Vec` is a separate heap allocation. A single function type might involve 5+ allocations. Cache locality is poor — following a `Box<Type>` pointer chases into a random heap location.

3. **Cloning is deep.** Salsa (Ori's incremental computation framework) requires all query results to implement `Clone + Eq + Hash`. Deep-cloning a recursive type tree is expensive, and deep-hashing it even more so.

**Type interning** solves all three problems by storing each unique type *exactly once* in a shared pool and referring to it by a small handle (typically a 32-bit index). Two types are equal if and only if they have the same handle — a single integer comparison. No tree traversal, no allocation chasing, no deep cloning.

The technique has roots in LISP's symbol tables (1960s), where string interning ensured each symbol existed once in memory. Java interns strings in a similar way. Extending this idea from strings to structured types is the insight behind modern compiler type pools.

### The Hash-Consing Tradition

The specific technique of interning structured values is called **hash-consing**, named after LISP's `cons` operator. The idea appeared in Ershov (1958) and was formalized by Goto (1974). The key insight is that if you hash a value and find the same hash already in the pool, the value already exists — return the existing handle instead of creating a duplicate.

Hash-consing provides three guarantees:

1. **Uniqueness** — each structurally distinct type exists exactly once
2. **Identity by index** — same index means same type (O(1) equality)
3. **Structural sharing** — compound types reference their children by index, not by owned copies

Modern compilers use hash-consing extensively. [Zig](https://github.com/ziglang/zig)'s `InternPool`, [rustc](https://github.com/rust-lang/rust)'s `TyKind` interning, and [GHC](https://gitlab.haskell.org/ghc/ghc)'s `Unique`-based type representation all employ variations of this technique. Ori's pool is most directly inspired by Zig's design.

## What Makes Ori's Pool Distinctive

### Structure-of-Arrays Layout

Most compiler type pools use an Array-of-Structures (AoS) layout — each entry is a struct containing all the data for one type:

```rust
// AoS: each entry is a complete struct
struct TypeEntry {
    tag: Tag,        // 1 byte
    data: u32,       // 4 bytes
    flags: u32,      // 4 bytes
    hash: u64,       // 8 bytes
}
// Vec<TypeEntry> — entries contiguous, fields interleaved
```

Ori instead uses a **Structure-of-Arrays (SoA)** layout — parallel arrays where each array holds one field across all types:

```rust
struct Pool {
    items: Vec<Item>,           // tags + data, packed
    flags: Vec<TypeFlags>,      // metadata, separate
    hashes: Vec<u64>,           // dedup verification, separate
    extra: Vec<u32>,            // variable-length data
    intern_map: FxHashMap<u64, Idx>,
    // ...
}
```

The SoA layout matters because most operations access only one or two fields per type. When the type checker scans for variables (checking `HAS_VAR` flags), it reads only the `flags` array — the `items` and `hashes` arrays don't pollute the CPU cache. When constructing types, only `items` and `extra` are touched. The hot path for type comparison (checking `Idx` equality) doesn't touch the pool at all.

This is the same optimization used in Entity Component Systems (ECS) for game engines, [Apache Arrow](https://arrow.apache.org/) for columnar data, and Zig's `InternPool`. The key insight is that real-world access patterns are columnar, not row-based — you rarely need all fields of an entry at once.

### Fixed-Size Items with External Extra Storage

The `Item` struct is exactly 5 logical bytes (1-byte `Tag` + 4-byte `data`):

```rust
#[repr(C)]
pub struct Item {
    pub tag: Tag,     // 1 byte — type kind
    pub data: u32,    // 4 bytes — interpretation depends on tag
}
```

The `data` field's meaning depends on the tag:

| Tag Category | Data Interpretation |
|-------------|-------------------|
| Primitives (`Int`, `Bool`, ...) | Unused (tag is sufficient) |
| Simple containers (`List`, `Option`, `Set`, ...) | Child `Idx` as `u32` |
| Two-child containers (`Map`, `Result`) | Index into `extra` array |
| Complex types (`Function`, `Tuple`, `Struct`) | Index into `extra` array |
| Named types (`Named`, `Applied`) | Index into `extra` array or `Name` |
| Type variables (`Var`) | Variable ID for `var_states` lookup |

This is a critical design choice: `Item` is fixed-size regardless of how many parameters a function has or how many elements a tuple contains. Variable-length data lives in the separate `extra: Vec<u32>` array, keeping the main `items` array densely packed. A function type `(int, str, bool) -> float` stores its parameter and return type indices in `extra`, with `data` pointing to the start of that span.

### Merkle-Style Hash Propagation

When a compound type is constructed, its hash is computed from its tag and children's hashes — a Merkle tree pattern. This means the hash of `[Option<int>]` incorporates the hash of `Option<int>`, which incorporates the hash of `int`. Two structurally identical types will always produce the same hash, regardless of when or in what order they were constructed.

This Merkle property enables cross-module type identity without sharing pool indices. When module A exports a function returning `[Option<int>]` and module B imports it, they don't need to share the same pool. Each pool independently constructs `[Option<int>]` and arrives at the same hash, which can be used for identity comparison across compilation boundaries.

## Pool Structure

```rust
pub struct Pool {
    items: Vec<Item>,              // Parallel array: tag + data per type
    flags: Vec<TypeFlags>,         // Parallel array: cached metadata
    hashes: Vec<u64>,              // Parallel array: for dedup verification
    extra: Vec<u32>,               // Variable-length data (func params, tuple elems)
    intern_map: FxHashMap<u64, Idx>,  // Hash → Idx for deduplication
    resolutions: FxHashMap<Idx, Idx>, // Named/Applied → concrete Struct/Enum
    var_states: Vec<VarState>,     // Type variable state (separate from items)
    next_var_id: u32,              // Counter for fresh variable generation
}
```

### Parallel Arrays

The `items`, `flags`, and `hashes` vectors are indexed by the same `Idx`. For a given type at index `i`:

- `items[i]` — The `Item` containing the type's `Tag` and data field
- `flags[i]` — Pre-computed `TypeFlags` for O(1) property queries
- `hashes[i]` — The structural hash used for interning deduplication

This parallel array layout means that adding a new type appends one entry to each of the three arrays simultaneously, maintaining index consistency.

### Extra Array Layout

Variable-length type data is stored in the `extra: Vec<u32>` array. The layout for a complex type at extra index `i`:

```
extra[i]     = count (number of children)
extra[i+1]   = child_0 (as u32, interpreted as Idx)
extra[i+2]   = child_1
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

This layout is compact (no per-entry overhead beyond the count prefix) and supports O(1) random access to any parameter by index. The `extra` array grows monotonically — old entries are never modified, only new ones appended.

## Initialization

`Pool::new()` pre-allocates with capacity hints (256 for items/flags/hashes, 1024 for extra) and pre-interns the 12 primitive types at fixed indices 0–11, then pads to index 64:

```rust
impl Pool {
    pub fn new() -> Self {
        let mut pool = Pool { /* empty vecs with capacity hints */ };
        // Primitives at indices 0-11
        pool.push(Tag::Int, 0);      // Idx::INT = 0
        pool.push(Tag::Float, 0);    // Idx::FLOAT = 1
        pool.push(Tag::Bool, 0);     // Idx::BOOL = 2
        // ... through Ordering at index 11
        // Pad indices 12-63 (reserved for future built-in types)
        pool
    }
}
```

This guarantees that primitive `Idx` values are stable constants across all compilations. Checking `idx == Idx::INT` is always correct because `INT` is always index 0. The reserved range 12–63 provides room for future built-in types without breaking existing indices.

## Type Construction

Type construction methods live in the pool's `construct` module. Every method follows the same protocol:

1. Compute the structural hash for the type
2. Check the `intern_map` for an existing entry with that hash
3. If found, return the existing `Idx` (deduplication)
4. If not found, append to the parallel arrays and return the new `Idx`

This protocol guarantees the interning invariant: each structurally unique type exists exactly once.

### Simple Containers

Single-child types store the child `Idx` directly in the `data` field — zero indirection:

```rust
pub fn list(&mut self, elem: Idx) -> Idx       // [T]
pub fn option(&mut self, inner: Idx) -> Idx     // Option<T>
pub fn set(&mut self, elem: Idx) -> Idx         // Set<T>
pub fn channel(&mut self, elem: Idx) -> Idx     // Channel<T>
pub fn range(&mut self, elem: Idx) -> Idx       // Range<T>
```

### Two-Child Containers

Two-child types store both children in the `extra` array:

```rust
pub fn map(&mut self, key: Idx, value: Idx) -> Idx     // {K: V}
pub fn result(&mut self, ok: Idx, err: Idx) -> Idx      // Result<T, E>
```

### Complex Types

Functions and tuples use the `extra` array with a length prefix:

```rust
pub fn function(&mut self, params: &[Idx], ret: Idx) -> Idx
pub fn function0(&mut self, ret: Idx) -> Idx              // () -> T (arity-specialized)
pub fn function1(&mut self, param: Idx, ret: Idx) -> Idx  // (A) -> T
pub fn function2(&mut self, p1: Idx, p2: Idx, ret: Idx) -> Idx
pub fn tuple(&mut self, elems: &[Idx]) -> Idx  // empty tuple → Idx::UNIT
pub fn pair(&mut self, a: Idx, b: Idx) -> Idx
pub fn triple(&mut self, a: Idx, b: Idx, c: Idx) -> Idx
```

The arity-specialized constructors (`function0`, `function1`, `pair`, `triple`) avoid slice allocation for the most common cases. Since zero-argument and single-argument functions are extremely frequent, these shortcuts measurably reduce allocation pressure.

### Type Variables

Variables are tracked separately in `var_states`, not in the main `items` array's extra data:

```rust
pub fn fresh_var(&mut self) -> Idx
pub fn fresh_var_with_rank(&mut self, rank: Rank) -> Idx
pub fn fresh_named_var(&mut self, name: Name) -> Idx
pub fn rigid_var(&mut self, name: Name) -> Idx  // From type annotation
```

A fresh variable creates an `Item` with `Tag::Var` and `data = var_id`, plus a `VarState::Unbound` entry in `var_states`. The `Idx` lives in the main pool for uniform handling, but the variable's mutable state (unbound, linked, generalized) lives in the separate `var_states` vector, which can be mutated during unification without affecting the append-only type arrays.

### Named and Applied Types

Named types represent unresolved type references from the parser. Applied types add generic arguments:

```rust
pub fn named(&mut self, name: Name) -> Idx                   // e.g., Point
pub fn applied(&mut self, name: Name, args: &[Idx]) -> Idx   // e.g., Option<int>
pub fn struct_type(&mut self, name: Name, fields: &[(Name, Idx)]) -> Idx
pub fn enum_type(&mut self, name: Name, variants: &[EnumVariant]) -> Idx
```

The `Named → Struct/Enum` resolution is tracked in the `resolutions` map. When the parser creates `Named("Point")`, it doesn't know Point's fields. During Pass 0 (type registration), the type checker creates the concrete `Struct` type with fields and records `Named("Point") → Struct(Point)` in `resolutions`. Later phases can resolve named types without accessing the `TypeRegistry`.

### Schemes

```rust
pub fn scheme(&mut self, vars: &[u32], body: Idx) -> Idx
// Returns body directly if vars is empty (monomorphic optimization)
```

A scheme wraps a type body with a list of generalized variable IDs. When `vars` is empty, the type is monomorphic and the scheme wrapper is elided entirely — an important optimization since most bindings are monomorphic.

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
pub fn function_param(&self, idx: Idx, i: usize) -> Idx  // O(1) random access
pub fn function_params(&self, idx: Idx) -> Vec<Idx>       // allocating
pub fn function_return(&self, idx: Idx) -> Idx

// Tuples
pub fn tuple_elem_count(&self, idx: Idx) -> usize
pub fn tuple_elem(&self, idx: Idx, i: usize) -> Idx
pub fn tuple_elems(&self, idx: Idx) -> Vec<Idx>

// Containers
pub fn list_elem(&self, idx: Idx) -> Idx     // O(1), data field
pub fn option_inner(&self, idx: Idx) -> Idx   // O(1), data field
pub fn map_key(&self, idx: Idx) -> Idx
pub fn map_value(&self, idx: Idx) -> Idx
pub fn result_ok(&self, idx: Idx) -> Idx
pub fn result_err(&self, idx: Idx) -> Idx
```

Note the distinction between `function_param(idx, i)` (O(1), returns one parameter) and `function_params(idx)` (allocating, returns all parameters as a `Vec`). The single-parameter accessor is preferred in hot paths where only one parameter is needed.

## Type Variable State

Type variables are managed through a separate `var_states: Vec<VarState>` vector, indexed by variable ID (not by `Idx`). This separation is a deliberate design choice: the main parallel arrays (`items`, `flags`, `hashes`) are append-only after construction, but variable state changes frequently during unification. Keeping mutable state in a separate vector avoids contaminating the immutable arrays.

```rust
pub enum VarState {
    Unbound { id: u32, rank: Rank, name: Option<Name> },
    Link { target: Idx },
    Rigid { name: Name },
    Generalized { id: u32, name: Option<Name> },
}
```

**`Unbound`** — A fresh, unconstrained type variable. The `rank` field tracks scope depth for let-polymorphism (variables at higher ranks can be generalized when exiting their scope). The optional `name` provides better error messages — `T` from a type annotation vs. an anonymous `?a`.

**`Link`** — The variable has been unified with another type. The `target` points to the unified type (which may itself be another variable, forming a chain). Path compression during `resolve()` updates intermediate links to point directly to the final target:

```
Before resolve: T0 → T1 → T2 → int
After resolve:  T0 → int, T1 → int, T2 → int
```

**`Rigid`** — A type variable from a user annotation (e.g., `T` in `@foo<T>(x: T)`). Rigid variables cannot be unified with concrete types or other rigid variables with different names — they represent universally quantified type parameters that must remain abstract.

**`Generalized`** — A variable captured in a type scheme during let-polymorphism generalization. When the scheme is instantiated at a use site, each generalized variable is replaced with a fresh `Unbound` variable at the current rank.

## TypeFlags

TypeFlags use `bitflags!` with a `u32` backing type, organized into four 8-bit regions:

```
Bits 0-7:   Presence flags (HAS_VAR, HAS_ERROR, HAS_INFER, ...)
Bits 8-15:  Category flags (IS_PRIMITIVE, IS_CONTAINER, IS_FUNCTION, ...)
Bits 16-23: Optimization flags (NEEDS_SUBST, IS_RESOLVED, IS_MONO, ...)
Bits 24-31: Capability flags (HAS_CAPABILITY, IS_PURE, HAS_IO, ...)
```

### Flag Propagation

A `PROPAGATE_MASK` determines which flags percolate from children to parents during construction. For example, if any child `HAS_VAR`, the parent also gets `HAS_VAR`. This enables powerful O(1) optimizations:

```rust
// Skip occurs check — the most impactful optimization
if !pool.flags(idx).contains(TypeFlags::HAS_VAR) {
    return false;  // No variables anywhere in this type tree
}

// Skip substitution
if !pool.flags(idx).contains(TypeFlags::NEEDS_SUBST) {
    return idx;  // Nothing to substitute, return as-is
}

// Check if type is fully resolved
if pool.flags(idx).contains(TypeFlags::IS_MONO) {
    // Can skip generalization check
}
```

Without propagated flags, each of these checks would require a full recursive traversal of the type tree. With flags, they're a single bitwise AND operation — O(1) regardless of type complexity.

### TypeCategory

`TypeCategory` provides a coarse-grained classification derived from flags:

```rust
pub enum TypeCategory {
    Primitive, Function, Container, Composite,
    Scheme, Named, Variable, Unknown,
}
```

This is useful for error messages and diagnostic formatting, where the detailed tag is too granular but the broad category communicates intent ("expected a function, got a container").

## Named Type Resolutions

The `resolutions: FxHashMap<Idx, Idx>` field bridges parser-level type references to their concrete pool definitions. The parser creates `Named` types (e.g., `Named("Point")`) because it doesn't know the type's fields at parse time. During type registration, `set_resolution()` records the mapping to the concrete `Struct`/`Enum` `Idx`:

```rust
impl Pool {
    pub fn set_resolution(&mut self, named: Idx, concrete: Idx);
    pub fn resolve(&self, idx: Idx) -> Option<Idx>;
}
```

Resolution follows chains up to a bounded depth (preventing infinite loops from cyclic type aliases). This mapping allows codegen and later phases to resolve types without accessing `TypeRegistry`, keeping the pool self-contained for type queries.

## Type Formatting

The pool includes formatting methods for human-readable type strings in error messages:

```rust
pub fn format_type(&self, idx: Idx) -> String
pub fn format_type_resolved(&self, idx: Idx, interner: &StringInterner) -> String
```

| Type | Formatted |
|------|-----------|
| `List(Int)` | `[int]` |
| `Option(Str)` | `Option<str>` |
| `Function([Int, Str], Bool)` | `(int, str) -> bool` |
| `Tuple([Int, Str, Bool])` | `(int, str, bool)` |
| `Map(Str, Int)` | `{str: int}` |
| `Result(Int, Str)` | `Result<int, str>` |

The `format_type_resolved` variant uses a `StringInterner` to resolve `Name` values, producing `MyStruct` instead of `<named:42>`.

## Prior Art

### Zig's InternPool

[Zig](https://github.com/ziglang/zig)'s `InternPool` (`src/InternPool.zig`) is the most direct influence on Ori's pool design. It uses the same pattern of a flat index-based pool with separate extra storage, and handles both types and compile-time values in the same pool. Zig's InternPool is somewhat larger in scope (it also stores compile-time function evaluation results) but shares the core architectural decisions: fixed-size entries, SoA-style field separation, and hash-based deduplication.

### rustc's TyKind

[Rust's compiler](https://github.com/rust-lang/rust) uses `TyKind` (a recursive enum) with `Ty<'tcx>` (an interned reference into a per-session arena). This is a hybrid approach: the type *definition* is recursive (natural to write), but the *storage* is interned (efficient to compare). Ori goes further by making the definition flat as well — there is no recursive `Type` enum, only `Item(Tag, u32)`.

### GHC's Uniques

[GHC](https://gitlab.haskell.org/ghc/ghc) assigns each type a `Unique` identifier for O(1) comparison, but stores types in a more traditional recursive structure. The `Unique` approach gives the equality benefit of interning without the SoA layout benefit. GHC's trade-off favors implementation simplicity (pattern matching on recursive types is more natural in Haskell) over cache performance.

### V8's Object Shapes

[V8](https://v8.dev/blog/fast-properties) (the JavaScript engine) uses a similar approach for object "shapes" (hidden classes) — each unique object layout is stored once and referenced by a pointer. While not a type system in the static typing sense, V8's shape interning solves the same problem: fast comparison of structural information without deep traversal.

### Entity Component Systems

The SoA layout in Ori's pool follows the same principle as ECS architectures (used in game engines like [Bevy](https://bevyengine.org/) and [flecs](https://www.flecs.dev/)): store each component type in its own contiguous array, indexed by entity ID. This maximizes cache utilization when iterating over a single component, which is exactly what the type checker does when scanning flags or resolving tags.

## Design Tradeoffs

**SoA vs. AoS.** The SoA layout improves cache performance for columnar access (scanning all flags, scanning all tags) at the cost of slightly more complex indexing code and potentially worse performance for row-based access (reading all fields of one type). In practice, type checker access patterns are overwhelmingly columnar, making SoA the right choice.

**Fixed-size Items vs. variable-size entries.** Storing variable-length data in the `extra` array adds indirection for complex types (two memory accesses instead of one) but keeps the main `items` array densely packed. Since most type operations only need the tag (1 byte), this indirection is rarely exercised on hot paths.

**Separate VarState vs. inline in items.** Variable state is mutable (links change during unification), while the items array is append-only. Separating them avoids the design tension of having a mostly-immutable array with some mutable entries. The cost is an additional indirection for variable lookups, which is acceptable since variable operations are less frequent than tag checks.

**Session-scoped pool vs. global interning.** Each compilation creates a fresh pool, so `Idx` values are not globally unique. Cross-module type identity requires Merkle hashing rather than index comparison. A global pool would simplify cross-module operations but create lifetime management complexity and potential contention in parallel compilation.
