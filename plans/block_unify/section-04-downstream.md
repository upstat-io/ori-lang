---
section: "04"
title: Downstream Consumers
status: complete
goal: Update canonicalization, evaluator, LLVM codegen, and formatter to use unified types
sections:
  - id: "04.1"
    title: Canonicalization (ori_canon)
    status: complete
  - id: "04.2"
    title: Evaluator (ori_eval)
    status: complete
  - id: "04.3"
    title: LLVM Codegen (ori_llvm)
    status: complete
  - id: "04.4"
    title: Formatter (ori_fmt)
    status: complete
  - id: "04.5"
    title: ARC Optimizer (ori_arc)
    status: complete
  - id: "04.6"
    title: oric Integration
    status: complete
---

# Section 04: Downstream Consumers

**Status:** Complete
**Goal:** Every crate downstream of `ori_ir` compiles and works with the unified types. No references to `SeqBinding`, `SeqBindingRange`, or `FunctionSeq::Run` remain anywhere.

**Completed via Section 01 cascade.** When `FunctionSeq::Run`, `SeqBinding`, and `SeqBindingRange` were removed from `ori_ir` in Section 01, all downstream consumers were updated simultaneously to compile. No residual references remain.

---

## 04.1–04.6 All Subsections

All verified complete:

- [x] `grep -r 'SeqBinding\b' compiler/` returns zero results
- [x] `grep -r 'SeqBindingRange' compiler/` returns zero results
- [x] `grep -r 'FunctionSeq::Run' compiler/` returns zero results
- [x] `grep -r 'infer_run_seq\|check_run_seq' compiler/` returns zero results
- [x] `grep -r 'lower_seq_bindings\b' compiler/` returns zero results
- [x] `grep -r 'emit_seq_binding\|emit_run' compiler/` returns zero results
- [x] `grep -r 'alloc_seq_bindings\|get_seq_bindings' compiler/` returns zero results
- [x] `grep -r 'copy_seq_binding' compiler/` returns zero results
- [x] Every crate compiles: `cargo c`
- [x] Every crate's tests pass: `./test-all.sh` — 10,219 passed, 0 failed
- [x] Clippy clean: `./clippy-all.sh`

**Exit Criteria:** Zero references to old types in any compiler crate. Every crate compiles and passes tests. ✅ Met.
