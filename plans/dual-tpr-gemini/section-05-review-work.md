---
section: "05"
title: "/review-work dual-source + Task #10 fix"
status: not-started
reviewed: true
goal: "Rewrite .claude/skills/review-work/SKILL.md for dual-source transport following the Section 04 pattern, AND fix the self-contradicting NEVER/ALWAYS background directives (Task #10) in the same edit pass. The command file .claude/commands/review-work.md is NOT touched — it remains the parallel 'Claude self-reviews directly' workflow per the Step 1E command-file boundary."
success_criteria:
  - ".claude/skills/review-work/SKILL.md rewritten for dual-source, following Section 04's validated pattern"
  - "The NEVER/ALWAYS background contradiction (Task #10) is FIXED — the skill is internally consistent, with run_in_background: true as the canonical invocation (matching tpr-review)"
  - "Lines 78-80 of the pre-rewrite file (the 'ABSOLUTE: NEVER Background' block) no longer exist in the new file"
  - "Lines 117-145 of the pre-rewrite file (the 'Always use run_in_background' block) are consistent with the new dual-source transport invocation"
  - ".claude/commands/review-work.md is BYTE-IDENTICAL to its pre-plan state (verified by git diff --exit-code)"
  - "At least 1 real review-work scenario runs successfully with both reviewers, invoked via the Skill tool (`Skill: review-work`), NOT via typing `/review-work` (the `/review-work` slash command continues to hit the untouched command file per Step 1E)"
depends_on: ["04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Fix Task #10 contradiction and rewrite for dual-source transport"
    status: not-started
  - id: "05.2"
    title: "Validate against real review-work scenarios"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: /review-work dual-source + Task #10 fix

**Status:** Not Started
**Goal:** Apply the Section 04 dual-source pattern to `.claude/skills/review-work/SKILL.md` AND fix the Task #10 NEVER/ALWAYS background contradiction in the same edit pass. The rewrite removes the contradictory directive block while preserving the correct background-invocation pattern.

**Success Criteria:**

- [ ] `.claude/skills/review-work/SKILL.md` rewritten for dual-source transport using the same pattern as Section 04's `/tpr-review` rewrite
- [ ] The NEVER/ALWAYS background contradiction is FIXED: the rewritten file has `run_in_background: true` as the canonical invocation (matching `/tpr-review`) with NO contradictory directive. The "ABSOLUTE: NEVER Background" block from lines 78-80 of the pre-rewrite file is removed entirely.
- [ ] `.claude/commands/review-work.md` is BYTE-IDENTICAL to its pre-plan state (verified by `git diff --exit-code`). The parallel "Claude self-reviews directly" workflow is intentionally preserved per the Step 1E command-file boundary.
- [ ] At least 1 real `/review-work` scenario runs successfully end-to-end with both reviewers producing findings and the merged output appearing in the expected location (plan TPR block or bug-tracker)
- [ ] The review-work skill's specific "If no owning plan section, file as bug in plans/bug-tracker/" logic is preserved in the dual-source version

**Context:** This section applies the validated Section 04 pattern to a second wrapper. Because Section 04 has already proven the transport works, Section 05 is mostly mechanical replication with one specific addition: fixing Task #10's self-contradiction. The contradiction is structural (two ABSOLUTE directives in the same file saying opposite things), so the only clean fix is rewriting the file such that only one is present. Since the rewrite is happening anyway for dual-source, Task #10 closes as a side effect.

The command file at `.claude/commands/review-work.md` is NOT touched — per the Step 1E command-file boundary decision, that file is a parallel "Claude self-reviews directly" workflow with intentionally different value props, and leaving it alone is the locked architectural decision.

**Reference implementations:**
- Section 04 (`/tpr-review` dual-source rewrite) — the validated pattern this section replicates
- Existing `.claude/skills/review-work/SKILL.md` — the file being rewritten (256 lines including the contradiction)
- Task #10 in the task tracker — tracks the contradiction that this section closes

**Depends on:** Section 04 (the validated dual-source pattern). Section 05 does not start until Section 04's validation gate passes.

---

## 05.1 Fix Task #10 contradiction and rewrite for dual-source transport

**File(s):** `.claude/skills/review-work/SKILL.md` (rewrite)

**Context:** The existing file at lines 78-80 says "ABSOLUTE: NEVER Background — This skill MUST run in the foreground" which directly contradicts lines 117-145 which say "Always use `run_in_background: true`". The rewrite removes the 78-80 block entirely. The correct canonical invocation — background via `dual-invoke-with-retry.sh` — is inherited from the Section 04 pattern.

Tasks:

- [ ] Read `.claude/skills/review-work/SKILL.md` in full to identify the contradiction block (lines 78-80 per the Phase 2 research finding, empirically re-verified by Codex Step 8B).

- [ ] Rewrite `.claude/skills/review-work/SKILL.md` following Section 04's dual-source wrapper structure, but adapted for review-work's specific concerns:
  - Preserve frontmatter: `name: review-work`, description updated to mention dual-source
  - Preserve `## Step 0 — MANDATORY: Re-read CLAUDE.md`
  - Preserve `## ABSOLUTE: You May NEVER Reason Out of Findings`
  - Preserve `## ABSOLUTE: Correct Architectural Solutions Only`
  - Preserve `## When to Trigger`
  - **DELETE the `## ABSOLUTE: NEVER Background` block entirely** (this is the Task #10 fix)
  - Update `## Loop Protocol` to match Section 04's dual-source loop semantics
  - Rewrite `## Steps (Per Iteration)` to use the Section 02 transport scripts (same pattern as Section 04)
  - Preserve `## If NO owning plan section exists -> file as bug in plans/bug-tracker/` — this is review-work specific

- [ ] Verify Task #10 is fixed:
  ```bash
  grep -c 'ABSOLUTE: NEVER Background' .claude/skills/review-work/SKILL.md
  # Expected: 0 (the block is removed)
  grep -c 'run_in_background: true' .claude/skills/review-work/SKILL.md
  # Expected: >= 1 (the canonical invocation is present via the transport pattern)
  ```

- [ ] Verify `.claude/commands/review-work.md` is unchanged:
  ```bash
  git diff --exit-code .claude/commands/review-work.md
  echo "exit=$?"  # expected: 0
  ```

- [ ] **Subsection close-out (05.1)** — MANDATORY before starting 05.2:
  - [ ] Task #10 contradiction removed, rewrite follows Section 04 pattern, command file unchanged
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively — was there friction in applying Section 04's pattern to review-work? Should there be a `rewrite-wrapper-for-dual-source.sh` that takes an existing single-source wrapper file and scaffolds the dual-source version? Implement improvements.

---

## 05.2 Validate against real review-work scenarios

**File(s):** Validation only

**Context:** Section 05 is smaller than Section 04 because the transport is already validated. This subsection runs at least one real `/review-work` scenario to verify the rewrite works end-to-end and the bug-tracker fallback (for findings without an owning plan section) still functions.

Tasks:

- [ ] **Scenario — Orphan finding (bug-tracker fallback)**: Run `/review-work` against a piece of work in a subsystem that has NO owning plan section. Verify:
  - Both reviewers produce findings
  - Merged findings are written to `plans/bug-tracker/section-XX-<subsystem>.md` using the BUG format, NOT to a plan TPR block
  - Reviewer-tagged IDs are still correctly formatted
  - The wrapper correctly decides "no owning plan exists" and routes to bug-tracker

- [ ] **Scenario — Plan-owned finding**: Run `/review-work` against a piece of work inside an active plan. Verify findings land in the plan section TPR block (same as Section 04's validation).

- [ ] Record scenario results in working notes.

- [ ] **Subsection close-out (05.2)** — MANDATORY before section completion:
  - [ ] Both scenarios pass (bug-tracker fallback works, plan-owned routing works)
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively — did the bug-tracker routing logic feel DRY with Section 04's plan TPR routing logic? Should there be a shared `route-merged-findings.py` that picks between plan TPR and bug-tracker based on the owning-plan check? Implement improvements.

---

## 05.R Third Party Review Findings

- None.

---

## 05.N Completion Checklist

- [ ] Both subsections (05.1, 05.2) marked `complete`
- [ ] `.claude/skills/review-work/SKILL.md` rewritten for dual-source
- [ ] **Task #10 FIXED**: `grep -c 'ABSOLUTE: NEVER Background' .claude/skills/review-work/SKILL.md` returns 0
- [ ] `.claude/commands/review-work.md` unchanged: `git diff --exit-code .claude/commands/review-work.md` exits 0
- [ ] Both scenarios (bug-tracker fallback, plan-owned routing) pass in validation
- [ ] `timeout 150 ./test-all.sh` green
- [ ] Plan annotation cleanup clean
- [ ] **Plan sync**: Section 05 frontmatter → `complete`, Quick Reference updated, Task #10 marked resolved in the task tracker
- [ ] `/tpr-review` (dual-source) passed against this section's work
- [ ] `/impl-hygiene-review` passed after TPR clean
- [ ] `/improve-tooling` section-close sweep done

**Exit Criteria:** `.claude/skills/review-work/SKILL.md` runs dual-source reviews successfully. The Task #10 contradiction is gone. `.claude/commands/review-work.md` is unmodified. Real scenarios verify both the plan-TPR routing and the bug-tracker fallback. Section 06 can begin.
