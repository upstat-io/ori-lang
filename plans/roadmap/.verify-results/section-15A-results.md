# Section 15A Verification Results: Attributes & Comments

**Verified**: 2026-03-19
**Section status**: 0/93 (0%) -- not-started
**Sampling strategy**: Spot-checked 8 unchecked items to confirm genuinely not implemented

---

## Unchecked Items Sampled (confirming incomplete)

### 15A.1 Simplified Attribute Syntax

| Item | Status | Evidence |
|------|--------|----------|
| Update lexer to emit `Hash` token instead of `HashBracket` | VERIFIED INCOMPLETE | `HashBracket` references still exist in `ori_lexer/src/trivial/mod.rs`, `trivial/tests.rs`, `cooker/tests.rs`. Current lexer uses bracket syntax. |
| Update parser to parse `#name(...)` syntax | VERIFIED INCOMPLETE | No `tests/spec/attributes/` directory exists. Current test files still use `#[derive(...)]` and `#[compile_fail(...)]` bracket syntax (53 occurrences across 5 test files in `tests/compiler/typeck/`). |
| Support migration: accept both syntaxes temporarily | VERIFIED INCOMPLETE | No migration tests or code found. |

### 15A.2 function_seq vs function_exp Formalization

| Item | Status | Evidence |
|------|--------|----------|
| Verify AST has separate `FunctionSeq` and `FunctionExp` types | VERIFIED INCOMPLETE (partially exists) | `FunctionSeq` and `FunctionExp` variants exist in `ori_ir` (17 files reference them), but the formalization work (distinguishing and enforcing) is incomplete. No `tests/spec/patterns/function_seq_exp.ori` exists. |

### 15A.3 Inline Comments Prohibition

| Item | Status | Evidence |
|------|--------|----------|
| Update lexer to reject inline comments | VERIFIED INCOMPLETE | No `tests/compile-fail/inline_comments.ori` exists. Lexer does not reject inline comments. |
| Add clear error message for inline comments | VERIFIED INCOMPLETE | No error message infrastructure for this. |

### 15A.4 Simplified Doc Comment Syntax

| Item | Status | Evidence |
|------|--------|----------|
| Update `CommentKind` enum | VERIFIED INCOMPLETE | No `tests/spec/comments/` directory exists. |
| Update lexer comment classification | VERIFIED INCOMPLETE | No new comment classification code found. |

---

## Summary

All sampled items are genuinely not implemented. The 0% status is accurate.

**NOTE**: Some items have partial infrastructure (e.g., `FunctionSeq`/`FunctionExp` AST nodes exist but formalization is incomplete). Current test files use `#derive(...)` (new syntax) alongside `#[compile_fail(...)]` (old bracket syntax), indicating partial organic migration but no systematic implementation.

**Accuracy**: Section status is CORRECT at 0%.
