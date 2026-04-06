---
section: "03"
title: "LLVM Emission & Test Wrappers"
status: complete
reviewed: false
goal: "JIT test wrappers use invoke/landingpad catch-all; InvokeIndirect emits LLVM invoke; void-return Apply defines dst"
inspired_by:
  - "Existing emit_invoke in terminators.rs (compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs:249)"
  - "Existing emit_abi_resolved_call void handling (terminators.rs:446-452)"
  - "Existing emit_apply_indirect for closure call pattern (apply.rs:286)"
depends_on: ["01", "02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Two-layer test wrappers"
    status: complete
  - id: "03.2"
    title: "InvokeIndirect terminator emission"
    status: complete
  - id: "03.3"
    title: "Void-return Apply dst (BUG-04-024)"
    status: complete
  - id: "03.4"
    title: "Update evaluator run_test"
    status: complete
  - id: "03.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "03.N"
    title: "Completion Checklist"
    status: complete
---

# Section 03: LLVM Emission & Test Wrappers

**Status:** Complete
**Goal:** JIT test wrappers catch uncaught panics via LLVM `invoke`/`landingpad` (no setjmp). `InvokeIndirect` terminators emit LLVM `invoke` through closure fat pointers. Void-returning `Apply` defines the dst variable (BUG-04-024).

**Implementation summary:** All items in this section have been implemented.

---

## 03.1 Two-layer test wrappers — COMPLETE

**File(s):** `compiler/ori_llvm/src/codegen/function_compiler/impls.rs`

Verified: `compile_tests` (line 31) generates two functions per test:
- Inner body (`_ori_test_<name>_body`): compiled through ARC pipeline with `fastcc`
- Outer wrapper (`_ori_test_<name>`): uses `invoke` to call inner body with catch-all `landingpad`

The `test_wrappers` map stores the outer wrapper name.

- [x] Two-layer test wrappers with invoke/landingpad catch-all
- [x] C calling convention for JIT compatibility
- [x] Outer wrapper name stored in `test_wrappers` map

---

## 03.2 InvokeIndirect terminator emission — COMPLETE

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/terminators.rs`

Verified: `ArcTerminator::InvokeIndirect` dispatch arm exists at line 198. The `emit_invoke_indirect` method (line 392) extracts fn_ptr and env_ptr from the closure, builds args, and emits LLVM `invoke` with normal/unwind blocks. Void returns define dst as unit constant.

- [x] `InvokeIndirect` dispatch in `emit_terminator`
- [x] `emit_invoke_indirect` implemented with closure extraction + invoke

---

## 03.3 Void-return Apply dst (BUG-04-024) — COMPLETE

**File(s):** `compiler/ori_llvm/src/codegen/arc_emitter/apply.rs`

Verified: at line 277-282, when the call result is void (result is None), the code defines dst as `EmittedValue::Immediate(const_i64(0))`.

- [x] Void-return `Apply` defines dst as unit constant

---

## 03.4 Update evaluator run_test — COMPLETE

**File(s):** `compiler/ori_llvm/src/evaluator/mod.rs`

Verified: `run_test` (line 140) calls the wrapper function directly via `unsafe { test_fn.call() }` (line 163), checks `did_panic()` after (line 166), and includes ARC leak checking. No `jit_run_protected` is used.

- [x] Direct call to test wrapper (no jit_run_protected)
- [x] `reset_panic_state()` before call
- [x] `did_panic()` check after call
- [x] ARC leak check preserved

---

## 03.R Third Party Review Findings

- None (section complete before plan creation).

---

## 03.N Completion Checklist

- [x] Two-layer test wrappers with invoke/landingpad catch-all
- [x] `InvokeIndirect` terminator emits LLVM invoke through closure
- [x] Void-return Apply defines dst variable (BUG-04-024 fixed)
- [x] Evaluator uses direct call + did_panic() (no jit_run_protected)

**Exit Criteria:** Met. JIT test runner uses LLVM landingpads for panic recovery. `catch(expr:)` works in JIT mode. BUG-04-024 resolved.
