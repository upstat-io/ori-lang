# Parser 2.0 Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

Quick-reference keyword index for finding Parser 2.0 implementation sections.

---

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file: `section-{ID}-*.md`

---

## Keyword Clusters by Section

### Section 01: Data-Oriented AST
**File:** `section-01-data-oriented-ast.md` | **Status:** Not Started

```
MultiArrayList, SoA, struct of arrays
index-based, u32 indices, node index
cache-friendly, memory layout
extra data buffer, variable-length
pre-allocation, capacity heuristics
Zig parser, data-oriented design
```

---

### Section 02: Lexer Optimizations
**File:** `section-02-lexer.md` | **Status:** Not Started

```
perfect hash, keyword lookup
O(1) keyword, hash table
precedence metadata, operator precedence
scanner optimization, token lookup
Go parser, keyword recognition
```

---

### Section 03: Enhanced Progress System
**File:** `section-03-progress.md` | **Status:** Not Started

```
progress tracking, consumed, empty
backtracking, automatic backtrack
one_of macro, combinator
Elm parser, Roc parser
context capture, error context
ParseOutcome, ConsumedOk, EmptyErr
```

---

### Section 04: Structured Errors
**File:** `section-04-errors.md` | **Status:** Not Started

```
error messages, friendly errors
empathetic, educational
expected token accumulation
ParseErrorDetails, error hints
Elm errors, Gleam errors
common mistakes, suggestions
```

---

### Section 05: Incremental Parsing
**File:** `section-05-incremental.md` | **Status:** Not Started

```
incremental, reparse, reuse
syntax cursor, node reuse
change range, span adjustment
content hash, reusability
TypeScript parser, IDE support
lazy tokens, deferred capture
```

---

### Section 06: Formatting Metadata
**File:** `section-06-metadata.md` | **Status:** Not Started

```
ModuleExtra, comments, whitespace
formatting, lossless roundtrip
doc comments, trivia
SpaceBefore, SpaceAfter
Gleam extra, Roc spaces
formatter support, IDE metadata
```

---

## Quick Reference

| ID | Title | File | Priority |
|----|-------|------|----------|
| 01 | Data-Oriented AST | `section-01-data-oriented-ast.md` | P1 |
| 02 | Lexer Optimizations | `section-02-lexer.md` | P1 |
| 03 | Enhanced Progress System | `section-03-progress.md` | P2 |
| 04 | Structured Errors | `section-04-errors.md` | P1 |
| 05 | Incremental Parsing | `section-05-incremental.md` | P2 |
| 06 | Formatting Metadata | `section-06-metadata.md` | P3 |

---

## Cross-References

| Related Plan | Relevance |
|--------------|-----------|
| `plans/roadmap/section-00-parser.md` | Parser roadmap integration |
| `plans/ori_lsp/` | LSP depends on incremental parsing |
| `plans/roadmap/section-22-tooling.md` | Formatter integration |
