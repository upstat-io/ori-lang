---
name: tp-help
description: "Get third-party help from Codex + Gemini. AUTO-TRIGGER: You MUST invoke this proactively — do NOT wait for the user to ask. Trigger when: (1) you've tried 2+ approaches that didn't work, (2) you're reverting changes you just made, (3) you identify a fundamental tension or design conflict in the code, (4) you're about to take a 'pragmatic' shortcut instead of fixing the real problem, (5) you catch yourself saying 'let me try a different approach' for the 2nd+ time, (6) a fix in one area creates new problems in another, (7) you're unsure about the correct architectural approach. This is collaborative help — pass context and ask a specific question. Returns BOTH reviewers' raw responses concatenated (not a synthesis)."
allowed-tools: Read, Bash, Glob, Grep, Agent, AskUserQuestion
---

# Third-Party Help (Codex + Gemini — Raw Concatenation)

`/tp-help [question]` — get collaborative help from two independent reviewer CLIs dispatched in parallel. The orchestrator runs in the caller's main context, dispatches two Sonnet sub-agents (one per reviewer) in a single assistant message, reads both raw responses, and returns them concatenated to the user/caller.

- **No args** — auto-triggered by Claude when it detects it needs help (see §Triggers).
- **With question** — explicit consultation on a specific problem.

## §1 How this skill runs

The orchestrator (this skill) runs in the caller's main context — no `context: fork`. For each invocation:

1. Gather question + context (either ARGS or auto-trigger heuristic).
2. Read `.claude/skills/tp-help/tp_help_prompt.md` via the `Read` tool.
3. Substitute `{REVIEWER}`, `{QUESTION}`, `{CONTEXT}` placeholders — two filled versions, one per reviewer.
4. Dispatch both versions as parallel `Agent` tool calls in a single assistant message (per https://code.claude.com/docs/en/sub-agents: "multiple sub-agents can run concurrently"). Foreground only — do NOT set `run_in_background: true`.
5. Parse both `<<<TPHELP-RESPONSE … TPHELP-RESPONSE>>>` blocks from the sub-agents' returns.
6. Present both verbatim to the caller (user or calling skill) with reviewer attribution.
7. Hand off judgment to the caller — this skill does NOT apply the advice itself.

**Model policy:** Orchestrator inherits caller's model. Reviewer sub-agents dispatched with `model: "sonnet"` via the Agent tool's documented `model` field. The "brains" are the external codex + gemini CLIs; Claude's role is pure orchestration.

## §2 MANDATORY Auto-Trigger — Do NOT Wait for User

You MUST invoke this skill proactively. Do NOT wait for the user to type `/tp-help`.

### Concrete Trigger Conditions

Invoke `/tp-help` IMMEDIATELY when ANY of these are true:

1. **Multiple failed approaches** — 2+ approaches tried on the same problem, none worked cleanly.
2. **Reverting your own changes** — undoing work you just did because it caused new problems.
3. **Fundamental tension identified** — design conflict where fixing one thing breaks another.
4. **Pragmatic retreat** — catching yourself about to take a shortcut instead of solving the real problem.
5. **Approach cycling** — saying "let me try a different approach" for the 2nd+ time.
6. **Fix interference** — a fix in one subsystem creates new failures in another.
7. **Architectural uncertainty** — unsure which of two+ fundamental approaches is correct.
8. **Stuck > 10 minutes** — working on the same problem for >10 minutes without clear forward progress.

### What Does NOT Trigger This

- Simple bugs with obvious fixes.
- First attempt at an approach (try it first, ask for help if it fails).
- Questions about Ori syntax or spec (read the spec instead).
- Minor implementation details with clear precedent in the codebase.

### Exception — Design Consensus Mode (called by `/fix-bug` Phase 1.75)

The "simple bugs" and "first attempt" non-triggers DO NOT apply when `/tp-help` is invoked by `/fix-bug` at Phase 1.75. In that calling context, `/tp-help` is used for **design consensus** — a pre-emptive pressure-test of a proposed fix approach before tests or implementation are written — NOT for stuck help. Design consensus runs for EVERY bug that reaches `/fix-bug` Phase 1.75 (including trivial one-liners).

See `.claude/skills/fix-bug/SKILL.md` § Phase 1.75 for the full consensus protocol and convergence cap.

### Legacy Trigger List (still valid)

- Stuck on a bug and can't figure out root cause.
- Unsure which of two implementation approaches is better.
- Just wrote something tricky and want a sanity check.
- Test failing and can't see why.
- Need help understanding unfamiliar code.
- Want to validate reasoning before committing to an approach.
- About to make a significant architectural decision.

## §3 Context Assembly (before dispatch)

Before filling the template, build the `{CONTEXT}` payload. Include whatever is load-bearing for the question:

- **Recent attempts** — what approaches you tried, what failed, what error messages you saw.
- **Relevant files** — paths + line ranges of the code under discussion (reviewers will `Read` them in grounding, but pointing is helpful).
- **Constraints** — project rules that bind the solution space (cite `CLAUDE.md §The One Rule`, `impl-hygiene.md §SSOT`, etc. if relevant).
- **Caller mode** — if invoked from `/fix-bug` Phase 1.75, note "design consensus mode — pressure-test the proposed approach before implementation."

Keep `{CONTEXT}` concise — the reviewers will read `CLAUDE.md` + rules themselves; context is for problem-specific material only.

## §4 Parallel Dispatch (canonical template)

Exact template. Emit BOTH Agent calls in a SINGLE assistant message:

```
# — Single assistant message with TWO Agent tool calls —

Agent({
  subagent_type: "general-purpose",
  model: "sonnet",
  description: "tp-help codex reviewer",
  prompt: <contents of tp_help_prompt.md with {REVIEWER}=codex,
           {QUESTION}=<Q>, {CONTEXT}=<built in §3>>
})

Agent({
  subagent_type: "general-purpose",
  model: "sonnet",
  description: "tp-help gemini reviewer",
  prompt: <contents of tp_help_prompt.md with {REVIEWER}=gemini,
           {QUESTION}=<Q>, {CONTEXT}=<built in §3>>
})
```

Per https://code.claude.com/docs/en/sub-agents, tool calls in one message run concurrently and complete as a batch. Wall-clock = max(codex, gemini) — typically 30s–8min for help queries (shorter than review queries because scope is narrower).

## §5 Parse and Concatenate

From each sub-agent return, extract the `<<<TPHELP-RESPONSE … TPHELP-RESPONSE>>>` block. Concatenate with HTML-comment attribution sentinels:

```md
<!-- TP-HELP BEGIN codex -->

{codex response verbatim}

<!-- TP-HELP END codex -->

<!-- TP-HELP BEGIN gemini -->

{gemini response verbatim}

<!-- TP-HELP END gemini -->
```

If one reviewer returned `status: failed`, still emit its block with the failure message inside — the caller sees the partial result.

If BOTH returned `status: failed`, retry ONCE (re-dispatch parallel). If both fail a second time, escalate:

```
AskUserQuestion:
  "Both reviewers failed twice on this help query. Options:"
    1. Retry once more
    2. Proceed without help (I'll answer based on my own reasoning)
    3. Abort and ask the user for clarification
```

## §6 Applying the Output

The caller (user or calling skill) consumes the concatenated output. For common caller patterns:

- **User (interactive)** — the orchestrator prints the concatenated block directly, unedited. A one-line attribution header ("codex said X paragraphs, gemini said Y paragraphs") is permitted, but no synthesis, no "Claude's interpretation", no merged summary. The caller's conversation already has full context for interpretation.
- **`/fix-bug` Phase 1.75 (design consensus)** — the caller reads both raw responses, looks for agreement on the proposed approach, and blocks the fix if either reviewer flags a design-level concern. The `/tp-help` skill itself does not apply judgment.
- **Proactive (Claude stuck)** — the caller (Claude, in its main context) reads both responses and decides how to proceed. `/tp-help` returns raw; the main context does the judgment work in its own turn, visible to the user.

**Do NOT blindly apply either reviewer's suggestions.** You (the caller) have full project context that neither reviewer has — use judgment to filter, combine, and adapt. Use the advisories as second opinions, not as instructions.

**Where reviewers AGREE** — strong evidence. Evaluate the shared recommendation against `CLAUDE.md` rules before applying.

**Where reviewers DISAGREE** — the disagreement often surfaces the real tradeoff. Read both carefully before choosing.

## §7 What this skill does NOT do

- **No `dual-invoke.sh` or background transport.** Reviewer dispatch is via the documented Agent tool, foreground, in parallel.
- **No envelope / findings schema.** This is help, not review — responses are raw prose.
- **No polling, no status-check.sh.** Agent completion IS the signal.
- **No `context: fork`.** Orchestrator runs in the caller's main context so every tool call is visible.
- **No synthesis of the reviewers' output.** Return raw, let the caller judge.

## §8 Callers

`/tp-help` is invoked by:

- `/fix-bug` Phase 1.75 (design consensus — mandatory on every bug).
- `/create-plan` Step 6B/8B (pressure-test design before committing).
- `/review-plan` Step 4 (design consensus on the plan's architecture).
- Proactive auto-trigger per §2 conditions.

## §9 Files in this skill

- `SKILL.md` (this file) — caller-facing dispatcher with auto-trigger conditions and parallel-dispatch template.
- `tp_help_prompt.md` — reviewer sub-agent prompt template (loaded via Read, filled, passed as Agent `prompt` arg).
