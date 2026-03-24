---
name: review-plan
description: Review an entire plan as one cohesive implementation strategy. Use when the user asks to review a plan directory, a plan file, or a section as part of its owning plan; perform a deep investigation against `CLAUDE.md`, all repo rule files, the current codebase, relevant specs, and recent plan context, then edit the plan files directly to fix validated issues while leaving every `reviewed` frontmatter value unchanged.
---

# Review Plan

Review the plan as an implementation contract. Treat the current codebase, git history, the
worktree, `CLAUDE.md`, `.claude/rules/*.md`, relevant spec docs, and all files in the target plan
as the evidence set. The goal is to review the entire plan in one go and improve it until it is
technically accurate, executable, cohesive, and complete.

This skill is for independent, adversarial review:

- trust current files, fresh command output, and git objects
- distrust summaries, checklists, status metadata, and prior agent claims until verified
- review the real codebase and the real plan, not the story about them

## Scope Inputs

Accept any of these:

- a plan directory:
  - `plans/iter-rc-contract/`
- a plan file inside the target plan:
  - `plans/iter-rc-contract/index.md`
  - `plans/iter-rc-contract/00-overview.md`
  - `plans/iter-rc-contract/section-02-elem-dec-fn.md`
- a section id or keywords that map to a plan:
  - `section-02`
  - `02`
  - `elem_dec_fn`
- a plan name or keywords:
  - `iter-rc-contract`
  - `iter rc contract`

If the user gives a specific section file or section id, treat it as an entry point to the owning
plan. The review still covers the full plan directory, not only that section.

## Scope Resolution

Resolve the target in this order:

1. Existing path from the user.
2. Plan match from `plans/*/index.md`, `plans/*/00-overview.md`, and `plans/*/section-*.md`.
3. If the user named a section file or section id, resolve its parent plan directory and review
   the entire plan.
4. If nothing explicit was given, start with the most relevant recent local plan context:
   - `git diff --name-only HEAD -- plans/`
   - `git diff --name-only --cached -- plans/`
   - `git diff --name-only -- plans/`
   - recent local commits touching `plans/`
5. If multiple candidate plans remain after step 4, ask one concise question to narrow it.

The scope is the whole owning plan:

- `index.md`
- `00-overview.md`
- every `section-*.md` file in the plan directory

Do not perform a section-only review with this skill.

## Required Inputs To Gather

Build the review packet mechanically before forming conclusions.

### 1. Plan Inventory

Read the full target plan:

- `index.md`
- `00-overview.md`
- all `section-*.md` files

Record:

- section ordering and dependencies
- stated mission and success criteria
- status metadata and completion claims
- existing `Third Party Review Findings` blocks
- sections that look thin, vague, or stale

### 2. Evidence Packet

Cross-check the plan against the current repository state.

Collect whichever apply:

- `git status --short`
- recent local commits touching the target plan
- recent local commits touching code paths the plan claims it will modify
- current files, modules, tests, and specs named by the plan
- current staged and unstaged changes that materially affect the plan's claims

From the plan, identify:

- all referenced files, crates, modules, functions, and types
- the tests that should validate each code-modifying section
- adjacent callers, callees, helpers, data definitions, and registration points needed to verify
  feasibility

Read the full referenced files when they are central to the plan's claims, not only search hits.
Expand into neighboring files when needed to verify invariants and downstream impact.

### 3. Standards Packet

The review is not complete until you have checked the plan against the repository standards.

Always read:

- `CLAUDE.md`
- `.claude/rules/tests.md`
- `.claude/rules/compiler.md`
- `.claude/rules/impl-hygiene.md`
- `.claude/rules/roadmap.md`

Also read every file under `.claude/rules/*.md` before finalizing conclusions. Prioritize rules
that match the plan's touched domains first, but the final review must account for the full rule
set, marking non-applicable rules as such in your own reasoning rather than silently skipping them.

### 4. Spec And Plan Context

Gather the surrounding context needed to verify the plan's claims and detect drift:

1. relevant spec files the plan cites or depends on
2. recent plan files changed in the target plan
3. neighboring plans or roadmap entries explicitly referenced by the target plan
4. related plan files touched in recent local history when they change assumptions the target plan
   relies on

Use this context to answer:

- whether the plan's mission still matches the current roadmap/spec intent
- whether each section still matches the current codebase
- whether cross-plan or cross-section dependencies are reflected accurately
- whether completion claims still match the repository state
- whether existing third-party findings were integrated, silently deferred, or contradicted

## Review Workflow

1. Resolve the owning plan and read the entire plan directory in full.
2. Gather the evidence packet, standards packet, and surrounding spec/plan context.
3. Cross-reference every technical claim in the plan against the current codebase, rules, specs,
   and recent repository history.
4. Reconstruct the plan's mission, execution order, and intended end state from the plan files
   themselves.
5. Perform an independent verification pass:
   - rerun key tests, scripts, and diagnostics named by the plan when feasible
   - use repo-native debugging or diagnostic workflows before line-by-line speculation when the
     rule set says to do so
   - verify claims from current outputs, current files, and actual git objects
   - if a cited verification step cannot be reproduced, record that as a verification gap and fix
     the plan if it made unsupported claims
6. Review with a plan-review mindset:
   - technical inaccuracies
   - infeasible implementation steps
   - missing prerequisites or hidden dependencies
   - broken section sequencing or circular dependencies
   - scope gaps where the mission is only partially covered
   - vague or non-verifiable checklist items
   - missing tests, missing matrix coverage, or missing semantic pins
   - rule violations or hygiene violations implied by the plan
   - plan / implementation drift
   - stale or mishandled `Third Party Review Findings`
   - partial work presented as complete
7. Edit the plan files directly to fix validated issues:
   - correct inaccurate paths, types, functions, and module references
   - expand thin sections until they are executable
   - add missing prerequisites, test steps, sync points, and cross-section dependencies
   - reorder sections or checklist items when the current sequence is invalid
   - add new sections when the stated mission is not fully covered
8. Preserve every existing `reviewed` frontmatter value exactly as found. Whole-plan review does
   not flip, normalize, or rewrite `reviewed`, even when a section needed corrections.
9. Report findings and substantive edits to the user first, ordered by severity or importance.

## Deep Investigation Standard

This is not a plan skim.

- Read the whole plan, not only the file the user named.
- Read enough referenced code to verify that the plan's claims are real and the approach can work.
- Trace dependencies across section, module, crate, and phase boundaries.
- Inspect both tests the plan already names and tests the plan should name but does not.
- Use history and current tree state together to catch stale assumptions hidden by old status text.
- Prefer diagnostics, tracing, and repo-native verification tools over guesswork.

When a plan touches ARC, AOT, lowering, runtime, tests, spec, or roadmap-owned areas, assume the
failure surface is wider than a single section and expand the review accordingly.

## Mandatory Standards Checks

Every review must explicitly test the plan against these expectations from `CLAUDE.md` and the
ruleset:

- the plan fulfills its stated mission completely rather than stopping at a partial milestone
- sections can be executed sequentially without hidden prerequisites or circular dependencies
- each code-modifying section includes concrete tests, matrix coverage, and at least one semantic
  pin when the changed path is shared
- TDD ordering, debug verification, and release verification are spelled out when the rules require
  them
- checklist items are concrete, actionable, and specific enough to implement without guessing
- plan boundaries and cross-section dependencies are updated when the work crosses ownership lines
- no silent workarounds, dummy values, or hand-wavy "future work" exemptions
- touched Rust areas still respect hygiene expectations:
  - sibling `tests.rs`
  - file-size / split expectations
  - tracing instead of `println!`
  - no dead code, unjustified lint suppression, or hidden invariants
- domain-specific rules under `.claude/rules/*.md` are satisfied for the relevant subsystems
- `Third Party Review Findings` are rejected only when factually incorrect, not because they seem
  inconvenient or out of scope
- every `reviewed` frontmatter value remains unchanged during whole-plan review

If the plan violates `CLAUDE.md` or a rule file, that is a review problem even if the plan sounds
plausible.

## Verification Standard

`review-plan` is an independent review workflow, not a transcription workflow.

- Never rely on prior agent summaries, checklist state, or status metadata as proof.
- Prefer rerunning the exact verification commands named in the plan when feasible.
- If a cheaper command proves the same fact, say so explicitly.
- If tests or scripts are infeasible in the current environment, state the blocker and keep the
  plan honest about the residual risk.
- Distinguish clearly between:
  - fresh verification
  - direct file inspection
  - git-history evidence
  - inference

## Plan Edit Rules

Use the target plan files as the authoritative artifact. Do not create a separate review document
unless the user explicitly asks for one.

When editing the plan:

- preserve existing findings history
- preserve every existing `reviewed` frontmatter value exactly as found
- update `index.md`, `00-overview.md`, and affected `section-*.md` files together when structure,
  sequencing, or ownership changes
- fix inaccuracies directly in the plan instead of only describing them externally
- expand scope when needed to satisfy the mission; do not scope down to make the plan easier
- remove soft deferral language like "future work", "nice to have", or "bonus" unless there is a
  real blocker that must be made explicit
- keep `Third Party Review Findings` blocks aligned with the current plan state when the reviewed
  content changes around them
- do not mark a section, overview, or index complete/resolved merely because the review improved
  wording

If a section is stale but currently marked `reviewed: true`, fix the section content and call out
the stale assumption in your response if it matters, but do not change the `reviewed` field.

## Output Pattern

When responding after a review:

1. List findings or substantive plan corrections first, ordered by severity or importance, with
   file references.
2. State the reviewed scope:
   - target plan
   - plan files reviewed
   - major code/spec areas cross-checked
3. State which standards were checked:
   - `CLAUDE.md`
   - relevant `.claude/rules/*.md`
   - relevant spec and plan files
4. State whether the plan files were edited.
5. If no problems were found, say so explicitly and mention any verification gaps.
