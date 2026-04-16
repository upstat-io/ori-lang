# /tp-help Workflow — Sub-Agent Reference Document

This file is read by the Sonnet sub-agent dispatched from SKILL.md. It contains the full dual-source orchestration protocol (Steps 1-7). Steps 8-9 (Apply the Answer, Brief the User) run in the parent's context after this sub-agent returns.

**Mode:** Concatenation mode — NOT the findings envelope schema used by `/tpr-review` and `/review-work`. The output is **both reviewers' raw responses concatenated with HTML-comment attribution sentinels**, not a merged findings list.

## Model Policy

**This workflow runs end-to-end on Sonnet.** The Claude-side work is pure orchestration — the "brains" are the external codex + gemini CLIs, and the contract is to return **both raw responses concatenated** with no synthesis. There is no triage, accept/reject, or code-writing step inside this workflow.

### Heuristic

**Opus for judgment-writing; Sonnet for mechanical-writing and orchestration.**

- **Judgment-writing** (Opus-only) = the output depends on a decision made in the same step: architecture synthesis, accept/reject triage of reviewer findings, fix implementation where content is not predetermined.
- **Mechanical-writing** (Sonnet-safe) = the output is determined by a decision already made elsewhere: expanding a template, filing by a static routing rule, flipping a boolean frontmatter field, reformatting parser output.
- **Orchestration** (Sonnet-safe) = shell launches, JSONL parsing, polling, merging envelopes by deterministic rule.

### Phase table

| Phase | Model | Rationale |
|---|---|---|
| Step 1 — Build Context Package | Sonnet | File reads + template assembly |
| Step 2 — Create the Scratch Dir and Snapshot the Worktree | Sonnet | Shell (orchestration) |
| Step 3 — Write Both Reviewer Prompts | Sonnet | Mechanical-writing: static HARD RULES + grounding + adversarial framing; rule files cited, not summarized |
| Step 4 — Launch `dual-invoke.sh` in the Foreground | Sonnet | Shell launch (orchestration) |
| Step 5 — Parse Both Responses with the Raw Parsers | Sonnet | Python parser wrappers (orchestration) |
| Step 6 — Worktree-Guard Compare | Sonnet | Shell diff against SSOT helper (orchestration) |
| Step 7 — Concatenate with HTML-Comment Sentinel Attribution | Sonnet | Mechanical-writing: helper-sourced sentinel format from `tp-help-sentinels.sh` |

## Runtime Budget

Dual-source runs both Codex and Gemini in parallel via a foreground Bash call with a 10-minute timeout (`timeout: 600000`). Codex typically finishes in 1-3 minutes; Gemini may take 10-15 minutes, which can exceed the foreground timeout on complex prompts.

For fast iteration or to avoid timeout on Gemini-heavy runs, restrict to one reviewer via `ORI_TPR_REVIEWERS`:
- `ORI_TPR_REVIEWERS=codex` — codex only (fast, ~1-3 min wall time, always within timeout)
- `ORI_TPR_REVIEWERS=gemini` — gemini only (slow, ~10-15 min wall time, may hit timeout)
- `ORI_TPR_REVIEWERS=both` — default (both reviewers, ~10-15 min wall time)

The escape hatch is honored in `dual-invoke.sh`. All dual-source consumers (`/tpr-review`, `/review-work`, `/review-plan`, `/tp-help`) respect the same env var.

---

## Step 1: Build Context Package

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

## Step 2: Create the Scratch Dir and Snapshot the Worktree

Create a per-run scratch dir via `scratch-dir.sh`. This produces a unique temp directory under `/tmp` that holds the prompt files, JSONL outputs, and worktree snapshots for this run.

**Worktree snapshot (BEFORE — inline worktree-guard START):** In concat mode, `/tp-help` invokes `dual-invoke.sh` DIRECTLY — not through `dual-invoke-with-retry.sh` which is where `worktree-guard.sh` normally composes into the pipeline. So the skill itself is the guardrail. Capture the worktree state BEFORE the dual-source call:

```bash
Bash:
  RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
  git status --porcelain > "$RUN/worktree.before"
  echo "RUN=$RUN" >&2  # so you can reference it in later steps
```

## Step 3: Write Both Reviewer Prompts

**Step 3a — Codex prompt (HARD RULES + adversarial framing + Grounding Block).** Write the full context package to `$RUN/codex.prompt.md`. The prompt MUST include FOUR blocks before the question, in this exact order: (1) the HARD RULES read-only enforcement preamble, (2) the adversarial consultation framing, (3) the static Grounding Block listing rule files the reviewer must read, and (4) the question context. The orchestrator does NOT pre-summarize the rule files — codex reads them directly.

**Why these blocks are non-negotiable:**
- **HARD RULES preamble** — Codex runs under `--full-auto` which gives it unrestricted file-editing authority. The `.codex/skills/tp-help/SKILL.md` file provides skill-level read-only enforcement, but the prompt-level HARD RULES are the belt to the skill-file's suspenders. On 2026-04-09, a `/tp-help` run WITHOUT prompt-level HARD RULES resulted in Codex editing files during a read-only consultation — the worktree guard caught and reverted the drift, but the edit should never have happened. Both layers (skill file + prompt HARD RULES) are now mandatory.
- **Adversarial framing** — Without it, codex answers as a neutral generic assistant and produces smoothed responses instead of the sharp critique that justifies asking for a second opinion.
- **Mandatory Grounding Block** — Without it, codex answers from general knowledge and produces generic findings instead of project-native vocabulary (LEAK, DRIFT, GAP, WASTE from `impl-hygiene.md`).

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

**Step 3b — Gemini prompt (HARD RULES preamble + adversarial framing + Mandatory Grounding Block).** Gemini has NO dedicated `.gemini/skills/tp-help/` file. Without a dedicated skill file, gemini is invoked as a generic assistant under `--approval-mode yolo`, and the prompt text IS the ONLY guardrail.

The gemini prompt MUST begin with FOUR blocks in this exact order, before the question:
1. **HARD RULES preamble** — read-only enforcement
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

Write the full gemini prompt to `$RUN/gemini.prompt.md`. The adversarial framing and Mandatory Grounding Block are IDENTICAL to the codex-side versions — this is intentional SSOT: both reviewers operate under the same posture and the same rules, so their findings are directly comparable.

## Step 4: Launch `dual-invoke.sh` in the Foreground

Launch `dual-invoke.sh` directly (NOT `dual-invoke-with-retry.sh` — concat mode is one-shot; infra failure surfaces directly to the user without retry). Run in the **foreground** with an explicit timeout.

**Do NOT pass `--schema`:** Passing a schema in concat mode would be architecturally misleading (there is no envelope to validate).

```
Bash (foreground, timeout: 600000):
  bash .claude/skills/dual-tpr/scripts/dual-invoke.sh \
    --run "$RUN" \
    --skill tp-help \
    --codex-prompt "$RUN/codex.prompt.md" \
    --gemini-prompt "$RUN/gemini.prompt.md"
```

The Bash tool's `timeout: 600000` (10 minutes) accommodates codex (1-3 min) and most dual-source runs. If Gemini's cold-start pushes past 10 minutes, the call will time out — retry with `ORI_TPR_REVIEWERS=codex` for a faster single-reviewer run, or set `ORI_TPR_REVIEWERS=gemini` for a gemini-only run if codex already returned.

**DO NOT:**
- Use `run_in_background: true` — `/tp-help` runs foreground.
- Wrap dual-invoke in an Agent — the Agent adds no value and costs an extra process.
- Invoke `dual-invoke-with-retry.sh` — the retry wrapper is for envelope-mode consumers.

## Step 5: Parse Both Responses with the Raw Parsers

When the foreground Bash call completes with exit code 0, parse the two JSONL streams using the raw-mode sibling parsers (NOT the envelope parsers):

```
Bash:
  CODEX_RAW=$(.claude/skills/dual-tpr/scripts/parse-codex-raw.py --jsonl "$RUN/codex.jsonl" 2>&1) \
    || { echo "codex parse failed: $CODEX_RAW" >&2; CODEX_RAW="(codex response unavailable — see $RUN/codex.jsonl for raw stream)"; }
  GEMINI_RAW=$(.claude/skills/dual-tpr/scripts/parse-gemini-raw.py --jsonl "$RUN/gemini.jsonl" 2>&1) \
    || { echo "gemini parse failed: $GEMINI_RAW" >&2; GEMINI_RAW="(gemini response unavailable — see $RUN/gemini.jsonl for raw stream)"; }
```

If either parser fails, DO NOT drop the partial output — include a placeholder message and let the user see that one side failed. Never silently drop a reviewer.

Per the ORI_TPR_REVIEWERS filter, one of the JSONL files may legitimately be absent. If `ORI_TPR_REVIEWERS=codex` was set, skip the gemini parse step entirely; if `=gemini`, skip the codex parse step entirely.

## Step 6: Worktree-Guard Compare (delegates to SSOT script)

Compare the post-run worktree state against the BEFORE snapshot using the canonical `worktree-guard.sh compare` helper. The helper flags ONLY new modifications that weren't present in BEFORE (reviewer-caused drift), not lines removed from BEFORE (drift cleaned up during the run).

```
Bash:
  if ! .claude/skills/dual-tpr/scripts/worktree-guard.sh compare \
       "$RUN/worktree.before" "$RUN/worktree.after"; then
    echo "WORKTREE DRIFT DETECTED — at least one reviewer modified the working tree" >&2
    echo "Before: $RUN/worktree.before" >&2
    echo "After:  $RUN/worktree.after" >&2
  fi
```

This delegates to the **SSOT** `worktree-guard.sh` helper — the same script that `dual-invoke-with-retry.sh` uses at the launcher layer for `/tpr-review` and `/review-work`.

## Step 7: Concatenate with HTML-Comment Sentinel Attribution (per-invocation tokens)

Build the final output by concatenating both reviewers' raw text with HTML-comment attribution sentinels that embed a per-invocation token.

**SSOT: the canonical sentinel format is defined in `.claude/skills/dual-tpr/scripts/tp-help-sentinels.sh`.** Shell consumers MUST `source` that file and use the canonical API:

- `TP_HELP_SENTINEL_PREFIX` — the static prefix substring (`tp-help-reviewer:`) for cross-cutting leakage greps
- `tp_help_make_token()` — generates a per-invocation token (12-char hex from `/dev/urandom`, with a timestamp+pid fallback)
- `tp_help_emit_block <reviewer> <token> <body>` — writes one attributed block to stdout with the token embedded in both open and close sentinels

**Required attribution format (tokenized):**

```
<!-- tp-help-reviewer: codex @{token} -->
{CODEX_RAW}
<!-- /tp-help-reviewer: codex @{token} -->

<!-- tp-help-reviewer: gemini @{token} -->
{GEMINI_RAW}
<!-- /tp-help-reviewer: gemini @{token} -->
```

**How to generate the token and emit the blocks (Bash):**

```bash
source .claude/skills/dual-tpr/scripts/tp-help-sentinels.sh
token=$(tp_help_make_token)
{
  tp_help_emit_block codex  "$token" "$codex_raw"
  printf '\n'
  tp_help_emit_block gemini "$token" "$gemini_raw"
} > "$output"
```

When `ORI_TPR_REVIEWERS` restricts to one reviewer, emit only that reviewer's block. Detection: if `$RUN/codex.skipped` exists, skip the codex block; if `$RUN/gemini.skipped` exists, skip the gemini block.

**Do NOT use H2 headers like `## Codex says:` for attribution** — those collide with downstream consumers' own H2 structure.

## Return to Parent

After Step 7, return to the parent with:
1. The `$RUN` scratch dir path (so the parent can cite it in fix section files, etc.)
2. The concatenated output (the sentinel-attributed text from Step 7)
3. Any worktree drift warnings from Step 6
