---
name: review-plan
description: Review and improve a plan for accuracy, correctness, feasibility, strategic cohesion, executability, and testing rigor — expand to fulfill the mission, never scope down.
allowed-tools: Read, Grep, Glob, Agent, AskUserQuestion, Bash, Edit, Write
---

# Review Plan Command

Review and improve a plan so that it is accurate, correct, feasible, and forms one cohesive strategy that can be worked sequentially. The goal is to ensure the plan as a whole and each section is executable, fulfills the mission in its entirety, and meets CLAUDE.md testing rigor requirements. If something cannot be fulfilled, the plan must be **expanded** (add sections, add checkboxes, add detail) — never scoped down. 4 sequential review agents each edit the plan directly. **Every agent has full authority** to restructure, reorganize, add/remove/merge/split sections — not just fix details within the existing structure. Each agent brings a different primary lens but is not restricted to it.

## Reviewed Field Semantics — CRITICAL

The `reviewed: true/false` field in section frontmatter is a **pre-implementation gate** — it tracks whether a section has been validated against the current codebase right before implementation begins.

**Why this exists:** As earlier sections are implemented, reality changes — deviations, discoveries, refactors, bug fixes. Later sections were written with assumptions that may now be stale. `reviewed: false` means "not yet validated against implementation reality."

**Two modes — the mode determines whether `reviewed` gets flipped:**

**Single-section review** (`/review-plan plans/foo/section-03.md`):
This is the pre-implementation gate. You're validating one section right before working on it. After all 4 agents complete, flip `reviewed: true` — the sequential pipeline IS the validation (Agent 1 fixes issues, Agents 2-4 each verify the updated plan against the codebase). The only exception: if agents flagged issues they could NOT fix (requiring human judgement), leave `reviewed: false`.

**Whole-plan review** (`/review-plan plans/foo/`):
Improves quality across all sections, but does **NOT** change any `reviewed` values. You're reviewing the plan holistically, not gating specific sections for implementation. Fix content issues, but leave every section's `reviewed` field as-is.

**Both modes:**
- Section 01 should already be `reviewed: true` (starting point). Only flip to `false` if genuinely stale.
- For a section to be marked `reviewed: true`, the agent must confirm:
  1. All file paths, types, functions referenced still exist and are accurate
  2. The approach is still valid given changes made by prior sections
  3. No assumptions were invalidated by earlier implementation work

## Usage

```
/review-plan <plan-path>
```

- `plan-path`: **Required.** Path to the plan directory or a specific plan file (e.g., `plans/hygiene-ori-lexer/`, `plans/roadmap/section-05.md`).
  - If a directory: reviews all files in the directory
  - If a single file: reviews that file (and reads siblings for context)

## Workflow

### Step 0: Read CLAUDE.md (ABSOLUTE FIRST — NO EXCEPTIONS)

**Before doing ANYTHING else**, read the ENTIRE CLAUDE.md file — every single word, top to bottom:

```
Read file: CLAUDE.md
```

This is mandatory. Do not skip, skim, or partially read. The rules in CLAUDE.md govern ALL behavior in this command. Proceed to Step 1 only after reading the complete file.

### Step 1: Read the Plan

Read the plan file(s) specified in `$ARGUMENTS`. If the path doesn't exist, report the error and stop.

- If a directory, read all `.md` files: `index.md`, `00-overview.md`, and all `section-*.md` files
- If a single file, read it plus any sibling plan files for context

### Step 1B: Plan-Wide Accuracy Audit (MANDATORY — before any section-specific review)

**Before starting the section-specific review**, verify the ENTIRE plan's status metadata is accurate and up-to-date. This catches stale statuses from prior work that would mislead the review agents.

1. **Read every section file's frontmatter** — compare each section's `status` field against its actual checkbox state:
   - All `[x]` but `status: in-progress` → fix to `complete`
   - Mixed `[x]`/`[ ]` but `status: not-started` → fix to `in-progress`
   - All `[ ]` but `status: complete` → fix to `not-started` (or `in-progress` if partially done)
   - Subsection statuses must agree with their checkboxes too
2. **Check for "effectively complete" sections** — sections where all own implementation work is done but marked `in-progress` because of external blockers (other sections, cross-cutting infrastructure). If a section's remaining unchecked items are ALL blocked by external issues (not the section's own work), mark it `complete` with a note on the blocker.
3. **Verify `00-overview.md` Quick Reference table** — every section's status must match its frontmatter. Fix any mismatches.
4. **Verify `index.md` section statuses** — must match frontmatter. Fix any mismatches.
5. **Verify Estimated Effort table** (if it exists) — statuses must match reality.
6. **Report fixes** to the user before proceeding: "Plan-wide accuracy audit: fixed N stale statuses before starting review."

This step ensures the review agents are working with accurate metadata, not stale statuses that mask completed work or hide incomplete sections.

### Step 2: Load Hygiene Rules

The full rule set is embedded below (source of truth files — do not maintain separate copies). These rules inform all review agents for checking registration sync points, file size limits, phase boundary discipline, crate dependency ordering, and other hygiene requirements.

**Hygiene Rules** (`.claude/rules/impl-hygiene.md`):
@.claude/rules/impl-hygiene.md

**Compiler Guidelines** (`.claude/rules/compiler.md`):
@.claude/rules/compiler.md

### Step 3: Initial Assessment

Before launching agents, do a quick read-through and report to the user:
- Plan name and scope
- Number of sections/files
- Note: "Running 4 sequential review passes..."

### Step 3B: Third-Party Blind Spot Check via /tp-help

**SEQUENTIAL & FOREGROUND — MANDATORY.** This `/tp-help` call MUST run in the foreground (NOT `run_in_background`). You MUST wait for it to complete and read its output before proceeding to Step 4. Do NOT launch this in parallel with any other agent or skill invocation.

**Before launching the 4 review agents**, call `/tp-help` to identify blind spots the review should focus on.

Build a `/tp-help` prompt that includes:
- The plan's mission/goal (from overview)
- The section list with their goals and statuses
- A brief summary of the plan's scope (which crates, which subsystems)
- Whether this is a single-section or whole-plan review

Ask Codex specifically:
- "Given this plan's scope, what are the most likely failure modes the review should watch for?"
- "What architectural risks or blind spots would you flag?"
- "Are there cross-cutting concerns that might fall between section boundaries?"

Use Codex's response to inform the review — add specific items to watch for in the agent prompts if Codex identifies something non-obvious that the standard review lenses might miss.

### Step 4: Sequential Independent Review (4 Agents)

Run **4 review agents in sequence** (NOT parallel). Each agent:

- Receives **only the plan files** — no conversation context, no reasoning behind the plan
- Has **FULL AUTHORITY** to restructure, reorganize, add sections, remove sections, merge sections, split sections, reorder sections, rewrite the overview/index, and make any structural change they deem necessary
- Is instructed to **read the plan, review it, and edit the files directly** to fix issues
- Sees edits made by all previous agents (because they run sequentially)
- Brings a **primary lens** (what they focus on most deeply) but is NOT restricted to that lens — if they see something wrong outside their primary focus, they fix it

This creates an iterative refinement pipeline: each reviewer builds on the last with escalating structural authority. Agent 1 might fix paths; Agent 2 might reorganize the entire plan; Agent 3 might split oversized sections; Agent 4 ties it all together.

**IMPORTANT**: Run these agents ONE AT A TIME. Wait for each to complete before starting the next.

#### Agent 1: Primary Lens — Technical Accuracy & Feasibility

Spawn an Agent with the following prompt (substitute `{plan_dir}` with the actual plan directory path):

```
You are reviewing an existing plan for the Ori compiler at {plan_dir}/.

PRIMARY LENS: Technical accuracy and feasibility — verify every technical claim is accurate AND every step is actually feasible. But this is your LENS, not your BOUNDARY.

## FULL AUTHORITY — READ THIS FIRST

You have FULL AUTHORITY to make ANY structural change to this plan. You are not limited to fixing inaccuracies within the existing structure. If the plan's structure is wrong, fix the structure. Specifically, you may and should:

- **Add new sections** if coverage gaps exist
- **Remove sections** that are redundant or misguided
- **Merge sections** that are artificially split
- **Split sections** that try to do too much
- **Reorder sections** if the dependency flow is wrong
- **Rewrite the overview and index** to match structural changes
- **Restructure the entire plan** if the current organization doesn't serve the mission
- **Rewrite checklist items** that are vague, wrong, or missing the point
- **Change section boundaries** — move items between sections if they belong elsewhere

The plan exists to serve the mission. If the structure fights the mission, change the structure. You are not a proofreader — you are an architect with editing authority. Think about whether this plan, as structured, is the RIGHT plan — not just whether its details are correct.

Never scope down — expand or redesign the approach to make it work.

CRITICAL PREREQUISITE: Before starting, read the ENTIRE CLAUDE.md file (every word):
```
Read file: CLAUDE.md
```

INSTRUCTIONS:

## Part 1: Technical Accuracy (Primary Focus)
1. Read ALL files in {plan_dir}/ (index.md, 00-overview.md, and all section-*.md files)
2. Cross-reference every technical claim against the actual codebase:
   - Do referenced files, types, functions, modules exist?
   - Are crate dependency assumptions correct? (ori_lexer → ori_parse → ori_ir → ori_types → ori_eval → ori_llvm → oric)
   - Are described code patterns accurate?
3. Check claims against the spec in docs/ori_lang/v2026/spec/ (grammar.ebnf, operator-rules.md, clause files)
4. For every inaccuracy found, EDIT the plan files directly to fix them
5. If a section references nonexistent code paths or wrong file locations, correct them

## Part 2: Feasibility Assessment
6. For each section, assess whether the described implementation approach will actually work:
   - Can each checklist item be implemented as described?
   - Are there hidden prerequisites or dependencies not mentioned?
   - Does the approach handle the full problem space, or only a subset?
   - Are there architectural constraints (file size limits, phase boundaries, crate deps) that would block the approach?
7. If a step is infeasible:
   - Do NOT remove it or mark it as "future work"
   - EXPAND the approach: add prerequisite steps, restructure the section, or add a new section that addresses the blocker
   - If the step needs architectural change, describe that architectural change as concrete checklist items
8. If a section's scope is too narrow to fulfill the plan's stated mission for that area:
   - EXPAND it with additional checklist items covering the missing scope
   - Add detail on what was missing and why it matters

## Part 3: Structural Assessment
9. Step back and assess the plan AS A WHOLE:
   - Is this the right set of sections? Would a different decomposition serve the mission better?
   - Are sections at the right granularity? (Too fine = overhead; too coarse = unexecutable)
   - Does the section ordering reflect actual implementation dependencies?
   - If you see a better structure, IMPLEMENT IT — don't just note it

## Part 4: TPR Findings
10. When reviewing TPR (Third Party Review) findings: you MUST NOT dismiss findings because they are "not related" to the current plan, "out of scope", or "pre-existing." Per CLAUDE.md there is no "unrelated" or "out of scope." If a finding identifies a real issue, it must be accepted. Only reject findings that are factually incorrect (the issue does not actually exist).

## CRITICAL: `reviewed` field in frontmatter

Each section has a `reviewed: true/false` field in its YAML frontmatter. This tracks whether the section's assumptions have been validated against the CURRENT codebase right before implementation.

**Two modes — check which one you're in:**

**Mode A — Single-section review** (you were given a specific section file, not a directory):
This is the pre-implementation gate. After confirming ALL technical claims are accurate:
- If everything checks out (no fixes needed): set `reviewed: true`
- If you found inaccuracies and fixed them: set `reviewed: true` — the 4-agent sequential pipeline validates the fixes (Agents 2-4 each verify the updated plan against the codebase)
- If you found issues you could NOT fix (require human judgement): LEAVE as `reviewed: false`

**Mode B — Whole-plan review** (you were given a directory):
Do NOT change any `reviewed` values. Fix inaccuracies in content, but leave `reviewed: true/false` as-is on every section. The whole-plan review improves quality but is not the pre-implementation gate.

**Both modes:**
- Section 01 should normally be `reviewed: true`. Only flip to `false` if it has genuinely stale content.
- Sections already `reviewed: true`: verify they're still accurate. If stale, flip to `false` and note why.

Add a brief comment near each fix: <!-- reviewed: accuracy/feasibility fix -->
After editing, list what you changed and why — including any structural changes (sections added, removed, merged, split, reordered).
```

#### Agent 2: Primary Lens — Strategic Cohesion & Mission Fulfillment

```
You are reviewing an existing plan for the Ori compiler at {plan_dir}/.

PRIMARY LENS: Strategic cohesion and mission fulfillment — ensure the plan works as ONE cohesive strategy that delivers its mission completely. But this is your LENS, not your BOUNDARY.

## FULL AUTHORITY — READ THIS FIRST

You have FULL AUTHORITY to make ANY structural change to this plan. You are not limited to gap-filling within the existing structure. If the plan's structure is wrong, fix the structure. Specifically, you may and should:

- **Add new sections** if coverage gaps exist
- **Remove sections** that are redundant or misguided
- **Merge sections** that are artificially split
- **Split sections** that try to do too much
- **Reorder sections** if the dependency flow is wrong
- **Rewrite the overview and index** to match structural changes
- **Restructure the entire plan** if the current organization doesn't serve the mission
- **Rewrite checklist items** that are vague, wrong, or missing the point
- **Change section boundaries** — move items between sections if they belong elsewhere

The plan exists to serve the mission. If the structure fights the mission, change the structure. You are not a gap-filler — you are an architect with editing authority. Think about whether this plan, as structured, is the RIGHT plan — not just whether it covers enough.

A previous agent (Agent 1) may have already made structural changes. Build on those changes — validate them, improve them, or redo them if they don't serve the mission.

CRITICAL PREREQUISITE: Before starting, read the ENTIRE CLAUDE.md file (every word):
```
Read file: CLAUDE.md
```

INSTRUCTIONS:

## Part 1: Mission Fulfillment
1. Read ALL files in {plan_dir}/ (index.md, 00-overview.md, and all section-*.md files)
2. Identify the plan's stated mission/goal (from index.md or 00-overview.md)
3. For each aspect of the mission, verify there is at least one section that addresses it:
   - If a mission goal has NO section addressing it: ADD a new section
   - If a mission goal is partially addressed: EXPAND the relevant section(s) with additional checkboxes
   - If a mission goal is mentioned but deferred: REMOVE the deferral language and make it concrete
4. The plan must not just "get started" on its mission — it must deliver the mission completely. If the plan currently ends at 70% of the mission, add sections for the remaining 30%.

## Part 2: Sequential Flow & Dependencies
5. Verify that sections can be worked in order (section N before section N+1):
   - Does each section's output provide what the next section needs as input?
   - Are there circular dependencies between sections? (These must be resolved by reordering or splitting)
   - If section N depends on something from section M where M > N, that's a dependency violation — reorder
6. For each section, verify its prerequisites are explicit:
   - What must be true in the codebase before this section can begin?
   - Which prior sections must be complete?
   - If prerequisites are implicit, make them explicit
7. Check for "orphan sections" — sections that neither depend on nor feed into other sections. These may indicate scope gaps or misplacement.

## Part 3: Completeness & Gap Analysis
8. Review each section for completeness:
   - Are there missing steps that would block implementation?
   - Are edge cases and error handling accounted for?
   - Are dependencies between sections correctly identified?
   - Check for missing sync points — if the plan adds enum variants, new types, or registration entries, does it list ALL locations that must be updated together?
9. For every gap found, EDIT the plan files directly to add the missing content — add checklist items, steps, or entire sections
10. Flag and fix deferral traps: items labeled "bonus", "future", "lower priority", "nice to have", "stretch goal", or "requires architectural change." These invite the implementer to skip them. Rewrite as concrete, mandatory tasks or mark with explicit `<!-- blocked-by:X -->` if genuinely blocked. Remove all soft deferral language.
11. When reviewing TPR (Third Party Review) findings: you MUST NOT dismiss findings because they are "not related" to the current plan, "out of scope", or "pre-existing." Per CLAUDE.md there is no "unrelated" or "out of scope." If a finding identifies a real issue, it must be accepted. Only reject findings that are factually incorrect (the issue does not actually exist).

## Part 4: Expansion, Not Scoping Down
12. THE CARDINAL RULE: Never scope down. If the plan is too small for the mission, grow the plan. If a section is too thin, add checkboxes. If a goal requires architectural work, add the architectural work as concrete steps. "Requires architectural change" is not a reason to defer — it IS the work.
13. Check the overview (00-overview.md) and index.md — do they accurately reflect all sections? Update them if sections were added or restructured.

Add a brief comment near each addition: <!-- reviewed: cohesion fix -->
After editing, list what you changed and why — especially any structural changes (sections added, removed, merged, split, reordered) and significant scope expansions.
```

#### Midpoint Check: /tp-help Between Agent 2 and Agent 3

**SEQUENTIAL & FOREGROUND — MANDATORY.** This `/tp-help` call MUST run in the foreground (NOT `run_in_background`). You MUST wait for it to complete and read its output before launching Agent 3. Do NOT launch this in parallel with Agent 3 or any other agent.

**After Agent 2 completes**, call `/tp-help` for a midpoint structural check before the executability and testing passes.

Build a `/tp-help` prompt that includes:
- The plan's mission (one line)
- A summary of what Agents 1 and 2 changed (structural changes, accuracy fixes, cohesion fixes, sections added/removed/reordered)
- The current section list after Agents 1-2's modifications

Ask Codex specifically:
- "Agents 1-2 made these structural changes. Do you see any executability or hygiene concerns with the resulting structure?"
- "Are there sections that look too large or too vague to implement in a single session?"
- "Any cross-section dependency issues in this ordering?"

Feed relevant insights into Agent 3's prompt as additional focus areas. This ensures the executability review is informed by an outside perspective on the post-restructuring state.

#### Agent 3: Primary Lens — Section Executability & Codebase Hygiene

```
You are reviewing an existing plan for the Ori compiler at {plan_dir}/.

PRIMARY LENS: Section executability and codebase hygiene — ensure every checklist item is a concrete, actionable task and that the plan accounts for existing code issues. But this is your LENS, not your BOUNDARY.

## FULL AUTHORITY — READ THIS FIRST

You have FULL AUTHORITY to make ANY structural change to this plan. You are not limited to expanding items within the existing structure. If the plan's structure is wrong, fix the structure. Specifically, you may and should:

- **Add new sections** if coverage gaps exist
- **Remove sections** that are redundant or misguided
- **Merge sections** that are artificially split
- **Split sections** that try to do too much (especially sections with 20+ checklist items)
- **Reorder sections** if the dependency flow is wrong
- **Rewrite the overview and index** to match structural changes
- **Restructure the entire plan** if the current organization doesn't serve the mission
- **Rewrite checklist items** that are vague, wrong, or missing the point
- **Change section boundaries** — move items between sections if they belong elsewhere

The plan exists to serve the mission. If the structure fights executability, change the structure. Previous agents (1-2) may have already made structural changes. Build on those — validate, improve, or redo as needed.

CRITICAL PREREQUISITE: Before starting, read the ENTIRE CLAUDE.md file (every word):
```
Read file: CLAUDE.md
```

INSTRUCTIONS:

## Part 1: Section Executability (Primary Focus)

1. Read ALL files in {plan_dir}/ (index.md, 00-overview.md, and all section-*.md files)
2. For each section, assess executability — could an implementer sit down and work through every checklist item in order?
   - Is each checklist item a concrete, verifiable task (not a vague goal like "improve X" or "handle edge cases")?
   - Does each item specify WHAT to do and WHERE (file paths, function names, crate)?
   - Are there hidden steps between checklist items that aren't written down?
   - Would an implementer need to make design decisions not covered by the plan?
3. For vague or under-specified items, EXPAND them:
   - Break vague items into specific sub-items with file paths and approach
   - Add missing intermediate steps
   - Add "WHERE:" annotations when the location isn't obvious
4. If a section is too thin to be worked (fewer than 3 substantive checklist items), it needs expansion:
   - Research the codebase to understand what the section actually requires
   - Add concrete checklist items based on what you find
   - A section that says "implement X" with one checkbox is not executable — it needs the HOW
5. If a section is too large to be worked in one sitting (20+ items, or mixes unrelated concerns), SPLIT IT into focused sections
6. Check for items that would violate implementation hygiene:
   - Read the hygiene rules at .claude/rules/impl-hygiene.md and compiler guidelines at .claude/rules/compiler.md
   - Does the plan respect file size limits (500 lines)?
   - Does it maintain phase boundary discipline?
   - Are implementation steps ordered correctly (upstream crates before downstream)?
   - Does it follow test file conventions (sibling tests.rs)?
7. Reorder items within sections if they violate crate dependency ordering (ori_lexer → ori_parse → ori_ir → ori_types → ori_eval → ori_llvm → oric)
8. Add warnings for steps that are particularly complex or risky
9. Verify every code-modifying section includes matrix testing requirements:
   - Does the section specify its test matrix dimensions (which types x which patterns)?
   - Does it include at least one semantic pin requirement (a test that ONLY passes with the new semantics)?
   - Does it specify TDD ordering (failing tests FIRST, debug+release verification LAST)?
   - If missing, add concrete test checklist items based on the codebase research — identify the types and patterns that flow through the code the section modifies

## Part 2: Codebase Scan — "Leave It Better Than You Found It"

10. Extract from the plan every file path, crate, and module that will be touched
11. Actually READ those files (up to 30 files; prioritize files mentioned in multiple sections)
12. Audit each file against the hygiene rules, looking for existing issues:
    - **BLOAT**: Files over 500 lines that the plan will touch but doesn't plan to split
    - **WASTE**: Unnecessary clones, allocations, stale comments, dead code, commented-out code
    - **DRIFT**: Registration sync points that are already out of sync
    - **EXPOSURE**: Internal state leaking through boundary types
    - **LEAK**: Phase bleeding in files the plan modifies
    - **STYLE**: Missing docs on pub items, bare TODOs, decorative banners, inline test modules
13. EDIT the plan files to weave "fix along the way" checklist items into the appropriate sections, using:
    - [ ] **[BLOAT]** `file:line` — Split into submodules (currently N lines, exceeds 500-line limit)
    - [ ] **[WASTE]** `file:line` — Remove stale comment / dead code / unnecessary clone
    - [ ] **[DRIFT]** `file:line` — Sync missing variant with parallel location at `other_file:line`
    Place these near existing items that touch the same file. Group under "Cleanup" sub-heading if 3+ findings per section.
14. If findings cluster (5+ in one module), add: "⚠ Clustered findings suggest deeper design issue — consider architectural review before proceeding"
15. Do NOT fabricate findings. Every finding must reference a real file:line with a real issue.

Add a brief comment near each change: <!-- reviewed: executability/hygiene fix -->
After editing, list:
- Structural changes (sections added, removed, merged, split, reordered)
- Sections expanded and how many items added
- Vague items made concrete
- Codebase findings woven in, by category
- Files scanned vs files with findings
```

#### Agent 4: Primary Lens — Testing Rigor, Clarity & Final Integration

```
You are reviewing an existing plan for the Ori compiler at {plan_dir}/.

PRIMARY LENS: Testing rigor, clarity, and final integration — ensure every section has adequate test strategy, the plan reads coherently, and all prior agents' changes are consistent. But this is your LENS, not your BOUNDARY.

## FULL AUTHORITY — READ THIS FIRST

You have FULL AUTHORITY to make ANY structural change to this plan. You are the final agent — you see the cumulative work of Agents 1-3. If their structural changes created inconsistencies, or if you see a better structure now that the dust has settled, FIX IT. Specifically, you may and should:

- **Add new sections** if coverage gaps exist
- **Remove sections** that are redundant or misguided
- **Merge sections** that are artificially split
- **Split sections** that try to do too much
- **Reorder sections** if the dependency flow is wrong
- **Rewrite the overview and index** to match structural changes
- **Restructure the entire plan** if the current organization doesn't serve the mission
- **Rewrite checklist items** that are vague, wrong, or missing the point
- **Change section boundaries** — move items between sections if they belong elsewhere
- **Undo or revise changes from Agents 1-3** if they made the plan worse

You are the final architect. The plan that exists after you are done is the plan that gets executed. Make it right.

CRITICAL PREREQUISITE: Before starting, read the ENTIRE CLAUDE.md file (every word):
```
Read file: CLAUDE.md
```

INSTRUCTIONS:

## Part 1: Testing Rigor (Primary Focus — per CLAUDE.md)

1. Read ALL files in {plan_dir}/ (index.md, 00-overview.md, and all section-*.md files)
2. For EVERY section that modifies compiler code, verify it has a test strategy that meets CLAUDE.md requirements:

   **CLAUDE.md TDD Requirements** (these are non-negotiable):
   - Every fix requires a test that catches its regression (no fix lands without a test)
   - **Matrix tests**: not just "multiple tests" — every fix requires:
     - Exact failing case (the specific input that triggered the bug)
     - Edge cases (empty, single-element, boundary conditions)
     - Cross-type coverage (if type-dependent: test ALL relevant types through same code path — str, [int], Option<str>, closures, structs, maps, sets)
     - Cross-pattern coverage (if pattern-dependent: test ALL relevant control-flow patterns — full iteration, break, yield, guard, nested, two-call)
     - Semantic pin: at least one test that ONLY passes with the new semantics
   - Tests verify fail FIRST, then fix, then tests pass unchanged
   - Debug AND release builds must pass

3. For each section, check:
   - Does it specify what types of tests are needed (Rust unit, Ori spec, AOT, etc.)?
   - Does it describe the test matrix dimensions (which types x which patterns)?
   - Does it include semantic pin tests?
   - Does it specify that tests should be written BEFORE the fix (TDD)?
   - Does it account for both debug and release testing?
4. If a section's test strategy is missing or inadequate:
   - ADD concrete test checklist items with matrix dimensions
   - Specify which test files to create or update
   - Add "- [ ] Write failing test matrix BEFORE implementation" as the FIRST item
   - Add "- [ ] Verify all tests pass in both debug and release" as the LAST item
   - Add semantic pin requirements: "- [ ] Add semantic pin test that only passes with new behavior"
5. If the plan has a testing section, verify it covers the full scope. If there is no dedicated testing section AND sections lack embedded test strategies, ADD a testing section or expand each section's test items.

## Part 2: Clarity & Consistency

6. Review for clarity and internal consistency:
   - Are section descriptions clear and unambiguous?
   - Is terminology consistent across sections?
   - Does the overview (00-overview.md) accurately reflect the section contents?
   - Does index.md have accurate keyword clusters for each section?
   - Are there contradictions between sections?
7. Fix inconsistent terminology
8. Update the overview and index if sections have changed during prior reviews

## Part 3: Final Integration & Cleanup

9. Remove all <!-- reviewed: ... --> comments left by previous reviewers
10. **Validate Agents 1-3's structural changes**: Read the plan as a whole. Do the structural changes made by prior agents (added sections, reordering, merging, splitting) create a coherent plan? If not:
    - Fix inconsistencies between sections
    - Ensure new sections added by prior agents have proper frontmatter, numbering, and are reflected in overview/index
    - If a prior agent's structural change was misguided, undo or revise it
11. Verify and finalize `reviewed` field in frontmatter:
    - Every section file MUST have a `reviewed: true/false` field
    - If a section is missing the field, add `reviewed: false`
    - **Single-section review (Mode A):** After your final coherence check, set `reviewed: true` — the 4-agent pipeline has validated the section. Exception: if any agent flagged unfixable issues requiring human judgement, leave `reviewed: false`.
    - **Whole-plan review (Mode B):** Do NOT change any `reviewed` values.
    - Report any sections missing the field
12. Final coherence check: read through the entire plan one more time. Does it tell a complete, sequential story from start to finish? Is this the RIGHT plan for the mission — not just a cleaned-up version of whatever was there before?

After editing, list what you changed and why — especially any test strategy gaps filled, any structural changes, and any corrections to prior agents' work.
```

### Step 5: Present Verdict

After all four agents complete, consolidate their findings into a summary ranked by severity (**Critical** > **Major** > **Minor**).

```
## Plan Review: {plan name}

### Changes Made

#### Agent 1 — Technical Accuracy & Feasibility (+ structural changes)
- {list of edits made}

#### Agent 2 — Strategic Cohesion & Mission Fulfillment (+ structural changes)
- {list of edits made}

#### Agent 3 — Section Executability & Codebase Hygiene (+ structural changes)
- {list of edits made}

#### Agent 4 — Testing Rigor, Final Integration (+ structural changes)
- {list of edits made}

### Review Status

| Section | `reviewed` Before | `reviewed` After | Reason |
|---------|------------------|-----------------|--------|
| 01 | true | true | Starting point, confirmed accurate |
| 02 | false | true/false | {reason} |
| ... | ... | ... | ... |

### Remaining Concerns

{Any issues the agents flagged but could not fix automatically,
ranked by severity: Critical > Major > Minor}

---

## Verdict

**{CLEAN | MINOR FIXES APPLIED | SIGNIFICANT REWORK APPLIED | NEEDS MANUAL ATTENTION}**

{2-3 sentence overall assessment. Note the plan's strengths as well as weaknesses.
State total number of edits made across all agents. Flag anything that
requires human judgement rather than mechanical fixes.}
```

**Verdict definitions:**
- **CLEAN**: No issues found. Plan is ready for implementation.
- **MINOR FIXES APPLIED**: Small corrections made (typos, wrong paths, minor gaps). Plan is ready.
- **SIGNIFICANT REWORK APPLIED**: Substantial edits (reordered steps, added missing sections, fixed incorrect assumptions). Review the diff before proceeding.
- **RESTRUCTURED**: Plan structure was fundamentally changed (sections added/removed/merged/split/reordered). Review the new structure before proceeding.
- **NEEDS MANUAL ATTENTION**: Issues found that require human judgement — architectural decisions, ambiguous scope, conflicting requirements. Cannot be auto-fixed.

## Important Rules

0. **ALL external consultations (`/tp-help`, `/tpr-review`) are SEQUENTIAL and FOREGROUND** — NEVER launch these as background tasks (`run_in_background: true`). NEVER launch them in parallel with each other or with review agents. Each must complete fully and its output must be read and incorporated before proceeding to the next step. The entire pipeline is sequential by design — each step's output informs the next.
1. **Every agent has FULL AUTHORITY** — Each agent can add, remove, merge, split, reorder sections, restructure the entire plan, rewrite the overview/index, and make any change they deem necessary. The "primary lens" shapes what they focus on, NOT what they're permitted to do. A review agent that notices a structural problem but doesn't fix it because "that's not my focus area" has failed.
2. **Agents edit directly** — This is not a report-only review. Agents fix what they find.
3. **Sequential, not parallel** — Each agent sees prior agents' edits. Order matters. Later agents validate, build on, or undo earlier agents' structural changes.
4. **Be specific** — Every change needs evidence: a spec clause, a file:line, or concrete reasoning.
5. **Cross-reference, don't guess** — Agents must actually read spec files and source code.
6. **Check crate dependency order** — Implementation steps must respect: `ori_lexer → ori_parse → ori_ir → ori_types → ori_eval → ori_llvm → oric`.
7. **Clean up after yourself** — Agent 4 removes all `<!-- reviewed: ... -->` markers.
8. **Flag what can't be auto-fixed** — Architectural decisions and scope questions go in "Remaining Concerns" for human review.
9. **NEVER scope down — always expand** — If the plan doesn't fulfill its mission, grow the plan. Add sections, add checkboxes, add detail. "Requires architectural change" is not a reason to defer — it IS the work. Every gap in mission fulfillment must be filled with concrete, actionable items.
10. **No deferral traps** — Flag any plan items that create temptation to defer during implementation. Items labeled "bonus", "future", "lower priority", or "requires architectural change" are red flags. Every checkbox in a section must be implementable by the agent executing the section. If an item genuinely cannot be implemented within the section's scope (missing language feature, external dependency), it should be marked `<!-- blocked-by:X -->` with a concrete blocker — not soft language that invites skipping. Agents should rewrite soft deferral language into concrete, actionable tasks or explicit blockers.
11. **No dismissing TPR findings as "unrelated"** — When triaging Third Party Review findings, you MUST NOT dismiss a finding because it is "not related" to the current plan, "out of scope", or "pre-existing." Per CLAUDE.md: there is no "unrelated", "pre-existing", or "out of scope." If a TPR finding identifies a real issue in the codebase, it must be accepted and addressed. The ONLY valid reason to reject a TPR finding is that the described issue does not actually exist in the codebase (factually incorrect).
12. **Testing rigor is non-negotiable** — Every section that modifies code must have a test strategy meeting CLAUDE.md requirements: matrix tests (type x pattern coverage with explicit dimension names), semantic pins (tests that ONLY pass with the new semantics), TDD ordering (failing tests as first item, debug+release as last item), and cross-section coverage when touching shared code paths. Plans should arrive with these from `/create-plan`, but if missing, Agent 3 and Agent 4 must add them — a section without matrix dimensions and semantic pins is not executable.
13. **Cohesive sequential strategy** — The plan must read as one continuous strategy. Each section builds on prior sections. No orphan sections, no circular dependencies, no implicit prerequisites.
