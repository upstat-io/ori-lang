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

- [ ] **Implement**: Update parser to parse `#name(...)` syntax
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — simplified attribute parsing
  - [ ] **Ori Tests**: `tests/spec/attributes/simplified_syntax.ori`

- [ ] **Implement**: Support migration: accept both syntaxes temporarily
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — migration compatibility
  - [ ] **Ori Tests**: `tests/spec/attributes/migration.ori`

- [ ] **Implement**: Add deprecation warning for bracket syntax

- [ ] **Implement**: Update `ori fmt` to auto-migrate

---

## 15.2 function_seq vs function_exp Formalization

**Proposal**: `proposals/approved/function-seq-exp-distinction.md`

Formalize the distinction between sequential patterns and named-expression patterns.

**function_seq** (special syntax): `run`, `try`, `match`, `catch`
**function_exp** (named args): `recurse`, `parallel`, `spawn`, `timeout`, `cache`, `with`, `for`
~~**function_val** (positional): `int`, `float`, `str`, `byte`~~ — **REMOVED** by `as` proposal

> **NOTE**: The `as` conversion proposal (`proposals/drafts/as-conversion-proposal.md`)
> removes `function_val` entirely. Type conversions now use `x as T` / `x as? T` syntax,
> eliminating the special case for positional arguments.

### Implementation

- [ ] **Implement**: Verify AST has separate `FunctionSeq` and `FunctionExp` types
  - [ ] **Rust Tests**: `ori_ir/src/ast/expr.rs` — AST variant tests
  - [ ] **Ori Tests**: `tests/spec/patterns/function_seq_exp.ori`

- [ ] **Implement**: Parser allows positional for type conversions only
  - [ ] **Rust Tests**: `ori_parse/src/grammar/call.rs` — positional arg handling
  - [ ] **Ori Tests**: `tests/spec/expressions/type_conversions.ori`

- [ ] **Implement**: Parser enforces named args for all other builtins
  - [ ] **Rust Tests**: `ori_parse/src/grammar/call.rs` — named arg enforcement
  - [ ] **Ori Tests**: `tests/spec/expressions/builtin_named_args.ori`

- [ ] **Implement**: Add clear error message for positional args in builtins
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — positional arg error
  - [ ] **Ori Tests**: `tests/compile-fail/builtin_positional_args.ori`

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

- [ ] **Implement**: Update error messages to show new syntax
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — syntax error messages
  - [ ] **Ori Tests**: `tests/compile-fail/named_arg_syntax.ori`

- [ ] **Implement**: Add migration tool `ori migrate remove-dot-prefix`

- [ ] **Implement**: Update formatter with width-based stacking rule

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

- [ ] **Implement**: Add clear error message for inline comments

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

- [ ] **Implement**: Parser: Enforce position (pre_check first, post_check last)

- [ ] **Implement**: Type checker: Validate pre_check is `bool` or `[bool]`

- [ ] **Implement**: Type checker: Validate post_check is `T -> bool` or `T -> [bool]`

- [ ] **Implement**: Support custom messages with `| "message"` syntax

- [ ] **Implement**: Support list of conditions `[cond1, cond2]`

- [ ] **Implement**: Desugar to conditional checks and panics

- [ ] **Implement**: Add `$check_mode` global config (enforce/observe/ignore)

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

- [ ] **Implement**: Handle `{expr}` interpolation boundaries

- [ ] **Implement**: Handle `\{` and `\}` escape for literal braces

- [ ] **Implement**: Handle `` \` `` escape for literal backtick

### Parser

- [ ] **Implement**: Parse template strings as sequence of parts
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — template string parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/interpolation.ori`

- [ ] **Implement**: Parse interpolated expressions

- [ ] **Implement**: Parse optional format specifiers `{expr:spec}`

### Type System

- [ ] **Implement**: Interpolated expressions must implement `Printable`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/interpolation.rs` — printable constraint
  - [ ] **Ori Tests**: `tests/spec/types/printable.ori`

### Codegen

- [ ] **Implement**: Desugar template strings to concatenation

- [ ] **Implement**: Generate `to_str()` calls for interpolations

---

## 15.7 `as` Conversion Syntax

**Proposal**: `proposals/drafts/as-conversion-proposal.md`

Replace `int()`, `float()`, `str()`, `byte()` with `as`/`as?` keyword syntax.

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

### Parser

- [ ] **Implement**: Parse `expression as Type` as conversion expression
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — as expression parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/as_syntax.ori`

- [ ] **Implement**: Parse `expression as? Type` as fallible conversion
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — as? expression parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/as_fallible_syntax.ori`

### Type Checker

- [ ] **Implement**: Validate `as` only used with `As<T>` trait implementations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/as_expr.rs`
  - [ ] **Ori Tests**: `tests/compile-fail/as_not_implemented.ori`

- [ ] **Implement**: Validate `as?` only used with `TryAs<T>` trait implementations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/as_expr.rs`
  - [ ] **Ori Tests**: `tests/compile-fail/try_as_not_implemented.ori`

- [ ] **Implement**: Error when using `as` for fallible conversion (must use `as?`)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/as_expr.rs`
  - [ ] **Ori Tests**: `tests/compile-fail/as_fallible_conversion.ori`

### Codegen

- [ ] **Implement**: Desugar `x as T` to `As<T>.as(self: x)`
- [ ] **Implement**: Desugar `x as? T` to `TryAs<T>.try_as(self: x)`

### Migration

- [ ] **Implement**: Remove `int()`, `float()`, `str()`, `byte()` from parser
- [ ] **Implement**: Update error messages to suggest `as` syntax

---

## 15.8 Phase Completion Checklist

- [ ] All implementation items have checkboxes marked `[x]`
- [ ] All spec docs updated
- [ ] CLAUDE.md updated with syntax changes
- [ ] Migration tools working
- [ ] All tests pass: `cargo test && ori test tests/spec/`

**Exit Criteria**: All approved syntax changes implemented and documented
