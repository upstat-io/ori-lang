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

The orchestrator does NOT pre-summarize rules. Codex and gemini have
full filesystem access and are capable of reading the rule files
themselves — pre-composing a brief burns orchestrator context, risks
staleness against the canonical files, and duplicates work the
reviewers can do in parallel.

Both prompts MUST contain this block verbatim (or with additional
rule files appended when the diff touches a specialized subsystem):

    ## Grounding — read these files FIRST before reviewing

    Before you review anything, read these rule files in full. This
    grounding is MANDATORY — a review written without reading the
    rules produces generic noise instead of project-native findings.

    1. CLAUDE.md (project root) — correctness, no deferral, phase purity
    2. .claude/rules/impl-hygiene.md — finding vocabulary (LEAK, DRIFT,
       GAP, WASTE, EXPOSURE, BLOAT, NOTE), SSOT, algorithmic DRY
    3. .claude/rules/tests.md — matrix testing, semantic/negative pins
    4. .claude/rules/compiler.md — architecture, phase boundaries
    5. Any other `.claude/rules/*.md` file relevant to the changed
       paths (e.g. `arc.md` for ARC/memory, `parse.md` for parser,
       `registry.md` for type-system, `codegen-rules.md` for codegen).
       Identify these from the diff yourself — do not rely on the
       orchestrator to pre-filter.

    Every finding MUST use the vocabulary from `impl-hygiene.md`
    (LEAK / DRIFT / GAP / WASTE / EXPOSURE / BLOAT / NOTE) and cite
    the specific rule anchor (TR-2, NR-1, RL-2, etc.) it violates.
    Generic "this looks odd" feedback is not useful.

Skills invoking the transport may append subsystem-specific rule
files to item 5 when the changed paths clearly point at one
subsystem — but the orchestrator must NOT read, summarize, or
synthesize those files. The reviewers do that work.

**Why grounding is load-bearing:** Without it, reviewers produce
findings against unknown conventions — generic "this looks odd"
noise instead of precise category-tagged findings that match the
project's actual rules. Grounded reviewers emit findings like
`LEAK:scattered-knowledge at dual-invoke-with-retry.sh:99`; ungrounded
reviewers emit findings like "this function could be clearer".

Wrappers that skip grounding entirely should be treated as buggy
and their envelopes treated with extra scrutiny by the consuming
Claude instance.

## Reviewer Circuit Breaker (global, cross-session)

`dual-invoke-with-retry.sh` consults a per-reviewer circuit breaker BEFORE
each round. State lives under `$HOME/.cache/ori-tpr-circuit/` — global per
user, shared across Claude sessions, skills, and worktrees (the failing
resource is the API, not the workspace).

**Trip condition:** 3 api/transport failures for the same reviewer inside a
sliding 1-hour window → reviewer parked for 1 hour.

**Counted categories** (via `circuit-breaker.sh fail`):

- `<reviewer>_api_capacity`, `<reviewer>_api_auth`, `<reviewer>_api_error`
- `<reviewer>_missing_jsonl` (subprocess crashed before any output)
- `reviewer_stalled_<r>` (watchdog killed a hung reviewer)
- `launch_or_exit_fail` (legacy catch-all; attributed to both active reviewers)

**NOT counted** (semantic/content failures — the reviewer's output is the
problem, not infra): parse_fail, schema_violation, missing_envelope,
missing_*_sentinel, failed_partial, missing_dependency, dirty_worktree.

**Behavior when tripped:**

- `ORI_TPR_REVIEWERS=both` — the retry wrapper narrows to the surviving
  reviewer and continues normally. The merger and consumer skills see
  single-reviewer output, which they already handle.
- `ORI_TPR_REVIEWERS=codex` or `=gemini` (explicit single-reviewer) — the
  wrapper fails loud with a reset instruction; there is no fallback the
  operator can have forbidden without meaning to.
- Both tripped — the wrapper fails loud; operator waits or runs
  `circuit-breaker.sh reset all`.

**Reset behavior:**

- On a successful round, the fails counter is cleared for each reviewer
  that produced a valid envelope (but an active timeout still runs its
  full 1 hour — a tripped reviewer cannot have reset itself because it
  was skipped).
- On natural timeout expiry, `check` clears both the timeout sentinel
  and the fails counter so a fresh window starts.
- Manual override: `circuit-breaker.sh reset {codex|gemini|all}`.

**Tuning env (all optional):**

- `ORI_TPR_CIRCUIT_OFF=1` — disable the breaker entirely (diagnostics)
- `ORI_TPR_CIRCUIT_THRESHOLD` — fails that trip the breaker (default 3)
- `ORI_TPR_CIRCUIT_WINDOW_SEC` — sliding-window length (default 3600)
- `ORI_TPR_CIRCUIT_TIMEOUT_SEC` — timeout duration (default 3600)
- `ORI_TPR_CIRCUIT_DIR` — state directory (default `$HOME/.cache/ori-tpr-circuit`)

**Observability:** `.claude/skills/dual-tpr/scripts/circuit-breaker.sh status`
prints a two-line summary suitable for the skill-layer polling protocol.

**Why it exists:** repeated API capacity / stall failures waste both wall
time (each round is 5-20 minutes) and quota (both reviewers' retries burn
provider quota). When one provider is clearly degraded, the pipeline is
better off running single-reviewer than hammering both in lockstep.

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
5 attempts per reviewer per round with default backoff
(1s, 2s, 4s, 30s, 60s) and a capacity-aware schedule
(30s, 60s, 120s, 120s, 120s) when the API reports capacity errors.
After the attempts are exhausted, `dual-invoke-with-retry.sh` exits
non-zero and prints the failure category and postmortem directory path.
See `.claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh` for the
SSOT schedule.

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
   5 infra attempts per reviewer)
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
