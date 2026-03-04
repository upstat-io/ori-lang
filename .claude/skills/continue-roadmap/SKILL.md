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

### Step 0: Check for Active Reroute

The scanner automatically detects reroutes from `plans/*/index.md` frontmatter. Each plan's `index.md` has:

```yaml
reroute: true       # or parallel: true
name: "Short Name"
full_name: "Full Plan Name"
status: active       # active | queued | resolved
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
2. If queued reroutes exist, pick the highest-priority one:
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
2. **Check formatting impact** — If syntax was added or changed:
   - Does the formatter handle the new syntax? Check `compiler/ori_fmt/`
   - Are formatting tests needed? Check/update `tests/spec/formatting/`
   - Run `./fmt-all.sh` to ensure formatter still works
3. **Update section file** — Check off completed items with `[x]`
4. **Update YAML frontmatter** — See "Updating Section File Frontmatter" below
5. **Commit with clear message** — Reference the section and task

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

### Why This Matters

The website dynamically loads roadmap data from these YAML frontmatter blocks. Incorrect status values cause the roadmap page to show wrong progress information.

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
