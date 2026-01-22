# Sigil Compiler V2 Design Documentation

## Table of Contents

This documentation describes the architecture for `sigilc-v2`, a complete rewrite of the Sigil compiler implementing **Semantic Structural Compilation (SSC)** - a novel approach that exploits Sigil's pattern-based design for unprecedented compilation speed and incrementality.

### Reading Order

**Start Here** (Required Reading):
1. [Overview](01-overview.md) - Executive summary, architecture diagram, key innovations
2. [Design Principles](02-design-principles.md) - SOLID, DRY, performance philosophy

**Implementation Phases** (Sequential):
3. [Phase 1: Foundation](phases/03-phase-1-foundation.md) - String interner, flat AST, Salsa (Weeks 1-4)
4. [Phase 2: Type System](phases/04-phase-2-type-system.md) - Type interning, bidirectional inference (Weeks 5-8)
5. [Phase 3: Patterns](phases/05-phase-3-patterns.md) - Templates, fusion, self-registration (Weeks 9-12)
6. [Phase 4: Parallelism](phases/06-phase-4-parallelism.md) - Rayon, work-stealing, parallel pipeline (Weeks 13-16)
7. [Phase 5: Advanced](phases/07-phase-5-advanced.md) - Test-gating, semantic hashing, LSP (Weeks 17-20)
8. [Phase 6: Formatter](phases/08-phase-6-formatter.md) - CST-based formatter (Weeks 21-22)

**Technical Specifications** (Reference):
- [A: Data Structures](specs/A-data-structures.md) - ExprId, TypeId, Name, ExprRange, memory layouts
- [B: Query System](specs/B-query-system.md) - Salsa queries, durability, caching strategies
- [C: Pattern System](specs/C-pattern-system.md) - Templates, fusion rules, self-registration
- [D: Parallelism](specs/D-parallelism.md) - Work-stealing, DashMap, thread pool configuration
- [E: Error Handling](specs/E-error-handling.md) - Diagnostics, recovery strategies, error codes
- [F: LSP Architecture](specs/F-lsp-architecture.md) - Response time budgets, incremental analysis

**Appendices** (Supporting Materials):
- [G: Benchmarks](appendices/G-benchmarks.md) - Targets, methodology, Criterion setup
- [H: Migration](appendices/H-migration.md) - V1 compatibility, test validation, rollout strategy
- [I: Dependencies](appendices/I-dependencies.md) - Cargo dependencies with rationale
- [J: File Structure](appendices/J-file-structure.md) - Complete `sigilc-v2/src/` layout

**Research Background** (Context):
- [K: V8 Optimizations](research/K-v8-optimizations.md) - Lazy parsing, hidden classes, allocation sinking
- [L: Rustc Queries](research/L-rustc-queries.md) - Red-green algorithm, query system deep dive
- [M: LSP Patterns](research/M-lsp-patterns.md) - rust-analyzer, clangd response time patterns

---

## Performance Targets

| Metric | V1 Current | V2 Target | Improvement |
|--------|------------|-----------|-------------|
| Cold compile (1K LOC) | 500ms | 50ms | **10x** |
| Cold compile (10K LOC) | 5s | 300ms | **16x** |
| Incremental (1 file) | 500ms | <50ms | **10x** |
| Memory (10K LOC) | 200MB | 50MB | **4x** |
| Type check throughput | 5K LOC/s | 50K LOC/s | **10x** |

### LSP Response Time Budget

| Operation | Target | Strategy |
|-----------|--------|----------|
| Hover | <20ms | Cached type lookup |
| Diagnostics | <50ms | Incremental validation |
| Completions | <100ms | Scope + type filtering |
| Go-to-definition | <50ms | Indexed symbol table |
| Formatting | <50ms | CST transformation |

---

## Key Innovations

### From Design-v2.md (Eric's Original Design)
- **Test-Gated Invalidation** - Tests as semantic contracts between modules
- **Pattern Template Compilation** - Compile once, instantiate many times
- **Pattern Fusion** - map→filter→fold = single pass
- **Durability Levels** - HIGH for stdlib (never revalidate)
- **Semantic Hashing** - Content-addressed compilation artifacts

### From Research (Novel Additions)
- **Lazy Parsing** - Parse function bodies on-demand (V8-inspired)
- **Tiered Compilation** - `check` vs `build` vs `build --opt`
- **CST Formatter** - Lossless syntax tree for perfect formatting
- **Allocation Sinking** - Escape analysis for stack allocation in codegen

### Sigil-Specific Advantages
- **No semicolons** - Clean statement boundaries for parallel parsing
- **Pattern keywords** - Context-sensitive, no global reservations
- **Mandatory testing** - Natural semantic hash boundaries
- **Explicit sigils** - `@` for functions, `$` for config = fast tokenization

---

## Document Conventions

### Code Examples

All Rust code examples are **illustrative pseudocode** unless marked with:
```rust
// [VERIFIED] - Compiles against specified dependencies
```

### Cross-References

- Internal links: `[Document Title](relative/path.md)`
- Section links: `[Section Name](file.md#section-anchor)`
- External sources: Linked in footnotes or Research documents

### Terminology

| Term | Definition |
|------|------------|
| **SSC** | Semantic Structural Compilation - the V2 architecture |
| **Query** | Salsa memoized function call |
| **Interned** | Deduplicated, content-addressed storage |
| **Durability** | How often an input changes (LOW/MEDIUM/HIGH) |
| **Fusion** | Combining multiple patterns into single pass |
| **Template** | Pre-compiled pattern code with holes for specialization |

---

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 0.1 | 2025-01 | Initial design document |
| 0.2 | 2025-01 | Restructured into modular documentation |

---

## Contributing

When updating this documentation:

1. **Keep phase files self-contained** - Each phase should be readable independently
2. **Update cross-references** - Check all `[links](paths.md)` after changes
3. **Verify code examples** - Mark verified snippets, update on API changes
4. **Maintain sync** - Changes to specs should reflect in phases and vice versa
