# Phase 6: Capabilities System

**Goal**: Effect tracking (moved earlier to unblock Phase 8 cache and Phase 11 FFI)

> **SPEC**: `spec/14-capabilities.md`
> **DESIGN**: `design/14-capabilities/index.md`

---

## 6.1 Capability Declaration

- [x] **Implement**: `uses` clause — spec/14-capabilities.md § Capability Declaration, design/14-capabilities/02-uses-clause.md
  - [x] **Rust Tests**: `ori_parse/src/lib.rs` — uses clause parsing (4 tests)
  - [x] **Ori Tests**: `tests/spec/capabilities/declaration.ori` (3 tests)

- [x] **Implement**: Multiple capabilities — spec/14-capabilities.md § Multiple Capabilities
  - [x] **Rust Tests**: `ori_parse/src/lib.rs` — multiple capabilities parsing
  - [x] **Ori Tests**: `tests/spec/capabilities/declaration.ori` — @save_and_log example

---

## 6.2 Capability Traits

- [x] **Implement**: Capability traits — spec/14-capabilities.md § Capability Traits
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 7 tests for capability trait validation
  - [x] **Ori Tests**: `tests/spec/capabilities/traits.ori` — 5 tests for capability traits

---

## 6.3 Async Capability

- [x] **Implement**: Explicit suspension declaration — spec/14-capabilities.md § Async Capability
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 4 tests (marker trait, signature storage, combined capabilities, sync function)
  - [x] **Ori Tests**: `tests/spec/capabilities/async.ori` (5 tests)

- [x] **Implement**: Sync vs async behavior — spec/14-capabilities.md § Async Capability
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs::test_sync_function_no_async_capability`
  - [x] **Ori Tests**: `tests/spec/capabilities/async.ori` — sync_fetch vs async_fetch examples

- [x] **Implement**: No `async` type modifier — spec/14-capabilities.md § Async Capability
  - [x] **Rust Tests**: `ori_parse/src/lib.rs::test_no_async_type_modifier`, `test_async_keyword_reserved`
  - [x] **Ori Tests**: `tests/spec/capabilities/async.ori` — design notes document this

- [x] **Implement**: No `await` expression — spec/14-capabilities.md § Async Capability
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs::test_await_syntax_not_supported`
  - [x] **Ori Tests**: `tests/spec/capabilities/async.ori` — design notes document this

- [ ] **Implement**: Concurrency with `parallel` — spec/14-capabilities.md § Async Capability
  - [ ] **Deferred to Phase 8**: `parallel` pattern evaluation
  - [ ] **Ori Tests**: `tests/spec/patterns/parallel.ori` (Phase 8)

---

## 6.4 Providing Capabilities

- [x] **Implement**: `with...in` expression — spec/14-capabilities.md § Providing Capabilities, design/14-capabilities/index.md
  - [x] **Rust Tests**: `ori_parse/src/lib.rs` — with expression parsing (3 tests)
  - [x] **Rust Tests**: `oric/src/eval/evaluator/mod.rs` — with expression evaluation
  - [x] **Ori Tests**: `tests/spec/capabilities/providing.ori` (7 tests)

- [x] **Implement**: Scoping — spec/14-capabilities.md § Capability Scoping
  - [x] **Rust Tests**: `oric/src/eval/evaluator/mod.rs` — capability scoping via push_scope/pop_scope
  - [x] **Ori Tests**: `tests/spec/capabilities/providing.ori` — scoping and shadowing tests

---

## 6.5 Capability Propagation

- [x] **Implement**: Runtime capability propagation — capabilities flow through function calls
  - [x] **Changes**: `FunctionValue` now stores capabilities, `eval_call` passes them to called functions
  - [x] **Ori Tests**: `tests/spec/capabilities/traits.ori` — tests capability propagation

- [x] **Implement**: Static transitive requirements — spec/14-capabilities.md § Capability Propagation
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 7 tests for capability propagation (E2014)
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — 8 tests for propagation

- [x] **Implement**: Providing vs requiring — spec/14-capabilities.md § Capability Propagation
  - [x] **Rust Tests**: `oric/src/typeck/infer/call.rs` — check_capability_propagation function
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — tests with...in providing capabilities

---

## 6.6 Standard Capabilities

> **STATUS**: Trait definitions complete in `library/std/prelude.ori`
> Real implementations deferred to Phase 7 (Stdlib).

- [x] **Define**: Trait interfaces — spec/14-capabilities.md § Standard Capabilities
  - [x] **Location**: `library/std/prelude.ori` — trait definitions
  - [x] **Traits**: Http, FileSystem, Cache, Clock, Random, Logger, Env

- [ ] **Implement** (Phase 7): Real capability implementations
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

- [x] **Implement**: Mock implementations — spec/14-capabilities.md § Testing with Capabilities
  - [x] **Rust Tests**: Type checking handles trait implementations for capability mocking
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — MockHttp, MockLogger examples

- [x] **Implement**: Test example — spec/14-capabilities.md § Testing with Capabilities
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — shows test patterns with `with...in`

---

## 6.8 Capability Constraints

> **STATUS**: Complete — compile-time enforcement via E2014 propagation errors

- [x] **Implement**: Compile-time enforcement — spec/14-capabilities.md § Compile-time Enforcement
  - [x] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — 7 tests for E2014 propagation errors
  - [x] **Ori Tests**: `tests/spec/capabilities/propagation.ori` — caller must declare or provide capabilities

---

## 6.9 Unsafe Capability (FFI Prep)

> **PREREQUISITE FOR**: Phase 11 (FFI)
> The Unsafe capability is required for FFI. Implement this before starting FFI work.

- [x] **Implement**: `Unsafe` marker capability
  - [x] Defined in prelude as marker trait (no methods): `library/std/prelude.ori`
  - [ ] **Ori Tests**: `tests/spec/capabilities/unsafe.ori` — basic tests

- [ ] **Implement**: Unsafe capability requirements (Phase 11)
  - [ ] Required for: raw pointer operations (future)
  - [ ] Required for: extern function calls (future)
  - [ ] Required for: unsafe blocks (future)
  - [ ] Tests added when FFI implemented

- [ ] **Implement**: AllowUnsafe provider type (Phase 11)
  - [ ] Concrete type that satisfies Unsafe capability
  - [ ] For use in tests: `with Unsafe = AllowUnsafe in ...`
  - [ ] Added when FFI tests need it

---

## 6.10 Default Implementations (`def impl`)

**Proposal**: `proposals/approved/default-impl-proposal.md`

Introduce `def impl` syntax to declare a default implementation for a trait. Importing a trait with a `def impl` automatically binds the default.

### Implementation

- [ ] **Implement**: Add `def` keyword to lexer — grammar.ebnf § DECLARATIONS
  - [ ] **Rust Tests**: `ori_lexer/src/lib.rs` — `def` token recognition
  - [ ] **Ori Tests**: `tests/spec/capabilities/default-impl.ori`

- [ ] **Implement**: Parse `def impl Trait { ... }` — grammar.ebnf § DECLARATIONS
  - [ ] **Rust Tests**: `ori_parse/src/lib.rs` — DefImpl AST node parsing
  - [ ] **Ori Tests**: `tests/spec/capabilities/default-impl.ori`

- [ ] **Implement**: IR representation for DefImpl
  - [ ] Add `DefImpl` to module items
  - [ ] Track which traits have defaults in module metadata

- [ ] **Implement**: Type checking for `def impl`
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs`
  - [ ] Verify all trait methods implemented
  - [ ] Verify method signatures match trait
  - [ ] Verify no `self` parameter (stateless)
  - [ ] Verify one `def impl` per trait per module

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

- [ ] **Implement**: Async binding prohibition — `with Async = ...` is compile error
  - [ ] **Rust Tests**: `oric/src/typeck/checker/tests.rs` — async prohibition (E1203)
  - [ ] **Ori Tests**: `tests/spec/capabilities/composition.ori`

- [ ] **Implement**: Error codes E1200-E1203
  - [ ] E1200: missing capability
  - [ ] E1201: unbound capability
  - [ ] E1202: type doesn't implement capability trait
  - [ ] E1203: Async cannot be explicitly bound

- [ ] **Implement**: LLVM backend support
  - [ ] **LLVM Rust Tests**: `ori_llvm/tests/capability_composition_tests.rs`

---

## 6.12 Phase Completion Checklist

- [x] 6.1-6.5 complete (declaration, traits, async, providing, propagation)
- [x] 6.6 trait definitions in prelude (implementations in Phase 7)
- [x] 6.7-6.8 complete (testing/mocking, compile-time enforcement)
- [x] 6.9 Unsafe marker trait defined (FFI enforcement in Phase 11)
- [ ] 6.10 Default implementations (`def impl`) — pending implementation
- [ ] 6.11 Capability Composition — pending implementation
- [x] Spec updated: `spec/14-capabilities.md` reflects implementation
- [x] CLAUDE.md updated with capabilities syntax
- [x] 27 capability tests passing
- [x] Full test suite: `./test-all`

**Exit Criteria**: Effect tracking works per spec (6.1-6.9 ✅, 6.10-6.11 pending)

**Remaining for Phase 7 (Stdlib)**:
- Real capability implementations (Http, FileSystem, etc.)
- Integration with stdlib modules

**Remaining for Phase 11 (FFI)**:
- Unsafe capability enforcement for extern functions
