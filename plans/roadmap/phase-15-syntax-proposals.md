# Phase 15: Approved Syntax Proposals

**Goal**: Implement approved syntax changes from V3 Phase 15.1-15.5

> **Source**: `docs/ori_lang/proposals/approved/`
> **Proposals**: `docs/ori_lang/proposals/approved/`

---

## 15.1 Simplified Attribute Syntax

**Proposal**: `proposals/approved/simplified-attributes-proposal.md`

Change attribute syntax from `#[name(...)]` to `#name(...)`.

```ori
// Before
#[derive(Eq, Clone)]
#[skip("reason")]

// After
#derive(Eq, Clone)
#skip("reason")
```

### Implementation

- [ ] **Implement**: Update lexer to emit `Hash` token instead of `HashBracket`
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — attribute token tests
  - [ ] **Ori Tests**: `tests/spec/attributes/simplified_syntax.ori`
  - [ ] **LLVM Support**: LLVM codegen for simplified attribute token
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/attribute_tests.rs` — simplified attribute token codegen

- [ ] **Implement**: Update parser to parse `#name(...)` syntax
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — simplified attribute parsing
  - [ ] **Ori Tests**: `tests/spec/attributes/simplified_syntax.ori`
  - [ ] **LLVM Support**: LLVM codegen for simplified attribute parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/attribute_tests.rs` — simplified attribute parsing codegen

- [ ] **Implement**: Support migration: accept both syntaxes temporarily
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — migration compatibility
  - [ ] **Ori Tests**: `tests/spec/attributes/migration.ori`
  - [ ] **LLVM Support**: LLVM codegen for attribute migration compatibility
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/attribute_tests.rs` — attribute migration codegen

- [ ] **Implement**: Add deprecation warning for bracket syntax
  - [ ] **LLVM Support**: LLVM codegen for deprecation warning
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/attribute_tests.rs` — deprecation warning codegen

- [ ] **Implement**: Update `ori fmt` to auto-migrate
  - [ ] **LLVM Support**: LLVM codegen for ori fmt auto-migrate
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/attribute_tests.rs` — ori fmt auto-migrate codegen

---

## 15.2 function_seq vs function_exp Formalization

**Proposal**: `proposals/approved/function-seq-exp-distinction.md`

Formalize the distinction between sequential patterns and named-expression patterns.

**function_seq** (special syntax): `run`, `try`, `match`, `catch`
**function_exp** (named args): `recurse`, `parallel`, `spawn`, `timeout`, `cache`, `with`, `for`
~~**function_val** (positional): `int`, `float`, `str`, `byte`~~ — **REMOVED** by `as` proposal

> **NOTE**: The `as` conversion proposal (`proposals/approved/as-conversion-proposal.md`)
> removes `function_val` entirely. Type conversions now use `x as T` / `x as? T` syntax,
> eliminating the special case for positional arguments.

### Implementation

- [ ] **Implement**: Verify AST has separate `FunctionSeq` and `FunctionExp` types
  - [ ] **Rust Tests**: `ori_ir/src/ast/expr.rs` — AST variant tests
  - [ ] **Ori Tests**: `tests/spec/patterns/function_seq_exp.ori`
  - [ ] **LLVM Support**: LLVM codegen for FunctionSeq and FunctionExp
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — FunctionSeq/FunctionExp codegen

- [ ] **Implement**: Parser allows positional for type conversions only
  - [ ] **Rust Tests**: `ori_parse/src/grammar/call.rs` — positional arg handling
  - [ ] **Ori Tests**: `tests/spec/expressions/type_conversions.ori`
  - [ ] **LLVM Support**: LLVM codegen for positional type conversions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — positional type conversions codegen

- [ ] **Implement**: Parser enforces named args for all other builtins
  - [ ] **Rust Tests**: `ori_parse/src/grammar/call.rs` — named arg enforcement
  - [ ] **Ori Tests**: `tests/spec/expressions/builtin_named_args.ori`
  - [ ] **LLVM Support**: LLVM codegen for named arg enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — named arg enforcement codegen

- [ ] **Implement**: Add clear error message for positional args in builtins
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — positional arg error
  - [ ] **Ori Tests**: `tests/compile-fail/builtin_positional_args.ori`
  - [ ] **LLVM Support**: LLVM codegen for positional arg error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — positional arg error codegen

---

## 15.3 Remove Dot Prefix from Named Arguments

**Proposal**: `proposals/approved/remove-dot-prefix-proposal.md`

Change named argument syntax from `.name: value` to `name: value`.

```ori
// Before
fetch_user(.id: 1)
map(.over: items, .transform: x -> x * 2)

// After
fetch_user(id: 1)
map(over: items, transform: x -> x * 2)
```

### Implementation

- [ ] **Implement**: Update parser to expect `IDENTIFIER ':'` instead of `'.' IDENTIFIER ':'`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/call.rs` — named arg syntax
  - [ ] **Ori Tests**: `tests/spec/expressions/named_args.ori`
  - [ ] **LLVM Support**: LLVM codegen for named arg syntax
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — named arg syntax codegen

- [ ] **Implement**: Update error messages to show new syntax
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — syntax error messages
  - [ ] **Ori Tests**: `tests/compile-fail/named_arg_syntax.ori`
  - [ ] **LLVM Support**: LLVM codegen for syntax error messages
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — syntax error messages codegen

- [ ] **Implement**: Add migration tool `ori migrate remove-dot-prefix`
  - [ ] **LLVM Support**: LLVM codegen for ori migrate tool
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — ori migrate codegen

- [ ] **Implement**: Update formatter with width-based stacking rule
  - [ ] **LLVM Support**: LLVM codegen for formatter width-based stacking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — formatter stacking codegen

---

## 15.4 Inline Comments Prohibition

Comments must appear on their own line. Inline comments are not allowed.

```ori
// This is valid
let x = 42

let y = 42  // SYNTAX ERROR
```

### Implementation

- [ ] **Implement**: Update lexer to reject inline comments
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — inline comment rejection
  - [ ] **Ori Tests**: `tests/compile-fail/inline_comments.ori`
  - [ ] **LLVM Support**: LLVM codegen for inline comment rejection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — inline comment rejection codegen

- [ ] **Implement**: Add clear error message for inline comments
  - [ ] **LLVM Support**: LLVM codegen for inline comment error message
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — inline comment error codegen

---

## 15.5 Pre/Post Checks for `run` Pattern

**Proposal**: `proposals/approved/checks-proposal.md`

Extend `run` pattern with `pre_check:` and `post_check:` properties.

```ori
@divide (a: int, b: int) -> int = run(
    pre_check: b != 0,
    a div b,
    post_check: r -> r * b <= a,
)
```

### Implementation

- [ ] **Implement**: Parser: Add `pre_check:` and `post_check:` to run pattern
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — check property parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/checks.ori`
  - [ ] **LLVM Support**: LLVM codegen for pre_check and post_check parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — pre_check/post_check parsing codegen

- [ ] **Implement**: Parser: Enforce position (pre_check first, post_check last)
  - [ ] **LLVM Support**: LLVM codegen for check position enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — check position enforcement codegen

- [ ] **Implement**: Type checker: Validate pre_check is `bool` or `[bool]`
  - [ ] **LLVM Support**: LLVM codegen for pre_check type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — pre_check type validation codegen

- [ ] **Implement**: Type checker: Validate post_check is `T -> bool` or `T -> [bool]`
  - [ ] **LLVM Support**: LLVM codegen for post_check type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — post_check type validation codegen

- [ ] **Implement**: Support custom messages with `| "message"` syntax
  - [ ] **LLVM Support**: LLVM codegen for custom check messages
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — custom check messages codegen

- [ ] **Implement**: Support list of conditions `[cond1, cond2]`
  - [ ] **LLVM Support**: LLVM codegen for list of conditions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — list of conditions codegen

- [ ] **Implement**: Desugar to conditional checks and panics
  - [ ] **LLVM Support**: LLVM codegen for check desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — check desugaring codegen

- [ ] **Implement**: Add `$check_mode` global config (enforce/observe/ignore)
  - [ ] **LLVM Support**: LLVM codegen for check_mode global config
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — check_mode global config codegen

---

## 15.6 String Interpolation

**Proposal**: `proposals/approved/string-interpolation-proposal.md`

Add template strings with backtick delimiters and `{expr}` interpolation.

```ori
let name = "Alice"
let age = 30
print(`Hello, {name}! You are {age} years old.`)
```

### Lexer

- [ ] **Implement**: Add template string literal tokenization
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — template string tokenization
  - [ ] **Ori Tests**: `tests/spec/lexical/template_strings.ori`
  - [ ] **LLVM Support**: LLVM codegen for template string tokenization
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template string tokenization codegen

- [ ] **Implement**: Handle `{expr}` interpolation boundaries
  - [ ] **LLVM Support**: LLVM codegen for interpolation boundaries
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — interpolation boundaries codegen

- [ ] **Implement**: Handle `\{` and `\}` escape for literal braces
  - [ ] **LLVM Support**: LLVM codegen for brace escaping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — brace escaping codegen

- [ ] **Implement**: Handle `` \` `` escape for literal backtick
  - [ ] **LLVM Support**: LLVM codegen for backtick escaping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — backtick escaping codegen

### Parser

- [ ] **Implement**: Parse template strings as sequence of parts
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — template string parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/interpolation.ori`
  - [ ] **LLVM Support**: LLVM codegen for template string parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template string parsing codegen

- [ ] **Implement**: Parse interpolated expressions
  - [ ] **LLVM Support**: LLVM codegen for interpolated expressions
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — interpolated expressions codegen

- [ ] **Implement**: Parse optional format specifiers `{expr:spec}`
  - [ ] **LLVM Support**: LLVM codegen for format specifiers
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — format specifiers codegen

### Type System

- [ ] **Implement**: Interpolated expressions must implement `Printable`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/interpolation.rs` — printable constraint
  - [ ] **Ori Tests**: `tests/spec/types/printable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Printable constraint
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — Printable constraint codegen

### Codegen

- [ ] **Implement**: Desugar template strings to concatenation
  - [ ] **LLVM Support**: LLVM codegen for template string desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — template string desugaring codegen

- [ ] **Implement**: Generate `to_str()` calls for interpolations
  - [ ] **LLVM Support**: LLVM codegen for to_str() calls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/interpolation_tests.rs` — to_str() calls codegen

---

## 15.7 `as` Conversion Syntax

**Proposal**: `proposals/approved/as-conversion-proposal.md`

Replace `int()`, `float()`, `str()`, `byte()` with `as`/`as?` keyword syntax backed by `As<T>` and `TryAs<T>` traits.

```ori
// Before (special-cased positional args)
let x = int("42")
let y = float(value)

// After (consistent keyword syntax)
let x = "42" as? int
let y = value as float
```

### Lexer

- [ ] **Implement**: `as` keyword token (if not already reserved)
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — as keyword tokenization
  - [ ] **Ori Tests**: `tests/spec/lexical/as_keyword.ori`
  - [ ] **LLVM Support**: LLVM codegen for as keyword
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as keyword codegen

### Parser

- [ ] **Implement**: Parse `expression as Type` as conversion expression
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — as expression parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/as_syntax.ori`
  - [ ] **LLVM Support**: LLVM codegen for as expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as expression codegen

- [ ] **Implement**: Parse `expression as? Type` as fallible conversion
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — as? expression parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/as_fallible_syntax.ori`
  - [ ] **LLVM Support**: LLVM codegen for as? expression
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as? expression codegen

### Type Checker

- [ ] **Implement**: Validate `as` only used with `As<T>` trait implementations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/as_expr.rs`
  - [ ] **Ori Tests**: `tests/compile-fail/as_not_implemented.ori`
  - [ ] **LLVM Support**: LLVM codegen for As<T> trait validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — As<T> trait validation codegen

- [ ] **Implement**: Validate `as?` only used with `TryAs<T>` trait implementations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/as_expr.rs`
  - [ ] **Ori Tests**: `tests/compile-fail/try_as_not_implemented.ori`
  - [ ] **LLVM Support**: LLVM codegen for TryAs<T> trait validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — TryAs<T> trait validation codegen

- [ ] **Implement**: Error when using `as` for fallible conversion (must use `as?`)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/as_expr.rs`
  - [ ] **Ori Tests**: `tests/compile-fail/as_fallible_conversion.ori`
  - [ ] **LLVM Support**: LLVM codegen for fallible conversion error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — fallible conversion error codegen

### Codegen

- [ ] **Implement**: Desugar `x as T` to `As<T>.as(self: x)`
  - [ ] **LLVM Support**: LLVM codegen for as desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as desugaring codegen
- [ ] **Implement**: Desugar `x as? T` to `TryAs<T>.try_as(self: x)`
  - [ ] **LLVM Support**: LLVM codegen for as? desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as? desugaring codegen

### Migration

- [ ] **Implement**: Remove `int()`, `float()`, `str()`, `byte()` from parser
  - [ ] **LLVM Support**: LLVM codegen for type conversion removal
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — type conversion removal codegen
- [ ] **Implement**: Update error messages to suggest `as` syntax
  - [ ] **LLVM Support**: LLVM codegen for as syntax suggestion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/as_conversion_tests.rs` — as syntax suggestion codegen

---

## 15.8 Phase Completion Checklist

- [ ] All implementation items have checkboxes marked `[x]`
- [ ] All spec docs updated
- [ ] CLAUDE.md updated with syntax changes
- [ ] Migration tools working
- [ ] All tests pass: `./test-all`

**Exit Criteria**: All approved syntax changes implemented and documented
