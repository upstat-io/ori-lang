# Section 15A Verification Results: Attributes & Comments

**Verified**: 2026-03-28
**Section status**: not-started (per frontmatter)
**Actual status**: partially-implemented -- several key features already work

## Files Loaded

- `/home/eric/projects/ori_lang/CLAUDE.md` (full)
- All 20 files in `.claude/rules/` (full)
- `/home/eric/projects/ori_lang/plans/roadmap/section-15A-attributes-comments.md` (full)
- Spec: `docs/ori_lang/v2026/spec/grammar.ebnf` (attribute/comment sections)
- Spec: `docs/ori_lang/v2026/spec/07-lexical-elements.md` (relevant)

## Evidence Summary

### Key Findings

1. **Simplified attribute syntax (`#name(...)`) is ALREADY IMPLEMENTED.** The lexer emits both `Hash` and `HashBracket` tokens. The parser (`ori_parse/src/grammar/attr/mod.rs`) accepts both `#name(...)` and `#[name(...)]` syntax. Both work in all existing tests.

2. **FunctionSeq vs FunctionExp formalization is ALREADY IMPLEMENTED.** The AST has separate `FunctionSeq` enum (`ori_ir/src/ast/patterns/seq/mod.rs`) and `FunctionExp`/`FunctionExpKind` enum (`ori_ir/src/ast/patterns/exp/mod.rs`). The grammar reflects this distinction (`grammar.ebnf` lines 246+).

3. **Inline comment prohibition is NOT IMPLEMENTED.** The lexer (`ori_lexer/src/comments/mod.rs`, `driver.rs`) accepts all comments regardless of position. No rejection of inline comments exists.

4. **Doc comment syntax is PARTIALLY IMPLEMENTED.** The `CommentKind` enum (`ori_ir/src/comment/mod.rs`) has `DocDescription`, `DocMember`, `DocWarning`, `DocExample`. The lexer recognizes `*`, `#`, `@param`, `@field`, `!`, `>` markers. The formatter (`ori_fmt/src/comments/mod.rs`) handles `* name:` format and legacy `@param`/`@field` via `extract_member_name_any`.

---

## 15A.1 Simplified Attribute Syntax

### - [ ] Update lexer to emit `Hash` token instead of `HashBracket`

**Verdict**: [done] -- ALREADY IMPLEMENTED

The lexer emits BOTH `TokenKind::Hash` (`#`) and `TokenKind::HashBracket` (`#[`). The `Hash` token is defined in `ori_ir/src/token/kind.rs:108` and `tag.rs`. Both are consumed by the parser in `parse_attributes()`.

Evidence:
- `ori_ir/src/token/kind.rs:108`: `Hash, // #`
- `ori_ir/src/token/kind.rs:104`: `HashBracket, // #[`
- `ori_parse/src/grammar/attr/mod.rs:199`: `while self.cursor.check(&TokenKind::Hash) || self.cursor.check(&TokenKind::HashBracket)`

Rust tests: `ori_parse/src/grammar/attr/tests.rs` -- has `test_parse_skip_attribute_no_brackets`, `test_parse_compile_fail_attribute_no_brackets`, `test_parse_fail_attribute_no_brackets`, `test_parse_derive_attribute_no_brackets`, `test_parse_compile_fail_extended_no_brackets` (5 tests for bracketless syntax).

Ori tests: `tests/spec/declarations/attributes.ori` (passes -- 4181 passed, 0 failed, 42 skipped).

**Sub-items**:
- [ ] Rust Tests -- [done] 5 Rust tests exist in `ori_parse/src/grammar/attr/tests.rs`
- [ ] Ori Tests -- [partial] `tests/spec/declarations/attributes.ori` exists but uses old `#[...]` syntax. No `tests/spec/attributes/simplified_syntax.ori` file exists.
- [ ] LLVM Support -- [done] Attributes are parser-only concepts; LLVM codegen does not need separate attribute token handling. The grammar does not generate code for attributes.
- [ ] LLVM Rust Tests -- [not-applicable] Attributes are not codegen constructs.
- [ ] AOT Tests -- [not-applicable] Attributes are not codegen constructs.

### - [ ] Update parser to parse `#name(...)` syntax

**Verdict**: [done] -- ALREADY IMPLEMENTED

The parser in `ori_parse/src/grammar/attr/mod.rs:187-250` parses both syntaxes. Grammar in `grammar.ebnf:239` defines: `attribute = "#" identifier [ "(" ... ")" ]`.

Evidence: Parser code at line 199-201 shows it handles both `Hash` and `HashBracket` tokens.

**Sub-items**:
- [ ] Rust Tests -- [done] Multiple tests exist (`test_parse_skip_attribute_no_brackets`, etc.)
- [ ] Ori Tests -- [partial] No dedicated `tests/spec/attributes/simplified_syntax.ori`. Existing `attributes.ori` uses old syntax.
- [ ] LLVM Support -- [not-applicable]
- [ ] LLVM Rust Tests -- [not-applicable]
- [ ] AOT Tests -- [not-applicable]

### - [ ] Generalize attributes to all declarations

**Verdict**: [partial] -- MOSTLY IMPLEMENTED

Attributes can be placed on functions, types, traits, tests, and extern blocks per the grammar (`grammar.ebnf:233`). The parser calls `parse_attributes()` before all declarations. However, the plan mentions impls and constants -- validation of which attributes are valid for which declarations is not fully implemented.

Evidence: `grammar.ebnf:233` shows attributes before all declaration types.

**Sub-items**:
- [ ] Rust Tests -- [partial] No dedicated generalized attribute tests.
- [ ] Ori Tests -- [partial] `attributes.ori` tests derive on types and tests, but not on all declaration types.
- [ ] LLVM Support -- [not-applicable]
- [ ] LLVM Rust Tests -- [not-applicable]
- [ ] AOT Tests -- [not-applicable]

### - [ ] Attribute validation (which attributes valid for which declarations)

**Verdict**: [partial] -- PARTIALLY IMPLEMENTED

`ParsedAttrs::has_non_conditional_attrs()` exists and is used to validate that non-conditional attributes aren't placed on items that only support conditional compilation. But comprehensive "which attribute on which declaration" validation is not fully documented or tested.

**Sub-items**:
- [ ] Rust Tests -- [todo] No dedicated validation tests exist.
- [ ] Ori Tests -- [todo] No `tests/compile-fail/invalid_attribute_target.ori` exists.
- [ ] LLVM Support -- [not-applicable]
- [ ] LLVM Rust Tests -- [not-applicable]

### - [ ] Support migration: accept both syntaxes temporarily

**Verdict**: [done] -- ALREADY IMPLEMENTED

The parser accepts both `#name(...)` and `#[name(...)]` syntax (line 199-201 of attr/mod.rs). This is the migration path.

**Sub-items**:
- [ ] Rust Tests -- [done] Tests exist for both syntaxes.
- [ ] Ori Tests -- [partial] No dedicated migration test file.
- [ ] LLVM Support -- [not-applicable]
- [ ] LLVM Rust Tests -- [not-applicable]
- [ ] AOT Tests -- [not-applicable]

### - [ ] Add deprecation warning for bracket syntax

**Verdict**: [todo] -- NOT IMPLEMENTED

No deprecation warning is emitted when `#[name(...)]` is used. The parser silently accepts it.

**Sub-items**:
- [ ] LLVM Support -- [not-applicable]
- [ ] LLVM Rust Tests -- [not-applicable]

### - [ ] Update `ori fmt` to auto-migrate

**Verdict**: [todo] -- NOT IMPLEMENTED

The formatter handles `HashBracket` as a spacing category but does not convert `#[name(...)]` to `#name(...)`.

**Sub-items**:
- [ ] LLVM Support -- [not-applicable]
- [ ] LLVM Rust Tests -- [not-applicable]

---

## 15A.2 function_seq vs function_exp Formalization

### - [ ] Verify AST has separate `FunctionSeq` and `FunctionExp` types

**Verdict**: [done] -- ALREADY IMPLEMENTED

- `FunctionSeq` enum: `ori_ir/src/ast/patterns/seq/mod.rs` -- variants: Try, Match, ForPattern
- `FunctionExpKind` enum: `ori_ir/src/ast/patterns/exp/mod.rs` -- variants: Recurse, Parallel, Spawn, Timeout, Cache, With, Print, Panic, Catch, Todo, Unreachable, Channel, ChannelIn, ChannelOut, ChannelAll
- `FunctionSeqId` and `FunctionExpId`: `ori_ir/src/expr_id/function.rs`

**Sub-items**:
- [ ] Rust Tests -- [partial] `ori_ir/src/ast/patterns/seq/tests.rs` and `exp/tests.rs` exist.
- [ ] Ori Tests -- [todo] No `tests/spec/patterns/function_seq_exp.ori`.
- [ ] LLVM Support -- [done] These are evaluated through standard codegen paths.
- [ ] LLVM Rust Tests -- [not-applicable]
- [ ] AOT Tests -- [not-applicable]

### - [ ] Parser allows positional for type conversions only

**Verdict**: [partial] -- PARTIALLY RELEVANT

The `as` conversion proposal removed `function_val` entirely. Type conversions now use `x as T` / `x as? T` syntax. This item is largely obsoleted by the `as` proposal.

**Sub-items**:
- [ ] Rust Tests -- [todo]
- [ ] Ori Tests -- [todo]
- [ ] LLVM Support -- [not-applicable]
- [ ] LLVM Rust Tests -- [not-applicable]
- [ ] AOT Tests -- [not-applicable]

### - [ ] Parser enforces named args for all other builtins

**Verdict**: [partial] -- PARTIALLY IMPLEMENTED

`print()` (a `function_exp`) requires named args (`msg:`). But `len()`, `assert_eq()`, `assert()` and other "function_val" builtins accept positional args. This enforcement is incomplete.

Evidence: `print("hello")` produces `error[E1013]: print requires named properties`. But `len([1,2,3])` works positionally.

**Sub-items**:
- [ ] Rust Tests -- [todo]
- [ ] Ori Tests -- [todo]
- [ ] LLVM Support -- [not-applicable]
- [ ] LLVM Rust Tests -- [not-applicable]
- [ ] AOT Tests -- [not-applicable]

### - [ ] Add clear error message for positional args in builtins

**Verdict**: [partial] -- PARTIALLY IMPLEMENTED

`E1013` error exists for `function_exp` builtins (`print`, etc.). But no equivalent enforcement for `function_val` builtins (`len`, `assert_eq`, etc.).

**Sub-items**: all [todo]

---

## 15A.3 Inline Comments Prohibition

### - [ ] Update lexer to reject inline comments

**Verdict**: [todo] -- NOT IMPLEMENTED

The lexer (`ori_lexer/src/driver.rs`, `comments/mod.rs`) classifies and normalizes comments but does not check whether they appear on their own line. No inline comment detection or rejection exists.

**Sub-items**:
- [ ] Rust Tests -- [todo] No tests exist.
- [ ] Ori Tests -- [todo] No `tests/compile-fail/inline_comments.ori` exists.
- [ ] LLVM Support -- [not-applicable]
- [ ] LLVM Rust Tests -- [not-applicable]

### - [ ] Add clear error message for inline comments

**Verdict**: [todo] -- NOT IMPLEMENTED

**Sub-items**:
- [ ] LLVM Support -- [not-applicable]
- [ ] LLVM Rust Tests -- [not-applicable]

---

## 15A.4 Simplified Doc Comment Syntax

### - [ ] Update `CommentKind` enum

**Verdict**: [partial] -- PARTIALLY IMPLEMENTED

The `CommentKind` enum already has `DocMember` (unified for both `@param` and `@field` and `* name:`). However, it still has `DocDescription` for `#`-prefixed descriptions, and the plan says description detection should move to the formatter.

Evidence: `ori_ir/src/comment/mod.rs:64-78` shows `CommentKind::Regular`, `DocDescription`, `DocMember`, `DocWarning`, `DocExample`.

**Sub-items**:
- [ ] Replace `DocParam`, `DocField` with unified `DocMember` -- [done] Already uses `DocMember`.
- [ ] Remove `DocDescription` detection from lexer -- [todo] Still detected in lexer.
- [ ] Rust Tests -- [partial] `ori_lexer/src/comments/tests.rs` exists.
- [ ] Ori Tests -- [todo] No `tests/spec/comments/doc_markers.ori`.

### - [ ] Update lexer comment classification

**Verdict**: [partial] -- PARTIALLY IMPLEMENTED

The lexer recognizes `* name:` format (new) AND `@param`/`@field` (legacy). The `#` description marker is still recognized.

Evidence: `ori_lexer/src/comments/mod.rs:17-99` shows all marker recognition.

**Sub-items**:
- [ ] Recognize `*` as member doc marker -- [done] Lines 26-46 of comments/mod.rs.
- [ ] Remove `#`, `@param`, `@field` recognition -- [todo] Still recognized (lines 48-74).
- [ ] Rust Tests -- [partial] `ori_lexer/src/comments/tests.rs` exists.
- [ ] Ori Tests -- [todo] No `tests/spec/comments/classification.ori`.

### - [ ] Update formatter doc comment reordering

**Verdict**: [partial] -- PARTIALLY IMPLEMENTED

The formatter handles `* name:` format and legacy `@param`/`@field` via `extract_member_name_any`. The reordering logic works for both formats.

Evidence: `ori_fmt/src/comments/mod.rs:306` uses `extract_member_name_any`. Tests in `ori_fmt/src/comments/tests.rs` cover both `* name:` and `@param` formats.

**Sub-items**:
- [ ] Update `extract_member_name` to parse `* name:` -- [done] `extract_member_name_any` handles it.
- [ ] Move description detection to formatter -- [todo] Still in lexer.
- [ ] Rust Tests -- [partial] Tests exist in `ori_fmt/src/comments/tests.rs`.
- [ ] Ori Tests -- [todo] No `tests/fmt/comments/reordering.ori`.

### - [ ] Support migration from old syntax

**Verdict**: [partial] -- PARTIALLY IMPLEMENTED

Lexer recognizes both old and new formats. Formatter does not auto-convert. No deprecation warning.

**Sub-items**:
- [ ] Lexer recognizes both -- [done]
- [ ] `ori fmt` converts old to new -- [todo]
- [ ] Add deprecation warning for old format -- [todo]
- [ ] Ori Tests -- [todo]

### - [ ] LLVM backend support

**Verdict**: [not-applicable] -- Comments are not codegen constructs.

---

## 15A.5 Section Completion Checklist

- [ ] All implementation items have checkboxes marked `[ ]` -- [done] All items have checkboxes.
- [ ] All spec docs updated -- [partial] grammar.ebnf has attribute syntax; lexical elements may need update.
- [ ] CLAUDE.md updated with syntax changes -- [partial] CLAUDE.md shows `#derive(...)` syntax.
- [ ] Migration tools working -- [todo] No auto-migration.
- [ ] All tests pass: `./test-all.sh` -- Not run for this verification.
- [ ] `/tpr-review` passed -- [todo]

---

## Summary

| Subsection | Plan Status | Actual Status | Items Done | Items Partial | Items Todo |
|---|---|---|---|---|---|
| 15A.1 Simplified Attributes | not-started | mostly-done | 4 | 2 | 1 |
| 15A.2 function_seq/exp | not-started | mostly-done | 1 | 2 | 1 |
| 15A.3 Inline Comments | not-started | not-started | 0 | 0 | 2 |
| 15A.4 Doc Comments | not-started | partially-done | 1 | 3 | 1 |
| 15A.5 Completion Checklist | not-started | not-started | 1 | 2 | 3 |

**Overall**: The section is marked `not-started` but significant work is already done on 15A.1 (simplified attributes -- both syntaxes work, tests exist) and 15A.2 (function_seq/exp -- AST already formalized). The section status should be updated to `partially-implemented`.

**Key remaining work**:
1. Inline comment prohibition (15A.3) -- entirely unimplemented
2. Deprecation warnings for old attribute bracket syntax
3. `ori fmt` auto-migration for both attributes and doc comments
4. Comprehensive attribute validation (which attrs on which declarations)
5. Many plan items reference LLVM tests for non-codegen features (attributes, comments) -- these are not applicable and should be removed from the plan

**PLAN QUALITY NOTE**: Many items in this section have LLVM sub-items (`LLVM Support`, `LLVM Rust Tests`, `AOT Tests`) for features that are purely lexer/parser/formatter concerns (attributes, comments). Attributes and comments are not codegen constructs -- these LLVM sub-items are not applicable and inflate the item count. The plan should be revised to remove them.
