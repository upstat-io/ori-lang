---
section: "03"
title: Type Checker Unification
status: complete
goal: Merge Run/Block inference paths, fix TypeEnv mutability tracking, unify statement type checking
sections:
  - id: "03.1"
    title: Merge Block/Run Inference
    status: complete
  - id: "03.2"
    title: Unify Statement Type Checking
    status: complete
  - id: "03.3"
    title: Fix TypeEnv Mutability
    status: complete
  - id: "03.4"
    title: Update Try Inference
    status: complete
  - id: "03.5"
    title: Update Bidirectional Checking
    status: complete
---

# Section 03: Type Checker Unification

**Status:** Complete
**Goal:** One code path for block inference. One statement type-checking function. Clean TypeEnv with no parallel maps.

---

## 03.1 Merge Block/Run Inference

**Completed via Section 01 cascade.** `FunctionSeq::Run` was removed from `ori_ir`, so all Run inference paths were removed. `infer_block()` is the sole block inference path. `infer_function_seq()` only handles `Try`, `Match`, `ForPattern`.

- [x] `infer_run_seq()` removed — no longer exists
- [x] `FunctionSeq::Run` match arm removed from `infer_function_seq()`
- [x] `check_run_seq()` removed from `mod.rs`
- [x] `infer_block()` handles: sequential let bindings, expression statements, final expression, mutability, let-polymorphism, closure self-capture
- [x] `infer_pre_checks()` / `infer_post_checks()` removed (contracts not yet implemented)
- [x] `grep -r 'infer_run_seq\|check_run_seq' compiler/` returns zero results
- [x] `cargo c -p ori_types` clean

---

## 03.2 Unify Statement Type Checking

**Completed via Section 01 cascade.** `SeqBinding` type removed from `ori_ir`. `infer_try_stmt()` in `sequences.rs` handles `StmtKind::Let` and `StmtKind::Expr`. `infer_block()` in `blocks.rs` iterates `StmtRange` via `arena.get_stmt_range()`.

- [x] `infer_seq_binding()` no longer exists — replaced by `infer_try_stmt()` and block inference
- [x] Parameter types use `&ori_ir::Stmt` (via `StmtKind::Let`/`StmtKind::Expr`)
- [x] `infer_block()` and `infer_try_seq()` both iterate `StmtRange`
- [x] `grep -r 'infer_seq_binding' compiler/` returns zero results
- [x] `cargo t -p ori_types` passes

---

## 03.3 Fix TypeEnv Mutability

**File:** `compiler/ori_types/src/infer/env/mod.rs`

Merged two parallel `FxHashMap`s (`bindings` + `mutability`) into single map with `Binding` struct.

- [x] Create `Binding` struct with `ty: Idx` and `mutable: Option<Mutability>`
- [x] Replace two maps with one: `bindings: FxHashMap<Name, Binding>`
- [x] Update `bind()` — creates `Binding { ty, mutable: None }`
- [x] Update `bind_with_mutability()` — takes `Mutability` enum, creates `Binding { ty, mutable: Some(mutability) }`
- [x] Update `lookup()` — returns `Option<Idx>` by extracting `.ty`
- [x] Update `is_mutable()` — extracts from `Binding.mutable` via `Mutability::is_mutable`
- [x] Update `bind_scheme()` — delegates to `bind()` (no mutability info)
- [x] Update ALL callers of `bind_with_mutability()` — 4 sites in `sequences.rs` now pass `Mutability` directly
- [x] `is_mutable()` callers unchanged — still returns `Option<bool>` (compatible interface)
- [x] All tests pass: 10,219 passed, 0 failed
- [x] Clippy clean

---

## 03.4 Update Try Inference

**Completed via Section 01 cascade.** `infer_try_seq()` already takes `stmts: StmtRange` and iterates via `arena.get_stmt_range(stmts)`. Error type tracking works through `StmtKind::Let` pattern matching.

- [x] `infer_try_seq()` parameter is `stmts: ori_ir::StmtRange`
- [x] Iteration via `arena.get_stmt_range(stmts)` returns `&[Stmt]`
- [x] Uses `infer_try_stmt()` for each statement with auto-unwrap
- [x] Error type tracking works (extracts Result error type from try bindings)
- [x] Try-related spec tests pass
- [x] Type checker tests pass

---

## 03.5 Update Bidirectional Checking

**Completed via Section 01 cascade.** No `check_run_seq()` exists. `check_expr()` handles `ExprKind::Block` with expected type propagation to the result expression.

- [x] `check_run_seq()` removed
- [x] `check_expr()` handles `ExprKind::Block` with expected type propagation
- [x] `grep -r 'check_run_seq' compiler/` returns zero results
- [x] Full type checker test suite passes

---

## 03.6 Completion Checklist

- [x] `infer_run_seq()` does not exist
- [x] `check_run_seq()` does not exist
- [x] `infer_seq_binding()` does not exist (replaced by `infer_try_stmt()` + block inference)
- [x] `TypeEnv` has a single `bindings: FxHashMap<Name, Binding>` map
- [x] `bind_with_mutability()` takes `Mutability`, not `bool`
- [x] `infer_try_seq()` uses `StmtRange`, not `SeqBindingRange`
- [x] All type checker tests pass: `cargo t -p ori_types`
- [x] All spec tests pass: `cargo st`
- [x] All 10,219 tests pass: `./test-all.sh`
- [x] Clippy clean: `./clippy-all.sh`

**Exit Criteria:** The type checker has one path for block inference, one path for statement inference, and `TypeEnv` has no parallel data structures. ✅ Met.
