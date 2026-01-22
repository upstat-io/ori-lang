# Compiler Speed Optimization Research

## Executive Summary
This document compiles optimization techniques from high-performance compilers (Zig, Go, Rust, V8, SWC, Turbopack) that could apply to Sigil's compiler for faster build speeds.

## 1. Arena Allocation (Zig)
From Zig's arena_allocator.zig (/tmp/compiler-research/zig/lib/std/heap/arena_allocator.zig):
- Single bulk free: Allocate many times, free once via `deinit`
- Linked list of exponentially growing buffers
- `retain_capacity` mode for loops: After warmup, no more allocations needed
- Free individual items only if most recent (else no-op)
- 50% growth factor for new buffers

Key insight: ArenaAllocator wraps a child allocator with bulk deallocation. Perfect for AST nodes that all die together.

## 2. String Interning (Rust-analyzer, Salsa)
From rust-analyzer's intern.rs:
- Global `DashMap` with sharding (concurrent hashmap)
- `triomphe::Arc` for pointer-based equality/hashing
- O(1) equality via pointer comparison instead of string comparison
- GC vs non-GC modes for different lifetimes
- Shards = number of CPU cores to reduce lock contention

Key techniques:
- Hash value once, store 32-bit ID
- Compare IDs instead of strings (single integer compare)
- Sharded maps to reduce contention

## 3. Incremental Computation (Salsa/rustc)
From Salsa's interned.rs and memo.rs:
- Query-based architecture: Everything is a memoized function call
- Red-green algorithm: Track dependencies, recompute only what changed
- Fingerprints: 128-bit hashes of results for change detection
- Early cutoff: If inputs changed but output is same, stop propagation
- Durability levels: LOW/MEDIUM/HIGH to optimize change detection
- LRU with revisions: Track when values were last used

Key data structures:
- Sharded hashmaps with LRU linked lists
- Intrusive linked lists for memory efficiency
- AtomicRevision for lock-free revision tracking

## 4. Go Compiler Design Decisions
- **No cyclic dependencies**: DAG structure enables parallel compilation
- **No symbol table for parsing**: Grammar designed to parse without lookahead
- **Unused imports = error**: Prevents bloat
- **Simple grammar**: Only 25 keywords
- **Import magic**: Each import contains info about entire dependency tree
- **Single-file compilation units**: Each package compiles independently

Key insight: Go prioritized fast compilation in language design itself.

## 5. V8 Lazy Parsing
- **Lazy parsing**: Don't fully parse functions until called
- **PreParser**: Lightweight pass to find function boundaries
- **IIFE detection**: Immediately-invoked functions get eager parsing
- **Explicit compile hints**: Magic comments for eager compilation
- **Background compilation**: Parse on background threads

Key insight: Parse only what you need, when you need it.

## 6. SWC/Turbopack Parallelism
From SWC's sync.rs:
- Feature-flagged parallelism: `concurrent` feature swaps Rc↔Arc, RefCell↔Mutex
- parking_lot locks (faster than std)
- Zero-cost single-threaded mode

From Turbopack:
- Fine-grained dependency graph
- Function-level caching (Turbo Engine)
- Unified graph for all environments
- True parallelism via Rust (no GIL)

## 7. Flattened AST (Data-Oriented Design)
From compiler research:
- Store AST nodes in contiguous arrays
- Use 32-bit indices instead of 64-bit pointers
- Cache-friendly iteration
- Cheap allocation (bump allocator on array)
- ECS-like architecture

Benefits:
- 3x memory reduction vs pointer-based trees
- Better cache locality
- Trivial serialization

## 8. Hash Function Selection
- FxHash (rustc_hash): Fast for integers and small data
- Use `BuildHasher` trait for configurable hashing
- Avoid SipHash for internal compiler data (security not needed)

## Recommended Optimizations for Sigil

### High Impact, Lower Effort
1. **Arena allocator for AST** - All nodes allocated in arena, freed together
2. **String interning with sharding** - Already have intern/pool.rs skeleton
3. **FxHash everywhere** - Replace std HashMap with rustc_hash

### Medium Impact, Medium Effort
4. **Lazy parsing of function bodies** - Don't parse until needed
5. **Parallel file processing** - Each file can parse/check independently
6. **Flattened AST with indices** - Replace Box/pointer with u32 indices

### High Impact, Higher Effort
7. **Salsa-style query system** - Full incremental compilation
8. **Codegen unit parallelism** - Generate code for multiple functions in parallel

## Implementation Priority
1. Arena allocation (immediate win)
2. String interning improvements (already started)
3. Parallel file parsing
4. Lazy function body parsing
5. Query-based incremental compilation (long-term)

---

## PART 2: Deep Dive Research

### 9. Salsa Query System - Deep Architecture Analysis

From examining `/tmp/compiler-research/salsa/src/`:

#### Core Concepts

**Revisions** (`revision.rs`):
```rust
// Every input change bumps the global revision
pub struct Revision {
    generation: NonZeroUsize,  // Starts at 1, never 0
}

// Atomic version for concurrent access
pub struct AtomicRevision {
    data: AtomicUsize,
}
```

**Durability Levels** (`durability.rs`):
```rust
pub const LOW: Durability    // User-edited code (changes often)
pub const MEDIUM: Durability // Config files (changes sometimes)
pub const HIGH: Durability   // Stdlib/dependencies (rarely changes)
```
Key insight: If only LOW durability inputs changed, skip validating MEDIUM/HIGH queries entirely.

#### The Fetch Hot Path (`function/fetch.rs`)

```rust
pub fn fetch<'db>(&'db self, db, zalsa, zalsa_local, id) -> &'db Output {
    // 1. Check for cancellation
    zalsa.unwind_if_revision_cancelled(zalsa_local);

    // 2. Try hot path first (already validated this revision)
    let memo = self.refresh_memo(db, zalsa, zalsa_local, id);

    // 3. Record the read for dependency tracking
    zalsa_local.report_tracked_read(
        database_key_index,
        memo.revisions.durability,
        memo.revisions.changed_at,
        memo.cycle_heads(),
    );

    memo_value
}

fn fetch_hot(&self, zalsa, id, memo_idx) -> Option<&Memo> {
    let memo = self.get_memo_from_table_for(zalsa, id, memo_idx)?;
    memo.value.as_ref()?;

    // Shallow verify: just check verified_at == current_revision
    let can_shallow_update = self.shallow_verify_memo(zalsa, key, memo);

    if can_shallow_update.yes() && !memo.may_be_provisional() {
        self.update_shallow(zalsa, key, memo, can_shallow_update);
        Some(memo)
    } else {
        None  // Fall through to cold path
    }
}
```

#### The Verification Algorithm (`function/maybe_changed_after.rs`)

```rust
pub enum VerifyResult {
    Changed,                    // Must recompute
    Unchanged { accumulated },  // Can reuse cached value
}

fn maybe_changed_after(&self, db, id, revision, cycle_heads) -> VerifyResult {
    loop {
        // 1. Hot path: Already verified this revision?
        let memo = self.get_memo_from_table_for(...)?;
        let can_shallow_update = self.shallow_verify_memo(...);

        if can_shallow_update.yes() && !memo.may_be_provisional() {
            self.update_shallow(...);
            return if memo.revisions.changed_at > revision {
                VerifyResult::changed()
            } else {
                VerifyResult::unchanged()
            };
        }

        // 2. Cold path: Deep verification needed
        // ... claim the query, verify dependencies recursively
    }
}
```

#### Thread-Local State (`zalsa_local.rs`)

```rust
pub struct ZalsaLocal {
    // Stack of currently executing queries (for cycle detection)
    query_stack: RefCell<QueryStack>,

    // Cache of most recent page per ingredient (allocation optimization)
    most_recent_pages: UnsafeCell<FxHashMap<IngredientIndex, PageIndex>>,
}
```

Key insight: Thread-local page caching avoids contention on the global allocator.

#### How to Implement for Sigil

1. **Define Inputs** (source files):
```rust
#[salsa::input]
pub struct SourceFile {
    #[returns(ref)]
    pub text: String,
    pub path: PathBuf,
}
```

2. **Define Interned Types** (symbols, types):
```rust
#[salsa::interned]
pub struct TypeId<'db> {
    pub kind: TypeKind,
}
```

3. **Define Tracked Functions** (compiler passes):
```rust
#[salsa::tracked]
pub fn parse(db: &dyn Db, file: SourceFile) -> Ast { ... }

#[salsa::tracked]
pub fn type_check(db: &dyn Db, file: SourceFile) -> TypedAst { ... }

#[salsa::tracked]
pub fn codegen(db: &dyn Db, file: SourceFile) -> CCode { ... }
```

4. **Query System Benefits**:
   - Edit one file → only re-typecheck that file + dependents
   - stdlib marked HIGH durability → never re-validated
   - Cycle detection built-in for mutual recursion

---

### 10. SIMD Lexing (simdjson patterns)

From [simdjson](https://github.com/simdjson/simdjson):

**Two-Stage Architecture**:
1. **Stage 1**: SIMD scan for structural characters (`{`, `}`, `[`, `]`, `:`, `,`, `"`)
   - Process 64 bytes at once with AVX2
   - Build bit-index of all structural positions
   - Validate UTF-8 in parallel

2. **Stage 2**: Process structural indices sequentially
   - No character-by-character scanning
   - Jump directly to next structural char

**Branchless Techniques**:
```c
// Instead of: if (c == '"') ...
// Use lookup table:
uint8_t classify[256] = {...};
uint64_t structural_mask = _mm256_movemask_epi8(
    _mm256_cmpeq_epi8(chunk, quote_char)
);
```

**Applicability to Sigil**:
- Stage 1 SIMD scan for `@`, `$`, `{`, `}`, `(`, `)`, `[`, `]`, `->`, `=`
- Build token-start indices array
- Stage 2 processes tokens without rescanning

---

### 11. TCC Single-Pass Compilation

From [TCC](https://bellard.org/tcc/):

**Key Tricks**:
1. **No intermediate AST**: Parse directly to machine code
2. **Value Stack**: Instead of AST nodes, push values onto a stack
   ```c
   // SValue tracks where each value lives
   struct SValue {
       CType type;
       int r;        // Register or storage class
       int r2;       // Secondary register
       CValue c;     // Constant value if known
   };
   ```

3. **Three-Register Allocation**: Only 3 temp registers, spill to stack when needed

4. **Integrated Assembler/Linker**: No external tools, no temp files

**Speed**: 9x faster than GCC -O0

**Applicability**: Less relevant for Sigil (we want optimization), but shows that extreme simplicity wins.

---

### 12. LuaJIT Trace Compiler

From [LuaJIT](https://luajit.org/) and [SSA IR docs](http://wiki.luajit.org/SSA-IR-2.0):

**Linear IR Design**:
```
// No pointers, just indices into a linear array
struct IRIns {
    uint8_t op;      // Opcode
    uint8_t t;       // Type
    IRRef1 op1;      // 16-bit index to operand 1
    IRRef2 op2;      // 16-bit index to operand 2
};
```

**Key Insights**:
- IR instructions are 8 bytes each (cache-line friendly)
- No allocation during compilation (everything pre-sized)
- Biased references: negative = constants, positive = instructions

**Snapshot System**:
- Snapshots capture bytecode state at trace exits
- Sparse + compressed representation
- Enables deoptimization without runtime overhead

---

### 13. Cranelift E-Graph Optimization

From [Cranelift](https://cranelift.dev/):

**E-Graph Architecture**:
- Multiple equivalent representations stored together
- Rewrite rules applied to entire equivalence class
- Extract best version at the end

**Register Allocation (regalloc2)**:
- 20% faster compilation than old allocator
- Live-range splitting (value in different places at different times)
- 10-20% better generated code on register-pressure benchmarks

---

### 14. Memory Layout Optimization

**Struct Packing Rules**:
```rust
// Bad: 24 bytes with padding
struct Node {
    tag: u8,        // 1 byte + 7 padding
    children: u64,  // 8 bytes
    span: u32,      // 4 bytes + 4 padding
}

// Good: 16 bytes, no padding
struct Node {
    children: u64,  // 8 bytes (largest first)
    span: u32,      // 4 bytes
    tag: u8,        // 1 byte + 3 padding at end
}
```

**Cache Line Considerations**:
- Cache lines are 64 bytes
- Keep related data together
- Use `#[repr(align(64))]` for thread-local data to avoid false sharing

**Tools**:
- `pahole` - shows struct padding
- `clang -Wpadded` - warns about padding

---

### 15. Logos Lexer Performance

From [logos](https://github.com/maciejhirsz/logos):

**How It Achieves Speed**:
1. Compiles all token patterns into single DFA
2. Jump tables for state transitions (no branches)
3. Batched reads to minimize bounds checks
4. All heavy lifting at compile time

**Benchmark**: ~1200 MB/s throughput

**Beating Logos** (from [blog post](https://alic.dev/blog/fast-lexing)):
- Hand-rolled lexer can be 20-30% faster
- Better speculative execution when aggressively inlined

---

### 16. Parallel Parsing Research

From academic research:

**PAPAGENO Parser Generator**:
- Operator precedence grammars enable data-parallel parsing
- Split input into chunks at operator boundaries
- Parse chunks independently, merge results
- Near-linear speedup on multicore

**Requirements**:
- Grammar must be "locally parsable" (bounded lookahead)
- Sigil's clean syntax (no semicolons, clear boundaries) is well-suited

---

## Updated Implementation Roadmap for Sigil

### Phase 1: Quick Wins (1-2 weeks each)
1. **Add `bumpalo` arena for AST** - Immediate 2-5x parse speedup
2. **Switch to `rustc_hash::FxHashMap`** - 10-30% type checking speedup
3. **Use `logos` for lexer** - Already fast, validate or replace current

### Phase 2: Parallelism (2-4 weeks each)
4. **Parallel file parsing with `rayon`** - Linear speedup with cores
5. **Parallel type checking per module** - More complex, needs careful design

### Phase 3: Incremental (1-2 months)
6. **Integrate Salsa for query system**:
   - Define `SourceFile` as input
   - Define `parse`, `type_check`, `codegen` as tracked functions
   - Mark stdlib as HIGH durability
   - Get incremental rebuilds for free

### Phase 4: Advanced (long-term)
7. **Flattened AST with u32 indices**
8. **SIMD lexer for Stage 1 structural scan**
9. **Cranelift backend for JIT mode**

---

## Sources
- Zig: https://github.com/ziglang/zig/blob/master/lib/std/heap/arena_allocator.zig
- Salsa: https://github.com/salsa-rs/salsa
- rust-analyzer: https://github.com/rust-analyzer/rust-analyzer/tree/master/crates/intern
- SWC: https://github.com/swc-project/swc
- Go compiler: https://github.com/golang/go/tree/master/src/cmd/compile
- V8: https://v8.dev/blog/preparser
- Turbopack: https://vercel.com/blog/turbopack
- Flattening ASTs: https://www.cs.cornell.edu/~asampson/blog/flattening.html
- simdjson: https://github.com/simdjson/simdjson
- TCC: https://bellard.org/tcc/
- LuaJIT IR: http://wiki.luajit.org/SSA-IR-2.0
- Cranelift: https://bytecodealliance.org/articles/cranelift-progress-2022
- Logos: https://github.com/maciejhirsz/logos
- Structure Packing: http://www.catb.org/esr/structure-packing/
- PAPAGENO: https://www.sciencedirect.com/science/article/pii/S0167642315002610
