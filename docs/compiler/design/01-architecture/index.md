---
title: "Architecture Overview"
description: "Ori Compiler Design — Architecture Overview"
order: 100
section: "Architecture"
---

# Architecture Overview

The Ori compiler (`oric`) is an incremental compiler built on Salsa, a framework for on-demand, incremental computation. The architecture prioritizes:

1. **Incrementality** - Only recompute what changes
2. **Memory efficiency** - Arena allocation, string interning
3. **Extensibility** - Registry-based patterns and diagnostics
4. **Testability** - Dependency injection via SharedRegistry

## High-Level Structure

The compiler is organized as a Cargo workspace with multiple crates:

```
compiler/
├── ori_ir/                   # Core IR types (tokens, spans, AST, interning)
│   └── src/
│       ├── lib.rs            # Module organization, static_assert_size! macro
│       ├── ast/              # Expression and statement types
│       ├── token.rs          # Token definitions
│       ├── span.rs           # Source location tracking
│       ├── arena.rs          # Expression arena allocation
│       ├── interner.rs       # String interning
│       └── visitor.rs        # AST visitor pattern
├── ori_diagnostic/           # Error reporting
│   └── src/
│       ├── lib.rs            # Module organization and re-exports
│       ├── error_code.rs     # ErrorCode enum, as_str(), Display
│       ├── diagnostic.rs     # Diagnostic, Label, Severity, Applicability, Suggestion
│       ├── guarantee.rs      # ErrorGuaranteed type-level proof
│       ├── queue.rs          # DiagnosticQueue (deduplication, limits)
│       ├── span_utils.rs     # Line/column computation from spans
│       ├── errors/           # Embedded error documentation
│       ├── emitter/          # Output formatting (terminal, JSON, SARIF)
│       │   ├── mod.rs        # Emitter trait, trailing_comma() helper
│       │   ├── terminal.rs   # Terminal output (colored)
│       │   ├── json.rs       # JSON output
│       │   └── sarif.rs      # SARIF format (BTreeSet for rule dedup)
│       └── fixes/            # Code suggestions and fixes
├── ori_lexer_core/            # Low-level scanner (raw tokenization)
│   └── src/
│       ├── lib.rs            # Module exports
│       ├── raw_scanner.rs    # RawScanner, byte-level tokenization
│       ├── source_buffer.rs  # SourceBuffer, cursor management
│       ├── cursor.rs         # Cursor utilities
│       └── tag.rs            # RawTag definitions
├── ori_lexer/                # Tokenization (logos-based, wraps ori_lexer_core)
│   └── src/lib.rs            # lex() function, token processing
├── ori_parse/                # Recursive descent parser
│   └── src/
│       ├── lib.rs            # Parser struct, parse() entry point
│       ├── error.rs          # Parse error types
│       └── grammar/          # Grammar modules (expr, item, type, etc.)
├── ori_types/                # Type system + type checking (Pool, InferEngine, registries)
│   └── src/
│       ├── lib.rs            # Module exports
│       ├── check/            # Type checker core
│       │   ├── mod.rs        # Type checker orchestration
│       │   ├── api.rs        # Public API
│       │   ├── bodies.rs     # Function body checking
│       │   ├── signatures.rs # Signature checking
│       │   ├── registration.rs # Type/trait registration
│       │   └── integration_tests.rs # Checker tests
│       ├── output/           # Type checker output types
│       └── ...               # Pool, InferEngine, registries, unification
├── ori_patterns/             # Pattern system, Value types
│   └── src/
│       ├── lib.rs            # PatternDefinition, TypeCheckContext
│       ├── registry.rs       # PatternRegistry, SharedPattern
│       ├── value/            # Value types, Heap, FunctionValue
│       ├── errors.rs         # EvalError, EvalResult
│       └── *.rs              # Pattern implementations
├── ori_eval/                 # Core interpreter (tree-walking evaluator)
│   └── src/
│       ├── lib.rs            # Module exports, re-exports from ori_patterns
│       ├── environment/      # Environment, Scope, LocalScope
│       ├── errors.rs         # EvalError factories
│       ├── operators.rs      # Binary operator dispatch
│       ├── unary_operators.rs # Unary operator dispatch
│       ├── methods/          # Built-in method dispatch, EVAL_BUILTIN_METHODS constant
│       ├── function_val.rs   # Type conversion functions (int, float, str, byte)
│       ├── method_key.rs     # Method dispatch key types
│       ├── user_methods.rs   # UserMethodRegistry
│       ├── print_handler/    # Print output capture
│       ├── shared.rs         # SharedRegistry, SharedMutableRegistry
│       ├── derives/          # Derived trait method evaluation
│       ├── diagnostics/      # Evaluator diagnostic utilities
│       ├── eval_mode/        # Evaluation mode configuration
│       ├── module_registration/ # Module-level registration logic
│       ├── exec/             # Expression execution
│       │   ├── expr.rs       # Expression evaluation
│       │   ├── call/         # Function call evaluation
│       │   ├── control.rs    # Control flow (if, for, loop)
│       │   └── decision_tree/ # Decision tree evaluation (compiled patterns)
│       └── interpreter/      # Core interpreter
│           ├── mod.rs        # Interpreter struct
│           ├── builder.rs    # InterpreterBuilder
│           ├── can_eval.rs   # Canonical IR evaluation dispatch
│           ├── scope_guard/  # RAII scope management
│           ├── function_call.rs # User function calls
│           ├── format.rs     # Value formatting utilities (+ format/ subdir)
│           ├── interned_names.rs # Pre-interned name constants
│           ├── derived_methods.rs # Derived trait methods
│           ├── method_dispatch/ # Method resolution
│           │   ├── mod.rs    # Dispatch orchestration
│           │   └── iterator/ # Iterator method dispatch
│           └── resolvers/    # Method resolution chain
│               ├── mod.rs    # MethodDispatcher, MethodResolver trait
│               ├── user_registry.rs  # User methods
│               ├── collection.rs     # List/range methods
│               └── builtin.rs        # Built-in methods
├── ori_canon/                # Canonical IR lowering (AST → sugar-free IR)
│   └── src/
│       ├── lib.rs            # Module exports, public API
│       ├── lower.rs          # lower_module() entry point, Lowerer
│       ├── desugar.rs        # Named calls → positional, templates → concat
│       ├── patterns.rs       # Pattern compilation (Maranget 2008 → decision trees)
│       ├── const_fold.rs     # Compile-time constant folding
│       └── validate.rs       # Post-lowering validation
├── ori_arc/                  # ARC analysis (reference counting optimization)
│   └── src/
│       ├── lib.rs            # Module exports
│       ├── classify.rs       # Type classification (owned vs borrowed)
│       ├── borrow.rs         # Borrow inference
│       ├── rc_insert.rs      # Reference count insertion
│       ├── rc_elim.rs        # Redundant RC elimination
│       ├── reset_reuse.rs    # Reset/reuse optimization
│       ├── expand_reuse.rs   # Reuse expansion
│       ├── liveness.rs       # Liveness analysis
│       ├── ownership.rs      # Ownership tracking
│       ├── graph.rs          # Control flow graph
│       ├── ir.rs             # ARC IR types
│       ├── drop.rs           # Drop insertion
│       └── lower/            # ARC lowering passes
│           ├── mod.rs        # Lowering orchestration
│           ├── expr.rs       # Expression lowering
│           ├── control_flow.rs # Control flow lowering
│           ├── patterns.rs   # Pattern lowering
│           ├── calls.rs      # Call lowering
│           └── collections.rs # Collection lowering
├── ori_fmt/                  # Source code formatter (5-layer architecture)
│   └── src/
│       ├── lib.rs            # Public API, tabs_to_spaces()
│       ├── spacing/          # Layer 1: Token spacing (O(1) lookup)
│       ├── packing/          # Layer 2: Container packing decisions
│       ├── shape/            # Layer 3: Width tracking
│       ├── rules/            # Layer 4: Breaking rules (8 rules)
│       ├── formatter/        # Layer 5: Orchestration
│       ├── width/            # Width calculation
│       ├── declarations/     # Module-level formatting
│       ├── comments/         # Comment preservation
│       └── ...               # Other formatting modules
├── ori_stack/                # Stack safety utilities
│   └── src/
│       └── lib.rs            # grow_stack(), stack checks
├── ori_rt/                   # Runtime library (for AOT compilation)
│   └── src/
│       └── lib.rs            # Runtime support functions
├── ori_llvm/                 # LLVM backend (native code generation)
│   └── src/
│       ├── lib.rs            # Module exports
│       ├── context.rs        # SimpleCx - LLVM context wrapper
│       ├── evaluator.rs      # LlvmEvaluator - JIT execution
│       ├── runtime.rs        # Runtime support
│       ├── codegen/          # Code generation
│       │   ├── mod.rs        # Codegen orchestration
│       │   ├── ir_builder.rs # IrBuilder - main LLVM IR construction
│       │   ├── function_compiler.rs # Function compilation
│       │   ├── expr_lowerer.rs # Expression lowering
│       │   ├── arc_emitter.rs # ARC operation emission
│       │   ├── type_registration.rs # Type mapping (Ori → LLVM)
│       │   ├── type_info.rs  # Type layout information
│       │   ├── runtime_decl.rs # Runtime function declarations
│       │   ├── lower_*.rs    # Lowering: calls, collections, constructs, control flow, etc.
│       │   ├── scope.rs      # Scope management
│       │   ├── abi.rs        # ABI conventions
│       │   └── value_id.rs   # Value tracking
│       ├── aot/              # Ahead-of-time compilation (linker, mangling)
│       └── tests/            # Test suite
└── oric/                     # CLI orchestrator + Salsa queries
    └── src/
        ├── lib.rs            # Module organization
        ├── main.rs           # CLI dispatcher (thin: delegates to commands/)
        ├── commands/         # Command handlers (extracted from main.rs)
        │   ├── mod.rs        # Re-exports all command functions
        │   ├── run.rs        # run_file()
        │   ├── test.rs       # run_tests()
        │   ├── check.rs      # check_file()
        │   ├── compile.rs    # compile_file()
        │   ├── explain.rs    # explain_error(), parse_error_code()
        │   └── debug.rs      # parse_file(), lex_file()
        ├── db.rs             # Salsa database definition
        ├── query/            # Salsa query definitions
        ├── typeck.rs         # Type checking orchestration (delegates to ori_types)
        ├── eval/             # High-level evaluator (wraps ori_eval)
        │   ├── mod.rs        # Re-exports, value module
        │   ├── output.rs     # EvalOutput, ModuleEvalResult
        │   ├── evaluator/    # Evaluator wrapper
        │   │   ├── mod.rs    # Evaluator struct
        │   │   ├── builder.rs # EvaluatorBuilder
        │   │   └── module_loading.rs # Module loading, prelude
        │   └── module/       # Import resolution
        │       └── import.rs # Module import handling
        ├── test/             # Test runner
        └── debug.rs          # Debug flags
```

### Crate Dependencies

```
ori_ir (base — no compiler crate dependencies)
    ├── ori_diagnostic ──→ ori_ir
    ├── ori_lexer_core → ori_lexer ──→ ori_ir
    ├── ori_parse ──→ ori_ir, ori_lexer
    ├── ori_types ──→ ori_ir, ori_diagnostic
    ├── ori_patterns ──→ ori_ir (NOT ori_types — values are type-agnostic)
    ├── ori_eval ──→ ori_ir, ori_patterns (NOT ori_types — evaluator is untyped)
    ├── ori_arc ──→ ori_ir, ori_types
    ├── ori_canon ──→ ori_ir, ori_types, ori_arc (NOT ori_patterns)
    └── oric ──→ ALL (orchestrator)

ori_llvm (excluded from main workspace — build with `cargo bl`/`cargo blr`)
    └── depends on: ori_ir, ori_types, ori_parse, ori_patterns, ori_arc, ori_rt
```

**Key dependency invariants:**
- `ori_patterns` depends only on `ori_ir`, not `ori_types` — the Value system and pattern evaluation are type-agnostic
- `ori_eval` depends on `ori_patterns` (for Value types and pattern dispatch), not directly on `ori_types`
- `ori_canon` depends on `ori_arc` (for decision tree types), not on `ori_patterns`

**Layered architecture:**
- `ori_ir`: Core IR types, canonical IR definitions (no compiler crate dependencies)
- `ori_lexer_core`: Low-level byte scanner, raw tokenization
- `ori_lexer`: Token cooking, string interning (wraps `ori_lexer_core`)
- `ori_types`: Type system, type checking (Pool, InferEngine, registries)
- `ori_patterns`: Pattern definitions, Value types, EvalError (single source of truth)
- `ori_eval`: Core tree-walking interpreter (Interpreter, Environment, exec, method dispatch)
- `ori_canon`: Canonical IR lowering (desugaring, pattern compilation, constant folding)
- `ori_arc`: ARC analysis (type classification, borrow inference, RC insertion)
- `ori_fmt`: Source code formatter (AST pretty-printing)
- `ori_stack`: Stack safety utilities (stacker integration)
- `ori_rt`: Runtime library for AOT-compiled binaries
- `oric`: CLI orchestrator with Salsa queries, type checker, high-level Evaluator wrapper
- `ori_llvm`: LLVM backend for native code generation (excluded from main workspace)

Pure functions live in library crates; Salsa queries live in `oric`.

## Design Principles

### Salsa-First Architecture

Every major computation is a Salsa query. This provides:

- **Automatic caching** - Query results are memoized
- **Dependency tracking** - Salsa knows what depends on what
- **Early cutoff** - If output unchanged, dependents skip recomputation

```rust
// Primary pipeline queries:
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList { ... }

#[salsa::tracked]
pub fn parsed(db: &dyn Db, file: SourceFile) -> ParseResult { ... }

#[salsa::tracked]
pub fn typed(db: &dyn Db, file: SourceFile) -> TypedModule { ... }

#[salsa::tracked]
pub fn evaluated(db: &dyn Db, file: SourceFile) -> EvalResult { ... }
```

These four are the main pipeline, but intermediate queries exist at each stage. For example, lexing has a chain of queries — `tokens_with_metadata()` (full lex output with comments), `lex_result()` (tokens + errors), `tokens()` (just token list), `lex_errors()` (just errors) — plus utility queries like `line_count()`, `non_empty_line_count()`, and `first_line()`. See [Compilation Pipeline](pipeline.md) for the full query graph.

### Query Characteristics

| Query | Input | Output | Caching |
|-------|-------|--------|---------|
| `tokens` | `SourceFile` | `TokenList` | High reuse (syntax changes rare) |
| `parsed` | `SourceFile` | `ParseResult` | Moderate (structure changes) |
| `typed` | `SourceFile` | `TypedModule` | Type info changes with signatures |
| `evaluated` | `SourceFile` | `EvalResult` | Re-run on any code change |

**Early Cutoff**: If a query's output is identical to its cached result, Salsa skips recomputation of all dependent queries. For example, whitespace-only changes to a file may produce identical tokens, avoiding re-parsing.

**Session-scoped side-caches**: Some data cannot satisfy Salsa's `Clone + Eq + Hash` requirements (e.g., the type `Pool`, which contains interned types and inference state). These are stored in session-scoped caches on the Salsa database:

- **`PoolCache`** — Caches `Arc<Pool>` per file path, keyed by `PathBuf`. Populated by `typed()`, consumed by canonicalization and error rendering.
- **`CanonCache`** — Caches `SharedCanonResult` per file path. Populated by `canonicalize_cached()`, consumed by `evaluated()`, the test runner, and the `check` command.
- **`ImportsCache`** — Caches `Arc<ResolvedImports>` per file path. Populated during module loading, reused across evaluation of imported modules.

All three are `Arc<RwLock<HashMap<PathBuf, _>>>` and live on the `CompilerDb`.

### Flat Data Structures

The AST uses arena allocation instead of `Box<T>`:

```rust
// Instead of this:
struct Expr {
    kind: ExprKind,
    children: Vec<Box<Expr>>,
}

// We use this:
struct Expr {
    kind: ExprKind,
    span: Span,
}

struct ExprArena {
    exprs: Vec<Expr>,  // Indexed by ExprId(u32)
}
```

Benefits:
- Better cache locality
- Simpler memory management
- Efficient serialization for Salsa

### String Interning

All identifiers are interned:

```rust
// Name is just a u32 index
let name1: Name = interner.intern("foo");
let name2: Name = interner.intern("foo");
assert_eq!(name1, name2);  // O(1) comparison
```

### Registry Pattern

Patterns and diagnostics use registries for extensibility:

```rust
pub enum Pattern {
    Recurse(RecursePattern),
    Parallel(ParallelPattern),
    Spawn(SpawnPattern),
    // ... one variant per pattern kind
}

impl PatternDefinition for Pattern {
    fn name(&self) -> &'static str {
        match self {
            Pattern::Recurse(p) => p.name(),
            Pattern::Parallel(p) => p.name(),
            // ... delegates to inner pattern
        }
    }
}

pub struct PatternRegistry {
    _private: (),  // Marker to prevent external construction
}

impl PatternRegistry {
    /// Get the pattern definition for a given kind.
    /// Returns a `Pattern` enum for static dispatch.
    pub fn get(&self, kind: FunctionExpKind) -> Pattern {
        match kind {
            FunctionExpKind::Recurse => Pattern::Recurse(RecursePattern),
            FunctionExpKind::Parallel => Pattern::Parallel(ParallelPattern),
            // ... direct enum construction
        }
    }
}
```

All patterns are zero-sized types (ZSTs) wrapped in a `Pattern` enum, providing:
- Zero heap allocation overhead (enum is 1 byte)
- Static dispatch via enum (no trait objects, no `dyn`)
- Direct dispatch (no HashMap lookup)
- `Copy` + `Send` + `Sync`

## Key Types

| Type | Crate | Purpose |
|------|-------|---------|
| `SourceFile` | `oric` | Salsa input - source text |
| `TokenList` | `ori_ir` | Lexer output |
| `Token` | `ori_ir` | Individual token with kind and span |
| `Span` | `ori_ir` | Source location (start/end offsets) |
| `Module` | `ori_ir` | Parsed module structure |
| `ExprArena` | `ori_ir` | Expression storage |
| `ExprId` | `ori_ir` | Index into ExprArena |
| `Name` | `ori_ir` | Interned string identifier |
| `TypeId` | `ori_ir` | Interned type identifier (flat u32 index) |
| `Idx` | `ori_types` | Universal type handle (u32 index into Pool) |
| `Tag` | `ori_types` | Type kind discriminant (u8) for tag-driven dispatch |
| `Pool` | `ori_types` | Unified type storage (items + extra + flags + hashes) |
| `TypeFlags` | `ori_types` | Pre-computed type metadata (bitflags) |
| `Value` | `ori_patterns` | Runtime values (re-exported via `ori_eval`) |
| `Interpreter` | `ori_eval` | Core tree-walking interpreter |
| `Environment` | `ori_eval` | Variable scoping (scope stack) |
| `Evaluator` | `oric` | High-level evaluator (module loading, prelude) |
| `CanonResult` | `ori_ir` | Canonical IR output (CanArena, DecisionTrees, ConstantPool) |
| `SharedCanonResult` | `ori_ir` | Arc-wrapped CanonResult for cross-query sharing |
| `IrBuilder` | `ori_llvm` | LLVM IR construction (codegen orchestrator) |
| `SimpleCx` | `ori_llvm` | LLVM context wrapper (module, builder, target) |
| `LlvmEvaluator` | `ori_llvm` | JIT execution via LLVM |
| `Diagnostic` | `ori_diagnostic` | Rich error with suggestions |
| `ErrorGuaranteed` | `ori_diagnostic` | Proof that an error was emitted |
| `Applicability` | `ori_diagnostic` | Fix confidence level |
| `ParseResult` | `ori_parse` | Parser output (module + arena + errors) |

## Crate Organization

| Crate | Purpose |
|-------|---------|
| `ori_ir` | Core IR types: tokens, spans, AST, arena, string interning, TypeId, canonical IR (CanonResult) |
| `ori_diagnostic` | Error reporting (split: error_code, diagnostic, guarantee), DiagnosticQueue, emitters, error docs |
| `ori_lexer_core` | Low-level scanner: RawScanner, byte-level tokenization, SourceBuffer |
| `ori_lexer` | Tokenization via logos (wraps ori_lexer_core), token cooking |
| `ori_types` | Type system + type checking: Pool, InferEngine, registries, unification, check/bodies |
| `ori_parse` | Recursive descent parser |
| `ori_patterns` | Pattern definitions, Value types, EvalError (single source of truth) |
| `ori_eval` | Core tree-walking interpreter: Interpreter, Environment, exec, method dispatch |
| `ori_canon` | Canonical IR lowering: desugaring, pattern compilation (decision trees), constant folding |
| `ori_arc` | ARC analysis: type classification, borrow inference, RC insertion/elimination, reset/reuse |
| `ori_fmt` | Source code formatter: 5-layer architecture (spacing, packing, shape, rules, orchestration) |
| `ori_stack` | Stack safety utilities: stacker integration for deep recursion |
| `ori_rt` | Runtime library: support functions for AOT-compiled binaries |
| `ori_llvm` | LLVM backend: IrBuilder, SimpleCx, JIT execution, native codegen |
| `oric` | CLI orchestrator, Salsa queries, high-level Evaluator, patterns |

### DRY Re-exports

To avoid code duplication, `oric` re-exports from source crates rather than maintaining duplicate definitions:

| oric Module | Re-exports From |
|---------------|-----------------|
| `oric::ir` | `ori_ir` |
| `oric::parser` | `ori_parse` |
| `oric::diagnostic` | `ori_diagnostic` |

Note: `ori_types` is accessed via `oric::typeck`, which orchestrates type checking by delegating to `ori_types`. There is no separate `oric::types` re-export module.

This pattern ensures:
- Single source of truth for each type
- Consistent behavior across the codebase
- Easier maintenance and refactoring

## File Size Guidelines

To maintain code quality, files follow size limits:

- **Target**: ~500 lines per file
- **Maximum**: 800 lines per file
- **Exception**: Grammar files may be larger due to many variants

When files exceed limits, extract submodules:
- `evaluator.rs` -> `eval/exec/expr.rs`, `eval/exec/call.rs`, etc.
- `infer/expr.rs` -> `infer/expressions/` subdirectory with focused modules
- `checker/mod.rs` -> `checker/api.rs`, `checker/orchestration.rs`, `checker/utilities.rs`
- `registry/trait_registry.rs` -> `registry/trait_types.rs`, `registry/impl_types.rs`, etc.

## LLVM Backend

The `ori_llvm` crate provides native code generation via LLVM 17. It is **excluded from the main workspace** (along with `ori_rt`) because it requires LLVM 17 to be installed. Build both with `cargo bl` (debug) or `cargo blr` (release).

**Key components:**
- `IrBuilder`: Main LLVM IR construction, codegen orchestrator
- `SimpleCx`: Wraps LLVM context, module, builder, and target info
- `LlvmEvaluator`: JIT execution for running compiled code

**Development workflow:**
- Unit tests require Docker (LLVM environment): `./llvm-test.sh`
- Build/clippy/format run directly: `./llvm-build.sh`, `./llvm-clippy.sh`, `cargo fmt --manifest-path compiler/ori_llvm/Cargo.toml`

See `.claude/rules/llvm.md` for development guidelines.

## Related Documents

- [Compilation Pipeline](pipeline.md) - Detailed pipeline description
- [Salsa Integration](salsa-integration.md) - How Salsa is used
- [Data Flow](data-flow.md) - Data movement through phases
