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
├── ori_ir/           # Core IR types (tokens, spans, AST, interning)
│   └── src/
│       ├── lib.rs          # Module organization, static_assert_size! macro
│       ├── ast/            # Expression and statement types
│       ├── token.rs        # Token definitions
│       ├── span.rs         # Source location tracking
│       ├── arena.rs        # Expression arena allocation
│       ├── interner.rs     # String interning
│       └── visitor.rs      # AST visitor pattern
├── ori_diagnostic/   # Error reporting
│   └── src/
│       ├── lib.rs          # Diagnostic, Applicability, ErrorCode, ErrorGuaranteed
│       ├── queue.rs        # DiagnosticQueue (deduplication, limits, emit_error)
│       ├── errors/         # Embedded error documentation for --explain
│       ├── emitter/        # Output formatting (terminal, JSON, SARIF)
│       └── fixes/          # Code suggestions and fixes
├── ori_lexer/        # Tokenization (logos-based)
│   └── src/lib.rs          # lex() function, token processing
├── ori_types/        # Type system definitions
│   └── src/
│       ├── lib.rs          # Module exports
│       ├── core.rs         # Type enum (external API)
│       ├── data.rs         # TypeData enum (internal representation)
│       ├── type_interner.rs # TypeInterner, SharedTypeInterner
│       ├── context.rs      # InferenceContext (TypeId-based unification)
│       ├── env.rs          # TypeEnv for scoping
│       ├── traverse.rs     # TypeFolder, TypeVisitor, TypeIdFolder, TypeIdVisitor
│       └── error.rs        # TypeError
├── ori_parse/        # Recursive descent parser
│   └── src/
│       ├── lib.rs          # Parser struct, parse() entry point
│       ├── error.rs        # Parse error types
│       ├── stack.rs        # Stack safety (stacker integration)
│       └── grammar/        # Grammar modules (expr, item, type, etc.)
├── ori_patterns/     # Pattern system, Value types
│   └── src/
│       ├── lib.rs          # PatternDefinition, TypeCheckContext, EvalContext
│       ├── registry.rs     # PatternRegistry, SharedPattern
│       ├── value/          # Value types, Heap, FunctionValue
│       ├── errors.rs       # EvalError, EvalResult, error constructors
│       └── *.rs            # Pattern implementations (recurse, parallel, etc.)
├── ori_eval/         # Core evaluator components
│   └── src/
│       ├── lib.rs          # Re-exports
│       ├── environment.rs  # Environment, Scope, LocalScope
│       └── operators.rs    # BinaryOperator, OperatorRegistry
├── ori-macros/       # Proc-macro crate
│   └── src/
│       ├── lib.rs          # Diagnostic/Subdiagnostic derives
│       ├── diagnostic.rs   # #[derive(Diagnostic)] impl
│       └── subdiagnostic.rs # #[derive(Subdiagnostic)] impl
└── oric/             # CLI orchestrator + Salsa queries
    └── src/
        ├── lib.rs          # Module organization
        ├── main.rs         # CLI entry point
        ├── db.rs           # Salsa database definition
        ├── query/          # Salsa query definitions
        ├── typeck/         # Type checking and inference
        ├── eval/           # Tree-walking interpreter (uses ori_eval)
        ├── test/           # Test runner
        └── debug.rs        # Debug flags
```

### Crate Dependencies

```
ori_ir (base)
    ├── ori_diagnostic
    ├── ori_lexer
    ├── ori_types
    ├── ori_parse
    └── ori_patterns ──→ ori_types
            │
            └── ori_eval ──→ ori_patterns
                    │
                    └── oric ──→ ALL (orchestrator)
```

**Layered architecture:**
- `ori_ir`: Core IR types (no dependencies)
- `ori_patterns`: Pattern definitions, Value types, EvalError (single source of truth)
- `ori_eval`: Core evaluator components (Environment, operators)
- `oric`: CLI orchestrator with Salsa queries, type checker, evaluator

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
| `Value` | `oric` | Runtime values |
| `Diagnostic` | `ori_diagnostic` | Rich error with suggestions |
| `ErrorGuaranteed` | `ori_diagnostic` | Proof that an error was emitted |
| `Applicability` | `ori_diagnostic` | Fix confidence level |
| `ParseResult` | `ori_parse` | Parser output (module + arena + errors) |

## Crate Organization

| Crate | Purpose |
|-------|---------|
| `ori_ir` | Core IR types: tokens, spans, AST, arena, string interning, TypeId |
| `ori_diagnostic` | Error reporting, DiagnosticQueue, ErrorGuaranteed, emitters, error docs |
| `ori_lexer` | Tokenization via logos |
| `ori_types` | Type system: Type/TypeData, TypeInterner, InferenceContext, TypeIdFolder |
| `ori_parse` | Recursive descent parser |
| `ori-macros` | Proc-macros (`#[derive(Diagnostic)]`, etc.) |
| `oric` | CLI orchestrator, Salsa queries, typeck, eval, patterns |

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
- `types.rs` -> `typeck/infer/expr.rs`, `typeck/infer/call.rs`, etc.

## Related Documents

- [Compilation Pipeline](pipeline.md) - Detailed pipeline description
- [Salsa Integration](salsa-integration.md) - How Salsa is used
- [Data Flow](data-flow.md) - Data movement through phases
