---
section: "03"
title: "Deferred Work Elimination"
status: not-started
goal: "Eliminate or track every deferred/future/TODO-style AIMS code path with immediate remediation."
depends_on: ["01", "02"]
sections:
  - id: "03.1"
    title: "Deferred Item Inventory"
    status: not-started
  - id: "03.2"
    title: "Tracked vs Untracked Classification"
    status: not-started
  - id: "03.3"
    title: "Fix Execution"
    status: not-started
  - id: "03.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Deferred Work Elimination

**Status:** Not Started
**Goal:** Ensure ZERO DEFERRAL compliance across AIMS-related code.

## 03.1 Deferred Item Inventory

**File(s):** `compiler/ori_arc/src/aims/**/*.rs`, `compiler/ori_arc/src/pipeline/aims_pipeline.rs`, `compiler/ori_llvm/src/codegen/arc_emitter/**/*.rs`

- [ ] Enumerate all deferred/future/TODO/FIXME/HACK/XXX comment items.
- [ ] Enumerate all `#[expect(dead_code)]` and `#[allow(dead_code)]` cases tied to deferred behavior.
- [ ] Deduplicate by root issue, preserving all source file references.

## 03.2 Tracked vs Untracked Classification

**File(s):** `plans/aims/*.md`

- [ ] For each deferred item, mark whether it is explicitly tracked in `plans/aims/`.
- [ ] Assign severity (`critical`, `high`, `medium`, `low`) and class (`bug`, `feature`, `scope`).
- [ ] Mark untracked deferred items as policy violations requiring immediate fix planning.

## 03.3 Fix Execution

**File(s):** issue-specific code files

- [ ] Add failing tests first for each remediated bug.
- [ ] Implement architectural fixes (no stopgaps).
- [ ] Re-run affected unit/integration tests after each fix set.

## 03.4 Completion Checklist

- [ ] No unresolved untracked deferred items remain in AIMS code paths.
- [ ] Every remaining deferred note is either removed or converted to active tracked work.
- [ ] All remediation changes are verified by tests.
