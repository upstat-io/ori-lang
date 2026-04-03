---
plan: "clang-arc-lessons"
title: "Clang ARC Lessons: AIMS Optimization Enhancements"
status: not-started
references:
  - "~/projects/reference_repos/lang_repos/llvm-project/llvm/lib/Transforms/ObjCARC/"
  - "~/projects/reference_repos/lang_repos/swift/lib/SILOptimizer/ARC/"
  - "~/projects/reference_repos/lang_repos/lean4/src/Lean/Compiler/IR/RC.lean"
  - "compiler/ori_arc/src/aims/"
---

# Clang ARC Lessons: AIMS Optimization Enhancements

## Mission

Adopt battle-tested ARC optimization patterns from Clang/LLVM and Swift into Ori's AIMS pipeline: effect-aware coalescing barriers, compile-time statistics, physical-refcount-based nested pair elimination, late COW compound contraction, and PRE-style global RC code motion. AIMS is architecturally stronger than Clang's per-pointer state machine — these enhancements add the "outer ring" of legality/profitability machinery around the existing semantic core.

## Architecture

```
                         AIMS Pipeline (current + new passes)
                         ════════════════════════════════════
    analyze_program()  ─── MemoryContract per function (SCC fixpoint)
          │
    analyze_function() ─── Backward dataflow → AimsStateMap (7D lattice)
          │
    realize_rc_reuse() ─── Phase 1: RC + reuse + arg_ownership (pre-merge)
          │                    ┌──────────────────────────────┐
          │                    │  emit_rc_unified()           │
          │                    │    └─ coalesce_block_rc()    │
          │                    │       ┌────────────────────┐ │
          │                    │       │ flush_all() barrier│◄│─── Section 02: selective barriers
          │                    │       └────────────────────┘ │
          │                    └──────────────────────────────┘
          │
    FIP pre-check (5a)
          │
          │                    ┌──────────────────────────────┐
          ├────────────────────│  NEW: KnownSafe analysis     │◄─── Section 03: nested pair elim
          │                    │  (post-emission, pre-verify)  │
          │                    └──────────────────────────────┘
          │
          │                    ┌──────────────────────────────┐
          ├────────────────────│  NEW: PRE-style RC motion    │◄─── Section 05: global placement
          │                    │  (bidirectional, pre-merge)   │
          │                    └──────────────────────────────┘
          │
    verify + AIMS-verify + tail calls + unwind cleanup (6-8)
          │
    merge_blocks() (9)
          │
    realize_annotations() ─── Phase 2: COW + drop hints (10)
          │                    ┌──────────────────────────────┐
          │                    │  NEW: COW contraction        │◄─── Section 04: compound ops
          │                    └──────────────────────────────┘
          │
    final verify (11) + FBIP (12)

    Throughout: Section 01 statistics track RC ops before/after each pass
```

## Design Principles

1. **AIMS core is sacrosanct.** These enhancements wrap the existing 7D lattice and realization pipeline — they never replace or modify the core backward analysis or decision functions. AIMS derives semantic facts; the new passes use those facts for legality/profitability decisions on already-emitted RC ops.

2. **Legality and profitability are separate.** Following Clang's pattern: "pair safe" (KnownSafe — this pair CAN be eliminated) is independent from "motion safe" (CFGHazardAfflicted — we CAN move the ops). A transformation is applied only when both are satisfied.

3. **Statistics drive development.** Every optimization pass reports before/after counts. The first section (statistics) enables measurement for all subsequent sections. No optimization is considered "done" without measurable improvement on real programs.

## Section Dependency Graph

```
  01 Statistics ──────────────────────────────┐
       │                                      │
       ▼                                      │
  02 Barriers ──────┬──────────┐              │
       │            │          │              │
       ▼            │          ▼              │
  03 KnownSafe ─────┤    04 COW Contract      │
       │            │          │              │
       ▼            ▼          │              │
  05 RC Motion (requires 01, 02, 03) ◄────────┘
       │
       ▼
  06 Verification (requires all)

  Note: 03 and 04 are parallel — both depend on 01+02,
  neither depends on the other. 05 requires 03 but not 04.
```

- Section 01 is independent — pure additive, no behavioral change.
- Section 02 depends on 01 (uses statistics to measure improvement).
- Section 03 depends on 01 and 02 (builds on barrier improvements; selective barriers change which RC ops survive into KnownSafe analysis).
- Section 04 depends on 01 and 02 (COW contraction operates on post-emission IR shaped by barrier decisions; statistics measure contraction value).
- Section 05 depends on 01, 02, 03 (uses barriers, KnownSafe, and statistics).
- Section 06 requires all sections complete.

**Cross-section interactions (must be co-tested):**
- **Section 02 + 03**: Selective barriers change which RC ops survive into the KnownSafe analysis. KnownSafe elimination must work correctly with the reduced barrier set.
- **Section 02 + 04**: Selective barriers change which RC ops survive around COW diamond patterns. The contraction pass must correctly identify COW diamonds regardless of barrier changes.
- **Section 03 + 05**: KnownSafe flags computed in Section 03 are consumed by Section 05's code motion. The KnownSafe analysis must produce the correct flags for the PRE placement to be legal.

## Implementation Sequence

```
Phase 0 - Instrumentation
  └─ 01: Compile-time ARC statistics (SynergyMetrics extension)
  Gate: `ORI_LOG=ori_arc=info ori build tests/spec/` shows RC op counts per function

Phase 1 - Local Optimization
  └─ 02: Effect-aware coalescing barriers (coalesce/mod.rs)
  └─ 03: KnownSafe nested pair elimination (new pass)
  Gate: statistics show measurable RC operation reduction on real programs

Phase 2 - Backend Integration (requires Phase 0 + Phase 1 barriers)
  └─ 04: Late COW compound contraction (ori_llvm arc_emitter)
  Gate: compound COW ops emitted for provably-unique mutations

Phase 3 - Global Optimization  [CRITICAL PATH]
  └─ 05: PRE-style global RC code motion (new bidirectional pass)
  Gate: RC ops moved across basic blocks with correct CFG hazard checking

Phase 4 - Verification
  └─ 06: Full test matrix, behavioral equivalence, code journey
  Gate: ./test-all.sh green, dual-exec parity, no regressions
```

**Why this order:**
- Phase 0 (statistics) is pure instrumentation — no behavioral changes, enables measurement.
- Phase 1 (barriers + KnownSafe) operates on single blocks and local regions — simpler to verify.
- Phase 2 (COW contraction) extends to the LLVM emission layer — requires Phase 0 statistics and Phase 1's barrier improvements (selective barriers shape which RC ops are present around COW diamonds).
- Phase 3 (global motion) is the most complex — requires all prior infrastructure.

## Metrics (Current State)

| Component | Production LOC | Test LOC | Total |
|-----------|---------------|----------|-------|
| `coalesce/` | ~189 | ~214 | ~403 |
| `realize/metrics.rs` | ~126 | — | ~126 |
| `realize/decide.rs` | ~485 | — | ~485 |
| `realize/mod.rs` | ~464 | ~1572 | ~2036 |
| `pipeline/aims_pipeline.rs` | ~590 | — | ~590 (exceeds 500-line limit; must split before adding passes) |
| `emit_rc/cow.rs` | ~79 | — | ~79 |
| **Total touched** | **~1933** | **~1786** | **~3719** |

## Estimated Effort

| Section | Est. New Lines | Complexity | Depends On |
|---------|---------------|------------|------------|
| 01 Statistics | ~150 | Low | — |
| 02 Barriers | ~300 | High | 01 |
| 03 KnownSafe | ~500 | Medium | 01, 02 |
| 04 COW Contraction | ~400 (+match arm updates in ~30 files) | Medium-High | 01, 02 |
| 05 RC Motion | ~800 | High | 01, 02, 03 |
| 06 Verification | ~400 | Medium | All |
| **Total new** | **~2550** | | |
| **Total deleted** | **~50** | | |

## Prerequisites

| Task | Reason | Blocking | Owner |
|------|--------|----------|-------|
| Split `aims_pipeline.rs` (590 lines, exceeds 500-line limit) | Extract `verify_and_merge()`, `emit_postprocess()`, or second-pass helpers into `pipeline/second_pass.rs` submodule. Must be done before any new pipeline passes are added (Sections 03, 04, 05). | Sections 03, 04, 05 | First section to touch pipeline |

This is not a separate section — it is a concrete implementation task that MUST be completed by whoever starts Section 03, 04, or 05 (whichever is first). It is tracked here rather than in a section because it crosses section boundaries.

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| Coalesce flushes all vars at every call | Conservative barrier design | Section 02 | Not Started |
| No compile-time RC op counts | Missing instrumentation | Section 01 | Not Started |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Compile-Time ARC Statistics | `section-01-statistics.md` | Not Started |
| 02 | Effect-Aware Coalescing Barriers | `section-02-barriers.md` | Not Started |
| 03 | KnownSafe Nested Pair Elimination | `section-03-knownsafe.md` | Not Started |
| 04 | Late COW Compound Contraction | `section-04-cow-contraction.md` | Not Started |
| 05 | PRE-Style Global RC Code Motion | `section-05-rc-motion.md` | Not Started |
| 06 | Verification | `section-06-verification.md` | Not Started |
