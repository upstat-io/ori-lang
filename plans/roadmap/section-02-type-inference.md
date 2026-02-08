---
section: 2
title: Complete Type Inference
status: not-started
tier: 1
goal: Full Hindley-Milner type inference
spec:
  - spec/06-types.md
  - spec/07-properties-of-types.md
sections:
  - id: "2.1"
    title: Unification Algorithm
    status: not-started
  - id: "2.2"
    title: Expression Type Inference
    status: not-started
  - id: "2.3"
    title: Type Error Improvements
    status: not-started
  - id: "2.4"
    title: Section Completion Checklist
    status: not-started
---

# Section 2: Complete Type Inference

**Goal**: Full Hindley-Milner type inference

> **SPEC**: `spec/06-types.md`, `spec/07-properties-of-types.md`

---

## 2.1 Unification Algorithm

- [ ] **Implement**: Occurs check — spec/06-types.md § Type Inference
  - [ ] **Rust Tests**: `oric/src/typeck/context.rs` — `test_occurs_check`, `test_unify_*`
  - [ ] **Ori Tests**: `tests/spec/inference/unification.ori` (28 tests)

- [ ] **Implement**: Substitution application via `resolve()` — spec/06-types.md § Type Inference
  - [ ] **Rust Tests**: `oric/src/typeck/context.rs` — substitution tests
  - [ ] **Ori Tests**: `tests/spec/inference/unification.ori`

- [ ] **Implement**: Generalization (let-polymorphism) — spec/06-types.md § Type Inference
  - [ ] **Rust Tests**: `oric/src/typeck/context.rs` — `test_generalize*`
  - [ ] **Ori Tests**: `tests/spec/inference/polymorphism.ori`

- [ ] **Implement**: Instantiation — spec/06-types.md § Type Inference
  - [ ] **Rust Tests**: `oric/src/typeck/context.rs` — `test_instantiate*`
  - [ ] **Ori Tests**: `tests/spec/inference/polymorphism.ori`

---

## 2.2 Expression Type Inference

- [ ] **Implement**: Local variable inference — spec/06-types.md § Type Inference
  - [ ] **Rust Tests**: `oric/src/typeck/infer/` — binding inference tests
  - [ ] **Ori Tests**: `tests/spec/expressions/bindings.ori`

- [ ] **Implement**: Lambda parameter inference — spec/06-types.md § Type Inference
  - [ ] **Rust Tests**: `oric/src/typeck/infer/` — lambda inference tests
  - [ ] **Ori Tests**: `tests/spec/expressions/lambdas.ori`

- [ ] **Implement**: Generic type argument inference — spec/06-types.md § Type Inference
  - [ ] **Rust Tests**: `oric/src/typeck/infer/` — generic inference tests
  - [ ] **Ori Tests**: `tests/spec/inference/generics.ori`

- [ ] **Implement**: Collection element type inference — spec/06-types.md § Type Inference
  - [ ] **Rust Tests**: `oric/src/typeck/infer/` — collection inference tests
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori`

---

## 2.3 Type Error Improvements

- [ ] **Implement**: Expected vs found messages — spec/06-types.md § Type Errors
  - [ ] **Rust Tests**: `oric/src/typeck/` — error message formatting tests
  - [ ] **Ori Tests**: `tests/compile-fail/type_mismatch_arg.ori`

- [ ] **Implement**: Type conversion hints — spec/06-types.md § Type Errors
  - [ ] **Rust Tests**: `oric/src/typeck/` — hint generation tests
  - [ ] **Ori Tests**: `tests/compile-fail/type_hints.ori` (5 tests)

- [ ] **Implement**: Source location in errors — spec/06-types.md § Type Errors
  - [ ] **Rust Tests**: `oric/src/typeck/` — span tracking tests
  - [ ] **Ori Tests**: `tests/compile-fail/return_type_mismatch.ori`

**Compile-fail test harness**: Implemented via `#compile_fail("expected_error")` attribute.
All 13 compile-fail tests pass: `ori test tests/compile-fail/`

---

## 2.4 Section Completion Checklist

- [ ] All 2.1 and 2.2 items complete
- [ ] All 2.3 items complete
- [ ] 256 unit tests pass
- [ ] Spec and compile-fail tests pass
- [ ] Run full test suite: `./test-all.sh`

**Exit Criteria Met**: Complete type inference with error messages and hints.
