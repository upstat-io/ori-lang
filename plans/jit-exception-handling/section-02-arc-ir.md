---
section: "02"
title: "ARC IR InvokeIndirect & Short-Circuit"
status: complete
reviewed: false
goal: "Closure calls inside catch(expr:) use InvokeIndirect terminator; && and || use branch-based short-circuit lowering"
inspired_by:
  - "Existing ARC IR Invoke terminator pattern (compiler/ori_arc/src/ir/mod.rs:260)"
  - "Existing lower_coalesce branching pattern (compiler/ori_arc/src/lower/expr/mod.rs:536)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Add InvokeIndirect to ArcTerminator"
    status: complete
  - id: "02.2"
    title: "Update all ArcTerminator match sites"
    status: complete
  - id: "02.3"
    title: "Builder and lowering support"
    status: complete
  - id: "02.4"
    title: "Short-circuit &&/|| lowering"
    status: complete
  - id: "02.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "02.N"
    title: "Completion Checklist"
    status: complete
---

# Section 02: ARC IR InvokeIndirect & Short-Circuit

**Status:** Complete
**Goal:** Indirect closure calls inside `catch(expr:)` generate `InvokeIndirect` terminators (with unwind blocks) instead of `ApplyIndirect` instructions (no unwind). Logical `&&`/`||` use branch-based lowering instead of eager evaluation.

**Implementation summary:** All items in this section have been implemented.

---

## 02.1 Add InvokeIndirect to ArcTerminator — COMPLETE

**File(s):** `compiler/ori_arc/src/ir/mod.rs`

Verified: `InvokeIndirect` variant exists at ir/mod.rs:276 with `dst`, `ty`, `closure`, `args`, `normal`, `unwind`. `used_vars()` includes `closure` + `args` (line 311-317). `uses_var()` checks both. `substitute_var()` handles both.

- [x] `InvokeIndirect` variant added
- [x] `used_vars()`, `uses_var()`, `substitute_var()` updated

---

## 02.2 Update all ArcTerminator match sites — COMPLETE

Verified across all crates:

- [x] `ori_arc/src/aims/emit_rc/trampoline.rs` — handles `InvokeIndirect`
- [x] `ori_arc/src/aims/interprocedural/mod.rs` — handles `InvokeIndirect`
- [x] `ori_arc/src/aims/transfer/mod.rs` — handles `InvokeIndirect`
- [x] `ori_arc/src/block_merge/compact.rs` — handles `InvokeIndirect`
- [x] `ori_arc/src/borrow/update.rs` — handles `InvokeIndirect`
- [x] `ori_arc/src/graph/mod.rs` — returns `[normal, unwind]` for successor blocks; collects `dst` at `normal`
- [x] `ori_repr/src/range/fixpoint/terminator.rs` — handles `InvokeIndirect`
- [x] `ori_llvm/src/codegen/arc_emitter/field_scan/mod.rs` — marks `closure` + `args` as used/needs-load
- [x] `ori_llvm/src/codegen/arc_emitter/rpo.rs` — DFS traversal handles `InvokeIndirect`
- [x] `ori_llvm/src/codegen/arc_emitter/dead_unwind.rs` — registers unwind block
- [x] `ori_llvm/src/codegen/function_compiler/nounwind/analyze.rs` — returns `false` (may unwind)
- [x] `oric/src/arc_dot/mod.rs` — edge emission handles `InvokeIndirect`
- [x] `oric/src/arc_dump/instr.rs` — display handles `InvokeIndirect`

---

## 02.3 Builder and lowering support ��� COMPLETE

**File(s):** `compiler/ori_arc/src/lower/builder/mod.rs`, `compiler/ori_arc/src/lower/builder/emission.rs`, `compiler/ori_arc/src/lower/calls/mod.rs`

Verified:
- `terminate_invoke_indirect()` exists in builder/mod.rs:322
- `emit_invoke_indirect()` exists in builder/emission.rs:205, uses `catch_unwind_target` for unwind routing
- `lower/calls/mod.rs:121` checks `catch_unwind_target.is_some()` → uses `emit_invoke_indirect`
- `lower/calls/mod.rs:186` same check for catch-all indirect call path

- [x] `terminate_invoke_indirect` in builder
- [x] `emit_invoke_indirect` in emission
- [x] Indirect calls inside `catch(expr:)` use `InvokeIndirect`

---

## 02.4 Short-circuit &&/|| lowering �� COMPLETE

**File(s):** `compiler/ori_arc/src/lower/expr/mod.rs`

Verified:
- Short-circuit intercept at line 419-423 for `BinaryOp::And` and `BinaryOp::Or`
- `lower_short_circuit_and` at line 522: evaluates LHS → branch → RHS or false
- `lower_short_circuit_or` at line 573: evaluates LHS → branch → true or RHS

- [x] Short-circuit intercept before eager path
- [x] `lower_short_circuit_and` implemented
- [x] `lower_short_circuit_or` implemented

---

## 02.R Third Party Review Findings

- None (section complete before plan creation).

---

## 02.N Completion Checklist

- [x] `InvokeIndirect` variant in `ArcTerminator` with all match sites updated
- [x] `terminate_invoke_indirect` and `emit_invoke_indirect` in builder
- [x] Indirect calls inside `catch(expr:)` use `InvokeIndirect`
- [x] `&&`/`||` use branch-based short-circuit lowering

**Exit Criteria:** Met. All match sites handle `InvokeIndirect`. Short-circuit lowering is implemented.
