---
section: "01"
title: "Parser & Lexer"
status: complete
goal: "Track and resolve all known parser/lexer bugs"
sections: []
---

# Section 01: Parser & Lexer

**Subsystem:** `compiler/ori_parse/`, `compiler/ori_lexer/`

Bugs in tokenization, parsing, syntax error recovery, AST construction, and grammar handling.

---

## Open Bugs

- None.

---

## Resolved Bugs

- [x] `[BUG-01-001][high]` **Soft keyword cache contamination** — found by impl-hygiene-review.
  Repro: `let cache = 42; cache(key: "x", op: () -> fetch("x"))` — second `cache` lexed as identifier instead of `cache` keyword.
  Subsystem: `compiler/ori_lexer/src/cooker/mod.rs:316-372` — `IdentCache` caches soft keyword text as `Ident` on first non-keyword occurrence; subsequent occurrences in keyword context hit the cache and bypass `soft_keyword_lookup()`.
  Found: 2026-04-05 | Source: impl-hygiene-review
  Resolved: 2026-04-05 | Fix: Guard `ident_cache.insert()` with `!could_be_soft_keyword(text)` — soft keyword candidates are never cached, forcing re-evaluation every occurrence. Tests: `soft_keyword_ident_then_keyword_all` (semantic pin), 6 matrix tests × 4 orderings, negative pins for hard keyword and regular identifier caching.
