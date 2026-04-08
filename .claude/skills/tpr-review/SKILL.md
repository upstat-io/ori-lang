---
name: tpr-review
description: "Run an independent dual-source (codex + gemini) third-party review of your work in parallel, then fix findings and re-run until BOTH reviewers come back clean — TRIGGER proactively after completing ANY non-trivial work: bug fixes, new features, refactors, multi-file changes, compiler changes, codegen changes, test additions, plan implementations, or anything touching correctness-sensitive code. When in doubt, run it. The cost of an unnecessary review is near zero; the cost of a missed bug is high."
---

# Dual-Source TPR Review (Codex + Gemini)

Run BOTH the Codex CLI AND the Gemini CLI non-interactively in parallel to perform independent review-work passes, merge their findings with reviewer tagging, then fix any findings and re-run until BOTH reviewers return zero actionable findings. Codex and Gemini each have their own context, rules, and skills — they figure out scope on their own.

This wrapper is built on the Section 02 dual-source transport utility. All launching, parsing, schema validation, worktree-guarding, and infra retry logic lives in `.claude/skills/dual-tpr/scripts/` — this skill is purely the **semantic** fix-and-re-run loop that consumes merged findings. See `.claude/skills/dual-tpr/transport.md` for the transport contract.

## Step 0 — MANDATORY: Re-read CLAUDE.md

**Before doing ANYTHING else, re-read the entire project CLAUDE.md.** This is non-negotiable. Even if you believe it is in memory, you MUST physically read it with the Read tool. Context compression may have dropped critical rules. Do this every single time this skill runs.

```
Read CLAUDE.md (the project root one)
```

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

**Also run when:**
- You're unsure whether the change warrants review (default: run it)
- The work involved multiple steps or non-obvious decisions
- The change touches code paths shared across subsystems
- You fixed something that was interfering with other code

**The only time NOT to run:** purely cosmetic single-line changes (typo fixes, comment edits, formatting-only).

## Loop Protocol — MANDATORY

```
+---------------------------------------------------------+
|              DUAL-SOURCE TPR REVIEW LOOP                |
|                                                         |
|  0. CLAUDE re-reads CLAUDE.md (MANDATORY)               |
|        |                                                |
|  1. TRANSPORT launches BOTH reviewers in parallel:      |
|     - codex exec (envelope-only mode)                   |
|     - gemini  (review-work skill activation)            |
|     Infra retries (3 per reviewer, exp. backoff)        |
|     are INSIDE the transport — they do NOT consume      |
|     semantic iterations.                                |
|        |                                                |
|  2. CLAUDE merges findings via merge-findings.py        |
|        |                                                |
|  3. Zero actionable findings? --YES--> DONE (clean)     |
|        |                                                |
|       NO                                                |
|        |                                                |
|  4. CLAUDE files findings in plan/bug-tracker           |
|  5. CLAUDE fixes each finding (code + tests)            |
|  6. CLAUDE commits fixes via /commit-push               |
|        |                                                |
|  7. Go to step 1 (BOTH reviewers re-review fixed code)  |
|                                                         |
+---------------------------------------------------------+
```

**Three actors:**
- **Codex** (external reviewer #1): runs `.codex/skills/review-work/SKILL.md` in envelope-only mode. Does NOT fix anything.
- **Gemini** (external reviewer #2): runs `.gemini/skills/review-work/SKILL.md`. Does NOT fix anything. Can issue `google_web_search` for external claim verification.
- **Claude** (you): reads merged findings, fixes the code, commits, re-invokes the transport.

**A round succeeds only when BOTH reviewers complete cleanly AND the merged finding list contains zero actionable findings.** Filing findings without fixing and re-running is deferral. Fixing findings without re-running BOTH reviewers to confirm clean is incomplete. A partial re-run (only one reviewer) is NOT a valid clean pass.

**Maximum semantic iterations: 10.** Infra retries inside `dual-invoke-with-retry.sh` do NOT count against this budget — the budget is for finding-fixing rounds, not transport failures. If after 10 semantic cycles findings are still surfacing, surface the remaining merged findings to the user via `AskUserQuestion`.

### Loop State Machine

The loop protocol above is an illustrative diagram; the state machine below is the authoritative contract. Infra retries are invisible to `iteration_counter` — they happen inside step `dual-invoke-with-retry.sh` and either resolve (round continues) or exhaust (user escalation, counter untouched).

```
iteration_counter = 0
while iteration_counter < 10:
    RUN = scratch-dir.sh
    write codex.prompt.md and gemini.prompt.md into RUN
    if dual-invoke-with-retry.sh fails:
        # infra retries (3 per reviewer, 1s/2s/4s backoff) already
        # exhausted inside the transport — do NOT increment, do NOT
        # retry the semantic loop
        surface failure category + $RUN to user via AskUserQuestion
        EXIT
    else:
        # both envelopes passed parser + schema + worktree-guard
        merged = merge-findings.py(codex.envelope.json, gemini.envelope.json)
        if merged has zero actionable findings:
            CLEAN PASS — exit with iteration_counter for the report
        for each actionable finding in merged:
            file into owning plan TPR block or bug-tracker
            fix the code
            run `timeout 150 ./test-all.sh`
        commit via /commit-push
        iteration_counter += 1
# After 10 semantic iterations without a clean pass:
surface remaining merged findings to user via AskUserQuestion
```

**Invariants:**
- `iteration_counter` increments ONLY after a successful round that found actionable findings AND those findings were fixed AND the commit landed. Any earlier exit (infra failure, clean pass) skips the increment.
- Infra retries (inside the transport) and semantic iterations (this loop) are **orthogonal budgets**. One cannot consume the other. A transient network hiccup burning 3 infra retries still leaves the 10-iteration semantic budget untouched.
- A clean pass on any iteration is a terminal state: the report includes `iteration_counter` at exit so "clean on iteration 1" vs "clean on iteration 3 after fixing N findings" are distinguishable in the final output.
- The 10-iteration cap is a **user-facing stopping rule**, not a correctness guarantee — if findings keep surfacing, that is signal, not noise, and the user decides whether to continue, abandon, or dig into a recurring finding.

## Steps (Per Iteration)

### 1. Create a per-run scratch directory

```
Bash:
  RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
  echo "$RUN"
```

Each semantic iteration gets a fresh `$RUN` (e.g. `/tmp/ori-tpr-XXXXXXXX`). Reuse across iterations is forbidden — a stale envelope from the previous round would corrupt the merge.

### 2. Write both reviewer prompts

The codex and gemini prompts share the same evidence packet but differ in their activation preamble. See `.claude/skills/dual-tpr/transport.md` for the canonical preambles.

- **Codex prompt** MUST include the literal keyword `envelope-only` in its first 500 characters — this dispatches `.codex/skills/review-work/SKILL.md` into envelope-only mode.
- **Gemini prompt** MUST start with the literal activation phrase `Activate the review-work skill and follow its instructions exactly.` — gemini does NOT auto-activate from description matching; the phrase is load-bearing.

#### Mandatory Grounding Block

**Every reviewer prompt MUST contain a "Grounding — read these files FIRST" section before the scope hint.** Without this grounding, reviewers produce findings against unknown conventions — generic "this looks odd" noise instead of precise `LEAK:scattered-knowledge at path:line` findings that match the project's actual rules.

The grounding block is IDENTICAL for both reviewers and MUST list:

1. `CLAUDE.md` (project root) — correctness above all, no deferral, stabilization discipline
2. `.claude/rules/impl-hygiene.md` — SSOT, No Side Logic, finding categories (LEAK/DRIFT/GAP/WASTE/EXPOSURE/BLOAT/NOTE), canonical homes, algorithmic DRY, test-function-naming rules
3. `.claude/rules/tests.md` — matrix testing rule, interaction testing, negative pin protocol, regression discipline
4. Any `.claude/rules/*.md` file relevant to the files under review (e.g. `parse.md`, `arc.md`, `registry.md`) — list the specific ones in the prompt

Write both prompts to the scratch dir:

```
Bash:
  cat > "$RUN/codex.prompt.md" <<'PROMPT'
  Run the /review-work skill in envelope-only mode. Emit the JSON
  envelope per .claude/skills/dual-tpr/findings-schema.json; do NOT
  write findings to plan files.

  ## Grounding — read these files FIRST before reviewing

  Before you look at any of the changed code, read these files in full
  so your findings are scoped to the project's actual rules. Every
  finding must use the finding categories and architectural vocabulary
  defined in impl-hygiene.md (LEAK, DRIFT, GAP, WASTE, etc.).

  1. CLAUDE.md (project root)
  2. .claude/rules/impl-hygiene.md
  3. .claude/rules/tests.md
  4. <any other .claude/rules/*.md relevant to the files under review>

  ## Scope: <scope hint — e.g. "HEAD~5..HEAD", a plan section name, or explicit files>

  <evidence packet: what changed, why, what to look for>
  PROMPT

  cat > "$RUN/gemini.prompt.md" <<'PROMPT'
  Activate the review-work skill and follow its instructions exactly.
  Emit the JSON envelope per .claude/skills/dual-tpr/findings-schema.json;
  do NOT write findings to plan files.

  ## Grounding — read these files FIRST before reviewing

  Before you look at any of the changed code, read these files in full
  so your findings are scoped to the project's actual rules. Every
  finding must use the finding categories and architectural vocabulary
  defined in impl-hygiene.md (LEAK, DRIFT, GAP, WASTE, etc.).

  1. CLAUDE.md (project root)
  2. .claude/rules/impl-hygiene.md
  3. .claude/rules/tests.md
  4. <any other .claude/rules/*.md relevant to the files under review>

  ## Scope: <same scope hint>

  <evidence packet: same>
  PROMPT
```

The evidence packet is INFORMATIONAL, not authoritative — reviewers expand scope as they see fit. The GROUNDING block, in contrast, is AUTHORITATIVE — reviewers that skip it produce noise and their envelopes should be treated with extra scrutiny.

### 3. Invoke the dual-source transport in the background

The transport launches both reviewers in parallel, handles infra retries (3 per reviewer, exponential backoff 1s / 2s / 4s), runs the schema validators, and applies the dirty-worktree guard. A full round typically takes 5-15 minutes — BOTH reviewers running concurrently, so wall time is roughly `max(codex_walltime, gemini_walltime)`, not the sum.

Running the transport in the Bash foreground either hits the 2-minute tool timeout or gets auto-backgrounded with output truncated. Always use `run_in_background: true`. The `.claude/hooks/block-banned-commands.sh` hook explicitly allows backgrounded codex and gemini commands.

```
Bash (run_in_background: true):
  .claude/skills/dual-tpr/scripts/dual-invoke-with-retry.sh \
    --run "$RUN" \
    --skill review-work \
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

### Polling Protocol — 5-Minute Status Snapshots (MANDATORY)

**After launching, do NOT go silent.** The operator needs real-time visibility into what each reviewer is doing during the 5–35 minute wait. Poll `status-check.sh` every 5 minutes and produce a brief status update.

```
Bash (foreground, wall-clock 5-min interval):
  sleep 300 && .claude/skills/dual-tpr/scripts/status-check.sh "$RUN" --events 5
```

The status script is SAFE to call because it reads ONLY append-only artifacts (`round.log`, `codex.jsonl`, `gemini.jsonl`) and existence-checks the atomic-write files (`*.exit`, `*.envelope.json`, `*.parse-error`) without reading their contents mid-stream. It surfaces:

- Local timestamp (so the operator knows WHEN the poll landed)
- Round log tail (orchestration progress)
- Per-reviewer subshell state (running / done with rc + walltime)
- Last N streamed events per reviewer — the actual commands, reasoning, tool calls, and messages each reviewer is producing RIGHT NOW
- Envelope parse state / parse-error state
- Worktree state

Keep polling every 5 minutes until the background-task completion notification arrives. The notification is the authoritative done-signal; the status script is for situational awareness in between.

**The notification's exit code is authoritative** — the transport exits 0 on success, non-zero with `infra_retries_exhausted: <category>` on failure. Do not infer success from the absence of an error in stdout; check the notification's exit code.

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
3. **Trace the reasoning** — if the finding says "X is unreachable" / "Y is broken" / "Z is missing", prove it by walking the code yourself. Grep for the symbol, follow the call chain, check the test coverage.
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

**IMPORTANT: Err on the side of "actionable"** (after verification). The following are ALWAYS actionable:
- Dead code paths (code that can never execute)
- Precision regressions (over-approximation that loses optimization opportunities)
- Missing tests for plumbed-through data
- Name collisions or aliasing that cause incorrect behavior
- Pipeline gaps where data is computed but never consumed

**Agreement is a priority signal, not a filter.** When an entry has `agreement: true`, both reviewers independently flagged the same `(location, title)` — the strongest possible signal, so prioritize these fixes. When an entry is tagged `-codex` or `-gemini` only (`agreement: false`), the finding is STILL real after verification — provenance is not severity. Single-reviewer findings get fixed just like agreement findings.

**Agreement is not a substitute for verification.** Two reviewers can be wrong about the same thing — agreement amplifies the hypothesis but doesn't prove it. Verify the claim against the code even when both reviewers flagged it.

### 6. If Zero Actionable Findings -> Clean Pass (EXIT)

Report to the user:
- "Dual-source TPR review passed clean — both reviewers returned zero actionable findings."
- Iteration count (e.g. "clean on iteration 1" or "clean on iteration 3 after fixing N findings").
- Merge summary from the final iteration (`codex_findings`, `gemini_findings`, `agreements`).
- **This is the ONLY clean exit from the loop.**

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

### 8. After Max Iterations (10) — User Escalation

If after 10 semantic iterations findings are still surfacing, surface the remaining merged findings to the user via `AskUserQuestion`:
- Summary of semantic iterations run
- Count of findings per iteration (shows whether progress is being made)
- The current merged finding list (from the latest `$RUN/merged.json`)
- Ask: should we continue past the 10-iteration cap, file remaining findings and stop, or dig into a specific finding that keeps recurring?

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
| `dirty_worktree` | `worktree-guard.sh compare` detected tracked-file modifications during the reviewer run on all 3 attempts. The reviewer violated its read-only contract. |
| `unknown_failure` | Fallback — the script exhausted retries without recording a specific category (rare; investigate round.log) |

### Escalation procedure

When the transport fails, surface the failure to the user via `AskUserQuestion` with:

1. **Failure category** — the literal string from the transport stderr tail, including suffix if any
2. **Postmortem directory** — the `$RUN` path so the user can inspect it directly
3. **Files to inspect in `$RUN`:**
   - `round.log` — orchestration timeline (every attempt, every backoff, every failure category)
   - `codex.jsonl` / `gemini.jsonl` — raw reviewer output streams (may be empty if launch failed)
   - `codex.envelope.json` / `gemini.envelope.json` — parsed envelopes (absent if parse failed)
   - `codex.parse-error` / `gemini.parse-error` — parser error output (first line = failure reason)
   - `worktree-error` — diff of tracked files modified during the reviewer run (present only on `dirty_worktree`)
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
- Total semantic iterations run
- For each iteration: merged summary (`codex_findings` / `gemini_findings` / `agreements`)
- Findings surfaced and fixed per iteration
- Final status: `clean`, `max iterations reached with N remaining findings`, or `aborted due to transport failure`
- Where each finding was filed (plan TPR section or bug-tracker)
