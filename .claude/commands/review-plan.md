---
name: review-plan
description: Review a plan for problems — technical accuracy, completeness, hygiene compliance, and crate dependency ordering.
allowed-tools: Read, Grep, Glob, Agent, AskUserQuestion, Bash, Edit, Write
---

# Review Plan Command

Read a plan, cross-reference it against the codebase, spec, and hygiene rules, then fix problems directly via 4 sequential review agents. Report findings as a verdict.

## Reviewed Field Semantics — CRITICAL

The `reviewed: true/false` field in section frontmatter is a **pre-implementation gate** — it tracks whether a section has been validated against the current codebase right before implementation begins.

**Why this exists:** As earlier sections are implemented, reality changes — deviations, discoveries, refactors, bug fixes. Later sections were written with assumptions that may now be stale. `reviewed: false` means "not yet validated against implementation reality."

**Two modes — the mode determines whether `reviewed` gets flipped:**

**Single-section review** (`/review-plan plans/foo/section-03.md`):
This is the pre-implementation gate. You're validating one section right before working on it. After agents confirm ALL technical claims are accurate, flip `reviewed: true`. If fixes were needed, leave `reviewed: false` — fixes need their own validation pass.

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

### Step 1: Read the Plan

Read the plan file(s) specified in `$ARGUMENTS`. If the path doesn't exist, report the error and stop.

- If a directory, read all `.md` files: `index.md`, `00-overview.md`, and all `section-*.md` files
- If a single file, read it plus any sibling plan files for context

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

### Step 4: Sequential Independent Review (4 Agents)

Run **4 review agents in sequence** (NOT parallel). Each agent:

- Receives **only the plan files** — no conversation context, no reasoning behind the plan
- Is instructed to **read the plan, review it, and edit the files directly** to fix issues
- Sees edits made by all previous agents (because they run sequentially)

This creates an iterative refinement pipeline: each reviewer builds on the last.

**IMPORTANT**: Run these agents ONE AT A TIME. Wait for each to complete before starting the next.

#### Agent 1: Technical Accuracy Review

Spawn an Agent with the following prompt (substitute `{plan_dir}` with the actual plan directory path):

```
You are reviewing an existing plan for the Ori compiler at {plan_dir}/.

INSTRUCTIONS:
1. Read ALL files in {plan_dir}/ (index.md, 00-overview.md, and all section-*.md files)
2. Cross-reference every technical claim against the actual codebase:
   - Do referenced files, types, functions, modules exist?
   - Are crate dependency assumptions correct? (ori_lexer → ori_parse → ori_ir → ori_types → ori_eval → ori_llvm → oric)
   - Are described code patterns accurate?
3. Check claims against the spec in docs/ori_lang/v2026/spec/ (grammar.ebnf, operator-rules.md, clause files)
4. For every inaccuracy found, EDIT the plan files directly to fix them
5. If a section references nonexistent code paths or wrong file locations, correct them
6. Add a brief comment near each fix: <!-- reviewed: accuracy fix -->

## CRITICAL: `reviewed` field in frontmatter

Each section has a `reviewed: true/false` field in its YAML frontmatter. This tracks whether the section's assumptions have been validated against the CURRENT codebase right before implementation.

**Two modes — check which one you're in:**

**Mode A — Single-section review** (you were given a specific section file, not a directory):
This is the pre-implementation gate. After confirming ALL technical claims are accurate:
- If everything checks out: set `reviewed: true`
- If you found inaccuracies and fixed them: LEAVE as `reviewed: false` — the fixes need validation
- If you found issues you could not fix: LEAVE as `reviewed: false`

**Mode B — Whole-plan review** (you were given a directory):
Do NOT change any `reviewed` values. Fix inaccuracies in content, but leave `reviewed: true/false` as-is on every section. The whole-plan review improves quality but is not the pre-implementation gate.

**Both modes:**
- Section 01 should normally be `reviewed: true`. Only flip to `false` if it has genuinely stale content.
- Sections already `reviewed: true`: verify they're still accurate. If stale, flip to `false` and note why.

You may add missing sections, expand scope, or restructure if the plan is genuinely incomplete.
After editing, list what you changed and why.
```

#### Agent 2: Completeness & Gap Review

```
You are reviewing an existing plan for the Ori compiler at {plan_dir}/.

INSTRUCTIONS:
1. Read ALL files in {plan_dir}/ (index.md, 00-overview.md, and all section-*.md files)
2. Review each section for completeness:
   - Are there missing steps that would block implementation?
   - Are edge cases and error handling accounted for?
   - Are dependencies between sections correctly identified?
   - Are test strategies adequate for each section?
3. Check for missing sync points — if the plan adds enum variants, new types, or registration entries, does it list ALL locations that must be updated together?
4. For every gap found, EDIT the plan files directly to add the missing content
5. Add missing checklist items, missing steps, missing test requirements
6. Flag deferral traps: items labeled "bonus", "future", "lower priority", "nice to have", "stretch goal", or "requires architectural change". These invite the implementer to skip them. Rewrite as concrete, mandatory tasks or mark with explicit `<!-- blocked-by:X -->` if genuinely blocked. Remove all soft deferral language.
7. Add a brief comment near each addition: <!-- reviewed: completeness fix -->

You may add new sections, restructure, or expand scope if the plan has genuine gaps.
After editing, list what you changed and why.
```

#### Agent 3: Hygiene & Feasibility Review (Codebase-Aware)

```
You are reviewing an existing plan for the Ori compiler at {plan_dir}/.

Your job is twofold: (1) ensure the plan itself follows hygiene rules, and (2) scan the actual codebase areas the plan will touch to find existing issues that should be cleaned up along the way. The principle: every plan section should leave the code better and cleaner than before.

INSTRUCTIONS:

## Part 1: Plan-Level Hygiene

1. Read ALL files in {plan_dir}/ (index.md, 00-overview.md, and all section-*.md files)
2. Read the hygiene rules at .claude/rules/impl-hygiene.md and compiler guidelines at .claude/rules/compiler.md
3. Review the plan against these rules:
   - Does the plan respect file size limits (500 lines)?
   - Does it maintain phase boundary discipline?
   - Does it follow the test file conventions (sibling tests.rs)?
   - Are implementation steps ordered correctly (upstream before downstream)?
   - Are there steps that are impractical or underestimate complexity?
4. Reorder steps if they violate crate dependency ordering
5. Add warnings for steps that are particularly complex or risky

## Part 2: Codebase Scan — "Leave It Better Than You Found It"

6. Extract from the plan every file path, crate, and module that will be touched (look for file:line references, crate names, module paths in checklist items and prose)
7. Actually READ those files (up to 30 files; prioritize files mentioned in multiple sections or that are core to the plan's goal)
8. Audit each file against the hygiene rules, looking for existing issues:
   - **BLOAT**: Files over 500 lines that the plan will touch but doesn't plan to split
   - **WASTE**: Unnecessary clones, allocations, stale comments, dead code, commented-out code
   - **DRIFT**: Registration sync points that are already out of sync
   - **EXPOSURE**: Internal state leaking through boundary types
   - **LEAK**: Phase bleeding in files the plan modifies
   - **STYLE**: Missing docs on pub items, bare TODOs, decorative banners, inline test modules
   - Any other violations from impl-hygiene.md
9. For each finding, identify which plan section touches that file/area
10. EDIT the plan files to weave "fix along the way" checklist items into the appropriate sections, using this format:
    - [ ] **[BLOAT]** `file:line` — Split into submodules (currently N lines, exceeds 500-line limit)
    - [ ] **[WASTE]** `file:line` — Remove stale comment / dead code / unnecessary clone
    - [ ] **[DRIFT]** `file:line` — Sync missing variant with parallel location at `other_file:line`
    Place these items near the existing checklist items that touch the same file, so the implementer fixes them in the same pass. Group them under a "Cleanup" sub-heading within the section if there are 3+ findings for that section.
11. If findings cluster (5+ in one module), add a note: "⚠ Clustered findings suggest deeper design issue — consider architectural review before proceeding"
12. Do NOT fabricate findings. Every finding must reference a real file:line with a real issue. If the touched code is already clean, say so.

## Output

Add a brief comment near each change: <!-- reviewed: hygiene fix -->
After editing, list:
- Plan-level fixes made (reordering, warnings, etc.)
- Codebase findings woven in, by category (e.g., 3 BLOAT, 2 WASTE, 1 DRIFT)
- Files scanned vs files with findings
```

#### Agent 4: Clarity & Consistency Review

```
You are reviewing an existing plan for the Ori compiler at {plan_dir}/.

INSTRUCTIONS:
1. Read ALL files in {plan_dir}/ (index.md, 00-overview.md, and all section-*.md files)
2. Review for clarity and internal consistency:
   - Are section descriptions clear and unambiguous?
   - Do checklist items describe concrete, actionable tasks (not vague goals)?
   - Is terminology consistent across sections?
   - Does the overview (00-overview.md) accurately reflect the section contents?
   - Does index.md have accurate keyword clusters for each section?
   - Are there contradictions between sections?
3. For every issue found, EDIT the plan files directly to improve clarity
4. Sharpen vague checklist items into specific, verifiable tasks
5. Fix inconsistent terminology
6. Update the overview if sections have changed during prior reviews
7. Remove all <!-- reviewed: ... --> comments left by previous reviewers (clean up)
8. Verify `reviewed` field consistency in frontmatter:
   - Every section file MUST have a `reviewed: true/false` field
   - If a section is missing the field, add `reviewed: false`
   - Do NOT change any `reviewed` values — Agent 1 handles that based on accuracy validation
   - Report any sections missing the field

After editing, list what you changed and why.
```

### Step 5: Present Verdict

After all four agents complete, consolidate their findings into a summary ranked by severity (**Critical** > **Major** > **Minor**).

```
## Plan Review: {plan name}

### Changes Made

#### Agent 1 — Technical Accuracy
- {list of edits made}

#### Agent 2 — Completeness & Gaps
- {list of edits made}

#### Agent 3 — Hygiene & Feasibility
- {list of edits made}

#### Agent 4 — Clarity & Consistency
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
- **NEEDS MANUAL ATTENTION**: Issues found that require human judgement — architectural decisions, ambiguous scope, conflicting requirements. Cannot be auto-fixed.

## Important Rules

1. **Agents edit directly** — This is not a report-only review. Agents fix what they find.
2. **Sequential, not parallel** — Each agent sees prior agents' edits. Order matters.
3. **Be specific** — Every change needs evidence: a spec clause, a file:line, or concrete reasoning.
4. **Cross-reference, don't guess** — Agents must actually read spec files and source code.
5. **Check crate dependency order** — Implementation steps must respect: `ori_lexer → ori_parse → ori_ir → ori_types → ori_eval → ori_llvm → oric`.
6. **Clean up after yourself** — Agent 4 removes all `<!-- reviewed: ... -->` markers.
7. **Flag what can't be auto-fixed** — Architectural decisions and scope questions go in "Remaining Concerns" for human review.
8. **No deferral traps** — Flag any plan items that create temptation to defer during implementation. Items labeled "bonus", "future", "lower priority", or "requires architectural change" are red flags. Every checkbox in a section must be implementable by the agent executing the section. If an item genuinely cannot be implemented within the section's scope (missing language feature, external dependency), it should be marked `<!-- blocked-by:X -->` with a concrete blocker — not soft language that invites skipping. Agents should rewrite soft deferral language into concrete, actionable tasks or explicit blockers.
