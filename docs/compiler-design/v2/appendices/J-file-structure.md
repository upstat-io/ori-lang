# J: File Structure

This document specifies the complete source tree layout for `sigilc-v2`.

---

## Top-Level Structure

```
compiler/sigilc-v2/
├── Cargo.toml
├── README.md
├── src/
│   ├── lib.rs                 # Public API
│   ├── db.rs                  # Salsa database definition
│   ├── cli/                   # Command-line interface
│   ├── intern/                # Interning infrastructure
│   ├── syntax/                # Lexing and parsing
│   ├── hir/                   # High-level IR (name-resolved)
│   ├── check/                 # Type checking
│   ├── tir/                   # Typed IR (lowered)
│   ├── patterns/              # Pattern system
│   ├── eval/                  # Tree-walking interpreter
│   ├── codegen/               # C code generation
│   ├── format/                # Code formatter
│   ├── lsp/                   # Language server
│   ├── tests/                 # Test infrastructure
│   └── errors/                # Diagnostics
├── tests/
│   ├── compatibility/         # V1/V2 comparison tests
│   ├── incremental/           # Incrementality tests
│   ├── parallel/              # Parallelism tests
│   └── integration/           # End-to-end tests
└── benches/
    ├── lexer.rs
    ├── parser.rs
    ├── type_check.rs
    ├── codegen.rs
    ├── incremental.rs
    └── corpus/                # Benchmark input files
```

---

## Source Directory Details

### lib.rs

```rust
//! Sigil Compiler V2
//!
//! High-performance incremental compiler for the Sigil language.

pub mod db;
pub mod intern;
pub mod syntax;
pub mod hir;
pub mod check;
pub mod tir;
pub mod patterns;
pub mod eval;
pub mod codegen;
pub mod format;
pub mod errors;

#[cfg(feature = "lsp")]
pub mod lsp;

pub mod tests;

// Re-exports
pub use db::{Db, CompilerDb};
pub use errors::Diagnostic;
pub use syntax::{parse, lex};
```

### db.rs

```rust
//! Salsa database definition

use salsa::Database;

/// Main compiler database trait
#[salsa::db]
pub trait Db: Database {
    fn interner(&self) -> &StringInterner;
    fn type_interner(&self) -> &TypeInterner;
    fn pattern_registry(&self) -> &PatternRegistry;
}

/// Concrete database implementation
#[salsa::db]
pub struct CompilerDb {
    storage: salsa::Storage<Self>,
    interner: StringInterner,
    type_interner: TypeInterner,
    pattern_registry: PatternRegistry,
}
```

### intern/

```
intern/
├── mod.rs                     # Module exports
├── strings.rs                 # String interner (Name)
└── types.rs                   # Type interner (TypeId)
```

**strings.rs:**
```rust
//! Sharded string interner for O(1) identifier comparison

/// Interned string identifier
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Name(u32);

/// Thread-safe sharded string interner
pub struct StringInterner {
    shards: [RwLock<InternShard>; 16],
}
```

**types.rs:**
```rust
//! Type interner for O(1) type comparison

/// Interned type identifier
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct TypeId(u32);

/// Thread-safe type interner
pub struct TypeInterner {
    map: DashMap<TypeKind, TypeId>,
    types: RwLock<Vec<TypeKind>>,
}
```

### syntax/

```
syntax/
├── mod.rs                     # Module exports
├── token.rs                   # Token definitions
├── lexer.rs                   # Logos-based lexer
├── ast.rs                     # Flattened AST definitions
├── arena.rs                   # Expression arena
├── parser.rs                  # Recursive descent parser
├── parser/
│   ├── expr.rs                # Expression parsing
│   ├── stmt.rs                # Statement parsing
│   ├── item.rs                # Top-level item parsing
│   ├── type_expr.rs           # Type expression parsing
│   └── pattern.rs             # Pattern parsing
└── cst.rs                     # Concrete syntax tree (for formatter)
```

### hir/

```
hir/
├── mod.rs                     # Module exports
├── def.rs                     # Definition IDs
├── body.rs                    # Function bodies
├── resolver.rs                # Name resolution
├── scope.rs                   # Scope tracking
└── import.rs                  # Import resolution
```

### check/

```
check/
├── mod.rs                     # Module exports
├── context.rs                 # Type checking context
├── infer.rs                   # Type inference (synthesis)
├── check.rs                   # Type checking (analysis)
├── unify.rs                   # Type unification
├── traits.rs                  # Trait resolution
├── capabilities.rs            # Capability tracking
└── diagnostics.rs             # Type error construction
```

### tir/

```
tir/
├── mod.rs                     # Module exports
├── expr.rs                    # Typed expression IR
├── lower.rs                   # HIR → TIR lowering
└── optimize.rs                # TIR optimizations
```

### patterns/

```
patterns/
├── mod.rs                     # Module exports, PatternDefinition trait
├── registry.rs                # Pattern registry
├── signature.rs               # Pattern signatures
├── templates.rs               # Template compilation and caching
├── fusion.rs                  # Pattern fusion detection and execution
├── builtins/
│   ├── mod.rs                 # Built-in pattern exports
│   ├── run.rs                 # run pattern
│   ├── try_pattern.rs         # try pattern
│   ├── match_pattern.rs       # match pattern
│   ├── map.rs                 # map pattern
│   ├── filter.rs              # filter pattern
│   ├── fold.rs                # fold pattern
│   ├── find.rs                # find pattern
│   ├── collect.rs             # collect pattern
│   ├── recurse.rs             # recurse pattern
│   ├── parallel.rs            # parallel pattern
│   ├── timeout.rs             # timeout pattern
│   ├── retry.rs               # retry pattern
│   ├── cache.rs               # cache pattern
│   └── validate.rs            # validate pattern
└── parse.rs                   # Pattern argument parsing
```

### eval/

```
eval/
├── mod.rs                     # Module exports
├── value.rs                   # Runtime value representation
├── env.rs                     # Evaluation environment
├── exec.rs                    # Expression evaluation
├── struct_layout.rs           # Struct field indexing
└── builtin.rs                 # Built-in functions
```

### codegen/

```
codegen/
├── mod.rs                     # Module exports
├── context.rs                 # Codegen context
├── c/
│   ├── mod.rs                 # C backend
│   ├── expr.rs                # Expression codegen
│   ├── stmt.rs                # Statement codegen
│   ├── type.rs                # Type codegen
│   └── runtime.rs             # Runtime support code
├── templates.rs               # Pattern template instantiation
└── link.rs                    # Linking support
```

### format/

```
format/
├── mod.rs                     # Module exports
├── cst.rs                     # Concrete syntax tree
├── ir.rs                      # Format intermediate representation
├── formatter.rs               # CST → IR transformation
├── printer.rs                 # IR → String printing
├── rules.rs                   # Formatting rules
└── cache.rs                   # Format caching
```

### lsp/

```
lsp/
├── mod.rs                     # Module exports
├── server.rs                  # LSP server implementation
├── handlers/
│   ├── mod.rs                 # Handler exports
│   ├── hover.rs               # Hover handler
│   ├── completion.rs          # Completion handler
│   ├── definition.rs          # Go-to-definition handler
│   ├── references.rs          # Find references handler
│   ├── rename.rs              # Rename handler
│   ├── diagnostics.rs         # Diagnostic publisher
│   └── formatting.rs          # Format handler
├── index.rs                   # Symbol index
└── lazy_module.rs             # Lazy parsing support
```

### tests/

```
tests/
├── mod.rs                     # Module exports
├── runner.rs                  # Parallel test runner
├── coverage.rs                # Coverage checking
├── result.rs                  # Test result types
└── harness.rs                 # Test harness utilities
```

### errors/

```
errors/
├── mod.rs                     # Module exports
├── diagnostic.rs              # Diagnostic type
├── codes.rs                   # Error codes
├── builder.rs                 # Diagnostic builder
├── render.rs                  # Terminal rendering
└── lsp.rs                     # LSP diagnostic conversion
```

### cli/

```
cli/
├── mod.rs                     # Module exports
├── main.rs                    # Entry point (separate binary)
├── run.rs                     # `sigil run` command
├── build.rs                   # `sigil build` command
├── check.rs                   # `sigil check` command
├── test.rs                    # `sigil test` command
├── fmt.rs                     # `sigil fmt` command
└── args.rs                    # Argument parsing
```

---

## Test Directory Details

```
tests/
├── compatibility/
│   ├── mod.rs
│   └── run_both.rs            # Run V1 and V2, compare outputs
├── incremental/
│   ├── mod.rs
│   ├── edit_tests.rs          # Edit scenarios
│   └── cache_tests.rs         # Cache behavior tests
├── parallel/
│   ├── mod.rs
│   ├── scaling.rs             # Scaling tests
│   └── correctness.rs         # Parallel correctness tests
└── integration/
    ├── mod.rs
    ├── e2e.rs                 # End-to-end tests
    └── fixtures/              # Test input files
```

---

## Benchmark Directory Details

```
benches/
├── lexer.rs                   # Lexer benchmarks
├── parser.rs                  # Parser benchmarks
├── type_check.rs              # Type checking benchmarks
├── codegen.rs                 # Code generation benchmarks
├── incremental.rs             # Incremental compilation benchmarks
├── memory.rs                  # Memory usage benchmarks
├── e2e.rs                     # End-to-end benchmarks
├── corpus/
│   ├── small/                 # <1K LOC test files
│   ├── medium/                # 1K-10K LOC test files
│   └── large/                 # >10K LOC test files
└── utils.rs                   # Benchmark utilities
```

---

## Module Dependency Graph

```
                    ┌─────────┐
                    │   db    │
                    └────┬────┘
                         │
         ┌───────────────┼───────────────┐
         ▼               ▼               ▼
    ┌─────────┐    ┌─────────┐    ┌─────────┐
    │ intern  │    │ errors  │    │ patterns│
    └────┬────┘    └────┬────┘    └────┬────┘
         │              │              │
         ▼              │              ▼
    ┌─────────┐         │         ┌─────────┐
    │ syntax  │◄────────┤         │templates│
    └────┬────┘         │         └────┬────┘
         │              │              │
         ▼              ▼              ▼
    ┌─────────┐    ┌─────────┐    ┌─────────┐
    │   hir   │───►│  check  │◄───│  eval   │
    └────┬────┘    └────┬────┘    └─────────┘
         │              │
         ▼              ▼
    ┌─────────┐    ┌─────────┐
    │   tir   │◄───│codegen  │
    └─────────┘    └─────────┘
```

---

## Build Artifacts

```
target/
├── debug/
│   ├── sigilc-v2              # Debug binary
│   └── libsigilc_v2.rlib      # Debug library
├── release/
│   ├── sigilc-v2              # Release binary
│   └── libsigilc_v2.rlib      # Release library
└── criterion/
    └── reports/               # Benchmark HTML reports
```
