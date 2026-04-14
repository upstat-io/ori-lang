---
name: tpr-review
description: "Run an independent dual-source (codex + gemini) third-party review in parallel, then fix findings and re-run until BOTH reviewers come back clean (full consensus). Reviews ANYTHING — code, plans, skills, docs, designs, tooling, processes, or any custom objective. TRIGGER proactively after completing ANY non-trivial work, OR when you want iterative improvement driven by multi-agent consensus. When in doubt, run it. The cost of an unnecessary review is near zero; the cost of a missed bug is high."
---

# Dual-Source TPR Review (Codex + Gemini)

Run BOTH the Codex CLI AND the Gemini CLI non-interactively in parallel to perform independent review passes, merge their findings with reviewer tagging, then fix any findings and re-run until BOTH reviewers return zero actionable findings (full consensus). Codex and Gemini each have their own context, rules, and skills — they figure out scope on their own.

**This is a GENERAL-PURPOSE third-party review.** The name is "TPR" — Third-Party Review — not "Third-Party Code Review." It reviews ANYTHING: code, plans, skills, docs, designs, tooling, processes, or any custom objective the user provides. The loop runs until full consensus across all agents.

**Three reviewer modes** (selected via `ARGS`):
- **Default (`review-work`)**: no ARGS, or explicit `--skill review-work` — reviewers use their `review-work` skill for code-oriented review
- **Plan review (`--skill review-plan`)**: reviewers use their `review-plan` skill — plan-oriented review (mission criteria, cross-section coherence, executability). Invoked by `/review-plan`.
- **Custom objective (any other ARGS)**: ARGS text becomes the reviewer's objective directly — general-purpose review of anything. Reviewers do NOT activate a fixed skill; they receive the objective inline with envelope-emission instructions. Use this for iterative improvement, skill refinement, doc quality, design review, or any non-code/non-plan review task. The loop iterates until BOTH reviewers report zero issues (full consensus).

This wrapper is built on the Section 02 dual-source transport utility. All launching, parsing, schema validation, worktree-guarding, and infra retry logic lives in `.claude/skills/dual-tpr/scripts/` — this skill is purely the **semantic** fix-and-re-run loop that consumes merged findings. See `.claude/skills/dual-tpr/transport.md` for the transport contract.

## Step 0 — MANDATORY: Re-read CLAUDE.md

**Before doing ANYTHING else, re-read the entire project CLAUDE.md.** This is non-negotiable. Even if you believe it is in memory, you MUST physically read it with the Read tool. Context compression may have dropped critical rules. Do this every single time this skill runs.

```
Read CLAUDE.md (the project root one)
```

## Step 0.5 — MANDATORY (code/plan modes only): Spec/Grammar Proposal Gate Audit

**Skip this step entirely in custom objective mode** — custom objectives are not scoped to git diffs. Proceed directly to Step 1.

**Before launching reviewers, check whether the diff touches spec or grammar files.** Run:

```
Bash:
  git diff --cached --name-only HEAD~5 2>/dev/null | grep -E '^docs/ori_lang/v2026/spec/' || true
```

(Adjust `HEAD~5` to match the review scope — the point is to catch ALL spec/grammar files in the diff.)

**If ANY files match** (any file under `docs/ori_lang/v2026/spec/`, including `grammar.ebnf` and `operator-rules.md`):

1. Check the git log for those files — does the commit message reference `Proposal:` with an approved proposal filename?
2. Check `docs/ori_lang/proposals/approved/` — does an approved proposal exist that covers this spec change?

**If NO approved proposal exists for a spec/grammar change, this is a CRITICAL finding.** Do NOT launch reviewers. Instead:

- File it immediately as a **CRITICAL** finding:
  ```
  `[SPEC-GATE-CRITICAL]` Spec/grammar file modified without approved proposal.
  Files: <list of spec files in diff>
  Required: Run /create-draft-proposal → /review-draft-proposal BEFORE modifying spec.
  ```
- Surface to the user via `AskUserQuestion`: "Spec/grammar files were modified without an approved proposal. This violates the proposal gate. Should I revert the spec changes and start the proposal workflow, or do you have an approved proposal to reference?"
- Do NOT proceed with the TPR review loop until the proposal gate is satisfied.

**If an approved proposal exists**, note the proposal filename in the reviewer prompts' evidence packet so reviewers can cross-check the spec changes against the approved proposal.

## Step 0.75 — CONDITIONAL: Intelligence Pre-Query

Query the intelligence graph for context relevant to the review. **In custom objective mode**, use the intelligence graph if the objective involves any Ori code, skills, or compiler artifacts — the symbol index is the fastest way to resolve references.

Follow the canonical intel-summary injection protocol:

@.claude/skills/dual-tpr/compose-intel-summary.md

**Placement for dual-source review prompts**: In Step 2 (Write both reviewer prompts), write the summary directly into BOTH `codex.prompt.md` and `gemini.prompt.md`, after the `## Scope:` header. Do NOT use shell variable interpolation — the prompts use single-quoted heredocs (`<<'PROMPT'`) which suppress expansion. Instead, assemble the prompt content programmatically (e.g., using the Write tool or a double-quoted heredoc for the section that includes the summary). Reviewers should use the intelligence summary as a pointer to investigate, not as authoritative evidence.

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

**The size of the fix is irrelevant.** If the correct fix requires cross-crate refactoring across 10 files, that IS the work. "Requires architectural change" is not a reason to skip — it IS the work.

**"Future improvement" requires a concrete artifact.** If you ever say something will be tracked, you MUST in the same response create: a bug-tracker entry (`/add-bug`), plan section `- [ ]` item, or roadmap checkbox. Ask yourself: "When would this get done? Who would find it?" If nobody/never, fix it now.

## ABSOLUTE: Correct Architectural Solutions Only

**Before fixing ANY finding, read `.claude/rules/impl-hygiene.md`.** This is non-negotiable. The hygiene rules define SSOT (Single Source of Truth), No Side Logic, canonical homes, phase boundaries, and finding categories (LEAK, DRIFT, GAP, etc.). Every fix must respect these principles.

**Fixes must be the correct, proper architectural solution — never quick fixes, workarounds, counters, flags, or hacks.** Specifically:

- **SSOT**: if the finding reveals scattered knowledge or duplicated dispatch, the fix is to establish/use the canonical home — not to patch each copy
- **No Side Logic**: if logic lives outside its canonical home, the fix is to move it — not to add another copy that "works"
- **Canonical Homes**: every behavioral decision has exactly ONE file that defines it. If a fix would create a second source of truth, it is wrong
- **Phase Boundaries**: fixes must not bleed phase responsibilities. If fixing a codegen bug requires adding type-checking logic to the codegen pass, that's the wrong fix — the type checker should provide the information
- **Registry as Source of Truth**: builtin type behavior (methods, operators, memory) lives in `ori_registry`. Fixes that hardcode type behavior outside the registry are LEAKs
- **Enforcement**: when a fix adds a new variant, sync point, or dispatch arm, it MUST have enforcement (exhaustive match, exhaustiveness test, or registry-driven generation) to prevent future drift

**The "quick fix" test**: if your fix would not survive a code review by someone who has read `impl-hygiene.md`, it's wrong. The correct fix may touch 10 files across 3 crates — that IS the fix. A workaround that passes tests is not a fix.

## When to Trigger — Bias Toward Running

**Run this skill after completing ANY of the following:**
- Bug fixes (any severity)
- New features or feature extensions
- Refactors or code reorganization
- Multi-file changes (2+ files)
- Any change to compiler crates (`ori_types`, `ori_eval`, `ori_llvm`, `ori_arc`, `ori_parse`, `ori_lexer`, `ori_rt`)
- Any change to codegen, type checking, or evaluation
- Any change to the ARC/AIMS pipeline
- Test matrix additions or test infrastructure changes
- Plan section implementations
- Stdlib or registry changes
- Changes to error handling or diagnostics

**Also run when (code/plan mode):**
- You're unsure whether the change warrants review (default: run it)
- The work involved multiple steps or non-obvious decisions
- The change touches code paths shared across subsystems
- You fixed something that was interfering with other code

**Run with a custom objective when:**
- The user wants iterative improvement of any artifact (skills, docs, tooling, config, designs)
- The user wants multi-agent consensus on quality ("make this perfect")
- The subject is not code or a plan — e.g., reviewing a skill file, a diagnostic script, a doc, a process
- The user explicitly provides an objective that doesn't fit code-review or plan-review scope

**The only time NOT to run:** purely cosmetic single-line changes (typo fixes, comment edits, formatting-only).

## Loop Protocol — MANDATORY

```
+---------------------------------------------------------+
|              DUAL-SOURCE TPR REVIEW LOOP                |
|                                                         |
|  0. CLAUDE re-reads CLAUDE.md (MANDATORY)               |
|        |                                                |
|  0.5 CLAUDE checks spec/grammar proposal gate           |
|     (BLOCKS if spec files changed without proposal)     |
|        |                                                |
|  0.75 CLAUDE queries intelligence graph (CONDITIONAL)   |
|     (skip silently if unavailable or no results)        |
|        |                                                |
|  1. TRANSPORT launches BOTH reviewers in parallel       |
|     Infra retries (3 per reviewer) inside transport —   |
|     they do NOT consume semantic iterations.            |
|        |                                                |
|  2. CLAUDE merges findings via merge-findings.py        |
|        |                                                |
|  3. CLAUDE judges thoroughness (ALWAYS, every round)    |
|     inputs: status-check.sh asymmetry (walltime,        |
|             events, bytes) + scope_actually_reviewed    |
|             (files_read, rules_consulted) per envelope  |
|     output: thoroughness_ok = True | False              |
|        |                                                |
|        +-- findings count × thoroughness_ok -----+      |
|        |                                          |     |
|        v                                          v     |
|  4.  ZERO findings?                    FINDINGS > 0?    |
|        |                                          |     |
|     +--+------+                        +----------+     |
|     |         |                        |          |     |
|    thorough  thin                    thorough    thin   |
|     |         |                        |          |     |
|     v         v                        v          v     |
|  DONE      Re-run,                FILE/FIX/    FILE/FIX/ |
|  (clean)   nothing                COMMIT       COMMIT   |
|            to save.               clear flag.  KEEP     |
|            counter++,             counter=0,   flag!    |
|            flag=True,             iter++,      counter=0|
|            GOTO 1                 GOTO 1       iter++   |
|                                                 GOTO 1  |
|                                                         |
|  CRITICAL: findings are NEVER discarded on a thin       |
|  review. The flag persists to the NEXT round's prompts, |
|  but the captured findings are always filed and fixed.  |
|                                                         |
+---------------------------------------------------------+
```

**Three actors:**
- **Codex** (external reviewer #1): runs `.codex/skills/{skill_name}/SKILL.md` in envelope-only mode. Does NOT fix anything. Default: `review-work`; plan review: `review-plan`.
- **Gemini** (external reviewer #2): runs `.gemini/skills/{skill_name}/SKILL.md`. Does NOT fix anything. Can issue `google_web_search` for external claim verification. Default: `review-work`; plan review: `review-plan`.
- **Claude** (you): reads merged findings, fixes the code/plan, commits, re-invokes the transport.

**A round succeeds only when BOTH reviewers complete cleanly AND the merged finding list contains zero actionable findings AND Claude judges reviewer thoroughness to be sufficient.** Filing findings without fixing and re-running is deferral. Fixing findings without re-running BOTH reviewers to confirm clean is incomplete. A partial re-run (only one reviewer) is NOT a valid clean pass. A "no findings" outcome from a thin/skimming review is ALSO not a valid clean pass — see Step 4 (Thoroughness Judgment) below.

**Maximum semantic iterations: 10** (finding-fixing rounds). **Maximum thoroughness-reject iterations: 3** (consecutive). Infra retries inside `dual-invoke-with-retry.sh` do NOT count against either budget — they are for transport failures, not semantic progress. If after 10 finding-fixing rounds findings are still surfacing, surface the remaining merged findings to the user via `AskUserQuestion`. If 3 thoroughness rejections in a row fail to elicit deeper review, escalate separately — prompt discipline alone cannot coax depth that's not there.

### Loop State Machine

The loop protocol above is an illustrative diagram; the state machine below is the authoritative contract. Infra retries are invisible to `iteration_counter` — they happen inside step `dual-invoke-with-retry.sh` and either resolve (round continues) or exhaust (user escalation, counter untouched).

```
iteration_counter = 0                # finding-fixing rounds (cap: 10)
thoroughness_reject_counter = 0      # consecutive WASTED rounds (cap: 3)
strengthened_language_required = False
while iteration_counter < 10 and thoroughness_reject_counter < 3:
    RUN = scratch-dir.sh
    write codex.prompt.md and gemini.prompt.md into RUN
    if strengthened_language_required:
        # previous round had thin depth — prepend the Thoroughness
        # Re-review Directive (see §6.5) to BOTH prompts before the
        # scope hint. Note: thin depth can come from either a
        # zero-findings thin round OR a findings-present thin round;
        # both carry the flag forward.
        prepend Thoroughness Re-review Directive to both prompts
    if dual-invoke-with-retry.sh fails:
        # infra retries (3 per reviewer, 1s/2s/4s backoff) already
        # exhausted inside the transport — do NOT increment, do NOT
        # retry the semantic loop
        surface failure category + $RUN to user via AskUserQuestion
        EXIT

    # both envelopes passed parser + schema + worktree-guard
    merged = merge-findings.py(codex.envelope.json, gemini.envelope.json)

    # Thoroughness judgment — ALWAYS, regardless of finding count.
    # Returns a boolean indicating whether the reviewers actually did
    # the work (deep investigation per the command-file methodology).
    # Claude inspects:
    #   1. walltime / event / byte ratios from status-check.sh
    #   2. scope_actually_reviewed.files_read on each envelope
    #   3. scope_actually_reviewed.rules_consulted on each envelope
    #   4. verification.tests_rerun / diagnostics_run on each envelope
    # If the faster reviewer's envelope has thin files_read, empty
    # rules_consulted, and status-check.sh shows ASYMMETRY: HIGH (or
    # MODERATE with supporting evidence of skimming), thoroughness_ok
    # is False. See §6 for the full rubric.
    run status-check.sh "$RUN" for the final asymmetry snapshot
    thoroughness_ok = CLAUDE judges thoroughness_sufficient(
        codex_env, gemini_env, status_check
    )

    if merged.summary.actionable == 0:
        # ── Zero-findings branch ────────────────────────────────
        if thoroughness_ok:
            CLEAN PASS — exit with iteration_counter for the report
            # (informational findings are non-actionable; they don't block clean pass)
        else:
            # Zero findings + thin = nothing captured, pure waste.
            # Mandatory re-review with strengthened language.
            thoroughness_reject_counter += 1
            strengthened_language_required = True
            # iteration_counter NOT incremented — nothing was fixed,
            # so this round does not consume the finding-fixing budget
            continue
    else:
        # ── Actionable-findings branch ──────────────────────────
        # CRITICAL: findings are ALWAYS filed and fixed, even on a
        # thin review. They represent real captured work — discarding
        # them because "the review was thin anyway" wastes the only
        # output that round produced. The thoroughness flag affects
        # the NEXT round's prompting, NEVER whether to keep THIS
        # round's findings.
        for each actionable finding in merged (severity != "informational"):
            file into owning plan TPR block or bug-tracker
            fix the code
            run `timeout 150 ./test-all.sh`
        commit via /commit-push
        iteration_counter += 1

        # Findings were found — this round was NOT wasted. Reset the
        # consecutive-wasted-rounds counter regardless of depth.
        thoroughness_reject_counter = 0

        # The strengthened-language flag tracks the DEPTH of this
        # round, not progress. If the review was thin, the re-review
        # of the fixed code must STILL demand deeper investigation,
        # because the reviewers may have caught one bug and missed
        # several more. Only a thorough round clears the flag.
        strengthened_language_required = not thoroughness_ok

# Exit reasons — escalate to user via AskUserQuestion:
if thoroughness_reject_counter >= 3:
    surface "3 consecutive wasted rounds (zero findings + thin review) —
             prompt discipline could not elicit depth. Last merged
             envelopes and status-check.sh output are at $RUN." to user
elif iteration_counter >= 10:
    surface remaining merged findings to user
```

**Invariants:**
- `iteration_counter` increments ONLY after a successful round that found actionable findings AND those findings were fixed AND the commit landed. Any earlier exit (infra failure, clean pass, wasted thoroughness-reject round) skips the increment.
- `thoroughness_reject_counter` increments ONLY on the **zero-findings + thin-review** cell — the sole "pure waste" case where the round produced nothing and depth was inadequate. It RESETS to zero on any round that produces actionable findings (findings = progress regardless of depth — findings-present rounds are never wasted, even if thin).
- `strengthened_language_required` tracks the **depth of the last round**, independent of finding count. It is set True after any thin round (with OR without findings) and cleared only after a thorough round. A findings-present thin round KEEPS the flag True so the re-review of the fixed code still receives the directive; only a thorough round clears it.
- **Findings are NEVER discarded on a thin review.** The fix path (file → fix → test → commit) runs unconditionally when `merged.summary.actionable > 0`. Discarding real findings because "the reviewer was thin anyway" would waste the only output that round produced and punish the reviewers for having caught anything at all. The thin signal propagates via the flag, not by throwing away data.
- Infra retries (inside the transport), finding-fixing iterations, and thoroughness-reject iterations are **three orthogonal budgets**. None can consume another. A transient network hiccup burning 3 infra retries leaves both semantic budgets untouched.
- A clean pass on any iteration is a terminal state: the report includes `iteration_counter` AND `thoroughness_reject_counter` peak at exit so "clean on iteration 1" vs "clean on iteration 3 after fixing N findings and 2 thin rounds" are distinguishable in the final output.
- The 10-iteration cap is a **user-facing stopping rule**, not a correctness guarantee. The 3-thoroughness-reject cap is the same — and it only counts "pure waste" rounds (zero findings + thin), not rounds that merely had thin depth. A round that caught even one bug on a thin review is forward progress and resets the counter.
- **Thoroughness judgment is Claude's call, not a static threshold.** `status-check.sh` surfaces ratios and flags them red/yellow/green, but the accept/reject decision belongs to Claude. Large ratios with thin `files_read` = thin. Large ratios with rich `files_read` on both sides (e.g., one reviewer was just faster) = thorough. The script surfaces signals; Claude makes the call.

## Steps (Per Iteration)

### 1. Create a per-run scratch directory

```
Bash:
  RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
  echo "$RUN"
```

**This MUST be a separate foreground Bash call.** The `$RUN` path must be visible in Claude's context before Steps 2-3 run. If `$RUN` is created inside a `run_in_background: true` compound command, Claude never sees the path and will poll the wrong directory — potentially a parallel session's directory from a different project.

Each semantic iteration gets a fresh `$RUN` (e.g. `/tmp/ori-tpr-XXXXXXXX`). Reuse across iterations is forbidden — a stale envelope from the previous round would corrupt the merge.

### 1.5. Compose the Rules Brief (MANDATORY)

Before writing reviewer prompts, dynamically compose a tailored rules brief
so reviewers get focused, relevant rules — not a 3,000-line firehose they
will skim.

**Step A — Classify changed files into subsystems:**

```
Bash:
  python3 scripts/rules-for-review.py --diff {review_range} --mode json
```

(For staged/unstaged reviews, use `--files` with the file list from
`git diff --name-only` instead of `--diff`.)

This outputs JSON with: `changed_files`, `subsystems`, and `rules`
(split into `critical` and `reference` lists, each with `file` and `why`).

**Step B — Spawn a Sonnet subagent to compose the brief:**

```
Agent(model=sonnet):
  description: "Compose rules brief for {subsystems} review"
  prompt: <use the template from .claude/skills/dual-tpr/compose-rules-brief.md,
           filling in {CLASSIFIER_JSON}, {DIFF_SUMMARY}, {REVIEW_OBJECTIVE}>
```

The Sonnet agent reads the classified rule files + CLAUDE.md, identifies the
specific rules/invariants relevant to the diff, and outputs a ~200-400 line
Rules Brief with:
- Finding vocabulary (LEAK/DRIFT/GAP/WASTE/etc.)
- Per-file sections with rule anchors (TR-2, NR-1, RL-2, etc.)
- Only the rules that matter for this specific change

**Step C — Hold the Rules Brief in context.** The output is used in Step 2
to replace the static grounding block in both reviewer prompts. See the
updated prompt templates below.

If the classifier detects no files or the subagent fails, fall back to the
static grounding block listing CLAUDE.md + impl-hygiene.md + tests.md +
compiler.md (the "always" core) plus any rule files you manually identify
as relevant.

### 2. Write both reviewer prompts

The codex and gemini prompts share the same evidence packet but differ in their activation preamble. See `.claude/skills/dual-tpr/transport.md` for the canonical preambles.

**Reviewer mode selection** — determines how the reviewer prompts are constructed. Three modes:

1. **Default (`review-work`)**: no ARGS, or explicit `--skill review-work`. Reviewers activate their `review-work` skill. Use preambles below as-is.
2. **Plan review (`--skill review-plan`)**: ARGS contains `--skill review-plan`. Reviewers activate their `review-plan` skill. Substitute `review-plan` for `review-work` in both preambles — see `transport.md` §Codex/Gemini preamble sections.
3. **Custom objective**: ARGS is non-empty AND does NOT start with `--skill`. The entire ARGS text IS the objective. Reviewers do NOT activate any fixed skill — the objective and envelope instructions are given inline in the prompt. Use the **custom objective prompt templates** below instead of the skill-dispatch templates.

**Mode detection logic** (Claude evaluates this at the start of Step 2):
```
if ARGS is empty or ARGS == "--skill review-work":
    mode = "review-work"
elif ARGS starts with "--skill review-plan":
    mode = "review-plan"
else:
    mode = "custom"
    objective = ARGS  # the raw text IS the objective
```

**For `review-work` and `review-plan` modes:**
- **Codex prompt** MUST include the literal keyword `envelope-only` in its first 500 characters — this dispatches `.codex/skills/review-work/SKILL.md` (or `.codex/skills/review-plan/SKILL.md` for plan review) into envelope-only mode.
- **Gemini prompt** MUST start with the literal activation phrase `Activate the review-work skill and follow its instructions exactly.` (or `Activate the review-plan skill and follow its instructions exactly.` for plan review) — gemini does NOT auto-activate from description matching; the phrase is load-bearing.

**For `custom` mode:**
- **Neither prompt activates a fixed skill.** The objective and envelope-emission instructions are given inline. This is what allows `/tpr-review` to review ANYTHING — not just code or plans.
- **Both prompts still include the grounding block** (CLAUDE.md, rules files) — reviewers need project context regardless of the objective.
- **Both prompts still require envelope output** — the envelope schema is the contract. Findings represent issues/gaps/improvements identified against the objective.
- **The `--skill` parameter to the transport** should be `custom` for logging purposes.

#### Mandatory Grounding Block (Dynamic Rules Brief)

**Every reviewer prompt MUST contain a grounding section before the scope
hint.** Without grounding, reviewers produce generic "this looks odd" noise
instead of precise `LEAK:scattered-knowledge at path:line` findings.

The grounding block is IDENTICAL for both reviewers and consists of:

1. **The Rules Brief** from Step 1.5 — a ~200-400 line, dynamically-composed
   summary of the specific rules relevant to this diff. This is the primary
   grounding content. It includes finding vocabulary, rule anchors, and
   per-subsystem invariants.
2. **A "read for full context" list** — the critical rule file paths from the
   classifier output, so reviewers can deep-dive if needed.
3. `CLAUDE.md` is always listed — reviewers must read it for overarching rules.

The Rules Brief replaces the old static file list. Instead of telling
reviewers "read these 7 files (3,000 lines)" and hoping they do, the brief
**inlines the key rules** so reviewers consume them as part of the prompt.

Write both prompts to the scratch dir. **Use the template matching the active mode:**

#### Prompt templates for `review-work` and `review-plan` modes (skill-dispatch)

```
Bash:
  cat > "$RUN/codex.prompt.md" <<'PROMPT'
  Run the /{skill_name} skill in envelope-only mode. Emit the JSON
  envelope per .claude/skills/dual-tpr/findings-schema.json; do NOT
  write findings to plan files.
  # NOTE: {skill_name} is review-work (default) or review-plan (plan review)

  ## Rules — these apply to this review

  The following rules brief was composed for this specific review.
  These are the rules your findings MUST cite. Use the finding categories
  (LEAK, DRIFT, GAP, WASTE, EXPOSURE, BLOAT, NOTE) and rule anchors
  (TR-2, NR-1, RL-2, etc.) defined below.

  <INSERT THE RULES BRIEF FROM STEP 1.5 HERE — the full output of the
   Sonnet subagent. This is ~200-400 lines of focused, relevant rules.>

  For full rule details, also read:
  - CLAUDE.md (project root — overarching project rules)
  <list the critical rule file paths from the classifier output>

  ## Scope: <scope hint — e.g. "HEAD~5..HEAD", a plan section name, or explicit files>

  <If Step 0.75 produced an Intelligence Summary, insert it here as literal text>

  <evidence packet: what changed, why, what to look for>
  PROMPT

  cat > "$RUN/gemini.prompt.md" <<'PROMPT'
  Activate the {skill_name} skill and follow its instructions exactly.
  Emit the JSON envelope per .claude/skills/dual-tpr/findings-schema.json;
  do NOT write findings to plan files.
  # NOTE: {skill_name} is review-work (default) or review-plan (plan review)

  ## Rules — these apply to this review

  The following rules brief was composed for this specific review.
  These are the rules your findings MUST cite. Use the finding categories
  (LEAK, DRIFT, GAP, WASTE, EXPOSURE, BLOAT, NOTE) and rule anchors
  (TR-2, NR-1, RL-2, etc.) defined below.

  <INSERT THE RULES BRIEF FROM STEP 1.5 HERE — same content as codex prompt>

  For full rule details, also read:
  - CLAUDE.md (project root — overarching project rules)
  <list the critical rule file paths from the classifier output>

  ## Scope: <same scope hint>

  <If Step 0.75 produced an Intelligence Summary, insert it here as literal text>

  <evidence packet: same>
  PROMPT
```

#### Prompt templates for `custom` mode (objective-direct — NO skill dispatch)

In custom mode, the ARGS text IS the objective. The prompts give the objective directly and include inline envelope-emission instructions. Neither prompt activates a reviewer skill — the reviewers operate on the objective alone.

**CRITICAL for codex**: The codex prompt must still include the keyword `envelope-only` in the first 500 characters. Even though no skill is being dispatched, codex's output parser (`parse-codex.py`) expects the final agent message to be raw JSON. The keyword signals this contract.

**CRITICAL for gemini**: The gemini prompt must still instruct sentinel-wrapped envelope output. Without sentinels, `parse-gemini.py` rejects the response.

```
Bash:
  cat > "$RUN/codex.prompt.md" <<'PROMPT'
  You are performing a third-party review in envelope-only mode.
  Do NOT activate any skill. Follow these instructions directly.

  ## Objective

  {objective}

  ## Your task

  Thoroughly assess the objective above. Read ALL relevant files,
  understand the current state, and identify EVERY issue, gap,
  inconsistency, missing element, or improvement needed to make the
  subject of the objective as good as it can be. Be exhaustive and
  specific — vague observations are not findings.

  For each issue found, produce a finding with:
  - severity (critical / high / medium / low / informational)
  - location (file path and line number, or file path if line N/A)
  - title (one-line summary)
  - evidence (what you observed, what's wrong, why it matters)
  - required_plan_update (the specific fix or improvement needed)

  If after thorough investigation you find ZERO issues, that is a
  valid outcome — but emit at least one `informational` finding
  describing what you verified and why the subject is sound.

  ## Rules — these apply to this review

  <INSERT THE RULES BRIEF FROM STEP 1.5 HERE — for custom mode, the
   classifier may use --files with paths extracted from the objective,
   or fall back to the static core if no files are identifiable.>

  For full rule details, also read:
  - CLAUDE.md (project root — overarching project rules)
  <list the critical rule file paths from the classifier output>

  <If Step 0.75 produced an Intelligence Summary, insert it here>

  ## Envelope output

  Your ENTIRE final message must be a single JSON object conforming
  to .claude/skills/dual-tpr/findings-schema.json. No markdown, no
  prose wrapper — just the raw JSON envelope. Read the schema file
  and .claude/skills/dual-tpr/envelope-format.md for field semantics.

  Set "skill" to "custom" in the envelope.
  PROMPT

  cat > "$RUN/gemini.prompt.md" <<'PROMPT'
  You are performing a third-party review. Do NOT activate any skill.
  Follow these instructions directly.

  ## Objective

  {objective}

  ## Your task

  Thoroughly assess the objective above. Read ALL relevant files,
  understand the current state, and identify EVERY issue, gap,
  inconsistency, missing element, or improvement needed to make the
  subject of the objective as good as it can be. Be exhaustive and
  specific — vague observations are not findings.

  For each issue found, produce a finding with:
  - severity (critical / high / medium / low / informational)
  - location (file path and line number, or file path if line N/A)
  - title (one-line summary)
  - evidence (what you observed, what's wrong, why it matters)
  - required_plan_update (the specific fix or improvement needed)

  If after thorough investigation you find ZERO issues, that is a
  valid outcome — but emit at least one `informational` finding
  describing what you verified and why the subject is sound.

  ## Rules — these apply to this review

  <INSERT THE RULES BRIEF FROM STEP 1.5 HERE — same content as codex>

  For full rule details, also read:
  - CLAUDE.md (project root — overarching project rules)
  <list the critical rule file paths from the classifier output>

  <If Step 0.75 produced an Intelligence Summary, insert it here>

  ## Envelope output — MANDATORY SENTINELS

  Your response MUST end with a JSON envelope bracketed by sentinels.
  Without the sentinels, parse-gemini.py rejects your entire response
  and the review is wasted.

  Format:
  (free-form prose about what you investigated and why)

  <!-- BEGIN-ORI-DUAL-TPR-V1 -->
  ```json
  { ...complete envelope per .claude/skills/dual-tpr/findings-schema.json... }
  ```
  <!-- END-ORI-DUAL-TPR-V1 -->

  Read the schema file and .claude/skills/dual-tpr/envelope-format.md
  for field semantics. Set "skill" to "custom" in the envelope.
  PROMPT
```

**Custom mode evidence packet**: Unlike code/plan modes which use git diffs as the evidence packet, custom mode's "evidence" is the objective itself plus any files Claude identifies as relevant. Claude should add a brief context section after the objective in both prompts listing the specific files the reviewers should focus on (e.g., "The primary file under review is `.claude/skills/tpr-review/SKILL.md`. Also relevant: `.claude/skills/dual-tpr/transport.md`"). This helps reviewers scope their investigation without being restrictive.

The evidence packet is INFORMATIONAL, not authoritative — reviewers expand scope as they see fit. The GROUNDING block, in contrast, is AUTHORITATIVE — reviewers that skip it produce noise and their envelopes should be treated with extra scrutiny.

### 3. Invoke the dual-source transport in the background

The transport launches both reviewers in parallel, handles infra retries (3 per reviewer, exponential backoff 1s / 2s / 4s), runs the schema validators, and applies the dirty-worktree guard. A full round typically takes 5-15 minutes — BOTH reviewers running concurrently, so wall time is roughly `max(codex_walltime, gemini_walltime)`, not the sum.

Running the transport in the Bash foreground either hits the 2-minute tool timeout or gets auto-backgrounded with output truncated. Always use `run_in_background: true`. The `.claude/hooks/block-banned-commands.sh` hook explicitly allows backgrounded codex and gemini commands.

The `--skill` parameter controls the transport log label. Default: `review-work`. If `ARGS` contains `--skill review-plan`, use `review-plan`. For custom objective mode, use `custom`.

```
Bash (run_in_background: true):
  .claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh \
    --run "$RUN" \
    --skill {skill_name} \
    --codex-prompt "$RUN/codex.prompt.md" \
    --gemini-prompt "$RUN/gemini.prompt.md" \
    --schema .claude/skills/dual-tpr/findings-schema.json
```

**DO NOT:**
- Run the transport in the Bash foreground.
- Set a `timeout:` parameter on the Bash call.
- Wrap the transport in an Agent subagent — the subagent cannot itself be backgrounded, so it reintroduces the foreground cap.
- Poll `$RUN/*.envelope.json` or `$RUN/merged.json` — those files use atomic-write semantics and reading them mid-stream can see a partial file.
- Add a trailing `echo "transport_exit=$?"` (or any other trailing command). The bash script's overall exit code is the exit code of the LAST executed command — a trailing echo ALWAYS exits 0 and masks the transport's real failure (BUG-08-007). The task notification's reported exit code is the source of truth.
- **Combine `scratch-dir.sh` + prompt writing + transport launch in a single `run_in_background` call.** The `$RUN` path is lost — Claude never sees it and will poll the wrong directory. Steps 1-2 MUST be foreground; only Step 3 (the transport itself) runs in the background. Incident: 2026-04-13 §08 iteration 8 merged oriterm findings because `$RUN` was created inside a background compound command and polling used filesystem discovery (`ls | tail -1`) which picked a parallel session's directory.
- **Use filesystem discovery (`ls -d /tmp/ori-tpr-* | sort | tail -1`) to find `$RUN`.** Multiple sessions (different projects) create `/tmp/ori-tpr-*` directories concurrently. The `$RUN` value from Step 1 is the ONLY reliable identifier. If you lose `$RUN`, the round is invalid — re-create from Step 1.

### Polling Protocol — Canonical SSOT

**Protocol lives in `.claude/skills/dual-tpr/polling-protocol.md` — `@`-included below. Follow it verbatim.**

`/tp-help`, `/tpr-review`, `/review-work`, and any future dual-source consumer share a single canonical polling protocol. It lives in one file and is expanded here via `@`-include so updates propagate automatically. Prior to 2026-04-08, each skill inlined its own copy — they drifted (tpr-review + review-work used identical text, tp-help had slight wording drift) and produced poor real-time visibility (silent 5-min periods from `sleep 300` backgrounded polls, relative "T+N min" timestamps without absolute anchors). Consolidation into `polling-protocol.md` is the SSOT fix per `impl-hygiene.md` §SSOT / §Algorithmic DRY.

@.claude/skills/dual-tpr/polling-protocol.md

**After the protocol above**, move to Step 4 (merge envelopes on success).

### 4. On success: merge both envelopes

When the completion notification arrives AND the transport exited 0, both envelopes passed parser + schema + worktree-guard validation (the transport is responsible for all of those checks). Run the merger:

```
Bash:
  .claude/skills/dual-tpr/scripts/merge-findings.py \
    --codex "$RUN/codex.envelope.json" \
    --gemini "$RUN/gemini.envelope.json" \
    --section "<NN>" \
    --out "$RUN/merged.json"
```

`<NN>` is the owning plan-section number (e.g. `04`), or `XX` if no owning plan exists. Then read `$RUN/merged.json`. Each entry has:

- `id` — reviewer-tagged, e.g. `[TPR-04-001-codex]` / `[TPR-04-002-gemini]`
- `reviewer` — `codex` or `gemini`
- `agreement` — `true` if a matching `(location, title)` exists in the other reviewer's envelope; `false` otherwise
- `agreement_partner_id` — partner tag when `agreement: true`; `null` otherwise
- `finding` — original finding object (severity, location, title, evidence, impact, basis, confidence, optional citations)

The `summary` block reports `codex_findings`, `gemini_findings`, `agreements`, `codex_only`, `gemini_only`.

### 5. Classify merged findings (and VERIFY each one independently)

**Reviewer findings are hypotheses, not facts.** For every actionable finding, Claude MUST independently verify the claim against the actual code BEFORE acting on it — regardless of which reviewer produced it.

#### Verification protocol (mandatory for every finding)

For each merged finding:

1. **Read the cited code** — open the file at the cited line number, read the surrounding context (not just the one line)
2. **Confirm the claim matches reality** — does the code actually say what the finding claims? Does it actually behave the way the finding describes?
3. **Trace the reasoning** — if the finding says "X is unreachable" / "Y is broken" / "Z is missing", prove it by walking the code yourself. Use `scripts/intel-query.sh callers "<symbol>" --repo ori` and `callees` to trace the call chain (faster and more complete than grep), then check the test coverage.
4. **Check the required_plan_update** — does the proposed fix actually address the root cause, or is it a surface patch that would leave the underlying issue?

If verification proves the finding is wrong, mark it `[x]` with a verification note explaining what you checked and what you found — this is the ONLY valid way to reject a finding. Rejecting without verification is banned; accepting without verification is banned.

#### Trust tiers (set verification depth, not pass/fail)

Both reviewers can be wrong. The trust tier sets how deep the verification goes:

- **Codex: HIGH trust.** Codex tends to cite accurate file/line numbers and its claims usually match the code. Spot-check each finding: read the cited lines, confirm the specific claim, move on if it holds.
- **Gemini: LOWER trust.** Gemini is more prone to confabulation — invented line numbers, misquoted code, claims about behavior that don't match reality, and "positive observations" that reframe correct code as findings. Every gemini finding needs FULL verification: read the cited file in full (not just the cited line), trace the code path end-to-end, and confirm against what the code actually does. This is especially important for:
  - Claims about untested code paths (gemini may miss the test that covers it)
  - Claims about architectural issues (gemini may not have read the canonical home)
  - Claims involving specific line numbers (gemini sometimes invents them)
  - Positive confirmations (e.g. "the fix is correctly done") — only useful if actually correct

Never treat gemini's `citations` URLs as authoritative — if gemini cites a spec or external doc, verify the claim independently instead of trusting the URL as truth.

#### Actionability

After verification confirms a finding is real:

- **Actionable finding**: real code issue — bug, hygiene violation, missing test, incorrect behavior, file size limit exceeded, precision regression, dead code path, etc. Must be fixed.
- **Non-actionable observation**: style preference or observation that isn't a defect, precision loss, or dead code. Note it but don't block the loop on it.
- **Informational finding** (`severity: "informational"`): non-actionable by definition. The reviewer had no actionable findings but wanted to note an observation. Treat as non-actionable — do not fix, do not block the loop. The merge summary's `actionable` count already excludes these.

**IMPORTANT: Err on the side of "actionable"** (after verification). The following are ALWAYS actionable:
- Dead code paths (code that can never execute)
- Precision regressions (over-approximation that loses optimization opportunities)
- Missing tests for plumbed-through data
- Name collisions or aliasing that cause incorrect behavior
- Pipeline gaps where data is computed but never consumed

**Agreement is a priority signal, not a filter.** When an entry has `agreement: true`, both reviewers independently flagged the same `(location, title)` — the strongest possible signal, so prioritize these fixes. When an entry is tagged `-codex` or `-gemini` only (`agreement: false`), the finding is STILL real after verification — provenance is not severity. Single-reviewer findings get fixed just like agreement findings.

**Agreement is not a substitute for verification.** Two reviewers can be wrong about the same thing — agreement amplifies the hypothesis but doesn't prove it. Verify the claim against the code even when both reviewers flagged it.

### 6. Thoroughness Judgment (MANDATORY — runs EVERY round, regardless of findings)

**This step is MANDATORY on every loop iteration**, not only zero-finding rounds. Thoroughness is about whether the reviewers actually DID THE WORK — a separate question from whether they found anything. A round can produce findings AND still be thin (the reviewers caught one obvious bug but skimmed the rest), and a round can produce no findings AND still be thorough (the reviewers investigated deeply and confirmed the code is sound). Both dimensions matter independently.

"No findings" conflates two structurally different cases:

1. ✅ **Genuine clean**: reviewers did a thorough investigation and correctly found nothing to fix. Clean pass, exit the loop.
2. ❌ **Shallow skim (zero-findings thin)**: reviewers did a superficial pass and therefore found nothing *because they did not look hard enough*. Nothing was captured — the round is pure waste. Mandatory re-run.

"Findings present" conflates two structurally different cases:

1. ✅ **Thorough review with findings**: reviewers did the work AND surfaced real issues. Fix them, clear the strengthening flag, proceed to the next round normally.
2. ❌ **Thin review with findings**: reviewers caught one or two obvious issues but skimmed the rest. The findings they DID catch are real and must be fixed. But the reviewers probably missed more. KEEP the strengthening flag True so the re-review of the fixed code still demands deeper investigation.

Case ❌ in either table is the failure mode this step exists to catch. If Claude accepts a thin round as "good enough," the /tpr-review ceremony becomes a rubber-stamp. If Claude *discards* findings from a thin round because "the review was thin anyway," the one piece of real work from that round is wasted. Neither is acceptable.

**Thoroughness judgment is Claude's call**, not a static threshold. No set of numeric rules can perfectly distinguish "fast but thorough" from "fast because skimming" — some scopes really are quickly reviewable, and event count is noisy. The tooling (`status-check.sh`) surfaces the signals; Claude reads them and decides.

#### 6a. Gather thoroughness signals

Run `status-check.sh` one more time against the final `$RUN` for the asymmetry block:

```
Bash:
  .claude/skills/dual-tpr/scripts/status-check.sh "$RUN" --events 10
```

The script's "thoroughness comparison:" block renders walltime, event count, and byte count for both reviewers side-by-side with max/min ratios. Each dimension is flagged:

- **`r >= 3.0x`**: red flag — very likely the faster reviewer skipped depth
- **`r >= 2.0x` and `r < 3.0x`**: yellow — worth a spot-check
- **`r < 2.0x`**: comparable depth

And the aggregate verdict:

- **`ASYMMETRY: HIGH`** (2+ dimensions red) — thin-candidate
- **`ASYMMETRY: MODERATE`** (1 red or 2+ yellow) — spot-check before judging
- **`ASYMMETRY: LOW`** (all dimensions < 2.0x) — thorough-candidate

Also read both envelopes directly (both are fully written by this point — completion notification has arrived):

```
Read: $RUN/codex.envelope.json
Read: $RUN/gemini.envelope.json
```

From each envelope, extract:

- `scope_actually_reviewed.files_read` — how many files did the reviewer actually read?
- `scope_actually_reviewed.rules_consulted` — did the reviewer read the grounding rules?
- `scope_actually_reviewed.specs_consulted` — did the reviewer check the spec for affected behavior?
- `scope_actually_reviewed.expanded_beyond_packet` — did the reviewer broaden the scope as the methodology requires?
- `verification.tests_rerun` — did the reviewer run any tests?
- `verification.diagnostics_run` — did the reviewer run any diagnostic scripts?

#### 6b. Make the judgment — outputs `thoroughness_ok`

Using the signals from 6a, evaluate a boolean: `thoroughness_ok`. Evaluate this on EVERY round, regardless of how many findings the round produced. The flag is NOT a terminal decision — it feeds into the finding-count branches in 6c below.

**Judge `thoroughness_ok = False` (thin review) when ANY of these are true:**

- `status-check.sh` shows `ASYMMETRY: HIGH` (2+ red dimensions) AND the faster reviewer's envelope has thin `files_read` (e.g., fewer than a small handful of files on a non-trivial scope) OR empty `rules_consulted`.
- `status-check.sh` shows `ASYMMETRY: MODERATE` AND the faster reviewer's envelope has OTHER symptoms of skimming: empty `rules_consulted`, empty `specs_consulted` on a subsystem the spec governs, or `files_read` clearly shorter than the diff's file list.
- Either envelope has **empty `rules_consulted`** when the grounding block required at least `CLAUDE.md` + `.claude/rules/impl-hygiene.md` + `.claude/rules/tests.md`. Grounding skipped = methodology skipped; this alone justifies `thoroughness_ok = False` even with `ASYMMETRY: LOW`.
- Either envelope's `files_read` is **clearly shorter than the diff's file list** on a non-trivial scope. The methodology requires reading whole changed files, not just diff hunks.
- Either envelope's `verification` block is empty when the scope clearly warranted a `fresh_verification` basis (e.g., a codegen change with no diagnostic run).
- The faster reviewer's event stream (tail of `status-check.sh`) shows almost no `tool_use` / `command_execution` events — i.e., the reviewer "read nothing and ran nothing" before emitting the envelope.

**Judge `thoroughness_ok = True` (thorough review) when ALL of these are true:**

- `status-check.sh` shows `ASYMMETRY: LOW` OR `MODERATE` with thorough `files_read` / `rules_consulted` on both sides.
- Both envelopes have non-empty `rules_consulted` covering at least `CLAUDE.md` + the relevant `.claude/rules/*.md`.
- Both envelopes have `files_read` consistent with the scope (not obviously truncated).
- At least one envelope has a `verification` basis (tests or diagnostics run) when the scope warranted it.

**Judgment calls** between the two extremes — use sense. The goal is to filter out skimming, not to manufacture non-existent problems. A genuinely small scope with a short but correct review should be judged thorough; a broad scope with a quick skim should not.

#### 6c. Apply the judgment — the four-cell decision matrix

Branch on finding count × thoroughness. Every round falls into exactly one of these four cells:

| | `thoroughness_ok = True` | `thoroughness_ok = False` |
|---|---|---|
| **Zero actionable findings** | **CLEAN PASS** — exit the loop. Report per §6d. | **Mandatory re-review** — nothing to preserve. Increment `thoroughness_reject_counter`, set `strengthened_language_required = True`, `continue` loop. See §6e. |
| **Actionable findings exist** | **Fix and re-run (normal)** — proceed to §7. Clear `strengthened_language_required`. Reset `thoroughness_reject_counter` (it's already 0 in this sub-sequence). | **Fix and re-run WITH strengthening** — proceed to §7. Findings are filed and fixed normally (they are NOT discarded). KEEP `strengthened_language_required = True` so the re-review of the fixed code still demands deeper investigation. Reset `thoroughness_reject_counter` (findings = progress, even if depth was thin). See §6f. |

**CRITICAL RULE: findings are NEVER discarded on a thin review.** When a round produces actionable findings AND thoroughness was thin, the fix path in §7 still runs — file, fix, commit. The thin-depth signal propagates to the next round via `strengthened_language_required`, NOT by throwing away findings that were already captured. Discarding real findings because "the review was thin anyway" would waste the only output that round produced and punish the reviewers for having caught anything at all.

#### 6d. CLEAN PASS — zero findings + thorough

Report to the user:
- "Dual-source TPR review passed clean — both reviewers returned zero actionable findings and Claude's thoroughness judgment accepted the round."
- Iteration count (e.g. "clean on iteration 1" or "clean on iteration 3 after fixing N findings and 1 thin round").
- Thoroughness summary: `ASYMMETRY: {LOW|MODERATE}` + one-sentence rationale referencing the envelopes' `files_read` / `rules_consulted` counts.
- Merge summary from the final iteration (`codex_findings`, `gemini_findings`, `agreements`).
- **This is the ONLY clean exit from the loop.**

#### 6e. Mandatory re-review — zero findings + thin (pure waste)

Do NOT treat a thin-no-findings round as a clean pass. Do NOT file "no findings" anywhere (there's nothing to file). This is the one cell where the round is PURE WASTE — no captured work of any kind. Handle it as follows:

1. Increment `thoroughness_reject_counter` (separate from `iteration_counter`; cap = 3). This counter is the "consecutive wasted rounds" safety net — it only grows on this one cell, because this is the only cell where a round produced literally nothing: no findings AND no verified depth.
2. Set `strengthened_language_required = True` so the next round's prompts include the Thoroughness Re-review Directive.
3. Write a brief rejection note for yourself — which signals triggered the reject (walltime ratio, empty `rules_consulted`, etc.) — so your eventual escalation report has specifics to cite.
4. `continue` the loop back to Step 1 — a new `$RUN`, new prompts, new transport invocation. Do NOT increment `iteration_counter`; nothing was fixed.
5. If `thoroughness_reject_counter` reaches 3, escalate via §8b — three consecutive wasted rounds means prompt discipline alone is not coaxing depth and the user needs to intervene.

#### 6f. Fix path with persisted strengthening — findings + thin

Proceed to §7 (file, fix, commit, re-run) EXACTLY as you would for a thorough round — findings are real captured work and MUST NOT be discarded. The only differences from the thorough branch are what happens to the flags AFTER §7 runs:

1. After §7 commits the fixes, KEEP `strengthened_language_required = True` (do NOT clear it). The next round's re-review of the fixed code will still receive the Thoroughness Re-review Directive, because the reviewers were thin on this round and may have missed findings beyond the ones they caught.
2. Reset `thoroughness_reject_counter = 0` (findings = progress, regardless of depth — this counter only tracks "wasted rounds," and this round was NOT wasted).
3. Increment `iteration_counter` normally (this is a finding-fixing round).
4. `continue` the loop.

The net effect: thin-findings rounds make progress on the findings they DID catch, while the next iteration keeps hunting for the findings they missed. The flag acts as a persistent "look harder" signal that only clears when a round finally runs thoroughly — whether or not that round produces additional findings.

**Why the counter and the flag measure different things.** The counter (`thoroughness_reject_counter`) tracks "consecutive wasted rounds" — rounds that produced literally nothing. The flag (`strengthened_language_required`) tracks "depth of the last round" for the NEXT round's prompting. A thin-findings round is NOT wasted (findings got fixed) but IS thin (next round still needs strengthening), so the counter resets but the flag stays True. Conflating these two was a design error fixed in this version of §6.

**Why "wasted" is narrower than "thin".** The escalation cap should fire only when the loop is producing *nothing*, not when it's producing *some* things thinly. A round that caught even one bug on a thin review is forward progress — the counter resets. Only the zero-findings + thin cell (§6e) truly represents "we ran the loop and got nothing," which is the condition the 3-count cap is designed to catch.

### 6.5. Thoroughness Re-review Directive (prepend to BOTH prompts when `strengthened_language_required`)

When the previous round was rejected as insufficient thoroughness, the next round's prompts MUST begin with this directive, prepended BEFORE the normal grounding block. The directive is SYMMETRIC — both reviewers receive the identical text, because rejecting one reviewer asymmetrically would introduce a new bias that breaks the dual-source independence contract. Both get the strengthened rubric; both must meet it.

```
## THOROUGHNESS RE-REVIEW DIRECTIVE — MANDATORY

The previous review round terminated with zero actionable findings, but
the round was REJECTED by the orchestrator for insufficient thoroughness.
One or more of these asymmetry signals crossed the threshold where a
"no findings" outcome cannot be trusted as a genuine clean pass:

- walltime ratio (max/min) between the two reviewers
- event-count ratio (max/min) between the two reviewers
- stream-byte ratio (max/min) between the two reviewers
- thin `scope_actually_reviewed.files_read` or empty
  `scope_actually_reviewed.rules_consulted` in the prior envelope

This re-review is MANDATORY. Both reviewers must now meet the "Deep
Investigation Standard" from `.claude/skills/dual-tpr/command-file.md`
before emitting an envelope. Specifically, on this re-review you MUST:

1. READ EVERY CHANGED FILE IN FULL — not only the diff hunks. The diff
   is a scope selector, not a content filter. `scope_actually_reviewed.
   files_read` MUST list every changed file, not a subset.

2. READ THE NEIGHBORING CODE required to understand invariants and
   boundary contracts. If a function calls into another module, read
   the callee. If a test asserts on a behavior, read the behavior's
   implementation. Add those files to `files_read`.

3. READ THE GROUNDING RULES IN FULL. `scope_actually_reviewed.
   rules_consulted` MUST list at minimum `CLAUDE.md`,
   `.claude/rules/impl-hygiene.md`, `.claude/rules/tests.md`, plus any
   `.claude/rules/*.md` file relevant to the changed code's subsystem.
   Empty `rules_consulted` on this re-review will be rejected AGAIN.

4. TRACE DATA FLOW across at least two layers of call chain beyond the
   diff — function → caller → caller, or function → callee → callee.
   Record the traced files in `files_read`.

5. RUN AT LEAST ONE DIAGNOSTIC OR TEST as a `fresh_verification` basis.
   If there is genuinely nothing runnable for this scope, explain why
   in `verification.verification_gaps`. "Nothing to run" is a weaker
   basis — justify it.

6. POPULATE `scope_actually_reviewed` HONESTLY. The orchestrator
   compares `files_read` / `rules_consulted` / `specs_consulted` /
   `verification` against the wall-time and event-count you spent.
   Thin scope fields on this re-review WILL trigger another rejection.

If after this deeper pass you STILL find zero actionable issues, that is
a valid outcome — but your envelope must reflect real depth. Emit at
least one `informational`-severity entry that describes WHAT you
verified and WHY the changed code is sound, so the orchestrator can
calibrate trust on the no-findings outcome. An informational entry is
not a "finding" in the actionable sense; it is a proof-of-work note
from a deep review that genuinely found nothing to fix.

A superficial pass WILL be rejected again. The previous round's failure
was not a disagreement about findings — it was a depth-of-investigation
failure. This directive does not ask you to manufacture findings; it
asks you to do the investigation at the level where a no-findings
outcome is credible.
```

The directive is inserted BEFORE the normal grounding block (which reminds the reviewer WHAT to read), so the reviewer sees the "why this time is different" framing before the reading list. Do not edit the grounding block itself — keep the two concerns separate.

### 7. If Actionable Findings Exist -> Fix and Re-run

#### 7a. File Findings

For each validated finding, decide where it lives:

1. **Is there an owning plan section?** — check whether an active plan (roadmap or reroute) has a section covering the affected code.
2. **If yes** — record the entry (or both halves of an agreement) in that section's `## {NN}.R Third Party Review Findings` block using the reviewer-tagged IDs from `merge-findings.py` verbatim:
   ```md
   - [ ] `[TPR-04-001-codex][high]` `compiler/foo.rs:123` — Add dec on early-exit branch.
     Evidence: ... Impact: ... Required plan update: ...
     Basis: fresh_verification. Confidence: high.
   - [ ] `[TPR-04-001-gemini][high]` `compiler/foo.rs:123` — Add dec on early-exit branch.
     Evidence: ... Impact: ... Required plan update: ...
     Basis: direct_file_inspection. Confidence: high. Citations: [{url: "...", description: "..."}]
   ```
   Update plan metadata (`third_party_review.status: findings`, `updated: {today}`).

3. **If no owning plan exists** — file as a bug in `plans/bug-tracker/` under the appropriate subsystem section using the **canonical `BUG-{section}-{ordinal}` format — no reviewer suffix**. Reviewer provenance lives in the body, not the ID. This is the SSOT contract enforced by `.claude/skills/add-bug/SKILL.md:75`, `plans/bug-tracker/00-overview.md:41`, `.claude/commands/review-work.md:108`, and consumed by `/fix-bug BUG-XX-NNN`, `/review-bugs`, and `fix-BUG-XX-NNN.md` filenames. Suffixed IDs would create a shadow bug-ID home that breaks all of those downstream consumers.

   **For an agreement finding** (both reviewers flagged the same `(location, title)`), file ONE BUG entry covering both reviewers' observations — the agreement doesn't require two bug entries:
   ```md
   - [ ] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}** — found by tpr-review (dual-source).
     Repro: {evidence summary from both reviewers}
     Subsystem: {crate/file path}
     Found: {YYYY-MM-DD} | Source: tpr-review | Reviewers: codex + gemini (agreement)
     Fix: `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md` (via `/fix-bug`)
   ```

   **For a single-reviewer finding** (only one reviewer flagged it — `agreement: false`), file ONE BUG entry and note which reviewer surfaced it:
   ```md
   - [ ] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}** — found by tpr-review.
     Repro: {evidence from the single reviewer}
     Subsystem: {crate/file path}
     Found: {YYYY-MM-DD} | Source: tpr-review | Reviewer: codex
     Fix: `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md` (via `/fix-bug`)
   ```

   Each BUG entry gets ONE ordinal regardless of how many reviewers found it — the ordinal space belongs to the subsystem section, not the reviewers. This preserves the canonical `BUG-XX-NNN` ID shape that all downstream tooling expects.

Subsystem mapping (unchanged from single-source version):
- `ori_parse`/`ori_lexer` -> section-01
- `ori_types` -> section-02
- `ori_eval`/`ori_patterns` -> section-03
- `ori_llvm`/`ori_arc` -> section-04
- `ori_rt` -> section-05
- `library/std`/`ori_registry` -> section-06
- `oric`/`ori_fmt`/`ori_diagnostic` -> section-07
- `docs/`/`.claude/`/`plans/` -> section-08

#### 7b. Fix Each Finding — branch by destination

**YOU (Claude) fix the code.** Actual implementation — not just filing, not scope notes, not rationalizations. CODE CHANGES. **The fix path differs based on where the finding was filed in Step 7a** — plan-owned findings are fixed inline; bug-tracker findings hand off to `/fix-bug`. Do NOT conflate the two paths — bug-tracker findings that skip the `/fix-bug` handoff bypass the mandatory TDD matrix, TPR review, and hygiene review per `.claude/skills/fix-bug/SKILL.md` and `CLAUDE.md` §"Bug fix rigor with `/fix-bug`".

##### 7b-i. Plan-owned findings (filed in `## {NN}.R Third Party Review Findings`)

Fix inline with the same rigor as the owning plan section:

- **Read `.claude/rules/impl-hygiene.md` before fixing** — SSOT, canonical homes, no side logic, phase boundaries. Every fix must be the correct architectural solution.
- Read the affected code and understand the issue
- Identify the **canonical home** for the knowledge/logic involved — the fix must respect it
- Follow TDD when appropriate (failing test -> fix -> test passes)
- Run `timeout 150 ./test-all.sh` after fixes
- **Self-check**: would this fix survive `/impl-hygiene-review`? If it introduces scattered knowledge, duplicated dispatch, or a shadow source of truth, it's wrong — find the proper architectural fix
- Mark the filed TPR finding as `[x]` resolved in the plan with a note referencing the code fix:
  ```md
  - [x] `[TPR-04-001-codex][high]` ...
    Resolved: Fixed on YYYY-MM-DD. [description of CODE fix].
  - [x] `[TPR-04-001-gemini][high]` ...
    Resolved: Fixed on YYYY-MM-DD. Same fix as [TPR-04-001-codex] (agreement).
  ```

##### 7b-ii. Bug-tracker findings (filed in `plans/bug-tracker/section-NN-*.md`)

**DO NOT fix inline. Hand off to `/fix-bug BUG-{section}-{ordinal}` for each bug.**

The `/fix-bug` skill creates a fix-section file (`plans/bug-tracker/fix-BUG-{section}-{ordinal}.md`) with full plan-section rigor: investigation, root cause analysis, TDD matrix (semantic + negative pins), implementation, and a completion checklist that includes `test-all.sh`, `/tpr-review`, and `/impl-hygiene-review`. This rigor is non-negotiable per `CLAUDE.md` §"Bug fix rigor with `/fix-bug`": "No ad-hoc bug fixes — every bug gets a fix section, even 'obvious' ones."

For each bug-tracker entry filed in Step 7a:
1. Invoke the Skill tool: `Skill: fix-bug BUG-{section}-{ordinal}`
2. Wait for `/fix-bug` to complete its workflow (which includes its own commit via `/commit-push` AND updates the bug-tracker entry to `[x]` resolved per `.claude/skills/fix-bug/SKILL.md:169` "Update the bug entry")
3. **Verify — do not re-edit.** After `/fix-bug` returns, check that the bug-tracker entry is already `[x]` and uses the canonical `Resolved: Fixed on YYYY-MM-DD` + `Fix: plans/bug-tracker/fix-BUG-XX-NNN.md` form from `plans/bug-tracker/00-overview.md:52`. If the entry is correctly updated, the wrapper's job is done for that bug — **do NOT re-author or edit the entry**. Bug-entry closure is `/fix-bug`'s canonical responsibility; duplicating it in the wrapper is a LEAK (scattered knowledge).
4. If the entry is somehow NOT updated after `/fix-bug` returns (rare — would indicate a bug in `/fix-bug` itself), file a follow-up bug against `/fix-bug` rather than patching the entry manually. Manual patches create drift from the canonical form.

**Why the wrapper must not edit the entry**: `.claude/skills/fix-bug/SKILL.md` owns bug-entry-closure logic as a single source of truth. If the wrapper re-edits the entry after `/fix-bug` completes, it creates a second copy of closure logic that can drift from the canonical form (as a prior version of this wrapper did — see `plans/dual-tpr-gemini/section-05-review-work.md §05.R [TPR-05-003-codex]`). The wrapper's contract is: invoke `/fix-bug`, then verify its output — nothing more.

**Why the hand-off matters**: skipping `/fix-bug` leaves no fix-section record, no TDD matrix, no TPR validation, and no hygiene review for the bug. It also leaves `/review-bugs` to report the lifecycle gap, and breaks `/fix-next-bug` autopilot which expects fix-sections to exist. The canonical contract exists precisely because bug-tracker bugs are often cross-cutting and benefit from the extra investigation rigor that a fix-section provides.

**If a bug-tracker finding genuinely requires zero investigation** (a typo fix or a single-line change with obvious root cause), the `/fix-bug` skill itself handles this efficiently — it still produces a fix-section, but the investigation/TDD phases are lightweight. The fix-section is the permanent record, not a gate.

#### 7c. Commit Fixes

Run `/commit-push` to commit the fixes. The commit message should reference the reviewer-tagged TPR IDs fixed (e.g. `fix(arc): release iterator on early break — [TPR-04-001-codex] [TPR-04-001-gemini]`).

#### 7d. Re-run the Dual-Source Transport (GO TO STEP 1)

Go back to Step 1. BOTH reviewers re-review the FIXED code to confirm the issues are actually resolved and no new issues were introduced by the fixes. **This re-run is not optional, and a partial re-run (only one reviewer) is not a valid clean pass.**

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

## Transport Failure Handling

If `dual-invoke-with-retry.sh` exits non-zero, the transport has exhausted its 3 internal infra retries and the round cannot proceed. The script prints the failure category on the last line of stderr and preserves the postmortem files under `$RUN` for inspection.

**DO NOT silently retry the semantic loop on infra failure.** The 10-iteration budget is for finding-fixing rounds, not transport failures. Incrementing the semantic counter on a transport failure hides real infrastructure bugs and falsely claims iteration progress — the state machine invariant above forbids it.

### Failure taxonomy

The transport reports one of these categories on its stderr tail (prefixed `infra_retries_exhausted:`):

| Category | Meaning |
|---|---|
| `launch_or_exit_fail` | Either reviewer process failed to start or exited non-zero on all 3 attempts (includes crashes, missing CLI, auth errors) |
| `codex_*` | `parse-codex.py` rejected the codex JSONL stream on all 3 attempts. Suffix is the parser's first error line (`codex_schema_violation`, `codex_missing_envelope`, `codex_parse_error`, etc.) |
| `gemini_*` | `parse-gemini.py` rejected the gemini stream-json on all 3 attempts. Suffix is the parser's first error line (`gemini_missing_terminator`, `gemini_no_begin`, `gemini_no_end`, `gemini_parse_error`, etc.) |
| `dirty_worktree` | **Strict mode only** (`ORI_TPR_STRICT_WORKTREE=1`): `worktree-guard.sh compare` detected tracked-file drift and strict mode escalated it to terminal. **Without strict mode (default)**: drift is a non-blocking WARNING — the round succeeds, envelopes are parsed, and drift details are saved to `$RUN/worktree-drift.txt`. Most drift is from parallel agents or user edits, not reviewer violations. |
| `unknown_failure` | Fallback — the script exhausted retries without recording a specific category (rare; investigate round.log) |

### Worktree drift (non-blocking, default behavior)

After a successful round, check for `$RUN/worktree-drift.txt`. If it exists and is non-empty, tracked files changed during the review. This is **expected** when parallel agents or the user are editing files. The drift details are also logged in `$RUN/round.log` under `WARNING: worktree drift detected`. No action is required — drift does not affect the review envelopes. If you want strict enforcement (e.g., in a CI context), set `ORI_TPR_STRICT_WORKTREE=1`.

### Escalation procedure

When the transport fails, surface the failure to the user via `AskUserQuestion` with:

1. **Failure category** — the literal string from the transport stderr tail, including suffix if any
2. **Postmortem directory** — the `$RUN` path so the user can inspect it directly
3. **Files to inspect in `$RUN`:**
   - `round.log` — orchestration timeline (every attempt, every backoff, every failure category)
   - `codex.jsonl` / `gemini.jsonl` — raw reviewer output streams (may be empty if launch failed)
   - `codex.envelope.json` / `gemini.envelope.json` — parsed envelopes (absent if parse failed)
   - `codex.parse-error` / `gemini.parse-error` — parser error output (first line = failure reason)
   - `worktree-drift.txt` — tracked-file drift details (present when drift detected, even on successful rounds)
   - `worktree-after.txt` — post-run worktree snapshot (when drift detected)
   - `codex.exit` / `gemini.exit` — reviewer exit codes
   - `codex.walltime` / `gemini.walltime` — wall time per reviewer (useful when one hung)
4. **Recommended user actions:**
   - **Triage the failure** — open `$RUN/round.log` first, then the specific files indicated by the category (e.g. `codex.parse-error` for a `codex_*` failure). Fix the root cause (the prompt, a reviewer bug, a transport bug, a dirty reviewer skill) and re-run `/tpr-review`.
   - **Retry immediately** — if the failure is a known-transient cloud outage and the user wants Claude to launch another round as-is. Use this sparingly; most transport failures reflect real infrastructure bugs worth triaging.
   - **Abandon the review** — if the review cannot proceed (e.g. reviewer CLI is offline, credentials missing, persistent schema violation). Log the failure category + `$RUN` path in any owning plan's working notes so the operator can follow up later, then stop.

### What NOT to do on transport failure

- Do NOT retry the semantic loop silently (violates the state machine invariant).
- Do NOT fabricate a clean pass to unblock the user — a transport failure is a real signal and must be surfaced.
- Do NOT delete `$RUN` before the user triages — the postmortem is the evidence trail.
- Do NOT rewrite the prompts and retry without telling the user — if the prompt needs changing, that is a user decision.

## Merged Finding Format

This section specifies how merged findings are written into the owning plan's `## {NN}.R Third Party Review Findings` block (or the bug-tracker, if there is no owning plan). Claude produces these entries in Step 7a above; the format is load-bearing because future `/tpr-review` runs, `/review-bugs`, and plan audits depend on it.

### Ordinal numbering is independent per reviewer

`merge-findings.py` assigns ordinals by **insertion order within each reviewer's envelope**, independently. The first finding in the codex envelope is `-001-codex`, the first in the gemini envelope is `-001-gemini`. There is NO shared ordinal space: `[TPR-04-001-codex]` and `[TPR-04-001-gemini]` are not required to be the same finding — the `agreement: true` flag from the merger is the authoritative cross-reference.

### Agreement case — both reviewers flagged the same (location, title)

When `merge-findings.py` reports `agreement: true`, both halves are filed adjacent with a cross-reference annotation. Both entries point at each other via the `Agreement:` line so the plan's TPR block preserves the independence contract while still making the convergence visible:

```md
- [ ] `[TPR-04-001-codex][high]` `compiler/ori_arc/src/lower/iter.rs:218` — Add dec on early-exit branch of iterator loop.
  Evidence: On `break` inside `for x in iter do ...`, the iterator value's RC is never decremented before the loop exits, leaving a leaked reference on the remaining elements. Reproduced via `tests/valgrind/iter_break.ori`.
  Impact: Memory leak on every early-exit iteration; severity scales with iterator payload size.
  Required plan update: Add a `dec` emission to the early-exit branch in `ori_arc/src/lower/control_flow/for_loop.rs`; verify via `ORI_CHECK_LEAKS=1` on the matrix tests.
  Basis: fresh_verification. Confidence: high.
  Agreement: [TPR-04-001-gemini] (both reviewers flagged this location/title)
- [ ] `[TPR-04-001-gemini][high]` `compiler/ori_arc/src/lower/iter.rs:218` — Add dec on early-exit branch of iterator loop.
  Evidence: The lowering pass emits an iterator inc on loop entry but the early-exit path in `lower_break` does not call the matching dec. Verified against Swift's SILOptimizer/ARC/ARCContract.cpp, which explicitly handles the analogous case.
  Impact: Same as above (agreement finding).
  Required plan update: Same as above.
  Basis: direct_file_inspection. Confidence: high.
  Citations: [{url: "https://github.com/apple/swift/blob/main/lib/SILOptimizer/ARC/ARCContract.cpp", description: "Swift's equivalent arc contract pass, for cross-reference"}]
  Agreement: [TPR-04-001-codex] (both reviewers flagged this location/title)
```

**Why both halves are filed** — filing only the codex half would erase the gemini reviewer's independent observation (and its citations), which violates the dual-source independence contract. Filing only the gemini half would erase the codex finding's ordinal continuity. Both are recorded; the `Agreement:` cross-reference makes the convergence clear to any human or tool auditing the block.

### Gemini-only case — a finding with no codex counterpart

```md
- [ ] `[TPR-04-002-gemini][medium]` `library/std/prelude.ori:5` — Replace println with tracing::debug.
  Evidence: The prelude emits a `println` on module load to report version info. `println` writes to stdout, which pollutes test captures; the project convention is to use `tracing::debug` with the `ori_*` target (see CLAUDE.md §Tracing).
  Impact: Test snapshot churn on every prelude load; violates the "no println" rule.
  Required plan update: Switch to `tracing::debug!(target: "ori_prelude", ...)` and add a `#[tracing::instrument]` on the prelude loader.
  Basis: inference. Confidence: medium. (Gemini-only finding — no codex counterpart.)
```

**Why single-tag is still actionable** — per Step 5 (Classify), provenance is not severity. A gemini-only finding gets fixed the same way as an agreement finding; the tag is audit metadata, not a filter.

### Codex-only case — symmetric to gemini-only

```md
- [ ] `[TPR-04-003-codex][low]` `compiler/ori_parse/src/expr/binary.rs:142` — Tighten error span on operator-precedence mismatch.
  Evidence: The current error points at the left operand; the spec's operator-rules.md §B.PRECEDENCE example shows the caret should sit on the operator itself.
  Impact: Diagnostic UX regression in a code path that already has a regression test; trivial fix.
  Required plan update: Update `binary.rs:142` to emit the span on the operator token.
  Basis: direct_file_inspection. Confidence: high. (Codex-only finding — no gemini counterpart.)
```

### Resolution format — always preserve the reviewer tag

When a finding is fixed (Step 7b), mark the entry `[x]` and append a `Resolved:` line referencing the code fix. For agreement findings, both halves are resolved together — the second resolution can reference the first rather than duplicating the fix description:

```md
- [x] `[TPR-04-001-codex][high]` ...
  Resolved: Fixed on 2026-04-07 in commit abc123. Added `dec` emission in `lower_break` early-exit branch; verified via `ORI_CHECK_LEAKS=1 cargo t -p ori_arc iter_break`.
- [x] `[TPR-04-001-gemini][high]` ...
  Resolved: Fixed on 2026-04-07 in commit abc123. Same fix as [TPR-04-001-codex] (agreement).
```

**NEVER delete a resolved finding.** Mark it `[x]` with a resolution note — deletion erases the audit trail and invites re-filing by the next review pass.

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
