---
name: tpr-review
description: "Run an independent dual-source (codex + gemini) third-party review in parallel, then fix findings and re-run until BOTH reviewers come back clean (full consensus). Reviews ANYTHING — code, plans, skills, docs, designs, tooling, processes, or any custom objective. TRIGGER proactively after completing ANY non-trivial work, OR when you want iterative improvement driven by multi-agent consensus. When in doubt, run it. The cost of an unnecessary review is near zero; the cost of a missed bug is high."
---

# Dual-Source TPR Review (Codex + Gemini)

Run BOTH the Codex CLI AND the Gemini CLI non-interactively in parallel to perform independent review passes, merge findings with reviewer tagging, verify each finding against the actual code, fix, and re-run until BOTH reviewers return zero actionable findings AND thoroughness is judged sufficient (full consensus).

**This is a GENERAL-PURPOSE third-party review.** The name is "TPR" — Third-Party Review — not "Third-Party Code Review." It reviews ANYTHING: code, plans, skills, docs, designs, tooling, processes, or any custom objective. The loop runs until full consensus across all agents.

**Three reviewer modes** (selected via `ARGS`):
- **Default (`review-work`)**: no ARGS, or explicit `--skill review-work` — reviewers use their `review-work` skill.
- **Plan review (`--skill review-plan`)**: reviewers use their `review-plan` skill. Invoked by `/review-plan`.
- **Custom objective** (any other ARGS): ARGS text becomes the reviewer's objective directly.

This wrapper is built on the Section 02 dual-source transport utility. All launching, parsing, schema validation, worktree-guarding, and infra retry logic lives in `.claude/skills/dual-tpr/scripts/` — this skill is purely the **semantic** fix-and-re-run loop that consumes merged findings. See `.claude/skills/dual-tpr/transport.md` for the transport contract.

## How this skill runs

SKILL.md is a thin loop coordinator. Each round of the loop dispatches two sub-agents:

- **Setup sub-agent (Sonnet)** — reads `step-1-round-setup.md`, runs Steps 0–4 + polling + merge, writes `merged.json`.
- **Triage sub-agent (Opus)** — reads `step-2-round-triage.md`, verifies findings, judges thoroughness, files + fixes + commits, writes `triage.json`.

The coordinator itself only reads the small `triage.json` output to decide loop continuation. The full reviewer prompts, envelopes, merge logic, verification-against-code, and fix implementation never touch the coordinator's context.

After the loop exits (clean pass, cap hit, or transport failure), the coordinator dispatches a **final-report sub-agent (Sonnet)** that reads all round artifacts and writes the user-facing summary.

**FOREGROUND MANDATORY — ALL Agent dispatches.** Every `Agent({})` call in the loop state machine below — setup, triage, and final-report — MUST run in the foreground (do NOT set `run_in_background: true`). The loop is sequential: setup result informs triage dispatch, triage result informs loop continuation. There is no independent work to parallelize. Backgrounding breaks the sequential contract and forces unnecessary polling.

**Model policy:** setup and final-report on Sonnet; triage on Opus. The triage agent's Opus dispatch is non-negotiable because Gemini confabulation detection requires independent verification against code — a weaker model silently accepts bad findings. The full rationale lives in `step-2-round-triage.md` §"Trust tiers (set verification depth, not pass/fail)" and in `.claude/rules/impl-hygiene.md` §"No Side Logic" (LOWER trust for gemini = mandatory FULL verification; HIGH trust for codex = spot-check). The invoker's session model is irrelevant; the dispatch boundary enforces the split.

## Finding-handling policy — SSOT reference

Finding handling is entirely the triage sub-agent's responsibility. The canonical home for that policy — "You May NEVER Reason Out of Findings", banned response list, and "Correct Architectural Solutions Only" — lives in `step-2-round-triage.md` §ABSOLUTE blocks. The coordinator (this file) restates none of it; it dispatches the triage sub-agent, reads the resulting `triage.json`, and branches the loop. This single source of truth exists to prevent coordinator and triage semantics from drifting independently (the prior version duplicated the policy here, which is exactly the `impl-hygiene.md` §Algorithmic DRY violation this refactor eliminated).

**Coordinator contract:** if `triage.round_summary` is missing or empty, that is a protocol violation — escalate to the user via `AskUserQuestion` rather than continuing silently (see the loop state machine below). The coordinator does NOT reinterpret or second-guess the triage sub-agent's accept/reject/fix disposition; its job is dispatch + branch, not policy.

## When to Trigger — Bias Toward Running

**Run this skill after completing ANY of the following:**
- Bug fixes (any severity)
- New features or feature extensions
- Refactors or code reorganization
- Multi-file changes (2+ files)
- Any change to compiler crates, codegen, type checking, evaluation, ARC/AIMS pipeline
- Test matrix additions or test infrastructure changes
- Plan section implementations
- Stdlib or registry changes
- Changes to error handling or diagnostics

**Also run when** unsure whether a change warrants review (default: run it), work involved multiple steps or non-obvious decisions, the change touches code paths shared across subsystems, or you fixed something that was interfering with other code.

**Run with a custom objective when** the user wants iterative improvement of any artifact, multi-agent consensus on quality, or the subject is not code or a plan.

**The only time NOT to run:** purely cosmetic single-line changes (typo fixes, comment edits, formatting-only).

## Loop State Machine (authoritative contract)

Infra retries are invisible to `iteration_counter` — they happen inside `dual-invoke-with-retry.sh` and either resolve (round continues) or exhaust (user escalation, counter untouched).

```
run_id = <generated e.g. /tmp/tpr-abc123>
iteration_counter = 0                # finding-fixing rounds (cap: 10)
thoroughness_reject_counter = 0      # consecutive WASTED rounds (cap: 3)
strengthened_language_required = false
loop_started_at = now()              # unix seconds — global walltime anchor
loop_max_walltime = env("ORI_TPR_LOOP_MAX_WALLTIME", default=2700)  # 45 min
single_agent_mode = false            # detected from merged.json reviewer_mode
saw_high_severity = false            # any round had critical/high findings
last_high_severity_round = -1        # most recent round with critical/high
# persist state (incl. loop_started_at) to {run_id}/state.json for sub-agents

while iteration_counter < 10 and thoroughness_reject_counter < 3:
    # ── GLOBAL WALLTIME CAP (hard ceiling, ALL rounds combined) ─
    # The per-reviewer stall/walltime caps in dual-invoke.sh bound a single
    # reviewer invocation. This cap bounds the ENTIRE /tpr-review loop —
    # setup + triage + final-report across every round. Users rely on /tpr-
    # review being a bounded operation; without this cap, 3-4 slow rounds
    # can silently consume 2+ hours. Default 45 min. Overridable only via
    # env at invocation time. The cap fires BEFORE the next round's setup
    # dispatch so we never start a round we can't finish in the remaining
    # budget.
    elapsed = now() - loop_started_at
    if elapsed >= loop_max_walltime:
        break  # exit loop; final-report sub-agent will render the walltime cap
    round_n = iteration_counter + thoroughness_reject_counter  # monotonic
    mkdir -p {run_id}/round-{round_n}/

    # ── SETUP DISPATCH (Sonnet) ─────────────────────────────
    Agent({
      subagent_type: "general-purpose",
      model: "sonnet",
      description: "tpr-review round setup",
      prompt: `
        Read .claude/skills/tpr-review/step-1-round-setup.md and execute it.
        run_id: {run_id}
        round_n: {round_n}
        args: {ARGS}            # empty | "--skill review-plan" | custom objective text
        strengthened_language_required: {strengthened_language_required}
        Read the run-state from {run_id}/state.json.
        Write merged findings to {run_id}/round-{round_n}/merged.json and a short
        summary to stdout. If the transport fails, return an escalation payload.
      `
    })

    # Read the tiny summary, not the full merged.json
    setup_out = tail -3 of the Sonnet agent's stdout

    if setup_out indicates transport failure:
        surface failure + {run_id} to user via AskUserQuestion (per Transport
        Failure Handling in step-1-round-setup.md)
        EXIT  # no counter increment

    # ── TRIAGE DISPATCH (Opus) ──────────────────────────────
    Agent({
      subagent_type: "general-purpose",
      model: "opus",
      description: "tpr-review round triage",
      prompt: `
        Read .claude/skills/tpr-review/step-2-round-triage.md and execute it.
        run_id: {run_id}
        round_n: {round_n}
        Read merged findings from {run_id}/round-{round_n}/merged.json.
        Read run-state from {run_id}/state.json.
        Verify each finding against the actual code (Gemini trust tier LOWER —
        full verification; Codex HIGH — spot-check). Judge thoroughness. File
        findings, fix them, commit via /commit-push. Write the outcome to
        {run_id}/round-{round_n}/triage.json per the schema in step-2.
      `
    })

    # Read only triage.json (small — a handful of fields + round_summary markdown)
    triage = read {run_id}/round-{round_n}/triage.json

    # ── USER-FACING ROUND RENDER (MANDATORY) ────────────────
    # Print triage.round_summary verbatim to the user BEFORE the decision
    # branches below. This is the only place the user sees per-finding
    # disposition between rounds; the coordinator deliberately does not
    # read merged.json, so if round_summary is missing or truncated the
    # user cannot track progress across rounds. If triage.round_summary
    # is absent, that is a contract violation by the triage sub-agent —
    # escalate rather than continuing silently.
    if "round_summary" not in triage or triage.round_summary is empty:
        surface contract violation to user via AskUserQuestion
        EXIT
    print triage.round_summary

    # ── SINGLE-AGENT MODE DETECTION ───────────────────────────
    # Read reviewer_mode from merged.json (set by merge-findings.py).
    # When the circuit breaker tripped one reviewer, we enter single-
    # agent mode: consensus is impossible, so we compensate with more
    # rounds and stricter severity gating.
    merged_meta = read {run_id}/round-{round_n}/merged.json  # only reviewer_mode + summary fields
    if merged_meta.get("reviewer_mode") == "single":
        single_agent_mode = true
    # Track high-severity findings across rounds
    round_max_sev = merged_meta.get("summary", {}).get("max_severity", "informational")
    if round_max_sev in ("critical", "high"):
        saw_high_severity = true
        last_high_severity_round = round_n

    # ── SINGLE-AGENT MIN-ROUNDS GATE ───────────────────────────
    # In single-agent mode, consensus is impossible (only one reviewer).
    # Compensate by requiring a minimum of 3 finding-fixing rounds
    # before accepting a clean pass. This prevents the loop from
    # exiting after a single shallow clean pass with no cross-
    # validation. The 3-round minimum ensures the surviving reviewer
    # has had multiple chances to find issues from different angles
    # (strengthened language auto-fires after each round in single
    # mode to vary the reviewer's focus).
    single_agent_min_rounds = 3  # minimum iterations before clean exit in single mode

    if triage.actionable_after_triage == 0 and triage.thoroughness_ok:
        # CLEAN PASS candidate — but gate on single-agent constraints
        if single_agent_mode:
            if iteration_counter < single_agent_min_rounds:
                # Not enough rounds yet — force another pass
                strengthened_language_required = true
                iteration_counter += 1  # count the clean round toward the minimum
                persist state; continue
            if saw_high_severity and (round_n - last_high_severity_round) < 2:
                # High-severity findings were present recently — require at
                # least one full clean round AFTER the last high-severity fix
                # before accepting. This prevents premature exit when a
                # high-sev fix might have side effects the reviewer hasn't
                # seen yet.
                strengthened_language_required = true
                iteration_counter += 1
                persist state; continue
        # All gates passed — exit clean
        break

    if triage.get("exit_clean") is True:
        # CONVERGENCE GATE (step-2 §6c.1) — the triage agent has
        # fixed this round's findings AND judged the loop has
        # converged on LOW-only cosmetic residue. The remaining
        # fixes are committed; continuing would burn rounds on
        # polishing polish. Exit clean; final-report will frame
        # the exit as "converged on cosmetics" instead of
        # "zero findings." The convergence rationale is in
        # triage.convergence_rationale and audited in round_summary.
        #
        # In single-agent mode, convergence gate still applies BUT
        # only after the min-rounds gate above is satisfied (it runs
        # first). If we reach here, the min-rounds + high-severity
        # gates have already passed.
        break

    if triage.actionable_after_triage == 0 and not triage.thoroughness_ok:
        # Pure waste — zero findings + thin review
        thoroughness_reject_counter += 1
        strengthened_language_required = true
        # iteration_counter NOT incremented — nothing was fixed
        persist state; continue

    if triage.actionable_after_triage > 0:
        # Findings filed and fixed by the triage agent
        iteration_counter += 1
        thoroughness_reject_counter = 0   # findings = progress
        strengthened_language_required = not triage.thoroughness_ok
        # In single-agent mode, always strengthen language to vary focus
        if single_agent_mode:
            strengthened_language_required = true
        persist state; continue

# ── EXIT ────────────────────────────────────────────────────
# Determine exit_reason so the final-report sub-agent renders the right
# Branch in its output schema (see step-3-final-report.md §Output Schema).
# Check order matters — the global walltime cap is the only cap that fires
# mid-iteration, so it wins over the two mid-loop caps even if one would
# nominally have also fired on the same iteration.
elapsed = now() - loop_started_at
if elapsed >= loop_max_walltime:
    exit_reason = "global_walltime_cap"
elif iteration_counter >= 10:
    exit_reason = "max_iterations_reached"
elif thoroughness_reject_counter >= 3:
    exit_reason = "max_thoroughness_rejections_reached"
elif last_triage.get("exit_clean") is True:
    exit_reason = "converged"
elif single_agent_mode:
    exit_reason = "single_agent_clean"  # clean pass in degraded mode
else:
    exit_reason = "clean"
persist exit_reason + elapsed + single_agent_mode + saw_high_severity to {run_id}/state.json

# Dispatch final-report sub-agent (Sonnet) — reads all round artifacts,
# writes the user-facing summary, frames cap-hit escalations per
# step-3-final-report.md.
Agent({
  subagent_type: "general-purpose",
  model: "sonnet",
  description: "tpr-review final report",
  prompt: `
    Read .claude/skills/tpr-review/step-3-final-report.md and execute it.
    run_id: {run_id}
    exit_reason: {exit_reason}         # clean | converged | max_iterations_reached
                                       # | max_thoroughness_rejections_reached
                                       # | global_walltime_cap
    loop_elapsed_seconds: {elapsed}
    loop_max_walltime: {loop_max_walltime}
    Read run-state from {run_id}/state.json and every
    {run_id}/round-*/triage.json file.
    Emit the final user-facing summary. If a cap was hit, frame the
    escalation and output the AskUserQuestion payload the coordinator
    should present.
  `
})
```

**Invariants:**
- `iteration_counter` increments ONLY after a successful round that found actionable findings AND those findings were fixed AND the commit landed.
- `thoroughness_reject_counter` increments ONLY on the zero-findings + thin-review cell. Resets to zero on any round that produces actionable findings.
- `strengthened_language_required` tracks the depth of the last round, independent of finding count. Set true after any thin round, cleared only after a thorough round.
- **Findings are NEVER discarded on a thin review.** The fix path runs unconditionally when findings exist; the thin signal propagates via the flag, not by throwing away data.
- Infra retries (transport), finding-fixing iterations, and thoroughness-reject iterations are three orthogonal budgets.
- Maximum semantic iterations: 10. Maximum thoroughness-reject iterations: 3 (consecutive). Hitting either cap escalates to user via AskUserQuestion.
- Thoroughness judgment is Opus's call (in the triage sub-agent), not a static threshold.

## AskUserQuestion on escalation (MANDATORY)

When the final-report sub-agent emits an escalation payload (cap hit, transport failure, or triage agent's own `"escalate": true`), the coordinator MUST invoke `AskUserQuestion` with the payload's `question` + `options` verbatim. Never dump escalations as prose.

## Files in this skill

- `SKILL.md` (this file) — loop coordinator + model policy + triggers + absolute rules.
- `step-1-round-setup.md` — Sonnet sub-agent protocol: Steps 0–4 + polling + merge + thoroughness re-review directive + transport failure handling.
- `step-2-round-triage.md` — Opus sub-agent protocol: Step 5 (verify) + Step 6 (thoroughness) + Step 7 (file + fix + commit) + merged finding format.
- `step-3-final-report.md` — Sonnet sub-agent protocol: final report + user escalation framing.

None of the `step-*.md` files are registered as skills. They are reference documents read by dispatched Agents.
