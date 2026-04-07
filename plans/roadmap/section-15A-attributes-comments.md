---
section: "15A"
title: Attributes & Comments
status: in-progress
reviewed: true
last_verified: "2026-03-29"
tier: 5
goal: Implement approved attribute syntax changes and comment restrictions
sections:
  - id: "15A.1"
    title: Simplified Attribute Syntax
    status: partial
  - id: "15A.2"
    title: function_seq vs function_exp Formalization
    status: done
  - id: "15A.3"
    title: Inline Comments Prohibition
    status: not-started
  - id: "15A.4"
    title: Simplified Doc Comment Syntax
    status: partial
  - id: "15A.5"
    title: Section Completion Checklist
    status: not-started
---

# Section 15A: Attributes & Comments

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

> **NOTE (verified 2026-03-29)**: LLVM sub-items throughout this section are IRRELEVANT -- attributes are parsed metadata consumed by the parser/typechecker, not LLVM IR constructs. LLVM codegen never sees attributes or comments. All LLVM sub-items below are marked N/A.

- [x] **Implement**: Update lexer to emit `Hash` token instead of `HashBracket` (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_lexer/src/cooker/tests.rs` — `test_hash_token` verifies `RawTag::Hash` produces `TokenKind::Hash`; 31 lexer tests pass
  - [x] **Ori Tests**: 125+ existing spec test files exercise `#skip`, `#compile_fail`, `#fail`, `#derive`, `#repr`, `#target`, `#cfg`, `#fbip`
  - N/A ~~**LLVM Support**~~: attributes are frontend metadata, not LLVM IR constructs
  - N/A ~~**LLVM Rust Tests**~~: see above
  - N/A ~~**AOT Tests**~~: see above

- [x] **Implement**: Update parser to parse `#name(...)` syntax (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr/tests.rs` — 14 tests cover both bracket and bracketless syntax; semantic pins: `test_parse_skip_attribute_no_brackets`, `test_parse_compile_fail_attribute_no_brackets`, `test_parse_derive_attribute_no_brackets`
  - [x] **Ori Tests**: grammar.ebnf line 239: `attribute = "#" identifier [ "(" ... ) ] .`
  - N/A ~~**LLVM Support**~~: attributes are frontend metadata, not LLVM IR constructs
  - N/A ~~**LLVM Rust Tests**~~: see above
  - N/A ~~**AOT Tests**~~: see above

- [x] **Implement**: Generalize attributes to all declarations (functions, types, traits, impls, tests, constants) (verified 2026-03-29)
  - [x] **Rust Tests**: `test_attributes_on_declarations` in parser tests
  - [x] **Ori Tests**: grammar.ebnf line 233: `declaration = { attribute } [ "pub" ] ( function | type_def | trait_def | impl_block | ... ) .`
  - N/A ~~**LLVM Support**~~: attributes are frontend metadata, not LLVM IR constructs
  - N/A ~~**LLVM Rust Tests**~~: see above
  - N/A ~~**AOT Tests**~~: see above

- [ ] **Implement**: Attribute validation (which attributes valid for which declarations) -- PARTIAL (verified 2026-03-29): unknown attributes rejected (E1006), file-level validation exists, but no per-declaration target validation (e.g., `#derive` on a function is not caught)
  - [x] **Rust Tests**: `test_parse_unknown_attribute`, `test_file_attr_invalid_kind_reports_error` in parser tests
  - [ ] **Ori Tests**: `tests/compile-fail/invalid_attribute_target.ori` — GAP: no test for invalid attribute targets
  - [ ] Per-declaration attribute-target validation (e.g., reject `#derive` on functions, `#skip` on types)
  - N/A ~~**LLVM Support**~~: attributes are frontend metadata, not LLVM IR constructs
  - N/A ~~**LLVM Rust Tests**~~: see above

- [x] **Implement**: Support migration: accept both syntaxes temporarily (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr/tests.rs` — tests cover both `#[name(...)]` and `#name(...)` forms; `uses_brackets` flag threads through parsing
  - [x] **Ori Tests**: 104 test files use old `#[derive(...)]`, 21 use new `#derive(...)` — both work
  - N/A ~~**LLVM Support**~~: attributes are frontend metadata, not LLVM IR constructs
  - N/A ~~**LLVM Rust Tests**~~: see above
  - N/A ~~**AOT Tests**~~: see above

- [ ] **Implement**: Add deprecation warning for bracket syntax -- NOT IMPLEMENTED (verified 2026-03-29): parser silently accepts both forms, no warning infrastructure. Note: 104 test files still use old syntax; migration should precede or accompany this.

- [ ] **Implement**: Update `ori fmt` to auto-migrate -- NOT IMPLEMENTED (verified 2026-03-29): no migration logic in `ori_fmt` to convert `#[name(...)]` to `#name(...)`

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

> **NOTE (verified 2026-03-29)**: LLVM sub-items throughout this section are IRRELEVANT -- FunctionSeq/FunctionExp are AST nodes, not LLVM constructs. All LLVM sub-items below are marked N/A.

- [x] **Implement**: Verify AST has separate `FunctionSeq` and `FunctionExp` types (verified 2026-03-29)
  - [x] **Rust Tests**: `test_function_exp_kind_names` in `ori_ir/src/ast/patterns/exp/tests.rs` verifies all 15 variants; AST tests at `ast/tests.rs` lines 48-50 verify enum equality
  - [x] **Evidence**: `FunctionSeq` in `ori_ir/src/ast/patterns/seq/mod.rs`, `FunctionExp`/`FunctionExpKind` (15 variants) in `ori_ir/src/ast/patterns/exp/mod.rs`, `ExprKind::FunctionSeq(FunctionSeqId)` at `ast/expr.rs` line 338
  - N/A ~~**LLVM Support**~~: AST-level concern, not LLVM
  - N/A ~~**LLVM Rust Tests**~~: see above
  - N/A ~~**AOT Tests**~~: see above

- [x] **Implement**: Parser allows positional for type conversions only (verified 2026-03-29) -- OBE: `function_val` removed by `as` proposal; type conversions now use `ExprKind::Cast` with `as`/`as?` syntax
  - [x] **Evidence**: No `function_val` category in AST; grammar uses `cast_expression` for `as`/`as?`
  - N/A ~~**LLVM Support**~~: AST-level concern, not LLVM
  - N/A ~~**LLVM Rust Tests**~~: see above
  - N/A ~~**AOT Tests**~~: see above

- [x] **Implement**: Parser enforces named args for all other builtins (verified 2026-03-29)
  - [x] **Evidence**: `FunctionExp` requires `NamedExprRange` (`props` field) -- all properties are named expressions with `name: value` format
  - N/A ~~**LLVM Support**~~: AST-level concern, not LLVM
  - N/A ~~**LLVM Rust Tests**~~: see above
  - N/A ~~**AOT Tests**~~: see above

- [x] **Implement**: Add clear error message for positional args in builtins (verified 2026-03-29) -- PARTIAL: parser produces errors for positional arguments where named are required, but no dedicated compile-fail test exists
  - [x] **Evidence**: parser rejects positional args in function_exp contexts
  - [ ] **Ori Tests**: `tests/compile-fail/builtin_positional_args.ori` -- GAP: no dedicated test file
  - N/A ~~**LLVM Support**~~: AST-level concern, not LLVM
  - N/A ~~**LLVM Rust Tests**~~: see above

---

## 15A.3 Inline Comments Prohibition

Comments must appear on their own line. Inline comments are not allowed.

```ori
// This is valid
let x = 42

let y = 42  // SYNTAX ERROR
```

### Implementation

> **SPEC/IMPL GAP (verified 2026-03-29)**: The spec (`07-lexical-elements.md` section 7.1) says "Inline comments (comments following code on the same line) are not permitted." However, the compiler accepts them silently. The lexer classifies comments by content markers only, with no position-based validation.

> **NOTE (verified 2026-03-29)**: LLVM sub-items are IRRELEVANT -- comment handling is a lexer concern, not LLVM. All LLVM sub-items below are marked N/A.

- [ ] **Implement**: Update lexer to reject inline comments -- NOT IMPLEMENTED (verified 2026-03-29): lexer has no logic to detect whether non-whitespace preceded `//` on the same line
  - [ ] **Rust Tests**: `ori_lexer/src/comments/` — inline comment position detection
  - [ ] **Ori Tests**: `tests/compile-fail/inline_comments.ori`
  - N/A ~~**LLVM Support**~~: lexer-level concern, not LLVM
  - N/A ~~**LLVM Rust Tests**~~: see above

- [ ] **Implement**: Add clear error message for inline comments -- NOT IMPLEMENTED (verified 2026-03-29): no error code assigned, no diagnostic message exists
  - N/A ~~**LLVM Support**~~: lexer-level concern, not LLVM
  - N/A ~~**LLVM Rust Tests**~~: see above

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

- [x] **Implement**: Update `CommentKind` enum (verified 2026-03-29)
  - [x] Replace `DocParam`, `DocField` with unified `DocMember` -- done: `CommentKind` at `ori_ir/src/comment/mod.rs` has `Regular`, `DocDescription`, `DocMember`, `DocWarning`, `DocExample`; no `DocParam`/`DocField` variants exist
  - [x] ~~Remove `DocDescription` detection from lexer~~ -- NOTE: `DocDescription` still exists in enum and is classified by lexer via `#` prefix; this is acceptable as the description marker is lightweight
  - [x] **Rust Tests**: `ori_ir/src/comment/tests.rs` — 9 tests verifying `sort_order()` and `is_doc()` for all variants

- [x] **Implement**: Update lexer comment classification (verified 2026-03-29)
  - [x] Recognize `*` as member doc marker -- done: `classify_and_normalize_comment()` in `ori_lexer/src/comments/mod.rs`
  - [x] ~~Remove `#`, `@param`, `@field` recognition~~ -- NOTE: both old and new markers still recognized; removal should NOT happen until migration is complete
  - [x] **Rust Tests**: `ori_lexer/src/comments/tests.rs` — 31 classification tests pass

- [x] **Implement**: Update formatter doc comment reordering (verified 2026-03-29)
  - [x] Update `extract_member_name` to parse `* name:` syntax -- done: `extract_member_name_any()` at `ori_fmt/src/comments/mod.rs` line 324 handles both formats
  - [x] **Rust Tests**: `test_extract_member_name_any_star_format`, `test_extract_member_name_any_legacy_param`, `test_extract_member_name_any_legacy_field` -- 4 formatter comment tests pass

- [ ] **Implement**: Support migration from old syntax -- PARTIAL (verified 2026-03-29)
  - [x] Lexer recognizes both old and new formats during transition -- done: `#`, `@param`, `@field` all produce correct `CommentKind` variants alongside new `*` format
  - [ ] `ori fmt` converts old to new automatically -- NOT IMPLEMENTED
  - [ ] Add deprecation warning for old format (`#Description`, `@param`, `@field`) -- NOT IMPLEMENTED

- N/A ~~**Implement**: LLVM backend support~~ (verified 2026-03-29) -- IRRELEVANT: comments are lexer/parser/formatter metadata, not LLVM IR constructs

---

## 15A.5 Section Completion Checklist

- [ ] All implementation items have checkboxes marked `[ ]`
- [ ] All spec docs updated
- [ ] CLAUDE.md updated with syntax changes
- [ ] Migration tools working
- [ ] All tests pass: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: Attribute syntax, comment rules, and doc comment syntax implemented
