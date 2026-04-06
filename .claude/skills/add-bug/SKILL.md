---
name: add-bug
description: Add a bug to the bug-tracker plan. Minimal research at add-time — capture repro, location, severity, and source. TRIGGER proactively when ANY bug is encountered during ANY work — unrelated bugs, edge cases, test failures, suspicious behavior, code smells that look like bugs. If in doubt, file it. Better safe than sorry — verification happens at review time.
allowed-tools: Read, Grep, Glob, Edit, Write, Bash
argument-hint: "[description or file:line]"
---

# Add Bug

File a bug in `plans/bug-tracker/` under the correct subsystem section.

## Proactive Triggering — MANDATORY

This skill MUST be invoked proactively whenever you encounter a bug that is **not part of your current task**. Do NOT:
- Gloss over it as "not related"
- Note it mentally and move on
- Say "this is a separate issue" without filing
- Assume someone else will catch it
- Skip it because you're "in the middle of something"

**If in doubt, file it.** Verification happens when bugs are reviewed (`/review-bugs`). A false positive costs nothing; a missed bug costs everything.

### When to trigger (non-exhaustive)
- You see a test failure unrelated to your current work
- You notice suspicious behavior while reading code
- A code journey or exploration reveals unexpected output
- You encounter an edge case that probably doesn't work
- You find a TODO/FIXME/HACK comment that describes an unfixed bug
- A compiler error message is wrong or misleading
- You notice a mismatch between spec and implementation
- Any test is `#skip`-ped and the reason looks fixable

## Usage

```
/add-bug [description]
```

The description can be:
- A free-text bug description: `/add-bug tuple field .0.1 fails because lexer tokenizes 0.1 as float`
- A file reference: `/add-bug compiler/ori_eval/src/iterator/next.rs:45 — double iteration on break`
- Context from the current conversation (no args needed if a bug was just discussed)

## Workflow

### Step 1: Determine Subsystem

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

If unclear, check the file path or ask. If it spans subsystems, file in the one where the **fix** belongs (not where the symptom appears).

### Step 2: Check for Duplicates

Before adding, scan the target section file for existing bugs that match:

```
Read plans/bug-tracker/section-{NN}-*.md
```

If a duplicate exists, note it to the user instead of adding a new entry.

### Step 3: Assign ID and Severity

**ID format:** `BUG-{section}-{ordinal}` — ordinal is the next sequential number in that section (count existing bugs + 1).

**Severity:**
- `critical` — blocks correctness in the subsystem, data corruption, crash
- `high` — wrong output, silent failure, should fix when touching adjacent code
- `medium` — edge case failure, workaround exists, fix opportunistically
- `low` — cosmetic, minor inconvenience, tracked for dedicated passes

### Step 4: Minimal Research

Do just enough to write a useful bug entry. DO NOT deep-dive — the code may change before the fix:

1. Confirm the bug exists (quick grep or test run if trivial)
2. Identify the approximate location (crate + file, not exact line)
3. Note any obvious repro (existing test file, or 2-3 line Ori snippet)

### Step 5: Write the Bug Entry

Append to the `## Open Bugs` section of the target file:

```markdown
- [ ] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}** — found by {source}.
  Repro: {test file path or minimal repro steps}
  Subsystem: {crate/file path}
  Found: {YYYY-MM-DD} | Source: {tpr-review | code-journey | manual | continue-roadmap | review-work}
```

If a fix section already exists (from a prior `/fix-bug` that was interrupted), add a cross-ref:
```markdown
  Fix: `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md`
```

**Source values:**
- `tpr-review` — found by Codex TPR
- `code-journey` — found by /code-journey
- `manual` — found by the user or during manual work
- `continue-roadmap` — found while working on the roadmap
- `review-work` — found by /review-work

### Step 6: Cross-Reference Check

Quick check: is there an active roadmap section or reroute plan touching this area?

```
Grep for the affected file/function in plans/roadmap/section-*.md and plans/*/section-*.md
```

If an active plan section covers this area, note it in the bug entry:
```markdown
  Note: Active work in roadmap section {NN} touches this area.
```

This is informational only — the bug still belongs in the bug-tracker (the plan may not cover this specific issue).

### Step 7: Confirm to User

Report what was filed:
```
Filed: [BUG-{section}-{ordinal}][{severity}] {title}
  Section: {section name} (plans/bug-tracker/section-{NN}-*.md)
  Cross-ref: {any active plan sections, or "none"}
```

## Fix Workflow — What Happens Next

Filing a bug is capture only. When a bug is picked up for fixing (via `/review-bugs`, `/continue-roadmap`, or direct request), the **`/fix-bug`** command enforces plan-section rigor:

1. **Investigation** — root cause analysis, spec consultation, reference compiler review
2. **Fix section file** — `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md` created with full plan-section structure
3. **TDD matrix** — all tests written and verified failing BEFORE implementation
4. **Implementation** — fix applied, tests pass unchanged
5. **Completion checklist** — test-all, clippy-all, TPR review, impl-hygiene review

**Every bug fix gets this rigor.** No ad-hoc fixes. The fix section file is the permanent record of investigation, approach, and verification — it stays in the bug tracker alongside the section files.

See `/fix-bug` for the full workflow and fix section template.
