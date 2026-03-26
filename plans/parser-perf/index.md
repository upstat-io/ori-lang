---
reroute: true
name: "Parser Perf"
full_name: "Parser Frontend Performance & API"
status: queued
order: 3
---

# Parser Frontend Performance Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: Performance Baselines
**File:** `section-01-baselines.md` | **Status:** Not Started

```
baseline, benchmark, throughput, criterion, performance measurement
lexer_core, lexer, parser, raw, salsa, incremental
MiB/s, tokens/second, bytes/second, regression detection
cargo bench, lexer_core/raw/throughput, parser/raw
CompilerDb, SourceFile, black_box, Throughput::Bytes
```

---

### Section 02: File Hygiene
**File:** `section-02-hygiene.md` | **Status:** Not Started

```
file split, hygiene, bloat, 500-line limit, refactor
lib.rs, copier.rs, kind.rs, cursor.rs, outcome.rs, match_patterns.rs, function/mod.rs
arena/mod.rs, ast/expr.rs, query/mod.rs, decorative banners
Parser, Cursor, ParseOutcome, ParseContext, KnownNames
parser_driver, parser_state, parse_module, error_handling
incremental, SyntaxCursor, ChangeMarker, deep_copy_expr
ParseErrorKind, ErrorContext, ParseExpected
```

---

### Section 03: Lexer Optimizations
**File:** `section-03-lexer.md` | **Status:** Not Started

```
lexer, tokenizer, inline, #[inline], cross-crate, hot path
TokenCooker, CookResult, IdentCache, try_trivial, lex_driver
RawScanner, RawTag, SourceBuffer, cook_ident, cook_int
LexOutput, LexResult, TokenList, with_capacity, pre-allocation
slice_source, from_utf8_unchecked, finalize_flags, make_span
TokenFlags, SPACE_BEFORE, NEWLINE_BEFORE, ADJACENT, IS_DOC
ori_lexer_core, ori_lexer, driver.rs, cooker/mod.rs
```

---

### Section 04: Parser Optimizations
**File:** `section-04-parser.md` | **Status:** Not Started

```
parser, expression, pratt, binding power, inline
parse_expr, parse_binary_pratt, parse_unary, infix_binding_power
OPER_TABLE, OperInfo, define_operators!, compound_assign_op
Cursor, advance, check, check_tag, current_tag, expect
ExprArena, alloc_expr, with_capacity, arena pre-allocation
ParseOutcome, ConsumedOk, EmptyOk, ConsumedErr, EmptyErr
ParserSnapshot, snapshot, restore, speculative parsing
parse_call, apply_postfix_ops, parse_primary, parse_range
ori_parse, grammar/expr/mod.rs, grammar/expr/operators.rs
cursor/mod.rs, outcome/mod.rs, snapshot/mod.rs
```

---

### Section 05: Salsa Integration
**File:** `section-05-salsa.md` | **Status:** Not Started

```
salsa, query, incremental, caching, early cutoff, overhead
lex_result, tokens, parsed, typed, tokens_with_metadata
CompilerDb, SourceFile, Db, salsa::tracked
PoolCache, CanonCache, ImportsCache, CacheGuard, ParseCache
incremental parsing, parse_incremental, TextChange, compute_text_change
SyntaxCursor, ChangeMarker, copier, deep_copy_expr
query granularity, per-function, invalidation cascade
oric/src/query/mod.rs, ori_parse/incremental/
```

---

### Section 06: Verification
**File:** `section-06-verification.md` | **Status:** Not Started

```
verification, benchmark, regression, throughput comparison
criterion, baseline comparison, before/after, improvement
test-all.sh, clippy-all.sh, cargo bench
lexer throughput, parser throughput, salsa overhead
code journey, dual-exec, behavioral equivalence
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Performance Baselines | `section-01-baselines.md` |
| 02 | File Hygiene | `section-02-hygiene.md` |
| 03 | Lexer Optimizations | `section-03-lexer.md` |
| 04 | Parser Optimizations | `section-04-parser.md` |
| 05 | Salsa Integration | `section-05-salsa.md` |
| 06 | Verification | `section-06-verification.md` |
