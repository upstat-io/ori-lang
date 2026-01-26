# Phase 15: Approved Syntax Proposals

**Goal**: Implement approved syntax changes from V3 Phase 15.1-15.5

> **Source**: `docs/sigil_lang/proposals/approved/`
> **Proposals**: `docs/sigil_lang/proposals/approved/`

---

## 15.1 Simplified Attribute Syntax

**Proposal**: `proposals/approved/simplified-attributes-proposal.md`

Change attribute syntax from `#[name(...)]` to `#name(...)`.

```sigil
// Before
#[derive(Eq, Clone)]
#[skip("reason")]

// After
#derive(Eq, Clone)
#skip("reason")
```

### Implementation

- [ ] **Implement**: Update lexer to emit `Hash` token instead of `HashBracket`
  - [ ] **Rust Tests**: `sigil_lexer/src/lib.rs` — attribute token tests
  - [ ] **Sigil Tests**: `tests/spec/attributes/simplified_syntax.si`

- [ ] **Implement**: Update parser to parse `#name(...)` syntax
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/attr.rs` — simplified attribute parsing
  - [ ] **Sigil Tests**: `tests/spec/attributes/simplified_syntax.si`

- [ ] **Implement**: Support migration: accept both syntaxes temporarily
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/attr.rs` — migration compatibility
  - [ ] **Sigil Tests**: `tests/spec/attributes/migration.si`

- [ ] **Implement**: Add deprecation warning for bracket syntax

- [ ] **Implement**: Update `sigil fmt` to auto-migrate

---

## 15.2 function_seq vs function_exp Formalization

**Proposal**: `proposals/approved/function-seq-exp-distinction.md`

Formalize the distinction between sequential patterns and named-expression patterns.

**function_seq** (special syntax): `run`, `try`, `match`, `catch`
**function_exp** (named args): `recurse`, `parallel`, `spawn`, `timeout`, `cache`, `with`, `for`
**function_val** (positional): `int`, `float`, `str`, `byte`

### Implementation

- [ ] **Implement**: Verify AST has separate `FunctionSeq` and `FunctionExp` types
  - [ ] **Rust Tests**: `sigil_ir/src/ast/expr.rs` — AST variant tests
  - [ ] **Sigil Tests**: `tests/spec/patterns/function_seq_exp.si`

- [ ] **Implement**: Parser allows positional for type conversions only
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/call.rs` — positional arg handling
  - [ ] **Sigil Tests**: `tests/spec/expressions/type_conversions.si`

- [ ] **Implement**: Parser enforces named args for all other builtins
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/call.rs` — named arg enforcement
  - [ ] **Sigil Tests**: `tests/spec/expressions/builtin_named_args.si`

- [ ] **Implement**: Add clear error message for positional args in builtins
  - [ ] **Rust Tests**: `sigil_diagnostic/src/problem.rs` — positional arg error
  - [ ] **Sigil Tests**: `tests/compile-fail/builtin_positional_args.si`

---

## 15.3 Remove Dot Prefix from Named Arguments

**Proposal**: `proposals/approved/remove-dot-prefix-proposal.md`

Change named argument syntax from `.name: value` to `name: value`.

```sigil
// Before
fetch_user(.id: 1)
map(.over: items, .transform: x -> x * 2)

// After
fetch_user(id: 1)
map(over: items, transform: x -> x * 2)
```

### Implementation

- [ ] **Implement**: Update parser to expect `IDENTIFIER ':'` instead of `'.' IDENTIFIER ':'`
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/call.rs` — named arg syntax
  - [ ] **Sigil Tests**: `tests/spec/expressions/named_args.si`

- [ ] **Implement**: Update error messages to show new syntax
  - [ ] **Rust Tests**: `sigil_diagnostic/src/problem.rs` — syntax error messages
  - [ ] **Sigil Tests**: `tests/compile-fail/named_arg_syntax.si`

- [ ] **Implement**: Add migration tool `sigil migrate remove-dot-prefix`

- [ ] **Implement**: Update formatter with width-based stacking rule

---

## 15.4 Inline Comments Prohibition

Comments must appear on their own line. Inline comments are not allowed.

```sigil
// This is valid
let x = 42

let y = 42  // SYNTAX ERROR
```

### Implementation

- [ ] **Implement**: Update lexer to reject inline comments
  - [ ] **Rust Tests**: `sigil_lexer/src/lib.rs` — inline comment rejection
  - [ ] **Sigil Tests**: `tests/compile-fail/inline_comments.si`

- [ ] **Implement**: Add clear error message for inline comments

---

## 15.5 Pre/Post Checks for `run` Pattern

**Proposal**: `proposals/approved/checks-proposal.md`

Extend `run` pattern with `pre_check:` and `post_check:` properties.

```sigil
@divide (a: int, b: int) -> int = run(
    pre_check: b != 0,
    a div b,
    post_check: r -> r * b <= a,
)
```

### Implementation

- [ ] **Implement**: Parser: Add `pre_check:` and `post_check:` to run pattern
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/pattern.rs` — check property parsing
  - [ ] **Sigil Tests**: `tests/spec/patterns/checks.si`

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

```sigil
let name = "Alice"
let age = 30
print(`Hello, {name}! You are {age} years old.`)
```

### Lexer

- [ ] **Implement**: Add template string literal tokenization
  - [ ] **Rust Tests**: `sigil_lexer/src/lib.rs` — template string tokenization
  - [ ] **Sigil Tests**: `tests/spec/lexical/template_strings.si`

- [ ] **Implement**: Handle `{expr}` interpolation boundaries

- [ ] **Implement**: Handle `\{` and `\}` escape for literal braces

- [ ] **Implement**: Handle `` \` `` escape for literal backtick

### Parser

- [ ] **Implement**: Parse template strings as sequence of parts
  - [ ] **Rust Tests**: `sigil_parse/src/grammar/expr.rs` — template string parsing
  - [ ] **Sigil Tests**: `tests/spec/expressions/interpolation.si`

- [ ] **Implement**: Parse interpolated expressions

- [ ] **Implement**: Parse optional format specifiers `{expr:spec}`

### Type System

- [ ] **Implement**: Interpolated expressions must implement `Printable`
  - [ ] **Rust Tests**: `sigilc/src/typeck/checker/interpolation.rs` — printable constraint
  - [ ] **Sigil Tests**: `tests/spec/types/printable.si`

### Codegen

- [ ] **Implement**: Desugar template strings to concatenation

- [ ] **Implement**: Generate `to_str()` calls for interpolations

---

## 15.7 Phase Completion Checklist

- [ ] All implementation items have checkboxes marked `[x]`
- [ ] All spec docs updated
- [ ] CLAUDE.md updated with syntax changes
- [ ] Migration tools working
- [ ] All tests pass: `cargo test && sigil test tests/spec/`

**Exit Criteria**: All approved syntax changes implemented and documented
