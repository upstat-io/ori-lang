---
section: "01"
title: "ARC merge-edge scoped cleanup"
status: in-progress
reviewed: false
goal: "Restore edge-scoped ARC cleanup without dropping any escaping projected child from the same parent aggregate."
depends_on: []
third_party_review:
  status: findings
  updated: 2026-03-23
sections:
  - id: "01.1"
    title: "Merge-edge deferred dec scoping"
    status: in-progress
  - id: "01.R"
    title: "Third Party Review Findings"
    status: in-progress
  - id: "01.N"
    title: "Completion Checklist"
    status: pending
---

# Section 01: ARC merge-edge scoped cleanup

**Status:** In Progress
**Goal:** Edge-scoped ARC cleanup must preserve every escaping projected child across merge edges, including multiple fields projected from the same parent aggregate on the same outgoing edge.

**Context:** CI exposed a macOS AOT failure in `arc::test_rc_project_merge_edge_scoped_cleanup`, and commit `9d323f30` fixed the first-order bug by scoping `DeferredDec` to the correct successor edge. Review of the current implementation found a remaining aggregate-projection hole in the same path. The section stays open until escaping projections are tracked precisely enough to protect all live children.

**Depends on:** None.

---

## 01.1 Merge-edge deferred dec scoping

**File(s):** `compiler/ori_arc/src/aims/realize/emit_unified.rs`, `compiler/ori_llvm/tests/aot/arc.rs`

The original merge-edge cleanup fix is in place, but the current projection bookkeeping still folds all escaping children from the same aggregate parent into a single compensation destination. This subsection remains open until the projection escape model is widened to preserve each escaping child separately and the regression suite proves it.

- [ ] Preserve escaping projections by arg identity or `(parent, child)` identity instead of a single `parent -> project_dst` slot in `emit_unified.rs`.
- [ ] Add a regression in `compiler/ori_llvm/tests/aot/arc.rs` where one edge passes two distinct projected fields from the same parent aggregate and both remain alive after edge cleanup.
- [ ] Re-run the existing merge-edge ARC tests plus the new multi-field projection test.

---

## 01.R Third Party Review Findings

- [ ] `[TPR-01-001][high]` `compiler/ori_arc/src/aims/realize/emit_unified.rs:358` — Multi-field escapes from the same parent aggregate are still unsound.
  Evidence: `find_edge_decced_project_parents()` stores a single `parent -> Project dst` entry, and `emit_project_escape_incs()` reuses that one `project_dst` for every outgoing arg tied to the same parent. If both `a.first` and `a.second` escape, the later `Project` overwrites the former and only one child is compensated. The current regression coverage only exercises one projected field per parent.
  Impact: Edge cleanup can still free an escaping child field while the successor keeps using it, which reintroduces use-after-free or double-dec behavior on multi-field aggregates.
  Required plan update: Track escaping projections per arg or per `(parent, child)` pair, then add an AOT regression covering two escaping projected fields from the same parent on the same merge edge.

---

## 01.N Completion Checklist

- [ ] `TPR-01-001` is resolved in code, with the accepted fix reflected in `01.1`.
- [ ] `cargo test -p ori_llvm test_rc_project_merge_two_distinct_parents -- --nocapture` passes.
- [ ] A new regression covering two escaping fields from one aggregate parent passes.
- [ ] `cargo test -p ori_llvm test_rc_project_merge_edge_scoped_cleanup -- --nocapture` passes without modification.

**Exit Criteria:** Merge-edge ARC cleanup preserves all escaping children on every successor edge, including multiple projected fields from the same aggregate parent, and the dedicated AOT regressions pass with no new RC failures in the surrounding ARC suite.
