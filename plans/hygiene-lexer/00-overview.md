---
plan: "hygiene-lexer"
title: "Lexer Hygiene: Exhaustive Implementation Plan"
status: not-started
references:
  - ".claude/rules/impl-hygiene.md"
  - ".claude/rules/compiler.md"
---

# Lexer Hygiene: Exhaustive Implementation Plan

## Mission

Achieve cohesive, DRY architecture in `ori_lexer_core` and `ori_lexer` — where the cooker layer has five clusters of algorithmically-duplicated functions sharing identical control-flow skeletons, the scanner has six parallel operator functions that differ only in tag values, and a correctness bug in the identifier cache silently prevents soft keyword resolution. The standard is `.claude/rules/impl-hygiene.md`.

## Mission Success Criteria

- [ ] BUG-01-001 (soft keyword cache contamination) fixed with regression tests — `let cache = 42; cache(key: "x", op: () -> 1)` correctly lexes second `cache` as keyword
- [ ] Template cooking consolidated: 4 functions → 1 generic + 4 call sites
- [ ] Unescape functions consolidated: shared scanning core with context-specific escapes
- [ ] Integer cooking consolidated: 3 functions → 1 generic with radix parameter
- [ ] Duration/size cooking consolidated: 2 cooking + 2 suffix detection functions → generics
- [ ] Simple operator scanning consolidated: 6 functions → shared helper
- [ ] `cook()` match is exhaustive (no `_ =>` catch-all for non-trivial tags)
- [ ] Soft keyword sync guard test exists (SOFT_KEYWORDS ↔ could_be_soft_keyword consistency)
- [ ] Duplicate `span()`/`make_span()` unified to single function
- [ ] `./test-all.sh` green — no regressions
- [ ] `./clippy-all.sh` green
- [ ] All section success criteria met

## Architecture

```
ori_lexer_core (standalone, zero ori_* deps)
├── cursor/          — byte-level scanning
├── raw_scanner/     — token scanning dispatch
│   ├── mod.rs       — main dispatch
│   ├── operators.rs — [Section 03: consolidate 6 simple operators]
│   ├── numbers.rs   — number scanning
│   ├── strings.rs   — string/char scanning
│   └── templates.rs — template string scanning
├── source_buffer/   — source text + encoding detection
└── tag/             — RawTag enum + display

ori_lexer (depends on ori_lexer_core + ori_ir)
├── driver.rs        — main lex loop [Section 04: unify span helper]
├── cooker/          — RawTag → TokenKind cooking
│   ├── mod.rs       — dispatch [Section 01: cache bug] [Section 04: exhaustive match]
│   ├── identifier.rs — ident cache [Section 01: cache bug]
│   ├── escape_cooking.rs — [Section 02: consolidate 4 template fns]
│   ├── numeric.rs   — [Section 02: consolidate 3 int fns]
│   └── duration_size.rs — [Section 02: consolidate 2+2 fns]
├── cook_escape/     — escape processing
│   └── mod.rs       — [Section 02: consolidate 2 unescape fns]
├── keywords/        — keyword lookup [Section 04: sync guard]
├── lex_error/       — error types + factories
├── trivial/         — fast-path operator mapping
└── output.rs        — output types
```

## Design Principles

1. **Algorithmic DRY** — Every multi-step algorithm has exactly one canonical implementation. Variants are parameterized, not copy-pasted. When the protocol changes (e.g., new escape sequence), exactly one function changes.

2. **Correctness first** — The soft keyword cache bug is a real tokenization error. Fix it before any refactoring. All refactoring must be behavior-preserving with comprehensive regression tests.

## Section Dependency Graph

```
Section 01 (Bug Fix)
  │
  ├─── Section 02 (Cooker DRY) ─┐
  │                               │
  ├─── Section 03 (Scanner DRY) ─┼─── Section 05 (Cleanup)
  │                               │
  └─── Section 04 (Drift/Gap) ──┘
```

- Section 01 is first: correctness fix before any refactoring.
- Sections 02, 03, 04 are independent — can be worked in any order after 01.
- Section 05 requires all others complete.

## Implementation Sequence

```
Phase 0 - Bug Fix
  └─ Section 01: Fix soft keyword cache contamination + regression tests

Phase 1 - Algorithmic DRY (independent, any order)
  ├─ Section 02: Cooker layer consolidation (template, unescape, numeric, duration/size)
  ├─ Section 03: Scanner layer consolidation (simple operators)
  └─ Section 04: Drift/gap fixes (exhaustive match, sync guard, span dedup)
  Gate: ./test-all.sh green, ./clippy-all.sh green

Phase 2 - Verification & Cleanup
  └─ Section 05: Final verification + plan deletion
```

**Why this order:**
- Phase 0 is a correctness fix — must land first and independently.
- Phase 1 items are independent refactorings — no shared state or ordering constraints.
- Phase 2 is verification — all code changes must be complete first.

## Metrics (Current State)

| Crate | Production LOC | Test LOC | Total |
|-------|---------------|----------|-------|
| `ori_lexer_core` | ~2,655 | ~2,346 | ~4,875 |
| `ori_lexer` | ~3,722 | ~2,805 | ~6,527 |
| **Total** | **~6,377** | **~5,151** | **~11,402** |

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Depends On |
|---------|-------------------|------------|------------|
| 01 Bug Fix | ~30 | Low | — |
| 02 Cooker DRY | ~200 | Medium | 01 |
| 03 Scanner DRY | ~80 | Low | 01 |
| 04 Drift/Gap | ~60 | Low | 01 |
| 05 Cleanup | ~5 | Low | 01-04 |
| **Total changed** | **~375** | | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| BUG-01-001: Soft keyword cache contamination | IdentCache caches soft keyword text as Ident on first non-keyword use; cache hit bypasses soft keyword check | Section 01 | Not Started |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Bug Fix — Soft Keyword Cache | `section-01-soft-keyword-bug.md` | Not Started |
| 02 | Cooker Layer Algorithmic DRY | `section-02-cooker-dry.md` | Not Started |
| 03 | Scanner Layer Algorithmic DRY | `section-03-scanner-dry.md` | Not Started |
| 04 | Drift, Gap & Polish | `section-04-drift-gap-polish.md` | Not Started |
| 05 | Cleanup | `section-05-cleanup.md` | Not Started |
