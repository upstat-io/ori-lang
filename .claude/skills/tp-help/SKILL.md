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

**Step 3a — Codex prompt.** Write the full context package (question + files + what you tried + constraints) to `$RUN/codex.prompt.md`. The codex prompt has no special preamble because codex runs under `--full-auto` with `worktree-guard.sh` catching any drift (BUG-08-002 enforces it even in concat mode — the inline snapshot/compare in this skill is the guard).

```
You are helping with the Ori compiler (Rust codebase, LLVM backend, ARC memory management).

## Question
{The specific question or problem}

## Context
{Key file contents, error messages, diffs — whatever is relevant}

## What I've Tried
{If applicable — what approaches were attempted and why they didn't work}

## Constraints
{Any rules from CLAUDE.md or .claude/rules/ that apply — e.g., "no workarounds, must be architecturally correct"}
```

**Step 3b — Gemini prompt (MANDATORY read-only-reviewer preamble).** Gemini has NO dedicated `.gemini/skills/tp-help/` file (unlike `/review-work` and `/review-plan`, which each got a dedicated gemini skill in §03 of the dual-tpr-gemini plan). Without a dedicated skill file, gemini is invoked as a generic assistant under `--approval-mode yolo`, and the prompt text IS the ONLY guardrail. The gemini prompt MUST begin with this preamble, verbatim:

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
```

After the preamble, include the same context package (question + files + what you tried + constraints) that the codex prompt received. Write the full gemini prompt to `$RUN/gemini.prompt.md`.

### Step 4: Launch `dual-invoke.sh` in the Background

Dual-source reviews legitimately take 10-15 minutes. Bash's 2-minute foreground default will kill or auto-background the call. Launch `dual-invoke.sh` directly (NOT `dual-invoke-with-retry.sh` — concat mode is one-shot; infra failure surfaces directly to the user without retry), and use `run_in_background: true`.

**Do NOT pass `--schema`:** §07.0 of the dual-tpr-gemini plan made the flag optional. Passing a schema in concat mode would be architecturally misleading (there is no envelope to validate).

```
Bash (run_in_background: true):
  rm -f "$RUN/done"
  bash .claude/skills/dual-tpr/scripts/dual-invoke.sh \
    --run "$RUN" \
    --skill tp-help \
    --codex-prompt "$RUN/codex.prompt.md" \
    --gemini-prompt "$RUN/gemini.prompt.md"
  ec=$?
  touch "$RUN/done"
  echo "dual-invoke exit=$ec"
```

The `.claude/hooks/block-banned-commands.sh` hook explicitly allows `run_in_background: true` on codex and gemini. Backgrounding is the preferred path because it has no timeout cap; the harness will notify you when dual-invoke finishes.

**DO NOT:**
- Run `dual-invoke.sh` in the Bash foreground without `run_in_background: true` (will hit the 2-minute default or get auto-backgrounded; either way output may be truncated).
- Set a short `timeout:` parameter on the Bash call (the hook blocks short timeouts on codex/gemini commands; backgrounding sidesteps this entirely).
- Wrap dual-invoke in an Agent — the Agent adds no value and costs an extra process.
- Invoke `dual-invoke-with-retry.sh` — the retry wrapper is for envelope-mode consumers that need parse-level validation; concat mode has no envelope to validate, so retries would just duplicate the raw responses.

### Step 5: Parse Both Responses with the Raw Parsers

When the harness notifies you the background job completed, parse the two JSONL streams using the raw-mode sibling parsers (NOT the envelope parsers):

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

### Step 7: Concatenate with HTML-Comment Sentinel Attribution

Build the final output by concatenating both reviewers' raw text with HTML-comment attribution sentinels. HTML comments are invisible to Markdown renderers but CANNOT collide with any Markdown header level (H1/H2/H3/...) — downstream consumers (impl-hygiene-review, review-plan, create-plan) can safely render or re-paste the text without attribution leaking into their own Markdown structure, yet text-search tooling can still locate the boundaries unambiguously.

**Required attribution format (exact strings — DO NOT invent alternatives):**

```
<!-- tp-help-reviewer: codex -->
{CODEX_RAW}
<!-- /tp-help-reviewer: codex -->

<!-- tp-help-reviewer: gemini -->
{GEMINI_RAW}
<!-- /tp-help-reviewer: gemini -->
```

When `ORI_TPR_REVIEWERS` restricts to one reviewer, emit only that reviewer's block — do not emit an empty block for the skipped reviewer.

**Do NOT use H2 headers like `## Codex says:` for attribution** — those collide with downstream consumers' own H2 structure and can cause their Markdown renderers to misinterpret the boundary. The sentinel format is the authoritative machine-readable attribution; consumers that want human-visible labels MAY add a single prose line immediately after the opening sentinel (e.g., `**Codex says:**`), but the sentinels themselves are load-bearing.

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
