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
**File:** `section-01-data-oriented-ast.md` | **Status:** In Progress (01.1-01.2 analysis complete)

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
**File:** `section-02-lexer.md` | **Status:** ✅ Complete (logos already optimal)

```
perfect hash, keyword lookup
O(1) keyword, hash table
precedence metadata, operator precedence
scanner optimization, token lookup
Go parser, keyword recognition
```

---

### Section 03: Enhanced Progress System
**File:** `section-03-progress.md` | **Status:** ✅ Complete

```
progress tracking, consumed, empty
backtracking, automatic backtrack
one_of macro, try_outcome, require, chain
Elm parser, Roc parser
context capture, error context
ParseOutcome, ConsumedOk, EmptyErr
ErrorContext, in_error_context, with_error_context
with_outcome, handle_outcome, module dispatch
in_error_context_result, while parsing
ParseResult migration, _with_outcome wrappers
```

---

### Section 04: Structured Errors
**File:** `section-04-errors.md` | **Status:** ✅ Complete

```
error messages, friendly errors
empathetic, educational
expected token accumulation
ParseErrorDetails, error hints, details()
ExtraLabel, CodeSuggestion, Applicability
Elm errors, Gleam errors
common mistakes, suggestions
detect_common_mistake, educational_note
```

---

### Section 05: Incremental Parsing
**File:** `section-05-incremental.md` | **Status:** ✅ Complete

```
incremental, reparse, reuse
syntax cursor, node reuse
change range, span adjustment
content hash, reusability
TypeScript parser, IDE support
lazy tokens, deferred capture
SyntaxCursor, AstCopier, ChangeMarker
TokenCapture, token_range, get_range
parse_incremental, parse_module_incremental
```

---

### Section 06: Formatting Metadata
**File:** `section-06-metadata.md` | **Status:** ✅ Complete

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

| ID | Title | File | Priority | Status |
|----|-------|------|----------|--------|
| 01 | Data-Oriented AST | `section-01-data-oriented-ast.md` | P1 | 🔶 Deferred (already efficient) |
| 02 | Lexer Optimizations | `section-02-lexer.md` | P1 | ✅ Complete |
| 03 | Enhanced Progress System | `section-03-progress.md` | P2 | ✅ Complete |
| 04 | Structured Errors | `section-04-errors.md` | P1 | ✅ Complete |
| 05 | Incremental Parsing | `section-05-incremental.md` | P2 | ✅ Complete |
| 06 | Formatting Metadata | `section-06-metadata.md` | P3 | ✅ Complete |

---

## Performance Validation

### Quick Check

Use the `/benchmark` skill for quick validation:

```bash
/benchmark short   # ~30s, sanity check
/benchmark medium  # ~2min, standard validation
```

### When to Benchmark

Run `/benchmark short` after modifying:
- Data-oriented AST (Section 01) — if implemented
- Lexer optimizations (Section 02)

**Skip benchmarks** for: progress system (03), error messages (04), incremental parsing (05), metadata (06).

### Current Performance (February 2026)

| Metric | Throughput | Status |
|--------|------------|--------|
| Parser raw | ~120 MiB/s | ✅ Meets target |
| Lexer raw | ~270 MiB/s | ✅ Exceeds target |

See `.claude/skills/benchmark/baselines.md` for detailed targets.

---

## Cross-References

| Related Plan | Relevance |
|--------------|-----------|
| `plans/roadmap/section-00-parser.md` | Parser roadmap integration |
| `plans/ori_lsp/` | LSP depends on incremental parsing |
| `plans/roadmap/section-22-tooling.md` | Formatter integration |
