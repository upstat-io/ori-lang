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

## Bug Entry Format

```markdown
- [ ] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}** — found by {source}.
  Repro: {test file or minimal repro steps}
  Subsystem: {crate/file path}
  Found: {YYYY-MM-DD} | Source: {tpr-review | code-journey | manual | continue-roadmap}
```

When resolved:

```markdown
- [x] `[BUG-{section}-{ordinal}][{severity}]` **{Short title}** — found by {source}.
  Resolved: {Fixed | OBE} on {YYYY-MM-DD}. {Brief explanation}.
```

## Severity Levels

| Severity | Meaning | `/continue-roadmap` behavior |
|----------|---------|------------------------------|
| `critical` | Blocks correctness in the subsystem | Surfaced as blocker — must fix before new work in that subsystem |
| `high` | Should fix when touching adjacent code | Mentioned as "you might want to address these first" |
| `medium` | Fix opportunistically | Listed for awareness |
| `low` | Tracked for dedicated passes | Informational only |

## Integration Points

- **`/add-bug`** — files new bugs here with minimal research
- **`/review-bugs`** — triages open bugs, checks for OBE, prioritizes
- **`/continue-roadmap`** — checks for critical/high bugs in the subsystem being worked on
- **`/review-work`** and **`/tpr-review`** — fall back to filing bugs here when no owning plan exists

## Quick Reference

| ID | Subsystem | File | Open Bugs |
|----|-----------|------|-----------|
| 01 | Parser & Lexer | `section-01-parser-lexer.md` | 0 |
| 02 | Type Checker | `section-02-typeck.md` | 0 |
| 03 | Evaluator | `section-03-eval.md` | 0 |
| 04 | Codegen & LLVM | `section-04-codegen-llvm.md` | 2 |
| 05 | Runtime & ARC | `section-05-runtime-arc.md` | 0 |
| 06 | Stdlib | `section-06-stdlib.md` | 0 |
| 07 | Tooling & CLI | `section-07-tooling-cli.md` | 1 |
| 08 | Spec & Docs | `section-08-spec-docs.md` | 0 |
