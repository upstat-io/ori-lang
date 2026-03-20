---
section: 5
title: Type Declarations
status: in-progress
reviewed: false
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

**Status**: In-progress — Structs (5.1), sum types (5.2), newtypes (5.3), generics (5.4), built-in generics (5.6), derive (5.7), visibility (5.8), associated functions (5.9) all working in evaluator. Compound type inference (5.5) entirely pending. LLVM tests missing. Verified 2026-02-10.

> **SYNC POINT**: Adding a new built-in type (e.g., `Duration`, `Size`, `CPtr`, `Nursery`) requires updates across `ori_ir` (type variant), `ori_types` (registration + method signatures), `ori_eval` (runtime representation + method dispatch), `ori_llvm` (type layout + codegen), and `library/std/` (prelude or module definitions). This section primarily covers user-defined types, but built-in types added by other sections (11, 16, 17, 18) follow the same pattern.

---

## 5.1 Struct Types

- [x] **Implement**: Parse `type Name = { field: Type, ... }` — spec/08-types.md § Struct Types, spec/10-declarations.md § Type Declarations [done] (2026-02-10)
  - [x] **Rust Tests**: STALE: `ori_parse/src/grammar/attr.rs — test_parse_struct_type` does not exist; actual parser tests in `ori_parse/src/tests/compositional.rs` (9 struct-related tests)
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` (30+ tests: basic, single field, empty, nested, many fields, mixed types, generic, with Option/List/Tuple/Function fields)

- [x] **Implement**: Register struct in type environment — spec/10-declarations.md § Type Declarations [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/check/registration/` — type registry tests
  - [x] **Ori Tests**: All struct tests verify type registration

- [x] **Implement**: Parse struct literals `Name { field: value }` — spec/08-types.md § Struct Types [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — struct literal parsing
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — all tests create struct literals

- [x] **Implement**: Type check struct literals — spec/08-types.md § Struct Types [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/` — struct literal type checking
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct literal construction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/struct_tests.rs` — struct literal codegen (file does not exist)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/structs.rs` — STALE: was "30 tests, 1 ignored"; now 32 passed, 0 ignored (struct construction with int/bool/str/mixed fields, update syntax, nested structs, function params/returns, closures, derived Eq)

- [x] **Implement**: Shorthand `Point { x, y }` — spec/08-types.md § Struct Types [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — shorthand parsing
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — `test_shorthand_init`, `test_mixed_shorthand`
  - [ ] **LLVM Support**: LLVM codegen for shorthand struct construction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/struct_tests.rs` — shorthand struct codegen (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet — `ori_llvm/tests/aot/structs.rs` does not test shorthand syntax

- [x] **Implement**: Field access — spec/08-types.md § Struct Types [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/` — field access type inference
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — field chaining (`c.ceo.name`, deep nesting)
  - [ ] **LLVM Support**: LLVM codegen for struct field access
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/struct_tests.rs` — field access codegen (file does not exist)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/structs.rs` — all 30 tests exercise field access; nested field chains in `test_struct_nested_basic`, `test_struct_nested_three_levels`

- [x] **Implement**: Destructuring — spec/14-expressions.md § Destructuring [done] (2026-02-10)
  - [x] **Rust Tests**: Parser in `ori_parse/src/grammar/expr/primary.rs` — `parse_binding_pattern()`
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — `test_destructure`, `test_destructure_partial`, `test_destructure_rename`
  - [ ] **LLVM Support**: LLVM codegen for struct destructuring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/struct_tests.rs` — destructuring codegen (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet — `ori_llvm/tests/aot/structs.rs` does not test struct destructuring

---

## 5.2 Sum Types (Enums) — COMPLETED 2026-01-28

- [x] **Implement**: Parse `type Name = Variant1 | Variant2(Type)` [done] (2026-02-10)
  - [x] **Rust Tests**: STALE: `ori_parse/src/grammar/attr.rs — test_parse_sum_type` does not exist
  - [x] **Ori Tests**: `tests/spec/declarations/sum_types.ori` (30+ tests)

- [x] **Implement**: Unit variants [done] (2026-02-10)
  - [x] **Rust Tests**: STALE: `ori_parse/src/grammar/attr.rs — test_parse_sum_type` does not exist
  - [x] **Ori Tests**: Color (Red|Green|Blue), Direction (NSEW), Toggle (On|Off), Status (4 variants)

- [x] **Implement**: Single-field variants `Variant(Type)` [done] (2026-02-10)
  - [x] **Rust Tests**: STALE: `ori_parse/src/grammar/attr.rs` reference does not exist
  - [x] **Ori Tests**: MyOption (MySome/MyNone), Message (Text/Empty)
  - [ ] **LLVM Support**: LLVM codegen for single-field variants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sum_type_tests.rs` (file does not exist)
  - [x] **AOT Tests**: STALE: was "No AOT coverage yet"; `cargo test -p ori_llvm -- enum` shows 17 passing tests (unit variants, construction, mixed variants, recursive enum, params/returns, derived Eq)

- [x] **Implement**: Multi-field variants `Variant(x: Type, y: Type)` [done] (2026-02-10)
  - [x] **Rust Tests**: STALE: `ori_parse/src/grammar/attr.rs` reference does not exist
  - [x] **Ori Tests**: Shape (Circle/Rectangle), Point3D, Event (Click/KeyPress/Quit), Response (Success/Error)
  - [ ] **LLVM Support**: LLVM codegen for multi-field variants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sum_type_tests.rs` (file does not exist)
  - [x] **AOT Tests**: STALE: was "No AOT coverage yet"; covered by enum AOT tests above

- [x] **Implement**: Struct variants [done] (2026-02-10)
  - [x] **Rust Tests**: STALE: `ori_parse/src/grammar/attr.rs` reference does not exist
  - [x] **Ori Tests**: All multi-field variants use named fields

- [x] **Implement**: Variant constructors [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/` — variant constructor type checking
  - [x] **Ori Tests**: All sum type tests construct variants; generic sum types (MyResult, MyOptional, LinkedList, Tree)
  - [ ] **LLVM Support**: LLVM codegen for variant constructors
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sum_type_tests.rs` (file does not exist)
  - [x] **AOT Tests**: STALE: was "No AOT coverage yet"; covered by enum AOT tests above

- [x] **Implement**: Pattern matching on variants [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/` — variant pattern matching
  - [x] **Ori Tests**: Exhaustive match, wildcard, nested match, variable binding, recursive Expr eval
  - [ ] **LLVM Support**: LLVM codegen for variant pattern matching
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` (file does not exist)
  - [x] **AOT Tests**: STALE: was "No AOT coverage yet"; covered by enum AOT tests above
  - STALE: Note claimed `#derive(Eq)` for sum types NOT working -- it IS working (tests pass in `sum_types.ori` lines 437-463 and `tests/spec/traits/derive/eq_sum.ori`)

---

## 5.3 Newtypes

**Proposal**: `proposals/approved/newtype-pattern-proposal.md`

- [x] **Implement**: Parse `type Name = ExistingType` [done] (2026-02-10)
  - [x] **Rust Tests**: STALE: `ori_parse/src/grammar/attr.rs — test_parse_newtype` does not exist
  - [x] **Ori Tests**: `tests/spec/types/newtypes.ori` (UserId, Email, Age, Score)

- [x] **Implement**: Distinct type identity (nominal) [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/check/ (type registry)` — nominal type identity
  - [x] **Ori Tests**: `tests/spec/types/newtypes.ori` — separate UserId/Email types, validate_user/validate_email
  - [ ] **LLVM Support**: Transparent at runtime (same as underlying type)

- [x] **Implement**: Wrapping/unwrapping [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_eval/src/methods.rs` — newtype unwrap method
  - [x] **Ori Tests**: `tests/spec/types/newtypes.ori` — 9 tests (construction, unwrap, equality, params, computation)
  - [ ] **LLVM Support**: Transparent at runtime (newtype constructor just stores underlying value)

- [ ] **Implement**: Change `.unwrap()` to `.inner` accessor — spec/08-types.md § Newtypes
  - [ ] **Rust Tests**: Update `ori_eval/src/methods.rs` tests to use `.inner`
  - [ ] **Ori Tests**: Update `tests/spec/types/newtypes.ori` to use `.inner`
  - [ ] **LLVM Support**: Update LLVM codegen to use `.inner` field access
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/newtype_tests.rs` — `.inner` accessor codegen
  - [ ] **AOT Tests**: No AOT coverage yet
  - [ ] **Note**: `.inner` is always public regardless of newtype visibility
  - Note: Currently using `.unwrap()` — migration to `.inner` pending

---

## 5.4 Generic Types

- [x] **Implement**: Parse `type Name<T> = ...` [done] (2026-02-10)
  - [x] **Rust Tests**: STALE: `ori_parse/src/grammar/attr.rs — test_parse_generic_type_with_bounds` does not exist; actual tests in `ori_parse/src/grammar/ty/tests.rs` — `test_parse_generic_type`
  - [x] **Ori Tests**: `tests/spec/types/generic.ori` (Box<T>, Pair<A,B>, Container<T>, Wrapper<T>)

- [x] **Implement**: Multiple parameters `<T, U>` [done] (2026-02-10)
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

- [x] **Implement**: Generic application / Instantiation [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/` — `infer_struct` handles instantiation
  - [x] **Ori Tests**: `tests/spec/types/generic.ori` — 14 tests (Box<int>, Box<str>, Pair<int,str>, nested, chained access, method calls on fields)
  - **Note**: `Type::Applied` tracks instantiated generic types.

- [ ] **Implement**: Constraint checking — spec/08-types.md § Generic Types
  - [ ] **Rust Tests**: `ori_types/src/check/bound_checking.rs` — constraint checking
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — `GenericDerived<T: Eq>` constraint checked
  - [ ] **LLVM Support**: LLVM codegen for constraint checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/generic_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 5.5 Compound Types

Note: `tests/spec/types/collections.ori` is ENTIRELY COMMENTED OUT — type checker doesn't support collection type inference yet. Lists, tuples, maps work in the evaluator but type checker support is pending.

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

- [ ] **Implement**: Tuple `(T, U)` — spec/08-types.md § Tuple Types
  - [x] **Parser**: Numeric field access `.0`, `.1` — `expect_member_name()` accepts integer literals (2026-02-15)
  - [x] **Parser Rust Tests**: `ori_parse/src/tests/parser.rs` — `test_tuple_field_access`, `test_chained_tuple_field_access_with_parens`, `test_bare_chained_tuple_field_is_error` (2026-02-15)
  - [x] **Ori Tests**: `tests/spec/types/tuple_types.ori` — 6 field access tests: pair, different types, triple, nested with parens, single-element, return value (2026-02-15)
  - [x] **Spec/Grammar**: Updated `grammar.ebnf` § `member_name`, `09-expressions.md`, `06-types.md` (2026-02-15)
  - [ ] **Rust Tests**: `ori_types/src/infer/` — tuple type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori` — all commented out
  - [ ] **LLVM Support**: LLVM codegen for tuple type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` (file does not exist)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/tuples.rs` — 28 tests, 6 ignored (construction, field access, destructuring, nested tuples, function param/return, strings, control flow, closures, equality)
  - [ ] **Parser**: Chained field access without parens (`t.0.1`) — requires source text in parser or text in `Float` token
  - Note: Tuples work in evaluator (used in struct_types.ori destructuring) but type inference commented out
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

---

## 5.6 Built-in Generic Types

- [x] **Implement**: `Option<T>` with `Some`/`None` [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/` — Option type handling
  - [x] **Ori Tests**: Used throughout test suite (struct_types.ori, sum_types.ori, traits/)
  - [ ] **LLVM Support**: LLVM codegen for Option type — inline IR in lower_calls.rs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_result_tests.rs` (file does not exist)
  - [x] **AOT Tests**: STALE: was "No AOT coverage yet"; `cargo test -p ori_llvm -- option` shows 94 passing tests (3 ignored)

- [x] **Implement**: `Result<T, E>` with `Ok`/`Err` [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/` — Result type handling
  - [x] **Ori Tests**: Used in traits/core/ tests, test suite
  - [ ] **LLVM Support**: LLVM codegen for Result type — inline IR in lower_calls.rs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_result_tests.rs` (file does not exist)
  - [x] **AOT Tests**: STALE: was "No AOT coverage yet"; `cargo test -p ori_llvm -- result` shows 48 passing tests

- [x] **Implement**: `Ordering` with `Less`/`Equal`/`Greater` [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/` — Ordering type handling
  - [x] **Ori Tests**: `tests/spec/types/ordering/methods.ori` (32 tests)
  - [ ] **LLVM Support**: LLVM codegen for Ordering type — i8 comparison in lower_calls.rs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/ordering_tests.rs` (file does not exist)
  - [x] **AOT Tests**: STALE: was "No AOT coverage yet"; `cargo test -p ori_llvm -- ordering` shows 7 passing tests

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

- [x] **Implement**: Parse `#[derive(Trait1, Trait2)]` [done] (2026-02-10) — to be replaced by `:` trait clause
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — derive attribute parsing
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` (15+ tests)
  - [ ] **LLVM Support**: LLVM codegen for derive attributes
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `#derive(Eq)` [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/check/ (derive Eq)` — derive Eq generation
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — EqPoint, EmptyDerived, SingleFieldDerived, nested derive, generic derive
  - [ ] **LLVM Support**: LLVM codegen for derived Eq
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)
  - [x] **AOT Tests**: `ori_llvm/tests/aot/structs.rs` — `test_struct_derived_eq`, `test_struct_derived_eq_string` (struct Eq with int and string fields)
  - STALE: Note claimed Derive(Eq) for SUM TYPES not working — it IS working (tests pass in `sum_types.ori` and `tests/spec/traits/derive/eq_sum.ori`)

- [x] **Implement**: `#derive(Clone)` [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/check/ (derive Clone)` — derive Clone generation
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — ClonePoint, SingleFieldDerived
  - [ ] **LLVM Support**: LLVM codegen for derived Clone
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `#derive(Hashable)` [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/check/ (derive Hashable)` — derive Hashable generation
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — HashPoint, MultiAttrPoint
  - [ ] **LLVM Support**: LLVM codegen for derived Hashable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `#derive(Printable)` — spec/10-declarations.md § Attributes [done]
  - [x] **Rust Tests**: `ori_types/src/check/derives/printable.rs` — derive Printable generation
  - [x] **Ori Tests**: STALE: was "skipped" — `tests/spec/traits/derive/printable.ori` has 6 active passing tests (struct basic, str field, nested, payloadless variant, payload variant, multi-payload variant); `tests/spec/declarations/attributes.ori` `test_derive_printable` also passes
  - [ ] **LLVM Support**: LLVM codegen for derived Printable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: `#derive(Default)` — spec/10-declarations.md § Attributes [done]
  - [x] **Rust Tests**: `ori_types/src/check/derives/default.rs` — derive Default generation
  - [x] **Ori Tests**: STALE: was "Not tested" — `tests/spec/traits/derive/default.ori` has 6 active passing tests (basic int fields, multiple types, single field, float fields, Eq integration, nested default)
  - [ ] **LLVM Support**: LLVM codegen for derived Default
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 5.8 Visibility

- [x] **Implement**: Parse `pub type Name = ...` [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/item.rs` — pub type parsing
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — `pub type PublicStruct`, `tests/spec/declarations/sum_types.ori` — `pub type PublicStatus`
  - [ ] **LLVM Support**: LLVM codegen for pub type visibility
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/visibility_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Public visible from other modules [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_eval/src/interpreter/module/visibility.rs` — public visibility
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` — `pub type Point` imported cross-module
  - [ ] **LLVM Support**: LLVM codegen for public visibility
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/visibility_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

- [x] **Implement**: Private only in declaring module [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_eval/src/interpreter/module/visibility.rs` — private visibility
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` — `type InternalPoint` (private)
  - [ ] **LLVM Support**: LLVM codegen for private visibility
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/visibility_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 5.9 Associated Functions

**Proposal**: `proposals/approved/associated-functions-language-feature.md`

Generalize associated functions to work for ANY type with an `impl` block, removing hardcoded type checks. Enables syntax like `Point.origin()`, `Builder.new()`, `Duration.from_seconds(s: 10)`.

### Migration

- [x] **Implement**: Remove `is_type_name_for_associated_functions()` hardcoded checks [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/ (calls)` — general type name resolution
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — user types (Point, Builder, Counter, Rectangle, Pair) all work

### Parsing

- [x] **Implement**: Parse `Type.method(...)` syntax in expression position [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — type-prefixed method call
  - [x] **Ori Tests**: `Point.origin()`, `Builder.new()`, `Counter.zero()`, `Rectangle.square(size: 5)`

- [x] **Implement**: Distinguish type name vs value in resolution [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/` — type vs value resolution
  - [x] **Ori Tests**: `test_instance_vs_associated` — `Pair.create()` (type) vs `p.sum()` (value)

### Associated Function Registry

- [x] **Implement**: Track methods without `self` in impl blocks [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/check/ (method registry)` — associated function registry
  - [x] **Ori Tests**: Point.origin(), Point.new(), Builder.new(), Counter.zero(), Counter.starting_at(), Rectangle.square/from_dimensions/unit(), Pair.create()

- [x] **Implement**: Built-in associated functions for Duration [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — `Duration.from_seconds(s: 5)` verified
  - Duration.from_nanoseconds(ns:), from_microseconds(us:), from_milliseconds(ms:)
  - Duration.from_seconds(s:), from_minutes(m:), from_hours(h:)

- [x] **Implement**: Built-in associated functions for Size [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — `Size.from_megabytes(mb: 2)` verified
  - Size.from_bytes(b:), from_kilobytes(kb:), from_megabytes(mb:)
  - Size.from_gigabytes(gb:), from_terabytes(tb:)

### Generic Types

- [ ] **Implement**: Full type arguments required for generic associated functions
  - [ ] **Ori Tests**: Not tested — `Option<int>.some(value: 42)` pattern not verified
  - Example: `Option<int>.some(value: 42)`

### Self Return Type

- [x] **Implement**: Allow `Self` as return type in associated functions [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — Point.origin() -> Self, Builder.new() -> Self, Counter.zero() -> Self, Counter.increment(self) -> Self

### Trait Associated Functions

- [ ] **Implement**: Traits can define associated functions without `self`
  - [ ] **Rust Tests**: `ori_types/src/check/registration/traits.rs` — trait associated functions
  - [ ] **Ori Tests**: Not tested — `trait Default { @default () -> Self }` pattern
  - Example: `trait Default { @default () -> Self }`

### LLVM Support

- [ ] **LLVM Support**: Codegen for associated function calls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/associated_function_tests.rs` (file does not exist)
  - [ ] **AOT Tests**: No AOT coverage yet

---

## 5.10 Section Completion Checklist

- [x] Struct types (definition, literal, shorthand, field access, destructuring, spread, generic) [done]
- [x] Sum types (unit, single-field, multi-field, generic, recursive, pattern matching) [done]
- [x] Newtypes (construction, unwrap, nominal identity) [done]
- [x] Generic types (single/multiple params, instantiation, field access chain) [done]
- [x] Built-in generics: Option, Result, Ordering [done]
- [x] Derive: Eq, Clone, Hashable on structs [done]
- [x] Visibility: pub type, private by default [done]
- [x] Associated functions: Type.method() for user types, Duration, Size [done]
- [ ] Compound type inference (5.5) — entirely pending
- [x] Derive: Printable, Default — STALE: was "not working"; both are working with dedicated test files (`tests/spec/traits/derive/printable.ori`, `tests/spec/traits/derive/default.ori`) [done]
- [x] Derive(Eq) for sum types — STALE: was "not working"; IS working (tests pass in `sum_types.ori` and `tests/spec/traits/derive/eq_sum.ori`) [done]
- [ ] Newtype `.inner` accessor migration — pending
- [ ] Generic associated functions with type args — not tested
- [ ] Trait associated functions — not tested
- [ ] LLVM codegen for all type declarations — STALE: was "no dedicated test files"; significant AOT coverage exists (32 struct, 17 enum, 27 tuple, 94 Option, 48 Result, 7 Ordering tests); remaining gaps: shorthand/destructuring AOT, newtype AOT
- [ ] Run full test suite: `./test-all.sh`

**Exit Criteria**: User-defined structs and enums work
**Status**: Evaluator support complete for core features. Type checker compound inference pending. Derive(Eq) for sum types, Printable, Default all working (previously claimed not working). Significant AOT coverage exists. Verified 2026-02-10.
