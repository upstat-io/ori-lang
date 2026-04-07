---
section: 5
title: Type Declarations
status: in-progress
last_verified: "2026-03-29"
reviewed: true
tier: 1
goal: User-defined types
spec:
  - spec/08-types.md
  - spec/09-properties-of-types.md
  - spec/10-declarations.md
sections:
  - id: "5.1"
    title: Struct Types
    status: in-progress
  - id: "5.2"
    title: Sum Types (Enums)
    status: in-progress
  - id: "5.3"
    title: Newtypes
    status: in-progress
  - id: "5.4"
    title: Generic Types
    status: in-progress
  - id: "5.5"
    title: Compound Types
    status: in-progress
  - id: "5.6"
    title: Built-in Generic Types
    status: in-progress
  - id: "5.7"
    title: "with Clause on Type Declarations (Capability Unification)"
    status: not-started
  - id: "5.8"
    title: Visibility
    status: in-progress
  - id: "5.9"
    title: Associated Functions
    status: in-progress
  - id: "5.10"
    title: Section Completion Checklist
    status: in-progress
---

# Section 5: Type Declarations

**Goal**: User-defined types

> **SPEC**: `spec/08-types.md`, `spec/09-properties-of-types.md`, `spec/10-declarations.md`

**Status**: In-progress — Structs (5.1), sum types (5.2), newtypes (5.3), generics (5.4), built-in generics (5.6), derive (5.7), visibility (5.8), associated functions (5.9) all working in evaluator and AOT. Compound type inference (5.5) entirely pending. AOT coverage exists for structs, sum types, tuples, and all 7 derivable traits. Verified 2026-03-29.

> **SYNC POINT**: Adding a new built-in type (e.g., `Duration`, `Size`, `CPtr`, `Nursery`) requires updates across `ori_ir` (type variant), `ori_types` (registration + method signatures), `ori_eval` (runtime representation + method dispatch), `ori_llvm` (type layout + codegen), and `library/std/` (prelude or module definitions). This section primarily covers user-defined types, but built-in types added by other sections (11, 16, 17, 18) follow the same pattern.

---

## 5.1 Struct Types

- [x] **Implement**: Parse `type Name = { field: Type, ... }` — spec/08-types.md § Struct Types, spec/10-declarations.md § Type Declarations [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/tests/parser.rs`, `ori_parse/src/tests/compositional.rs` — struct parsing tests
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` (35+ tests: basic, single field, empty, nested, many fields, mixed types, generic, shorthand, spread, with Option/List/Tuple/Function fields)

- [x] **Implement**: Register struct in type environment — spec/10-declarations.md § Type Declarations [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/check/registration/` — type registry tests (implicit in all struct tests)
  - [x] **Ori Tests**: All struct tests verify type registration

- [x] **Implement**: Parse struct literals `Name { field: value }` — spec/08-types.md § Struct Types [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/tests/parser.rs` — `test_struct_literal_in_expression`, `test_struct_literal_in_if_then_body`, `test_if_condition_disallows_struct_literal` (negative pin)
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — all tests create struct literals

- [x] **Implement**: Type check struct literals — spec/08-types.md § Struct Types [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/` — struct literal type checking
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/structs.rs` — 30 tests passing (struct construction with int/bool/str/mixed fields, update syntax, nested structs, function params/returns, closures, derived Eq) (verified 2026-03-29)

- [x] **Implement**: Shorthand `Point { x, y }` — spec/08-types.md § Struct Types [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — shorthand parsing
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — `test_shorthand_init`, `test_mixed_shorthand`
  - [ ] **AOT Tests**: No AOT coverage yet — `ori_llvm/tests/aot/structs.rs` does not test shorthand syntax specifically
  - [ ] WEAK TESTS: Only int types tested in shorthand. Missing str/bool shorthand coverage.

- [x] **Implement**: Field access — spec/08-types.md § Struct Types [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/` — field access type inference
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — field chaining (`c.ceo.name`, deep nesting)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/structs.rs` — all 30 tests exercise field access; nested field chains in `test_struct_nested_basic`, `test_struct_nested_three_levels` (verified 2026-03-29)

- [x] **Implement**: Destructuring — spec/14-expressions.md § Destructuring [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Parser in `ori_parse/src/grammar/expr/primary.rs` — `parse_binding_pattern()`
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — `test_destructure`, `test_destructure_partial`, `test_destructure_rename`
  - [ ] **AOT Tests**: No AOT coverage yet — `ori_llvm/tests/aot/structs.rs` does not test struct destructuring
  - [ ] WEAK TESTS: Only int fields (BasicStruct) tested. Missing str/nested/generic destructure, match arm destructuring.

- [ ] **Subsection close-out (5.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.2 Sum Types (Enums) — COMPLETED 2026-01-28

- [x] **Implement**: Parse `type Name = Variant1 | Variant2(Type)` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/tests/parser.rs`, `ori_parse/src/tests/compositional.rs` — sum type parsing tests
  - [x] **Ori Tests**: `tests/spec/declarations/sum_types.ori` (30+ tests: unit, single-field, multi-field, generic, recursive, pattern matching)
  - [x] **AOT Tests**: 30+ AOT tests across `fat_matrix/f12_sum_payload.rs` (8 tests), `fat_matrix/f06_pattern_matching.rs` (14 tests), `fat_ptr_iter/struct_sum_tuple.rs` (9 tests), `derives.rs` (sum type Eq tests) (verified 2026-03-29)

- [x] **Implement**: Unit variants [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Covered in parser tests
  - [x] **Ori Tests**: Color (Red|Green|Blue), Direction (NSEW), Toggle (On|Off), Status (4 variants)

- [x] **Implement**: Single-field variants `Variant(Type)` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Covered in parser tests
  - [x] **Ori Tests**: MyOption (MySome/MyNone), Message (Text/Empty)
  - [x] **AOT Tests**: `fat_matrix/f12_sum_payload.rs` — sum payload codegen (verified 2026-03-29)

- [x] **Implement**: Multi-field variants `Variant(x: Type, y: Type)` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Covered in parser tests
  - [x] **Ori Tests**: Shape (Circle/Rectangle), Point3D, Event (Click/KeyPress/Quit), Response (Success/Error)
  - [x] **AOT Tests**: `fat_matrix/f12_sum_payload.rs`, `fat_ptr_iter/struct_sum_tuple.rs` — multi-field variant codegen (verified 2026-03-29)

- [x] **Implement**: Struct variants [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Covered in parser tests — struct variant parsing (named fields)
  - [x] **Ori Tests**: All multi-field variants use named fields

- [x] **Implement**: Variant constructors [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/` — variant constructor type checking
  - [x] **Ori Tests**: All sum type tests construct variants; generic sum types (MyResult, MyOptional, LinkedList, Tree)
  - [x] **AOT Tests**: All sum type AOT tests exercise variant construction (verified 2026-03-29)

- [x] **Implement**: Pattern matching on variants [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/` — variant pattern matching
  - [x] **Ori Tests**: Exhaustive match, wildcard, nested match, variable binding, recursive Expr eval
  - [x] **AOT Tests**: `fat_matrix/f06_pattern_matching.rs` — 14 pattern matching AOT tests (verified 2026-03-29)
  - Note: `#derive(Eq)` for sum types verified working [done] (verified 2026-03-28) -- 16 tests in eq_sum.ori + EqColor/EqOption in sum_types.ori all pass

- [ ] **Subsection close-out (5.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.3 Newtypes

**Proposal**: `proposals/approved/newtype-pattern-proposal.md`

- [x] **Implement**: Parse `type Name = ExistingType` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/tests/parser.rs` — newtype parsing tests
  - [x] **Ori Tests**: `tests/spec/types/newtypes.ori` (UserId, Email, Age, Score — 9 tests)
  - [ ] WEAK TESTS: Only str and int underlying types tested. Missing bool, float, char, collections.
  - [ ] NEEDS PIN: No compile_fail test rejecting cross-newtype assignment (e.g., passing UserId where Email is expected).

- [x] **Implement**: Distinct type identity (nominal) [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/check/ (type registry)` — nominal type identity
  - [x] **Ori Tests**: `tests/spec/types/newtypes.ori` — separate UserId/Email types, validate_user/validate_email
  - [ ] **LLVM Support**: Transparent at runtime (same as underlying type)

- [x] **Implement**: Wrapping/unwrapping [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_eval/src/methods.rs` — newtype unwrap method
  - [x] **Ori Tests**: `tests/spec/types/newtypes.ori` — 9 tests (construction, unwrap, equality, params, computation)
  - [ ] **LLVM Support**: Transparent at runtime (newtype constructor just stores underlying value)
  - Note: Currently uses `.unwrap()` but spec mandates `.inner` — migration tracked in 5.3.4 below

- [ ] **Implement**: Change `.unwrap()` to `.inner` accessor — spec/08-types.md § Newtypes (verified not implemented 2026-03-29)
  - [ ] **Rust Tests**: Update `ori_eval/src/methods.rs` tests to use `.inner`
  - [ ] **Ori Tests**: Update `tests/spec/types/newtypes.ori` to use `.inner` (all 9 tests use `.unwrap()`)
  - [ ] **LLVM Support**: Update LLVM codegen to use `.inner` field access
  - [ ] **AOT Tests**: No AOT coverage yet
  - [ ] **Note**: `.inner` is always public regardless of newtype visibility
  - BUG FOUND: Tests use `.unwrap()` but spec/08-types.md section 8.6.3 mandates `.inner`. All 9 newtype tests will need migration when implemented.

- [ ] **Subsection close-out (5.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.4 Generic Types

- [x] **Implement**: Parse `type Name<T> = ...` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/tests/parser.rs` — generic type parsing tests
  - [x] **Ori Tests**: `tests/spec/types/generic.ori` (14 tests: Box<T>, Pair<A,B>, Container<T>, Wrapper<T> — int/str/bool/list types, nested generics, method calls on fields)

- [x] **Implement**: Multiple parameters `<T, U>` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: Covered by parser tests
  - [x] **Ori Tests**: `tests/spec/types/generic.ori` — `test_pair_int_str`, `test_pair_str_bool`

- [ ] **Implement**: Constrained `<T: Trait>` — spec/10-declarations.md § Generic Declarations
  - [ ] **Rust Tests**: `ori_types/src/check/bound_checking.rs` — constrained generics
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — `GenericDerived<T: Eq>` works
  - [ ] **LLVM Support**: LLVM codegen for constrained generics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/generic_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Multiple bounds `<T: A + B>` — spec/10-declarations.md § Generic Declarations
  - [ ] **Rust Tests**: `ori_types/src/check/bound_checking.rs` — multiple bounds
  - [ ] **Ori Tests**: Not tested yet
  - [ ] **LLVM Support**: LLVM codegen for multiple bounds
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/generic_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Generic application / Instantiation [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/` — `infer_struct` handles instantiation
  - [x] **Ori Tests**: `tests/spec/types/generic.ori` — 14 tests (Box<int>, Box<str>, Pair<int,str>, nested, chained access, method calls on fields)
  - **Note**: `Type::Applied` tracks instantiated generic types.

- [ ] **Implement**: Constraint checking — spec/08-types.md § Generic Types
  - [ ] **Rust Tests**: `ori_types/src/check/bound_checking.rs` — constraint checking
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — `GenericDerived<T: Eq>` constraint checked
  - [ ] **LLVM Support**: LLVM codegen for constraint checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/generic_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (5.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.5 Compound Types

Note: `tests/spec/types/collections.ori` is ENTIRELY COMMENTED OUT (416 lines) — type checker doesn't support collection type inference yet. Lists, tuples, maps work in the evaluator but type checker support is pending.
HYGIENE: Commented-out code violates impl-hygiene.md ("No commented-out code ever: use version control"). File should have active tests or be empty with a tracking note.

- [ ] **Implement**: List `[T]` — spec/08-types.md § List Type
  - [ ] **Rust Tests**: `ori_types/src/infer/collections.rs` — list type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori` — all commented out
  - [ ] **LLVM Support**: LLVM codegen for list type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet
  - Note: Lists work in evaluator (used in struct_types.ori, sum_types.ori) but type inference commented out

- [ ] **Implement**: Map `{K: V}` — spec/08-types.md § Map Type
  - [ ] **Rust Tests**: `ori_types/src/infer/collections.rs` — map type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori` — all commented out
  - [ ] **LLVM Support**: LLVM codegen for map type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Set `Set<T>` — spec/08-types.md § Set Type
  - [ ] **Rust Tests**: `ori_types/src/infer/collections.rs` — set type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori` — all commented out
  - [ ] **LLVM Support**: LLVM codegen for set type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Tuple `(T, U)` — spec/08-types.md § Tuple Types [partial] (verified 2026-03-29)
  - [x] **Parser**: Numeric field access `.0`, `.1` — `expect_member_name()` accepts integer literals (2026-02-15)
  - [x] **Parser Rust Tests**: `ori_parse/src/tests/parser.rs` — `test_tuple_field_access`, `test_chained_tuple_field_access_with_parens`, `test_bare_chained_tuple_field_is_error` (2026-02-15)
  - [x] **Ori Tests**: `tests/spec/types/tuple_types.ori` — 30+ tests: unit, pair, triple, quad, mixed types, nested, collections, Option/Result, destructuring, params/returns, inference, equality, field access (verified 2026-03-29)
  - [x] **Spec/Grammar**: Updated `grammar.ebnf` § `member_name`, `09-expressions.md`, `06-types.md` (2026-02-15)
  - [ ] **Rust Tests**: `ori_types/src/infer/` — tuple type inference
  - [x] **AOT Tests**: `ori_llvm/tests/aot/tuples.rs` — 28 tests, 6 ignored (construction, field access, destructuring, nested tuples, function param/return, strings, control flow, closures, equality) (verified 2026-03-29)
  - [ ] **Parser**: Chained field access without parens (`t.0.1`) — requires source text in parser or text in `Float` token
  - Note: Tuples work in evaluator and AOT. Comprehensive coverage across int/str/bool/float/char types, nested tuples, collections, Option/Result.
  - BUG FOUND: decision tree column count mismatch on tuple patterns with nested constructors + wildcard (e.g., `(None, _)`) — see `compile/mod.rs:67`

- [ ] **Implement**: Range `Range<T>` — spec/08-types.md § Range Type
  - [ ] **Rust Tests**: `ori_types/src/infer/` — range type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for range type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: Function `(T) -> U` — spec/08-types.md § Function Types
  - [ ] **Rust Tests**: `ori_types/src/infer/lambda.rs` — function type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/lambdas.ori`
  - [ ] **LLVM Support**: LLVM codegen for function types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet
  - Note: Function types work in evaluator (used in struct_types.ori `WithFunction`)

- [ ] **Subsection close-out (5.5)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.5 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.5: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.6 Built-in Generic Types

- [x] **Implement**: `Option<T>` with `Some`/`None` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/` — Option type handling
  - [x] **Ori Tests**: Used throughout test suite (struct_types.ori, sum_types.ori, traits/)
  - [x] **AOT Tests**: AOT coverage exists in `fat_ptr_iter/struct_sum_tuple.rs`, `fat_matrix/f12_sum_payload.rs` (verified 2026-03-29)

- [x] **Implement**: `Result<T, E>` with `Ok`/`Err` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/` — Result type handling
  - [x] **Ori Tests**: Used in traits/core/ tests, tuple_types.ori, test suite
  - [x] **AOT Tests**: AOT coverage exists in `fat_ptr_iter` tests (verified 2026-03-29)

- [x] **Implement**: `Ordering` with `Less`/`Equal`/`Greater` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/` — Ordering type handling
  - [x] **Ori Tests**: `tests/spec/types/ordering/methods.ori` (32+ tests), `then_with.ori`, `match_ordering.ori`
  - [ ] **AOT Tests**: No dedicated Ordering AOT tests yet

- [ ] **Implement**: `Error` type — spec/17-errors-and-panics.md § Error Conventions
  - [ ] **Rust Tests**: `ori_types/src/infer/` — Error type handling
  - [ ] **Ori Tests**: `tests/spec/types/error.ori` — not verified
  - [ ] **LLVM Support**: LLVM codegen for Error type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Implement**: `Channel<T>` — spec/08-types.md § Channel
  - [ ] **Rust Tests**: `ori_types/src/infer/` — Channel type handling
  - [ ] **Ori Tests**: `tests/spec/types/channel.ori` — not implemented
  - [ ] **LLVM Support**: LLVM codegen for Channel type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/channel_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [ ] **Subsection close-out (5.6)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.6 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.6: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.7 `with` Clause on Type Declarations (Capability Unification)

**Proposal**: `proposals/approved/capability-unification-generics-proposal.md` — Phase 1

Replace `#derive(Trait)` with `type T: Trait = ...`. Derivation moves from attribute syntax into the type declaration grammar as a `:` trait clause.

> **SUPERSEDES**: Previous syntax `#derive(Eq, Clone)` and `#[derive(Eq, Clone)]` are both replaced by `type T: Eq, Clone = { ... }`.

### Syntax Migration

- [ ] **Implement**: Parser — parse `:` trait clause in `parse_type_decl()` between generics and `where`/`=`
  - [ ] **Rust Tests**: `ori_parse/src/tests/parser.rs` — `:` trait clause parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/trait_clause.ori`
- [ ] **Implement**: IR — add `derive_traits: Vec<DerivedTrait>` to `TypeDef` node (replacing attribute-sourced data)
- [ ] **Implement**: Remove `parse_derive_attr()` from parser; keep as migration error suggesting `:`
  - [ ] **Ori Tests**: `tests/compile-fail/old_derive_syntax.ori`
- [ ] **Migration**: Update all `#derive(...)` in spec tests to `:` trait clause (~193 files)
- [ ] **Migration**: Migration script (`scripts/migrate_derive_syntax.py`)
- [ ] **Update Spec**: `grammar.ebnf` — `type_def` production
- [ ] **Update**: `.claude/rules/ori-syntax.md` — derive syntax
- [ ] **Verify**: `./test-all.sh` passes after migration

### Existing Derive Functionality (carries forward)

All 7 derivable traits have AOT coverage in `ori_llvm/tests/aot/derives.rs` (43 tests passing, verified 2026-03-29).

- [x] **Implement**: Parse `#[derive(Trait1, Trait2)]` [done] (verified 2026-03-29) — to be replaced by `:` trait clause
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — derive attribute parsing
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` (15+ tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/derives.rs` — 43 tests covering all 7 derivable traits (verified 2026-03-29)

- [x] **Implement**: `#derive(Eq)` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/check/ (derive Eq)` — derive Eq generation
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — EqPoint, EmptyDerived, SingleFieldDerived, nested derive, generic derive; `tests/spec/traits/derive/eq.ori`, `eq_sum.ori` (16 tests)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/structs.rs` — `test_struct_derived_eq`, `test_struct_derived_eq_string`; `ori_llvm/tests/aot/derives.rs` — `test_aot_derive_eq_basic`, `test_aot_derive_eq_mixed_types`, `test_aot_derive_eq_single_field`, `test_aot_derive_eq_with_strings`, `test_aot_derive_eq_payload_sum_type`, `test_aot_derive_eq_mixed_variant_comparison`, `test_aot_derive_eq_single_payload_variant`, `test_aot_derive_multiple_traits`, `test_aot_journey_11_derived_eq` (9+ tests, verified 2026-03-29)
  - Note: Derive(Eq) for sum types verified working [done] (verified 2026-03-28) -- eq_sum.ori 16 tests + sum_types.ori EqColor/EqOption all pass

- [x] **Implement**: `#derive(Clone)` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/check/ (derive Clone)` — derive Clone generation
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — ClonePoint, SingleFieldDerived; `tests/spec/traits/derive/clone.ori`, `all_derives.ori`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/derives.rs` — `test_aot_derive_clone_basic`, `test_aot_derive_clone_large_struct` + 12 primitive clone tests (int, float, bool, str, list_int, list_empty, option_some/none, result_ok/err, tuple_pair/triple) (14+ tests, verified 2026-03-29)

- [x] **Implement**: `#derive(Hashable)` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/check/ (derive Hashable)` — derive Hashable generation
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — HashPoint, MultiAttrPoint; `tests/spec/traits/derive/hashable.ori`; negative pin: `tests/compile-fail/hashable_without_eq.ori`
  - [x] **AOT Tests**: `ori_llvm/tests/aot/derives.rs` — `test_aot_derive_hash_equal_values`, `test_aot_derive_hash_different_values`, `test_aot_derive_hash_float_neg_zero`, `test_aot_derive_hash_str_content`, `test_aot_derive_hash_byte_field` (5 tests, verified 2026-03-29)

- [x] **Implement**: `#derive(Debug)` — spec/10-declarations.md § Attributes [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/check/derives/` — derive Debug generation
  - [x] **Ori Tests**: `tests/spec/traits/derive/debug.ori`, `tests/spec/declarations/attributes.ori` — DebugPoint
  - [x] **AOT Tests**: `ori_llvm/tests/aot/derives.rs` — `test_aot_derive_debug_short_str_no_leak`, `test_aot_derive_debug_str_field_no_leak`, `test_aot_derive_debug_mixed_str_int_no_leak`, `test_aot_derive_debug_multi_str_no_leak` (4 tests, verified 2026-03-29)
  - Note: attributes.ori DebugPoint test only checks `debug_str.len() > 0` — WEAK assertion, does not verify format. Dedicated debug.ori and AOT tests are stronger.

- [x] **Implement**: `#derive(Printable)` — spec/10-declarations.md § Attributes [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/check/derives/printable.rs` — derive Printable generation
  - [x] **Ori Tests**: `tests/spec/traits/derive/printable.ori` — struct basic, single str field, nested struct, payloadless variants, sum type variants with payloads (all pass)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/derives.rs` — `test_aot_derive_printable_basic` (1 test, verified 2026-03-29)

- [x] **Implement**: `#derive(Default)` — spec/10-declarations.md § Attributes [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/check/derives/default.rs` — derive Default generation
  - [x] **Ori Tests**: `tests/spec/traits/derive/default.ori` — Point (int->0), Config (str->"", int->0, bool->false), single field, float fields (all pass). Negative: `tests/compile-fail/default_sum_type.ori` rejects derive(Default) on sum types.
  - [x] **AOT Tests**: `ori_llvm/tests/aot/derives.rs` — `test_aot_derive_default_basic`, `test_aot_derive_default_str_field`, `test_aot_derive_default_mixed_types`, `test_aot_derive_default_nested`, `test_aot_derive_default_eq_integration` (5 tests, verified 2026-03-29)

- [x] **Implement**: `#derive(Comparable)` — spec/08-types.md § 8.6.4 [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_ir/derives/mod.rs` — DerivedTrait::Comparable variant
  - [x] **Ori Tests**: `tests/spec/traits/derive/comparable.ori` (struct comparison), `tests/spec/traits/derive/comparable_sum.ori` (sum type variant ordering)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/derives.rs` — `test_aot_derive_comparable_basic`, `test_aot_derive_comparable_single_field`, `test_aot_derive_comparable_with_strings`, `test_aot_derive_comparable_first_field_wins` (4 tests, verified 2026-03-29)

- [ ] **Subsection close-out (5.7)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.7 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.7: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.8 Visibility

- [x] **Implement**: Parse `pub type Name = ...` [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/item.rs` — pub type parsing
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — `pub type PublicStruct`, `tests/spec/declarations/sum_types.ori` — `pub type PublicStatus`
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Public visible from other modules [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_eval/src/interpreter/module/visibility.rs` — public visibility
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` — `pub type Point` imported cross-module
  - [ ] **AOT Tests**: No AOT coverage yet
  - [ ] WEAK TESTS: No cross-module import test importing from a separate file and verifying pub types are accessible.

- [x] **Implement**: Private only in declaring module [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_eval/src/interpreter/module/visibility.rs` — private visibility
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` — `type InternalPoint` (private)
  - [ ] **AOT Tests**: No AOT coverage yet
  - [ ] NEEDS PIN: No compile_fail test for private type import rejection from another module.

- [ ] **Subsection close-out (5.8)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.8 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.8: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.9 Associated Functions

**Proposal**: `proposals/approved/associated-functions-language-feature.md`

Generalize associated functions to work for ANY type with an `impl` block, removing hardcoded type checks. Enables syntax like `Point.origin()`, `Builder.new()`, `Duration.from_seconds(s: 10)`.

### Migration

- [x] **Implement**: Remove `is_type_name_for_associated_functions()` hardcoded checks [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/ (calls)` — general type name resolution
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — user types (Point, Builder, Counter, Rectangle, Pair) all work (12+ tests)

### Parsing

- [x] **Implement**: Parse `Type.method(...)` syntax in expression position [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — type-prefixed method call
  - [x] **Ori Tests**: `Point.origin()`, `Builder.new()`, `Counter.zero()`, `Rectangle.square(size: 5)`

- [x] **Implement**: Distinguish type name vs value in resolution [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/` — type vs value resolution
  - [x] **Ori Tests**: `test_instance_vs_associated` — `Pair.create()` (type) vs `p.sum()` (value)

### Associated Function Registry

- [x] **Implement**: Track methods without `self` in impl blocks [done] (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/check/ (method registry)` — associated function registry
  - [x] **Ori Tests**: Point.origin(), Point.new(), Builder.new(), Counter.zero(), Counter.starting_at(), Rectangle.square/from_dimensions/unit(), Pair.create()

- [x] **Implement**: Built-in associated functions for Duration [done] (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — `Duration.from_seconds(s: 5)` verified
  - Duration.from_nanoseconds(ns:), from_microseconds(us:), from_milliseconds(ms:)
  - Duration.from_seconds(s:), from_minutes(m:), from_hours(h:)

- [x] **Implement**: Built-in associated functions for Size [done] (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — `Size.from_megabytes(mb: 2)` verified
  - Size.from_bytes(b:), from_kilobytes(kb:), from_megabytes(mb:)
  - Size.from_gigabytes(gb:), from_terabytes(tb:)

### Generic Types

- [ ] **Implement**: Full type arguments required for generic associated functions
  - [ ] **Ori Tests**: Not tested — `Option<int>.some(value: 42)` pattern not verified
  - Example: `Option<int>.some(value: 42)`

### Self Return Type

- [x] **Implement**: Allow `Self` as return type in associated functions [done] (verified 2026-03-29)
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — Point.origin() -> Self, Builder.new() -> Self, Counter.zero() -> Self, Counter.increment(self) -> Self

### Trait Associated Functions

- [ ] **Implement**: Traits can define associated functions without `self`
  - [ ] **Rust Tests**: `ori_types/src/check/registration/traits.rs` — trait associated functions
  - [ ] **Ori Tests**: Not tested — `trait Default { @default () -> Self }` pattern
  - Example: `trait Default { @default () -> Self }`

### LLVM Support

- [ ] **LLVM Support**: Codegen for associated function calls [partial] (verified 2026-03-29)
  - [x] **AOT Tests**: Partial — `ori_llvm/tests/aot/traits.rs` has `test_aot_inherent_impl_method`, `test_aot_impl_method_field_access` (inherent impl methods work in AOT, verified 2026-03-29)
  - [ ] **AOT Tests**: No dedicated associated function (no `self`) AOT tests yet

- [ ] **Subsection close-out (5.9)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.9 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.9: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 5.10 Section Completion Checklist

- [x] Struct types (definition, literal, shorthand, field access, destructuring, spread, generic) [done] (verified 2026-03-29)
- [x] Sum types (unit, single-field, multi-field, generic, recursive, pattern matching) [done] (verified 2026-03-29)
- [x] Newtypes (construction, unwrap, nominal identity) [done] (verified 2026-03-29)
- [x] Generic types (single/multiple params, instantiation, field access chain) [done] (verified 2026-03-29)
- [x] Built-in generics: Option, Result, Ordering [done] (verified 2026-03-29)
- [x] Derive: Eq, Clone, Hashable on structs [done] (verified 2026-03-29)
- [x] Derive: Debug, Printable, Default — working [done] (verified 2026-03-29)
- [x] Derive: Comparable — working [done] (verified 2026-03-29)
- [x] Derive(Eq) for sum types — working [done] (verified 2026-03-28)
- [x] Visibility: pub type, private by default [done] (verified 2026-03-29)
- [x] Associated functions: Type.method() for user types, Duration, Size [done] (verified 2026-03-29)
- [x] AOT coverage: structs (30 tests), sum types (30+ tests), tuples (28 tests), all 7 derives (43 tests), Option, Result [done] (verified 2026-03-29)
- [ ] Compound type inference (5.5) — entirely pending (collections.ori all commented out)
- [ ] Newtype `.inner` accessor migration — pending (spec/08-types.md mandates `.inner`, tests use `.unwrap()`)
- [ ] Generic associated functions with type args — not tested
- [ ] Trait associated functions — not tested
- [ ] Capability unification syntax migration (`:` trait clause) — not started
- [ ] Struct destructuring AOT tests — missing
- [ ] Struct shorthand AOT tests — missing
- [ ] Newtype negative pin tests (cross-newtype assignment) — missing
- [ ] Visibility negative pin tests (private type import rejection) — missing
- [ ] Commented-out `collections.ori` cleanup — hygiene violation
- [ ] Run full test suite: `./test-all.sh`
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria**: User-defined structs and enums work
**Status**: Evaluator and AOT support complete for core features. All 7 derivable traits verified with eval + AOT coverage (43 AOT derive tests in derives.rs). Type checker compound inference, capability unification syntax, and newtype `.inner` accessor pending. Verified 2026-03-29.

- [ ] **Subsection close-out (5.10)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-5.10 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 5.10: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

