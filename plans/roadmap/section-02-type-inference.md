---
phase: 2
title: Complete Type Inference
status: complete
tier: 1
goal: Full Hindley-Milner type inference
spec:
  - spec/06-types.md
  - spec/07-properties-of-types.md
sections:
  - id: "2.1"
    title: Unification Algorithm
    status: complete
  - id: "2.2"
    title: Expression Type Inference
    status: complete
  - id: "2.3"
    title: Type Error Improvements
    status: complete
  - id: "2.4"
    title: Phase Completion Checklist
    status: complete
---

# Phase 2: Complete Type Inference

**Goal**: Full Hindley-Milner type inference

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`

---

## 2.1 Unification Algorithm

- [x] **Implement**: Occurs check — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `oric/src/typeck/context.rs` — `test_occurs_check`, `test_unify_*`
  - [x] **Ori Tests**: `tests/spec/inference/unification.ori` (28 tests)

- [x] **Implement**: Substitution application via `resolve()` — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `oric/src/typeck/context.rs` — substitution tests
  - [x] **Ori Tests**: `tests/spec/inference/unification.ori`

- [x] **Implement**: Generalization (let-polymorphism) — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `oric/src/typeck/context.rs` — `test_generalize*`
  - [x] **Ori Tests**: `tests/spec/inference/polymorphism.ori`

- [x] **Implement**: Instantiation — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `oric/src/typeck/context.rs` — `test_instantiate*`
  - [x] **Ori Tests**: `tests/spec/inference/polymorphism.ori`

---

## 2.2 Expression Type Inference

- [x] **Implement**: Local variable inference — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `oric/src/typeck/infer/` — binding inference tests
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori`

- [x] **Implement**: Lambda parameter inference — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `oric/src/typeck/infer/` — lambda inference tests
  - [x] **Ori Tests**: `tests/spec/expressions/lambdas.ori`

- [x] **Implement**: Generic type argument inference — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `oric/src/typeck/infer/` — generic inference tests
  - [x] **Ori Tests**: `tests/spec/inference/generics.ori`

- [x] **Implement**: Collection element type inference — spec/06-types.md § Type Inference
  - [x] **Rust Tests**: `oric/src/typeck/infer/` — collection inference tests
  - [x] **Ori Tests**: `tests/spec/types/collections.ori`

---

## 2.3 Type Error Improvements

- [x] **Implement**: Expected vs found messages — spec/06-types.md § Type Errors
  - [x] **Rust Tests**: `oric/src/typeck/` — error message formatting tests
  - [x] **Ori Tests**: `tests/compile-fail/type_mismatch_arg.ori`

- [x] **Implement**: Type conversion hints — spec/06-types.md § Type Errors
  - [x] **Rust Tests**: `oric/src/typeck/` — hint generation tests
  - [x] **Ori Tests**: `tests/compile-fail/type_hints.ori` (5 tests)

- [x] **Implement**: Source location in errors — spec/06-types.md § Type Errors
  - [x] **Rust Tests**: `oric/src/typeck/` — span tracking tests
  - [x] **Ori Tests**: `tests/compile-fail/return_type_mismatch.ori`

**Compile-fail test harness**: Implemented via `#compile_fail("expected_error")` attribute.
All 13 compile-fail tests pass: `ori test tests/compile-fail/`

---

## 2.4 Phase Completion Checklist

- [x] All 2.1 and 2.2 items complete
- [x] All 2.3 items complete
- [x] 256 unit tests pass
- [x] Spec and compile-fail tests pass
- [x] Run full test suite: `./test-all`

**Exit Criteria Met**: Complete type inference with error messages and hints.
