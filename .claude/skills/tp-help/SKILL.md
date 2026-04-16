---
name: tp-help
description: "Get third-party help from Codex + Gemini. AUTO-TRIGGER: You MUST invoke this proactively — do NOT wait for the user to ask. Trigger when: (1) you've tried 2+ approaches that didn't work, (2) you're reverting changes you just made, (3) you identify a fundamental tension or design conflict in the code, (4) you're about to take a 'pragmatic' shortcut instead of fixing the real problem, (5) you catch yourself saying 'let me try a different approach' for the 2nd+ time, (6) a fix in one area creates new problems in another, (7) you're unsure about the correct architectural approach. This is collaborative help — pass context and ask a specific question. Returns BOTH reviewers' raw responses concatenated (not a synthesis)."
allowed-tools: Read, Bash, Glob, Grep, Agent, AskUserQuestion
---

# /tp-help

`/tp-help [question]` — dispatch the codex and gemini CLIs in parallel as sub-agents. Read both raw responses. Concatenate them with attribution sentinels. Return to the caller.

- No args → auto-triggered (see §2).
- With question → explicit consultation.

## §1 How this skill runs

1. Gather question + context (ARGS or auto-trigger heuristic).
2. `Read` `.claude/skills/tp-help/tp_help_prompt.md`.
3. Substitute `{REVIEWER}`, `{QUESTION}`, `{CONTEXT}` — two filled versions, one per reviewer.
4. Dispatch both versions as parallel Agent tool calls in a single assistant message (§4). Foreground only.
5. Parse both `<<<TPHELP-RESPONSE>>>` blocks from the sub-agents' returns.
6. Present both verbatim to the caller (§5). Do not synthesize.
7. Hand off judgment to the caller (§6).

## §2 MANDATORY Auto-Trigger — Do NOT Wait for User

Invoke IMMEDIATELY when ANY of these are true:

1. **Multiple failed approaches** — 2+ approaches tried on the same problem, none worked cleanly.
2. **Reverting your own changes** — undoing work you just did because it caused new problems.
3. **Fundamental tension identified** — design conflict where fixing one thing breaks another.
4. **Pragmatic retreat** — catching yourself about to take a shortcut instead of solving the real problem.
5. **Approach cycling** — saying "let me try a different approach" for the 2nd+ time.
6. **Fix interference** — a fix in one subsystem creates new failures in another.
7. **Architectural uncertainty** — unsure which of two+ fundamental approaches is correct.
8. **Stuck without forward progress** — working on the same problem and not making visible progress.

### Do NOT trigger for

- Simple bugs with obvious fixes.
- First attempt at an approach (try it first, ask if it fails).
- Questions about Ori syntax or spec (read the spec instead).
- Minor implementation details with clear precedent in the codebase.

### Exception — Design Consensus Mode (called by `/fix-bug` Phase 1.75)

The "simple bugs" and "first attempt" non-triggers DO NOT apply when `/tp-help` is invoked by `/fix-bug` at Phase 1.75. In that calling context, `/tp-help` is used for **design consensus** — a pre-emptive pressure-test of a proposed fix approach before tests or implementation are written. Design consensus runs for EVERY bug that reaches `/fix-bug` Phase 1.75.

See `.claude/skills/fix-bug/SKILL.md` § Phase 1.75.

### Legacy triggers (still valid)

- Stuck on a bug and can't figure out root cause.
- Unsure which of two implementation approaches is better.
- Just wrote something tricky and want a sanity check.
- Test failing and can't see why.
- Need help understanding unfamiliar code.
- Want to validate reasoning before committing to an approach.
- About to make a significant architectural decision.

## §3 Context Assembly (before dispatch)

Build the `{CONTEXT}` payload. Include what's load-bearing for the question:

- **Recent attempts** — approaches tried, what failed, error messages.
- **Relevant files** — paths + line ranges of code under discussion.
- **Constraints** — project rules that bind the solution space (cite `CLAUDE.md §The One Rule`, `impl-hygiene.md §SSOT`, etc. as relevant).
- **Caller mode** — if invoked from `/fix-bug` Phase 1.75, note "design consensus mode — pressure-test the proposed approach before implementation."

Keep `{CONTEXT}` concise. The reviewers read `CLAUDE.md` + rules themselves; context is for problem-specific material.

## §4 Parallel Dispatch (canonical template)

Emit BOTH Agent calls in a SINGLE assistant message:

```
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

Foreground only — never `run_in_background: true`.

## §5 Parse and Concatenate

From each sub-agent return, extract the `<<<TPHELP-RESPONSE … TPHELP-RESPONSE>>>` block. Concatenate with attribution sentinels:

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

- **User (interactive)** — print the concatenated block directly, unedited. A one-line attribution header ("codex said X paragraphs, gemini said Y paragraphs") is permitted, but no synthesis, no interpretation, no merged summary.
- **`/fix-bug` Phase 1.75 (design consensus)** — the caller reads both raw responses, looks for agreement on the proposed approach, and blocks the fix if either reviewer flags a design-level concern. `/tp-help` does not apply judgment.
- **Proactive (Claude stuck)** — the caller reads both responses and decides how to proceed, doing the judgment work in its own turn, visible to the user.

**Do NOT blindly apply either reviewer's suggestions.** Use the advisories as second opinions, not as instructions.

**Where reviewers AGREE** — strong signal. Evaluate the shared recommendation against `CLAUDE.md` rules before applying.

**Where reviewers DISAGREE** — the disagreement often surfaces the real tradeoff. Read both carefully before choosing.

## §7 Callers

- `/fix-bug` Phase 1.75 (design consensus — mandatory on every bug).
- `/create-plan` Step 6B/8B (pressure-test design before committing).
- `/review-plan` Step 4 (design consensus on the plan's architecture).
- Proactive auto-trigger per §2.

## §8 Files

- `SKILL.md` (this file) — dispatcher, auto-trigger conditions, parallel-dispatch template.
- `tp_help_prompt.md` — reviewer sub-agent prompt template.
