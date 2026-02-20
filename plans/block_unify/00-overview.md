# Block Unification Plan — Overview

> **Goal:** Eliminate the `FunctionSeq::Run` / `ExprKind::Block` duality by adopting the Gleam pattern — one unified block representation, one statement type, no parallel dispatch.

---

## The Problem

The block-syntax refactor introduced `ExprKind::Block { stmts: StmtRange, result: ExprId }` alongside the existing `FunctionSeq::Run { pre_checks, bindings, result, post_checks }`. These are **two representations of the same concept** — sequential code with let bindings and a result expression.

**Blast radius**: 27 dispatch sites across 7 crates must handle both. Adding a new block-like construct requires ~14 locations. `SeqBinding` and `StmtKind` are 90% identical types carrying the same data.

**Specific locations** (exhaustive inventory):

| Crate | File(s) | What It Handles |
|-------|---------|-----------------|
| ori_ir | `ast/patterns/seq/mod.rs` | `FunctionSeq` + `SeqBinding` definitions |
| ori_ir | `ast/stmt.rs` | `Stmt` + `StmtKind` definitions |
| ori_ir | `arena/mod.rs` | `alloc_seq_bindings()`, `get_seq_bindings()` |
| ori_ir | `visitor.rs` | `walk_function_seq()`, `walk_seq_binding()` |
| ori_ir | `expr_id/mod.rs` | `FunctionSeqId` newtype |
| ori_parse | `grammar/expr/primary.rs` | `parse_block_expr_body()` |
| ori_parse | `grammar/expr/patterns.rs` | `parse_try_block()`, `parse_try_let_binding()` |
| ori_parse | `incremental/copier.rs` | `copy_seq_binding_range()`, `copy_seq_binding()` |
| ori_types | `infer/expr/sequences.rs` | `infer_function_seq()`, `infer_run_seq()`, `infer_try_seq()`, `infer_seq_binding()` |
| ori_types | `infer/expr/mod.rs` | `check_run_seq()` bidirectional checking |
| ori_types | `infer/env/mod.rs` | `TypeEnv.mutability` parallel HashMap |
| ori_canon | `lower/sequences.rs` | `lower_function_seq()`, `lower_seq_bindings()`, `lower_seq_bindings_try()` |
| ori_eval | `interpreter/can_eval.rs` | `eval_can_block()` |
| ori_fmt | `formatter/stacked.rs` | `emit_function_seq()`, `emit_try_block()`, `emit_seq_binding()` |
| ori_fmt | `formatter/inline.rs` | Inline formatting of sequences |
| ori_fmt | `rules/run_rule.rs` | Run-specific formatting rules |
| ori_fmt | `width/mod.rs` | Width calculation for sequences |
| ori_llvm | `codegen/lower_control_flow.rs` | Block/statement codegen |

---

## The Gleam Pattern (Our Target)

Gleam's compiler has **one block type, one statement type**, parameterized over typed/untyped:

```rust
// Gleam's approach (simplified)
enum Statement<TypeT, ExpressionT> {
    Expression(ExpressionT),
    Assignment(Box<Assignment<TypeT, ExpressionT>>),
}

enum Expr {
    Block { location: Span, statements: Vec1<Statement> },
    Case { subjects, clauses },  // match — separate, NOT block-like
    // ... other variants
}
```

Key properties:
- **Blocks are sequences of statements.** That's it. No parallel "FunctionSeq::Run".
- **Match/case is separate from blocks.** It has pattern arms, not sequential statements.
- **Try is not a special block.** Gleam uses `use` + Result instead. Ori's `try { }` will keep its own node but use `StmtKind` instead of `SeqBinding`.
- **Last statement is the block's value.** Same as Ori's current `ExprKind::Block`.

---

## Architectural Decisions

### What Gets Killed
- `FunctionSeq::Run` — desugared to `ExprKind::Block` at parse time
- `SeqBinding` enum — replaced by `StmtKind` everywhere
- `SeqBindingRange` — replaced by `StmtRange`
- All `infer_run_seq()`, `lower_seq_bindings()`, `emit_run_*()` functions — merged into block handlers
- `TypeEnv.mutability` parallel HashMap — merged into single `Binding` struct

### What Stays (With Modifications)
- `FunctionSeq::Try` — keeps its own node but uses `StmtRange` + `StmtKind` instead of `SeqBindingRange` + `SeqBinding`
- `FunctionSeq::Match` — unchanged (pattern arms are genuinely different from sequential statements)
- `FunctionSeq::ForPattern` — unchanged (iterator + pattern arm is genuinely different)
- `ExprKind::Block { stmts, result }` — becomes THE canonical block representation
- `StmtKind` — upgraded: `bool` → `Mutability` enum, spans preserved in `Stmt` wrapper

### What Gets Fixed Along the Way
- `StmtKind::Let.mutable: bool` → `StmtKind::Let.mutable: Mutability` (type consistency)
- `TypeEnv` two-map pattern → single map with `Binding { ty: Idx, mutable: Option<Mutability> }`
- Duplicated block-parsing loops in parser → shared `parse_block_statements()` helper

### Where Contracts (pre/post checks) Go
- `FunctionSeq::Run.pre_checks` / `post_checks` → moved to `Function` declaration (they're function-level metadata, not block-level). The parser already produces blocks for function bodies; contracts belong on the function, not inside the body.

---

## Phasing

| Phase | Section | What | Risk |
|-------|---------|------|------|
| 1 | 01 | IR types: unify `StmtKind`, kill `SeqBinding`, restructure `FunctionSeq` | Medium — foundational; breaks compilation until consumers update |
| 2 | 02 | Parser: desugar Run→Block, unify block parsing, migrate contracts | Medium — must produce valid IR for all downstream |
| 3 | 03 | Type checker: merge inference paths, fix TypeEnv | Low — follows from IR changes |
| 4 | 04 | Downstream: canon, eval, codegen, formatter | Low — mechanical once IR is settled |
| 5 | 05 | Cleanup: dead code, full test suite, benchmarks | Low — verification pass |

**Critical path**: Phase 1 (IR) blocks everything. Phases 2-4 can be interleaved but each must compile before proceeding. Phase 5 is final validation.

---

## Exit Criteria

- [ ] `FunctionSeq::Run` variant does not exist
- [ ] `SeqBinding` type does not exist
- [ ] `SeqBindingRange` type does not exist
- [ ] All block-like constructs use `StmtKind` / `StmtRange`
- [ ] `StmtKind::Let.mutable` uses `Mutability` enum
- [ ] `TypeEnv` has a single bindings map (no parallel mutability map)
- [ ] `./test-all.sh` passes (all 10,219 tests)
- [ ] `./clippy-all.sh` passes
- [ ] No performance regression >5% on parser benchmarks
