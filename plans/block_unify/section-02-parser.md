---
section: "02"
title: Parser Unification
status: not-started
goal: Desugar all Run-producing paths to Block, unify block parsing, migrate contracts
sections:
  - id: "02.1"
    title: Kill Run Production
    status: not-started
  - id: "02.2"
    title: Unify Block Parsing
    status: not-started
  - id: "02.3"
    title: Migrate Contracts
    status: not-started
  - id: "02.4"
    title: Update Try Parsing
    status: not-started
  - id: "02.5"
    title: Update Incremental Copier
    status: not-started
---

# Section 02: Parser Unification

**Status:** 📋 Planned
**Goal:** The parser produces `ExprKind::Block` for all sequential code. `FunctionSeq::Run` is never constructed. Try blocks use `StmtRange`. Block-parsing logic is shared, not duplicated.

**BE EXHAUSTIVE.** Grep for every construction of `FunctionSeq::Run`, `SeqBinding::Let`, `SeqBinding::Stmt` in the parser. Every single one must be replaced. Do not leave any code path that constructs the old types. After each sub-step, run `cargo c -p ori_parse` to verify.

---

## 02.1 Kill Run Production

**Goal:** No code path in the parser constructs `FunctionSeq::Run`.

- [ ] Find every location that constructs `FunctionSeq::Run`:
  - `grep -r 'FunctionSeq::Run' compiler/ori_parse/` — list ALL hits
  - Check function body parsing — does the parser still produce `Run` for function bodies, or has it already switched to `Block`?
  - Check `run()` migration error path — it should emit an error, not produce a `Run`
- [ ] For each construction site, replace with `ExprKind::Block` production:
  - Function bodies → `ExprKind::Block { stmts, result }` (may already be done)
  - Any remaining `run()` usage → error (already done per commit message)
- [ ] Remove `run()` error migration code if it constructs a `FunctionSeq::Run` even as error recovery
- [ ] Verify: `grep -r 'FunctionSeq::Run' compiler/ori_parse/` returns zero results
- [ ] Run `cargo c -p ori_parse`

---

## 02.2 Unify Block Parsing

**Goal:** Extract shared block-statement parsing logic into a single helper.

**Files:**
- `compiler/ori_parse/src/grammar/expr/primary.rs` — `parse_block_expr_body()` (~83 lines)
- `compiler/ori_parse/src/grammar/expr/patterns.rs` — `parse_try_block()` (~120 lines)

These two functions have ~30% identical code (loop structure, let check, semicolon consumption, newline skipping). Extract the shared pattern.

- [ ] Create a shared `parse_block_statements()` helper that:
  - Parses a sequence of statements inside `{ ... }`
  - Handles `let` bindings (with `$` mutability via `parse_binding_pattern()`)
  - Handles expression statements with `;` termination
  - Handles the "last expression without `;` is the result" rule
  - Returns `(Vec<Stmt>, Option<ExprId>)` — statements + optional result expression
  - Takes a configuration parameter or closure for statement-specific behavior (try auto-unwrap vs plain block)
- [ ] Rewrite `parse_block_expr_body()` to use the shared helper
- [ ] Rewrite `parse_try_block()` to use the shared helper
  - Try blocks still need `FunctionSeq::Try` for auto-unwrap semantics
  - But their statements should now be `Stmt`/`StmtKind`, not `SeqBinding`
  - The shared helper returns `Vec<Stmt>`, which try allocates via `arena.alloc_stmts()` to get `StmtRange`
- [ ] Verify both paths produce identical semicolon/newline/error behavior
- [ ] Run ALL parser tests: `cargo t -p ori_parse`
- [ ] Run spec tests that exercise blocks and try: `cargo st tests/spec/patterns/try.ori tests/spec/expressions/block_scope.ori`

---

## 02.3 Migrate Contracts (pre/post checks)

**Goal:** Move function contracts from `FunctionSeq::Run` to `Function` declaration.

**Files:**
- `compiler/ori_ir/src/ast/items/function.rs` — `Function` struct
- `compiler/ori_ir/src/ast/patterns/seq/mod.rs` — `FunctionSeq::Run.pre_checks`, `.post_checks`

- [ ] Audit: Are contracts (`pre_check:`, `post_check:`) currently used in any `.ori` test files?
  - `grep -r 'pre_check\|post_check\|pre()\|post()' tests/ library/`
  - If no tests use them, this is simpler — just ensure the infrastructure is preserved
- [ ] Add `pre_checks: CheckRange` and `post_checks: CheckRange` fields to `Function` struct
  - These were previously on `FunctionSeq::Run` which wrapped the function body
  - They belong on the function declaration — they're about the function contract, not the body
- [ ] Update function parsing to populate the new fields
  - Check `compiler/ori_parse/src/grammar/item/function/mod.rs` for where contracts are parsed
  - They may already be parsed at function level and threaded into `FunctionSeq::Run` — reverse that
- [ ] Update function type-checking to read contracts from `Function` instead of `FunctionSeq::Run`
  - `compiler/ori_types/src/infer/expr/sequences.rs` — `infer_pre_checks()`, `infer_post_checks()` may need to move
- [ ] Update function evaluation — same migration
- [ ] If contracts are not yet implemented (just infrastructure), document this and move the fields cleanly
- [ ] Run `cargo c`

---

## 02.4 Update Try Parsing

**Goal:** `parse_try_block()` produces `FunctionSeq::Try` with `StmtRange` instead of `SeqBindingRange`.

- [ ] Change `parse_try_block()` to collect `Vec<Stmt>` instead of `Vec<SeqBinding>`
- [ ] Change `parse_try_let_binding()` to return `Stmt` instead of `SeqBinding`
  - `SeqBinding::Let { pattern, ty, value, mutable, span }` → `Stmt::new(StmtKind::Let { pattern, ty, init: value, mutable: Mutability::from(mutable) }, span)`
  - Note the field rename: `value` → `init` (StmtKind's name)
- [ ] Allocate statements via `arena.start_stmts()` / `arena.push_stmt()` / `arena.finish_stmts()` or batch `arena.alloc_stmts()`
  - Check which pattern the block parser uses and match it
- [ ] Construct `FunctionSeq::Try { stmts: StmtRange, result, span }` instead of `FunctionSeq::Try { bindings: SeqBindingRange, result, span }`
- [ ] Run parser tests: `cargo t -p ori_parse`
- [ ] Run try-related spec tests: `cargo st tests/spec/patterns/try.ori tests/spec/patterns/catch.ori`

---

## 02.5 Update Incremental Copier

**File:** `compiler/ori_parse/src/incremental/copier.rs`

- [ ] Remove `copy_seq_binding_range()` method
- [ ] Remove `copy_seq_binding()` method
- [ ] Update `FunctionSeq` copying to use `StmtRange` copying for `Try` variant:
  - The copier should already have `copy_stmt_range()` or similar for `ExprKind::Block`
  - Reuse that for `FunctionSeq::Try.stmts`
- [ ] Remove `Run` variant from copier's `FunctionSeq` match
- [ ] Run incremental parsing tests: `cargo t -p ori_parse -- incremental`

---

## 02.6 Completion Checklist

- [ ] `grep -r 'SeqBinding' compiler/ori_parse/` returns zero results
- [ ] `grep -r 'FunctionSeq::Run' compiler/ori_parse/` returns zero results
- [ ] `grep -r 'alloc_seq_bindings\|get_seq_bindings' compiler/ori_parse/` returns zero results
- [ ] Block and try parsing share a common statement-parsing helper
- [ ] All parser tests pass: `cargo t -p ori_parse`
- [ ] Spec tests for blocks, try, match all pass

**Exit Criteria:** The parser never constructs `FunctionSeq::Run` or `SeqBinding`. Block parsing has a single shared implementation. Try blocks use `StmtRange`.
