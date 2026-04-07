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

### ABSOLUTE RULE: Commits via /commit-push ONLY

**NEVER run `git add`, `git commit`, or any direct git commit command.** All commits MUST go through the `/commit-push` skill. This applies everywhere in this workflow: clean-tree gates, subsection pauses, after-work commits, final commits. Invoke `/commit-push` via the Skill tool.

### Step -1: Read CLAUDE.md (ABSOLUTE FIRST — NO EXCEPTIONS)

**Before doing ANYTHING else**, use the Read tool to read the ENTIRE CLAUDE.md file — every single line, top to bottom:

```
Read file: CLAUDE.md
```

**This is a BLOCKING requirement.** You MUST issue a Read tool call for CLAUDE.md and process every line of the result. Do not skip, skim, summarize, or partially read. Do not assume you already know the contents from earlier in the conversation — the file may have changed. Do not rely on CLAUDE.md content loaded into system context — issue the Read tool call explicitly. The rules in CLAUDE.md govern ALL behavior in this command. Proceed to Step 0 only after reading the complete file via the Read tool.

### Step -1B: Re-read CLAUDE.md Between Tasks (MANDATORY)

**Every time you finish a task and start the next one** (e.g., completing one checklist item and moving to the next, finishing TPR triage and starting implementation, switching between subsections), you MUST re-read CLAUDE.md in full via the Read tool before beginning the new task:

```
Read file: CLAUDE.md
```

This is not optional. Context window compression can silently drop CLAUDE.md rules that were loaded earlier. A fresh read ensures every rule is active in your working context. This applies within a single `/continue-roadmap` session — not just at the start.

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

1. Update the completed plan's frontmatter: `status: resolved` (or `complete`)
2. **Verify frontmatter consistency** (see "Plan Completion Frontmatter Gate" below)
3. **Move the plan to `plans/completed/`**: `git mv plans/<plan-dir> plans/completed/<plan-dir>`
4. If queued reroutes exist, pick the one with the lowest `order` value:
   - Update its frontmatter: `status: active`
   - Inform the user that the next reroute has been promoted to active
5. If no queued reroute exists, inform the user that normal roadmap work resumes

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

### Step 1.55: Stale Plan Annotation Check

Run the plan annotation scanner to detect stale annotations from already-completed plans:

```bash
bash .claude/skills/impl-hygiene-review/plan-annotations.sh --count
```

**If annotations exist for completed sections** (sections with `status: complete`):

1. **These are stale** — the plan work is done but the scaffolding was never cleaned up
2. **Report the count** to the user: "Found N stale plan annotations from completed sections"
3. **Clean them up** before starting new work — remove all plan-specific annotations (TPR, CROSS, BUG, §, Phase, section- refs) from `.rs` files. Spec references (`Spec: Clause N.M`) are permanent and must NOT be removed.
4. **Commit the cleanup** via `/commit-push`

**If annotations exist only for in-progress or not-started sections**, they are legitimate scaffolding — no action needed.

This check catches annotations that slipped through prior section completions. It runs once at startup, not on every subsection boundary.

### Step 1.6: Schema Compliance Check

The plan schema lives at `.claude/skills/create-plan/plan-schema.md`. When working on a plan (reroute or roadmap section), verify the focus section's frontmatter conforms to the schema:

**Required frontmatter fields for section files:**
- `section` — section number (string or number)
- `title` — section title
- `status` — `not-started | in-progress | complete`
- `reviewed` — `true | false`
- `goal` — one-line measurable goal
- `sections` — array of `{ id, title, status }` subsection entries
- `third_party_review` — `{ status: none | findings | resolved, updated: date | null }`

**Required frontmatter fields for overview files (`00-overview.md`):**
- `plan` — plan directory name
- `title` — full plan title
- `status` — `not-started | in-progress | complete`

**Required frontmatter fields for index files (`index.md`):**
- `reroute: true` or `parallel: true` (for website-visible plans)
- `name` — short display name
- `full_name` — full display name
- `status` — `active | queued | resolved`

**Auto-fix:** If a field is missing or uses a non-standard value (e.g., `status: done` instead of `status: complete`), fix it silently. If the structure is fundamentally wrong (e.g., missing `sections` array entirely), note it and fix.

This check is lightweight — only verify the focus section and its parent overview/index. Do not scan all sections on every invocation.

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
   - **BANNED: Resolving findings with scope notes or rationalizations.** Marking a finding as `[x] Resolved: Scoped as known limitation...` or `Resolved: Pre-existing architectural issue...` is DEFERRAL, not resolution. The finding must be FIXED (code change + tests) or have a concrete plan created and executed. If the fix requires cross-crate refactoring, that IS the work. If genuinely blocked, use `AskUserQuestion`.
   - **Rejected findings**: Do not delete — mark resolved with rejection rationale. **A finding may ONLY be rejected if it is factually incorrect** (the described issue does not actually exist in the codebase). "Not related to current plan", "out of scope", "pre-existing", "conservative/safe", "architectural limitation", and "not our problem" are NOT valid rejection reasons — per CLAUDE.md, there is no "unrelated" or "out of scope":
     ```markdown
     - [x] `[TPR-02-002][medium]` `compiler/oric/src/qux.rs` — Description.
       Resolved: Rejected after validation on YYYY-MM-DD. [Rationale — must explain why the issue does not actually exist].
     ```
4. **After all findings are triaged**:
   - Update `third_party_review.updated` to today's date
   - If ALL findings were rejected (no accepted findings created new `[ ]` items):
     - Update `third_party_review.status` to `resolved`
   - If ANY accepted findings created new `[ ]` implementation items:
     - **Keep** `third_party_review.status: findings` — do NOT set to `resolved`
     - Status transitions to `resolved` only when the accepted implementation tasks are complete and revalidated
   - Section `status` stays `in-progress` while `third_party_review.status: findings`
5. **Continue** to normal implementation (Step 2+) only after all open review findings are triaged

**If `third_party_review.status` is `none` or `resolved`**, proceed normally.

**Status rules enforced by this gate:**
- A section cannot be `complete` while unchecked TPR items exist
- `third_party_review.status: findings` forces section `status` to `in-progress`
- All findings must be triaged before any new implementation work begins in that section

### Step 1.92: Bug Tracker Check

After identifying the focus section, **check the bug tracker for relevant known bugs** in the subsystem being worked on.

Map the focus section to bug-tracker subsystems:

| Roadmap Section | Bug Tracker Section(s) |
|----------------|----------------------|
| 00 (Parser) | 01 (Parser & Lexer) |
| 01-02 (Types, Inference) | 02 (Type Checker) |
| 03 (Traits) | 02 (Type Checker), 06 (Stdlib) |
| 04 (Modules) | 02 (Type Checker), 07 (Tooling) |
| 05 (Type Decls) | 02 (Type Checker) |
| 06 (Capabilities) | 02 (Type Checker), 03 (Evaluator) |
| 07A-D (Stdlib) | 03 (Evaluator), 06 (Stdlib) |
| 08-10 (Patterns, Match, Control Flow) | 03 (Evaluator), 04 (Codegen) |
| 11-12 (FFI, Variadics) | 04 (Codegen), 05 (Runtime) |
| 15D (Bindings) | 02 (Type Checker), 03 (Evaluator) |
| 21A-B (LLVM, AOT) | 04 (Codegen), 05 (Runtime) |
| 22 (Tooling) | 07 (Tooling) |
| 23 (Evaluator) | 03 (Evaluator) |

Read the mapped bug-tracker section file(s) and check for `- [ ]` items.

**If `critical` bugs exist in the mapped subsystem(s):**

1. **STOP** — present them to the user as blockers
2. List each critical bug with its ID, title, and repro
3. Use AskUserQuestion:
   - **Fix critical bugs first (Recommended)** — use `/fix-bug BUG-XX-NNN` for each critical bug. This creates a fix section file with full plan-section rigor (root cause analysis, TDD matrix, TPR, hygiene review). Do NOT fix bugs ad-hoc — the `/fix-bug` workflow is mandatory.
   - **Proceed anyway** — user accepts the risk of working around known critical bugs

**If `high` bugs exist:**

1. **Mention them** — "There are N high-severity bugs in this area you may want to address"
2. List the bug IDs and titles briefly
3. If user wants to fix them: use `/fix-bug BUG-XX-NNN` for each
4. Continue to the next step — high bugs are informational, not blocking

**If only `medium`/`low` or no bugs exist**, proceed normally.

### Step 1.95: Clean Working Tree Gate

Before starting implementation work, **check for pending changes** in the working tree:

```bash
git status --short
```

**If the working tree is clean** (no output), proceed to Step 2.

**If there are pending changes** (staged, unstaged, or untracked files):

1. **STOP** — do not proceed to implementation work
2. **Show a brief summary** of what's pending:
   - Number of modified files, staged files, untracked files
   - List the filenames (truncate to first 10 if many)
3. **Use AskUserQuestion** with these options:
   - **Run /commit-push (Recommended)** — commit and push all pending changes before continuing
   - **Proceed anyway** — continue with a dirty working tree (user accepts the risk of mixing work)

**Why:** This gate runs after TPR triage (Step 1.9) so that serious bugs surfaced by third-party review are fixed before the commit prompt. Committing before TPR triage would lock in code that may need immediate changes. After TPR fixes are applied, a clean working tree ensures the next section's work is cleanly separable in git history.

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

**Planned work requirement**: A `<!-- blocked-by:X -->` tag is ONLY valid if Section X contains a concrete `- [ ]` item whose completion will resolve the blocker. Adding a `blocked-by` reference to a section that has no planned resolution work is creating an unplanned blocker — which is not allowed (see Step 2.6). If no such item exists in Section X, you must add one before tagging.

**No prose-only blockers**: `<!-- blocked: some description -->` without a section reference is a temporary annotation only. Step 2.6 will convert these to either (a) planned subsections in the current plan, or (b) `blocked-by:X` references pointing to concrete plan items. Prose-only blockers cannot persist across `/continue-roadmap` invocations.

This ensures:
- The scanner correctly counts blocked vs unblocked items
- When completing a blocker, you can `grep 'unblocks:'` to find what it unblocks
- When reviewing a blocked item, `grep 'blocked-by:'` shows what prerequisite is missing
- **Every blocker has a resolution path** — no open-ended blockers accumulate silently

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

### Step 2.6: Impediment Resolution — No Unplanned Blockers

**ABSOLUTE RULE: Every blocker must point to planned, actionable work.** A blocker that references a section or plan must have concrete `- [ ]` items in that section that will resolve the blocker. A blocker that describes a missing capability without referencing any plan is an **unplanned blocker** — and unplanned blockers are not allowed to remain open-ended.

**After classifying blockers in Step 2.5, validate every blocker:**

1. **For each `<!-- blocked-by:X -->` reference**: Read Section X and verify it contains a `- [ ]` item whose completion will resolve this blocker. If Section X has no such item, the blocker is unplanned — treat it as an impediment (see below).

2. **For each prose `<!-- blocked: ... -->` comment** (no section reference): This is always an unplanned blocker. It describes a missing capability that nobody has planned to fix. It MUST be resolved — either by adding planned work or by determining it's actually fixable now.

**Blocker categories after validation:**

| Category | Example | Action |
|----------|---------|--------|
| **Planned cross-section blocker** | `<!-- blocked-by:19 -->` and Section 19 has `- [ ] Implement existential types` | Valid blocker — skip or tackle Section 19 |
| **Unplanned cross-section blocker** | `<!-- blocked-by:19 -->` but Section 19 has no item that resolves this | Invalid — add the missing item to Section 19, or reclassify as impediment |
| **Unplanned impediment (prose)** | `<!-- blocked: ARC IR lacks visibility metadata -->` | Must be planned NOW — invoke `/create-plan` to add a subsection |
| **Fixable impediment** | Prose blocker where upstream data already exists | Plan it and implement it immediately |

**When unplanned blockers or impediments are detected:**

1. **Investigate each one** — use an Explore agent to verify whether the missing capability is truly unavailable or just unplumbed. Check:
   - Does the data exist upstream? (e.g., does `ori_types` already have this info?)
   - What's the plumbing path? (How many files need changes?)
   - Is this a 50-line fix or a 500-line architectural change?

2. **Plan the resolution** — every unplanned blocker must get planned work somewhere:
   - **If the fix belongs in the current plan** (most common for impediments): Invoke `/create-plan` to add a new subsection:
     ```
     /create-plan add "ARC IR function metadata" subsection to plans/repr-opt
     ```
     The new subsection should: describe the impediment, list the implementation steps, include tests, and reference which blocked items it unblocks.
   - **If the fix belongs in a different plan or roadmap section**: Add a concrete `- [ ]` item to that section describing the work needed, and update the blocker to use `<!-- blocked-by:X -->` pointing to the section. The blocker is now planned.
   - **If the fix requires a new plan entirely** (large scope, new subsystem): Invoke `/create-plan` to create the new plan, then update the blocker to reference it.

3. **After planning, decide whether to implement now or later:**
   - **Implement now (recommended for impediments)** — if the fix is localized (< 200 lines, < 5 files), implement it immediately after planning. The impediment IS the next task.
   - **Implement later (for large cross-section work)** — if the fix is a full section of work, it may be better to tackle it as a separate pass. But it MUST be planned — no open-ended blockers.

4. **After implementation** (or after planning, if deferred):
   - Remove prose `<!-- blocked: ... -->` comments from items that are now unblocked
   - Update `<!-- blocked-by:X -->` references if the blocker section changed
   - Check off resolved items

**Why this matters:** A prose `<!-- blocked: ... -->` comment without a plan is invisible deferral. It looks responsible ("I documented the dependency!") but creates permanent blockers that nobody resolves because they aren't tracked as actionable work anywhere. By requiring every blocker to point to planned work, blockers become visible, trackable, and eventually resolvable. The system cannot accumulate open-ended blockers that silently prevent sections from completing.

**This step is MANDATORY whenever blocked items exist.** Before presenting the blocker tree to the user, you MUST validate that every blocker points to planned work. Unplanned blockers must be resolved in this step — either by adding plan items or by determining they're actually fixable impediments that can be implemented now.

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
1. **Resolve impediments (Recommended if any exist)** — If Step 2.6 identified fixable impediments, plan and implement them to unblock items in the current section
2. **Tackle deepest ready blocker** — Work on the READY blocker that unblocks the most items (for true cross-section blockers)
3. **Show blocker details** — See what the blocker requires and its dependency chain
4. **Switch sections** — Work on a different section

### Step 5.5: Subsection Pacing

**After the user chooses to start work**, ask how they want to pace the section using AskUserQuestion:

1. **Full section** — Run all subsections continuously without pausing
2. **Subsection-by-subsection (Recommended)** — Pause after completing each subsection for review before continuing to the next

**Why:** This gives the user control over execution granularity. Large sections can produce significant changes — pausing between subsections allows review, course-correction, and incremental commits.

**Subsection close-out is MANDATORY regardless of pacing choice.** Whether the user picked "Full section" or "Subsection-by-subsection", every subsection close-out MUST include the per-subsection `/improve-tooling` retrospective BEFORE moving to the next subsection. The pacing choice only controls whether you pause for user review — it does NOT skip the retrospective. Pacing determines who reviews; the retrospective determines whether tooling grows.

**Subsection close-out sequence** (run for every subsection, both pacing modes):

1. All subsection tasks are `[x]` and the subsection's behavior is verified.
2. Update the subsection's `status` in section frontmatter to `complete`.
3. **Run `/improve-tooling` retrospectively on THIS subsection** — invoke the skill in per-subsection mode (see `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"). Reflect on this subsection's debugging journey: which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement every accepted improvement NOW (zero deferral). Commit each via SEPARATE `/commit-push` using a valid conventional-commit type (e.g., `build(diagnostics): add X to Y.sh — surfaced by {plan}/section-{NN}.M retrospective` — use `build` for dev/diagnostic scripts, `test` for test-harness, `chore` for general tooling, `ci` for CI, `docs` for tool docs; do NOT use `tools(...)` — the lefthook commit-msg hook rejects any type outside the standard set `feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert`). Verify each improvement against the original friction. If genuinely no gaps, document briefly: "Retrospective {NN}.M: no tooling gaps". Mandatory even when nothing felt painful — that is exactly when blind spots accumulate. **Do NOT batch this to section close** — pain memory has already decayed by then.
4. `/commit-push` for the subsection's implementation work (separate from any tooling commits in step 3).

**Then** apply the pacing choice:
- If user chose "Full section", proceed directly to the next subsection (the close-out above already ran).
- If user chose "Subsection-by-subsection", present a brief status update including the retrospective outcome ("Retrospective {NN}.M: 2 improvements committed: {commits}" OR "Retrospective {NN}.M: no gaps") and use AskUserQuestion with:
  1. **Continue to next subsection** — Proceed to the next incomplete subsection
  2. **Run /commit-push and continue** — Commit any remaining changes, then proceed
  3. **Stop here** — End work for now (run `/commit-push` first if there are changes)

### Step 6: Execute Work

Based on user choice:
- **Start next task**: Begin implementing the first unblocked item, following the Implementation Guidelines below
- **Show task details**: Read relevant spec sections, explore codebase for implementation location
- **Pick different task**: List all unblocked incomplete items in the section, let user choose
- **Resolve impediments**: Invoke `/create-plan` to add a subsection to the current plan that resolves the impediment (see Step 2.6). After the subsection is created and reviewed, implement it immediately. Then return to the previously-blocked items — they should now be unblocked.
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

### ALL Deferrals Must Have Implementation Anchors

**When an item IS deferred (with valid reason), it MUST point to a concrete `- [ ]` item where the work WILL be done.** Valid deferral reasons and their anchor requirements:

1. **Dependency** — blocked by work in another section. The deferred item MUST have `<!-- blocked-by:X -->` pointing to a specific `- [ ]` item in section X that, when completed, unblocks this item.
2. **Better location** — the work fits more naturally in a future section. The deferred item MUST reference a specific `- [ ]` item in that section. If no such item exists, CREATE it before deferring.

**Anchor-free deferrals are banned.** A deferred item with no pointer to where it will actually get done is invisible — it will never be implemented. The following rationalizations for skipping WITHOUT an anchor are ALL banned:

- "Nice-to-have" / "not blocking" — if the user approved it in the plan, it is mandatory
- "Existing tests already cover it" — approved test items are not redundant by definition
- "Test gap but not a blocker" — an unchecked checkbox IS a blocker for section completion
- "Deferred to separate cleanup pass" — what cleanup pass? Where is the `- [ ]` item?
- Scope, effort, complexity, difficulty, number of call sites — irrelevant per CLAUDE.md

**Before marking any item as deferred, verify:** Does a concrete `- [ ]` item exist in the target section that will produce the deferred work? If not, either (a) create the item in the target section, or (b) do the work now.

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
2. **Check for interference** — if your fix introduces NEW failures that weren't failing before, this is INTERFERENCE from another bug, not a "pre-existing issue." The correct response: revert your fix, fix the interfering bug first using `/fix-bug` (it's now a dependency — the interfering bug gets full plan-section rigor: root cause analysis, TDD matrix, TPR, hygiene review), then re-apply your fix. Never declare a bug fixed when the test suite has more failures than before your fix. Never rationalize the new failures as "pre-existing" — the interference made them your problem.
3. **Verify matrix coverage** — if the fix is type-dependent or pattern-dependent, confirm that tests cover all relevant type x pattern combinations. Missing cells in the matrix are potential regressions. See `.claude/rules/tests.md` Matrix Testing Rule.
4. **Check plan boundary integrity** — did this fix modify code referenced by another section's tasks? If yes, update that section's plan to reflect the change. No silent cross-section absorption.
5. **Check formatting impact** — If syntax was added or changed:
   - Does the formatter handle the new syntax? Check `compiler/ori_fmt/`
   - Are formatting tests needed? Check/update `tests/spec/formatting/`
   - Run `./fmt-all.sh` to ensure formatter still works
6. **Update section file** — Check off completed items with `[x]`
7. **Update YAML frontmatter** — See "Updating Section File Frontmatter" below
8. **Clean up plan annotations** — Run `.claude/skills/impl-hygiene-review/plan-annotations.sh --plan NN` (where NN is the section number) to find annotations in source code referencing the completed section. Remove all stale annotations (TPR-NN-XXX, CROSS-NN-XXX, BUG-NN-XX, §NN.X, Phase refs, etc.) from `.rs` files. Spec references (`Spec: Clause N.M`) are permanent and must NOT be removed. This is mandatory before marking a section complete.
9. **Run `/commit-push`** — NEVER commit directly with `git commit`. Always use the `/commit-push` skill.
10. **Run `/tpr-review` after section completion — MUST PASS CLEAN** — When ALL checkboxes in a section are checked and the section is about to be marked `complete`, run `/tpr-review` for an independent Codex review. **The TPR must come back completely clean before the section can be closed out.** If `/tpr-review` surfaces ANY findings: (1) triage them through Step 1.9 (TPR Triage Gate), (2) fix all accepted findings, (3) **re-run `/tpr-review`** to confirm clean. Repeat this cycle until the review passes with zero unresolved findings. A section CANNOT be marked `complete` until a clean `/tpr-review` pass is achieved — "all findings triaged" is not sufficient, the re-run must confirm they are actually resolved. **This rule is definitive and non-negotiable. Do not reason about whether a TPR pass is "close enough", whether remaining findings are "minor", or whether the section is "effectively complete". There is no judgement call — either the TPR is clean or the section stays open. No exceptions, no rationalizations, no shortcuts.**
11. **Run `/impl-hygiene-review` after TPR is clean — MUST PASS** — After `/tpr-review` passes clean, run `/impl-hygiene-review` (or scope to the section's commits) for a deep hygiene sweep: phase boundaries, SSOT, algorithmic DRY, naming, file organization. If `/impl-hygiene-review` surfaces critical or major findings: fix them, re-run `/tpr-review` (since code changed), then re-run `/impl-hygiene-review`. **A section CANNOT be marked `complete` until both `/tpr-review` AND `/impl-hygiene-review` pass clean.** The hygiene review is the final quality gate — it catches plumbing-layer issues (leaked dispatch, scattered knowledge, algorithmic duplication) that TPR is not designed to detect. Same non-negotiable rule as TPR: clean or open, no middle ground.
12. **Run `/improve-tooling` section-close sweep after both reviews are clean — MANDATORY safety net at every section close** — Once `/tpr-review` and `/impl-hygiene-review` both pass clean, invoke `/improve-tooling` in **section-close sweep mode** (see the skill's "Retrospective Mode" → "Section-Close Sweep Workflow"). The sweep is NOT the primary tooling capture — that already happened per-subsection in Step 5.5 — the sweep is a safety net that does TWO things: (1) **Verify** every subsection in this section has either an "improvements made" entry (with commits) or a documented "no gaps" negative finding from its own per-subsection retrospective. If any subsection skipped its retrospective, **STOP** — go back and run it now. The sweep cannot substitute for missed per-subsection captures. (2) **Look for cross-subsection patterns** invisible at per-item scope: command sequences repeated when transitioning between *different* subsections, integration test failures with worse messages than within-subsection failures, mental cross-referencing across files no tool combined, instrumentation that only became obvious after seeing all subsections together. Add ONLY new items that emerged from these cross-cutting patterns — do not duplicate per-subsection findings. Implement every accepted item NOW (zero deferral). Commit via `/commit-push` as **separate commits** using a valid conventional-commit type: `build(diagnostics): add X — surfaced by section-NN close sweep` (or `test(...)`, `chore(...)`, `ci(...)`, `docs(...)` as appropriate; do NOT use `tools(...)` — the lefthook commit-msg hook rejects it). Verify each improvement against the original scenario. **Most sweeps produce zero new findings when per-subsection captures are thorough — that is the expected, healthy outcome** and must be documented: "Section-close sweep: per-subsection retrospectives covered everything; no cross-subsection patterns required new tooling." If your sweep produces 8 findings while per-subsection retrospectives produced 0, the per-subsection captures were skipped — diagnose the process failure, not the symptoms. Do not silently skip.

---

## Bugs Discovered During Plan Execution

When implementing a roadmap section, you WILL discover bugs — test failures, wrong output, crashes, spec/impl mismatches, edge cases. These bugs need structured handling, not ad-hoc fixes.

### Decision Tree

```
Bug discovered during plan execution
  │
  ├─ 1. Is it CRITICAL severity?
  │     └── YES → /fix-bug immediately (even if not directly blocking)
  │               Critical bugs compound — they will interfere with later work
  │
  ├─ 2. Does it BLOCK the current task?
  │     └── YES → /fix-bug NOW
  │               Creates fix section, TDD matrix, completion checklist
  │               Resume plan work after /fix-bug completes
  │
  ├─ 3. Is it HIGH severity?
  │     └── /add-bug now (tracked), /fix-bug when entering adjacent code
  │
  └─ 4. MEDIUM or LOW severity?
        └── /add-bug — file and continue
```

**Evaluation order matters:** check critical first (always fix), then blocking (always fix), then high (file now, fix soon), then medium/low (file and continue).

### Key Rules

1. **Every bug gets at least `/add-bug`** — no bug discovered during plan work goes untracked. "I'll remember it" is not tracking.

2. **Blocking bugs get `/fix-bug`** — if a bug prevents the current plan task from completing, invoke `/fix-bug BUG-XX-NNN`. This pauses plan work, creates a fix section with full rigor, and resumes plan work after completion.

3. **The fix section is the plan section for the bug** — it has the same rigor as a roadmap section: root cause analysis, TDD matrix (semantic + negative pins), implementation, completion checklist (test-all, TPR, hygiene). No ad-hoc fixes, even for "obvious" bugs.

4. **Fix sections track back to the plan** — note in the fix section: "Discovered during roadmap section {NN} implementation." Note in the plan section: "Blocked by BUG-XX-NNN (see `plans/bug-tracker/fix-BUG-XX-NNN.md`)."

5. **Interference bugs get `/fix-bug` too** — when your plan work surfaces a new bug (test that was passing now fails), revert your change, `/fix-bug` the interfering bug, then re-apply your plan work.

### What NOT to Do

- **Do NOT fix bugs inline without a fix section** — "it's a one-liner" is not an excuse. The fix section ensures TDD, TPR, and hygiene happen.
- **Do NOT skip `/add-bug` for bugs you won't fix now** — the bug tracker is the system of record. Mental notes are deferral.
- **Do NOT rationalize away discovered bugs** — "that's pre-existing" is diagnosis, not justification. File it (`/add-bug`) or fix it (`/fix-bug`).
- **Do NOT batch bug fixes without fix sections** — each bug (or tightly related cluster) gets its own fix section.

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

The website dynamically loads roadmap data from these YAML frontmatter blocks. Incorrect status values cause the roadmap page to show wrong progress information. Overview and index files are the first thing read when resuming work — stale status there causes wasted time re-analyzing completed work.

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
- [ ] Run `/tpr-review` — MUST PASS CLEAN (zero unresolved findings). If findings surface: fix, re-run, repeat until clean. This is definitive — no reasoning about "close enough" or "minor remaining". Clean or open, no middle ground.
- [ ] Run `/impl-hygiene-review` — MUST PASS CLEAN after `/tpr-review` is clean. Catches plumbing-layer issues (SSOT, algorithmic DRY, phase boundaries, naming) that TPR doesn't detect. If findings surface: fix, re-run `/tpr-review` (code changed), then re-run `/impl-hygiene-review`. Both must be clean before section closure.
- [ ] Run `/improve-tooling` **section-close sweep** — MANDATORY safety net at every section close, after both reviews are clean. Per-subsection retrospectives are the PRIMARY tooling capture and should already have been run + committed during Step 5.5 for every subsection in this section. The sweep does TWO things: (1) **Verify** every subsection has either an "improvements made" entry (with commits) or a documented "no gaps" negative finding from its own retrospective. If any subsection skipped its retrospective, STOP and run it now — the sweep cannot substitute for missed per-subsection captures. (2) **Look for cross-subsection patterns** invisible at per-item scope: command sequences repeated when transitioning between *different* subsections, integration test failures with worse messages, mental cross-referencing across files no tool combined, instrumentation that only became obvious after seeing all subsections together. Add only NEW cross-cutting items — do not duplicate per-subsection findings. Implement immediately, commit separately using a valid conventional-commit type (`build(diagnostics): add X — surfaced by section-NN close sweep` — `build`/`test`/`chore`/`ci`/`docs` are the valid types for tooling work; do NOT use `tools(...)`, the lefthook commit-msg hook rejects it), verify against the original scenario. Most sweeps produce zero new findings when per-subsection captures were thorough — that is the expected outcome and must be documented: "Section-close sweep: per-subsection retrospectives covered everything; no cross-subsection patterns required new tooling." Do not silently skip.
- [ ] **Plan-wide accuracy audit** (MANDATORY on every section completion):
  - [ ] Read the ENTIRE `00-overview.md` — verify every section's status in the Quick Reference table matches reality (check frontmatter of each section file). Fix any stale statuses.
  - [ ] Read the ENTIRE `index.md` — verify every section's status matches reality. Fix any stale statuses.
  - [ ] Verify the Estimated Effort table (if it exists) reflects actual status for ALL sections, not just the one you completed.
  - [ ] Check for sections that are effectively complete but still marked "In Progress" due to external blockers (not their own remaining work). If a section's own implementation is done and the only remaining items are blocked by OTHER sections or cross-cutting infrastructure issues, mark it `complete` with a note on the blocker.
  - [ ] Verify `00-overview.md` frontmatter `status` is consistent with overall plan progress (e.g., don't leave it as `not-started` when 6/12 sections are complete).
- [ ] Update parent plan files (if section status changed):
  - [ ] Update `00-overview.md` effort table and Quick Reference table
  - [ ] Update `index.md` section status and Quick Reference table
  - [ ] If plan complete: run "Plan Completion Frontmatter Gate" (see below), then move to `plans/completed/`
- [ ] Run `/commit-push` — NEVER commit directly with `git commit`

---

## Plan Completion Frontmatter Gate

**When ALL sections of a plan are complete**, run this gate before archival:

1. **Verify `00-overview.md` frontmatter**: `status` must be `complete` or `resolved` (not `in-progress` or `not-started`)
2. **Verify `index.md` frontmatter** (if it exists): `status` must be `complete` or `resolved`
3. **Verify Quick Reference table**: every section row in the `| ID | Title | File | Status |` table must show `Complete`
4. **Verify Estimated Effort table** (if it exists): every section row must show `Complete`
5. **Scan for stale `Not Started` or `In Progress`**: grep the overview and index for these strings — if found in section status columns, fix them

6. **Verify plan annotations are cleaned up**: Run `bash .claude/skills/impl-hygiene-review/plan-annotations.sh` and confirm zero annotations remain for this plan's sections. Any remaining TPR, CROSS, BUG, §, Phase, or section- references in `.rs` files are stale scaffolding that must be removed before archival.

If any check fails, fix the issue first, then proceed.

## Plan Archival Protocol

After the frontmatter gate passes:

1. **Move the plan directory**: `git mv plans/<plan-dir> plans/completed/<plan-dir>`
2. **Verify the move**: `ls plans/completed/<plan-dir>/` to confirm files are present
3. **Commit**: use `/commit-push` with a message like `chore: archive completed plan <plan-name>`

Completed plans in `plans/completed/` are still served by the website at the same URLs — no URL changes needed. The `completed/` directory is purely organizational; the website scans both `plans/` and `plans/completed/` for plan content.

---

## Maintaining the Roadmap Index

**IMPORTANT:** When adding new items to the roadmap, update `plans/roadmap/index.md`:

1. **Adding items to existing section**: Add relevant keywords to that section's keyword cluster
2. **Creating a new section**: Add a new keyword cluster block and table entry
3. **Removing/renaming sections**: Update the corresponding entries

The index enables quick topic-based navigation. Keep keyword clusters concise (3-8 lines) and include both formal names and common aliases developers might search for.
