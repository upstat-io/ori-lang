# Step 6 — /tpr-review convergence loop

Read by a Sonnet sub-agent dispatched from `/review-plan`. Not a registered skill.

## Input

Read `/tmp/review-plan-context.json` for `mode`, `plan_dir`, `target_section`.

## Dispatch /tpr-review

Use the Skill tool with `--skill review-plan` so that /tpr-review:

- Uses `review-plan` activation preambles (codex: `Run the /review-plan skill in envelope-only mode.`; gemini: `Activate the review-plan skill and follow its instructions exactly.`)
- Passes `--skill review-plan` to the transport (correct `round.log` attribution)
- Launches Codex and Gemini in parallel using the `review-plan` reviewer skill
- Merges findings from both reviewers
- Fixes actionable findings directly
- Re-runs until both reviewers return zero actionable findings (max 10 iterations)

```
Skill: tpr-review
Args: --skill review-plan
```

In single-section mode, scope the review to `{target_section}` (pass as the path arg to `/tpr-review`). In whole-plan mode, pass `{plan_dir}`.

**Wait for /tpr-review to complete fully.** Do not return partial results.

## Parse /tpr-review output

Extract:

- `iterations`: how many convergence rounds ran
- `converged`: `true` if both reviewers returned zero actionable findings on the final iteration; `false` if max iterations reached with remaining findings
- `per_iteration_counts`: array of finding counts per iteration (e.g., `[12, 5, 1, 0]`)
- `final_findings`: if not converged, the remaining findings on the last iteration

## Output

Write `/tmp/review-plan-tpr.json`. When /tpr-review converges cleanly:

```json
{
  "iterations": 3,
  "converged": true,
  "per_iteration_counts": [12, 5, 1, 0],
  "final_findings": [],
  "summary": "Phase 4: clean on iteration 3 (counts: 12→5→1→0)",
  "escalate": false
}
```

When `converged: false` and max iterations reached, embed `question` + `options` inside the JSON handoff itself so the parent can invoke `AskUserQuestion` verbatim without reconstructing prompt text:

```json
{
  "iterations": 10,
  "converged": false,
  "per_iteration_counts": [12, 8, 5, 4, 3, 3, 2, 2, 2, 2],
  "final_findings": [/* ...remaining findings... */],
  "summary": "Phase 4: max iterations reached with 2 findings remaining",
  "escalate": true,
  "question": "/tpr-review reached its 10-iteration cap with 2 findings still open. How do you want to proceed?",
  "options": [
    {"key": "accept-remaining", "label": "Accept remaining findings and continue to verify"},
    {"key": "retry-with-hints", "label": "Retry /tpr-review with user-provided hints"},
    {"key": "abort", "label": "Abort review — findings need manual attention"}
  ]
}
```

`question` and `options` MUST live inside the JSON handoff object when `escalate: true` — the parent uses them as-is. Never emit `options` as a sibling code block outside the handoff schema.

## Do NOT

- Reimplement /tpr-review logic inline
- Add polling/foreground/background directives — /tpr-review manages its own transport
- Run /tpr-review without `--skill review-plan` (wrong reviewer preambles would load)
