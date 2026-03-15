---
plan: "aims-gaps"
title: "AIMS Gaps: Exhaustive Closure Plan"
status: complete
audits:
  - "plans/aims/"
references:
  - "plans/aims/00-overview.md"
  - "plans/aims/index.md"
  - "diagnostics/aims-baseline.sh"
  - "diagnostics/aims-compare.sh"
  - "diagnostics/aims-measure.sh"
---

# AIMS Gaps: Exhaustive Closure Plan

## Mission

Close the gap between AIMS plan claims and current code reality by: (1) reconciling status contradictions across plan files, (2) regenerating fresh metrics from actual command output, and (3) rewording all deferred-style comments to comply with ZERO DEFERRAL policy.

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

- **Evidence over declaration:** Completion status comes from executable verification (test runs, grep counts), not checkbox density.
- **Zero deferral compliance:** Every "deferred"/"future work" comment is either reworded to reflect its true status (language dependency, optimization opportunity) or converted to an immediate fix.
- **Plan-code parity:** `plans/aims/` text must not contradict current implementation behavior as verified by tests and code inspection.

## Known Status Contradictions (pre-audit)

These are verified contradictions between `plans/aims/` files as of 2026-03-15:

1. **Index vs overview frontmatter**: `plans/aims/index.md` says `status: resolved`; `plans/aims/00-overview.md` says `status: in-progress`.
2. **Overview body vs index**: The overview Implementation Sections table (line 380-392) marks all 13 sections "Complete"; the index Quick Reference table marks sections 08, 11, 13 as "In Progress".
3. **Overview body internal**: §4 Realization Status describes open issues for Verification ("Partially Realized") and Normalization ("Bug 2 partial"), but §6 Remaining Work marks most items as resolved with strikethrough. Section 13 Bug 2 (full contract refresh) is listed as still open.
4. **Section 13 Bug 2 stale in overview**: Overview §6 item 2 says Bug 2 is open ("Full contract refresh... requires SCC peer data threading"), but Section 13's own checklist (line 1092-1099) confirms it was fully resolved on 2026-03-15 via `run_second_pass()` re-running `extract_contract()`. The code (`pipeline/aims_pipeline.rs:426-472`) confirms full contract refresh is implemented.
5. **Overview §4 Normalization row stale**: Says "Full contract refresh deferred (requires SCC peer data threading)" as Open Issue — contradicted by Section 13 completion and code reality.
6. **Overview §6 item 7 stale**: Says `borrowed_rooted_vars` "traces aliases through `Let { dst, value: Var(src) }` instructions only" — code (`emit_function.rs:307-316`) now also handles Jump terminator block-param passing via fixpoint loop.
7. **Section 13 summary stale**: Section 13 lines 57-59 say "Bug 2 partial fix" as a remaining gap, but the section's own checklist (line 1092) says "[x] Full contract refresh after TRMC rewrite (Bug 2 complete fix)."
8. **Test count inconsistency**: Overview line 183 says "56 ARC unit tests" for normalize; lines 128 and 226 say "52". Actual count is 52 (#[test] in normalize/tests.rs). Overview §6 item 3 says "64 realize" tests; actual count is 65 (#[test] in realize/tests.rs).
9. **Index keyword sections stale**: Section 08 status says "In Progress (08.5a: H7 Valgrind gap, ARC unit tests not H-specific)" but section-08 frontmatter says `status: complete`. Section 11 says "In Progress (.fold() bug, SynergyMetrics)" — both resolved (overview §6 items 4, 5). Section 13 says "In Progress (Bug 2 contract refresh, borrowed_rooted_vars)" — both resolved.

## Section Dependency Graph

```text
01 (status reconciliation)
  ├─> 02 (fresh metrics)        [depends on: 01]
  ├─> 03 (comment remediation)  [depends on: 01, 02]
  └─> 04 (sync + final)         [depends on: 01, 02, 03]
```

**Note:** Section 03 depends on 02 because deferred-item classification needs
fresh metrics to determine whether a "stale metric" deferred item is actually
resolved. Sections 02 and 03 cannot fully run in parallel.

## Implementation Sequence

```text
Phase 1 - Reconcile claims
  - Section 01: Build contradiction matrix and completion verdict.

Phase 2 - Refresh evidence
  - Section 02: Re-run diagnostics/tests and capture current metrics.
  Gate: all reported numbers come from fresh command output.

Phase 3 - Remediate deferred comments
  - Section 03: Classify and reword every deferred/future comment for ZERO DEFERRAL compliance.
  Gate: no unresolved untracked deferred comments in AIMS code paths.

Phase 4 - Final sync and verify
  - Section 04: Update plans/aims status text and verify consistency.
  Gate: contradiction matrix empty, verification commands green.
```

**Ordering notes:**
- Section 03 comment rewords (03.3 items 1-3) are pure documentation changes
  that do NOT depend on Section 02 fresh metrics. However, 03.2 classification
  does depend on 02 to determine if stale-metric deferred items are resolved.
  Partial parallelism is possible: 03.3 comment rewords can start with 01.
- Section 04 is strictly serial — it depends on all prior sections being complete.

**Complexity warnings:**
- Section 04 has the most file:line edits (~25 individual corrections across
  4 plan files). Risk of merge conflicts if plans/aims/ files are modified
  by other work during this plan's execution.
- Section 02 is mechanical (run commands, record output) but `diagnostics/aims-baseline.sh`
  may need updating if its output format has drifted since last use.

## Metrics (as of 2026-03-15)

| Metric | Current value | Notes |
|--------|---------------|-------|
| `cargo test -p ori_arc --lib -- aims` | 495 passed | Filter subset; 986 total ori_arc tests |
| `cargo test -p ori_llvm aims_interactions` | 22 passed | Matrix H interaction tests |
| AIMS code lines (`compiler/ori_arc/src/aims`, excl. tests) | 11,702 | Non-test `.rs` files (42 files) |
| AIMS total lines (`compiler/ori_arc/src/aims`, incl. tests) | 25,604 | All `.rs` files |
| AIMS test lines (`tests.rs` files only) | 13,902 | Sibling `tests.rs` files |
| Normalize tests (`normalize/tests.rs`) | 52 | #[test] count |
| Realize tests (`realize/tests.rs`) | 65 | #[test] count |
| TRMC AOT tests (`ori_llvm/tests/aot/trmc.rs`) | 12 | #[test] count |
| AIMS interaction tests (`aims_interactions.rs`) | 22 | #[test] count |
| Synergy Ori programs (`tests/aims/synergy/`) | 8 | .ori files |
| Baseline cross-dim evidence (golden/spec/bench) | 137 / 2 / 222 | Stale; re-measure in Section 02 via `diagnostics/aims-baseline.sh` |

## Codebase Hygiene Findings (pre-scan)

Files touched by this plan were scanned against `impl-hygiene.md` rules.
30 files scanned; findings below are woven into relevant sections.

### BLOAT (files over 500 lines, excluding tests)

Files in `compiler/ori_arc/src/aims/` (plan scope):
- `intraprocedural/state_map.rs` — 646 lines
- `normalize/rewrite.rs` — 569 lines
- `normalize/verify.rs` — 559 lines
- `lattice/mod.rs` — 552 lines
- `intraprocedural/block.rs` — 536 lines
- `interprocedural/extract.rs` — 517 lines
- `transfer/mod.rs` — 512 lines
- `interprocedural/mod.rs` — 507 lines
- `pipeline/aims_pipeline.rs` — 553 lines

Files in `compiler/ori_llvm/src/codegen/arc_emitter/` (scope-adjacent):
- `emit_function.rs` — 506 lines
- `builtins/collections/mod.rs` — 528 lines

These are NOT blocking for this plan (which is a documentation/metrics plan), but
implementers touching these files for Section 03 comment rewords should note the
500-line limit. See individual section cleanup items.

### STYLE (`#[allow(` should be `#[expect(`)

In `arc_emitter/` (scope-adjacent, not AIMS core):
- `builtins/prelude.rs:20,30` — `#[allow(dead_code)]` should be `#[expect(dead_code)]`
- `mod.rs:84` — `#[allow(dead_code)]` should be `#[expect(dead_code)]`
- `context.rs:185` — `#[allow(dead_code)]` should be `#[expect(dead_code)]`
- `builtins/mod.rs:54,106,160,166,234,238` — multiple `#[allow(dead_code)]`

These are in `arc_emitter/` which is scope-adjacent (Section 03 mentions it).
Not blocking, but should be fixed if those files are modified.

### STYLE (bare TODO without plan reference)

- `arc_emitter/operators/mod.rs:76` — `// TODO(typeck): register missing methods...` — no plan/roadmap reference

### Clean Areas

- `compiler/ori_arc/src/aims/` — ZERO `TODO`/`FIXME`/`HACK`/`XXX`, ZERO `#[allow(` (confirmed)
- `compiler/ori_arc/src/aims/` — ZERO decorative banners (`// ===`, `// ---`)
- `compiler/ori_arc/src/pipeline/aims_pipeline.rs` — ZERO `#[allow(`, ZERO TODO/FIXME

**Summary:** 11 BLOAT, 8 STYLE (`#[allow(`), 1 STYLE (bare TODO). All findings are
in scope-adjacent files (arc_emitter), not AIMS core. AIMS core code is notably clean.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Plan Status and Evidence Reconciliation | `section-01-status-reconciliation.md` | Not Started |
| 02 | Fresh Metrics and Baseline Regeneration | `section-02-fresh-metrics.md` | Not Started |
| 03 | Deferred Comment Remediation | `section-03-zero-deferral-remediation.md` | Not Started |
| 04 | Plan-Code Sync and Exit Verification | `section-04-sync-and-exit.md` | Not Started |
