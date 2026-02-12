---
plan: "dpr_architecture_02112026"
title: "Design Pattern Review: Compiler Architecture"
status: draft
---

# Design Pattern Review: Compiler Architecture

## Ori Today

Ori is a 16-crate Salsa-first incremental compiler with a clean unidirectional pipeline: `SourceFile` -> `lex_result()` -> `tokens()` -> `parsed()` -> `typed()` -> `evaluated()` (interpreter) or `check_source()` -> ARC analysis -> LLVM codegen. The Salsa database (`CompilerDb` in `oric/src/db.rs`) wraps all phases with automatic memoization and early cutoff. String interning (`SharedInterner`), arena allocation (`ExprArena` + `ExprId`), and newtypes (`Name(u32)`, `Idx(u32)`, `TypeId(u32)`) ensure cache-friendly, Salsa-compatible data flow. Phase boundaries are well-enforced: `ori_ir` and `ori_diagnostic` sit at the bottom of the dependency graph with no upward dependencies, and each phase consumes the output of the previous one without back-references.

The architecture has several concrete strengths. The type system V2 (`ori_types`) uses a unified `Pool` with compact 5-byte `Item` storage, `Tag`-driven dispatch, and pre-interned primitive indices 0-11 aligned with `TypeId` in `ori_ir`. The `InferEngine` provides rank-based HM inference with path-compressed union-find unification. The canonicalization pass (`ori_canon`) bridges the AST and backend by eliminating syntactic sugar, compiling pattern matches into decision trees, and attaching resolved types to every node. The ARC pipeline (`ori_arc`) implements the full Lean 4-inspired sequence: borrow inference -> liveness -> RC insertion -> reset/reuse detection -> expansion -> RC elimination, with a clean three-way type classification (`Scalar`/`DefiniteRef`/`PossibleRef`). The pattern system (`ori_patterns`) follows the Open/Closed principle with trait-based `PatternDefinition` dispatch.

What is missing or fragile: (1) The Salsa/codegen boundary is a cliff edge -- everything up to `typed()` is incrementally cached, but `ori_arc` and `ori_llvm` operate outside Salsa because LLVM types are not `Clone + Eq + Hash`. The `ArtifactCache` in `compile_common.rs` is an ad-hoc mitigation. (2) Error recovery is asymmetric across phases: the lexer produces `LexError` vectors, the parser uses `ParseError` with progress-aware `ParseOutcome`, the type checker accumulates `TypeCheckError` with `ErrorGuaranteed` guarantees, and the evaluator uses `EvalError` -- each with different strategies and no shared accumulation trait. (3) The `ori_canon` crate is a recent addition positioned between type checking and evaluation/codegen, but its integration into the incremental pipeline is not yet defined: changes to canonicalization logic currently invalidate all downstream work. (4) Import resolution (`oric/src/imports.rs`, `typeck.rs`) combines file I/O, Salsa queries, and type registration in a single call chain, making it difficult to test and extend to multi-module workspaces. (5) The ARC pipeline has no type-level enforcement of pass ordering -- the correct sequence is documented in comments and validated by integration tests, but a mis-ordered call compiles without error.

## Prior Art

### Rust -- Query-Based Incremental System

Rust's compiler uses Salsa-style queries as the backbone for incremental compilation. Every computation is a node in a dependency graph with fingerprints for change detection. The key architectural insight is the `Callbacks` trait (`rustc_driver_impl`), which provides well-defined hooks at phase boundaries (`after_expansion`, `after_analysis`) while keeping the compilation pipeline as a library. The `DiagCtxt` is a thread-safe global singleton, and `ErrorGuaranteed` tokens provide type-level proof that errors were reported. This eliminates the "silent error" class of bugs where the compiler continues past an error without recording it.

What makes Rust's approach work is the combination of implicit dependency tracking (via `TyCtxt`'s thread-local context) and explicit phase hooks. The driver is separate from the compilation engine (`rustc_driver` wraps `rustc_driver_impl`), making it possible to embed the compiler as a library (rust-analyzer does this). The tradeoff is high cognitive overhead: understanding when a query will re-execute requires understanding Salsa's memoization semantics, and debugging cache misses is notoriously difficult.

### Zig -- Monolithic Staged Work Queue

Zig's `Compilation` struct is a single monolithic type that holds all compiler state: allocators, config, work queues, module data, and build artifacts. The three IR levels (ZIR -> AIR -> target code) are processed through staged work queues with a single `mutex` protecting shared state. Granular invalidation uses `AnalUnit` (a packed u64 encoding thread ID + function/type/nav) to track dependencies at the function level, not the file level. Multiple dependency maps (`src_hash_deps`, `nav_val_deps`, `nav_ty_deps`, `interned_deps`) enable precise invalidation: changing a function's body re-analyzes only its callers, not the entire module.

The strength of Zig's approach is its explicit staging and predictable behavior. The `Config` type is separated from `Compilation` as a standalone struct (`Compilation/Config.zig`), keeping configuration values immutable after initialization. The work queue model makes it easy to reason about what runs when, and the single-mutex design eliminates data races at the cost of potential contention. For Ori, the key takeaway is Zig's granular invalidation model: tracking dependencies at function granularity rather than file granularity would significantly improve incremental compilation for multi-function files.

### Go -- Concurrent SSA Pipeline

Go uses a sequential front-end (typecheck -> devirtualize/inline -> escape analysis -> walk) feeding into a concurrent back-end where SSA compilation runs per-function in parallel. The front-end uses a fixed-point loop that discovers new work (reflects, calls, externs) during compilation and re-runs until no new work appears. The back-end uses a simple work queue sorted by function body size (longest first) for load balancing, with a semaphore controlling parallelism.

The distinctive design choice is Go's stateless pipeline: `ir.CurFunc = nil` after walk enforces that each pass is complete before the next begins. There is no incremental compilation -- every build starts fresh. This simplicity makes Go's compiler extremely fast for clean builds (the entire compiler is ~100K lines) and easy to debug. The relevant pattern for Ori is the concurrent back-end: once type checking is complete, ARC analysis and LLVM codegen for independent functions can run in parallel. Go also demonstrates that a fixed-point discovery loop can be cleaner than upfront dependency resolution for capabilities like reflect that introduce implicit dependencies.

### Gleam -- Pure Library Architecture

Gleam's `compiler-core` crate is a pure library with zero global state. All compiler state is passed explicitly through function parameters. Phases are pure functions: `parse(input) -> parsed`, `analyse(parsed, modules) -> analysed`, `type_::check(analysed) -> typed`, `codegen::generate(typed, target) -> code`. The `ProjectCompiler` struct orchestrates the pipeline, but the individual phases have no knowledge of the orchestrator. Module interfaces are serialized via Cap'n Proto for cross-module type information, enabling fast recompilation by reading only signatures (not bodies) of dependencies.

The strength is testability and composability. Each phase can be tested in isolation with synthetic inputs. The CLI is a thin shell that calls the library; IDEs call the same library with different orchestration. The tradeoff is the lack of shared state: each phase re-reads what it needs from its inputs, and there is no opportunity for cross-phase memoization. For Ori, Gleam's pattern of "compiler as pure library with thin CLI driver" is directly applicable and would improve testability of the compilation pipeline. Gleam's module interface serialization is also relevant for Ori's future package management story.

## Proposed Best-of-Breed Design

### Core Idea

Ori should combine Rust's Salsa-based incremental query system (already in place) with Zig's function-granularity invalidation, Gleam's pure-library phase architecture, and Go's concurrent back-end parallelism. The key architectural change is to push the Salsa boundary deeper: instead of stopping at `typed()` and falling off a cliff into ad-hoc caching, the pipeline should define a `canonicalized()` query and a content-hash-based cache for ARC IR and object artifacts that integrates cleanly with Salsa's invalidation model. Each phase should be a pure function from input IR to output IR, with the Salsa query layer handling memoization, and the `oric` CLI acting as a thin orchestrator (following Gleam's pattern).

The second major change is introducing a typed pass pipeline for post-type-checking transformations. Currently, the ARC passes (`borrow inference -> liveness -> RC insertion -> reset/reuse -> expand -> eliminate`) are ordered by convention and tested by integration tests. Following Zig's staged work queue pattern and Rust's pass manager concepts, these should be encoded as a typed pipeline where each pass declares its inputs and outputs, and the pipeline enforces correct ordering at compile time. This eliminates the fragility of comment-documented ordering and makes it possible to add new optimization passes (e.g., closure capture analysis, copy-on-write detection) without risking mis-ordering.

### Key Design Choices

1. **Function-granularity Salsa queries** (inspired by Zig's `AnalUnit`). Instead of a single `typed(db, file) -> TypeCheckResult` that type-checks the entire module, introduce `typed_function(db, file, fn_name) -> FunctionTypeResult`. This enables Salsa's early cutoff to skip re-checking unchanged functions even when other functions in the same file change. Zig demonstrates this works well: its `AnalUnit` tracks dependencies at the function level, and Ori's existing `ExprArena` + `ExprId` architecture already supports per-function slicing via `ExprRange`.

2. **Canonicalization as a Salsa query** (inspired by Gleam's pure-function phases). The `ori_canon::lower_module()` call in `evaluated()` should become a tracked Salsa query `canonicalized(db, file) -> CanonResult`. This gives automatic memoization and early cutoff: if the `TypeCheckResult` is unchanged, canonicalization is skipped entirely. Currently, canonicalization runs unconditionally every time `evaluated()` is called, even if the types have not changed.

3. **Content-hash bridge to non-Salsa back-end** (combining Zig's content hashing with Ori's existing `ArtifactCache` concept). At the `canonicalized()` -> ARC/codegen boundary, compute a content hash per function from the `CanonResult`. This hash becomes the cache key for ARC IR and object artifacts. If a function's content hash is unchanged, its ARC analysis and codegen are skipped entirely -- even across compiler restarts (persistent cache). Zig uses a similar approach: `src_hash_deps` tracks source hashes for granular invalidation.

4. **Typed pass pipeline for ARC optimization** (inspired by Rust's pass manager and Zig's staged work queue). Replace the current free-function call chain (`insert_rc_ops(); detect_reset_reuse(); expand_reset_reuse(); eliminate_rc_ops()`) with a typed pipeline that enforces ordering at the type level:

    ```rust
    // Each pass transforms ArcFunction through a newtype wrapper
    fn insert_rc(func: Lowered<ArcFunction>) -> RcInserted<ArcFunction>;
    fn detect_reuse(func: RcInserted<ArcFunction>) -> ReuseDetected<ArcFunction>;
    fn expand_reuse(func: ReuseDetected<ArcFunction>) -> ReuseExpanded<ArcFunction>;
    fn eliminate_rc(func: ReuseExpanded<ArcFunction>) -> Optimized<ArcFunction>;
    ```

   The compiler rejects out-of-order calls because the types don't match. This is the same pattern as Rust's `ErrorGuaranteed` -- using the type system to make illegal states unrepresentable.

5. **Unified error accumulation trait** (inspired by Rust's `DiagCtxt` and Gleam's `Error` enum). Define a `DiagnosticAccumulator` trait that all phases implement, providing a common interface for error collection, deduplication, and emission. Each phase produces phase-specific error types (`LexError`, `ParseError`, `TypeCheckError`), but all implement `Into<Diagnostic>` for uniform rendering. Rust's `ErrorGuaranteed` pattern should be extended: every function that can produce errors returns `Result<T, ErrorGuaranteed>`, making it impossible to forget to report an error.

6. **Concurrent back-end per function** (inspired by Go's concurrent SSA compilation). Once canonicalization is complete, ARC analysis and LLVM codegen for each function can run independently in parallel. Go uses a work queue sorted by function body size for load balancing; Ori should do the same. The `ArcClassifier` is immutable and can be shared across threads via `&dyn ArcClassification`. LLVM contexts are thread-local, so each codegen thread creates its own. This is a natural extension of Go's pattern adapted to Ori's ARC-first compilation model.

7. **Compiler-as-library with thin CLI driver** (inspired by Gleam). The `oric` crate should be split into `ori_compiler` (a pure library exporting the compilation pipeline as functions) and `oric` (a thin CLI that calls the library). This enables the future LSP (`ori_lsp`) to call the same compilation pipeline without reimplementing orchestration logic. Gleam demonstrates this works cleanly: `compiler-core` is used by both the CLI and the language server.

8. **Module interface signatures for cross-module compilation** (inspired by Gleam's Cap'n Proto serialization). For Ori's future package management and workspace support, each module should emit a serialized interface file containing only type signatures, trait definitions, and constant values. Dependents read these interfaces for type checking without re-parsing or re-checking the dependency's body. This is the same strategy Gleam uses for fast recompilation and is essential for scaling beyond single-file compilation.

### What Makes Ori's Approach Unique

Ori's combination of ARC memory management, capability-based effects, and mandatory tests creates architectural opportunities that none of the reference compilers exploit:

**ARC-aware Salsa invalidation.** Because Ori uses ARC (not GC or borrow-checking), the type of memory management operations is determined by type information. This means the ARC classification (`Scalar`/`DefiniteRef`/`PossibleRef`) can be cached as part of the type-checking result, and changes to a function's body that don't change its type signature don't require re-running ARC classification for callers. Neither Rust (borrow-checking is local) nor Swift (ARC optimization is a late SIL pass) exploit this opportunity. Lean 4 is closest, but its functional purity means it doesn't need to cache across incremental rebuilds.

**Capability effects as dependency edges.** Ori's `uses Http`, `uses FileSystem` annotations create explicit dependency edges that the incremental system can exploit. A function using only `Pure` capabilities is a pure computation that Salsa can cache aggressively (HIGH durability). A function using `Print` has observable side effects and should be re-evaluated on every run. This is a natural extension of Salsa's durability model that no reference compiler implements: Koka tracks effects for type safety, but doesn't use them for incremental caching; Zig tracks effects for safety but not for caching.

**Mandatory tests as incremental invalidation sources.** Ori requires tests for every function. This means the compiler knows the dependency graph between tests and functions statically. When a function changes, Salsa can invalidate only the tests that target that function (via `TestDef.targets`), rather than re-running all tests. This is a form of incremental test execution that Go (which re-runs everything) and Rust (which uses heuristic package-level caching) cannot achieve.

**Expression-based semantics enable simpler ARC analysis.** Because every block evaluates to its last expression and there is no `return` keyword, the control flow graph for ARC analysis is simpler than in languages with arbitrary return points. Every block has exactly one exit value, which means RC analysis for block exit values is a single backward pass from the terminator, not a dataflow analysis across multiple return sites. This is a structural advantage over Swift (multiple `return` statements) and even Lean 4 (which has `do` notation with early returns).

### Concrete Types & Interfaces

```rust
// === Phase 1: Function-granularity Salsa queries ===

/// Per-function type checking result (replaces module-level typed())
#[salsa::tracked]
pub fn typed_function(
    db: &dyn Db,
    file: SourceFile,
    fn_index: u32,
) -> FunctionTypeResult {
    let parse_result = parsed(db, file);
    let func = &parse_result.module.functions[fn_index as usize];
    // Type check this function in the context of the module's registries
    type_check_function(db, &parse_result, func, file.path(db))
}

/// Per-function canonical IR
#[salsa::tracked]
pub fn canonicalized_function(
    db: &dyn Db,
    file: SourceFile,
    fn_index: u32,
) -> CanonFunctionResult {
    let typed = typed_function(db, file, fn_index);
    let parse_result = parsed(db, file);
    // Lower single function to canonical IR
    ori_canon::lower_function(&parse_result, &typed)
}

// === Phase 2: Typed ARC Pass Pipeline ===

/// Newtype wrappers enforcing pass ordering at the type level.
/// Each wrapper is a zero-cost abstraction (#[repr(transparent)]).
#[repr(transparent)]
pub struct Lowered<T>(pub T);

#[repr(transparent)]
pub struct BorrowAnnotated<T>(pub T);

#[repr(transparent)]
pub struct RcInserted<T>(pub T);

#[repr(transparent)]
pub struct ReuseDetected<T>(pub T);

#[repr(transparent)]
pub struct ReuseExpanded<T>(pub T);

#[repr(transparent)]
pub struct Optimized<T>(pub T);

/// The ARC pipeline as a typed state machine.
/// Each method consumes the current state and produces the next.
pub struct ArcPipeline<'a> {
    classifier: &'a dyn ArcClassification,
}

impl<'a> ArcPipeline<'a> {
    pub fn new(classifier: &'a dyn ArcClassification) -> Self {
        Self { classifier }
    }

    pub fn lower(
        &self,
        canon: &CanonResult,
        pool: &Pool,
        interner: &StringInterner,
    ) -> Lowered<ArcFunction> {
        Lowered(lower_function_can(canon, pool, interner, self.classifier))
    }

    pub fn infer_borrows(
        &self,
        func: Lowered<ArcFunction>,
    ) -> BorrowAnnotated<ArcFunction> {
        let mut f = func.0;
        let sigs = borrow::infer_borrows(&[f.clone()], self.classifier);
        borrow::apply_borrows(&mut [&mut f], &sigs);
        BorrowAnnotated(f)
    }

    pub fn insert_rc(
        &self,
        func: BorrowAnnotated<ArcFunction>,
    ) -> RcInserted<ArcFunction> {
        let mut f = func.0;
        let liveness = compute_liveness(&f, self.classifier);
        insert_rc_ops(&mut f, self.classifier, &liveness);
        RcInserted(f)
    }

    pub fn detect_reuse(
        &self,
        func: RcInserted<ArcFunction>,
    ) -> ReuseDetected<ArcFunction> {
        let mut f = func.0;
        detect_reset_reuse(&mut f, self.classifier);
        ReuseDetected(f)
    }

    pub fn expand_reuse(
        &self,
        func: ReuseDetected<ArcFunction>,
    ) -> ReuseExpanded<ArcFunction> {
        let mut f = func.0;
        expand_reset_reuse(&mut f, self.classifier);
        ReuseExpanded(f)
    }

    pub fn eliminate_rc(
        &self,
        func: ReuseExpanded<ArcFunction>,
    ) -> Optimized<ArcFunction> {
        let mut f = func.0;
        eliminate_rc_ops(&mut f);
        Optimized(f)
    }

    /// Run the full pipeline in one call (convenience method).
    pub fn run(&self, func: Lowered<ArcFunction>) -> Optimized<ArcFunction> {
        let annotated = self.infer_borrows(func);
        let inserted = self.insert_rc(annotated);
        let detected = self.detect_reuse(inserted);
        let expanded = self.expand_reuse(detected);
        self.eliminate_rc(expanded)
    }
}

// === Phase 3: Content-hash bridge ===

/// Content hash for a canonicalized function, used as cache key
/// for ARC IR and object artifacts.
#[derive(Clone, Copy, Eq, PartialEq, Hash, Debug)]
pub struct FunctionContentHash(u64);

impl FunctionContentHash {
    pub fn compute(canon_func: &CanonResult, fn_name: Name) -> Self {
        use std::hash::{Hash, Hasher};
        let mut hasher = rustc_hash::FxHasher::default();
        // Hash the canonical IR for this function
        canon_func.root_for(fn_name).hash(&mut hasher);
        Self(hasher.finish())
    }
}

/// Persistent cache for ARC-optimized IR, keyed by content hash.
pub struct ArcIrCache {
    entries: FxHashMap<FunctionContentHash, Optimized<ArcFunction>>,
}

impl ArcIrCache {
    pub fn new() -> Self {
        Self { entries: FxHashMap::default() }
    }

    pub fn get(
        &self,
        hash: FunctionContentHash,
    ) -> Option<&Optimized<ArcFunction>> {
        self.entries.get(&hash)
    }

    pub fn insert(
        &mut self,
        hash: FunctionContentHash,
        func: Optimized<ArcFunction>,
    ) {
        self.entries.insert(hash, func);
    }
}

// === Phase 4: Unified error accumulation ===

/// Trait for phase-specific error types that can be rendered as Diagnostics.
pub trait IntoDiagnostic {
    fn into_diagnostic(
        self,
        interner: &StringInterner,
    ) -> ori_diagnostic::Diagnostic;
}

/// Accumulator that collects errors from all phases.
/// Implements Rust's ErrorGuaranteed pattern: calling `emit()` returns
/// proof that an error was reported.
pub struct PhaseErrors {
    diagnostics: Vec<ori_diagnostic::Diagnostic>,
    guaranteed: bool,
}

impl PhaseErrors {
    pub fn new() -> Self {
        Self { diagnostics: Vec::new(), guaranteed: false }
    }

    /// Emit an error, returning proof that it was reported.
    pub fn emit(
        &mut self,
        error: impl IntoDiagnostic,
        interner: &StringInterner,
    ) -> ErrorGuaranteed {
        self.diagnostics.push(error.into_diagnostic(interner));
        self.guaranteed = true;
        ErrorGuaranteed::new_unchecked()
    }

    pub fn has_errors(&self) -> bool {
        self.guaranteed
    }

    pub fn into_diagnostics(self) -> Vec<ori_diagnostic::Diagnostic> {
        self.diagnostics
    }
}

// === Phase 5: Compiler-as-library ===

/// The compiler library entry point.
/// CLI and LSP both call this; only orchestration differs.
pub struct CompilationPipeline {
    db: CompilerDb,
}

impl CompilationPipeline {
    pub fn new() -> Self {
        Self { db: CompilerDb::new() }
    }

    pub fn with_interner(interner: SharedInterner) -> Self {
        Self { db: CompilerDb::with_interner(interner) }
    }

    /// Check a single file: lex -> parse -> type check.
    /// Returns all accumulated errors.
    pub fn check(&self, path: &Path) -> CheckResult {
        let file = self.db.load_file(path).expect("file not found");
        let lex_errs = lex_errors(&self.db, file);
        let parse_result = parsed(&self.db, file);
        let type_result = typed(&self.db, file);
        CheckResult { lex_errs, parse_result, type_result }
    }

    /// Full compilation: check -> canonicalize -> ARC -> codegen.
    /// Returns the compiled artifact or accumulated errors.
    pub fn compile(
        &self,
        path: &Path,
        target: CompileTarget,
    ) -> CompileResult {
        let check = self.check(path);
        if check.has_errors() {
            return CompileResult::Errors(check.all_errors());
        }
        // Canonicalize, run ARC pipeline, generate code...
        todo!("full compilation pipeline")
    }

    /// Access the database for direct Salsa queries (advanced use).
    pub fn db(&self) -> &CompilerDb {
        &self.db
    }
}

// === Phase 6: Concurrent back-end ===

/// Per-function compilation unit for parallel back-end processing.
/// Immutable after construction; safe to send across threads.
pub struct FunctionCompileUnit {
    pub name: Name,
    pub content_hash: FunctionContentHash,
    pub arc_ir: Optimized<ArcFunction>,
    pub signature: FunctionSig,
    pub body_size: usize, // for Go-style longest-first scheduling
}

/// Parallel back-end scheduler.
/// Processes function compile units on a thread pool, sorted by
/// body_size descending (Go's load-balancing strategy).
pub fn compile_functions_parallel(
    units: Vec<FunctionCompileUnit>,
    thread_count: usize,
) -> Vec<CompiledFunction> {
    let mut sorted = units;
    sorted.sort_by(|a, b| b.body_size.cmp(&a.body_size));

    // Each thread gets its own LLVM Context (thread-local)
    // and processes functions from the sorted work queue.
    std::thread::scope(|scope| {
        let (tx, rx) = std::sync::mpsc::channel();
        let work = std::sync::Mutex::new(sorted.into_iter());

        for _ in 0..thread_count {
            let tx = tx.clone();
            let work = &work;
            scope.spawn(move || {
                // Each thread creates its own LLVM context
                loop {
                    let unit = {
                        let mut guard = work.lock().unwrap();
                        guard.next()
                    };
                    let Some(unit) = unit else { break };
                    let compiled = compile_single_function(unit);
                    tx.send(compiled).unwrap();
                }
            });
        }
        drop(tx);
        rx.into_iter().collect()
    })
}
```

## Implementation Roadmap

### Phase 1: Foundation

- [ ] Split `oric` into `ori_compiler` (library) and `oric` (thin CLI driver), following Gleam's pattern. Move Salsa queries, import resolution, and typeck bridge into `ori_compiler`. CLI only handles argument parsing, file I/O, and diagnostic emission.
- [ ] Introduce `IntoDiagnostic` trait and implement it for `LexError`, `ParseError`, `TypeCheckError`, and `EvalError`. This unifies error rendering across all phases without changing existing error types.
- [ ] Promote `ori_canon::lower_module()` to a Salsa query `canonicalized(db, file) -> CanonResult`. Wire it into `evaluated()` and `compile_common.rs` to replace the current inline call.
- [ ] Add content hashing infrastructure: `FunctionContentHash::compute()` from `CanonResult`, and a basic in-memory `ArcIrCache` keyed by content hash. Integrate into `compile_common.rs` as an opt-in optimization.

### Phase 2: Core

- [ ] Implement typed ARC pass pipeline with `Lowered`/`BorrowAnnotated`/`RcInserted`/`ReuseDetected`/`ReuseExpanded`/`Optimized` newtype wrappers. Refactor `ori_arc` public API to use these types. Existing tests should pass with minimal changes (only call-site type annotations change).
- [ ] Add function-granularity type checking: `typed_function(db, file, fn_index)` Salsa query. This requires splitting `check_module()` into a per-function `check_function()` that shares registries. The module-level `typed()` query becomes a thin wrapper that calls `typed_function()` for each function.
- [ ] Implement concurrent ARC + codegen back-end using `std::thread::scope`. Each function is an independent compile unit processed in parallel. Sort by body size (Go's strategy) for load balancing. Guard with a `--jobs N` CLI flag (default: `num_cpus`).
- [ ] Add `ErrorGuaranteed` propagation to all phase transitions. `parsed()` returns `Result<ParseOutput, ErrorGuaranteed>`. `typed()` returns `Result<TypeCheckResult, ErrorGuaranteed>`. The guarantees chain: a `typed()` error proves a diagnostic was emitted, so `evaluated()` can early-return without re-reporting.

### Phase 3: Polish

- [ ] Implement module interface serialization: emit `.ori-interface` files containing type signatures, trait definitions, and constant values. Use a compact binary format (consider Cap'n Proto or a custom format aligned with `Pool` internals). Dependents type-check against interfaces without re-parsing source.
- [ ] Add capability-aware Salsa durability: functions annotated `uses Pure` (or no capabilities) get `Durability::HIGH`, functions with side-effect capabilities get `Durability::LOW`. This allows Salsa to skip revalidation of pure computations when unrelated files change.
- [ ] Implement persistent `ArcIrCache`: serialize `Optimized<ArcFunction>` to disk, keyed by `FunctionContentHash`. On incremental rebuild, functions with unchanged content hashes skip both ARC analysis and codegen. Similar to Zig's object cache but keyed by content hash rather than source hash.
- [ ] Add incremental test execution: use `TestDef.targets` to build a test-to-function dependency graph. When a function's type result changes, only re-run tests that target it. Emit "N tests skipped (unchanged)" alongside "N tests passed".

## References

**Rust**
- `~/projects/reference_repos/lang_repos/rust/compiler/rustc_driver_impl/src/lib.rs` -- Driver/library split, Callbacks trait
- `~/projects/reference_repos/lang_repos/rust/compiler/rustc_errors/src/lib.rs` -- DiagCtxt, ErrorGuaranteed pattern

**Zig**
- `~/projects/reference_repos/lang_repos/zig/src/Compilation.zig` -- Monolithic compilation state, Config struct, staged work queues
- `~/projects/reference_repos/lang_repos/zig/src/Compilation/Config.zig` -- Separated config type

**Go**
- `~/projects/reference_repos/lang_repos/golang/src/cmd/compile/internal/gc/main.go` -- Fixed-point compilation loop, concurrent SSA back-end
- `~/projects/reference_repos/lang_repos/golang/src/cmd/compile/internal/ssagen/pgen.go` -- Per-function parallel codegen

**Gleam**
- `~/projects/reference_repos/lang_repos/gleam/compiler-core/src/error.rs` -- Unified error types
- `~/projects/reference_repos/lang_repos/gleam/compiler-core/src/build/` -- ProjectCompiler, pure library architecture

**Lean 4**
- `~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/RC.lean` -- ExplicitRC pass, DerivedValInfo for per-variable ownership
- `~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/Borrow.lean` -- Fixed-point borrow inference with OwnedSet
- `~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/ExpandResetReuse.lean` -- CFG-wide reset/reuse detection

**Swift**
- `~/projects/reference_repos/lang_repos/swift/lib/SILOptimizer/ARC/ARCSequenceOpts.cpp` -- Dual-pass dataflow ARC optimization
- `~/projects/reference_repos/lang_repos/swift/lib/SILOptimizer/ARC/GlobalARCSequenceDataflow.cpp` -- Global ARC dataflow analysis

**Koka**
- `~/projects/reference_repos/lang_repos/koka/src/Core/Borrowed.hs` -- Borrow parameter tracking
- `~/projects/reference_repos/lang_repos/koka/src/Core/CheckFBIP.hs` -- FBIP (frame-bounded in-place) verification
