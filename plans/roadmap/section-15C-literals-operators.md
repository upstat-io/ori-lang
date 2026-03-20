---
section: "15C"
title: Literals & Operators
status: in-progress
reviewed: false
tier: 5
goal: Implement string interpolation, spread operator, range step syntax, and pipe operator
sections:
  - id: "15C.1"
    title: String Interpolation
    status: in-progress
  - id: "15C.2"
    title: Spread Operator
    status: not-started
  - id: "15C.3"
    title: Range with Step
    status: not-started
  - id: "15C.4"
    title: Computed Map Keys
    status: not-started
  - id: "15C.5"
    title: Floor Division (div) Operator Fix
    status: not-started
  - id: "15C.6"
    title: Decimal Duration and Size Literals
    status: not-started
  - id: "15C.7"
    title: Null Coalesce Operator
    status: in-progress
  - id: "15C.8"
    title: Compound Assignment Operators
    status: in-progress
  - id: "15C.9"
    title: MatMul Operator
    status: in-progress
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

- [x] **Implement**: Add template string literal tokenization (backtick delimited)
  - Implemented in `ori_lexer_core/src/raw_scanner/templates.rs` with SIMD-accelerated `skip_to_template_delim()`. Tags: `TemplateHead`, `TemplateTail`, `TemplateComplete`.
  - [ ] **Ori Tests**: `tests/spec/lexical/template_strings.ori`
  - [ ] **LLVM Support**: LLVM codegen for template string tokenization
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template string tokenization codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Handle `{expr}` interpolation boundaries (switch lexer modes)
  - Scanner manages `template_depth` stack for nested interpolation.
  - [ ] **LLVM Support**: LLVM codegen for interpolation boundaries
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — interpolation boundaries codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Handle `{{` and `}}` escape for literal braces
  - Implemented in templates.rs (line 29-33: peek for `{` to distinguish escape from interpolation).
  - [ ] **Ori Tests**: `tests/spec/lexical/template_brace_escape.ori`
  - [ ] **LLVM Support**: LLVM codegen for brace escaping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — brace escaping codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Handle `` \` `` escape for literal backtick
  - [ ] **Ori Tests**: `tests/spec/lexical/template_backtick_escape.ori`
  - [ ] **LLVM Support**: LLVM codegen for backtick escaping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — backtick escaping codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Support escapes: `\\`, `\n`, `\t`, `\r`, `\0` in template strings
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

- [ ] **Implement**: Parse template strings as sequence of `StringPart` (Literal | Interpolation)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — template string parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/interpolation.ori`
  - [ ] **LLVM Support**: LLVM codegen for template string parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template string parsing codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Parse interpolated expressions (full expression grammar inside `{}`)
  - [ ] **Ori Tests**: `tests/spec/expressions/interpolation_expressions.ori`
  - [ ] **LLVM Support**: LLVM codegen for interpolated expressions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — interpolated expressions codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Parse optional format specifiers `{expr:spec}`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/format_spec.rs` — format spec parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/format_specifiers.ori`
  - [ ] **LLVM Support**: LLVM codegen for format specifiers
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format specifiers codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Parse format spec grammar: `[[fill]align][width][.precision][type]`
  - [ ] **Ori Tests**: `tests/spec/expressions/format_spec_grammar.ori`
  - [ ] **LLVM Support**: LLVM codegen for format spec grammar
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format spec grammar codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Type System

- [ ] **Implement**: Interpolated expressions must implement `Printable`
  - [ ] **Rust Tests**: `ori_types/src/check/interpolation.rs` — printable constraint
  - [ ] **Ori Tests**: `tests/spec/types/printable_interpolation.ori`
  - [ ] **LLVM Support**: LLVM codegen for Printable constraint
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — Printable constraint codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Validate format spec type compatibility (e.g., `x`/`X`/`b`/`o` only for int)
  - [ ] **Rust Tests**: `ori_types/src/check/format_spec.rs` — format spec type validation
  - [ ] **Ori Tests**: `tests/compile-fail/format_spec_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for format spec type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format spec type validation codegen

### Standard Library

- [ ] **Implement**: `Formattable` trait definition
  - [ ] **Ori Tests**: `tests/spec/traits/formattable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Formattable trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — Formattable trait codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `FormatSpec` type definition
  - [ ] **Ori Tests**: `tests/spec/types/format_spec.ori`
  - [ ] **LLVM Support**: LLVM codegen for FormatSpec type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — FormatSpec type codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Alignment` and `FormatType` sum types
  - [ ] **Ori Tests**: `tests/spec/types/format_enums.ori`
  - [ ] **LLVM Support**: LLVM codegen for format enums
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format enums codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Blanket impl `T: Formattable` where `T: Printable`
  - [ ] **Ori Tests**: `tests/spec/traits/formattable_blanket.ori`
  - [ ] **LLVM Support**: LLVM codegen for blanket impl
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — blanket impl codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `apply_format` helper for width/alignment/padding
  - [ ] **Ori Tests**: `tests/spec/stdlib/apply_format.ori`
  - [ ] **LLVM Support**: LLVM codegen for apply_format
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — apply_format codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Codegen

- [ ] **Implement**: Desugar template strings to concatenation with `to_str()` calls
  - [ ] **Rust Tests**: `ori_types/src/check/interpolation_desugar.rs` — template desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/interpolation_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for template string desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template string desugaring codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Desugar format specifiers to `format(value, FormatSpec {...})` calls
  - [ ] **Rust Tests**: `ori_types/src/check/format_spec_desugar.rs` — format spec desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/format_spec_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for format spec desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format spec desugaring codegen
  - [ ] **AOT Tests**: No AOT coverage yet

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
  - [ ] **AOT Tests**: No AOT coverage yet

### Parser

- [ ] **Implement**: Parse `...expression` in list literals
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — list spread parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/list_spread.ori`
  - [ ] **LLVM Support**: LLVM codegen for list spread
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — list spread codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Parse `...expression` in map literals
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — map spread parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/map_spread.ori`
  - [ ] **LLVM Support**: LLVM codegen for map spread
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — map spread codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Parse `...expression` in struct literals
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — struct spread parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/struct_spread.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct spread
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — struct spread codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Type Checker

- [ ] **Implement**: Verify list spread expression is `[T]` matching container
  - [ ] **Rust Tests**: `ori_types/src/check/spread.rs` — list spread type checking
  - [ ] **Ori Tests**: `tests/compile-fail/list_spread_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for list spread type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — list spread type checking codegen

- [ ] **Implement**: Verify map spread expression is `{K: V}` matching container
  - [ ] **Rust Tests**: `ori_types/src/check/spread.rs` — map spread type checking
  - [ ] **Ori Tests**: `tests/compile-fail/map_spread_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for map spread type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — map spread type checking codegen

- [ ] **Implement**: Verify struct spread is same struct type (no subset/superset)
  - [ ] **Rust Tests**: `ori_types/src/check/spread.rs` — struct spread type checking
  - [ ] **Ori Tests**: `tests/compile-fail/struct_spread_wrong_type.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct spread type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — struct spread type checking codegen

- [ ] **Implement**: Track struct field coverage (spread + explicit must cover all fields)
  - [ ] **Rust Tests**: `ori_types/src/check/struct_lit.rs` — field coverage tracking
  - [ ] **Ori Tests**: `tests/compile-fail/struct_spread_missing_fields.ori`
  - [ ] **LLVM Support**: LLVM codegen for field coverage tracking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — field coverage tracking codegen

### Code Generation

- [ ] **Implement**: Desugar list spread to concatenation (`[a] + b + [c]`)
  - [ ] **Rust Tests**: `ori_llvm/src/codegen/spread.rs` — list spread desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/list_spread_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for list spread desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — list spread desugaring codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Desugar map spread to merge calls
  - [ ] **Rust Tests**: `ori_llvm/src/codegen/spread.rs` — map spread desugaring
  - [ ] **Ori Tests**: `tests/spec/expressions/map_spread_desugar.ori`
  - [ ] **LLVM Support**: LLVM codegen for map spread desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/spread_tests.rs` — map spread desugaring codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Desugar struct spread to explicit field assignments
  - [ ] **Rust Tests**: `ori_llvm/src/codegen/spread.rs` — struct spread desugaring
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
  - [ ] **AOT Tests**: No AOT coverage yet

### Parser

- [ ] **Implement**: Extend `range_expr` to accept `[ "by" shift_expr ]`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — range step parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/range_step.ori`
  - [ ] **LLVM Support**: LLVM codegen for range step parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — range step parsing codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Type Checker

- [ ] **Implement**: Validate step expression has same type as range bounds
  - [ ] **Rust Tests**: `ori_types/src/check/range.rs` — step type checking
  - [ ] **Ori Tests**: `tests/compile-fail/range_step_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for step type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — step type checking codegen

- [ ] **Implement**: Restrict `by` to integer ranges only (compile-time error for float)
  - [ ] **Rust Tests**: `ori_types/src/check/range.rs` — int-only restriction
  - [ ] **Ori Tests**: `tests/compile-fail/range_step_float.ori`
  - [ ] **LLVM Support**: LLVM codegen for int-only restriction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — int-only restriction codegen

### Code Generation / Evaluator

- [ ] **Implement**: Extend Range type with optional step field (default 1)
  - [ ] **Rust Tests**: `ori_ir/src/types.rs` — Range type extension
  - [ ] **Ori Tests**: `tests/spec/types/range_with_step.ori`
  - [ ] **LLVM Support**: LLVM codegen for Range type extension
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — Range type extension codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Iterator for stepped ranges (ascending and descending)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/iter.rs` — stepped range iteration
  - [ ] **Ori Tests**: `tests/spec/expressions/range_step_iteration.ori`
  - [ ] **LLVM Support**: LLVM codegen for stepped range iteration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — stepped range iteration codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Runtime panic for zero step
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/range.rs` — zero step panic
  - [ ] **Ori Tests**: `tests/spec/expressions/range_step_zero_panic.ori`
  - [ ] **LLVM Support**: LLVM codegen for zero step panic
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_step_tests.rs` — zero step panic codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Empty range for mismatched direction (no panic)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/range.rs` — direction mismatch
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

- [ ] **Implement**: Add `TokenKind::Div` case to `match_multiplicative_op()`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr/operators.rs` — div operator parsing
  - [ ] **Ori Tests**: `tests/spec/operators/div_floor.ori`
  - [ ] **LLVM Support**: LLVM codegen for div operator
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

- [ ] **Implement**: Parse decimal duration literals (`1.5s`, `0.25h`, etc.)
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — decimal duration tokenization
  - [ ] **Ori Tests**: `tests/spec/lexical/decimal_duration.ori`
  - [ ] **LLVM Support**: LLVM codegen for decimal duration literals
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/literal_tests.rs` — decimal duration codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Parse decimal size literals (`1.5kb`, `0.5mb`, etc.)
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — decimal size tokenization
  - [ ] **Ori Tests**: `tests/spec/lexical/decimal_size.ori`
  - [ ] **LLVM Support**: LLVM codegen for decimal size literals
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/literal_tests.rs` — decimal size codegen
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Integer arithmetic conversion (no floats involved)
  - [ ] **Rust Tests**: `ori_lexer/src/convert.rs` — integer-only decimal conversion
  - [ ] **Ori Tests**: `tests/spec/lexical/decimal_precision.ori`

- [ ] **Implement**: Validation for whole-number results
  - [ ] **Rust Tests**: `ori_lexer/src/convert.rs` — non-whole number rejection
  - [ ] **Ori Tests**: `tests/compile-fail/decimal_duration_not_whole.ori`
  - [ ] **Ori Tests**: `tests/compile-fail/decimal_size_not_whole.ori`

### Token Changes

- [ ] **Implement**: Remove `FloatDurationError` and `FloatSizeError` token types
  - [ ] **Rust Tests**: `ori_ir/src/token.rs` — token type cleanup

- [ ] **Implement**: Store Duration/Size tokens as computed base unit value
  - [ ] **Rust Tests**: `ori_ir/src/token.rs` — token value storage

### Error Messages

- [ ] **Implement**: E0911 error for non-representable decimal literals
  - [ ] **Rust Tests**: `ori_diagnostic/src/error_code.rs` — E0911 update
  - [ ] **Ori Tests**: `tests/compile-fail/e0911_decimal_precision.ori`

### Size Unit Change

- [ ] **Implement**: Change Size unit multipliers from 1024 to 1000
  - [ ] **Rust Tests**: `ori_lexer/src/convert.rs` — SI unit multipliers
  - [ ] **Ori Tests**: `tests/spec/types/size_si_units.ori`
  - [ ] **LLVM Support**: LLVM codegen with SI units
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

**Status**: Parser complete (tokenization, AST), evaluator incomplete.

### Evaluator

- [x] **Implement**: Evaluate `??` for `Option<T>` — extract Some value or use default
  - Implemented in `can_eval/operators.rs`. Lexer (`QuestionQuestion` token), parser (coalesce precedence level), evaluator, and type checker all handle `??`. Tests exist in `tests/spec/expressions/coalesce.ori` (all pass in full suite).
  - [ ] **LLVM Support**: LLVM codegen for null coalesce operator
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/coalesce_tests.rs` — coalesce codegen
  - [ ] **AOT Tests**: No AOT coverage yet

### Type Checker

- [x] **Implement**: Infer type for `a ?? b` — result is `T` where `a: Option<T>` and `b: T`
  - Implemented in `ori_types/src/infer/expr/operators.rs`.
  - [ ] **Ori Tests**: `tests/spec/types/coalesce_inference.ori`

- [ ] **Implement**: Error for non-Option left operand
  - [ ] **Rust Tests**: `ori_types/src/infer/ (tests)` — coalesce type error
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/coalesce_non_option.ori`

### Edge Cases

- [x] **Implement**: Short-circuit evaluation — don't evaluate right side if left is Some
  - Confirmed by tests that panic on default evaluation when left is Some.
  - [ ] **Ori Tests**: `tests/spec/expressions/coalesce_short_circuit.ori`

- [ ] **Implement**: Chained coalesce — `a ?? b ?? c`
  - Partially implemented. Some chaining tests have type info issues.
  - [ ] **Ori Tests**: `tests/spec/expressions/coalesce_chained.ori`

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

- [x] **Implement**: Add 13 new raw token tags to `ori_lexer_core/src/tag/mod.rs`
  - `PlusEq`, `MinusEq`, `StarEq`, `SlashEq`, `PercentEq`, `AtEq`, `AmpEq`, `PipeEq`, `CaretEq`, `ShlEq`, `ShrEq`, `AmpAmpEq`, `PipePipeEq` — all present (21 files reference compound assignment).
  - [ ] **Rust Tests**: `ori_lexer_core/src/tag/tests.rs` — lexeme and display tests
  - [ ] **Rust Tests**: `ori_lexer_core/src/raw_scanner/tests.rs` — scanning tests

- [x] **Implement**: Update raw scanner to scan compound assignment tokens
  - Two-char and three-char compound tokens handled in `ori_lexer_core/src/raw_scanner/operators.rs`.
  - [ ] **Rust Tests**: `ori_lexer_core/src/raw_scanner/tests.rs` — replace `no_compound_assignment` test

- [ ] **Implement**: Map raw tags to `TokenKind` in cooker
  - [ ] **Rust Tests**: `ori_lexer/src/cooker/tests.rs` — compound assignment cooking

### Parser

- [x] **Implement**: Parse compound assignment and desugar to `Assign { target, value: Binary/And/Or }`
  - Implemented in `ori_parse/src/grammar/expr/mod.rs` with `compound_assign_op()` and `desugar_compound_assign()`. Desugars to `Assign { target, value: Binary }`.
  - [ ] **Ori Tests**: `tests/spec/operators/compound_assignment/basic.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/compound_assignment/field_access.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/compound_assignment/subscript.ori`
  - [ ] **Ori Tests**: `tests/spec/operators/compound_assignment/logical.ori`

- [ ] **Implement**: Remove compound assignment from "common mistake" detection
  - Remove `+=`, `-=`, `*=`, `/=`, `%=` from `mistakes.rs`
  - Remove `&&=`, `||=` from `mistakes.rs`
  - Keep `??=` as mistake (still unsupported)
  - [ ] **Rust Tests**: `ori_parse/src/error/tests.rs` — update detection tests

### Error Messages

- [ ] **Implement**: Error for compound assignment on immutable binding (`$`)
  - Message: "cannot use compound assignment on immutable binding `$y`. Remove `$` for mutability: `let y = ...`"
  - [ ] **Ori Tests**: `tests/compile-fail/compound_assign_immutable.ori`

- [ ] **Implement**: Error for compound assignment as expression
  - Message: "compound assignment is a statement, not an expression"
  - [ ] **Ori Tests**: `tests/compile-fail/compound_assign_as_expression.ori`

### LLVM Support

- [ ] **LLVM Support**: No changes needed — parser desugars before reaching LLVM codegen
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/compound_assign_tests.rs` — verify desugared form compiles correctly
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 15C.9 MatMul Operator (`@`)

**Proposal**: `proposals/approved/matmul-operator-proposal.md`

Add `@` as a binary operator for matrix multiplication. Desugars to `MatMul` trait method `matrix_multiply()`. Same precedence as `*`/`/`/`%`/`div` (level 4, multiplicative). The `@` token is disambiguated by syntactic context (item position = function declaration, expression position = matmul, pattern position = at-binding).

### IR

- [x] **Implement**: Add `MatMul` variant to `BinaryOp` + arms in `as_symbol()`, `precedence()`, `trait_method_name()`, `trait_name()`
  - `BinaryOp::MatMul` exists with `as_symbol()` returning `"@"`, precedence 3, `trait_method_name()` returning `"mat_mul"`, `trait_name()` returning `"MatMul"`. Evaluator dispatch at `operator_dispatch.rs`.
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

**Exit Criteria**: Literal and operator syntax proposals implemented
