# Block Unification Plan — Index

> **Maintenance Notice:** Update this index when adding/modifying sections.

## Motivation

The block-syntax refactor (commit `4e0c1611`) exposed a fundamental architectural problem: **two parallel representations for sequential code**. `FunctionSeq::Run` + `SeqBinding` (the old pattern-based representation) and `ExprKind::Block` + `StmtKind` (the new block expression) carry the same data through 27 dispatch sites across 7 crates. Every downstream consumer must handle both.

This plan adopts the **Gleam pattern**: one unified `Block` expression type, one `Statement` type, no parallel sequence representation.

## How to Use

1. Search this file (Ctrl+F) for keywords
2. Find the section ID
3. Open the section file

---

## Keyword Clusters by Section

### Section 01: IR Type Unification
**File:** `section-01-ir-types.md` | **Status:** Complete

```
SeqBinding, StmtKind, Stmt, Statement
FunctionSeq, FunctionSeqId, SeqBindingRange, StmtRange
Block, ExprKind::Block, ExprKind::FunctionSeq
Mutability, mutable, bool, immutable, dollar
ori_ir, ast, patterns, seq
arena, alloc_seq_bindings, get_seq_bindings
visitor, walk_function_seq, walk_seq_binding
```

---

### Section 02: Parser Unification
**File:** `section-02-parser.md` | **Status:** Complete

```
parse_block_expr_body, parse_try_block, parse_block_let_binding
parse_try_let_binding, block parsing, statement parsing
FunctionSeq::Run, desugar, function body
pre_checks, post_checks, contracts
semicolons, expression statement, let binding
ori_parse, grammar, expr, patterns
copier, incremental, copy_seq_binding
```

---

### Section 03: Type Checker Unification
**File:** `section-03-type-checker.md` | **Status:** Complete

```
infer_function_seq, infer_run_seq, infer_try_seq
infer_block, infer_seq_binding, infer_stmt
TypeEnv, bind_with_mutability, mutability map
check_run_seq, bidirectional, Expected
sequences.rs, blocks.rs
ori_types, infer, env
```

---

### Section 04: Downstream Consumers
**File:** `section-04-downstream.md` | **Status:** Complete

```
ori_canon, lower_function_seq, lower_seq_bindings
ori_eval, eval_can_block, can_eval
ori_llvm, lower_control_flow, lower_block
ori_fmt, emit_function_seq, emit_seq_binding
stacked, inline, width, run_rule
```

---

### Section 05: Cleanup & Verification
**File:** `section-05-cleanup.md` | **Status:** Complete

```
dead code, unused imports, FunctionSeq::Run removal
tests, test-all, clippy-all, spec tests
regression, migration, consistency
benchmark, performance, parser throughput
```

---

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | IR Type Unification | `section-01-ir-types.md` | Complete |
| 02 | Parser Unification | `section-02-parser.md` | Complete |
| 03 | Type Checker Unification | `section-03-type-checker.md` | Complete |
| 04 | Downstream Consumers | `section-04-downstream.md` | Complete |
| 05 | Cleanup & Verification | `section-05-cleanup.md` | Complete |
