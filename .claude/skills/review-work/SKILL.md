---
name: review-work
description: Review actual implementation work, not just a plan. Use when the user asks to review work done by Claude or another agent across committed history, staged changes, unstaged changes, or a plan section; perform a deep investigation against `CLAUDE.md`, all repo rule files, and recently modified plans, then record validated findings in the owning plan section when one exists.
---

# Review Work

Review the implementation first. Treat git history, the index, the worktree, current files,
`CLAUDE.md`, `.claude/rules/*.md`, and recent plan files as the evidence set. A plan is a
coordination artifact, not the sole source of scope.

This skill is for independent, adversarial review:

- trust current files, fresh command output, and git objects
- distrust summaries, checklists, commit messages, and prior agent claims until verified
- review the real work, not the story about the work

## Scope Inputs

Accept any of these:

- a plan directory or section file:
  - `plans/iter-rc-contract/`
  - `plans/iter-rc-contract/section-02-elem-dec-fn.md`
- a section id or keywords:
  - `section-02`
  - `02`
  - `elem_dec_fn`
- a git range or commit selector:
  - `HEAD~5..HEAD`
  - `abc123..def456`
  - `last commit`
  - `last 3 commits`
  - `abc123`
- uncommitted work selectors:
  - `staged`
  - `unstaged`
  - `worktree`
  - `current branch`
- explicit files or directories:
  - `compiler/ori_arc/src/lower/control_flow/`
  - `compiler/ori_llvm/tests/aot/fat_ptr_iter.rs`

## Scope Resolution

Resolve the target in this order:

1. Existing path from the user.
2. Explicit git range or commit selector.
3. Explicit uncommitted-work selector (`staged`, `unstaged`, `worktree`, `current branch`).
4. Plan match from `plans/*/index.md`, `plans/*/00-overview.md`, and `plans/*/section-*.md`.
5. If nothing explicit was given, start with a recent committed slice plus any uncommitted work:
   - committed changes from `HEAD~3..HEAD` (or the available local history when fewer than 3
     commits exist)
   - staged changes from `git diff --cached`
   - unstaged changes from `git diff`
6. If that initial slice is too narrow to review coherently, broaden it before continuing. Expand
   stepwise to `HEAD~5..HEAD`, then `HEAD~10..HEAD`, then `git merge-base master HEAD..HEAD`
   until the scope includes the full implementation story for the touched code.

Treat the initial 3-commit slice as too narrow when any of these are true:

- the commits are an obvious follow-up or fixup for code mostly changed just before the slice
- tests in the slice depend on behavior introduced just before the slice
- the owning plan section or nearby plan edits describe work spanning more than the slice
- the slice contains partial reverts, cleanup-only commits, or verification-only commits that do
  not show the primary behavior change
- file or module history shows the relevant implementation landing immediately before the slice

If even the broadened branch-local scope is huge and still not coherent enough to review well, ask
one concise question to narrow it. Otherwise proceed.

Commits and diffs are scope selectors, not content filters. Once a file or module is in scope,
review it completely enough to judge correctness, tests, rule compliance, and plan drift.

## Required Inputs To Gather

Build the review packet mechanically before forming conclusions.

### 1. Git Evidence

Collect whichever apply:

- committed diff stat for the review range
- committed patch for the review range
- commit log for the review range
- staged diff: `git diff --cached`
- unstaged diff: `git diff`
- current status: `git status --short`

If reviewing committed work, inspect both:

- the cumulative diff for the whole range
- the individual commits in the range, to catch partial reverts, contradictory edits, or tests
  added after code changes

If reviewing staged or unstaged work, compare the right layers:

- staged work against `HEAD`
- unstaged work against the index

### 2. File Inventory

From the git evidence, identify:

- all changed files
- the tests that should cover them
- adjacent callers, callees, helpers, and data definitions needed to understand behavior

Read the full changed files, not only the diff hunks. Expand into neighboring files when needed
to verify invariants and downstream impact.

### 3. Standards Packet

The review is not complete until you have checked the work against the repository standards.

Always read:

- `CLAUDE.md`
- `.claude/rules/tests.md`
- `.claude/rules/compiler.md`
- `.claude/rules/impl-hygiene.md`
- `.claude/rules/roadmap.md`

Also read every file under `.claude/rules/*.md` before finalizing findings. Prioritize rules that
match the changed paths or domain first, but the final review must account for the full rule set,
marking non-applicable rules as such in your own reasoning rather than silently skipping them.

### 4. Plan Context

Gather recently modified plans so the review can detect plan drift, missing updates, and active
known work:

1. `git diff --name-only HEAD -- plans/`
2. `git diff --name-only --cached -- plans/`
3. plan files changed inside the committed review range
4. if the review range is omitted, plan files changed in the last few local commits touching
   `plans/`
5. if the user named a plan or section, include that plan's `index.md`, `00-overview.md`, target
   section, and any explicitly referenced neighboring sections

Read the discovered plan files directly. If a section is extremely large, read its overview/index
plus the specific headings tied to the changed work.

Use plan context to answer:

- which section claims ownership of the code being changed
- whether cross-section edits were reflected in related plans
- whether completion claims still match the repository state
- whether discovered problems were already planned, partially addressed, or silently deferred

## Review Workflow

1. Resolve the concrete work scope from git, paths, and plan hints.
2. Gather the git evidence, standards packet, and recent plan context.
3. Read the changed files in full. Then read the surrounding files needed to understand the real
   behavior.
4. Reconstruct what the work is trying to do from code, tests, diffs, plans, and commit history.
5. Perform an independent verification pass:
   - rerun the key tests, scripts, and diagnostics when feasible
   - use the repo's required debugging/diagnostic scripts before line-by-line speculation when
     the rule set says to do so
   - verify claims from current outputs, current files, and actual git objects
   - if a cited verification step cannot be reproduced, record that as a review finding or
     verification gap
6. Review with a code-review mindset:
   - correctness bugs
   - regressions
   - memory / RC / ownership issues
   - unsafe / FFI hazards
   - missing or weak tests
   - spec drift
   - rule violations
   - hygiene problems
   - plan / implementation drift
   - partial work presented as complete
7. Compare committed, staged, and unstaged layers together when multiple layers exist:
   - does staged work fix or worsen committed issues?
   - does unstaged work partially revert staged or committed work?
   - are tests/docs/plans updated in the same layer that changes behavior?
8. Report findings to the user first, ordered by severity, with file references and concrete
   evidence.
9. If an owning plan section exists and the user asked for plan updates or the existing workflow
   clearly expects them, record the validated findings in that section's `Third Party Review
   Findings` block.

## Deep Investigation Standard

This is not a diff skim.

- Read whole changed files.
- Read enough neighboring code to understand invariants, ownership, and boundary contracts.
- Trace data flow across function, module, and phase boundaries touched by the work.
- Inspect both tests that changed and tests that should have changed but did not.
- Use commit-by-commit history to catch accidental regressions hidden by the final tree.
- Prefer diagnostics, tracing, and repo-native verification tools over guesswork.

When a change touches ARC, AOT, lowering, runtime, tests, spec, or roadmap-owned areas, assume
the failure surface is wider than the diff and expand the review accordingly.

## Mandatory Standards Checks

Every review must explicitly test the work against these expectations from `CLAUDE.md` and the
ruleset:

- bugs are fixed completely, not deferred behind comments or vague follow-up
- tests come with the fix, and bug fixes have matrix coverage plus at least one semantic pin when
  the changed path is shared
- debug and release verification requirements are respected when the rules require them
- plan boundaries are updated when a fix crosses section boundaries
- no silent workarounds, dummy values, or hand-wavy "pre-existing" exemptions
- touched Rust files respect hygiene expectations:
  - test placement in sibling `tests.rs`
  - file-size / split expectations
  - tracing instead of `println!`
  - no dead code, unjustified lint suppression, or hidden invariants
- domain-specific rules under `.claude/rules/*.md` are satisfied for the relevant subsystems

If the work violates `CLAUDE.md` or a rule file, that is a review finding even if the code
"works."

## Verification Standard

`review-work` is an independent review workflow, not a transcription workflow.

- Never rely on prior agent summaries, plan checklist state, or commit messages as proof.
- Prefer rerunning the exact verification commands named in the plan or commit discussion.
- If a cheaper command proves the same fact, say so explicitly.
- If tests or scripts are infeasible in the current environment, state the blocker and record the
  residual risk.
- Distinguish clearly between:
  - fresh verification
  - direct file inspection
  - git-history evidence
  - inference

## Plan Update Rules

Use the owning section file as the authoritative findings store when one can be identified.
Do not create a separate review document unless the user explicitly asks for one.

When a plan owner exists:

- preserve existing findings history
- identify the owning plan's `00-overview.md` and `index.md` alongside the section file
- add frontmatter if missing:

```yaml
third_party_review:
  status: none
  updated: null
```

- add the reserved heading if missing, immediately before the completion checklist:

```md
## {NN}.R Third Party Review Findings

- None.
```

When open findings exist:

- set section frontmatter `status: in-progress`
- if the owning plan `00-overview.md` is marked `complete`, set it back to `in-progress`
- if the owning plan `index.md` is marked `resolved` or otherwise indicates the plan is done,
  set it back to `active`
- set `third_party_review.status: findings`
- set `third_party_review.updated` to today's date
- append new unchecked items under `Third Party Review Findings`
- replace `- None.` if present
- update the plan overview/status metadata in the same edit pass as the section findings so the
  TPR remains discoverable to downstream readers

When no new findings exist:

- do not manufacture plan edits
- leave status alone unless the file is obviously stale and the user asked you to correct it

If no owning plan section can be identified:

- do not guess
- present findings to the user without editing plans unless the user explicitly wants a new plan
  entry or review note created

## Finding Format

Use stable IDs when writing plan findings:

```md
- [ ] `[TPR-02-001][high]` `compiler/oric/src/foo.rs:123` — Short finding summary.
  Evidence: Explain the specific mismatch, regression, or missing case.
  Impact: Explain why the work is incomplete, unsafe, or non-compliant.
  Required plan update: State what must be validated and integrated.
```

Rules:

- use `TPR-{section}-{ordinal}` when the section number is known
- use severity tags: `high`, `medium`, or `low`
- keep one issue per item
- include test gaps when they materially affect correctness
- cite the concrete file/layer involved:
  - committed range
  - staged diff
  - unstaged diff
  - current file state

If a prior finding is no longer valid, do not delete it. Mark it resolved:

```md
- [x] `[TPR-02-002][medium]` `path/to/file.rs:45` — Summary.
  Resolved: Rejected after validation on 2026-03-18. Reason: ...
```

## Review Boundaries

Do not:

- accept "done" claims because a checklist is checked off
- treat commit messages as proof that the implementation is correct
- ignore staged or unstaged deltas when they materially change the reviewed work
- flag speculative issues without evidence
- convert findings straight into completed implementation checklist items
- mark a section `complete` while unchecked third-party findings remain
- hide rule violations because they look stylistic

Do:

- review committed, staged, and unstaged work together when relevant
- call out mismatches between branch history and current tree state
- surface `CLAUDE.md` and rule-file violations explicitly
- annotate when a finding is already covered by a recent plan
- mention residual risk when verification was blocked
- keep findings sharp enough for a later implementation pass to act on directly
- reopen completed plan metadata when new third-party findings are added so the findings are not
  hidden behind a completed/resolved plan state

## Output Pattern

When responding after a review:

1. List findings first, ordered by severity, with file references.
2. State the reviewed scope:
   - commit range
   - staged and/or unstaged inclusion
   - major files/modules reviewed
3. State which standards were checked:
   - `CLAUDE.md`
   - relevant `.claude/rules/*.md`
   - recent plan files
4. State whether a plan section was updated.
5. If no findings were found, say so explicitly and mention any verification gaps.
