---
title: "Salsa Integration"
description: "Ori Compiler Design — Salsa Integration"
order: 103
section: "Architecture"
---

# Salsa Integration

## What Is Incremental Computation?

Most programs compute results from inputs. When an input changes, the naive approach is to recompute everything from scratch. **Incremental computation** is the art of recomputing only what's affected by the change, reusing cached results for everything else.

The problem is fundamental to software engineering. Build systems face it (only recompile changed files). Spreadsheets face it (only recalculate cells that depend on the edited cell). Databases face it (only update materialized views affected by new data). And compilers face it — acutely, because compilation is expensive and developers edit files constantly.

The challenge is not caching (that's easy) but **invalidation**: knowing exactly which cached results are stale after a change. Cache too aggressively and you serve stale results (correctness bugs). Cache too conservatively and you recompute too much (performance lost). The ideal is a framework that tracks dependencies automatically and invalidates precisely — and this is exactly what Salsa provides.

### Why Compilers Need Incrementality

A batch compiler that re-lexes, re-parses, re-type-checks, and re-evaluates the entire program on every keystroke is unusable in an IDE. Even for CLI compilation, re-processing unchanged imports on every build wastes time.

But compiler computations are heavily interdependent. Changing one function's signature can affect type checking of every function that calls it. Adding an import can change name resolution throughout the module. These dependencies form a graph, and tracking that graph manually is error-prone and labor-intensive.

The Salsa framework solves this by treating the compiler as a set of pure, memoized functions ("queries") with automatic dependency tracking. The compiler author writes each phase as a normal function; Salsa handles caching, dependency tracking, and minimal recomputation.

## What Makes Ori's Salsa Integration Distinctive

### Hybrid Architecture: Queries + Side-Caches

Not all compiler data can live inside Salsa's dependency graph. Salsa requires query results to implement `Clone + Eq + Hash` — reasonable for most data, but impossible for types like `Pool` (which contains type-level state with internal indices) and impractical for large shared structures like `CanonResult` (where equality comparison would be expensive).

Ori solves this with a **hybrid architecture**: major computations are Salsa queries (benefiting from automatic dependency tracking and early cutoff), while three categories of data live in session-scoped side-caches outside Salsa's graph. A `CacheGuard` safety token prevents the correctness bugs that typically plague manual cache invalidation.

### Early Cutoff at Every Level

Ori's query chain is designed to maximize early cutoff opportunities. The lexer query chain (`tokens_with_metadata → lex_result → tokens`) has three levels, so metadata-only changes (formatting, comments) cutoff before reaching the parser. Whitespace-only changes cutoff before reaching type checking. The more granular the queries, the more opportunities for cutoff.

## How Salsa Works

Salsa is a Rust framework that provides four capabilities:

1. **Memoization** — query results are cached and returned without recomputation when inputs haven't changed
2. **Dependency tracking** — Salsa records which queries each query reads, building a dependency graph automatically
3. **Incremental recomputation** — when an input changes, only queries transitively affected by the change re-run
4. **Early cutoff** — if a query re-runs but produces the same result as before, its dependents don't re-run

These four capabilities combine to make the compiler feel like a batch system to its authors (each phase is a normal function) while behaving like an incremental system to its users (only changed computations re-run).

## Database Setup

The Salsa database is the central coordination point. All queries operate on a shared database that owns the memoized results:

```rust
#[salsa::db]
pub trait Db: salsa::Database {
    /// Get the string interner for interning identifiers and strings.
    fn interner(&self) -> &StringInterner;

    /// Load a source file by path, creating a SourceFile input if needed.
    /// Returns None if the file cannot be read.
    fn load_file(&self, path: &Path) -> Option<SourceFile>;
}

#[salsa::db]
#[derive(Clone)]
pub struct CompilerDb {
    /// Salsa's internal storage for all queries.
    storage: salsa::Storage<Self>,

    /// String interner for identifiers and string literals.
    /// Shared via Arc so Clone works and strings persist.
    interner: SharedInterner,

    /// Cache of loaded source files by path.
    /// Uses parking_lot::RwLock for efficient concurrent access.
    /// This is an index for deduplication only — SourceFile values
    /// are Salsa inputs and are properly tracked.
    file_cache: Arc<RwLock<HashMap<PathBuf, SourceFile>>>,

    /// Event logs for testing/debugging (optional).
    logs: Arc<Mutex<Option<Vec<String>>>>,
}
```

Key design points:

- **`CompilerDb` must implement `Clone`** — Salsa's internal snapshot mechanism requires it. All fields use `Arc` to make cloning cheap.
- **`file_cache` prevents duplicate inputs** — without it, loading the same import twice would create two separate `SourceFile` inputs, breaking dependency tracking.
- **`load_file()` creates Salsa inputs** — this is the proper entry point for imported files, ensuring Salsa tracks changes to them.
- **`SharedInterner` is `Arc`-wrapped** — string interning must survive database clones and be shared across all queries.

The event handler provides debugging visibility into which queries run:

```rust
#[salsa::db]
impl salsa::Database for CompilerDb {
    fn salsa_event(&self, event: &dyn Fn() -> salsa::Event) {
        if let Some(logs) = &mut *self.logs.lock() {
            let event = event();
            if let salsa::EventKind::WillExecute { .. } = event.kind {
                logs.push(format!("{event:?}"));
            }
        }
    }
}
```

## Inputs and Tracked Functions

Salsa distinguishes between two kinds of data:

### Inputs — External Ground Truth

Inputs are the raw data that comes from outside the compiler. In Ori, the primary input is source text:

```rust
#[salsa::input]
pub struct SourceFile {
    #[return_ref]
    pub text: String,

    #[return_ref]
    pub path: PathBuf,
}
```

Creating an input:
```rust
let file = SourceFile::new(&db, source_text, path);
```

Updating an input (triggers recomputation of all dependent queries):
```rust
file.set_text(&mut db).to(new_text);
```

The `#[return_ref]` annotation means accessors return `&String` and `&PathBuf` rather than cloning, avoiding unnecessary allocations for large source files.

### Tracked Functions — Derived Data

Tracked functions compute data from inputs and other tracked functions:

```rust
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList {
    let text = file.text(db);
    lexer::tokenize(db, text)
}

#[salsa::tracked]
pub fn parsed(db: &dyn Db, file: SourceFile) -> ParseOutput {
    let tokens = tokens(db, file);  // Dependency on tokens() recorded automatically
    parser::parse(db, tokens)
}
```

When `parsed()` calls `tokens()`, Salsa records that `parsed` depends on `tokens`. If the source text changes, Salsa knows to re-run `tokens()` first, then only re-run `parsed()` if the token list actually changed.

## Salsa Compatibility Requirements

All types in Salsa query signatures must implement:

```rust
#[derive(Clone, Eq, PartialEq, Hash, Debug)]
```

This is because Salsa needs to:
- **Clone** values for caching (snapshot isolation)
- **Compare** old and new outputs for early cutoff (`Eq`)
- **Hash** values for storage

Types that satisfy these requirements include primitives, standard collections with compatible elements, and custom types with derived traits. Types that **cannot** satisfy them include function pointers (not `Eq`/`Hash`), trait objects (not `Clone`), and `Arc<Mutex<T>>` (can change without Salsa knowing — breaks dependency tracking).

For complex types that can't derive these traits, the solution is **interning** — assigning each unique value a small integer ID, then passing the ID through Salsa instead of the value:

```rust
// Instead of passing Type (complex enum) through queries,
// intern it to get a comparable ID
#[salsa::interned]
struct InternedType {
    #[return_ref]
    ty: Type,
}
// Now InternedType implements Clone + Eq + Hash automatically (by ID)
```

## Early Cutoff

Early cutoff is Salsa's most powerful optimization. When a query re-runs but produces the **same output** as its cached result, all downstream queries skip re-running entirely:

```rust
// 1. Developer adds a comment to source
file.set_text(&mut db).to("// new comment\nlet x = 42");

// 2. tokens() re-runs (input changed)
//    Produces identical TokenList (comments aren't tokens)

// 3. parsed() checks: tokens output unchanged → return cached AST
// 4. typed() checks: parsed output unchanged → return cached types
// 5. evaluated() checks: typed output unchanged → return cached value
```

Steps 3–5 don't execute any parsing, type-checking, or evaluation code — they compare the dependency's output to the cached value and return immediately. This is why `Eq` is required on all query types: Salsa must compare old and new outputs to detect cutoff opportunities.

The practical effect is dramatic for IDE-style workflows. Most keystrokes — adding characters to a string literal, editing a comment, adjusting whitespace — produce token-level changes that don't affect parsing, type-checking, or evaluation. Early cutoff turns these from full recompilations into cheap comparisons.

## Session-Scoped Side-Caches

Three categories of compiler data can't satisfy Salsa's requirements. These live in session-scoped caches **outside** Salsa's dependency graph:

| Cache | Type | Purpose |
|-------|------|---------|
| `PoolCache` | `Arc<RwLock<HashMap<PathBuf, Arc<Pool>>>>` | Type pools from `typed()` |
| `CanonCache` | `Arc<RwLock<HashMap<PathBuf, SharedCanonResult>>>` | Canonical IR from `canonicalize_cached()` |
| `ImportsCache` | `Arc<RwLock<HashMap<PathBuf, Arc<ResolvedImports>>>>` | Resolved import maps |

These caches are keyed by `PathBuf` and live on `CompilerDb`. Each uses `Arc<RwLock<...>>` for concurrent access and cheap cloning when `CompilerDb` is cloned.

**Why these can't be Salsa queries:**

- **`Pool`** contains interned type representations with internal indices that don't support meaningful `Eq` — two pools with identical types but different interning order would compare as unequal, defeating early cutoff.
- **`CanonResult`** contains arenas, decision tree pools, and constant pools. Deep structural equality would be expensive and provide little cutoff benefit (if types changed, the canonical IR almost certainly changed too).
- **`ResolvedImports`** contains cross-file resolution data that evolves during multi-file compilation.

### Invalidation Protocol

Because Salsa doesn't track these caches, stale entries can silently cause correctness bugs — the most dangerous kind. The `typed()` query calls `invalidate_file_caches()` before re-type-checking to clear entries for the affected file:

```rust
fn invalidate_file_caches(db: &dyn Db, path: &Path) -> CacheGuard {
    db.pool_cache().invalidate(path);
    db.canon_cache().invalidate(path);
    db.imports_cache().invalidate(path);
    CacheGuard(())
}
```

This is a manual process — and manual invalidation is a known source of bugs in every system that uses it. Ori adds a mechanical safeguard: the `CacheGuard`.

### CacheGuard Safety Token

`CacheGuard` is a zero-sized type that proves invalidation was performed. It is a required parameter of `type_check_with_imports_and_pool()`, so callers physically cannot skip invalidation:

```rust
pub(crate) struct CacheGuard(());
```

It can only be created by:
- `invalidate_file_caches()` — for Salsa-tracked files (the normal path)
- `CacheGuard::untracked()` — for synthetic or imported modules not tracked by Salsa (no cache entries exist to invalidate)

This pattern converts a "remember to invalidate" policy into a "compilation won't compile without invalidating" guarantee. Any future code path that triggers re-type-checking **must** obtain a `CacheGuard`, which forces it through the invalidation function. Forgetting to invalidate becomes a compile error, not a silent bug.

## Design Tradeoffs

### What Salsa Costs

Salsa isn't free. It imposes several constraints on the compiler's design:

- **Trait requirements** — every query output must be `Clone + Eq + Hash + Debug`, which precludes types with interior mutability, function pointers, or trait objects. This sometimes forces architectural compromises (the side-cache system exists because of this).
- **Determinism** — queries must be pure functions of their inputs. No reading environment variables, no using wall-clock time, no randomness. Any nondeterminism breaks memoization correctness.
- **Cloning overhead** — Salsa clones query results for snapshot isolation. For large results (like `ParseOutput` with its arena), this adds memory pressure. `Arc` wrapping mitigates this for shared data.
- **Debugging complexity** — when queries don't re-run as expected, diagnosing why requires understanding Salsa's dependency graph and cutoff semantics. The `salsa_event` hook helps but is coarser than normal debugging.

### What Salsa Gives Back

The costs are worth paying for several reasons:

- **Free incrementality** — the compiler author writes normal functions; Salsa adds memoization and dependency tracking automatically.
- **IDE readiness** — the same queries that power `ori check` can power a language server, with incremental updates on every keystroke.
- **Correctness guarantees** — Salsa's dependency tracking is automatic, eliminating the class of bugs where a manual invalidation is forgotten.
- **Parallelism** — independent queries on different files can run in parallel without explicit synchronization.

### Alternatives Not Taken

- **Manual memoization** — simpler to implement but error-prone. Every cache needs manual invalidation, and forgetting one produces silent correctness bugs. Salsa's automatic dependency tracking eliminates this class of errors.
- **File-level granularity** — some compilers (Go, Zig) use file-level caching: if a file hasn't changed, skip it entirely. Simpler but coarser — a whitespace change forces a full recompile of the file, whereas Salsa's early cutoff skips downstream phases even within the same file.
- **Full query-based architecture** — some compilers (Roslyn) make every computation a query, with no side-caches. Cleaner but requires all data to satisfy query constraints, which can force expensive workarounds for types that don't naturally support equality.

## Prior Art

### rust-analyzer — Salsa's Origin

Salsa was created specifically for [rust-analyzer](https://rust-analyzer.github.io/), the Rust language server. rust-analyzer demonstrates the full potential of query-based compilation: diagnostics, completions, go-to-definition, and refactoring all share the same cached query graph. When a file changes, only affected queries re-run, giving sub-100ms response times for most IDE operations. Ori uses the same framework with a simpler query graph (fewer files, simpler module system).

### Roslyn — .NET's Query Architecture

Microsoft's [Roslyn](https://github.com/dotnet/roslyn) compiler for C# and VB.NET uses a "compilation" object that caches syntax trees, semantic models, and emit results. Like Salsa, it provides incremental recomputation when source changes. Unlike Salsa, Roslyn uses immutable snapshots and explicit version tracking rather than automatic dependency tracking — more manual work for the compiler author, but more control over caching granularity.

### Make / Build Systems — The Original Incremental Computation

The `make` utility (Feldman, 1979) is arguably the first practical incremental computation framework: it tracks dependencies between files and only rebuilds targets whose dependencies have changed. Salsa can be understood as "make for in-process computations" — instead of file timestamps, it uses content equality; instead of shell commands, it uses Rust functions; instead of files, it caches query results in memory.
