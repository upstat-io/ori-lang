---
section: "01"
title: "Parser & Lexer"
status: in-progress
goal: "Track and resolve all known parser/lexer bugs"
sections: []
---

# Section 01: Parser & Lexer

**Subsystem:** `compiler/ori_parse/`, `compiler/ori_lexer/`

Bugs in tokenization, parsing, syntax error recovery, AST construction, and grammar handling.

---

## Open Bugs

- [ ] `[BUG-01-002][medium]` **Parser: `impl<T>` method-level generics `@map<U> (self, f: T -> U) -> Box<U>` rejected with `expected (, found <`**
  Repro: `impl<T> Box<T> { @map<U> (self, f: T -> U) -> Box<U> = ... }` — parser rejects method-level generic parameters with `expected (, found <`. No grammar production exists for method-level generics on inherent impl methods; the method-header rule accepts only the `@name (params) -> ret` shape, with no `<generics>` slot between the method name and the parameter list.
  Subsystem: `compiler/ori_parse/` (method-header grammar in inherent / trait impl blocks)
  Test case: `compiler/ori_llvm/tests/aot/fixtures/generics/` fixture + `compiler/ori_llvm/tests/aot/generics.rs::test_generic_method_on_generic_type` (currently `#[ignore]`; will green when this bug AND `BUG-04-091` both land — see Related).
  Related spec drift (separate concern): `docs/ori_lang/v2026/spec/grammar.ebnf:311` writes `inherent_impl = "impl" [ generics ] type_path ...` where `type_path` is dotted identifiers only (no `type_args` per line 341), but the shipped parser accepts a more permissive form (e.g. `impl<T> Box<T>`). Tracked separately as `BUG-08-015`; this bug covers method-level generics, not the impl-header drift.
  Related: `BUG-04-091` — the same ignored test `test_generic_method_on_generic_type` ALSO fails on the reduced, no-method-generics shape (just `@unwrap (self) -> T` on `impl<T> Box<T>`), which is a codegen gap. The test blocks on BOTH bugs.
  Found: 2026-04-21 | Source: manual (close-out of `plans/empty-container-typeck-phase-contract/section-04-codegen-assertions.md §04.2.B`)

---

## Resolved Bugs

- [x] `[BUG-01-001][high]` **Soft keyword cache contamination** — found by impl-hygiene-review.
  Repro: `let cache = 42; cache(key: "x", op: () -> fetch("x"))` — second `cache` lexed as identifier instead of `cache` keyword.
  Subsystem: `compiler/ori_lexer/src/cooker/mod.rs:316-372` — `IdentCache` caches soft keyword text as `Ident` on first non-keyword occurrence; subsequent occurrences in keyword context hit the cache and bypass `soft_keyword_lookup()`.
  Found: 2026-04-05 | Source: impl-hygiene-review
  Resolved: 2026-04-05 | Fix: Guard `ident_cache.insert()` with `!could_be_soft_keyword(text)` — soft keyword candidates are never cached, forcing re-evaluation every occurrence. Tests: `soft_keyword_ident_then_keyword_all` (semantic pin), 6 matrix tests × 4 orderings, negative pins for hard keyword and regular identifier caching.
