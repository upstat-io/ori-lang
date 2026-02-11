---
section: "05"
title: Strip Dual Body from FunctionValue/UserMethod
status: complete
goal: Remove legacy ExprId fields from value types — canonical CanId becomes the sole body representation
sections:
  - id: "05.1"
    title: Clean FunctionValue
    status: complete
  - id: "05.2"
    title: Clean UserMethod
    status: complete
  - id: "05.3"
    title: Update Module Registration
    status: complete
  - id: "05.4"
    title: Completion Checklist
    status: complete
---

# Section 05: Strip Dual Body from FunctionValue/UserMethod

**Status:** Complete
**Goal:** Remove legacy fields from value types. `FunctionValue` and `UserMethod` carry only canonical `CanId` data for dispatch.

**Prerequisite:** Sections 03 and 04 complete (no code uses the legacy fields for dispatch anymore).

---

## 05.1 Clean FunctionValue — Complete

**File:** `ori_patterns/src/value/composite.rs`

Removed:
- [x] `defaults: Vec<Option<ExprId>>` field — replaced by `can_defaults`
- [x] `has_canon()` method — no callers remain
- [x] `has_can_defaults()` method — no callers remain
- [x] `with_defaults()` constructor — callers use `with_capabilities()` + `set_can_defaults()`
- [x] `with_shared_captures_and_defaults()` constructor — no callers

Updated:
- [x] `required_param_count()` — uses `can_defaults` instead of `defaults`
- [x] All constructors — no longer initialize `defaults` field

Kept (deferred to Section 06):
- `body: ExprId` — used by Debug impl, Hash impl, legacy lambda creation
- `arena: SharedArena` — still needed for `create_function_interpreter`
- `canon: Option<SharedCanonResult>` — can't be non-optional while legacy lambda path exists
- `set_canon()` / `set_can_defaults()` — still used by module_registration and lambda creation

## 05.2 Clean UserMethod — Complete

**File:** `ori_patterns/src/user_methods.rs`

- [x] Removed `has_canon()` method — no callers remain
- Kept `body: ExprId`, `set_canon()` for same reasons as FunctionValue

## 05.3 Update Module Registration — Complete

**File:** `ori_eval/src/module_registration.rs`

- [x] Replaced `FunctionValue::with_defaults(params, defaults, ...)` with `FunctionValue::with_capabilities(params, ...)`
- [x] Removed extraction of legacy `defaults` from parsed parameters

## 05.4 Fix Named-Arg Default Evaluation — Complete (bonus)

**File:** `ori_eval/src/interpreter/function_call.rs`

- [x] `eval_call_named` now uses `f.can_defaults()` + `eval_can(can_id)` instead of `f.defaults` + `eval(default_expr)`
- [x] `call_interpreter.canon` set BEFORE parameter binding loop (was set after, which would have caused panics for canonical defaults)
- [x] `check_named_arg_count` uses `can_defaults()` instead of `defaults`

## Completion Checklist

- [x] No `defaults: Vec<Option<ExprId>>` in FunctionValue
- [x] `has_canon()` removed from FunctionValue and UserMethod
- [x] `has_can_defaults()` removed from FunctionValue
- [x] `with_defaults()` constructor removed
- [x] `with_shared_captures_and_defaults()` constructor removed
- [x] Named-arg default evaluation uses canonical path
- [x] `cargo c` compiles clean
- [x] `cargo t` — zero failures
- [x] `cargo st tests/spec/` — 3052 passed, 0 failed, 58 skipped
- [x] `./clippy-all.sh` — all checks passed

**Exit Criteria:** No `defaults: Vec<Option<ExprId>>` in FunctionValue. No `has_canon()` checks. Named-arg defaults use canonical evaluation. Remaining legacy fields (`body`, `arena`, `canon: Option`) deferred to Section 06 when legacy eval path is deleted.
