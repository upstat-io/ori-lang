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

`/tpr-review` has FOUR terminal statuses (see `.claude/skills/tpr-review/step-3-final-report.md` §"Output Schema (MANDATORY)", Branches 1–4). Each must map to a defined handoff shape — a missing case would leave the /review-plan pipeline ambiguous exactly when it should stop cleanly and ask the user.

Extract from the final-report agent's output (field names are exactly as emitted by `/tpr-review` in `final-report.json` — do NOT rename):

- `status`: one of
  - `clean` — both reviewers returned zero actionable findings AND thoroughness judgment accepted
  - `max_iterations_reached` — 10-iteration finding-fixing cap hit with remaining findings
  - `max_thoroughness_rejections_reached` — 3-reject cap hit (reviewers produced three consecutive thin-waste rounds despite strengthening prompts)
  - `transport_failure` — dual-invoke transport exhausted infra retries before any loop iteration could complete
- `iteration_counter`: how many convergence rounds ran (may be 0 on `transport_failure`)
- `per_iteration_counts`: array of finding counts per iteration (e.g., `[12, 5, 1, 0]`; present on `max_iterations_reached`; may be absent/empty on other statuses)
- `remaining_findings`: remaining findings on the last iteration (present on `max_iterations_reached`; absent when `status == "clean"` or `"transport_failure"`)
- `thoroughness_reject_counter_peak`: peak value of the wasted-round counter (0 unless thoroughness-cap path fired)
- Any `question` + `options` payload the final-report agent emitted for cap/failure paths

**Field-name parity**: the consumer-side handoff below (`/tmp/review-plan-tpr.json`) uses the shorter names `iterations`, `final_findings`, `thoroughness_reject_counter` as a deliberate rename at the `/review-plan` boundary. Copy values through explicitly from `final-report.json`:

- `iterations` ← `iteration_counter`
- `final_findings` ← `remaining_findings` (if present)
- `thoroughness_reject_counter` ← `thoroughness_reject_counter_peak`

Do NOT assume the producer emits the shorter names; they are consumer-side aliases only.

## Output

Write `/tmp/review-plan-tpr.json`. Exactly one of the four branches below applies, keyed by `status`. The top-level schema is the same; only the `escalate`/`question`/`options` fields differ.

### Branch 1 — `status: "clean"` (no escalation)

```json
{
  "status": "clean",
  "iterations": 3,
  "converged": true,
  "per_iteration_counts": [12, 5, 1, 0],
  "final_findings": [],
  "thoroughness_reject_counter": 0,
  "summary": "Phase 4: clean on iteration 3 (counts: 12→5→1→0)",
  "escalate": false
}
```

### Branch 2 — `status: "max_iterations_reached"` (finding-fixing cap)

```json
{
  "status": "max_iterations_reached",
  "iterations": 10,
  "converged": false,
  "per_iteration_counts": [12, 8, 5, 4, 3, 3, 2, 2, 2, 2],
  "final_findings": [/* ...remaining findings... */],
  "thoroughness_reject_counter": 0,
  "summary": "Phase 4: max iterations reached with 2 findings remaining",
  "escalate": true,
  "question": "/tpr-review reached its 10-iteration finding-fixing cap with 2 findings still open. How do you want to proceed?",
  "options": [
    {"key": "accept-remaining", "label": "Accept remaining findings and continue to verify"},
    {"key": "retry-with-hints", "label": "Retry /tpr-review with user-provided hints"},
    {"key": "abort", "label": "Abort review — findings need manual attention"}
  ]
}
```

### Branch 3 — `status: "max_thoroughness_rejections_reached"` (depth cap — 3 wasted rounds)

```json
{
  "status": "max_thoroughness_rejections_reached",
  "iterations": 0,
  "converged": false,
  "per_iteration_counts": [],
  "final_findings": [],
  "thoroughness_reject_counter": 3,
  "summary": "Phase 4: max thoroughness rejections reached (3 wasted rounds — reviewers produced zero findings AND thin depth despite strengthening)",
  "escalate": true,
  "question": "/tpr-review rejected 3 consecutive rounds as thin (zero findings + insufficient depth). Prompt discipline is not eliciting the required investigation. How do you want to proceed?",
  "options": [
    {"key": "accept-best-effort", "label": "Accept the last round as a best-effort clean pass (informed override)"},
    {"key": "narrow-scope", "label": "Narrow the review scope and retry"},
    {"key": "change-intervention", "label": "Change the intervention — swap a reviewer or adjust the rubric"},
    {"key": "abandon-review", "label": "Abandon this review — leave the plan un-reviewed with a note"}
  ]
}
```

### Branch 4 — `status: "transport_failure"` (infra retries exhausted)

```json
{
  "status": "transport_failure",
  "iterations": 0,
  "converged": false,
  "per_iteration_counts": [],
  "final_findings": [],
  "thoroughness_reject_counter": 0,
  "summary": "Phase 4: aborted due to transport failure before any round could complete",
  "escalate": true,
  "failure_category": "<literal category string from the transport — e.g. launch_or_exit_fail, codex_parse_fail, gemini_missing_terminator>",
  "postmortem_dir": "<$RUN path so the operator can inspect envelopes / round.log / parse-error files>",
  "question": "/tpr-review aborted because the dual-source transport exhausted its 5 infra retries. The postmortem is preserved. How do you want to proceed?",
  "options": [
    {"key": "triage-failure", "label": "Triage the failure — open $RUN/round.log and the indicated files"},
    {"key": "retry-immediately", "label": "Retry /tpr-review immediately (use sparingly — transport failures usually reflect real infra bugs)"},
    {"key": "abandon-review", "label": "Abandon this review — log the failure category in the plan's working notes"}
  ]
}
```

### Invariants

- `question` and `options` MUST live INSIDE the JSON handoff object when `escalate: true` — the parent uses them as-is with `AskUserQuestion`. Never emit `options` as a sibling code block outside the handoff schema.
- `status` is MANDATORY — the parent's Step 6 consumer branches on it. An absent or unknown `status` is a contract violation by this step.
- When `status == "clean"`, `escalate` MUST be `false` and `question`/`options` MUST be absent.
- When `status != "clean"`, `escalate` MUST be `true` AND `question` + `options` MUST be present.

## Do NOT

- Reimplement /tpr-review logic inline
- Add polling/foreground/background directives — /tpr-review manages its own transport
- Run /tpr-review without `--skill review-plan` (wrong reviewer preambles would load)
