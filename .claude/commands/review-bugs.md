---
name: review-bugs
description: Review open bugs in the bug-tracker plan — check for OBE, prioritize, decide what to fix, and verify that completed fixes followed plan-section rigor.
allowed-tools: Read, Grep, Glob, Bash, Edit, Write, AskUserQuestion, Skill
argument-hint: "[subsystem or 'all']"
---

# Review Bugs

Review open bugs in `plans/bug-tracker/`, check for OBE (Overtaken By Events), verify fix rigor on resolved bugs, and prioritize what to fix next.

## Usage

```
/review-bugs [target]
```

- No args: review all subsystems
- `01` or `parser`: review Parser & Lexer bugs only
- `all`: review everything
- `critical`: review only critical/high bugs across all subsystems

## Workflow

### Step 1: Gather Open Bugs

Read each section file (or the targeted one) and collect all `- [ ]` items:

```
plans/bug-tracker/section-01-parser-lexer.md
plans/bug-tracker/section-02-typeck.md
plans/bug-tracker/section-03-eval.md
plans/bug-tracker/section-04-codegen-llvm.md
plans/bug-tracker/section-05-runtime-arc.md
plans/bug-tracker/section-06-stdlib.md
plans/bug-tracker/section-07-tooling-cli.md
plans/bug-tracker/section-08-spec-docs.md
```

Also check for any existing fix section files:
```
plans/bug-tracker/fix-BUG-*.md
```

### Step 2: OBE Check

For each open bug, check if it's been overtaken by events:

1. **Grep for the repro file** — does the test now pass?
   ```bash
   timeout 30 cargo test {test_name} 2>&1 | tail -5
   ```
   Or if it's an Ori test:
   ```bash
   timeout 30 cargo run -- test {test_file} 2>&1 | tail -5
   ```

2. **Check if the affected code was rewritten** — has the file/function been significantly changed since the bug was filed?
   ```bash
   git log --oneline --since="{found_date}" -- {subsystem_path} | head -10
   ```

3. **Check if a recent plan fixed it** — grep completed plans for the bug area:
   ```bash
   grep -r "{keyword}" plans/completed/ | head -5
   ```

If the bug is OBE, mark it resolved:
```markdown
- [x] `[BUG-{section}-{ordinal}][{severity}]` **{title}** — found by {source}.
  Resolved: OBE on {YYYY-MM-DD}. {What fixed it — commit, plan, or rewrite}.
```

### Step 3: Validate Remaining Bugs

For bugs that aren't OBE, do a quick sanity check:
- Is the severity still accurate? (code changes may have made it worse or better)
- Is the subsystem assignment still correct? (code may have moved)
- Is the repro still valid?

Update entries if needed.

### Step 4: Audit Recently Resolved Bugs for Fix Rigor

For each bug marked `- [x]` since the last review, verify that the fix followed plan-section rigor:

1. **Check for fix section file** — does `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md` exist?
   - If YES: verify the completion checklist is fully checked off (especially TPR and hygiene review)
   - If NO: flag as **rigor gap** — the fix landed without plan-section discipline

2. **Check for test coverage** — does the resolution mention specific tests?
   - Semantic pin test present? (test that only passes with correct semantics)
   - Negative pin test present? (test that rejects old/broken behavior)
   - If resolution says "OBE" this is acceptable — OBE bugs were fixed as a side effect of other work

3. **Check for TPR/hygiene** — was the fix reviewed?
   - Critical/High bugs: TPR + hygiene review is MANDATORY
   - Medium bugs: TPR is expected, hygiene review recommended
   - Low bugs: neither required but documented if done

**Rigor gap handling:**
- Bugs fixed without a fix section BEFORE this workflow was established: note as legacy, no action needed
- Bugs fixed without a fix section AFTER this workflow: flag to user — the fix may be incomplete.
  Ask: "BUG-XX-NNN was resolved without a fix section. Should we retroactively verify the fix quality, or accept it as-is?"

### Step 5: Present Summary

```
## Bug Tracker Review — {date}

### Summary
- Total open: {N}
- Checked for OBE: {N}
- Resolved (OBE): {N}
- Still open: {N}
- Fix sections in progress: {N} (list IDs)

### By Severity
- Critical: {N} {list titles}
- High: {N} {list titles}
- Medium: {N}
- Low: {N}

### By Subsystem
| Subsystem | Open | Critical | High | In-Progress Fixes |
|-----------|------|----------|------|-------------------|
| Parser & Lexer | {N} | {N} | {N} | {N} |
| ... | | | | |

### OBE Resolutions
{List of bugs resolved as OBE with brief explanation}

### Fix Rigor Audit
- Bugs with fix sections (complete): {N}
- Bugs with fix sections (in-progress): {N}
- Bugs resolved without fix sections: {N} (legacy: {N}, gap: {N})
- TPR coverage on critical/high fixes: {N}/{total}

### Recommended Actions
{Prioritized list of bugs worth fixing now, considering:
 - Critical bugs block work
 - High bugs in areas with active roadmap sections
 - Clusters of bugs in the same file/function (fix together via single fix section)
 - In-progress fix sections that should be completed
}
```

### Step 6: Ask What to Do

Use AskUserQuestion with options:
1. **Fix a specific bug** — pick one to work on now (will invoke `/fix-bug`)
2. **Fix all critical bugs** — work through critical bugs in priority order using `/fix-bug` for each
3. **Resume an in-progress fix** — continue a fix section that was started but not completed
4. **Done reviewing** — no action needed right now

### Step 7: Dispatch to /fix-bug

When the user selects a bug to fix, invoke `/fix-bug BUG-XX-NNN` to begin the fix with full plan-section rigor. Do NOT start fixing the bug ad-hoc — the `/fix-bug` workflow ensures:
- A fix section file is created BEFORE any code changes
- TDD matrix tests are written and verified failing BEFORE the fix
- The completion checklist is followed (including TPR + hygiene review)
- The fix section provides a permanent record of investigation and verification

**MANDATORY:** Every bug selected for fixing goes through `/fix-bug`. No exceptions, no "this one is simple enough to fix inline." The rigor is the process.
