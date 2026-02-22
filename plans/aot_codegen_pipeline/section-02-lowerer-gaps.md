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

---

## 02.2 Implement Missing Lowerings

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
