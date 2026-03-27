---
section: "01"
title: "ARC merge-edge scoped cleanup"
status: complete
reviewed: true
goal: "Restore edge-scoped ARC cleanup without dropping any escaping projected child from the same parent aggregate."
depends_on: []
third_party_review:
  status: resolved
  updated: 2026-03-23
sections:
  - id: "01.1"
    title: "Merge-edge deferred dec scoping"
    status: complete
  - id: "01.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "01.N"
    title: "Completion Checklist"
    status: complete
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

- [x] Preserve escaping projections by arg identity or `(parent, child)` identity instead of a single `parent -> project_dst` slot in `emit_unified.rs`.
  Done: Changed `find_edge_decced_project_parents()` to return `FxHashSet<ArcVarId>` (doomed parent set) instead of `FxHashMap<ArcVarId, ArcVarId>`. Each arg now gets its own `RcInc` keyed to its own identity, not a shared project_dst from the parent.
- [x] Add a regression in `compiler/ori_llvm/tests/aot/arc.rs` where one edge passes two distinct projected fields from the same parent aggregate and both remain alive after edge cleanup.
  Done: Added `test_rc_project_merge_edge_two_fields_escape` with 3 cases — struct destructuring (2 fields), conditional merge, and 3-field variant.
- [x] Re-run the existing merge-edge ARC tests plus the new multi-field projection test.
  Done: All 13,649 tests pass (0 failures).

---

## 01.R Third Party Review Findings

- [x] `[TPR-01-001][high]` `compiler/ori_arc/src/aims/realize/emit_unified.rs:358` — Multi-field escapes from the same parent aggregate are still unsound.
  Evidence: `find_edge_decced_project_parents()` stores a single `parent -> Project dst` entry, and `emit_project_escape_incs()` reuses that one `project_dst` for every outgoing arg tied to the same parent. If both `a.first` and `a.second` escape, the later `Project` overwrites the former and only one child is compensated. The current regression coverage only exercises one projected field per parent.
  Impact: Edge cleanup can still free an escaping child field while the successor keeps using it, which reintroduces use-after-free or double-dec behavior on multi-field aggregates.
  Required plan update: Track escaping projections per arg or per `(parent, child)` pair, then add an AOT regression covering two escaping projected fields from the same parent on the same merge edge.
  Resolved: Validated against codebase on 2026-03-23 — confirmed `FxHashMap<ArcVarId, ArcVarId>` overwrites on duplicate parent. Implementation tasks integrated into 01.1.

---

## 01.N Completion Checklist

- [x] `TPR-01-001` is resolved in code, with the accepted fix reflected in `01.1`.
- [x] `cargo test -p ori_llvm test_rc_project_merge_two_distinct_parents -- --nocapture` passes.
- [x] A new regression covering two escaping fields from one aggregate parent passes.
- [x] `cargo test -p ori_llvm test_rc_project_merge_edge_scoped_cleanup -- --nocapture` passes without modification.

**Exit Criteria:** Merge-edge ARC cleanup preserves all escaping children on every successor edge, including multiple projected fields from the same aggregate parent, and the dedicated AOT regressions pass with no new RC failures in the surrounding ARC suite.
