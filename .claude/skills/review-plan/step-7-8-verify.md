# Steps 7 + 8 — reviewed-flip + audit verify loop

Read by a Sonnet sub-agent dispatched from `/review-plan`. Not a registered skill.

## Input

Read:

- `/tmp/review-plan-context.json` — `mode`, `plan_dir`, `target_section`
- `/tmp/review-plan-editor.json` — Step 5 editor handoff. The `escalate` field indicates whether the editor surfaced unresolvable human-judgment issues; if absent, treat as `false`.
- `/tmp/review-plan-tpr.json` — must have `"converged": true`; if `false`, this sub-skill does NOT flip `reviewed: true` and reports the tpr-review non-convergence as the reason.

## Step 7 — Flip `reviewed` field (single-section mode ONLY)

If `mode == "single-section"` AND `tpr.converged == true` AND `editor.escalate != true`:

- Set `reviewed: true` in `{target_section}`'s frontmatter via the Edit tool.
- Record `reviewed_flipped: true` in the output.

If `mode == "whole-plan"`: skip this step entirely. Never touch `reviewed` fields in whole-plan mode.

If `tpr.converged == false` OR `editor.escalate == true`: leave `reviewed: false` and record the reason in the output (`tpr-non-convergence` or `editor-escalated`).

## Step 8 — Post-edit audit verify loop

Run plan-audit.py in verify mode until clean (max 5 iterations — escalate if it doesn't converge):

```bash
python3 .claude/skills/plan-audit/plan-audit.py {plan_dir} --verify --json \
  > /tmp/plan-audit-verify.json 2>&1
```

Read results. If `critical > 0` OR `major > 0`:

1. Apply the fixes via Edit (trust plan-audit.py's recommendations — they're deterministic)
2. Re-run the audit
3. Repeat until clean (zero critical + zero major) OR 5 iterations reached

Commit any fixes via `Skill: commit-push` with message `chore(plans): post-review audit verify fixes`.

## Output

Write `/tmp/review-plan-verify.json`. On a clean converge:

```json
{
  "reviewed_flipped": true,
  "reviewed_flipped_section": "plans/foo/section-03.md",
  "reviewed_flipped_reason": "clean | tpr-non-convergence | whole-plan-mode | editor-escalated",
  "verify_iterations": 2,
  "verify_converged": true,
  "remaining_critical": 0,
  "remaining_major": 0,
  "remaining_minor": 2,
  "summary": "Step 7+8: reviewed flipped on section-03.md, verify converged in 2 iterations (0 critical, 0 major, 2 minor remaining)",
  "escalate": false
}
```

If the verify loop does not converge within 5 iterations, include `question` + `options` inside the handoff object so the parent can pass them verbatim to `AskUserQuestion`:

```json
{
  "reviewed_flipped": false,
  "reviewed_flipped_section": "plans/foo/section-03.md",
  "reviewed_flipped_reason": "verify-non-convergence",
  "verify_iterations": 5,
  "verify_converged": false,
  "remaining_critical": 0,
  "remaining_major": 3,
  "remaining_minor": 2,
  "summary": "Step 7+8: verify loop did NOT converge in 5 iterations (3 major, 2 minor remaining)",
  "escalate": true,
  "question": "Post-edit audit verify reached the 5-iteration cap with 3 major findings still open. How do you want to proceed?",
  "options": [
    {"key": "accept-minor", "label": "Accept remaining minor findings and finish review"},
    {"key": "dispatch-editor-round-2", "label": "Run a second editor pass to clean remaining findings"},
    {"key": "abort", "label": "Abort — findings need manual attention"}
  ]
}
```

`question` and `options` MUST live inside the JSON handoff object when `escalate: true`. Never emit `options` as a sibling code block outside the handoff schema — the parent reads the fields from the JSON and passes them directly to `AskUserQuestion`.

## Do NOT

- Flip `reviewed: true` in whole-plan mode (hard rule)
- Flip `reviewed: true` when tpr-review didn't converge (hard rule)
- Edit `.rs` / `.ori` files (plan-docs only)
- Exceed 5 verify iterations without escalating
