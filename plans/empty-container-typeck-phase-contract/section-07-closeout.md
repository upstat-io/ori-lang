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
  - "All plan annotations removed from all four trees — verifiable via: `rg 'BUG-04-074|empty-container-typeck' compiler/ --glob '*.rs'`, `rg 'BUG-04-074|empty-container-typeck' compiler/ --glob '*.ori'`, `rg 'BUG-04-074|empty-container-typeck' tests/ --glob '*.ori'`, and `rg 'BUG-04-074|empty-container-typeck' library/ --glob '*.ori'` — all four returning zero hits."
  - "07.3 (TPR + hygiene) passes with zero actionable findings BEFORE any bug-tracker entries are updated to `complete`."
  - "`plans/bug-tracker/fix-BUG-04-074.md` status is updated to `complete` with a pointer to this plan."
  - "`plans/bug-tracker/00-overview.md` reflects the reduced open-bug count."
  - "`plans/bug-tracker/section-04-codegen-llvm.md` entry for BUG-04-074 updated to resolved."
  - "Final `/tpr-review` (whole plan arc) passes with zero actionable findings."
  - "`/impl-hygiene-review` on the full work arc (all sections) passes."
  - "`timeout 150 ./test-all.sh` green (debug). `timeout 150 cargo test --release -p ori_types` and `timeout 150 cargo test --release -p ori_llvm` green (release)."
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
    title: "Final TPR + hygiene review gate"
    status: not-started
  - id: "07.4"
    title: "Bug-tracker updates (runs AFTER 07.3 validation passes)"
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

Run the sweep over **all three trees** (compiler source, spec tests, and stdlib), using
only this plan's specific markers (not generic `§NN` tokens that match unrelated plans):

```bash
# Plan-specific markers in compiler source (Rust files) — must return zero hits
rg 'BUG-04-074|empty-container-typeck' compiler/ --glob '*.rs'

# Plan-specific markers in AOT fixture files (Ori files in compiler tree) — must return zero hits
# These are created in §05.3 at compiler/ori_llvm/tests/aot/fixtures/empty_list/*.ori
rg 'BUG-04-074|empty-container-typeck' compiler/ --glob '*.ori'

# Plan-specific markers in spec tests (regression comments added in §06.2)
rg 'BUG-04-074|empty-container-typeck' tests/ --glob '*.ori'

# Plan-specific markers in stdlib (edits made in §06.3)
rg 'BUG-04-074|empty-container-typeck' library/ --glob '*.ori'
```

Note: Generic `§NN` tokens (e.g. `§01`, `§03`) are intentionally excluded from the
sweep pattern because they exist in unrelated plans (e.g., `compiler/ori_repr/` contains
`§03` and `§04` references from a different plan). Searching for bare `§NN` produces
false positives and sends the implementer chasing unrelated code. Use only the
plan-specific identifiers above.

Every hit must be removed or converted to a permanent spec comment
(`Spec: Clause N.M` format). No plan-referencing comments in shipped code.

---

## 07.3 Final TPR + Hygiene Gate

**Run this BEFORE updating any external bug trackers.** All validation gates must pass
before declaring BUG-04-074 resolved — marking the bug complete before gates pass leaves
the tracker out of sync if a gate fails.

First, validate the entire plan directory against the plan corpus schema:

```bash
python -m scripts.plan_corpus check plans/empty-container-typeck-phase-contract/
```

This command must pass with zero schema errors before the plan can be marked complete.
If it fails on sections 01–04 or `index.md` (sections written before the plan_corpus
schema was current), update their frontmatter to satisfy the schema before proceeding.

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

## 07.4 Bug-Tracker Updates

**Only update bug trackers AFTER 07.3 validation gates pass.** This sequencing ensures
the bug is not prematurely marked complete.

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

## 07.R Third Party Review Findings

Round 2 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings addressed
in this revision.

### [[TPR-07-001-codex]] [MEDIUM] Scope the annotation cleanup sweep to this plan's markers and all affected trees

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:77`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Section 07 originally grepped `compiler/` for
`BUG-04-074|empty-container-typeck|§01|§02|§03|§04`. That misses temporary plan comments
in spec tests (section-05-test-matrix.md instructs implementers to add `Regression:
BUG-04-074` comments under `tests/spec/...`). It also produces unrelated false-positive
hits because generic `§03`/`§04` markers already exist in
`compiler/ori_repr/src/narrowing/tests.rs:1` and `compiler/ori_parse/src/dispatch.rs:134`
(from a different plan).

**Fix:** Split the sweep into two targeted commands that use only plan-specific identifiers
(`BUG-04-074|empty-container-typeck`) and cover both `compiler/` and `tests/`. Dropped
generic `§NN` tokens from the pattern to eliminate false positives from unrelated plans.
Updated the success_criteria in the frontmatter to match.

---

### [[TPR-07-002-gemini]] [LOW] Expand plan annotation cleanup regex to cover all sections

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:77`
**Reviewer:** Gemini | **Status:** Fixed (subsumed by TPR-07-001-codex)

**Evidence:** Section 07.2 used a regex that explicitly omitted `§05`, `§06`, and `§07`.
If developers left code annotations referencing those sections, the final cleanup sweep
would miss them.

**Fix:** Same as TPR-07-001-codex — the updated sweep uses plan-specific identifiers only
(`BUG-04-074|empty-container-typeck`) which covers all sections by design: any
annotation referencing this plan's work will include the bug ID or plan name.

---

### [[TPR-07-003-codex]] [MEDIUM] Add a whole-plan corpus validation gate before close-out

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:104`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Section 07's final gate originally ran only `/tpr-review` and
`/impl-hygiene-review`. The command
`python -m scripts.plan_corpus check plans/empty-container-typeck-phase-contract/`
currently fails on `index.md`, `section-01-value-restriction.md`,
`section-02-validator-module.md`, and `section-04-codegen-assertions.md`. Without an
explicit gate, Section 07 could declare the plan complete while the owning plan directory
is still schema-invalid to the plan tooling.

**Fix:** Added `python -m scripts.plan_corpus check plans/empty-container-typeck-phase-contract/`
as the first step in 07.4. The gate must pass before TPR or hygiene review proceeds.
Also added to the 07.N checklist as an explicit `- [ ]` item.

Round 3 — Dual-source TPR on sections 05, 06, 07 (Codex + Gemini). Findings addressed
in this revision.

### [[TPR-07-004-codex]] [MEDIUM] Sweep library/ directory for plan markers during close-out

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:§07.2`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Section 06.3 directs implementers to annotate `library/std/` code. Section 07.2
only swept `compiler/` (with `*.rs` filter, missing `.ori` AOT fixtures) and `tests/`. The
`library/` tree was entirely missing from the cleanup sweep.

**Fix:** Added two new sweep commands: one for `compiler/ --glob '*.ori'` (AOT fixtures) and
one for `library/ --glob '*.ori'` (stdlib). Updated success_criteria to reference all four
trees. Updated 07.N checklist to explicitly call out all four trees.

---

### [[TPR-07-005-codex]] [MEDIUM] Add release ori_llvm test and section-04 tracker update to 07.N

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:§07.N`
**Reviewer:** Codex | **Status:** Fixed

**Evidence:** Section 07.1 specifies `cargo test --release -p ori_llvm` but the 07.N checklist
item for "07.1 complete" only said "`./test-all.sh` green debug + release". Section 07.3 (now
07.4) step 3 requires updating `plans/bug-tracker/section-04-codegen-llvm.md` but the 07.N
checklist for "07.3 complete" (now "07.4 complete") omitted this.

**Fix:** Updated 07.N "07.1 complete" item to include `cargo test --release -p ori_llvm`. Updated
"07.4 complete" item to explicitly include `plans/bug-tracker/section-04-codegen-llvm.md` resolution.

---

### [[TPR-07-006-gemini]] [HIGH] Reorder 07.3/07.4 so validation gates run before bug-tracker closure

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:§07.3/07.4`
**Reviewer:** Gemini | **Status:** Fixed

**Evidence:** Section 07.3 was "Bug-Tracker Updates" (marks BUG-04-074 complete) and 07.4 was
"Final TPR + Hygiene Gate". This ordering allows the bug to be declared resolved in external
trackers before the final `/tpr-review` and `/impl-hygiene-review` pass. If a gate fails after
the tracker is updated, the tracker is out of sync.

**Fix:** Swapped 07.3 and 07.4: 07.3 is now "Final TPR + Hygiene Gate" and 07.4 is
"Bug-Tracker Updates (runs AFTER 07.3 validation passes)". Added explicit note in 07.4 header
that bug trackers must only be updated after 07.3 passes. Updated frontmatter sections list
and success_criteria to reflect the new ordering.

---

### [[TPR-07-007-gemini]] [LOW] section-04-codegen-llvm.md missing from 07.N 07.3 checklist item

**Location:** `plans/empty-container-typeck-phase-contract/section-07-closeout.md:§07.N`
**Reviewer:** Gemini | **Status:** Fixed (subsumed by TPR-07-005-codex)

**Evidence:** The 07.N completion checklist for the bug-tracker step only listed
`fix-BUG-04-074.md` and `00-overview.md`, omitting the `section-04-codegen-llvm.md` update
that step 3 of the bug-tracker section explicitly requires.

**Fix:** Same as TPR-07-005-codex — the updated 07.N checklist for "07.4 complete" now
explicitly includes `plans/bug-tracker/section-04-codegen-llvm.md entry resolved`.

---

## 07.N Completion Checklist

- [ ] **07.1 complete** — `./test-all.sh` green debug + release; `cargo test --release -p ori_llvm` green; dual-exec parity verified on ≥3 spec files
- [ ] **07.2 complete** — zero `BUG-04-074|empty-container-typeck` hits in all four trees: `compiler/*.rs`, `compiler/*.ori` (AOT fixtures), `tests/*.ori`, `library/*.ori`
- [ ] **07.3 complete** — `plan_corpus check plans/empty-container-typeck-phase-contract/` passes; final `/tpr-review` clean; `/impl-hygiene-review` clean
- [ ] **07.4 complete** — `fix-BUG-04-074.md` status `complete`; `00-overview.md` updated; `plans/bug-tracker/section-04-codegen-llvm.md` entry resolved
- [ ] This section's own status set to `complete` in frontmatter
- [ ] `plans/empty-container-typeck-phase-contract/` plan index updated: all sections complete

**Exit criteria:** BUG-04-074 is formally closed. The empty-container typeck
phase-contract enforcement plan is complete. The empty list ambiguity bug no longer
exists in the Ori compiler.
