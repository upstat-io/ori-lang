# Architecture Overview

The Sigil compiler (`sigilc`) is an incremental compiler built on Salsa, a framework for on-demand, incremental computation. The architecture prioritizes:

1. **Incrementality** - Only recompute what changes
2. **Memory efficiency** - Arena allocation, string interning
3. **Extensibility** - Registry-based patterns and diagnostics
4. **Testability** - Dependency injection via SharedRegistry

## High-Level Structure

```
compiler/
├── sigil-macros/       # Proc-macro crate
│   └── src/
│       ├── lib.rs          # Diagnostic/Subdiagnostic derives
│       ├── diagnostic.rs   # #[derive(Diagnostic)] impl
│       └── subdiagnostic.rs # #[derive(Subdiagnostic)] impl
└── sigilc/src/
    ├── lib.rs              # Module organization
    ├── main.rs             # CLI entry point
    ├── db.rs               # Salsa database definition
    ├── query/              # Salsa query definitions
    ├── lexer.rs            # Tokenization
    ├── parser/             # Recursive descent parser
    ├── ir/                 # AST and intermediate types
    ├── types.rs            # Type definitions
    ├── typeck/             # Type checking and inference
    ├── eval/               # Tree-walking interpreter
    ├── patterns/           # Pattern system
    ├── diagnostic/         # Error reporting
    ├── test/               # Test runner
    ├── stack.rs            # Stack safety utilities
    └── debug.rs            # Debug flags
```

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

| Type | Location | Purpose |
|------|----------|---------|
| `SourceFile` | `db.rs` | Salsa input - source text |
| `TokenList` | `ir/token.rs` | Lexer output |
| `Module` | `ir/ast.rs` | Parsed module structure |
| `ExprArena` | `ir/arena.rs` | Expression storage |
| `Type` | `types.rs` | Type representation |
| `Value` | `eval/value/` | Runtime values |
| `Diagnostic` | `diagnostic/` | Rich error with suggestions |
| `Applicability` | `diagnostic/` | Fix confidence level |

## Crate Organization

| Crate | Purpose |
|-------|---------|
| `sigilc` | Main compiler library and CLI |
| `sigil-macros` | Proc-macros (`#[derive(Diagnostic)]`, etc.) |

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
