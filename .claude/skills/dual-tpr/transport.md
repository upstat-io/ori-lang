# Dual-TPR Transport — Wrapper Invocation Pattern

This document specifies the wrapper invocation pattern that all four
dual-source review skill wrappers (Sections 04-07) use to launch
both reviewers and parse their output via the shared transport
utility.

## Wrapper invocation structure

Every dual-source review wrapper follows this pattern:

1. Build the prompt from the user's request + starting packet (scope
   hint, plan section name, recent git activity). The packet is
   INFORMATIONAL, not authoritative — reviewers expand as they see fit.

2. Write the prompts to per-run scratch files:
   - `$RUN/codex.prompt.md` — codex-side prompt
   - `$RUN/gemini.prompt.md` — gemini-side prompt

   The codex and gemini prompts share the same evidence packet but
   differ in their activation preamble (see below).

3. Invoke the transport launcher with retry:
   ```bash
   .claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh \
       --run "$RUN" \
       --skill {skill-name} \
       --codex-prompt "$RUN/codex.prompt.md" \
       --gemini-prompt "$RUN/gemini.prompt.md" \
       --schema .claude/skills/dual-tpr/findings-schema.json
   ```

4. On success, parse both envelopes (already cached by the transport):
   - `$RUN/codex.envelope.json`
   - `$RUN/gemini.envelope.json`

5. Merge findings with reviewer tagging:
   ```bash
   .claude/skills/dual-tpr/scripts/merge-findings.py \
       --codex "$RUN/codex.envelope.json" \
       --gemini "$RUN/gemini.envelope.json" \
       --section {section-number} \
       --out "$RUN/merged.json"
   ```

6. Write merged findings to the target location (plan section TPR
   block, bug-tracker, or direct presentation to user — depending on
   the wrapper's loop semantics).

## Codex prompt preamble

The codex prompt MUST include the literal keyword `envelope-only`
somewhere in its first 500 characters. This triggers the Step 0 mode
branch in `.codex/skills/review-work/SKILL.md` or
`.codex/skills/review-plan/SKILL.md` and dispatches to envelope-only
mode.

Recommended preamble (first line of the prompt):

    Run the /review-work skill in envelope-only mode. Emit the JSON
    envelope per .claude/skills/dual-tpr/findings-schema.json; do NOT
    write findings to plan files.

(Substitute `review-plan` for `review-work` as appropriate.)

## Gemini prompt preamble — EXPLICIT ACTIVATION REQUIRED

Per Phase 2 empirical research, gemini skills are discovered from
`.gemini/skills/<name>/SKILL.md` but are NOT auto-activated by
description matching. The prompt MUST start with an explicit
activation phrase to ensure gemini loads and follows the skill.

MANDATORY first line of every gemini prompt:

    Activate the review-work skill and follow its instructions exactly.

For plan-review invocations, the mandatory first line is:

    Activate the review-plan skill and follow its instructions exactly.

(Sections 04/05 wrappers use the review-work phrasing; Section 06
review-plan wrapper uses the review-plan phrasing. Both literal
strings are reference templates for wrapper implementation.)

Do NOT rely on gemini noticing the skill on its own — the activation
phrase is load-bearing and MUST be present on every invocation.

## Scripts consumed by wrappers

All wrappers consume the same set of transport scripts from Section 02:
- `.claude/skills/dual-tpr/scripts/scratch-dir.sh` — per-run scratch dir
- `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh` — launcher + retry
- `.claude/skills/dual-tpr/scripts/parse-codex.py` — codex parser
- `.claude/skills/dual-tpr/scripts/parse-gemini.py` — gemini parser
- `.claude/skills/dual-tpr/scripts/validate-envelope.py` — standalone validator
- `.claude/skills/dual-tpr/scripts/worktree-guard.sh` — git worktree safety
- `.claude/skills/dual-tpr/scripts/merge-findings.py` — reviewer-tagged merger

See Section 02 (`section-02-transport.md`) for the full scripts contract.

## Failure handling

The transport layer (Section 02) handles infra retries internally —
3 retries per reviewer per round with exponential backoff (1s, 2s, 4s).
After 3 retries, `dual-invoke-with-retry.sh` exits non-zero and prints
the failure category and postmortem directory path.

Wrappers should:
- On success: proceed to parse + merge + write
- On failure: surface the failure category and postmortem path to the
  user via AskUserQuestion, including the `$RUN` directory where the
  JSONL streams and error messages are retained for inspection
- NEVER consume a semantic iteration of the wrapper's outer loop on
  infra failure — the 10-iteration loop is for finding-fixing rounds,
  not transport failures

## Wrapper loop semantics

`/tpr-review` and `/review-work` use the 10-iteration find+fix+rerun
loop. Each iteration:
1. Runs the dual-source transport (both reviewers per round, max
   3 infra retries per reviewer)
2. Claude reads the merged findings
3. If zero actionable findings: clean pass, exit loop
4. Otherwise: Claude fixes findings, commits, re-runs (increment
   semantic iteration counter)
5. After 10 iterations: surface remaining findings to user via
   AskUserQuestion

`/review-plan` does NOT loop — it emits proposed edits once per
invocation. The wrapper applies them (or presents them for user
approval) and does not re-invoke.

`/tp-help` does NOT loop and does NOT use the findings schema — it
emits raw concatenated responses from both reviewers (see Section 07
for the tp-help-specific envelope).
