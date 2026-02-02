# ori_parse Improvements Plan

> **Hybrid Architecture** — Adopting the best patterns from Rust, Go, TypeScript, Zig, Gleam, Elm, and Roc parsers to enhance Ori's parser.

## Background

This plan is the result of a comprehensive analysis of parser architectures across 7 major language implementations:

- **Rust** (`rustc_parse`): ~24K LOC, snapshot/restore, restrictions bitflags
- **Go** (`syntax`): ~2.9K LOC, token bitsets, embedded scanner
- **TypeScript** (`parser.ts`): ~10.8K LOC, incremental parsing, speculative parsing
- **Zig** (`Parse.zig`): Flat SoA AST, two-tier storage, scratch buffer
- **Gleam** (`parse.rs`): ~5K LOC, series combinator, rich error types
- **Elm** (`Parse/`): ~4K LOC Haskell, CPS combinators, byte-level parsing
- **Roc** (`parse/`): ~19K LOC, progress tracking, SIMD preprocessing

---

## Current State: Ori Parser (ori_parse)

**Strengths** (already excellent):
- ✅ Progress-aware parsing (Roc-inspired)
- ✅ Flat arena allocation with indices (like Zig)
- ✅ Context flags via `ParseContext` bitfield
- ✅ Macro-generated precedence levels
- ✅ Recovery sets for synchronization
- ✅ Salsa-compatible types (Clone, Eq, Hash)
- ✅ Soft keyword handling

**Architecture Stats**:
- ~8,045 lines across 23 Rust files
- Recursive descent with precedence climbing
- Separate lexer (`ori_lexer`) with `TokenList`

---

## Identified Gaps

| Gap | Best-in-Class | Impact | Effort |
|-----|---------------|--------|--------|
| Token bitsets for recovery | Go | High | Low |
| Scratch buffer for temps | Zig | Low | Low |
| Richer error variants | Gleam/Roc | High | Medium |
| Speculative parsing | Rust/TypeScript | Medium | Medium |
| Two-tier inline/overflow storage | Zig | Medium | Medium |
| Incremental parsing | TypeScript | High | High |
| SIMD preprocessing | Roc | Medium | High |

---

## Phase Overview

### Phase 1: Quick Wins (Section 01)
**Timeline**: 1-2 weeks | **Impact**: High | **Effort**: Low

- Token bitsets for recovery set operations
- Scratch buffer for temporary collections
- Expanded error type hierarchy

### Phase 2: Medium-Term (Section 02)
**Timeline**: 2-4 weeks | **Impact**: Medium | **Effort**: Medium

- Speculative parsing with snapshots
- Two-tier inline/overflow storage
- Series combinator abstraction

### Phase 3: Long-Term (Section 03)
**Timeline**: 4-8 weeks | **Impact**: High | **Effort**: High

- Incremental parsing infrastructure
- Language server integration
- SIMD preprocessing for large files

---

## Dependency Graph

```
Phase 1 (Quick Wins)
├── 1.1 Token Bitsets        [no deps]
├── 1.2 Scratch Buffer       [no deps]
└── 1.3 Rich Error Types     [no deps]

Phase 2 (Medium-Term)
├── 2.1 Snapshots            [depends on 1.3 for error handling]
├── 2.2 Two-Tier Storage     [no deps]
└── 2.3 Series Combinator    [depends on 1.2 for scratch usage]

Phase 3 (Long-Term)
├── 3.1 Incremental Parsing  [depends on 2.1 snapshots, 2.2 storage]
├── 3.2 LSP Integration      [depends on 3.1 incremental]
└── 3.3 SIMD Preprocessing   [depends on ori_lexer changes]
```

---

## Success Criteria

A section is complete when:

1. **Implemented**: Changes in `compiler/ori_parse/src/`
2. **Tested**: Unit tests + integration tests
3. **Benchmarked**: Performance comparison before/after
4. **Documented**: Code comments + CLAUDE.md if API changes

---

## Non-Goals

- **Parser generator migration**: Hand-written recursive descent is the right choice for Ori's grammar complexity and error message quality
- **Removing progress tracking**: This is a core strength, not something to replace
- **Embedded scanner**: Ori benefits from separate lexer for cleaner architecture

---

## References

| Document | Purpose |
|----------|---------|
| `index.md` | Keyword search for sections |
| `section-01-quick-wins.md` | Phase 1 detailed tasks |
| `section-02-medium-term.md` | Phase 2 detailed tasks |
| `section-03-long-term.md` | Phase 3 detailed tasks |

### External References

| Reference | Location |
|-----------|----------|
| Ori Parser | `compiler/ori_parse/src/` |
| Ori Lexer | `compiler/ori_lexer/src/` |
| Rust Parser | `~/lang_repos/rust/compiler/rustc_parse/` |
| Go Parser | `~/lang_repos/golang/src/cmd/compile/internal/syntax/` |
| TypeScript Parser | `~/lang_repos/typescript/src/compiler/parser.ts` |
| Zig Parser | `~/lang_repos/zig/lib/std/zig/` |
| Gleam Parser | `~/lang_repos/gleam/compiler-core/src/parse/` |
| Elm Parser | `~/lang_repos/elm/compiler/src/Parse/` |
| Roc Parser | `~/lang_repos/roc/crates/compiler/parse/` |
