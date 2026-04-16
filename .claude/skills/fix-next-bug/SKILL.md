---
name: fix-next-bug
description: Iterate through the bug tracker, auto-picking the highest priority open bug and fixing it via /fix-bug. Each bug gets full /fix-bug rigor including mandatory /tp-help design consensus at Phase 1.75 before implementation (adds ~10–45 min per bug). After each fix, prompts the user to continue to the next bug or stop.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash, Agent, AskUserQuestion, Skill
---

# Fix Next Bug

Automatically pick the highest priority open bug from the bug tracker and fix it using `/fix-bug`. After each fix completes, prompt the user to continue with the next highest priority bug or stop.

## Usage

```
/fix-next-bug
```

No arguments needed — the skill auto-selects based on priority.

## How this skill runs

SKILL.md has two parts:

1. **Part 1 — Thin dispatcher**: Sends the queue scan and mode selection to a Sonnet sub-agent via `workflow.md`. Sonnet reads all bug-tracker section files, sorts the queue, runs the blast-radius preview, asks for mode, and returns a handoff.

2. **Part 2 — Opus loop manager**: After the handoff, the parent (Opus) drives the fix loop — invoking `/fix-bug` for each bug, running the commit verification gate, and re-dispatching Sonnet for re-scans between iterations.

**FOREGROUND MANDATORY — ALL Agent dispatches.**

---

## Part 1: Queue Scan (via Sonnet)

**This is the ONLY action before reading the handoff.** Dispatch the scan sub-agent:

```
Agent({
  description: "fix-next-bug queue scan + mode selection",
  subagent_type: "general-purpose",
  model: "sonnet",
  prompt: `
You are the queue-scan agent for /fix-next-bug. Read .claude/skills/fix-next-bug/workflow.md
in full and execute it end-to-end.

Rules:
- Follow Steps 1 through 5 literally.
- Read plans/bug-tracker/ files ONLY. Never open compiler/, library/, tests/.
- Do NOT invoke /fix-bug or make any code changes.
- Commits via Skill(commit-push) only — never run git commit directly.
- Return the handoff block in the EXACT format specified at the end of workflow.md.
  `
})
```

**Do not scan section files, sort bugs, or ask for mode yourself.** The dispatch is the only action in Part 1.

---

## Part 2: After the Handoff

### Step A — Print Queue Display (MANDATORY FIRST)

**Before any other output**, print the sub-agent's `### Queue display` block verbatim to the user. This ensures the user sees which bug is selected and the full queue before the loop begins.

### Step B — Check Queue Empty

If handoff shows `Queue empty: true`:
```
No open bugs in the tracker. All clear!
```
Stop — nothing to do.

### Step C — Enter the Fix Loop

Based on `Mode` in the handoff:

- **`Mode: interactive`** → go to Step D (interactive mode)
- **`Mode: autopilot`** → go to Step E (autopilot mode)

---

### Step D: Interactive Mode

**Invoke `/fix-bug` via the Skill tool** — `Skill(fix-bug, args: "BUG-{section}-{ordinal}")` (without `--autopilot`). MUST use the Skill tool — never inline the workflow.

**Let `/fix-bug` run its complete workflow** — do NOT shortcut any phase.

**Run the Commit Verification Gate** (see Step F) after `/fix-bug` returns.

After commit verification passes, use `AskUserQuestion` to ask:

- **Question**: `Fix complete for [BUG-{section}-{ordinal}].\n\nNext bug in queue: [BUG-{next}][{severity}] {title}\n{N-2} more bugs remaining after that.\n\nContinue with the next bug?`
- **Options**: `Yes`, `No`, `Skip`

Loop behavior:
- **Yes**: Re-dispatch Sonnet for a fresh scan (Step G) to get the new queue. Pick the new highest priority and invoke `/fix-bug` again.
- **Skip**: Re-dispatch Sonnet, exclude the skipped bug ID for this session only. Pick the next one.
- **No**: Print the session summary (Step H) and stop.

---

### Step E: Autopilot Mode

**This mode runs until the bug queue is empty or the user manually interrupts. NOTHING ELSE STOPS IT.**

**Before entering the loop**, create a persistent reminder task:
- **Subject**: `"AUTOPILOT: Do NOT stop until bug queue is empty"`
- **Description**: `"After EVERY /fix-bug outcome (fixed, escalated, blocked, OBE): commit gate → re-scan → pick next bug. The session summary is ONLY printed when re-scan returns zero open bugs. There is NO 'natural stopping point.' The count of bugs processed is irrelevant — only the queue state matters. If you are about to write a session summary, STOP and check: is the queue empty? If no, pick the next bug."`

This task must remain `in_progress` for the entire autopilot session. Only mark it `completed` when you print the final report (queue empty or user stopped).

**CRITICAL: This is the ONLY task for the entire autopilot session.** Do NOT use `TaskCreate` for any other purpose during autopilot.

**Autopilot loop:**

1. **Invoke `/fix-bug` via the Skill tool** — `Skill(fix-bug, args: "--autopilot BUG-{section}-{ordinal}")`. MUST use the Skill tool — never inline the /fix-bug workflow by hand. The `--autopilot` flag tells `/fix-bug` to operate with zero user interaction, full rigor, no hacks.
2. **Run the Commit Verification Gate** (Step F).
3. After commit is verified, **immediately re-dispatch Sonnet** (Step G) for a fresh scan. Do NOT output a summary, do NOT pause, do NOT reflect on what was done.
4. If open bugs remain (handoff `Queue empty: false`), pick the next highest priority bug and invoke `/fix-bug --autopilot` via the Skill tool again.
5. If no open bugs remain (`Queue empty: true`), **ONLY THEN** stop, mark the TaskCreate as completed, and print the final report (Step H).

**BANNED in autopilot mode — these are NOT valid reasons to stop:**
- "Session summary" or "progress report" mid-loop — the summary is ONLY printed when the queue is empty
- "Natural stopping point" — there is no such thing; the loop continues until the queue is empty
- "Already processed N bugs" — the count is irrelevant; the queue state is all that matters
- "Bug was complex/couldn't fix" — mark escalated or blocked, then CONTINUE
- "Bug was latent/OBE" — mark it, then CONTINUE

**Valid `/fix-bug` outcomes in autopilot — ALL require continuing to the next bug:**
- **Fixed** → continue
- **Escalated** (marked `Escalated: requires plan — {reason}` in autopilot) → continue
- **Blocked** → continue
- **OBE** → continue

**Consensus deadlocks** (autopilot): `/fix-bug` Phase 1.75 may deadlock after 3 `/tp-help` rounds. It proceeds with Claude's best-grounded approach and flags it. These MUST appear in the final report so the user can audit.

---

### Step F: Commit Verification Gate (After EVERY Fix)

**After `/fix-bug` completes (in EITHER mode), before doing anything else:**

1. Run `git status` to check for uncommitted changes
2. If there are uncommitted changes:
   - Invoke `Skill(commit-push)` to commit all changes
   - Verify the commit succeeded (clean `git status`)
3. If `git status` is clean, proceed

**This gate is non-negotiable.** A fix that isn't committed doesn't exist. Never proceed to the next bug with uncommitted work.

---

### Step G: Re-scan for Next Iteration

Re-dispatch the Sonnet sub-agent to get a fresh queue:

```
Agent({
  description: "fix-next-bug re-scan",
  subagent_type: "general-purpose",
  model: "sonnet",
  prompt: `
You are the queue-scan agent for /fix-next-bug. Read .claude/skills/fix-next-bug/workflow.md
in full and execute it end-to-end.

{If a bug was skipped this session: "Skip these bug IDs for this session: BUG-XX-NNN"}

Rules:
- Follow Steps 1 through 5 literally. Do NOT ask the mode question (Step 5) — mode is already
  set to {interactive|autopilot}. Return the handoff with Mode: {interactive|autopilot}.
- Read plans/bug-tracker/ files ONLY. Never open compiler/, library/, tests/.
- Return the handoff block in the EXACT format specified at the end of workflow.md.
  `
})
```

Scanner output may have changed — OBE resolutions, new bugs filed, escalations — so ALWAYS re-scan rather than reusing prior queue state.

---

### Handling Plan Escalation

When `/fix-bug` determines a bug needs a plan:

- **Interactive mode**: `/fix-bug` invokes `/create-plan` normally. After it returns, ask to continue to the next bug.
- **Autopilot mode**: `/fix-bug` marks the bug entry with `Escalated: requires plan — {reason}`. Run the Commit Verification Gate (the entry update needs committing), then immediately continue to the next bug. The user creates the plan after the autopilot session ends.

Escalated and blocked bugs are excluded by workflow.md's lifecycle-marker filter — they won't appear in re-scans.

---

### Step H: Final Report

**Generated ONLY when the queue is empty (all bugs processed) or the user manually stops.** NEVER generate this mid-loop.

```
## Fix Next Bug — Session Summary

Mode: {interactive | autopilot}
Bugs processed this session: {total}

Fixed: {N}
{For each:}
  - [BUG-XX-NNN][severity] title — fixed

Escalated to plans (interactive — plan created): {N}
{For each:}
  - [BUG-XX-NNN][severity] title — escalated to plans/{plan-name}/

Escalated (autopilot — requires plan, user action needed): {N}
{For each:}
  - [BUG-XX-NNN][severity] title — requires plan: {reason}

Blocked (prerequisite missing): {N}
{For each:}
  - [BUG-XX-NNN][severity] title — blocked: {reason}

Resolved as OBE: {N}
{For each:}
  - [BUG-XX-NNN][severity] title — already fixed

{If any autopilot consensus deadlocks:}
Consensus deadlocks (autopilot — require user audit): {N}
{For each:}
  - [BUG-XX-NNN][severity] title — Phase 1.75 consensus deadlocked after 3 /tp-help rounds;
    proceeded with Claude's best-grounded approach. See fix-BUG-XX-NNN.md § 1.5 Round 3 for details.

{If any skipped (interactive mode only):}
Skipped: {N}
  - [BUG-XX-NNN][severity] title — skipped

Remaining open bugs: {N}
```

**Consensus deadlocks are load-bearing in the final report.** In autopilot mode, the session summary is the only surfacing point. If a consensus-deadlocked fix later proves wrong, the user's remediation path is to read the fix section's § 1.5 Round 3 entry.

---

## Key Rules

- **Always re-scan** before picking the next bug — the queue is dynamic (Step G)
- **Full `/fix-bug` rigor** — every bug goes through the complete workflow via the Skill tool, no shortcuts
- **Never skip phases** — investigation, TDD, implementation, TPR, hygiene — all mandatory per `/fix-bug`
- **Mode is chosen once** — the mode question is asked only at the start (by Sonnet in workflow.md), not after each bug
- **Autopilot = zero interaction, zero stopping** — no questions, no confirmations, no pauses, no mid-loop summaries between bugs
- **Every `/fix-bug` outcome continues the loop** — fixed, escalated, blocked, OBE — ALL lead to picking the next bug
- **The session summary IS the exit** — generating it means the loop is over. NEVER generate it unless the queue is empty or the user stopped you
- **Flaky tests ARE bugs** — do NOT retry and move on. Research the root cause and fix it. File via `/add-bug` if discovered during another fix; fix immediately if it blocks the current work
- **NEVER investigate "pre-existing?"** — do NOT use git archaeology. The only question is: is it fixed?

## Files in this skill

- `SKILL.md` (this file) — Phase 0 dispatcher + Opus loop manager
- `workflow.md` — Sonnet sub-agent: scans queue, sorts by priority, runs blast-radius preview, asks mode, returns handoff
