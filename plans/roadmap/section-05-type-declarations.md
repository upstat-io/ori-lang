---
section: 5
title: Type Declarations
status: not-started
tier: 1
goal: User-defined types
spec:
  - spec/06-types.md
  - spec/07-properties-of-types.md
  - spec/08-declarations.md
sections:
  - id: "5.1"
    title: Struct Types
    status: not-started
  - id: "5.2"
    title: Sum Types (Enums)
    status: not-started
  - id: "5.3"
    title: Newtypes
    status: not-started
  - id: "5.4"
    title: Generic Types
    status: not-started
  - id: "5.5"
    title: Compound Types
    status: not-started
  - id: "5.6"
    title: Built-in Generic Types
    status: not-started
  - id: "5.7"
    title: Derive Attributes
    status: not-started
  - id: "5.8"
    title: Visibility
    status: not-started
  - id: "5.9"
    title: Associated Functions
    status: not-started
  - id: "5.10"
    title: Section Completion Checklist
    status: not-started
---

# Section 5: Type Declarations

**Goal**: User-defined types

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`, `spec/08-declarations.md`

**Status**: Partial — Core complete (5.1-5.2, 5.4 basic); newtypes `.inner` (5.3), constrained generics (5.4), associated functions (5.9) pending

---

## 5.1 Struct Types

- [ ] **Implement**: Parse `type Name = { field: Type, ... }` — spec/06-types.md § Struct Types, spec/08-declarations.md § Type Declarations
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_struct_type`
  - [ ] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [ ] **Implement**: Register struct in type environment — spec/08-declarations.md § Type Declarations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/type_registration.rs` — type registry tests
  - [ ] **Ori Tests**: N/A (tested via Rust unit tests)

- [ ] **Implement**: Parse struct literals `Name { field: value }` — spec/06-types.md § Struct Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — struct literal parsing
  - [ ] **Ori Tests**: `tests/spec/traits/declaration.ori`
  - **Note**: Added 2024-01-25 — struct literal parsing was missing from postfix.rs

- [ ] **Implement**: Type check struct literals — spec/06-types.md § Struct Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — struct literal type checking
  - [ ] **Ori Tests**: `tests/spec/types/struct.ori`

- [ ] **Implement**: Shorthand `Point { x, y }` — spec/06-types.md § Struct Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — shorthand parsing
  - [ ] **Ori Tests**: `tests/spec/types/struct.ori`

- [ ] **Implement**: Field access — spec/06-types.md § Struct Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/postfix.rs` — field access type inference
  - [ ] **Ori Tests**: `tests/spec/types/struct.ori`

- [ ] **Implement**: Destructuring — spec/09-expressions.md § Destructuring
  - [ ] **Rust Tests**: Parser in `ori_parse/src/grammar/expr/primary.rs` — `parse_binding_pattern()`
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori` — 8 new tests for struct/list/tuple destructuring

---

## 5.2 Sum Types (Enums) — COMPLETED 2026-01-28

- [ ] **Implement**: Parse `type Name = Variant1 | Variant2(Type)` — spec/06-types.md § Sum Types, spec/08-declarations.md § Type Declarations
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_sum_type`
  - [ ] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [ ] **Implement**: Unit variants — spec/06-types.md § Sum Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — included in `test_parse_sum_type`
  - [ ] **Ori Tests**: `tests/spec/types/sum_types.ori` — unit variant tests

- [ ] **Implement**: Single-field variants `Variant(Type)` — spec/06-types.md § Sum Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — single-field variant parsing
  - [ ] **Ori Tests**: `tests/spec/types/sum_types.ori` — single-field variant tests
  - [ ] **LLVM Support**: LLVM codegen for single-field variants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sum_type_tests.rs` — single-field variant codegen

- [ ] **Implement**: Multi-field variants `Variant(x: Type, y: Type)` — spec/06-types.md § Sum Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — multi-field variant parsing
  - [ ] **Ori Tests**: `tests/spec/types/sum_types.ori` — multi-field variant tests (`Click(x, y)`)
  - [ ] **LLVM Support**: LLVM codegen for multi-field variants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sum_type_tests.rs` — multi-field variant codegen

- [ ] **Implement**: Struct variants — spec/06-types.md § Sum Types
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — struct variant parsing (named fields)
  - [ ] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [ ] **Implement**: Variant constructors — spec/06-types.md § Sum Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — variant constructor type checking
  - [ ] **Ori Tests**: `tests/spec/types/sum_types.ori` — 11 tests for variant construction
  - [ ] **LLVM Support**: LLVM codegen for variant constructors
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sum_type_tests.rs` — variant constructor codegen

- [ ] **Implement**: Pattern matching on variants — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/pattern.rs` — variant pattern matching
  - [ ] **Ori Tests**: `tests/spec/types/sum_types.ori` — variant pattern matching tests
  - [ ] **LLVM Support**: LLVM codegen for variant pattern matching
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` — variant pattern codegen

---

## 5.3 Newtypes

**Proposal**: `proposals/approved/newtype-pattern-proposal.md`

- [ ] **Implement**: Parse `type Name = ExistingType` — spec/06-types.md § Newtypes, spec/08-declarations.md § Type Declarations
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_newtype`
  - [ ] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [ ] **Implement**: Distinct type identity (nominal) — spec/07-properties-of-types.md § Type Equivalence
  - [ ] **Rust Tests**: `ori_typeck/src/registry/tests/type_registry_tests.rs` — nominal type identity
  - [ ] **Ori Tests**: `tests/spec/types/newtypes.ori`
  - [ ] **LLVM Support**: Transparent at runtime (same as underlying type)
  - [ ] **Note**: `TypeKind::Newtype` returns `Type::Named(name)`, not the underlying type

- [ ] **Implement**: Wrapping/unwrapping — spec/06-types.md § Newtypes
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — newtype unwrap method
  - [ ] **Ori Tests**: `tests/spec/types/newtypes.ori` — 9 tests
  - [ ] **LLVM Support**: Transparent at runtime (newtype constructor just stores underlying value)
  - [ ] **Implementation**: `UserId(value)` wraps, `user_id.unwrap()` unwraps

- [ ] **Implement**: Change `.unwrap()` to `.inner` accessor — spec/06-types.md § Newtypes
  - [ ] **Rust Tests**: Update `ori_eval/src/methods.rs` tests to use `.inner`
  - [ ] **Ori Tests**: Update `tests/spec/types/newtypes.ori` to use `.inner`
  - [ ] **LLVM Support**: Update LLVM codegen to use `.inner` field access
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/newtype_tests.rs` — `.inner` accessor codegen
  - [ ] **Note**: `.inner` is always public regardless of newtype visibility

---

## 5.4 Generic Types

- [ ] **Implement**: Parse `type Name<T> = ...` — spec/06-types.md § Generic Types, spec/08-declarations.md § Generic Declarations
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_generic_type_with_bounds`
  - [ ] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [ ] **Implement**: Multiple parameters `<T, U>` — spec/08-declarations.md § Generic Declarations
  - [ ] **Rust Tests**: Covered by parser tests
  - [ ] **Ori Tests**: `tests/spec/types/generic.ori` — `test_pair_int_str`, `test_pair_str_bool`

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

- [ ] **Implement**: Generic application / Instantiation — spec/06-types.md § Generic Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — `infer_struct` handles instantiation
  - [ ] **Ori Tests**: `tests/spec/types/generic.ori` — all 14 tests cover instantiation
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
> See Section 15 (Approved Syntax Proposals) § 15.1. Implement with new syntax directly.

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

## 5.9 Associated Functions

**Proposal**: `proposals/approved/associated-functions-language-feature.md`

Generalize associated functions to work for ANY type with an `impl` block, removing hardcoded type checks. Enables syntax like `Point.origin()`, `Builder.new()`, `Duration.from_seconds(s: 10)`.

### Migration

- [ ] **Implement**: Remove `is_type_name_for_associated_functions()` hardcoded checks
  - [ ] **Rust Tests**: `ori_typeck/src/infer/call.rs` — general type name resolution
  - [ ] **Ori Tests**: `tests/spec/types/associated_functions.ori`

### Parsing

- [ ] **Implement**: Parse `Type.method(...)` syntax in expression position
  - [ ] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — type-prefixed method call
  - [ ] **Ori Tests**: `tests/spec/types/associated_functions.ori`

- [ ] **Implement**: Distinguish type name vs value in resolution
  - [ ] **Rust Tests**: `oric/src/typeck/infer/postfix.rs` — type vs value resolution
  - [ ] **Ori Tests**: `tests/spec/types/associated_functions.ori`

### Associated Function Registry

- [ ] **Implement**: Track methods without `self` in impl blocks
  - [ ] **Rust Tests**: `oric/src/typeck/registry/methods.rs` — associated function registry
  - [ ] **Ori Tests**: `tests/spec/types/associated_functions.ori`

- [ ] **Implement**: Built-in associated functions for Duration
  - [ ] **Ori Tests**: `tests/spec/types/duration_factory.ori`
  - Duration.from_nanoseconds(ns:), from_microseconds(us:), from_milliseconds(ms:)
  - Duration.from_seconds(s:), from_minutes(m:), from_hours(h:)

- [ ] **Implement**: Built-in associated functions for Size
  - [ ] **Ori Tests**: `tests/spec/types/size_factory.ori`
  - Size.from_bytes(b:), from_kilobytes(kb:), from_megabytes(mb:)
  - Size.from_gigabytes(gb:), from_terabytes(tb:)

### Generic Types

- [ ] **Implement**: Full type arguments required for generic associated functions
  - [ ] **Ori Tests**: `tests/spec/types/associated_functions.ori`
  - Example: `Option<int>.some(value: 42)`

### Self Return Type

- [ ] **Implement**: Allow `Self` as return type in associated functions
  - [ ] **Ori Tests**: `tests/spec/types/associated_functions.ori`
  - Example: `impl Point { @origin () -> Self = Point { x: 0, y: 0 } }`

### Trait Associated Functions

- [ ] **Implement**: Traits can define associated functions without `self`
  - [ ] **Rust Tests**: `oric/src/typeck/registry/traits.rs` — trait associated functions
  - [ ] **Ori Tests**: `tests/spec/traits/associated_functions.ori`
  - Example: `trait Default { @default () -> Self }`

### LLVM Support

- [ ] **LLVM Support**: Codegen for associated function calls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/associated_function_tests.rs`

---

## 5.10 Section Completion Checklist

- [ ] All items above have all three checkboxes marked `[ ]`
- [ ] 80+% test coverage
- [ ] Run full test suite: `./test-all.sh`

**Exit Criteria**: User-defined structs and enums work
