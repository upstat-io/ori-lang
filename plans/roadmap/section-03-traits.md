---
section: 3
title: Traits and Implementations
status: in-progress
tier: 1
goal: Trait-based polymorphism
spec:
  - spec/07-properties-of-types.md
  - spec/08-declarations.md
sections:
  - id: "3.0"
    title: Core Library Traits
    status: in-progress
  - id: "3.1"
    title: Trait Declarations
    status: in-progress
  - id: "3.2"
    title: Trait Implementations
    status: in-progress
  - id: "3.3"
    title: Trait Bounds
    status: complete
  - id: "3.4"
    title: Associated Types
    status: complete
  - id: "3.5"
    title: Derive Traits
    status: complete
  - id: "3.6"
    title: Section Completion Checklist
    status: in-progress
  - id: "3.7"
    title: Clone Trait Formal Definition
    status: complete
  - id: "3.8"
    title: Iterator Traits
    status: in-progress
  - id: "3.8.1"
    title: Iterator Performance and Semantics
    status: not-started
  - id: "3.9"
    title: Debug Trait
    status: not-started
  - id: "3.10"
    title: Trait Resolution and Conflict Handling
    status: in-progress
  - id: "3.11"
    title: Object Safety Rules
    status: not-started
  - id: "3.12"
    title: Custom Subscripting (Index Trait)
    status: not-started
  - id: "3.13"
    title: Additional Core Traits
    status: not-started
  - id: "3.14"
    title: Comparable and Hashable Traits
    status: not-started
  - id: "3.15"
    title: Derived Traits Formal Semantics
    status: not-started
  - id: "3.16"
    title: Formattable Trait
    status: not-started
  - id: "3.17"
    title: Into Trait
    status: not-started
  - id: "3.18"
    title: Ordering Type
    status: in-progress
  - id: "3.19"
    title: Default Type Parameters on Traits
    status: complete  # verified 2026-02-15: all items checked
  - id: "3.20"
    title: Default Associated Types
    status: complete
  - id: "3.21"
    title: Operator Traits
    status: in-progress
---

# Section 3: Traits and Implementations

**Goal**: Trait-based polymorphism

> **SPEC**: `spec/07-properties-of-types.md`, `spec/08-declarations.md`

**Status**: In-progress — Core evaluator complete (3.0-3.6, 3.18-3.21), LLVM AOT tests 49 passing (39 traits + 10 derives, 0 ignored), proposals pending (3.7-3.17). Verified 2026-02-15: ~239 Ori tests + 49 AOT tests pass, 8931 total. Derive codegen complete (Eq, Clone, Hashable, Printable). Clone on compound types complete. Phase boundary hygiene review: speculative hash/equals on compound types reverted (eval/LLVM not implemented); byte/char clone added to LLVM.

---

## Implementation Location

> **Cross-Reference:** `plans/types_v2/section-08b-module-checker.md`

Trait support exists in **two type checker implementations**:

| System | Location | Status | Notes |
|--------|----------|--------|-------|
| **Current** (`ori_typeck`) | `compiler/ori_typeck/` | [done] Working | This section's items implemented here |
| **Types V2** (`ori_types`) | `compiler/ori_types/src/check/` | [todo] Stubbed | Migration target |

All items in this section (3.0-3.21) are implemented in `ori_typeck`. The **Types V2 migration**
(`plans/types_v2/`) will re-implement trait support using the new `Pool`/`Idx` infrastructure.

**Key Files (Current Implementation):**
- `ori_typeck/src/registry/trait_registry.rs` — Trait/impl storage
- `ori_typeck/src/checker/trait_registration.rs` — Registration passes
- `ori_typeck/src/checker/bound_checking.rs` — Constraint satisfaction
- `ori_typeck/src/infer/builtin_methods/` — Built-in trait methods

---

## PRIORITY NOTE

Per the "Lean Core, Rich Libraries" principle, most built-in functions have been moved
from the compiler core to trait methods. The compiler now only provides:

**Remaining built-ins:**
- `print(msg: str)` - I/O
- `panic(msg: str)` - Control flow

**Moved to traits (must implement in this section):**
- `len(collection)` -> `Len` trait with `.len()` method
- `is_empty(collection)` -> `IsEmpty` trait with `.is_empty()` method
- `is_some(option)`, `is_none(option)` -> `Option` methods
- `is_ok(result)`, `is_err(result)` -> `Result` methods
- `compare(left, right)` -> `Comparable` trait with `.compare()` method
- `min(a, b)`, `max(a, b)` -> `Comparable` trait or standalone functions via traits
- `assert(condition)` -> `Assert` trait or testing module
- `assert_eq(actual, expected)`, `assert_ne(actual, unexpected)` -> Testing traits

Without traits, tests cannot use assertions - this blocks the testing workflow.

---

## 3.0 Core Library Traits

**STATUS: COMPLETE**

Core library traits are implemented via:
1. **Runtime**: Evaluator's `MethodRegistry` provides hardcoded Rust dispatch for methods like `.len()`, `.is_empty()`, `.is_some()`, etc.
2. **Type checking**: `infer_builtin_method()` provides type inference for these methods
3. **Trait bounds**: `primitive_implements_trait()` in `bound_checking.rs` recognizes when types implement these traits

This approach follows the "Lean Core, Rich Libraries" principle — the runtime implementation stays in Rust for efficiency, while the type system recognizes the trait bounds for generic programming.

### 3.0.1 Len Trait

- [x] **Implemented**: Trait bound `Len` recognized for `[T]`, `str`, `{K: V}`, `Set<T>`, `Range<T>` [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — `test_len_bound_satisfied_by_*`
  - [x] **Ori Tests**: `tests/spec/traits/core/len.ori` — 14 tests (all pass)
- [x] **Implemented**: `.len()` method works on all collection types [done] (2026-02-10)
  - [x] **Tests**: `ori_eval/src/methods.rs` — list/string/range method tests
  - [x] **LLVM Support**: LLVM codegen for `.len()` — inline IR via field extraction in `lower_calls.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — `.len()` on lists (3 tests) and strings (2 tests) [done] (2026-02-13)

### 3.0.2 IsEmpty Trait

- [x] **Implemented**: Trait bound `IsEmpty` recognized for `[T]`, `str`, `{K: V}`, `Set<T>` [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — `test_is_empty_bound_satisfied_by_*`
  - [x] **Ori Tests**: `tests/spec/traits/core/is_empty.ori` — 13 tests (all pass)
- [x] **Implemented**: `.is_empty()` method works on all collection types [done] (2026-02-10)
  - [x] **Tests**: `ori_eval/src/methods.rs` — list/string method tests
  - [x] **LLVM Support**: LLVM codegen for `.is_empty()` — inline IR in `lower_calls.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — `.is_empty()` on lists (2 tests) and strings (2 tests) [done] (2026-02-13)

### 3.0.3 Option Methods

- [x] **Implemented**: `.is_some()`, `.is_none()`, `.unwrap()`, `.unwrap_or(default:)` methods [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_eval/src/methods.rs` — `option_methods` module
  - [x] **Ori Tests**: `tests/spec/traits/core/option.ori` — 16 tests (all pass)
  - [x] **LLVM Support**: LLVM codegen for Option — tag-based dispatch in `lower_calls.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — `.is_some()` (2), `.is_none()` (2), `.unwrap()` (1), `.unwrap_or()` (2) all pass [done] (2026-02-13)
- [x] **Type checking**: `infer_builtin_method()` handles Option methods [done] (2026-02-10)

### 3.0.4 Result Methods

- [x] **Implemented**: `.is_ok()`, `.is_err()`, `.unwrap()` methods [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_eval/src/methods.rs` — `result_methods` module
  - [x] **Ori Tests**: `tests/spec/traits/core/result.ori` — 14 tests (all pass)
  - [x] **LLVM Support**: LLVM codegen for Result — tag-based dispatch in `lower_calls.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — `.is_ok()` (2), `.is_err()` (2), `.unwrap()` (1) all pass [done] (2026-02-13)
- [x] **Type checking**: `infer_builtin_method()` handles Result methods [done] (2026-02-10)

### 3.0.5 Comparable Trait

- [x] **Implemented**: Trait bound `Comparable` recognized for `int`, `float`, `bool`, `str`, `char`, `byte`, `Duration`, `Size`, `[T]`, `Option<T>`, `Result<T, E>`, `Ordering` [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — `test_comparable_bound_satisfied_by_*`
  - [x] **Ori Tests**: `tests/spec/traits/core/comparable.ori` — 58 tests (all pass)
- [x] **Type checking**: All comparable types have `.compare(other:)` returning `Ordering` [done] (2026-02-10)
  - [x] **Type Checking**: `ori_typeck/src/infer/builtin_methods/` — numeric.rs, string.rs, list.rs, option.rs, result.rs, units.rs, ordering.rs
  - [x] **LLVM Support**: LLVM codegen for `.compare()` — inline arithmetic/comparison in `lower_calls.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — 7 tests passing: `.compare()` + Ordering methods (is_less, is_equal, is_greater, reverse, is_less_or_equal, is_greater_or_equal) [done] (2026-02-13)

### 3.0.6 Eq Trait

- [x] **Implemented**: Trait bound `Eq` recognized for all primitive types [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — `test_eq_bound_satisfied_by_*`
  - [x] **Ori Tests**: `tests/spec/traits/core/eq.ori` — 23 tests (all pass)
  - [x] **LLVM Support**: LLVM codegen for `==`/`!=` on all primitives [done]
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — `==`/`!=` for int, bool, str (3 tests) [done] (2026-02-13)

### Additional Traits

The following traits are also recognized in trait bounds:
- **Clone**: All primitives, collections
- **Hashable**: `int`, `bool`, `str`, `char`, `byte`
- **Default**: `int`, `float`, `bool`, `str`, `Unit`, `Option<T>`
- **Printable**: All primitives

---

## 3.1 Trait Declarations

- [x] **Implement**: Parse `trait Name { ... }` — spec/08-declarations.md § Trait Declarations [done] (2026-02-10)
  - [x] **Write test**: `tests/spec/traits/declaration.ori` — 16 tests (all pass)
  - [x] **Run test**: `ori test tests/spec/traits/declaration.ori`

- [x] **Implement**: Required method signatures — spec/08-declarations.md § Trait Declarations [done] (2026-02-10)
  - [x] **Write test**: `tests/spec/traits/declaration.ori` — Greeter, Counter, Calculator traits
  - [x] **Run test**: All pass

- [x] **Implement**: Default method implementations — spec/08-declarations.md § Trait Declarations [done] (2026-02-10)
  - [x] **Write test**: `tests/spec/traits/declaration.ori` (test_default_method: summarize(), is_large())
  - [x] **Run test**: All pass
  - **Note**: Added default trait method dispatch in `module_loading.rs:collect_impl_methods()`
  - [x] **LLVM Support**: LLVM codegen for default trait method dispatch [done] (2026-02-13)
    - Fixed at 3 levels: method registration (register_impl), body type checking (check_impl_block), LLVM codegen (compile_impls)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — `test_aot_trait_default_method` passing [done] (2026-02-13)

- [x] **Implement**: Associated types — spec/08-declarations.md § Associated Types [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — associated type parsing
  - [x] **Ori Tests**: `tests/spec/traits/associated_types.ori` — 2 tests + 1 compile_fail (all pass)

- [x] **Implement**: `self` parameter — spec/08-declarations.md § self Parameter [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — self parameter handling
  - [x] **Ori Tests**: `tests/spec/traits/self_param.ori` — 9 tests (all pass)

- [x] **Implement**: `Self` type reference — spec/08-declarations.md § Self Type [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — Self type resolution
  - [x] **Ori Tests**: `tests/spec/traits/self_type.ori` — 7 tests (all pass)

- [x] **Implement**: Trait inheritance `trait Child: Parent` — spec/08-declarations.md § Trait Inheritance [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — trait inheritance
  - [x] **Ori Tests**: `tests/spec/traits/inheritance.ori` — 6 tests including 3-level deep inheritance (all pass)

- [x] **BUG**: Static methods `Type.method()` not supported — commented out in declaration.ori (Point.new(), Point.origin()) [done] (2026-02-13)
  - Infrastructure was already working (TypeRef dispatch in method_dispatch.rs). Test file was missing `@new`/`@origin` impl methods + had stale TODO comments. Added methods, uncommented tests, 2 new tests pass.

---

## 3.2 Trait Implementations

- [x] **Implement**: Inherent impl `impl Type { ... }` — spec/08-declarations.md § Inherent Implementations [done] (2026-02-10)
  - [x] **Write test**: `tests/spec/traits/declaration.ori` (Widget.get_name(), Widget.get_value(), Point.distance_from_origin())
  - [x] **Run test**: All pass
  - [x] **LLVM Support**: LLVM codegen — type-qualified method dispatch (`_ori_Type$method` mangling)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — inherent impl (3 tests: method, params, field access), impl_method_field_access (1 test) [done] (2026-02-13)

- [x] **Implement**: Trait impl `impl Trait for Type { ... }` — spec/08-declarations.md § Trait Implementations [done] (2026-02-10)
  - [x] **Write test**: `tests/spec/traits/declaration.ori` (Widget.greet(), Widget.describe(), Widget.summarize())
  - [x] **Run test**: All pass
  - [x] **LLVM Support**: LLVM codegen — trait method dispatch (`_ori_Type$$Trait$method` mangling)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — trait impl (2 tests: single method, multiple methods) [done] (2026-02-13)

- [x] **Implement**: Generic impl `impl<T: Bound> Trait for Container<T>` — spec/08-declarations.md § Generic Implementations [done] (2026-02-10)
  - [x] **Rust Tests**: Parser tests in `ori_parse/src/grammar/item.rs`
  - [x] **Ori Tests**: `tests/spec/traits/generic_impl.ori` — 4 tests (inherent + trait impls on generic types, all pass)
  - **Note**: Added `parse_impl_type()` to handle `Box<T>` syntax in impl blocks. Also added
    `Type::Applied` for tracking instantiated generic types with their type arguments.
  - [ ] **LLVM Support**: LLVM codegen for generic impl method dispatch — not explicitly tested (no monomorphization)
  - [ ] **LLVM Rust Tests**: Skipped — generic functions are skipped in AOT codegen (no monomorphization pipeline)

- [x] **Implement**: Where clauses — spec/08-declarations.md § Where Clauses [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — where clause parsing
  - [x] **Ori Tests**: `tests/spec/traits/associated_types.ori` — `where C.Item: Eq` verified

- [x] **Implement**: Method resolution in type checker — spec/08-declarations.md § Method Resolution [done] (2026-02-10)
  - `TraitRegistry.lookup_method()` checks inherent impls, then trait impls, then default methods
  - `infer_method_call()` uses trait registry, falls back to built-in methods
  - [x] **Rust Tests**: Covered by existing tests in `typeck/infer/call.rs`
  - [x] **Ori Tests**: `tests/spec/traits/declaration.ori`, `tests/spec/traits/generic_impl.ori`, `tests/spec/traits/method_call_test.ori`
  - [x] **LLVM Support**: 4-tier dispatch: built-in → type-qualified → bare-name → LLVM module lookup
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — method resolution (1 test: inherent takes priority over trait impl) [done] (2026-02-13)

- [x] **Implement**: User-defined impl method dispatch in evaluator [done] (2026-02-10)
  - Created `UserMethodRegistry` to store impl method definitions
  - Methods registered via `load_module` -> `register_impl_methods`
  - `eval_method_call` checks user methods first, falls back to built-in
  - Added `self_path` to `ImplDef` AST for type name resolution
  - [x] **Write test**: Rust unit tests in `eval/evaluator.rs` (4 tests covering dispatch, self access, args, fallback)
  - [x] **Run test**: All pass
  - [x] **LLVM Support**: LLVM codegen for user-defined impl method dispatch — `compile_impls()` in `function_compiler.rs`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — user method dispatch covered by inherent impl and trait impl tests [done] (2026-02-13)

- [x] **Implement**: Coherence checking — spec/08-declarations.md § Coherence [done] (2026-02-10)
  - `register_impl` returns `Result<(), CoherenceError>` and checks for conflicts
  - Duplicate trait impls for same type rejected
  - Duplicate inherent methods on same type rejected
  - Multiple inherent impl blocks allowed if methods don't conflict (merged)
  - Added `E2010` error code for coherence violations
  - [x] **Write test**: Rust unit tests in `typeck/type_registry.rs` (3 tests)
  - [x] **Run test**: All pass

---

## 3.3 Trait Bounds

**Complete Implementation:** [done] (verified 2026-02-14)
- [x] Parser supports generic parameters with bounds `<T: Trait>`, `<T: A + B>` — `parse_generics()` + `parse_bounds()` in `ori_parse/src/grammar/item/generics/mod.rs`
- [x] Parser supports where clauses `where T: Clone, U: Default` — `parse_where_clauses()` in `ori_parse/src/grammar/item/generics/mod.rs`
- [x] `Function` AST node stores `generics: GenericParamRange` and `where_clauses: Vec<WhereClause>` — `ori_ir/src/ast/items/function.rs`
- [x] `FunctionSig` in type checker stores `type_param_bounds: Vec<Vec<Name>>` with bounds — `ori_types/src/output/mod.rs`
- [x] `Param` AST stores type annotation as `ty: Option<ParsedType>` — `ori_ir/src/ast/items/function.rs`
- [x] Type parsing captures identifier names implicitly in `ParsedType` nodes — `ori_parse/src/grammar/ty/mod.rs`
- [x] `infer_function_signature_with_arena()` creates fresh type vars for generics and maps params — `ori_types/src/check/signatures/mod.rs`
- [x] `signatures: FxHashMap<Name, FunctionSig>` stores signatures for call-time lookup — `ori_types/src/check/mod.rs`
- [x] Bound checking at call sites via inline checks + `type_satisfies_trait()` — `ori_types/src/infer/expr/calls.rs`
- [x] E2009 error code for missing trait bound violations — `ori_diagnostic/src/error_code/mod.rs`
- [x] Unit tests verify end-to-end — `ori_parse/src/grammar/item/generics/tests.rs` (5 where-clause tests), `ori_parse/src/grammar/ty/tests.rs` (trait bound tests), `ori_types/src/infer/expr/tests.rs`

**What Works Now:**
- Parsing generic functions: `@compare<T: Comparable> (a: T, b: T) -> Ordering`
- Parsing multiple bounds: `@process<T: Eq + Clone> (x: T) -> T`
- Parsing where clauses: `@transform<T> (x: T) -> T where T: Clone = x`
- Constraint satisfaction checking at call sites
- Error messages when types don't satisfy required bounds

**Implementation Details:**
- `Param.ty` stores type annotations as `ParsedType`; generic parameter names resolved via `FunctionSig.type_params`
- `FunctionSig.type_param_bounds` stores bounds per generic parameter as `Vec<Vec<Name>>`
- `infer_function_signature_with_arena()` creates fresh type vars and builds `generic_param_mapping`
- When a param's type matches a generic, the type var is used instead of inferring
- Bound checking in `calls.rs` resolves type vars after unification and verifies trait impls
- `type_satisfies_trait()` uses trait registry to verify implementations

- [x] **Implement**: Single bound `<T: Trait>` — spec/08-declarations.md § Generic Declarations [done] (2026-02-10)
  - [x] **Write test**: Rust unit tests in `typeck/checker.rs::tests` (10 tests pass)
  - [x] **Run test**: All pass

- [x] **Implement**: Multiple bounds `<T: A + B>` — spec/08-declarations.md § Generic Declarations [done] (2026-02-10)
  - [x] **Write test**: `test_multiple_bounds_parsing` in Rust unit tests
  - [x] **Run test**: All pass

- [x] **Implement**: Constraint satisfaction checking — spec/07-properties-of-types.md § Trait Bounds [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 10+ constraint satisfaction tests
  - [x] **Ori Tests**: `tests/spec/traits/associated_types.ori` — `needs_eq_item` + `compile_fail` for violated bound

---

## 3.4 Associated Types

**STATUS: COMPLETE**

Infrastructure implemented:
- `ParsedType::AssociatedType` variant in `ori_ir/src/parsed_type.rs`
- `Type::Projection` variant in `ori_types/src/lib.rs`
- Parser handles `Self.Item` and `T.Item` syntax in type positions
- `ImplAssocType` for associated type definitions in impl blocks
- `ImplEntry.assoc_types` stores associated type definitions
- `TraitRegistry.lookup_assoc_type()` resolves associated types

- [x] **Implement**: Associated type declarations — spec/08-declarations.md § Associated Types [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/ty.rs` — associated type parsing tests
  - [x] **Ori Tests**: `tests/spec/traits/associated_types.ori` — 2 tests (all pass)
  - [x] **Ori Tests**: `tests/spec/traits/associated_types_verify.ori` — 2 tests (all pass)

- [x] **Implement**: Constraints `where T.Item: Eq` — spec/08-declarations.md § Where Clauses [done] (2026-02-10)
  - [x] **Rust Tests**: Parser/type checker support in `bound_checking.rs`
  - [x] **Ori Tests**: `tests/spec/traits/associated_types.ori` — `test_fnbox_fails_eq_constraint` compile_fail passes
  - **Note**: Added `WhereConstraint` struct with projection support. Parser handles `where C.Item: Eq`.
    Bound checking resolves associated types via `lookup_assoc_type_by_name()`.

- [x] **Implement**: Impl validation (require all associated types defined) [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/trait_registration.rs` — `validate_associated_types`
  - [x] **Ori Tests**: `tests/compile-fail/impl_missing_assoc_type.ori` — test exists and passes [done] (verified 2026-02-14)
  - **Note**: Added validation in `register_impls()` that checks all required associated types are defined.

---

## 3.5 Derive Traits

**STATUS: COMPLETE**

All 5 derive traits implemented in `oric/src/typeck/derives/mod.rs`.
Tests at `tests/spec/traits/derive/all_derives.ori` (7 tests pass).

- [x] **Implement**: Auto-implement `Eq` — spec/08-declarations.md § Attributes [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — `test_process_struct_derives`
  - [x] **Ori Tests**: `tests/spec/traits/derive/all_derives.ori` + `tests/spec/traits/derive/eq.ori` — 3+13 tests (all pass)
  - [x] **LLVM Support**: Synthetic LLVM IR for derived Eq — field-by-field `icmp eq` with short-circuit AND [done] (2026-02-13)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/derives.rs` — 4 AOT tests (basic, strings, mixed types, single field) [done] (2026-02-13)

- [x] **Implement**: Auto-implement `Clone` — spec/08-declarations.md § Attributes [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — `test_process_multiple_derives`
  - [x] **Ori Tests**: `tests/spec/traits/derive/all_derives.ori` — `.clone()` on derived Point (passes)
  - [x] **LLVM Support**: Synthetic LLVM IR for derived Clone — identity return for value types [done] (2026-02-13)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/derives.rs` — 2 AOT tests (basic, large struct sret) [done] (2026-02-13)

- [x] **Implement**: Auto-implement `Hashable` — spec/08-declarations.md § Attributes [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/derives/mod.rs`
  - [x] **Ori Tests**: `tests/spec/traits/derive/all_derives.ori` — `.hash()` on derived Point (passes)
  - [x] **LLVM Support**: Synthetic LLVM IR for derived Hashable — FNV-1a in pure LLVM IR [done] (2026-02-13)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/derives.rs` — 2 AOT tests (equal values, different values) [done] (2026-02-13)

- [x] **Implement**: Auto-implement `Printable` — spec/08-declarations.md § Attributes [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/derives/mod.rs`
  - [x] **Ori Tests**: `tests/spec/traits/derive/all_derives.ori` — `.to_string()` on derived Point (passes)
  - [x] **LLVM Support**: Synthetic LLVM IR for derived Printable — runtime str concat via `ori_str_*` [done] (2026-02-13)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/derives.rs` — 1 AOT test (basic non-empty check) [done] (2026-02-13)

- [x] **Implement**: Auto-implement `Default` — spec/08-declarations.md § Attributes [done] (2026-02-14)
  - [x] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — `create_derived_method_def` handles Default
  - [x] **Ori Tests**: `tests/spec/traits/derive/default.ori` — 6 tests (basic, multi-type, single field, float, eq integration, nested) [done] (2026-02-14)
  - [x] **LLVM Support**: LLVM codegen for derived Default — `const_zero` produces correct zero-init structs [done] (2026-02-14)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/derives.rs` — 3 AOT tests (basic, mixed types, eq integration) [done] (2026-02-15)
    - **Fixed**: Static method dispatch bug in LLVM codegen — `TypeRef` receivers now handled as static calls (no self param)

---

## 3.6 Section Completion Checklist

- [x] Core library traits (3.0): Len, IsEmpty, Option, Result, Comparable, Eq — all complete [done] (2026-02-10)
  - [x] **Gap**: Clone/Hashable/Default/Printable methods NOT callable on primitives — FIXED: V2 type checker resolvers return correct types for clone/hash/equals on primitives [done] (2026-02-15). Clone also works on compound types (collections, wrappers, tuples). hash/equals on compound types reverted (phase boundary leak — evaluator/LLVM not implemented); tracked under 3.14.
- [x] Trait declarations (3.1): Parse, required methods, default methods, self, Self, inheritance — all complete [done] (2026-02-10)
  - [x] **Gap**: Static methods `Type.method()` — FIXED, was stale TODO [done] (2026-02-13)
- [x] Trait implementations (3.2): Inherent, trait, generic impls, method resolution, coherence — all complete [done] (2026-02-10)
- [x] Trait bounds (3.3): Single, multiple, constraint satisfaction — all complete [done] (2026-02-10)
- [x] Associated types (3.4): Declaration, `Self.Item`, where constraints — all complete [done] (2026-02-10)
- [x] Derive traits (3.5): Eq, Clone, Hashable, Printable complete; Default NOT tested [done] (2026-02-10)
- [x] ~239 trait test annotations pass (len: 14, is_empty: 13, option: 16, result: 14, comparable: 58, eq: 23, declaration: 16, self_param: 9, self_type: 7, inheritance: 6, generic_impl: 4, associated_types: 4, default_type_params: 2, default_assoc_types: 4, derive: 16, ordering: 32, method_call: 1) [done] (2026-02-10)
- [x] Run full test suite: `./test-all.sh` — 3,068 passed, 0 failed [done] (2026-02-10)
- [x] LLVM AOT tests: `ori_llvm/tests/aot/traits.rs` — 39 passing, 0 ignored [done] (2026-02-13)
  - [x] **Fixed**: `.compare()` return type resolved as Ordering — added to V2 type checker [done] (2026-02-13)
  - [x] **Fixed**: `.unwrap_or()` added to LLVM Option dispatch table [done] (2026-02-13)
  - [x] **Fixed**: Default trait methods compiled in LLVM [done] (2026-02-13)
  - [x] **Fixed**: Indirect ABI parameter passing — self loaded from pointer for >16B structs [done] (2026-02-13)
  - [x] **Fixed**: Derive methods wired into LLVM codegen — synthetic IR functions for Eq, Clone, Hashable, Printable [done] (2026-02-13)
- [x] Operator traits (3.21): User-defined operator dispatch complete — type checker desugaring, evaluator dispatch, LLVM codegen, error messages [done] (2026-02-15)
  - [ ] Remaining: derive support for newtypes (optional), spec update, CLAUDE.md update
- [ ] Proposals (3.8-3.17): Iterator Phase 1+2 complete (core next(), consumers, lazy adapters) [in-progress] (2026-02-15). Debug, Formattable, Into, etc. — not started (3.7 Clone complete [done])

**Exit Criteria**: Core trait-based code compiles and runs in evaluator [done]. LLVM codegen for built-in and user methods works [done]. User-defined operator traits complete [done] (2026-02-15). Formal trait proposals (3.8-3.17) pending.

---

## 3.7 Clone Trait Formal Definition

**Proposal**: `proposals/approved/clone-trait-proposal.md`

Formalizes the `Clone` trait that enables explicit value duplication. The trait is already recognized in trait bounds and derivable, but this proposal adds the formal definition and comprehensive prelude implementations.

### Implementation

- [x] **Implement**: Formal `Clone` trait definition in type system
  - [x] **Ori Tests**: `tests/spec/traits/clone/definition.ori` — derived Clone on structs (6 tests)
  - [x] **LLVM Support**: LLVM codegen for Clone trait (identity for value types, derive for structs)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/derives.rs` — Clone definition codegen (derive_clone_basic, derive_clone_large_struct)
  - Note: Type checker V2 `resolve_*_method` returns correct types for `clone` on all primitives and compound types. Static method dispatch fix enabled `Type.default()` calls in LLVM codegen. `hash`/`equals` resolved for primitives only (compound types deferred to 3.14 — evaluator/LLVM codegen not yet implemented).

- [x] **Implement**: Clone implementations for all primitives (int, float, bool, str, char, byte, Duration, Size)
  - [x] **Ori Tests**: `tests/spec/traits/clone/primitives.ori` — all 8 primitive types (13 tests)
  - [x] **LLVM Support**: LLVM codegen for primitive clone methods (identity operation)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/derives.rs` — primitive clone codegen (clone_int, clone_float, clone_bool, clone_str)

- [x] **Implement**: Clone implementations for collections ([T], {K: V}, Set<T>) with element-wise cloning [done] (2026-02-15)
  - [x] **Rust Tests**: `ori_types/src/infer/expr/tests.rs` — `test_clone_satisfied_by_list`, `test_clone_satisfied_by_map`, `test_clone_satisfied_by_set`
  - [x] **Ori Tests**: `tests/spec/traits/clone/collections.ori` — list clone (3 tests)
  - [x] **LLVM Support**: LLVM codegen for collection clone — identity (ARC shares data) in `lower_list_method()`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/derives.rs` — `test_aot_clone_list_int`, `test_aot_clone_list_empty`

- [x] **Implement**: Clone implementations for Option<T> and Result<T, E> [done] (2026-02-15)
  - [x] **Rust Tests**: `ori_types/src/infer/expr/tests.rs` — `test_clone_satisfied_by_option`, `test_clone_satisfied_by_result`
  - [x] **Ori Tests**: `tests/spec/traits/clone/wrappers.ori` — Option Some/None, Result Ok/Err (4 tests)
  - [x] **LLVM Support**: LLVM codegen for Option/Result clone — identity (value types) in `lower_option_method()`, `lower_result_method()`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/derives.rs` — `test_aot_clone_option_some`, `test_aot_clone_option_none`, `test_aot_clone_result_ok`, `test_aot_clone_result_err`

- [x] **Implement**: Clone implementations for tuples (all arities) [done] (2026-02-15)
  - [x] **Rust Tests**: `ori_types/src/infer/expr/tests.rs` — `test_clone_satisfied_by_tuple`, `test_clone_satisfied_by_tuple_triple`
  - [x] **Ori Tests**: `tests/spec/traits/clone/tuples.ori` — pair and triple clone (2 tests)
  - [x] **LLVM Support**: LLVM codegen for tuple clone — identity (value type) via `TypeInfo::Tuple` match in `lower_builtin_method()`
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/derives.rs` — `test_aot_clone_tuple_pair`, `test_aot_clone_tuple_triple`

- [x] **Update Spec**: `06-types.md` — add Clone trait section (already present at § Clone Trait, lines 924–970+)
- [x] **Update Spec**: `12-modules.md` — update prelude traits description (Clone listed in prelude traits table, line 269/279)
- [x] **Update**: `CLAUDE.md` — Clone is documented in spec; CLAUDE.md is a compiler dev guide, not language reference

- [x] **Hygiene review** (2026-02-15): Phase boundary audit of commit 01051607
  - [x] **Fixed LEAK**: LLVM codegen missing `byte.clone()` and `char.clone()` — added `Idx::BYTE | Idx::CHAR` identity arms in `lower_builtin_method()` (`lower_calls.rs`)
  - [x] **Fixed LEAK**: Type checker speculatively accepted `hash`/`equals` on collections (list, map, set), wrappers (Option, Result), and tuples — **reverted**. Evaluator and LLVM codegen have no handlers for these methods. Type checker now only accepts `clone` on compound types. `hash`/`equals` on compound types tracked under 3.14.
  - [x] **Deferred WASTE**: `abi.clone()` in `lower_calls.rs` (4 sites) — pre-existing borrow-conflict workaround, cheap clone. Not worth refactoring risk now.
  - [x] **Deferred WASTE**: `method_str` String allocation per method call — pre-existing, requires `Name`-based API change across many call sites.
  - [x] **Hygiene review pass 2** (2026-02-15): Phase boundary audit of commit da22ae17
    - [x] **Fixed LEAK**: `type_satisfies_trait()` claimed compound types satisfy `Eq` (`COLLECTION_TRAITS`, `WRAPPER_TRAITS`, `RESULT_TRAITS` all contained `"Eq"`) — but no `.equals()` method exists in any downstream phase. Removed `"Eq"` from all 3 arrays. Re-add under 3.14 when `equals()` is implemented.
    - [x] **Fixed LEAK**: `resolve_tuple_method()` accepted `to_list` — dubious semantics (only works if all elements same type), no evaluator/LLVM handler. Removed; simplified function signature (dropped unused `engine` param).
    - [x] **Fixed LEAK**: `dispatch_map_method()` in evaluator had no `clone` handler — fell through to `no_such_method("clone", "map")`. Added clone handler (Arc identity, same as list).
    - [x] **Fixed LEAK**: `dispatch_tuple_method()` in evaluator had no `len` handler — fell through to `no_such_method("len", "tuple")`. Added len handler extracting `Value::Tuple(elems).len()`.
    - [x] **Fixed LEAK**: `lower_builtin_method()` in LLVM codegen had no Map/Set clone handling — fell through to `None` → "unresolved method call". Added `Map | Set` identity pattern (Arc-managed structs).
    - [x] **Fixed LEAK**: `lower_builtin_method()` in LLVM codegen had no Tuple.len() handling. Added compile-time constant from `TypeInfo::Tuple { elements }` count.
    - [x] **Added Tests**: 6 unit tests verifying compound types do NOT satisfy Eq (`test_eq_not_satisfied_by_{list,map,set,option,result,tuple}`).

---

## 3.8 Iterator Traits

**Proposal**: `proposals/approved/iterator-traits-proposal.md`

Formalizes iteration with four core traits: `Iterator`, `DoubleEndedIterator`, `Iterable`, and `Collect`. Enables generic programming over any iterable, user types participating in `for` loops, and transformation methods.

### Implementation

- [x] **Implement**: `Iterator` trait with functional `next()` returning `(Option<Self.Item>, Self)` (2026-02-15)
  - [x] **Rust Tests**: `ori_patterns/src/value/iterator/tests.rs` — 13 unit tests for IteratorValue (2026-02-15)
  - [x] **Ori Tests**: `tests/spec/traits/iterator/iterator.ori` — 9 spec tests (2026-02-15)
  - [ ] **LLVM Support**: LLVM codegen for iterator trait methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: `DoubleEndedIterator` trait with `next_back()` method
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — double-ended iterator bounds
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/double_ended.ori`
  - [ ] **LLVM Support**: LLVM codegen for double-ended iterator methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [x] **Implement**: `Iterable` trait with `iter()` method — built-in dispatch for list, map, range, str (2026-02-15)
  - [x] **Rust Tests**: Consistency tests verify eval/typeck method sync (2026-02-15)
  - [x] **Ori Tests**: `tests/spec/traits/iterator/iterator.ori` — covers .iter() on all types (2026-02-15)
  - [ ] **LLVM Support**: LLVM codegen for iterable trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: `Collect` trait with `from_iter()` method
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — collect trait bounds
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/collect.ori`
  - [ ] **LLVM Support**: LLVM codegen for collect trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [x] **Implement**: Iterator Phase 2 methods — consumers (fold, count, find, any, all, for_each, collect) and lazy adapters (map, filter, take, skip) (2026-02-15)
  - [x] **Rust Tests**: `ori_patterns/src/value/iterator/tests.rs` — 10 adapter variant unit tests (2026-02-15)
  - [x] **Ori Tests**: `tests/spec/traits/iterator/methods.ori` — 31 spec test assertions (2026-02-15)
  - [ ] **LLVM Support**: LLVM codegen for all iterator methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`
  - [ ] **Remaining Phase 2C/2D**: enumerate, zip, chain, flatten, flat_map, cycle, DoubleEndedIterator

- [ ] **Implement**: DoubleEndedIterator default methods (rev, last, rfind, rfold)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — double-ended method type inference
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/double_ended_methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for double-ended methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: `repeat(value)` function for infinite iterators
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — repeat function evaluation
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/infinite.ori`
  - [ ] **LLVM Support**: LLVM codegen for repeat
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: Standard implementations for built-in types
  - [x] `[T]` implements `Iterable` (2026-02-15) <!-- DoubleEndedIterator, Collect pending -->
  - [x] `{K: V}` implements `Iterable` (2026-02-15) (NOT double-ended — unordered)
  - [x] `Set<T>` implements `Iterable` (2026-02-15) <!-- Collect pending --> (NOT double-ended — unordered)
  - [x] `str` implements `Iterable` (2026-02-15) <!-- DoubleEndedIterator pending -->
  - [x] `Range<int>` implements `Iterable` (2026-02-15) <!-- DoubleEndedIterator pending -->
  - [ ] `Option<T>` implements `Iterable`
  - [ ] **Note**: `Range<float>` does NOT implement `Iterable` (precision issues)
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/builtin_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for all builtin iterator impls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [x] **Implement**: Helper iterator types (ListIterator, RangeIterator, MapIterator, SetIterator, StrIterator) + adapter types (Mapped, Filtered, TakeN, SkipN) (2026-02-15)
  - [x] **Rust Tests**: `ori_patterns/src/value/iterator/tests.rs` — 22 unit tests (2026-02-15)
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/helper_types.ori`
  - [ ] **LLVM Support**: LLVM codegen for all helper iterator types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [x] **Implement**: Fused iterator guarantee (once None, always None) (2026-02-15)
  - [x] **Rust Tests**: `ori_patterns/src/value/iterator/tests.rs` — list_iterator_fused (2026-02-15)
  - [x] **Ori Tests**: `tests/spec/traits/iterator/iterator.ori` — test_list_iter_fused (2026-02-15)
  - [ ] **LLVM Support**: LLVM codegen respects fused guarantee
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: `for` loop desugaring to `Iterable.iter()` and functional `next()`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — for loop type checking
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/for_loop.ori`
  - [ ] **LLVM Support**: LLVM codegen for desugared for loops
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: `for...yield` desugaring to `.iter().map().collect()`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — for yield type checking
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/for_yield.ori`
  - [ ] **LLVM Support**: LLVM codegen for desugared for yield
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: Add traits and `repeat` to prelude
  - [ ] `Iterator`, `DoubleEndedIterator`, `Iterable`, `Collect` traits in prelude
  - [ ] `repeat` function in prelude
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/prelude.ori`

- [ ] **Update Spec**: `06-types.md` — add Iterator traits section
- [ ] **Update Spec**: `10-patterns.md` — document for loop desugaring
- [ ] **Update Spec**: `12-modules.md` — add to prelude
- [ ] **Update**: `CLAUDE.md` — add iterator documentation to quick reference

---

## 3.8.1 Iterator Performance and Semantics

**Proposal**: `proposals/approved/iterator-performance-semantics-proposal.md`

Formalizes the performance characteristics and precise semantics of Ori's functional iterator model. Specifies copy elision guarantees, lazy evaluation, compiler optimizations, and introduces infinite range syntax (`start..`).

### Implementation

- [ ] **Implement**: Copy elision for iterator rebinding patterns
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — copy elision verification
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/copy_elision.ori`
  - [ ] **LLVM Support**: LLVM codegen respects copy elision
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: Infinite range syntax `start..` in lexer/parser
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — infinite range parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/infinite_range.ori`
  - [ ] **LLVM Support**: LLVM codegen for infinite ranges
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs`

- [ ] **Implement**: Infinite range with step `start.. by step`
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr.rs` — infinite range step parsing
  - [ ] **Ori Tests**: `tests/spec/expressions/infinite_range_step.ori`
  - [ ] **LLVM Support**: LLVM codegen for stepped infinite ranges
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/range_tests.rs`

- [ ] **Implement**: Infinite range iteration (implements Iterable but NOT DoubleEndedIterator)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — infinite range trait bounds
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/infinite_range.ori`
  - [ ] **LLVM Support**: LLVM codegen for infinite range iteration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: Lint warnings for obvious infinite iteration patterns (SHOULD warn)
  - [ ] `repeat(...).collect()` without `take`
  - [ ] `(start..).collect()` without `take`
  - [ ] `iter.cycle().collect()` without `take`
  - [ ] **Rust Tests**: `oric/src/lint/tests.rs` — infinite iteration lint tests
  - [ ] **Ori Tests**: `tests/lint/infinite_iteration.ori`

- [ ] **Implement**: Guaranteed compiler optimizations
  - [ ] Copy elision when iterator rebound immediately
  - [ ] Inline expansion for iterator methods
  - [ ] Deforestation (intermediate iterator elimination)
  - [ ] Loop fusion (adjacent maps/filters combined)
  - [ ] **Rust Tests**: `ori_llvm/tests/optimization_tests.rs`

- [ ] **Update Spec**: `06-types.md` — add infinite range type variant
- [ ] **Update Spec**: `09-expressions.md` — add infinite range syntax section
- [ ] **Update Spec**: `grammar.ebnf` — update range_expr production
- [ ] **Update**: `CLAUDE.md` — add infinite range syntax and iterator performance notes

---

## 3.9 Debug Trait

**Proposal**: `proposals/approved/debug-trait-proposal.md`

Adds a `Debug` trait separate from `Printable` for developer-facing structural representation of values. `Debug` is automatically derivable and shows complete internal structure, while `Printable` remains for intentional user-facing output. Mirrors Rust's `Display` vs `Debug` distinction.

### Dependencies

- `as` conversion syntax (`as-conversion-proposal.md`) — for `self as str` conversions
- `str.escape()` method — stdlib method for escaping special characters
- `Iterator.join()` method — stdlib method for joining iterator elements

### Implementation

- [ ] **Implement**: `Debug` trait definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — debug trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/debug/definition.ori`

- [ ] **Implement**: Debug implementations for all primitives (int, float, bool, str, char, byte, void)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — primitive debug bounds
  - [ ] **Ori Tests**: `tests/spec/traits/debug/primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for primitive debug methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs`

- [ ] **Implement**: Debug implementations for Duration and Size
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — duration/size debug bounds
  - [ ] **Ori Tests**: `tests/spec/traits/debug/special_types.ori`
  - [ ] **LLVM Support**: LLVM codegen for duration/size debug
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs`

- [ ] **Implement**: Debug implementations for collections ([T], {K: V}, Set<T>)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — collection debug bounds
  - [ ] **Ori Tests**: `tests/spec/traits/debug/collections.ori`
  - [ ] **LLVM Support**: LLVM codegen for collection debug
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs`

- [ ] **Implement**: Debug implementations for Option<T> and Result<T, E>
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — option/result debug
  - [ ] **Ori Tests**: `tests/spec/traits/debug/wrappers.ori`
  - [ ] **LLVM Support**: LLVM codegen for option/result debug
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs`

- [ ] **Implement**: Debug implementations for tuples (all arities)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — tuple debug bounds
  - [ ] **Ori Tests**: `tests/spec/traits/debug/tuples.ori`
  - [ ] **LLVM Support**: LLVM codegen for tuple debug
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs`

- [ ] **Implement**: `#[derive(Debug)]` macro for user-defined types
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — debug derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/debug/derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for derived debug
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs`

- [ ] **Implement**: `str.escape()` method (dependency)
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — string escape tests
  - [ ] **Ori Tests**: `tests/spec/traits/debug/escape.ori`
  - [ ] **LLVM Support**: LLVM codegen for string escape
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs`

- [ ] **Implement**: `Iterator.join()` method (dependency)
  - [ ] **Rust Tests**: `ori_eval/src/methods.rs` — iterator join tests
  - [ ] **Ori Tests**: `tests/spec/traits/debug/join.ori`
  - [ ] **LLVM Support**: LLVM codegen for iterator join
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/debug_tests.rs`

- [ ] **Update Spec**: `06-types.md` — add Debug trait section
- [ ] **Update Spec**: `08-declarations.md` — add Debug to derivable traits list
- [ ] **Update Spec**: `12-modules.md` — add Debug to prelude traits
- [ ] **Update**: `CLAUDE.md` — add Debug to prelude traits list

---

## 3.10 Trait Resolution and Conflict Handling

**Proposal**: `proposals/approved/trait-resolution-conflicts-proposal.md`

Specifies rules for resolving trait implementation conflicts: diamond problem, coherence/orphan rules, method resolution order, super trait calls, and extension method conflicts.

### Implementation

- [x] **Implement**: Diamond problem resolution — single impl satisfies all inheritance paths
  - [x] **Rust Tests**: `ori_types/src/registry/traits/tests.rs` — `all_super_traits_diamond`, `collected_methods_deduplication`
  - [x] **Ori Tests**: `tests/spec/traits/resolution/diamond.ori`

- [x] **Implement**: Conflicting default detection — error when multiple supertraits provide conflicting defaults (E2022)
  - [x] **Rust Tests**: `ori_types/src/registry/traits/tests.rs` — `find_conflicting_defaults` covered in unit tests
  - [x] **Ori Tests**: `tests/compile-fail/conflicting_defaults.ori`, `tests/spec/traits/resolution/conflicting_defaults.ori`

- [ ] **Implement**: Coherence/orphan rules — at least one of trait or type must be local  <!-- blocked-by:4 -->
  - [ ] **Rust Tests**: orphan rule tests
  - [ ] **Ori Tests**: `tests/compile-fail/orphan_impl.ori`

- [ ] **Implement**: Blanket impl restrictions — orphan rules for `impl<T> Trait for T`  <!-- blocked-by:4 -->
  - [ ] **Rust Tests**: blanket impl tests
  - [ ] **Ori Tests**: `tests/compile-fail/orphan_blanket.ori`

- [x] **Implement**: Method resolution order — Inherent > Trait > Extension priority
  - [x] **Rust Tests**: `ori_types/src/registry/traits/tests.rs` — `lookup_method_checked` tests
  - [x] **Ori Tests**: `tests/spec/traits/resolution/method_priority.ori`
  - [ ] **LLVM Support**: LLVM codegen for method resolution order dispatch
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_resolution_tests.rs` — method resolution codegen

- [x] **Implement**: Ambiguous method detection (E2023) — error when multiple trait impls provide same method
  - [x] **Rust Tests**: `ori_types/src/registry/traits/tests.rs` — `MethodLookupResult::Ambiguous` tested
  - [x] **Ori Tests**: `tests/spec/traits/resolution/ambiguous_method.ori`
  - [ ] **Implement**: Fully-qualified syntax `Trait.method(x)` for disambiguation  <!-- blocked-by:parser -->

- [ ] **Implement**: Super trait calls with `Trait.method(self)` syntax  <!-- blocked-by:parser -->
  - [ ] **Rust Tests**: super call tests
  - [ ] **Ori Tests**: `tests/spec/traits/resolution/super_calls.ori`
  - [ ] **LLVM Support**: LLVM codegen for super trait call dispatch
  - [ ] **LLVM Rust Tests**: super trait call codegen

- [ ] **Implement**: Extension method conflict detection (including re-exports)  <!-- blocked-by:4 -->
  - [ ] **Rust Tests**: extension conflict tests
  - [ ] **Ori Tests**: `tests/compile-fail/extension_conflict.ori`

- [ ] **Implement**: Associated type disambiguation with `Type::Trait::AssocType` syntax  <!-- blocked-by:parser --><!-- unblocks:0.9.1 -->
  - [ ] **Rust Tests**: associated type disambiguation
  - [ ] **Ori Tests**: `tests/spec/traits/resolution/assoc_type_disambiguation.ori`

- [x] **Implement**: Implementation specificity (Concrete > Constrained > Generic)
  - [x] **Rust Tests**: `ori_types/src/registry/traits/tests.rs` — `ImplSpecificity` enum + specificity-aware lookup
  - [ ] **Ori Tests**: `tests/spec/traits/resolution/specificity.ori` — needs generic impls in type checker

- [x] **Implement**: Overlapping impl detection — compile error for equal-specificity impls (E2021)
  - [x] **Rust Tests**: `ori_types/src/registry/traits/tests.rs` — overlap detection in `lookup_method_checked`
  - [ ] **Ori Tests**: `tests/compile-fail/overlapping_impls.ori` — needs generic impls in type checker

- [x] **Implement**: Error codes E2010, E2021-E2023
  - [x] E2010: Duplicate implementation — `TypeErrorKind::DuplicateImpl`, `tests/compile-fail/duplicate_impl.ori`
  - [x] E2021: Overlapping implementations — `TypeErrorKind::OverlappingImpls`, `errors/E2021.md`
  - [x] E2022: Conflicting defaults — `TypeErrorKind::ConflictingDefaults`, `errors/E2022.md`
  - [x] E2023: Ambiguous method call — `TypeErrorKind::AmbiguousMethod`, `errors/E2023.md`

- [ ] **Update Spec**: `08-declarations.md` — add coherence, resolution, super calls sections
- [ ] **Update**: `CLAUDE.md` — add trait resolution rules to quick reference

---

## 3.11 Object Safety Rules

**Proposal**: `proposals/approved/object-safety-rules-proposal.md`

Formalizes the rules that determine whether a trait can be used as a trait object for dynamic dispatch. Defines three object safety rules and associated error codes.

### Implementation

- [ ] **Implement**: Object safety checking in type checker
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — object safety detection
  - [ ] **Ori Tests**: `tests/spec/traits/object_safety/detection.ori`

- [ ] **Implement**: Rule 1 — No `Self` in return position
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — self-return detection
  - [ ] **Ori Tests**: `tests/spec/traits/object_safety/self_return.ori`
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/object_safety_self_return.ori`

- [ ] **Implement**: Rule 2 — No `Self` in parameter position (except receiver)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — self-param detection
  - [ ] **Ori Tests**: `tests/spec/traits/object_safety/self_param.ori`
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/object_safety_self_param.ori`

- [ ] **Implement**: Rule 3 — No generic methods
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — generic method detection
  - [ ] **Ori Tests**: `tests/spec/traits/object_safety/generic_methods.ori`
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/object_safety_generic_method.ori`

- [ ] **Implement**: Error messages (E0800-E0802)
  - [ ] E0800: Self in return position
  - [ ] E0801: Self as non-receiver parameter
  - [ ] E0802: Generic method in trait

- [ ] **Implement**: Object safety checking at trait object usage sites
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — usage site detection
  - [ ] **Ori Tests**: `tests/spec/traits/object_safety/usage_sites.ori`
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/object_safety_usage.ori`

- [ ] **Implement**: Bounded trait objects (`Printable + Hashable`) — all components must be object-safe
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — bounded trait object tests
  - [ ] **Ori Tests**: `tests/spec/traits/object_safety/bounded.ori`
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/object_safety_bounded.ori`

- [ ] **Update Spec**: `06-types.md` — expand Object Safety section with all three rules
- [ ] **Update Spec**: `08-declarations.md` — add guidance on object-safe trait design
- [ ] **Update**: `CLAUDE.md` — add object safety rules to quick reference

---

## 3.12 Custom Subscripting (Index Trait)

**Proposals**:
- `proposals/approved/custom-subscripting-proposal.md` — Design and motivation
- `proposals/approved/index-trait-proposal.md` — Formal specification and error messages

Introduces the `Index` trait for read-only custom subscripting, allowing user-defined types to use `[]` syntax. Supports multiple index types per type (e.g., `JsonValue` with both `str` and `int` keys) and flexible return types (`T`, `Option<T>`, or `Result<T, E>`).

### Implementation

- [ ] **Implement**: `Index<Key, Value>` trait definition in prelude
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — index trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/index/definition.ori`

- [ ] **Implement**: Desugaring `x[k]` to `x.index(key: k)` in parser/desugarer
  - [ ] **Rust Tests**: `oric/src/desugar/tests.rs` — subscript desugaring tests
  - [ ] **Ori Tests**: `tests/spec/traits/index/desugaring.ori`
  - [ ] **LLVM Support**: LLVM codegen for desugared index calls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/index_tests.rs`

- [ ] **Implement**: Type inference for subscript expressions (resolve which `Index` impl based on key type)
  - [ ] **Rust Tests**: `oric/src/typeck/infer/tests.rs` — subscript type inference tests
  - [ ] **Ori Tests**: `tests/spec/traits/index/inference.ori`

- [ ] **Implement**: Multiple `Index` impls per type (different key types)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — multiple impl resolution
  - [ ] **Ori Tests**: `tests/spec/traits/index/multiple_impls.ori`

- [ ] **Implement**: Built-in `Index` implementations for `[T]`, `[T, max N]`, `{K: V}`, `str`
  - [ ] `[T]` implements `Index<int, T>` (panics on out-of-bounds)
  - [ ] `[T, max N]` implements `Index<int, T>` (same as `[T]`)
  - [ ] `{K: V}` implements `Index<K, Option<V>>`
  - [ ] `str` implements `Index<int, str>` (single codepoint, panics on out-of-bounds)
  - [ ] **Ori Tests**: `tests/spec/traits/index/builtin_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for builtin Index impls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/index_tests.rs`

- [ ] **Implement**: Error messages for Index trait (E0950-E0952)
  - [ ] E0950: mismatched types in index expression
  - [ ] E0951: type cannot be indexed (Index not implemented)
  - [ ] E0952: ambiguous index key type
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — error message tests
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/index_errors.ori`

- [ ] **Update Spec**: `09-expressions.md` — expand Index Trait section with fixed-capacity list
- [ ] **Update Spec**: `06-types.md` — add Index trait to prelude section
- [ ] **Update**: `CLAUDE.md` — add Index trait to prelude and subscripting documentation

---

## 3.13 Additional Core Traits

**Proposal**: `proposals/approved/additional-traits-proposal.md`

Formalizes three core traits: `Printable`, `Default`, and `Traceable`. The `Iterable`, `Iterator`, `DoubleEndedIterator`, and `Collect` traits are already defined in the spec and implemented in Section 3.8.

### Implementation

- [ ] **Implement**: `Printable` trait formal definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — printable trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/printable/definition.ori`
  - [ ] **LLVM Support**: LLVM codegen for Printable trait methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_method_tests.rs` — Printable codegen

- [ ] **Implement**: Printable derivation with `Point(1, 2)` format (type name + values)
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — printable derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/printable/derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for Printable derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_method_tests.rs` — Printable derivation codegen

- [ ] **Implement**: `Default` trait formal definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — default trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/default/definition.ori`
  - [ ] **LLVM Support**: LLVM codegen for Default trait methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_method_tests.rs` — Default codegen

- [ ] **Implement**: Default derivation for structs only (error on sum types)
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — default derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/default/derive.ori`
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/default_sum_type.ori`
  - [ ] **LLVM Support**: LLVM codegen for Default derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_method_tests.rs` — Default derivation codegen

- [ ] **Implement**: `Traceable` trait formal definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — traceable trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/traceable/definition.ori`
  - [ ] **LLVM Support**: LLVM codegen for Traceable trait methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_method_tests.rs` — Traceable codegen

- [ ] **Implement**: Traceable for Error type with trace storage
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — error trace evaluation
  - [ ] **Ori Tests**: `tests/spec/traits/traceable/error.ori`
  - [ ] **LLVM Support**: LLVM codegen for Traceable Error type
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_method_tests.rs` — Traceable Error codegen

- [ ] **Implement**: Traceable delegation for Result<T, E: Traceable>
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — result traceable bounds
  - [ ] **Ori Tests**: `tests/spec/traits/traceable/result.ori`
  - [ ] **LLVM Support**: LLVM codegen for Traceable Result delegation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/trait_method_tests.rs` — Traceable Result codegen

- [ ] **Implement**: Error messages (E1040, E1042)
  - [ ] E1040: Missing Printable for string interpolation
  - [ ] E1042: Cannot derive Default for sum type

- [ ] **Update Spec**: `07-properties-of-types.md` — add Printable, Default, Traceable sections (DONE)
- [ ] **Update**: `CLAUDE.md` — ensure traits documented in quick reference

---

## 3.14 Comparable and Hashable Traits

**Proposal**: `proposals/approved/comparable-hashable-traits-proposal.md`

Formalizes the `Comparable` and `Hashable` traits with complete definitions, mathematical invariants, standard implementations, and derivation rules. Adds `Result<T, E>` to both trait implementations and introduces `hash_combine` as a prelude function.

> **Phase boundary discipline**: Each method must be implemented across ALL THREE phases (type checker → evaluator → LLVM codegen) before the type checker may accept it. Commit 01051607 added `hash`/`equals` to the type checker for compound types without evaluator/LLVM handlers — this was reverted in the hygiene review (see 3.7). When implementing items below, add to type checker LAST, after evaluator and LLVM codegen are working.
>
> **`equals()` on compound types** is also tracked here. The Eq trait (3.0.6) covers primitives only. Collection/wrapper `equals()` methods (list, map, set, Option, Result, tuple) require the same all-phase implementation as `hash()`.

### Implementation

- [ ] **Implement**: Formal `Comparable` trait definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — comparable trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/comparable/definition.ori`

- [ ] **Implement**: Comparable implementations for all primitives (int, float, bool, str, char, byte, Duration, Size)
  - [ ] **Rust Implementation**: `ori_eval/src/methods.rs` — dispatch_*_method with compare()
  - [ ] **Type Checking**: `ori_typeck/src/infer/builtin_methods/` — compare() returns Ordering for all primitives
  - [ ] **Ori Tests**: `tests/spec/traits/core/comparable.ori` — all primitive compare() tests
  - [ ] **Ori Tests**: `tests/spec/types/duration_size_comparable.ori` — Duration/Size compare() tests
  - [ ] **LLVM Support**: LLVM codegen for primitive compare methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: Comparable implementations for lists ([T])
  - [ ] **Rust Implementation**: `ori_eval/src/methods.rs` — dispatch_list_method with compare()
  - [ ] **Type Checking**: `ori_typeck/src/infer/builtin_methods/list.rs` — compare() returns Ordering
  - [ ] **Ori Tests**: `tests/spec/traits/core/comparable.ori` — list compare() tests
  - [ ] **LLVM Support**: LLVM codegen for list compare
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: Comparable implementations for tuples
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — tuple comparable bounds
  - [ ] **Ori Tests**: `tests/spec/traits/comparable/tuples.ori`
  - [ ] **LLVM Support**: LLVM codegen for tuple compare
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: Comparable implementation for Option<T>
  - [ ] **Rust Implementation**: `ori_eval/src/methods.rs` — dispatch_option_method with compare()
  - [ ] **Type Checking**: `ori_typeck/src/infer/builtin_methods/option.rs` — compare() returns Ordering
  - [ ] **Ori Tests**: `tests/spec/traits/core/comparable.ori` — Option compare() tests
  - [ ] **LLVM Support**: LLVM codegen for option compare
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: Comparable implementation for Result<T, E>
  - [ ] **Rust Implementation**: `ori_eval/src/methods.rs` — dispatch_result_method with compare()
  - [ ] **Type Checking**: `ori_typeck/src/infer/builtin_methods/result.rs` — compare() returns Ordering
  - [ ] Result: `Ok(_) < Err(_)`, then compare inner values
  - [ ] **Ori Tests**: `tests/spec/traits/core/comparable.ori` — Result compare() tests
  - [ ] **LLVM Support**: LLVM codegen for result compare
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: Float IEEE 754 total ordering (NaN handling)
  - [ ] **Rust Implementation**: `ori_eval/src/methods.rs` — uses `total_cmp()` for IEEE 754 ordering
  - [ ] **Ori Tests**: `tests/spec/traits/comparable/float_nan.ori`
  - [ ] **LLVM Support**: LLVM codegen for float total ordering
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: Comparable implementation for Ordering
  - [ ] **Rust Implementation**: `ori_eval/src/methods.rs` — dispatch_ordering_method with compare()
  - [ ] **Type Checking**: `ori_typeck/src/infer/builtin_methods/ordering.rs` — compare() returns Ordering
  - [ ] Ordering: `Less < Equal < Greater`
  - [ ] **Ori Tests**: `tests/spec/traits/core/comparable.ori` — Ordering compare() tests
  - [ ] **LLVM Support**: LLVM codegen for ordering compare
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: Comparison operator derivation (`<`, `<=`, `>`, `>=` via Ordering methods)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — operator desugaring
  - [ ] **Ori Tests**: `tests/spec/traits/comparable/operators.ori`
  - [ ] **LLVM Support**: LLVM codegen for comparison operators
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: Formal `Hashable` trait definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — hashable trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/hashable/definition.ori`

- [ ] **Implement**: Hashable implementations for all primitives (int, float, bool, str, char, byte, Duration, Size)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — primitive hashable bounds
  - [ ] **Ori Tests**: `tests/spec/traits/hashable/primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for primitive hash methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/hashable_tests.rs`

- [ ] **Implement**: Hashable implementations for collections ([T], {K: V}, Set<T>, tuples)
  - [ ] **Evaluator**: `dispatch_list_method`, `dispatch_map_method`, `dispatch_set_method`, `dispatch_tuple_method` — add `hash` handler
  - [ ] **LLVM Support**: LLVM codegen for collection hash
  - [ ] **Type Checker**: `resolve_*_method` — add `hash` back (LAST, after eval+LLVM)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — collection hashable bounds
  - [ ] **Ori Tests**: `tests/spec/traits/hashable/collections.ori`
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/hashable_tests.rs`

- [ ] **Implement**: Hashable implementations for Option<T> and Result<T, E>
  - [ ] **Evaluator**: `dispatch_option_method`, `dispatch_result_method` — add `hash` handler
  - [ ] **LLVM Support**: LLVM codegen for option/result hash
  - [ ] **Type Checker**: `resolve_option_method`, `resolve_result_method` — add `hash` back (LAST, after eval+LLVM)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — option/result hashable
  - [ ] **Ori Tests**: `tests/spec/traits/hashable/wrappers.ori`
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/hashable_tests.rs`

- [ ] **Implement**: Float hashing consistency (+0.0 == -0.0, NaN == NaN for hash)
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — float hash edge cases
  - [ ] **Ori Tests**: `tests/spec/traits/hashable/float_hash.ori`
  - [ ] **LLVM Support**: LLVM codegen for float hash normalization
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/hashable_tests.rs`

- [ ] **Implement**: `hash_combine` function in prelude
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — hash_combine evaluation
  - [ ] **Ori Tests**: `tests/spec/traits/hashable/hash_combine.ori`
  - [ ] **LLVM Support**: LLVM codegen for hash_combine
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/hashable_tests.rs`

- [ ] **Implement**: `#[derive(Comparable)]` macro for user-defined types
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — comparable derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/comparable/derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for derived comparable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: `#[derive(Hashable)]` macro for user-defined types
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — hashable derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/hashable/derive.ori`
  - [ ] **LLVM Support**: LLVM codegen for derived hashable
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/hashable_tests.rs`

- [ ] **Implement**: Error messages (E0940-E0942)
  - [ ] E0940: Cannot derive Hashable without Eq
  - [ ] E0941: Hashable implementation violates hash invariant
  - [ ] E0942: Type cannot be used as map key (missing Hashable)

- [ ] **Update Spec**: `07-properties-of-types.md` — add Comparable and Hashable sections
- [ ] **Update Spec**: `12-modules.md` — add hash_combine to prelude functions
- [ ] **Update**: `CLAUDE.md` — add Comparable, Hashable, hash_combine documentation

- [ ] **Implement**: `equals()` on compound types (Eq trait extension beyond primitives)
  - [ ] Evaluator: `dispatch_list_method` — element-wise equality
  - [ ] Evaluator: `dispatch_option_method` — tag + inner equality
  - [ ] Evaluator: `dispatch_result_method` — tag + inner equality
  - [ ] Evaluator: `dispatch_map_method` — key-set + value equality
  - [ ] Evaluator: `dispatch_tuple_method` — element-wise equality
  - [ ] LLVM codegen: `lower_builtin_method` — inline equals for all compound types
  - [ ] Type checker: `resolve_*_method` — add `equals` back (LAST, after eval+LLVM)
  - [ ] Type checker: `type_satisfies_trait()` — re-add `"Eq"` to `COLLECTION_TRAITS`, `WRAPPER_TRAITS`, `RESULT_TRAITS` (removed in 3.7 hygiene pass 2)
  - [ ] **Ori Tests**: `tests/spec/traits/eq/compound_types.ori`
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/aot/` — compound equals codegen

---

## 3.15 Derived Traits Formal Semantics

**Proposal**: `proposals/approved/derived-traits-proposal.md`

Formalizes the `#derive` attribute semantics: derivable traits list, derivation rules, field constraints, generic type handling, and error messages.

### Implementation

- [ ] **Implement**: Eq derivation for structs — field-wise equality
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — eq derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/derive/eq.ori`
  - [ ] **LLVM Support**: LLVM codegen for Eq struct derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — Eq struct derive codegen

- [ ] **Implement**: Eq derivation for sum types — variant matching
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — eq sum type tests
  - [ ] **Ori Tests**: `tests/spec/traits/derive/eq_sum.ori`
  - [ ] **LLVM Support**: LLVM codegen for Eq sum type derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — Eq sum type derive codegen

- [ ] **Implement**: Hashable derivation — combined field hashes via `hash_combine`
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — hashable derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/derive/hashable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Hashable derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — Hashable derive codegen

- [ ] **Implement**: Comparable derivation — lexicographic field comparison
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — comparable derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/derive/comparable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Comparable derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — Comparable derive codegen

- [ ] **Implement**: Clone derivation — field-wise clone
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — clone derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/derive/clone.ori`
  - [ ] **LLVM Support**: LLVM codegen for Clone derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — Clone derive codegen

- [ ] **Implement**: Default derivation for structs only
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — default derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/derive/default.ori`
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/derive_default_sum.ori`
  - [ ] **LLVM Support**: LLVM codegen for Default derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — Default derive codegen

- [ ] **Implement**: Debug derivation — structural representation with type name
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — debug derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/derive/debug.ori`
  - [ ] **LLVM Support**: LLVM codegen for Debug derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — Debug derive codegen

- [ ] **Implement**: Printable derivation — human-readable format `TypeName(field1, field2)`
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — printable derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/derive/printable.ori`
  - [ ] **LLVM Support**: LLVM codegen for Printable derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — Printable derive codegen

- [ ] **Implement**: Generic type conditional derivation — bounded impls
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — generic derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/derive/generic.ori`
  - [ ] **LLVM Support**: LLVM codegen for generic conditional derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — generic derive codegen

- [ ] **Implement**: Recursive type derivation
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — recursive derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/derive/recursive.ori`
  - [ ] **LLVM Support**: LLVM codegen for recursive type derivation
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/derive_tests.rs` — recursive derive codegen

- [ ] **Implement**: Error messages (E0880-E0882)
  - [ ] E0880: Cannot derive trait for type (field missing trait)
  - [ ] E0881: Trait is not derivable
  - [ ] E0882: Cannot derive Default for sum type

- [ ] **Implement**: Warning W0100 — Hashable derived without Eq

- [ ] **Update Spec**: `06-types.md` — expand Derive section with formal semantics
- [ ] **Update Spec**: `07-properties-of-types.md` — add cross-reference to derive semantics
- [ ] **Update**: `CLAUDE.md` — update derive documentation

---

## 3.16 Formattable Trait

**Proposal**: `proposals/approved/formattable-trait-proposal.md`

Formalizes the `Formattable` trait and format specification syntax for customized string formatting. Defines `FormatSpec` type structure, format spec syntax, and the relationship between `Formattable` and `Printable` via blanket implementation.

### Implementation

- [ ] **Implement**: `Formattable` trait definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — formattable trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/formattable/definition.ori`

- [ ] **Implement**: `FormatSpec` type and related types (`Alignment`, `Sign`, `FormatType`) in prelude
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — format spec type definitions
  - [ ] **Ori Tests**: `tests/spec/traits/formattable/format_spec.ori`

- [ ] **Implement**: Format spec parsing in template strings
  - [ ] **Rust Tests**: `oric/src/parse/template_string.rs` — format spec parsing
  - [ ] **Ori Tests**: `tests/spec/traits/formattable/parsing.ori`
  - [ ] **LLVM Support**: LLVM codegen for format spec parsing
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/formattable_tests.rs`

- [ ] **Implement**: Blanket `Formattable` implementation for `Printable` types
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — blanket impl resolution
  - [ ] **Ori Tests**: `tests/spec/traits/formattable/blanket_impl.ori`

- [ ] **Implement**: `Formattable` for `int` with binary, octal, hex format types
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — int format evaluation
  - [ ] **Ori Tests**: `tests/spec/traits/formattable/int.ori`
  - [ ] **LLVM Support**: LLVM codegen for int formatting
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/formattable_tests.rs`

- [ ] **Implement**: `Formattable` for `float` with scientific, fixed, percentage format types
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — float format evaluation
  - [ ] **Ori Tests**: `tests/spec/traits/formattable/float.ori`
  - [ ] **LLVM Support**: LLVM codegen for float formatting
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/formattable_tests.rs`

- [ ] **Implement**: `Formattable` for `str` with width, alignment, precision
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — str format evaluation
  - [ ] **Ori Tests**: `tests/spec/traits/formattable/str.ori`
  - [ ] **LLVM Support**: LLVM codegen for str formatting
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/formattable_tests.rs`

- [ ] **Implement**: Sign specifiers (`+`, `-`, ` `) for numeric formatting
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — sign format evaluation
  - [ ] **Ori Tests**: `tests/spec/traits/formattable/sign.ori`

- [ ] **Implement**: Alternate form (`#`) for prefix formatting (0b, 0o, 0x)
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — alternate form evaluation
  - [ ] **Ori Tests**: `tests/spec/traits/formattable/alternate.ori`

- [ ] **Implement**: Zero-pad shorthand (`0`) for numeric formatting
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — zero-pad evaluation
  - [ ] **Ori Tests**: `tests/spec/traits/formattable/zero_pad.ori`

- [ ] **Implement**: Error messages (E0970-E0972)
  - [ ] E0970: Invalid format specification
  - [ ] E0971: Format type not supported for type
  - [ ] E0972: Type does not implement Formattable

- [ ] **Update Spec**: `07-properties-of-types.md` — add Formattable trait section
- [ ] **Update Spec**: `12-modules.md` — add FormatSpec, Alignment, Sign, FormatType to prelude
- [ ] **Update**: `CLAUDE.md` — update Formattable entry with full format spec syntax

---

## 3.17 Into Trait

**Proposal**: `proposals/approved/into-trait-proposal.md`

Formalizes the `Into` trait for semantic, lossless type conversions. Defines trait signature, standard implementations, relationship to `as`/`as?`, and rules for custom implementations.

### Implementation

- [ ] **Implement**: `Into<T>` trait definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — into trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/into/definition.ori`

- [ ] **Implement**: Into implementation for str→Error
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — str to error conversion
  - [ ] **Ori Tests**: `tests/spec/traits/into/str_to_error.ori`
  - [ ] **LLVM Support**: LLVM codegen for str→Error conversion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/into_tests.rs`

- [ ] **Implement**: Into implementation for int→float (numeric widening)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — int to float conversion
  - [ ] **Ori Tests**: `tests/spec/traits/into/int_to_float.ori`
  - [ ] **LLVM Support**: LLVM codegen for int→float conversion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/into_tests.rs`

- [ ] **Implement**: Into implementation for Set<T>→[T] (with T: Eq + Hashable constraint)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — set to list conversion
  - [ ] **Ori Tests**: `tests/spec/traits/into/set_to_list.ori`
  - [ ] **LLVM Support**: LLVM codegen for Set→List conversion
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/into_tests.rs`

- [ ] **Implement**: Custom Into implementations for user types
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — custom into impl
  - [ ] **Ori Tests**: `tests/spec/traits/into/custom_impl.ori`

- [ ] **Implement**: No blanket identity (no impl<T> Into<T> for T)
  - [ ] **Ori Tests**: `tests/compile-fail/into_no_identity.ori`

- [ ] **Implement**: No automatic conversion chaining
  - [ ] **Ori Tests**: `tests/compile-fail/into_no_chaining.ori`

- [ ] **Implement**: Orphan rule enforcement for Into implementations
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — orphan rule tests
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/into_orphan_rule.ori`

- [ ] **Implement**: Error messages (E0960-E0961)
  - [ ] E0960: Type does not implement Into<T>
  - [ ] E0961: Multiple Into implementations apply (ambiguous)

- [ ] **Update Spec**: `07-properties-of-types.md` — add Into trait section
- [ ] **Update Spec**: `12-modules.md` — verify Into in prelude traits list
- [ ] **Update**: `CLAUDE.md` — add Into documentation to prelude

---

## 3.18 Ordering Type

**STATUS: Partial — Core methods + `then` complete, `then_with` deferred (needs closure calling in method dispatch)**

**Proposal**: `proposals/approved/ordering-type-proposal.md`

Formalizes the `Ordering` type that represents comparison results. Defines the three variants (`Less`, `Equal`, `Greater`), methods (`is_less`, `is_equal`, `is_greater`, `is_less_or_equal`, `is_greater_or_equal`, `reverse`, `then`, `then_with`), and trait implementations.

### Implementation

- [x] **Implement**: `Ordering` type definition (already in spec as `type Ordering = Less | Equal | Greater`) [done] (2026-02-10)
  - Type checking via `Type::Named("Ordering")`
  - Runtime values via `Value::Variant { type_name: "Ordering", ... }`
  - [x] **Variants available as bare names**: `Less`, `Equal`, `Greater` are registered as built-in enum variants
    - Type registry: `register_builtin_types()` in `ori_typeck/src/registry/mod.rs`
    - Evaluator: `register_prelude()` in `ori_eval/src/interpreter/mod.rs`
  - [x] **Ori Tests**: `tests/spec/types/ordering/methods.ori` — 32 tests (all pass)

- [x] **Implement**: Ordering predicate methods (`is_less`, `is_equal`, `is_greater`, `is_less_or_equal`, `is_greater_or_equal`) [done] (2026-02-10)
  - [x] **Type checker**: `ori_typeck/src/infer/builtin_methods/ordering.rs`
  - [x] **Evaluator**: `ori_eval/src/methods.rs` — `dispatch_ordering_method`
  - [x] **Ori Tests**: `tests/spec/types/ordering/methods.ori` — 15 predicate tests (all pass)
  - [x] **LLVM Support**: i8 comparison/arithmetic in `lower_calls.rs`

- [x] **Implement**: `reverse` method for Ordering [done] (2026-02-10)
  - [x] **Type checker**: Returns `Type::Named("Ordering")`
  - [x] **Evaluator**: Swaps Less↔Greater, preserves Equal
  - [x] **Ori Tests**: `tests/spec/types/ordering/methods.ori` — 4 reverse tests including involution

- [x] **Implement**: `then` method for lexicographic comparison chaining [done] (2026-02-15)
  - Keyword conflict resolved: keywords now valid as member names after `.` (grammar.ebnf § member_name)
  - [x] **Parser**: `expect_member_name()` in cursor, used by postfix.rs
  - [x] **Type checker**: `resolve_ordering_method()` — returns `Idx::ORDERING`
  - [x] **Evaluator**: `dispatch_ordering_method()` — Equal chains, non-Equal keeps self
  - [x] **IR registry**: `builtin_methods.rs` — `MethodDef` with `ParamSpec::SelfType`
  - [x] **Eval registry**: `EVAL_BUILTIN_METHODS` — `("Ordering", "then")`
  - [x] **Ori Tests**: `tests/spec/types/ordering/methods.ori` — 5 tests (equal chains, non-equal keeps self, chaining)
  - [x] **Rust Tests**: `ori_eval/src/tests/methods_tests.rs` — `then_equal_chains`, `then_non_equal_keeps_self`

- [ ] **Implement**: `then_with` method for lazy lexicographic chaining
  - Deferred: requires closure-calling capability in method dispatch (`DispatchCtx` only has names/interner)
  - [ ] **Ori Tests**: `tests/spec/types/ordering/then_with.ori`

- [x] **Implement**: Trait methods for Ordering (Clone, Printable, Hashable) [done] (2026-02-10)
  - [x] `clone()` → returns self — verified in ordering/methods.ori
  - [x] `to_str()` → "Less"/"Equal"/"Greater" — verified in ordering/methods.ori
  - [ ] `debug()` → "Less"/"Equal"/"Greater" — NOT implemented (Debug trait doesn't exist yet)
  - [x] `hash()` → distinct values for each variant — verified in ordering/methods.ori

- [ ] **Implement**: Default value is `Equal` (via associated function `Ordering.default()`) — NOT testable (static methods not supported)

- [ ] **Update Spec**: `06-types.md` — expand Ordering section with methods and trait implementations
- [ ] **Update**: `CLAUDE.md` — Ordering methods already documented in quick reference

---

## 3.19 Default Type Parameters on Traits

**STATUS: COMPLETE**

**Proposal**: `proposals/approved/default-type-parameters-proposal.md`

Allow type parameters on traits to have default values, enabling `trait Add<Rhs = Self>` where `Rhs` defaults to `Self` if not specified. Essential prerequisite for operator traits.

### Implementation

- [x] **Implement**: Parse default type in `type_param` grammar rule (`identifier [ ":" bounds ] [ "=" type ]`) [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/item/generics.rs` — `parse_generics()` handles `= Type` after bounds
  - [x] **Ori Tests**: `tests/spec/traits/default_type_params.ori` — 2 tests (all pass)

- [x] **Implement**: Store default types in trait definition AST [done] (2026-02-10)
  - [x] **Rust Tests**: `GenericParam` in `ori_ir/src/ast/items/traits.rs` has `default_type: Option<ParsedType>`
  - [x] **Rust Tests**: `TraitEntry` in `ori_typeck/src/registry/trait_types.rs` has `default_types: Vec<Option<ParsedType>>`

- [x] **Implement**: Fill missing type arguments with defaults in impl checking [done] (2026-02-10)
  - [x] **Rust Tests**: `resolve_trait_type_args()` in `trait_registration.rs`
  - [x] **Ori Tests**: `tests/spec/traits/default_type_params.ori` — `impl Addable for Point` omits Rhs, uses Self default

- [x] **Implement**: Substitute `Self` with implementing type in defaults [done] (2026-02-10)
  - [x] **Rust Tests**: `resolve_parsed_type_with_self_substitution()` in `trait_registration.rs`
  - [x] **Ori Tests**: `tests/spec/traits/default_type_params.ori` — `test_add` uses Self default

- [x] **Implement**: Ordering constraint enforcement (defaults must follow non-defaults) [done] (2026-02-10)
  - [x] **Rust Tests**: `validate_default_type_param_ordering()` in `trait_registration.rs`
  - [x] **Error Code**: E2015 (type parameter ordering violation)

- [x] **Implement**: Later parameters can reference earlier ones in defaults [done] (2026-02-10)
  - [x] **Design**: Stored as `ParsedType`, resolved at impl time with substitution
  - [x] **Verified**: `trait Transform<Input = Self, Output = Input>` in default_type_params.ori

- [x] **Update Spec**: `grammar.ebnf` § Generics — `type_param = identifier [ ":" bounds ] [ "=" type ] .` [done] (verified 2026-02-15, already present)
- [x] **Update Spec**: `08-declarations.md` — Default Type Parameters section under Traits [done] (verified 2026-02-15, already present at line 230)
- [x] **Update**: `CLAUDE.md` — `trait N<T = Self>` syntax documented [done] (verified 2026-02-15, already in ori-syntax.md)

---

## 3.20 Default Associated Types

**STATUS: COMPLETE**

**Proposal**: `proposals/approved/default-associated-types-proposal.md`

Allow associated types in traits to have default values, enabling `type Output = Self` where implementors can omit the associated type if the default is acceptable. Works alongside default type parameters to enable operator traits.

### Implementation

- [x] **Implement**: Parse default type in `assoc_type` grammar rule (`"type" identifier [ ":" bounds ] [ "=" type ]`) [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/grammar/item/trait_def.rs` — default assoc type parsing
  - [x] **Ori Tests**: `tests/spec/traits/default_assoc_types.ori` — 4 tests (all pass)

- [x] **Implement**: Store default types in trait definition AST for associated types [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_ir/src/ast/items/traits.rs` — `TraitAssocType.default_type: Option<ParsedType>`

- [x] **Implement**: Fill missing associated types with defaults in impl checking [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_typeck/src/checker/trait_registration.rs` — `validate_associated_types()` uses defaults
  - [x] **Ori Tests**: `tests/spec/traits/default_assoc_types.ori` — Point impl omits Output, uses Self default

- [x] **Implement**: Substitute `Self` with implementing type in defaults [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_typeck/src/registry/trait_registry.rs` — `resolve_parsed_type_with_self_substitution()`
  - [x] **Ori Tests**: `tests/spec/traits/default_assoc_types.ori` — verified with Point (default Output=Self) and Number (overridden Output=int)

- [x] **Implement**: Defaults can reference type parameters and other associated types [done] (2026-02-10)
  - Note: Basic support implemented; complex cascading defaults deferred

- [ ] **Implement**: Bounds checking — verify default satisfies any bounds after substitution
  - Note: Deferred to future enhancement; bounds on associated types not yet fully implemented

- [x] **Update Spec**: `grammar.ebnf` — update assoc_type production [done] (verified 2026-02-15, already present: `assoc_type = "type" identifier [ ":" bounds ] [ "=" type ]`)
- [x] **Update Spec**: `08-declarations.md` — add Default Associated Types section [done] (verified 2026-02-15, already present at line 272 with bounds section)
- [x] **Update**: `CLAUDE.md` — add default associated type syntax to Traits section [done] (verified 2026-02-15, already in ori-syntax.md: `type Output = Self` default)

---

## 3.21 Operator Traits

**STATUS: Complete — Type checker desugaring, evaluator dispatch, LLVM codegen, error messages all working. Derive for newtypes optional/deferred.**

**Proposal**: `proposals/approved/operator-traits-proposal.md`

Defines traits for arithmetic, bitwise, and unary operators that user-defined types can implement to support operator syntax. The compiler desugars operators to trait method calls. Enables Duration and Size types to move to stdlib.

### Dependencies

- [x] Default Type Parameters on Traits (3.19) — for `trait Add<Rhs = Self>` [done] (2026-02-10)
- [x] Default Associated Types (3.20) — for `type Output = Self` [done] (2026-02-10)

### Implementation

- [x] **Implement**: Define operator traits in prelude (via trait registry lookup) [done] (2026-02-15)
  - [x] `Add<Rhs = Self>`, `Sub<Rhs = Self>`, `Mul<Rhs = Self>`, `Div<Rhs = Self>`, `FloorDiv<Rhs = Self>`, `Rem<Rhs = Self>`
  - [x] `Neg`, `Not`, `BitNot`
  - [x] `BitAnd<Rhs = Self>`, `BitOr<Rhs = Self>`, `BitXor<Rhs = Self>`, `Shl<Rhs = int>`, `Shr<Rhs = int>`
  - [x] **Files**: `library/std/prelude.ori` — all operator traits defined with associated `Output` type
  - [x] **Ori Tests**: `tests/spec/traits/operators/user_defined.ori` — 16 tests covering all operators

- [x] **Implement**: Operator desugaring in type checker [done] (2026-02-15)
  - [x] `a + b` → `a.add(rhs: b)` (etc. for all binary operators)
  - [x] `-a` → `a.negate()`, `!a` → `a.not()`, `~a` → `a.bit_not()` (unary operators)
  - [x] **Files**: `ori_types/src/infer/expr/operators.rs` — `resolve_binary_op_via_trait()`, `resolve_unary_op_via_trait()`
  - [x] **Files**: `ori_types/src/infer/mod.rs` — `intern_name()` helper on InferEngine

- [x] **Implement**: Operator dispatch in evaluator via trait impls [done] (2026-02-15)
  - [x] **Files**: `ori_eval/src/interpreter/mod.rs` — `eval_binary()`, `binary_op_to_method()`
  - [x] **Files**: `ori_eval/src/methods.rs` — operator methods for primitives
  - [x] **Ori Tests**: `tests/spec/traits/operators/user_defined.ori` — 16 tests (Add, Sub, Neg, Mul, Div, Rem, FloorDiv, BitAnd, BitOr, BitXor, Shl, Shr, BitNot, Not, chaining, double negation)
  - [x] **LLVM Support**: LLVM codegen for operator trait dispatch — Tier 1 (`lower_operators.rs`) and Tier 2 (`arc_emitter.rs`) [done] (2026-02-15)
  - [x] **LLVM Rust Tests**: `ori_llvm/tests/aot/traits.rs` — 7 AOT tests (add, sub, neg, mul_mixed, chained, bitwise, not) [done] (2026-02-15)

- [x] **Implement**: Built-in operator implementations for primitives (NOT trait-based, direct evaluator dispatch) [done] (2026-02-10)
  - [x] `int`: Add, Sub, Mul, Div, FloorDiv, Rem, Neg, BitAnd, BitOr, BitXor, Shl, Shr, BitNot
  - [x] `float`: Add, Sub, Mul, Div, Neg
  - [x] `bool`: Not
  - [x] `str`: Add (concatenation)
  - [x] `list`: Add (concatenation)
  - [x] `Duration`: Add, Sub, Mul (with int), Div (with int), Rem, Neg
  - [x] `Size`: Add, Sub, Mul (with int), Div (with int), Rem
  - [x] **Files**: `ori_eval/src/methods.rs` — `dispatch_int_method()`, `dispatch_float_method()`, etc.

- [x] **Implement**: User-defined operator implementations [done] (2026-02-15)
  - [x] **Ori Tests**: `tests/spec/traits/operators/user_defined.ori` — 16 tests all passing
  - [x] Type checker desugars `a + b` to `a.add(rhs: b)` via TraitRegistry lookup
  - [x] Evaluator dispatches to impl methods for non-primitive types

- [x] **Implement**: Mixed-type operations with explicit both-direction impls [done] (2026-02-10)
  - [x] Example: `Duration * int` and `int * Duration`
  - [x] **Files**: `ori_eval/src/interpreter/mod.rs` — `is_mixed_primitive_op()`

- [x] **Implement**: Error messages for missing operator trait implementations [done] (2026-02-15)
  - [x] E2020: Type does not implement operator trait (for user-defined types)
  - [x] Primitives keep original error messages (e.g., "cannot apply `-` to `str`")
  - [x] **Files**: `ori_types/src/type_error/check_error/mod.rs` — `UnsupportedOperator` variant
  - [x] **Files**: `ori_diagnostic/src/error_code/mod.rs` — E2020 error code
  - [x] **Files**: `ori_diagnostic/src/errors/E2020.md` — error documentation
  - [x] **Ori Compile-Fail Tests**: `tests/compile-fail/operator_trait_missing.ori` — 5 tests (Add, Neg, Not, BitNot, BitAnd)

- [ ] **Implement**: Derive support for operator traits on newtypes (OPTIONAL)
  - [ ] `#derive(Add, Sub, Mul, Div)` generates field-wise operations
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — operator derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/operators/derive.ori`

- [x] **Update Spec**: `09-expressions.md` — Operator Traits section [done] (verified 2026-02-15, already present at line 403 with full trait/method/desugaring tables)
- [x] **Update**: `CLAUDE.md` — operator traits in prelude and operators section [done] (verified 2026-02-15, already in ori-syntax.md lines 93 and 191)
