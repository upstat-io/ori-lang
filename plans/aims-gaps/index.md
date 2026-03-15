# AIMS Gaps Index

> **Maintenance Notice:** Update this index whenever `aims-gaps` sections change.
> **Scope:** Closure plan for AIMS plan/code drift, deferred work, and verification deltas.

## How to Use

1. Search this file for a gap keyword.
2. Open the mapped section file.
3. Execute checklist items in order.

---

## Keyword Clusters by Section

### Section 01: Plan Status and Evidence Reconciliation
**File:** `section-01-status-reconciliation.md` | **Status:** Not Started

```text
status drift, in-progress vs complete, index mismatch, frontmatter mismatch
evidence claim audit, stale claims, contradiction matrix, completion verdict
plans/aims/index.md, plans/aims/00-overview.md, section-08, section-11, section-13
```

---

### Section 02: Fresh Metrics and Baseline Regeneration
**File:** `section-02-fresh-metrics.md` | **Status:** Not Started

```text
fresh metrics, diagnostics/aims-baseline.sh, current test counts, line counts
golden corpus, spec corpus, benchmarks, cross-dimension evidence
cargo test -p ori_arc -- aims, cargo test -p ori_llvm -- aims_interactions
```

---

### Section 03: Deferred Work Elimination
**File:** `section-03-zero-deferral-remediation.md` | **Status:** Not Started

```text
ZERO DEFERRAL, TODO/FIXME, deferred, future work, pending design decision
effect purity gate, MaybeShared cross-block reuse, layered lookup clone
dead_code expectations, untracked debt, tracked vs untracked classification
```

---

### Section 04: Plan-Code Sync and Exit Verification
**File:** `section-04-sync-and-exit.md` | **Status:** Not Started

```text
sync plans/aims with code reality, remove stale statements, enforce closure
verification gates, lsp diagnostics, build/test pass, no deferred comments left
final report, completion criteria, contradiction-free status matrix
```

---

## Quick Reference

| ID | Title | File |
|----|-------|------|
| 01 | Plan Status and Evidence Reconciliation | `section-01-status-reconciliation.md` |
| 02 | Fresh Metrics and Baseline Regeneration | `section-02-fresh-metrics.md` |
| 03 | Deferred Work Elimination | `section-03-zero-deferral-remediation.md` |
| 04 | Plan-Code Sync and Exit Verification | `section-04-sync-and-exit.md` |
