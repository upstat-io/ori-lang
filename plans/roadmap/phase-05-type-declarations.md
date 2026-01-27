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

- [ ] **Implement**: Destructuring — spec/09-expressions.md § Destructuring
  - [ ] **Rust Tests**: `oric/src/typeck/infer/pattern.rs` — struct destructuring
  - [ ] **Ori Tests**: `tests/spec/types/struct.ori`

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

- [x] **Implement**: Struct variants — spec/06-types.md § Sum Types
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — struct variant parsing (named fields)
  - [x] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [ ] **Implement**: Variant constructors — spec/06-types.md § Sum Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — variant constructor type checking
  - [ ] **Ori Tests**: `tests/spec/types/sum.ori`

- [ ] **Implement**: Pattern matching on variants — spec/10-patterns.md § Pattern Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/pattern.rs` — variant pattern matching
  - [ ] **Ori Tests**: `tests/spec/types/sum.ori`

---

## 5.3 Newtypes

- [x] **Implement**: Parse `type Name = ExistingType` — spec/06-types.md § Newtypes, spec/08-declarations.md § Type Declarations
  - [x] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — `test_parse_newtype`
  - [x] **Ori Tests**: N/A (parser tested via Rust unit tests)

- [ ] **Implement**: Distinct type identity (nominal) — spec/07-properties-of-types.md § Type Equivalence
  - [ ] **Rust Tests**: `oric/src/typeck/checker/type_registry.rs` — nominal type identity
  - [ ] **Ori Tests**: `tests/spec/types/newtype.ori`

- [ ] **Implement**: Wrapping/unwrapping — spec/06-types.md § Newtypes
  - [ ] **Rust Tests**: `oric/src/eval/exec/expr.rs` — newtype construction/extraction
  - [ ] **Ori Tests**: `tests/spec/types/newtype.ori`

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

- [ ] **Implement**: Multiple bounds `<T: A + B>` — spec/08-declarations.md § Generic Declarations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/bound_checking.rs` — multiple bounds
  - [ ] **Ori Tests**: `tests/spec/types/generic.ori`

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

---

## 5.5 Compound Types

- [ ] **Implement**: List `[T]` — spec/06-types.md § List Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/collections.rs` — list type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori`

- [ ] **Implement**: Map `{K: V}` — spec/06-types.md § Map Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/collections.rs` — map type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori`

- [ ] **Implement**: Set `Set<T>` — spec/06-types.md § Set Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/collections.rs` — set type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori`

- [ ] **Implement**: Tuple `(T, U)` — spec/06-types.md § Tuple Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — tuple type inference
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori`

- [ ] **Implement**: Range `Range<T>` — spec/06-types.md § Range Type
  - [ ] **Rust Tests**: `oric/src/typeck/infer/expr.rs` — range type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/loops.ori`

- [ ] **Implement**: Function `(T) -> U` — spec/06-types.md § Function Types
  - [ ] **Rust Tests**: `oric/src/typeck/infer/lambda.rs` — function type inference
  - [ ] **Ori Tests**: `tests/spec/expressions/lambdas.ori`

---

## 5.6 Built-in Generic Types

- [ ] **Implement**: `Option<T>` with `Some`/`None` — spec/06-types.md § Option
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Option type handling
  - [ ] **Ori Tests**: `tests/spec/types/option.ori`

- [ ] **Implement**: `Result<T, E>` with `Ok`/`Err` — spec/06-types.md § Result
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Result type handling
  - [ ] **Ori Tests**: `tests/spec/types/result.ori`

- [ ] **Implement**: `Ordering` with `Less`/`Equal`/`Greater` — spec/06-types.md § Ordering
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Ordering type handling
  - [ ] **Ori Tests**: `tests/spec/types/ordering.ori`

- [ ] **Implement**: `Error` type — spec/20-errors-and-panics.md § Error Conventions
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Error type handling
  - [ ] **Ori Tests**: `tests/spec/types/error.ori`

- [ ] **Implement**: `Channel<T>` — spec/06-types.md § Channel
  - [ ] **Rust Tests**: `oric/src/typeck/infer/builtins.rs` — Channel type handling
  - [ ] **Ori Tests**: `tests/spec/types/channel.ori`

---

## 5.7 Derive Attributes

> **NOTE - Pending Syntax Change**: Approved proposal changes attribute syntax:
> - Current: `#[derive(Eq, Clone)]`
> - New: `#derive(Eq, Clone)`
> See Phase 15 (Approved Syntax Proposals) § 15.1. Implement with new syntax directly.

- [ ] **Implement**: Parse `#derive(Trait1, Trait2)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `ori_parse/src/grammar/attr.rs` — derive attribute parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`

- [ ] **Implement**: `#derive(Eq)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/eq.rs` — derive Eq generation
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`

- [ ] **Implement**: `#derive(Clone)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/clone.rs` — derive Clone generation
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`

- [ ] **Implement**: `#derive(Hashable)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/hashable.rs` — derive Hashable generation
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`

- [ ] **Implement**: `#derive(Printable)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/printable.rs` — derive Printable generation
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`

- [ ] **Implement**: `#derive(Default)` — spec/08-declarations.md § Attributes
  - [ ] **Rust Tests**: `oric/src/typeck/derive/default.rs` — derive Default generation
  - [ ] **Ori Tests**: `tests/spec/declarations/derive.ori`

---

## 5.8 Visibility

- [ ] **Implement**: Parse `pub type Name = ...` — spec/08-declarations.md § Visibility
  - [ ] **Rust Tests**: `ori_parse/src/grammar/item.rs` — pub type parsing
  - [ ] **Ori Tests**: `tests/spec/declarations/visibility.ori`

- [ ] **Implement**: Public visible from other modules — spec/08-declarations.md § Visibility
  - [ ] **Rust Tests**: `oric/src/eval/module/visibility.rs` — public visibility
  - [ ] **Ori Tests**: `tests/spec/declarations/visibility.ori`

- [ ] **Implement**: Private only in declaring module — spec/08-declarations.md § Visibility
  - [ ] **Rust Tests**: `oric/src/eval/module/visibility.rs` — private visibility
  - [ ] **Ori Tests**: `tests/spec/declarations/visibility.ori`

---

## 5.9 Phase Completion Checklist

- [ ] All items above have all three checkboxes marked `[x]`
- [ ] 80+% test coverage
- [ ] Run full test suite: `cargo test && ori test tests/spec/`

**Exit Criteria**: User-defined structs and enums work
