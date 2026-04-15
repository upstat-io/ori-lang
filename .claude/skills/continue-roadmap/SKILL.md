---
name: continue-roadmap
description: Resume work on the Ori compiler roadmap. Dispatches scanning and gate-checking to an Opus sub-agent; code execution hands off to /roadmap-work after the sub-agent returns.
argument-hint: "[section]"
---

# Continue Roadmap

`/continue-roadmap [section]` — resume roadmap work.

- No args: auto-detect first incomplete item sequentially
- `section-4`, `4`, or a keyword: focus that section (keywords resolved via `plans/roadmap/index.md`)

## How this skill runs

SKILL.md is a thin dispatcher. The full protocol (gates, triage, pacing, checklist) lives in `workflow.md` and is executed by a dispatched sub-agent — not inline. The parent takes over only after the sub-agent returns a structured handoff block.

## Caller action (the ONLY inline action)

Before any other tool call, invoke the Agent tool. Substitute `<ARGS>` with the user's `/continue-roadmap` arguments (empty string if none):

```
Agent({
  description: "continue-roadmap scan + gates",
  subagent_type: "general-purpose",
  model: "sonnet",
  prompt: `
You are the scan-and-gate agent for /continue-roadmap. Read .claude/skills/continue-roadmap/workflow.md
in full and execute it end-to-end.

Args from the user: <ARGS>
(Empty string means auto-detect the first incomplete section.)

Rules:
- Follow Steps 1 through 5 literally. Do NOT read CLAUDE.md — the scanner
  pre-computes every gate decision and focus-context field.
- Stop at the end of Step 5. Do NOT execute /roadmap-work. Code execution
  is the parent's job.
- You touch plan docs (plans/**/*.md), frontmatter, and checkboxes ONLY.
  Never edit .rs, .ori, or anything under compiler/, library/, tests/.
- Commits via /commit-push only — never run git commit directly.
- If a gate requires invoking a separate skill (/verify-tpr for TPR finding
  validation, /review-plan for unreviewed plans, /fix-bug for critical bugs,
  /create-plan for unplanned blockers), STOP and return <escalate-to-parent>
  with the question and relevant paths. The parent invokes the named skill
  directly.
- Return the handoff block per Step 5 of workflow.md — EXACT format.
  The '### Focus context' block is MANDATORY on every return, including
  escalation. Omitting it is a contract violation.
  `
})
```

**Do not execute any step of the workflow yourself.** Do not read CLAUDE.md, run the scanner, or open plan files. The dispatch is the only action.

## After the sub-agent returns

The parent reads the handoff block and acts per its `Next command for the parent` line.

### Step A — ALWAYS print the Focus Context block first (MANDATORY)

**Before any other user-facing output** — before Insight blocks, before `AskUserQuestion`, before dispatching any skill — emit the sub-agent's `### Focus context` block verbatim as plain text to the user. This is the `## Focus: <plan full name> — Section NN: <title>` block with plan description, plan progress, section goal, section progress, and the full subsection list.

Without this block, gate questions reach the user with no idea what plan, section, or goal they're about to decide on. The sub-agent put the block in the handoff precisely so the parent can surface it — do NOT skip it, summarize it, or defer it until after a gate is answered.

After the Focus Context block, emit a one-line gate summary (e.g., "2 gates fired: unreviewed_plan, dirty_tree") so the user knows how many prompts are coming. Then proceed to Step B.

### Step B — Normal path or Escalation path

- **Normal path** (no `<escalate-to-parent>` in handoff) — invoke `Skill: roadmap-work <section-file> <subsection-id>` for code execution.
- **`<escalate-to-parent>` path** — follow the AskUserQuestion contract below.

### AskUserQuestion contract (MANDATORY on escalation)

When the handoff contains `<escalate-to-parent>` with one or more `User questions for AskUserQuestion` blocks, YOU MUST invoke the `AskUserQuestion` tool for each one — using the exact question text and `options` array the sub-agent provided. **Do NOT dump the escalation as prose text.** The sub-agent pre-structured the options so the user gets a native UI prompt, not a wall of text.

Flow for each question, in order:
1. Invoke `AskUserQuestion` with the question and its options.
2. When the user picks an option with a `next_skill`, invoke that skill with the provided arg (`/review-plan <path>`, `/verify-tpr <path>`, `/fix-bug <BUG-ID>`, `/commit-push`, `/create-plan`, etc.).
3. When the user picks `proceed` / `proceed anyway` / `pick-different`, honor that choice — do not invoke a skill, move on to the next question (or finish if none remain).
4. After every question is answered AND every chosen skill has resolved, re-dispatch a fresh sub-agent (re-run this skill). Scanner output, frontmatter, and bug state may have changed across the escalation.

**Never re-execute the sub-agent's steps inline.** If the user re-invokes `/continue-roadmap`, dispatch a fresh sub-agent. Caching prior scan state across invocations is banned.

## Files in this skill

- `SKILL.md` (this file) — caller-facing dispatcher, intentionally minimal.
- `workflow.md` — full protocol (Steps -1..5.5 for the sub-agent, Step 6 + implementation guidelines + checklist as reference for `/roadmap-work`).
- `roadmap-scan.sh` / `roadmap_scan.py` — workspace scanner. Sub-agent runs this at Step 1.
