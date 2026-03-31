---
plan: "hygiene-full"
title: "Full Project Implementation Hygiene: Exhaustive Implementation Plan"
status: not-started
references:
  - ".claude/rules/impl-hygiene.md"
  - ".claude/rules/registry.md"
  - ".claude/rules/compiler.md"
---

# Full Project Implementation Hygiene: Exhaustive Implementation Plan

## Mission

Eliminate all implementation hygiene violations discovered during a full-project review: LEAKs (scattered knowledge, duplicated dispatch), DRIFTs (sync point gaps), GAPs (missing contracts), and BLOATs (oversized files, missing docs). The goal is to enforce single source of truth (SSOT) through `ori_registry`, extract shared algorithms between eval and LLVM backends, replace magic numbers with named constants, add missing cross-phase invariant contracts, and clean up surface hygiene issues -- producing a codebase where every piece of knowledge has exactly one canonical home.

## Architecture

```
                    ori_registry (SSOT)
                    ┌──────────────────┐
                    │ TypeDef + OpDefs │ ◄── Sections 01, 02, 09
                    │ MethodDef        │
                    │ OpStrategy       │
                    └──────┬───────────┘
                           │ query
          ┌────────────────┼────────────────┐
          ▼                ▼                ▼
   ori_types          ori_eval          ori_llvm
   (typeck)           (interp)          (codegen)
   §01: op dispatch   §03: shared       §03: shared
   §02: trait sat.    algorithms         algorithms
   §08: contracts     §06: internal DRY  §06: internal DRY
   §10: cleanup       §04: constants     §04: constants
                                         §05: layout

   ori_arc             ori_rt            ori_repr
   §05: layout         §07: RC protocol  §05: layout
   §08: contracts      §04: constants    §10: cleanup
   §11: annotations

   Cross-cutting: §09 registration sync, §11 stale annotations, §12 surface hygiene
```

## Design Principles

1. **SSOT through the registry.** `ori_registry` defines what operators and methods each type supports. The type checker, evaluator, and codegen should *query* the registry rather than maintaining parallel dispatch tables. This is the single most impactful change -- it eliminates ~10 LEAK findings and prevents future drift.

2. **Algorithmic DRY via shared metadata.** When eval and LLVM have the same control-flow skeleton (e.g., Option/Result routing, equals/compare dispatch), extract the *metadata* into shared data structures and let each backend use its own emission strategy on top. This eliminates ~7 algorithmic duplication findings without creating inappropriate coupling.

3. **Named constants eliminate magic numbers.** Tag values (Some=0, None=1), field indices (len=0, cap=1, data=2), and hash constants (FNV basis/prime) appear as bare literals in 20+ files. Named constants in canonical locations make invariants explicit and prevent silent drift.

## Section Dependency Graph

```
Independent (can be worked in any order):
  01 ──┐
  02 ──┤
  04 ──┤  (pure additions, no behavioral changes)
  07 ──┤
  08 ──┤
  11 ──┘

After 01+02:
  09 ── (sync enforcement builds on registry SSOT)
  10 ── (cleanup uses registry queries)

After 04:
  05 ── (layout unification is independent but benefits from shared constants)
  06 ── (LLVM DRY references shared constants)

After 03 depends on understanding 01+02:
  03 ── (cross-backend DRY needs registry alignment first)

After all:
  12 ── (surface hygiene is last, touches many files)
```

Sections 01, 02, 04, 07, 08, 11 are independent and can be worked in any order.
Section 03 benefits from 01+02 being complete (registry alignment first).
Sections 05, 06 benefit from 04 (constants defined first).
Sections 09, 10 benefit from 01+02 (registry SSOT established first).
Section 12 is last -- surface cleanup after all architectural changes land.

## Implementation Sequence

```
Phase 1 - Foundation (independent, any order)
  └─ 04: Named constants for tags, field indices, FNV
  └─ 07: Runtime RC protocol DRY + immortal check
  └─ 08: Cross-phase invariant contracts
  └─ 11: Stale plan annotation removal

Phase 2 - Registry SSOT
  └─ 01: Registry SSOT for operator dispatch
  └─ 02: Registry SSOT for methods & traits
  Gate: ori_types queries ori_registry for operator/trait decisions

Phase 3 - DRY extraction
  └─ 03: Cross-backend algorithmic DRY
  └─ 05: Layout computation unification
  └─ 06: LLVM internal algorithmic DRY

Phase 4 - Enforcement & cleanup
  └─ 09: Registration sync & enforcement
  └─ 10: Scattered knowledge cleanup
  └─ 12: Surface hygiene
  Gate: ./test-all.sh and ./clippy-all.sh clean
```

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Depends On |
|---------|-------------------|------------|------------|
| 01 Registry SSOT (Operators) | ~400 | Medium | -- |
| 02 Registry SSOT (Methods & Traits) | ~300 | Medium | -- |
| 03 Cross-Backend DRY | ~500 | High | 01, 02 |
| 04 Named Constants | ~200 | Low | -- |
| 05 Layout Unification | ~150 | Medium | -- |
| 06 LLVM Internal DRY | ~400 | Medium | 04 |
| 07 Runtime RC Protocol | ~150 | Medium | -- |
| 08 Invariant Contracts | ~100 | Low | -- |
| 09 Registration Sync | ~200 | Low | 01, 02 |
| 10 Scattered Knowledge | ~300 | Medium | 01, 02 |
| 11 Stale Annotations | ~180 deletions | Low | -- |
| 12 Surface Hygiene | ~400 | Low | all |
| **Total** | **~3280** | | |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Registry SSOT (Operator Dispatch) | `section-01-registry-operator-dispatch.md` | Not Started |
| 02 | Registry SSOT (Methods & Traits) | `section-02-registry-methods-traits.md` | Not Started |
| 03 | Cross-Backend DRY (eval / LLVM) | `section-03-cross-backend-dry.md` | Not Started |
| 04 | Named Constants | `section-04-named-constants.md` | Not Started |
| 05 | Layout Computation Unification | `section-05-layout-unification.md` | Not Started |
| 06 | LLVM Internal DRY | `section-06-llvm-internal-dry.md` | Not Started |
| 07 | Runtime RC Protocol DRY + Correctness | `section-07-runtime-rc-protocol.md` | Not Started |
| 08 | Cross-Phase Invariant Contracts | `section-08-invariant-contracts.md` | Not Started |
| 09 | Registration Sync & Enforcement | `section-09-registration-sync.md` | Not Started |
| 10 | Scattered Knowledge Cleanup | `section-10-scattered-knowledge.md` | Not Started |
| 11 | Stale Plan Annotations | `section-11-stale-annotations.md` | Not Started |
| 12 | Surface Hygiene | `section-12-surface-hygiene.md` | Not Started |

## Cleanup

After all sections are complete:
- Run `timeout 150 ./test-all.sh` to verify no behavior changes
- Run `./clippy-all.sh` to verify no regressions
- Delete this plan directory: `rm -rf plans/hygiene-full/`
