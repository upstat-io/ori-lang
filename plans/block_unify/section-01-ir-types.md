---
section: "01"
title: IR Type Unification
status: complete
goal: Eliminate SeqBinding, upgrade StmtKind, restructure FunctionSeq to remove Run variant
sections:
  - id: "01.1"
    title: Upgrade StmtKind
    status: complete
  - id: "01.2"
    title: Kill SeqBinding
    status: complete
  - id: "01.3"
    title: Restructure FunctionSeq
    status: complete
  - id: "01.4"
    title: Update Arena & Ranges
    status: complete
  - id: "01.5"
    title: Update Visitor
    status: complete
  - id: "01.6"
    title: Update Exports & Imports
    status: complete
---

# Section 01: IR Type Unification

**Status:** Complete
**Goal:** Make `StmtKind` the single canonical statement type. Kill `SeqBinding`. Remove `FunctionSeq::Run`. Fix type inconsistencies.

**BE EXHAUSTIVE.** Every match arm, every import, every re-export, every test that references the old types must be found and updated. Use `grep -r` across the entire `compiler/` tree after each sub-step to verify zero remaining references to removed types. Do not move to the next sub-step until the current one compiles cleanly.

---

## 01.1 Upgrade `StmtKind`

**File:** `compiler/ori_ir/src/ast/stmt.rs`

The current `StmtKind` uses `bool` for mutability. Fix this and ensure it carries all information that `SeqBinding` currently carries.

- [x] Change `StmtKind::Let.mutable` from `bool` to `Mutability`
  - Import `Mutability` from `crate::BindingPattern` (already in `ori_ir::ast::patterns::binding`)
  - Update all construction sites — grep for `StmtKind::Let {` across entire `compiler/` tree
  - Update all match arms — grep for `StmtKind::Let {` pattern matches
  - Update all tests referencing `mutable: true` / `mutable: false` on `StmtKind::Let`
  - Also upgraded `ExprKind::Let.mutable` and `CanExpr::Let.mutable` from `bool` to `Mutability` for consistency
- [x] Verify `Stmt` wrapper carries span (it does — `Stmt { kind: StmtKind, span: Span }`)
  - This means `StmtKind` doesn't need its own span — confirm `SeqBinding::Let.span` and `SeqBinding::Stmt.span` are redundant with the `Stmt.span` wrapper
- [x] Add `StmtKind::Let` documentation matching current `SeqBinding::Let` docs
- [x] Run `cargo c` — must compile (formatter, parser, type checker will have errors — that's expected, fix them all)

**Exhaustiveness check:** `grep -r 'StmtKind::Let' compiler/ | grep -c 'mutable'` — every hit must use `Mutability`, not `bool`.

---

## 01.2 Kill `SeqBinding`

**Files:**
- `compiler/ori_ir/src/ast/patterns/seq/mod.rs` — `SeqBinding` definition
- `compiler/ori_ir/src/ast/ranges/mod.rs` — `SeqBindingRange` re-export
- `compiler/ori_ir/src/lib.rs` — public re-exports
- `compiler/ori_ir/src/arena/mod.rs` — arena methods

**Strategy:** Replace every use of `SeqBinding` with `Stmt` / `StmtKind`. Replace every use of `SeqBindingRange` with `StmtRange`.

- [x] Audit every `SeqBinding` reference — find ALL of them
- [x] Delete `SeqBinding` enum from `ori_ir/src/ast/patterns/seq/mod.rs`
- [x] Delete `SeqBindingRange` range type
- [x] Delete `alloc_seq_bindings()` and `get_seq_bindings()` from arena
- [x] Delete `visit_seq_binding()` and `walk_seq_binding()` from visitor
- [x] Delete re-exports from `ori_ir/src/lib.rs`
- [x] Run `grep -r 'SeqBinding' compiler/` — returns zero results (source files only)
- [x] Run `grep -r 'SeqBindingRange' compiler/` — returns zero results
- [x] Run `grep -r 'alloc_seq_bindings\|get_seq_bindings' compiler/` — returns zero results

---

## 01.3 Restructure `FunctionSeq`

**File:** `compiler/ori_ir/src/ast/patterns/seq/mod.rs`

Remove `FunctionSeq::Run`. Rewrite `FunctionSeq::Try` to use `StmtRange` instead of `SeqBindingRange`.

- [x] Remove `FunctionSeq::Run` variant entirely
  - All function bodies are now `ExprKind::Block` — the parser already produces this
  - Verified no code constructs `FunctionSeq::Run` — `grep -r 'FunctionSeq::Run'` returns zero results
- [x] Rewrite `FunctionSeq::Try` to use `StmtRange`:
  ```rust
  Try {
      stmts: StmtRange,    // was: bindings: SeqBindingRange
      result: ExprId,
      span: Span,
  }
  ```
- [x] Update `FunctionSeq::name()` method — removed `Run` arm
- [x] Update `FunctionSeq::span()` method — removed `Run` arm
- [x] Update ALL match arms on `FunctionSeq` across entire codebase (7 crates, 15+ files)
- [x] Run `grep -r 'FunctionSeq::Run' compiler/` — returns zero results
- [x] Run `grep -r 'infer_run_seq' compiler/` — returns zero results

---

## 01.4 Update Arena & Ranges

**File:** `compiler/ori_ir/src/arena/mod.rs`

- [x] Remove `alloc_seq_bindings()` method
- [x] Remove `get_seq_bindings()` method
- [x] Remove `seq_bindings: Vec<SeqBinding>` storage field
- [x] Remove `checks: Vec<CheckExpr>` storage field (dead after Run removal)
- [x] Remove `alloc_checks()` and `get_checks()` methods
- [x] Remove `CheckExpr` and `CheckRange` types and range definitions
- [x] Remove `define_direct_append!` for checks
- [x] Verify `start_stmts()` / `push_stmt()` / `finish_stmts()` are sufficient for all block and try statement needs
- [x] Update range tests in `compiler/ori_ir/src/ast/ranges/tests.rs`
- [x] Run `cargo c -p ori_ir` — compiles cleanly

---

## 01.5 Update Visitor

**File:** `compiler/ori_ir/src/visitor.rs`

- [x] Remove `visit_seq_binding()` from `Visitor` trait
- [x] Remove `walk_seq_binding()` function
- [x] Update `walk_function_seq()`:
  - Removed `Run` match arm
  - Updated `Try` arm to iterate `StmtRange` using existing `visit_stmt()` / `walk_stmt()`
  - `Match` and `ForPattern` arms unchanged
- [x] Verified no external implementors of `visit_seq_binding`
- [x] Run `cargo c -p ori_ir` — compiles cleanly

---

## 01.6 Update Exports & Imports

**Files:** `compiler/ori_ir/src/lib.rs`, all consuming crates' `use` statements

- [x] Remove `SeqBinding` from `ori_ir` public exports
- [x] Remove `SeqBindingRange` from `ori_ir` public exports
- [x] Remove `CheckExpr` from `ori_ir` public exports
- [x] Remove `CheckRange` from `ori_ir` public exports
- [x] Ensured `Stmt`, `StmtKind`, `StmtRange`, `Mutability` are exported
- [x] Removed `SeqBinding` imports from ALL consuming crates:
  - `compiler/ori_parse/src/grammar/expr/patterns.rs`
  - `compiler/ori_parse/src/incremental/copier.rs`
  - `compiler/ori_types/src/infer/expr/sequences.rs`
  - `compiler/ori_canon/src/lower/sequences.rs`
  - `compiler/ori_fmt/src/formatter/stacked.rs`
  - `compiler/ori_ir/src/visitor.rs`
  - `compiler/ori_ir/src/arena/mod.rs`
- [x] Run `cargo c` across full workspace — compiles cleanly

---

## 01.7 Completion Checklist

- [x] `SeqBinding` type does not exist anywhere in the codebase
- [x] `SeqBindingRange` type does not exist anywhere in the codebase
- [x] `CheckExpr` type does not exist anywhere in the codebase
- [x] `CheckRange` type does not exist anywhere in the codebase
- [x] `FunctionSeq::Run` variant does not exist
- [x] `StmtKind::Let.mutable` uses `Mutability` enum
- [x] All consuming crates compile cleanly
- [x] `grep -r 'SeqBinding\b' compiler/ --include='*.rs'` returns zero results
- [x] `grep -r 'FunctionSeq::Run' compiler/ --include='*.rs'` returns zero results
- [x] `grep -r 'CheckExpr\|CheckRange' compiler/ --include='*.rs'` returns zero results
- [x] `grep -r 'infer_run_seq\|lower_seq_bindings\b\|emit_seq_binding' compiler/ --include='*.rs'` returns zero results
- [x] All 10,215 tests pass
- [x] Clippy clean

**Exit Criteria:** The `ori_ir` crate has one block representation (`ExprKind::Block`) and one statement type (`StmtKind`). `FunctionSeq` has exactly three variants: `Try`, `Match`, `ForPattern`. ✅ Met.
