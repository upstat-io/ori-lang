---
section: "02"
title: Parser Unification
status: complete
goal: Desugar all Run-producing paths to Block, unify block parsing, migrate contracts
sections:
  - id: "02.1"
    title: Kill Run Production
    status: complete
  - id: "02.2"
    title: Unify Block Parsing
    status: complete
  - id: "02.3"
    title: Migrate Contracts
    status: complete
  - id: "02.4"
    title: Update Try Parsing
    status: complete
  - id: "02.5"
    title: Update Incremental Copier
    status: complete
---

# Section 02: Parser Unification

**Status:** Complete
**Goal:** The parser produces `ExprKind::Block` for all sequential code. `FunctionSeq::Run` is never constructed. Try blocks use `StmtRange`. Block-parsing logic is shared, not duplicated.

**BE EXHAUSTIVE.** Grep for every construction of `FunctionSeq::Run`, `SeqBinding::Let`, `SeqBinding::Stmt` in the parser. Every single one must be replaced. Do not leave any code path that constructs the old types. After each sub-step, run `cargo c -p ori_parse` to verify.

---

## 02.1 Kill Run Production

**Goal:** No code path in the parser constructs `FunctionSeq::Run`.

- [x] Find every location that constructs `FunctionSeq::Run`:
  - `grep -r 'FunctionSeq::Run' compiler/ori_parse/` — returns zero results
  - Function body parsing already switched to `Block` during Section 01
  - `run()` migration emits E1002 error, does not produce a `Run`
- [x] For each construction site, replace with `ExprKind::Block` production:
  - Function bodies → `ExprKind::Block { stmts, result }` (already done in Section 01)
  - Any remaining `run()` usage → error (already done per commit message)
- [x] Remove `run()` error migration code if it constructs a `FunctionSeq::Run` even as error recovery
  - Verified: error path returns `ParseOutcome::consumed_err`, does not construct `Run`
- [x] Verify: `grep -r 'FunctionSeq::Run' compiler/ori_parse/` returns zero results
- [x] Run `cargo c -p ori_parse`

---

## 02.2 Unify Block Parsing

**Goal:** Extract shared block-statement parsing logic into a single helper.

**Files:**
- `compiler/ori_parse/src/grammar/expr/blocks.rs` — shared `collect_block_stmts()` + `parse_let_stmt()`
- `compiler/ori_parse/src/grammar/expr/primary.rs` — `parse_block_expr_body()` (now 20 lines)
- `compiler/ori_parse/src/grammar/expr/patterns.rs` — `parse_try_block()` (now 25 lines)

- [x] Create a shared `collect_block_stmts()` helper that:
  - Parses a sequence of statements inside `{ ... }` (assumes `{` already consumed)
  - Handles `let` bindings (with `$` mutability via `parse_binding_pattern()`)
  - Handles expression statements with `;` termination
  - Handles the "last expression without `;` is the result" rule
  - Returns `(Vec<Stmt>, ExprId, Span)` — statements + result + closing brace span
  - Takes `block_name: &str` for parameterized error messages ("block" vs "try block")
- [x] Create unified `parse_let_stmt()` helper (replaces both `parse_block_let_binding` and `parse_try_let_binding`)
- [x] Rewrite `parse_block_expr_body()` to use the shared helper
- [x] Rewrite `parse_try_block()` to use the shared helper
  - Try blocks still use `FunctionSeq::Try` for auto-unwrap semantics
  - Statements are `Stmt`/`StmtKind` (not `SeqBinding`)
  - Shared helper returns `Vec<Stmt>`, try batch-pushes to get `StmtRange`
- [x] Verify both paths produce identical semicolon/newline/error behavior
- [x] Run ALL parser tests: `cargo t -p ori_parse` — all pass
- [x] Run spec tests that exercise blocks and try: `cargo st` — 3848 passed, 0 failed

---

## 02.3 Migrate Contracts (pre/post checks)

**Goal:** Move function contracts from `FunctionSeq::Run` to `Function` declaration.

- [x] Audit: Are contracts (`pre_check:`, `post_check:`) currently used in any `.ori` test files?
  - No tests use them. Old contract infrastructure (`CheckExpr`, `CheckRange`) was fully removed during Section 01.
  - `FunctionSeq::Run` (which carried `pre_checks`/`post_checks`) no longer exists.
- [x] Contract types (`CheckExpr`, `CheckRange`) fully removed in Section 01
- [x] When contracts are implemented in the future, they will be added to `Function` struct directly
  - Future syntax defined in block-expression-syntax.md proposal
- [x] No code to migrate — old infrastructure cleanly removed, nothing to move
- [x] Run `cargo c` — compiles cleanly

---

## 02.4 Update Try Parsing

**Goal:** `parse_try_block()` produces `FunctionSeq::Try` with `StmtRange` instead of `SeqBindingRange`.

- [x] `parse_try_block()` collects `Vec<Stmt>` (done in Section 01)
- [x] `parse_try_let_binding()` replaced by unified `parse_let_stmt()` returning `Stmt` (done in 02.2)
- [x] Statements allocated via `arena.start_stmts()` / `push_stmt()` / `finish_stmts()` batch pattern
- [x] Constructs `FunctionSeq::Try { stmts: StmtRange, result, span }` correctly
- [x] Run parser tests: `cargo t -p ori_parse` — all pass
- [x] Run try-related spec tests: `cargo st tests/spec/patterns/try.ori tests/spec/patterns/catch.ori` — all pass

---

## 02.5 Update Incremental Copier

**File:** `compiler/ori_parse/src/incremental/copier.rs`

- [x] `copy_seq_binding_range()` method already removed (Section 01)
- [x] `copy_seq_binding()` method already removed (Section 01)
- [x] `FunctionSeq` copying uses `StmtRange` copying for `Try` variant
- [x] `Run` variant removed from copier's `FunctionSeq` match (Section 01)
- [x] Run incremental parsing tests: `cargo t -p ori_parse` — all pass

---

## 02.6 Completion Checklist

- [x] `grep -r 'SeqBinding' compiler/ori_parse/` returns zero results
- [x] `grep -r 'FunctionSeq::Run' compiler/ori_parse/` returns zero results
- [x] `grep -r 'alloc_seq_bindings\|get_seq_bindings' compiler/ori_parse/` returns zero results
- [x] Block and try parsing share a common statement-parsing helper (`collect_block_stmts`)
- [x] All parser tests pass: `cargo t -p ori_parse`
- [x] Spec tests for blocks, try, match all pass
- [x] All 10,219 tests pass: `./test-all.sh`
- [x] Clippy clean: `./clippy-all.sh`

**Exit Criteria:** The parser never constructs `FunctionSeq::Run` or `SeqBinding`. Block parsing has a single shared implementation (`blocks.rs`). Try blocks use `StmtRange`. ✅ Met.
