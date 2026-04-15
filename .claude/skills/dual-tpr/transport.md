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
somewhere in its first 500 characters. For skill-dispatch modes
(`review-work`, `review-plan`), this triggers the Step 0 mode
branch in `.codex/skills/review-work/SKILL.md` or
`.codex/skills/review-plan/SKILL.md` and dispatches to envelope-only
mode. For custom objective mode, it signals the output contract
(raw JSON final message) even though no skill is dispatched.

Recommended preamble for skill-dispatch modes:

    Run the /review-work skill in envelope-only mode. Emit the JSON
    envelope per .claude/skills/dual-tpr/findings-schema.json; do NOT
    write findings to plan files.

(Substitute `review-plan` for `review-work` as appropriate.)

Recommended preamble for custom objective mode:

    You are performing a third-party review in envelope-only mode.
    Do NOT activate any skill. Follow these instructions directly.

## Gemini prompt preamble — EXPLICIT ACTIVATION REQUIRED

Per Phase 2 empirical research, gemini skills are discovered from
`.gemini/skills/<name>/SKILL.md` but are NOT auto-activated by
description matching. For skill-dispatch modes, the prompt MUST start
with an explicit activation phrase to ensure gemini loads and follows
the skill.

MANDATORY first line for skill-dispatch modes:

    Activate the review-work skill and follow its instructions exactly.

For plan-review invocations, the mandatory first line is:

    Activate the review-plan skill and follow its instructions exactly.

(Sections 04/05 wrappers use the review-work phrasing; Section 06
review-plan wrapper uses the review-plan phrasing. Both literal
strings are reference templates for wrapper implementation.)

Do NOT rely on gemini noticing the skill on its own — the activation
phrase is load-bearing and MUST be present on every skill-dispatch
invocation.

For custom objective mode, the gemini prompt does NOT activate a skill.
Instead it gives the objective directly with inline envelope instructions
including the mandatory sentinel markers. See `tpr-review/SKILL.md`
§"Prompt templates for custom mode" for the canonical template.

## Custom Objective Mode

Custom objective mode is used when `/tpr-review` is invoked with freeform
ARGS (not `--skill review-plan` and not empty). In this mode:

- Neither reviewer activates a fixed skill — the objective is given inline
- Both reviewers still receive the grounding block (CLAUDE.md, rules files)
- Both reviewers still emit envelopes (the schema is mode-independent)
- The `--skill` parameter to the transport is `custom` for logging
- The loop semantics are identical to code/plan modes — fix findings,
  re-run until both reviewers return zero actionable findings (consensus)

This enables `/tpr-review` to review ANYTHING — skills, docs, designs,
tooling, processes — not just code or plans.

## Mandatory Grounding Block (both reviewers)

**Every reviewer prompt — codex and gemini — MUST contain a grounding
section between the activation preamble and the scope hint.** The
grounding block is identical for both reviewers.

### Dynamic Rules Brief (preferred)

The grounding block is now dynamically composed via a two-step process:

1. **Classify** — `scripts/rules-for-review.py` maps changed files to
   subsystems and resolves which rule files are relevant.
2. **Compose** — a Sonnet subagent reads the classified rule files and
   the diff, then composes a ~200-400 line **Rules Brief** containing
   only the specific rules, invariants, and finding vocabulary relevant
   to this review.

The Rules Brief is injected INLINE into both prompts under a
`## Rules — these apply to this review` header. This replaces the old
static "read these files in full" file list. Reviewers consume the
inline content as part of the prompt — no need to go read separate files.

After the inline brief, a "For full rule details, also read:" section
lists the critical file paths from the classifier for optional deep dives.

See `.claude/skills/dual-tpr/compose-rules-brief.md` for the Sonnet
subagent prompt template. See `/tpr-review` SKILL.md Step 1.5 for
integration into the review loop.

### Static Fallback

If the classifier or Sonnet agent fails, fall back to the static core:

    ## Grounding — read these files FIRST before reviewing

    1. CLAUDE.md (project root)
    2. .claude/rules/impl-hygiene.md
    3. .claude/rules/tests.md
    4. .claude/rules/compiler.md

**Why grounding is load-bearing:** Without it, reviewers produce
findings against unknown conventions — generic "this looks odd"
noise instead of precise category-tagged findings that match the
project's actual rules. Grounded reviewers emit findings like
`LEAK:scattered-knowledge at dual-invoke-with-retry.sh:99`; ungrounded
reviewers emit findings like "this function could be clearer".

Wrappers that skip grounding entirely should be treated as buggy
and their envelopes treated with extra scrutiny by the consuming
Claude instance.

## Finding Verification Contract (Claude-side)

**Reviewer findings are hypotheses, not facts.** When the wrapper's
consuming Claude instance receives merged findings from the
transport, it MUST independently verify EVERY actionable finding
against the actual code before acting on it — regardless of which
reviewer produced it.

**Trust tiers (set verification depth, not pass/fail):**

- **Codex: HIGH trust.** Citations and line numbers tend to match
  reality. Spot-check each finding: read the cited lines, confirm
  the specific claim, move on if it holds.
- **Gemini: LOWER trust.** More prone to confabulation — invented
  line numbers, misquoted code, reframed-as-finding positive
  observations. Every gemini finding needs FULL verification: read
  the cited file in full, trace the code path end-to-end, confirm
  the claim against what the code actually does.

Both reviewers can be wrong. Agreement amplifies the hypothesis but
does not substitute for verification. The verification step is
codified in `.claude/skills/tpr-review/SKILL.md` §5 "Classify merged
findings (and VERIFY each one independently)" — all consuming
wrappers of this transport must implement an equivalent step.

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

`/tpr-review` (all three modes: review-work, review-plan, custom) and
`/review-work` use the 10-iteration find+fix+rerun loop. Each iteration:
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
