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
├── ori_lexer/                # Tokenization (logos-based)
│   └── src/lib.rs            # lex() function, token processing
├── ori_types/                # Type system definitions
│   └── src/
│       ├── lib.rs            # Module exports
│       ├── core.rs           # Type enum (external API)
│       ├── data.rs           # TypeData enum (internal representation)
│       ├── type_interner.rs  # TypeInterner, SharedTypeInterner
│       ├── context.rs        # InferenceContext (TypeId-based unification)
│       ├── env.rs            # TypeEnv for scoping
│       ├── traverse.rs       # TypeFolder, TypeVisitor
│       └── error.rs          # TypeError
├── ori_parse/                # Recursive descent parser
│   └── src/
│       ├── lib.rs            # Parser struct, parse() entry point
│       ├── error.rs          # Parse error types
│       ├── stack.rs          # Stack safety (stacker integration)
│       └── grammar/          # Grammar modules (expr, item, type, etc.)
├── ori_typeck/               # Type checking
│   └── src/
│       ├── lib.rs            # Module exports
│       ├── operators.rs      # Operator type rules
│       ├── checker/          # TypeChecker core
│       │   ├── mod.rs        # TypeChecker struct
│       │   ├── api.rs        # Public API functions
│       │   ├── orchestration.rs # check_module 4-pass logic
│       │   ├── builder.rs    # TypeCheckerBuilder
│       │   ├── components.rs # Component structs
│       │   └── ...           # Other checker modules
│       ├── infer/            # Type inference
│       │   ├── mod.rs        # Inference dispatcher
│       │   ├── expressions/  # Expression inference (8 modules)
│       │   ├── free_vars.rs  # Free variable collection
│       │   └── ...           # Other inference modules
│       ├── registry/         # Type and trait registries
│       │   ├── mod.rs        # TypeRegistry
│       │   ├── trait_registry.rs # TraitRegistry
│       │   └── ...           # Registry types
│       └── derives/          # Derive macro support
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
│       ├── environment.rs    # Environment, Scope, LocalScope
│       ├── errors.rs         # EvalError factories
│       ├── operators.rs      # Binary operator dispatch
│       ├── unary_operators.rs # Unary operator dispatch
│       ├── methods.rs        # Built-in method dispatch, EVAL_BUILTIN_METHODS constant
│       ├── function_val.rs   # Type conversion functions (int, float, str, byte)
│       ├── user_methods.rs   # UserMethodRegistry
│       ├── print_handler.rs  # Print output capture
│       ├── shared.rs         # SharedRegistry, SharedMutableRegistry
│       ├── stack.rs          # Stack safety (stacker)
│       ├── exec/             # Expression execution
│       │   ├── expr.rs       # Expression evaluation
│       │   ├── call.rs       # Function call evaluation
│       │   ├── control.rs    # Control flow (if, for, loop)
│       │   └── pattern.rs    # Pattern matching
│       └── interpreter/      # Core interpreter
│           ├── mod.rs        # Interpreter struct
│           ├── builder.rs    # InterpreterBuilder
│           ├── scope_guard.rs # RAII scope management
│           ├── function_call.rs # User function calls
│           ├── function_seq.rs  # run/try/match evaluation
│           ├── method_dispatch.rs # Method resolution
│           ├── derived_methods.rs # Derived trait methods
│           └── resolvers/    # Method resolution chain
│               ├── mod.rs    # MethodDispatcher, MethodResolver trait
│               ├── user_registry.rs  # User methods
│               ├── collection.rs     # List/range methods
│               └── builtin.rs        # Built-in methods
├── ori-macros/               # Proc-macro crate
│   └── src/
│       ├── lib.rs            # Diagnostic/Subdiagnostic derives
│       ├── diagnostic.rs     # #[derive(Diagnostic)] impl
│       └── subdiagnostic.rs  # #[derive(Subdiagnostic)] impl
├── ori_llvm/                 # LLVM backend (excluded from workspace)
│   └── src/
│       ├── lib.rs            # Module exports, LlvmBackend trait
│       ├── builder.rs        # CodeBuilder - main codegen orchestrator
│       ├── context.rs        # CompilationContext - LLVM context wrapper
│       ├── module.rs         # ModuleBuilder - LLVM module creation
│       ├── declare.rs        # Function/type declarations
│       ├── types.rs          # Type mapping (Ori types → LLVM types)
│       ├── operators.rs      # Binary/unary operator codegen
│       ├── control_flow.rs   # If/loop/for codegen
│       ├── matching.rs       # Pattern match codegen
│       ├── runtime.rs        # Runtime function declarations
│       ├── evaluator.rs      # LlvmEvaluator - JIT execution
│       ├── traits.rs         # Backend trait definitions
│       ├── functions/        # Function codegen
│       │   ├── mod.rs        # Function compilation entry
│       │   ├── body.rs       # Function body codegen
│       │   ├── calls.rs      # Function call codegen
│       │   ├── builtins.rs   # Built-in function codegen
│       │   ├── lambdas.rs    # Lambda/closure codegen
│       │   ├── sequences.rs  # run/try/match codegen
│       │   ├── expressions.rs # Expression codegen
│       │   ├── helpers.rs    # Codegen utilities
│       │   └── phi.rs        # PHI node helpers
│       ├── collections/      # Collection type codegen
│       │   ├── mod.rs        # Collection utilities
│       │   ├── lists.rs      # List operations
│       │   ├── maps.rs       # Map operations
│       │   ├── strings.rs    # String operations
│       │   ├── tuples.rs     # Tuple operations
│       │   ├── structs.rs    # Struct operations
│       │   ├── ranges.rs     # Range operations
│       │   ├── wrappers.rs   # Option/Result wrappers
│       │   └── indexing.rs   # Index operations
│       └── tests/            # Comprehensive test suite
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
        ├── typeck/           # Type checking and inference
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
ori_ir (base)
    ├── ori_diagnostic
    ├── ori_lexer
    ├── ori_types
    ├── ori_parse
    ├── ori_typeck ──→ ori_types, ori_parse
    └── ori_patterns ──→ ori_types
            │
            └── ori_eval ──→ ori_patterns
                    │
                    └── oric ──→ ALL (orchestrator)

ori_llvm (separate, excluded from workspace)
    └── depends on: ori_ir, ori_types, ori_parse, ori_patterns, ori_typeck
```

**Layered architecture:**
- `ori_ir`: Core IR types (no dependencies)
- `ori_patterns`: Pattern definitions, Value types, EvalError (single source of truth)
- `ori_eval`: Core tree-walking interpreter (Interpreter, Environment, exec, method dispatch)
- `oric`: CLI orchestrator with Salsa queries, type checker, high-level Evaluator wrapper
- `ori_llvm`: LLVM backend for native code generation (excluded from main workspace to avoid LLVM linking overhead)

Pure functions live in library crates; Salsa queries live in `oric`.

## Design Principles

### Salsa-First Architecture

Every major computation is a Salsa query. This provides:

- **Automatic caching** - Query results are memoized
- **Dependency tracking** - Salsa knows what depends on what
- **Early cutoff** - If output unchanged, dependents skip recomputation

```rust
#[salsa::tracked]
pub fn tokens(db: &dyn Db, file: SourceFile) -> TokenList { ... }

#[salsa::tracked]
pub fn parsed(db: &dyn Db, file: SourceFile) -> ParseResult { ... }

#[salsa::tracked]
pub fn typed(db: &dyn Db, file: SourceFile) -> TypedModule { ... }
```

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
pub struct PatternRegistry {
    patterns: HashMap<Name, Box<dyn PatternDefinition>>,
}

impl PatternRegistry {
    pub fn register(&mut self, name: &str, pattern: impl PatternDefinition) { ... }
    pub fn get(&self, name: Name) -> Option<&dyn PatternDefinition> { ... }
}
```

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
| `TypeId` | `ori_ir` | Interned type identifier (sharded: 4-bit shard + 28-bit local) |
| `Type` | `ori_types` | External type representation (uses Box) |
| `TypeData` | `ori_types` | Internal type representation (uses TypeId) |
| `TypeInterner` | `ori_types` | Sharded type interning for O(1) equality |
| `Value` | `ori_patterns` | Runtime values (re-exported via `ori_eval`) |
| `Interpreter` | `ori_eval` | Core tree-walking interpreter |
| `Environment` | `ori_eval` | Variable scoping (scope stack) |
| `Evaluator` | `oric` | High-level evaluator (module loading, prelude) |
| `CodeBuilder` | `ori_llvm` | LLVM codegen orchestrator |
| `LlvmEvaluator` | `ori_llvm` | JIT execution via LLVM |
| `Diagnostic` | `ori_diagnostic` | Rich error with suggestions |
| `ErrorGuaranteed` | `ori_diagnostic` | Proof that an error was emitted |
| `Applicability` | `ori_diagnostic` | Fix confidence level |
| `ParseResult` | `ori_parse` | Parser output (module + arena + errors) |

## Crate Organization

| Crate | Purpose |
|-------|---------|
| `ori_ir` | Core IR types: tokens, spans, AST, arena, string interning, TypeId |
| `ori_diagnostic` | Error reporting (split: error_code, diagnostic, guarantee), DiagnosticQueue, emitters, error docs |
| `ori_lexer` | Tokenization via logos |
| `ori_types` | Type system: Type/TypeData, TypeInterner, InferenceContext, TypeIdFolder |
| `ori_parse` | Recursive descent parser |
| `ori_typeck` | Type checking: TypeChecker, inference, registries |
| `ori_patterns` | Pattern definitions, Value types, EvalError (single source of truth) |
| `ori_eval` | Core tree-walking interpreter: Interpreter, Environment, exec, method dispatch |
| `ori-macros` | Proc-macros (`#[derive(Diagnostic)]`, etc.) |
| `ori_llvm` | LLVM backend: CodeBuilder, JIT execution, native codegen (excluded from workspace) |
| `oric` | CLI orchestrator, Salsa queries, high-level Evaluator, patterns |

### DRY Re-exports

To avoid code duplication, `oric` re-exports from source crates rather than maintaining duplicate definitions:

| oric Module | Re-exports From |
|---------------|-----------------|
| `oric::ir` | `ori_ir` |
| `oric::parser` | `ori_parse` |
| `oric::diagnostic` | `ori_diagnostic` |
| `oric::types` | `ori_types` |

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

The `ori_llvm` crate provides native code generation via LLVM 17. It is **excluded from the main workspace** to avoid LLVM linking overhead during normal development.

**Key components:**
- `CodeBuilder`: Main codegen orchestrator, walks the typed AST
- `CompilationContext`: Wraps LLVM context, module, and builder
- `LlvmEvaluator`: JIT execution for running compiled code

**Development workflow:**
- Unit tests require Docker (LLVM environment): `./llvm-test`
- Build/clippy/format run directly: `./llvm-build`, `./llvm-clippy`, `cargo fmt --manifest-path compiler/ori_llvm/Cargo.toml`

See `.claude/rules/llvm.md` for development guidelines.

## Related Documents

- [Compilation Pipeline](pipeline.md) - Detailed pipeline description
- [Salsa Integration](salsa-integration.md) - How Salsa is used
- [Data Flow](data-flow.md) - Data movement through phases
