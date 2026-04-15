# Final Report Protocol

Read by a Sonnet sub-agent dispatched from `/tpr-review` after the loop exits (clean pass, finding-fixing cap, thoroughness-reject cap, or transport failure). Not a registered skill.

The report agent reads every `/tmp/tpr-{run}/round-{N}/` directory in order plus the coordinator state file, and emits the final user-facing summary. Also handles user-escalation framing when a cap was hit — the coordinator dispatches this agent even on cap-hit exits because the wording and option set are consistent with clean exits.

---

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

---

## Final Report (After Loop Exits)

Tell the user:
- Total finding-fixing iterations run (`iteration_counter`)
- Total consecutive thoroughness rejections that occurred (`thoroughness_reject_counter` peak value — often 0)
- For each iteration: merged summary (`codex_findings` / `gemini_findings` / `agreements`)
- Findings surfaced and fixed per iteration
- For the final round: the thoroughness-judgment outcome (`ASYMMETRY: LOW|MODERATE|HIGH` from `status-check.sh`) and a one-sentence rationale referencing the envelopes' `files_read` / `rules_consulted` counts
- Final status — one of:
  - `clean` (both reviewers returned zero actionable findings AND thoroughness judgment accepted)
  - `max iterations reached with N remaining findings` (10-iteration finding-fixing cap hit)
  - `max thoroughness rejections reached` (3-reject cap hit — needs user intervention per §8b)
  - `aborted due to transport failure`
- Where each finding was filed (plan TPR section or bug-tracker)
