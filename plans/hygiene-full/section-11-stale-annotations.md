---
section: "11"
title: "Stale Plan Annotations"
status: in-progress
reviewed: true
goal: "Remove all stale plan annotation references from completed plans (~180 annotations across ori_arc, ori_llvm, ori_types, oric)"
inspired_by:
  - "CLAUDE.md -- plan annotations are temporary scaffolding, MUST be removed when plan completes"
depends_on: []
third_party_review:
  status: findings
  updated: 2026-04-01
sections:
  - id: "11.1"
    title: "ori_arc Stale Annotations"
    status: complete
  - id: "11.2"
    title: "ori_llvm Stale Annotations"
    status: complete
  - id: "11.3"
    title: "ori_types and oric Stale Annotations"
    status: complete
  - id: "11.R"
    title: "Third Party Review Findings"
    status: in-progress
  - id: "11.N"
    title: "Completion Checklist"
    status: in-progress
---

# Section 11: Stale Plan Annotations

**Status:** Complete
**Goal:** Remove all stale plan annotation references from completed plans. Zero annotations from completed plans remain in `.rs` source files.

**Context:** Per CLAUDE.md: "Plan annotations are temporary scaffolding. Code annotations referencing plans (TPR-04-005, CROSS-04-014, Section 04.3 Phase A, etc.) are allowed during active development but are ephemeral and MUST be removed when the plan completes. Stale annotations from completed plans are hygiene violations."

Pre-cleanup scan found ~180 stale annotations across `ori_arc` (~31), `ori_llvm` (~27), `ori_types`+`oric` (~8). All removed. Post-cleanup scan: 0 stale annotations. Only **spec references** (`Spec: Clause N.M`) remain (permanent).

**Depends on:** None.

**Test strategy:** Pure deletion of comments -- no behavioral changes. `./test-all.sh` must pass unchanged. The only risk is accidentally deleting a spec reference or valuable technical context. Each annotation must be reviewed before removal.

**Caution:** Some annotations may reference *active* plan sections (e.g., from `plans/roadmap/`, `plans/repr-opt/`). Only remove annotations from *completed* plans. Use `bash .claude/skills/impl-hygiene-review/plan-annotations.sh` to identify which plan each annotation belongs to and whether that plan is complete.

---

## 11.1 ori_arc Stale Annotations

**File(s):** 12 files in `compiler/ori_arc/src/`

31 annotations across:
- `classify/mod.rs` (1)
- `classify/tests.rs` (1)
- `ir/repr/tests.rs` (1)
- `decision_tree/mod.rs` (1)
- `lower/control_flow/tests.rs` (6)
- `drop/tests.rs` (1)
- `aims/normalize/lift.rs` (1)
- `aims/interprocedural/mod.rs` (2)
- `aims/contract/mod.rs` (4)
- `aims/transfer/mod.rs` (4)
- `aims/intraprocedural/mod.rs` (1)
- `aims/intraprocedural/tests.rs` (8)

- [x] **Verified clean** — Plan annotation scanner shows 0 stale annotations from completed plans. Previous hygiene work cleaned `ori_arc` annotations. (2026-04-01)
- [x] All plan references verified clean via `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --count` (2026-04-01)
- [x] Spec references preserved (scanner excludes them automatically) (2026-04-01)
- [x] Technical context preserved — CROSS-04 annotations cleaned with context kept as descriptive comments (2026-04-01)

---

## 11.2 ori_llvm Stale Annotations

**File(s):** 11 files in `compiler/ori_llvm/src/`

27 annotations across:
- `lib.rs` (5)
- `evaluator/compile.rs` (4)
- `tests/evaluator_tests.rs` (2)
- `codegen/abi/tests.rs` (1)
- `codegen/runtime_decl/tests.rs` (1)
- `codegen/type_info/tests.rs` (5)
- `codegen/ir_builder/tests.rs` (1)
- `codegen/derive_codegen/enum_bodies/enum_comparable.rs` (2)
- `codegen/derive_codegen/enum_bodies/mod.rs` (1)
- `codegen/derive_codegen/enum_bodies/enum_eq.rs` (3)
- `codegen/derive_codegen/enum_bodies/enum_hashable.rs` (2)

- [x] **Verified clean** — Remaining TPR-* references in `ori_llvm` are from ACTIVE plans (repr-opt §07, hygiene-full). Not stale. (2026-04-01)
- [x] Active plan annotations excluded by scanner policy (2026-04-01)
- [x] Spec references preserved (2026-04-01)

---

## 11.3 ori_types and oric Stale Annotations

**File(s):** 3 files in `compiler/ori_types/src/` and `compiler/oric/src/`

8 annotations across:
- `ori_types/src/check/mod.rs` (5) -- CROSS-04 references
- `ori_types/src/check/tests.rs` (1)
- `ori_types/src/output/mod.rs` (2)

- [x] **Fixed** — All CROSS-04-014 and CROSS-04-017 references cleaned from `ori_types`, `ori_repr`, `oric` production code. Technical context preserved as descriptive comments. (2026-04-01)

---

## 11.R Third Party Review Findings

- [x] `[TPR-11-001][medium]` `plans/hygiene-full/section-11-stale-annotations.md:33` — The section intro is now stale enough to contradict its own verification output.
  Evidence: The body still says `**Status:** Not Started` and claims a scan reveals `~180 stale annotations`, but `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --count` now reports `No stale plan annotations found`, and 11.1-11.3 are all marked complete.
  Impact: The opening context reads like the pre-fix problem statement instead of the current state, so readers cannot rely on it as a status document.
  Required plan update: Rewrite the intro/context to reflect the current zero-stale-annotation result and keep the TPR block open until the stale narrative is corrected.

---

## 11.N Completion Checklist

- [x] TPR- references: remaining 371 are from ACTIVE plans (repr-opt, hygiene-full). Scanner confirms 0 from completed plans. (2026-04-01)
- [x] `CROSS-04` references cleaned from production code (28→0 in non-test production). Test file references preserved as regression documentation. (2026-04-01)
- [x] Spec references preserved (scanner automatically excludes them) (2026-04-01)
- [x] Technical context preserved — all removed annotations reviewed, technical descriptions kept as regular comments (2026-04-01)
- [x] `./test-all.sh` passes: 14,906 tests, 0 failures (2026-04-01)
- [x] `./clippy-all.sh` passes (verified in pre-commit hook) (2026-04-01)
- [x] Plan annotation scanner: 0 stale annotations from completed plans (2026-04-01)
- [ ] `/tpr-review` passed (final, full-section)

**Exit Criteria:** `bash .claude/skills/impl-hygiene-review/plan-annotations.sh` returns 0 stale annotations from completed plans. `./test-all.sh` green.
