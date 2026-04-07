---
name: fix-bug
description: Fix a bug with full plan-section rigor — root cause analysis, TDD matrix, implementation, TPR review, and impl-hygiene review. Creates a fix-BUG-XX-NNN.md file in the bug tracker. TRIGGER when picking up a bug for fixing from /review-bugs or when explicitly told to fix a specific bug.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash, Agent, AskUserQuestion, Skill
argument-hint: "[BUG-XX-NNN or description]"
---

# Fix Bug

Fix a bug with the same rigor as a plan section: investigation, root cause analysis, TDD-first testing, implementation, and full completion checklist including `/tpr-review` and `/impl-hygiene-review`.

## Usage

```
/fix-bug BUG-04-033
/fix-bug BUG-02-005
/fix-bug [description of the bug if ID unknown]
/fix-bug --autopilot BUG-04-033    (called by /fix-next-bug in autopilot mode)
```

## Phase -1: Ground Truth — MANDATORY, EVERY TIME

**Before ANY other work**, re-read the project's CLAUDE.md files to ground yourself in the rules:

1. **Read `CLAUDE.md`** (project root) — the One Rule, ownership, deferral, TDD, fix completeness, stabilization discipline, coding guidelines, commands
2. **Read `.claude/rules/` files relevant to the bug's subsystem** — e.g., `.claude/rules/tests.md` for test patterns, `.claude/rules/registry.md` for registry bugs, `.claude/rules/arc.md` for ARC/memory bugs

This is NOT optional. Context drift across long sessions causes rule violations. Re-reading ensures every fix follows the same standard regardless of when it runs in a session.

## Autopilot Mode

When invoked with `--autopilot` (by `/fix-next-bug` in autopilot mode), the following rules apply:

- **ZERO user interaction** — do NOT use `AskUserQuestion` for any reason. Make the correct decision yourself based on CLAUDE.md rules and the spec.
- **No pausing, no stopping** — continue through every phase until the bug is fully fixed, reviewed, and committed. No matter the scope. If the correct fix touches 15 files across 4 crates, that IS the fix. Do it.
- **No hacks, no shortcuts, no workarounds** — follow CLAUDE.md's "The One Rule: Correctness Above All" with absolute fidelity. The correct fix is the ONLY fix. Scope, effort, complexity are irrelevant.
- **Full rigor is non-negotiable** — every phase (investigation, TDD matrix, implementation, TPR, hygiene) runs to completion. Autopilot means autonomous, NOT abbreviated.
- **New bugs → `/add-bug`** — file them and keep going. The `/fix-next-bug` loop will pick them up on the next re-scan.
- **Fix interference → handle it** — if a newly discovered bug blocks this fix, shelve, fix the blocker first via a nested `/fix-bug --autopilot`, then resume.
- **Do NOT use `TaskCreate`** — the `/fix-next-bug` autopilot loop maintains a single persistent reminder task. Creating additional tasks would signal work completion and cause the loop to terminate prematurely. Track your progress via the fix section file, not tasks.

When NOT in autopilot mode, `AskUserQuestion` is available as normal for genuinely ambiguous decisions.

## Flaky Tests ARE Bugs — ALWAYS

If a test passes sometimes and fails sometimes, that is a bug — not noise. Do NOT retry and move on. Research the root cause (race condition, timing dependency, temp file collision, state leakage, non-deterministic ordering, filesystem caching) and fix it so the test is deterministic. If discovered during a different bug fix, file via `/add-bug` immediately.

## NEVER Investigate "Pre-existing?" — BANNED

Do NOT use `git checkout`, `git stash`, `git bisect`, `git log --diff-filter`, or any git archaeology to determine whether a bug or test failure existed before your changes. **It does not matter.** The question "was this pre-existing?" is banned. The only valid question is: "is it fixed?"

Spending time checking out old commits to see if something "was already broken" produces zero value. It's broken now → fix it now. No exceptions, no "just to understand the timeline." The timeline is irrelevant. The fix is everything.

## Workflow

### Phase 0: Locate the Bug

1. **Find the bug entry** — read the bug-tracker section files to find the bug:
   ```
   plans/bug-tracker/section-{NN}-*.md
   ```
   If the user gave a description instead of an ID, search all section files for a match.

2. **If no bug entry exists yet** — create one now using the `/add-bug` entry format. This is common when a blocking bug is discovered during plan work and goes straight to `/fix-bug`. Write the entry in the appropriate section file before proceeding. Assign the next sequential ID.

3. **Extract context** from the entry: severity, repro, subsystem, source, any notes or cross-refs.

4. **Check for an existing fix file** — if `plans/bug-tracker/fix-BUG-XX-NNN.md` already exists, resume from where it left off instead of creating a new one.

### Phase 1: Investigation (Research Before Writing)

**Do NOT start coding yet.** Understand the bug first.

1. **Read the affected code** — not just the file listed in the bug entry, but the surrounding context. Understand the data flow, the invariants, and the phase boundaries. Read at least 2-3 files that interact with the buggy code.

2. **Reproduce the bug** — run the repro from the bug entry. If it passes now, the bug may be OBE — verify and mark resolved if so.

3. **Consult the spec** — check `docs/ori_lang/v2026/spec/` for the intended behavior. The spec is authoritative.

4. **Root cause analysis** — trace the bug to its *root cause*, not just the symptom. Follow the chain:
   - What was observed? (symptom)
   - What code produced this? (proximate cause)
   - Why did that code do the wrong thing? (root cause)
   - Is the root cause localized or systemic? (blast radius)

5. **Check reference compilers** (if the bug involves a design question) — consult `~/projects/reference_repos/lang_repos/` for prior art on how other compilers handle this case.

6. **Identify all affected code paths** — the fix may need changes in multiple places. List every file and function that needs to change.

### Phase 1.5: Scope Assessment — MANDATORY GATE

After investigation, assess whether this bug is a **point fix** (inline bug fix) or requires a **plan** (architectural change, cross-system redesign, multi-section work).

**Point fix criteria** (ALL must be true):
- Root cause is localized to 1-3 files
- Fix does not require redesigning an existing subsystem
- Fix does not span multiple plan sections or roadmap areas
- The "fix approach" can be described in a paragraph, not a document

**Plan escalation criteria** (ANY triggers escalation):
- Root cause is systemic — affects 4+ files across multiple crates/subsystems
- Fix requires architectural change or redesigning an existing system
- Fix involves new abstractions, new pipeline passes, or new data structures
- Fix naturally belongs as a roadmap section or plan with multiple phases
- You cannot write a TDD matrix because the fix approach itself is unclear

**If point fix** → proceed to Phase 2.

**If plan needed** → escalate:
1. **Do NOT proceed to Phase 2.** Do NOT write code. Do NOT create a fix section file.
2. **Run `/create-plan`** — create a proper plan for this work. The plan IS the deliverable.
3. **Update the bug entry** in the section file — add a note: `Escalated to plan: plans/{plan-name}/`. Do NOT mark it `[x]` — it is not fixed, it is planned.
4. **Report the escalation** to the user:
   ```
   Escalated: [BUG-{section}-{ordinal}][{severity}] {title}
     Reason: {why this needs a plan, not an inline fix}
     Plan created: plans/{plan-name}/
     Bug entry updated with plan reference
   ```
5. **Stop.** This bug is done for `/fix-bug`. The plan will be worked separately.

**If blocked/latent** (prerequisite feature doesn't exist yet):
1. **Do NOT proceed to Phase 2.** The bug cannot be fixed because the infrastructure doesn't exist.
2. **Update the bug entry** — add a note explaining the blocker: `Blocked: {reason — e.g., "LLVM FFI codegen not yet implemented"}`. Do NOT mark it `[x]`.
3. **Report the blocker:**
   ```
   Blocked: [BUG-{section}-{ordinal}][{severity}] {title}
     Reason: {what prerequisite is missing}
     Blocker: {roadmap section, plan, or feature that must land first}
     Bug entry updated with blocker reference
   ```
4. **Stop.** This bug is done for `/fix-bug` until the blocker is resolved.

**In autopilot mode**: the same rules apply for ALL non-point-fix outcomes — escalate to `/create-plan`, mark as blocked, or mark as OBE, then return to the caller (`/fix-next-bug`) which will immediately pick the next bug. Autopilot means autonomous, not reckless. Creating a plan for a large-scope bug or correctly identifying a blocker IS the correct autonomous decision. The key: always return to the caller — never just "document and stop."

### Phase 2: Create the Fix Section File

Create `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md` using the template in [fix-section-template.md](fix-section-template.md). This file is the plan section for this bug fix — it documents the investigation, drives the TDD process, and tracks completion.

**IMPORTANT:** Write the fix file BEFORE writing any code. The fix file is the plan; the plan comes before the implementation.

### Phase 3: TDD — Write Tests First

Follow the TDD discipline from the fix section:

1. **Write all matrix tests** from the fix section's TDD plan
2. **Run them and verify they fail** — if any pass, either the bug is OBE or the test doesn't test what you think
3. **If any test reveals a DIFFERENT bug** (unexpected failure, wrong error message, crash in unrelated code path) — **STOP and invoke `/add-bug`** to file it immediately. Then decide: if it blocks this fix, switch to `/fix-bug` for the new bug first; otherwise continue with the current fix.
4. **Do NOT proceed to implementation until all tests are written and verified failing**

Update the fix section: check off each test as written, note test file paths.

### Phase 4: Implementation

1. **Implement the fix** as described in the fix section
2. **Run the test matrix** — all previously-failing tests should now pass WITHOUT modification
3. If tests need modification after the fix, either the tests were wrong or the fix was wrong — investigate
4. **Run the full suite**: `timeout 150 ./test-all.sh`
5. **If test-all reveals new failures unrelated to this fix** — invoke `/add-bug` for each one immediately. These are bugs your fix surfaced (interference) or pre-existing bugs you're now seeing. File them, don't ignore them.
6. **Commit via `/commit-push`** — NEVER commit directly with `git commit`. All changes must be committed before review.

Update the fix section: check off implementation tasks, note any discoveries.

### Phase 5: Completion Checklist

Work through the completion checklist in order:

1. **Verify all matrix items** — tests, builds, leak checks
2. **Update the bug entry** in the section file — mark `- [x]` with resolution details
3. **Update the fix section** — set status to `complete`, fill in exit criteria
4. **Update the overview** — adjust open bug count
5. **Run `/tpr-review`** — independent third-party review of the fix
6. **Handle TPR findings** — fix any issues found, re-run if needed
7. **Run `/impl-hygiene-review`** — AFTER TPR is clean
8. **Run `/improve-tooling` retrospectively** — MANDATORY at fix close, AFTER both reviews are clean. Bug fixes are the richest source of tooling gaps because you've just spent time fighting the diagnostic surface during root cause analysis. Reflect on: which `diagnostics/` scripts you ran, where you added ad-hoc `dbg!`/`tracing` calls (and what each one was looking for), where the original failure message was unhelpful, where matrix tests were tedious because helpers were missing, what instrumentation would have made the bug obvious in 1 minute instead of 30. Capture every gap you noticed. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push` (e.g., `tools(diagnostics): add --bb-level RC tracking — surfaced by BUG-XX-NNN retrospective`). The retrospective is mandatory even when nothing felt painful — this is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full look-back protocol.

### Phase 6: Report

Report the fix to the user:
```
Fixed: [BUG-{section}-{ordinal}][{severity}] {title}
  Fix section: plans/bug-tracker/fix-BUG-{section}-{ordinal}.md
  Tests added: {count} ({test file paths})
  Files changed: {list}
  TPR: {passed | findings resolved}
  Hygiene: {passed | findings resolved}
```

## Scaling Rules

### Simple bugs (1-2 files, obvious fix)
- The fix section is still MANDATORY — but sections 1-3 can be brief
- TDD matrix can be smaller if the bug is narrowly scoped — but semantic + negative pins are ALWAYS required
- Completion checklist is NEVER shortened for critical/high severity
- For medium severity: TPR expected, hygiene recommended
- For low severity: TPR + hygiene recommended but not required — document if skipped

### Complex bugs (3+ files, architectural)
- Full investigation with reference compilers
- Multiple design approaches with tradeoffs documented
- TPR checkpoints during implementation if it spans multiple logical steps
- Consider whether the fix belongs in a proper plan section instead of a bug fix

### Clusters (multiple related bugs)
- If 2+ bugs share a root cause, create a single fix section covering all of them
- Name it after the primary bug: `fix-BUG-04-033.md` with the others listed as "Also fixes: BUG-04-034, BUG-04-035"
- The TDD matrix covers ALL bugs in the cluster

## Discovering New Bugs During a Fix — MANDATORY

Fixing a bug often uncovers other bugs. This is expected and valuable. **You MUST invoke `/add-bug` immediately** whenever you encounter a new bug during any phase of this workflow:

- **Phase 1 (Investigation)**: reading adjacent code reveals a different issue
- **Phase 3 (TDD)**: a test exposes an unexpected failure in a different code path
- **Phase 4 (Implementation)**: `test-all.sh` shows new failures unrelated to this fix
- **Phase 5 (Completion)**: TPR or hygiene review surfaces additional issues

Do NOT gloss over these as "not my bug" or "separate issue" — file them via `/add-bug` so they enter the tracker. Then decide: if the new bug **blocks** this fix (interference), shelve this fix and `/fix-bug` the blocker first (per fix-interference rules). If it's independent, continue with the current fix — the new bug is now tracked and will be picked up by `/fix-next-bug` or `/review-bugs`.

## Integration Points

- **`/add-bug`** — invoke during ANY phase when a new bug is discovered. This is the most common integration — fixing bugs surfaces more bugs, and every one must be filed.
- **`/fix-next-bug`** — orchestrates picking bugs from the tracker and invoking this skill in priority order
- **`/review-bugs`** — triages bugs; recommends `/fix-bug` for selected bugs
- **`/create-plan`** — MANDATORY when Phase 1.5 determines the bug needs a plan instead of an inline fix. The plan IS the deliverable — do NOT skip this.
- **`/commit-push`** — used in Phase 4 to commit changes before review
- **`/tpr-review`** — called during completion checklist
- **`/impl-hygiene-review`** — called during completion checklist, AFTER TPR
