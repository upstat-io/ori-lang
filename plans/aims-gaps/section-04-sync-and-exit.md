---
section: "04"
title: "Plan-Code Sync and Exit Verification"
status: not-started
goal: "Synchronize AIMS plan documents with implementation state and prove closure with verification gates."
depends_on: ["01", "02", "03"]
sections:
  - id: "04.1"
    title: "Plan Sync"
    status: not-started
  - id: "04.2"
    title: "Verification Gates"
    status: not-started
  - id: "04.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: Plan-Code Sync and Exit Verification

**Status:** Not Started
**Goal:** Finish with no status contradictions, no stale metrics, and clean verification.

## 04.1 Plan Sync

**File(s):** `plans/aims/index.md`, `plans/aims/00-overview.md`, `plans/aims/section-*.md`

- [ ] Update stale status labels and section summaries to match verified reality.
- [ ] Remove claims invalidated by code evolution.
- [ ] Link each corrected claim to supporting tests/diagnostics.

## 04.2 Verification Gates

**File(s):** changed files in `compiler/` and `plans/`

- [ ] Run `lsp_diagnostics` on all changed Rust source files and resolve all errors.
- [ ] Run applicable build/test commands for touched code paths.
- [ ] Confirm final deferred-audit table has no unresolved critical/high untracked bugs.

## 04.3 Completion Checklist

- [ ] AIMS completion verdict finalized and contradiction-free.
- [ ] Fresh metrics section reflects current command output.
- [ ] Deferred work audit complete with tracked/untracked and severity classification.
- [ ] `aims-gaps` plan is fully linked from final report.
