# Phase 15C: Literals & Operators

**Goal**: Implement string interpolation, spread operator, and range step syntax

> **Source**: `docs/ori_lang/proposals/approved/`

---

## 15C.1 String Interpolation

**Proposal**: `proposals/approved/string-interpolation-proposal.md`

Add template strings with backtick delimiters and `{expr}` interpolation.

```ori
let name = "Alice"
let age = 30
print(msg: `Hello, {name}! You are {age} years old.`)
```

Two string types:
- `"..."` — regular strings, no interpolation, braces are literal
- `` `...` `` — template strings with `{expr}` interpolation

### Lexer

- [ ] **Implement**: Add template string literal tokenization (backtick delimited)
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — template string tokenization
  - [ ] **Ori Tests**: `tests/spec/lexical/template_strings.ori`
  - [ ] **LLVM Support**: LLVM codegen for template string tokenization
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template string tokenization codegen

- [ ] **Implement**: Handle `{expr}` interpolation boundaries (switch lexer modes)
  - [ ] **LLVM Support**: LLVM codegen for interpolation boundaries
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — interpolation boundaries codegen

- [ ] **Implement**: Handle `{{` and `}}` escape for literal braces
  - [ ] **Ori Tests**: `tests/spec/lexical/template_brace_escape.ori`
  - [ ] **LLVM Support**: LLVM codegen for brace escaping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — brace escaping codegen

- [ ] **Implement**: Handle `` \` `` escape for literal backtick
  - [ ] **Ori Tests**: `tests/spec/lexical/template_backtick_escape.ori`
  - [ ] **LLVM Support**: LLVM codegen for backtick escaping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — backtick escaping codegen

- [ ] **Implement**: Support escapes: `\\`, `\n`, `\t`, `\r`, `\0` in template strings
  - [ ] **Ori Tests**: `tests/spec/lexical/template_escapes.ori`
  - [ ] **LLVM Support**: LLVM codegen for template escapes
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template escapes codegen

- [ ] **Implement**: Multi-line template strings (preserve whitespace exactly)
  - [ ] **Ori Tests**: `tests/spec/lexical/template_multiline.ori`
  - [ ] **LLVM Support**: LLVM codegen for multiline template strings
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — multiline codegen

### Parser

- [ ] **Implement**: Parse template strings as sequence of `StringPart` (Literal | Interpolation)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — template string parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/interpolation.ori`
  - [ ] **LLVM Support**: LLVM codegen for template string parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template string parsing codegen

- [ ] **Implement**: Parse interpolated expressions (full expression grammar inside `{}`)
  - [ ] **Ori Tests**: `tests/spec/expressions/interpolation_expressions.ori`
  - [ ] **LLVM Support**: LLVM codegen for interpolated expressions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — interpolated expressions codegen

- [ ] **Implement**: Parse optional format specifiers `{expr:spec}`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/format_spec.rs` — format spec parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/format_specifiers.ori`
  - [ ] **LLVM Support**: LLVM codegen for format specifiers
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format specifiers codegen

- [ ] **Implement**: Parse format spec grammar: `[[fill]align][width][.precision][type]`
  - [ ] **Ori Tests**: `tests/spec/expressions/format_spec_grammar.ori`
  - [ ] **LLVM Support**: LLVM codegen for format spec grammar
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format spec grammar codegen

### Type System

- [ ] **Implement**: Interpolated expressions must implement `Printable`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/interpolation.rs` — printable constraint
  - [ ] **Ori Tests**: `tests/spec/types/printable_interpolation.ori`
  - [ ] **LLVM Support**: LLVM codegen for Printable constraint
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — Printable constraint codegen

- [ ] **Implement**: Validate format spec type compatibility (e.g., `x`/`X`/`b`/`o` only for int)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/format_spec.rs` — format spec type validation
  - [ ] **Ori Tests**: `tests/compile-fail/format_spec_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for format spec type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format spec type validation codegen

### Standard Library

- [ ] **Implement**: `Formattable` trait definition
  - [ ] **Ori Tests**: `tests/spec/traits/formattable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Formattable trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — Formattable trait codegen

- [ ] **Implement**: `FormatSpec` type definition
  - [ ] **Ori Tests**: `tests/spec/types/format_spec.ori`
  - [ ] **LLVM Support**: LLVM codegen for FormatSpec type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — FormatSpec type codegen

- [ ] **Implement**: `Alignment` and `FormatType` sum types
  - [ ] **Ori Tests**: `tests/spec/types/format_enums.ori`
  - [ ] **LLVM Support**: LLVM codegen for format enums
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format enums codegen

- [ ] **Implement**: Blanket impl `Formattable for T: Printable`
  - [ ] **Ori Tests**: `tests/spec/traits/formattable_blanket.ori`
  - [ ] **LLVM Support**: LLVM codegen for blanket impl
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — blanket impl codegen

- [ ] **Implement**: `apply_format` helper for width/alignment/padding
  - [ ] **Ori Tests**: `tests/spec/stdlib/apply_format.ori`
  - [ ] **LLVM Support**: LLVM codegen for apply_format
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — apply_format codegen

### Codegen

- [ ] **Implement**: Desugar template strings to concatenation with `to_str()` calls
  - [ ] **Rust Tests**: `oric/src/desugar/interpolation.rs` — template desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/interpolation_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for template string desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template string desugaring codegen

- [ ] **Implement**: Desugar format specifiers to `format(value, FormatSpec {...})` calls
  - [ ] **Rust Tests**: `oric/src/desugar/format_spec.rs` — format spec desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/format_spec_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for format spec desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format spec desugaring codegen

---

## 15C.2 Spread Operator

**Proposal**: `proposals/approved/spread-operator-proposal.md`

Add a spread operator `...` for expanding collections and structs in literal contexts.

```ori
let combined = [...list1, ...list2]
let merged = {...defaults, ...overrides}
let updated = Point { ...original, x: 10 }
```

### Lexer

- [ ] **Implement**: Add `...` as a token (Ellipsis)
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — ellipsis token tests
  - [ ] **Ori Tests**: `tests/spec/lexical/spread_token.ori`
  - [ ] **LLVM Support**: LLVM codegen for ellipsis token
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — ellipsis token codegen

### Parser

- [ ] **Implement**: Parse `...expression` in list literals
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — list spread parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/list_spread.ori`
  - [ ] **LLVM Support**: LLVM codegen for list spread
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — list spread codegen

- [ ] **Implement**: Parse `...expression` in map literals
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — map spread parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/map_spread.ori`
  - [ ] **LLVM Support**: LLVM codegen for map spread
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — map spread codegen

- [ ] **Implement**: Parse `...expression` in struct literals
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — struct spread parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/struct_spread.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct spread
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — struct spread codegen

### Type Checker

- [ ] **Implement**: Verify list spread expression is `[T]` matching container
  - [ ] **Rust Tests**: `oric/src/typeck/checker/spread.rs` — list spread type checking
  - [ ] **Ori Tests**: `tests/compile-fail/list_spread_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for list spread type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — list spread type checking codegen

- [ ] **Implement**: Verify map spread expression is `{K: V}` matching container
  - [ ] **Rust Tests**: `oric/src/typeck/checker/spread.rs` — map spread type checking
  - [ ] **Ori Tests**: `tests/compile-fail/map_spread_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for map spread type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — map spread type checking codegen

- [ ] **Implement**: Verify struct spread is same struct type (no subset/superset)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/spread.rs` — struct spread type checking
  - [ ] **Ori Tests**: `tests/compile-fail/struct_spread_wrong_type.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct spread type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — struct spread type checking codegen

- [ ] **Implement**: Track struct field coverage (spread + explicit must cover all fields)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/struct_lit.rs` — field coverage tracking
  - [ ] **Ori Tests**: `tests/compile-fail/struct_spread_missing_fields.ori`
  - [ ] **LLVM Support**: LLVM codegen for field coverage tracking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — field coverage tracking codegen

### Code Generation

- [ ] **Implement**: Desugar list spread to concatenation (`[a] + b + [c]`)
  - [ ] **Rust Tests**: `oric/src/codegen/spread.rs` — list spread desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/list_spread_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for list spread desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — list spread desugaring codegen

- [ ] **Implement**: Desugar map spread to merge calls
  - [ ] **Rust Tests**: `oric/src/codegen/spread.rs` — map spread desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/map_spread_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for map spread desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — map spread desugaring codegen

- [ ] **Implement**: Desugar struct spread to explicit field assignments
  - [ ] **Rust Tests**: `oric/src/codegen/spread.rs` — struct spread desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/struct_spread_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct spread desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — struct spread desugaring codegen

### Edge Cases

- [ ] **Implement**: Empty spread produces nothing (valid)
  - [ ] **Ori Tests**: `tests/spec/expressions/spread_empty.ori`
  - [ ] **LLVM Support**: LLVM codegen for empty spread
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — empty spread codegen

- [ ] **Implement**: Spread preserves evaluation order (left-to-right)
  - [ ] **Ori Tests**: `tests/spec/expressions/spread_eval_order.ori`
  - [ ] **LLVM Support**: LLVM codegen for spread evaluation order
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — spread evaluation order codegen

- [ ] **Implement**: Error for spread in function call arguments
  - [ ] **Rust Tests**: `oric/src/typeck/checker/call.rs` — spread in call error
  - [ ] **Ori Tests**: `tests/compile-fail/spread_in_function_call.ori`
  - [ ] **LLVM Support**: LLVM codegen for spread in call error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — spread in call error codegen

---

## 15C.3 Range with Step

**Proposal**: `proposals/approved/range-step-proposal.md`

Add a `by` keyword to range expressions for non-unit step values.

```ori
0..10 by 2      // 0, 2, 4, 6, 8
10..0 by -1     // 10, 9, 8, ..., 1
0..=10 by 2     // 0, 2, 4, 6, 8, 10
```

### Lexer

- [ ] **Implement**: Add `by` as contextual keyword token following range operators
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — by keyword tokenization
  - [ ] **Ori Tests**: `tests/spec/lexical/by_keyword.ori`
  - [ ] **LLVM Support**: LLVM codegen for by keyword
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — by keyword codegen

### Parser

- [ ] **Implement**: Extend `range_expr` to accept `[ "by" shift_expr ]`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — range step parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/range_step.ori`
  - [ ] **LLVM Support**: LLVM codegen for range step parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — range step parsing codegen

### Type Checker

- [ ] **Implement**: Validate step expression has same type as range bounds
  - [ ] **Rust Tests**: `oric/src/typeck/checker/range.rs` — step type checking
  - [ ] **Ori Tests**: `tests/compile-fail/range_step_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for step type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — step type checking codegen

- [ ] **Implement**: Restrict `by` to integer ranges only (compile-time error for float)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/range.rs` — int-only restriction
  - [ ] **Ori Tests**: `tests/compile-fail/range_step_float.ori`
  - [ ] **LLVM Support**: LLVM codegen for int-only restriction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — int-only restriction codegen

### Code Generation / Interpreter

- [ ] **Implement**: Extend Range type with optional step field (default 1)
  - [ ] **Rust Tests**: `oric/src/ir/types.rs` — Range type extension
  - [ ] **Ori Tests**: `tests/spec/types/range_with_step.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range type extension
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — Range type extension codegen

- [ ] **Implement**: Iterator for stepped ranges (ascending and descending)
  - [ ] **Rust Tests**: `oric/src/eval/iter.rs` — stepped range iteration
  - [ ] **Ori Tests**: `tests/spec/expressions/range_step_iteration.ori`
  - [ ] **LLVM Support**: LLVM codegen for stepped range iteration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — stepped range iteration codegen

- [ ] **Implement**: Runtime panic for zero step
  - [ ] **Rust Tests**: `oric/src/eval/range.rs` — zero step panic
  - [ ] **Ori Tests**: `tests/spec/expressions/range_step_zero_panic.ori`
  - [ ] **LLVM Support**: LLVM codegen for zero step panic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — zero step panic codegen

- [ ] **Implement**: Empty range for mismatched direction (no panic)
  - [ ] **Rust Tests**: `oric/src/eval/range.rs` — direction mismatch
  - [ ] **Ori Tests**: `tests/spec/expressions/range_step_empty.ori`
  - [ ] **LLVM Support**: LLVM codegen for direction mismatch
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — direction mismatch codegen

---

## 15C.4 Phase Completion Checklist

- [ ] All implementation items have checkboxes marked `[x]`
- [ ] All spec docs updated
- [ ] CLAUDE.md updated with syntax changes
- [ ] Migration tools working
- [ ] All tests pass: `./test-all`

**Exit Criteria**: Literal and operator syntax proposals implemented
