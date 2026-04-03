---
section: "10"
title: "Cleanup"
status: not-started
reviewed: false
goal: "Verify all hygiene fixes, run full test suite, and delete this plan directory"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "10.1"
    title: "Final Verification"
    status: not-started
  - id: "10.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "10.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 10: Cleanup

**Status:** Not Started
**Goal:** Verify no behavior changes across the entire hygiene sweep, then delete this plan.

**Depends on:** All previous sections.

---

## 10.1 Final Verification

- [ ] `timeout 150 ./test-all.sh` — all tests pass
- [ ] `./clippy-all.sh` — zero warnings
- [ ] `./fmt-all.sh` — no formatting changes
- [ ] `bash .claude/skills/impl-hygiene-review/plan-annotations.sh` — zero stale annotations
- [ ] No production files >500 lines (excluding validated exemptions)
- [ ] All unsafe blocks in ori_rt have SAFETY comments
- [ ] Delete this plan directory: `rm -rf plans/hygiene-full-2/`

---

## 10.R Third Party Review Findings

- None.

---

## 10.N Completion Checklist

- [ ] All verification checks pass
- [ ] Plan directory deleted
