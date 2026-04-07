---
section: 2
title: Complete Type Inference
status: in-progress
reviewed: true
last_verified: "2026-03-29"
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
    status: in-progress
---

# Section 2: Complete Type Inference

**Goal**: Full Hindley-Milner type inference

> **SPEC**: `spec/08-types.md`, `spec/09-properties-of-types.md`

**Status**: Complete — Full Hindley-Milner inference with actionable type error messages. 744 Rust tests in ori_types (all pass), ~97 active Ori spec tests across inference/bindings/lambdas (all pass; collections.ori is entirely commented out), all 39 compile-fail test files pass including 5 conversion hint tests (`int(x)`, `float(x)`, `str(x)`, `byte(x)`, `[x]`). Verified 2026-03-29.

---

## 2.1 Unification Algorithm

- [x] **Implement**: Occurs check — spec/08-types.md § Type Inference [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/unify/mod.rs` — `occurs_check_detects_infinite_type` (prevents T = [T])
  - [x] **Ori Tests**: `tests/spec/inference/unification.ori` — 25 tests (all pass)
  - Semantic pin: `occurs_check_detects_infinite_type`

- [x] **Implement**: Substitution application via `resolve()` — spec/08-types.md § Type Inference [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/unify/mod.rs` — path_compression, error_propagates, never_unifies_with_anything
  - [x] **Ori Tests**: `tests/spec/inference/unification.ori` — substitution verified through unification tests
  - Semantic pin: `path_compression`

- [x] **Implement**: Generalization (let-polymorphism) — spec/08-types.md § Type Inference [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/unify/mod.rs` — `generalize_identity_function`, `generalize_monomorphic`, `generalize_does_not_generalize_outer_vars`, `let_polymorphism_example`, `generalize_finds_vars_in_borrowed` (7 total)
  - [x] **Ori Tests**: `tests/spec/inference/polymorphism.ori` — 8 tests (all pass)
  - [x] **Verified**: Polymorphic identity `@id (x) = x` works with both int and str
  - Semantic pin: `let_polymorphism_example`

- [x] **Implement**: Instantiation — spec/08-types.md § Type Inference [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/unify/mod.rs` — `instantiate_identity_scheme`, `instantiate_non_scheme`, `instantiate_twice_gives_different_vars`
  - [x] **Ori Tests**: `tests/spec/inference/polymorphism.ori`
  - Semantic pin: `instantiate_twice_gives_different_vars`

---

## 2.2 Expression Type Inference

- [x] **Implement**: Local variable inference — spec/08-types.md § Type Inference [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/` — 183 inference-related tests
  - [x] **Ori Tests**: `tests/spec/expressions/bindings.ori` — 17 tests (all pass)
  - [x] **Verified**: `let x = 42` infers int, `let x = x + 1` chains correctly
  - Matrix: 9 types (int, str, bool, float, char, struct, nested struct, [int], (int,str)) / 7 patterns (inferred, annotated, shadow, struct destructure, list destructure, tuple destructure, nested destructure)

- [x] **Implement**: Lambda parameter inference — spec/08-types.md § Type Inference [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/expr.rs` — lambda inference tests
  - [x] **Ori Tests**: `tests/spec/expressions/lambdas.ori` — 30 tests (all pass)
  - [x] **Verified**: `apply(x -> x + 1, 41)` correctly infers x: int from context
  - Matrix: 10+ patterns (simple, multi-param, no-param, typed, closures, IIFE, HOF, currying, conditional, complex body)

- [x] **Fix**: Closure-returning-closure inference bug [done] (verified 2026-03-29)
  - [x] `(n: int) -> (int) -> int = { (x: int) -> int = base + n + x }` — both annotated and unannotated patterns pass. `test_aot_closure_capturing_closure` in AOT and `test_closure_returning_closure_annotated` in spec both pass.
  - Dual-execution verified (interpreter + LLVM AOT)
  - Semantic pin: `test_closure_returning_closure_annotated`

- [x] **Implement**: Generic type argument inference — spec/08-types.md § Type Inference [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/` — generic inference tests
  - [x] **Ori Tests**: `tests/spec/inference/generics.ori` — 17 active tests (all pass)
  - [ ] **Cleanup**: Remove 5 orphan stub functions (infer_option_map, infer_option_and_then, infer_result_map, infer_result_map_err, infer_result_unwrap_or) that return constants without assertions -- dead code inflating test counts
  - Semantic pin: `test_infer_option_in_result`

- [x] **Implement**: Collection element type inference — spec/08-types.md § Type Inference [done] (2026-02-10) (verified 2026-03-29)
  - [x] **Rust Tests**: `ori_types/src/infer/expr.rs` — collection inference tests
  - [ ] **Ori Tests**: `tests/spec/types/collections.ori` — STALE TEST: file is entirely commented out (0/33 active tests). Collection inference is tested indirectly via unification.ori and generics.ori. Uncomment tests that now pass, remove those still failing, or delete if fully redundant.
  - [x] **Verified**: `[1, 2, 3]` infers `[int]`, `{"a": 1}` infers `{str: int}` (verified through other test files)

---

## 2.3 Type Error Improvements

- [x] **Implement**: Expected vs found messages — spec/08-types.md § Type Errors [done] (2026-02-10) (verified 2026-03-29) WEAK TESTS
  - [x] **Rust Tests**: `ori_types/src/` — 20+ type error tests
  - [x] **Ori Tests**: `tests/compile-fail/type_mismatch_arg.ori` — 1 test (passes)
  - WEAK TESTS: single compile-fail scenario (str->int argument mismatch). No matrix of type pairs. Rust unit tests compensate.
  - [ ] **Improve**: Add compile-fail tests for more type pairs (int->str, bool->int, struct->primitive, etc.)

- [x] **Implement**: Type conversion hints — spec/08-types.md § Type Errors [done] (2026-02-16) (verified 2026-03-29)
  - [x] **Implement**: Edit-distance typo suggestions ("did you mean?") [done] (2026-02-10)
  - [x] **Rust Tests**: `ori_types/src/infer/env.rs` — 21 tests including edit distance for typo suggestions
  - [x] **Ori Tests**: `tests/compile-fail/type_hints.ori` — 5 non-skipped tests pass
  - [x] **Implement**: Conversion function suggestions in type mismatch errors (`int(x)`, `float(x)`, `str(x)`, `byte(x)`, `[x]`) [done] (2026-02-16)
  - [x] **Ori Tests**: `tests/compile-fail/type_hints.ori` — 5 compile_fail tests pass (float->int, int->float, int->str, str->byte, int->[int]) [done] (2026-02-16)

- [x] **Implement**: Source location in errors — spec/08-types.md § Type Errors [done] (2026-02-10) (verified 2026-03-29) WEAK TESTS
  - [x] **Ori Tests**: `tests/compile-fail/return_type_mismatch.ori` — 1 test (passes)
  - [x] **Verified**: All type errors include span information
  - WEAK TESTS: single scenario, does not verify span accuracy (only tests that the error is produced)

**Compile-fail test harness**: Implemented via `#compile_fail("expected_error")` attribute.
All 39 compile-fail test files pass: `cargo st tests/compile-fail/` (count updated 2026-03-29)
- [ ] **Cleanup**: Standardize compile-fail syntax -- `type_mismatch_arg.ori` and `return_type_mismatch.ori` use old `#[compile_fail("...")]` (square brackets) while `type_hints.ori` uses `#compile_fail("...")` (no brackets). Both work but should be consistent.

---

## 2.4 Section Completion Checklist

- [x] All 2.1 items complete — unification, occurs check, generalization, instantiation [done] (2026-02-10) (verified 2026-03-29)
- [x] All 2.2 items complete — local variable, lambda, generic, collection inference [done] (verified 2026-03-29) — closure-returning-closure bug fixed
- [x] All 2.3 items complete — expected/found, hints, source locations [done] (2026-02-10) (verified 2026-03-29)
- [x] 744 Rust unit tests pass (ori_types) [done] (2026-02-10, count updated 2026-03-29)
- [x] Spec and compile-fail tests pass — ~97 active Ori spec tests + 39 compile-fail files [done] (2026-02-10, counts updated 2026-03-29)
- [x] Run full test suite: `./test-all.sh` — all pass [done] (2026-02-10)
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria Met**: Complete type inference with error messages and hints.

### Hygiene Notes (non-blocking, from verification 2026-03-29)

- [ ] **BLOAT**: 4 source files over 500-line limit in unify/infer modules -- split when next touched:
  - `compiler/ori_types/src/unify/mod.rs` -- 761 lines
  - `compiler/ori_types/src/infer/expr/control_flow.rs` -- 748 lines
  - `compiler/ori_types/src/infer/mod.rs` -- 689 lines
  - `compiler/ori_types/src/infer/expr/operators.rs` -- 614 lines
- [ ] **MISSING NEGATIVE PINS**: No dedicated compile-fail test for unification rejection (e.g., `let x: int = "hello"` should fail with type mismatch). Partially covered by existing tests but not comprehensive.

- [ ] **Subsection close-out (2.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-2.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 2.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

