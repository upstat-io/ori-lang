---
name: review-plan
description: Review and improve a plan via a 6-phase pipeline. Each phase runs in its own Sonnet sub-agent via Agent({}); the Opus parent handles only path normalization, cross-plan invalidation judgment, and the final verdict. Change `model:` on the Step 5 Agent dispatch if empirical results show Sonnet produces vague edits.
argument-hint: "<plan-path>"
---

# Review Plan

`/review-plan <plan-path>` — review and improve a plan. `plan-path` is a plan directory (whole-plan mode) or a single section file (single-section mode).

The parent does Step 1 inline, dispatches each of Steps 2–8 as an independent Sonnet `Agent({})` sub-agent reading a step-specific `.md` protocol file, then handles Steps 8.5 and 9 on Opus.

## Reviewed-field semantics

- **Single-section mode** (file path): after the FULL pipeline clean pass, Step 7+8 flips `reviewed: true` on `{target_section}`. If unresolved issues remain, leaves `reviewed: false`.
- **Whole-plan mode** (directory path): NEVER touch `reviewed` fields. Fix content issues only.

## Step 1 — Path Normalization (parent, inline)

Inspect `$ARGUMENTS`:

- **Single file** (e.g. `plans/foo/section-03.md`): `mode = "single-section"`, `plan_dir = "plans/foo"`, `target_section = "plans/foo/section-03.md"`.
- **Directory** (e.g. `plans/foo/`): `mode = "whole-plan"`, `plan_dir = "plans/foo"`, `target_section = null`.

If the path does not exist, stop and report.

Write `/tmp/review-plan-context.json`:

```json
{
  "mode": "single-section | whole-plan",
  "plan_dir": "plans/foo",
  "target_section": "plans/foo/section-03.md | null"
}
```

## Steps 2–8 — Sequential Sub-agent Dispatch

Invoke six Agents in order. Each dispatch follows the same shape; only `description`, the step `.md` file, and model differ. After each Agent returns, read `/tmp/review-plan-{step}.json` for the summary + any escalation payload, then proceed.

**Dispatch template** (substitute `<STEP>`, `<PROTOCOL_FILE>`, and `<MODEL>`):

```
Agent({
  description: "review-plan step <STEP>",
  subagent_type: "general-purpose",
  model: "<MODEL>",
  prompt: `
You are the sub-agent for /review-plan Step <STEP>. Read
.claude/skills/review-plan/<PROTOCOL_FILE>
in full and execute it end-to-end.

Read these handoff files first for context:
- /tmp/review-plan-context.json (plan mode, plan_dir, target_section)
{for steps 3+: list /tmp/review-plan-*.json files from prior steps}

Write your own handoff to /tmp/review-plan-<STEP>.json per the protocol's
"Output" section. Never read CLAUDE.md unless the protocol explicitly
requires it (only Step 5 does).

Commits via /commit-push only — never run git commit directly.

Touch only plan docs (plans/**/*.md) unless the protocol explicitly
authorizes other edits. Never edit .rs, .ori, or anything under compiler/,
library/, tests/.

If the protocol says to escalate, return the escalation payload in the
handoff JSON (escalate: true) and stop. The parent handles escalations.
  `
})
```

**Per-step dispatch parameters:**

| Step | `<STEP>` | `<PROTOCOL_FILE>` | `<MODEL>` | Writes |
|---|---|---|---|---|
| 2 | `2-precheck` | `step-2-precheck.md` | `opus` | `/tmp/review-plan-precheck.json` |
| 3 | `3-audit` | `step-3-audit.md` | `sonnet` | `/tmp/review-plan-audit.json` |
| 4 | `4-blind-spots` | `step-4-blind-spots.md` | `sonnet` | `/tmp/review-plan-blind-spots.json` |
| 5 | `5-editor` | `step-5-editor.md` | `opus` | `/tmp/review-plan-editor.json` |
| 6 | `6-tpr` | `step-6-tpr.md` | `sonnet` | `/tmp/review-plan-tpr.json` |
| 7+8 | `7-8-verify` | `step-7-8-verify.md` | `sonnet` | `/tmp/review-plan-verify.json` |

**Model policy note:** Step 5 (the editor) runs on Sonnet by current policy. If empirical convergence metrics show Sonnet produces vague edits, change the Step 5 dispatch's `<MODEL>` to `opus` — no other file changes required.

## Escalation handling (MANDATORY)

After each Agent returns, read `/tmp/review-plan-{step}.json`. If `"escalate": true`:

1. Invoke `AskUserQuestion` with the sub-agent's `question` + `options` verbatim. Never dump as prose.
2. When the user picks an option with a `next_skill`, invoke that skill with the provided arg.
3. When the user picks `proceed` / `abort` / `leave-as-is`, honor that choice. If `abort`, stop the pipeline and emit a partial verdict at Step 9.
4. Resume the next step only after the escalation resolves.

Precheck is the most common escalation source; steps 3, 5, 6, and 7+8 can also escalate on non-convergence or human-judgment ambiguities. Every escalation-capable step's protocol file defines its own machine-readable escalation schema — see each `step-*.md` for the exact shape. Step 4 (blind-spots) never escalates.

## Step 8.5 — Cross-Plan Invalidation (parent, Opus)

After Step 7+8 returns clean, run the invalidation detector inline on the parent:

```bash
python3 .claude/skills/plan-audit/plan-invalidate.py {plan_dir} --json > /tmp/review-plan-invalidate.json
```

Read the output. If `status == "clean"`, skip to Step 9.

If stale sections exist, invoke `AskUserQuestion`:

- **Q**: "This review changed scope that overlaps with N reviewed sections across M other plans. How should their `reviewed: true` fields be handled?"
- **Options**:
  1. "Invalidate all N sections"
  2. "Invalidate high-impact only (weight ≥ 4)"
  3. "Skip — leave reviews as-is"

Apply the user's choice:

```bash
# Option 1:
python3 .claude/skills/plan-audit/plan-invalidate.py {plan_dir} --apply

# Option 2:
python3 .claude/skills/plan-audit/plan-invalidate.py {plan_dir} --apply --min-weight 4

# Option 3: no action
```

Skip this step entirely if the review made only cosmetic/formatting changes (no scope shifts).

## Step 9 — Present Verdict (parent, Opus)

Read the `summary` line from each `/tmp/review-plan-*.json` (not the full handoffs) and synthesize:

```
## Plan Review: {plan name}

### Pipeline Summary
- **Pre-check** (Step 2): {N effectively-complete sections corrected}
- **Phase 1** (Step 3, plan-audit.py): {N findings, M auto-fixed, K remaining}
- **Phase 2** (Step 4, /tp-help): {N blind spots}
- **Phase 3** (Step 5, editor): {structural changes + content edits}
- **Phase 4** (Step 6, /tpr-review): {clean on iteration N | max reached}
- **Post-edit verification** (Step 7+8): {CLEAN | N remaining}

### Review Status
| Section | `reviewed` Before | `reviewed` After | Reason |
| ... | ... | ... | ... |

### Cross-Plan Invalidation
{Results or "Skipped — changes were cosmetic."}

### Remaining Concerns
{Human-judgment issues ranked Critical > Major > Minor, or "none"}

---

## Verdict
**{CLEAN | MINOR FIXES APPLIED | SIGNIFICANT REWORK APPLIED | RESTRUCTURED | NEEDS MANUAL ATTENTION}**

{2-3 sentences summarizing total edits across all phases.}
```

**Verdict definitions:**
- **CLEAN** — no issues found
- **MINOR FIXES APPLIED** — small corrections
- **SIGNIFICANT REWORK APPLIED** — substantial edits (reordered, added missing sections, fixed incorrect assumptions)
- **RESTRUCTURED** — structure fundamentally changed
- **NEEDS MANUAL ATTENTION** — issues requiring human judgment

## Critical Rules

1. **Sequential phases** — each Agent completes before the next starts. Handoffs flow via `/tmp/*.json` files.
2. **`reviewed` flip is LAST** — only in single-section mode, only after Step 6 converges clean. Handled by Step 7+8's agent, never inside the editor.
3. **Whole-plan mode never touches `reviewed`** — not even to add missing ones.
4. **NEVER scope down — always expand** — grow the plan if it doesn't fulfill its mission.
5. **`/tpr-review` is MANDATORY** — Step 6 runs it unconditionally.
6. **Never dismiss TPR findings as "unrelated"** — per CLAUDE.md, no "pre-existing" / "out of scope".
7. **AskUserQuestion on escalation** — never dump escalation payloads as prose.

## Files in this skill

- `SKILL.md` (this file) — parent dispatcher + Steps 1, 8.5, 9.
- `step-2-precheck.md` — Step 2 protocol (effectively-complete detection).
- `step-3-audit.md` — Step 3 protocol (plan-audit.py orchestration).
- `step-4-blind-spots.md` — Step 4 protocol (/tp-help dispatch + distill).
- `step-5-editor.md` — Step 5 protocol (4-lens editor).
- `step-6-tpr.md` — Step 6 protocol (/tpr-review convergence loop).
- `step-7-8-verify.md` — Steps 7+8 protocol (reviewed flip + audit verify loop).

None of `step-*.md` files are registered as skills. They are reference documents read by dispatched Agents.
