---
section: "06"
title: "/review-plan new Claude skill (parallel to existing command file)"
status: not-started
reviewed: true
goal: "Create a NEW .claude/skills/review-plan/SKILL.md from scratch as a dual-source codex+gemini wrapper for plan review. This runs PARALLEL to the existing .claude/commands/review-plan.md (595-line 4-agent Claude pipeline), which is left UNTOUCHED per the Step 1E command-file boundary decision. Two paths coexist by design: the existing command file for the multi-agent Claude pipeline, the new skill for dual-source codex+gemini review."
success_criteria:
  - ".claude/skills/review-plan/SKILL.md exists as a new file (currently there is no Claude-side review-plan wrapper)"
  - "The new skill follows Section 04's dual-source pattern adapted for plan-review semantics"
  - "Findings in envelope-only mode describe PROPOSED plan edits rather than applying them; the wrapper applies edits to plan files after the user approves them"
  - "At least 1 real plan-review scenario runs successfully with both reviewers producing proposed edits"
  - ".claude/commands/review-plan.md is BYTE-IDENTICAL to its pre-plan state (verified by git diff --exit-code)"
  - "The existing 4-agent Claude pipeline invoked by .claude/commands/review-plan.md continues to work unchanged (regression test)"
depends_on: ["04"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Create .claude/skills/review-plan/SKILL.md from scratch"
    status: not-started
  - id: "06.2"
    title: "Validate against real plan-review scenarios"
    status: not-started
  - id: "06.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "06.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: /review-plan new Claude skill (parallel to existing command file)

**Status:** Not Started
**Goal:** Create the greenfield `.claude/skills/review-plan/SKILL.md` wrapper that provides dual-source review capability for plan review. The existing 595-line `.claude/commands/review-plan.md` is intentionally left in place as a parallel workflow.

**Success Criteria:**

- [ ] `.claude/skills/review-plan/SKILL.md` exists as a new file (it did not exist before this section)
- [ ] The new skill follows Section 04's dual-source pattern, adapted for plan review — findings are proposed plan edits rather than code changes
- [ ] `.claude/commands/review-plan.md` is BYTE-IDENTICAL to its pre-plan state (regression test)
- [ ] The existing 595-line 4-agent Claude pipeline continues to work unchanged when invoked as `/review-plan` via the command file
- [ ] The new skill is invokable via the Skill tool with the plan directory as an argument (e.g., `Skill: review-plan, args: plans/foo/`); typing `/review-plan` continues to hit the untouched command file per Step 1E
- [ ] At least 1 real plan-review scenario runs the new skill successfully with both reviewers producing proposed edits, invoked via the Skill tool NOT via the `/review-plan` slash command
- [ ] The wrapper applies approved edits to plan files after the user reviews the proposed edits (single-writer property: Claude writes, not the reviewers)

**Context:** This section creates a new Claude-side skill that has NO pre-existing version — `.claude/skills/review-plan/` did not exist before. The existing `/review-plan` functionality lives entirely in the 595-line `.claude/commands/review-plan.md` command file which runs a 4-agent Claude pipeline and invokes `/tp-help` internally for blind-spot checks. That command file is untouched.

The new skill file is greenfield, which is architecturally simpler than Section 05 (no contradiction to fix, no existing workflow to preserve) but introduces a new semantic question: what does "envelope-only" mean for plan review? Answer (from Section 03.2's design): each finding describes a PROPOSED plan edit (file path, line number, change description) rather than applying the edit. The wrapper then presents the proposed edits to the user (or applies them after confirmation) — Claude is the single writer to plan files, not the reviewers.

**Reference implementations:**
- Section 04 (`/tpr-review` dual-source) — the validated pattern
- Section 03.2's codex review-plan envelope-only mode — defines what a review-plan finding looks like in envelope-only mode
- Existing `.claude/commands/review-plan.md` (595 lines) — left untouched, kept as parallel workflow

**Depends on:** Section 04 (validated dual-source pattern).

---

## 06.1 Create .claude/skills/review-plan/SKILL.md from scratch

**File(s):** `.claude/skills/review-plan/SKILL.md` (new)

**Context:** Greenfield skill file. Follows Section 04's dual-source pattern but adapted for plan review: the wrapper invokes both codex and gemini in envelope-only mode (both reviewers run `.codex/skills/review-plan/SKILL.md` and `.gemini/skills/review-plan/SKILL.md` respectively), receives envelopes describing proposed plan edits, presents them to the user, and applies approved edits to plan files.

Tasks:

- [ ] Create directory `.claude/skills/review-plan/`.

- [ ] Write `.claude/skills/review-plan/SKILL.md` with the following structure:
  - Frontmatter: `name: review-plan`, description mentions "dual-source codex+gemini review of a plan directory or section"
  - `## Step 0 — MANDATORY: Re-read CLAUDE.md`
  - `## Usage`: `/review-plan <plan-path>` where plan-path is a plan directory or specific section file
  - `## Parallel Workflow Notice`: explicit statement that `.claude/commands/review-plan.md` exists as a separate parallel workflow (4-agent Claude pipeline) and this skill is the dual-source codex+gemini alternative. The user chooses which workflow they want — the command file for multi-agent Claude orchestration, this skill for cross-model codex+gemini review.
  - `## Loop Protocol`: UNLIKE review-work and tpr-review, /review-plan does NOT loop. Plan review runs once per invocation and produces proposed edits. Looping would require re-reviewing after applying edits, which is a different workflow.
  - `## Steps (Single Pass)`:
    1. Resolve plan directory/section from the user's argument
    2. Create per-run scratch dir via `scratch-dir.sh`
    3. Write codex prompt with `envelope-only` keyword and the target plan path
    4. Write gemini prompt with "Activate the review-plan skill and..." preamble and the target plan path
    5. Invoke `dual-invoke-with-retry.sh`
    6. Parse both envelopes (already cached by transport)
    7. Merge findings via `merge-findings.py` — each finding is a proposed plan edit
    8. Present proposed edits to the user via `AskUserQuestion`, grouped by target plan file, with agreement/disagreement annotations
    9. For each approved edit, apply it to the target plan file (Claude is the single writer)
    10. Report summary to user: how many edits applied, how many rejected, which files modified
  - `## Finding Format for Plan Edits`: each finding in the envelope has:
    - `location`: plan file + line number (e.g., `plans/foo/section-02-bar.md:45`)
    - `title`: imperative description of the edit (e.g., "Add worktree guard mention to Section 02 success criteria")
    - `evidence`: the current plan content that is inaccurate or missing
    - `impact`: why the plan is incomplete/wrong without the edit
    - `required_plan_update`: the proposed replacement text or addition
  - `## Failure Handling`: same as Section 04 — infra failures surface to user with $RUN path; no silent retry
  - `## Reviewed Field Semantics`: DO NOT flip any section's `reviewed: true` during whole-plan review (preserved from the existing codex review-plan skill's behavior)

- [ ] Verify the skill file has correct YAML frontmatter and is readable by the Skill tool mechanism.

- [ ] **Subsection close-out (06.1)** — MANDATORY before starting 06.2:
  - [ ] New skill file exists, follows dual-source pattern, does NOT modify `.claude/commands/review-plan.md`
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively — was it clear how to adapt Section 04's pattern for a single-pass (non-looping) wrapper? Should there be a `dual-source-single-pass-template.md` to complement the looping pattern? Implement improvements.

---

## 06.2 Validate against real plan-review scenarios

**File(s):** Validation only

**Context:** Verify the new skill works end-to-end and doesn't interfere with the existing command-file workflow.

Tasks:

- [ ] **Scenario — Dual-source plan review**: Run the new `.claude/skills/review-plan/SKILL.md` against a small test plan (perhaps a completed plan in `plans/completed/` for a read-only test). Verify:
  - Both reviewers produce envelopes with proposed edits as findings
  - The wrapper presents proposed edits grouped by target plan file
  - User approval flow (`AskUserQuestion`) works
  - Approved edits are applied to plan files by Claude (verify via `git diff`)
  - Rejected edits are not applied

- [ ] **Scenario — Regression: existing command file unchanged**: Run the existing `/review-plan` via the command file on the same test plan. Verify:
  - The command file's 4-agent pipeline runs (not the new skill)
  - `git diff --exit-code .claude/commands/review-plan.md` returns 0 (unchanged)
  - The command file's output shape is unchanged from pre-plan state

- [ ] Record scenario results in working notes.

- [ ] **Subsection close-out (06.2)** — MANDATORY before section completion:
  - [ ] Dual-source scenario passes
  - [ ] Command file regression scenario passes
  - [ ] Update this subsection's `status` to `complete`
  - [ ] Run `/improve-tooling` retrospectively — was routing between the two workflows (command file vs new skill) clear to the user? Should the skill description explicitly mention both paths? Implement improvements.

---

## 06.R Third Party Review Findings

- None.

---

## 06.N Completion Checklist

- [ ] Both subsections (06.1, 06.2) marked `complete`
- [ ] `.claude/skills/review-plan/SKILL.md` exists as a new file
- [ ] `.claude/commands/review-plan.md` is BYTE-IDENTICAL: `git diff --exit-code .claude/commands/review-plan.md` exits 0
- [ ] Dual-source scenario passes (both reviewers produce proposed edits; user approval flow works; Claude applies approved edits)
- [ ] Command-file regression scenario passes (existing 4-agent pipeline unchanged)
- [ ] `timeout 150 ./test-all.sh` green
- [ ] Plan annotation cleanup clean
- [ ] **Plan sync**: Section 06 frontmatter → `complete`, Quick Reference updated
- [ ] `/tpr-review` (dual-source) passed
- [ ] `/impl-hygiene-review` passed after TPR clean
- [ ] `/improve-tooling` section-close sweep done

**Exit Criteria:** The new `.claude/skills/review-plan/SKILL.md` provides dual-source review for plans via codex+gemini. The existing `.claude/commands/review-plan.md` 4-agent pipeline continues to work unchanged. Users can choose which workflow they want. Section 07 can begin.
