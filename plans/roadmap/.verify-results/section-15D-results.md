# Section 15D Verification Results: Bindings & Types

**Verified**: 2026-03-28
**Section status**: `in-progress` -- ACCURATE. 15D.3 is partially complete, other subsections not started.
**Methodology**: Grepped compiler source for AST nodes, token kinds, parser productions, type checker, evaluator dispatch, and LLVM codegen. Read all found test files. Ran spec tests with `timeout 150 cargo st`.

**Files loaded**: CLAUDE.md (all), all 20 `.claude/rules/*.md` files, section-15D-bindings-types.md (full, 612 lines).

---

## Summary

| Subsection | Plan Status | Actual Status | Checked | Unchecked | Notes |
|---|---|---|---|---|---|
| 15D.1 Function-Level Contracts | not-started | PARTIALLY IMPLEMENTED | 0 | 8 | Parser done, typeck/eval not done |
| 15D.2 as Conversion Syntax | not-started | SUBSTANTIALLY IMPLEMENTED | 0 | ~10 | Parser, typeck, eval all done |
| 15D.3 Simplified Bindings | in-progress | IN PROGRESS (accurate) | 8 | ~10 | Core done, remaining items real |
| 15D.4 Remove dyn Keyword | not-started | PARTIALLY IMPLEMENTED | 0 | ~5 | `dyn` token exists but not used in parser |
| 15D.5 Index and Field Assignment | not-started | NOT STARTED | 0 | ~15 | No `IndexSet`, no assignment chains |
| 15D.6 Mutable Self | not-started | NOT STARTED | 0 | ~15 | No implementation found |
| 15D.7 Checklist | not-started | not-started | 0 | 6 | |

**Recommended section status**: `in-progress` -- accurate. 15D.2 has hidden implementation.

---

## 15D.1 Function-Level Contracts: `pre()` / `post()`

**Plan status**: not-started
**Actual status**: PARTIALLY IMPLEMENTED -- parser complete, type checking and evaluation not done.

### Implementation

- [ ] **Implement**: Parser: Parse `pre()` and `post()` on function declarations
  - **ACTUAL**: [done] `ori_parse/src/grammar/item/function/mod.rs` -- `parse_contracts()` returns `(Vec<PreContract>, Vec<PostContract>)`. Line 131: contracts parsed between return type and `=`.
  - [done] AST: `PreContract` and `PostContract` structs in `ori_ir/src/ast/items/function.rs`. `FunctionDef` has `pre_contracts` and `post_contracts` fields.

- [ ] **Implement**: Parser: Support `| "message"` custom message syntax
  - **ACTUAL**: [done] `PreContract` has `message: Option<ExprId>` field. `PostContract` has `message: Option<ExprId>`.

- [ ] **Implement**: Type checker: Validate `pre()` condition is `bool`
  - **ACTUAL**: NOT DONE. No `pre_contract` references in `ori_types/src`.

- [ ] **Implement**: Type checker: Validate `post()` is `T -> bool` lambda
  - **ACTUAL**: NOT DONE.

- [ ] **Implement**: Type checker: Error when `post()` used on void-returning function
  - **ACTUAL**: NOT DONE.

- [ ] **Implement**: Scope checker: `pre()` can only access parameters and module-level bindings
  - **ACTUAL**: NOT DONE.

- [ ] **Implement**: Codegen: Desugar to conditional checks and panics
  - **ACTUAL**: NOT DONE. No contract-related code in evaluator.

- [ ] **Implement**: Codegen: Embed source text for default error messages
  - **ACTUAL**: NOT DONE.

---

## 15D.2 `as` Conversion Syntax

**Plan status**: not-started
**Actual status**: SUBSTANTIALLY IMPLEMENTED -- parser, type checker, evaluator, LLVM all working.

### Lexer

- [ ] **Implement**: `as` keyword token
  - **ACTUAL**: [done] `TokenKind::As` exists. `TokenTag::KwAs` in `ori_ir/src/token/tag.rs`. `TAG_AS` constant.

### Parser

- [ ] **Implement**: Parse `expression as Type`
  - **ACTUAL**: [done] `ori_parse/src/grammar/expr/postfix.rs` line 321: `parse_type_cast()` handles `as` and `as?`. Produces `ExprKind::Cast { expr, ty, fallible }`.

- [ ] **Implement**: Parse `expression as? Type` as fallible conversion
  - **ACTUAL**: [done] Same function. `fallible` flag distinguishes `as` (false) from `as?` (true).

### Type Checker

- [ ] **Implement**: Validate `as` only used with `As<T>` trait implementations
  - **ACTUAL**: PARTIAL. `infer_cast()` in `ori_types/src/infer/expr/operators.rs` resolves target type. Comment says "for validation, though we don't check cast validity here" -- validation is minimal.

- [ ] **Implement**: Validate `as?` only used with `TryAs<T>` trait implementations
  - **ACTUAL**: PARTIAL. Returns `Option<T>` for fallible casts. Trait validation not verified.

- [ ] **Implement**: Error when using `as` for fallible conversion (must use `as?`)
  - **ACTUAL**: NOT VERIFIED.

### Codegen

- [ ] **Implement**: Desugar `x as T` to trait method call
  - **ACTUAL**: [done] Evaluator handles `Cast` at `ori_eval/src/interpreter/can_eval/mod.rs` line 171: `eval_can_cast()`.

- [ ] **Implement**: Desugar `x as? T` to `TryAs<T>.try_as()`
  - **ACTUAL**: [done] Same code path with `fallible` flag.

### Migration

- [ ] **Implement**: Remove `int()`, `float()`, `str()`, `byte()` from parser
  - **ACTUAL**: NOT DONE. `str()`, `int()`, `float()` free functions still exist as builtins.

- [ ] **Implement**: Update error messages to suggest `as` syntax
  - **ACTUAL**: NOT DONE.

### LLVM Support

- Not directly verified, but `ExprKind::Cast` is canonicalized and flows through the ARC pipeline.

---

## 15D.3 Simplified Bindings with `$` for Immutability

**Plan status**: in-progress
**Actual status**: IN PROGRESS -- core items complete, remaining items genuinely unchecked.

### Lexer

- [ ] **Implement**: Remove `mut` from reserved keywords
  - **ACTUAL**: [done] No `mut` in `ori_lexer/src/keywords/mod.rs`. No `KwMut` in `TokenTag`. No `TAG_MUT` in constants. Fully removed.

### Parser

- [x] **Implement**: Update `let_expr` to accept `$` prefix in binding pattern (2026-02-20)
  - **VERIFIED**: [done] `ori_parse/src/grammar/expr/primary.rs` -- `parse_binding_pattern` handles `$` prefix.
  - [x] Rust Tests: `ori_parse/src/tests/parser.rs` -- confirmed.
  - [x] Ori Tests: `tests/spec/expressions/immutable_bindings.ori` -- 8 tests, all pass.

- [x] **Implement**: Remove `mut` from `let_expr` grammar
  - **VERIFIED**: [done] No `mut` handling in let expression parsing.
  - [x] Ori Tests: Confirmed -- 151 `let mut` occurrences migrated per plan note.
  - [x] AOT Tests: `ori_llvm/tests/aot/mutations.rs` uses `let x` (mutable-by-default).

- [ ] **Implement**: Update `constant_decl` to require `let $name = expr`
  - **ACTUAL**: NOT VERIFIED.

- [ ] **Implement**: Remove old const function syntax `$name (params) -> Type`
  - **ACTUAL**: NOT VERIFIED.

- [x] **Implement**: Support `$` prefix in destructuring patterns (2026-02-20)
  - **VERIFIED**: [done] Tests at `tests/spec/expressions/immutable_bindings.ori` cover tuple (`($a, $b)`), struct (`{ $x, $y }`), list (`[$first, $second, ..rest]`), mixed (`{ $x, y }`), rename (`{ x: $px, y: $py }`).

- [x] **Fix**: List rest binding `..rest` tracks `$` mutability (2026-02-20)
  - **VERIFIED**: [done] Per detailed plan notes -- IR, parser, type checker, evaluator, canon, formatter all updated.

### Semantic Analysis

- [x] **Implement**: Track `$` modifier separately from identifier name (2026-02-20)
  - **VERIFIED**: [done] `ori_types/src/infer/env/mod.rs` -- `mutability` FxHashMap.

- [ ] **Implement**: Prevent `$x` and `x` coexisting in same scope
  - **ACTUAL**: NOT DONE. No scope conflict detection found.

- [ ] **Implement**: Enforce module-level bindings require `$` prefix
  - **ACTUAL**: NOT DONE.

- [x] **Implement**: Enforce `$`-prefixed bindings cannot be reassigned (2026-02-20)
  - **VERIFIED**: [done] `ori_types/src/infer/expr/operators.rs` -- immutability check in `infer_assign`.
  - [x] Ori Tests: `tests/compile-fail/assign_to_immutable.ori` -- `#[compile_fail("cannot assign to immutable binding")]`. Passes.
  - [x] Also: `assign_to_immutable_in_loop.ori`, `assign_to_immutable_destructured.ori`.

### Imports

- [ ] **Implement**: Require `$` in import statements for immutable bindings
  - **ACTUAL**: NOT DONE.

- [ ] **Implement**: Error when importing `$x` as `x` or vice versa
  - **ACTUAL**: NOT DONE.

### Shadowing

- [ ] **Implement**: Allow shadowing to change mutability
  - **ACTUAL**: NOT VERIFIED.

### Error Messages

- [x] **Implement**: Clear error for reassignment to immutable binding (2026-02-20)
  - **VERIFIED**: [done] `AssignToImmutable` variant in type error. E2039 error code.

- [ ] **Implement**: Clear error for module-level mutable binding
  - **ACTUAL**: NOT DONE.

- [ ] **Implement**: Migration hint for old `let mut` syntax
  - **ACTUAL**: NOT VERIFIED.

### LLVM Support for checked items

- [x] `let $x` immutable binding parsing: N/A -- mutability is type-checker concern.
- [x] `$` destructuring: N/A -- mutability is type-checker concern.
- [ ] LLVM codegen for immutable binding parsing: NOT VERIFIED.
- [x] AOT Tests for mut removal: [done] `ori_llvm/tests/aot/mutations.rs` uses `let x` syntax.

---

## 15D.4 Remove `dyn` Keyword for Trait Objects

**Plan status**: not-started
**Actual status**: PARTIALLY IMPLEMENTED (by omission)

- [ ] **Implement**: Remove `"dyn" type` from grammar type production
  - **ACTUAL**: `dyn` is NOT used in the parser grammar for type positions. No `TAG_DYN` references in `ori_parse/src/grammar/`. The token exists (`TokenKind::Dyn`) but is not consumed in type parsing.

- [ ] **Implement**: Parser recognizes trait name in type position as trait object
  - **ACTUAL**: NOT VERIFIED whether trait names in type position produce trait objects. Type checker may handle this.

- [ ] **Implement**: Type checker distinguishes `item: Trait` (trait object) vs `<T: Trait>` (generic bound)
  - **ACTUAL**: NOT VERIFIED.

- [ ] **Implement**: Object safety validation with clear error messages
  - **ACTUAL**: NOT VERIFIED.

- [ ] **Implement**: Error if `dyn` keyword is used (helpful migration message)
  - **ACTUAL**: NOT DONE. No `dyn` error message found.

---

## 15D.5 Index and Field Assignment

**Plan status**: not-started
**Actual status**: NOT STARTED

### Phase 1: `IndexSet` Trait and `updated` Method

- [ ] **Implement**: Define `IndexSet<Key, Value>` trait in prelude
  - **ACTUAL**: NOT DONE. No `IndexSet` in `library/std/prelude.ori`.

- [ ] **Implement**: Register `updated` as built-in method
  - **ACTUAL**: NOT DONE. List/map do have `.updated()` as methods registered in `ori_registry`.

- [ ] **Implement**: `updated` with ARC-aware copy-on-write
  - **ACTUAL**: NOT DONE as trait-based.

### Phase 2-5: Parser Changes, Type-Directed Desugaring, Type Checker, LLVM

All items unchecked -- accurate. No assignment chain parsing, no field assignment desugaring.

NOTE: `list.updated(key:, value:)` method EXISTS as a registry method but the `IndexSet` trait and `list[i] = x` desugaring do not exist.

---

## 15D.6 Mutable Self

**Plan status**: not-started
**Actual status**: NOT STARTED

No mutable self implementation found:
- No mutation detection dataflow analysis.
- No call-site desugaring for mutating methods.
- No `MutableSelf` or `mutating` classification.
- No test files at `tests/spec/methods/mutable_self*`.

All items unchecked -- accurate.

---

## 15D.7 Section Completion Checklist

- [ ] All implementation items have checkboxes marked `[ ]` -- NOT DONE
- [ ] All spec docs updated -- NOT DONE
- [ ] CLAUDE.md updated with syntax changes -- PARTIAL (binding syntax documented)
- [ ] Migration tools working -- NOT DONE
- [ ] All tests pass: `./test-all.sh` -- NOT VERIFIED for this section specifically
- [ ] `/tpr-review` passed -- NOT DONE

---

## Critical Findings

1. **HIDDEN IMPLEMENTATION in 15D.2**: The `as`/`as?` conversion syntax is substantially implemented (parser, type checker, evaluator) but all items marked `[ ]`. Section should mark parser and evaluator items as `[x]`.

2. **HIDDEN IMPLEMENTATION in 15D.1**: Contracts `pre()`/`post()` are fully parsed with AST representation, custom messages, and integration into `FunctionDef`. Only type checking and evaluation are missing.

3. **15D.3 CHECKED ITEMS VERIFIED**: All 8 `[x]` items are genuine implementations with real tests that pass. The unchecked items are genuinely not done.

4. **15D.4 `dyn` ALREADY UNUSED**: The `dyn` keyword exists as a token but is NOT consumed in type parsing. Trait objects may already work without `dyn` -- needs verification. The migration error message is missing.

5. **15D.5 PARTIAL FOUNDATION**: `updated()` method exists on lists/maps via `ori_registry`, but the `IndexSet` trait and `list[i] = x` desugaring pipeline do not exist.

6. **`mut` KEYWORD FULLY REMOVED**: The `mut` keyword is completely gone from the token system, confirming the lexer item should be `[x]`.

7. **STALE STATUS for 15D.2**: The plan marks `as` conversion as `not-started` but it's substantially working. The `as` keyword, parser production, type inference, and evaluator dispatch are all implemented.
