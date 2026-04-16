# /add-bug — Sub-agent Workflow

**This file is read by the Sonnet sub-agent dispatched from `SKILL.md`.** Execute all 8 steps in order. This is a plan-doc-only workflow — no code changes, no compiler changes.

**You do NOT:**
- Edit `.rs`, `.ori`, or anything under `compiler/`, `library/`, `tests/`
- Run `git add` / `git commit` directly — commits go through `/commit-push`
- Deep-dive into code — minimal research only (the bug may change before fix time)

**You DO:**
- Read `plans/bug-tracker/section-{NN}-*.md` files (markdown reads)
- Append to bug-tracker section files (markdown writes)
- Run `scripts/intel-query.sh` for blast-radius (shell commands)
- Run quick grep/test to confirm the bug exists (shell commands)
- Invoke `/commit-push` via the Skill tool for any file changes

---

## Step 1: Determine Subsystem

Map the bug to one of the 8 subsystem sections:

| Section | Subsystem | Crates/Paths |
|---------|-----------|--------------|
| 01 | Parser & Lexer | `ori_parse`, `ori_lexer` |
| 02 | Type Checker | `ori_types` |
| 03 | Evaluator | `ori_eval`, `ori_patterns` |
| 04 | Codegen & LLVM | `ori_llvm`, `ori_arc` |
| 05 | Runtime & ARC | `ori_rt` |
| 06 | Stdlib | `library/std`, `ori_registry` |
| 07 | Tooling & CLI | `oric`, `ori_fmt`, `ori_diagnostic` |
| 08 | Spec & Docs | `docs/`, `.claude/`, `plans/` |

If unclear, use the file path or subsystem in the bug description. If it spans subsystems, file in the one where the **fix** belongs (not where the symptom appears).

## Step 2: Check for Duplicates

Before adding, scan the target section file for existing bugs that match:

```
Read plans/bug-tracker/section-{NN}-*.md
```

If a duplicate exists, note it to the caller instead of adding a new entry.

## Step 3: Assign ID and Severity

**ID format:** `BUG-{section}-{ordinal}` — ordinal is the next sequential number in that section (count existing bugs + 1).

**Severity:**
- `critical` — blocks correctness in the subsystem, data corruption, crash
- `high` — wrong output, silent failure, should fix when touching adjacent code
- `medium` — edge case failure, workaround exists, fix opportunistically
- `low` — cosmetic, minor inconvenience, tracked for dedicated passes

## Step 4: Minimal Research

Do just enough to write a useful bug entry. DO NOT deep-dive — the code may change before the fix:

1. Confirm the bug exists (quick grep or test run if trivial)
2. Identify the approximate location (crate + file, not exact line)
3. Note any obvious repro (existing test file, or 2-3 line Ori snippet)
4. Intelligence graph blast-radius check. Follow the canonical intel-summary injection protocol:

   @.claude/skills/query-intel/compose-intel-summary.md

   Per SSOT Step F — /add-bug uses `callers "<buggy function>" --repo ori` to assess blast radius and `file-symbols "<subsystem path>" --repo ori` to identify related code.

## Step 5: Write the Bug Entry

Append to the `## Open Bugs` section of the target file:

```markdown
- [ ] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}**
  Repro: {test file path or minimal repro steps}
  Subsystem: {crate/file path}
  Found: {YYYY-MM-DD} | Source: {source value from the canonical list below}
```

If a fix section already exists (from a prior `/fix-bug` that was interrupted), add a cross-ref:
```markdown
  Fix: `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md`
```

**Canonical source values** (use exactly one — this is the SSOT for bug provenance):
- `tpr-review` — found by `/tpr-review` dual-source review
- `code-journey` — found by `/code-journey`
- `manual` — found by the user or during manual work
- `continue-roadmap` — found while working on the roadmap
- `review-work` — found by `/review-work`
- `fix-bug` — found during an active `/fix-bug` workflow (Phase 1 investigation, Phase 3 TDD, Phase 4 test-all, Phase 5 TPR/hygiene)
- `fix-next-bug` — found during `/fix-next-bug` autopilot iteration
- `impl-hygiene-review` — found by `/impl-hygiene-review`
- `review-bugs` — found during `/review-bugs` triage
- `independent-review` — found by `/independent-review`
- `design-pattern-review` — found by `/design-pattern-review`

When filing a bug from a dual-source review, add reviewer provenance to the body (not the Source field): `Reviewer: codex`, `Reviewer: gemini`, or `Reviewers: codex + gemini (agreement)`.

## Step 6: Cross-Reference Check

Quick check: is there an active roadmap section or reroute plan touching this area?

```
Grep for the affected file/function in plans/roadmap/section-*.md and plans/*/section-*.md
```

If an active plan section covers this area, note it in the bug entry:
```markdown
  Note: Active work in roadmap section {NN} touches this area.
```

This is informational only — the bug still belongs in the bug-tracker (the plan may not cover this specific issue).

## Step 7: Confirm to Caller

Report what was filed:
```
Filed: [BUG-{section}-{ordinal}][{severity}] {title}
  Section: {section name} (plans/bug-tracker/section-{NN}-*.md)
  Cross-ref: {any active plan sections, or "none"}
```

## Step 8: Resume Prior Workflow — MANDATORY

**`/add-bug` is almost always invoked mid-task** (proactive filing during `/continue-roadmap`, `/tpr-review`, `/fix-bug`, etc.). After confirming the filing, **immediately resume the interrupted workflow.** Do NOT stop, wait for caller input, or present the filing as a standalone deliverable. The bug filing is a side-effect — the main task is still in progress.

The caller's interrupted workflow context was provided in the sub-agent prompt. Return to it after Step 7.
