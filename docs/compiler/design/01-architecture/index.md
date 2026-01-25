# Architecture Overview

The Sigil compiler (`sigilc`) is an incremental compiler built on Salsa, a framework for on-demand, incremental computation. The architecture prioritizes:

1. **Incrementality** - Only recompute what changes
2. **Memory efficiency** - Arena allocation, string interning
3. **Extensibility** - Registry-based patterns and diagnostics
4. **Testability** - Dependency injection via SharedRegistry

## High-Level Structure

The compiler is organized as a Cargo workspace with multiple crates:

```
compiler/
├── sigil_ir/           # Core IR types (tokens, spans, AST, interning)
│   └── src/
│       ├── lib.rs          # Module organization, static_assert_size! macro
│       ├── ast/            # Expression and statement types
│       ├── token.rs        # Token definitions
│       ├── span.rs         # Source location tracking
│       ├── arena.rs        # Expression arena allocation
│       ├── interner.rs     # String interning
│       └── visitor.rs      # AST visitor pattern
├── sigil_diagnostic/   # Error reporting
│   └── src/
│       ├── lib.rs          # Diagnostic, Applicability, ErrorCode
│       ├── emitter/        # Output formatting (terminal, JSON)
│       └── fixes/          # Code suggestions and fixes
├── sigil_lexer/        # Tokenization (logos-based)
│   └── src/lib.rs          # lex() function, token processing
├── sigil_types/        # Type system definitions
│   └── src/lib.rs          # Type enum, TypeError
├── sigil_parse/        # Recursive descent parser
│   └── src/
│       ├── lib.rs          # Parser struct, parse() entry point
│       ├── error.rs        # Parse error types
│       ├── stack.rs        # Stack safety (stacker integration)
│       └── grammar/        # Grammar modules (expr, item, type, etc.)
├── sigil-macros/       # Proc-macro crate
│   └── src/
│       ├── lib.rs          # Diagnostic/Subdiagnostic derives
│       ├── diagnostic.rs   # #[derive(Diagnostic)] impl
│       └── subdiagnostic.rs # #[derive(Subdiagnostic)] impl
└── sigilc/             # CLI orchestrator + Salsa queries
    └── src/
        ├── lib.rs          # Module organization
        ├── main.rs         # CLI entry point
        ├── db.rs           # Salsa database definition
        ├── query/          # Salsa query definitions
        ├── typeck/         # Type checking and inference
        ├── eval/           # Tree-walking interpreter
        ├── patterns/       # Pattern system
        ├── test/           # Test runner
        └── debug.rs        # Debug flags
```

### Crate Dependencies

```
sigil_ir ──────────────────────────────────────────────┐
    │                                                  │
    ▼                                                  │
sigil_diagnostic ◄─────────────────────────────────────┤
    │                                                  │
    ▼                                                  │
sigil_lexer                                            │
    │                                                  │
    ▼                                                  │
sigil_types                                            │
    │                                                  │
    ▼                                                  │
sigil_parse                                            │
    │                                                  │
    ▼                                                  │
sigilc (orchestrator + typeck + eval + patterns)◄──────┘
```

Pure functions live in library crates; Salsa queries live in `sigilc`.

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
| `SourceFile` | `sigilc` | Salsa input - source text |
| `TokenList` | `sigil_ir` | Lexer output |
| `Token` | `sigil_ir` | Individual token with kind and span |
| `Span` | `sigil_ir` | Source location (start/end offsets) |
| `Module` | `sigil_ir` | Parsed module structure |
| `ExprArena` | `sigil_ir` | Expression storage |
| `ExprId` | `sigil_ir` | Index into ExprArena |
| `Name` | `sigil_ir` | Interned string identifier |
| `Type` | `sigil_types` | Type representation |
| `Value` | `sigilc` | Runtime values |
| `Diagnostic` | `sigil_diagnostic` | Rich error with suggestions |
| `Applicability` | `sigil_diagnostic` | Fix confidence level |
| `ParseResult` | `sigil_parse` | Parser output (module + arena + errors) |

## Crate Organization

| Crate | Purpose |
|-------|---------|
| `sigil_ir` | Core IR types: tokens, spans, AST, arena, interning |
| `sigil_diagnostic` | Error reporting, suggestions, applicability levels |
| `sigil_lexer` | Tokenization via logos |
| `sigil_types` | Type system definitions |
| `sigil_parse` | Recursive descent parser |
| `sigil-macros` | Proc-macros (`#[derive(Diagnostic)]`, etc.) |
| `sigilc` | CLI orchestrator, Salsa queries, typeck, eval, patterns |

## File Size Guidelines

To maintain code quality, files follow size limits:

- **Target**: ~300 lines per file
- **Maximum**: 500 lines per file
- **Exception**: Grammar files may be larger due to many variants

When files exceed limits, extract submodules:
- `evaluator.rs` -> `eval/exec/expr.rs`, `eval/exec/call.rs`, etc.
- `types.rs` -> `typeck/infer/expr.rs`, `typeck/infer/call.rs`, etc.

## Related Documents

- [Compilation Pipeline](pipeline.md) - Detailed pipeline description
- [Salsa Integration](salsa-integration.md) - How Salsa is used
- [Data Flow](data-flow.md) - Data movement through phases
