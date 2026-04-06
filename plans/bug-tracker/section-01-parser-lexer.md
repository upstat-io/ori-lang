---
section: "01"
title: "Parser & Lexer"
status: not-started
goal: "Track and resolve all known parser/lexer bugs"
sections: []
---

# Section 01: Parser & Lexer

**Subsystem:** `compiler/ori_parse/`, `compiler/ori_lexer/`

Bugs in tokenization, parsing, syntax error recovery, AST construction, and grammar handling.

---

## Open Bugs

- [ ] `[BUG-01-001][high]` **Soft keyword cache contamination** — found by impl-hygiene-review.
  Repro: `let cache = 42; cache(key: "x", op: () -> fetch("x"))` — second `cache` lexed as identifier instead of `cache` keyword.
  Subsystem: `compiler/ori_lexer/src/cooker/mod.rs:316-372` — `IdentCache` caches soft keyword text as `Ident` on first non-keyword occurrence; subsequent occurrences in keyword context hit the cache and bypass `soft_keyword_lookup()`.
  Found: 2026-04-05 | Source: impl-hygiene-review
  Note: Active work in `plans/parser-perf/section-03-lexer.md` touches `IdentCache` (perf tuning, not this correctness bug).

---

## Resolved Bugs

- None.
