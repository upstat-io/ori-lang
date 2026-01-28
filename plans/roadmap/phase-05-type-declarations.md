# Phase 5: Type Declarations

**Goal**: User-defined types

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`, `spec/08-declarations.md`

---

## 5.1 Struct Types

- [x] **Implement**: Parse `type Name = { field: Type, ... }` — spec/06-types.md § Struct Types, spec/08-declarations.md § Type Declarations
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_struct_type`
  - [x] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [x] **Implement**: Register struct in type environment — spec/08-declarations.md § Type Declarations
  - [x] **Rust Tests**: `oric/src/typeck/checker/type_registration.rs` — type registry tests
  - [x] **Ori Tests**: N/A (tested via Rust unit tests)

- [x] **Implement**: Parse struct literals `Name { field: value }` — spec/06-types.md § Struct Types
  - [x] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — struct literal parsing
  - [x] **Ori Tests**: `tests/spec/traits/declaration.ori`
  - **Note**: Added 2024-01-25 — struct literal parsing was missing from postfix.rs

- [x] **Implement**: Type check struct literals — spec/06-types.md § Struct Types
  - [x] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — struct literal type checking
  - [x] **Ori Tests**: `tests/spec/types/struct.ori`

- [x] **Implement**: Shorthand `Point { x, y }` — spec/06-types.md § Struct Types
  - [x] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — shorthand parsing
  - [x] **Ori Tests**: `tests/spec/types/struct.ori`

- [x] **Implement**: Field access — spec/06-types.md § Struct Types
  - [x] **Rust Tests**: `oric/src/typeck/infer/postfix.rs` — field access type inference
  - [x] **Ori Tests**: `tests/spec/types/struct.ori`

- [x] **Implement**: Destructuring — spec/09-expressions.md § Destructuring
  - [x] **Rust Tests**: Parser in `ori_parse/src/grammar/expr/primary.rs` — `parse_binding_pattern()`
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — 8 new tests for struct/list/tuple destructuring

---

## 5.2 Sum Types (Enums)

- [x] **Implement**: Parse `type Name = Variant1 | Variant2(Type)` — spec/06-types.md § Sum Types, spec/08-declarations.md § Type Declarations
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_sum_type`
  - [x] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [x] **Implement**: Unit variants — spec/06-types.md § Sum Types
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — included in `test_parse_sum_type`
  - [x] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [ ] **Implement**: Tuple variants — spec/06-types.md § Sum Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — tuple variant parsing
  - [ ] **Ori Tests**: `tests/spec/types/sum.ori`
  - [ ] **LLVM Support**: LLVM codegen for tuple variants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sum_type_tests.rs` — tuple variant codegen

- [x] **Implement**: Struct variants — spec/06-types.md § Sum Types
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — struct variant parsing (named fields)
  - [x] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [ ] **Implement**: Variant constructors — spec/06-types.md § Sum Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — variant constructor type checking
  - [ ] **Ori Tests**: `tests/spec/types/sum.ori`
  - [ ] **LLVM Support**: LLVM codegen for variant constructors
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sum_type_tests.rs` — variant constructor codegen

- [ ] **Implement**: Pattern matching on variants — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/pattern.rs` — variant pattern matching
  - [ ] **Ori Tests**: `tests/spec/types/sum.ori`
  - [ ] **LLVM Support**: LLVM codegen for variant pattern matching
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — variant pattern codegen

---

## 5.3 Newtypes

- [x] **Implement**: Parse `type Name = ExistingType` — spec/06-types.md § Newtypes, spec/08-declarations.md § Type Declarations
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_newtype`
  - [x] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [ ] **Implement**: Distinct type identity (nominal) — spec/07-properties-of-types.md § Type Equivalence
  - [ ] **Rust Tests**: `oric/src/typeck/checker/type_registry.rs` — nominal type identity
  - [ ] **Ori Tests**: `tests/spec/types/newtype.ori`
  - [ ] **LLVM Support**: LLVM codegen for newtype identity
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/type_tests.rs` — newtype codegen

- [ ] **Implement**: Wrapping/unwrapping — spec/06-types.md § Newtypes
  - [ ] **Rust Tests**: `oric/src/eval/exec/expr.rs` — newtype construction/extraction
  - [ ] **Ori Tests**: `tests/spec/types/newtype.ori`
  - [ ] **LLVM Support**: LLVM codegen for newtype wrapping/unwrapping
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/type_tests.rs` — newtype wrap/unwrap codegen

---

## 5.4 Generic Types

- [x] **Implement**: Parse `type Name<T> = ...` — spec/06-types.md § Generic Types, spec/08-declarations.md § Generic Declarations
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_generic_type_with_bounds`
  - [x] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [x] **Implement**: Multiple parameters `<T, U>` — spec/08-declarations.md § Generic Declarations
  - [x] **Rust Tests**: Covered by parser tests
  - [x] **Ori Tests**: `tests/spec/types/generic.ori` — `test_pair_int_str`, `test_pair_str_bool`

- [ ] **Implement**: Constrained `<T: Trait>` — spec/08-declarations.md § Generic Declarations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/bound_checking.rs` — constrained generics
  - [ ] **Ori Tests**: `tests/spec/types/generic.ori`
  - [ ] **LLVM Support**: LLVM codegen for constrained generics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/generic_tests.rs` — constrained generic codegen

- [ ] **Implement**: Multiple bounds `<T: A + B>` — spec/08-declarations.md § Generic Declarations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/bound_checking.rs` — multiple bounds
  - [ ] **Ori Tests**: `tests/spec/types/generic.ori`
  - [ ] **LLVM Support**: LLVM codegen for multiple bounds
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/generic_tests.rs` — multiple bounds codegen

- [x] **Implement**: Generic application / Instantiation — spec/06-types.md § Generic Types
  - [x] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — `infer_struct` handles instantiation
  - [x] **Ori Tests**: `tests/spec/types/generic.ori` — all 14 tests cover instantiation
  - **Note**: Added `Type::Applied` variant to track instantiated generic types with type args.
    Struct literal inference creates fresh type vars for type params, substitutes in field types,
    and returns `Type::Applied { name, args }`. Field access on `Type::Applied` substitutes
    type args into field types.

- [ ] **Implement**: Constraint checking — spec/06-types.md § Generic Types
  - [ ] **Rust Tests**: `oric/src/typeck/checker/bound_checking.rs` — constraint checking
  - [ ] **Ori Tests**: `tests/spec/types/generic.ori`
  - [ ] **LLVM Support**: LLVM codegen for constraint checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/generic_tests.rs` — constraint checking codegen

---

## 5.5 Compound Types

- [ ] **Implement**: List `[T]` — spec/06-types.md § List Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/collections.rs` — list type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori`
  - [ ] **LLVM Support**: LLVM codegen for list type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — list type codegen

- [ ] **Implement**: Map `{K: V}` — spec/06-types.md § Map Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/collections.rs` — map type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori`
  - [ ] **LLVM Support**: LLVM codegen for map type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — map type codegen

- [ ] **Implement**: Set `Set<T>` — spec/06-types.md § Set Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/collections.rs` — set type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori`
  - [ ] **LLVM Support**: LLVM codegen for set type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — set type codegen

- [ ] **Implement**: Tuple `(T, U)` — spec/06-types.md § Tuple Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — tuple type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori`
  - [ ] **LLVM Support**: LLVM codegen for tuple type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` — tuple type codegen

- [ ] **Implement**: Range `Range<T>` — spec/06-types.md § Range Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — range type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for range type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` — range type codegen

- [ ] **Implement**: Function `(T) -> U` — spec/06-types.md § Function Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/lambda.rs` — function type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/lambdas.ori`
  - [ ] **LLVM Support**: LLVM codegen for function types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_tests.rs` — function type codegen

---

## 5.6 Built-in Generic Types

- [ ] **Implement**: `Option<T>` with `Some`/`None` — spec/06-types.md § Option
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Option type handling
  - [ ] **Ori Tests**: `tests/spec/types/option.ori`
  - [ ] **LLVM Support**: LLVM codegen for Option type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_result_tests.rs` — Option codegen

- [ ] **Implement**: `Result<T, E>` with `Ok`/`Err` — spec/06-types.md § Result
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Result type handling
  - [ ] **Ori Tests**: `tests/spec/types/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Result type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_result_tests.rs` — Result codegen

- [ ] **Implement**: `Ordering` with `Less`/`Equal`/`Greater` — spec/06-types.md § Ordering
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Ordering type handling
  - [ ] **Ori Tests**: `tests/spec/types/ordering.ori`
  - [ ] **LLVM Support**: LLVM codegen for Ordering type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/ordering_tests.rs` — Ordering codegen

- [ ] **Implement**: `Error` type — spec/20-errors-and-panics.md § Error Conventions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Error type handling
  - [ ] **Ori Tests**: `tests/spec/types/error.ori`
  - [ ] **LLVM Support**: LLVM codegen for Error type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_tests.rs` — Error codegen

- [ ] **Implement**: `Channel<T>` — spec/06-types.md § Channel
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Channel type handling
  - [ ] **Ori Tests**: `tests/spec/types/channel.ori`
  - [ ] **LLVM Support**: LLVM codegen for Channel type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/channel_tests.rs` — Channel codegen

---

## 5.7 Derive Attributes

> **NOTE - Pending Syntax Change**: Approved proposal changes attribute syntax:
> - Current: `#[derive(Eq, Clone)]`
> - New: `#derive(Eq, Clone)`
> See Phase 15 (Approved Syntax Proposals) § 15.1. Implement with new syntax directly.

- [ ] **Implement**: Parse `#derive(Trait1, Trait2)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — derive attribute parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for derive attributes
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — derive attribute codegen

- [ ] **Implement**: `#derive(Eq)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/eq.rs` — derive Eq generation
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for derived Eq
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — derived Eq codegen

- [ ] **Implement**: `#derive(Clone)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/clone.rs` — derive Clone generation
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for derived Clone
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — derived Clone codegen

- [ ] **Implement**: `#derive(Hashable)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/hashable.rs` — derive Hashable generation
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for derived Hashable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — derived Hashable codegen

- [ ] **Implement**: `#derive(Printable)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/printable.rs` — derive Printable generation
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for derived Printable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — derived Printable codegen

- [ ] **Implement**: `#derive(Default)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/default.rs` — derive Default generation
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for derived Default
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — derived Default codegen

---

## 5.8 Visibility

- [ ] **Implement**: Parse `pub type Name = ...` — spec/08-declarations.md § Visibility
  - [ ] **Rust Tests**: `ori_parse/src/grammar/item.rs` — pub type parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/visibility.ori`
  - [ ] **LLVM Support**: LLVM codegen for pub type visibility
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/visibility_tests.rs` — pub type codegen

- [ ] **Implement**: Public visible from other modules — spec/08-declarations.md § Visibility
  - [ ] **Rust Tests**: `oric/src/eval/module/visibility.rs` — public visibility
  - [ ] **Ori Tests**: `tests/spec/declarations/visibility.ori`
  - [ ] **LLVM Support**: LLVM codegen for public visibility
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/visibility_tests.rs` — public visibility codegen

- [ ] **Implement**: Private only in declaring module — spec/08-declarations.md § Visibility
  - [ ] **Rust Tests**: `oric/src/eval/module/visibility.rs` — private visibility
  - [ ] **Ori Tests**: `tests/spec/declarations/visibility.ori`
  - [ ] **LLVM Support**: LLVM codegen for private visibility
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/visibility_tests.rs` — private visibility codegen

---

## 5.9 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] 80+% test coverage
- [ ] Run full test suite: `./test-all`

**Exit Criteria**: User-defined structs and enums work
