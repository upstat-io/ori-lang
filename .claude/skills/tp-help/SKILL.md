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

### Step 2: Create the Scratch Dir and Snapshot the Worktree

Create a per-run scratch dir via `scratch-dir.sh`. This produces a unique temp directory under `/tmp` that holds the prompt files, JSONL outputs, and worktree snapshots for this run.

**Worktree snapshot (BEFORE — inline worktree-guard START):** In concat mode, `/tp-help` invokes `dual-invoke.sh` DIRECTLY — not through `dual-invoke-with-retry.sh` which is where `worktree-guard.sh` normally composes into the pipeline. So the skill itself is the guardrail. Capture the worktree state BEFORE the dual-source call:

```bash
Bash:
  RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
  git status --porcelain > "$RUN/worktree.before"
  echo "RUN=$RUN" >&2  # so you can reference it in later steps
```

### Step 3: Write Both Reviewer Prompts

**Step 3a — Codex prompt.** Write the full context package (adversarial framing + mandatory grounding + question + files + what you tried + constraints) to `$RUN/codex.prompt.md`. The prompt MUST include THREE blocks before the question, in this exact order: (1) the adversarial consultation framing, (2) the Mandatory Grounding Block instructing codex to read CLAUDE.md and the project rules FIRST, and (3) the question context.

**Why these blocks are non-negotiable:**
- **Adversarial framing** — same pattern as `.claude/commands/review-work.md:11-14`, `.claude/skills/dual-tpr/command-file.md`, and `.codex/skills/review-work/SKILL.md:12-16`. Without it, codex answers as a neutral generic assistant and produces smoothed responses instead of the sharp critique that justifies asking for a second opinion.
- **Mandatory Grounding Block** — same pattern as `/tpr-review` SKILL.md §"Mandatory Grounding Block". Without it, codex answers from general knowledge and produces generic findings instead of project-native vocabulary (LEAK, DRIFT, GAP, WASTE from `impl-hygiene.md`). Reviewers that skip the grounding produce noise; this block exists because prior /tpr-review runs empirically showed that ungrounded findings are systematically weaker.

Codex runs under `--full-auto` with `worktree-guard.sh` catching any drift (BUG-08-002 enforces it even in concat mode — the inline snapshot/compare in this skill is the guard), so there is no need for a read-only HARD RULES block on the codex side.

```
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
1. **HARD RULES preamble** — read-only enforcement (codex doesn't need this because `--full-auto` + `worktree-guard.sh` enforces it at the launcher layer)
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

### Step 4.5: Polling Protocol — 5-Minute Status Snapshots (MANDATORY)

**After launching, do NOT go silent.** The operator needs real-time visibility into what each reviewer is doing during the wait (which may span ~20 seconds for a simple prompt or up to 25+ minutes for a deep code-review prompt). Poll `status-check.sh` every 5 minutes and produce a brief status update. This is the SAME polling protocol used by `/tpr-review` and `/review-work`.

```
Bash (foreground, wall-clock 5-min interval):
  sleep 300 && .claude/skills/dual-tpr/scripts/status-check.sh "$RUN" --events 5
```

The status script is SAFE to call because it reads ONLY append-only artifacts (`round.log`, `codex.jsonl`, `gemini.jsonl`) and existence-checks the atomic-write files (`*.exit`, `*.parse-error`) without reading their contents mid-stream. It surfaces:

- Local timestamp (so the operator knows WHEN the poll landed)
- Round log tail (orchestration progress)
- Per-reviewer subshell state (running / done with rc + walltime)
- Last N streamed events per reviewer — the actual commands, reasoning, tool calls, and messages each reviewer is producing RIGHT NOW

**Use BOTH the notification and polling together — neither alone is sufficient:**

- The **background-task completion notification** is the authoritative done-signal and delivers the authoritative exit code. Do NOT infer success from the absence of an error in stdout — check the notification's exit code.
- The **polling protocol** is for situational awareness in between. If you only wait for the notification, the operator has zero visibility for the duration of the wait; if a reviewer is stuck on a tool call or running in a loop, polling surfaces it immediately. If you only poll, you may miss the authoritative exit code and act on a partial file.

Keep polling every 5 minutes until the background-task completion notification arrives. Then stop polling and move to Step 5.

**Polling hygiene (SAFE vs UNSAFE):**

- SAFE to read during a poll: `round.log`, `codex.jsonl`, `gemini.jsonl`, `worktree.before` (append-only / immutable once written)
- UNSAFE to read during a poll: `codex.exit` / `gemini.exit` / `codex.walltime` / `gemini.walltime` (atomic-write — you may see an empty or partially-written file if the subshell is mid-write). `status-check.sh` handles these safely via existence checks.

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

### Step 6: Worktree-Guard Snapshot (AFTER) + Diff

Snapshot the worktree state AGAIN after the dual-source call and diff it against the BEFORE snapshot. If they differ, at least one reviewer violated prompt discipline by modifying a tracked file.

```
Bash:
  git status --porcelain > "$RUN/worktree.after"
  if ! diff -u "$RUN/worktree.before" "$RUN/worktree.after"; then
    echo "WORKTREE DRIFT DETECTED — at least one reviewer modified the working tree" >&2
    echo "Before: $RUN/worktree.before" >&2
    echo "After:  $RUN/worktree.after" >&2
    # Do NOT auto-revert. Surface the diff to the user and let them decide.
  fi
```

This inline guard catches the "gemini ignored the read-only-reviewer preamble" failure mode exactly one layer above the launcher. It mirrors §02's `worktree-guard.sh` snapshot/compare pattern but is inlined into the skill because `/tp-help` skips the retry wrapper (which is where `worktree-guard.sh` normally composes into the pipeline).

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
