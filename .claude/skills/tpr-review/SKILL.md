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

Write both prompts to the scratch dir:

```
Bash:
  cat > "$RUN/codex.prompt.md" <<'PROMPT'
  Run the /review-work skill in envelope-only mode. Emit the JSON
  envelope per .claude/skills/dual-tpr/findings-schema.json; do NOT
  write findings to plan files.

  Scope: <scope hint — e.g. "HEAD~5..HEAD", a plan section name, or explicit files>

  <evidence packet: what changed, why, what to look for>
  PROMPT

  cat > "$RUN/gemini.prompt.md" <<'PROMPT'
  Activate the review-work skill and follow its instructions exactly.
  Emit the JSON envelope per .claude/skills/dual-tpr/findings-schema.json;
  do NOT write findings to plan files.

  Scope: <same scope hint>

  <evidence packet: same>
  PROMPT
```

The evidence packet is INFORMATIONAL, not authoritative — reviewers expand scope as they see fit.

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
  echo "transport_exit=$?"
```

**DO NOT:**
- Run the transport in the Bash foreground.
- Set a `timeout:` parameter on the Bash call.
- Wrap the transport in an Agent subagent — the subagent cannot itself be backgrounded, so it reintroduces the foreground cap.
- Poll `$RUN/*.jsonl` or `$RUN/*.envelope.json` in a sleep loop — wait for the completion notification.

After launching, continue with other work or wait idle. You will receive a completion notification when the background task finishes.

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

### 5. Classify merged findings

For each entry, determine if the underlying finding is actionable:

- **Actionable finding**: real code issue — bug, hygiene violation, missing test, incorrect behavior, file size limit exceeded, precision regression, dead code path, etc. Must be fixed.
- **Non-actionable observation**: style preference or observation that isn't a defect, precision loss, or dead code. Note it but don't block the loop on it.

**IMPORTANT: Err on the side of "actionable".** The following are ALWAYS actionable:
- Dead code paths (code that can never execute)
- Precision regressions (over-approximation that loses optimization opportunities)
- Missing tests for plumbed-through data
- Name collisions or aliasing that cause incorrect behavior
- Pipeline gaps where data is computed but never consumed

**Agreement is a priority signal, not a filter.** When an entry has `agreement: true`, both reviewers independently flagged the same `(location, title)` — the strongest possible signal, so prioritize these fixes. When an entry is tagged `-codex` or `-gemini` only (`agreement: false`), the finding is STILL real — provenance is not severity. Single-reviewer findings get fixed just like agreement findings.

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

3. **If no owning plan exists** — file as a bug in `plans/bug-tracker/` under the appropriate subsystem section using the reviewer-tagged IDs.

Subsystem mapping (unchanged from single-source version):
- `ori_parse`/`ori_lexer` -> section-01
- `ori_types` -> section-02
- `ori_eval`/`ori_patterns` -> section-03
- `ori_llvm`/`ori_arc` -> section-04
- `ori_rt` -> section-05
- `library/std`/`ori_registry` -> section-06
- `oric`/`ori_fmt`/`ori_diagnostic` -> section-07
- `docs/`/`.claude/`/`plans/` -> section-08

#### 7b. Fix Each Finding

**YOU (Claude) fix the code.** Actual implementation — not just filing, not scope notes, not rationalizations. CODE CHANGES.

- **Read `.claude/rules/impl-hygiene.md` before fixing** — SSOT, canonical homes, no side logic, phase boundaries. Every fix must be the correct architectural solution.
- Read the affected code and understand the issue
- Identify the **canonical home** for the knowledge/logic involved — the fix must respect it
- Follow TDD when appropriate (failing test -> fix -> test passes)
- Run `timeout 150 ./test-all.sh` after fixes
- **Self-check**: would this fix survive `/impl-hygiene-review`? If it introduces scattered knowledge, duplicated dispatch, or a shadow source of truth, it's wrong — find the proper architectural fix
- Mark the filed findings as `[x]` resolved in the plan with a note referencing the code fix:
  ```md
  - [x] `[TPR-04-001-codex][high]` ...
    Resolved: Fixed on YYYY-MM-DD. [description of CODE fix].
  - [x] `[TPR-04-001-gemini][high]` ...
    Resolved: Fixed on YYYY-MM-DD. Same fix as [TPR-04-001-codex] (agreement).
  ```

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

If `dual-invoke-with-retry.sh` exits non-zero, the transport has exhausted its 3 internal infra retries and the round cannot proceed. Read the failure category and postmortem dir path from the script's stderr, then surface the failure to the user via `AskUserQuestion`. Include the `$RUN` path so the user can inspect the postmortem files (raw JSONL streams, parse errors, worktree diff, round log).

**DO NOT silently retry the semantic loop on infra failure.** The 10-iteration budget is for finding-fixing rounds, not transport failures. Incrementing the semantic counter on a transport failure hides real infrastructure bugs and falsely claims iteration progress.

The full escalation text, file-inspection checklist, and explicit loop state machine are documented in subsection 04.2 of `plans/dual-tpr-gemini/section-04-tpr-review.md`, which augments this file after subsection 04.1 is complete.

## Final Report (After Loop Exits)

Tell the user:
- Total semantic iterations run
- For each iteration: merged summary (`codex_findings` / `gemini_findings` / `agreements`)
- Findings surfaced and fixed per iteration
- Final status: `clean`, `max iterations reached with N remaining findings`, or `aborted due to transport failure`
- Where each finding was filed (plan TPR section or bug-tracker)
