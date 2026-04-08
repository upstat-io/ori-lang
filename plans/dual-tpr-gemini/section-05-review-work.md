---
section: "05"
title: "/review-work dual-source + Task #10 fix"
status: in-progress
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
  status: resolved
  updated: 2026-04-08
  note: "Scenario 2 round 1 surfaced 2 codex findings against the 05.1 rewrite + lint extension: one HIGH DRIFT on the bug-tracker fallback ID format, one MEDIUM GAP on the Step 7b fix-path branching. Both verified against the canonical contract (add-bug, bug-tracker overview, fix-bug skill, review-work command file) and fixed in the same commit across BOTH wrappers (tpr-review and review-work — cross-section fix touching §04's work product per 'Plan boundaries = implementation boundaries'). Gemini's round 1 envelope failed schema_violation due to missing schema_version field; root cause traced to .gemini/skills/review-work/SKILL.md not documenting the field; fixed in a separate commit that also extended the lint. Round 2 re-validation pending."
sections:
  - id: "05.1"
    title: "Fix Task #10 contradiction and rewrite for dual-source transport"
    status: complete
  - id: "05.2"
    title: "Validate against real review-work scenarios"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: complete
  - id: "05.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: /review-work dual-source + Task #10 fix

**Status:** In Progress (05.1 complete 2026-04-08; 05.2 validation pending)
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

- [x] Read `.claude/skills/review-work/SKILL.md` in full to identify the contradiction block (lines 78-80 per the Phase 2 research finding, empirically re-verified by Codex Step 8B).
  Verified 2026-04-08: the pre-rewrite file contained `## ABSOLUTE: NEVER Background` at L78-80 (3 lines: header + blank + paragraph), directly contradicted by `Always use run_in_background: true` in Step 1 at L117-145. The whole file was 256 lines.

- [x] Rewrite `.claude/skills/review-work/SKILL.md` following Section 04's dual-source wrapper structure, but adapted for review-work's specific concerns:
  - Preserve frontmatter: `name: review-work`, description updated to mention dual-source
  - Preserve `## Step 0 — MANDATORY: Re-read CLAUDE.md`
  - Preserve `## ABSOLUTE: You May NEVER Reason Out of Findings`
  - Preserve `## ABSOLUTE: Correct Architectural Solutions Only`
  - Preserve `## When to Trigger`
  - **DELETE the `## ABSOLUTE: NEVER Background` block entirely** (this is the Task #10 fix)
  - Update `## Loop Protocol` to match Section 04's dual-source loop semantics
  - Rewrite `## Steps (Per Iteration)` to use the Section 02 transport scripts (same pattern as Section 04)
  - Preserve `## If NO owning plan section exists -> file as bug in plans/bug-tracker/` — this is review-work specific

- [x] Verify Task #10 is fixed:
  ```bash
  grep -c 'ABSOLUTE: NEVER Background' .claude/skills/review-work/SKILL.md
  # Expected: 0 (the block is removed)
  grep -c 'run_in_background: true' .claude/skills/review-work/SKILL.md
  # Expected: >= 1 (the canonical invocation is present via the transport pattern)
  ```
  Verified 2026-04-08: `ABSOLUTE: NEVER Background` count = 0; `run_in_background: true` count = 2. Task #10 contradiction removed. Locked into the extended `lint-dual-tpr-docs.sh` via a negative assertion (forbidden phrase) that will catch any future regression.

- [x] Verify `.claude/commands/review-work.md` is unchanged:
  ```bash
  git diff --exit-code .claude/commands/review-work.md
  echo "exit=$?"  # expected: 0
  ```
  Verified 2026-04-08: `git diff --exit-code .claude/commands/review-work.md` → exit 0. The parallel "Claude self-reviews directly" workflow at `.claude/commands/review-work.md` is byte-identical to its pre-plan state, preserving the Step 1E command-file boundary per the plan overview.

- [x] **Subsection close-out (05.1)** — MANDATORY before starting 05.2:
  - [x] Task #10 contradiction removed, rewrite follows Section 04 pattern, command file unchanged
  - [x] Update this subsection's `status` to `complete`
  - [x] Run `/improve-tooling` retrospectively — was there friction in applying Section 04's pattern to review-work? Should there be a `rewrite-wrapper-for-dual-source.sh` that takes an existing single-source wrapper file and scaffolds the dual-source version? Implement improvements.

    **Retrospective 05.1 — outcome: 1 improvement implemented, 1 candidate rejected.**

    **Friction points observed during the rewrite:**

    1. **Plan estimate was wrong about file size.** The plan's 05.1 Context said "the skill file shrinks from 256 to roughly 180 lines" but the actual dual-source rewrite grew to **550 lines**, nearly identical to tpr-review's 539-line dual-source wrapper. The estimate was anchored to the single-source original instead of the already-validated dual-source template. This is a one-time documentation error with limited future impact (Sections 06/07 can use tpr-review's actual 539 lines as the reference now), so it does not warrant new tooling — just a mental correction.

    2. **grep -c + set -e gotcha in the verification pipeline.** The initial verification command `grep -c 'ABSOLUTE: NEVER Background' ... && grep -c 'run_in_background: true' ... && git diff ...` failed after the first grep because `grep -c` exits 1 when it finds 0 matches (the expected success case for a forbidden phrase), which killed the `&&` chain under bash's default error propagation. This gotcha is already documented in the §03.2 retrospective (committed in `795648d1`) and re-confirmed here. No new tooling needed — the `|| true` guard pattern from the §03.2 lesson applies directly.

    3. **The real risk: copy-paste erasure of preserved safety blocks.** The rewrite was mechanically derived from tpr-review/SKILL.md with targeted adaptations (frontmatter, title, intro, bug-tracker fallback emphasis, Task #10 deletion). The risk is that a future rewrite (Section 06 /review-plan, Section 07 /tp-help) could accidentally drop one of the preserved safety blocks (Step 0 CLAUDE.md re-read, the two ABSOLUTE blocks) or one of the transport invocation phrases (`dual-invoke-with-retry.sh`, `merge-findings.py`, `scratch-dir.sh`, `envelope-only`, `Activate the review-work skill`). The existing `lint-dual-tpr-docs.sh` already guards these for tpr-review but does NOT yet guard them for review-work — a gap that would bite Sections 06/07 on copy-paste.

    **Rejected candidate:** `rewrite-wrapper-for-dual-source.sh` — a scaffolding script that takes a source SKILL.md and a target skill name and scaffolds the dual-source version. Rejected because (a) Section 06 is a NEW file (greenfield, not a rewrite — so a rewrite tool doesn't apply); (b) Section 07 consolidates two existing files and uses a different schema (raw concat mode, not findings) — so the tool's assumptions don't fit; (c) Section 05 is the ONLY legitimate call site for a "rewrite" tool. Building for zero future invocations is over-tooling.

    **Accepted improvement:** extend `lint-dual-tpr-docs.sh` to cover the review-work wrapper with the same 8 preservation assertions as tpr-review, plus 2 new negative regression guards that ensure `ABSOLUTE: NEVER Background` cannot creep back into either wrapper via copy-paste. Introduces a general `FORBIDDEN` array + loop that any future "phrase must be absent" assertion can reuse (e.g., Section 06/07 may add their own forbidden-phrase checks). Delta: ~40 lines added to `lint-dual-tpr-docs.sh`. Baseline passed 27/27; extended lint passes 39/39 (27 baseline + 8 review-work preservation + 1 file inventory + 1 internal path resolution + 2 negative regression guards). Committed separately per the Step 5.5 "commit each tooling improvement" rule.

    **Carry-forward for 05.2:** The 05.2 validation scenarios will exercise the rewritten file via real dual-source runs. If 05.2 surfaces structural issues the lint didn't catch, that's a lint coverage gap worth extending. Current lint assertions are conservative — they only catch copy-paste erasure, not semantic regressions in the loop logic or the merged-finding format.

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

### Round 1 — Scenario 2 (plan-owned, 05.1 commits) — 2026-04-08

Run: `/tmp/ori-tpr-jRQHAIYL` (second attempt; first attempt at `/tmp/ori-tpr-iLiS3COd` aborted with `codex_parse_fail` due to a prompt-authoring error by the orchestrator — not a reviewer fault; see retrospective). Both reviewers were scoped to commits `16ca2e34..7ebf112d` (review-work rewrite + lint extension). Codex completed cleanly (2 findings); gemini completed cleanly but produced a schema-invalid envelope (missing `schema_version`) — diagnosed and fixed in a separate commit. Merger did not run for round 1 because gemini's envelope failed parse.

- [x] `[TPR-05-001-codex][high]` `.claude/skills/review-work/SKILL.md:378` — Realign orphan bug IDs with the canonical add-bug contract.
  Evidence: Step 7a tells Claude to file no-plan findings as `BUG-{section}-{ordinal}-codex` and `...-gemini`. That is DRIFT from the repo's canonical bug-tracker contract — `plans/bug-tracker/00-overview.md:41`, `.claude/skills/add-bug/SKILL.md:75-105`, and `.claude/commands/review-work.md:106-111` all define the unsuffixed `BUG-{section}-{ordinal}` format. The rewrite created a shadow bug-ID home for the exact fallback path Section 05 calls load-bearing.
  Impact: A real orphan-finding run would produce bug entries that the rest of the tracker workflow does not recognize cleanly — `/fix-bug` expects `BUG-XX-NNN`, fix-section filenames are `fix-BUG-XX-NNN.md`, and `/review-bugs` audits the same unsuffixed shape. Agreement cases would also become two malformed bugs instead of one.
  Required plan update: Replace reviewer-suffixed BUG IDs with the canonical `BUG-{section}-{ordinal}` flow from `/add-bug`, and preserve reviewer provenance inside the bug body (via `Source:` and `Reviewers:` fields) rather than in the primary ID. Document how agreement findings collapse to one bug entry while keeping both reviewer references in the body.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed 2026-04-08. **Cross-section fix** — the same DRIFT existed in tpr-review/SKILL.md Step 7a (the template review-work was derived from), so both files were corrected in the same commit. Section 7a now documents the canonical `BUG-{section}-{ordinal}` format with explicit examples for the agreement case (ONE bug entry) and single-reviewer case (ONE bug entry with `Reviewer: codex` or `Reviewer: gemini`). Each BUG entry gets ONE ordinal — ordinal space belongs to the subsystem section, not the reviewers. Verified against `plans/bug-tracker/00-overview.md:41`, `.claude/skills/add-bug/SKILL.md:75`, `.claude/commands/review-work.md:108`. See the cross-section note added to §04.N.

- [x] `[TPR-05-002-codex][medium]` `.claude/skills/review-work/SKILL.md:400` — Restore the missing /fix-bug phase for bug-tracker resolutions.
  Evidence: After filing fallback bugs, Step 7b immediately tells Claude to fix the code and mark the bug-tracker entry resolved. That is a GAP in the canonical bug lifecycle. `CLAUDE.md` §"Bug fix rigor with `/fix-bug`", `plans/bug-tracker/00-overview.md:24-35`, and `.claude/skills/fix-bug/SKILL.md:1-18,56-68` all require bug fixes to go through `/fix-bug BUG-XX-NNN`, which creates the fix section and enforces TDD, TPR, and hygiene completion. The rewritten wrapper skipped that mandatory phase for the no-owning-plan case it is supposed to serve.
  Impact: Even if the filing format were corrected, operators following this wrapper would close fallback bugs without a `fix-BUG-XX-NNN.md` record or the required completion checklist. That leaves `/review-bugs` to report rigor gaps and breaks the project's tracked-bug discipline for review-work-discovered defects.
  Required plan update: Branch Step 7b by destination: plan-owned findings may stay in the section TPR flow, but bug-tracker findings must hand off to `/fix-bug BUG-XX-NNN` and only mark the tracker entry resolved through that fix-section workflow.
  Basis: direct_file_inspection. Confidence: high.
  Resolved: Fixed 2026-04-08. **Cross-section fix** — the same GAP existed in tpr-review/SKILL.md Step 7b (no destination branching), so both files were corrected in the same commit. Step 7b now has two subsections: 7b-i for plan-owned findings (fix inline, mark TPR resolved — unchanged from previous behavior) and 7b-ii for bug-tracker findings (DO NOT fix inline; invoke `Skill: fix-bug BUG-{section}-{ordinal}`; wait for fix-section complete; only then mark the bug-tracker entry resolved). 7b-ii references the canonical contract at `.claude/skills/fix-bug/SKILL.md` and `CLAUDE.md` §"Bug fix rigor with `/fix-bug`", and explains *why* the hand-off matters (no fix-section record, no TDD, no TPR, no hygiene, broken `/fix-next-bug` autopilot). See the cross-section note added to §04.N.

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
