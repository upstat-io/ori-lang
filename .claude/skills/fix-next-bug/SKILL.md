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

## Priority Ordering

Bugs are selected in this order:
1. **Severity** — `critical` > `high` > `medium` > `low`
2. **Pipeline position** — lower section number first (earlier in the compiler pipeline = higher impact):
   - 01 Parser & Lexer → 02 Type Checker → 03 Evaluator → 04 Codegen & LLVM → 05 Runtime & ARC → 06 Stdlib → 07 Tooling & CLI → 08 Spec & Docs
3. **Ordinal** — lower bug number first within the same section and severity

## Workflow

### Step 1: Scan All Open Bugs

Read all section files to collect every `- [ ]` entry:

```
plans/bug-tracker/section-01-parser-lexer.md
plans/bug-tracker/section-02-typeck.md
plans/bug-tracker/section-03-eval.md
plans/bug-tracker/section-04-codegen-llvm.md
plans/bug-tracker/section-05-runtime-arc.md
plans/bug-tracker/section-06-stdlib.md
plans/bug-tracker/section-07-tooling-cli.md
plans/bug-tracker/section-08-spec-docs.md
```

For each `- [ ]` entry, extract:
- **ID**: `BUG-{section}-{ordinal}`
- **Severity**: critical, high, medium, or low
- **Title**: the bold text after severity
- **Repro**: repro line if present
- **Subsystem**: subsystem line if present
- **Lifecycle markers**: check for `Escalated to plan:`, `Blocked:`, or `Escalated:` notes in the entry body

**Exclude non-fixable entries** — remove from the candidate list any `- [ ]` entry whose body contains:
- `Escalated to plan:` or `Escalated:` — the bug has been promoted to a plan; it is no longer an inline fix candidate
- `Blocked:` — the bug has a prerequisite that hasn't been met yet

These entries remain `- [ ]` (unchecked) because they are not resolved, but they are not actionable by `/fix-bug` until the plan completes or the blocker clears. Only genuinely open, unblocked, non-escalated bugs enter the priority queue.

### Step 2: Sort by Priority

Sort all open bugs using the priority ordering above:
1. Group by severity (critical first)
2. Within each severity group, sort by section number ascending
3. Within each section, sort by ordinal ascending

### Step 3: Check for Empty Queue

If there are no open bugs:
```
No open bugs in the tracker. All clear!
```
Stop — nothing to do.

### Step 4: Present the Selected Bug

Show the user what's been selected and the queue behind it:

```
## Fix Next Bug — Queue

Selected: [BUG-{section}-{ordinal}][{severity}] {title}
  Repro: {repro}
  Subsystem: {subsystem}

Remaining queue ({N-1} bugs):
  1. [BUG-...][severity] title
  2. [BUG-...][severity] title
  ...
```

### Step 5: Choose Mode

Before fixing the first bug, use `AskUserQuestion` to ask:

```
Ready to start with: [BUG-{section}-{ordinal}][{severity}] {title}

How would you like to proceed?

1. **One at a time** — Fix this bug, then ask before each next bug
2. **Fix all bugs non-stop** — Loop through ALL open bugs automatically with zero interaction. No questions, no pauses, no stops. Runs /fix-bug for every bug in priority order until the queue is empty or you manually stop me.
```

- If **One at a time**: proceed to Step 6 (interactive mode)
- If **Fix all**: proceed to Step 7 (autopilot mode)

### Step 5.5: Commit Verification Gate — After EVERY Fix

**After `/fix-bug` completes (in EITHER mode), before doing anything else:**

1. Run `git status` to check for uncommitted changes (staged, unstaged, or untracked files in compiler/library/tests paths)
2. If there are uncommitted changes:
   - Invoke `/commit-push` to commit all changes
   - Verify the commit succeeded (clean `git status`)
3. If `git status` is clean, proceed

**This gate is non-negotiable.** A fix that isn't committed doesn't exist. Never proceed to the next bug with uncommitted work — it will contaminate the next fix's diff, TPR review, and git history.

### Step 6: Interactive Mode — Fix and Prompt

**Invoke `/fix-bug` via the Skill tool**: `Skill(fix-bug, args: "BUG-{section}-{ordinal}")` (without `--autopilot`). MUST use the Skill tool — never inline the workflow.

**Let `/fix-bug` run its complete workflow** — investigation, fix section creation, TDD matrix, implementation, completion checklist (including TPR and hygiene review). Do NOT shortcut any phase.

**Run the Commit Verification Gate (Step 5.5)** after `/fix-bug` returns.

After commit verification passes, use `AskUserQuestion` to ask:

```
Fix complete for [BUG-{section}-{ordinal}].

Next bug in queue: [BUG-{next}][{severity}] {title}
{N-2} more bugs remaining after that.

Continue with the next bug?
```

Options:
- **Yes** — go back to Step 1 (re-scan, in case OBE or new bugs were filed during the fix)
- **No** — stop here
- **Skip** — skip this next bug and move to the one after it

Loop behavior:
- If **Yes**: re-scan the bug tracker (Step 1) to get a fresh view — bugs may have been resolved as OBE during the previous fix, or new bugs may have been filed. Pick the new highest priority and continue.
- If **Skip**: re-scan and exclude the skipped bug ID from selection for this session only. Pick the next one.
- If **No**: report a summary of what was done and stop.

### Step 7: Autopilot Mode — Fix All Non-Stop

**This mode runs until the bug queue is empty or the user manually interrupts. NOTHING ELSE STOPS IT.**

**Before entering the loop**, create a persistent reminder task using `TaskCreate`:
```
Subject: "AUTOPILOT: Do NOT stop until bug queue is empty"
Description: "After EVERY /fix-bug outcome (fixed, escalated, blocked, OBE): commit gate → re-scan → pick next bug. The session summary is ONLY printed when re-scan returns zero open bugs. There is NO 'natural stopping point.' The count of bugs processed is irrelevant — only the queue state matters. If you are about to write a session summary, STOP and check: is the queue empty? If no, pick the next bug."
```
This task must remain `in_progress` for the entire autopilot session. Only mark it `completed` when you print the final report (queue empty or user stopped). This ensures the reminder stays visible even if earlier conversation context is compressed.

**CRITICAL: This is the ONLY task for the entire autopilot session.** Do NOT use `TaskCreate` for any other purpose during autopilot — not for tracking `/fix-bug` phases, not for sub-steps, not for anything. Additional tasks would become the "current work" and their completion would signal the loop is done, causing premature termination.

Loop:
1. **Invoke `/fix-bug` via the Skill tool** — use `Skill(fix-bug, args: "--autopilot BUG-{section}-{ordinal}")`. This is a BLOCKING REQUIREMENT: you MUST use the Skill tool, not inline the /fix-bug workflow by hand. Inlining drops phases (fix section files, /tp-help consensus, TDD-first, TPR/hygiene). The Skill tool loads the full /fix-bug SKILL.md fresh each time, preventing context drift across a long autopilot session. The `--autopilot` flag tells `/fix-bug` to operate with zero user interaction, no pausing, no `AskUserQuestion`, full rigor, no hacks. It must complete the fix no matter the scope.
2. **Run the Commit Verification Gate (Step 5.5)** — check `git status`, commit via `/commit-push` if anything is uncommitted. Do NOT proceed until clean.
3. When commit is verified, **immediately** re-scan the bug tracker (Step 1) — do NOT output a summary, do NOT pause, do NOT reflect on what was done
4. If open bugs remain, pick the next highest priority bug and invoke `/fix-bug --autopilot` via the Skill tool again
5. If no open bugs remain, **ONLY THEN** stop and print the final report

**BANNED: inlining the /fix-bug workflow.** Reading the bug entry and directly jumping to code changes — without a Skill tool invocation, without creating a fix section file, without /tp-help consensus, without TDD-first testing — is the most common autopilot failure mode. Each of those phases exists because "obvious" fixes have hidden implications. The Skill tool invocation is the enforcement mechanism.

**No questions. No pauses. No user interaction. No mid-loop summaries.** Just pick → fix → pick → fix until done.

**The `--autopilot` flag is critical** — it tells `/fix-bug` that:
- It CANNOT pause and ask questions — it must make decisions autonomously based on CLAUDE.md rules and the spec
- It MUST re-read CLAUDE.md at the start (Phase -1) to ground itself in the rules
- It MUST follow "The One Rule: Correctness Above All" — no hacks, no shortcuts, no workarounds regardless of scope
- It MUST continue until the bug is fully fixed, reviewed (TPR + hygiene), and committed
- It MUST return control to `/fix-next-bug` after each outcome (fixed, escalated, blocked, OBE) — never just stop

**The ONLY things that stop autopilot mode:**
- The bug queue is empty (all open bugs processed)
- The user manually interrupts/stops the session

**BANNED in autopilot mode — these are NOT valid reasons to stop:**
- "Session summary" or "progress report" mid-loop — the summary is ONLY printed when the queue is empty
- "Natural stopping point" — there is no such thing; the loop continues until the queue is empty
- "Already processed N bugs" — the count is irrelevant; the queue state is all that matters
- "Bug was complex/couldn't fix" — escalate to `/create-plan` or mark blocked, then CONTINUE to the next bug
- "Bug was latent/OBE" — mark it, then CONTINUE to the next bug
- Generating output that looks like a wrap-up — a session summary IS an exit; do NOT write one until the queue is empty

**Valid `/fix-bug` outcomes in autopilot — ALL require continuing to the next bug:**
- **Fixed** — bug resolved, tests pass, TPR clean → continue
- **Escalated** — too large, `/create-plan` run → continue
- **Blocked** — prerequisite missing, bug entry updated → continue
- **OBE** — already fixed, marked resolved → continue

After EVERY outcome, the next action is ALWAYS: commit gate → re-scan → pick next bug. No exceptions.

### Handling Plan Escalation

When `/fix-bug` determines a bug needs a plan (Phase 1.5 scope assessment), this is a valid outcome — not a failure.

In the loop:
- **Interactive mode**: `/fix-bug` invokes `/create-plan` normally (with user approval gates), then reports the escalation. Ask to continue to the next bug as normal.
- **Autopilot mode**: `/fix-bug` marks the bug entry with `Escalated: requires plan — {reason}` (it does NOT invoke `/create-plan` since that requires interactive approval). Run the Commit Verification Gate (the entry update needs committing), then immediately continue to the next bug. The user creates the plan after the autopilot session ends.

Escalated and blocked bugs are already excluded by Step 1's lifecycle-marker filter — they will not appear in the re-scan.

### Final Report

**This is ONLY generated when the queue is empty (all bugs processed) or the user manually stops.** NEVER generate this mid-loop as a "checkpoint" or "progress report."

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
  - [BUG-XX-NNN][severity] title — Phase 1.75 consensus deadlocked after 3 /tp-help rounds; proceeded with Claude's best-grounded approach. See fix-BUG-XX-NNN.md § 1.5 Round 3 for details.

{If any skipped (interactive mode only):}
Skipped: {N}
  - [BUG-XX-NNN][severity] title — skipped

Remaining open bugs: {N}
```

**Consensus deadlocks are load-bearing in the final report.** In autopilot mode, /fix-bug Phase 1.75 is allowed to proceed with Claude's best-grounded approach when /tp-help cannot reach consensus in 3 rounds (autopilot rules forbid AskUserQuestion). The user MUST be able to audit every such bug after the run — the session summary is the only surfacing point. If a consensus-deadlocked fix later proves wrong, the user's remediation path is to read the fix section's § 1.5 Round 3 entry, understand Claude's reasoning, and decide whether to revert or revise.

## Key Rules

- **Always re-scan** before picking the next bug — the queue is dynamic
- **Full `/fix-bug` rigor** — every bug goes through the complete workflow, no shortcuts
- **Never skip phases** — investigation, TDD, implementation, TPR, hygiene — all mandatory per `/fix-bug`
- **Mode is chosen once** — the mode question is asked only at the start, not after each bug
- **Autopilot = zero interaction, zero stopping** — no questions, no confirmations, no pauses, no mid-loop summaries between bugs. The user chose this mode knowing what it means. The loop runs until the queue is EMPTY.
- **Every `/fix-bug` outcome continues the loop** — fixed, escalated, blocked, OBE — ALL of these are valid outcomes that lead to picking the next bug. None of them are reasons to stop.
- **The session summary IS the exit** — generating it means the loop is over. NEVER generate it unless the queue is empty or the user stopped you. If you find yourself writing a summary, ask: "is the queue empty?" If no, you are stopping prematurely.
- **Flaky tests ARE bugs** — if a test passes sometimes and fails sometimes, that is a bug. Do NOT retry and move on. Research the root cause (race condition, timing, temp files, state leakage, non-deterministic ordering) and fix it so the test is deterministic. File via `/add-bug` if discovered during another fix; fix immediately if it blocks the current work.
- **NEVER investigate "pre-existing?"** — do NOT use `git checkout`, `git stash`, `git bisect`, `git log`, or any other git archaeology to determine whether a bug or test failure existed before your changes. It does not matter. The question "was this pre-existing?" is banned. If it's broken now, fix it now. Spending time checking out old commits to see if something "was already broken" is wasted time that produces zero value. The only question is: is it fixed?
