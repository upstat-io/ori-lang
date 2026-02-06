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

| Metric | Current | Target | Improvement | Status |
|--------|---------|--------|-------------|--------|
| Memory per node | ~88 bytes (Expr) | ~32 bytes (SoA) | 64% reduction | ✅ Complete (2026-02-05) |
| Keyword lookup | O(1) logos DFA | O(1) hash | N/A | ✅ Already optimal |
| Incremental reparse | Full reparse | 70-90% reuse | 5-10x faster | ✅ Infrastructure complete |
| Token capture | Copy tokens | Index-based | O(1) lookup | ✅ Complete (TokenCapture) |
| AST traversal | Random access | Sequential (SoA) | 2-3x cache hits | ✅ Complete (SoA split) |
| Error message quality | Good | Elm-tier | Qualitative | ✅ Complete |

**Note:** SoA migration completed 2026-02-05. `Expr` reduced from 88 to 32 bytes (64%). Storage split into parallel `Vec<ExprKind>` + `Vec<Span>`. `ExprKind` shrunk from 80 to 24 bytes via arena-allocation of large embedded types. `ExprList` eliminated in favor of `ExprRange`.

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
**Target: Weeks 1-4** | **Status: ✅ Complete (2026-02-04)**

| Task | Section | Risk | Impact | Status |
|------|---------|------|--------|--------|
| Perfect hash keywords | 2.1-2.2 | Low | High | ✅ Satisfied by logos |
| Expected token accumulation | 3.3 | Low | High | ✅ Complete |
| Integrate scratch buffer | 1.5 | Low | Medium | 🔶 Deferred |
| Empathetic error templates | 4.2 | Low | High | ✅ Complete |

**Phase 1 Summary:**
- **2.1-2.2 Keywords:** Discovered logos lexer already provides O(1)-equivalent keyword recognition via DFA state machine. No changes needed.
- **3.3 Expected Tokens:** Implemented `TokenSet` iterator, mutation methods, `format_expected()`, and `TokenKind::friendly_name_from_index()`.
- **4.2 Error Templates:** Added `title()` and `empathetic_message()` methods to `ParseErrorKind` with Elm-style conversational phrasing.
- **1.5 Scratch Buffer:** Deferred - infrastructure exists in `scratch.rs` but integration requires refactoring `series()` and all call sites.

### Phase 2: Core Architecture (Medium-risk, High-impact)
**Target: Weeks 5-10** | **Status: ✅ Complete (2026-02-04)**

| Task | Section | Risk | Impact | Status |
|------|---------|------|--------|--------|
| MultiArrayList-style storage | 1.1-1.2 | Medium | High | ✅ Complete (SoA migration) |
| Extra data buffer | 1.3 | Medium | High | ✅ Already implemented (`expr_lists`) |
| Pre-allocation heuristics | 1.4 | Low | Medium | ✅ Already implemented |
| ParseErrorDetails structure | 4.1 | Low | High | ✅ Complete |

**Phase 2 Summary:**
- **1.1-1.2 SoA Storage:** ✅ Fully implemented (2026-02-05). `ExprKind` 80→24 bytes, `Expr` 88→32 bytes. Storage split to parallel `Vec<ExprKind>` + `Vec<Span>`. 57 files changed across 9 crates.
- **1.3-1.4 Extra Buffer & Pre-allocation:** Already implemented! `ExprArena` uses flat `Vec` storage with source-based capacity heuristics. `ExprList` eliminated, `ExprRange` used everywhere.
- **4.1 ParseErrorDetails:** Comprehensive error detail structure with `ExtraLabel` for cross-references, `CodeSuggestion` for auto-fixes, and `details()` method on all `ParseErrorKind` variants.

### Phase 3: Enhanced Progress (Medium-risk, High-impact)
**Target: Weeks 11-14** | **Status: ✅ Complete (2026-02-04)**

| Task | Section | Risk | Impact | Status |
|------|---------|------|--------|--------|
| ParseOutcome with context | 3.1 | Medium | High | ✅ Complete |
| one_of! macro | 3.2 | Medium | High | ✅ Complete |
| Context wrapping utilities | 3.4 | Medium | Medium | ✅ Complete |
| Common mistake detection | 4.3 | Low | High | ✅ Complete |

### Phase 4: IDE Support (Medium-risk, High-impact)
**Target: Weeks 15-20** | **Status: ✅ Complete (2026-02-04)**

| Task | Section | Risk | Impact | Status |
|------|---------|------|--------|--------|
| Syntax cursor | 5.1 | Medium | High | ✅ Complete with CursorStats |
| Reusability predicates | 5.2 | Medium | High | ✅ Complete |
| Change range propagation | 5.3 | Medium | High | ✅ Complete (AstCopier) |
| Lazy token capture | 5.4 | Medium | Medium | ✅ Complete (TokenCapture) |
| ModuleExtra structure | 6.1-6.2 | Low | Medium | Not started |

**Phase 4 Summary:**
- **5.1 SyntaxCursor:** Complete with `CursorStats` for performance tracking.
- **5.2 Reusability:** `ChangeMarker::intersects()` handles all cases. All 9 DeclKind variants supported.
- **5.3 Span Adjustment:** `AstCopier` provides deep copy with span adjustment for entire AST.
- **5.4 Lazy Tokens:** `TokenCapture` type with index-based lazy access. Integrated with `ParsedAttrs`.

### Phase 5: Polish (Low-risk, Medium-impact)
**Target: Weeks 21-24** | **Status: ✅ Complete (2026-02-04)**

| Task | Section | Risk | Impact | Status |
|------|---------|------|--------|--------|
| ModuleExtra structure | 6.1 | Low | Medium | ✅ Complete |
| Comment collection | 6.2 | Low | Medium | ✅ Complete |
| Blank line tracking | 6.2 | Low | Medium | ✅ Complete |
| Doc comment attachment | 6.2 | Low | High | ✅ Complete |
| Detached doc comment warnings | 6.4 | Low | Low | ✅ Complete |
| Cross-file error labels | 4.4 | Low | Medium | ✅ Complete |
| SpaceBefore/SpaceAfter | 6.3 | Low | Medium | 🔶 Deferred |
| Performance tuning | All | Low | Medium | 🔶 Deferred |

**Phase 5 Update (2026-02-04):**
- **4.4 Cross-file Labels:** Fully implemented in `ori_diagnostic`. `SourceInfo` type, `Label::*_cross_file()` constructors, `Diagnostic::with_cross_file_*_label()` builders, terminal/JSON/SARIF emitters updated, and `ParseErrorDetails::to_diagnostic()` conversion.
- **6.1 ModuleExtra:** Implemented in `ori_ir/src/metadata.rs`. Stores comments, blank lines, newlines, trailing commas with query methods.
- **6.2 Comment Collection:** `lex_with_comments()` captures comments, blank lines, newlines. `into_parts()` and `into_metadata()` for parser integration.
- **6.2 Doc Comment Attachment:** `ModuleExtra::doc_comments_for()` returns doc comments for a declaration with blank line/regular comment barrier detection.
- **6.4 Detached Warnings:** `ParseWarning::DetachedDocComment` with `DetachmentReason` enum. `ParseOutput::check_detached_doc_comments()` populates warnings.
- **6.3 SpaceBefore/SpaceAfter:** Deferred - requires significant AST changes for marginal benefit. Current `ModuleExtra` suffices for formatters.

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
| Arena allocation | **Excellent** | ExprArena SoA: Vec<ExprKind> + Vec<Span>, 32 bytes/expr |
| Context flags | Good | Bitfield (u16) |
| TokenSet recovery | **Excellent** | 128-bit bitset with O(1) membership |
| Series combinator | Good | Like Gleam |
| Snapshot isolation | Excellent | ~10 bytes |
| Incremental infrastructure | Prepared | Not yet integrated |
| Error hints | Good | Smart suggestions |
| Keyword recognition | **Excellent** | Logos DFA - O(1) equivalent |
| Flat list storage | **Excellent** | ExprRange into flat expr_lists buffer (ExprList removed) |
| Extra data buffer | **Excellent** | expr_lists for variable-length data |
| Pre-allocation | **Good** | ~1 expr per 20 bytes heuristic |

This plan builds on these strengths rather than replacing them.

**Key Finding (2026-02-04):** Many "planned" features were already implemented in Ori. The parser architecture is more mature than initially assessed.

**Migration Complete (2026-02-05):** `ParseOutcome<T>` is now the operational return type for module-level dispatch:
- `parse_module()` and `parse_module_incremental()` use `handle_outcome()` + `with_outcome()`
- 9 `_with_progress` wrappers migrated to `_with_outcome` returning `ParseOutcome<T>`
- `parse_primary()` natively returns `ParseOutcome<ExprId>`
- 12 key grammar functions wrapped with `in_error_context_result()` for "while parsing" context
- Dead code removed: `with_progress`, `handle_parse_result`, `progress_since`, `try_parse!`, `try_result!`
- All 8490 tests pass (unit + spec + LLVM + WASM)
