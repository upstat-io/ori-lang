# Code Quality Refactoring Roadmap — Execution Plan

## How to Use This Plan

### Prerequisites

Before starting:
1. Familiarize yourself with `CLAUDE.md` (project overview)
2. Review `docs/compiler/design/appendices/E-coding-guidelines.md` (coding standards)
3. Ensure `cargo t` passes (all Rust unit tests)
4. Ensure `cargo st` passes (all Sigil language tests)

### Execution Rules

1. **Follow phase order strictly** — Dependencies are encoded in the numbering
2. **Within each phase**, complete sections in order (X.1 → X.2 → ...)
3. **Within each section**, complete items top to bottom
4. **Each item requires**: Implementation → Tests → Verification
5. **Do not skip phases** unless marked complete or explicitly skipped
6. **Run tests after each change** — `cargo t` for Rust, `cargo st` for Sigil

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

## Phase 1: TypeChecker Builder Pattern

**Goal**: Eliminate constructor duplication in TypeChecker by introducing a builder pattern.

**Current State**: `compiler/sigilc/src/typeck/checker/mod.rs:79-185`
- 4 constructors (`new`, `with_source`, `with_context`, `with_source_and_config`)
- Each repeats 18 field initializations
- Adding a new field requires 4 separate changes

**Reference Pattern**: `compiler/sigilc/src/eval/evaluator/builder.rs` (EvaluatorBuilder)

### 1.1 Create TypeCheckerBuilder

- [ ] **Implement**: Create `TypeCheckerBuilder` struct in new file `compiler/sigilc/src/typeck/checker/builder.rs`
  - [ ] Define builder struct with required fields (`arena`, `interner`) and optional fields
  - [ ] Implement `new()` constructor taking required references
  - [ ] Add `#[must_use]` chainable setter methods: `with_source()`, `with_context()`, `with_diagnostic_config()`
  - [ ] Implement `build()` method that constructs `TypeChecker` with defaults
  - [ ] **Verify**: Builder compiles and matches EvaluatorBuilder pattern
  - [ ] **Test**: `cargo test --lib -- typeck::checker::builder`

### 1.2 Integrate Builder into mod.rs

- [ ] **Implement**: Add `mod builder;` and `pub use builder::TypeCheckerBuilder;` to `compiler/sigilc/src/typeck/checker/mod.rs`
  - [ ] **Verify**: Module compiles without errors

### 1.3 Refactor Constructors to Use Builder

- [ ] **Implement**: Refactor `TypeChecker::new()` — `compiler/sigilc/src/typeck/checker/mod.rs:79-100`
  - [ ] Replace field-by-field initialization with `TypeCheckerBuilder::new(arena, interner).build()`
  - [ ] **Verify**: Existing tests pass unchanged

- [ ] **Implement**: Refactor `TypeChecker::with_source()` — `compiler/sigilc/src/typeck/checker/mod.rs:105-126`
  - [ ] Replace with builder: `.with_source(source).build()`
  - [ ] **Verify**: Existing tests pass unchanged

- [ ] **Implement**: Refactor `TypeChecker::with_context()` — `compiler/sigilc/src/typeck/checker/mod.rs:131-156`
  - [ ] Replace with builder: `.with_context(context).build()`
  - [ ] **Verify**: Existing tests pass unchanged

- [ ] **Implement**: Refactor `TypeChecker::with_source_and_config()` — `compiler/sigilc/src/typeck/checker/mod.rs:159-185`
  - [ ] Replace with builder: `.with_source(source).with_diagnostic_config(config).build()`
  - [ ] **Verify**: Existing tests pass unchanged

### 1.4 Add Builder Tests

- [ ] **Implement**: Add comprehensive builder tests
  - [ ] Test default construction
  - [ ] Test each optional parameter
  - [ ] Test combinations of optional parameters
  - [ ] **Test**: `cargo test --lib -- typeck::checker::builder::tests`

### 1.5 Final Verification

- [ ] **Verify**: Run full test suite: `cargo t && cargo st`
- [ ] **Verify**: No constructor duplication remains (each field initialized once in `build()`)

---

## Phase 2: RAII Guards for Context

**Goal**: Replace manual save/restore patterns with RAII guards to prevent bugs from forgotten restores.

**Current State**: `compiler/sigilc/src/typeck/checker/mod.rs:514-532`
```rust
let old_caps = std::mem::take(&mut self.current_function_caps);
self.current_function_caps = func.capabilities.iter().map(|c| c.name).collect();
let old_provided = std::mem::take(&mut self.provided_caps);
// ... work ...
self.current_function_caps = old_caps;
self.provided_caps = old_provided;
```

This pattern repeats in `check_function` (line 496, save at 514), `check_test` (line 536, save at 572), and is error-prone.

### 2.1 Create CapabilityScope Guard

- [ ] **Implement**: Create `CapabilityScope` struct in `compiler/sigilc/src/typeck/checker/scope_guards.rs`
  ```rust
  pub struct CapabilityScope<'a, 'b> {
      checker: &'a mut TypeChecker<'b>,
      old_caps: HashSet<Name>,
      old_provided: HashSet<Name>,
  }
  ```
  - [ ] Implement `new()` that saves and replaces capabilities
  - [ ] Implement `Drop` that restores saved capabilities
  - [ ] **Verify**: Guard compiles and follows RAII pattern

### 2.2 Create ImplScope Guard

- [ ] **Implement**: Create `ImplScope` struct for `current_impl_self` context
  - [ ] Save and restore `current_impl_self` on drop
  - [ ] Replace manual `enter_impl`/`exit_impl` pattern at `mod.rs:687-694`
  - [ ] **Verify**: Guard compiles

### 2.3 Integrate Guards into mod.rs

- [ ] **Implement**: Add `mod scope_guards;` to `compiler/sigilc/src/typeck/checker/mod.rs`

### 2.4 Refactor check_function

- [ ] **Implement**: Replace manual save/restore in `check_function` — `compiler/sigilc/src/typeck/checker/mod.rs:496-533`
  - [ ] Create `CapabilityScope` at function entry (after line 511)
  - [ ] Remove manual save at lines 514-516
  - [ ] Remove manual restoration at lines 528-529
  - [ ] **Verify**: Tests pass, capabilities correctly restored even on early return

### 2.5 Refactor check_test

- [ ] **Implement**: Replace manual save/restore in `check_test` — `compiler/sigilc/src/typeck/checker/mod.rs:536-590`
  - [ ] Create `CapabilityScope` at test entry (after line 568)
  - [ ] Remove manual save at lines 572-573
  - [ ] Remove manual restoration at lines 585-586
  - [ ] **Verify**: Tests pass

### 2.6 Refactor check_impl_methods

- [ ] **Implement**: Replace `enter_impl`/`exit_impl` with `ImplScope` — `compiler/sigilc/src/typeck/checker/mod.rs:593-602`
  - [ ] Create `ImplScope` instead of calling `enter_impl`
  - [ ] Remove explicit `exit_impl` call
  - [ ] **Verify**: Tests pass

### 2.7 Add Guard Tests

- [ ] **Implement**: Add tests for scope guards
  - [ ] Test that capabilities are restored after normal return
  - [ ] Test that capabilities are restored after early return
  - [ ] Test nested scopes
  - [ ] **Test**: `cargo test --lib -- typeck::checker::scope_guards`

### 2.8 Final Verification

- [ ] **Verify**: Run full test suite: `cargo t && cargo st`
- [ ] **Verify**: No manual save/restore patterns remain for capability context

---

## Phase 3: Standard Traits for Value

**Goal**: Implement `Eq` and `Hash` traits for `Value` to enable use in collections and eliminate duplicate helper functions.

**Current State**:
- `Value` has `PartialEq` implemented — `compiler/sigil_patterns/src/value/mod.rs:455-474`
- `values_equal()` duplicates equality logic — `compiler/sigilc/src/eval/evaluator/method_dispatch.rs:455-499`
- `hash_value()` standalone function — `compiler/sigilc/src/eval/evaluator/method_dispatch.rs:502-557`
- Neither `Eq` nor `Hash` are implemented for `Value`

### 3.1 Implement Eq for Value

- [ ] **Implement**: Add `impl Eq for Value {}` — `compiler/sigil_patterns/src/value/mod.rs` after line 474
  - [ ] The existing `PartialEq` implementation is reflexive, so `Eq` is valid
  - [ ] **Verify**: Compiles without errors

### 3.2 Implement Hash for Value

- [ ] **Implement**: Add `impl Hash for Value` — `compiler/sigil_patterns/src/value/mod.rs`
  - [ ] Port logic from `hash_value()` at `method_dispatch.rs:501-557`
  - [ ] Handle all Value variants consistently with PartialEq
  - [ ] Use discriminant tags to distinguish variants
  - [ ] **Verify**: Compiles without errors
  - [ ] **Test**: `cargo test --lib -- value::tests`

### 3.3 Add Hash Tests

- [ ] **Implement**: Add hash consistency tests in `compiler/sigil_patterns/src/value/mod.rs`
  - [ ] Test that equal values have equal hashes
  - [ ] Test that Value can be used in HashSet
  - [ ] Test that Value can be used as HashMap key
  - [ ] **Test**: `cargo test --lib -- value::tests::test_hash`

### 3.4 Refactor values_equal to Use PartialEq

- [ ] **Implement**: Replace `values_equal()` with `PartialEq::eq` — `compiler/sigilc/src/eval/evaluator/method_dispatch.rs:455-499`
  - [ ] Update `eval_derived_eq` at line 358 to use `sv == ov` instead of `values_equal(sv, ov)`
  - [ ] Delete the `values_equal()` function
  - [ ] **Verify**: Tests pass

### 3.5 Refactor hash_value to Use Hash Trait

- [ ] **Implement**: Replace `hash_value()` with `Hash::hash` — `compiler/sigilc/src/eval/evaluator/method_dispatch.rs:502-557`
  - [ ] Update `eval_derived_hash` at line 409 to use `val.hash(&mut hasher)` instead of `hash_value(val, &mut hasher)`
  - [ ] Delete the `hash_value()` function
  - [ ] **Verify**: Tests pass

### 3.6 Final Verification

- [ ] **Verify**: Run full test suite: `cargo t && cargo st`
- [ ] **Verify**: `values_equal` and `hash_value` functions are deleted
- [ ] **Verify**: Value can be used in `HashSet<Value>` and `HashMap<Value, _>`

---

## Phase 4: Unified Registry Key Types

**Goal**: Create consistent key types across registries to improve type safety and reduce string allocations.

**Current State**:
- `UserMethodRegistry` uses `HashMap<(String, String), _>` — `compiler/sigil_eval/src/user_methods.rs:136`
- `TraitRegistry` uses `HashMap<(Name, String), _>` — `compiler/sigilc/src/typeck/type_registry/trait_registry.rs:121`
- Inconsistent: one uses `String`, one uses `Name` for type identifier

### 4.1 Create MethodKey Type

- [ ] **Implement**: Create `MethodKey` newtype in `compiler/sigil_eval/src/method_key.rs`
  ```rust
  #[derive(Clone, Eq, PartialEq, Hash, Debug)]
  pub struct MethodKey {
      pub type_name: String,
      pub method_name: String,
  }
  ```
  - [ ] Implement `MethodKey::new(type_name: impl Into<String>, method_name: impl Into<String>)`
  - [ ] Implement `Display` for debugging
  - [ ] **Verify**: Type compiles with required derives

### 4.2 Integrate into sigil_eval

- [ ] **Implement**: Add `mod method_key; pub use method_key::MethodKey;` to `compiler/sigil_eval/src/lib.rs`

### 4.3 Refactor UserMethodRegistry

- [ ] **Implement**: Update `UserMethodRegistry` to use `MethodKey` — `compiler/sigil_eval/src/user_methods.rs:134-139`
  - [ ] Change `methods: HashMap<(String, String), UserMethod>` to `methods: HashMap<MethodKey, UserMethod>`
  - [ ] Change `derived_methods: HashMap<(String, String), DerivedMethodInfo>` to use `MethodKey`
  - [ ] Update `register()` at line 156-158
  - [ ] Update `register_derived()` at line 166-173
  - [ ] Update `lookup()` at line 178-181
  - [ ] Update `lookup_derived()` at line 186-189
  - [ ] Update `lookup_any()` at line 194-206
  - [ ] Update `has_method()` at line 209-211
  - [ ] **Verify**: Existing tests pass
  - [ ] **Test**: `cargo test --lib -- user_methods::tests`

### 4.4 Update Callers

- [ ] **Implement**: Update method dispatch callers — `compiler/sigilc/src/eval/evaluator/method_dispatch.rs`
  - [ ] Update calls to `lookup()`, `lookup_derived()` to construct `MethodKey`
  - [ ] **Verify**: Compiles without errors

### 4.5 Final Verification

- [ ] **Verify**: Run full test suite: `cargo t && cargo st`
- [ ] **Verify**: No raw tuple keys remain in UserMethodRegistry

---

## Phase 5: Extract Type Conversion Logic

**Goal**: Extract repeated type conversion patterns in TypeChecker into dedicated helper methods.

**Current State**: `compiler/sigilc/src/typeck/checker/mod.rs`
- `parsed_type_to_type()` at line 322-432 handles many cases inline
- `resolve_parsed_type_with_generics()` at line 438-493 duplicates some of this logic

### 5.1 Identify Common Patterns

- [ ] **Analyze**: Document repeated patterns in type conversion
  - [ ] Generic type handling (Option, Result, Set, Range, Channel) — lines 334-377
  - [ ] Function type handling — lines 395-404
  - [ ] Associated type handling — lines 413-430
  - [ ] **Document**: List patterns that appear in both methods

### 5.2 Extract Generic Type Resolution

- [ ] **Implement**: Create `resolve_generic_type()` helper method
  - [ ] Handle well-known generic types (Option, Result, Set, Range, Channel)
  - [ ] Accept callback for inner type resolution
  - [ ] Call from both `parsed_type_to_type()` and `resolve_parsed_type_with_generics()`
  - [ ] **Verify**: Both methods produce identical results
  - [ ] **Test**: `cargo test --lib -- typeck::checker::tests`

### 5.3 Extract Associated Type Resolution

- [ ] **Implement**: Create `resolve_associated_type()` helper method
  - [ ] Handle `ParsedType::AssociatedType` conversion
  - [ ] Create appropriate `Type::Projection`
  - [ ] **Verify**: Both methods produce identical results

### 5.4 Simplify Main Methods

- [ ] **Implement**: Refactor `parsed_type_to_type()` to use helpers
  - [ ] Replace inline generic handling with `resolve_generic_type()`
  - [ ] Replace inline associated type handling with `resolve_associated_type()`
  - [ ] **Verify**: Tests pass

- [ ] **Implement**: Refactor `resolve_parsed_type_with_generics()` to use helpers
  - [ ] Reduce duplication with `parsed_type_to_type()`
  - [ ] **Verify**: Tests pass

### 5.5 Final Verification

- [ ] **Verify**: Run full test suite: `cargo t && cargo st`
- [ ] **Verify**: Type conversion logic is DRY

---

## Phase 6: TypeChecker Component Extraction

**Goal**: Split TypeChecker's 18 fields into logical component structs for better organization and testability.

**Current State**: `compiler/sigilc/src/typeck/checker/mod.rs:40-75`
- 18 fields covering inference, registries, diagnostics, and context
- Violates Single Responsibility Principle

**Target Structure**:
```rust
pub struct TypeChecker<'a> {
    context: CheckContext<'a>,      // arena, interner
    inference: InferenceState,       // ctx, env, base_env, expr_types
    registries: Registries,          // pattern, type_op, types, traits
    diagnostics: DiagnosticCollector,// errors, queue, source
    // ... remaining fields
}
```

### 6.1 Create CheckContext

- [ ] **Implement**: Create `CheckContext` struct in `compiler/sigilc/src/typeck/checker/context.rs`
  ```rust
  pub struct CheckContext<'a> {
      pub arena: &'a ExprArena,
      pub interner: &'a StringInterner,
  }
  ```
  - [ ] **Verify**: Struct compiles

### 6.2 Create InferenceState

- [ ] **Implement**: Create `InferenceState` struct
  ```rust
  pub struct InferenceState {
      pub ctx: InferenceContext,
      pub env: TypeEnv,
      pub base_env: Option<TypeEnv>,
      pub expr_types: HashMap<usize, Type>,
  }
  ```
  - [ ] Implement `Default` for default initialization
  - [ ] **Verify**: Struct compiles

### 6.3 Create Registries Bundle

- [ ] **Implement**: Create `Registries` struct
  ```rust
  pub struct Registries {
      pub pattern: SharedRegistry<PatternRegistry>,
      pub type_op: TypeOperatorRegistry,
      pub types: TypeRegistry,
      pub traits: TraitRegistry,
  }
  ```
  - [ ] Implement `Default` for default initialization
  - [ ] **Verify**: Struct compiles

### 6.4 Create DiagnosticCollector

- [ ] **Implement**: Create `DiagnosticCollector` struct
  ```rust
  pub struct DiagnosticCollector {
      pub errors: Vec<TypeCheckError>,
      pub queue: Option<DiagnosticQueue>,
      pub source: Option<String>,
  }
  ```
  - [ ] Move `report_type_error()` logic into this struct
  - [ ] Move `limit_reached()` logic into this struct
  - [ ] **Verify**: Struct compiles

### 6.5 Refactor TypeChecker

- [ ] **Implement**: Update `TypeChecker` to use component structs
  - [ ] Replace individual fields with component structs
  - [ ] Update all field accesses throughout the codebase
  - [ ] Update builder to construct components
  - [ ] **Verify**: All tests pass
  - [ ] **Test**: `cargo t && cargo st`

### 6.6 Update Internal References

- [ ] **Implement**: Update methods that access component fields
  - [ ] Update `check_function`, `check_test`, `check_impl_methods`
  - [ ] Update inference module calls
  - [ ] **Verify**: Tests pass

### 6.7 Final Verification

- [ ] **Verify**: Run full test suite: `cargo t && cargo st`
- [ ] **Verify**: TypeChecker fields are logically grouped

---

## Phase 7: Method Dispatch Chain of Responsibility

**Goal**: Replace cascading if-else in method dispatch with extensible Chain of Responsibility pattern.

**Current State**: `compiler/sigilc/src/eval/evaluator/method_dispatch.rs:16-42`
```rust
// First, check user-defined methods
if let Some(user_method) = self.user_method_registry.lookup(&type_name, method_name) { ... }
// Second, check derived methods
if let Some(derived_info) = self.user_method_registry.lookup_derived(...) { ... }
// Third, check collection methods
if let Some(result) = self.try_eval_collection_method(...)? { ... }
// Fall back to built-in methods
self.method_registry.dispatch(...)
```

### 7.1 Define MethodResolver Trait

- [ ] **Implement**: Create `MethodResolver` trait in `compiler/sigilc/src/eval/evaluator/resolvers/mod.rs`
  ```rust
  pub trait MethodResolver: Send + Sync {
      fn resolve(&self, receiver: &Value, method_name: &str) -> Option<MethodResolution>;
      fn priority(&self) -> u8;
  }

  pub enum MethodResolution {
      UserMethod(UserMethod),
      DerivedMethod(DerivedMethodInfo),
      CollectionMethod(CollectionMethod),
      BuiltinMethod,
  }
  ```
  - [ ] **Verify**: Trait compiles

### 7.2 Implement UserMethodResolver

- [ ] **Implement**: Create `UserMethodResolver` in `resolvers/user.rs`
  - [ ] Wrap `UserMethodRegistry` reference
  - [ ] Implement `resolve()` to check user methods first
  - [ ] Return priority 0 (highest)
  - [ ] **Verify**: Resolver compiles

### 7.3 Implement DerivedMethodResolver

- [ ] **Implement**: Create `DerivedMethodResolver` in `resolvers/derived.rs`
  - [ ] Wrap `UserMethodRegistry` reference
  - [ ] Implement `resolve()` to check derived methods
  - [ ] Return priority 1
  - [ ] **Verify**: Resolver compiles

### 7.4 Implement CollectionMethodResolver

- [ ] **Implement**: Create `CollectionMethodResolver` in `resolvers/collection.rs`
  - [ ] Handle list, range, map method detection
  - [ ] Return priority 2
  - [ ] **Verify**: Resolver compiles

### 7.5 Implement BuiltinMethodResolver

- [ ] **Implement**: Create `BuiltinMethodResolver` in `resolvers/builtin.rs`
  - [ ] Wrap `MethodRegistry` reference
  - [ ] Implement `resolve()` to check built-in methods
  - [ ] Return priority 3 (lowest)
  - [ ] **Verify**: Resolver compiles

### 7.6 Create MethodDispatcher

- [ ] **Implement**: Create `MethodDispatcher` struct
  ```rust
  pub struct MethodDispatcher {
      resolvers: Vec<Box<dyn MethodResolver>>,
  }
  ```
  - [ ] Implement `new()` that registers all resolvers sorted by priority
  - [ ] Implement `dispatch()` that tries resolvers in order
  - [ ] **Verify**: Dispatcher compiles

### 7.7 Integrate into Evaluator

- [ ] **Implement**: Update `Evaluator` to use `MethodDispatcher`
  - [ ] Replace `eval_method_call` cascading logic with dispatcher
  - [ ] Move method execution logic to separate methods
  - [ ] **Verify**: All method call tests pass
  - [ ] **Test**: `cargo test --lib -- eval::evaluator::tests`

### 7.8 Final Verification

- [ ] **Verify**: Run full test suite: `cargo t && cargo st`
- [ ] **Verify**: Method dispatch is extensible via new resolvers

---

## Running Tests

```bash
# Rust unit tests (all workspace crates)
cargo t

# Sigil language tests (all)
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

## Code Quality Metrics (Target)

After completing all phases:

| Metric | Before | After Target |
|--------|--------|--------------|
| TypeChecker constructors | 4 (duplicated) | 1 builder + build() |
| Manual save/restore sites | 3+ | 0 (RAII guards) |
| Duplicate equality functions | 2 | 0 (PartialEq trait) |
| Duplicate hash functions | 1 | 0 (Hash trait) |
| TypeChecker fields | 18 flat | 4 component structs |
| Method dispatch branches | 4 cascading | N extensible resolvers |

---

## References

### Files to Modify

| Phase | Primary Files |
|-------|---------------|
| 1 | `compiler/sigilc/src/typeck/checker/mod.rs`, new `builder.rs` |
| 2 | `compiler/sigilc/src/typeck/checker/mod.rs`, new `scope_guards.rs` |
| 3 | `compiler/sigil_patterns/src/value/mod.rs`, `compiler/sigilc/src/eval/evaluator/method_dispatch.rs` |
| 4 | `compiler/sigil_eval/src/user_methods.rs`, new `method_key.rs` |
| 5 | `compiler/sigilc/src/typeck/checker/mod.rs` |
| 6 | `compiler/sigilc/src/typeck/checker/mod.rs`, new component files |
| 7 | `compiler/sigilc/src/eval/evaluator/method_dispatch.rs`, new `resolvers/` |

### Existing Patterns to Reference

- **Builder Pattern**: `compiler/sigilc/src/eval/evaluator/builder.rs`
- **RAII Guards**: Rust stdlib `MutexGuard`, `RefMut`
- **Trait Implementation**: `compiler/sigil_patterns/src/value/mod.rs:455` (PartialEq)
- **Registry Pattern**: `compiler/sigil_eval/src/user_methods.rs`
