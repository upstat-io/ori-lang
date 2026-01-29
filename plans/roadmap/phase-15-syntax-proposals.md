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

Extend `run` pattern with `pre_check:` and `post_check:` properties for contract-style defensive programming.

```ori
@divide (a: int, b: int) -> int = run(
    pre_check: b != 0,
    a div b,
    post_check: r -> r * b <= a,
)

// Multiple conditions via multiple properties
@transfer (from: Account, to: Account, amount: int) -> (Account, Account) = run(
    pre_check: amount > 0 | "amount must be positive",
    pre_check: from.balance >= amount | "insufficient funds",
    // ... body ...,
    post_check: (f, t) -> f.balance == from.balance - amount,
    post_check: (f, t) -> t.balance == to.balance + amount,
)
```

### Key Design Decisions

- **Multiple properties, not list syntax**: Use multiple `pre_check:` / `post_check:` properties instead of `[cond1, cond2]` lists
- **`|` for messages**: Custom messages use `condition | "message"` syntax (parser disambiguates by context)
- **Scope constraints**: `pre_check:` can only access outer scope; `post_check:` can access body bindings
- **Void body**: Compile error if `post_check:` used with void body
- **Check modes deferred**: `check_mode:` (enforce/observe/ignore) deferred to future proposal

### Implementation

- [ ] **Implement**: Parser: Add `pre_check:` and `post_check:` to run pattern
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — check property parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/checks.ori`
  - [ ] **LLVM Support**: LLVM codegen for pre_check and post_check parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — pre_check/post_check parsing codegen

- [ ] **Implement**: Parser: Enforce position (pre_check first, post_check last)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — position enforcement
  - [ ] **Ori Tests**: `tests/compile-fail/checks/mispositioned_checks.ori`
  - [ ] **LLVM Support**: LLVM codegen for check position enforcement
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — check position enforcement codegen

- [ ] **Implement**: Parser: Support `| "message"` custom message syntax
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — message parsing
  - [ ] **Ori Tests**: `tests/spec/patterns/check_messages.ori`
  - [ ] **LLVM Support**: LLVM codegen for custom check messages
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — custom check messages codegen

- [ ] **Implement**: Type checker: Validate pre_check is `bool`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/pattern.rs` — pre_check type validation
  - [ ] **Ori Tests**: `tests/compile-fail/checks/pre_check_not_bool.ori`
  - [ ] **LLVM Support**: LLVM codegen for pre_check type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — pre_check type validation codegen

- [ ] **Implement**: Type checker: Validate post_check is `T -> bool` lambda
  - [ ] **Rust Tests**: `oric/src/typeck/checker/pattern.rs` — post_check type validation
  - [ ] **Ori Tests**: `tests/compile-fail/checks/post_check_not_lambda.ori`
  - [ ] **LLVM Support**: LLVM codegen for post_check type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — post_check type validation codegen

- [ ] **Implement**: Type checker: Error when post_check used with void body
  - [ ] **Rust Tests**: `oric/src/typeck/checker/pattern.rs` — void body error
  - [ ] **Ori Tests**: `tests/compile-fail/checks/post_check_void_body.ori`
  - [ ] **LLVM Support**: LLVM codegen for void body error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — void body error codegen

- [ ] **Implement**: Scope checker: pre_check can only access outer scope
  - [ ] **Rust Tests**: `oric/src/resolve/scope.rs` — pre_check scope validation
  - [ ] **Ori Tests**: `tests/compile-fail/checks/pre_check_scope.ori`
  - [ ] **LLVM Support**: LLVM codegen for pre_check scope validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — pre_check scope validation codegen

- [ ] **Implement**: Codegen: Desugar to conditional checks and panics
  - [ ] **Rust Tests**: `oric/src/desugar/checks.rs` — check desugaring
  - [ ] **Ori Tests**: `tests/spec/patterns/checks_desugaring.ori`
  - [ ] **LLVM Support**: LLVM codegen for check desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — check desugaring codegen

- [ ] **Implement**: Codegen: Embed source text for default error messages
  - [ ] **Rust Tests**: `oric/src/desugar/checks.rs` — source text embedding
  - [ ] **Ori Tests**: `tests/spec/patterns/checks_error_messages.ori`
  - [ ] **LLVM Support**: LLVM codegen for source text embedding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/syntax_tests.rs` — source text embedding codegen

---

## 15.6 String Interpolation

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

## 15.8 Default Parameter Values

**Proposal**: `proposals/approved/default-parameters-proposal.md`

Allow function parameters to specify default values, enabling callers to omit arguments.

```ori
@greet (name: str = "World") -> str = `Hello, {name}!`

greet()               // "Hello, World!"
greet(name: "Alice")  // "Hello, Alice!"
```

### Parser

- [ ] **Implement**: Extend `param` production to accept `= expression` after type
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — default parameter parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/default_params.ori`
  - [ ] **LLVM Support**: LLVM codegen for default parameter parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default parameter parsing codegen

- [ ] **Implement**: Parse default expressions with correct precedence
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — default expression precedence
  - [ ] **Ori Tests**: `tests/spec/declarations/default_params_precedence.ori`
  - [ ] **LLVM Support**: LLVM codegen for default expression precedence
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default expression precedence codegen

### Type Checker

- [ ] **Implement**: Verify default expression has parameter's type
  - [ ] **Rust Tests**: `oric/src/typeck/checker/params.rs` — default type checking
  - [ ] **Ori Tests**: `tests/compile-fail/default_param_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for default type checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default type checking codegen

- [ ] **Implement**: Verify default doesn't reference other parameters
  - [ ] **Rust Tests**: `oric/src/typeck/checker/params.rs` — default param reference checking
  - [ ] **Ori Tests**: `tests/compile-fail/default_param_references_other.ori`
  - [ ] **LLVM Support**: LLVM codegen for default param reference checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default param reference checking codegen

- [ ] **Implement**: Track which parameters have defaults for call validation
  - [ ] **Rust Tests**: `oric/src/typeck/checker/call.rs` — optional parameter tracking
  - [ ] **Ori Tests**: `tests/spec/expressions/call_with_defaults.ori`
  - [ ] **LLVM Support**: LLVM codegen for optional parameter tracking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — optional parameter tracking codegen

- [ ] **Implement**: Capability checking for default expressions
  - [ ] **Rust Tests**: `oric/src/typeck/checker/params.rs` — default capability checking
  - [ ] **Ori Tests**: `tests/spec/capabilities/default_param_capabilities.ori`
  - [ ] **LLVM Support**: LLVM codegen for default capability checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default capability checking codegen

### Call Site Validation

- [ ] **Implement**: Required parameters (no default) must be provided
  - [ ] **Rust Tests**: `oric/src/typeck/checker/call.rs` — required param validation
  - [ ] **Ori Tests**: `tests/compile-fail/missing_required_param.ori`
  - [ ] **LLVM Support**: LLVM codegen for required param validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — required param validation codegen

- [ ] **Implement**: Allow omitting parameters with defaults
  - [ ] **Rust Tests**: `oric/src/typeck/checker/call.rs` — optional param omission
  - [ ] **Ori Tests**: `tests/spec/expressions/omit_default_params.ori`
  - [ ] **LLVM Support**: LLVM codegen for optional param omission
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — optional param omission codegen

- [ ] **Implement**: Clear error message when required param missing
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — missing param error
  - [ ] **Ori Tests**: `tests/compile-fail/missing_required_param_message.ori`
  - [ ] **LLVM Support**: LLVM codegen for missing param error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — missing param error codegen

### Code Generation

- [ ] **Implement**: Insert default expressions for omitted arguments
  - [ ] **Rust Tests**: `oric/src/codegen/call.rs` — default insertion
  - [ ] **Ori Tests**: `tests/spec/expressions/default_insertion.ori`
  - [ ] **LLVM Support**: LLVM codegen for default insertion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — default insertion codegen

- [ ] **Implement**: Evaluate defaults at call time (not definition time)
  - [ ] **Rust Tests**: `oric/src/codegen/call.rs` — call-time evaluation
  - [ ] **Ori Tests**: `tests/spec/expressions/default_call_time_eval.ori`
  - [ ] **LLVM Support**: LLVM codegen for call-time evaluation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — call-time evaluation codegen

- [ ] **Implement**: Correct evaluation order (explicit args first, then defaults in param order)
  - [ ] **Rust Tests**: `oric/src/codegen/call.rs` — evaluation order
  - [ ] **Ori Tests**: `tests/spec/expressions/default_eval_order.ori`
  - [ ] **LLVM Support**: LLVM codegen for evaluation order
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — evaluation order codegen

### Trait Method Defaults

- [ ] **Implement**: Allow defaults in trait method signatures
  - [ ] **Rust Tests**: `ori_parse/src/grammar/trait.rs` — trait method default parsing
  - [ ] **Ori Tests**: `tests/spec/traits/method_defaults.ori`
  - [ ] **LLVM Support**: LLVM codegen for trait method defaults
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — trait method defaults codegen

- [ ] **Implement**: Allow implementations to override/remove defaults
  - [ ] **Rust Tests**: `oric/src/typeck/checker/impl.rs` — impl default override
  - [ ] **Ori Tests**: `tests/spec/traits/impl_override_defaults.ori`
  - [ ] **LLVM Support**: LLVM codegen for impl default override
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — impl default override codegen

- [ ] **Implement**: Trait object calls use trait's declared default
  - [ ] **Rust Tests**: `oric/src/codegen/dyn_dispatch.rs` — dyn default dispatch
  - [ ] **Ori Tests**: `tests/spec/traits/dyn_trait_defaults.ori`
  - [ ] **LLVM Support**: LLVM codegen for dyn default dispatch
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_params_tests.rs` — dyn default dispatch codegen

---

## 15.9 Multiple Function Clauses

**Proposal**: `proposals/approved/function-clauses-proposal.md`

Allow functions to be defined with multiple clauses that pattern match on arguments.

```ori
@factorial (0: int) -> int = 1
@factorial (n) -> int = n * factorial(n - 1)

@abs (n: int) -> int if n < 0 = -n
@abs (n) -> int = n
```

### Parser

- [ ] **Implement**: Allow `match_pattern` in parameter position (`clause_param`)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — clause parameter parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/function_clauses.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause parameter parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause parameter parsing codegen

- [ ] **Implement**: Parse `if` guard clause between `where_clause` and `=`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — guard clause parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/function_clause_guards.ori`
  - [ ] **LLVM Support**: LLVM codegen for guard clause parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — guard clause parsing codegen

- [ ] **Implement**: Group multiple declarations with same name into single function
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — clause grouping
  - [ ] **Ori Tests**: `tests/spec/declarations/function_clause_grouping.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause grouping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause grouping codegen

### Semantic Analysis

- [ ] **Implement**: Validate all clauses have same parameter count
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — parameter count validation
  - [ ] **Ori Tests**: `tests/compile-fail/clause_param_count_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for parameter count validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — parameter count validation codegen

- [ ] **Implement**: Validate all clauses have same return type
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — return type validation
  - [ ] **Ori Tests**: `tests/compile-fail/clause_return_type_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for return type validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — return type validation codegen

- [ ] **Implement**: Validate all clauses have same capabilities (`uses`)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — capability validation
  - [ ] **Ori Tests**: `tests/compile-fail/clause_capability_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for capability validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — capability validation codegen

- [ ] **Implement**: First clause rules (visibility, generics, types)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — first clause signature
  - [ ] **Ori Tests**: `tests/spec/declarations/first_clause_rules.ori`
  - [ ] **LLVM Support**: LLVM codegen for first clause rules
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — first clause rules codegen

- [ ] **Implement**: Type inference for subsequent clause parameters
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — clause type inference
  - [ ] **Ori Tests**: `tests/spec/declarations/clause_type_inference.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause type inference
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause type inference codegen

- [ ] **Implement**: Error if visibility/generics repeated on subsequent clauses
  - [ ] **Rust Tests**: `oric/src/typeck/checker/clauses.rs` — duplicate modifier errors
  - [ ] **Ori Tests**: `tests/compile-fail/clause_duplicate_modifiers.ori`
  - [ ] **LLVM Support**: LLVM codegen for duplicate modifier errors
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — duplicate modifier errors codegen

### Exhaustiveness & Reachability

- [ ] **Implement**: Exhaustiveness checking across all clauses
  - [ ] **Rust Tests**: `oric/src/typeck/exhaustiveness.rs` — clause exhaustiveness
  - [ ] **Ori Tests**: `tests/compile-fail/clause_non_exhaustive.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause exhaustiveness
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause exhaustiveness codegen

- [ ] **Implement**: Unreachable clause detection and warnings
  - [ ] **Rust Tests**: `oric/src/typeck/exhaustiveness.rs` — unreachable clause warning
  - [ ] **Ori Tests**: `tests/warnings/unreachable_clause.ori`
  - [ ] **LLVM Support**: LLVM codegen for unreachable clause warning
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — unreachable clause warning codegen

### Code Generation

- [ ] **Implement**: Desugar clauses to single function with `match`
  - [ ] **Rust Tests**: `oric/src/codegen/clauses.rs` — clause desugaring
  - [ ] **Ori Tests**: `tests/spec/declarations/clause_desugaring.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause desugaring codegen

- [ ] **Implement**: Desugar `if` guards to `.match()` in patterns
  - [ ] **Rust Tests**: `oric/src/codegen/clauses.rs` — guard desugaring
  - [ ] **Ori Tests**: `tests/spec/declarations/guard_desugaring.ori`
  - [ ] **LLVM Support**: LLVM codegen for guard desugaring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — guard desugaring codegen

### Integration

- [ ] **Implement**: Named argument reordering before pattern matching
  - [ ] **Rust Tests**: `oric/src/codegen/call.rs` — argument reordering
  - [ ] **Ori Tests**: `tests/spec/expressions/clause_named_args.ori`
  - [ ] **LLVM Support**: LLVM codegen for argument reordering
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — argument reordering codegen

- [ ] **Implement**: Default parameter filling before pattern matching
  - [ ] **Rust Tests**: `oric/src/codegen/call.rs` — default filling with clauses
  - [ ] **Ori Tests**: `tests/spec/expressions/clause_default_params.ori`
  - [ ] **LLVM Support**: LLVM codegen for default filling with clauses
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — default filling codegen

- [ ] **Implement**: Tests target function name (cover all clauses)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/test.rs` — clause test targeting
  - [ ] **Ori Tests**: `tests/spec/testing/clause_tests.ori`
  - [ ] **LLVM Support**: LLVM codegen for clause test targeting
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_clauses_tests.rs` — clause test targeting codegen

---

## 15.10 Spread Operator

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

## 15.11 Simplified Bindings with `$` for Immutability

**Proposal**: `proposals/approved/simplified-bindings-proposal.md`

Simplify the binding model: `let x` is mutable, `let $x` is immutable. Remove `mut` keyword. Module-level bindings require `$` prefix.

```ori
// Before
let x = 5         // immutable
let mut x = 5     // mutable
$timeout = 30s    // config variable

// After
let x = 5         // mutable
let $x = 5        // immutable
let $timeout = 30s // module-level constant (let and $ required)
```

### Lexer

- [ ] **Implement**: Remove `mut` from reserved keywords
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — keyword list update
  - [ ] **Ori Tests**: `tests/spec/lexical/mut_not_keyword.ori`
  - [ ] **LLVM Support**: LLVM codegen for mut removal
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — mut removal codegen

### Parser

- [ ] **Implement**: Update `let_expr` to accept `$` prefix in binding pattern
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — immutable binding parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/immutable_bindings.ori`
  - [ ] **LLVM Support**: LLVM codegen for immutable binding parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — immutable binding parsing codegen

- [ ] **Implement**: Remove `mut` from `let_expr` grammar
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — mut removal
  - [ ] **Ori Tests**: `tests/compile-fail/let_mut_removed.ori`
  - [ ] **LLVM Support**: LLVM codegen for mut removal
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — mut removal codegen

- [ ] **Implement**: Update `constant_decl` to require `let $name = expr`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — constant declaration parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/constants.ori`
  - [ ] **LLVM Support**: LLVM codegen for constant declaration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — constant declaration codegen

- [ ] **Implement**: Remove old const function syntax `$name (params) -> Type`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — const function removal
  - [ ] **Ori Tests**: `tests/compile-fail/old_const_function_syntax.ori`
  - [ ] **LLVM Support**: LLVM codegen for const function removal
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — const function removal codegen

- [ ] **Implement**: Support `$` prefix in destructuring patterns
  - [ ] **Rust Tests**: `ori_parse/src/grammar/pattern.rs` — destructure immutable parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/destructure_immutable.ori`
  - [ ] **LLVM Support**: LLVM codegen for destructure immutable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — destructure immutable codegen

### Semantic Analysis

- [ ] **Implement**: Track `$` modifier separately from identifier name
  - [ ] **Rust Tests**: `oric/src/resolve/binding.rs` — binding modifier tracking
  - [ ] **Ori Tests**: `tests/spec/expressions/binding_modifiers.ori`
  - [ ] **LLVM Support**: LLVM codegen for binding modifier tracking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — binding modifier tracking codegen

- [ ] **Implement**: Prevent `$x` and `x` coexisting in same scope
  - [ ] **Rust Tests**: `oric/src/resolve/binding.rs` — same-name conflict detection
  - [ ] **Ori Tests**: `tests/compile-fail/dollar_and_non_dollar_conflict.ori`
  - [ ] **LLVM Support**: LLVM codegen for same-name conflict detection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — same-name conflict detection codegen

- [ ] **Implement**: Enforce module-level bindings require `$` prefix
  - [ ] **Rust Tests**: `oric/src/resolve/module.rs` — module binding immutability
  - [ ] **Ori Tests**: `tests/compile-fail/module_level_mutable.ori`
  - [ ] **LLVM Support**: LLVM codegen for module binding immutability
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — module binding immutability codegen

- [ ] **Implement**: Enforce `$`-prefixed bindings cannot be reassigned
  - [ ] **Rust Tests**: `oric/src/typeck/checker/assignment.rs` — immutable assignment error
  - [ ] **Ori Tests**: `tests/compile-fail/assign_to_immutable.ori`
  - [ ] **LLVM Support**: LLVM codegen for immutable assignment error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — immutable assignment error codegen

### Imports

- [ ] **Implement**: Require `$` in import statements for immutable bindings
  - [ ] **Rust Tests**: `oric/src/resolve/import.rs` — import with dollar
  - [ ] **Ori Tests**: `tests/spec/modules/import_immutable.ori`
  - [ ] **LLVM Support**: LLVM codegen for import with dollar
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — import with dollar codegen

- [ ] **Implement**: Error when importing `$x` as `x` or vice versa
  - [ ] **Rust Tests**: `oric/src/resolve/import.rs` — import modifier mismatch
  - [ ] **Ori Tests**: `tests/compile-fail/import_dollar_mismatch.ori`
  - [ ] **LLVM Support**: LLVM codegen for import modifier mismatch
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — import modifier mismatch codegen

### Shadowing

- [ ] **Implement**: Allow shadowing to change mutability
  - [ ] **Rust Tests**: `oric/src/resolve/binding.rs` — shadowing mutability change
  - [ ] **Ori Tests**: `tests/spec/expressions/shadow_mutability.ori`
  - [ ] **LLVM Support**: LLVM codegen for shadowing mutability change
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — shadowing mutability change codegen

### Error Messages

- [ ] **Implement**: Clear error for reassignment to immutable binding
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — immutable reassign error
  - [ ] **Ori Tests**: `tests/compile-fail/immutable_reassign_message.ori`
  - [ ] **LLVM Support**: LLVM codegen for immutable reassign error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — immutable reassign error codegen

- [ ] **Implement**: Clear error for module-level mutable binding
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — module mutable error
  - [ ] **Ori Tests**: `tests/compile-fail/module_mutable_message.ori`
  - [ ] **LLVM Support**: LLVM codegen for module mutable error
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — module mutable error codegen

- [ ] **Implement**: Migration hint for old `let mut` syntax
  - [ ] **Rust Tests**: `ori_diagnostic/src/problem.rs` — let mut migration hint
  - [ ] **Ori Tests**: `tests/compile-fail/let_mut_migration.ori`
  - [ ] **LLVM Support**: LLVM codegen for let mut migration hint
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/binding_tests.rs` — let mut migration hint codegen

---

## 15.12 Remove `dyn` Keyword for Trait Objects

**Proposal**: `proposals/approved/remove-dyn-keyword-proposal.md`

Remove the `dyn` keyword for trait objects. Trait names used directly as types mean "any value implementing this trait."

```ori
// Before
@process (item: dyn Printable) -> void = ...
let items: [dyn Serializable] = ...

// After
@process (item: Printable) -> void = ...
let items: [Serializable] = ...
```

### Implementation

- [ ] **Implement**: Remove `"dyn" type` from grammar type production
  - [ ] **Rust Tests**: `ori_parse/src/grammar/ty.rs` — dyn removal tests
  - [ ] **Ori Tests**: `tests/spec/types/trait_objects.ori`
  - [ ] **LLVM Support**: LLVM codegen for trait objects without dyn
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_object_tests.rs`

- [ ] **Implement**: Parser recognizes trait name in type position as trait object
  - [ ] **Rust Tests**: `ori_parse/src/grammar/ty.rs` — trait-as-type parsing
  - [ ] **Ori Tests**: `tests/spec/types/trait_objects.ori`
  - [ ] **LLVM Support**: LLVM codegen for trait-as-type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_object_tests.rs`

- [ ] **Implement**: Type checker distinguishes `item: Trait` (trait object) vs `<T: Trait>` (generic bound)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/trait_objects.rs`
  - [ ] **Ori Tests**: `tests/spec/types/trait_vs_bound.ori`
  - [ ] **LLVM Support**: LLVM codegen for trait object vs bound distinction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_object_tests.rs`

- [ ] **Implement**: Object safety validation with clear error messages
  - [ ] **Rust Tests**: `oric/src/typeck/checker/object_safety.rs`
  - [ ] **Ori Tests**: `tests/compile-fail/non_object_safe_trait.ori`
  - [ ] **LLVM Support**: LLVM codegen for object safety validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_object_tests.rs`

- [ ] **Implement**: Error if `dyn` keyword is used (helpful migration message)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/ty.rs` — dyn keyword error
  - [ ] **Ori Tests**: `tests/compile-fail/dyn_keyword_removed.ori`

---

## 15.13 Range with Step

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

## 15.14 Phase Completion Checklist

- [ ] All implementation items have checkboxes marked `[x]`
- [ ] All spec docs updated
- [ ] CLAUDE.md updated with syntax changes
- [ ] Migration tools working
- [ ] All tests pass: `./test-all`

**Exit Criteria**: All approved syntax changes implemented and documented
