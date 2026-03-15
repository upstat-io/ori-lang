---
plan: "aims-codegen-quality"
title: "AIMS Codegen Quality: All Journeys ≥ 9.8"
status: not-started
references:
  - "plans/code-journeys/overview.md"
  - "plans/aims/"
---

# AIMS Codegen Quality: All Journeys ≥ 9.8

## Mission

Bring all 13 code journey scores to 9.8/10 or higher (simple journeys to 10/10) on the AIMS branch before merging to master. Fix the two AIMS regressions (J5 closure leak, J10 lost `drop_unique`), close all attribute compliance gaps, eliminate control flow waste, and polish IR quality.

## Architecture

```
Journey Score Breakdown — Where Points Are Lost
═══════════════════════════════════════════════
         Instr  ARC   Attr   CF    IR    Bin   Other  Score
J1         10    10     8    10    10    10     10     9.8   ← uwtable only
J2         10    10     8     8    10    10     10     9.2   ← attr + CF
J3          9    10     6     7     9    10     10     8.9   ← attr + CF + IR
J4         10    10     7    10    10    10     10     9.7   ← attr only
J5          9    10     5     8     9    10      7     8.5   ← AIMS regression + attr
J6         10    10     7    10    10    10     10     9.7   ← attr only
J7          9    10     8     7     8    10     10     9.2   ← CF + IR + attr
J8         10    10     8    10    10    10     10     9.8   ← attr only
J9          9    10     6     7     8    10     10     8.8   ← attr + CF + IR
J10         9    10     5     7     8    10     10     8.7   ← AIMS regression + attr + CF
J11        10    10     7    10    10    10     10     9.7   ← attr only
J12        10    10     9     7    10    10     10     9.2   ← CF
J13        10    10     4    10    10    10     10     9.4   ← attr only

Fix Priorities:
  Section 01: AIMS regressions    → J5, J10 (blocks merge)
  Section 02: Attribute compliance → ALL journeys (biggest impact)
  Section 03: Control flow cleanup → J2, J3, J5, J7, J9, J10, J12
  Section 04: IR quality polish   → J3, J5, J7, J9, J10
  Section 05: Verification        → Re-run all 13, confirm ≥ 9.8
```

## Design Principles

1. **Fix at the source, not per-journey** — Every finding appears in multiple journeys. Fix the codegen infrastructure once, all journeys improve together.
2. **AIMS regressions first** — J5's potential memory leak and J10's lost optimization block the merge. These are non-negotiable.
3. **Attribute compliance is the #1 lever** — It's the lowest-scoring category across 10 of 13 journeys. A single infrastructure fix (e.g., adding `noundef` to struct params) lifts multiple journeys simultaneously.

## Section Dependency Graph

```
  01 AIMS Regressions  (independent — blocks merge)
  02 Attributes         (independent — biggest ROI)
  03 Control Flow       (independent)
  04 IR Quality         (depends on 01, 03 — re-run metrics after earlier fixes)
  05 Verification       (depends on 01-04)
```

Sections 01-03 are independent and can be worked in any order or in parallel. Section 04 depends on 01 and 03 (many unjustified instructions will be auto-fixed by EH block removal and empty block elimination). Section 05 is the final gate — re-run all journeys and confirm scores.

**Cross-section shared fixes:**
- **nounwind analysis** is shared between Section 01 (invoke → call conversion) and Section 02 (nounwind attribute compliance). The root cause is the same: the two-pass nounwind analysis in `nounwind.rs` + runtime function nounwind declarations in `codegen/runtime_decl/runtime_functions.rs`. Fix this infrastructure once, both sections benefit.
- **Empty block elimination** (Section 03) will auto-fix some IR quality deductions (Section 04) by removing instruction-count inflation from dead blocks.
- **Section 01.2 EH block removal** will auto-fix J5's unjustified instruction count (Section 04) by eliminating dead landingpad blocks.

## Implementation Sequence

```
Phase 1 — Critical (blocks merge)
  └─ 01.1: Fix J5 closure env RC dec (potential memory leak)
  └─ 01.2: Fix J5 unnecessary invoke/landingpad EH blocks
  └─ 01.3: Restore J10 drop_unique optimization
  Gate: J5 ≥ 9.0, J10 ≥ 9.0, no CRITICAL/HIGH findings

Phase 2 — High Impact (attribute compliance)
  └─ 02.1: Add noundef to struct/enum params
  └─ 02.2: Add uwtable to main wrapper
  └─ 02.3: Improve nounwind analysis
  └─ 02.4: Add memory(...) annotations
  Gate: All journeys attribute score ≥ 8/10

Phase 3 — Medium Impact (control flow + IR)
  └─ 03.1: Eliminate empty trampoline blocks
  └─ 03.2: Merge redundant entry blocks
  └─ 04.1: Unjustified instruction audit (re-run metrics after 01-03)
  └─ 04.2: Range materialization cleanup (J7)
  └─ 04.3: SSO branch reduction (J9)
  └─ 04.4: Parameter materialization cleanup (J10)
  Gate: All journeys CF score ≥ 9/10

Phase 4 — Verification
  └─ 05.1: Re-run all 13 journeys
  └─ 05.2: Confirm all ≥ 9.8 (simple ≥ 10.0)
  └─ 05.3: Run test-all.sh
  Gate: MERGE READY
```

## Metrics (Current State)

| Journey | Score | Target | Gap | Primary Blockers |
|---------|-------|--------|-----|-----------------|
| J1 | 9.8 | 10.0 | 0.2 | Attr (uwtable) |
| J2 | 9.2 | 9.8 | 0.6 | Attr + CF (empty blocks) |
| J3 | 8.9 | 9.8 | 0.9 | Attr (77.8%) + CF (4) + IR (1) |
| J4 | 9.7 | 10.0 | 0.3 | Attr (noundef) |
| J5 | 8.5 | 9.8 | 1.3 | AIMS regression + Attr (60%) |
| J6 | 9.7 | 10.0 | 0.3 | Attr (noundef) |
| J7 | 9.2 | 9.8 | 0.6 | CF (5) + IR (2) + Attr |
| J8 | 9.8 | 10.0 | 0.2 | Attr (noundef) |
| J9 | 8.8 | 9.8 | 1.0 | Attr (73.9%) + CF (4) + IR (4) |
| J10 | 8.7 | 9.8 | 1.1 | AIMS regression + Attr (66.7%) + CF |
| J11 | 9.7 | 10.0 | 0.3 | Attr (noundef) |
| J12 | 9.2 | 9.8 | 0.6 | CF (3) + IR (1) |
| J13 | 9.4 | 9.8 | 0.4 | Attr (52.6%) |
| **Avg** | **9.2** | **9.8+** | | |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 AIMS Regressions | ~150 | High | — |
| 02 Attribute Compliance | ~200 | Medium | — |
| 03 Control Flow Cleanup | ~100 | Medium | — |
| 04 IR Quality Polish | ~80 | Low | 01, 03 (metrics depend on earlier fixes) |
| 05 Verification | ~0 (run journeys) | Low | 01-04 |
| **Total new** | **~530** | | |

## Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| `nounwind` applied to function that CAN unwind → runtime crash | **Critical** | Conservative: only mark functions without any `invoke` instructions as nounwind. Verify with test suite + Valgrind. |
| `noundef` on aggregate param where a field could be poison | **Low** | Ori's type system guarantees all fields are initialized. Safe for all Ori-emitted types. |
| Post-emission block simplification breaks phi nodes | **Medium** | Only merge blocks with single predecessor. Preserve phi-node semantics. Add debug_assert on phi incoming counts. |
| `memory(none)` on function that actually reads memory → miscompile at O2+ | **High** | Conservative detection: only mark functions with zero `load`/`store`/`call` instructions. Run `./test-all.sh` at both O0 and release. |
| Impl method nounwind fix changes calling convention → ABI mismatch | **Medium** | Test with `diagnostics/dual-exec-verify.sh` which compares eval vs AOT output. |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | AIMS Regressions | `section-01-aims-regressions.md` | Not Started |
| 02 | Attribute Compliance | `section-02-attribute-compliance.md` | Not Started |
| 03 | Control Flow Cleanup | `section-03-control-flow.md` | Not Started |
| 04 | IR Quality Polish | `section-04-ir-quality.md` | Not Started |
| 05 | Verification | `section-05-verification.md` | Not Started |
