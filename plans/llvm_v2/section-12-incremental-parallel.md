---
section: "12"
title: Incremental & Parallel Codegen
status: not-started
goal: Function-level incremental compilation with two-layer caching (ARC IR + object code), Salsa hybrid integration, and dependency-respecting parallel compilation
sections:
  - id: "12.1"
    title: Existing Infrastructure (Preserve)
    status: not-started
  - id: "12.2"
    title: Function-Level Incremental Compilation
    status: not-started
  - id: "12.3"
    title: Two-Layer Cache
    status: not-started
  - id: "12.4"
    title: Salsa Integration (Hybrid)
    status: not-started
  - id: "12.5"
    title: Parallel Compilation
    status: not-started
  - id: "12.6"
    title: Multi-File Integration
    status: not-started
---

# Section 12: Incremental & Parallel Codegen

**Status:** Not Started
**Goal:** Don't recompile the world when one function changes. Compile independent codegen units in parallel. Function-level granularity with two-layer caching (ARC IR + object code), hybrid Salsa/ArtifactCache invalidation, and dependency-respecting multi-threaded execution.

**Reference compilers:**
- **Zig** `src/codegen/llvm.zig` -- `updateFunc` for per-function incremental updates; nav_map for function-to-object mapping. Zig compiles each function independently and patches object files in place. Ori's initial approach compiles each function to its own small object file rather than in-place patching.
- **Rust** `compiler/rustc_codegen_ssa/src/base.rs` -- CGU partitioning + `CguReuse` (PreLto, PostLto, No). Work product fingerprinting for reuse detection.
- **Lean 4** `src/Lean/Compiler/IR/RC.lean` -- ARC IR as a serializable intermediate form, enabling caching of borrow-analyzed IR.

**Current state:** `ori_llvm/src/aot/incremental/` contains ~2,400 lines of well-tested production code across four modules: `hash.rs` (SourceHasher, ContentHash), `cache.rs` (ArtifactCache, CacheKey, CacheConfig), `deps.rs` (DependencyGraph, DependencyTracker), `parallel.rs` (CompilationPlan, ParallelCompiler, compile_parallel). Multi-file compilation in `multi_file.rs` builds dependency graphs, topologically sorts, compiles each module to a `.o`, and links. V2 preserves this infrastructure and layers function-level granularity on top.

---

## 12.1 Existing Infrastructure (Preserve)

The `aot/incremental/` module provides file-level incremental compilation. V2 preserves all of the following and layers function-level tracking on top.

**`hash.rs` — Source Hashing (521 lines):** `SourceHasher` with FxHash-based content hashing, metadata-based quick checks (size + mtime), normalized hashing mode (whitespace-insensitive), and `ContentHash` newtype with hex serialization. `combine_hashes()` and `hash_string()` utility functions.

**`cache.rs` — Artifact Cache (573 lines):** `ArtifactCache` stores compiled `.o` files by content hash. `CacheKey` combines source hash + dependency hash + flags hash (compiler version, opt level, target triple). `CacheConfig` builder with version-based invalidation. Cache directory structure: `objects/` for `.o` files, `meta/` for metadata, `version` for compiler version check.

**`deps.rs` — Dependency Graph (560 lines):** `DependencyGraph` tracks file-level import relationships. `DependencyTracker` wraps the graph with a cache directory. Features: transitive dependency/dependent computation, topological ordering (deterministic via path sorting), cycle detection, `files_to_recompile()` for change propagation.

**`parallel.rs` — Parallel Compilation (668 lines):** `CompilationPlan` with dependency-respecting scheduling (ready queue + pending set + reverse dependency index for O(1) completion notification). `ParallelCompiler` coordinates execution with progress tracking. `compile_parallel()` free function for multi-threaded compilation via `std::thread`.

**`multi_file.rs` — Multi-File Pipeline (714 lines):** `build_dependency_graph()` recursively resolves imports, `derive_module_name()` generates mangled module names, `resolve_relative_import()` handles file and directory module resolution. Produces `DependencyBuildResult` with topological compilation order.

**What V2 preserves:** DependencyGraph, ArtifactCache structure, CompilationPlan scheduling, SourceHasher infrastructure. These become the file-level scaffolding. Function-level tracking is layered on top without replacing the file-level infrastructure, which remains the fallback granularity.

- [ ] Verify all existing incremental infrastructure works after V2 module restructuring
- [ ] Ensure file-level incremental remains functional as fallback

---

## 12.2 Function-Level Incremental Compilation

This is the core new capability of V2. Individual functions are the unit of incremental recompilation. The design follows Zig's approach: per-function hashing, per-function dependency tracking, per-function caching. Each function can be compiled to its own LLVM module, enabling maximum parallelism.

### Function Content Hashing

A function's content hash captures everything that affects its compiled output. The hash includes four components:

```rust
/// Content hash for a single function, capturing all inputs that affect codegen.
///
/// This is the primary cache key for function-level incremental compilation.
/// If this hash is unchanged, the function's compiled output is identical.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FunctionContentHash {
    /// Hash of the function body AST (span-stripped).
    body_hash: ContentHash,
    /// Hash of the function's type signature (params + return type).
    signature_hash: ContentHash,
    /// Hash of the type signatures of all called functions.
    callees_hash: ContentHash,
    /// Hash of all referenced globals' types.
    globals_hash: ContentHash,
    /// Combined hash for cache lookup.
    combined: ContentHash,
}

impl FunctionContentHash {
    /// Compute the content hash for a function.
    ///
    /// Inputs:
    /// - `body`: The function's AST body (stripped of spans for position-independence)
    /// - `signature`: The function's resolved type signature
    /// - `callees`: Type signatures of all functions this function calls
    /// - `globals`: Types of all globals this function references
    pub fn compute(
        body: &[u8],
        signature: &[u8],
        callees: &[ContentHash],
        globals: &[ContentHash],
    ) -> Self {
        let body_hash = hash_bytes(body);
        let signature_hash = hash_bytes(signature);
        let callees_hash = combine_hashes(callees);
        let globals_hash = combine_hashes(globals);
        let combined = combine_hashes(&[
            body_hash, signature_hash, callees_hash, globals_hash,
        ]);
        Self { body_hash, signature_hash, callees_hash, globals_hash, combined }
    }
}
```

**AST hashing strategy:** Hash the function's AST nodes, stripping span information so that whitespace-only changes (which shift spans but don't change semantics) produce the same hash. This is computed from the typed AST after type inference, ensuring that inferred types are part of the hash input via the signature.

### Function Dependency Tracking

Function-level dependencies are more granular than file-level imports. When function A calls function B:

- If B's **body** changes but its **signature** is unchanged: A is NOT recompiled. Only B is recompiled. This is the key optimization that makes function-level tracking superior to file-level.
- If B's **signature** changes: A MUST be recompiled because the calling convention, parameter types, or return type may have changed.

```rust
/// Dependency information for a single function.
#[derive(Debug, Clone)]
pub struct FunctionDeps {
    /// Mangled name of this function (globally unique identifier).
    pub name: String,
    /// Functions this function calls (by mangled name).
    pub callees: Vec<String>,
    /// Globals this function references (by mangled name).
    pub referenced_globals: Vec<String>,
    /// The function's type signature hash (for callers to depend on).
    pub signature_hash: ContentHash,
    /// Full content hash (body + signature + callees + globals).
    pub content_hash: FunctionContentHash,
}

/// Function-level dependency graph, layered on top of file-level DependencyGraph.
#[derive(Debug, Default)]
pub struct FunctionDependencyGraph {
    /// Map from mangled function name to its dependency info.
    functions: FxHashMap<String, FunctionDeps>,
    /// Reverse map: function name -> functions that call it (for invalidation).
    callers: FxHashMap<String, FxHashSet<String>>,
}

impl FunctionDependencyGraph {
    /// Determine which functions need recompilation given a set of changed functions.
    ///
    /// A function needs recompilation if:
    /// 1. Its own body changed (content hash differs), OR
    /// 2. A function it calls changed its signature (signature hash differs)
    ///
    /// Internal body changes to callees do NOT trigger recompilation of callers.
    pub fn functions_to_recompile(
        &self,
        changed: &FxHashMap<String, FunctionContentHash>,
    ) -> FxHashSet<String> {
        let mut result = FxHashSet::default();

        for (name, new_hash) in changed {
            // The changed function itself always needs recompilation
            result.insert(name.clone());

            // Check if the signature changed (not just the body)
            if let Some(old_deps) = self.functions.get(name) {
                if old_deps.content_hash.signature_hash != new_hash.signature_hash {
                    // Signature changed — all callers must recompile
                    if let Some(caller_set) = self.callers.get(name) {
                        result.extend(caller_set.iter().cloned());
                    }
                }
                // If only the body changed, callers are NOT affected
            }
        }

        result
    }
}
```

### Per-Function Compilation

Each function is compiled to its own LLVM module (one function per module). This enables maximum parallelism and fine-grained caching. Functions that don't need recompilation produce no LLVM work at all.

```
Source file
    ↓
Parse + Type check (via Salsa — Section 12.4)
    ↓
For each function in the module:
    ├── Compute FunctionContentHash
    ├── Check ARC IR cache (Section 12.3, Layer 1)
    │   ├── Hit: load cached ARC IR
    │   └── Miss: run ARC analysis (borrow inference, RC insertion, elimination)
    ├── Check object code cache (Section 12.3, Layer 2)
    │   ├── Hit: reuse cached .o file
    │   └── Miss: create LLVM module, lower ARC IR, emit .o
    └── Store results in cache
    ↓
Link all .o files (updated + cached)
    ↓
Executable
```

### Fallback to File-Level

Function-level tracking is not always possible. The following cases fall back to file-level recompilation:

- **Module-level initialization:** Top-level code that runs at module load time cannot be attributed to a single function.
- **Complex cross-function dependency cycles:** Mutually recursive functions within the same module where signature changes cascade.
- **First compilation:** No cached data exists, so all functions compile fresh (equivalent to file-level).
- **Compiler version change:** Cache invalidation clears all function-level caches.

The existing file-level infrastructure (DependencyGraph, `files_to_recompile()`) handles these cases. Function-level granularity is an optimization layered on top, not a replacement.

- [ ] Implement `FunctionContentHash` with AST-body + signature + callees + globals hashing
- [ ] Implement `FunctionDependencyGraph` with signature-aware invalidation
- [ ] Implement per-function LLVM module creation (one function per module)
- [ ] Wire up fallback to file-level for module-level initialization and dependency cycles
- [ ] Test: changing function body without signature change does NOT recompile callers
- [ ] Test: changing function signature DOES recompile all callers

---

## 12.3 Two-Layer Cache

V2 introduces a two-layer caching strategy that can skip both ARC analysis and LLVM compilation independently. This is a first-class feature, not an afterthought.

### Layer 1: ARC IR Cache

Serialized ARC IR per function. If a function's content hash is unchanged, the entire ARC analysis pipeline (borrow inference, RC insertion, RC elimination, constructor reuse) is skipped and cached ARC IR is loaded directly.

```rust
/// Cache key for ARC IR artifacts.
///
/// The ARC IR cache sits between type checking and LLVM emission.
/// A cache hit skips: borrow inference, RC insertion, RC elimination,
/// and constructor reuse analysis.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArcIrCacheKey {
    /// Function content hash (body + signature + callees + globals).
    function_hash: ContentHash,
}

/// Cached ARC IR for a single function.
///
/// ARC IR types (ArcFunction, ArcBlock, ArcInstr) need Serialize/Deserialize
/// derives for binary serialization via bincode.
#[derive(Debug)]  // + Serialize, Deserialize via serde
pub struct CachedArcIr {
    /// The serialized ARC IR (bincode format for speed).
    data: Vec<u8>,
    /// Hash of the serialized data (for Layer 2 cache key).
    hash: ContentHash,
}
```

**Serialization format:** Binary via bincode for speed. ARC IR types (`ArcFunction`, `ArcBlock`, `ArcInstr`, `ArcTerminator`, `ArcVarId`, `ArcBlockId`, `ArcParam`, `ArcValue`) need `Serialize` and `Deserialize` derives added (Section 06 types). JSON is too slow for incremental builds; bincode adds negligible overhead.

### Layer 2: Object Code Cache

Compiled object code per function. If the ARC IR hash is unchanged AND the optimization config hash is unchanged, LLVM compilation is skipped entirely and the cached `.o` file is reused.

```rust
/// Cache key for compiled object code.
///
/// Combines the ARC IR hash with the optimization configuration so that
/// switching between debug and release invalidates object caches but not
/// ARC IR caches.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObjectCacheKey {
    /// Hash of the ARC IR (output of Layer 1).
    arc_ir_hash: ContentHash,
    /// Hash of optimization config (level, LTO mode, target triple).
    opt_config_hash: ContentHash,
    /// Combined key for file naming.
    combined: ContentHash,
}
```

### Pipeline With Caching

The full pipeline with both cache layers:

```
Source → [FunctionContentHash check]
    ├── Hash unchanged → Load cached ARC IR (Layer 1 hit)
    │       ├── [ObjectCacheKey check]
    │       │   ├── Hit → Reuse cached .o (Layer 2 hit) ← fastest path
    │       │   └── Miss → LLVM emission → cache .o
    │       └── (continue)
    └── Hash changed → ARC analysis → cache ARC IR
            ├── [ObjectCacheKey check]
            │   └── Always miss (ARC IR changed) → LLVM emission → cache .o
            └── (continue)
```

**Key insight:** Layer 1 and Layer 2 are independent. Changing optimization level (e.g., debug to release) invalidates Layer 2 (object code) but not Layer 1 (ARC IR). The ARC IR is optimization-level-independent. This means switching between debug and release rebuilds LLVM modules but skips the entire ARC analysis pipeline.

### Cache Directory Structure (V2)

```text
build/
└── cache/
    ├── version                    # Compiler version for full invalidation
    ├── hashes.json                # File-level content hashes (existing)
    ├── deps/                      # File-level dependency graphs (existing)
    ├── objects/                   # File-level cached .o files (existing)
    ├── functions/                 # NEW: function-level caches
    │   ├── arc_ir/                # Layer 1: cached ARC IR per function
    │   │   ├── <function_hash>.bin
    │   │   └── ...
    │   ├── objects/               # Layer 2: cached .o per function
    │   │   ├── <object_key>.o
    │   │   └── ...
    │   └── deps.json              # Function dependency graph
    └── meta/                      # Metadata (existing)
```

- [ ] Add `Serialize`/`Deserialize` derives to ARC IR types in `ori_arc`
- [ ] Implement `ArcIrCacheKey` and `CachedArcIr` with bincode serialization
- [ ] Implement `ObjectCacheKey` combining ARC IR hash + optimization config
- [ ] Implement function-level cache directory structure within existing `ArtifactCache`
- [ ] Test: Layer 1 cache hit (unchanged function skips ARC analysis)
- [ ] Test: Layer 2 cache hit (unchanged ARC IR + same opt level skips LLVM)
- [ ] Test: changing opt level invalidates Layer 2 but not Layer 1
- [ ] Benchmark: measure ARC IR serialization/deserialization overhead vs. recomputation

---

## 12.4 Salsa Integration (Hybrid)

V2 uses a hybrid approach: Salsa for front-end invalidation detection, ArtifactCache for back-end caching. Codegen is NOT a Salsa query.

### Why Codegen Is Not a Salsa Query

Salsa query return types must satisfy `Clone + Eq + PartialEq + Hash + Debug`. LLVM types (`Module<'ctx>`, `FunctionValue<'ctx>`, `BasicBlock<'ctx>`) are:
- Lifetime-bound to an LLVM `Context` (not `'static`)
- Not `Clone` (LLVM modules are unique owners)
- Not `Eq`/`Hash` (no structural equality on LLVM IR)
- Contain raw FFI pointers (`LLVMModuleRef`, `LLVMValueRef`)

Making codegen a Salsa query would require serializing LLVM IR to a Salsa-compatible format (e.g., bitcode bytes), which adds complexity for no benefit since ArtifactCache already handles caching compiled artifacts.

### Salsa Front-End Pipeline

The existing Salsa query graph handles the front-end:

```
SourceFile (input, #[salsa::input])
    ↓ file.text(db)
tokens(db, file)         — #[salsa::tracked], early cutoff on token equality
    ↓
parsed(db, file)         — #[salsa::tracked], early cutoff on AST equality
    ↓
typed(db, file)          — #[salsa::tracked], early cutoff on type result equality
    ↓
[codegen boundary — not a Salsa query]
    ↓
ARC analysis → LLVM emission → object file (managed by ArtifactCache)
```

**Salsa's early cutoff** is the key mechanism. When `file.set_text()` is called with new source:
1. `tokens()` re-lexes. If tokens are identical (e.g., whitespace-only change), parsing is skipped entirely.
2. `parsed()` re-parses. If the AST is identical, type checking is skipped.
3. `typed()` re-checks. If the `TypeCheckResult` is identical (same types, same function signatures), codegen is skipped.

Step 3 is where the hybrid handoff occurs. If `typed()` returns the same result, the function content hashes will be unchanged, and both cache layers (12.3) produce hits. No codegen work occurs.

### Existing Salsa Setup

The existing `db.rs` and `query/mod.rs` provide the foundation:

- **`CompilerDb`** (`compiler/oric/src/db.rs`): Concrete Salsa database with `salsa::Storage`, `SharedInterner`, `file_cache` (`RwLock<HashMap<PathBuf, SourceFile>>`), and event logging. `load_file()` creates `SourceFile` inputs with path canonicalization and deduplication.
- **`SourceFile`** (`#[salsa::input]`): Salsa input with `path` and `text` fields. `set_text()` triggers invalidation of all dependent queries.
- **`#[salsa::input]` durability:** `SourceFile` inputs use default durability. For build-time constants (e.g., compiler flags, target triple), `salsa::Durability::HIGH` prevents re-checking queries that depend only on stable inputs. V2 should use `HIGH` durability for `CacheConfig`-equivalent data that changes only between build invocations.

### Hybrid Flow

```
1. User edits source file
2. Salsa: file_cache detects change → set_text() on SourceFile input
3. Salsa: tokens() → parsed() → typed() cascade with early cutoff
4. typed() result compared to previous:
   ├── Unchanged → No codegen needed (Salsa early cutoff)
   └── Changed → Extract per-function changes
5. Per-function: compute FunctionContentHash from typed() output
6. ArtifactCache: check Layer 1 (ARC IR) and Layer 2 (object code)
7. Compile only functions with cache misses
8. Link updated .o files with cached .o files
```

- [ ] Document the Salsa/ArtifactCache boundary in code comments
- [ ] Use `Durability::HIGH` for build configuration inputs
- [ ] Implement the typed()-to-FunctionContentHash extraction
- [ ] Verify early cutoff works for whitespace-only changes (tokens unchanged → no reparse)
- [ ] Verify early cutoff works for comment-only changes (AST unchanged → no recheck)
- [ ] Test: modify function body without signature change → typed() result changes → only that function recompiles

---

## 12.5 Parallel Compilation

V2 uses `std::thread` for parallel compilation. No rayon dependency.

### Existing Discrepancy

The current code has two parallel execution paths with different behavior:

1. **`compile_parallel()`** (free function): Uses `std::thread` with `AtomicUsize` round-robin index. Items are processed in arbitrary order — dependency ordering is IGNORED. Worker threads grab the next item by index, regardless of whether its dependencies are satisfied.

2. **`ParallelCompiler::execute()`** (method): Uses `CompilationPlan` with dependency-respecting scheduling (ready queue + pending set). However, execution is SINGLE-THREADED — it calls `plan.take_next()` in a loop on the calling thread.

**V2 fix:** Combine the best of both. Use `CompilationPlan`'s dependency-respecting scheduling WITH multi-threaded execution. Worker threads wait for items to become ready (dependencies satisfied) before processing them.

### V2 Parallel Executor

```rust
/// Execute a compilation plan with multi-threaded, dependency-respecting scheduling.
///
/// Worker threads pull from a shared ready queue. When a function completes,
/// its dependents may become ready and are added to the queue. This ensures
/// correct ordering while maximizing parallelism.
pub fn execute_parallel<F>(
    plan: CompilationPlan,
    jobs: usize,
    compile_fn: F,
) -> Result<CompilationStats, Vec<CompileError>>
where
    F: Fn(&WorkItem) -> Result<CompileResult, CompileError> + Send + Sync,
{
    let jobs = effective_jobs(jobs);

    // Shared state protected by Mutex + Condvar for notification
    let shared = Arc::new(SharedPlanState {
        plan: Mutex::new(plan),
        condvar: Condvar::new(),
    });

    // Spawn worker threads
    let handles: Vec<_> = (0..jobs).map(|_| {
        let shared = Arc::clone(&shared);
        let compile_fn = &compile_fn; // borrow, not move
        thread::spawn(move || {
            loop {
                // Wait for a ready item or completion
                let item = {
                    let mut plan = shared.plan.lock().unwrap();
                    loop {
                        if let Some(item) = plan.take_next() {
                            break item.clone();
                        }
                        if plan.is_complete() {
                            return; // All done
                        }
                        // Wait for notification that a dependency completed
                        plan = shared.condvar.wait(plan).unwrap();
                    }
                };

                // Compile outside the lock
                let result = compile_fn(&item);

                // Report completion and wake other threads
                let mut plan = shared.plan.lock().unwrap();
                match result {
                    Ok(_) => plan.complete(&item.path),
                    Err(_) => { /* don't mark complete — dependents can't proceed */ }
                }
                shared.condvar.notify_all();
            }
        })
    }).collect();

    // Join all threads
    for handle in handles {
        handle.join().unwrap();
    }

    // Extract results
    // ...
}
```

### Thread Safety Model

- **One LLVM Context per thread.** This is already the pattern in `build.rs`. LLVM Contexts are not thread-safe; each worker thread creates its own `Context` and builds modules within it. No cross-context value sharing (see Section 02: ValueId scoping).
- **ARC IR computed once, distributed to threads.** The ARC analysis pipeline (`ori_arc`) runs on the main thread (or a dedicated analysis thread) and produces `ArcFunction` values. These are read-only data distributed to LLVM emission threads. No mutation of ARC IR during emission.
- **ArtifactCache is thread-safe.** Cache lookups and stores use atomic file operations (write to temp file, rename). Multiple threads can read/write the cache concurrently without locking.

### Connection to Section 02

Section 02 establishes that `ValueId` is scoped to a single LLVM Context. The parallel compilation model respects this: each thread's `IrBuilder` operates on its own Context and arena. No `ValueId` crosses a thread boundary. This is enforced by the lifetime parameter on `IrBuilder<'ctx>`.

- [ ] Implement `execute_parallel` combining `CompilationPlan` scheduling with `std::thread` workers
- [ ] Replace the existing `compile_parallel` round-robin function with dependency-respecting version
- [ ] Ensure one LLVM Context per thread (already the pattern, verify preserved)
- [ ] Test: parallel compilation produces identical output to sequential compilation
- [ ] Test: dependency ordering is respected (dependent functions wait for prerequisites)
- [ ] Benchmark: measure parallelism scaling (2, 4, 8, 16 threads)

---

## 12.6 Multi-File Integration

The existing `multi_file.rs` pipeline integrates with function-level caching via a two-level scheduling strategy: module-level topological ordering first, then function-level parallelism within each module.

### Two-Level Scheduling

```
Level 1: Module ordering (existing DependencyGraph)
    modules in topological order: [core.ori, utils.ori, lib.ori, main.ori]

Level 2: Function parallelism (new FunctionDependencyGraph)
    For each module (in dependency order):
        1. Parse + type check (via Salsa)
        2. Extract functions and compute content hashes
        3. Check function-level caches (Layer 1 + Layer 2)
        4. Compile uncached functions in parallel (one LLVM module per function)
        5. Collect .o files (cached + freshly compiled)

Level 3: Link
    All per-function .o files → linker → executable
```

**Module ordering constraint:** A module's type information must be available before its dependents can type-check. This is inherent to the Salsa query graph (imports are resolved during `typed()`). Function-level parallelism operates WITHIN a module, not across modules that have dependency relationships.

**Cross-module function references** are resolved at link time via mangled names (Section 04.5). Each per-function `.o` file declares (but does not define) external functions it calls. The linker resolves these symbols from the `.o` files of other modules.

### Incremental Linking

For debug builds where link time dominates:
- **Object file replacement:** Updated per-function `.o` files replace their predecessors. The linker re-links only the changed objects. This is the simplest incremental linking strategy and sufficient for initial V2.
- **Future: `ld -r` partial linking:** Combine groups of related functions into pre-linked relocatable objects. Reduces the number of objects the final link must process.

For release builds:
- Full link with LTO (Section 11.5) runs on every build. LTO inherently requires whole-program analysis, so incremental linking does not apply.

### Integration With Existing Pipeline

The existing `build_dependency_graph()` → topological sort → compile → link pipeline in `multi_file.rs` remains the outer shell. V2 replaces the per-module `compile_module_to_object()` step with a per-function compilation loop that checks caches and compiles only what's needed.

```rust
/// V2 per-module compilation: function-level granularity.
///
/// Replaces the existing compile_module_to_object with per-function caching.
fn compile_module_functions(
    db: &dyn Db,
    file: SourceFile,
    cache: &ArtifactCache,
    func_cache: &FunctionCache,
) -> Result<Vec<PathBuf>, MultiFileError> {
    // 1. Type check via Salsa (early cutoff may skip this entirely)
    let type_result = typed(db, file);

    // 2. Extract per-function hashes
    let functions = extract_function_hashes(&type_result);

    // 3. Partition into cached vs uncached
    let (cached, uncached): (Vec<_>, Vec<_>) = functions
        .into_iter()
        .partition(|f| func_cache.has_object(&f.content_hash));

    // 4. Compile uncached functions in parallel
    let new_objects = compile_functions_parallel(&uncached, cache, func_cache)?;

    // 5. Collect all .o paths (cached + new)
    let mut all_objects: Vec<PathBuf> = cached
        .iter()
        .filter_map(|f| func_cache.get_object_path(&f.content_hash))
        .collect();
    all_objects.extend(new_objects);

    Ok(all_objects)
}
```

- [ ] Implement two-level scheduling (module ordering + function parallelism)
- [ ] Replace `compile_module_to_object` with `compile_module_functions`
- [ ] Ensure cross-module symbol resolution via mangled names works with per-function .o files
- [ ] Test: multi-module program with per-function incremental compilation
- [ ] Test: changing one function in a dependency does not recompile dependent modules (unless signature changed)

---

## Completion Checklist

- [ ] Function-level content hashing implemented (body + signature + callees + globals)
- [ ] Function-level dependency graph with signature-aware invalidation
- [ ] ARC IR cache (Layer 1): serialize/deserialize ARC IR per function via bincode
- [ ] Object code cache (Layer 2): per-function .o files keyed by ARC IR hash + opt config
- [ ] Salsa hybrid integration: front-end queries with early cutoff, ArtifactCache for back-end
- [ ] Codegen is NOT a Salsa query (documented with rationale)
- [ ] `execute_parallel` replaces `compile_parallel` with dependency-respecting multi-threaded execution
- [ ] `std::thread` used throughout (no rayon)
- [ ] One LLVM Context per thread, no cross-context ValueId sharing
- [ ] Two-level scheduling: module topological order + function-level parallelism
- [ ] Cross-module references resolved at link time via mangled names
- [ ] Fallback to file-level for module-level initialization and dependency cycles
- [ ] All existing incremental infrastructure preserved and functional

**Exit Criteria:** Changing one function recompiles only that function (and callers if the signature changed). Multi-core machines compile faster via dependency-respecting parallel execution. ARC analysis results are cached and reused across builds. Switching between debug and release skips ARC analysis (Layer 1 cache hit) while recompiling LLVM modules (Layer 2 cache miss).
