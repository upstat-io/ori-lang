---
name: fix-next-bug
description: Iterate through the bug tracker, auto-picking the highest priority open bug and fixing it via /fix-bug. Each bug gets full /fix-bug rigor including mandatory /tp-help design consensus at Phase 1.75 before implementation (adds ~10–45 min per bug). After each fix, prompts the user to continue to the next bug or stop.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash, Agent, AskUserQuestion, Skill, ToolSearch
---

# Fix Next Bug

Auto-pick the highest priority open bug from `plans/bug-tracker/` and fix it using `/fix-bug`. After each fix, prompt the user to continue or stop (interactive mode) or keep going until the queue is empty (autopilot mode).

## Usage

```
/fix-next-bug
```

No arguments — the skill auto-selects based on priority.

## How this skill runs

**Queue scanning is a Python script (`bug_queue_scan.py`), not a sub-agent.** The scanner is deterministic, runs in ~35 ms, and costs essentially zero tokens. It replaces a prior Sonnet sub-agent that burned 140k tokens and ~3 minutes doing the same mechanical work.

The skill runs entirely in the parent (Opus) context:

1. **Scan** — `bash`-invoke `bug_queue_scan.py --json`, parse the JSON, print the queue display.
2. **Optional blast-radius preview** — run one `scripts/intel-query.sh` query if available.
3. **Mode selection** — `AskUserQuestion` for interactive vs autopilot (this MUST be in the parent because sub-agents cannot talk to the user).
4. **Fix loop** — invoke `/fix-bug` per iteration, run the commit-verification gate, re-scan, repeat.

**FOREGROUND MANDATORY — ALL Agent / Skill dispatches.** Never `run_in_background: true` on `/fix-bug` or `/commit-push`.

---

## Step 1: Scan the Queue

Run the Python scanner. The parent Opus context must execute this as its first action — do NOT dispatch a sub-agent for scanning.

```
Bash (foreground):
  python3 .claude/skills/fix-next-bug/bug_queue_scan.py --json
```

Parse the JSON. Key fields:

- `queue_empty` — boolean; true iff no open, non-excluded bugs
- `selected` — `{ bug_id, severity, title, repro, subsystem }` or `null`
- `remaining` — array of `{ bug_id, severity, title }` after the selected
- `total_open_candidates` — size of the candidate queue
- `total_excluded` — count of `- [ ]` entries filtered by lifecycle markers

### Empty queue

If `queue_empty: true`:

```
No open bugs in the tracker. All clear!
```

Stop — nothing to do.

### Non-empty queue — print the queue display

Print a verbatim queue display to the user (scope: roughly what the Python text mode would render; you can also run the scanner without `--json` for a pre-rendered version):

```
Selected: [BUG-XX-NNN][severity] title
  Repro: …
  Subsystem: …

Remaining queue (N bugs):
  1. [BUG-XX-NNN][severity] title
  2. …
  … and M more
```

The user MUST see this block before the mode question.

---

## Step 2: Blast-Radius Preview (Optional)

Before the mode question, attempt a lightweight blast-radius preview on the selected bug's repro symbol.

Follow the canonical intel-summary injection protocol:

@.claude/skills/dual-tpr/compose-intel-summary.md

Per SSOT Step F — `/fix-next-bug` uses `callers "<repro symbol>" --repo ori` as a lightweight blast-radius preview. If `scripts/intel-query.sh status` returns unavailable, skip this and omit the preview line silently.

If the query returns results, append one line to the queue display:

```
  Blast radius: <symbol> called by N sites across M modules
```

This is a **preview only** — `/fix-bug` Phase 1 (investigation) runs its own full intelligence queries during the fix.

---

## Step 3: Choose Mode (Parent — Opus)

`AskUserQuestion` runs in the parent context where the user can respond.

- **Question**: `Ready to start with: [BUG-XX-NNN][severity] title\n\nHow would you like to proceed?`
- **Options**:
  - `One at a time` — Fix this bug, then ask before each next bug
  - `Fix all bugs non-stop` — Loop through ALL open bugs automatically, zero interaction, no pauses

Record the choice.

---

## Step 4: Enter the Fix Loop

### Step 4.A — Interactive Mode

1. Invoke `Skill(fix-bug, args: "BUG-XX-NNN")` (no `--autopilot` flag). MUST use the Skill tool — never inline the /fix-bug workflow.
2. Let `/fix-bug` run its complete workflow — do NOT shortcut any phase.
3. After `/fix-bug` returns, run the **Commit Verification Gate** (Step 5).
4. `AskUserQuestion`:
   - **Question**: `Fix complete for [BUG-XX-NNN].\n\nNext bug in queue: [BUG-YY-MMM][severity] title\n{K-1} more bugs remaining after that.\n\nContinue with the next bug?`
   - **Options**: `Yes`, `No`, `Skip`
5. Handle the response:
   - `Yes` → re-scan (Step 6), pick the new highest priority, go to Step 4.A step 1.
   - `Skip` → re-scan with `--skip-ids BUG-XX-NNN,...` (accumulate the skip list across the session), pick the next one, go to Step 4.A step 1.
   - `No` → print the Final Report (Step 7) and stop.

### Step 4.B — Autopilot Mode

**This mode runs until the bug queue is empty or the user manually interrupts. NOTHING ELSE STOPS IT.**

Before entering the loop, create a persistent reminder task via `TaskCreate`:
- **Subject**: `AUTOPILOT: Do NOT stop until bug queue is empty`
- **Description**: `After EVERY /fix-bug outcome (fixed, escalated, blocked, OBE): commit gate → re-scan → pick next bug. The session summary is ONLY printed when re-scan returns queue_empty: true. There is NO 'natural stopping point.' The count of bugs processed is irrelevant — only queue_empty is the exit condition. If you are about to write a session summary, STOP and check: is queue_empty true? If no, pick the next bug.`

Keep this task `in_progress` for the entire autopilot session. Mark it `completed` only when printing the final report.

**CRITICAL: This is the ONLY `TaskCreate` task for the entire autopilot session.**

**Autopilot loop:**

1. Invoke `Skill(fix-bug, args: "--autopilot BUG-XX-NNN")`. The `--autopilot` flag tells `/fix-bug` to run zero-interaction with full rigor.
2. Run the **Commit Verification Gate** (Step 5).
3. **Immediately re-scan** (Step 6). Do NOT output a summary, do NOT pause, do NOT reflect.
4. If `queue_empty: false` in the re-scan, pick the next bug and go to step 1.
5. If `queue_empty: true`, **ONLY THEN** stop, mark the TaskCreate as completed, and print the Final Report (Step 7).

**BANNED in autopilot — NOT valid reasons to stop:**
- "Session summary" / "progress report" mid-loop — summary only at queue_empty.
- "Natural stopping point" — doesn't exist.
- "Already processed N bugs" — count is irrelevant; queue_empty is the only exit.
- "Bug was complex / couldn't fix" — mark escalated or blocked, then CONTINUE.
- "Bug was latent / OBE" — mark it, then CONTINUE.

**All `/fix-bug` outcomes continue the loop:**
- **Fixed** → continue
- **Escalated** (marked `Escalated: requires plan — {reason}` in autopilot) → continue
- **Blocked** → continue
- **OBE** → continue

**Consensus deadlocks** (autopilot): `/fix-bug` Phase 1.75 may deadlock after 3 `/tp-help` rounds. It proceeds with Claude's best-grounded approach and flags it. These MUST appear in the final report so the user can audit.

---

## Step 5: Commit Verification Gate (After EVERY Fix)

After `/fix-bug` completes (in EITHER mode), before anything else:

1. `git status` — check for uncommitted changes.
2. If uncommitted changes exist:
   - `Skill(commit-push)` to commit them.
   - Verify clean `git status` after.
3. If already clean, proceed.

**Non-negotiable.** A fix that isn't committed doesn't exist. Never proceed to the next bug with uncommitted work.

---

## Step 6: Re-scan for Next Iteration

Re-run the scanner to get a fresh queue. State may have changed — OBE resolutions, new bugs filed, escalations.

```
Bash (foreground):
  python3 .claude/skills/fix-next-bug/bug_queue_scan.py --json \
    [--skip-ids BUG-XX-NNN,BUG-YY-MMM]
```

Pass `--skip-ids` accumulated across the session if the user has skipped bugs in interactive mode.

---

## Handling Plan Escalation

When `/fix-bug` determines a bug needs a plan:

- **Interactive mode**: `/fix-bug` invokes `/create-plan` normally. After it returns, ask to continue to the next bug (Step 4.A step 4).
- **Autopilot mode**: `/fix-bug` marks the bug entry with `Escalated: requires plan — {reason}`. Run the Commit Verification Gate (the entry update needs committing), then immediately continue to the next bug. The user creates the plan after the autopilot session ends.

Escalated and blocked bugs are excluded by `bug_queue_scan.py`'s lifecycle-marker filter — they won't appear in re-scans.

---

## Step 7: Final Report

**Generated ONLY when the queue is empty OR the user manually stops.** NEVER generate mid-loop.

```
## Fix Next Bug — Session Summary

Mode: {interactive | autopilot}
Bugs processed this session: {total}

Fixed: {N}
  - [BUG-XX-NNN][severity] title — fixed

Escalated to plans (interactive — plan created): {N}
  - [BUG-XX-NNN][severity] title — escalated to plans/{plan-name}/

Escalated (autopilot — requires plan, user action needed): {N}
  - [BUG-XX-NNN][severity] title — requires plan: {reason}

Blocked (prerequisite missing): {N}
  - [BUG-XX-NNN][severity] title — blocked: {reason}

Resolved as OBE: {N}
  - [BUG-XX-NNN][severity] title — already fixed

{If any autopilot consensus deadlocks:}
Consensus deadlocks (autopilot — require user audit): {N}
  - [BUG-XX-NNN][severity] title — Phase 1.75 consensus deadlocked after 3 /tp-help rounds;
    proceeded with Claude's best-grounded approach. See fix-BUG-XX-NNN.md § 1.5 Round 3 for details.

{If any skipped (interactive mode only):}
Skipped: {N}
  - [BUG-XX-NNN][severity] title — skipped

Remaining open bugs: {N}
```

**Consensus deadlocks are load-bearing in the final report.** In autopilot mode, this is the only surfacing point. If a consensus-deadlocked fix later proves wrong, the user's remediation path is to read the fix section's § 1.5 Round 3 entry.

---

## Key Rules

- **Python scanner, not Sonnet sub-agent** — queue scanning is deterministic mechanical work. The scanner runs in ~35 ms for near-zero tokens.
- **AskUserQuestion lives in the parent** — sub-agents cannot talk to the user. The mode prompt MUST be issued by the Opus parent context.
- **Always re-scan** before picking the next bug — queue state is dynamic.
- **Full `/fix-bug` rigor** — every bug goes through the complete workflow via the Skill tool, no shortcuts.
- **Never skip phases** — investigation, TDD, implementation, TPR, hygiene — all mandatory per `/fix-bug`.
- **Mode is chosen once** — at the start; not after each bug.
- **Autopilot = zero interaction, zero stopping** — no questions, no confirmations, no pauses, no mid-loop summaries.
- **Every `/fix-bug` outcome continues the loop** — fixed, escalated, blocked, OBE.
- **The session summary IS the exit** — generating it means the loop is over. NEVER generate unless `queue_empty: true` OR user stopped.
- **Flaky tests ARE bugs** — do NOT retry and move on. Research the root cause. File via `/add-bug` if discovered during another fix; fix immediately if blocking.
- **NEVER investigate "pre-existing?"** — no git archaeology. The only question is: is it fixed?

## Files in this skill

- `SKILL.md` (this file) — full skill protocol (parent-driven, no sub-agent for scanning).
- `bug_queue_scan.py` — Python scanner. Emits JSON or text. Priority: severity > section > ordinal. Filters lifecycle markers.
- `bug-queue-scan.sh` — thin shell shim delegating to the Python scanner.
