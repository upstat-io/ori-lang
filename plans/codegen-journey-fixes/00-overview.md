---
plan: "codegen-journey-fixes"
title: "Code Journey Findings: Exhaustive Implementation Plan"
status: not-started
supersedes: []
references:
  - "plans/code-journeys/summary.md"
  - "plans/code-journeys/journey1-results.md"
  - "plans/code-journeys/journey2-results.md"
  - "plans/code-journeys/journey3-results.md"
  - "plans/code-journeys/journey4-results.md"
  - "plans/code-journeys/journey5-results.md"
  - "plans/code-journeys/journey6-results.md"
  - "plans/aot_codegen_pipeline/"
---

# Code Journey Findings: Exhaustive Implementation Plan

## Mission

Fix all 9 open findings (#2–#10) from code journeys 1–7 as one cohesive effort: from nounwind soundness through IR quality to developer tooling. The goal is to eliminate all known codegen defects before expanding feature coverage to iterators, strings, and ARC-heavy code.

## Architecture

```
                         ┌─────────────────────────────────────────────┐
                         │          ARC IR (ArcFunction)              │
                         └───────────────┬────────────────────────────┘
                                         │
                    ┌────────────────────┼────────────────────────┐
                    ▼                    ▼                        ▼
           ┌──────────────┐   ┌───────────────────┐   ┌──────────────────┐
           │  Nounwind    │   │  Runtime Decl      │   │  Type Info       │
           │  Analysis    │   │  (98 eager decls)  │   │  (struct naming) │
           │  §01         │   │  §02.1             │   │  §04             │
           └──────┬───────┘   └────────┬───────────┘   └──────────────────┘
                  │                    │
                  ▼                    ▼
           ┌──────────────┐   ┌───────────────────┐
           │  IR Emission │   │  Closure/Tramp    │
           │  (invoke→    │   │  Pipeline          │
           │   call, dead │   │  §03               │
           │   blocks)    │   │                    │
           │  §02.2–02.3  │   └───────────────────┘
           └──────────────┘

  Key files:
    function_compiler/mod.rs   — nounwind analysis, lambda compilation
    arc_emitter/mod.rs         — invoke→call downgrade, dead block elim, switch
    runtime_decl/mod.rs        — 98 runtime declarations
    type_info/mod.rs           — struct type naming
    builtins/trampolines.rs    — closure trampolines
    ir_builder/calls.rs        — call/invoke emission
```

## Design Principles

**1. Soundness before optimization.** Nounwind unsoundness (#2) is UB that corrupts program behavior. It must be fixed before any optimization work that depends on correct nounwind information (dead block elimination, trampoline nounwind). The Journey 4 finding showed that panicking closures can unwind through a `call` in a `nounwind` function — this is the highest priority.

**2. Incremental improvement, no behavioral changes.** Findings #4–#10 are quality improvements, not bug fixes. Each should produce identical program behavior with cleaner/smaller IR. This means every change can be verified by running the existing test suite — no new behavioral tests needed, only IR-inspection tests.

## Section Dependency Graph

```
  §01 Nounwind Soundness ──────────────────────┐
    │                                           │
    ▼                                           ▼
  §02 IR Emission Cleanup              §03 Closure Pipeline
    (#4 eager decls,                     (#6 non-capturing,
     #5 dead blocks,                      #9 trampoline nounwind)
     #7 match branches)
    │                                           │
    │         §04 IR Readability                │
    │           (#8 struct names)               │
    │             (independent)                 │
    │                                           │
    │         §05 Developer Tooling             │
    │           (#10 cargo run)                 │
    │             (independent)                 │
    │                                           │
    └──────────────┬────────────────────────────┘
                   ▼
              §06 Verification
```

- Sections §04 and §05 are fully independent — can be done in any order, at any time.
- Section §01 must land before §02 (dead block elimination depends on correct nounwind sets) and §03 (trampoline nounwind depends on sound analysis).
- Section §06 requires all others.

**Cross-section interactions (must be co-implemented):**
- **§01 + §02.2**: Fixing nounwind for indirect calls (§01) changes which functions are nounwind, which changes which blocks are dead (§02.2). If §02.2 lands without §01, dead block elimination uses unsound nounwind sets.
- **§01 + §03.2**: Trampoline nounwind (§03.2) depends on the trampoline's callee being correctly analyzed. If §01 is unsound, marking trampolines nounwind could introduce new UB.

## Implementation Sequence

```
Phase 0 - Prerequisites
  └─ §01: Fix nounwind analysis for indirect calls and monomorphized callees

Phase 1 - IR Quality (parallelizable)
  ├─ §02.1: Lazy runtime declarations
  ├─ §02.2: Dead unreachable block elimination
  ├─ §02.3: Redundant match arm branch elimination
  ├─ §03.1: Non-capturing lambda optimization
  ├─ §03.2: Trampoline nounwind propagation
  ├─ §04: Named struct types in IR
  └─ §05: Cargo run LLVM feature guard
  Gate: ./test-all.sh green, ./llvm-test.sh green, all journey programs produce same output

Phase 2 - Verification
  └─ §06: Full verification — code journeys, dual-exec, regression suite
  Gate: 0 open findings in summary.md, all journey programs re-verified
```

**Why this order:**
- Phase 0 is the only behavioral fix (preventing UB). All Phase 1 items are pure quality improvements.
- Phase 1 items are independent of each other and can be parallelized.
- Phase 2 re-verifies everything end-to-end and updates the journey summary.

## Metrics (Current State)

| Crate | Production LOC | Test LOC | Total |
|-------|---------------|----------|-------|
| `ori_llvm` (codegen/) | ~4,500 | ~1,100 | ~5,600 |
| `ori_llvm` (runtime_decl/) | ~425 | ~50 | ~475 |
| **Total affected** | **~4,925** | **~1,150** | **~6,075** |

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Depends On |
|---------|-------------------|------------|------------|
| 01 Nounwind Soundness | ~80 | Medium | — |
|   ↳ 01.1 Indirect calls | ~30 | Low | — |
|   ↳ 01.2 Monomorphized callees | ~50 | Medium | — |
| 02 IR Emission Cleanup | ~120 | Low–Medium | §01 |
|   ↳ 02.1 Lazy runtime decls | ~60 | Low | — |
|   ↳ 02.2 Dead block elimination | ~30 | Low | §01 |
|   ↳ 02.3 Match branch elim | ~30 | Low | — |
| 03 Closure Pipeline | ~80 | Medium | §01 |
|   ↳ 03.1 Non-capturing lambdas | ~60 | Medium | — |
|   ↳ 03.2 Trampoline nounwind | ~20 | Low | §01 |
| 04 IR Readability | ~15 | Low | — |
| 05 Developer Tooling | ~10 | Low | — |
| 06 Verification | ~0 (testing only) | Low | All |
| **Total new/changed** | **~305** | | |

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| #2 Nounwind unsound for indirect calls | `is_arc_function_nounwind` ignores `Apply` instructions with closure/fn-ptr targets | Section 01.1 | Not Started |
| #3 Nounwind misses monomorphized callees | Monomorphized functions compiled AFTER callers; not in `nounwind_functions` at analysis time | Section 01.2 | Not Started |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Nounwind Soundness | `section-01-nounwind-soundness.md` | Not Started |
| 02 | IR Emission Cleanup | `section-02-ir-emission-cleanup.md` | Not Started |
| 03 | Closure Pipeline | `section-03-closure-pipeline.md` | Not Started |
| 04 | IR Readability | `section-04-ir-readability.md` | Not Started |
| 05 | Developer Tooling | `section-05-developer-tooling.md` | Not Started |
| 06 | Verification | `section-06-verification.md` | Not Started |
