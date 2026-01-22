# V2 Compiler Overview

## Executive Summary

The Sigil V2 compiler implements **Semantic Structural Compilation (SSC)** - a novel architecture that exploits Sigil's pattern-based design for unprecedented compilation speed and incrementality.

### Goals

| Goal | Target | Strategy |
|------|--------|----------|
| **10x faster cold compile** | 50ms for 1K LOC | Interning, flat AST, parallelism |
| **Sub-100ms incremental** | <50ms single file | Salsa queries, test-gated invalidation |
| **Full V1 compatibility** | 100% test pass | Same semantics, new implementation |
| **LSP foundation** | <100ms responses | Lazy parsing, incremental analysis |

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              SALSA QUERY DATABASE                               │
│                                                                                 │
│   All compilation artifacts stored as memoized queries:                         │
│   - Inputs: SourceFile, Configuration                                           │
│   - Derived: Tokens, AST, Types, TIR, Code                                      │
│   - Durability: LOW (user code) | MEDIUM (config) | HIGH (stdlib)               │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
                                      │
                      ┌───────────────┼───────────────┐
                      ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                           CONTENT-ADDRESSED STORAGE                             │
│                                                                                 │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐           │
│   │  Interned   │  │  Flattened  │  │   Pattern   │  │    Type     │           │
│   │  Strings    │  │     AST     │  │  Templates  │  │   Interner  │           │
│   │  Name(u32)  │  │  ExprId(u32)│  │  (shared)   │  │  TypeId(u32)│           │
│   └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘           │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
                                      │
                      ┌───────────────┼───────────────┐
                      ▼               ▼               ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                          PARALLEL COMPILATION ENGINE                            │
│                                                                                 │
│   ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐           │
│   │   Parallel  │  │   Parallel  │  │   Parallel  │  │   Parallel  │           │
│   │   Lexing    │  │   Parsing   │  │ Type Check  │  │   Codegen   │           │
│   │  (per file) │  │ (per file)  │  │(per module) │  │  (per func) │           │
│   └─────────────┘  └─────────────┘  └─────────────┘  └─────────────┘           │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌─────────────────────────────────────────────────────────────────────────────────┐
│                         TEST-GATED INVALIDATION                                 │
│                                                                                 │
│   Implementation changes + tests pass = NO downstream invalidation              │
│   Tests act as semantic contracts between modules                               │
│   Semantic hashing enables content-addressed caching                            │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
```

---

## Core Design Principles

### The Five Pillars

1. **Query Everything**
   - Every compilation step is a Salsa query
   - Automatic dependency tracking and memoization
   - Incremental recomputation on input changes

2. **Intern Everything**
   - Strings → `Name(u32)` indices
   - Types → `TypeId(u32)` indices
   - AST subtrees → content-addressed nodes
   - O(1) equality comparison via integer compare

3. **Flatten Everything**
   - No `Box<Expr>`, use `ExprId(u32)` indices
   - Contiguous arrays for cache locality
   - 50-83% memory reduction vs V1

4. **Parallelize Everything**
   - Files lex/parse concurrently
   - Modules type-check concurrently
   - Functions codegen concurrently
   - Work-stealing for load balancing

5. **Cache Everything**
   - Cross-project pattern template sharing
   - Semantic hashing for content-addressed artifacts
   - Durability levels prevent unnecessary revalidation

---

## V1 vs V2 Comparison

| Aspect | V1 | V2 |
|--------|-----|-----|
| **Identifiers** | `String` | `Name(u32)` interned |
| **AST children** | `Box<Expr>` | `ExprId(u32)` |
| **Type storage** | `Clone` everywhere | `TypeId(u32)` interned |
| **Environment** | Clone on call | Persistent + local overlay |
| **Struct fields** | `HashMap<String, Value>` | `Vec<Value>` indexed |
| **Caching** | None | Salsa queries |
| **Parallelism** | None | Rayon + work-stealing |
| **Incrementality** | None | Full via Salsa |
| **Pattern templates** | Recompile each use | Compile once, instantiate |

### Memory Comparison

| Structure | V1 Size | V2 Size | Savings |
|-----------|---------|---------|---------|
| `Binary { left, right }` | 24 bytes (2 Box) | 12 bytes (2 u32 + op) | **50%** |
| `Call { func, args: Vec }` | 40+ bytes | 12 bytes (u32 + range) | **70%** |
| `Ident(String)` | 24 bytes | 4 bytes (Name) | **83%** |

---

## Compilation Modes

V2 introduces tiered compilation for different use cases:

### `sigil check` (Fast Feedback)
- Syntax validation only
- Basic type checking (no full inference)
- Target: <200ms for any project size
- Use: IDE integration, quick validation

### `sigil build` (Standard)
- Full type checking with inference
- Minimal optimization
- Target: 10x faster than V1
- Use: Development builds

### `sigil build --opt` (Release)
- Full optimization passes
- Pattern fusion
- Dead code elimination
- Use: Production builds

### `sigil test` (Parallel Testing)
- Parallel test execution
- Coverage verification
- Test-gated invalidation updates
- Use: CI/CD pipelines

---

## Novel Contributions

### Test-Gated Invalidation

Sigil's mandatory testing requirement enables a unique optimization:

```
Module A (changed) → Tests pass → Semantic hash unchanged
                                        ↓
                   Downstream modules skip revalidation
```

When a module's implementation changes but its tests still pass, downstream dependents don't need full revalidation - only signature compatibility checks. This exploits Sigil's philosophy that tests define the semantic contract.

### Pattern Template Compilation

Patterns like `map`, `filter`, `fold` have consistent shapes:

```rust
// Template compiled once
PatternTemplate {
    kind: Map,
    code: [LOAD_ITER, CALL_TRANSFORM_SLOT, COLLECT],
    transform_slot: 1,  // Placeholder for user's transform function
}

// Instantiation just fills the slot
instantiate(template, user_transform_fn)
```

Cross-project: If two projects use `map(.over: [int], .transform: int -> int)`, they share the same compiled template.

### Pattern Fusion

Detects and optimizes pattern chains:

```sigil
// Before (3 passes over data)
items |> map(.transform: f) |> filter(.predicate: p) |> fold(.init: 0, .op: g)

// After (1 pass)
FusedMapFilterFold { input: items, map: f, filter: p, init: 0, fold: g }
```

### Lazy Parsing (V8-Inspired)

Function bodies are parsed on-demand:

```
Initial parse:  @function_name (params) -> Type  // Signature only
                ~~~~~~~~~ body tokens saved but not parsed ~~~~~~~~~

First call:     Parse body, cache result
Subsequent:     Use cached AST
```

Critical for LSP: Hover on a function shows its type without parsing every function in the file.

---

## Implementation Phases

| Phase | Weeks | Focus | Deliverable |
|-------|-------|-------|-------------|
| **1: Foundation** | 1-4 | Interner, flat AST, Salsa | Incremental lex/parse |
| **2: Type System** | 5-8 | Type interner, inference | Type checking with caching |
| **3: Patterns** | 9-12 | Templates, fusion | Pattern system operational |
| **4: Parallelism** | 13-16 | Rayon, work-stealing | Full parallel pipeline |
| **5: Advanced** | 17-20 | Test-gating, LSP | Production-ready compiler |
| **6: Formatter** | 21-22 | CST formatter | `sigil fmt` command |

---

## Success Criteria

### Performance
- [ ] 10x cold compile speedup demonstrated on benchmark corpus
- [ ] Sub-100ms incremental rebuild on single-file changes
- [ ] LSP response times within budget (see [F: LSP Architecture](specs/F-lsp-architecture.md))

### Compatibility
- [ ] 100% of V1 tests pass on V2
- [ ] Same error messages (modulo formatting improvements)
- [ ] Same runtime semantics

### Quality
- [ ] All SOLID principles followed (see [02-design-principles.md](02-design-principles.md))
- [ ] Zero code duplication (DRY compliant)
- [ ] Comprehensive documentation (this document set)

---

## Next Steps

1. Read [Design Principles](02-design-principles.md) for architectural philosophy
2. Review [Phase 1: Foundation](phases/03-phase-1-foundation.md) for implementation starting point
3. Consult [A: Data Structures](specs/A-data-structures.md) for technical specifications
