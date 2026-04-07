---
section: "15C"
title: Literals & Operators
status: in-progress
reviewed: true
last_verified: "2026-03-29"
tier: 5
goal: Implement string interpolation, spread operator, range step syntax, and pipe operator
sections:
  - id: "15C.1"
    title: String Interpolation
    status: in-progress
  - id: "15C.2"
    title: Spread Operator
    status: in-progress
  - id: "15C.3"
    title: Range with Step
    status: in-progress
  - id: "15C.4"
    title: Computed Map Keys
    status: not-started
  - id: "15C.5"
    title: Floor Division (div) Operator Fix
    status: complete
  - id: "15C.6"
    title: Decimal Duration and Size Literals
    status: in-progress
  - id: "15C.7"
    title: Null Coalesce Operator
    status: complete
  - id: "15C.8"
    title: Compound Assignment Operators
    status: in-progress
  - id: "15C.9"
    title: MatMul Operator
    status: not-started
  - id: "15C.10"
    title: Power Operator
    status: not-started
  - id: "15C.11"
    title: Pipe Operator
    status: not-started
  - id: "15C.13"
    title: Byte Literals and Hex Escapes
    status: not-started
  - id: "15C.12"
    title: Section Completion Checklist
    status: not-started
---

# Section 15C: Literals & Operators

**Goal**: Implement string interpolation, spread operator, range step syntax, and pipe operator

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

- [x] **Implement**: Add template string literal tokenization (backtick delimited) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `oric/tests/phases/parse/lexer.rs` — `test_lex_template_literal`, `test_lex_template_full_content`, `test_lex_template_interpolation`, `test_lex_template_multiple_interpolations`, `test_lex_template_format_spec`, `test_lex_template_format_spec_complex`
  - [x] **Ori Tests**: `tests/spec/expressions/template_literals.ori` (all pass)
  - [x] **LLVM Support**: Format runtime in `ori_rt/src/format/` with `ori_format_int/float/str/bool/char`
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — WEAK TESTS: this file does not exist (verified 2026-03-29)
  - [x] **AOT Tests**: AOT spec tests in `ori_llvm/tests/aot/spec.rs` include template literal tests

- [x] **Implement**: Handle `{expr}` interpolation boundaries (switch lexer modes) [done] (verified 2026-03-29)
  - [ ] **LLVM Support**: LLVM codegen for interpolation boundaries
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — interpolation boundaries codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Handle `{{` and `}}` escape for literal braces
  - [ ] **Ori Tests**: `tests/spec/lexical/template_brace_escape.ori`
  - [ ] **LLVM Support**: LLVM codegen for brace escaping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — brace escaping codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Handle `` \` `` escape for literal backtick [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_lexer/src/cook_escape/tests.rs` — `template_backtick_escape` test passes
  - [ ] **Ori Tests**: `tests/spec/lexical/template_backtick_escape.ori`
  - [ ] **LLVM Support**: LLVM codegen for backtick escaping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — backtick escaping codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Support escapes: `\\`, `\n`, `\t`, `\r`, `\0` in template strings [done] (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/lexical/template_escapes.ori`
  - [ ] **LLVM Support**: LLVM codegen for template escapes
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template escapes codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Multi-line template strings (preserve whitespace exactly)
  - [ ] **Ori Tests**: `tests/spec/lexical/template_multiline.ori`
  - [ ] **LLVM Support**: LLVM codegen for multiline template strings
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — multiline codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Parser

- [x] **Implement**: Parse template strings as sequence of `StringPart` (Literal | Interpolation) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/expr/primary/specials.rs` — `parse_template_literal` produces `ExprKind::TemplateStr` with `TemplatePart` sequence
  - [ ] **Ori Tests**: `tests/spec/expressions/interpolation.ori`
  - [ ] **LLVM Support**: LLVM codegen for template string parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template string parsing codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Parse interpolated expressions (full expression grammar inside `{}`) [done] (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/expressions/interpolation_expressions.ori`
  - [ ] **LLVM Support**: LLVM codegen for interpolated expressions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — interpolated expressions codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Parse optional format specifiers `{expr:spec}` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_ir/src/format_spec.rs` — full format spec parser with `ParsedFormatSpec` struct
  - [ ] **Ori Tests**: `tests/spec/expressions/format_specifiers.ori`
  - [ ] **LLVM Support**: LLVM codegen for format specifiers
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format specifiers codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Parse format spec grammar: `[[fill]align][width][.precision][type]` [done] (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/expressions/format_spec_grammar.ori`
  - [ ] **LLVM Support**: LLVM codegen for format spec grammar
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format spec grammar codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Type System

- [x] **Implement**: Interpolated expressions must implement `Printable` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Type checker validates via `to_str()` call desugaring. Error E2034/E2035 for non-Printable interpolation.
  - [ ] **Ori Tests**: `tests/spec/types/printable_interpolation.ori`
  - [ ] **LLVM Support**: LLVM codegen for Printable constraint
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — Printable constraint codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Validate format spec type compatibility (e.g., `x`/`X`/`b`/`o` only for int) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_eval/src/interpreter/format.rs` validates format type against value type
  - [ ] **Ori Tests**: `tests/compile-fail/format_spec_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for format spec type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format spec type validation codegen

### Standard Library

- [x] **Implement**: `Formattable` trait definition [done] (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/traits/formattable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Formattable trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — Formattable trait codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `FormatSpec` type definition [done] (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/types/format_spec.ori`
  - [ ] **LLVM Support**: LLVM codegen for FormatSpec type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — FormatSpec type codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `Alignment` and `FormatType` sum types [done] (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/types/format_enums.ori`
  - [ ] **LLVM Support**: LLVM codegen for format enums
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format enums codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Blanket impl `T: Formattable` where `T: Printable` [done] (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/traits/formattable_blanket.ori`
  - [ ] **LLVM Support**: LLVM codegen for blanket impl
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — blanket impl codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `apply_format` helper for width/alignment/padding [done] (verified 2026-03-29)
  - [ ] **Ori Tests**: `tests/spec/stdlib/apply_format.ori`
  - [ ] **LLVM Support**: LLVM codegen for apply_format
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — apply_format codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Codegen

- [x] **Implement**: Desugar template strings to concatenation with `to_str()` calls [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_canon/src/desugar/mod.rs` — `desugar_template_literal()`. Canon test at `ori_canon/src/desugar/tests.rs`
  - [ ] **Ori Tests**: `tests/spec/expressions/interpolation_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for template string desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template string desugaring codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Desugar format specifiers to `format(value, FormatSpec {...})` calls [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Evaluator builds `FormatSpec` struct value and calls `format()` in `ori_eval/src/interpreter/format.rs`
  - [ ] **Ori Tests**: `tests/spec/expressions/format_spec_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for format spec desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format spec desugaring codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 15C.2 Spread Operator

**Proposal**: `proposals/approved/spread-operator-proposal.md`

> NEEDS TESTS (verified 2026-03-29): Zero Ori spec tests exist for spread operator. Core impl is done (parser, typeck, desugar) but only 1 Rust unit test (`desugar_list_with_spread_simple`) covers desugar. No AOT coverage. All planned Ori test files below are absent.

Add a spread operator `...` for expanding collections and structs in literal contexts.

```ori
let combined = [...list1, ...list2]
let merged = {...defaults, ...overrides}
let updated = Point { ...original, x: 10 }
```

### Lexer

- [x] **Implement**: Add `...` as a token (Ellipsis) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `TokenKind::DotDotDot` exists. Scanner handles `...`.
  - [ ] **Ori Tests**: `tests/spec/lexical/spread_token.ori`
  - [ ] **LLVM Support**: LLVM codegen for ellipsis token
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — ellipsis token codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Parser

- [x] **Implement**: Parse `...expression` in list literals [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/expr/primary/collections.rs` — produces `ExprKind::ListWithSpread(ListElementRange)`
  - [ ] **Ori Tests**: `tests/spec/expressions/list_spread.ori`
  - [ ] **LLVM Support**: LLVM codegen for list spread
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — list spread codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Parse `...expression` in map literals [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/expr/primary/collections.rs` — produces `ExprKind::MapWithSpread(MapElementRange)`
  - [ ] **Ori Tests**: `tests/spec/expressions/map_spread.ori`
  - [ ] **LLVM Support**: LLVM codegen for map spread
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — map spread codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Parse `...expression` in struct literals [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/expr/postfix.rs` — produces `ExprKind::StructWithSpread`
  - [ ] **Ori Tests**: `tests/spec/expressions/struct_spread.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct spread
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — struct spread codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Type Checker

- [x] **Implement**: Verify list spread expression is `[T]` matching container [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/expr/collections.rs` handles spread type checking
  - [ ] **Ori Tests**: `tests/compile-fail/list_spread_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for list spread type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — list spread type checking codegen

- [x] **Implement**: Verify map spread expression is `{K: V}` matching container [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/expr/collections.rs` handles map spread type checking
  - [ ] **Ori Tests**: `tests/compile-fail/map_spread_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for map spread type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — map spread type checking codegen

- [x] **Implement**: Verify struct spread is same struct type (no subset/superset) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/expr/structs/mod.rs` handles struct spread
  - [ ] **Ori Tests**: `tests/compile-fail/struct_spread_wrong_type.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct spread type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — struct spread type checking codegen

- [x] **Implement**: Track struct field coverage (spread + explicit must cover all fields) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Type checker validates field coverage
  - [ ] **Ori Tests**: `tests/compile-fail/struct_spread_missing_fields.ori`
  - [ ] **LLVM Support**: LLVM codegen for field coverage tracking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — field coverage tracking codegen

### Code Generation

- [x] **Implement**: Desugar list spread to concatenation (`[a] + b + [c]`) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_canon/src/desugar/spread.rs` — `ListWithSpread` to `List + .concat()`
  - [ ] **Ori Tests**: `tests/spec/expressions/list_spread_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for list spread desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — list spread desugaring codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Desugar map spread to merge calls [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_canon/src/desugar/spread.rs` — `MapWithSpread` to `Map + .merge()`
  - [ ] **Ori Tests**: `tests/spec/expressions/map_spread_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for map spread desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — map spread desugaring codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Desugar struct spread to explicit field assignments [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_canon/src/desugar/spread.rs` — `StructWithSpread` to `Struct` with all fields resolved
  - [ ] **Ori Tests**: `tests/spec/expressions/struct_spread_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct spread desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — struct spread desugaring codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Edge Cases

- [ ] **Implement**: Empty spread produces nothing (valid)
  - [ ] **Ori Tests**: `tests/spec/expressions/spread_empty.ori`
  - [ ] **LLVM Support**: LLVM codegen for empty spread
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — empty spread codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Spread preserves evaluation order (left-to-right)
  - [ ] **Ori Tests**: `tests/spec/expressions/spread_eval_order.ori`
  - [ ] **LLVM Support**: LLVM codegen for spread evaluation order
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — spread evaluation order codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Error for spread in function call arguments
  - [ ] **Rust Tests**: `ori_types/src/check/call.rs` — spread in call error
  - [ ] **Ori Tests**: `tests/compile-fail/spread_in_function_call.ori`
  - [ ] **LLVM Support**: LLVM codegen for spread in call error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — spread in call error codegen

---

## 15C.3 Range with Step

**Proposal**: `proposals/approved/range-step-proposal.md`

> BUG-15C-001 (verified 2026-03-29): **Dual-execution parity violation on zero step.** Interpreter `range_len()` in `ori_patterns/src/value/iterator/mod.rs` line 24 returns 0 for `step==0` (silent empty range), while LLVM panics with "range step cannot be zero". These must agree. LLVM behavior (panic) is correct -- zero step is a user error, not an empty range. Fix: change interpreter `range_len` from `return 0` to panic on zero step.

Add a `by` keyword to range expressions for non-unit step values.

```ori
0..10 by 2      // 0, 2, 4, 6, 8
10..0 by -1     // 10, 9, 8, ..., 1
0..=10 by 2     // 0, 2, 4, 6, 8, 10
```

### Lexer

- [x] **Implement**: Add `by` as contextual keyword token following range operators [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `TokenKind::By` exists. `ori_lexer/src/keywords/mod.rs` has `by`.
  - [ ] **Ori Tests**: `tests/spec/lexical/by_keyword.ori`
  - [ ] **LLVM Support**: LLVM codegen for by keyword
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — by keyword codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Parser

- [x] **Implement**: Extend `range_expr` to accept `[ "by" shift_expr ]` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/expr/mod.rs` line 313 — `if matches!(self.cursor.current_kind(), TokenKind::By)`
  - [ ] **Ori Tests**: `tests/spec/expressions/range_step.ori`
  - [ ] **LLVM Support**: LLVM codegen for range step parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — range step parsing codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Type Checker

- [x] **Implement**: Validate step expression has same type as range bounds [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/expr/mod.rs` handles range step type checking
  - [ ] **Ori Tests**: `tests/compile-fail/range_step_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for step type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — step type checking codegen

- [ ] **Implement**: Restrict `by` to integer ranges only (compile-time error for float)
  - [ ] **Rust Tests**: `ori_types/src/check/range.rs` — int-only restriction
  - [ ] **Ori Tests**: `tests/compile-fail/range_step_float.ori`
  - [ ] **LLVM Support**: LLVM codegen for int-only restriction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — int-only restriction codegen

### Code Generation / Evaluator

- [x] **Implement**: Extend Range type with optional step field (default 1) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ExprKind::Range { start, end, step, inclusive }` — `step: ExprId` in AST
  - [ ] **Ori Tests**: `tests/spec/types/range_with_step.ori`
  - [x] **LLVM Support**: `ori_arc/src/lower/control_flow/for_loops/for_range.rs` handles step
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — Range type extension codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Iterator for stepped ranges (ascending and descending) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_patterns/src/value/iterator/mod.rs` — `IteratorValue::Range { current, end, step, inclusive }`. `range_len()` handles step calculations.
  - [ ] **Ori Tests**: `tests/spec/expressions/range_step_iteration.ori`
  - [ ] **LLVM Support**: LLVM codegen for stepped range iteration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — stepped range iteration codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Runtime panic for zero step — BUG-15C-001: interpreter returns 0 (silent empty range) instead of panic; LLVM correctly panics (verified 2026-03-29)
  - [x] **LLVM**: LLVM codegen panics with "range step cannot be zero" (3 AOT zero-step panic tests pass)
  - [ ] **Interpreter**: `range_len()` at line 24: `if step == 0 { return 0; }` — MUST change to panic to match LLVM
  - [ ] **Ori Tests**: `tests/spec/expressions/range_step_zero_panic.ori`
  - [ ] **LLVM Support**: LLVM codegen for zero step panic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — zero step panic codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Empty range for mismatched direction (no panic) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `range_len()` handles direction mismatch by returning 0
  - [ ] **Ori Tests**: `tests/spec/expressions/range_step_empty.ori`
  - [ ] **LLVM Support**: LLVM codegen for direction mismatch
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — direction mismatch codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 15C.4 Computed Map Keys

**Proposal**: `proposals/approved/computed-map-keys-proposal.md`

Formalize map literal key semantics: bare identifiers are literal string keys (like TypeScript/JSON), and `[expression]` syntax enables computed keys.

```ori
{timeout: 30}           // {"timeout": 30} - bare identifier is literal string
{[key]: 30}             // computed key - evaluates key variable
{if: 1, type: "user"}   // reserved keywords valid as literal keys
```

### Lexer

- [ ] **Implement**: Recognize `[` in map key position as start of computed key
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — computed key bracket detection
  - [ ] **Ori Tests**: `tests/spec/lexical/computed_map_key.ori`
  - [ ] **LLVM Support**: LLVM codegen for computed key tokenization
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/computed_map_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

### Parser

- [ ] **Implement**: Parse bare identifier as literal string key in map context
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — bare identifier key parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/map_literal_keys.ori`
  - [ ] **LLVM Support**: LLVM codegen for bare identifier keys
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/computed_map_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Parse `[expression]` as computed key in map context
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — computed key parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/computed_map_key.ori`
  - [ ] **LLVM Support**: LLVM codegen for computed key parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/computed_map_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Allow reserved keywords as bare literal keys
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — keyword-as-key parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/map_keyword_keys.ori`
  - [ ] **LLVM Support**: LLVM codegen for keyword keys
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/computed_map_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

### Type Checker

- [ ] **Implement**: Bare identifier keys always produce `str` type
  - [ ] **Rust Tests**: `ori_types/src/check/map_lit.rs` — bare key type inference
  - [ ] **Ori Tests**: `tests/spec/types/map_key_types.ori`
  - [ ] **LLVM Support**: LLVM codegen for bare key types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/computed_map_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Computed keys must match map key type `K` in `{K: V}`
  - [ ] **Rust Tests**: `ori_types/src/check/map_lit.rs` — computed key type checking
  - [ ] **Ori Tests**: `tests/compile-fail/computed_key_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for computed key type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/computed_map_tests.rs`

- [ ] **Implement**: Error for bare literals in non-string-key maps
  - [ ] **Rust Tests**: `ori_types/src/check/map_lit.rs` — bare key in int-map error
  - [ ] **Ori Tests**: `tests/compile-fail/bare_key_non_string_map.ori`
  - [ ] **LLVM Support**: LLVM codegen for bare key error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/computed_map_tests.rs`

### Code Generation

- [ ] **Implement**: Desugar bare identifier keys to string literals
  - [ ] **Rust Tests**: `ori_llvm/src/codegen/map.rs` — bare key desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/map_key_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for bare key desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/computed_map_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Evaluate computed key expressions at runtime
  - [ ] **Rust Tests**: `ori_llvm/src/codegen/map.rs` — computed key evaluation
  - [ ] **Ori Tests**: `tests/spec/expressions/computed_key_eval.ori`
  - [ ] **LLVM Support**: LLVM codegen for computed key evaluation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/computed_map_tests.rs`
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 15C.5 Floor Division (`div`) Operator Fix

**Proposal**: `proposals/approved/grammar-sync-formalization-proposal.md`

Fix parser discrepancy where `div` operator is in grammar but missing from parser.

### Parser Fix

- [x] **Implement**: Add `TokenKind::Div` case to `match_multiplicative_op()` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/expr/operators.rs` line 134: `TokenKind::TAG_DIV, FloorDiv, bp::MULTIPLICATIVE, 1;` — `div` in operator table at multiplicative precedence. `BinaryOp::FloorDiv` with `trait_method_name() -> "floor_divide"`, `trait_name() -> "FloorDiv"`. Evaluator handles in `ori_eval/src/operators/mod.rs`. LLVM handles in `ori_llvm/src/codegen/arc_emitter/operators/strategy.rs`.
  - [ ] **Ori Tests**: `tests/spec/operators/div_floor.ori`
  - [x] **LLVM Support**: LLVM codegen for div operator
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/operator_tests.rs` — div codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Operator Test Infrastructure

- [ ] **Implement**: Create `tests/spec/operators/` directory structure
  - [ ] `tests/spec/operators/precedence/` — precedence relationship tests
  - [ ] `tests/spec/operators/associativity/` — associativity tests
  - [ ] `tests/spec/operators/operators/` — individual operator tests

- [ ] **Implement**: Add precedence tests for adjacent levels
  - [ ] **Ori Tests**: `tests/spec/operators/precedence/mul_over_add.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/precedence/add_over_shift.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/precedence/shift_over_range.ori`

- [ ] **Implement**: Add associativity tests for binary operators
  - [ ] **Ori Tests**: `tests/spec/operators/associativity/mul_left_assoc.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/associativity/add_left_assoc.ori`

---

## 15C.6 Decimal Duration and Size Literals

**Proposal**: `proposals/approved/decimal-duration-size-literals-proposal.md`

Allow decimal syntax in duration and size literals as compile-time sugar.

```ori
let t = 0.5s        // 500,000,000 nanoseconds
let t = 1.56s       // 1,560,000,000 nanoseconds
let s = 1.5kb       // 1,500 bytes (SI units)
let s = 0.25mb      // 250,000 bytes
```

**Key changes:**
- Decimal notation for duration/size literals (compile-time, no floats)
- Size units changed from binary (1024) to SI (1000)
- E0911 repurposed: "literal cannot be represented exactly"

### Lexer

- [x] **Implement**: Parse decimal duration literals (`1.5s`, `0.25h`, etc.) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_lexer/src/cooker/duration_size.rs` — `cook_duration()` calls `parse_decimal_unit_value()`. Tests at `ori_lexer/src/cooker/tests.rs`: `decimal_duration_seconds`, `decimal_duration_milliseconds`, `decimal_duration_hours`, `decimal_duration_half_second`, `decimal_duration_many_digits`, `decimal_duration_nanoseconds_error`, `decimal_duration_overflow_is_error`. Phase tests at `oric/tests/phases/parse/lexer.rs`.
  - [ ] **Ori Tests**: `tests/spec/lexical/decimal_duration.ori`
  - [ ] **LLVM Support**: LLVM codegen for decimal duration literals
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/literal_tests.rs` — decimal duration codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Parse decimal size literals (`1.5kb`, `0.5mb`, etc.) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_lexer/src/cooker/duration_size.rs` — `cook_size()` with `parse_decimal_unit_value()`. Tests: `decimal_size_kilobytes`, `decimal_size_megabytes`, `decimal_size_bytes_error`, `decimal_size_overflow_is_error`.
  - [ ] **Ori Tests**: `tests/spec/lexical/decimal_size.ori`
  - [ ] **LLVM Support**: LLVM codegen for decimal size literals
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/literal_tests.rs` — decimal size codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Integer arithmetic conversion (no floats involved) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `parse_decimal_unit_value()` in `ori_lexer/src/cooker/duration_size.rs`
  - [ ] **Ori Tests**: `tests/spec/lexical/decimal_precision.ori`

- [x] **Implement**: Validation for whole-number results [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Returns `None` if not whole number, triggers `DecimalNotRepresentable` lex error
  - [ ] **Ori Tests**: `tests/compile-fail/decimal_duration_not_whole.ori`
  - [ ] **Ori Tests**: `tests/compile-fail/decimal_size_not_whole.ori`

### Token Changes

- [ ] **Implement**: Remove `FloatDurationError` and `FloatSizeError` token types
  - [ ] **Rust Tests**: `ori_ir/src/token.rs` — token type cleanup

- [x] **Implement**: Store Duration/Size tokens as computed base unit value [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Tokens store computed i64 value

### Error Messages

- [x] **Implement**: E0911 error for non-representable decimal literals [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_lexer/src/lex_error/mod.rs` — `DecimalNotRepresentable` error. `oric/src/problem/lex.rs` formats with note about whole numbers.
  - [ ] **Ori Tests**: `tests/compile-fail/e0911_decimal_precision.ori`

### Size Unit Change

- [x] **Implement**: Change Size unit multipliers from 1024 to 1000 [done] (verified 2026-03-29)
  - [x] **Rust Tests**: SI units confirmed: `ori_ir/src/builtin_constants/mod.rs` comment "Uses SI units (1000-based): 1kb = 1000 bytes". AOT tests confirm: `ori_llvm/tests/aot/spec.rs` line 406: `let kb_ok = 1kb == 1000b;`.
  - [ ] **Ori Tests**: `tests/spec/types/size_si_units.ori`
  - [x] **LLVM Support**: LLVM codegen with SI units
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/literal_tests.rs` — SI unit codegen
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 15C.7 Null Coalesce Operator (`??`)

**Source**: `grammar.ebnf § coalesce_expr`, `spec/14-expressions.md § Operators`

The null coalesce operator `??` provides a default value when an `Option` is `None`.

```ori
let name = maybe_name ?? "Anonymous"
let count = get_count() ?? 0
```

**Status**: Fully implemented including LLVM [done] (verified 2026-03-29). NEEDS TESTS: No AOT-specific coalesce tests despite LLVM `emit_coalesce()` existing.

### Evaluator

- [x] **Implement**: Evaluate `??` for `Option<T>` — extract Some value or use default [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_eval/src/interpreter/can_eval/operators.rs` line 38: `BinaryOp::Coalesce` handles `Some(inner)` extraction and `None` default
  - [x] **Ori Tests**: `tests/spec/expressions/coalesce.ori` — 430-line test file with 30+ tests including short-circuit, chaining, Result coalescing, nested options, map lookups (all 4181 tests pass)
  - [x] **LLVM Support**: `ori_llvm/src/codegen/arc_emitter/operators/strategy.rs` — `emit_coalesce()` method
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/coalesce_tests.rs` — coalesce codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Type Checker

- [x] **Implement**: Infer type for `a ?? b` — result is `T` where `a: Option<T>` and `b: T` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/expr/operators.rs` line 285: `BinaryOp::Coalesce` branch
  - [ ] **Ori Tests**: `tests/spec/types/coalesce_inference.ori`

- [x] **Implement**: Error for non-Option left operand [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Type checker produces E2038 for non-Option/Result left operand
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/coalesce_non_option.ori`

### Edge Cases

- [x] **Implement**: Short-circuit evaluation — don't evaluate right side if left is Some [done] (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/expressions/coalesce.ori` — short-circuit tests pass

- [x] **Implement**: Chained coalesce — `a ?? b ?? c` [done] (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/expressions/coalesce.ori` — chain_all_none, chain_first_some, chain_middle_some, chain_last_some, chain_short_circuit tests pass

---

## 15C.8 Compound Assignment Operators

**Proposal**: `proposals/approved/compound-assignment-proposal.md`

Add compound assignment operators (`+=`, `-=`, `*=`, `/=`, `%=`, `@=`, `&=`, `|=`, `^=`, `<<=`, `>>=`, `&&=`, `||=`) that desugar to `x = x op y` at the parser level.

```ori
let sum = 0;
for item in items {
    sum += item.value;
}
```

### Lexer

- [x] **Implement**: Add 13 new raw token tags to `ori_lexer_core/src/tag/mod.rs` [done] (verified 2026-03-29)
  - `PlusEq` (62), `MinusEq` (63), `StarEq` (64), `SlashEq` (65), `PercentEq` (66), plus `AtEq`, `AmpEq`, `PipeEq`, `CaretEq`, `ShlEq`, `AmpAmpEq`, `PipePipeEq`
  - [x] **Rust Tests**: `ori_lexer_core/src/raw_scanner/tests.rs` line 340-344
  - [x] **Rust Tests**: `ori_lexer_core/src/tag/tests.rs`

- [x] **Implement**: Update raw scanner to scan compound assignment tokens [done] (verified 2026-03-29)
  - Two-char: `+=`, `-=`, `*=`, `/=`, `%=`, `@=`, `&=`, `|=`, `^=`
  - Three-char: `<<=`, `>>=`, `&&=`, `||=`
  - [x] **Rust Tests**: `ori_lexer_core/src/raw_scanner/operators.rs` handles all compound ops

- [x] **Implement**: Map raw tags to `TokenKind` in cooker [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Cooker maps all compound assignment tokens to `TokenKind` variants

### Parser

- [x] **Implement**: Parse compound assignment and desugar to `Assign { target, value: Binary/And/Or }` [done] (verified 2026-03-29)
  - Trait-based ops: map `PlusEq` to `BinaryOp::Add`, etc.
  - Logical ops: map `AmpAmpEq` to `ExprKind::And`, `PipePipeEq` to `ExprKind::Or`
  - [x] **Rust Tests**: `ori_parse/src/grammar/expr/operators.rs` — `compound_op_for_tag()` maps all 12 tag-based operators. `compound_assign_covers_all_tags` test verifies exhaustive coverage.
  - [ ] **Ori Tests**: `tests/spec/operators/compound_assignment/basic.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/compound_assignment/field_access.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/compound_assignment/subscript.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/compound_assignment/logical.ori`

- [x] **Implement**: Remove compound assignment from "common mistake" detection [done] (verified 2026-03-29)
  - [x] `ori_parse/src/error/mistakes.rs` does NOT list compound assignment operators as mistakes. Only `??=` is listed. Hints even suggest using `+=` as replacement for `++`.
  - [x] **Rust Tests**: Detection tests updated

### Error Messages

- [ ] **Implement**: Error for compound assignment on immutable binding (`$`)
  - Message: "cannot use compound assignment on immutable binding `$y`. Remove `$` for mutability: `let y = ...`"
  - [ ] **Ori Tests**: `tests/compile-fail/compound_assign_immutable.ori`

- [ ] **Implement**: Error for compound assignment as expression
  - Message: "compound assignment is a statement, not an expression"
  - [ ] **Ori Tests**: `tests/compile-fail/compound_assign_as_expression.ori`

### LLVM Support

- [x] **LLVM Support**: No changes needed — parser desugars before reaching LLVM codegen [done] (verified 2026-03-29)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/compound_assign_tests.rs` — verify desugared form compiles correctly
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 15C.9 MatMul Operator (`@`)

**Proposal**: `proposals/approved/matmul-operator-proposal.md`

Add `@` as a binary operator for matrix multiplication. Desugars to `MatMul` trait method `matrix_multiply()`. Same precedence as `*`/`/`/`%`/`div` (level 4, multiplicative). The `@` token is disambiguated by syntactic context (item position = function declaration, expression position = matmul, pattern position = at-binding).

### IR

- [ ] **Implement**: Add `MatMul` variant to `BinaryOp` + arms in `as_symbol()`, `precedence()`, `trait_method_name()`, `trait_name()`
  - [ ] **Rust Tests**: `ori_ir/src/ast/tests.rs`

### Parser

- [ ] **Implement**: Add `TokenKind::At` to multiplicative precedence level in expression parser
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr/tests.rs` — matmul parsing
  - [ ] **Ori Tests**: `tests/spec/operators/matmul/basic.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/matmul/precedence.ori`

### Evaluator

- [ ] **Implement**: Add `BinaryOp::MatMul` error arms to primitive type handlers (no primitive implements `MatMul`)
  - [ ] **Rust Tests**: `ori_eval/src/tests/` — matmul error on primitives

### Standard Library

- [ ] **Implement**: Add `MatMul` trait definition to `library/std/prelude.ori`
  - [ ] **Ori Tests**: `tests/spec/traits/operators/matmul_trait.ori`

### LLVM

- [ ] **LLVM Support**: Falls through via trait dispatch — no special-casing needed
  - [ ] **AOT Tests**: `ori_llvm/tests/aot/operators.rs` — matmul trait dispatch

---

## 15C.10 Power Operator (`**`)

**Proposal**: `proposals/approved/power-operator-proposal.md`

Add `**` as a right-associative binary operator for exponentiation. Desugars to `Pow` trait method `power()`. Binds tighter than unary `-` (precedence level 2): `-x ** 2 = -(x ** 2)`. Compound assignment `**=` included.

### Lexer

- [ ] **Implement**: Add `StarStar` and `StarStarEq` raw token tags to `ori_lexer_core/src/tag/mod.rs`
  - [ ] **Rust Tests**: `ori_lexer_core/src/tag/tests.rs` — lexeme and display tests
- [ ] **Implement**: Update raw scanner to recognize `**` and `**=` (longest-match: `*` → peek `*` → peek `=`)
  - [ ] **Rust Tests**: `ori_lexer_core/src/raw_scanner/tests.rs` — power token scanning
- [ ] **Implement**: Map `StarStar` → `TokenKind::Pow` and `StarStarEq` → compound assignment in cooker
  - [ ] **Rust Tests**: `ori_lexer/src/cooker/tests.rs` — power token cooking

### IR

- [ ] **Implement**: Add `Pow` variant to `BinaryOp` + arms in `as_symbol()`, `precedence()`, `trait_method_name()`, `trait_name()`
  - [ ] **Rust Tests**: `ori_ir/src/ast/tests.rs` — BinaryOp::Pow methods

### Parser

- [ ] **Implement**: Add `parse_power_expr()` between `parse_unary_expr()` and `parse_postfix_expr()` — right-associative
  - `unary_expr` calls `power_expr`; `power_expr` calls `postfix_expr`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr/tests.rs` — power expression parsing
  - [ ] **Ori Tests**: `tests/spec/operators/power/basic.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/power/right_assoc.ori` — `2 ** 3 ** 2 = 512`
  - [ ] **Ori Tests**: `tests/spec/operators/power/unary_minus.ori` — `-2 ** 2 = -4`
  - [ ] **Ori Tests**: `tests/spec/operators/power/precedence.ori` — `a * b ** 2 = a * (b ** 2)`
- [ ] **Implement**: Parse `**=` compound assignment (desugar to `x = x ** y`)
  - [ ] **Ori Tests**: `tests/spec/operators/power/compound_assign.ori`

### Type Checker

- [ ] **Implement**: Falls through via `BinaryOp::trait_name()` returning `"Pow"` — no special-casing
  - [ ] **Ori Tests**: `tests/compile-fail/power_no_impl.ori` — "type `str` does not implement `Pow`"

### Evaluator

- [ ] **Implement**: Built-in `int ** int` dispatch (binary exponentiation, panic on negative exponent)
  - [ ] **Rust Tests**: `ori_eval/src/tests/` — int power evaluation
  - [ ] **Ori Tests**: `tests/spec/operators/power/int_power.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/power/negative_exponent_panic.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/power/zero_pow_zero.ori` — `0 ** 0 = 1`
- [ ] **Implement**: Built-in `float ** float` dispatch (delegates to libm `pow()`)
  - [ ] **Rust Tests**: `ori_eval/src/tests/` — float power evaluation
  - [ ] **Ori Tests**: `tests/spec/operators/power/float_power.ori`
- [ ] **Implement**: Mixed-type dispatch: `float ** int`, `int ** float` → `float`
  - [ ] **Ori Tests**: `tests/spec/operators/power/mixed_types.ori`
- [ ] **Implement**: Overflow follows standard overflow behavior (panic in debug)
  - [ ] **Ori Tests**: `tests/spec/operators/power/overflow.ori`

### Standard Library

- [ ] **Implement**: Add `Pow` trait definition to `library/std/prelude.ori`
  - Trait: `trait Pow<Rhs = Self> { type Output = Self; @power (self, rhs: Rhs) -> Self.Output }`
  - Built-in impls: `Pow for int`, `Pow for float`, `Pow<int> for float`, `Pow<float> for int`
  - [ ] **Ori Tests**: `tests/spec/traits/operators/pow_trait.ori`
  - [ ] **Ori Tests**: `tests/spec/traits/operators/pow_user_defined.ori`

### LLVM

- [ ] **LLVM Support**: Primitive impls via `llvm.pow` intrinsic; user types via trait dispatch
  - [ ] **AOT Tests**: `ori_llvm/tests/aot/operators.rs` — power operator codegen

---

## 15C.11 Pipe Operator (`|>`)

**Proposal**: `proposals/approved/pipe-operator-proposal.md`

Add `|>` for left-to-right function composition with implicit fill. The piped value fills the single parameter that has no default and is not provided in the call. Method calls on the piped value use `.method()` syntax. Lambda pipe steps (`|> (x -> expr)`) handle expression-level operations.

```ori
data
    |> filter(predicate: x -> x > 0)
    |> map(transform: x -> x * 2)
    |> sum
```

### Lexer

- [ ] **Implement**: Add `PipeArrow` raw token tag to `ori_lexer_core/src/tag/mod.rs`
  - [ ] **Rust Tests**: `ori_lexer_core/src/tag/tests.rs` — lexeme and display tests
- [ ] **Implement**: Update raw scanner to recognize `|>` (disambiguate from `|`)
  - [ ] **Rust Tests**: `ori_lexer_core/src/raw_scanner/tests.rs` — pipe token scanning
- [ ] **Implement**: Map `PipeArrow` → `TokenKind::Pipe` in cooker
  - [ ] **Rust Tests**: `ori_lexer/src/cooker/tests.rs` — pipe token cooking

### IR

- [ ] **Implement**: Add `Pipe` expression variant to `ExprKind` (LHS expression + pipe step)
  - Pipe step variants: function call (implicit fill), method call (`.method`), lambda
  - [ ] **Rust Tests**: `ori_ir/src/ast/tests.rs` — Pipe expression AST node

### Parser

- [ ] **Implement**: Parse `|>` at precedence 16 (below `??` at 15); produce `Pipe` AST node
  - Grammar: `pipe_expr = coalesce_expr { "|>" pipe_step } .`
  - Pipe step: `.method()` | `postfix_expr [call_args]` | `lambda`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr/tests.rs` — pipe expression parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/pipe/basic.ori` — simple pipe chains
  - [ ] **Ori Tests**: `tests/spec/expressions/pipe/method_call.ori` — `.method()` on piped value
  - [ ] **Ori Tests**: `tests/spec/expressions/pipe/lambda.ori` — lambda pipe steps
  - [ ] **Ori Tests**: `tests/spec/expressions/pipe/precedence.ori` — `a + b |> f` = `(a + b) |> f`
  - [ ] **Ori Tests**: `tests/spec/expressions/pipe/nested.ori` — `a |> f(x: b |> g)`

### Type Checker

- [ ] **Implement**: Resolve implicit fill — identify single unspecified param (no default, not in call)
  - Desugar to let-binding + ordinary function call
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/tests.rs` — pipe implicit fill resolution
  - [ ] **Ori Tests**: `tests/spec/expressions/pipe/implicit_fill.ori` — fills correct param
  - [ ] **Ori Tests**: `tests/spec/expressions/pipe/defaults.ori` — params with defaults excluded
  - [ ] **Ori Tests**: `tests/spec/expressions/pipe/punning.ori` — `x |> f(weight:, bias:)`

- [ ] **Implement**: Error diagnostics for pipe
  - Zero unspecified: "all parameters already specified; nothing for pipe to fill"
  - Multiple unspecified: "ambiguous pipe target; specify all parameters except one"
  - [ ] **Ori Tests**: `tests/compile-fail/pipe_all_specified.ori`
  - [ ] **Ori Tests**: `tests/compile-fail/pipe_ambiguous.ori`
  - [ ] **Ori Tests**: `tests/compile-fail/pipe_zero_params.ori`

- [ ] **Implement**: Desugar `.method()` pipe steps to `__pipe.method(args)` call
  - [ ] **Ori Tests**: `tests/spec/expressions/pipe/method_desugar.ori`

- [ ] **Implement**: Desugar lambda pipe steps to `(lambda)(__pipe)` call
  - [ ] **Ori Tests**: `tests/spec/expressions/pipe/lambda_desugar.ori`

- [ ] **Implement**: Handle `?` on pipe steps — applies to desugared call result
  - [ ] **Ori Tests**: `tests/spec/expressions/pipe/error_propagation.ori`

### Formatter

- [ ] **Implement**: Format pipe chains with line-break-per-step, indented under first operand
  - [ ] **Rust Tests**: `ori_fmt/src/formatter/tests.rs` — pipe chain formatting

### LLVM

- [ ] **LLVM Support**: No changes needed — type checker desugars before reaching LLVM codegen
  - [ ] **AOT Tests**: `ori_llvm/tests/aot/pipe.rs` — verify desugared form compiles correctly

---

## 15C.13 Byte Literals and Hex Escapes

**Proposal**: `proposals/approved/byte-literals-proposal.md`

Add `b'x'` byte literal syntax and `\xHH` hex escapes. Byte literals produce type `byte` (0–255). `\xHH` is also added to char literals (restricted to `\x00`–`\x7F`).

```ori
let space: byte = b' ';
let esc: byte = b'\x1B';
let max: byte = b'\xFF';
let tab_char: char = '\x09';
```

### Raw Scanner

- [ ] **Implement**: Modify identifier dispatch in `ori_lexer_core/src/raw_scanner/mod.rs` — peek for `'` after `b` to start byte literal scanning
  - [ ] **Rust Tests**: `ori_lexer_core/src/raw_scanner/tests.rs` — byte literal boundary detection
- [ ] **Implement**: Add `RawTag::ByteLiteral` and `RawTag::UnterminatedByteLiteral` variants
  - [ ] **Rust Tests**: `ori_lexer_core/src/tag/tests.rs` — lexeme and display tests
- [ ] **Implement**: Add `scan_byte_literal()` method in `strings.rs` — reuse `skip_escape_body()` with `\x` extension
  - [ ] **Rust Tests**: `ori_lexer_core/src/raw_scanner/tests.rs` — byte literal scanning (escapes, unterminated, multi-char)
- [ ] **Implement**: Extend `skip_escape_body()` to handle `\x` — consume exactly 2 hex digits
  - [ ] **Rust Tests**: `ori_lexer_core/src/raw_scanner/tests.rs` — hex escape boundary detection

### Cooker

- [ ] **Implement**: Add `cook_byte_literal()` method in `escape_cooking.rs` — strips `b'...'`, calls `unescape_byte_v2()`
  - [ ] **Rust Tests**: `ori_lexer/src/cooker/tests.rs` — byte literal cooking
- [ ] **Implement**: Add `unescape_byte_v2()` function in `cook_escape/mod.rs` — handles `\\` `\'` `\n` `\t` `\r` `\0` `\xHH`; rejects `\u{...}` and `\"`; produces `u8`
  - [ ] **Rust Tests**: `ori_lexer/src/cook_escape/tests.rs` — byte escape processing
- [ ] **Implement**: Extend `unescape_char_v2()` to handle `\xHH` — restricted to `\x00`–`\x7F`; error for `\x80`–`\xFF`
  - [ ] **Rust Tests**: `ori_lexer/src/cook_escape/tests.rs` — char hex escape processing
- [ ] **Implement**: Extend `unescape_with_context()` to handle `\xHH` in string and template literals — add `'x' =>` arm in shared escape match (restricted to `\x00`–`\x7F`; error for `\x80`–`\xFF`). This is one new match arm thanks to the Section 02 DRY consolidation.
  - [ ] **Rust Tests**: `ori_lexer/src/cook_escape/tests.rs` — string and template hex escape processing (valid ASCII, out-of-range error, mixed with other escapes)

### Token Kind

- [ ] **Implement**: Add `TokenKind::Byte(u8)` variant to `ori_ir/src/token/kind.rs`
  - [ ] **Rust Tests**: `ori_ir/src/token/tests.rs` — display, discriminant

### Parser

- [ ] **Implement**: Parse `TokenKind::Byte(u8)` as literal expression in `ori_parse`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr/tests.rs` — byte literal parsing
  - [ ] **Ori Tests**: `tests/spec/literals/byte_literal.ori`

### Type Checker

- [ ] **Implement**: Infer `byte` type for byte literal expressions in `ori_types`
  - [ ] **Rust Tests**: `ori_types/src/infer/expr/tests.rs` — byte literal type inference
  - [ ] **Ori Tests**: `tests/spec/types/byte_literal_type.ori`

### Evaluator

- [ ] **Implement**: Evaluate byte literals to `Value::Byte(u8)` in `ori_eval`
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/tests.rs` — byte literal evaluation
  - [ ] **Ori Tests**: `tests/spec/literals/byte_literal_eval.ori`

### LLVM Codegen

- [ ] **LLVM Support**: Emit byte literal as `i8` constant in `ori_llvm`
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/literals.rs` — byte literal codegen
  - [ ] **AOT Tests**: `tests/spec/literals/byte_literal_aot.ori`

### Error Messages

- [ ] **Implement**: Error for `\u{...}` in byte literal — "byte literal cannot contain unicode escape"
  - [ ] **Ori Tests**: `tests/compile-fail/byte_literal_unicode_escape.ori`
- [ ] **Implement**: Error for `\"` in byte literal — "invalid escape in byte literal"
  - [ ] **Ori Tests**: `tests/compile-fail/byte_literal_double_quote_escape.ori`
- [ ] **Implement**: Error for non-ASCII character in byte literal — "non-ASCII character in byte literal"
  - [ ] **Ori Tests**: `tests/compile-fail/byte_literal_non_ascii.ori`
- [ ] **Implement**: Error for `\x80`–`\xFF` in char literal — "\\x value exceeds ASCII range in char literal (use \\u{...})"
  - [ ] **Ori Tests**: `tests/compile-fail/char_literal_hex_out_of_range.ori`

---

## 15C.12 Section Completion Checklist

- [ ] All implementation items have checkboxes marked `[ ]`
- [ ] All spec docs updated
- [ ] CLAUDE.md updated with syntax changes
- [ ] Migration tools working
- [ ] All tests pass: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Literal and operator syntax proposals implemented
