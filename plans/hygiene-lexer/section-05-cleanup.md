---
section: "05"
title: "Cleanup"
status: not-started
reviewed: false
goal: "Verify all fixes land cleanly and delete this plan"
success_criteria:
  - "./test-all.sh green with no regressions"
  - "./clippy-all.sh green"
  - "Plan directory deleted"
depends_on: ["01", "02", "03", "04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Final verification"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Cleanup

**Status:** Not Started
**Goal:** Final verification that all hygiene fixes land cleanly, then delete this disposable plan.

**Success Criteria:**

- [ ] All tests pass in both debug and release
- [ ] No clippy warnings introduced
- [ ] Plan directory removed
- [ ] Satisfies mission criterion: "All section success criteria met"

**Depends on:** Sections 01-04 (all must be complete).

---

## 05.1 Final verification

- [ ] `timeout 150 ./test-all.sh` — all tests pass, no regressions
- [ ] `timeout 150 ./clippy-all.sh` — no new warnings
- [ ] Verify no files in `ori_lexer` or `ori_lexer_core` exceed 500-line limit
- [ ] Verify BUG-01-001 is marked resolved in `plans/bug-tracker/section-01-parser-lexer.md`

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] `timeout 150 ./test-all.sh` green
- [ ] `timeout 150 ./clippy-all.sh` green
- [ ] No files over 500 lines in either lexer crate
- [ ] BUG-01-001 resolved in bug tracker
- [ ] Delete this plan directory: `rm -rf plans/hygiene-lexer/`

**Exit Criteria:** All hygiene fixes verified. Plan deleted. The lexer codebase is cleaner than we found it.
