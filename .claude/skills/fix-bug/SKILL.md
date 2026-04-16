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

## How this skill runs

SKILL.md has two parts:

1. **Part 1 — Thin dispatcher**: Sends Phase 0 (bug context setup) to a Sonnet sub-agent via `workflow.md`. Sonnet locates the bug entry, extracts context, checks for existing fix files, and returns a structured handoff. No code reading.

2. **Part 2 — Opus workflow**: After the handoff, the parent (Opus) runs Phases -1 through 6 inline. Investigation, TDD, and implementation are done directly — not via sub-agents.

**FOREGROUND MANDATORY — ALL Agent dispatches** in this skill.

---

## Part 1: Bug Context Setup (Phase 0 via Sonnet)

**This is the ONLY action before reading the handoff.** Substitute `<ARGS>` with the user's `/fix-bug` arguments:

```
Agent({
  description: "fix-bug Phase 0: bug context setup",
  subagent_type: "general-purpose",
  model: "sonnet",
  prompt: `
You are the bug-context agent for /fix-bug. Read .claude/skills/fix-bug/workflow.md
in full and execute Phase 0.

Bug ID or description from the user: <ARGS>

Rules:
- Execute Phase 0 Steps 1-5 only. Return the handoff block in the EXACT format
  specified at the end of workflow.md.
- Read plans/bug-tracker/ files ONLY. Never open compiler/, library/, tests/.
- Do NOT investigate root cause, write code, or proceed beyond Phase 0.
  `
})
```

**Do not locate the bug yourself, read section files, or extract context.** The dispatch is the only action in Part 1.

## Part 2: After the Handoff

Read the handoff and act based on its status flags.

**ERROR handoff (bug not found)**:
- Report: "Bug `{ID}` not found in the tracker." If description was given, offer `/add-bug`.
- Stop.

**`Already resolved: yes`**:
- The entry is `- [x]`. It may have been fixed already. Report to user and stop.

**Lifecycle markers present** (`Escalated:`, `Blocked:`, `<!-- blocked-by:`):
- This bug is not actionable by `/fix-bug`. Report the markers to the user and stop.

**Resume mode (`yes — pick up at Phase N`)**:
- Skip Phases 0 through (N-1). Resume at Phase N using the existing fix section file.
- Re-read the fix section file for full context before resuming.

**Otherwise — proceed to Phase -1 with the handoff context.**

---

## Autopilot Mode

When invoked with `--autopilot` (by `/fix-next-bug` in autopilot mode), the following rules apply:

- **ZERO user interaction** — do NOT use `AskUserQuestion` for any reason. Make the correct decision yourself based on CLAUDE.md rules and the spec.
- **No pausing, no stopping** — continue through every phase until the bug is fully fixed, reviewed, and committed. No matter the scope. If the correct fix touches 15 files across 4 crates, that IS the fix. Do it.
- **No hacks, no shortcuts, no workarounds** — follow CLAUDE.md's "The One Rule: Correctness Above All" with absolute fidelity. The correct fix is the ONLY fix. Scope, effort, complexity are irrelevant.
- **Full rigor is non-negotiable** — every phase (investigation, TDD matrix, implementation, TPR, hygiene) runs to completion. Autopilot means autonomous, NOT abbreviated.
- **New bugs → `/add-bug`** — file them and keep going. The `/fix-next-bug` loop will pick them up on the next re-scan.
- **Fix interference → handle it** — if a newly discovered bug blocks this fix, **shelve** the current fix, fix the blocker first via a nested `/fix-bug --autopilot`, then resume. "Shelve" means: (1) commit any partial work via `/commit-push` with message `chore(fix-bug): shelve BUG-XX-NNN — blocked by BUG-YY-MMM`, (2) note the WIP commit hash in the fix section file's implementation notes, (3) after the nested fix returns, continue from where you left off — the WIP commit keeps your partial changes safe and separate from the nested fix's commits.
- **Do NOT use `TaskCreate`** — the `/fix-next-bug` autopilot loop maintains a single persistent reminder task. Creating additional tasks would signal work completion and cause the loop to terminate prematurely. Track your progress via the fix section file, not tasks.

When NOT in autopilot mode, `AskUserQuestion` is available as normal for genuinely ambiguous decisions.

## Flaky Tests ARE Bugs — ALWAYS

If a test passes sometimes and fails sometimes, that is a bug — not noise. Do NOT retry and move on. Research the root cause (race condition, timing dependency, temp file collision, state leakage, non-deterministic ordering, filesystem caching) and fix it so the test is deterministic. If discovered during a different bug fix, file via `/add-bug` immediately.

## NEVER Investigate "Pre-existing?" — BANNED

Do NOT use `git checkout`, `git stash`, `git bisect`, `git log --diff-filter`, or any git archaeology to determine whether a bug or test failure existed before your changes. **It does not matter.** The question "was this pre-existing?" is banned. The only valid question is: "is it fixed?"

---

## Phase -1: Ground Truth — MANDATORY, EVERY TIME

**Before ANY investigation work**, re-read the project's CLAUDE.md files to ground yourself in the rules:

1. **Read `CLAUDE.md`** (project root) — the One Rule, ownership, deferral, TDD, fix completeness, stabilization discipline, coding guidelines, commands
2. **Read `.claude/rules/` files relevant to the bug's subsystem** — e.g., `.claude/rules/tests.md` for test patterns, `.claude/rules/registry.md` for registry bugs, `.claude/rules/arc.md` for ARC/memory bugs

This is NOT optional. Context drift across long sessions causes rule violations. Re-reading ensures every fix follows the same standard regardless of when it runs in a session.

## Phase 0: Locate the Bug (DONE by Sonnet — use handoff)

The handoff from the Sonnet sub-agent already contains the full bug entry text, status flags, and context. Use it directly:

1. **Bug entry** — use `Full bug entry text` from the handoff
2. **If no bug entry exists yet** — the handoff says "not found — will create from description". Create one now using the `/add-bug` entry format. Write the entry in the appropriate section file. Assign the next sequential ID.
3. **Check for existing fix file** — already checked by Sonnet. `Existing fix file` field in handoff.
4. **Extract context** from the handoff: severity, repro, subsystem, source, any notes or cross-refs.

## Phase 1: Investigation (Research Before Writing)

**Do NOT start coding yet.** Understand the bug first.

1. **Read the affected code** — not just the file listed in the bug entry, but the surrounding context. Understand the data flow, the invariants, and the phase boundaries. Read at least 2-3 files that interact with the buggy code.

2. **Reproduce the bug** — run the repro from the bug entry. If it passes now, the bug may be OBE — follow the OBE exit path below.

   **OBE Exit Path** (bug already fixed by other work):
   1. Verify the fix is real — run the repro AND check that the underlying code path is actually corrected, not just coincidentally passing
   2. Update the bug entry in the section file using the canonical OBE resolution format from `plans/bug-tracker/00-overview.md`:
      ```markdown
      - [x] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}**
        Resolved: OBE on {YYYY-MM-DD}. {What fixed it — commit, plan, or rewrite}.
      ```
   3. Update `plans/bug-tracker/00-overview.md` Quick Reference — decrement the open bug count for this subsystem
   4. Commit via `/commit-push` (the entry update is a deliverable — it must be committed)
   5. **Return OBE outcome** — report to the caller (user or `/fix-next-bug`):
      ```
      OBE: [BUG-{section}-{ordinal}][{severity}] {title}
        Resolved by: {what fixed it}
        Bug entry updated, overview count decremented.
      ```
   6. **Stop** — do NOT proceed to Phase 1.5 or beyond. The bug is resolved.

3. **Consult the spec** — check `docs/ori_lang/v2026/spec/` for the intended behavior. The spec is authoritative.

4. **Root cause analysis** — trace the bug to its *root cause*, not just the symptom. Follow the chain:
   - What was observed? (symptom)
   - What code produced this? (proximate cause)
   - Why did that code do the wrong thing? (root cause)
   - Is the root cause localized or systemic? (blast radius)

5. **Check reference compilers** (if the bug involves a design question):

   **5a. Intelligence Graph Query** — If the intelligence graph is available, query for similar bugs.

   Follow the canonical intel-summary injection protocol:

   @.claude/skills/query-intel/compose-intel-summary.md

   **Fix-bug extension** (per SSOT Step F — Phase 1 investigation), additional queries beyond the SSOT base set:
   - `scripts/intel-query.sh --human search "<bug description keywords>" --limit 5`
   - `scripts/intel-query.sh --human fixed "<bug category>" --repo rust,swift,koka,lean4 --limit 5`
   - `scripts/intel-query.sh --human similar "<buggy function or concept>" --repo rust,swift,koka,lean4 --limit 5`

   Look for: same failure mode in 2+ compilers, how they fixed it, what regressions it caused. Record relevant findings in the fix section's investigation notes. If intelligence is unavailable, skip 5a entirely and proceed to 5b.

   **5b. Manual Reference Compiler Inspection** — Consult `~/projects/reference_repos/lang_repos/` for prior art. Start with the repos and symbol hits returned by `similar` in 5a, then inspect the actual source code around those matches. Intelligence results from 5a narrow the search — check the repos and issues it flagged first — but always inspect the actual source code. This sub-step is MANDATORY for design-question bugs regardless of whether 5a produced results.

   If the bug is NOT a design question, skip Steps 5a and 5b entirely.

6. **Identify all affected code paths** — the fix may need changes in multiple places. List every file and function that needs to change.

## Phase 1.5: Scope Assessment — MANDATORY GATE

After investigation, assess whether this bug is a **point fix** (inline bug fix) or requires a **plan** (architectural change, cross-system redesign, multi-section work).

### Severity Reclassification — MANDATORY CHECK

Before scope assessment, **re-evaluate the bug's severity** based on what Phase 1 investigation revealed. The original severity from `/add-bug` was assigned with limited information — now you have root cause analysis, blast radius, and affected code paths.

**Reclassify upward when ANY of these are true:**
- **Blast radius is wider than expected** — the bug affects more code paths, types, or features than the original entry described
- **Root cause is in a complexity-elevated subsystem** (AIMS, CodeGen, LLVM, AOT, Runtime) but severity was rated `medium` or `low` — elevate to at least `high`
- **Silent corruption** — the bug produces wrong results without error/crash (more dangerous than a crash)
- **Cross-crate root cause** — the bug's fix requires changes in 3+ crates, indicating systemic scope
- **Downstream cascade** — fixing this bug would surface or interfere with other known bugs
- **Data loss risk** — the bug could cause incorrect compilation output that goes undetected at runtime

**Reclassify downward when:**
- Investigation reveals the bug is narrower than initially described (affects fewer cases, has natural guardrails)
- The "bug" is actually expected behavior per spec — mark OBE instead of reclassifying

**How to reclassify:**
1. Update the bug entry in `plans/bug-tracker/section-{NN}-*.md` — change the severity tag: `[{old}→{new}]` with a note: `Reclassified {date}: {one-line reason based on Phase 1 findings}`
2. Update the fix section frontmatter `severity:` field (if the fix section already exists from a prior attempt)
3. **Apply the new severity's treatment going forward** — this is the critical part. A bug reclassified from `medium` to `high` now gets mandatory Plan TPR (Phase 2.5). A bug in a complexity-elevated subsystem reclassified from `low` to `high` gets the full elevated treatment. The reclassified severity governs ALL downstream gates.

**The reclassification IS the investigation's deliverable** — it's not bureaucratic overhead. The original severity was a guess; the reclassified severity is informed by root cause analysis. All downstream decisions (Plan TPR gate, scaling rules, review intensity) flow from the *current* severity, not the original.

**In autopilot mode**: reclassify autonomously based on the criteria above. No `AskUserQuestion` needed — the criteria are objective.

### Scope Assessment

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

**If point fix** → proceed to Phase 1.6 (Create Fix Section File).

**If plan needed** → escalate:
1. **Do NOT proceed to Phase 1.6.** Do NOT write code. Do NOT create a fix section file.
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
1. **Do NOT proceed to Phase 1.6.** The bug cannot be fixed because the infrastructure doesn't exist.
2. **Update the bug entry** — add a note explaining the blocker: `Blocked: {reason — e.g., "LLVM FFI codegen not yet implemented"}`. Do NOT mark it `[x]`.
3. **Report the blocker:**
   ```
   Blocked: [BUG-{section}-{ordinal}][{severity}] {title}
     Reason: {what prerequisite is missing}
     Blocker: {roadmap section, plan, or feature that must land first}
     Bug entry updated with blocker reference
   ```
4. **Stop.** This bug is done for `/fix-bug` until the blocker is resolved.

**In autopilot mode**: the same rules apply for ALL non-point-fix outcomes, with one critical exception for `/create-plan`:

- **OBE** → follow the OBE exit path (Phase 1), return OBE outcome to caller
- **Blocked/latent** → update bug entry with `Blocked:` note, return blocked outcome to caller
- **Plan needed** → **do NOT invoke `/create-plan`** (it has mandatory interactive approval gates that conflict with autopilot's zero-interaction contract). Instead:
  1. Update the bug entry with `Escalated: requires plan — {brief reason why inline fix is insufficient}`
  2. Return escalated outcome to caller with the reason
  3. `/fix-next-bug` records this in the session summary; the user creates the plan after the autopilot run

When NOT in autopilot mode, invoke `/create-plan` normally per the escalation protocol above.

Autopilot means autonomous, not reckless. Correctly identifying that a bug needs a plan and deferring the plan creation to the interactive user IS the correct autonomous decision. The key: always return to the caller — never just "document and stop."

## Phase 1.6: Create Fix Section File — IMMEDIATELY After Scope Confirmation

**Create the fix section file NOW** — do NOT wait for `/tp-help` consensus. The fix file is the first user-visible artifact and must exist as soon as the investigation is complete and scope is confirmed.

Create `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md` using the template in [fix-section-template.md](fix-section-template.md). Fill in everything known so far:

- **Frontmatter**: all fields (bug ID, title, severity, status: `in-progress`, goal, subsystem, etc.)
- **§1 Root Cause Analysis**: populated from Phase 1 investigation findings (symptom, proximate cause, root cause, blast radius, affected files)
- **§1.5 Fix Consensus**: leave as `Pending — /tp-help consensus in Phase 1.75`
- **§2 TDD Matrix**: skeleton with section headers — fill in after consensus
- **§2.5 Fix Plan TPR Findings**: leave as gate placeholder — fill in during Phase 2.5 (or mark "Skipped" if gate criteria not met)
- **§3 Implementation**: skeleton with proposed approach from Phase 1 — may be revised by consensus
- **§R TPR Findings**: empty (populated during Phase 5)
- **§4 Completion Checklist**: full template from the fix-section-template

**Set frontmatter `status: in-progress`** — the fix is now actively being worked, even though implementation hasn't started.

## Phase 1.75: Fix Consensus (via /tp-help) — MANDATORY GATE

**Before writing tests or code, get independent dual-source consensus on the proposed fix approach.** This catches wrong-approach errors BEFORE they are locked into the test matrix or the implementation. The fix section file already exists (Phase 1.6) — this phase fills in its §1.5 Fix Consensus section.

This is NOT `/tp-help`'s usual "stuck help" use case — it is **design consensus**. You have investigation + root cause (Phase 1) + a confirmed point-fix scope (Phase 1.5) + a written fix plan (Phase 1.6) + a proposed approach. You are about to commit to implementation. Get Codex and Gemini to independently pressure-test the approach before you lock it in.

**Skip only when:** Phase 1.5 escalated to `/create-plan` or marked blocked. EVERY other bug runs through consensus.

1. **Articulate the proposed fix** — write out, in prose, for the `/tp-help` question:
   - Bug (one line) + root cause (Phase 1 output)
   - Affected files and what changes in each
   - Your proposed fix approach (what changes, where, why)
   - Alternatives you considered and why you rejected them

2. **Invoke `/tp-help`** with the articulation above as the question. Save the `$RUN` scratch dir path; you will cite it in § 1.5 of the fix section file.

3. **Independently verify every finding** against actual code per the `feedback_reviewer_grounding_and_trust.md` memory rule. Never trust reviewer claims blindly. Codex = HIGH trust (spot-check key claims), Gemini = LOWER trust (full verification — confabulation-prone). File:line cites are required in § 1.5 "Independent code verification".

4. **Reconcile** into one of three outcomes:
   - **Agreement** — Claude's approach + both reviewers converge → proceed to Phase 2
   - **Persuaded divergence** — reviewers propose a better approach, Claude verifies it against the code and adopts it → proceed to Phase 2 with the new approach
   - **Unpersuaded divergence** — Claude is not convinced by reviewers after code verification → run a follow-up `/tp-help` round with a counter-argument (include the prior round's responses and Claude's specific verification findings so the reviewers can refine)

5. **Convergence cap: 3 total `/tp-help` calls** (initial + up to 2 follow-up rounds). If still no convergence after round 3:
   - **Interactive mode**: escalate via `AskUserQuestion` with a summary of the deadlock — Claude's position, reviewers' positions, the specific disagreement, and why Claude cannot reconcile. The user breaks the tie.
   - **Autopilot mode**: document the deadlock in § 1.5 Fix Consensus → "Round 3" entry ("AUTOPILOT DEADLOCK"), then proceed with Claude's best-grounded approach. The deadlock MUST be flagged in the `/fix-next-bug` final session report so the user can audit after the autopilot run ends. Do NOT use `AskUserQuestion` (autopilot rule).

6. **Update the fix section file's §1.5 Fix Consensus** with the consensus outcome (per the template).

**Interaction with Phase 1.5**: If `/tp-help` reveals the bug is actually systemic (requires architectural change across 4+ files, new abstractions, cross-crate redesign), **return to Phase 1.5** and re-assess.

## Phase 2: Finalize the Fix Section File

The fix section file already exists from Phase 1.6. After `/tp-help` consensus (Phase 1.75), finalize it:

1. **Update §1.5 Fix Consensus** — already done in Phase 1.75 step 6
2. **Fill in §2 TDD Matrix** — design the test matrix based on the agreed approach. List all tests that will be written in Phase 3.
3. **Fill in §3 Implementation** — write the concrete implementation plan based on the consensus-agreed approach. Include code sketches.
4. **Verify §1 Root Cause Analysis** is still accurate after consensus — if `/tp-help` revealed new affected files or a refined root cause, update §1.

**IMPORTANT:** The fix file must be complete BEFORE writing any code. The file is the plan; the plan comes before the implementation.

## Phase 2.5: Fix Plan TPR — Adversarial Plan Review

**After the fix section is finalized (Phase 2) but BEFORE writing tests or code, run an adversarial `/tpr-review` on the fix PLAN itself.** This stress-tests the plan for edge cases, downstream impacts, and TDD gaps rather than helping converge on an approach.

**Why both `/tp-help` AND Plan TPR exist — they catch different failure classes:**

| Gate | Mode | Catches | Incentive |
|------|------|---------|-----------|
| Phase 1.75 `/tp-help` | **Consultative** — "help me find the right approach" | Wrong direction entirely, missed alternatives | Converge to agreement |
| Phase 2.5 Plan TPR | **Adversarial** — "try to break this plan" | Edge cases in TDD matrix, downstream impacts, missed interactions, architectural risks | Find flaws |

### Trigger Gate: Severity + Subsystem Complexity

**MANDATORY (always run Plan TPR) when ANY of these are true:**
- Bug severity is `critical` or `high`
- Bug is in a **complexity-elevated subsystem** (regardless of severity):

| Subsystem | Crate/Path Pattern | Why Elevated |
|-----------|-------------------|--------------|
| **AIMS** | `compiler/ori_arc/`, `compiler/ori_rt/src/rc/` | 7-dimension lattice, interprocedural fixpoints, pass ordering dependencies, RC invariants |
| **CodeGen** | `compiler/ori_llvm/src/codegen/` | IR generation touches types, ABI, optimization levels; silent wrong-output bugs |
| **LLVM integration** | `compiler/ori_llvm/` (broadly) | Debug/release divergence (FastISel vs SelectionDAG), LLVM pass interactions |
| **AOT** | `compiler/ori_llvm/tests/aot/`, AOT compilation paths | End-to-end: type system → IR → LLVM → linking → runtime; failures can be anywhere in the chain |
| **Runtime** | `compiler/ori_rt/` | FFI boundary, RC internals, platform-specific behavior; bugs here corrupt memory silently |

**SKIP (Plan TPR not required) when ALL of these are true:**
- Bug severity is `medium` or `low`
- Bug is NOT in a complexity-elevated subsystem
- `/tp-help` consensus (Phase 1.75) converged in round 1 with agreement (no divergence)

When skipped, record in the fix section's §2.5: `Plan TPR: Skipped — {severity} severity, non-elevated subsystem, round-1 consensus.`

### How to Run Plan TPR

1. **Invoke `/tpr-review`** via the `Skill` tool (NOT via `Agent`) with the fix section file as the review target: `Skill({skill: "tpr-review"})`. Using Agent would swallow all round summaries.
2. **Handle findings** — fix issues in the plan. Re-run Plan TPR if findings were significant.
3. **Update §2.5 Plan TPR Findings** in the fix section with the findings and resolutions.
4. **Proceed to Phase 3** only when Plan TPR is clean.

## Phase 3: TDD — Write Tests First

Follow the TDD discipline from the fix section:

1. **Write all matrix tests** from the fix section's TDD plan
2. **Run them and verify they fail** — if any pass, either the bug is OBE or the test doesn't test what you think
3. **If any test reveals a DIFFERENT bug** (unexpected failure, wrong error message, crash in unrelated code path) — **STOP and invoke `/add-bug`** to file it immediately. Then decide: if it blocks this fix, switch to `/fix-bug` for the new bug first; otherwise continue with the current fix.
4. **Do NOT proceed to implementation until all tests are written and verified failing**

Update the fix section: check off each test as written, note test file paths.

## Phase 4: Implementation

1. **Implement the fix** as described in the fix section
2. **Run the test matrix** — all previously-failing tests should now pass WITHOUT modification
3. If tests need modification after the fix, either the tests were wrong or the fix was wrong — investigate
4. **Run the full suite**: `timeout 150 ./test-all.sh`
5. **If test-all reveals new failures unrelated to this fix** — invoke `/add-bug` for each one immediately.
6. **Capability regression check — MANDATORY.** Ask: "Did this fix **disable, remove, or weaken** any existing capability — an optimization, analysis pass, feature, or code path — to achieve soundness?" If YES:
   - The disabled capability MUST have a concrete re-enablement path tracked as a `- [ ]` checkbox in the owning plan. If no owning plan exists, create a bug-tracker entry via `/add-bug` with the re-enablement scope.
   - The fix section's §3 Implementation MUST document: (a) what was disabled, (b) why (the soundness argument), (c) the tracked re-enablement item (plan path + section + checkbox text).
   - Any `#[ignore]`'d tests MUST reference the re-enablement item so they are un-ignored when the capability returns.
   - **A fix that disables a capability without tracking re-enablement is a deferral** — it violates CLAUDE.md §Zero Deferral.
7. **Commit via `/commit-push`** — NEVER commit directly with `git commit`. All changes must be committed before review.

Update the fix section: check off implementation tasks, note any discoveries. **Proceed directly to Phase 5 — do NOT pause, do NOT ask the user for confirmation.**

## Phase 5: Completion Checklist

**NO PAUSING** — Phase 5 is a direct continuation of Phase 4. After the Phase 4 commit, proceed immediately. Do NOT prompt the user, do NOT pause between items.

**FOREGROUND MANDATORY — ALL nested skill invocations** (`/tpr-review`, `/impl-hygiene-review`, `/improve-tooling`, `/sync-claude`, `/commit-push`). "Nested skill invocation" means: invoke via the **`Skill` tool**, NOT via `Agent`. An `Agent` dispatch (even foreground/non-background) runs the skill as a sub-agent, swallowing all intermediate output — round summaries, progress updates, and findings become invisible to the user until the Agent returns its final result. `Skill({skill: "tpr-review"})` runs the coordinator inline in the current context so per-round summaries are printed in real time.

1. **Verify all matrix items** — tests, builds, leak checks as specified in the fix section's completion checklist
2. **Invoke `/tpr-review`** via the `Skill` tool (Phase 5 — code review): `Skill({skill: "tpr-review"})` — independent third-party review of the **implementation**. NOT via Agent — see FOREGROUND MANDATORY note above.
3. **Handle code TPR findings** — fix any issues found, re-run until clean
4. **Run `/impl-hygiene-review`** — AFTER code TPR is clean
5. **Run `/improve-tooling` retrospectively** — MANDATORY at fix close, AFTER both reviews are clean. Reflect on: which `diagnostics/` scripts you ran, where you added ad-hoc `dbg!`/`tracing` calls (and what each one was looking for), where the original failure message was unhelpful, what instrumentation would have made the bug obvious in 1 minute instead of 30. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build`/`test`/`chore`/`ci`/`docs` — NOT `tools(...)`). See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full look-back protocol.
6. **Capability regression gate** — if Phase 4 step 6 identified a disabled capability, verify BEFORE closure: (a) re-enablement `- [ ]` item exists in the owning plan, (b) fix section §3 documents soundness argument + re-enablement path, (c) `#[ignore]`'d tests reference the re-enablement item.
7. **Update the bug entry** in the section file — mark `- [x]` with resolution details using the canonical format from `plans/bug-tracker/00-overview.md`
8. **Update the fix section** — set status to `complete`, fill in exit criteria
9. **Update the overview** — adjust open bug count in `plans/bug-tracker/00-overview.md`
10. **`/sync-claude` doc sync** — MANDATORY. Run `/sync-claude` to verify all Claude artifacts are consistent with the code changes.
11. **Final commit gate** — run `/commit-push` to commit the closure artifacts (bug entry, fix section status, overview count, any doc sync updates).

## Phase 6: Report

Report the fix to the user:
```
Fixed: [BUG-{section}-{ordinal}][{severity}] {title}
  Fix section: plans/bug-tracker/fix-BUG-{section}-{ordinal}.md
  Reclassified: {yes — {old}→{new}: {reason} | no}
  Tests added: {count} ({test file paths})
  Files changed: {list}
  Consensus: {converged | converged after N rounds | deadlocked (autopilot — proceeded with Claude's approach)}
  Plan TPR: {passed | findings resolved | skipped — {reason}}
  Code TPR: {passed | findings resolved}
  Hygiene: {passed | findings resolved}
```

## Scaling Rules

### Simple bugs (1-2 files, obvious fix, non-elevated subsystem)
- The fix section is still MANDATORY — but sections 1-3 can be brief
- TDD matrix can be smaller if the bug is narrowly scoped — but semantic + negative pins are ALWAYS required
- **Plan TPR (Phase 2.5) may be SKIPPED** — only when: severity is medium/low AND subsystem is not complexity-elevated AND `/tp-help` converged in round 1. See Phase 2.5 gate criteria.
- **Code TPR (Phase 5) and hygiene review are MANDATORY for ALL severities**

### Complex bugs (3+ files, architectural, or complexity-elevated subsystem)
- Full investigation with reference compilers
- Multiple design approaches with tradeoffs documented
- **Severity reclassification at Phase 1.5** — investigation often reveals the bug is worse than initially rated
- **Plan TPR (Phase 2.5) is MANDATORY**
- Code TPR (Phase 5) checkpoints during implementation if it spans multiple logical steps
- Consider whether the fix belongs in a proper plan section instead of a bug fix

### Clusters (multiple related bugs)
- If 2+ bugs share a root cause, create a single fix section covering all of them
- Name it after the primary bug: `fix-BUG-04-033.md` with the others listed as "Also fixes: BUG-04-034, BUG-04-035"
- The TDD matrix covers ALL bugs in the cluster

## Discovering New Bugs During a Fix — MANDATORY

Fixing a bug often uncovers other bugs. **You MUST invoke `/add-bug` immediately** whenever you encounter a new bug during any phase of this workflow.

Do NOT gloss over these as "not my bug" or "separate issue" — file them via `/add-bug` so they enter the tracker. Then decide: if the new bug **blocks** this fix (interference), shelve this fix and `/fix-bug` the blocker first (per fix-interference rules). If it's independent, continue with the current fix.

## Integration Points

- **`/add-bug`** — invoke during ANY phase when a new bug is discovered
- **`/fix-next-bug`** — orchestrates picking bugs from the tracker and invoking this skill in priority order
- **`/review-bugs`** — triages bugs; recommends `/fix-bug` for selected bugs
- **`/create-plan`** — MANDATORY in interactive mode when Phase 1.5 determines the bug needs a plan
- **`/tp-help`** — called in Phase 1.75 for consultative design consensus
- **`/tpr-review`** — called in Phase 2.5 (adversarial plan review) and Phase 5 (adversarial code review)
- **`/commit-push`** — used in Phase 4 to commit changes before review
- **`/impl-hygiene-review`** — called during Phase 5 completion checklist, AFTER code TPR
- **`/improve-tooling`** — called during Phase 5 completion checklist, AFTER both reviews are clean

## Files in this skill

- `SKILL.md` (this file) — Phase 0 dispatcher + full Opus workflow (Phases -1 through 6)
- `workflow.md` — Sonnet Phase 0 sub-agent: locates bug entry, extracts context, returns handoff
- `fix-section-template.md` — template for `plans/bug-tracker/fix-BUG-XX-NNN.md` files
