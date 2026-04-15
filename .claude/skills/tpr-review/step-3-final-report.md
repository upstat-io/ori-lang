# Final Report Protocol

Read by a Sonnet sub-agent dispatched from `/tpr-review` after the loop exits (clean pass, finding-fixing cap, thoroughness-reject cap, or transport failure). Not a registered skill.

The report agent reads every `/tmp/tpr-{run}/round-{N}/` directory in order plus the coordinator state file, emits the final user-facing summary, AND writes a machine-readable handoff JSON so the coordinator can drive `AskUserQuestion` mechanically on cap-hit / failure paths — see §Output Schema below.

## Output Schema (MANDATORY)

Write `/tmp/tpr-{run}/final-report.json` before returning control to the coordinator. The coordinator reads this file and, when `escalate: true`, feeds `question` + `options` directly into `AskUserQuestion` (per `SKILL.md` §200–202). A missing or malformed schema is a contract violation — the coordinator cannot present escalations as prose, per `CLAUDE.md` rules.

The schema is keyed on `status`, which mirrors the four terminal paths in §8 below:

### Branch 1 — `status: "clean"` (no escalation)

```json
{
  "status": "clean",
  "iteration_counter": 3,
  "thoroughness_reject_counter_peak": 0,
  "asymmetry": "LOW",
  "summary_markdown": "…the full user-facing summary from §Final Report below…",
  "escalate": false
}
```

### Branch 2 — `status: "max_iterations_reached"` (finding-fixing cap — §8a)

```json
{
  "status": "max_iterations_reached",
  "iteration_counter": 10,
  "thoroughness_reject_counter_peak": 0,
  "asymmetry": "LOW|MODERATE|HIGH",
  "summary_markdown": "…",
  "remaining_findings": [/* the latest $RUN/merged.json actionable entries */],
  "per_iteration_counts": [12, 8, 5, 4, 3, 3, 2, 2, 2, 2],
  "escalate": true,
  "question": "/tpr-review reached its 10-iteration finding-fixing cap with N findings still open. How do you want to proceed?",
  "options": [
    {"key": "continue-past-cap", "label": "Continue past the 10-iteration cap for another round"},
    {"key": "file-and-stop", "label": "File remaining findings to the owning plan/bug-tracker and stop"},
    {"key": "dig-into-finding", "label": "Dig into a specific recurring finding interactively"}
  ]
}
```

### Branch 3 — `status: "max_thoroughness_rejections_reached"` (depth cap — §8b)

```json
{
  "status": "max_thoroughness_rejections_reached",
  "iteration_counter": 0,
  "thoroughness_reject_counter_peak": 3,
  "asymmetry": "HIGH",
  "summary_markdown": "…",
  "rejection_rationales": [
    "round-A: walltime ratio 3.8x, empty rules_consulted",
    "round-B: files_read=2 on a 14-file diff",
    "round-C: verification block empty despite codegen scope"
  ],
  "run_path": "/tmp/tpr-{run}",
  "escalate": true,
  "question": "/tpr-review rejected 3 consecutive rounds as thin (zero findings + insufficient depth). Prompt discipline is not eliciting the required investigation. How do you want to proceed?",
  "options": [
    {"key": "accept-best-effort", "label": "Accept the last round as a best-effort clean pass (informed override)"},
    {"key": "narrow-scope", "label": "Narrow the review scope and retry"},
    {"key": "change-intervention", "label": "Change the intervention — swap a reviewer or adjust the rubric"},
    {"key": "abandon-review", "label": "Abandon this review — leave the work un-reviewed with a note"}
  ]
}
```

### Branch 4 — `status: "transport_failure"` (infra retries exhausted)

```json
{
  "status": "transport_failure",
  "iteration_counter": 0,
  "thoroughness_reject_counter_peak": 0,
  "asymmetry": null,
  "summary_markdown": "…",
  "failure_category": "launch_or_exit_fail|codex_*|gemini_*|dirty_worktree|unknown_failure",
  "run_path": "/tmp/tpr-{run}",
  "postmortem_files": ["round.log", "codex.parse-error", "gemini.parse-error", "…"],
  "escalate": true,
  "question": "/tpr-review aborted because the dual-source transport exhausted its 3 infra retries. The postmortem is preserved at {run_path}. How do you want to proceed?",
  "options": [
    {"key": "triage-failure", "label": "Triage the failure — open round.log and the indicated files"},
    {"key": "retry-immediately", "label": "Retry /tpr-review immediately (use sparingly)"},
    {"key": "abandon-review", "label": "Abandon this review — log the failure category in the owning plan's notes"}
  ]
}
```

### Invariants

- `status` is MANDATORY and MUST be one of the four values above — the coordinator branches on it.
- When `escalate: true`, `question` and `options` MUST both be present and non-empty — this is the canonical `AskUserQuestion` payload the coordinator uses verbatim.
- When `status == "clean"`, `escalate` MUST be `false` AND `question`/`options` MUST be absent.
- `summary_markdown` is the same user-facing summary rendered in §Final Report below, stored here so the coordinator can print it even if it chooses not to render the report directly.


### 8. User Escalation — Finding-Fixing Cap or Thoroughness-Reject Cap

Two distinct cap-hit escalations exist. Use the right one; they describe different failure modes and warrant different user decisions.

#### 8a. After Max Finding-Fixing Iterations (10) — findings keep surfacing

If after 10 semantic iterations actionable findings are still surfacing, surface the remaining merged findings to the user via `AskUserQuestion`:
- Summary of semantic iterations run
- Count of findings per iteration (shows whether progress is being made)
- The current merged finding list (from the latest `$RUN/merged.json`)
- Ask: should we continue past the 10-iteration cap, file remaining findings and stop, or dig into a specific finding that keeps recurring?

#### 8b. After Max Wasted Rounds (3) — prompt discipline not eliciting depth

If `thoroughness_reject_counter` reaches 3, the reviewers have produced three consecutive **wasted rounds** — the specific zero-findings + thin-review cell (§6e), where each round captured nothing: no findings AND no verified depth. Note this cap only counts the "pure waste" cell; findings-present thin rounds do NOT increment the counter (they were still progress, even if thin). Hitting this cap therefore means: the last three rounds produced literally nothing despite Claude explicitly requesting deeper review each time. This is a fundamentally different failure mode from 8a: the loop has NOT been making forward progress, it has been spinning on empty rounds while Claude refused to accept skimming passes.

Surface to the user via `AskUserQuestion` with:
- The three rejection rationales (which signals triggered each reject — walltime ratio, event count, thin `files_read`, empty `rules_consulted`, etc.)
- The final `$RUN` path with both envelopes and `status-check.sh` output
- The `status-check.sh` final asymmetry snapshot from the last round
- Ask the user to choose one of:
  1. **Accept the last round as a best-effort clean pass** — if the user reviews the envelopes directly and judges the depth acceptable, override Claude's rejection and exit clean. This is an informed override, not a concession.
  2. **Narrow the scope** — the reviewers may be skimming because the scope is too broad for the time budget. A narrower scope often elicits deeper investigation.
  3. **Change the intervention** — prompt discipline isn't working; the user may want to swap a reviewer, adjust the rubric in `command-file.md`, or escalate to a human review.
  4. **Abandon this review** — if none of the above fits, stop the loop and leave the work un-reviewed with a note in any owning plan's working notes recording `$RUN` for later inspection.

Never silently continue past the 3-thoroughness-reject cap. Doing so either (a) eventually accepts a skimming pass without informed override, defeating the whole thoroughness judgment mechanism, or (b) burns unbounded rounds chasing a depth the reviewers structurally cannot produce.


## Final Report (After Loop Exits)

Tell the user:
- Total finding-fixing iterations run (`iteration_counter`)
- Total consecutive thoroughness rejections that occurred (`thoroughness_reject_counter` peak value — often 0)
- For each iteration: **reuse `triage.json.round_summary` verbatim** rather than re-deriving the per-round detail from `merged.json`. This is the same markdown the coordinator already printed between rounds (per `step-2-round-triage.md` §Round Summary Rendering), so reusing it keeps the final report consistent with what the user saw in real time and avoids wording drift between the two renderings. If a round is missing `round_summary` (contract violation by the triage sub-agent), note the gap and fall back to counts from the other `triage.json` fields, but flag it as a skill bug.
- Findings surfaced and fixed per iteration (already covered by the per-round summaries above — do not duplicate)
- For the final round: the thoroughness-judgment outcome (`ASYMMETRY: LOW|MODERATE|HIGH` from `status-check.sh`) and a one-sentence rationale referencing the envelopes' `files_read` / `rules_consulted` counts
- Final status — one of:
  - `clean` (both reviewers returned zero actionable findings AND thoroughness judgment accepted)
  - `max iterations reached with N remaining findings` (10-iteration finding-fixing cap hit)
  - `max thoroughness rejections reached` (3-reject cap hit — needs user intervention per §8b)
  - `aborted due to transport failure`
- Where each finding was filed (plan TPR section or bug-tracker)
