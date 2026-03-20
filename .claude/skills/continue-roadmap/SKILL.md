---
name: continue-roadmap
description: Resume work on the Ori compiler roadmap, picking up where we left off
argument-hint: "[section]"
---

# Continue Roadmap

Resume work on the Ori compiler roadmap, picking up where we left off.

## Usage

```
/continue-roadmap [section]
```

- No args: Auto-detect first incomplete item sequentially (00 → 01 → ...)
- `section-4`, `4`, or `modules`: Continue Section 4 (Modules)
- Any section number or keyword: Use `plans/roadmap/index.md` to find sections by keyword

## Finding Sections by Topic

Use `plans/roadmap/index.md` to find sections by keyword. The index contains searchable keyword clusters for each section.

---

## Workflow

### Step -1: Read CLAUDE.md (ABSOLUTE FIRST — NO EXCEPTIONS)

**Before doing ANYTHING else**, read the ENTIRE CLAUDE.md file — every single word, top to bottom:

```
Read file: CLAUDE.md
```

This is mandatory. Do not skip, skim, or partially read. The rules in CLAUDE.md govern ALL behavior in this command. Proceed to Step 0 only after reading the complete file.

### Step 0: Check for Active Reroute

The scanner automatically detects reroutes from `plans/*/index.md` frontmatter. Each plan's `index.md` has:

```yaml
reroute: true       # or parallel: true
name: "Short Name"
full_name: "Full Plan Name"
status: active       # active | queued | resolved
order: 1             # queue priority (lower = promoted first, default 999)
```

The scanner outputs an `=== REROUTES ===` block at the top with `[ACTIVE reroute]` and `[queued reroute]` lines.

**If an ACTIVE reroute exists:**

1. **Read the rerouted plan** — its `index.md` and `00-overview.md`
2. **Run the scanner on the rerouted plan**:
   ```bash
   .claude/skills/continue-roadmap/roadmap-scan.sh plans/<rerouted-plan>
   ```
3. **Follow the rerouted plan's execution order** — use the plan's recommended section order, not the roadmap's
4. **Present the rerouted plan status** to the user, making clear this is a reroute from the main roadmap
5. **When the rerouted plan is complete** — update its frontmatter `status: resolved`, then promote queued reroutes (see below)

**When an ACTIVE reroute completes (promotion protocol):**

1. Update the completed plan's frontmatter: `status: resolved`
2. If queued reroutes exist, pick the one with the lowest `order` value:
   - Update its frontmatter: `status: active`
   - Inform the user that the next reroute has been promoted to active
3. If no queued reroute exists, inform the user that normal roadmap work resumes

**Active parallel plans** (`parallel: true`) run alongside the roadmap — they don't block normal work. Only `reroute: true` plans with `status: active` take priority.

**Do NOT skip reroutes.** They exist because continuing normal roadmap work without completing the rerouted plan would compound architectural debt.

**Do NOT skip the queue.** Queued reroutes must complete before resuming normal roadmap work.

### Step 1: Run the Scanner

Run the roadmap scanner script to get current status:

```bash
.claude/skills/continue-roadmap/roadmap-scan.sh plans/roadmap
```

This outputs:
- Reroute status block (if any active/queued reroutes detected from `plans/*/index.md` frontmatter)
- One line per section: `[done]` or `[open]` with progress stats
- Detail block for the **first incomplete section**: subsection statuses (with blocked counts), first 5 **unblocked** items, blocker summary, and blocker chain

### Step 1.5: Fix Stale Frontmatter

The scanner detects frontmatter/body mismatches (`!! MISMATCH` annotations) at both section and subsection level. **When mismatches are found, fix them immediately** — do not proceed to the focus section with stale data.

**Auto-fix rules (no user prompt needed):**

1. **`frontmatter=complete` but unchecked items exist** — Set frontmatter to `in-progress` (or `not-started` if 0 checked)
2. **`frontmatter=not-started` but checked items exist** — Set frontmatter to `in-progress` (or `complete` if 0 unchecked)
3. **`frontmatter=in-progress` but all items checked** — Set frontmatter to `complete`
4. **`frontmatter=in-progress` but 0 items checked** — Set frontmatter to `not-started`
5. **Subsection status stale** — Apply the same rules per subsection, then recalculate section status
6. **Section status stale after subsection fix** — If all subsections are `complete`, set section to `complete`
7. **TPR consistency** — If `third_party_review.status: findings` but no unchecked TPR items exist, set to `resolved`. If unchecked TPR items exist but `third_party_review.status` is `none` or `resolved`, set to `findings`. If section `status` is `complete` but `third_party_review.status: findings`, set section to `in-progress`.

**When to ask instead of auto-fix:**

- If a section shows `complete` but has many unchecked items (>5), use AskUserQuestion — the checkboxes may be stale rather than the frontmatter
- If items are marked `[ ]` but have a `<!-- blocked:` or `<!-- deferred:` comment indicating they were intentionally left open, call them out and ask whether to mark complete or leave as-is

After fixing, briefly note what was corrected (e.g., "Fixed stale frontmatter: Section 09 status updated to `complete` (all subsections done)").

### Step 1.7: Unreviewed Plan Gate

After the scanner identifies the focus section, **check its frontmatter for `reviewed: false`**. This flag means the section's assumptions have NOT been validated against the current codebase — earlier section implementations may have changed the landscape.

**If `reviewed: false` is present on the focus section:**

1. **STOP** — do not begin implementation
2. **Warn the user** via AskUserQuestion:
   - "Section N has `reviewed: false` — its assumptions haven't been validated against the current codebase (which may have changed during earlier section work). Implementing an unreviewed plan risks wasted work."
   - Options: **Run /review-plan now (Recommended)** | **Proceed anyway** | **Pick a different section**
3. **If user chooses to review**: Run `/review-plan` on the **specific section file** (e.g., `plans/roadmap/section-03.md`). This is a single-section review — the pre-implementation gate. After the review agents confirm accuracy, the section is flipped to `reviewed: true`.
4. **If user chooses to proceed**: Continue, but note the risk in the summary output. Leave `reviewed: false` — the user accepted the risk but the section is still unvalidated.

**If `reviewed: false` is NOT present** (field absent or `reviewed: true`), proceed normally.

### Step 1.9: Third Party Review Triage Gate

After identifying the focus section, **check its frontmatter for `third_party_review.status: findings`**. This means an external reviewer (e.g. Codex) has recorded unresolved findings in the section's `## {NN}.R Third Party Review Findings` block.

**If `third_party_review.status` is `findings`:**

1. **STOP** — do not begin new implementation work
2. **Read all unchecked items** in the `## {NN}.R Third Party Review Findings` block
3. **Triage findings in priority order** (high → medium → low):
   - For each finding, validate it against the codebase, spec, and current plan
   - **CRITICAL: You MUST NOT dismiss a TPR finding because it is "not related" to the current plan or work.** Per CLAUDE.md: there is no "unrelated", "pre-existing", or "out of scope." If a TPR finding identifies a real issue in the codebase, it must be accepted and addressed — regardless of whether it falls within the current plan's stated scope. The only valid reason to reject a finding is that it is factually incorrect (the issue does not actually exist).
   - **Accepted findings**: Add or update concrete implementation tasks in the relevant subsection(s). Mark the review item resolved with a note:
     ```markdown
     - [x] `[TPR-02-001][high]` `compiler/oric/src/foo.rs` — Description.
       Resolved: Validated and integrated into 02.2 and 02.5 on YYYY-MM-DD.
     ```
   - **Rejected findings**: Do not delete — mark resolved with rejection rationale. **A finding may ONLY be rejected if it is factually incorrect** (the described issue does not actually exist in the codebase). "Not related to current plan", "out of scope", "pre-existing", and "not our problem" are NOT valid rejection reasons — per CLAUDE.md, there is no "unrelated" or "out of scope":
     ```markdown
     - [x] `[TPR-02-002][medium]` `compiler/oric/src/qux.rs` — Description.
       Resolved: Rejected after validation on YYYY-MM-DD. [Rationale — must explain why the issue does not actually exist].
     ```
4. **After all findings are triaged**:
   - Update `third_party_review.status` to `resolved` (if history exists) or `none`
   - Update `third_party_review.updated` to today's date
   - If accepted findings created new `[ ]` items, section `status` stays `in-progress`
5. **Continue** to normal implementation (Step 2+) only after all open review findings are triaged

**If `third_party_review.status` is `none` or `resolved`**, proceed normally.

**Status rules enforced by this gate:**
- A section cannot be `complete` while unchecked TPR items exist
- `third_party_review.status: findings` forces section `status` to `in-progress`
- All findings must be triaged before any new implementation work begins in that section

### Step 2: Determine Focus Section

**If argument provided**, find the matching section file and skip to Step 3.

**If no argument provided**, use the scanner's `=== FOCUS ===` section — the first section with `[ ]` items, scanning sequentially from Section 00.

#### Dependency Skip Rule

Only skip a section if **all** of these are true:
1. The section has explicit dependencies listed in `plans/roadmap/00-overview.md` § Dependency Graph
2. One or more of those dependencies has `status: not-started` or `status: in-progress` (prerequisite isn't complete)
3. The incomplete work in the current section actually **requires** the blocker (not all items may be blocked)

If a section has some blocked items and some unblocked items, **work the unblocked items** rather than skipping.

#### Blocker References (2-Way)

When you discover a blocker, you **must** add a 2-way reference so both sides are linked:

1. **On the blocked item** — Add `<!-- blocked-by:X -->` where X is the blocker section number
2. **On the blocker item** — Add `<!-- unblocks:X.Y -->` where X.Y is the blocked subsection ID

**Tag format**: Machine-readable, no free text. Human-readable names come from frontmatter lookup.
- `<!-- blocked-by:18 -->` — blocked by Section 18
- `<!-- blocked-by:18 --><!-- blocked-by:3 -->` — blocked by multiple sections
- `<!-- unblocks:0.3.2 -->` — unblocks subsection 0.3.2

**Both references must be added at the same time.** A one-way reference is incomplete.

Example:
```markdown
## 5.3 Pattern Matching Exhaustiveness
- [ ] Implement exhaustiveness checker  <!-- blocked-by:1 -->

## In Section 1, subsection 1.2:
- [ ] ADT type representation  <!-- unblocks:5.3 -->
```

**Parent inheritance**: Nested `- [ ]` items (indented) inherit their parent's blocker. Only tag the top-level item.

This ensures:
- The scanner correctly counts blocked vs unblocked items
- When completing a blocker, you can `grep 'unblocks:'` to find what it unblocks
- When reviewing a blocked item, `grep 'blocked-by:'` shows what prerequisite is missing

### Step 2.5: Blocker Chain Resolution

When the scanner shows blocked items, analyze the blocker chain:

1. Read the **Blocker summary** and **Blocker chain** from scanner output
2. Classify each blocker:
   - **READY**: All its dependencies are `[complete]` — can start implementing now
   - **IN PROGRESS**: Section already being worked on — progress will eventually unblock
   - **WAITING**: Has incomplete dependencies — blocked itself, can't start yet
3. Build and present a blocker tree in the summary:
   ```
   Blocker Tree:
   ├─ Section 18: Const Generics [not-started] — READY (deps satisfied: 2 [complete])
   │  └─ blocks 17 items here
   ├─ Section 19: Existential Types [not-started] — WAITING on Section 3
   │  └─ blocks 6 items here
   ├─ Section 3: Traits [in-progress, 24%] — IN PROGRESS
   │  └─ blocks 2 items here, also blocks Section 19
   └─ Section 14: Testing [in-progress, 8%] — WAITING (deep chain: 13←12←11←10←9)
      └─ blocks 2 items here
   ```

### Step 3: Load Section Details

Read the focus section file at the line numbers reported by the scanner. Extract:

1. **Section title** from the `# Section N:` header
2. **Completion stats**: from scanner output
3. **First incomplete item**: The first `- [ ]` line and its context (subsection header, description)
4. **Recently completed items**: Last few `- [x]` items for context

### Step 4: Present Summary

Present to the user:

```
## Section N: [Name]

**Progress:** X/Y items complete (Z%)
**Actionable:** A unblocked, B blocked (by N sections)

### Recently Completed
- [last 2-3 completed items]

### Next Up (Unblocked)
**Subsection X.Y: [Subsection Name]**
- [ ] [First unblocked incomplete item]
  - [sub-items if any]

### Blockers
[Blocker tree from Step 2.5 — READY/IN PROGRESS/WAITING classification]

### Remaining in This Section
- [count of remaining unblocked items]
- [count of blocked items, with "blocked by N sections" note]
```

### Step 5: Ask What to Do

Use AskUserQuestion with options. The options depend on the blocker state:

**When there are unblocked items:**
1. **Start next task (Recommended)** — Begin implementing the first unblocked item
2. **Show task details** — See more context about the task (read spec, find related code)
3. **Pick different task** — Choose a specific unblocked task from this section
4. **Tackle a blocker** — Work on a READY blocker to unblock items (ranked by impact: most items unblocked first)
5. **Switch sections** — Work on a different section

**When ALL remaining items are blocked:**
1. **Tackle deepest ready blocker (Recommended)** — Work on the READY blocker that unblocks the most items
2. **Show blocker details** — See what the blocker requires and its dependency chain
3. **Switch sections** — Work on a different section

### Step 6: Execute Work

Based on user choice:
- **Start next task**: Begin implementing the first unblocked item, following the Implementation Guidelines below
- **Show task details**: Read relevant spec sections, explore codebase for implementation location
- **Pick different task**: List all unblocked incomplete items in the section, let user choose
- **Tackle a blocker**: Switch to the blocker section and begin implementing its first unchecked item. When the blocker is complete, return to update the blocked items.
- **Switch sections**: Ask which section to switch to

---

## Implementation Guidelines

### ZERO DEFERRAL — Implement, Don't Document For Later

**If you understand a task well enough to write an implementation plan, you implement it.** Writing a detailed description of how to do the work and moving it to another section/plan IS deferral. The following are ALL banned:

- Labeling an item "requires architectural change" and skipping it — architectural changes are the work, not a reason to avoid the work.
- Moving items to a different roadmap section "for later" — if the item is in the current section, do it now.
- Writing "deferred to roadmap X.Y" on an item — the item is HERE, in THIS section.
- Marking a section complete while unchecked items remain, regardless of how they're annotated.
- Describing an implementation approach in prose instead of implementing it — if you can write the approach, you can write the code.
- Labeling items "lower priority" or "bonus" as justification for skipping — every checkbox is equal.

**The ONLY valid reason to not implement an item is if you literally cannot** (missing information that requires user input, blocked on external dependency). In that case, use `AskUserQuestion` immediately — do not silently skip.

### Plan Boundary Integrity

**Fixes must not silently cross section boundaries.** When implementing a task in Section X:

1. **Before modifying code**: Check if the code being modified is referenced by another section's tasks (grep for the file/function name in other section plans)
2. **If cross-section modification is needed**: Update the other section's plan to reflect the change — add a note, update a checkbox, or add a new item
3. **After completing a task**: Verify that no changes you made require updates to other sections' plans

**Why:** Section 02/03 overlap happened because fixes in one section touched code paths critical to another section without updating that section's plan. This created invisible dependencies that compounded into cascading failures. Plan boundaries must match implementation boundaries.

### Scope Rule: ALL Checkboxes in the Section Are In Scope

**Every `- [ ]` checkbox within the current section is part of that section's work — no exceptions.** This includes:

- **LLVM Support** checkboxes (codegen verification)
- **LLVM Rust Tests** checkboxes (AOT end-to-end tests)
- **Ori Tests** checkboxes
- **Rust Tests** checkboxes
- Any other sub-item checkboxes nested under a parent item

**Do NOT defer items to other sections.** If subsection 1.1A has `[ ] LLVM Rust Tests: No AOT tests for Duration`, that checkbox is part of 1.1A — not Section 21A. Section 21A tracks LLVM *infrastructure* (codegen architecture, optimization passes). Individual feature sections track their own LLVM *coverage* (does this feature work in AOT?).

**A subsection is only complete when ALL its checkboxes are checked**, including LLVM items. Do not mark a subsection as complete or move to the next subsection while LLVM checkboxes remain unchecked.

### Verification Rule: Empty Checkboxes Must Be Verified

**Never check off a `[ ]` item without verifying it.** Before marking any item `[x]`:

1. **Read the relevant code** — confirm the feature/test actually exists
2. **Run the test** — if it's a test item, run it and confirm it passes
3. **Check the spec** — if it's an implementation item, verify behavior matches the spec

Checking off items without verification defeats the purpose of the roadmap.

### Skills Are Tools — Run Them, Don't Reimplement Them

**When a plan item says to run a skill (e.g., "Run `/code-journey`", "Run `.claude/skills/code-journey/extract-metrics.py`"), invoke it using the `Skill` tool.** Do NOT manually read the skill's SKILL.md and re-execute its steps yourself. The skill automates an entire pipeline — manually reimplementing it is less thorough, wastes context, and contradicts the plan's instruction.

This applies to ALL skills: `/code-journey`, `/review-plan`, `/sync-spec`, etc.

### Before Writing Code

1. **Read the spec** — Understand exactly what behavior is required
2. **Find existing tests** — Check `tests/spec/` for related test files
3. **Explore the codebase** — Use Explore agent to find where features should be implemented

### While Writing Code

1. **Follow existing patterns** — Match the style of surrounding code
2. **Add tests** — Create Ori spec tests in `tests/spec/category/`
3. **Add Rust tests** — Add unit tests for new Rust code
4. **Check off items** — Update section file checkboxes as you complete sub-items

### After Writing Code

1. **Run tests** — `./test-all.sh` to verify everything passes
2. **Check for interference** — if your fix introduces NEW failures that weren't failing before, this is INTERFERENCE from another bug, not a "pre-existing issue." The correct response: revert your fix, fix the interfering bug first (it's now a dependency), then re-apply your fix. Never declare a bug fixed when the test suite has more failures than before your fix. Never rationalize the new failures as "pre-existing" — the interference made them your problem.
3. **Verify matrix coverage** — if the fix is type-dependent or pattern-dependent, confirm that tests cover all relevant type x pattern combinations. Missing cells in the matrix are potential regressions. See `.claude/rules/tests.md` Matrix Testing Rule.
4. **Check plan boundary integrity** — did this fix modify code referenced by another section's tasks? If yes, update that section's plan to reflect the change. No silent cross-section absorption.
4. **Check formatting impact** — If syntax was added or changed:
   - Does the formatter handle the new syntax? Check `compiler/ori_fmt/`
   - Are formatting tests needed? Check/update `tests/spec/formatting/`
   - Run `./fmt-all.sh` to ensure formatter still works
5. **Update section file** — Check off completed items with `[x]`
6. **Update YAML frontmatter** — See "Updating Section File Frontmatter" below
7. **Commit with clear message** — Reference the section and task

---

## Gap Detection and Escalation Protocol

When implementing a roadmap item and you discover that a required language feature
is missing, incomplete, or blocks the current work:

### STOP — Do Not Work Around

**Never silently substitute a workaround.** If `.0` syntax doesn't work, don't
quietly switch to destructuring. If a pattern form panics, don't restructure the
test to avoid it. The workaround hides the gap from the user and from the roadmap.

### Flag Immediately

Use AskUserQuestion to escalate:

1. **What's missing**: Describe the exact gap (e.g., "parser rejects `.0` after dot — tuple field access not implemented")
2. **Where it's documented** (or not): Check spec, EBNF, roadmap for the feature
3. **Impact**: What current work is blocked or degraded
4. **Recommendation**: Fix now (if small, < 30 min), track and fix later (if large), or ask user

### Track in Roadmap

If the gap is deferred (not fixed immediately):
1. Add a `<!-- gap: description -->` comment on the blocked roadmap item
2. Add a `- [ ]` checkbox for the missing feature in the appropriate section
3. Add blocker references (`<!-- blocked-by:X -->` / `<!-- unblocks:X.Y -->`)

### Why This Matters

Silent workarounds create invisible technical debt. A gap that isn't flagged:
- Won't appear in the roadmap scanner output
- Won't be prioritized for implementation
- Will surprise users when they try the "supported" syntax
- Forces every future implementer to discover and work around it independently

---

## Updating Section File Frontmatter

Section files use YAML frontmatter for machine-readable status tracking. **You must keep this in sync** when completing tasks.

### Frontmatter Structure

```yaml
---
section: "1"
title: Type System Foundation
status: in-progress          # Section-level status
tier: 1
goal: Fix type checking...
sections:
  - id: "1.1"
    title: Primitive Types
    status: complete         # Subsection-level status
  - id: "1.1B"
    title: Never Type Semantics
    status: in-progress
---
```

### Status Values

- `not-started` — No checkboxes completed in subsection/section
- `in-progress` — Some checkboxes completed, some pending
- `complete` — All checkboxes completed

### When to Update

**After completing task checkboxes**, update the frontmatter:

1. **Update subsection status** based on checkboxes under that `## X.Y` header:
   - All `[x]` → `status: complete`
   - Mix of `[x]` and `[ ]` → `status: in-progress`
   - All `[ ]` → `status: not-started`

2. **Update section status** based on subsection statuses:
   - All subsections complete → `status: complete`
   - Any subsection in-progress → `status: in-progress`
   - All subsections not-started → `status: not-started`

3. **Update `third_party_review` frontmatter** if the TPR block was modified:
   - All TPR items resolved (checked) → `third_party_review.status: resolved`
   - Unchecked TPR items remain → `third_party_review.status: findings`
   - No TPR items (`- None.`) → `third_party_review.status: none`
   - A section cannot be `complete` while `third_party_review.status: findings`

### Why This Matters

The website dynamically loads roadmap data from these YAML frontmatter blocks. Incorrect status values cause the roadmap page to show wrong progress information.

**Catch-all:** If frontmatter drifts despite these rules, Step 1.5 (Stale Frontmatter Auto-Fix) catches and corrects it at the start of every `/continue-roadmap` invocation.

---

## Verification/Audit Workflow

When auditing roadmap accuracy (verifying status rather than implementing features), follow this workflow:

### Step 1: Compare Frontmatter to Body

Before testing anything, check if frontmatter matches checkbox state:

1. Read the YAML frontmatter subsection statuses
2. Scan the body for `[x]` and `[ ]` checkboxes under each `## X.Y` header
3. **If they don't match** — the roadmap is stale and needs updating

### Step 2: Test Claimed Status

Don't trust checkboxes blindly. Verify actual implementation:

1. **For `[x]` items**: Write quick test to confirm feature works
2. **For `[ ]` items**: Write quick test to confirm feature fails/is missing
3. **Document discrepancies**: Note items where claimed status doesn't match reality

### Step 3: Update Body Checkboxes

Fix checkboxes to match verified reality:

- Feature works → `[x]`
- Feature broken/missing → `[ ]`
- Add date stamps for verification: `(2026-02-04)`

### Step 4: Update Frontmatter Immediately

**Never leave frontmatter stale.** After updating body checkboxes:

1. Recalculate each subsection status from its checkboxes
2. Update subsection `status` values in frontmatter
3. Recalculate section status from subsection statuses
4. Update section `status` value in frontmatter

---

## Checklist

When completing a roadmap item:

- [ ] Read spec section thoroughly
- [ ] Implement feature in compiler
- [ ] Add Ori spec tests
- [ ] Add Rust unit tests (if applicable)
- [ ] Run `./test-all.sh` — all tests pass
- [ ] Check if formatting needs updates (if syntax changed):
  - [ ] Formatter handles new syntax (`compiler/ori_fmt/`)
  - [ ] Formatting tests cover new syntax (`tests/spec/formatting/`)
- [ ] Update section file:
  - [ ] Check off completed items with `[x]`
  - [ ] Update subsection `status` in YAML frontmatter if subsection is now complete
  - [ ] Update section `status` in YAML frontmatter if all subsections are now complete
- [ ] Commit with section reference in message

---

## Maintaining the Roadmap Index

**IMPORTANT:** When adding new items to the roadmap, update `plans/roadmap/index.md`:

1. **Adding items to existing section**: Add relevant keywords to that section's keyword cluster
2. **Creating a new section**: Add a new keyword cluster block and table entry
3. **Removing/renaming sections**: Update the corresponding entries

The index enables quick topic-based navigation. Keep keyword clusters concise (3-8 lines) and include both formal names and common aliases developers might search for.
