---
section: "04"
title: Downstream Consumers
status: not-started
goal: Update canonicalization, evaluator, LLVM codegen, and formatter to use unified types
sections:
  - id: "04.1"
    title: Canonicalization (ori_canon)
    status: not-started
  - id: "04.2"
    title: Evaluator (ori_eval)
    status: not-started
  - id: "04.3"
    title: LLVM Codegen (ori_llvm)
    status: not-started
  - id: "04.4"
    title: Formatter (ori_fmt)
    status: not-started
  - id: "04.5"
    title: ARC Optimizer (ori_arc)
    status: not-started
  - id: "04.6"
    title: oric Integration
    status: not-started
---

# Section 04: Downstream Consumers

**Status:** 📋 Planned
**Goal:** Every crate downstream of `ori_ir` compiles and works with the unified types. No references to `SeqBinding`, `SeqBindingRange`, or `FunctionSeq::Run` remain anywhere.

**BE EXHAUSTIVE.** Each sub-section lists known files, but you MUST grep the entire crate for any reference to the old types. Grep patterns to use after each crate:
```bash
grep -r 'SeqBinding\|FunctionSeq::Run\|infer_run_seq\|lower_seq_bindings\b\|emit_seq_binding\|alloc_seq_bindings\|get_seq_bindings' compiler/{crate}/
```
Every single grep must return zero results before moving on.

---

## 04.1 Canonicalization (ori_canon)

**Files:**
- `compiler/ori_canon/src/lower/sequences.rs` — `lower_function_seq()`, `lower_seq_bindings()`, `lower_seq_bindings_try()`
- `compiler/ori_canon/src/lower/expr.rs` — dispatch to `lower_function_seq()`
- `compiler/ori_canon/src/lower/collections.rs` — if references StmtRange

- [ ] Remove `lower_seq_bindings()` — was for Run (now dead)
- [ ] Rename `lower_seq_bindings_try()` → `lower_try_stmts()` or similar
  - Change to iterate `StmtRange` via `arena.get_stmts()` instead of `arena.get_seq_bindings()`
  - Match on `StmtKind::Let` / `StmtKind::Expr` instead of `SeqBinding::Let` / `SeqBinding::Stmt`
  - Note field renames: `value` → `init`, `mutable: bool` → `mutable: Mutability`
- [ ] Remove `FunctionSeq::Run` match arm from `lower_function_seq()`
- [ ] Update `FunctionSeq::Try` match arm to pass `stmts: StmtRange` instead of `bindings: SeqBindingRange`
- [ ] Verify `ExprKind::Block` lowering in `lower_expr()` — this is now the only path for sequential code
- [ ] Check `lower_expr()` for any `FunctionSeq::Run` special-casing
- [ ] Run `grep -r 'SeqBinding\|FunctionSeq::Run' compiler/ori_canon/` — MUST return zero results
- [ ] Run `cargo c -p ori_canon`
- [ ] Run `cargo t -p ori_canon`

---

## 04.2 Evaluator (ori_eval)

**Files:**
- `compiler/ori_eval/src/interpreter/can_eval.rs` — `eval_can_block()`
- `compiler/ori_eval/src/environment/mod.rs` — environment bindings

- [ ] Check if the evaluator references `FunctionSeq::Run` or `SeqBinding` anywhere:
  - `grep -r 'SeqBinding\|FunctionSeq::Run\|SeqBindingRange' compiler/ori_eval/`
  - The evaluator works on canonicalized IR (CanExpr), so it may not directly reference these types
  - But check for any residual references in tests or module registration
- [ ] If the evaluator has `eval_run()` or similar that handles Run directly, remove it
- [ ] Verify `eval_can_block()` handles the unified `CanExpr::Block` correctly
- [ ] Check `compiler/ori_eval/src/environment/mod.rs` for mutability tracking:
  - Does the eval environment also track mutability? If so, ensure it uses `Mutability` not `bool`
  - Grep for `mutable` in the environment module
- [ ] Update any eval tests that construct `SeqBinding` or `FunctionSeq::Run` directly:
  - `compiler/ori_eval/src/derives/tests.rs`
  - `compiler/ori_eval/src/module_registration/tests.rs`
- [ ] Run `grep -r 'SeqBinding\|FunctionSeq::Run' compiler/ori_eval/` — MUST return zero results
- [ ] Run `cargo t -p ori_eval`

---

## 04.3 LLVM Codegen (ori_llvm)

**Files:**
- `compiler/ori_llvm/src/codegen/lower_control_flow.rs` — block/statement codegen
- `compiler/ori_llvm/src/codegen/mod.rs` — module docs mentioning FunctionSeq
- `compiler/ori_llvm/src/codegen/expr_lowerer.rs` — expression dispatch

- [ ] Check all codegen files for `FunctionSeq::Run` or `SeqBinding` references:
  - `grep -r 'SeqBinding\|FunctionSeq::Run\|SeqBindingRange' compiler/ori_llvm/`
  - Codegen works on canonicalized IR, so references may be minimal
- [ ] Update `lower_control_flow.rs` if it directly handles `StmtKind::Let.mutable: bool` — change to `Mutability`
- [ ] Update module docs in `codegen/mod.rs` that mention `FunctionSeq` (remove Run references)
- [ ] Update AOT tests that embed Ori source using `run()` syntax:
  - These should already use block syntax from the prior migration
  - But verify: `grep -r 'run(' compiler/ori_llvm/tests/` — should return zero hits for run-as-block
- [ ] Run `grep -r 'SeqBinding\|FunctionSeq::Run' compiler/ori_llvm/` — MUST return zero results
- [ ] Run `cargo c -p ori_llvm` (requires LLVM — use `cargo bl` or `./llvm-build.sh`)
- [ ] Run LLVM tests: `./llvm-test.sh`

---

## 04.4 Formatter (ori_fmt)

**Files:**
- `compiler/ori_fmt/src/formatter/stacked.rs` — `emit_function_seq()`, `emit_try_block()`, `emit_seq_binding()`
- `compiler/ori_fmt/src/formatter/inline.rs` — inline formatting
- `compiler/ori_fmt/src/rules/run_rule.rs` — Run-specific formatting rules
- `compiler/ori_fmt/src/width/mod.rs` — width calculation
- `compiler/ori_fmt/src/width/patterns/mod.rs` — pattern width
- `compiler/ori_fmt/src/width/tests.rs` — width tests
- `compiler/ori_fmt/src/declarations/functions.rs` — function formatting
- `compiler/ori_fmt/tests/property_tests.rs` — property-based tests

This is the BIGGEST downstream consumer. The formatter has the most FunctionSeq-related code.

- [ ] **Stacked formatter** (`stacked.rs`):
  - Remove `FunctionSeq::Run` match arm from `emit_function_seq()`
  - Remove `emit_run_with_checks()` if Run-specific
  - Rename `emit_seq_binding()` → `emit_stmt()` or merge into existing block statement emission
  - Update `emit_try_block()` to iterate `StmtRange` instead of `SeqBindingRange`
  - Update match arms from `SeqBinding::Let`/`SeqBinding::Stmt` to `StmtKind::Let`/`StmtKind::Expr`
- [ ] **Inline formatter** (`inline.rs`):
  - Same changes as stacked — remove Run, update Try, rename seq_binding
- [ ] **Run rule** (`rules/run_rule.rs`):
  - Evaluate if this file is entirely Run-specific
  - If so, DELETE the entire file and remove it from `mod.rs`
  - If it has reusable parts, merge them into block formatting rules
- [ ] **Width calculation** (`width/mod.rs`, `width/patterns/mod.rs`):
  - Remove Run-specific width calculations
  - Update Try width to use `StmtRange`
  - Update any `SeqBinding` width handling → `StmtKind`
- [ ] **Width tests** (`width/tests.rs`, `width/patterns/tests.rs`):
  - Update or remove tests that construct `FunctionSeq::Run` or `SeqBinding`
- [ ] **Declaration formatters** (`declarations/functions.rs`, `declarations/impls.rs`, `declarations/traits.rs`, `declarations/types.rs`, `declarations/tests_fmt.rs`):
  - Check if they reference `FunctionSeq` for function body formatting
  - Update to use block formatting
- [ ] **Property tests** (`tests/property_tests.rs`):
  - These generate random Ori code — check if they generate `run()` syntax
  - Update generators to produce block syntax
- [ ] **Incremental tests** (`tests/incremental_tests.rs`):
  - Check for embedded `run()` syntax in test strings
- [ ] Run `grep -r 'SeqBinding\|FunctionSeq::Run\|emit_seq_binding\|emit_run' compiler/ori_fmt/` — MUST return zero results
- [ ] Run `cargo t -p ori_fmt`
- [ ] Run formatter spec tests: `cargo st tests/fmt/`

---

## 04.5 ARC Optimizer (ori_arc)

**Files:**
- `compiler/ori_arc/src/lower/control_flow/tests.rs`
- `compiler/ori_arc/src/lower/patterns/mod.rs`
- `compiler/ori_arc/src/lower/patterns/tests.rs`

- [ ] Check for `FunctionSeq::Run` or `SeqBinding` references:
  - `grep -r 'SeqBinding\|FunctionSeq::Run' compiler/ori_arc/`
  - ARC works on lowered IR, so references may be indirect
- [ ] Update any match arms or test constructions
- [ ] Run `cargo c -p ori_arc`
- [ ] Run `cargo t -p ori_arc`

---

## 04.6 oric Integration (oric)

**Files:**
- `compiler/oric/src/query/tests.rs` — Salsa query tests with embedded Ori
- `compiler/oric/src/test/runner/tests.rs` — test runner tests
- `compiler/oric/src/testing/harness/mod.rs` — test harness
- `compiler/oric/src/testing/harness/tests.rs` — harness tests
- `compiler/oric/tests/phases/` — phase integration tests (parse, typecheck, codegen)
- `compiler/oric/src/reporting/typeck/mod.rs` — error reporting

- [ ] Check ALL oric files for old type references:
  - `grep -r 'SeqBinding\|FunctionSeq::Run' compiler/oric/`
- [ ] Update error reporting if it special-cases `FunctionSeq::Run`:
  - `compiler/oric/src/reporting/typeck/mod.rs`
  - `compiler/oric/src/problem/` — problem conversion
- [ ] Update phase tests that construct or assert on `FunctionSeq::Run`:
  - `compiler/oric/tests/phases/common/parse/mod.rs`
  - `compiler/oric/tests/phases/common/parse/tests.rs`
  - `compiler/oric/tests/phases/common/typecheck/mod.rs`
  - `compiler/oric/tests/phases/common/typecheck/tests.rs`
- [ ] Update any embedded Ori source that uses `run()` (should already be migrated)
- [ ] Run `grep -r 'SeqBinding\|FunctionSeq::Run' compiler/oric/` — MUST return zero results
- [ ] Run `cargo t -p oric`
- [ ] Run `cargo st` (full spec test suite)

---

## 04.7 Completion Checklist

Run these across the ENTIRE compiler tree:

- [ ] `grep -r 'SeqBinding\b' compiler/` returns zero results
- [ ] `grep -r 'SeqBindingRange' compiler/` returns zero results
- [ ] `grep -r 'FunctionSeq::Run' compiler/` returns zero results
- [ ] `grep -r 'infer_run_seq\|check_run_seq' compiler/` returns zero results
- [ ] `grep -r 'lower_seq_bindings\b' compiler/` returns zero results
- [ ] `grep -r 'emit_seq_binding\|emit_run' compiler/` returns zero results
- [ ] `grep -r 'alloc_seq_bindings\|get_seq_bindings' compiler/` returns zero results
- [ ] `grep -r 'copy_seq_binding' compiler/` returns zero results
- [ ] Every crate compiles: `cargo c`
- [ ] Every crate's tests pass: `cargo t`

**Exit Criteria:** Zero references to old types in any compiler crate. Every crate compiles and passes tests.
