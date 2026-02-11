---
section: "03"
title: Remove Entry-Point Fallbacks
status: complete
goal: All oric entry points call eval_can() unconditionally — no legacy eval(ExprId) dispatch
sections:
  - id: "03.1"
    title: Remove run.rs Fallback
    status: complete
  - id: "03.2"
    title: Remove Test Runner Fallback
    status: complete
  - id: "03.3"
    title: Remove Query/Harness Fallbacks
    status: complete
  - id: "03.4"
    title: Remove Public eval(ExprId) Method
    status: complete
  - id: "03.5"
    title: Completion Checklist
    status: complete
---

# Section 03: Remove Entry-Point Fallbacks

**Status:** Complete
**Goal:** All entry points call `eval_can()` unconditionally. Remove `if root_for { eval_can } else { eval }` dispatch.

**Prerequisite:** Section 01 (audit confirms all functions have canonical roots) and Section 02 (all FunctionExp patterns handled canonically).

---

## 03.1 Remove run.rs Fallback — Complete

**File:** `oric/src/commands/run.rs`

- [x] Replaced `if/else` with `match` on `shared_canon.root_for(func.name)` — returns graceful failure on `None`
- [x] No `expect()` (denied by clippy)
- [x] Verified compilation and tests pass

## 03.2 Remove Test Runner Fallback — Complete

**File:** `oric/src/test/runner.rs`

- [x] Replaced `expect()` with `let Some(...) else` returning `TestResult::failed`
- [x] Verified `cargo st` passes (3052 passed, 0 failed, 58 skipped)

## 03.3 Remove Query/Harness Fallbacks — Complete

**Files:** `oric/src/query/mod.rs`, `oric/src/testing/harness.rs`

- [x] `query/mod.rs`: Replaced `expect()` with `let Some(...) else` returning `ModuleEvalResult::failure`
- [x] `harness.rs`: Converted `eval_expr()` and `eval_source()` from legacy `evaluator.eval(func.body)` to full canonical pipeline (lex → parse → typecheck → canonicalize → eval_can). All 6 harness tests pass.

## 03.4 Remove Public eval(ExprId) Method — Complete (partial)

- [x] Removed `Evaluator::eval()` wrapper from `oric/src/eval/evaluator/mod.rs` (dead code, no callers)
- [x] `Interpreter::eval()` remains `pub` because `PatternExecutor` trait (cross-crate in `ori_patterns`) delegates to it
- [x] Full removal deferred to Section 06 when `PatternExecutor` is deleted

## 03.5 Completion Checklist

- [x] `run.rs` calls `eval_can` unconditionally
- [x] Test runner uses canonical path only
- [x] Query entry point uses canonical path only
- [x] Harness uses full canonical pipeline
- [x] `Evaluator::eval(ExprId)` wrapper removed
- [x] `Interpreter::eval(ExprId)` remains `pub` for `PatternExecutor` (deferred to Section 06)
- [x] `cargo c -p oric` compiles clean
- [x] `./test-all.sh` passes (3052 passed, 0 failed, 58 skipped)
- [x] `./clippy-all.sh` passes

**Exit Criteria:** No code outside `ori_eval` calls `Interpreter::eval(ExprId)` except through `PatternExecutor` trait (deleted in Section 06). All entry points unconditionally use `eval_can(CanId)`.
