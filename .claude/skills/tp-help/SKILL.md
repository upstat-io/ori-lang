---
name: tp-help
description: "Get third-party help from Codex + Gemini. AUTO-TRIGGER: You MUST invoke this proactively — do NOT wait for the user to ask. Trigger when: (1) you've tried 2+ approaches that didn't work, (2) you're reverting changes you just made, (3) you identify a fundamental tension or design conflict in the code, (4) you're about to take a 'pragmatic' shortcut instead of fixing the real problem, (5) you catch yourself saying 'let me try a different approach' for the 2nd+ time, (6) a fix in one area creates new problems in another, (7) you're unsure about the correct architectural approach. This is collaborative help — pass context and ask a specific question. Returns BOTH reviewers' raw responses concatenated (not a synthesis)."
model: sonnet
---

# Third Party Help (Codex + Gemini — Dual Source, Concatenation Mode)

`/tp-help [question]` — get collaborative help from two independent models.

- No args: auto-triggered by Claude when it detects it needs help
- With question: explicit consultation on a specific problem

## How this skill runs

SKILL.md is a thin dispatcher. The full protocol (Steps 1-7: context building, prompt writing, dual-invoke launch, polling, parsing, worktree guard, concatenation) lives in `workflow.md` and is executed by a dispatched Sonnet sub-agent — not inline. The parent takes over only after the sub-agent returns the concatenated reviewer output.

**Mode:** Concatenation mode — NOT the findings envelope schema used by `/tpr-review` and `/review-work`. The output is both reviewers' raw responses concatenated with HTML-comment attribution sentinels.

**Model policy:** This skill runs end-to-end on Sonnet. The "brains" are the external codex + gemini CLIs; Claude's work is pure orchestration. Steps 8-9 (Apply + Brief) run in the caller's session model because they require judgment.

### Callers

`/tp-help` is invoked by `/fix-bug` (Phase 1.75 design consensus), `/create-plan` (Step 6B/8B), `/review-plan` (Step 4), and proactive auto-trigger conditions. The model policy is the same regardless of caller: Sonnet end-to-end for orchestration, raw concat return. The *caller* decides what model consumes the output.

## MANDATORY AUTO-TRIGGER — Do NOT Wait for User

**You MUST invoke this skill proactively.** Do NOT wait for the user to type `/tp-help`.

### Concrete Trigger Conditions

Invoke `/tp-help` IMMEDIATELY when ANY of these are true:

1. **Multiple failed approaches** — You've tried 2+ approaches to solve the same problem and none worked cleanly
2. **Reverting your own changes** — You're undoing work you just did because it caused new problems
3. **Fundamental tension identified** — You've identified a design conflict where fixing one thing breaks another
4. **Pragmatic retreat** — You catch yourself about to take a shortcut instead of solving the real problem
5. **Approach cycling** — You're saying "let me try a different approach" for the 2nd+ time
6. **Fix interference** — A fix in one subsystem creates new failures in another
7. **Architectural uncertainty** — You're unsure which of two+ fundamental approaches is correct
8. **Stuck > 10 minutes** — Working on the same problem for >10 minutes without clear forward progress

### What Does NOT Trigger This

- Simple bugs with obvious fixes
- First attempt at an approach (try it first, ask for help if it fails)
- Questions about Ori syntax or spec (read the spec instead)
- Minor implementation details with clear precedent in the codebase

### Exception — Design Consensus Mode (called by /fix-bug)

The "simple bugs" and "first attempt" non-triggers DO NOT apply when `/tp-help` is invoked by `/fix-bug` at Phase 1.75. In that calling context, `/tp-help` is used for **design consensus** — a pre-emptive pressure-test of a proposed fix approach before tests or implementation are written — NOT for stuck help. Design consensus runs for EVERY bug that reaches `/fix-bug` Phase 1.75 (including trivial one-liners).

See `.claude/skills/fix-bug/SKILL.md` § Phase 1.75 for the full consensus protocol, the 3-call convergence cap, and autopilot deadlock handling.

### Legacy Trigger List (still valid)

- You're stuck on a bug and can't figure out the root cause
- You're unsure which of two implementation approaches is better
- You just wrote something tricky and want a sanity check
- A test is failing and you can't see why
- You need help understanding unfamiliar code
- You want to validate your reasoning before committing to an approach
- You're about to make a significant architectural decision

## Caller action (the ONLY inline action)

Before any other tool call, invoke the Agent tool. Substitute `<ARGS>` with the user's `/tp-help` arguments (the question text, or empty if auto-triggered):

```
Agent({
  description: "tp-help dual-source consultation",
  subagent_type: "general-purpose",
  model: "sonnet",
  prompt: `
You are the orchestration agent for /tp-help. Read .claude/skills/tp-help/workflow.md
in full and execute Steps 1 through 7 end-to-end.

Question/context from the caller:
<ARGS>

Rules:
- Follow Steps 1 through 7 literally. The workflow file is the SSOT.
- You are Sonnet running orchestration — file reads, prompt assembly,
  shell launches, polling, parsing, worktree guard, concatenation.
- You DO NOT synthesize, triage, or apply the reviewer output.
  You return the raw concatenated output to the parent.
- You DO NOT write code, edit source files, or modify plan files.
- You DO NOT run tests or builds.
- Return: (1) the $RUN scratch dir path, (2) the concatenated
  sentinel-attributed output from Step 7, (3) any worktree drift
  warnings from Step 6.
  `
})
```

**Do not execute any step of the workflow yourself.** Do not read CLAUDE.md, build context, write prompts, or launch dual-invoke. The dispatch is the only action.

## After the sub-agent returns

The parent reads the sub-agent's return and performs Steps 8-9 in the caller's session model.

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

## Files in this skill

- `SKILL.md` (this file) — caller-facing dispatcher with auto-trigger conditions, intentionally minimal.
- `workflow.md` — full orchestration protocol (Steps 1-7 for the sub-agent).
