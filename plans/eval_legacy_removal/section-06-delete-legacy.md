---
section: "06"
title: Delete Legacy Code
status: complete
goal: Remove all dead legacy evaluation code — eval_inner, function_seq, dead exec functions, unused re-exports
sections:
  - id: "06.1"
    title: Delete Legacy Eval Core
    status: complete
  - id: "06.2"
    title: Delete function_seq.rs
    status: complete
  - id: "06.3"
    title: Delete Dead exec/ Functions
    status: complete
  - id: "06.4"
    title: Clean Up lib.rs and Re-exports
    status: complete
  - id: "06.5"
    title: Remove Section 01 Assertions
    status: complete
  - id: "06.6"
    title: Completion Checklist
    status: complete
---

# Section 06: Delete Legacy Code

**Status:** Complete
**Goal:** Remove all dead legacy evaluation code. ~1900 lines removed across ori_eval and oric.

**Prerequisite:** Section 05 complete (no code references legacy fields or methods).

---

## 06.1 Delete Legacy Eval Core

**File:** `ori_eval/src/interpreter/mod.rs`

- [x] Delete `eval_inner()` and its entire match body (~390 lines)
- [x] Delete `eval_expr_list()` — evaluates a list of ExprId expressions
- [x] Delete `eval_call_args()` — evaluates call arguments from ExprId
- [x] Delete `eval_cast()` — legacy type cast evaluation
- [x] Delete `eval_unary()` — legacy unary operator evaluation
- [x] Delete `eval_binary()` — legacy binary operator evaluation
- [x] Delete `eval_with_hash_length()` — legacy hash/length evaluation
- [x] Delete `eval_block()` — legacy block evaluation
- [x] Delete `eval_function_exp()` — legacy function expression evaluation
- [x] Delete `eval_for()` (including inline `ForIterator` enum) — legacy for-loop evaluation
- [x] Delete `eval_loop()` — legacy loop evaluation
- [x] Delete `eval_assign()` — legacy assignment evaluation
- [x] Delete `eval(ExprId)` entry point — replaced with panic in PatternExecutor
- [x] Delete `eval_call_named()` from function_call.rs (zero callers after eval_inner deletion)
- [x] Delete `types_match()` helper (only used by legacy eval_binary for `??`)
- [x] Delete `is_mixed_primitive_op()` (only used by legacy eval_binary)
- [x] Delete `registry: SharedRegistry<PatternRegistry>` field (only used by legacy eval_function_exp)
- [x] Delete `expr_types: Option<&'a [Idx]>` field (only used by legacy types_match)
- [x] PatternExecutor `eval()` method now panics with clear message
- [x] Updated module doc to describe `eval_can(CanId)` as sole evaluation path
- [x] Cleaned up ~20 unused imports from mod.rs

Result: `mod.rs` went from ~1847 → 749 lines (1098 lines removed).

---

## 06.2 Delete function_seq.rs

**File:** `ori_eval/src/interpreter/function_seq.rs` (~159 lines)

- [x] Verified no canonical path references `function_seq` module or types
- [x] Deleted the entire file
- [x] Removed `mod function_seq;` from `interpreter/mod.rs`

---

## 06.3 Delete Dead exec/ Functions

**Files:** `ori_eval/src/exec/expr.rs`, `ori_eval/src/exec/control.rs`

- [x] `exec::expr::eval_binary` — deleted (canonical has inline binary eval)
- [x] `exec::expr::eval_literal` — deleted (canonical has inline literals)
- [x] `exec::control::eval_match` — deleted (canonical uses decision trees)
- [x] `exec::control::eval_block` — deleted (canonical has `eval_can` sequences)
- [x] `exec::control::eval_loop` — deleted (canonical has `eval_can` loop)
- [x] `exec::control::eval_if` — deleted (test-only; canonical has inline if)
- [x] `exec::control::try_match` — deleted (canonical has `bind_can_pattern`)
- [x] `exec::control::bind_pattern` — deleted (canonical has `bind_can_pattern`)
- [x] `exec::control::eval_assign` — deleted (canonical has inline assign)
- [x] `exec::control::EnvScopeGuard` — deleted (only used by deleted functions)
- [x] `exec::control::lookup_resolution` — deleted (only used by try_match)
- [x] Deleted test modules: `bind_pattern_tests`, `eval_if_tests` (ori_eval), `literals` (oric), `if_else`, `pattern_binding`, `edge_cases` (oric)
- [x] Updated test file imports

Result: `control.rs` went from 672 → 33 lines (639 lines removed). Only `LoopAction` enum and `to_loop_action` remain.

---

## 06.4 Clean Up lib.rs and Re-exports

**Files:** `ori_eval/src/lib.rs`, `oric/src/eval/evaluator/builder.rs`

- [x] Removed `invalid_literal_pattern` from pub(crate) re-exports (dead after try_match deletion)
- [x] Removed `registry` field/method from oric's EvaluatorBuilder
- [x] Removed `expr_types` field/method from oric's EvaluatorBuilder
- [x] Removed `context` field/method from oric's EvaluatorBuilder (dead — never consumed)
- [x] Removed `.expr_types()` calls from 4 callers: run.rs, runner.rs, harness.rs, query/mod.rs
- [x] Cleaned up imports: `PatternRegistry`, `SharedRegistry`, `Idx`, `CompilerContext`
- [x] Zero dead code warnings from `cargo c -p ori_eval && cargo c -p oric`

Note: `CompilerContext` in oric is now fully dead (its only field `pattern_registry` is no longer consumed). Left for Section 07 hygiene pass.

---

## 06.5 Remove Section 01 Assertions

- [x] No Section 01 `debug_assert!` or `tracing::warn!` guards remain — all were in code deleted in 06.1-06.3

---

## 06.6 Completion Checklist

- [x] `eval_inner` deleted from `mod.rs`
- [x] All `eval_*` legacy helper functions deleted from `mod.rs`
- [x] `ForIterator` enum deleted
- [x] `function_seq.rs` deleted
- [x] All dead `exec/` functions deleted
- [x] `PatternExecutor` `eval()` panics with clear message (trait kept for other methods)
- [x] `lib.rs` re-exports cleaned up
- [x] Section 01 assertions removed (via code deletion)
- [x] No dead code warnings from `cargo c -p ori_eval`
- [x] `./clippy-all.sh` passes (zero warnings)
- [x] `./test-all.sh` passes (8419 passed, 0 failed)
- [x] `mod.rs` shrinks from ~1847 to 749 lines

**Additional fix:** Migrated WASM playground (`website/playground-wasm/src/lib.rs`) from legacy `interpreter.eval(body)` to canonical `interpreter.eval_can(can_id)`. Added `ori_canon` dependency. Passed `pattern_resolutions` and `canon` to builder. Passed `Some(&shared_canon)` to `collect_impl_methods`, `collect_extend_methods`, and `register_module_functions`.

**Exit Criteria Met:** No `eval_inner`, no `ForIterator`, no `function_seq`, no dead exec functions. Zero dead code warnings. The evaluator has a single, clean evaluation path through `can_eval.rs`. WASM playground migrated to canonical path. `./test-all.sh` fully green (8419 passed, 0 failed, WASM build passes).
