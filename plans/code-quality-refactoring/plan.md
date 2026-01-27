# Code Quality Refactoring Roadmap — Execution Plan

## How to Use This Plan

### Prerequisites

Before starting:
1. Familiarize yourself with `CLAUDE.md` (project overview)
2. Review `docs/compiler/design/appendices/E-coding-guidelines.md` (coding standards)
3. Ensure `cargo t` passes (all Rust unit tests)
4. Ensure `cargo st` passes (all Ori language tests)

### Execution Rules

1. **Follow phase order strictly** — Dependencies are encoded in the numbering
2. **Within each phase**, complete sections in order (X.1 → X.2 → ...)
3. **Within each section**, complete items top to bottom
4. **Each item requires**: Implementation → Tests → Verification
5. **Do not skip phases** unless marked complete or explicitly skipped
6. **Run tests after each change** — `cargo t` for Rust, `cargo st` for Ori

### Item Structure

```markdown
- [ ] **Implement**: [description] — [file:line reference]
  - [ ] **Verify**: [verification criteria]
  - [ ] **Test**: `cargo test --lib -- test_filter`
```

### Updating Progress

- Check boxes as items complete: `[ ]` → `[x]`
- Run full test suite after completing each section
- Document any deviations or issues encountered

---

## Phase Execution Order

### Tier 1: Low-Risk High-Impact (Start Here)

| Order | Phase | Focus | Risk |
|-------|-------|-------|------|
| 1 | Phase 1 | TypeChecker Builder Pattern | Low |
| 2 | Phase 2 | RAII Guards for Context | Low |
| 3 | Phase 3 | Standard Traits for Value | Low |

### Tier 2: Medium Complexity

| Order | Phase | Focus | Risk |
|-------|-------|-------|------|
| 4 | Phase 4 | Unified Registry Key Types | Low |
| 5 | Phase 5 | Extract Type Conversion Logic | Medium |

### Tier 3: Higher Complexity

| Order | Phase | Focus | Risk |
|-------|-------|-------|------|
| 6 | Phase 6 | TypeChecker Component Extraction | Medium |
| 7 | Phase 7 | Method Dispatch Chain of Responsibility | Medium |

---

## Phase 1: TypeChecker Builder Pattern ✅ COMPLETED

**Goal**: Eliminate constructor duplication in TypeChecker by introducing a builder pattern.

**Implementation**:
- Created `compiler/oric/src/typeck/checker/builder.rs` with `TypeCheckerBuilder`
- Refactored all 4 constructors to delegate to builder
- Added 5 builder tests (default, with_source, with_context, with_diagnostic_config, combined)
- All tests pass: `cargo t && cargo st`

### 1.1 Create TypeCheckerBuilder

- [x] **Implement**: Create `TypeCheckerBuilder` struct in new file `compiler/oric/src/typeck/checker/builder.rs`
  - [x] Define builder struct with required fields (`arena`, `interner`) and optional fields
  - [x] Implement `new()` constructor taking required references
  - [x] Add `#[must_use]` chainable setter methods: `with_source()`, `with_context()`, `with_diagnostic_config()`
  - [x] Implement `build()` method that constructs `TypeChecker` with defaults
  - [x] **Verify**: Builder compiles and matches EvaluatorBuilder pattern
  - [x] **Test**: `cargo test --lib -- typeck::checker::builder`

### 1.2 Integrate Builder into mod.rs

- [x] **Implement**: Add `mod builder;` and `pub use builder::TypeCheckerBuilder;` to `compiler/oric/src/typeck/checker/mod.rs`
  - [x] **Verify**: Module compiles without errors

### 1.3 Refactor Constructors to Use Builder

- [x] **Implement**: Refactor `TypeChecker::new()` — now delegates to builder
- [x] **Implement**: Refactor `TypeChecker::with_source()` — now delegates to builder
- [x] **Implement**: Refactor `TypeChecker::with_context()` — now delegates to builder
- [x] **Implement**: Refactor `TypeChecker::with_source_and_config()` — now delegates to builder

### 1.4 Add Builder Tests

- [x] **Implement**: Add comprehensive builder tests (5 tests in builder.rs)

### 1.5 Final Verification

- [x] **Verify**: Run full test suite: `cargo t && cargo st` — all pass

---

## Phase 2: RAII Guards for Context ✅ COMPLETED

**Goal**: Replace manual save/restore patterns with RAII guards to prevent bugs from forgotten restores.

**Implementation**:
- Created `compiler/oric/src/typeck/checker/scope_guards.rs`
- Implemented closure-based scope methods: `with_capability_scope()`, `with_empty_capability_scope()`, `with_impl_scope()`
- Refactored `check_function`, `check_test`, `check_impl_methods` to use scope methods
- Removed old `enter_impl`/`exit_impl` methods
- Added 4 scope guard tests
- All tests pass: `cargo t && cargo st`

### 2.1-2.3 Create and Integrate Scope Guards

- [x] Created `scope_guards.rs` with closure-based scope management
- [x] Implemented `with_capability_scope()` and `with_impl_scope()` methods on TypeChecker
- [x] Added module to mod.rs

### 2.4-2.6 Refactor Methods

- [x] Refactored `check_function` to use `with_capability_scope()`
- [x] Refactored `check_test` to use `with_empty_capability_scope()`
- [x] Refactored `check_impl_methods` to use `with_impl_scope()`
- [x] Removed unused `enter_impl`/`exit_impl` methods

### 2.7-2.8 Tests and Verification

- [x] Added 4 tests for scope guards in scope_guards.rs
- [x] All tests pass: `cargo t && cargo st`

---

## Phase 3: Standard Traits for Value ✅ COMPLETED

**Goal**: Implement `Eq` and `Hash` traits for `Value` to enable use in collections and eliminate duplicate helper functions.

**Implementation**:
- Added `impl Eq for Value {}` in `compiler/ori_patterns/src/value/mod.rs`
- Added `impl Hash for Value` with proper discriminant handling
- Updated `PartialEq` to include `Struct` and `Map` variants
- Deleted duplicate `values_equal()` and `hash_value()` functions from method_dispatch.rs
- Refactored usages to use standard traits (`sv != ov`, `val.hash(&mut hasher)`)
- Added 4 hash tests (consistency, HashSet usage, HashMap key usage, different types)
- All tests pass: `cargo t && cargo st`

### 3.1-3.2 Implement Eq and Hash

- [x] Added `impl Eq for Value {}`
- [x] Added `impl Hash for Value` with discriminant-based hashing
- [x] Updated `PartialEq` to handle `Struct` and `Map` variants

### 3.3-3.5 Tests and Refactoring

- [x] Added 4 hash tests in value/mod.rs
- [x] Replaced `values_equal(sv, ov)` with `sv != ov` in eval_derived_eq
- [x] Replaced `hash_value(val, &mut hasher)` with `val.hash(&mut hasher)` in eval_derived_hash
- [x] Deleted `values_equal()` function (~45 lines)
- [x] Deleted `hash_value()` function (~55 lines)

### 3.6 Final Verification

- [x] All tests pass: `cargo t && cargo st`
- [x] Value can now be used in `HashSet<Value>` and `HashMap<Value, _>`

---

## Phase 4: Unified Registry Key Types ✅ COMPLETED

**Goal**: Create consistent key types across registries to improve type safety and reduce string allocations.

**Implementation**:
- Created `compiler/ori_eval/src/method_key.rs` with `MethodKey` newtype
- Refactored `UserMethodRegistry` to use `MethodKey` instead of `(String, String)` tuples
- Updated all registry methods: `register`, `register_derived`, `lookup`, `lookup_derived`, `lookup_any`, `has_method`, `all_methods`, `all_derived_methods`
- Added 4 MethodKey tests
- All tests pass: `cargo t && cargo st`

### 4.1-4.2 Create and Integrate MethodKey

- [x] Created `MethodKey` struct with `new()`, `from_strs()`, and `Display`
- [x] Added derives: `Clone, Eq, PartialEq, Hash, Debug`
- [x] Added module and re-export to lib.rs

### 4.3-4.4 Refactor Registry

- [x] Changed `methods: HashMap<(String, String), UserMethod>` to `HashMap<MethodKey, UserMethod>`
- [x] Changed `derived_methods: HashMap<(String, String), DerivedMethodInfo>` to `HashMap<MethodKey, DerivedMethodInfo>`
- [x] Updated all registry methods to use `MethodKey`
- [x] No changes needed in callers (API unchanged, internal representation changed)

### 4.5 Final Verification

- [x] All tests pass: `cargo t && cargo st`
- [x] No raw tuple keys remain in UserMethodRegistry

---

## Phase 5: Extract Type Conversion Logic ✅ COMPLETED

**Goal**: Extract repeated type conversion patterns in TypeChecker into dedicated helper methods.

**Implementation**:
- Created `resolve_parsed_type_internal()` as unified internal method for type resolution
- Created `resolve_well_known_generic()` helper for Option, Result, Set, Range, Channel
- Created `make_projection_type()` helper for associated type resolution (renamed from `resolve_associated_type` due to existing method conflict in bound_checking.rs)
- Consolidated `parsed_type_to_type()` and `resolve_parsed_type_with_generics()` to use shared internal logic
- All tests pass: `cargo t && cargo st`

### 5.1 Identify Common Patterns

- [x] Analyzed repeated patterns in type conversion
- [x] Identified generic type handling, function type handling, associated type handling

### 5.2 Extract Generic Type Resolution

- [x] Created `resolve_well_known_generic()` helper method
- [x] Handles Option, Result, Set, Range, Channel with arity checking

### 5.3 Extract Associated Type Resolution

- [x] Created `make_projection_type()` helper method
- [x] Creates `Type::Projection` from `ParsedType::AssociatedType`

### 5.4 Simplify Main Methods

- [x] Created `resolve_parsed_type_internal()` as core implementation
- [x] `parsed_type_to_type()` now delegates to internal method
- [x] `resolve_parsed_type_with_generics()` now delegates to internal method with generics map

### 5.5 Final Verification

- [x] All tests pass: `cargo t && cargo st`
- [x] Type conversion logic consolidated into shared helpers

---

## Phase 6: TypeChecker Component Extraction ✅ COMPLETED

**Goal**: Split TypeChecker's 18 fields into logical component structs for better organization and testability.

**Implementation**:
- Created `compiler/oric/src/typeck/checker/components.rs`
- Defined component structs: `CheckContext`, `InferenceState`, `Registries`, `DiagnosticState`, `ScopeContext`
- Updated TypeChecker to use 5 component fields instead of 18 flat fields
- Updated TypeCheckerBuilder to construct component structs
- Migrated 420+ field accesses across checker submodules and infer module
- Updated scope_guards.rs to use component-based access
- All tests pass: `cargo t && cargo st`

### 6.1 Create CheckContext

- [x] Created `CheckContext<'a>` with `arena` and `interner` references
- [x] Implemented `new()` constructor

### 6.2 Create InferenceState

- [x] Created `InferenceState` with `ctx`, `env`, `base_env`, `expr_types`
- [x] Implemented `Default` and `new()`

### 6.3 Create Registries Bundle

- [x] Created `Registries` with `pattern`, `type_op`, `types`, `traits`
- [x] Implemented `Default`, `new()`, and `with_pattern_registry()`

### 6.4 Create DiagnosticState

- [x] Created `DiagnosticState` with `errors`, `queue`, `source`
- [x] Implemented `Default`, `new()`, and `with_source()`

### 6.5 Create ScopeContext

- [x] Created `ScopeContext` with `function_sigs`, `current_impl_self`, `config_types`, `current_function_caps`, `provided_caps`
- [x] Implemented `Default` and `new()`

### 6.6 Update TypeChecker

- [x] Refactored TypeChecker to use 5 component fields: `context`, `inference`, `registries`, `diagnostics`, `scope`
- [x] Updated builder.rs to construct components
- [x] Migrated all field accesses (e.g., `self.arena` → `self.context.arena`)

### 6.7 Full Migration

- [x] Updated all field accesses across checker submodules
- [x] Updated all field accesses in infer module
- [x] Updated scope_guards.rs for component-based access
- [x] Updated tests using old field names

---

## Phase 7: Method Dispatch Chain of Responsibility ✅ COMPLETED

**Goal**: Replace cascading if-else in method dispatch with extensible Chain of Responsibility pattern.

**Implementation**:
- Created `compiler/oric/src/eval/evaluator/resolvers/` module with:
  - `mod.rs` - MethodResolver trait, MethodDispatcher, MethodResolution enum, CollectionMethod enum
  - `user.rs` - UserMethodResolver (priority 0)
  - `derived.rs` - DerivedMethodResolver (priority 1)
  - `collection.rs` - CollectionMethodResolver (priority 2)
  - `builtin.rs` - BuiltinMethodResolver (priority 3)
- Implemented MethodDispatcher that chains resolvers in priority order
- Integrated dispatcher into Evaluator's method_dispatch.rs
- Refactored collection method evaluation into typed individual methods
- Added MapMethods dispatcher to MethodRegistry for map `len` and `is_empty` support
- All tests pass: `cargo t && cargo st`

### 7.1 Define Resolution Types

- [x] Created `MethodResolution` enum with variants: `User`, `Derived`, `Collection`, `Builtin`, `NotFound`
- [x] Created `CollectionMethod` enum with all method types (Map, Filter, Fold, Find, Collect, MapEntries, FilterEntries, Any, All)
- [x] Implemented `CollectionMethod::from_name()` helper

### 7.2 UserMethodResolver

- [x] Created `user.rs` with `UserMethodResolver` struct
- [x] Implements `MethodResolver` trait with priority 0 (highest)
- [x] Looks up methods from `SharedRegistry<UserMethodRegistry>`

### 7.3 DerivedMethodResolver

- [x] Created `derived.rs` with `DerivedMethodResolver` struct
- [x] Implements `MethodResolver` trait with priority 1
- [x] Looks up derived methods from `SharedRegistry<UserMethodRegistry>`

### 7.4 CollectionMethodResolver

- [x] Created `collection.rs` with `CollectionMethodResolver` struct
- [x] Implements `MethodResolver` trait with priority 2
- [x] Identifies collection methods based on receiver type (List, Range, Map)

### 7.5 BuiltinMethodResolver

- [x] Created `builtin.rs` with `BuiltinMethodResolver` struct
- [x] Implements `MethodResolver` trait with priority 3 (lowest)
- [x] Acts as fallback, delegates to MethodRegistry

### 7.6 MethodDispatcher

- [x] Created `MethodDispatcher` struct that chains resolvers
- [x] Sorts resolvers by priority on construction
- [x] Iterates through resolvers until one returns non-NotFound result

### 7.7 Integration

- [x] Integrated `resolve_method()` into Evaluator's method dispatch
- [x] Updated `eval_method_call` to use resolver-based dispatch
- [x] Refactored collection method evaluation into typed methods

### 7.8 Tests

- [x] Added test for `CollectionMethod::from_name()`
- [x] Added test for dispatcher priority ordering
- [x] Added tests for each resolver type
- [x] All tests pass: `cargo t && cargo st`

---

## Running Tests

```bash
# Rust unit tests (all workspace crates)
cargo t

# Ori language tests (all)
cargo st

# Filter by test pattern (use --lib for library tests)
cargo test --lib -- typeck::checker::tests   # TypeChecker tests (82 tests)
cargo test --lib -- eval::evaluator::tests   # Evaluator tests
cargo test --lib -- user_methods::tests      # UserMethodRegistry tests
cargo test --lib -- value::tests             # Value tests

# Single test by name
cargo test --lib -- test_associated_type_declaration

# Full test suite verification
cargo t && cargo st
```

---

## Phase Dependencies

```
Can start immediately:
  - Phase 1 (TypeChecker Builder) — no dependencies
  - Phase 3 (Standard Traits for Value) — no dependencies

After Phase 1:
  - Phase 2 (RAII Guards) — builds on cleaner TypeChecker structure
  - Phase 6 (Component Extraction) — requires builder for reconstruction

After Phase 3:
  - Phase 4 (Unified Registry Keys) — independent but similar pattern

After Phase 4:
  - Phase 7 (Method Dispatch Chain) — uses consistent key types

After Phase 1, 2, 4:
  - Phase 5 (Type Conversion) — cleaner with builder, guards, and keys

After Phase 6:
  - Can add new components more easily
```

### Recommended Parallel Execution

If working with multiple developers:
- **Developer A**: Phase 1 → Phase 2 → Phase 6
- **Developer B**: Phase 3 → Phase 4 → Phase 7
- **Developer C**: Phase 5 (after Phase 1 complete)

---

## Risk Assessment

| Phase | Risk | Impact | Mitigation |
|-------|------|--------|------------|
| 1: Builder Pattern | Low | Low | Builder is additive; old constructors can delegate |
| 2: RAII Guards | Low | Medium | Guards are additive; can be adopted incrementally |
| 3: Value Traits | Low | Medium | Traits are additive; existing code unchanged |
| 4: Registry Keys | Low | Low | Newtype wrapper; straightforward search-replace |
| 5: Type Conversion | Medium | Low | Refactoring internal methods; good test coverage required |
| 6: Component Extraction | Medium | High | Major restructuring; requires careful field migration |
| 7: Method Dispatch Chain | Medium | Medium | New abstraction layer; requires thorough testing |

### Rollback Strategy

Each phase can be rolled back independently:
1. **Phase 1**: Remove builder, restore direct construction
2. **Phase 2**: Remove guards, restore manual save/restore
3. **Phase 3**: Remove Eq/Hash, restore helper functions
4. **Phase 4**: Revert to tuple keys
5. **Phase 5**: Inline helper methods back
6. **Phase 6**: Flatten component structs back to fields
7. **Phase 7**: Replace dispatcher with cascading if-else

---

## Code Quality Metrics (Achieved)

**All 7 phases fully completed.**

| Metric | Before | After | Status |
|--------|--------|-------|--------|
| TypeChecker constructors | 4 (duplicated) | 1 builder + build() | ✅ Done |
| Manual save/restore sites | 3+ | 0 (closure-based scopes) | ✅ Done |
| Duplicate equality functions | 2 | 0 (PartialEq trait) | ✅ Done |
| Duplicate hash functions | 1 | 0 (Hash trait) | ✅ Done |
| Duplicate type conversion | 2 methods | 1 internal + helpers | ✅ Done |
| TypeChecker fields | 18 flat | 5 component structs | ✅ Done |
| Method dispatch branches | 4 cascading | Chain of Responsibility | ✅ Done |

**Lines removed**: ~100+ lines of duplicate code (values_equal, hash_value functions)

**New files created**:
- `compiler/oric/src/typeck/checker/builder.rs` - TypeCheckerBuilder
- `compiler/oric/src/typeck/checker/scope_guards.rs` - Closure-based scope management
- `compiler/oric/src/typeck/checker/components.rs` - Component struct definitions
- `compiler/ori_eval/src/method_key.rs` - MethodKey newtype
- `compiler/oric/src/eval/evaluator/resolvers/mod.rs` - MethodResolver trait, MethodDispatcher
- `compiler/oric/src/eval/evaluator/resolvers/user.rs` - UserMethodResolver
- `compiler/oric/src/eval/evaluator/resolvers/derived.rs` - DerivedMethodResolver
- `compiler/oric/src/eval/evaluator/resolvers/collection.rs` - CollectionMethodResolver
- `compiler/oric/src/eval/evaluator/resolvers/builtin.rs` - BuiltinMethodResolver

---

## References

### Files Modified

| Phase | Primary Files |
|-------|---------------|
| 1 | `compiler/oric/src/typeck/checker/mod.rs`, `builder.rs` |
| 2 | `compiler/oric/src/typeck/checker/mod.rs`, `scope_guards.rs` |
| 3 | `compiler/ori_patterns/src/value/mod.rs`, `compiler/oric/src/eval/evaluator/method_dispatch.rs` |
| 4 | `compiler/ori_eval/src/user_methods.rs`, `method_key.rs` |
| 5 | `compiler/oric/src/typeck/checker/mod.rs` |
| 6 | `compiler/oric/src/typeck/checker/mod.rs`, `components.rs`, `builder.rs`, all checker submodules, infer module |
| 7 | `compiler/oric/src/eval/evaluator/method_dispatch.rs`, `resolvers/mod.rs`, `resolvers/user.rs`, `resolvers/derived.rs`, `resolvers/collection.rs`, `resolvers/builtin.rs` |

### Existing Patterns to Reference

- **Builder Pattern**: `compiler/oric/src/eval/evaluator/builder.rs`
- **RAII Guards**: Rust stdlib `MutexGuard`, `RefMut`
- **Trait Implementation**: `compiler/ori_patterns/src/value/mod.rs:455` (PartialEq)
- **Registry Pattern**: `compiler/ori_eval/src/user_methods.rs`
