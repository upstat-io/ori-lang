---
name: add-bug
description: Add a bug to the bug-tracker plan. Minimal research at add-time — capture repro, location, severity, and source. TRIGGER proactively when ANY bug is encountered during ANY work — unrelated bugs, edge cases, test failures, suspicious behavior, code smells that look like bugs. If in doubt, file it. Better safe than sorry — verification happens at review time.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash, Agent, AskUserQuestion, Skill
argument-hint: "[description or file:line]"
---

# Add Bug

File a bug in `plans/bug-tracker/` under the correct subsystem section.

## Proactive Triggering — MANDATORY

This skill MUST be invoked proactively whenever you encounter a bug that is **not part of your current task**. Do NOT:
- Gloss over it as "not related"
- Note it mentally and move on
- Say "this is a separate issue" without filing
- Assume someone else will catch it
- Skip it because you're "in the middle of something"

**If in doubt, file it.** Verification happens when bugs are reviewed (`/review-bugs`). A false positive costs nothing; a missed bug costs everything.

### When to trigger (non-exhaustive)
- You see a test failure unrelated to your current work
- You notice suspicious behavior while reading code
- A code journey or exploration reveals unexpected output
- You encounter an edge case that probably doesn't work
- You find a TODO/FIXME/HACK comment that describes an unfixed bug
- A compiler error message is wrong or misleading
- You notice a mismatch between spec and implementation
- Any test is `#skip`-ped and the reason looks fixable

## How this skill runs

SKILL.md is a thin dispatcher. The full protocol (subsystem mapping, duplicate check, ID assignment, entry writing, cross-ref) lives in `workflow.md` and is executed by a Sonnet sub-agent. No code analysis is needed — this is a plan-doc-only workflow.

**FOREGROUND MANDATORY.** The sub-agent dispatch MUST run in the foreground (do NOT set `run_in_background: true`).

## Caller action (the ONLY inline action)

Before any other tool call, invoke the Agent tool. Substitute `<ARGS>` with the user's `/add-bug` arguments, and `<CONTEXT>` with any relevant context from the current conversation (bug description, file/line, repro, what was being worked on when the bug was discovered):

```
Agent({
  description: "add-bug filing",
  subagent_type: "general-purpose",
  model: "sonnet",
  prompt: `
You are the filing agent for /add-bug. Read .claude/skills/add-bug/workflow.md
in full and execute it end-to-end.

Bug description from the user: <ARGS>
Context from current task (interrupted workflow): <CONTEXT>

Rules:
- Follow Steps 1 through 8 literally.
- You touch plan docs (plans/bug-tracker/*.md) ONLY.
  Never edit .rs, .ori, or anything under compiler/, library/, tests/.
- Commits via Skill(commit-push) only — never run git commit directly.
- After Step 7, state the interrupted workflow context so the caller can resume it.
  `
})
```

**Do not execute any step of the workflow yourself.** Do not read section files, do not assign IDs, do not write entries. The dispatch is the only action.

## After the sub-agent returns

The sub-agent confirms the filing. If invoked mid-task (from `/fix-bug`, `/continue-roadmap`, `/tpr-review`, etc.), **immediately resume the interrupted workflow** — do not wait for user input.

## Files in this skill

- `SKILL.md` (this file) — caller-facing dispatcher, intentionally minimal.
- `workflow.md` — full protocol (Steps 1–8 for the Sonnet sub-agent).
