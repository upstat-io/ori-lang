# /tp-help Workflow — Sub-Agent Reference Document

This file is read by the Sonnet sub-agent dispatched from SKILL.md. It contains the orchestration protocol (Steps 1-4). Steps 5-6 (Apply the Answer, Brief the User) run in the parent's context after this sub-agent returns.

**Mode:** raw-concat. The dual-source SSOT `one-round.sh` handles launch, parse, worktree-guard, circuit-breaker pre-check, per-reviewer success/fail state tracking, and sentinel-attributed concatenation. This workflow owns only the *semantic* concerns: building the question, writing the prompts, and dispatching `one-round.sh --mode raw-concat`.

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
| Step 2 — Create the Scratch Dir | Sonnet | Shell (orchestration) |
| Step 3 — Write Both Reviewer Prompts | Sonnet | Mechanical-writing: static HARD RULES + grounding + adversarial framing; rule files cited, not summarized |
| Step 4 — Dispatch `one-round.sh --mode raw-concat` | Sonnet | Shell launch (orchestration); circuit-breaker / worktree-guard / parse / sentinel emission all happen inside `one-round.sh` |

## Runtime Budget

`one-round.sh --mode raw-concat` calls `dual-invoke.sh --mode raw` in the foreground with a 10-minute Bash timeout (`timeout: 600000`). Codex typically finishes in 1-3 minutes; Gemini may take 10-15 minutes on complex prompts.

**Per-reviewer independent retry (2026-04-16):** Each reviewer runs under its own `supervisor.sh` with a self-contained retry loop. A fast-failing reviewer (e.g. gemini hitting API capacity) exhausts its 5 retries on its own clock — typically 2-10 minutes — without waiting for the partner. The partner continues to completion obliviously, so no work is thrown away. Each supervisor records its own success / failure to the shared circuit-breaker state the moment it settles.

**Circuit-breaker auto-fallback:** `one-round.sh` consults the global per-user circuit-breaker state (shared across `/tpr-review`, `/tp-help`, `/review-work`, `/review-plan`) BEFORE launch. If a reviewer was recently tripped (e.g. a prior `/tpr-review` round hit 3 API failures on gemini within the last hour), `one-round.sh` auto-restricts to the healthy reviewer with a clear warning — no more "10-minute foreground timeout on a known-broken reviewer" failure mode. Because supervisors fire the breaker per-reviewer in real time, a degraded reviewer tripped mid-round during `/tpr-review` will auto-restrict the NEXT `/tp-help` invocation within 2-3 minutes instead of the prior 30+ minutes.

For fast iteration or to explicitly restrict to one reviewer, set `ORI_TPR_REVIEWERS`:
- `ORI_TPR_REVIEWERS=codex` — codex only (fast, ~1-3 min wall time, always within timeout)
- `ORI_TPR_REVIEWERS=gemini` — gemini only (slow, ~10-15 min wall time, may hit timeout)
- `ORI_TPR_REVIEWERS=both` — default (both reviewers, ~10-15 min wall time; auto-degraded to single if circuit-broken)

Operator intent (explicit `ORI_TPR_REVIEWERS=codex|gemini`) takes precedence over circuit-breaker auto-restriction — if you ask for a tripped reviewer explicitly, `one-round.sh` aborts rather than silently swapping.

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

## Step 2: Create the Scratch Dir

```bash
Bash:
  RUN=$(.claude/skills/dual-tpr/scripts/scratch-dir.sh)
  echo "RUN=$RUN" >&2  # so you can reference it in later steps
```

The worktree snapshot, worktree-guard compare, parse, and sentinel emission all happen *inside* `one-round.sh` — this workflow no longer carries those steps (previously Steps 2/5/6/7 inlined them, duplicating the logic that `dual-invoke-with-retry.sh` already implements for envelope-mode consumers).

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

## Step 4: Dispatch `one-round.sh --mode raw-concat`

Launch the canonical SSOT. **Run in the foreground with `timeout: 600000` (10 minutes).**

```
Bash (foreground, timeout: 600000):
  bash .claude/skills/dual-tpr/scripts/one-round.sh \
    --mode raw-concat \
    --run "$RUN" \
    --skill tp-help \
    --codex-prompt "$RUN/codex.prompt.md" \
    --gemini-prompt "$RUN/gemini.prompt.md"
```

`one-round.sh` handles — uniformly with the envelope-mode path used by `/tpr-review`:

1. **Circuit-breaker pre-check** — reads global state under `$HOME/.cache/ori-tpr-circuit/`. If a reviewer is tripped, auto-restricts `ORI_TPR_REVIEWERS` (operator explicit setting still wins). Aborts cleanly if BOTH tripped.
2. **Worktree snapshot** BEFORE dual-invoke.
3. **`dual-invoke.sh --mode raw` launch** — orchestrates two `supervisor.sh` processes (one per reviewer) as backgrounded siblings. Each supervisor runs its own retry loop (up to `MAX_RETRIES=5` attempts per lib-retry.sh) with per-reviewer watchdog, backoff, and failure classification. The supervisor model means a fast-failing reviewer does NOT block the partner's wall time — each terminates independently.
4. **Per-reviewer circuit-breaker update** — each supervisor calls `circuit-breaker.sh success` on clean completion or `circuit-breaker.sh fail <category>` on terminal give-up. No post-run aggregation needed at the one-round layer; the supervisors own that bookkeeping.
5. **Worktree-guard compare** via the canonical `worktree-guard.sh compare` helper. Drift is logged as a warning; with `ORI_TPR_STRICT_WORKTREE=1` it escalates to a failure exit.
6. **Raw-mode parsing** via `parse-codex-raw.py` / `parse-gemini-raw.py` — invoked inside each supervisor's `classify_reviewer_outcome` call (shared with envelope mode via `lib-retry.sh`). Successful raw parse writes the reviewer's prose to `$RUN/<reviewer>.envelope.json` (the filename is a generic "successful output" slot — in raw mode it holds plain text).
7. **Sentinel emission** via `tp-help-sentinels.sh` — per-invocation token embedded in `<!-- tp-help-reviewer: ... -->` open/close markers. Output lands at `$RUN/concat.md` (or `--output <file>` if specified).

On success, `one-round.sh` prints the output file path to stdout. On failure, exit codes are: 1 (both reviewers failed / both circuit-broken), 2 (usage), 3 (worktree strict-mode failure).

**DO NOT:**
- Use `run_in_background: true` — `/tp-help` runs foreground.
- Wrap `one-round.sh` in an Agent — the Agent adds no value and costs an extra process.
- Call `dual-invoke.sh` or `dual-invoke-with-retry.sh` directly — those are internal to `one-round.sh` now. Direct calls bypass the circuit-breaker pre-check that gives dual-source consumers shared operational awareness.

## Return to Parent

Read the concatenated output file:

```bash
Bash:
  cat "$RUN/concat.md"
```

Return to the parent with:
1. The `$RUN` scratch dir path (so the parent can cite it in fix section files, etc.)
2. The concatenated output (contents of `$RUN/concat.md`)
3. Any stderr warnings from `one-round.sh` (worktree drift, circuit-breaker auto-restriction, single-reviewer-only message, etc.)

If `one-round.sh` exited non-zero, return the failure category + stderr verbatim. Do not attempt to recover inline — the parent decides whether to re-invoke, present the degraded result, or escalate.
