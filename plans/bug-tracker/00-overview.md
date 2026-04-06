---
plan: "bug-tracker"
title: "Ori Compiler Bug Tracker"
status: in-progress
---

# Ori Compiler Bug Tracker

## Mission

Track known bugs discovered outside the scope of active plans — post-completion findings, cross-cutting issues, and bugs surfaced by TPR or code journeys that have no owning plan section. This is a permanent, always-open parallel plan that runs alongside the roadmap.

## How It Works

- **Organized by subsystem**, not chronologically — bugs go into the section matching the compiler component where the fix belongs
- **Minimal research at add-time** — capture repro, location, severity, and source; deep analysis happens at fix-time since surrounding code may change
- **Severity drives priority** — `critical` blocks work in that subsystem, `high` should be fixed when touching adjacent code, `medium` is opportunistic, `low` is tracked for dedicated passes
- **OBE checks** — bugs may be resolved by other work; `/review-bugs` checks for overtaken-by-events
- **Plan-section rigor at fix-time** — every bug fix goes through `/fix-bug`, which creates a fix section file with root cause analysis, TDD matrix, implementation plan, and completion checklist (including TPR + hygiene review)

## Bug Lifecycle

```
Discovery → /add-bug (minimal capture) → section file entry
                                              ↓
Review → /review-bugs (OBE check, triage, prioritize)
                                              ↓
Fix → /fix-bug (plan-section rigor)
  1. Investigation & root cause analysis
  2. Fix section file created: plans/bug-tracker/fix-BUG-XX-NNN.md
  3. TDD matrix — all tests written and verified failing
  4. Implementation — fix applied, tests pass unchanged
  5. Completion checklist — test-all, clippy, TPR, hygiene
                                              ↓
Resolved → section entry marked [x], fix section status → complete
```

## Bug Entry Format

```markdown
- [ ] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}** — found by {source}.
  Repro: {test file or minimal repro steps}
  Subsystem: {crate/file path}
  Found: {YYYY-MM-DD} | Source: {tpr-review | code-journey | manual | continue-roadmap}
```

When a fix section exists (created by `/fix-bug`):
```markdown
  Fix: `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md`
```

When resolved via `/fix-bug`:

```markdown
- [x] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}** — found by {source}.
  Resolved: Fixed on {YYYY-MM-DD}. {Brief explanation}.
  Fix: `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md`
```

When resolved as OBE (no fix section — fixed as side effect of other work):

```markdown
- [x] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}** — found by {source}.
  Resolved: OBE on {YYYY-MM-DD}. {What fixed it — commit, plan, or rewrite}.
```

## Fix Section Files

When a bug is picked up for fixing, `/fix-bug` creates a fix section file at `plans/bug-tracker/fix-BUG-{section}-{ordinal}.md`. This file provides plan-section rigor:

1. **Root Cause Analysis** — symptom, proximate cause, root cause, blast radius, affected files
2. **TDD Matrix** — exact failing case, edge cases, cross-type/pattern/feature coverage, semantic + negative pins
3. **Implementation** — fix approach with code examples
4. **Completion Checklist** — builds, tests, TPR, hygiene review

Fix sections are permanent records — they stay in the bug tracker even after the bug is resolved. They serve as:
- Documentation of why the fix was designed the way it was
- Proof that proper rigor was followed
- Reference for future bugs in the same area

See `plan-schema.md` § "Bug Fix Section Template" for the full template.

## Severity Levels

| Severity | Meaning | `/continue-roadmap` behavior |
|----------|---------|------------------------------|
| `critical` | Blocks correctness in the subsystem | Surfaced as blocker — must `/fix-bug` before new work in that subsystem |
| `high` | Should fix when touching adjacent code | Mentioned as "you might want to `/fix-bug` these first" |
| `medium` | Fix opportunistically | Listed for awareness |
| `low` | Tracked for dedicated passes | Informational only |

## Integration Points

- **`/add-bug`** — files new bugs here with minimal research
- **`/fix-bug`** — creates fix section file and drives the fix with plan-section rigor (TDD, TPR, hygiene)
- **`/review-bugs`** — triages open bugs, checks for OBE, audits fix rigor, prioritizes
- **`/continue-roadmap`** — checks for critical/high bugs in the subsystem being worked on; uses `/fix-bug` for bugs that must be fixed before continuing
- **`/review-work`** and **`/tpr-review`** — fall back to filing bugs here when no owning plan exists

## Quick Reference

| ID | Subsystem | File | Open Bugs |
|----|-----------|------|-----------|
| 01 | Parser & Lexer | `section-01-parser-lexer.md` | 0 |
| 02 | Type Checker | `section-02-typeck.md` | 0 |
| 03 | Evaluator | `section-03-eval.md` | 0 |
| 04 | Codegen & LLVM | `section-04-codegen-llvm.md` | 1 |
| 05 | Runtime & ARC | `section-05-runtime-arc.md` | 0 |
| 06 | Stdlib | `section-06-stdlib.md` | 0 |
| 07 | Tooling & CLI | `section-07-tooling-cli.md` | 1 |
| 08 | Spec & Docs | `section-08-spec-docs.md` | 0 |
