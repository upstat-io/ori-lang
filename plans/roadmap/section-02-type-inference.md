---
section: 2
title: Complete Type Inference
status: in-progress
reviewed: false
tier: 1
goal: Full Hindley-Milner type inference
spec:
  - spec/08-types.md
  - spec/09-properties-of-types.md
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
    title: Section Completion Checklist
    status: complete
---

# Section 2: Complete Type Inference

**Goal**: Full Hindley-Milner type inference

> **SPEC**: `spec/08-types.md`, `spec/09-properties-of-types.md`

**Status**: Complete — Full Hindley-Milner inference with actionable type error messages. 740 Rust tests in ori_types (all pass), ~102 Ori spec tests across inference/bindings/lambdas (all pass; collections.ori is entirely commented out), all 38 compile-fail test files pass including 5 conversion hint tests (`int(x)`, `float(x)`, `str(x)`, `byte(x)`, `[x]`). Verified 2026-03-28.

---

## 2.1 Unification Algorithm

- [x] **Implement**: Occurs check — spec/08-types.md § Type Inference [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/unify/mod.rs` — `occurs_check_detects_infinite_type` (prevents T = [T])
  - [x] **Ori Tests**: `tests/spec/inference/unification.ori` — 25 tests (all pass)

- [x] **Implement**: Substitution application via `resolve()` — spec/08-types.md § Type Inference [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/unify/mod.rs` — path_compression, error_propagates, never_unifies_with_anything
  - [x] **Ori Tests**: `tests/spec/inference/unification.ori` — substitution verified through unification tests

- [x] **Implement**: Generalization (let-polymorphism) — spec/08-types.md § Type Inference [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/unify/mod.rs` — `generalize_identity_function`, `generalize_monomorphic`, `generalize_does_not_generalize_outer_vars`, `let_polymorphism_example`
  - [x] **Ori Tests**: `tests/spec/inference/polymorphism.ori` — 8 tests (all pass)
  - [x] **Verified**: Polymorphic identity `@id (x) = x` works with both int and str

- [x] **Implement**: Instantiation — spec/08-types.md § Type Inference [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/unify/mod.rs` — `instantiate_identity_scheme`, `instantiate_non_scheme`, `instantiate_twice_gives_different_vars`
  - [x] **Ori Tests**: `tests/spec/inference/polymorphism.ori`

---

## 2.2 Expression Type Inference

- [x] **Implement**: Local variable inference — spec/08-types.md § Type Inference [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/expr.rs` — 126 expression inference tests
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — 17 tests (all pass)
  - [x] **Verified**: `let x = 42` infers int, `let x = x + 1` chains correctly

- [x] **Implement**: Lambda parameter inference — spec/08-types.md § Type Inference [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/expr.rs` — lambda inference tests
  - [x] **Ori Tests**: `tests/spec/expressions/lambdas.ori` — 30 tests (all pass)
  - [x] **Verified**: `apply(x -> x + 1, 41)` correctly infers x: int from context

- [x] **Fix**: Closure-returning-closure inference bug [done] (verified 2026-03-28)
  - [x] `(n: int) -> (int) -> int = { (x: int) -> int = base + n + x }` — both annotated and unannotated patterns pass. `test_aot_closure_capturing_closure` in AOT and `test_closure_returning_closure_annotated` in spec both pass.

- [x] **Implement**: Generic type argument inference — spec/08-types.md § Type Inference [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/` — generic inference tests
  - [x] **Ori Tests**: `tests/spec/inference/generics.ori` — 22 tests (all pass)

- [x] **Implement**: Collection element type inference — spec/08-types.md § Type Inference [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/expr.rs` — collection inference tests
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori` — STALE: file is entirely commented out (0 active tests). Collection inference is tested indirectly via unification.ori and generics.ori.
  - [x] **Verified**: `[1, 2, 3]` infers `[int]`, `{"a": 1}` infers `{str: int}` (verified through other test files)

---

## 2.3 Type Error Improvements

- [x] **Implement**: Expected vs found messages — spec/08-types.md § Type Errors [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/` — 20+ type error tests
  - [x] **Ori Tests**: `tests/compile-fail/type_mismatch_arg.ori` — 1 test (passes)

- [x] **Implement**: Type conversion hints — spec/08-types.md § Type Errors [done] (2026-02-16)
  - [x] **Implement**: Edit-distance typo suggestions ("did you mean?") [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/env.rs` — 21 tests including edit distance for typo suggestions
  - [x] **Ori Tests**: `tests/compile-fail/type_hints.ori` — 5 non-skipped tests pass
  - [x] **Implement**: Conversion function suggestions in type mismatch errors (`int(x)`, `float(x)`, `str(x)`, `byte(x)`, `[x]`) [done] (2026-02-16)
  - [x] **Ori Tests**: `tests/compile-fail/type_hints.ori` — 5 compile_fail tests pass (float->int, int->float, int->str, str->byte, int->[int]) [done] (2026-02-16)

- [x] **Implement**: Source location in errors — spec/08-types.md § Type Errors [done] (2026-02-10)
  - [x] **Ori Tests**: `tests/compile-fail/return_type_mismatch.ori` — 1 test (passes)
  - [x] **Verified**: All type errors include span information

**Compile-fail test harness**: Implemented via `#compile_fail("expected_error")` attribute.
All 38 compile-fail test files pass: `cargo st tests/compile-fail/` (count updated 2026-03-28)

---

## 2.4 Section Completion Checklist

- [x] All 2.1 items complete — unification, occurs check, generalization, instantiation [done] (2026-02-10)
- [x] All 2.2 items complete — local variable, lambda, generic, collection inference [done] (verified 2026-03-28) — closure-returning-closure bug fixed
- [x] All 2.3 items complete — expected/found, hints, source locations [done] (2026-02-10)
- [x] 740 Rust unit tests pass (ori_types) [done] (2026-02-10, count updated 2026-03-28)
- [x] Spec and compile-fail tests pass — ~102 Ori spec tests + 38 compile-fail files [done] (2026-02-10, counts updated 2026-03-28)
- [x] Run full test suite: `./test-all.sh` — all pass [done] (2026-02-10)
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria Met**: Complete type inference with error messages and hints.
