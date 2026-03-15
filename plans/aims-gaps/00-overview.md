---
plan: "aims-gaps"
title: "AIMS Gaps: Exhaustive Closure Plan"
status: not-started
supersedes:
  - "plans/aims/"
references:
  - "plans/aims/00-overview.md"
  - "plans/aims/index.md"
  - "diagnostics/aims-baseline.sh"
---

# AIMS Gaps: Exhaustive Closure Plan

## Mission

Close the gap between AIMS plan claims and current code reality by reconciling status contradictions, regenerating fresh metrics, and eliminating all deferred work comments that violate ZERO DEFERRAL policy.

## Architecture

```text
plans/aims/* claims
        |
        v
code + tests + diagnostics (source of truth)
        |
        v
gap classification (tracked/untracked, severity)
        |
        v
targeted fixes + plan/doc sync
        |
        v
final verification + closure verdict
```

## Design Principles

- Evidence over declaration: completion status comes from executable verification, not checkbox density.
- Zero deferral compliance: every deferred/future/TODO item is treated as active remediation work.
- Plan-code parity: `plans/aims/` must not contradict current implementation behavior.

## Section Dependency Graph

```text
01 (status reconciliation)
  -> 02 (fresh metrics)
  -> 03 (deferred remediation)
  -> 04 (sync + final verification)
```

## Implementation Sequence

```text
Phase 1 - Reconcile claims
  - Section 01: Build contradiction matrix and completion verdict.

Phase 2 - Refresh evidence
  - Section 02: Re-run diagnostics/tests and capture current metrics.
  Gate: all reported numbers come from fresh command output.

Phase 3 - Eliminate deferred debt
  - Section 03: Convert every deferred/future comment into concrete fix work.
  Gate: no unresolved untracked deferred comments in AIMS code paths.

Phase 4 - Final sync and verify
  - Section 04: Update plans/aims status text and verify consistency.
  Gate: contradiction matrix empty, verification commands green.
```

## Metrics (Current State)

| Metric | Current value |
|--------|---------------|
| `cargo test -p ori_arc -- aims` | 495 passed |
| `cargo test -p ori_llvm -- aims_interactions` | 22 passed |
| AIMS code lines (`compiler/ori_arc/src/aims`) | 25,656 |
| AIMS test lines (`tests.rs`) | 13,914 |
| Baseline cross-dim evidence (golden/spec/bench) | 137 / 2 / 222 |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Plan Status and Evidence Reconciliation | `section-01-status-reconciliation.md` | Not Started |
| 02 | Fresh Metrics and Baseline Regeneration | `section-02-fresh-metrics.md` | Not Started |
| 03 | Deferred Work Elimination | `section-03-zero-deferral-remediation.md` | Not Started |
| 04 | Plan-Code Sync and Exit Verification | `section-04-sync-and-exit.md` | Not Started |
