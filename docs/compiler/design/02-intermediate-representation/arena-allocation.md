---
title: "Arena Allocation"
description: "Ori Compiler Design — Arena Allocation"
order: 201
section: "Intermediate Representation"
---

# Arena Allocation

## What Is Arena Allocation?

**Arena allocation** (also called region-based allocation) is a memory management strategy where objects are allocated sequentially from a large, pre-allocated block of memory and freed all at once when the block is no longer needed. Instead of tracking each object's lifetime individually — allocating and freeing each one through the system allocator — the arena groups objects with similar lifetimes and reclaims their memory in a single operation.

The idea has deep roots in systems programming. Compilers are a natural fit because compilation is inherently phase-structured: the parser creates thousands of AST nodes, all of which live until the end of parsing (or until downstream phases finish reading them), and then all become garbage simultaneously. This lifecycle — allocate many, use for a phase, discard all at once — is exactly what arenas optimize for.

The academic foundations go back to Tofte and Talpin's work on [region-based memory management](https://dl.acm.org/doi/10.1145/174675.177855) (1994), which showed that memory regions with lexically-scoped lifetimes can be inferred automatically in functional languages. While Ori doesn't use inferred regions (it uses explicit arena ownership), the core insight is the same: objects that are born together and die together should be managed together.

## Why Compilers Use Arenas

Three properties of compiler workloads make arenas particularly effective.

### Phase-Scoped Lifetimes

Compilation is organized in phases, and most data has a lifetime bounded by a single phase. Tokens live from lexing until parsing consumes them. AST nodes live from parsing until type checking is done (or longer, if cached). Type information lives from type checking until code generation. This phase structure means objects naturally group into arenas — a "parse arena" that holds AST nodes, a "type arena" that holds types, and so on.

Freeing an arena is a single operation: drop the `Vec` backing the arena, and every object in it is reclaimed. No reference counting, no garbage collection, no finalizer ordering — just `Vec::drop()`, which releases one contiguous allocation back to the system allocator.

### Allocation Density

A parser allocates thousands of AST nodes in rapid succession. Each allocation through the system allocator involves bookkeeping: finding a free block of the right size, updating free lists, potentially acquiring a lock in multithreaded allocators. Arena allocation reduces this to a `Vec::push()` — a bounds check, a memory store, and an index increment. The amortized cost is near-zero because `Vec` pre-allocates capacity, and resizing happens logarithmically.

### Iteration Patterns

Compiler passes typically iterate over all nodes in a collection — all expressions during type checking, all types during monomorphization, all instructions during code generation. When nodes are scattered across the heap (as with individual `Box` allocations), iteration generates cache misses at every step because the CPU cannot predict which memory address will be accessed next. When nodes are contiguous in an arena, iteration is a linear scan — the CPU's hardware prefetcher loads cache lines ahead of the current position, hiding memory latency.

## Classical Arena Designs

### Bump Allocators

The simplest arena is a **bump allocator**: a large byte buffer with a pointer to the next free position. Allocation bumps the pointer forward by the requested size. Deallocation is a no-op — individual objects can't be freed. When the arena is done, the entire buffer is released.

[LLVM's `BumpPtrAllocator`](https://github.com/llvm/llvm-project/blob/main/llvm/include/llvm/Support/Allocator.h) is a well-known example. It allocates objects of any type from a chain of memory "slabs," each slab being a large contiguous allocation. When one slab fills up, a new (larger) one is allocated. The allocator is extremely fast — allocation is literally incrementing a pointer — but it can't free individual objects, and objects of different types are interleaved in the same buffer, reducing type-specific iteration performance.

### Typed Arenas

A **typed arena** (like Rust's [`typed-arena`](https://crates.io/crates/typed-arena) crate) restricts allocation to a single type. All objects in the arena have the same size, which means the arena is effectively a `Vec<T>`. The advantage over a bump allocator is type safety — the arena returns `&T` references rather than raw pointers — and the advantage over individual `Box<T>` allocations is contiguity.

The limitation is that typed arenas return references (`&T`), which carry Rust lifetimes. Any data structure that holds an arena reference must be parameterized by the arena's lifetime, creating infectious `'arena` parameters throughout the codebase. This is particularly problematic for incremental compilation frameworks like Salsa, which need to cache and compare query results — references with lifetimes can't be meaningfully cached across invocations.

### ID-Based Arenas

An **ID-based arena** stores objects in a `Vec<T>` and returns integer indices (`u32`) instead of references. The index is a lightweight, `Copy` handle that can be stored, compared, hashed, and serialized without lifetime concerns. Accessing the object requires the arena — `arena[id]` instead of `*reference` — but the index itself is completely decoupled from the arena's lifetime.

This is the approach Ori uses. The tradeoff is explicitness: every function that needs to inspect an AST node must receive the arena as a parameter. In return, the compiler gets indices that are cheap to copy (4 bytes vs 8-byte pointers), safe to cache in Salsa queries, and trivially comparable for equality.

## Ori's Arena Architecture

Ori's `ExprArena` combines the ID-based arena pattern with a **struct-of-arrays** (SoA) layout and a system of typed side tables for variable-length data.

### Struct-of-Arrays Layout

A traditional ID-based arena stores all fields of each node in a single struct:

```rust
// Array-of-Structs (AoS)
struct AoSArena {
    nodes: Vec<AstNode>,   // AstNode = { kind, span, flags }
}
```

Ori's arena splits each field into its own array:

```rust
// Struct-of-Arrays (SoA) — Ori's design
struct ExprArena {
    expr_kinds: Vec<ExprKind>,   // 24 bytes each — expression structure
    expr_spans: Vec<Span>,       // 8 bytes each — source locations

    // Side tables for variable-length data
    expr_lists: Vec<ExprId>,     // Flattened expression lists
    stmts: Vec<Stmt>,            // Statements
    params: Vec<Param>,          // Function parameters
    arms: Vec<MatchArm>,         // Match arms
    map_entries: Vec<MapEntry>,  // Map literal entries
    // ... 13+ more specialized side tables
}
```

The SoA layout matters because different compilation phases access different fields. Type checking iterates over `expr_kinds` without touching spans. Error reporting reads spans without caring about expression structure. Canonicalization reads both but not the side tables. By separating the fields, each phase pulls only the data it needs into cache.

The concrete benefit: when the type checker iterates over expressions, each cache line (64 bytes) holds roughly 2.5 `ExprKind` values at 24 bytes each. In an AoS layout where each node is `ExprKind(24) + Span(8) = 32 bytes`, only 2 nodes fit per cache line — a 20% reduction in effective throughput for any pass that doesn't need spans.

### Side Tables and Range Types

Variable-length children — function arguments, list elements, match arms, struct fields — are stored in typed side tables. Each side table is a `Vec` of the appropriate type, and expressions reference their children via compact range types:

```rust
#[repr(C)]
pub struct ExprRange {
    start: u32,   // Index into expr_lists
    len: u16,     // Number of elements
    // 2 bytes padding
}
```

When the parser encounters a function call with 4 arguments, it pushes 4 `ExprId` values into the `expr_lists` side table and stores an `ExprRange { start, len: 4 }` in the `ExprKind::Call` variant. The range is 8 bytes — versus 24 bytes for a `Vec<ExprId>` header, plus the heap allocation for its backing storage.

The arena provides a `start/push/finish` API pattern for building these ranges without intermediate allocation:

```rust
impl ExprArena {
    pub fn start_params(&mut self) -> u32 {
        self.params.len() as u32
    }

    pub fn push_param(&mut self, param: Param) {
        self.params.push(param);
    }

    pub fn finish_params(&mut self, start: u32) -> ParamRange {
        let len = (self.params.len() as u32 - start) as u16;
        ParamRange { start, len }
    }
}
```

This API ensures that side-table entries are allocated contiguously for each parent node. No temporary `Vec` is created and discarded — the parser writes directly into the arena's storage.

### Capacity Heuristics

The parser pre-allocates arena capacity using an empirically-derived heuristic:

```rust
impl ExprArena {
    pub fn with_capacity(source_len: usize) -> Self {
        let estimated_exprs = source_len / 20;
        ExprArena {
            expr_kinds: Vec::with_capacity(estimated_exprs),
            expr_spans: Vec::with_capacity(estimated_exprs),
            expr_lists: Vec::with_capacity(estimated_exprs / 2),
            // ... similar heuristics for other side tables
        }
    }
}
```

The ratio of approximately one expression per 20 bytes of source text was measured across a corpus of Ori source files. It's intentionally conservative — overestimating capacity wastes some memory but avoids reallocation, which is more expensive because it copies the entire arena to a new allocation.

The side-table capacities are fractions of the expression count: roughly half as many list entries as expressions, fewer match arms than list entries, and so on. These ratios are tuned to typical Ori code and occasionally overshoot for unusual files (a file that is mostly match expressions will have more arms than average), but the cost of over-allocation is modest compared to the cost of reallocation.

### SharedArena — Zero-Copy Sharing

After parsing completes, the arena is wrapped in `SharedArena(Arc<ExprArena>)` for shared ownership:

```rust
pub struct SharedArena(Arc<ExprArena>);

impl Deref for SharedArena {
    type Target = ExprArena;
    fn deref(&self) -> &Self::Target { &self.0 }
}
```

`SharedArena::clone()` is an atomic increment — a single instruction on x86 — not a deep copy. The type checker, canonicalizer, evaluator, and LLVM backend all hold `SharedArena` handles to the same underlying data. Because `ExprArena` is immutable after parsing (no methods take `&mut self` on `SharedArena`), concurrent reads are safe without locking.

The `Deref` implementation means code that holds a `SharedArena` can call `arena.kind(id)` transparently — no explicit unwrapping needed.

## Parallel Arrays

The SoA pattern extends beyond the arena itself. Any phase-specific data that corresponds 1:1 with expressions can be stored in a **parallel array** — a separate `Vec` indexed by the same `ExprId`:

```rust
// Type checker stores resolved types parallel to the arena
struct TypeCheckResult {
    expr_types: Vec<Idx>,  // expr_types[expr_id] = resolved type
}

// Access: same ExprId indexes both the arena and the type array
let kind = arena.kind(id);
let ty = result.expr_types[id.index()];
```

This pattern has three advantages:

1. **Immutability** — The AST arena remains immutable after parsing. Type information is stored separately, so the parser's output is never modified.
2. **Independent caching** — Salsa can cache the arena and the type information independently. A change that doesn't affect types (like adding a comment) can reuse the cached type information even if the arena is rebuilt.
3. **Phase isolation** — Each phase creates its own parallel data without knowing about other phases' data. The type checker doesn't need to know about canonicalization; the canonicalizer doesn't need to know about ARC analysis.

## Design Tradeoffs

### Arena-Only Growth

The arena only grows. Expressions cannot be deleted or replaced in place. This is acceptable because compilation is a forward-flowing pipeline — the parser builds the AST once, and subsequent phases create new IRs (canonical IR, ARC IR) rather than modifying the raw AST. The append-only constraint actually simplifies correctness: an `ExprId` that was valid when it was created remains valid forever (within the arena's lifetime), so no dangling reference checks are needed.

### Arena Threading

Every function that touches the AST must accept the arena as a parameter. This is the most visible cost of the ID-based approach — function signatures are slightly more verbose. In practice, the arena is bundled into context structs (parser context, type check engine, evaluator state) that are already threaded through the compiler, so the incremental burden is a field in an existing parameter.

### Sentinel Complexity

`ExprId::INVALID` (`u32::MAX`) provides a "no expression" marker for cases like optional else-branches. This is cheaper than `Option<ExprId>` (4 bytes vs 8 bytes with alignment), but it requires discipline: code that accesses the arena with an `INVALID` id will panic at runtime. Debug-mode bounds checks catch these bugs early, but they represent a class of error that doesn't exist with `Option`.

## Prior Art

### Zig — MultiArrayList

[Zig](https://github.com/ziglang/zig)'s `MultiArrayList` is purpose-built for the SoA pattern. It allocates a single backing buffer with fields stored at computed offsets, guaranteeing perfect co-allocation. Zig's AST, ZIR, AIR, and type system all use this data structure. Ori's `ExprArena` follows the same design principle (separate arrays per field), but uses independent `Vec`s rather than a unified buffer, trading Zig's guaranteed co-allocation for Rust's standard `Vec` API.

### rustc — Typed Arenas with Lifetimes

[Rust's compiler](https://github.com/rust-lang/rust) uses the [`typed-arena`](https://crates.io/crates/typed-arena) crate for AST allocation, returning references with a `'tcx` lifetime that threads through the entire type checker. This gives direct reference access (no `arena[id]` indirection) but requires pervasive lifetime parameters. Ori chose ID-based arenas over typed arenas specifically because of Salsa: query results must be `Clone + Eq + Hash`, which references can't satisfy but `u32` indices can.

### V8 — Zone Allocation

Google's [V8 JavaScript engine](https://github.com/nicknisi/v8-source-notes) uses "zones" — arena allocators that manage memory for parsing and compilation phases. Each zone is a chain of memory blocks. When a compilation phase completes, its zone is freed in bulk. V8's zones are untyped bump allocators (objects of different types share the same zone), which gives maximum flexibility but no type-specific iteration performance. Ori's typed side tables provide the iteration performance that V8's zones sacrifice.

### LLVM — BumpPtrAllocator

[LLVM's `BumpPtrAllocator`](https://github.com/llvm/llvm-project/blob/main/llvm/include/llvm/Support/Allocator.h) is a production-proven bump allocator used throughout the LLVM infrastructure for IR nodes, pass data, and temporary allocations. It allocates from a chain of slabs, growing as needed. LLVM pairs this with `SpecificBumpPtrAllocator<T>` for type-specific allocation, similar in spirit to Ori's typed side tables.

### ECS — Entity Component System

The struct-of-arrays pattern has independent origins in game engine architecture. Entity Component Systems (popularized by libraries like [Flecs](https://github.com/SanderMertens/flecs) and [Bevy ECS](https://github.com/bevyengine/bevy)) store component data in per-type arrays indexed by entity ID, for exactly the same cache-efficiency reasons that motivate SoA in compilers. The insight is domain-independent: when processing touches only a subset of an object's fields, separating fields into independent arrays improves cache utilization.

## Related Documents

- [Flat AST](flat-ast.md) — Why arena + ID over `Box`, with memory layout comparison
- [String Interning](string-interning.md) — The complementary interning strategy for identifiers
- [Type Representation](type-representation.md) — How the pool extends arena-style allocation to types
- [IR Overview](index.md) — How arena allocation fits into the three-tier IR pipeline
