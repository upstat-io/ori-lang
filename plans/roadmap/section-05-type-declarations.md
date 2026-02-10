---
section: 5
title: Type Declarations
status: in-progress
tier: 1
goal: User-defined types
spec:
  - spec/06-types.md
  - spec/07-properties-of-types.md
  - spec/08-declarations.md
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
    status: not-started
  - id: "5.6"
    title: Built-in Generic Types
    status: in-progress
  - id: "5.7"
    title: Derive Attributes
    status: in-progress
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

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`, `spec/08-declarations.md`

**Status**: In-progress — Structs (5.1), sum types (5.2), newtypes (5.3), generics (5.4), built-in generics (5.6), derive (5.7), visibility (5.8), associated functions (5.9) all working in evaluator. Compound type inference (5.5) entirely pending. LLVM tests missing. Verified 2026-02-10.

---

## 5.1 Struct Types

- [x] **Implement**: Parse `type Name = { field: Type, ... }` — spec/06-types.md § Struct Types, spec/08-declarations.md § Type Declarations ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_struct_type`
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` (30+ tests: basic, single field, empty, nested, many fields, mixed types, generic, with Option/List/Tuple/Function fields)

- [x] **Implement**: Register struct in type environment — spec/08-declarations.md § Type Declarations ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/type_registration.rs` — type registry tests
  - [x] **Ori Tests**: All struct tests verify type registration

- [x] **Implement**: Parse struct literals `Name { field: value }` — spec/06-types.md § Struct Types ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — struct literal parsing
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — all tests create struct literals

- [x] **Implement**: Type check struct literals — spec/06-types.md § Struct Types ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — struct literal type checking
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori`
  - [ ] **LLVM Support**: LLVM codegen for struct literal construction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/struct_tests.rs` — struct literal codegen (file does not exist)

- [x] **Implement**: Shorthand `Point { x, y }` — spec/06-types.md § Struct Types ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — shorthand parsing
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — `test_shorthand_init`, `test_mixed_shorthand`
  - [ ] **LLVM Support**: LLVM codegen for shorthand struct construction
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/struct_tests.rs` — shorthand struct codegen (file does not exist)

- [x] **Implement**: Field access — spec/06-types.md § Struct Types ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/infer/postfix.rs` — field access type inference
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — field chaining (`c.ceo.name`, deep nesting)
  - [ ] **LLVM Support**: LLVM codegen for struct field access
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/struct_tests.rs` — field access codegen (file does not exist)

- [x] **Implement**: Destructuring — spec/09-expressions.md § Destructuring ✅ (2026-02-10)
  - [x] **Rust Tests**: Parser in `ori_parse/src/grammar/expr/primary.rs` — `parse_binding_pattern()`
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — `test_destructure`, `test_destructure_partial`, `test_destructure_rename`
  - [ ] **LLVM Support**: LLVM codegen for struct destructuring
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/struct_tests.rs` — destructuring codegen (file does not exist)

---

## 5.2 Sum Types (Enums) — COMPLETED 2026-01-28

- [x] **Implement**: Parse `type Name = Variant1 | Variant2(Type)` ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_sum_type`
  - [x] **Ori Tests**: `tests/spec/declarations/sum_types.ori` (30+ tests)

- [x] **Implement**: Unit variants ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — included in `test_parse_sum_type`
  - [x] **Ori Tests**: Color (Red|Green|Blue), Direction (NSEW), Toggle (On|Off), Status (4 variants)

- [x] **Implement**: Single-field variants `Variant(Type)` ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — single-field variant parsing
  - [x] **Ori Tests**: MyOption (MySome/MyNone), Message (Text/Empty)
  - [ ] **LLVM Support**: LLVM codegen for single-field variants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sum_type_tests.rs` (file does not exist)

- [x] **Implement**: Multi-field variants `Variant(x: Type, y: Type)` ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — multi-field variant parsing
  - [x] **Ori Tests**: Shape (Circle/Rectangle), Point3D, Event (Click/KeyPress/Quit), Response (Success/Error)
  - [ ] **LLVM Support**: LLVM codegen for multi-field variants
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sum_type_tests.rs` (file does not exist)

- [x] **Implement**: Struct variants ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — struct variant parsing (named fields)
  - [x] **Ori Tests**: All multi-field variants use named fields

- [x] **Implement**: Variant constructors ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — variant constructor type checking
  - [x] **Ori Tests**: All sum type tests construct variants; generic sum types (MyResult, MyOptional, LinkedList, Tree)
  - [ ] **LLVM Support**: LLVM codegen for variant constructors
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/sum_type_tests.rs` (file does not exist)

- [x] **Implement**: Pattern matching on variants ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/infer/pattern.rs` — variant pattern matching
  - [x] **Ori Tests**: Exhaustive match, wildcard, nested match, variable binding, recursive Expr eval
  - [ ] **LLVM Support**: LLVM codegen for variant pattern matching
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/matching_tests.rs` (file does not exist)
  - Note: `#derive(Eq)` for sum types NOT working (skipped in tests)

---

## 5.3 Newtypes

**Proposal**: `proposals/approved/newtype-pattern-proposal.md`

- [x] **Implement**: Parse `type Name = ExistingType` ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_newtype`
  - [x] **Ori Tests**: `tests/spec/types/newtypes.ori` (UserId, Email, Age, Score)

- [x] **Implement**: Distinct type identity (nominal) ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_typeck/src/registry/tests/type_registry_tests.rs` — nominal type identity
  - [x] **Ori Tests**: `tests/spec/types/newtypes.ori` — separate UserId/Email types, validate_user/validate_email
  - [ ] **LLVM Support**: Transparent at runtime (same as underlying type)

- [x] **Implement**: Wrapping/unwrapping ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_eval/src/methods.rs` — newtype unwrap method
  - [x] **Ori Tests**: `tests/spec/types/newtypes.ori` — 9 tests (construction, unwrap, equality, params, computation)
  - [ ] **LLVM Support**: Transparent at runtime (newtype constructor just stores underlying value)

- [ ] **Implement**: Change `.unwrap()` to `.inner` accessor — spec/06-types.md § Newtypes
  - [ ] **Rust Tests**: Update `ori_eval/src/methods.rs` tests to use `.inner`
  - [ ] **Ori Tests**: Update `tests/spec/types/newtypes.ori` to use `.inner`
  - [ ] **LLVM Support**: Update LLVM codegen to use `.inner` field access
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/newtype_tests.rs` — `.inner` accessor codegen
  - [ ] **Note**: `.inner` is always public regardless of newtype visibility
  - Note: Currently using `.unwrap()` — migration to `.inner` pending

---

## 5.4 Generic Types

- [x] **Implement**: Parse `type Name<T> = ...` ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_generic_type_with_bounds`
  - [x] **Ori Tests**: `tests/spec/types/generic.ori` (Box<T>, Pair<A,B>, Container<T>, Wrapper<T>)

- [x] **Implement**: Multiple parameters `<T, U>` ✅ (2026-02-10)
  - [x] **Rust Tests**: Covered by parser tests
  - [x] **Ori Tests**: `tests/spec/types/generic.ori` — `test_pair_int_str`, `test_pair_str_bool`

- [ ] **Implement**: Constrained `<T: Trait>` — spec/08-declarations.md § Generic Declarations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/bound_checking.rs` — constrained generics
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — `GenericDerived<T: Eq>` works
  - [ ] **LLVM Support**: LLVM codegen for constrained generics
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/generic_tests.rs` (file does not exist)

- [ ] **Implement**: Multiple bounds `<T: A + B>` — spec/08-declarations.md § Generic Declarations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/bound_checking.rs` — multiple bounds
  - [ ] **Ori Tests**: Not tested yet
  - [ ] **LLVM Support**: LLVM codegen for multiple bounds
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/generic_tests.rs` (file does not exist)

- [x] **Implement**: Generic application / Instantiation ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — `infer_struct` handles instantiation
  - [x] **Ori Tests**: `tests/spec/types/generic.ori` — 14 tests (Box<int>, Box<str>, Pair<int,str>, nested, chained access, method calls on fields)
  - **Note**: `Type::Applied` tracks instantiated generic types.

- [ ] **Implement**: Constraint checking — spec/06-types.md § Generic Types
  - [ ] **Rust Tests**: `oric/src/typeck/checker/bound_checking.rs` — constraint checking
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — `GenericDerived<T: Eq>` constraint checked
  - [ ] **LLVM Support**: LLVM codegen for constraint checking
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/generic_tests.rs` (file does not exist)

---

## 5.5 Compound Types

Note: `tests/spec/types/collections.ori` is ENTIRELY COMMENTED OUT — type checker doesn't support collection type inference yet. Lists, tuples, maps work in the evaluator but type checker support is pending.

- [ ] **Implement**: List `[T]` — spec/06-types.md § List Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/collections.rs` — list type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori` — all commented out
  - [ ] **LLVM Support**: LLVM codegen for list type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` (file does not exist)
  - Note: Lists work in evaluator (used in struct_types.ori, sum_types.ori) but type inference commented out

- [ ] **Implement**: Map `{K: V}` — spec/06-types.md § Map Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/collections.rs` — map type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori` — all commented out
  - [ ] **LLVM Support**: LLVM codegen for map type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` (file does not exist)

- [ ] **Implement**: Set `Set<T>` — spec/06-types.md § Set Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/collections.rs` — set type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori` — all commented out
  - [ ] **LLVM Support**: LLVM codegen for set type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` (file does not exist)

- [ ] **Implement**: Tuple `(T, U)` — spec/06-types.md § Tuple Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — tuple type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori` — all commented out
  - [ ] **LLVM Support**: LLVM codegen for tuple type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/collection_tests.rs` (file does not exist)
  - Note: Tuples work in evaluator (used in struct_types.ori destructuring) but type inference commented out

- [ ] **Implement**: Range `Range<T>` — spec/06-types.md § Range Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — range type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`
  - [ ] **LLVM Support**: LLVM codegen for range type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs` (file does not exist)

- [ ] **Implement**: Function `(T) -> U` — spec/06-types.md § Function Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/lambda.rs` — function type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/lambdas.ori`
  - [ ] **LLVM Support**: LLVM codegen for function types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/function_tests.rs` (file does not exist)
  - Note: Function types work in evaluator (used in struct_types.ori `WithFunction`)

---

## 5.6 Built-in Generic Types

- [x] **Implement**: `Option<T>` with `Some`/`None` ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Option type handling
  - [x] **Ori Tests**: Used throughout test suite (struct_types.ori, sum_types.ori, traits/)
  - [ ] **LLVM Support**: LLVM codegen for Option type — inline IR in lower_calls.rs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_result_tests.rs` (file does not exist)

- [x] **Implement**: `Result<T, E>` with `Ok`/`Err` ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Result type handling
  - [x] **Ori Tests**: Used in traits/core/ tests, test suite
  - [ ] **LLVM Support**: LLVM codegen for Result type — inline IR in lower_calls.rs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/option_result_tests.rs` (file does not exist)

- [x] **Implement**: `Ordering` with `Less`/`Equal`/`Greater` ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Ordering type handling
  - [x] **Ori Tests**: `tests/spec/types/ordering/methods.ori` (32 tests)
  - [ ] **LLVM Support**: LLVM codegen for Ordering type — i8 comparison in lower_calls.rs
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/ordering_tests.rs` (file does not exist)

- [ ] **Implement**: `Error` type — spec/20-errors-and-panics.md § Error Conventions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Error type handling
  - [ ] **Ori Tests**: `tests/spec/types/error.ori` — not verified
  - [ ] **LLVM Support**: LLVM codegen for Error type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/error_tests.rs` (file does not exist)

- [ ] **Implement**: `Channel<T>` — spec/06-types.md § Channel
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Channel type handling
  - [ ] **Ori Tests**: `tests/spec/types/channel.ori` — not implemented
  - [ ] **LLVM Support**: LLVM codegen for Channel type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/channel_tests.rs` (file does not exist)

---

## 5.7 Derive Attributes

> **NOTE - Pending Syntax Change**: Approved proposal changes attribute syntax:
> - Current: `#[derive(Eq, Clone)]`
> - New: `#derive(Eq, Clone)`
> See Section 15 (Approved Syntax Proposals) § 15.1. Implement with new syntax directly.

- [x] **Implement**: Parse `#[derive(Trait1, Trait2)]` ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — derive attribute parsing
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` (15+ tests)
  - [ ] **LLVM Support**: LLVM codegen for derive attributes
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)
  - Note: Currently using `#[derive(...)]` syntax, not yet migrated to `#derive(...)` (pending 15A)

- [x] **Implement**: `#derive(Eq)` ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/derive/eq.rs` — derive Eq generation
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — EqPoint, EmptyDerived, SingleFieldDerived, nested derive, generic derive
  - [ ] **LLVM Support**: LLVM codegen for derived Eq
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)
  - Note: Derive(Eq) for SUM TYPES not working (skipped in tests)

- [x] **Implement**: `#derive(Clone)` ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/derive/clone.rs` — derive Clone generation
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — ClonePoint, SingleFieldDerived
  - [ ] **LLVM Support**: LLVM codegen for derived Clone
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)

- [x] **Implement**: `#derive(Hashable)` ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/derive/hashable.rs` — derive Hashable generation
  - [x] **Ori Tests**: `tests/spec/declarations/attributes.ori` — HashPoint, MultiAttrPoint
  - [ ] **LLVM Support**: LLVM codegen for derived Hashable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)

- [ ] **Implement**: `#derive(Printable)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/printable.rs` — derive Printable generation
  - [ ] **Ori Tests**: `tests/spec/declarations/attributes.ori` — skipped ("derive(Printable) not fully implemented")
  - [ ] **LLVM Support**: LLVM codegen for derived Printable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)
  - Note: Works in traits/derive/all_derives.ori but skipped in declarations/attributes.ori

- [ ] **Implement**: `#derive(Default)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/default.rs` — derive Default generation
  - [ ] **Ori Tests**: Not tested
  - [ ] **LLVM Support**: LLVM codegen for derived Default
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` (file does not exist)

---

## 5.8 Visibility

- [x] **Implement**: Parse `pub type Name = ...` ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/item.rs` — pub type parsing
  - [x] **Ori Tests**: `tests/spec/declarations/struct_types.ori` — `pub type PublicStruct`, `tests/spec/declarations/sum_types.ori` — `pub type PublicStatus`
  - [ ] **LLVM Support**: LLVM codegen for pub type visibility
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/visibility_tests.rs` (file does not exist)

- [x] **Implement**: Public visible from other modules ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/visibility.rs` — public visibility
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` — `pub type Point` imported cross-module
  - [ ] **LLVM Support**: LLVM codegen for public visibility
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/visibility_tests.rs` (file does not exist)

- [x] **Implement**: Private only in declaring module ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/module/visibility.rs` — private visibility
  - [x] **Ori Tests**: `tests/spec/modules/use_imports.ori` — `type InternalPoint` (private)
  - [ ] **LLVM Support**: LLVM codegen for private visibility
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/visibility_tests.rs` (file does not exist)

---

## 5.9 Associated Functions

**Proposal**: `proposals/approved/associated-functions-language-feature.md`

Generalize associated functions to work for ANY type with an `impl` block, removing hardcoded type checks. Enables syntax like `Point.origin()`, `Builder.new()`, `Duration.from_seconds(s: 10)`.

### Migration

- [x] **Implement**: Remove `is_type_name_for_associated_functions()` hardcoded checks ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_typeck/src/infer/call.rs` — general type name resolution
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — user types (Point, Builder, Counter, Rectangle, Pair) all work

### Parsing

- [x] **Implement**: Parse `Type.method(...)` syntax in expression position ✅ (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/postfix.rs` — type-prefixed method call
  - [x] **Ori Tests**: `Point.origin()`, `Builder.new()`, `Counter.zero()`, `Rectangle.square(size: 5)`

- [x] **Implement**: Distinguish type name vs value in resolution ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/infer/postfix.rs` — type vs value resolution
  - [x] **Ori Tests**: `test_instance_vs_associated` — `Pair.create()` (type) vs `p.sum()` (value)

### Associated Function Registry

- [x] **Implement**: Track methods without `self` in impl blocks ✅ (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/registry/methods.rs` — associated function registry
  - [x] **Ori Tests**: Point.origin(), Point.new(), Builder.new(), Counter.zero(), Counter.starting_at(), Rectangle.square/from_dimensions/unit(), Pair.create()

- [x] **Implement**: Built-in associated functions for Duration ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — `Duration.from_seconds(s: 5)` verified
  - Duration.from_nanoseconds(ns:), from_microseconds(us:), from_milliseconds(ms:)
  - Duration.from_seconds(s:), from_minutes(m:), from_hours(h:)

- [x] **Implement**: Built-in associated functions for Size ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — `Size.from_megabytes(mb: 2)` verified
  - Size.from_bytes(b:), from_kilobytes(kb:), from_megabytes(mb:)
  - Size.from_gigabytes(gb:), from_terabytes(tb:)

### Generic Types

- [ ] **Implement**: Full type arguments required for generic associated functions
  - [ ] **Ori Tests**: Not tested — `Option<int>.some(value: 42)` pattern not verified
  - Example: `Option<int>.some(value: 42)`

### Self Return Type

- [x] **Implement**: Allow `Self` as return type in associated functions ✅ (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/types/associated_functions.ori` — Point.origin() -> Self, Builder.new() -> Self, Counter.zero() -> Self, Counter.increment(self) -> Self

### Trait Associated Functions

- [ ] **Implement**: Traits can define associated functions without `self`
  - [ ] **Rust Tests**: `oric/src/typeck/registry/traits.rs` — trait associated functions
  - [ ] **Ori Tests**: Not tested — `trait Default { @default () -> Self }` pattern
  - Example: `trait Default { @default () -> Self }`

### LLVM Support

- [ ] **LLVM Support**: Codegen for associated function calls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/associated_function_tests.rs` (file does not exist)

---

## 5.10 Section Completion Checklist

- [x] Struct types (definition, literal, shorthand, field access, destructuring, spread, generic) ✅
- [x] Sum types (unit, single-field, multi-field, generic, recursive, pattern matching) ✅
- [x] Newtypes (construction, unwrap, nominal identity) ✅
- [x] Generic types (single/multiple params, instantiation, field access chain) ✅
- [x] Built-in generics: Option, Result, Ordering ✅
- [x] Derive: Eq, Clone, Hashable on structs ✅
- [x] Visibility: pub type, private by default ✅
- [x] Associated functions: Type.method() for user types, Duration, Size ✅
- [ ] Compound type inference (5.5) — entirely pending
- [ ] Derive: Printable, Default — not working
- [ ] Derive(Eq) for sum types — not working
- [ ] Newtype `.inner` accessor migration — pending
- [ ] Generic associated functions with type args — not tested
- [ ] Trait associated functions — not tested
- [ ] LLVM codegen for all type declarations — no dedicated test files
- [ ] Run full test suite: `./test-all.sh`

**Exit Criteria**: User-defined structs and enums work
**Status**: Evaluator support complete for core features. Type checker compound inference and several derive impls pending. Verified 2026-02-10.
