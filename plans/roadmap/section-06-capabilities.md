---
section: 6
title: Capabilities System
status: not-started
tier: 2
goal: Effect tracking (moved earlier to unblock Section 8 cache and Section 11 FFI)
spec:
  - spec/14-capabilities.md
sections:
  - id: "6.1"
    title: Capability Declaration
    status: not-started
  - id: "6.2"
    title: Capability Traits
    status: not-started
  - id: "6.3"
    title: Suspend Capability
    status: not-started
  - id: "6.4"
    title: Providing Capabilities
    status: not-started
  - id: "6.5"
    title: Capability Propagation
    status: not-started
  - id: "6.6"
    title: Standard Capabilities
    status: not-started
  - id: "6.7"
    title: Testing with Capabilities
    status: not-started
  - id: "6.8"
    title: Capability Constraints
    status: not-started
  - id: "6.9"
    title: Unsafe Capability (FFI Prep)
    status: not-started
  - id: "6.10"
    title: Default Implementations (def impl)
    status: not-started
  - id: "6.11"
    title: Capability Composition
    status: not-started
  - id: "6.12"
    title: Default Implementation Resolution
    status: not-started
  - id: "6.14"
    title: Intrinsics Capability
    status: not-started
  - id: "6.15"
    title: Section Completion Checklist
    status: not-started
---

# Section 6: Capabilities System

**Goal**: Effect tracking (moved earlier to unblock Section 8 cache and Section 11 FFI)

> **SPEC**: `spec/14-capabilities.md`
> **DESIGN**: `design/14-capabilities/index.md`

**Status**: Partial — Core complete (6.1-6.10, 31/31 tests); composition (6.11), resolution (6.12), intrinsics (6.14) pending

---

## 6.1 Capability Declaration

- [ ] **Implement**: `uses` clause — spec/14-capabilities.md § Capability Declaration, design/14-capabilities/02-uses-clause.md
  - [ ] **Rust Tests**: `ori_parse/src/lib.rs` — uses clause parsing (4 tests)
  - [ ] **Ori Tests**: `tests/spec/capabilities/declaration.ori` (3 tests)

- [ ] **Implement**: Multiple capabilities — spec/14-capabilities.md § Multiple Capabilities
  - [ ] **Rust Tests**: `ori_parse/src/lib.rs` — multiple capabilities parsing
  - [ ] **Ori Tests**: `tests/spec/capabilities/declaration.ori` — @save_and_log example

---

## 6.2 Capability Traits

- [ ] **Implement**: Capability traits — spec/14-capabilities.md § Capability Traits
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 7 tests for capability trait validation
  - [ ] **Ori Tests**: `tests/spec/capabilities/traits.ori` — 5 tests for capability traits

---

## 6.3 Suspend Capability

> **Note**: Renamed from `Async` to `Suspend` per `proposals/approved/rename-async-to-suspend-proposal.md`

- [ ] **Implement**: Explicit suspension declaration — spec/14-capabilities.md § Suspend Capability
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 4 tests (marker trait, signature storage, combined capabilities, sync function)
  - [ ] **Ori Tests**: `tests/spec/capabilities/suspend.ori` (5 tests)

- [ ] **Implement**: Sync vs suspending behavior — spec/14-capabilities.md § Suspend Capability
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs::test_sync_function_no_suspend_capability`
  - [ ] **Ori Tests**: `tests/spec/capabilities/suspend.ori` — sync_fetch vs suspending_fetch examples

- [ ] **Implement**: No `async` type modifier — spec/14-capabilities.md § Suspend Capability
  - [ ] **Rust Tests**: `ori_parse/src/lib.rs::test_no_async_type_modifier`, `test_async_keyword_reserved`
  - [ ] **Ori Tests**: `tests/spec/capabilities/suspend.ori` — design notes document this

- [ ] **Implement**: No `await` expression — spec/14-capabilities.md § Suspend Capability
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs::test_await_syntax_not_supported`
  - [ ] **Ori Tests**: `tests/spec/capabilities/suspend.ori` — design notes document this

- [ ] **Implement**: Concurrency with `parallel` — spec/14-capabilities.md § Suspend Capability
  - [ ] **Deferred to Section 8**: `parallel` pattern evaluation
  - [ ] **Ori Tests**: `tests/spec/patterns/parallel.ori` (Section 8)

---

## 6.4 Providing Capabilities

- [ ] **Implement**: `with...in` expression — spec/14-capabilities.md § Providing Capabilities, design/14-capabilities/index.md
  - [ ] **Rust Tests**: `ori_parse/src/lib.rs` — with expression parsing (3 tests)
  - [ ] **Rust Tests**: `oric/src/eval/evaluator/mod.rs` — with expression evaluation
  - [ ] **Ori Tests**: `tests/spec/capabilities/providing.ori` (7 tests)

- [ ] **Implement**: Scoping — spec/14-capabilities.md § Capability Scoping
  - [ ] **Rust Tests**: `oric/src/eval/evaluator/mod.rs` — capability scoping via push_scope/pop_scope
  - [ ] **Ori Tests**: `tests/spec/capabilities/providing.ori` — scoping and shadowing tests

---

## 6.5 Capability Propagation

- [ ] **Implement**: Runtime capability propagation — capabilities flow through function calls
  - [ ] **Changes**: `FunctionValue` now stores capabilities, `eval_call` passes them to called functions
  - [ ] **Ori Tests**: `tests/spec/capabilities/traits.ori` — tests capability propagation

- [ ] **Implement**: Static transitive requirements — spec/14-capabilities.md § Capability Propagation
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 7 tests for capability propagation (E2014)
  - [ ] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — 8 tests for propagation

- [ ] **Implement**: Providing vs requiring — spec/14-capabilities.md § Capability Propagation
  - [ ] **Rust Tests**: `oric/src/typeck/infer/call.rs` — check_capability_propagation function
  - [ ] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — tests with...in providing capabilities

---

## 6.6 Standard Capabilities

> **STATUS**: Trait definitions complete in `library/std/prelude.ori`
> Real implementations deferred to Section 7 (Stdlib).

- [ ] **Define**: Trait interfaces — spec/14-capabilities.md § Standard Capabilities
  - [ ] **Location**: `library/std/prelude.ori` — trait definitions
  - [ ] **Traits**: Http, FileSystem, Cache, Clock, Random, Logger, Env

- [ ] **Implement** (Section 7): Real capability implementations
  - [ ] `std.net.http` — Http capability impl
  - [ ] `std.fs` — FileSystem capability impl
  - [ ] `std.time` — Clock capability impl
  - [ ] `std.math.rand` — Random capability impl
  - [ ] `std.cache` — Cache capability impl (new module)
  - [ ] `std.log` — Logger capability impl
  - [ ] `std.env` — Env capability impl

---

## 6.7 Testing with Capabilities

> **STATUS**: Complete — mocking works via trait implementations, demonstrated in propagation.ori

- [ ] **Implement**: Mock implementations — spec/14-capabilities.md § Testing with Capabilities
  - [ ] **Rust Tests**: Type checking handles trait implementations for capability mocking
  - [ ] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — MockHttp, MockLogger examples

- [ ] **Implement**: Test example — spec/14-capabilities.md § Testing with Capabilities
  - [ ] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — shows test patterns with `with...in`

---

## 6.8 Capability Constraints

> **STATUS**: Complete — compile-time enforcement via E2014 propagation errors

- [ ] **Implement**: Compile-time enforcement — spec/14-capabilities.md § Compile-time Enforcement
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 7 tests for E2014 propagation errors
  - [ ] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — caller must declare or provide capabilities

---

## 6.9 Unsafe Capability (FFI Prep)

> **PREREQUISITE FOR**: Section 11 (FFI)
> The Unsafe capability is required for FFI. Implement this before starting FFI work.

- [ ] **Implement**: `Unsafe` marker capability
  - [ ] Defined in prelude as marker trait (no methods): `library/std/prelude.ori`
  - [ ] **Ori Tests**: `tests/spec/capabilities/unsafe.ori` — basic tests

- [ ] **Implement**: Unsafe capability requirements (Section 11)
  - [ ] Required for: raw pointer operations (future)
  - [ ] Required for: extern function calls (future)
  - [ ] Required for: unsafe blocks (future)
  - [ ] Tests added when FFI implemented

- [ ] **Implement**: AllowUnsafe provider type (Section 11)
  - [ ] Concrete type that satisfies Unsafe capability
  - [ ] For use in tests: `with Unsafe = AllowUnsafe in ...`
  - [ ] Added when FFI tests need it

---

## 6.10 Default Implementations (`def impl`)

**Proposal**: `proposals/approved/default-impl-proposal.md`
**Status**: Complete

Introduce `def impl` syntax to declare a default implementation for a trait. Importing a trait with a `def impl` automatically binds the default.

### Implementation

- [ ] **Implement**: Add `def` keyword to lexer — grammar.ebnf § DECLARATIONS
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — `def` token recognition
  - [ ] **Ori Tests**: `tests/spec/capabilities/default-impl.ori`

- [ ] **Implement**: Parse `def impl Trait { ... }` — grammar.ebnf § DECLARATIONS
  - [ ] **Rust Tests**: `ori_parse/src/grammar/item/impl_def.rs` — DefImpl AST node parsing (5 tests)
  - [ ] **Ori Tests**: `tests/spec/capabilities/default-impl.ori`

- [ ] **Implement**: IR representation for DefImpl
  - [ ] Add `DefImplDef` to module items (`ori_ir/src/ast/items/traits.rs`)
  - [ ] Track def_impls in Module struct

- [ ] **Implement**: Type checking for `def impl`
  - [ ] **Rust Tests**: `ori_typeck/src/checker/trait_registration.rs` — register_def_impls
  - [ ] Verify trait exists
  - [ ] Method signatures converted to ImplMethodDef
  - [ ] Methods are associated (no self parameter)
  - [ ] One `def impl` per trait per module (coherence check)

- [ ] **Implement**: Evaluator support for def impl dispatch
  - [ ] **Rust Tests**: `ori_eval/src/module_registration.rs` — collect_def_impl_methods (2 tests)
  - [ ] Methods registered under trait name for `TraitName.method()` calls

- [ ] **Implement**: Module export with default — 12-modules.md
  - [ ] Mark exports as "has default" when `def impl` exists
  - [ ] Bind default when importing trait

- [ ] **Implement**: Name resolution — 14-capabilities.md
  - [ ] Check `with...in` binding first
  - [ ] Check imported default second
  - [ ] Check module-local `def impl` third
  - [ ] **Ori Tests**: `tests/spec/capabilities/default-impl.ori`

- [ ] **Implement**: Evaluator support
  - [ ] Dispatch method calls to bound default
  - [ ] Override via `with...in` works
  - [ ] **Ori Tests**: `tests/spec/capabilities/default-impl.ori`

- [ ] **Implement**: LLVM backend support
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/default_impl_tests.rs`
  - [ ] Codegen for `def impl` methods

---

## 6.11 Capability Composition

**Proposal**: `proposals/approved/capability-composition-proposal.md`

Specifies how capabilities compose: partial provision, nested binding semantics, capability variance, and resolution priority order.

### Implementation

- [ ] **Implement**: Multi-binding `with` syntax — grammar.ebnf § EXPRESSIONS
  - [ ] **Parser**: Extend `with_expr` to support comma-separated bindings
  - [ ] **Rust Tests**: `ori_parse/src/lib.rs` — multi-binding with expression parsing
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Partial provision — providing some capabilities while others use defaults
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — partial capability provision
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Nested `with...in` shadowing — inner bindings shadow outer
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — capability shadowing
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Capability variance — more caps can call fewer, not reverse
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — variance checking
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Resolution priority order — inner with → outer with → imported def impl → local def impl → error
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — resolution priority
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Suspend binding prohibition — `with Suspend = ...` is compile error
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — suspend prohibition (E1203)
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Error codes E1200-E1203
  - [ ] E1200: missing capability
  - [ ] E1201: unbound capability
  - [ ] E1202: type doesn't implement capability trait
  - [ ] E1203: Suspend cannot be explicitly bound

- [ ] **Implement**: LLVM backend support
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_composition_tests.rs`

---

## 6.12 Default Implementation Resolution

**Proposal**: `proposals/approved/default-impl-resolution-proposal.md`

Specifies resolution rules for `def impl`: conflict handling, `without def` import syntax, re-export behavior, and resolution order.

### Implementation

- [ ] **Implement**: `without def` import syntax — grammar.ebnf § IMPORTS
  - [ ] Parse `use "module" { Trait without def }` to import trait without its default
  - [ ] **Rust Tests**: `ori_parse/src/lib.rs` — `without def` import modifier parsing
  - [ ] **Ori Tests**: `tests/spec/capabilities/def-impl-resolution.ori`

- [ ] **Implement**: Import conflict detection — one `def impl` per trait per scope
  - [ ] Error E1000: conflicting default implementations when two imports bring same trait's `def impl`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — import conflict detection
  - [ ] **Ori Tests**: `tests/spec/capabilities/def-impl-resolution.ori`

- [ ] **Implement**: Duplicate `def impl` detection — one per trait per module
  - [ ] Error E1001: duplicate default implementation in same module
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — duplicate def impl detection
  - [ ] **Ori Tests**: `tests/spec/capabilities/def-impl-resolution.ori`

- [ ] **Implement**: Resolution order — with...in > imported def > module-local def
  - [ ] Innermost `with...in` binding takes precedence
  - [ ] Imported `def impl` overrides module-local
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — resolution priority
  - [ ] **Ori Tests**: `tests/spec/capabilities/def-impl-resolution.ori`

- [ ] **Implement**: Re-export with `without def` — permanently strips default from export path
  - [ ] `pub use "module" { Trait without def }` re-exports trait without default
  - [ ] **Rust Tests**: `oric/src/eval/module/import.rs` — re-export stripping
  - [ ] **Ori Tests**: `tests/spec/capabilities/def-impl-resolution.ori`

- [ ] **Implement**: Error messages with help text
  - [ ] E1000: "use `Logger without def` to import trait without default"
  - [ ] E1001: show location of first definition
  - [ ] E1002: "`def impl` methods cannot have `self` parameter"
  - [ ] **Rust Tests**: `oric/src/errors/` — error formatting tests

- [ ] **Implement**: LLVM backend support
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/def_impl_resolution_tests.rs`

---

## 6.14 Intrinsics Capability

**Proposal**: `proposals/approved/intrinsics-capability-proposal.md`

Low-level SIMD, bit manipulation, and hardware feature detection. Atomics deferred to separate proposal.

### Implementation

- [ ] **Implement**: Add `Intrinsics` trait to prelude — spec/14-capabilities.md
  - [ ] Trait definition with all SIMD and bit operations
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics.ori`

- [ ] **Implement**: SIMD float operations (4-wide/128-bit)
  - [ ] `simd_add_f32x4`, `simd_sub_f32x4`, `simd_mul_f32x4`, `simd_div_f32x4`
  - [ ] `simd_min_f32x4`, `simd_max_f32x4`, `simd_sqrt_f32x4`, `simd_abs_f32x4`
  - [ ] `simd_eq_f32x4`, `simd_lt_f32x4`, `simd_gt_f32x4`
  - [ ] `simd_sum_f32x4` (horizontal reduction)
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-simd-f32x4.ori`

- [ ] **Implement**: SIMD float operations (8-wide/256-bit, AVX)
  - [ ] Same operations as 4-wide with `f32x8` suffix
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-simd-f32x8.ori`

- [ ] **Implement**: SIMD float operations (16-wide/512-bit, AVX-512)
  - [ ] Same operations as 4-wide with `f32x16` suffix
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-simd-f32x16.ori`

- [ ] **Implement**: SIMD 64-bit integer operations (2-wide/128-bit)
  - [ ] `simd_add_i64x2`, `simd_sub_i64x2`, `simd_mul_i64x2`
  - [ ] `simd_min_i64x2`, `simd_max_i64x2`
  - [ ] `simd_eq_i64x2`, `simd_lt_i64x2`, `simd_gt_i64x2`
  - [ ] `simd_sum_i64x2` (horizontal reduction)
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-simd-i64x2.ori`

- [ ] **Implement**: SIMD 64-bit integer operations (4-wide/256-bit, AVX2)
  - [ ] Same operations as 2-wide with `i64x4` suffix
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-simd-i64x4.ori`

- [ ] **Implement**: Bit manipulation operations
  - [ ] `count_leading_zeros`, `count_trailing_zeros`, `count_ones`
  - [ ] `rotate_left`, `rotate_right`
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-bits.ori`

- [ ] **Implement**: Hardware feature detection
  - [ ] `cpu_has_feature` with valid feature strings
  - [ ] Error E1062 for unknown features
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-feature-detect.ori`

- [ ] **Implement**: `def impl Intrinsics` (NativeWithFallback)
  - [ ] Native SIMD when platform supports
  - [ ] Scalar emulation fallback
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-fallback.ori`

- [ ] **Implement**: `EmulatedIntrinsics` provider
  - [ ] Always uses scalar operations
  - [ ] For testing and portability
  - [ ] **Ori Tests**: `tests/spec/capabilities/intrinsics-emulated.ori`

- [ ] **Implement**: Error messages
  - [ ] E1060: requires Intrinsics capability
  - [ ] E1062: unknown CPU feature
  - [ ] E1063: wrong SIMD vector size
  - [ ] **Rust Tests**: `oric/src/errors/` — error formatting tests

- [ ] **Implement**: LLVM backend SIMD codegen
  - [ ] Map to LLVM vector intrinsics
  - [ ] `count_ones` → `llvm.ctpop.i64`
  - [ ] `count_leading_zeros` → `llvm.ctlz.i64`
  - [ ] Runtime CPUID for feature detection
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/intrinsics_tests.rs`

---

## 6.15 Section Completion Checklist

- [ ] 6.1-6.5 complete (declaration, traits, async, providing, propagation)
- [ ] 6.6 trait definitions in prelude (implementations in Section 7)
- [ ] 6.7-6.8 complete (testing/mocking, compile-time enforcement)
- [ ] 6.9 Unsafe marker trait defined (FFI enforcement in Section 11)
- [ ] 6.10 Default implementations (`def impl`) — pending implementation
- [ ] 6.11 Capability Composition — pending implementation
- [ ] 6.12 Default Implementation Resolution — pending implementation
- [ ] 6.14 Intrinsics Capability — pending implementation
- [ ] Spec updated: `spec/14-capabilities.md` reflects implementation
- [ ] CLAUDE.md updated with capabilities syntax
- [ ] 27 capability tests passing
- [ ] Full test suite: `./test-all.sh`

**Exit Criteria**: Effect tracking works per spec (6.1-6.9, 6.10-6.14 pending)

**Remaining for Section 7 (Stdlib)**:
- Real capability implementations (Http, FileSystem, etc.)
- Integration with stdlib modules

**Remaining for Section 11 (FFI)**:
- Unsafe capability enforcement for extern functions
