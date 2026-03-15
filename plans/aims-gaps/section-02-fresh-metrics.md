---
section: "02"
title: "Fresh Metrics and Baseline Regeneration"
status: not-started
goal: "Regenerate all AIMS metrics from fresh command runs and record reproducible outputs."
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Test and Diagnostic Runs"
    status: not-started
  - id: "02.2"
    title: "Metric Normalization"
    status: not-started
  - id: "02.3"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Fresh Metrics and Baseline Regeneration

**Status:** Not Started
**Goal:** Replace stale historical numbers with current measured values.

## 02.1 Test and Diagnostic Runs

**File(s):** `diagnostics/aims-baseline.sh`

- [ ] Run `cargo test -p ori_arc -- aims` and capture pass/fail totals.
- [ ] Run `cargo test -p ori_llvm -- aims_interactions` and capture pass/fail totals.
- [ ] Run `diagnostics/aims-baseline.sh` and record golden/spec/benchmark evidence counts.
- [ ] Recompute AIMS code/test LOC from current source tree.

## 02.2 Metric Normalization

**File(s):** `plans/aims/00-overview.md`, `plans/aims/section-11-integration-verification.md`

- [ ] Compare newly measured values against plan text values.
- [ ] Mark each metric as current, stale, or contradictory.
- [ ] Queue plan updates for every stale metric reference.

## 02.3 Completion Checklist

- [ ] All reported metrics are sourced from fresh runs.
- [ ] Command list and resulting numbers are reproducible.
- [ ] No final report metric depends on prior plan snapshots.
