# Round Setup Protocol

Read by a Sonnet sub-agent dispatched from `/tpr-review` at the start of each round. Not a registered skill.

The setup agent handles Steps 0–4 + polling + merge: CLAUDE.md re-read, spec/grammar gate, intel pre-query, scratch dir creation, reviewer prompt generation (with Thoroughness Re-review Directive prepended when the round-state flag is set), transport launch, polling to completion, and envelope merging. Its sole output is `/tmp/tpr-{run}/round-{N}/merged.json` (and a brief summary) — downstream triage consumes this.

If the transport exits non-zero, the setup agent follows the Transport Failure Handling section and returns an escalation payload; it never retries the transport inline.


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

### 1. Create a per-run scratch directory

```
Bash:
  RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
  echo "$RUN"
```

**This MUST be a separate foreground Bash call.** The `$RUN` path must be visible in Claude's context before Steps 2-3 run. If `$RUN` is created inside a `run_in_background: true` compound command, Claude never sees the path and will poll the wrong directory — potentially a parallel session's directory from a different project.

Each semantic iteration gets a fresh `$RUN` (e.g. `/tmp/ori-tpr-XXXXXXXX`). Reuse across iterations is forbidden — a stale envelope from the previous round would corrupt the merge.

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

**Scope-hint extraction rule (mechanical, per mode):** the `<scope hint>`
placeholder in both prompts is NOT subjective. The setup agent resolves it
deterministically:

| Mode | `<scope hint>` value |
|---|---|
| `review-work` (default) | the git range the work spans — e.g. `HEAD~5..HEAD`, or an explicit range from `ARGS` if one is passed. Add a diff file list after the range. |
| `review-plan` | the plan path from `ARGS` (everything after `--skill review-plan `). Pass the directory for whole-plan mode or the section file for single-section mode, verbatim. |
| `custom` | the raw `objective` text (i.e. the ARGS text itself). Do NOT summarize or rephrase; the reviewers operate on the objective verbatim. |

If ARGS carries both a skill and a path (e.g. `--skill review-plan plans/foo/section-03.md`), inject the path — not the full `--skill review-plan plans/...` string — as the scope hint.

**Section-number assignment for `merge-findings.py --section <NN>`:**
The merger needs a two-digit section prefix for tagged finding IDs
(`[TPR-04-001-codex]`). The setup agent resolves `<NN>` as follows:

1. If ARGS resolves to a specific plan section (either `review-plan <path>`
   pointing at `section-NN*.md`, or a `review-work` scope that targets a
   known roadmap section), use that section number verbatim.
2. Otherwise, apply the canonical subsystem mapping below (same table as the
   triage agent's §7a — duplicated here only because the merger runs in
   Step 4 before triage):

    | File path pattern | Section |
    |---|---|
    | `compiler/ori_parse/`, `compiler/ori_lexer/` | `01` |
    | `compiler/ori_types/` | `02` |
    | `compiler/ori_eval/`, `compiler/ori_patterns/` | `03` |
    | `compiler/ori_llvm/`, `compiler/ori_arc/` | `04` |
    | `compiler/ori_rt/` | `05` |
    | `library/std/`, `compiler/ori_registry/` | `06` |
    | `compiler/oric/`, `compiler/ori_fmt/`, `compiler/ori_diagnostic/` | `07` |
    | `docs/`, `.claude/`, `plans/` | `08` |

   If the diff spans multiple mapping rows, pick the dominant row (highest
   line-count in the diff). Ties resolve to the lowest-numbered section.
3. For `custom` mode with no compiler surface, or when no mapping row
   matches, use the fallback `XX`. The triage agent may re-classify findings
   at filing time if they belong to a specific subsystem.

#### Mandatory Mission-Adamancy Block

**Every reviewer prompt MUST contain a Mission-Adamancy section BEFORE the grounding block.** This is the first thing the reviewer reads. It exists to block the `INVERTED-TDD` failure mode — where a reviewer sees green tests, reads the diff superficially, and signs off without verifying the stated deliverable is actually active on the inputs it was designed to cover. Reviewers MUST treat mission-verification as their first-order duty; ungrounded "the tests pass, looks good" reviews are themselves findings.

The block is IDENTICAL for both reviewers. Paste it verbatim at the top of each prompt, before any other instructions, and AFTER the Thoroughness Re-review Directive if that is active.

```
## MISSION ADAMANCY — the first thing you check, every time

Before looking at any code, test result, or commit message, state in your
own words (in the envelope's `mission` field if present, or in your first
finding's description otherwise):

  1. What is the stated deliverable of this subsection / section / plan /
     custom objective under review? (e.g., "wire validate_body_types into
     all 4 body-pass sites to enforce typeck.md §PC-2").
  2. What system invariant does that deliverable enforce? Cite the
     spec clause, rule anchor, or CLAUDE.md section (e.g., "PC-2: no
     Tag::Var in typed IR reaches AIMS / codegen").
  3. Which downstream subsystem consumes that invariant? (e.g., "AIMS
     analysis per aims-rules.md; codegen per codegen-rules.md TR-2").

Then VERIFY — not just read — that the code under review actually keeps
the deliverable active on the inputs the deliverable was designed to
cover. Green tests are NOT evidence of mission success; they are evidence
that the tests that exist passed. A deliverable can be inert on real
input paths while every test in the suite goes green — that is exactly
the INVERTED-TDD failure mode defined in `impl-hygiene.md §Finding
Categories`.

Specific things to hunt for (Critical findings when present):

  - Early-returns, feature flags, `#cfg` skips, or gate conditions near
    the stated validator/check/assertion/enforcement — trace whether
    any of them disable the deliverable on the inputs it was designed
    to catch. If yes: `INVERTED-TDD:gated-deliverable`.
  - Exemption sets (allow-lists, skip-lists, exempt var id sets) that
    grew in the diff — demand a spec citation for each addition; if
    the growth is justified only by "tests fail otherwise",
    that is `INVERTED-TDD:widened-exemption`.
  - Subsection / section / plan items marked `complete` while the
    deliverable is gated off or short-circuited on one or more input
    paths — `INVERTED-TDD:subsection-complete-with-deliverable-inert`.
  - Tests added for the WORKAROUND ("when gate is active, validator
    correctly skips X") rather than the DELIVERABLE
    ("deliverable fires on X") — `INVERTED-TDD:disabled-negative-pin`
    (or a fresh `INVERTED-TDD:workaround-test` subcategory).
  - A blocker bug filed via `/add-bug` when the bug is blocking the
    subsection's stated deliverable. The right mode for blockers is
    `/fix-bug` with full plan-section rigor. Call this out as
    `INVERTED-TDD:blocker-add-bug-only`.
  - Commit messages / plan updates / code comments containing the
    phrases "make tests pass", "pragmatic workaround", "gate the
    failing path", "accept current state and proceed", "file bugs
    and proceed" (without per-bug `/fix-bug` vs `/add-bug`
    classification), "mark as Known Failing Tests" (without a concrete
    `- [ ]` anchor). Each occurrence is a smell — cross-reference
    against the actual code change to confirm or clear.

INVERTED-TDD findings are Critical by default and block section
close-out. Remediation is always the architecturally-correct fix per
`CLAUDE.md §The One Rule`, regardless of scope / effort / cost / risk.
Never recommend "keep the gate, file a bug, proceed" — that
recommendation is itself deferral per `CLAUDE.md §ZERO DEFERRAL on
bugs`.

Mission-adamancy is NOT optional. A review that signs off on work whose
deliverable is inert misses the entire point of the review and harms
the project more than no review at all — it grants false confidence.
If you cannot articulate (1), (2), (3) above from the evidence packet
plus the rule files, that lack of articulation is itself the first
finding: the scope is under-specified.
```

#### Mandatory Grounding Block

**Every reviewer prompt MUST contain a grounding section before the scope
hint.** Without grounding, reviewers produce generic "this looks odd" noise
instead of precise `LEAK:scattered-knowledge at path:line` findings.

The grounding block is IDENTICAL for both reviewers. The orchestrator does
NOT pre-summarize these files — codex and gemini have full read access
and are capable agents; asking them to read rules directly avoids
staleness and preserves orchestrator context.

The authoritative policy is **"CLAUDE.md + every `.claude/rules/*.md`
file"**. The setup agent MUST enumerate the rule files mechanically from
the repository state rather than hand-selecting. This repo-level rule
corpus (CLAUDE.md + the rules directory) is the source of truth for
reviewer grounding; the enumeration below makes it auditable from
source and keeps the prompt in sync with the live filesystem.

**Enumeration procedure (the setup agent runs this once per round to
build the list literally pasted into the prompt):**

```
Bash:
  ls .claude/rules/*.md | sort
```

Take the sorted list verbatim. As of this writing the enumeration yields:

1. `CLAUDE.md` (project root) — overarching rules, correctness, deferral ban
2. `.claude/rules/aims-rules.md` — AIMS lattice, interprocedural contracts
3. `.claude/rules/aot.md` — AOT compilation flow
4. `.claude/rules/arc.md` — ARC IR shape, RC invariants
5. `.claude/rules/canon.md` — pipeline SSOT, phase map
6. `.claude/rules/canonicalization.md` — canonicalization phase rules
7. `.claude/rules/cargo.md` — cargo / build system
8. `.claude/rules/codegen-rules.md` — LLVM codegen TR/TM/NR/AB/VR anchors
9. `.claude/rules/compiler.md` — architecture, phase boundaries
10. `.claude/rules/diagnostic.md` — diagnostic scripts, error vocabulary
11. `.claude/rules/eval.md` — evaluator rules
12. `.claude/rules/fmt.md` — formatter rules
13. `.claude/rules/impl-hygiene.md` — finding vocabulary, SSOT, DRY
14. `.claude/rules/intelligence.md` — graph query protocol
15. `.claude/rules/ir.md` — IR shape, DerivedTrait contract
16. `.claude/rules/llvm.md` — LLVM-binding specifics
17. `.claude/rules/ori-lang.md` — Ori language quick reference
18. `.claude/rules/ori-syntax.md` — Ori syntax reference
19. `.claude/rules/parse.md` — lexer/parser LB/AR/DI anchors
20. `.claude/rules/patterns.md` — pattern compilation
21. `.claude/rules/proposals.md` — proposal workflow
22. `.claude/rules/registry.md` — builtin behavior SSOT
23. `.claude/rules/repr.md` — representation / layout
24. `.claude/rules/roadmap.md` — roadmap conventions
25. `.claude/rules/runtime.md` — runtime (ori_rt) rules
26. `.claude/rules/spec.md` — spec conventions
27. `.claude/rules/tests.md` — matrix testing / pin protocol
28. `.claude/rules/typeck.md` — type-checker PC/EX/CF/CP/DI anchors
29. `.claude/rules/types.md` — type pool, tags, interning

If the `ls` yields a different set (files added or removed), use the
current listing verbatim — do not hand-edit this markdown to match. The
prompt reflects the live filesystem, which is the source of truth.

The reviewers handle reading and filtering. Keep the prompt short.

Write both prompts to the scratch dir. **Use the template matching the active mode:**

#### Prompt templates for `review-work` and `review-plan` modes (skill-dispatch)

```
Bash:
  cat > "$RUN/codex.prompt.md" <<'PROMPT'
  Run the /{skill_name} skill in envelope-only mode. Emit the JSON
  envelope per .claude/skills/dual-tpr/findings-schema.json; do NOT
  write findings to plan files.
  # NOTE: {skill_name} is review-work (default) or review-plan (plan review)

  <Mission-Adamancy block — pasted verbatim from the Mandatory
   Mission-Adamancy Block section of step-1-round-setup.md>

  ## Grounding — read these files FIRST before reviewing

  Read these rule files in full before examining any scope files.
  This grounding is MANDATORY — an ungrounded review produces
  generic noise instead of project-native, rule-anchored findings.
  Every finding MUST cite the vocabulary from impl-hygiene.md
  (LEAK / DRIFT / GAP / WASTE / EXPOSURE / BLOAT / NOTE) and the
  specific rule anchor (TR-2, NR-1, RL-2, etc.) it violates.

  Read ALL of these in full (paste the full list from `ls .claude/rules/*.md`,
  enumerated in the Mandatory Grounding Block above — CLAUDE.md plus every
  .claude/rules/*.md file). The enumeration is policy; do not sub-select.
  Do NOT summarize these files in the prompt — the reviewer reads them.

  ## Scope: <scope hint — e.g. "HEAD~5..HEAD", a plan section name, or explicit files>

  <If Step 0.75 produced an Intelligence Summary, insert it here as literal text>

  <evidence packet: what changed, why, what to look for>
  PROMPT

  cat > "$RUN/gemini.prompt.md" <<'PROMPT'
  Activate the {skill_name} skill and follow its instructions exactly.
  Emit the JSON envelope per .claude/skills/dual-tpr/findings-schema.json;
  do NOT write findings to plan files.
  # NOTE: {skill_name} is review-work (default) or review-plan (plan review)

  <Mission-Adamancy block — pasted verbatim from the Mandatory
   Mission-Adamancy Block section of step-1-round-setup.md>

  ## Grounding — read these files FIRST before reviewing

  Read these rule files in full before examining any scope files.
  This grounding is MANDATORY — an ungrounded review produces
  generic noise instead of project-native, rule-anchored findings.
  Every finding MUST cite the vocabulary from impl-hygiene.md
  (LEAK / DRIFT / GAP / WASTE / EXPOSURE / BLOAT / NOTE) and the
  specific rule anchor (TR-2, NR-1, RL-2, etc.) it violates.

  1. CLAUDE.md (project root) — correctness, no deferral, phase purity
  2. .claude/rules/impl-hygiene.md — finding vocabulary, SSOT, DRY
  3. .claude/rules/tests.md — matrix testing, semantic/negative pins
  4. .claude/rules/compiler.md — architecture, phase boundaries
  <same rule file list as codex prompt — CLAUDE.md + every .claude/rules/*.md>

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

  ## Grounding — read these files FIRST before reviewing

  Read these rule files in full before examining the objective.
  An ungrounded review produces generic noise instead of project-
  native findings. Every finding MUST cite the vocabulary from
  impl-hygiene.md (LEAK / DRIFT / GAP / WASTE / EXPOSURE / BLOAT /
  NOTE) and the specific rule anchor it violates.

  Read ALL of these in full (the authoritative policy is "CLAUDE.md
  + every .claude/rules/*.md file", mechanically enumerated — see
  the Mandatory Grounding Block above for the exact list produced
  by `ls .claude/rules/*.md | sort`). The enumeration IS the policy;
  do not sub-select or trim to a convenient subset just because the
  objective looks narrow. Custom objectives review the full repo
  context, so the grounding must match.
  <paste the full sorted enumeration from the Mandatory Grounding Block — CLAUDE.md plus every .claude/rules/*.md file>

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

  ## Grounding — read these files FIRST before reviewing

  Read these rule files in full before examining the objective.
  An ungrounded review produces generic noise instead of project-
  native findings. Every finding MUST cite the vocabulary from
  impl-hygiene.md (LEAK / DRIFT / GAP / WASTE / EXPOSURE / BLOAT /
  NOTE) and the specific rule anchor it violates.

  Read ALL of these in full (the authoritative policy is "CLAUDE.md
  + every .claude/rules/*.md file", mechanically enumerated — see
  the Mandatory Grounding Block above for the exact list produced
  by `ls .claude/rules/*.md | sort`). The enumeration IS the policy;
  do not sub-select. Custom objectives review the full repo context,
  so the grounding must match.
  <paste the full sorted enumeration from the Mandatory Grounding Block — CLAUDE.md plus every .claude/rules/*.md file, identical to the codex prompt>

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

The transport launches both reviewers in parallel, handles infra retries (5 attempts per reviewer; default backoff `1s / 2s / 4s / 30s / 60s`; capacity-aware backoff `30s / 60s / 120s / 120s / 120s` when the API reports capacity errors — see `dual-invoke-with-retry.sh` for the SSOT schedule), runs the schema validators, and applies the dirty-worktree guard. A full round typically takes 5-15 minutes — BOTH reviewers running concurrently, so wall time is roughly `max(codex_walltime, gemini_walltime)`, not the sum.

Running the transport in the Bash foreground either hits the 2-minute tool timeout or gets auto-backgrounded with output truncated. Always use `run_in_background: true`. The `.claude/hooks/block-banned-commands.sh` hook explicitly allows backgrounded codex and gemini commands.

The `--skill` parameter controls the transport log label. Default: `review-work`. If `ARGS` contains `--skill review-plan`, use `review-plan`. For custom objective mode, use `custom`.

All dual-source consumers (`/tpr-review`, `/tp-help`, `/review-work`, `/review-plan`) route through the canonical SSOT `one-round.sh`. For envelope-mode consumers this is a thin passthrough to `dual-invoke-with-retry.sh` *plus* a shared circuit-breaker pre-check that trips cross-skill — so a gemini-degraded state recorded by a prior `/tp-help` run auto-restricts this round's `ORI_TPR_REVIEWERS` before the transport is even launched. Calling `dual-invoke-with-retry.sh` directly bypasses the pre-check and is disallowed for first-party callers.

```
Bash (run_in_background: true):
  .claude/skills/dual-tpr/scripts/one-round.sh \
    --mode envelope \
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

### 4. On success: merge envelopes

When the completion notification arrives AND the transport exited 0, the active reviewer(s)' envelopes passed parser + schema + worktree-guard validation. The merger handles missing envelopes gracefully (when a reviewer was skipped by the circuit breaker, its `.envelope.json` won't exist — the merger treats it as zero findings and sets `reviewer_mode: "single"`). Run:

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

Top-level fields added by the merger:
- `reviewer_mode` — `"dual"` (both reviewers ran) or `"single"` (circuit breaker skipped one)
- `active_reviewers` — list of reviewer names that produced envelopes (e.g. `["codex"]`)
- `tripped_reviewer` — name of the skipped reviewer, or `null` in dual mode

The `summary` block reports `codex_findings`, `gemini_findings`, `agreements`, `codex_only`, `gemini_only`, `max_severity`.

**Single-agent mode note:** When `reviewer_mode` is `"single"`, the coordinator's convergence loop automatically switches to single-agent rules (min 3 rounds, high-severity persistence gate). No setup-agent action is needed beyond passing the merger output through as normal. Include `reviewer_mode` and `tripped_reviewer` in the short summary returned to the coordinator so it can log the degraded mode.


## Thoroughness Re-review Directive (prepend when strengthened_language_required)

When the round coordinator passes `strengthened_language_required: true` in its dispatch prompt, prepend the following directive block verbatim BEFORE the grounding block in BOTH reviewer prompts. The directive is SYMMETRIC — both reviewers receive identical text to preserve dual-source independence.

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


## Transport Failure Handling

If `dual-invoke-with-retry.sh` exits non-zero, the transport has exhausted its 5 internal infra retry attempts per reviewer (default backoff `1s / 2s / 4s / 30s / 60s`; capacity-aware backoff `30s / 60s / 120s / 120s / 120s`) and the round cannot proceed. The script prints the failure category on the last line of stderr and preserves the postmortem files under `$RUN` for inspection.

**DO NOT silently retry the semantic loop on infra failure.** The 10-iteration budget is for finding-fixing rounds, not transport failures. Incrementing the semantic counter on a transport failure hides real infrastructure bugs and falsely claims iteration progress — the state machine invariant above forbids it.

### Failure taxonomy

The transport reports one of these categories on its stderr tail (prefixed `infra_retries_exhausted:`):

| Category | Meaning |
|---|---|
| `launch_or_exit_fail` | Either reviewer process failed to start or exited non-zero on all 5 attempts (includes crashes, missing CLI, auth errors) |
| `codex_*` | `parse-codex.py` rejected the codex JSONL stream on all 5 attempts. Suffix is the parser's first error line (`codex_schema_violation`, `codex_missing_envelope`, `codex_parse_error`, etc.) |
| `gemini_*` | `parse-gemini.py` rejected the gemini stream-json on all 5 attempts. Suffix is the parser's first error line (`gemini_missing_terminator`, `gemini_no_begin`, `gemini_no_end`, `gemini_parse_error`, etc.) |
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
