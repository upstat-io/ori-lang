---
section: "01"
title: IR Type Unification
status: not-started
goal: Eliminate SeqBinding, upgrade StmtKind, restructure FunctionSeq to remove Run variant
sections:
  - id: "01.1"
    title: Upgrade StmtKind
    status: not-started
  - id: "01.2"
    title: Kill SeqBinding
    status: not-started
  - id: "01.3"
    title: Restructure FunctionSeq
    status: not-started
  - id: "01.4"
    title: Update Arena & Ranges
    status: not-started
  - id: "01.5"
    title: Update Visitor
    status: not-started
  - id: "01.6"
    title: Update Exports & Imports
    status: not-started
---

# Section 01: IR Type Unification

**Status:** 📋 Planned
**Goal:** Make `StmtKind` the single canonical statement type. Kill `SeqBinding`. Remove `FunctionSeq::Run`. Fix type inconsistencies.

**BE EXHAUSTIVE.** Every match arm, every import, every re-export, every test that references the old types must be found and updated. Use `grep -r` across the entire `compiler/` tree after each sub-step to verify zero remaining references to removed types. Do not move to the next sub-step until the current one compiles cleanly.

---

## 01.1 Upgrade `StmtKind`

**File:** `compiler/ori_ir/src/ast/stmt.rs`

The current `StmtKind` uses `bool` for mutability. Fix this and ensure it carries all information that `SeqBinding` currently carries.

- [ ] Change `StmtKind::Let.mutable` from `bool` to `Mutability`
  - Import `Mutability` from `crate::BindingPattern` (already in `ori_ir::ast::patterns::binding`)
  - Update all construction sites — grep for `StmtKind::Let {` across entire `compiler/` tree
  - Update all match arms — grep for `StmtKind::Let {` pattern matches
  - Update all tests referencing `mutable: true` / `mutable: false` on `StmtKind::Let`
- [ ] Verify `Stmt` wrapper carries span (it does — `Stmt { kind: StmtKind, span: Span }`)
  - This means `StmtKind` doesn't need its own span — confirm `SeqBinding::Let.span` and `SeqBinding::Stmt.span` are redundant with the `Stmt.span` wrapper
- [ ] Add `StmtKind::Let` documentation matching current `SeqBinding::Let` docs
- [ ] Run `cargo c` — must compile (formatter, parser, type checker will have errors — that's expected, fix them all)

**Exhaustiveness check:** `grep -r 'StmtKind::Let' compiler/ | grep -c 'mutable'` — every hit must use `Mutability`, not `bool`.

---

## 01.2 Kill `SeqBinding`

**Files:**
- `compiler/ori_ir/src/ast/patterns/seq/mod.rs` — `SeqBinding` definition
- `compiler/ori_ir/src/ast/ranges/mod.rs` — `SeqBindingRange` re-export
- `compiler/ori_ir/src/lib.rs` — public re-exports
- `compiler/ori_ir/src/arena/mod.rs` — arena methods

**Strategy:** Replace every use of `SeqBinding` with `Stmt` / `StmtKind`. Replace every use of `SeqBindingRange` with `StmtRange`.

- [ ] Audit every `SeqBinding` reference — find ALL of them:
  - `compiler/ori_ir/src/ast/patterns/seq/mod.rs` — definition (REMOVE)
  - `compiler/ori_ir/src/ast/patterns/seq/tests.rs` — tests (REWRITE)
  - `compiler/ori_ir/src/arena/mod.rs` — `alloc_seq_bindings()`, `get_seq_bindings()` (REMOVE)
  - `compiler/ori_ir/src/visitor.rs` — `visit_seq_binding()`, `walk_seq_binding()` (REMOVE)
  - `compiler/ori_ir/src/lib.rs` — re-exports (REMOVE)
  - `compiler/ori_ir/src/ast/ranges/mod.rs` — range re-export (REMOVE)
  - `compiler/ori_ir/src/ast/ranges/tests.rs` — range tests (REMOVE)
  - `compiler/ori_parse/src/grammar/expr/patterns.rs` — `parse_try_let_binding()` returns `SeqBinding` (CHANGE to `Stmt`)
  - `compiler/ori_parse/src/incremental/copier.rs` — `copy_seq_binding_range()`, `copy_seq_binding()` (CHANGE to use `Stmt`/`StmtRange`)
  - `compiler/ori_types/src/infer/expr/sequences.rs` — `infer_seq_binding()` (CHANGE to `infer_stmt()`)
  - `compiler/ori_canon/src/lower/sequences.rs` — `lower_seq_bindings()`, `lower_seq_bindings_try()` (CHANGE)
  - `compiler/ori_fmt/src/formatter/stacked.rs` — `emit_seq_binding()` (CHANGE)
  - `compiler/ori_fmt/src/formatter/inline.rs` — if present (CHECK and CHANGE)
  - `compiler/ori_fmt/src/width/mod.rs` — width calculation (CHECK and CHANGE)
- [ ] Delete `SeqBinding` enum from `ori_ir/src/ast/patterns/seq/mod.rs`
- [ ] Delete `SeqBindingRange` range type
- [ ] Delete `alloc_seq_bindings()` and `get_seq_bindings()` from arena
- [ ] Delete `visit_seq_binding()` and `walk_seq_binding()` from visitor
- [ ] Delete re-exports from `ori_ir/src/lib.rs`
- [ ] Run `grep -r 'SeqBinding' compiler/` — MUST return zero results
- [ ] Run `grep -r 'SeqBindingRange' compiler/` — MUST return zero results
- [ ] Run `grep -r 'alloc_seq_bindings\|get_seq_bindings' compiler/` — MUST return zero results

---

## 01.3 Restructure `FunctionSeq`

**File:** `compiler/ori_ir/src/ast/patterns/seq/mod.rs`

Remove `FunctionSeq::Run`. Rewrite `FunctionSeq::Try` to use `StmtRange` instead of `SeqBindingRange`.

- [ ] Remove `FunctionSeq::Run` variant entirely
  - Move `pre_checks` / `post_checks` (contracts) to the `Function` declaration in `compiler/ori_ir/src/ast/items/function.rs`
  - All function bodies are now `ExprKind::Block` — the parser already produces this
  - Check if any code still constructs `FunctionSeq::Run` — grep for `FunctionSeq::Run` across entire tree
- [ ] Rewrite `FunctionSeq::Try` to use `StmtRange`:
  ```rust
  Try {
      stmts: StmtRange,    // was: bindings: SeqBindingRange
      result: ExprId,
      span: Span,
  }
  ```
- [ ] Update `FunctionSeq::name()` method — remove `Run` arm
- [ ] Update `FunctionSeq::span()` method — remove `Run` arm
- [ ] Update ALL match arms on `FunctionSeq` across entire codebase:
  - `compiler/ori_types/src/infer/expr/sequences.rs` — `infer_function_seq()` (remove Run arm)
  - `compiler/ori_types/src/infer/expr/mod.rs` — `check_run_seq()` (REMOVE — merge into `check_block()`)
  - `compiler/ori_canon/src/lower/sequences.rs` — `lower_function_seq()` (remove Run arm)
  - `compiler/ori_fmt/src/formatter/stacked.rs` — `emit_function_seq()` (remove Run arm)
  - `compiler/ori_fmt/src/formatter/inline.rs` — (CHECK for Run arm)
  - `compiler/ori_fmt/src/rules/run_rule.rs` — (REMOVE entire file if Run-specific)
  - `compiler/ori_fmt/src/width/mod.rs` — (remove Run width)
  - `compiler/ori_ir/src/visitor.rs` — `walk_function_seq()` (remove Run arm)
  - `compiler/ori_parse/src/incremental/copier.rs` — (remove Run copying)
  - `compiler/ori_ir/src/ast/patterns/seq/tests.rs` — (remove Run tests)
  - `compiler/ori_ir/src/canon/hash/mod.rs` — (CHECK for Run hashing)
  - `compiler/ori_ir/src/canon/patterns.rs` — (CHECK for Run canonicalization)
- [ ] Run `grep -r 'FunctionSeq::Run' compiler/` — MUST return zero results
- [ ] Run `grep -r 'infer_run_seq' compiler/` — MUST return zero results

---

## 01.4 Update Arena & Ranges

**File:** `compiler/ori_ir/src/arena/mod.rs`

After killing `SeqBinding`, the arena no longer needs seq binding allocation. Verify all arena methods are consistent.

- [ ] Remove `alloc_seq_bindings()` method
- [ ] Remove `get_seq_bindings()` method
- [ ] Remove `seq_bindings: Vec<SeqBinding>` storage (or whatever the backing store is)
- [ ] Verify `alloc_stmts()` / `get_stmts()` / `start_stmts()` / `push_stmt()` / `finish_stmts()` are sufficient for all block and try statement needs
  - If `FunctionSeq::Try` now uses `StmtRange`, its statements must be allocated through the same stmt arena path
- [ ] Update any range-related tests in `compiler/ori_ir/src/ast/ranges/tests.rs`
- [ ] Run `cargo c -p ori_ir` — must compile

---

## 01.5 Update Visitor

**File:** `compiler/ori_ir/src/visitor.rs`

- [ ] Remove `visit_seq_binding()` from `Visitor` trait
- [ ] Remove `walk_seq_binding()` function
- [ ] Update `walk_function_seq()`:
  - Remove `Run` match arm entirely
  - Update `Try` arm to iterate `StmtRange` using existing `visit_stmt()` / `walk_stmt()` pattern
  - Verify `Match` and `ForPattern` arms unchanged
- [ ] Verify all visitor implementations compile — grep for `visit_seq_binding` to find any external implementors
- [ ] Run `cargo c -p ori_ir` — must compile

---

## 01.6 Update Exports & Imports

**Files:** `compiler/ori_ir/src/lib.rs`, all consuming crates' `use` statements

- [ ] Remove `SeqBinding` from `ori_ir` public exports
- [ ] Remove `SeqBindingRange` from `ori_ir` public exports
- [ ] Ensure `Stmt`, `StmtKind`, `StmtRange`, `Mutability` are exported (they should already be)
- [ ] Grep ALL consuming crates for `SeqBinding` imports and remove:
  - `compiler/ori_parse/src/grammar/expr/patterns.rs`
  - `compiler/ori_types/src/infer/expr/sequences.rs`
  - `compiler/ori_canon/src/lower/sequences.rs`
  - `compiler/ori_fmt/src/formatter/stacked.rs`
  - Any others found by `grep -r 'use.*SeqBinding' compiler/`
- [ ] Run `cargo c` across full workspace — must compile

---

## 01.7 Completion Checklist

- [ ] `SeqBinding` type does not exist anywhere in the codebase
- [ ] `SeqBindingRange` type does not exist anywhere in the codebase
- [ ] `FunctionSeq::Run` variant does not exist
- [ ] `StmtKind::Let.mutable` uses `Mutability` enum
- [ ] All consuming crates compile cleanly
- [ ] `grep -r 'SeqBinding\b' compiler/` returns zero results
- [ ] `grep -r 'FunctionSeq::Run' compiler/` returns zero results
- [ ] `grep -r 'infer_run_seq\|lower_seq_bindings\b\|emit_seq_binding' compiler/` returns zero results

**Exit Criteria:** The `ori_ir` crate has one block representation (`ExprKind::Block`) and one statement type (`StmtKind`). `FunctionSeq` has exactly three variants: `Try`, `Match`, `ForPattern`.
