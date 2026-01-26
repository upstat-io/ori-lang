# Phase 2: Complete Type Inference

**Goal**: Full Hindley-Milner type inference

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`

---

## 2.1 Unification Algorithm

- [x] **Implement**: Occurs check — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `sigilc/src/typeck/context.rs` — `test_occurs_check`, `test_unify_*`
  - [x] **Sigil Tests**: `tests/spec/inference/unification.si` (28 tests)

- [x] **Implement**: Substitution application via `resolve()` — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `sigilc/src/typeck/context.rs` — substitution tests
  - [x] **Sigil Tests**: `tests/spec/inference/unification.si`

- [x] **Implement**: Generalization (let-polymorphism) — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `sigilc/src/typeck/context.rs` — `test_generalize*`
  - [x] **Sigil Tests**: `tests/spec/inference/polymorphism.si`

- [x] **Implement**: Instantiation — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `sigilc/src/typeck/context.rs` — `test_instantiate*`
  - [x] **Sigil Tests**: `tests/spec/inference/polymorphism.si`

---

## 2.2 Expression Type Inference

- [x] **Implement**: Local variable inference — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `sigilc/src/typeck/infer/` — binding inference tests
  - [x] **Sigil Tests**: `tests/spec/expressions/bindings.si`

- [x] **Implement**: Lambda parameter inference — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `sigilc/src/typeck/infer/` — lambda inference tests
  - [x] **Sigil Tests**: `tests/spec/expressions/lambdas.si`

- [x] **Implement**: Generic type argument inference — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `sigilc/src/typeck/infer/` — generic inference tests
  - [x] **Sigil Tests**: `tests/spec/inference/generics.si`

- [x] **Implement**: Collection element type inference — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `sigilc/src/typeck/infer/` — collection inference tests
  - [x] **Sigil Tests**: `tests/spec/types/collections.si`

---

## 2.3 Type Error Improvements

- [x] **Implement**: Expected vs found messages — spec/06-types.md § Type Errors
  - [x] **Rust Tests**: `sigilc/src/typeck/` — error message formatting tests
  - [x] **Sigil Tests**: `tests/compile-fail/type_mismatch_arg.si`

- [x] **Implement**: Type conversion hints — spec/06-types.md § Type Errors
  - [x] **Rust Tests**: `sigilc/src/typeck/` — hint generation tests
  - [x] **Sigil Tests**: `tests/compile-fail/type_hints.si` (5 tests)

- [x] **Implement**: Source location in errors — spec/06-types.md § Type Errors
  - [x] **Rust Tests**: `sigilc/src/typeck/` — span tracking tests
  - [x] **Sigil Tests**: `tests/compile-fail/return_type_mismatch.si`

**Compile-fail test harness**: Implemented via `#compile_fail("expected_error")` attribute.
All 13 compile-fail tests pass: `sigil test tests/compile-fail/`

---

## 2.4 Phase Completion Checklist

- [x] All 2.1 and 2.2 items complete
- [x] All 2.3 items complete
- [x] 256 unit tests pass
- [x] Spec and compile-fail tests pass
- [x] Run full test suite: `cargo test && sigil test tests/spec/`

**Exit Criteria Met**: Complete type inference with error messages and hints.
