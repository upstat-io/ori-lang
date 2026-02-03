---
section: "07"
title: Large Function Extraction
status: not-started
priority: high
goal: Split all functions >50 lines into focused helper functions
files:
  - compiler/ori_parse/src/incremental.rs
  - compiler/ori_eval/src/interpreter/mod.rs
  - compiler/oric/src/reporting/type_errors.rs
  - compiler/oric/src/reporting/semantic.rs
  - compiler/oric/src/reporting/parse.rs
  - compiler/oric/src/main.rs
  - compiler/ori_typeck/src/infer/mod.rs
  - compiler/ori_fmt/src/width/mod.rs
---

# Section 07: Large Function Extraction

**Status:** 📋 Planned
**Priority:** HIGH — Functions >50 lines are hard to understand and maintain
**Goal:** Split all functions exceeding 50 lines into focused, testable helpers

---

## Guidelines

From `.claude/rules/compiler.md`:
- Functions < 50 lines (target < 30)
- Files have single clear purpose
- No god modules (1000+ lines doing multiple things)

---

## 07.1 copy_expr (269 lines) — CRITICAL

Location: `compiler/ori_parse/src/incremental.rs:277`

This is the largest function in the codebase.

### Analysis

The function has a massive match statement copying each ExprKind variant. Group by category:

### Extraction Plan

- [ ] Create `copy/` submodule in incremental.rs or separate file

- [ ] Extract `copy_literal_expr()`
  - Int, Float, String, Char, Bool, Void

- [ ] Extract `copy_collection_expr()`
  - List, Map, Struct, Tuple, Range

- [ ] Extract `copy_operator_expr()`
  - Binary, Unary, Try, Await

- [ ] Extract `copy_control_flow_expr()`
  - If, For, Loop, Block, Match

- [ ] Extract `copy_call_expr()`
  - Call, MethodCall with named/unnamed variants

- [ ] Extract `copy_pattern_expr()`
  - FunctionSeq, FunctionExp calls

- [ ] Extract `copy_wrapper_expr()`
  - Ok, Err, Some, None

- [ ] Main function becomes dispatcher:
  ```rust
  fn copy_expr(&self, expr_id: ExprId) -> ExprId {
      let expr = self.old_arena.get_expr(expr_id);
      match &expr.kind {
          // Literals
          ExprKind::Int(_) | ExprKind::Float(_) | ... =>
              self.copy_literal_expr(expr),

          // Collections
          ExprKind::List(_) | ExprKind::Map(_) | ... =>
              self.copy_collection_expr(expr),

          // ... etc
      }
  }
  ```

---

## 07.2 eval_inner (261 lines)

Location: `compiler/ori_eval/src/interpreter/mod.rs:374`

### Current State

Already has some delegation to `exec::*` modules. Continue extraction:

### Extraction Plan

- [ ] Extract remaining inline match arms to exec modules

- [ ] Move For loop handling to `exec/control.rs`
  ```rust
  // In mod.rs:
  ExprKind::For { .. } => exec::control::eval_for(self, expr),
  ```

- [ ] Move FunctionSeq handling to `exec/pattern.rs`

- [ ] Move FunctionExp handling to `exec/pattern.rs`

- [ ] Move Match handling to `exec/control.rs`

- [ ] Target: Main match should only contain single-line delegations

---

## 07.3 type_errors::render (253 lines)

Location: `compiler/oric/src/reporting/type_errors.rs:11`

### Extraction Plan

- [ ] Group by error category:
  ```rust
  impl Render for TypeProblem {
      fn render(&self) -> Diagnostic {
          match self {
              // Type mismatches
              Self::TypeMismatch { .. } |
              Self::ReturnTypeMismatch { .. } |
              Self::IncompatibleTypes { .. } => self.render_type_mismatch(),

              // Unification errors
              Self::InfiniteType { .. } |
              Self::OccursCheck { .. } => self.render_unification_error(),

              // Method errors
              Self::UnknownMethod { .. } |
              Self::AmbiguousMethod { .. } => self.render_method_error(),

              // ... etc
          }
      }
  }
  ```

- [ ] Extract `render_type_mismatch()` (~50 lines)
- [ ] Extract `render_unification_error()` (~30 lines)
- [ ] Extract `render_method_error()` (~40 lines)
- [ ] Extract `render_trait_error()` (~40 lines)
- [ ] Extract `render_field_error()` (~30 lines)

**Note:** If Section 03 (ori_macros) is complete, this entire file is deleted!

---

## 07.4 main (220 lines)

Location: `compiler/oric/src/main.rs:12`

### Extraction Plan

- [ ] Extract command handlers:
  ```rust
  fn main() {
      let args = Args::parse();
      match args.command {
          Command::Run(opts) => commands::run::execute(opts),
          Command::Check(opts) => commands::check::execute(opts),
          Command::Build(opts) => commands::build::execute(opts),
          Command::Test(opts) => commands::test::execute(opts),
          Command::Fmt(opts) => commands::fmt::execute(opts),
          Command::Lsp(opts) => commands::lsp::execute(opts),
      }
  }
  ```

- [ ] Move argument parsing to `args.rs` or `cli.rs`
- [ ] Main should be <30 lines

---

## 07.5 infer_expr_inner (215 lines)

Location: `compiler/ori_typeck/src/infer/mod.rs:53`

### Current State

Already has some delegation to `expressions/*` modules.

### Extraction Plan

- [ ] Continue moving match arms to expression modules
- [ ] Create new modules as needed:
  - [ ] `expressions/operators.rs` for binary/unary
  - [ ] `expressions/patterns.rs` for FunctionSeq/FunctionExp
- [ ] Target: Main match only contains delegations

---

## 07.6 calculate_width (193 lines)

Location: `compiler/ori_fmt/src/width/mod.rs:128`

### Current State

Already split into submodules (calls.rs, collections.rs, etc.)

### Extraction Plan

- [ ] Review if main function can be simplified further
- [ ] Move any remaining inline handlers to submodules

---

## 07.7 semantic::render (192 lines)

Location: `compiler/oric/src/reporting/semantic.rs:10`

**Note:** If Section 03 (ori_macros) is complete, this entire file is deleted!

Otherwise:
- [ ] Group by error category
- [ ] Extract helper functions per category

---

## 07.8 Other Large Functions (100-200 lines)

Lower priority, fix when touching these files:

- [ ] `parse_match_pattern_base` (186 lines) — `grammar/expr/patterns.rs:328`
- [ ] `fusion::evaluate` (180 lines) — `ori_patterns/src/fusion.rs:92`
- [ ] `run_file_compiled` (179 lines) — `commands/run.rs:75`
- [ ] `parse_module` (162 lines) — `ori_parse/src/lib.rs:432`
- [ ] `infer_struct` (161 lines) — `infer/expressions/structs.rs:112`
- [ ] `parse::render` (157 lines) — `reporting/parse.rs:11`
- [ ] `check_module` (152 lines) — `checker/orchestration.rs:12`

---

## 07.9 Verification

- [ ] No functions >100 lines (except complex match dispatchers)
- [ ] All functions <50 lines (target <30)
- [ ] `./clippy-all` passes
- [ ] `./test-all` passes

---

## 07.N Completion Checklist

- [ ] copy_expr split into 7+ helpers
- [ ] eval_inner fully delegated
- [ ] render functions split by category (or deleted via Section 03)
- [ ] main is <30 lines
- [ ] infer_expr_inner fully delegated
- [ ] No functions >100 lines
- [ ] `./test-all` passes

**Exit Criteria:** All functions <50 lines; complex dispatchers only contain single-line delegations
