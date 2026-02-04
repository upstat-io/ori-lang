# Parser 2.0 Implementation Plan

> **ROADMAP**: Enhances `plans/roadmap/section-00-parser.md`
> **Best-of-Breed Parser Architecture** — Combining innovations from Rust, Go, Zig, TypeScript, Gleam, Elm, and Roc

## Design Philosophy

Based on deep analysis of 7 production-grade language parsers (~50,000+ lines of parser code), this plan synthesizes the best patterns into a novel architecture for Ori's parser:

1. **Data-Oriented Design** — Zig-style MultiArrayList for cache-friendly AST storage
2. **Zero-Cost Abstractions** — Go-style perfect hashing, counter-based context
3. **Automatic Backtracking** — Elm/Roc-style progress tracking with context capture
4. **Gold-Standard Errors** — Elm/Gleam-style empathetic, educational messages
5. **IDE-First Design** — TypeScript-style incremental parsing with syntax cursor
6. **Lossless Preservation** — Gleam/Roc-style metadata for formatting support

The goal is to create a parser that is:
- **2-3x more memory efficient** than traditional designs
- **5-10x faster for incremental edits** (IDE scenarios)
- **Industry-leading error messages** (Elm quality)
- **Formatter-ready by design** (lossless roundtrip)

---

## Section Overview

### Section 1: Data-Oriented AST

Replace pointer-based AST with index-based, cache-friendly storage.

| Subsection | Focus | Source |
|------------|-------|--------|
| 1.1 | MultiArrayList-style storage | Zig |
| 1.2 | Index-based node references | Zig |
| 1.3 | Extra data buffer | Zig |
| 1.4 | Pre-allocation heuristics | Zig |
| 1.5 | Scratch buffer integration | Ori (existing) |

### Section 2: Lexer Optimizations

Optimize keyword recognition and operator handling.

| Subsection | Focus | Source |
|------------|-------|--------|
| 2.1 | Perfect hash keywords | Go |
| 2.2 | Compile-time collision detection | Go |
| 2.3 | Precedence metadata in tokens | Go, Rust |
| 2.4 | Adjacent token optimization | Ori (existing) |

### Section 3: Enhanced Progress System

Extend progress tracking with context capture for better errors.

| Subsection | Focus | Source |
|------------|-------|--------|
| 3.1 | ParseOutcome with context | Elm, Roc |
| 3.2 | Automatic backtracking macros | Roc |
| 3.3 | Expected token accumulation | Rust |
| 3.4 | Context wrapping utilities | Elm |

### Section 4: Structured Errors

Build Elm-quality error messages with actionable suggestions.

| Subsection | Focus | Source |
|------------|-------|--------|
| 4.1 | ParseErrorDetails structure | Gleam |
| 4.2 | Empathetic message templates | Elm, Gleam |
| 4.3 | Common mistake detection | All |
| 4.4 | Cross-file error labels | Gleam |

### Section 5: Incremental Parsing

Enable efficient reparsing for IDE scenarios.

| Subsection | Focus | Source |
|------------|-------|--------|
| 5.1 | Syntax cursor with caching | TypeScript |
| 5.2 | Node reusability predicates | TypeScript |
| 5.3 | Change range propagation | Ori (existing) |
| 5.4 | Lazy token capture | Rust |

### Section 6: Formatting Metadata

Preserve non-semantic information for formatters.

| Subsection | Focus | Source |
|------------|-------|--------|
| 6.1 | ModuleExtra structure | Gleam |
| 6.2 | Comment collection | Gleam, Roc |
| 6.3 | SpaceBefore/SpaceAfter pattern | Roc |
| 6.4 | Detached doc comment warnings | Gleam |

---

## Performance Targets

| Metric | Current | Target | Improvement |
|--------|---------|--------|-------------|
| Memory per node | ~24 bytes | ~13 bytes | 46% reduction |
| Keyword lookup | O(log n) switch | O(1) hash | ~3x faster |
| Incremental reparse | Full reparse | 70-90% reuse | 5-10x faster |
| AST traversal | Random access | Sequential (SoA) | 2-3x cache hits |
| Error message quality | Good | Elm-tier | Qualitative |

---

## Dependency Graph

```
Section 1 (Data-Oriented AST) ─┬─► Section 5 (Incremental)
                               │
Section 2 (Lexer) ─────────────┤
                               │
Section 3 (Progress) ──────────┼─► Section 4 (Errors)
                               │
                               └─► Section 6 (Metadata)
```

**Key Dependencies**:
- Section 1 (AST) can proceed independently
- Section 2 (Lexer) can proceed independently
- Section 3 (Progress) enables Section 4 (Errors)
- Section 5 (Incremental) builds on Section 1 (AST)
- Section 6 (Metadata) can proceed after Section 1

---

## Implementation Phases

### Phase 1: Foundation (Low-risk, High-impact)
**Target: Weeks 1-4**

| Task | Section | Risk | Impact |
|------|---------|------|--------|
| Perfect hash keywords | 2.1-2.2 | Low | High |
| Expected token accumulation | 3.3 | Low | High |
| Integrate scratch buffer | 1.5 | Low | Medium |
| Empathetic error templates | 4.2 | Low | High |

### Phase 2: Core Architecture (Medium-risk, High-impact)
**Target: Weeks 5-10**

| Task | Section | Risk | Impact |
|------|---------|------|--------|
| MultiArrayList-style storage | 1.1-1.2 | Medium | High |
| Extra data buffer | 1.3 | Medium | High |
| Pre-allocation heuristics | 1.4 | Low | Medium |
| ParseErrorDetails structure | 4.1 | Low | High |

### Phase 3: Enhanced Progress (Medium-risk, High-impact)
**Target: Weeks 11-14**

| Task | Section | Risk | Impact |
|------|---------|------|--------|
| ParseOutcome with context | 3.1 | Medium | High |
| one_of! macro | 3.2 | Medium | High |
| Context wrapping utilities | 3.4 | Medium | Medium |
| Common mistake detection | 4.3 | Low | High |

### Phase 4: IDE Support (Medium-risk, High-impact)
**Target: Weeks 15-20**

| Task | Section | Risk | Impact |
|------|---------|------|--------|
| Syntax cursor | 5.1-5.2 | Medium | High |
| Change range propagation | 5.3 | Medium | High |
| Lazy token capture | 5.4 | Medium | Medium |
| ModuleExtra structure | 6.1-6.2 | Low | Medium |

### Phase 5: Polish (Low-risk, Medium-impact)
**Target: Weeks 21-24**

| Task | Section | Risk | Impact |
|------|---------|------|--------|
| SpaceBefore/SpaceAfter | 6.3 | Low | Medium |
| Detached doc comment warnings | 6.4 | Low | Low |
| Cross-file error labels | 4.4 | Low | Medium |
| Performance tuning | All | Low | Medium |

---

## Reference Implementations

### Files Analyzed

| Language | Parser Location | Lines | Key Innovation |
|----------|-----------------|-------|----------------|
| Rust | `compiler/rustc_parse/` | ~14K | Lazy tokens, recovery |
| Go | `src/cmd/compile/internal/syntax/` | ~6K | Perfect hash, speed |
| Zig | `lib/std/zig/Parse.zig` | ~10K | Data-oriented, GB/s |
| TypeScript | `src/compiler/parser.ts` | ~15K | Incremental, IDE |
| Gleam | `compiler-core/src/parse.rs` | ~6K | Friendly errors |
| Elm | `compiler/src/Parse/` | ~4K | Gold-standard errors |
| Roc | `crates/compiler/parse/` | ~19K | Progress tracking |

### Key Patterns Adopted

| Pattern | Source | Ori Section |
|---------|--------|-------------|
| MultiArrayList (SoA) | Zig | 1.1 |
| Index-based nodes | Zig | 1.2 |
| Extra data buffer | Zig | 1.3 |
| Perfect hash keywords | Go | 2.1 |
| Precedence in scanner | Go, Rust | 2.3 |
| Progress tracking | Roc, Elm | 3.1 |
| Expected accumulation | Rust | 3.3 |
| ParseErrorDetails | Gleam | 4.1 |
| Empathetic messages | Elm, Gleam | 4.2 |
| Syntax cursor | TypeScript | 5.1 |
| Node reusability | TypeScript | 5.2 |
| Lazy token capture | Rust | 5.4 |
| ModuleExtra | Gleam | 6.1 |
| SpaceBefore/After | Roc | 6.3 |

---

## Quick Reference

| Document | Purpose |
|----------|---------|
| `00-overview.md` | This file - plan overview |
| `index.md` | Keyword index for quick finding |
| `section-01-data-oriented-ast.md` | Cache-friendly AST storage |
| `section-02-lexer.md` | Keyword hashing, operator metadata |
| `section-03-progress.md` | Enhanced progress and backtracking |
| `section-04-errors.md` | Structured error messages |
| `section-05-incremental.md` | IDE-friendly incremental parsing |
| `section-06-metadata.md` | Formatting metadata preservation |

---

## Existing Ori Strengths

The current Ori parser already has excellent foundations:

| Feature | Quality | Notes |
|---------|---------|-------|
| Progress tracking | Excellent | Like Roc/Elm |
| Arena allocation | Good | ExprArena + ExprId |
| Context flags | Good | Bitfield (u16) |
| TokenSet recovery | Excellent | 128-bit bitset |
| Series combinator | Good | Like Gleam |
| Snapshot isolation | Excellent | ~10 bytes |
| Incremental infrastructure | Prepared | Not yet integrated |
| Error hints | Good | Smart suggestions |

This plan builds on these strengths rather than replacing them.
