---
section: "02"
title: "ARC Lowerer Gap Closure"
status: not-started
goal: "Handle all remaining CanExpr variants in ori_arc lowering"
sections:
  - id: "02.1"
    title: "Audit CanExpr coverage"
    status: not-started
  - id: "02.2"
    title: "Implement missing lowerings"
    status: not-started
  - id: "02.3"
    title: "Tests"
    status: not-started
---

# Section 02: ARC Lowerer Gap Closure

**Status:** Not Started
**Goal:** Every `CanExpr` variant lowers to ARC IR without `UnsupportedExpr` fallback.

**Context:** The `arc_codegen_unification` plan Section 01 identified 6 `CanExpr` variants that produced `UnsupportedExpr`. Several have been fixed in `lower/constructs.rs`, but audit is needed to confirm full coverage.

---

## 02.1 Audit CanExpr Coverage

- [ ] Grep `lower/expr/mod.rs` for all match arms — list every `CanExpr` variant and its handler
- [ ] Grep for `UnsupportedExpr` across all `lower/` files — these are the gaps
- [ ] Cross-reference against the full `CanExpr` enum in `ori_ir` to find any variants not matched at all
- [ ] Document which variants are handled, which are stubbed, and which are missing

**Expected gaps (from prior analysis):**

| Variant | Status | Handler |
|---------|--------|---------|
| `FunctionExp` | Partially done | `lower/constructs.rs` handles panic/unreachable/todo/print/println |
| `FunctionRef` | Needs verification | Should emit `PartialApply` with empty captures |
| `HashLength` | May need work | Length tracking in hash context |
| `FormatWith` | May need work | Type-dispatched `Apply` to `ori_format_*` |
| `Await` | Simple | Passthrough to inner expression |
| `WithCapability` | Simple | Passthrough to body expression |
| `TryOperator` | **Fixed** (uncommitted) | `lower/collections/mod.rs:266` — `Idx::ERROR` used as Err payload type instead of `pool.result_err(inner_ty)`. Fix applied in working tree. |

---

## 02.2 Implement Missing Lowerings

**Already fixed (uncommitted):**
- [ ] **`TryOperator` / `lower_try()`** — `Idx::ERROR` sentinel was used as the Err variant's projection type, causing the payload to be loaded as `i64` instead of the actual error type (e.g., `str = {i64, ptr}`). Fix: replaced with `pool.result_err(resolved_result)` at `lower/collections/mod.rs:267-268`. **Must be committed.**

For each gap found in 02.1:

- [ ] **`FunctionRef`** — emit as `PartialApply` with the referenced function and empty capture list:
  ```
  ArcInstr::PartialApply { dst, func: referenced_fn, captures: vec![] }
  ```

- [ ] **`HashLength`** — lower to a `Project` on the hash's internal length field, or an `Apply` to a runtime `ori_map_len` / `ori_set_len` function

- [ ] **`FormatWith`** — lower to `Apply` dispatching to the appropriate `ori_format_*` runtime function based on the argument types

- [ ] **`Await`** — for now, passthrough: lower the inner expression and return its result (async is post-0.1-alpha)

- [ ] **`WithCapability`** — passthrough: lower the body expression (capability tracking is a type-system concern, transparent at codegen)

- [ ] **Any other gaps** found during the audit in 02.1

- [ ] **Builtin tag-check lowering** (**cross-reference Section 04.4, option a**) — The canonical IR desugars `r.is_err()` to `is_err(r)` as a direct `Call` with `CanExpr::Ident`, so these go through `lower_call()` → `emit_call_or_invoke()`, NOT through `lower_method_call()`. The preferred fix (Section 04.4 option a) is to intercept in `emit_call_or_invoke()` and emit `Project(tag) + PrimOp::Binary(Eq)` instead of `Invoke` for known tag-check builtins (is_err, is_ok, is_some, is_none). No new `PrimOp` variants needed — reuses existing `PrimOp::Binary(BinaryOp::Eq)`. Receiver type looked up via `builder.var_type(receiver_var)`. See Section 04.4 for the full analysis of why this matters (ARC leak root cause).

---

## 02.3 Tests

- [ ] Add test cases to `compiler/ori_arc/src/lower/expr/tests.rs` for each newly implemented variant
- [ ] Add AOT integration tests for functions that exercise:
  - Passing functions as values (`FunctionRef`)
  - String formatting (`FormatWith`)
  - Collection length operations (`HashLength`)
- [ ] Verify all existing `lower/` tests still pass
- [ ] Run `./test-all.sh`

---

## 02.4 Completion Checklist

- [ ] Zero `UnsupportedExpr` instances remain in `lower/` code
- [ ] Every `CanExpr` variant has an explicit match arm (no wildcard fallthrough)
- [ ] All new lowerings have unit tests
- [ ] AOT integration tests exercise all paths
- [ ] `./test-all.sh` passes

**Exit Criteria:** `grep -r "UnsupportedExpr" compiler/ori_arc/src/lower/` returns zero results.
