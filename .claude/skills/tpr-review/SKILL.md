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

**Model policy:** setup and final-report on Sonnet; triage on Opus. The triage agent's Opus dispatch is non-negotiable because Gemini confabulation detection requires independent verification against code — a weaker model silently accepts bad findings. The full rationale lives in `step-2-round-triage.md` §"Trust tiers (set verification depth, not pass/fail)" and in `.claude/rules/impl-hygiene.md` §"No Side Logic" (LOWER trust for gemini = mandatory FULL verification; HIGH trust for codex = spot-check). The invoker's session model is irrelevant; the dispatch boundary enforces the split.

## ABSOLUTE: You May NEVER Reason Out of Findings

**There is NO circumstance under which you may dismiss, rationalize, scope-note, or defer a TPR finding.** The ONLY valid responses to a finding are:

1. **Fix it NOW** — write code, write tests, verify, commit
2. **Create a plan and execute it** — if too large for inline fix, create concrete implementation steps, then implement them
3. **AskUserQuestion** — if genuinely blocked (need user decision, missing domain knowledge)

**BANNED responses to findings — using ANY of these is a violation:**
- "Pre-existing issue" / "was already broken"
- "Architectural limitation" / "requires major refactor"
- "Out of scope" / "not a §03 deliverable"
- "Conservative/safe" / "only precision loss"
- "Not a regression" / "not introduced by this work"
- "Future improvement" / "tracked for later"
- "Scoped as known limitation"
- Marking `[x] Resolved:` with an explanation instead of a code fix

**The size of the fix is irrelevant.** If the correct fix requires cross-crate refactoring across 10 files, that IS the work.

**"Future improvement" requires a concrete artifact.** If you ever say something will be tracked, you MUST in the same response create: a bug-tracker entry (`/add-bug`), plan section `- [ ]` item, or roadmap checkbox.

## ABSOLUTE: Correct Architectural Solutions Only

The triage sub-agent is bound by `.claude/rules/impl-hygiene.md` — SSOT, No Side Logic, canonical homes, phase boundaries, finding categories (LEAK, DRIFT, GAP, etc.). Every fix must respect these principles. Quick fixes, workarounds, counters, flags, and hacks are banned. The correct fix may touch 10 files across 3 crates — that IS the fix.

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
# persist state to {run_id}/state.json for sub-agents to read

while iteration_counter < 10 and thoroughness_reject_counter < 3:
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

    # Read only triage.json (small — a handful of fields)
    triage = read {run_id}/round-{round_n}/triage.json

    if triage.actionable_after_triage == 0 and triage.thoroughness_ok:
        # CLEAN PASS — exit
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
        persist state; continue

# ── EXIT ────────────────────────────────────────────────────
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
