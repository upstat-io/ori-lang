# Create Plan Command

Create a new plan directory with index and section files using the standard template.

## Usage

```
/create-plan <name> [description]
```

- `name`: Directory name for the plan (kebab-case, e.g., `error-recovery`, `lsp-integration`)
- `description`: Optional one-line description of the plan's goal

## Workflow

### Step 0: Read CLAUDE.md (ABSOLUTE FIRST — NO EXCEPTIONS)

**Before doing ANYTHING else**, read the ENTIRE CLAUDE.md file — every single word, top to bottom:

```
Read file: CLAUDE.md
```

This is mandatory. Do not skip, skim, or partially read. The rules in CLAUDE.md govern ALL behavior in this command. Proceed to Step 1 only after reading the complete file.

### Step 1: Gather Information

If not provided via arguments, ask the user:

1. **Plan name** — kebab-case directory name
2. **Plan title** — Human-readable title (e.g., "Error Recovery System")
3. **Goal** — One-line description of what this plan accomplishes
4. **Sections** — List of major sections (at least 2-3)

Use AskUserQuestion if needed to clarify scope.

### Step 2: Read the Template

Read `plans/_template/plan.md` for the structure reference.

### Step 3: Load Hygiene Rules

The full rule set is embedded below (source of truth files — do not maintain separate copies). Use these rules when structuring plan sections to ensure plans account for registration sync points, file size limits, phase boundary discipline, and other hygiene requirements from the start.

**Hygiene Rules** (`.claude/rules/impl-hygiene.md`):
@.claude/rules/impl-hygiene.md

**Compiler Guidelines** (`.claude/rules/compiler.md`):
@.claude/rules/compiler.md

### Step 4: Create Directory Structure

Create the plan directory and files:

```
plans/{name}/
├── index.md           # Keyword index for discovery
├── 00-overview.md     # High-level goals and section summary
├── section-01-*.md    # First section
├── section-02-*.md    # Additional sections...
└── section-NN-*.md    # Final section
```

### Step 5: Generate index.md

Create the keyword index with:
- **Reroute frontmatter** (if this is a reroute plan — i.e., a parallel track alongside the main roadmap):
  ```yaml
  ---
  reroute: true
  name: "{Short Name}"
  full_name: "{Full Plan Name}"
  status: queued
  order: N
  ---
  ```
  The `name`, `full_name`, `status`, and `order` fields are the single source of truth for the website.
  `order` controls queue priority — lower value = promoted first (default 999 if omitted).
  `key` and `dir` are derived at load time from the directory name.
- Maintenance notice at the top
- How to use instructions
- Keyword cluster for each section (initially with placeholder keywords)
- Quick reference table

### Step 6: Generate 00-overview.md

Create overview with:
- Plan title and goal
- Section list with brief descriptions
- Dependencies (if any)
- Success criteria

### Step 7: Generate Section Files

For each section, create `section-{NN}-{name}.md` with:
- YAML frontmatter (section ID, title, status: not-started, goal, `reviewed`, `third_party_review: { status: none, updated: null }`)
- Section header with status emoji
- Placeholder subsections with `- [ ]` checkboxes
- `## {NN}.R Third Party Review Findings` block (empty, with `- None.`) before the completion checklist
- Completion checklist at the end

**`reviewed` field rules:**
- **Section 01**: `reviewed: true` — it is the starting point of implementation and was just reviewed during plan creation. Its assumptions are current.
- **All other sections (02+)**: `reviewed: false` — they have NOT been validated against actual implementation reality. As Section 01 is implemented, assumptions in later sections may become stale or wrong due to deviations, discoveries, and changed constraints. They must be re-reviewed before work begins on them.

See the "Reviewed Field Semantics" section below for the full rationale.

### Step 8: Report Progress

Show the user:
- Files created
- Note: "Running 4 independent review passes..."

### Step 9: Sequential Independent Review (4 Agents)

After the plan is fully created, run **4 review agents in sequence** (NOT parallel). Each agent:

- Receives **only the plan files** — no conversation context, no reasoning behind the plan
- Is instructed to **read the plan, review it, and edit the files directly** to fix issues
- Sees edits made by all previous agents (because they run sequentially)

This creates an iterative refinement pipeline: each reviewer builds on the last.

**IMPORTANT**: Run these agents ONE AT A TIME. Wait for each to complete before starting the next.

#### Agent 1: Technical Accuracy Review

Spawn an Agent with the following prompt (substitute `{plan_dir}` with the actual plan directory path):

```
You are reviewing a plan for the Ori compiler at {plan_dir}/.

CRITICAL PREREQUISITE: Before starting, read the ENTIRE CLAUDE.md file (every word):
Read file: CLAUDE.md

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
7. When reviewing TPR (Third Party Review) findings: you MUST NOT dismiss findings because they are "not related" to the current plan, "out of scope", or "pre-existing." Per CLAUDE.md there is no "unrelated" or "out of scope." If a finding identifies a real issue, it must be accepted. Only reject findings that are factually incorrect (the issue does not actually exist).

## `reviewed` field in frontmatter

Each section has a `reviewed: true/false` field. This is a NEW plan, so:
- Section 01 MUST be `reviewed: true` — it's the starting point, validated during creation
- ALL other sections MUST be `reviewed: false` — their assumptions are plans, not validated reality
- If any section is missing the `reviewed` field, add it with the correct value per these rules
- Do NOT mark later sections as `reviewed: true` even if they look accurate now — they haven't been tested against implementation reality yet

You may add missing sections, expand scope, or restructure if the plan is genuinely incomplete.
After editing, list what you changed and why.
```

#### Agent 2: Completeness & Gap Review

```
You are reviewing a plan for the Ori compiler at {plan_dir}/.

CRITICAL PREREQUISITE: Before starting, read the ENTIRE CLAUDE.md file (every word):
Read file: CLAUDE.md

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
8. When reviewing TPR (Third Party Review) findings: you MUST NOT dismiss findings because they are "not related" to the current plan, "out of scope", or "pre-existing." Per CLAUDE.md there is no "unrelated" or "out of scope." If a finding identifies a real issue, it must be accepted. Only reject findings that are factually incorrect (the issue does not actually exist).

You may add new sections, restructure, or expand scope if the plan has genuine gaps.
After editing, list what you changed and why.
```

#### Agent 3: Hygiene & Feasibility Review (Codebase-Aware)

```
You are reviewing a plan for the Ori compiler at {plan_dir}/.

CRITICAL PREREQUISITE: Before starting, read the ENTIRE CLAUDE.md file (every word):
Read file: CLAUDE.md

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
You are reviewing a plan for the Ori compiler at {plan_dir}/.

CRITICAL PREREQUISITE: Before starting, read the ENTIRE CLAUDE.md file (every word):
Read file: CLAUDE.md

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

After editing, list what you changed and why.
```

### Step 10: Report Summary

Show the user:
- Files created (with paths)
- Summary of what each review agent changed
- Next steps (fill in details, add keywords to index)

### Step 11: Ask About Reroute Status

After reporting the summary, use `AskUserQuestion` to ask the user whether this plan should be the active reroute. This determines the `reroute` frontmatter in `index.md`.

If the user says **yes**: add reroute frontmatter to `index.md` with `status: active` and `order: 1`.
If the user says **queued**: add reroute frontmatter with `status: queued` and ask for the `order` value.
If the user says **no**: do not add reroute frontmatter (plan is not a reroute).

---

## Example

**Input:** `/create-plan error-recovery "Improve compiler error messages and recovery"`

**Creates:**
```
plans/error-recovery/
├── index.md
├── 00-overview.md
├── section-01-error-types.md
├── section-02-recovery-strategies.md
└── section-03-user-facing-messages.md
```

---

## Section Naming Conventions

| Section Type | Naming Pattern |
|--------------|----------------|
| Setup/Infrastructure | `section-01-setup.md` |
| Core Implementation | `section-02-core.md` |
| Integration | `section-03-integration.md` |
| Testing | `section-04-testing.md` |
| Documentation | `section-05-docs.md` |

---

## Anti-Deferral Rule for Plan Items

**Every checklist item in a plan must be implementable by the agent executing that section.** When writing plan items:

- Do NOT use soft language that invites skipping: "bonus", "future", "lower priority", "nice to have", "if time permits", "stretch goal".
- Do NOT label items "requires architectural change" — architectural changes are implementation tasks, not deferrals. If a 30-line change across 3 files is needed, describe the change and make it a checkbox.
- Do NOT create items that are descriptions of work rather than work itself. "Investigate whether X" is acceptable; "Document the approach for Y" when Y can be implemented is not.
- If an item genuinely cannot be done within the section (blocked by an unimplemented language feature, needs user decision), use `<!-- blocked-by:X -->` with a concrete blocker reference — not vague language.
- Every item must pass this test: "Can the implementing agent, with access to the codebase, complete this item in a single session?" If no, break it into items that can.

## Reviewed Field Semantics

The `reviewed: true/false` field in section frontmatter is a **pre-implementation gate** — it tracks whether a section has been validated against the current codebase right before you start implementing it.

**Why this exists:** Plans are written with assumptions about how the code works. But as you implement Section 01, reality changes — deviations, discoveries, refactors, bug fixes. A section written before prior sections were implemented may reference stale file paths, wrong function signatures, or invalid approaches. `reviewed: false` means "not yet validated against implementation reality."

**Rules:**
- **Section 01** is always `reviewed: true` at creation — it's the starting point.
- **All other sections** are `reviewed: false` at creation — plans, not validated reality.
- **Single-section review** (`/review-plan plans/foo/section-03.md`): This is the pre-implementation gate. After confirming accuracy, flip to `reviewed: true`.
- **Whole-plan review** (`/review-plan plans/foo/`): Fixes issues, improves quality, but does NOT change `reviewed` values. You're improving the plan holistically, not gating specific sections.
- **`/continue-roadmap`** starting a `reviewed: false` section: triggers a single-section review first, which flips to `true` after validation.

---

## After Creation

Remind the user to:
1. Fill in section details with specific tasks
2. Add relevant keywords to `index.md` clusters
3. Update `00-overview.md` with dependencies and success criteria
4. **If performance-sensitive** (lexer, parser, typeck, eval, codegen): Add `/benchmark` checkpoints to relevant sections

## Performance-Sensitive Plans

For plans touching hot paths, include a "Performance Validation" section in `index.md`:

```markdown
## Performance Validation

Use `/benchmark short` after modifying hot paths.

**When to benchmark:** [list specific sections]
**Skip benchmarks for:** [list non-perf sections]
```

See `plans/_template/plan.md` for full guidance.

---

## Template Reference

The command uses `plans/_template/plan.md` as the structure reference. See that file for:
- Complete index.md template
- Section file template
- Status conventions
- The roadmap (`plans/roadmap/`) as a working example
