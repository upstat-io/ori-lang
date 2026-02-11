---
section: "04"
title: Remove Function/Method Call Fallbacks
status: complete
goal: Function and method calls always use the canonical path — no has_canon() dispatch
sections:
  - id: "04.1"
    title: Remove function_call.rs Fallbacks
    status: complete
  - id: "04.2"
    title: Remove method_dispatch.rs Fallbacks
    status: complete
  - id: "04.3"
    title: Remove Legacy Default Parameter Eval
    status: complete
  - id: "04.4"
    title: Completion Checklist
    status: complete
---

# Section 04: Remove Function/Method Call Fallbacks

**Status:** Complete
**Goal:** Function calls always use the canonical path. Remove all `has_canon()` checks.

**Prerequisite:** Section 02 (all FunctionExp patterns handled canonically).

---

## 04.1 Remove function_call.rs Fallbacks — Complete

**File:** `ori_eval/src/interpreter/function_call.rs`

- [x] Removed all `has_canon()` and `has_can_defaults()` checks (7 sites)
- [x] All function calls (regular, memoized, named-arg) unconditionally use canonical path
- [x] Always sets `call_interpreter.canon` from `f.canon()` and calls `eval_can(f.can_body)`

## 04.2 Remove method_dispatch.rs Fallbacks — Complete

**File:** `ori_eval/src/interpreter/method_dispatch.rs`

- [x] Removed `has_canon()` checks at 2 sites (user method + associated function)
- [x] Always uses `call_interpreter.canon.clone_from(&method.canon)` + `eval_can(method.can_body)`
- [x] Updated comments: "canonical path when available, legacy otherwise" → "via canonical IR"

## 04.3 Remove Legacy Default Parameter Eval — Complete

**File:** `ori_eval/src/exec/call.rs`

- [x] Deleted `bind_parameters_with_defaults` (legacy, evaluated `ExprId` defaults)
- [x] Renamed `bind_parameters_with_can_defaults` → `bind_parameters_with_defaults`
- [x] Updated doc comment (removed "canonical" qualifier)
- [x] Updated all callsites in `function_call.rs`

## 04.4 Completion Checklist

- [x] Zero `has_canon()` checks remain in `function_call.rs`
- [x] Zero `has_canon()` checks remain in `method_dispatch.rs`
- [x] `bind_parameters_with_defaults` (legacy) deleted from `exec/call.rs`
- [x] `bind_parameters_with_can_defaults` renamed to `bind_parameters_with_defaults`
- [x] No `eval(body)` calls remain in dispatch code
- [x] 3 legacy unit tests removed (`test_user_method_dispatch`, `test_user_method_with_self_access`, `test_user_method_with_args`) — created `UserMethod` without canonical IR, covered by 3052 spec tests
- [x] `cargo c -p ori_eval` compiles clean
- [x] `cargo t -p ori_eval` passes
- [x] `cargo t -p oric` passes
- [x] `./test-all.sh` passes (3052 passed, 0 failed, 58 skipped)
- [x] `./clippy-all.sh` passes

**Exit Criteria:** No `has_canon()` checks remain. No `eval(body: ExprId)` calls in dispatch code. All function/method calls unconditionally use the canonical path.
