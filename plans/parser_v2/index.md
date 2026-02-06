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
**File:** `section-01-data-oriented-ast.md` | **Status:** ✅ Complete (SoA migration, 2026-02-05)

```
MultiArrayList, SoA, struct of arrays
index-based, u32 indices, ExprId, FunctionSeqId, FunctionExpId, BindingPatternId
cache-friendly, memory layout, parallel arrays
extra data buffer, ExprRange, expr_lists
pre-allocation, capacity heuristics
Zig parser, data-oriented design
ExprKind 24 bytes, Expr 32 bytes, Copy semantics
sentinel pattern, ExprId::INVALID, is_present
```

---

### Section 02: Lexer Modernization
**File:** `section-02-lexer.md` | **Status:** In Progress (02.1-02.4 satisfied; 02.5-02.10 not started)

```
perfect hash, keyword lookup, logos DFA
precedence metadata, satisfied by parser
adjacent token, compound operator synthesis
HashBracket removal, simplified attributes
decimal duration, decimal size, compile-time sugar
doc comments, CommentKind, DocMember
template strings, string interpolation, backtick
TokenList SoA, cache locality, split storage
GtEq, Shr, dead token audit
phase separation, context-free lexer
```

---

### Section 03: Enhanced Progress System
**File:** `section-03-progress.md` | **Status:** ✅ Complete (infrastructure — see Section 07 for full adoption)

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

### Section 07: Full ParseOutcome Migration
**File:** `section-07-parseoutcome-migration.md` | **Status:** ✅ Complete (2026-02-06)

```
ParseOutcome migration, native adoption
one_of! macro, try_outcome!, require!, chain!, committed!
Result to ParseOutcome, grammar conversion
with_outcome removal, wrapper elimination
_inner pattern, collapse indirection
ConsumedOk, EmptyOk, ConsumedErr, EmptyErr
soft error, hard error, backtracking
primary expressions, Pratt loop, postfix
item declarations, type parsing, generics
in_error_context_result removal
parse_X_inner collapse, _with_outcome removal
one_of! dispatch, parse_primary, parse_match_pattern_base
TokenSet guards, EmptyErr guards, snapshot/restore
181 macro uses, 13 alternatives, 7 alternatives
```

---

## Quick Reference

| ID | Title | File | Priority | Status |
|----|-------|------|----------|--------|
| 01 | Data-Oriented AST | `section-01-data-oriented-ast.md` | P1 | ✅ Complete (SoA, 64% reduction) |
| 02 | Lexer Modernization | `section-02-lexer.md` | P1 | In Progress |
| 03 | Enhanced Progress System | `section-03-progress.md` | P2 | ✅ Complete (infrastructure) |
| 04 | Structured Errors | `section-04-errors.md` | P1 | ✅ Complete |
| 05 | Incremental Parsing | `section-05-incremental.md` | P2 | ✅ Complete |
| 06 | Formatting Metadata | `section-06-metadata.md` | P3 | ✅ Complete |
| 07 | Full ParseOutcome Migration | `section-07-parseoutcome-migration.md` | P1 | ✅ Complete (2026-02-06) |

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

**Skip benchmarks** for: progress system (03), error messages (04), incremental parsing (05), metadata (06), ParseOutcome migration (07).

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
