# Phase 3: Traits and Implementations

**Goal**: Trait-based polymorphism

> **SPEC**: `spec/07-properties-of-types.md`, `spec/08-declarations.md`

---

## PRIORITY NOTE

Per the "Lean Core, Rich Libraries" principle, most built-in functions have been moved
from the compiler core to trait methods. The compiler now only provides:

**Remaining built-ins:**
- `print(msg: str)` - I/O
- `panic(msg: str)` - Control flow

**Moved to traits (must implement in this phase):**
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

**STATUS: ✅ COMPLETE**

Core library traits are implemented via:
1. **Runtime**: Evaluator's `MethodRegistry` provides hardcoded Rust dispatch for methods like `.len()`, `.is_empty()`, `.is_some()`, etc.
2. **Type checking**: `infer_builtin_method()` provides type inference for these methods
3. **Trait bounds**: `primitive_implements_trait()` in `bound_checking.rs` recognizes when types implement these traits

This approach follows the "Lean Core, Rich Libraries" principle — the runtime implementation stays in Rust for efficiency, while the type system recognizes the trait bounds for generic programming.

### 3.0.1 Len Trait ✅

- [x] **Implemented**: Trait bound `Len` recognized for `[T]`, `str`, `{K: V}`, `Set<T>`, `Range<T>`
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — `test_len_bound_satisfied_by_*`
  - [x] **Ori Tests**: `tests/spec/traits/core/len.ori`
- [x] **Implemented**: `.len()` method works on all collection types
  - [x] **Tests**: `ori_eval/src/methods.rs` — list/string/range method tests

### 3.0.2 IsEmpty Trait ✅

- [x] **Implemented**: Trait bound `IsEmpty` recognized for `[T]`, `str`, `{K: V}`, `Set<T>`
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — `test_is_empty_bound_satisfied_by_*`
  - [x] **Ori Tests**: `tests/spec/traits/core/is_empty.ori`
- [x] **Implemented**: `.is_empty()` method works on all collection types
  - [x] **Tests**: `ori_eval/src/methods.rs` — list/string method tests

### 3.0.3 Option Methods ✅

- [x] **Implemented**: `.is_some()`, `.is_none()`, `.unwrap()`, `.unwrap_or()` methods
  - [x] **Rust Tests**: `ori_eval/src/methods.rs` — `option_methods` module
  - [x] **Ori Tests**: `tests/spec/traits/core/option.ori`
- [x] **Type checking**: `infer_builtin_method()` handles Option methods

### 3.0.4 Result Methods ✅

- [x] **Implemented**: `.is_ok()`, `.is_err()`, `.unwrap()` methods
  - [x] **Rust Tests**: `ori_eval/src/methods.rs` — `result_methods` module
  - [x] **Ori Tests**: `tests/spec/traits/core/result.ori`
- [x] **Type checking**: `infer_builtin_method()` handles Result methods

### 3.0.5 Comparable Trait ✅

- [x] **Implemented**: Trait bound `Comparable` recognized for `int`, `float`, `str`, `char`, `Duration`, `Size`
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — `test_comparable_bound_satisfied_by_*`
  - [x] **Ori Tests**: `tests/spec/traits/core/comparable.ori`
- [x] **Type checking**: `int.compare()` and `float.compare()` return `Ordering`

### 3.0.6 Eq Trait ✅

- [x] **Implemented**: Trait bound `Eq` recognized for all primitive types
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — `test_eq_bound_satisfied_by_*`
  - [x] **Ori Tests**: `tests/spec/traits/core/eq.ori`

### Additional Traits ✅

The following traits are also recognized in trait bounds:
- **Clone**: All primitives, collections
- **Hashable**: `int`, `bool`, `str`, `char`, `byte`
- **Default**: `int`, `float`, `bool`, `str`, `Unit`, `Option<T>`
- **Printable**: All primitives

---

## 3.1 Trait Declarations

- [x] **Implement**: Parse `trait Name { ... }` — spec/08-declarations.md § Trait Declarations
  - [x] **Write test**: `tests/spec/traits/declaration.ori`
  - [x] **Run test**: `ori test tests/spec/traits/declaration.ori` (4 tests pass)

- [x] **Implement**: Required method signatures — spec/08-declarations.md § Trait Declarations
  - [x] **Write test**: `tests/spec/traits/declaration.ori`
  - [x] **Run test**: `ori test tests/spec/traits/declaration.ori`

- [x] **Implement**: Default method implementations — spec/08-declarations.md § Trait Declarations
  - [x] **Write test**: `tests/spec/traits/declaration.ori` (test_default_method)
  - [x] **Run test**: `ori test tests/spec/traits/declaration.ori` (5 tests pass)
  - **Note**: Added default trait method dispatch in `module_loading.rs:collect_impl_methods()`

- [x] **Implement**: Associated types — spec/08-declarations.md § Associated Types
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — associated type parsing
  - [x] **Ori Tests**: `tests/spec/traits/associated_types.ori`

- [x] **Implement**: `self` parameter — spec/08-declarations.md § self Parameter
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — self parameter handling
  - [x] **Ori Tests**: `tests/spec/traits/self_param.ori`

- [x] **Implement**: `Self` type reference — spec/08-declarations.md § Self Type
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — Self type resolution
  - [x] **Ori Tests**: `tests/spec/traits/self_type.ori`

- [x] **Implement**: Trait inheritance `trait Child: Parent` — spec/08-declarations.md § Trait Inheritance
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — trait inheritance
  - [x] **Ori Tests**: `tests/spec/traits/inheritance.ori`

---

## 3.2 Trait Implementations

- [x] **Implement**: Inherent impl `impl Type { ... }` — spec/08-declarations.md § Inherent Implementations (PARSING + TYPE CHECK)
  - [x] **Write test**: `tests/spec/traits/declaration.ori` (tests `Widget.get_name()`, `Widget.get_value()`)
  - [x] **Run test**: `ori test tests/spec/traits/declaration.ori`

- [x] **Implement**: Trait impl `impl Trait for Type { ... }` — spec/08-declarations.md § Trait Implementations (PARSING + TYPE CHECK)
  - [x] **Write test**: `tests/spec/traits/declaration.ori` (tests `Widget.greet()`, `Widget.describe()`)
  - [x] **Run test**: `ori test tests/spec/traits/declaration.ori`

- [x] **Implement**: Generic impl `impl<T: Bound> Trait for Container<T>` — spec/08-declarations.md § Generic Implementations (PARSING + TYPE CHECK)
  - [x] **Rust Tests**: Parser tests in `ori_parse/src/grammar/item.rs`
  - [x] **Ori Tests**: `tests/spec/traits/generic_impl.ori` — 4 tests (inherent + trait impls on generic types)
  - **Note**: Added `parse_impl_type()` to handle `Box<T>` syntax in impl blocks. Also added
    `Type::Applied` for tracking instantiated generic types with their type arguments.

- [x] **Implement**: Where clauses — spec/08-declarations.md § Where Clauses (PARSING + TYPE CHECK)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — where clause parsing
  - [x] **Ori Tests**: `tests/spec/traits/declaration.ori` — uses where clauses in trait methods

- [x] **Implement**: Method resolution in type checker — spec/08-declarations.md § Method Resolution
  - `TraitRegistry.lookup_method()` checks inherent impls, then trait impls, then default methods
  - `infer_method_call()` uses trait registry, falls back to built-in methods
  - [x] **Rust Tests**: Covered by existing tests in `typeck/infer/call.rs`
  - [x] **Ori Tests**: `tests/spec/traits/declaration.ori`, `tests/spec/traits/generic_impl.ori`

- [x] **Implement**: User-defined impl method dispatch in evaluator
  - Created `UserMethodRegistry` to store impl method definitions
  - Methods registered via `load_module` -> `register_impl_methods`
  - `eval_method_call` checks user methods first, falls back to built-in
  - Added `self_path` to `ImplDef` AST for type name resolution
  - [x] **Write test**: Rust unit tests in `eval/evaluator.rs` (4 tests covering dispatch, self access, args, fallback)
  - [x] **Run test**: `cargo test --lib eval::evaluator::tests` (all pass)

- [x] **Implement**: Coherence checking — spec/08-declarations.md § Coherence
  - `register_impl` returns `Result<(), CoherenceError>` and checks for conflicts
  - Duplicate trait impls for same type rejected
  - Duplicate inherent methods on same type rejected
  - Multiple inherent impl blocks allowed if methods don't conflict (merged)
  - Added `E2010` error code for coherence violations
  - [x] **Write test**: Rust unit tests in `typeck/type_registry.rs` (3 tests)
  - [x] **Run test**: `cargo test --lib typeck::type_registry::tests` (all pass)

---

## 3.3 Trait Bounds

**Complete Implementation:**
- [x] Parser supports generic parameters with bounds `<T: Trait>`, `<T: A + B>`
- [x] Parser supports where clauses `where T: Clone, U: Default`
- [x] `Function` AST node stores `generics: GenericParamRange` and `where_clauses: Vec<WhereClause>`
- [x] `FunctionType` in type checker stores `generics: Vec<GenericBound>` with bounds and type vars
- [x] `Param` AST node stores `type_name: Option<Name>` to preserve type annotation names
- [x] `parse_type_with_name()` captures identifier names during parameter type parsing
- [x] `infer_function_signature` creates fresh type vars for generics and maps params correctly
- [x] `function_sigs: HashMap<Name, FunctionType>` stores signatures for call-time lookup
- [x] `check_generic_bounds()` verifies resolved types satisfy trait bounds at call sites
- [x] E2009 error code for missing trait bound violations
- [x] Unit tests verify end-to-end (10 tests in `typeck::checker::tests`)

**What Works Now:**
- Parsing generic functions: `@compare<T: Comparable> (a: T, b: T) -> Ordering`
- Parsing multiple bounds: `@process<T: Eq + Clone> (x: T) -> T`
- Parsing where clauses: `@transform<T> (x: T) -> T where T: Clone = x`
- Constraint satisfaction checking at call sites
- Error messages when types don't satisfy required bounds

**Implementation Details:**
- `Param.type_name` preserves the original type annotation name (e.g., `T` in `: T`)
- `GenericBound.type_var` stores the fresh type variable for each generic parameter
- `infer_function_signature` builds a `generic_type_vars: HashMap<Name, Type>` mapping
- When a param's `type_name` matches a generic, the type var is used instead of inferring
- `check_generic_bounds` in `call.rs` resolves type vars after unification and checks bounds
- `type_satisfies_bound` uses `TraitRegistry` to verify trait implementations

- [x] **Implement**: Single bound `<T: Trait>` — spec/08-declarations.md § Generic Declarations
  - [x] **Write test**: Rust unit tests in `typeck/checker.rs::tests`
  - [x] **Run test**: `cargo test --lib typeck::checker::tests` (10 tests pass)

- [x] **Implement**: Multiple bounds `<T: A + B>` — spec/08-declarations.md § Generic Declarations
  - [x] **Write test**: `test_multiple_bounds_parsing` in Rust unit tests
  - [x] **Run test**: `cargo test --lib typeck::checker::tests` (all pass)

- [x] **Implement**: Constraint satisfaction checking — spec/07-properties-of-types.md § Trait Bounds
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 10+ constraint satisfaction tests
  - [ ] **Ori Tests**: `tests/spec/traits/bounds.ori`

---

## 3.4 Associated Types

**STATUS: ✅ COMPLETE (2026-01-25)**

Infrastructure implemented:
- `ParsedType::AssociatedType` variant in `ori_ir/src/parsed_type.rs`
- `Type::Projection` variant in `ori_types/src/lib.rs`
- Parser handles `Self.Item` and `T.Item` syntax in type positions
- `ImplAssocType` for associated type definitions in impl blocks
- `ImplEntry.assoc_types` stores associated type definitions
- `TraitRegistry.lookup_assoc_type()` resolves associated types

- [x] **Implement**: Associated type declarations — spec/08-declarations.md § Associated Types
  - [x] **Rust Tests**: `ori_parse/src/grammar/ty.rs` — associated type parsing tests
  - [x] **Ori Tests**: `tests/spec/traits/associated_types.ori`

- [x] **Implement**: Constraints `where T.Item: Eq` — spec/08-declarations.md § Where Clauses
  - [x] **Rust Tests**: Parser/type checker support in `bound_checking.rs`
  - [x] **Ori Tests**: `tests/spec/traits/associated_types.ori` — `test_fnbox_fails_eq_constraint`
  - **Note**: Added `WhereConstraint` struct with projection support. Parser handles `where C.Item: Eq`.
    Bound checking resolves associated types via `lookup_assoc_type_by_name()`.

- [x] **Implement**: Impl validation (require all associated types defined)
  - [x] **Rust Tests**: `oric/src/typeck/checker/trait_registration.rs` — `validate_associated_types`
  - [x] **Ori Tests**: `tests/compile-fail/impl_missing_assoc_type.ori`
  - **Note**: Added validation in `register_impls()` that checks all required associated types are defined.

---

## 3.5 Derive Traits

**STATUS: ✅ COMPLETE (2026-01-25)**

All 5 derive traits implemented in `oric/src/typeck/derives/mod.rs`.
Tests at `tests/spec/traits/derive/all_derives.ori` (7 tests pass).

- [x] **Implement**: Auto-implement `Eq` — spec/08-declarations.md § Attributes
  - [x] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — `test_process_struct_derives`
  - [x] **Ori Tests**: `tests/spec/traits/derive/all_derives.ori`

- [x] **Implement**: Auto-implement `Clone` — spec/08-declarations.md § Attributes
  - [x] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — `test_process_multiple_derives`
  - [x] **Ori Tests**: `tests/spec/traits/derive/all_derives.ori`

- [x] **Implement**: Auto-implement `Hashable` — spec/08-declarations.md § Attributes
  - [x] **Rust Tests**: `oric/src/typeck/derives/mod.rs`
  - [x] **Ori Tests**: `tests/spec/traits/derive/all_derives.ori`

- [x] **Implement**: Auto-implement `Printable` — spec/08-declarations.md § Attributes
  - [x] **Rust Tests**: `oric/src/typeck/derives/mod.rs`
  - [x] **Ori Tests**: `tests/spec/traits/derive/all_derives.ori`

- [x] **Implement**: Auto-implement `Default` — spec/08-declarations.md § Attributes
  - [x] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — `create_derived_method_def` handles Default
  - [ ] **Ori Tests**: `tests/spec/traits/derive/default.ori` — test file not yet created

---

## 3.6 Phase Completion Checklist

- [x] Core library traits (3.0): All complete ✅
- [x] Trait declarations (3.1): All complete ✅
- [x] Trait implementations (3.2): All complete ✅
- [x] Trait bounds (3.3): All complete ✅
- [x] Associated types (3.4): All complete ✅ (2026-01-25)
- [x] Derive traits (3.5): All 5 derives implemented ✅
- [x] 148 trait tests pass
- [x] Run full test suite: `./test-all`

**Exit Criteria**: Trait-based code compiles and runs ✅

**Phase 3 Complete** (2026-01-25)

---

## 3.7 Clone Trait Formal Definition

**Proposal**: `proposals/approved/clone-trait-proposal.md`

Formalizes the `Clone` trait that enables explicit value duplication. The trait is already recognized in trait bounds and derivable, but this proposal adds the formal definition and comprehensive prelude implementations.

### Implementation

- [ ] **Implement**: Formal `Clone` trait definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — clone trait parsing
  - [ ] **Ori Tests**: `tests/spec/traits/clone/definition.ori`

- [ ] **Implement**: Clone implementations for all primitives (int, float, bool, str, char, byte, Duration, Size)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — primitive clone bounds
  - [ ] **Ori Tests**: `tests/spec/traits/clone/primitives.ori`

- [ ] **Implement**: Clone implementations for collections ([T], {K: V}, Set<T>) with element-wise cloning
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — collection clone bounds
  - [ ] **Ori Tests**: `tests/spec/traits/clone/collections.ori`

- [ ] **Implement**: Clone implementations for Option<T> and Result<T, E>
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — option/result clone
  - [ ] **Ori Tests**: `tests/spec/traits/clone/wrappers.ori`

- [ ] **Implement**: Clone implementations for tuples (all arities)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — tuple clone bounds
  - [ ] **Ori Tests**: `tests/spec/traits/clone/tuples.ori`

- [ ] **Update Spec**: `06-types.md` — add Clone trait section
- [ ] **Update Spec**: `12-modules.md` — update prelude traits description
- [ ] **Update**: `CLAUDE.md` — add Clone documentation to quick reference

---

## 3.8 Iterator Traits

**Proposal**: `proposals/approved/iterator-traits-proposal.md`

Formalizes iteration with four core traits: `Iterator`, `DoubleEndedIterator`, `Iterable`, and `Collect`. Enables generic programming over any iterable, user types participating in `for` loops, and transformation methods.

### Implementation

- [ ] **Implement**: `Iterator` trait with functional `next()` returning `(Option<Self.Item>, Self)`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — iterator trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/iterator.ori`
  - [ ] **LLVM Support**: LLVM codegen for iterator trait methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: `DoubleEndedIterator` trait with `next_back()` method
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — double-ended iterator bounds
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/double_ended.ori`
  - [ ] **LLVM Support**: LLVM codegen for double-ended iterator methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: `Iterable` trait with `iter()` method
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — iterable trait bounds
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/iterable.ori`
  - [ ] **LLVM Support**: LLVM codegen for iterable trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: `Collect` trait with `from_iter()` method
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — collect trait bounds
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/collect.ori`
  - [ ] **LLVM Support**: LLVM codegen for collect trait
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: Iterator default methods (map, filter, fold, find, collect, count, any, all, take, skip, enumerate, zip, chain, flatten, flat_map, cycle)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — default method type inference
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/methods.ori`
  - [ ] **LLVM Support**: LLVM codegen for all iterator methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

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
  - [ ] `[T]` implements `Iterable`, `DoubleEndedIterator`, `Collect`
  - [ ] `{K: V}` implements `Iterable` (NOT double-ended — unordered)
  - [ ] `Set<T>` implements `Iterable`, `Collect` (NOT double-ended — unordered)
  - [ ] `str` implements `Iterable`, `DoubleEndedIterator`
  - [ ] `Range<int>` implements `Iterable`, `DoubleEndedIterator`
  - [ ] `Option<T>` implements `Iterable`
  - [ ] **Note**: `Range<float>` does NOT implement `Iterable` (precision issues)
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/builtin_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for all builtin iterator impls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: Helper iterator types (ListIterator, RangeIterator, MapIterator, FilterIterator, RevIterator, CycleIterator, etc.)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — helper type inference
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/helper_types.ori`
  - [ ] **LLVM Support**: LLVM codegen for all helper iterator types
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/iterator_tests.rs`

- [ ] **Implement**: Fused iterator guarantee (once None, always None)
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — fused behavior tests
  - [ ] **Ori Tests**: `tests/spec/traits/iterator/fused.ori`
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

- [ ] **Implement**: Diamond problem resolution — single impl satisfies all inheritance paths
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — diamond inheritance tests
  - [ ] **Ori Tests**: `tests/spec/traits/resolution/diamond.ori`

- [ ] **Implement**: Conflicting default detection — error when multiple supertraits provide conflicting defaults
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — conflicting default tests
  - [ ] **Ori Tests**: `tests/spec/traits/resolution/conflicting_defaults.ori`

- [ ] **Implement**: Coherence/orphan rules — at least one of trait or type must be local
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — orphan rule tests
  - [ ] **Ori Tests**: `tests/compile-fail/orphan_impl.ori`

- [ ] **Implement**: Blanket impl restrictions — orphan rules for `impl<T> Trait for T`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — blanket impl tests
  - [ ] **Ori Tests**: `tests/compile-fail/orphan_blanket.ori`

- [ ] **Implement**: Method resolution order — Inherent > Trait > Extension priority
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — resolution order tests
  - [ ] **Ori Tests**: `tests/spec/traits/resolution/method_priority.ori`

- [ ] **Implement**: Ambiguous method detection with fully-qualified syntax
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — ambiguous method tests
  - [ ] **Ori Tests**: `tests/spec/traits/resolution/fully_qualified.ori`

- [ ] **Implement**: Super trait calls with `Trait.method(self)` syntax
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — super call tests
  - [ ] **Ori Tests**: `tests/spec/traits/resolution/super_calls.ori`

- [ ] **Implement**: Extension method conflict detection (including re-exports)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — extension conflict tests
  - [ ] **Ori Tests**: `tests/compile-fail/extension_conflict.ori`

- [ ] **Implement**: Associated type disambiguation with `Type::Trait::AssocType` syntax
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — associated type disambiguation
  - [ ] **Ori Tests**: `tests/spec/traits/resolution/assoc_type_disambiguation.ori`

- [ ] **Implement**: Implementation specificity (Concrete > Constrained > Generic)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — specificity tests
  - [ ] **Ori Tests**: `tests/spec/traits/resolution/specificity.ori`

- [ ] **Implement**: Overlapping impl detection — compile error for equal-specificity impls
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — overlap detection tests
  - [ ] **Ori Tests**: `tests/compile-fail/overlapping_impls.ori`

- [ ] **Implement**: Error messages (E0600-E0603)
  - [ ] E0600: Conflicting implementations
  - [ ] E0601: Orphan implementation
  - [ ] E0602: Ambiguous method call
  - [ ] E0603: Conflicting extension methods

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

**Proposal**: `proposals/approved/custom-subscripting-proposal.md`

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

- [ ] **Implement**: Built-in `Index` implementations for `[T]`, `{K: V}`, `str`
  - [ ] `[T]` implements `Index<int, T>` (panics on out-of-bounds)
  - [ ] `{K: V}` implements `Index<K, Option<V>>`
  - [ ] `str` implements `Index<int, str>` (single codepoint, panics on out-of-bounds)
  - [ ] **Ori Tests**: `tests/spec/traits/index/builtin_impls.ori`
  - [ ] **LLVM Support**: LLVM codegen for builtin Index impls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/index_tests.rs`

- [ ] **Implement**: Ambiguity detection when key type is ambiguous
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — ambiguous key type detection
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/index_ambiguous_key.ori`

- [ ] **Update Spec**: `09-expressions.md` — document Index trait and desugaring
- [ ] **Update Spec**: `06-types.md` — add Index trait to prelude section
- [ ] **Update**: `CLAUDE.md` — add Index trait to prelude and subscripting documentation

---

## 3.13 Additional Core Traits

**Proposal**: `proposals/approved/additional-traits-proposal.md`

Formalizes three core traits: `Printable`, `Default`, and `Traceable`. The `Iterable`, `Iterator`, `DoubleEndedIterator`, and `Collect` traits are already defined in the spec and implemented in Phase 3.8.

### Implementation

- [ ] **Implement**: `Printable` trait formal definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — printable trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/printable/definition.ori`

- [ ] **Implement**: Printable derivation with `Point(1, 2)` format (type name + values)
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — printable derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/printable/derive.ori`

- [ ] **Implement**: `Default` trait formal definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — default trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/default/definition.ori`

- [ ] **Implement**: Default derivation for structs only (error on sum types)
  - [ ] **Rust Tests**: `oric/src/typeck/derives/mod.rs` — default derive tests
  - [ ] **Ori Tests**: `tests/spec/traits/default/derive.ori`
  - [ ] **Ori Compile-Fail Tests**: `tests/compile-fail/default_sum_type.ori`

- [ ] **Implement**: `Traceable` trait formal definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — traceable trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/traceable/definition.ori`

- [ ] **Implement**: Traceable for Error type with trace storage
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — error trace evaluation
  - [ ] **Ori Tests**: `tests/spec/traits/traceable/error.ori`

- [ ] **Implement**: Traceable delegation for Result<T, E: Traceable>
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — result traceable bounds
  - [ ] **Ori Tests**: `tests/spec/traits/traceable/result.ori`

- [ ] **Implement**: Error messages (E1040, E1042)
  - [ ] E1040: Missing Printable for string interpolation
  - [ ] E1042: Cannot derive Default for sum type

- [ ] **Update Spec**: `07-properties-of-types.md` — add Printable, Default, Traceable sections (DONE)
- [ ] **Update**: `CLAUDE.md` — ensure traits documented in quick reference

---

## 3.14 Comparable and Hashable Traits

**Proposal**: `proposals/approved/comparable-hashable-traits-proposal.md`

Formalizes the `Comparable` and `Hashable` traits with complete definitions, mathematical invariants, standard implementations, and derivation rules. Adds `Result<T, E>` to both trait implementations and introduces `hash_combine` as a prelude function.

### Implementation

- [ ] **Implement**: Formal `Comparable` trait definition in type system
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — comparable trait parsing/bounds
  - [ ] **Ori Tests**: `tests/spec/traits/comparable/definition.ori`

- [ ] **Implement**: Comparable implementations for all primitives (int, float, bool, str, char, byte, Duration, Size)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — primitive comparable bounds
  - [ ] **Ori Tests**: `tests/spec/traits/comparable/primitives.ori`
  - [ ] **LLVM Support**: LLVM codegen for primitive compare methods
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: Comparable implementations for collections ([T], tuples)
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — collection comparable bounds
  - [ ] **Ori Tests**: `tests/spec/traits/comparable/collections.ori`
  - [ ] **LLVM Support**: LLVM codegen for collection compare
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: Comparable implementations for Option<T> and Result<T, E>
  - [ ] Option: `None < Some(_)`
  - [ ] Result: `Ok(_) < Err(_)`, then compare inner values
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — option/result comparable
  - [ ] **Ori Tests**: `tests/spec/traits/comparable/wrappers.ori`
  - [ ] **LLVM Support**: LLVM codegen for option/result compare
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/comparable_tests.rs`

- [ ] **Implement**: Float IEEE 754 total ordering (NaN handling)
  - [ ] **Rust Tests**: `oric/src/eval/tests/` — float comparison edge cases
  - [ ] **Ori Tests**: `tests/spec/traits/comparable/float_nan.ori`
  - [ ] **LLVM Support**: LLVM codegen for float total ordering
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
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — collection hashable bounds
  - [ ] **Ori Tests**: `tests/spec/traits/hashable/collections.ori`
  - [ ] **LLVM Support**: LLVM codegen for collection hash
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/hashable_tests.rs`

- [ ] **Implement**: Hashable implementations for Option<T> and Result<T, E>
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — option/result hashable
  - [ ] **Ori Tests**: `tests/spec/traits/hashable/wrappers.ori`
  - [ ] **LLVM Support**: LLVM codegen for option/result hash
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
