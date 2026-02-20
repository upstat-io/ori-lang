---
section: "03"
title: Type Checker Unification
status: not-started
goal: Merge Run/Block inference paths, fix TypeEnv mutability tracking, unify statement type checking
sections:
  - id: "03.1"
    title: Merge Block/Run Inference
    status: not-started
  - id: "03.2"
    title: Unify Statement Type Checking
    status: not-started
  - id: "03.3"
    title: Fix TypeEnv Mutability
    status: not-started
  - id: "03.4"
    title: Update Try Inference
    status: not-started
  - id: "03.5"
    title: Update Bidirectional Checking
    status: not-started
---

# Section 03: Type Checker Unification

**Status:** 📋 Planned
**Goal:** One code path for block inference. One statement type-checking function. Clean TypeEnv with no parallel maps.

**BE EXHAUSTIVE.** The type checker is the most complex consumer. Every function in `sequences.rs` that handles `SeqBinding` must be rewritten. Every match arm on `FunctionSeq::Run` must be removed. Every call to `infer_run_seq()` must be redirected. Do not leave dead code — remove it completely.

---

## 03.1 Merge Block/Run Inference

**Files:**
- `compiler/ori_types/src/infer/expr/sequences.rs` — `infer_function_seq()`, `infer_run_seq()`
- `compiler/ori_types/src/infer/expr/mod.rs` — `infer_expr()` dispatch, `check_run_seq()`
- `compiler/ori_types/src/infer/expr/blocks.rs` — `infer_block()` (if exists, or wherever block inference lives)

- [ ] Locate `infer_block()` — where does `ExprKind::Block { stmts, result }` get type-checked?
  - It should be in `mod.rs` or a `blocks.rs` sub-file
  - This is the SURVIVOR — all block inference routes through here
- [ ] Remove `infer_run_seq()` entirely:
  - Its logic (enter scope → process bindings → infer result → exit scope) is identical to `infer_block()`
  - The only addition is `infer_pre_checks()` / `infer_post_checks()` — these move to function-level checking (Section 02.3)
- [ ] Remove `FunctionSeq::Run` match arm from `infer_function_seq()`:
  - `infer_function_seq()` should now only have: `Try`, `Match`, `ForPattern`
- [ ] Remove `check_run_seq()` from `mod.rs`:
  - This was bidirectional checking for Run blocks — merge into `check_block()` or equivalent
  - Grep for `check_run_seq` to find all callers
- [ ] Verify `infer_block()` handles:
  - Sequential let bindings with scoping
  - Statement expressions (infer, discard result)
  - Final expression as block value
  - Mutability tracking via `bind_with_mutability()`
  - Let-polymorphism (generalization of lambda bindings)
  - Closure self-capture detection
- [ ] Remove `infer_pre_checks()` and `infer_post_checks()` from sequences.rs if they're now dead code
  - OR move them to wherever function-level contract checking will live
- [ ] Run `grep -r 'infer_run_seq\|check_run_seq' compiler/` — MUST return zero results
- [ ] Run `cargo c -p ori_types`

---

## 03.2 Unify Statement Type Checking

**Files:**
- `compiler/ori_types/src/infer/expr/sequences.rs` — `infer_seq_binding()`

- [ ] Rename `infer_seq_binding()` → `infer_stmt()` (or find existing `infer_stmt` and merge)
- [ ] Change the parameter from `&ori_ir::SeqBinding` to `&ori_ir::Stmt`:
  ```rust
  // Before
  pub(crate) fn infer_seq_binding(engine, arena, binding: &SeqBinding, try_unwrap: bool) -> ()

  // After
  pub(crate) fn infer_stmt(engine, arena, stmt: &Stmt, try_unwrap: bool) -> ()
  ```
- [ ] Update the match arms:
  - `SeqBinding::Let { pattern, ty, value, mutable, span }` → `StmtKind::Let { pattern, ty, init, mutable }` (span from `stmt.span`)
  - `SeqBinding::Stmt { expr, span }` → `StmtKind::Expr(expr)` (span from `stmt.span`)
  - Note the field renames: `value` → `init`
- [ ] Update ALL callers of `infer_seq_binding()`:
  - `infer_run_seq()` — GONE (removed in 03.1)
  - `infer_try_seq()` — now iterates `StmtRange` via `arena.get_stmts()` instead of `arena.get_seq_bindings()`
  - `infer_block()` — should already iterate stmts (verify it calls the unified function)
  - Any other callers found by `grep -r 'infer_seq_binding' compiler/`
- [ ] Ensure `infer_block()` and `infer_try_seq()` both call the same `infer_stmt()`:
  - `infer_block()` calls `infer_stmt(engine, arena, stmt, false)` (no try unwrap)
  - `infer_try_seq()` calls `infer_stmt(engine, arena, stmt, true)` (with try unwrap)
- [ ] Run `grep -r 'infer_seq_binding' compiler/` — MUST return zero results
- [ ] Run `cargo t -p ori_types`

---

## 03.3 Fix TypeEnv Mutability

**File:** `compiler/ori_types/src/infer/env/mod.rs`

The current `TypeEnv` has two parallel `FxHashMap`s that must stay in sync:
```rust
struct TypeEnvInner {
    bindings: FxHashMap<Name, Idx>,
    mutability: FxHashMap<Name, bool>,   // PROBLEM: parallel map
    parent: Option<TypeEnv>,
}
```

- [ ] Create a `Binding` struct:
  ```rust
  #[derive(Copy, Clone, Debug)]
  struct Binding {
      ty: Idx,
      mutable: Option<Mutability>,  // None = no mutability info (prelude, params)
  }
  ```
- [ ] Replace the two maps with one:
  ```rust
  struct TypeEnvInner {
      bindings: FxHashMap<Name, Binding>,
      parent: Option<TypeEnv>,
  }
  ```
- [ ] Update `bind()` — creates `Binding { ty, mutable: None }`
- [ ] Update `bind_with_mutability()` — creates `Binding { ty, mutable: Some(mutability) }`
  - Change parameter from `mutable: bool` to `mutable: Mutability`
  - Every caller passes `Mutability` directly instead of `bool`
- [ ] Update `lookup()` — returns `Option<Idx>` by extracting `.ty` from `Binding`
  - Or change to return `Option<Binding>` and update all callers — evaluate which is cleaner
  - If keeping `Option<Idx>`, add a separate `lookup_binding()` for when mutability info is needed
- [ ] Update `is_mutable()` — extracts from `Binding.mutable` instead of separate map
- [ ] Update `bind_scheme()` — same as `bind()` (no mutability info for schemes)
- [ ] Update ALL callers of `bind_with_mutability()`:
  - `compiler/ori_types/src/infer/expr/sequences.rs` — `bind_pattern()` function
  - Any others found by `grep -r 'bind_with_mutability' compiler/`
  - Each caller currently passes `bool` — change to pass `Mutability`
- [ ] Update ALL callers of `is_mutable()`:
  - `compiler/ori_types/src/infer/expr/operators.rs` — assignment checking
  - `compiler/ori_types/src/type_error/check_error/mod.rs` — `AssignToImmutable` error construction
  - Any others found by `grep -r 'is_mutable' compiler/ori_types/`
- [ ] Run `cargo t -p ori_types`

---

## 03.4 Update Try Inference

**File:** `compiler/ori_types/src/infer/expr/sequences.rs` — `infer_try_seq()`

- [ ] Change `infer_try_seq()` parameter from `bindings: SeqBindingRange` to `stmts: StmtRange`
- [ ] Change iteration from `arena.get_seq_bindings(bindings)` to `arena.get_stmts(stmts)`
  - Verify `get_stmts()` returns `&[Stmt]` (or `&[StmtId]` with arena lookup)
- [ ] Update the binding loop to use `infer_stmt(engine, arena, stmt, true)`
- [ ] Verify error type tracking still works (extracting Result error type from try bindings)
- [ ] Run try-related tests: `cargo st tests/spec/patterns/try.ori`
- [ ] Run type checker tests: `cargo t -p ori_types`

---

## 03.5 Update Bidirectional Checking

**File:** `compiler/ori_types/src/infer/expr/mod.rs`

- [ ] Locate all bidirectional checking paths that handle `FunctionSeq`:
  - `check_run_seq()` — REMOVE (merged into block checking per 03.1)
  - Any `check_expr()` match arm for `ExprKind::FunctionSeq` → `FunctionSeq::Run` — REMOVE
- [ ] Verify `check_expr()` handles `ExprKind::Block` with expected type propagation:
  - The expected type should propagate to the result expression of the block
  - Let binding type annotations should interact correctly with expected types
- [ ] Run `grep -r 'check_run_seq' compiler/` — MUST return zero results
- [ ] Run full type checker test suite: `cargo t -p ori_types`

---

## 03.6 Completion Checklist

- [ ] `infer_run_seq()` does not exist
- [ ] `check_run_seq()` does not exist
- [ ] `infer_seq_binding()` does not exist (renamed to `infer_stmt()` or merged)
- [ ] `TypeEnv` has a single `bindings: FxHashMap<Name, Binding>` map
- [ ] `bind_with_mutability()` takes `Mutability`, not `bool`
- [ ] `infer_try_seq()` uses `StmtRange`, not `SeqBindingRange`
- [ ] All type checker tests pass: `cargo t -p ori_types`
- [ ] All spec tests pass: `cargo st`

**Exit Criteria:** The type checker has one path for block inference, one path for statement inference, and `TypeEnv` has no parallel data structures.
