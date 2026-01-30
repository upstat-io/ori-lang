# Phase 15A: Attributes & Comments

**Goal**: Implement approved attribute syntax changes and comment restrictions

> **Source**: `docs/ori_lang/proposals/approved/`

---

## 15A.1 Simplified Attribute Syntax

**Proposal**: `proposals/approved/simplified-attributes-proposal.md`

Change attribute syntax from `#[name(...)]` to `#name(...)`. Attributes are now generalizable to all declarations.

```ori
// Before
#[derive(Eq, Clone)]
#[skip("reason")]

// After
#derive(Eq, Clone)
#skip("reason")
```

### Key Design Decisions

- **Generalized attributes**: Any attribute can appear before any declaration
- **Compiler validation**: The compiler validates which attributes are valid for which declarations
- **Positioning**: Attributes must appear immediately before the declaration they modify

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

- [ ] **Implement**: Generalize attributes to all declarations (functions, types, traits, impls, tests, constants)
  - [ ] **Rust Tests**: `ori_parse/src/grammar/decl.rs` — generalized attribute parsing
  - [ ] **Ori Tests**: `tests/spec/attributes/any_declaration.ori`
  - [ ] **LLVM Support**: LLVM codegen for generalized attributes
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/attribute_tests.rs` — generalized attribute codegen

- [ ] **Implement**: Attribute validation (which attributes valid for which declarations)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/attr.rs` — attribute validation
  - [ ] **Ori Tests**: `tests/compile-fail/invalid_attribute_target.ori`
  - [ ] **LLVM Support**: LLVM codegen for attribute validation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/attribute_tests.rs` — attribute validation codegen

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

## 15A.2 function_seq vs function_exp Formalization

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

## 15A.3 Inline Comments Prohibition

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

## 15A.4 Simplified Doc Comment Syntax

**Proposal**: `proposals/approved/simplified-doc-comments-proposal.md`

Simplify doc comment syntax by removing verbose markers:

```ori
// Before
// #Computes the sum.
// @param a The first operand.
// @param b The second operand.

// After
// Computes the sum.
// * a: The first operand.
// * b: The second operand.
```

### Key Design Decisions

- **Remove `#` marker for descriptions** — Unmarked comments before declarations are descriptions
- **Replace `@param`/`@field` with `*`** — Markdown-like list syntax, context determines meaning
- **Canonical spacing** — `// * name: description` with space after `*`, colon always required
- **Non-doc comment separation** — Blank line separates non-doc comments from declarations

### Implementation

- [ ] **Implement**: Update `CommentKind` enum
  - [ ] Replace `DocParam`, `DocField` with unified `DocMember`
  - [ ] Remove `DocDescription` detection from lexer (moved to formatter)
  - [ ] **Rust Tests**: `ori_ir/src/comment.rs` — enum variant tests
  - [ ] **Ori Tests**: `tests/spec/comments/doc_markers.ori`

- [ ] **Implement**: Update lexer comment classification
  - [ ] Recognize `*` as member doc marker
  - [ ] Remove `#`, `@param`, `@field` recognition
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — comment classification tests
  - [ ] **Ori Tests**: `tests/spec/comments/classification.ori`

- [ ] **Implement**: Update formatter doc comment reordering
  - [ ] Update `extract_member_name` to parse `* name:` syntax
  - [ ] Move description detection to formatter (check preceding declaration)
  - [ ] **Rust Tests**: `ori_fmt/src/comments.rs` — reordering tests
  - [ ] **Ori Tests**: `tests/fmt/comments/reordering.ori`

- [ ] **Implement**: Support migration from old syntax
  - [ ] Lexer recognizes both old and new formats during transition
  - [ ] `ori fmt` converts old to new automatically
  - [ ] Add deprecation warning for old format
  - [ ] **Ori Tests**: `tests/spec/comments/migration.ori`

- [ ] **Implement**: LLVM backend support
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comment_tests.rs`

---

## 15A.5 Phase Completion Checklist

- [ ] All implementation items have checkboxes marked `[x]`
- [ ] All spec docs updated
- [ ] CLAUDE.md updated with syntax changes
- [ ] Migration tools working
- [ ] All tests pass: `./test-all`

**Exit Criteria**: Attribute syntax, comment rules, and doc comment syntax implemented
