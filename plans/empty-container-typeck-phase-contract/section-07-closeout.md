---
section: "07"
title: "Close-out + Supersession"
status: not-started
reviewed: false
goal: >
  Final verification, plan annotation cleanup, bug-tracker update, and formal supersession
  of `plans/bug-tracker/fix-BUG-04-074.md`. This section marks BUG-04-074 as resolved
  and closes the empty-container typeck phase-contract enforcement plan.
success_criteria:
  - "All plan section annotations referencing this plan (e.g. `BUG-04-074`, `§01`, `§02`) are removed from compiler source files — verifiable via `rg 'BUG-04-074|empty-container-typeck' compiler/' returning zero hits."
  - "`plans/bug-tracker/fix-BUG-04-074.md` status is updated to `complete` with a pointer to this plan."
  - "`plans/bug-tracker/00-overview.md` reflects the reduced open-bug count."
  - "`plans/bug-tracker/section-04-codegen-llvm.md` entry for BUG-04-074 updated to resolved."
  - "Final `/tpr-review` (whole plan arc) passes with zero actionable findings."
  - "`/impl-hygiene-review` on the full work arc (all sections) passes."
  - "`timeout 150 ./test-all.sh` green (debug). `timeout 150 cargo test --release -p ori_types` green (release)."
depends_on: ["01", "02", "03", "04", "05", "06"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "07.1"
    title: "Final test-all.sh + release verification"
    status: not-started
  - id: "07.2"
    title: "Plan annotation cleanup sweep"
    status: not-started
  - id: "07.3"
    title: "Bug-tracker updates"
    status: not-started
  - id: "07.4"
    title: "Final TPR + hygiene review gate"
    status: not-started
  - id: "07.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "07.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 07: Close-out + Supersession

**Status:** Not Started
**Goal:** Formally close BUG-04-074 and this plan. Verify all code changes are clean,
all plan annotations are removed, and the bug-tracker is updated.

**Depends on:** All previous sections complete.

---

## 07.1 Final Test Verification

```bash
timeout 150 ./test-all.sh          # debug build — must be green
timeout 150 cargo test --release -p ori_types   # release build
timeout 150 cargo test --release -p ori_llvm    # LLVM + AOT release
```

Also verify dual-execution parity on the Section 05 spec tests:
```bash
diagnostics/dual-exec-verify.sh tests/spec/types/collections/empty_list/empty_list_annotated_with_push.ori
diagnostics/dual-exec-verify.sh tests/spec/types/collections/empty_list/empty_list_element_struct.ori
diagnostics/dual-exec-verify.sh tests/spec/types/collections/empty_list/empty_list_with_for_yield.ori
```

---

## 07.2 Plan Annotation Cleanup Sweep

Per `CLAUDE.md §Compiler Coding Guidelines`: "Plan annotations are temporary scaffolding.
They MUST be removed when the plan completes."

Run:
```bash
rg 'BUG-04-074|empty-container-typeck|§01|§02|§03|§04' compiler/ --include="*.rs"
```

Every hit must be removed or converted to a permanent spec comment
(`Spec: Clause N.M` format). No plan-referencing comments in shipped code.

---

## 07.3 Bug-Tracker Updates

1. Update `plans/bug-tracker/fix-BUG-04-074.md`:
   - Set `status: complete`
   - Add pointer: `"Superseded by plans/empty-container-typeck-phase-contract/"`
   - Set `resolved_in:` to the commit SHA of the Section 03 landing

2. Update `plans/bug-tracker/00-overview.md`:
   - Move BUG-04-074 from open to closed
   - Decrement open bug count

3. Update `plans/bug-tracker/section-04-codegen-llvm.md` (or wherever BUG-04-074
   is tracked in the section-scoped trackers):
   - Mark as resolved

---

## 07.4 Final TPR + Hygiene Gate

Run final TPR over the full work arc:
```bash
/tpr-review
```

The scope covers all compiler changes across Sections 01–04, all new test files from
Section 05, and all annotation changes from Section 06. The review must return zero
actionable findings before the plan can be marked complete.

Run hygiene review:
```bash
/impl-hygiene-review
```

Auto-scope mode covers the full work arc. Must pass.

---

## 07.R Third Party Review Findings

<!-- Populated after TPR round on this section. -->
- None.

---

## 07.N Completion Checklist

- [ ] **07.1 complete** — `./test-all.sh` green debug + release; dual-exec parity verified
- [ ] **07.2 complete** — zero plan-annotation hits in `compiler/`
- [ ] **07.3 complete** — `fix-BUG-04-074.md` status `complete`; `00-overview.md` updated
- [ ] **07.4 complete** — final `/tpr-review` clean; `/impl-hygiene-review` clean
- [ ] This section's own status set to `complete` in frontmatter
- [ ] `plans/empty-container-typeck-phase-contract/` plan index updated: all sections complete

**Exit criteria:** BUG-04-074 is formally closed. The empty-container typeck
phase-contract enforcement plan is complete. The empty list ambiguity bug no longer
exists in the Ori compiler.
