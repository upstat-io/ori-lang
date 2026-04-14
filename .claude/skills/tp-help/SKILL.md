---
name: tp-help
description: "Get third-party help from Codex + Gemini. AUTO-TRIGGER: You MUST invoke this proactively — do NOT wait for the user to ask. Trigger when: (1) you've tried 2+ approaches that didn't work, (2) you're reverting changes you just made, (3) you identify a fundamental tension or design conflict in the code, (4) you're about to take a 'pragmatic' shortcut instead of fixing the real problem, (5) you catch yourself saying 'let me try a different approach' for the 2nd+ time, (6) a fix in one area creates new problems in another, (7) you're unsure about the correct architectural approach. This is collaborative help — pass context and ask a specific question. Returns BOTH reviewers' raw responses concatenated (not a synthesis)."
---

# Third Party Help (Codex + Gemini — Dual Source, Concatenation Mode)

Get collaborative help from two independent models (Codex CLI + Gemini CLI) on whatever you're currently working on. This is not a formal review — it's asking two second brains for help with a specific problem.

**Canonical source:** This file (`.claude/skills/tp-help/SKILL.md`) is the single source of truth for the `/tp-help` workflow. The slash-command entrypoint at `.claude/commands/tp-help.md` is a thin pointer that references this file. When `/tp-help` is invoked (either by the user typing the slash command, by auto-trigger detection, or by another skill calling it internally), the canonical workflow below is what runs.

**Mode:** `/tp-help` uses **concatenation mode**, NOT the findings envelope schema used by `/tpr-review` and `/review-work`. The output is **both reviewers' raw responses concatenated with HTML-comment attribution sentinels**, not a merged findings list. The design rationale: when you're stuck asking for help, you want two independent perspectives — not a smoothed editorial synthesis that hides useful disagreement between the models.

## MANDATORY AUTO-TRIGGER — Do NOT Wait for User

**You MUST invoke this skill proactively.** Do NOT wait for the user to type `/tp-help`. The whole point is that YOU detect when you need help and ask for it automatically.

### Concrete Trigger Conditions

Invoke `/tp-help` IMMEDIATELY when ANY of these are true:

1. **Multiple failed approaches** — You've tried 2+ approaches to solve the same problem and none worked cleanly
2. **Reverting your own changes** — You're undoing work you just did because it caused new problems
3. **Fundamental tension identified** — You've identified a design conflict where fixing one thing breaks another (e.g., "borrowed-use vs capture-use callees have conflicting RC ownership requirements")
4. **Pragmatic retreat** — You catch yourself about to take a shortcut, partial fix, or "keep just the X part and revert the Y part" instead of solving the real problem
5. **Approach cycling** — You're saying "let me try a different approach" for the 2nd+ time
6. **Fix interference** — A fix in one subsystem creates new failures in another
7. **Architectural uncertainty** — You're unsure which of two+ fundamental approaches is correct (not minor implementation details — real architectural questions)
8. **Stuck > 10 minutes** — You've been working on the same problem for more than ~10 minutes without clear forward progress

### What Does NOT Trigger This

- Simple bugs with obvious fixes
- First attempt at an approach (try it first, ask for help if it fails)
- Questions about Ori syntax or spec (read the spec instead)
- Minor implementation details with clear precedent in the codebase

### Exception — Design Consensus Mode (called by /fix-bug)

The "simple bugs" and "first attempt" non-triggers DO NOT apply when `/tp-help` is invoked by `/fix-bug` at Phase 1.75. In that calling context, `/tp-help` is used for **design consensus** — a pre-emptive pressure-test of a proposed fix approach before tests or implementation are written — NOT for **stuck help**.

Design consensus runs for EVERY bug that reaches `/fix-bug` Phase 1.75 (including trivial one-liners), because what looks trivial often has architectural implications that only surface under independent review. The `/fix-bug` skill is responsible for enforcing this calling contract; `/tp-help` itself runs its normal workflow — the difference is purely in which non-triggers apply to the caller's decision to invoke.

See `.claude/skills/fix-bug/SKILL.md` § Phase 1.75 for the full consensus protocol, the 3-call convergence cap, and autopilot deadlock handling.

### Example Scenario That MUST Trigger Auto-Invoke

> "I've been trying multiple approaches but the pre-call RcInc leaks for borrowed-param closures while fixing capture closures. The RC ownership model for ApplyIndirect has a fundamental tension between borrowed-use and capture-use callees. Let me take the pragmatic approach: keep just the drop_hints fix and revert the AIMS-level RcInc."

This hits triggers #1 (multiple approaches), #3 (fundamental tension), #4 (pragmatic retreat), and #2 (reverting). You should have invoked `/tp-help` BEFORE reaching the "let me take the pragmatic approach" conclusion.

## Legacy Trigger List (still valid)

- You're stuck on a bug and can't figure out the root cause
- You're unsure which of two implementation approaches is better
- You just wrote something tricky and want a sanity check
- A test is failing and you can't see why
- You need help understanding unfamiliar code
- You want to validate your reasoning before committing to an approach
- You're about to make a significant architectural decision

## Runtime Budget — Dual-Source is ~10x Slower Than Single-Source

Dual-source `/tp-help` runs both Codex and Gemini in parallel. Wall time is dominated by Gemini (Codex typically finishes in 1-3 minutes; Gemini in 10-15 minutes per call). Total wall time is ~10-15 minutes per invocation.

For fast iteration (e.g., refining a prompt, debugging the tp-help pipeline itself), you can restrict to one reviewer via `ORI_TPR_REVIEWERS`:

- `ORI_TPR_REVIEWERS=codex` — codex only (fast, ~1-3 min wall time)
- `ORI_TPR_REVIEWERS=gemini` — gemini only (slow, ~10-15 min wall time)
- `ORI_TPR_REVIEWERS=both` — default (both reviewers, ~10-15 min wall time)

The escape hatch is honored in `dual-invoke.sh` (the single SSOT for the runtime toggle). All four dual-source consumers (`/tpr-review`, `/review-work`, `/review-plan`, `/tp-help`) respect the same env var.

## Usage

```
/tp-help [question]
```

Can also be invoked proactively by Claude when it determines outside help would be valuable.

## Workflow

### Step 1: Build Context Package

Gather the relevant context for the question. Be specific — both Codex and Gemini work best with concrete context, not vague requests.

**Always include:**
- The specific question or problem
- The file(s) involved (read them and include key sections)

**Include when relevant:**
- The error message or test failure output
- What you've already tried
- The two approaches you're deciding between
- The spec section that defines expected behavior
- Recent git diff showing what you changed
Additionally, enrich the context packet with intelligence-graph signals. Follow the canonical intel-summary injection protocol:

@.claude/skills/dual-tpr/compose-intel-summary.md

Per SSOT Step F — /tp-help uses `callers`/`callees`/`similar` on the discussed symbols to provide precise cross-file dependency and prior-art context.

### Step 2: Create the Scratch Dir and Snapshot the Worktree

Create a per-run scratch dir via `scratch-dir.sh`. This produces a unique temp directory under `/tmp` that holds the prompt files, JSONL outputs, and worktree snapshots for this run.

**Worktree snapshot (BEFORE — inline worktree-guard START):** In concat mode, `/tp-help` invokes `dual-invoke.sh` DIRECTLY — not through `dual-invoke-with-retry.sh` which is where `worktree-guard.sh` normally composes into the pipeline. So the skill itself is the guardrail. Capture the worktree state BEFORE the dual-source call:

```bash
Bash:
  RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
  git status --porcelain > "$RUN/worktree.before"
  echo "RUN=$RUN" >&2  # so you can reference it in later steps
```

### Step 2.5: Compose the Rules Brief (MANDATORY)

Before writing reviewer prompts, compose a tailored rules brief using the
same dynamic composition pipeline as `/tpr-review`. See `/tpr-review`
SKILL.md Step 1.5 for the full workflow:

1. Run `scripts/rules-for-review.py --files {context_files}` to classify
2. Spawn a Sonnet subagent (`.claude/skills/dual-tpr/compose-rules-brief.md`)
3. Hold the Rules Brief output for injection into both prompts below

For `/tp-help`, the "context files" are the files referenced in the question
context (Step 1's context files), not a git diff. If no files are
identifiable, fall back to the static core (CLAUDE.md + impl-hygiene.md +
tests.md + compiler.md).

### Step 3: Write Both Reviewer Prompts

**Step 3a — Codex prompt (HARD RULES + adversarial framing + Rules Brief).** Write the full context package to `$RUN/codex.prompt.md`. The prompt MUST include FOUR blocks before the question, in this exact order: (1) the HARD RULES read-only enforcement preamble, (2) the adversarial consultation framing, (3) the Rules Brief from Step 2.5 (or fallback grounding block), and (4) the question context.

**Why these blocks are non-negotiable:**
- **HARD RULES preamble** — Codex runs under `--full-auto` which gives it unrestricted file-editing authority. The `.codex/skills/tp-help/SKILL.md` file provides skill-level read-only enforcement, but the prompt-level HARD RULES are the belt to the skill-file's suspenders. On 2026-04-09, a `/tp-help` run WITHOUT prompt-level HARD RULES resulted in Codex editing `section-07-enum-repr.md` and `plan-schema.md` during a read-only consultation — the worktree guard caught and reverted the drift, but the edit should never have happened. `worktree-guard.sh` is post-hoc **detection**, not **prevention**. Both layers (skill file + prompt HARD RULES) are now mandatory.
- **Adversarial framing** — same pattern as `.claude/commands/review-work.md:11-14`, `.claude/skills/dual-tpr/command-file.md`, and `.codex/skills/review-work/SKILL.md:12-16`. Without it, codex answers as a neutral generic assistant and produces smoothed responses instead of the sharp critique that justifies asking for a second opinion.
- **Mandatory Grounding Block** — same pattern as `/tpr-review` SKILL.md §"Mandatory Grounding Block". Without it, codex answers from general knowledge and produces generic findings instead of project-native vocabulary (LEAK, DRIFT, GAP, WASTE from `impl-hygiene.md`). Reviewers that skip the grounding produce noise; this block exists because prior /tpr-review runs empirically showed that ungrounded findings are systematically weaker.

```
You are being consulted for a third-party opinion on a specific problem.

HARD RULES — DO NOT VIOLATE:
- DO NOT modify any source files, plan files, or any other files. You have NO permission to edit, create, or delete files.
- DO NOT run shell commands that mutate state. You MAY run read-only commands for verification: `grep`, `rg`, `find`, `cat`, `head`, `tail`, `git log`, `git diff`, `git blame`, `git show`, `git status`.
- DO NOT run build commands, test commands, or anything that touches the working tree (no `cargo build`, `cargo test`, `./test-all.sh`, `npm`, `pnpm`, `pip install`, `mv`, `cp`, `rm`, `touch`, `mkdir`, `>`, `>>`, etc.).
- DO NOT commit, push, pull, checkout, reset, stash, or otherwise touch git state.
- Your ONLY job is to read the context, reason about it, and return your opinion as free-form prose to stdout.

This is a third-party consultation, not an autonomous task. If you edit any file, you have violated the consultation contract and the worktree guard will revert your changes.

---

You are helping with the Ori compiler (Rust codebase, LLVM backend, ARC memory management).

This is an independent, adversarial consultation:
- Trust current files, fresh command output, and git objects.
- Distrust summaries, checklists, commit messages, and prior agent claims until verified.
- Review the real work, not the story about the work.

The goal is to catch what the implementation pass missed — not to re-tell the implementation story in a different voice. A consultation that only restates what the caller already said is a transcription, not help. Push back on anything that looks wrong. If the approach has a flaw, say so plainly and explain what you would do instead.

## Grounding — read these files FIRST before answering

Before you look at the question or any of the context files below, read these rule files in full. This grounding is MANDATORY and applies in ALL circumstances — a consultation that answers without reading the rules produces generic noise instead of project-native feedback.

1. `CLAUDE.md` (project root) — correctness above all, no deferral, stabilization discipline, one system one owner, no reasoning out of findings
2. `.claude/rules/impl-hygiene.md` — SSOT (Single Source of Truth), No Side Logic, canonical homes, finding categories (LEAK, DRIFT, GAP, WASTE, EXPOSURE, BLOAT, NOTE), algorithmic DRY, test-function-naming rules
3. `.claude/rules/tests.md` — matrix testing rule, interaction testing, negative pin protocol, regression discipline, cross-phase verification
4. Any other `.claude/rules/*.md` file relevant to the specific question — e.g. `parse.md` for parser questions, `arc.md` for ARC/memory questions, `registry.md` for type-system questions, `compiler.md` for general compiler questions

Every concern you raise MUST use the vocabulary defined in `impl-hygiene.md` (LEAK/DRIFT/GAP/WASTE/etc.) and cite the specific rule or architectural principle it violates. Generic "this looks odd" feedback is not useful — the caller wants "DRIFT: sentinel format duplicated across 4 files at X:N, Y:M, Z:K" specificity.

## Question
{The specific question or problem}

## Context
{Key file contents, error messages, diffs — whatever is relevant}

## What I've Tried
{If applicable — what approaches were attempted and why they didn't work}

## Constraints
{Any rules from CLAUDE.md or .claude/rules/ that apply — e.g., "no workarounds, must be architecturally correct"}
```

**Step 3b — Gemini prompt (HARD RULES preamble + adversarial framing + Mandatory Grounding Block).** Gemini has NO dedicated `.gemini/skills/tp-help/` file (unlike `/review-work` and `/review-plan`, which each got a dedicated gemini skill in §03 of the dual-tpr-gemini plan). Without a dedicated skill file, gemini is invoked as a generic assistant under `--approval-mode yolo`, and the prompt text IS the ONLY guardrail for ALL THREE of: prompt discipline (the HARD RULES), adversarial posture (the framing), and rules grounding (the grounding block).

The gemini prompt MUST begin with FOUR blocks in this exact order, before the question:
1. **HARD RULES preamble** — read-only enforcement (both codex and gemini get this — codex also has `.codex/skills/tp-help/SKILL.md` as a second layer)
2. **Adversarial consultation framing** — identical to Step 3a's framing (intentional SSOT symmetry)
3. **Mandatory Grounding Block** — identical to Step 3a's grounding block (intentional SSOT symmetry)
4. **Question context** — question + context + what I tried + constraints

```
You are being consulted for a third-party opinion on a specific problem.

HARD RULES — DO NOT VIOLATE:
- DO NOT modify any source files. You have NO permission to edit, create, or delete files.
- DO NOT run shell commands that mutate state. You MAY run read-only commands for verification: `grep`, `rg`, `find`, `cat`, `head`, `tail`, `git log`, `git diff`, `git blame`, `git show`, `git status`.
- DO NOT run build commands, test commands, or anything that touches the working tree (no `cargo build`, `cargo test`, `./test-all.sh`, `npm`, `pnpm`, `pip install`, `mv`, `cp`, `rm`, `touch`, `mkdir`, `>`, `>>`, etc.).
- DO NOT commit, push, pull, checkout, reset, stash, or otherwise touch git state.
- Your ONLY job is to read the context, reason about it, and return your opinion as free-form prose to stdout.

This is a third-party consultation, not an autonomous task. Prompt discipline violations are tracked.

---

You are helping with the Ori compiler (Rust codebase, LLVM backend, ARC memory management).

This is an independent, adversarial consultation:
- Trust current files, fresh command output, and git objects.
- Distrust summaries, checklists, commit messages, and prior agent claims until verified.
- Review the real work, not the story about the work.

The goal is to catch what the implementation pass missed — not to re-tell the implementation story in a different voice. A consultation that only restates what the caller already said is a transcription, not help. Push back on anything that looks wrong. If the approach has a flaw, say so plainly and explain what you would do instead.

## Grounding — read these files FIRST before answering

Before you look at the question or any of the context files below, read these rule files in full. This grounding is MANDATORY and applies in ALL circumstances — a consultation that answers without reading the rules produces generic noise instead of project-native feedback.

1. `CLAUDE.md` (project root) — correctness above all, no deferral, stabilization discipline, one system one owner, no reasoning out of findings
2. `.claude/rules/impl-hygiene.md` — SSOT (Single Source of Truth), No Side Logic, canonical homes, finding categories (LEAK, DRIFT, GAP, WASTE, EXPOSURE, BLOAT, NOTE), algorithmic DRY, test-function-naming rules
3. `.claude/rules/tests.md` — matrix testing rule, interaction testing, negative pin protocol, regression discipline, cross-phase verification
4. Any other `.claude/rules/*.md` file relevant to the specific question — e.g. `parse.md` for parser questions, `arc.md` for ARC/memory questions, `registry.md` for type-system questions, `compiler.md` for general compiler questions

Every concern you raise MUST use the vocabulary defined in `impl-hygiene.md` (LEAK/DRIFT/GAP/WASTE/etc.) and cite the specific rule or architectural principle it violates. Generic "this looks odd" feedback is not useful — the caller wants "DRIFT: sentinel format duplicated across 4 files at X:N, Y:M, Z:K" specificity.

## Question
{The specific question or problem}

## Context
{Key file contents, error messages, diffs — whatever is relevant}

## What I've Tried
{If applicable — what approaches were attempted and why they didn't work}

## Constraints
{Any rules from CLAUDE.md or .claude/rules/ that apply — e.g., "no workarounds, must be architecturally correct"}
```

Write the full gemini prompt to `$RUN/gemini.prompt.md`. The adversarial framing and Mandatory Grounding Block are IDENTICAL to the codex-side versions in Step 3a — this is intentional SSOT: both reviewers operate under the same posture and the same rules, so their findings are directly comparable. The only difference between the two prompts is the HARD RULES preamble on the gemini side (gemini runs as a generic assistant and needs explicit read-only enforcement; codex runs under `--full-auto` and has `worktree-guard.sh` as its enforcement layer).

### Step 4: Launch `dual-invoke.sh` in the Background

Dual-source `/tp-help` calls can take anywhere from ~20 seconds (simple smoke-test prompts) up to 25+ minutes (deep code review prompts where gemini has to read many files). The upper bound dominates for real /tp-help usage — the auto-trigger conditions (stuck > 10 min, multiple failed approaches, fundamental tension) virtually always involve complex prompts that exercise gemini's full wall time. Bash's 2-minute foreground default will kill or auto-background the call long before gemini finishes. Launch `dual-invoke.sh` directly (NOT `dual-invoke-with-retry.sh` — concat mode is one-shot; infra failure surfaces directly to the user without retry), and use `run_in_background: true`.

**Do NOT pass `--schema`:** §07.0 of the dual-tpr-gemini plan made the flag optional. Passing a schema in concat mode would be architecturally misleading (there is no envelope to validate).

**Do NOT add a trailing `echo` after `dual-invoke.sh`:** BUG-08-007 regression — the background task's reported exit code is the exit code of the LAST executed command, so any trailing `echo "exit=$?"` masks the transport's real failure. Let `dual-invoke.sh` be the last command in the invocation.

```
Bash (run_in_background: true):
  rm -f "$RUN/done"
  bash .claude/skills/dual-tpr/scripts/dual-invoke.sh \
    --run "$RUN" \
    --skill tp-help \
    --codex-prompt "$RUN/codex.prompt.md" \
    --gemini-prompt "$RUN/gemini.prompt.md"
```

The `.claude/hooks/block-banned-commands.sh` hook explicitly allows `run_in_background: true` on codex and gemini. Backgrounding is the preferred path because it has no timeout cap; the harness will notify you when dual-invoke finishes.

**DO NOT:**
- Run `dual-invoke.sh` in the Bash foreground without `run_in_background: true` (will hit the 2-minute default or get auto-backgrounded; either way output may be truncated).
- Set a short `timeout:` parameter on the Bash call (the hook blocks short timeouts on codex/gemini commands; backgrounding sidesteps this entirely).
- Wrap dual-invoke in an Agent — the Agent adds no value and costs an extra process.
- Invoke `dual-invoke-with-retry.sh` — the retry wrapper is for envelope-mode consumers that need parse-level validation; concat mode has no envelope to validate, so retries would just duplicate the raw responses.

### Step 4.5: Polling Protocol — Canonical SSOT

**Protocol lives in `.claude/skills/dual-tpr/polling-protocol.md` — `@`-included below. Follow it verbatim.**

`/tp-help`, `/tpr-review`, `/review-work`, and any future dual-source consumer share a single canonical polling protocol. It lives in one file and is expanded here via `@`-include so updates propagate automatically. Prior to 2026-04-08, each skill inlined its own copy — they drifted (tpr-review + review-work used identical text, tp-help had slight wording drift) and produced poor real-time visibility (silent 5-min periods from `sleep 300` backgrounded polls, relative "T+N min" timestamps without absolute anchors). Consolidation into `polling-protocol.md` is the SSOT fix per `impl-hygiene.md` §SSOT / §Algorithmic DRY.

@.claude/skills/dual-tpr/polling-protocol.md

**After the protocol above**, move to Step 5 (parse responses with the raw parsers).

### Step 5: Parse Both Responses with the Raw Parsers

When the background-task completion notification arrives AND the reported exit code is 0, parse the two JSONL streams using the raw-mode sibling parsers (NOT the envelope parsers):

```
Bash:
  CODEX_RAW=$(.claude/skills/dual-tpr/scripts/parse-codex-raw.py --jsonl "$RUN/codex.jsonl" 2>&1) \
    || { echo "codex parse failed: $CODEX_RAW" >&2; CODEX_RAW="(codex response unavailable — see $RUN/codex.jsonl for raw stream)"; }
  GEMINI_RAW=$(.claude/skills/dual-tpr/scripts/parse-gemini-raw.py --jsonl "$RUN/gemini.jsonl" 2>&1) \
    || { echo "gemini parse failed: $GEMINI_RAW" >&2; GEMINI_RAW="(gemini response unavailable — see $RUN/gemini.jsonl for raw stream)"; }
```

If either parser fails, DO NOT drop the partial output — include a placeholder message and let the user see that one side failed. Never silently drop a reviewer.

Per the ORI_TPR_REVIEWERS filter (Step 4's env), one of the JSONL files may legitimately be absent. If `ORI_TPR_REVIEWERS=codex` was set, skip the gemini parse step entirely; if `=gemini`, skip the codex parse step entirely. The skill file consumers (Claude) should check the env var before attempting to parse.

### Step 6: Worktree-Guard Compare (delegates to SSOT script)

Compare the post-run worktree state against the BEFORE snapshot using the canonical `worktree-guard.sh compare` helper. The helper has **correct drift-detection semantics**: it flags ONLY new modifications that weren't present in BEFORE (i.e., reviewer-caused drift), not lines removed from BEFORE (which would indicate drift was cleaned up during the run — legitimate workflow, not a violation). The AFTER snapshot is saved to `$RUN/worktree.after` via the script's optional second argument so it persists as a run artifact.

```
Bash:
  if ! .claude/skills/dual-tpr/scripts/worktree-guard.sh compare \
       "$RUN/worktree.before" "$RUN/worktree.after"; then
    echo "WORKTREE DRIFT DETECTED — at least one reviewer modified the working tree" >&2
    echo "Before: $RUN/worktree.before" >&2
    echo "After:  $RUN/worktree.after" >&2
    # Do NOT auto-revert. The script's stderr output above shows the NEW drift
    # (lines in AFTER not present in BEFORE). Surface it to the user and let
    # them decide how to clean up.
  fi
```

This delegates to the **SSOT** `worktree-guard.sh` helper — the same script that `dual-invoke-with-retry.sh` uses at the launcher layer for `/tpr-review` and `/review-work`. Before 2026-04-08, `/tp-help` had its own inline `diff -u "$BEFORE" "$AFTER"` check that flagged ANY difference (including drift CLEANED UP during the run — e.g., Claude committing pre-existing uncommitted edits mid-run produced a false positive). Surfaced during `plans/dual-tpr-gemini` §07.3 Scenario 1 execution. The fix: replace the inline check with a call to the SSOT script, and fix the script's `compare` mode to use `comm -13 <(sort BEFORE) <(sort AFTER)` — lines unique to AFTER = new drift. See `worktree-guard.sh` for the full corrected semantics.

This guard catches the "gemini ignored the read-only-reviewer preamble" failure mode at the skill layer, one level above the launcher. `/tp-help` is the only dual-source consumer that needs an explicit skill-level call because it bypasses `dual-invoke-with-retry.sh` (concat mode is one-shot; no retry wrapper → the launcher-layer guard is skipped → the skill layer IS the guard).

### Step 7: Concatenate with HTML-Comment Sentinel Attribution (per-invocation tokens)

Build the final output by concatenating both reviewers' raw text with HTML-comment attribution sentinels that embed a per-invocation token. HTML comments are invisible to Markdown renderers but CANNOT collide with any Markdown header level (H1/H2/H3/...) — downstream consumers (impl-hygiene-review, review-plan, create-plan) can safely render or re-paste the text without attribution leaking into their own Markdown structure, yet text-search tooling can still locate the boundaries unambiguously.

**SSOT: the canonical sentinel format is defined in `.claude/skills/dual-tpr/scripts/tp-help-sentinels.sh`.** Shell consumers (e.g., `validate-tp-help-consumers.sh`, future harnesses) MUST `source` that file and use the canonical API:

- `TP_HELP_SENTINEL_PREFIX` — the static prefix substring (`tp-help-reviewer:`) for cross-cutting leakage greps
- `tp_help_make_token()` — generates a per-invocation token (12-char hex from `/dev/urandom`, with a timestamp+pid fallback)
- `tp_help_emit_block <reviewer> <token> <body>` — writes one attributed block to stdout with the token embedded in both open and close sentinels

**Per-invocation tokens resolve the §07.3 TPR v2 EXPOSURE finding** (sentinel spoofing): a reviewer whose prose quotes the literal sentinel (e.g., when explaining the `/tp-help` format itself) would, under the original static format, produce a concat output with multiple "close" markers, and a naive consumer doing greedy open-to-close matching would prematurely terminate the block. The per-invocation token makes the fresh run's sentinels unambiguously distinguishable from any stale or quoted sentinel in the body — the token is generated AFTER the reviewer prompts are sent, so the reviewer cannot pre-guess it. Consumers pair open/close by token, which skips over any stale-token sentinels inside the body.

**Required attribution format (tokenized; canonically defined in `tp-help-sentinels.sh`):**

```
<!-- tp-help-reviewer: codex @{token} -->
{CODEX_RAW}
<!-- /tp-help-reviewer: codex @{token} -->

<!-- tp-help-reviewer: gemini @{token} -->
{GEMINI_RAW}
<!-- /tp-help-reviewer: gemini @{token} -->
```

The `{token}` is the SAME value for both codex and gemini blocks within a single run — both are siblings of the same invocation. Different runs get different tokens. Example with a concrete token:

```
<!-- tp-help-reviewer: codex @a7b3c5d9e1f2 -->
...codex response text...
<!-- /tp-help-reviewer: codex @a7b3c5d9e1f2 -->

<!-- tp-help-reviewer: gemini @a7b3c5d9e1f2 -->
...gemini response text...
<!-- /tp-help-reviewer: gemini @a7b3c5d9e1f2 -->
```

**How to generate the token and emit the blocks (Bash):**

```bash
# Source the canonical SSOT helper
source .claude/skills/dual-tpr/scripts/tp-help-sentinels.sh

# Generate ONE token for this invocation (shared by both reviewers)
token=$(tp_help_make_token)

# Emit each block via the helper — do NOT construct sentinel strings by hand
{
  tp_help_emit_block codex  "$token" "$codex_raw"
  printf '\n'
  tp_help_emit_block gemini "$token" "$gemini_raw"
} > "$output"
```

**If the format ever needs to change**, update `tp-help-sentinels.sh` FIRST, then mirror the change here. The SSOT-helper-first rule exists because hardcoding the format in multiple files is a DRIFT violation per `impl-hygiene.md` §SSOT; §07.3 TPR surfaced exactly this drift (four files carried the same sentinel literal) and the helper was introduced to prevent recurrence.

When `ORI_TPR_REVIEWERS` restricts to one reviewer, emit only that reviewer's block — do not emit an empty block for the skipped reviewer. Detection: if `$RUN/codex.skipped` exists, skip the codex block; if `$RUN/gemini.skipped` exists, skip the gemini block. (The `.skipped` marker files are written by `dual-invoke.sh` — see the `.skipped` handling in Step 4 / `status-check.sh`.)

**Do NOT use H2 headers like `## Codex says:` for attribution** — those collide with downstream consumers' own H2 structure and can cause their Markdown renderers to misinterpret the boundary. The tokenized sentinel format is the authoritative machine-readable attribution; consumers that want human-visible labels MAY add a single prose line immediately after the opening sentinel (e.g., `**Codex says:**`), but the sentinels themselves are load-bearing.

### Step 8: Apply the Answer

- If the two reviewers AGREE, that's strong evidence — evaluate the shared recommendation against CLAUDE.md rules before applying
- If the two reviewers DISAGREE, read both perspectives carefully — the disagreement often surfaces the real tradeoff
- If Codex found something Gemini missed (or vice versa), incorporate the insight
- If both disagree with your approach, present both perspectives to the user alongside your own analysis

**Do NOT blindly apply either reviewer's suggestions.** You have full project context that neither Codex nor Gemini has — use your judgment to filter, combine, and adapt.

### Step 9: Brief the User

Tell the user:
- What you asked the reviewers
- What each reviewer said (brief summary per reviewer — preserve the "two independent perspectives" character)
- Where they agreed, where they disagreed
- How you're applying it (or why you're not)
