# Sigil Compiler Design Documentation

This documentation describes the architecture and design decisions of the Sigil compiler.

## Overview

The Sigil compiler is a Rust-based incremental compiler built on the Salsa framework. It is organized as a **multi-crate workspace** with clear separation of concerns:

- **`sigil_ir`** - Core IR types with no dependencies
- **`sigil_diagnostic`** - Error reporting system
- **`sigil_lexer`** - Tokenization
- **`sigil_types`** - Type system definitions
- **`sigil_parse`** - Recursive descent parser
- **`sigilc`** - CLI orchestrator, Salsa queries, type checker, evaluator, patterns

The compiler features:

- **Incremental compilation** via Salsa's automatic caching and dependency tracking
- **Flat AST representation** using arena allocation for memory efficiency
- **String interning** for O(1) identifier comparison
- **Extensible pattern system** with registry-based pattern definitions
- **Comprehensive diagnostics** with code fixes and multiple output formats

## Statistics

| Component | Lines of Code | Purpose |
|-----------|--------------|---------|
| IR | ~4,500 | AST types, arena, visitor, interning |
| Evaluator | ~5,500 | Tree-walking interpreter |
| Type System | ~4,300 | Type checking, inference, TypeContext |
| Parser | ~3,200 | Recursive descent parsing |
| Patterns | ~3,000 | Pattern system and builtins |
| Diagnostics | ~2,800 | Error reporting, DiagnosticQueue, fixes |
| Lexer | ~700 | DFA-based tokenization |
| Tests | ~1,100 | Test discovery, execution, error matching |
| **Total** | **~30,000** | |

## Compilation Pipeline

```
SourceFile (Salsa input)
    |
    | tokens() query
    v
TokenList
    |
    | parsed() query
    v
ParseResult { Module, ExprArena, errors }
    |
    | typed() query
    v
TypedModule { expr_types, errors }
    |
    | evaluated() query
    v
ModuleEvalResult { Value, EvalOutput }
```

Each step is a Salsa query with automatic caching. If the input doesn't change, the cached output is returned immediately.

## Documentation Sections

### Architecture

- [Architecture Overview](01-architecture/index.md) - High-level compiler structure
- [Compilation Pipeline](01-architecture/pipeline.md) - Query-based pipeline design
- [Salsa Integration](01-architecture/salsa-integration.md) - Incremental compilation framework
- [Data Flow](01-architecture/data-flow.md) - How data moves through the compiler

### Intermediate Representation

- [IR Overview](02-intermediate-representation/index.md) - Data structures for compilation
- [Flat AST](02-intermediate-representation/flat-ast.md) - Arena-based expression storage
- [Arena Allocation](02-intermediate-representation/arena-allocation.md) - Memory management strategy
- [String Interning](02-intermediate-representation/string-interning.md) - Identifier deduplication
- [Type Representation](02-intermediate-representation/type-representation.md) - Runtime type encoding

### Lexer

- [Lexer Overview](03-lexer/index.md) - Tokenization design
- [Token Design](03-lexer/token-design.md) - Token types and structure

### Parser

- [Parser Overview](04-parser/index.md) - Parsing architecture
- [Recursive Descent](04-parser/recursive-descent.md) - Parsing approach
- [Error Recovery](04-parser/error-recovery.md) - Handling syntax errors
- [Grammar Modules](04-parser/grammar-modules.md) - Module organization

### Type System

- [Type System Overview](05-type-system/index.md) - Type checking architecture
- [Type Inference](05-type-system/type-inference.md) - Hindley-Milner inference
- [Unification](05-type-system/unification.md) - Type constraint solving
- [Type Environment](05-type-system/type-environment.md) - Scope-based type tracking
- [Type Registry](05-type-system/type-registry.md) - User-defined type storage

### Pattern System

- [Pattern System Overview](06-pattern-system/index.md) - Pattern architecture
- [Pattern Trait](06-pattern-system/pattern-trait.md) - PatternDefinition interface
- [Pattern Registry](06-pattern-system/pattern-registry.md) - Pattern lookup system
- [Pattern Fusion](06-pattern-system/pattern-fusion.md) - Optimization passes
- [Adding Patterns](06-pattern-system/adding-patterns.md) - How to add new patterns

### Evaluator

- [Evaluator Overview](07-evaluator/index.md) - Interpretation architecture
- [Tree Walking](07-evaluator/tree-walking.md) - Execution strategy
- [Environment](07-evaluator/environment.md) - Variable scoping
- [Value System](07-evaluator/value-system.md) - Runtime value representation
- [Module Loading](07-evaluator/module-loading.md) - Import resolution

### Diagnostics

- [Diagnostics Overview](08-diagnostics/index.md) - Error reporting system
- [Problem Types](08-diagnostics/problem-types.md) - Error categorization
- [Code Fixes](08-diagnostics/code-fixes.md) - Automatic fix suggestions
- [Emitters](08-diagnostics/emitters.md) - Output format handlers

### Testing

- [Testing Overview](09-testing/index.md) - Test system architecture
- [Test Discovery](09-testing/test-discovery.md) - Finding test functions
- [Test Runner](09-testing/test-runner.md) - Parallel test execution

### Appendices

- [Salsa Patterns](appendices/A-salsa-patterns.md) - Common Salsa usage patterns
- [Memory Management](appendices/B-memory-management.md) - Allocation strategies
- [Error Codes](appendices/C-error-codes.md) - Complete error code reference
- [Debugging](appendices/D-debugging.md) - Debug flags and tracing

## Source Paths

The compiler is organized as a multi-crate workspace:

| Crate | Path | Purpose |
|-------|------|---------|
| `sigil_ir` | `compiler/sigil_ir/src/` | Core IR types (tokens, spans, AST, arena, interning) |
| `sigil_diagnostic` | `compiler/sigil_diagnostic/src/` | DiagnosticQueue, error reporting, suggestions, emitters |
| `sigil_lexer` | `compiler/sigil_lexer/src/` | Tokenization via logos |
| `sigil_types` | `compiler/sigil_types/src/` | Type, TypeError, TypeContext, InferenceContext |
| `sigil_parse` | `compiler/sigil_parse/src/` | Recursive descent parser |
| `sigil-macros` | `compiler/sigil-macros/src/` | Diagnostic derive macros |
| `sigilc` | `compiler/sigilc/src/` | CLI, Salsa queries, typeck, eval, patterns |

**Note:** `sigilc` modules (`ir`, `parser`, `diagnostic`, `types`) re-export from source crates for DRY.

### sigilc Internal Paths

| Component | Path |
|-----------|------|
| Library root | `compiler/sigilc/src/lib.rs` |
| Salsa database | `compiler/sigilc/src/db.rs` |
| Query system | `compiler/sigilc/src/query/` |
| Type checker | `compiler/sigilc/src/typeck/` |
| Evaluator | `compiler/sigilc/src/eval/` |
| Patterns | `compiler/sigilc/src/patterns/` |
| Tests | `compiler/sigilc/src/test/` |
