---
reroute: true
name: "AIMS Gaps"
full_name: "AIMS Gaps — Plan/Code Drift Closure"
status: resolved
order: 1
---

# AIMS Gaps Index

> **Maintenance Notice:** Update this index whenever `aims-gaps` sections change.
> **Scope:** Closure plan for AIMS plan/code drift, deferred-comment remediation, and verification deltas.

## How to Use

1. Search this file for a gap keyword.
2. Open the mapped section file.
3. Execute checklist items in order within each section; sections must run in dependency order (01 -> 02 -> 03 -> 04).

---

## Keyword Clusters by Section

### Section 01: Plan Status and Evidence Reconciliation
**File:** `section-01-status-reconciliation.md` | **Status:** Complete

```text
status drift, in-progress vs complete, index mismatch, frontmatter mismatch
evidence claim audit, stale claims, contradiction matrix, completion verdict
plans/aims/index.md, plans/aims/00-overview.md, section-08, section-11, section-13
Bug 2 stale, borrowed_rooted_vars stale, test count 52 vs 56, 64 vs 65 realize
Module Tree missing context.rs, 12888 vs 986 test count, EffectPurityViolation
```

---

### Section 02: Fresh Metrics and Baseline Regeneration
**File:** `section-02-fresh-metrics.md` | **Status:** Complete

```text
fresh metrics, diagnostics/aims-baseline.sh, diagnostics/aims-compare.sh, diagnostics/aims-measure.sh
test counts, LOC counts, golden corpus, spec corpus, benchmarks, cross-dimension evidence
cargo test -p ori_arc, cargo test -p ori_llvm aims_interactions, per-module test counts
normalize 52, realize 65, TRMC AOT 12, AIMS interactions 22, synergy 8
```

---

### Section 03: Deferred Comment Remediation
**File:** `section-03-zero-deferral-remediation.md` | **Status:** Complete

```text
ZERO DEFERRAL, deferred comment, future work, pending design decision
effect purity gate, MaybeShared cross-block reuse, LayeredMap clone cost
EffectPurityViolation expect(dead_code), tracked vs untracked classification
code identifier vs comment distinction, PendingRc, terminator_deferred
3 root causes, language dependency, optimization opportunity, comment reword
```

---

### Section 04: Plan-Code Sync and Exit Verification
**File:** `section-04-sync-and-exit.md` | **Status:** Complete

```text
sync plans/aims with code reality, remove stale statements, enforce closure
verification gates, clippy-all.sh, test-all.sh, cargo check
final report, completion criteria, contradiction-free status matrix
index.md corrections, overview.md corrections, section-13 corrections, section-08 corrections
frontmatter status alignment, Module Tree context.rs, test count normalization
```

---

## Hygiene Review

**Reviewed:** 2026-03-15 | **Rules:** `.claude/rules/impl-hygiene.md`, `.claude/rules/compiler.md`
**Files scanned:** 30 (AIMS core, pipeline, arc_emitter) | **Files with findings:** 20 (11 BLOAT, 8 STYLE, 1 bare TODO)
**AIMS core cleanliness:** Excellent — ZERO TODO/FIXME/HACK, ZERO `#[allow(`, ZERO decorative banners.
**Findings location:** All findings in scope-adjacent files (`arc_emitter/`), not AIMS core.
**Details:** See `00-overview.md` Codebase Hygiene Findings and per-section Cleanup sub-headings.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Plan Status and Evidence Reconciliation | `section-01-status-reconciliation.md` | Complete |
| 02 | Fresh Metrics and Baseline Regeneration | `section-02-fresh-metrics.md` | Complete |
| 03 | Deferred Comment Remediation | `section-03-zero-deferral-remediation.md` | Complete |
| 04 | Plan-Code Sync and Exit Verification | `section-04-sync-and-exit.md` | Complete |
