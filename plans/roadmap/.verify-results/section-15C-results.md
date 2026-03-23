# Section 15C Verification Results: Literals & Operators

**Verified**: 2026-03-19
**Section status**: 0/484 (0%) -- not-started
**Sampling strategy**: Spot-checked 12 unchecked items; found several with partial or complete implementations

---

## Unchecked Items Sampled

### 15C.1 String Interpolation (Template Strings)

| Item | Status | Evidence |
|------|--------|----------|
| Add template string literal tokenization (backtick delimited) | PLAN INACCURACY | IMPLEMENTED. `ori_lexer_core/src/raw_scanner/templates.rs` has full template literal scanning with `TemplateHead`, `TemplateTail`, `TemplateComplete` tags. SIMD-accelerated `skip_to_template_delim()`. Should be checked `[x]`. |
| Handle `{expr}` interpolation boundaries | PLAN INACCURACY | IMPLEMENTED. Scanner handles `{` as interpolation start, manages `template_depth` stack, handles `{{` escape. Should be checked `[x]`. |
| Handle `{{` and `}}` escape for literal braces | PLAN INACCURACY | IMPLEMENTED. Line 29-33 of templates.rs: `if self.cursor.peek() == b'{' { ... continue; }`. Should be checked `[x]`. |

### 15C.5 Floor Division (`div`) Operator

| Item | Status | Evidence |
|------|--------|----------|
| Add `TokenKind::Div` case to `match_multiplicative_op()` | NEEDS VERIFICATION | `FloorDiv` exists in `BinaryOp` enum in `ori_ir/src/ast/operators.rs`. The parser likely handles it already but no `tests/spec/operators/div_floor.ori` test exists. |

### 15C.7 Null Coalesce Operator (`??`)

| Item | Status | Evidence |
|------|--------|----------|
| Evaluate `??` for `Option<T>` | PLAN INACCURACY | IMPLEMENTED. Lexer (`QuestionQuestion` token), parser (coalesce precedence level), evaluator (`BinaryOp::Coalesce` in `can_eval/operators.rs`), and type checker all handle `??`. Tests exist: `tests/spec/expressions/coalesce.ori` (all 4181 tests pass including coalesce tests). Should be checked `[x]`. |
| Infer type for `a ?? b` | PLAN INACCURACY | IMPLEMENTED. Type inference handles coalesce in `ori_types/src/infer/expr/operators.rs`. |
| Short-circuit evaluation | PLAN INACCURACY | IMPLEMENTED. Short-circuit confirmed by tests that panic on default evaluation when left is Some. |
| Chained coalesce `a ?? b ?? c` | PLAN INACCURACY | PARTIALLY IMPLEMENTED. Comments in coalesce.ori note "26/31 tests pass" with 3 chaining tests having type info issues. |

### 15C.8 Compound Assignment Operators

| Item | Status | Evidence |
|------|--------|----------|
| Add 13 new raw token tags | PLAN INACCURACY | IMPLEMENTED. `PlusEq`, `MinusEq`, `StarEq`, `SlashEq`, `PercentEq` and more exist in `ori_lexer_core/src/tag/mod.rs` (21 files reference compound assignment). |
| Parse compound assignment and desugar | PLAN INACCURACY | IMPLEMENTED. `ori_parse/src/grammar/expr/mod.rs` has `compound_assign_op()` and `desugar_compound_assign()` methods. Desugars to `Assign { target, value: Binary }`. |
| Update raw scanner to scan compound assignment tokens | PLAN INACCURACY | IMPLEMENTED. Scanner in `ori_lexer_core/src/raw_scanner/operators.rs` handles two-char and three-char compound tokens. |

### 15C.9 MatMul Operator (`@`)

| Item | Status | Evidence |
|------|--------|----------|
| Add `MatMul` variant to `BinaryOp` | PLAN INACCURACY | IMPLEMENTED. `BinaryOp::MatMul` exists with `as_symbol()` returning `"@"`, precedence 3, `trait_method_name()` returning `"mat_mul"`, `trait_name()` returning `"MatMul"`. Evaluator has dispatch at line 72 of operator_dispatch.rs. |

### 15C.10 Power Operator (`**`)

| Item | Status | Evidence |
|------|--------|----------|
| Add `StarStar` and `StarStarEq` raw token tags | VERIFIED INCOMPLETE | No `StarStar` or `StarStarEq` tokens found anywhere in the codebase. `BinaryOp::Pow` does NOT exist in `ori_ir/src/ast/operators.rs`. |

### 15C.11 Pipe Operator (`|>`)

| Item | Status | Evidence |
|------|--------|----------|
| Add `PipeArrow` raw token tag | NEEDS VERIFICATION | `PipeArrow`/`TokenKind::Pipe` references exist in 8 files but may be incomplete. No `tests/spec/expressions/pipe/` directory exists. |

### 15C.13 Byte Literals

| Item | Status | Evidence |
|------|--------|----------|
| Add `RawTag::ByteLiteral` | VERIFIED INCOMPLETE | No `ByteLiteral` token found. No `scan_byte_literal()` method. No `TokenKind::Byte(u8)` variant. |

---

## Summary

Plan status of 0% is significantly inaccurate. Multiple major features are implemented:

1. **Template string lexer** (15C.1) -- scanner fully implemented with SIMD, template depth tracking
2. **Null coalesce `??`** (15C.7) -- lexer + parser + evaluator + type checker + tests all working
3. **Compound assignment** (15C.8) -- lexer + parser desugaring fully implemented
4. **MatMul `@`** (15C.9) -- IR + evaluator dispatch implemented
5. **Pipe operator `|>`** (15C.11) -- partial implementation (tokenization exists)

Items genuinely not implemented:
- Power operator `**` (15C.10)
- Byte literals `b'x'` (15C.13)
- Range step `by` (15C.3) -- not checked but likely incomplete
- Spread operator `...` (15C.2) -- lexer has no Ellipsis token
- Decimal duration/size literals (15C.6)

**Accuracy**: Section progress should be approximately 15-25%. Many checkboxes need to be marked as complete.

**Plan inaccuracies found**: ~20+ items are implemented but unchecked.
