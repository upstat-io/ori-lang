---
name: review-work
description: Review actual implementation work, not just a plan. Use when the user asks to review work done across committed history, staged changes, unstaged changes, or a plan section; perform a deep investigation against `CLAUDE.md`, all repo rule files, and recently modified plans, then record validated findings in the owning plan section when one exists.
allowed-tools: Read, Grep, Glob, Bash, Task, Edit, Write
---

# Review Work Command

Review the implementation first. Treat git history, the index, the worktree, current files, `CLAUDE.md`, `.claude/rules/*.md`, and recent plan files as the evidence set. A plan is a coordination artifact, not the sole source of scope.

This command is for independent, adversarial review:
- Trust current files, fresh command output, and git objects.
- Distrust summaries, checklists, commit messages, and prior agent claims until verified.
- Review the real work, not the story about the work.

## Usage

```
/review-work [target]
```

`target` can be:
- A plan directory or section file (e.g., `plans/iter-rc-contract/`, `plans/iter-rc-contract/section-02-elem-dec-fn.md`)
- A section ID or keywords (e.g., `section-02`, `02`, `elem_dec_fn`)
- A git range or commit selector (e.g., `HEAD~5..HEAD`, `abc123..def456`, `last commit`)
- Uncommitted work selectors (`staged`, `unstaged`, `worktree`, `current branch`)
- Explicit files or directories

If no target is provided, it defaults to `HEAD~3..HEAD` plus any staged/unstaged changes.

---

## Workflow

### Step 1: Resolve Scope

Resolve the target in this order:
1. Existing path from the user.
2. Explicit git range or commit selector.
3. Explicit uncommitted-work selector (`staged`, `unstaged`, `worktree`, `current branch`).
4. Plan match from `plans/*/index.md`, `plans/*/00-overview.md`, and `plans/*/section-*.md`.
5. If nothing explicit was given, start with a recent committed slice plus any uncommitted work:
   - committed changes from `HEAD~3..HEAD`
   - staged changes from `git diff --cached`
   - unstaged changes from `git diff`

Broaden the scope if it's too narrow to be coherent (e.g., if it's just a fixup for previous commits).

### Step 2: Gather Evidence

#### 2.1 Git Evidence
Collect whichever apply:
- Committed diff stat and patch for the range.
- Commit log.
- Staged/unstaged diffs.
- `git status --short`.

#### 2.2 File Inventory
Identify all changed files, tests that should cover them, and adjacent code needed to understand behavior. **Read the full changed files, not just diff hunks.**

#### 2.3 Standards Packet
Read:
- `CLAUDE.md`
- `.claude/rules/tests.md`
- `.claude/rules/compiler.md`
- `.claude/rules/impl-hygiene.md`
- `.claude/rules/roadmap.md`
- Any other relevant files under `.claude/rules/*.md`.

#### 2.4 Plan Context
Gather recently modified plans to detect plan drift:
1. Check `plans/` for recent changes in git.
2. If a plan or section was named, read its `index.md`, `00-overview.md`, and target section.

### Step 3: Review Implementation

Perform an independent verification pass:
- Rerun key tests, scripts, and diagnostics.
- Use repo-native debugging/diagnostic scripts (e.g., in `diagnostics/`).
- Verify claims from current outputs and files.

Review for:
- Correctness bugs and regressions.
- Memory / RC / ownership issues (AIMS).
- Unsafe / FFI hazards.
- Missing or weak tests (Matrix coverage, Semantic pins).
- Spec drift.
- Rule violations (`CLAUDE.md`, `.claude/rules/*.md`).
- Hygiene problems (File size, ordering, naming).
- Plan / implementation drift.

### Step 4: Record Findings

1. Report findings to the user, ordered by severity.
2. If an owning plan section exists, record validated findings in that section's `Third Party Review Findings` block.

#### Finding Format:
```md
- [ ] `[TPR-{section}-{ordinal}][{severity}]` `file:line` — Short finding summary.
  Evidence: Explain the specific mismatch, regression, or missing case.
  Impact: Explain why the work is incomplete, unsafe, or non-compliant.
  Required plan update: State what must be validated and integrated.
```
Severities: `high`, `medium`, `low`.

### Step 5: Update Plan Metadata

If findings are added to a plan:
- Set section frontmatter `status: in-progress`.
- Set `third_party_review.status: findings`.
- Set `third_party_review.updated` to today's date.
- If the plan overview/index was marked complete/resolved, set it back to in-progress/active.

---

## Mandatory Standards Checks

Every review must explicitly test the work against:
- **TDD for Bugs**: Bug fixes must have matrix tests and semantic pins.
- **Fix Completeness**: No workarounds or hacks.
- **Hygiene**: Sibling `tests.rs`, file size < 500 lines, tracing instead of `println!`.
- **AIMS**: ARC/COW/FIP consistency.
- **Spec**: Adherence to `docs/ori_lang/v2026/spec/`.

## Output Pattern

1. List findings first, ordered by severity, with file references.
2. State the reviewed scope (commit range, diffs, major files).
3. State which standards were checked.
4. State whether a plan section was updated.
5. Mention any verification gaps.
