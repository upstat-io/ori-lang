---
section: 6
title: Capabilities System
status: in-progress
tier: 2
goal: Effect tracking (moved earlier to unblock Section 8 cache and Section 11 FFI)
spec:
  - spec/14-capabilities.md
sections:
  - id: "6.1"
    title: Capability Declaration
    status: in-progress
  - id: "6.2"
    title: Capability Traits
    status: in-progress
  - id: "6.3"
    title: Suspend Capability
    status: in-progress
  - id: "6.4"
    title: Providing Capabilities
    status: in-progress
  - id: "6.5"
    title: Capability Propagation
    status: in-progress
  - id: "6.6"
    title: Standard Capabilities
    status: in-progress
  - id: "6.7"
    title: Testing with Capabilities
    status: in-progress
  - id: "6.8"
    title: Capability Constraints
    status: in-progress
  - id: "6.9"
    title: Unsafe Capability (FFI Prep)
    status: not-started
  - id: "6.10"
    title: Default Implementations (def impl)
    status: in-progress
  - id: "6.11"
    title: Capability Composition
    status: not-started
  - id: "6.12"
    title: Default Implementation Resolution
    status: not-started
  - id: "6.13"
    title: Named Capability Sets (capset)
    status: not-started
  - id: "6.14"
    title: Intrinsics Capability
    status: not-started
  - id: "6.16"
    title: Stateful Handlers
    status: not-started
  - id: "6.17"
    title: Section Completion Checklist
    status: in-progress
---

# Section 6: Capabilities System

**Goal**: Effect tracking (moved earlier to unblock Section 8 cache and Section 11 FFI)

> **SPEC**: `spec/14-capabilities.md`
> **DESIGN**: `design/14-capabilities/index.md`

**Status**: In-progress — Core evaluator working (6.1-6.8, 6.10 partial, ~36 test annotations across 6 test files); composition (6.11), resolution (6.12), intrinsics (6.14), unsafe (6.9) pending. LLVM tests missing. Verified 2026-02-10.

---

## 6.1 Capability Declaration

- [x] **Implement**: `uses` clause [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/lib.rs` — uses clause parsing (4 tests)
  - [x] **Ori Tests**: `tests/spec/capabilities/declaration.ori` (3 tests)
  - [ ] **LLVM Support**: LLVM codegen for `uses` clause in function signatures
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)

- [x] **Implement**: Multiple capabilities [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/lib.rs` — multiple capabilities parsing
  - [x] **Ori Tests**: `tests/spec/capabilities/declaration.ori` — @save_and_log example
  - [ ] **LLVM Support**: LLVM codegen for multiple capabilities in function signatures
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)

---

## 6.2 Capability Traits

- [x] **Implement**: Capability traits [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 7 tests for capability trait validation
  - [x] **Ori Tests**: `tests/spec/capabilities/traits.ori` — 5 tests for capability traits
  - [ ] **LLVM Support**: LLVM codegen for capability trait dispatch
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)

---

## 6.3 Suspend Capability

> **Note**: Renamed from `Async` to `Suspend` per `proposals/approved/rename-async-to-suspend-proposal.md`

- [x] **Implement**: Explicit suspension declaration [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 4 tests (marker trait, signature storage, combined capabilities, sync function)
  - [x] **Ori Tests**: `tests/spec/capabilities/async.ori` (test file exists)
  - [ ] **LLVM Support**: LLVM codegen for explicit suspension declaration
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)

- [x] **Implement**: Sync vs suspending behavior [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs::test_sync_function_no_suspend_capability`
  - [x] **Ori Tests**: `tests/spec/capabilities/async.ori`

- [x] **Implement**: No `async` type modifier [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/lib.rs::test_no_async_type_modifier`, `test_async_keyword_reserved`
  - [x] **Ori Tests**: Design notes document this

- [x] **Implement**: No `await` expression [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs::test_await_syntax_not_supported`
  - [x] **Ori Tests**: Design notes document this

- [ ] **Implement**: Concurrency with `parallel` — spec/14-capabilities.md § Suspend Capability
  - [ ] **Deferred to Section 8**: `parallel` pattern evaluation
  - [ ] **Ori Tests**: `tests/spec/patterns/parallel.ori` (Section 8)
  - [ ] **Note**: Interpreter has a loud stub for parallel in `can_eval.rs` — replace when Suspend capability is implemented (see `plans/eval_legacy_removal/section-02-inline-patterns.md`)

---

## 6.4 Providing Capabilities

- [x] **Implement**: `with...in` expression [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_parse/src/lib.rs` — with expression parsing (3 tests)
  - [x] **Rust Tests**: `oric/src/eval/evaluator/mod.rs` — with expression evaluation
  - [x] **Ori Tests**: `tests/spec/capabilities/providing.ori` (17 test annotations)
  - [ ] **LLVM Support**: LLVM codegen for `with...in` capability binding
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)

- [x] **Implement**: Scoping [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/eval/evaluator/mod.rs` — capability scoping via push_scope/pop_scope
  - [x] **Ori Tests**: `tests/spec/capabilities/providing.ori` — scoping and shadowing tests
  - [ ] **LLVM Support**: LLVM codegen for capability scoping (push/pop)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)

---

## 6.5 Capability Propagation

- [ ] **Implement**: Runtime capability propagation [partial]
  - [x] **Changes**: `FunctionValue` now stores capabilities, `eval_call` passes them to called functions
  - [x] **Ori Tests**: `tests/spec/capabilities/traits.ori` — tests capability propagation (direct use)
  - [ ] **Implement**: `with...in` capability provision propagates to called functions — `with Cap = impl in callee()` should make `Cap` available inside `callee()`
  - [ ] **Ori Tests**: `tests/spec/expressions/with_expr.ori` — 2 tests `#skip("capability provision to called functions not implemented")`
  - [ ] **LLVM Support**: LLVM codegen for runtime capability propagation through calls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)

- [x] **Implement**: Static transitive requirements [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 7 tests for capability propagation (E2014)
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — 7 test annotations for propagation

- [x] **Implement**: Providing vs requiring [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/infer/call.rs` — check_capability_propagation function
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — tests with...in providing capabilities

---

## 6.6 Standard Capabilities

> **STATUS**: Trait definitions complete in `library/std/prelude.ori`
> Real implementations deferred to Section 7 (Stdlib).

- [x] **Define**: Trait interfaces [done] (2026-02-10)
  - [x] **Location**: `library/std/prelude.ori` — trait definitions
  - [x] **Traits**: Http, FileSystem, Cache, Clock, Random, Logger, Env — defined in prelude

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

- [x] **Implement**: Mock implementations [done] (2026-02-10)
  - [x] **Rust Tests**: Type checking handles trait implementations for capability mocking
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — MockHttp, MockLogger examples
  - [ ] **LLVM Support**: LLVM codegen for mock capability implementations
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` (file does not exist)

- [x] **Implement**: Test example [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — shows test patterns with `with...in`

---

## 6.8 Capability Constraints

> **STATUS**: Complete — compile-time enforcement via E2014 propagation errors

- [x] **Implement**: Compile-time enforcement [done] (2026-02-10)
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 7 tests for E2014 propagation errors
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — caller must declare or provide capabilities

---

## 6.9 Unsafe Capability (FFI Prep)

**Proposal**: `proposals/approved/unsafe-semantics-proposal.md`

> **PREREQUISITE FOR**: Section 11 (FFI)
> The Unsafe capability is required for FFI. Implement this before starting FFI work.

- [ ] **Implement**: `Unsafe` marker capability (compiler intrinsic, like `Suspend`)
  - [ ] Add `Unsafe` to standard capabilities list in type checker
  - [ ] Generalize E1203 to cover all marker capabilities (not just `Suspend`)
  - [ ] **Ori Tests**: `tests/spec/capabilities/unsafe/` — basic tests, E1203 binding error
  - [ ] **LLVM Support**: LLVM codegen for `unsafe { }` blocks (transparent — same as inner expr)
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_tests.rs` — unsafe block codegen

- [ ] **Implement**: `unsafe { }` block expression (Phases 1-4 of proposal)
  - [ ] Add `ExprKind::Unsafe(ExprId)` to IR
  - [ ] Parse `unsafe { block_body }` (block-only form — no parenthesized form)
  - [ ] Update grammar.ebnf (remove parenthesized form — done in proposal approval)
  - [ ] Type checker: `UnsafeContext` tracking, E1250 diagnostic
  - [ ] Evaluator: `ExprKind::Unsafe(inner)` → `eval_expr(inner)` (transparent)
  - [ ] Visitor support in `ori_ir/src/visitor.rs`
  - [ ] **Rust Tests**: `ori_parse/src/tests/parser.rs`, `ori_types/src/infer/expr/tests.rs`
  - [ ] **Ori Tests**: `tests/spec/capabilities/unsafe/eval.ori`

- [ ] **Implement**: Unsafe capability requirements (deferred to Section 11)
  - [ ] Required for: raw pointer operations (future)
  - [ ] Required for: C variadic function calls (future)
  - [ ] Required for: transmute operations (future)
  - [ ] Tests added when FFI implemented

> **Note:** `Unsafe` is a marker capability — it cannot be bound via `with...in`. There is no `AllowUnsafe` provider type. Unsafe code is tested by testing safe wrappers.

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

## 6.13 Named Capability Sets (`capset`)

**Proposal**: `proposals/approved/capset-proposal.md`

Transparent aliases for capability sets. Expanded during name resolution before type checking. Reduces signature noise and creates stable dependency surfaces.

### Implementation

- [ ] **Implement**: Add `capset` keyword to lexer — grammar.ebnf § DECLARATIONS
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — `capset` token recognition
  - [ ] **Ori Tests**: `tests/spec/capabilities/capset.ori`

- [ ] **Implement**: Parse `capset_decl` — grammar.ebnf § DECLARATIONS
  - [ ] **Rust Tests**: `ori_parse/src/grammar/item/capset.rs` — CapsetDecl AST node parsing
  - [ ] **Ori Tests**: `tests/spec/capabilities/capset.ori`

- [ ] **Implement**: Name resolution expansion
  - [ ] Expand capset names in `uses` clauses to constituent capabilities
  - [ ] Transitive expansion (capsets containing capsets)
  - [ ] Deduplication (set semantics)
  - [ ] **Rust Tests**: `oric/src/typeck/` — capset expansion tests

- [ ] **Implement**: Cycle detection
  - [ ] Topological sort of capset definitions
  - [ ] Error E1220 for cyclic definitions
  - [ ] **Ori Tests**: `tests/spec/capabilities/capset-errors.ori`

- [ ] **Implement**: Validation rules
  - [ ] Error E1221: empty capset
  - [ ] Error E1222: name collision with trait
  - [ ] Error E1223: member is not capability or capset
  - [ ] Warning W1220: redundant capability in `uses`
  - [ ] **Ori Tests**: `tests/spec/capabilities/capset-errors.ori`

- [ ] **Implement**: Visibility checking
  - [ ] `pub` capset must not reference non-accessible capabilities
  - [ ] **Ori Tests**: `tests/spec/capabilities/capset-visibility.ori`

- [ ] **Implement**: Enhanced E1200 error messages
  - [ ] Show capset expansion context in "missing capability" errors
  - [ ] **Rust Tests**: `oric/src/errors/` — error formatting tests

- [ ] **Implement**: LSP support
  - [ ] Show capset expansion on hover
  - [ ] Autocomplete capset names in `uses` clauses

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

## 6.16 Stateful Handlers

**Proposal**: `proposals/approved/stateful-mock-testing-proposal.md`

Extend `with...in` to support stateful effect handlers. The `handler(state: expr) { ... }` construct threads local mutable state through handler operations, enabling stateful capability mocking while preserving value semantics.

### Implementation

- [ ] **Implement**: Add `handler` as context-sensitive keyword — grammar.ebnf § EXPRESSIONS
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — `handler` token recognition (context-sensitive)
  - [ ] **Ori Tests**: `tests/spec/capabilities/stateful-handlers.ori`

- [ ] **Implement**: Parse `handler(state: expr) { op: expr, ... }` — grammar.ebnf § EXPRESSIONS
  - [ ] **Rust Tests**: `ori_parse/src/grammar/expr/with_expr.rs` — handler expression parsing
  - [ ] **Ori Tests**: `tests/spec/capabilities/stateful-handlers.ori`

- [ ] **Implement**: IR representation for handler expressions
  - [ ] Add `HandlerExpr` to expression AST (state initializer, operation map)
  - [ ] **Rust Tests**: `ori_ir/src/ast/expr/tests.rs` — handler AST node

- [ ] **Implement**: Type checker — verify handler operations match trait signature
  - [ ] State replaces `self` in operation signatures
  - [ ] Operations return `(S, R)` where S is state type, R is trait return type
  - [ ] State type inferred from initializer, consistent across all operations
  - [ ] All required trait methods must have handler operations
  - [ ] Default trait methods used if not overridden
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — handler type checking
  - [ ] **Ori Tests**: `tests/spec/capabilities/stateful-handlers.ori`

- [ ] **Implement**: Error codes E1204-E1207
  - [ ] E1204: handler missing required operation
  - [ ] E1205: handler operation signature mismatch
  - [ ] E1206: handler state type inconsistency
  - [ ] E1207: handler operation for non-existent trait method
  - [ ] **Rust Tests**: `oric/src/errors/` — handler error formatting
  - [ ] **Ori Tests**: `tests/spec/capabilities/stateful-handler-errors.ori`

- [ ] **Implement**: Evaluator — handler frame state threading
  - [ ] Create handler frame with initial state on `with...in` entry
  - [ ] Thread state through capability dispatch calls
  - [ ] Independent state per handler (nested handlers)
  - [ ] `with...in` returns body result only (state is internal)
  - [ ] **Rust Tests**: `ori_eval/src/interpreter/with_expr.rs` — handler evaluation
  - [ ] **Ori Tests**: `tests/spec/capabilities/stateful-handlers.ori`

- [ ] **Implement**: LLVM codegen for stateful handlers
  - [ ] Handler frame state allocation
  - [ ] State threading through operation calls
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/stateful_handler_tests.rs`

---

## 6.17 Section Completion Checklist

- [x] 6.1-6.5 complete (declaration, traits, suspend, providing, propagation) [done]
- [x] 6.6 trait definitions in prelude (implementations in Section 7) [done]
- [x] 6.7-6.8 complete (testing/mocking, compile-time enforcement) [done]
- [ ] 6.9 Unsafe marker trait defined (FFI enforcement in Section 11)
- [ ] 6.10 Default implementations (`def impl`) — test file exists (4 tests), implementation partial
- [ ] 6.11 Capability Composition — not started
- [ ] 6.12 Default Implementation Resolution — not started
- [ ] 6.13 Named Capability Sets (`capset`) — not started
- [ ] 6.14 Intrinsics Capability — not started
- [ ] 6.16 Stateful Handlers — not started
- [ ] LLVM codegen for capabilities — no test files exist
- [ ] Full test suite: `./test-all.sh`

**Exit Criteria**: Effect tracking works per spec (6.1-6.8 evaluator complete, 6.9-6.14, 6.16 pending)
**Status**: Verified 2026-02-10.

**Remaining for Section 7 (Stdlib)**:
- Real capability implementations (Http, FileSystem, etc.)
- Integration with stdlib modules

**Remaining for Section 11 (FFI)**:
- Unsafe capability enforcement for extern functions
