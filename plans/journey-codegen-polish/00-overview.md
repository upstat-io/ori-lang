---
plan: "journey-codegen-polish"
title: "Journey Codegen Polish: Exhaustive Implementation Plan"
status: not-started
references:
  - "plans/code-journeys/overview.md"
  - "plans/code-journeys/07-loops-results.md"
  - "plans/code-journeys/15-fat-nested-collections-results.md"
  - "plans/code-journeys/16-fat-ownership-transfer-results.md"
  - "plans/code-journeys/17-fat-closure-capture-results.md"
---

# Journey Codegen Polish: Exhaustive Implementation Plan

## Mission

Achieve 10.0/10 codegen quality scores across all 17 code journeys by fixing the 5 remaining codegen inefficiencies that prevent J07, J15, J16, and J17 from reaching perfect scores. These are all LLVM IR quality issues — no correctness bugs remain.

## Architecture

```
Source → Lexer → Parser → TypeChecker (→ CanonicalIR) → ARC Pipeline → LLVM Codegen → Binary
                                                           ↑                ↑
                                                           │                │
                                                      Section 05       Sections 01,02,03
                                                      (ARC lowering)   (IR emission)
                                                           │
                                                      Section 04 (both ARC lowering + IR emission)
```

## Design Principles

1. **Every instruction must justify its existence.** If an instruction in emitted IR cannot be justified as necessary for correctness, it must not be emitted. "LLVM will optimize it away" is not justification — clean input IR means faster compile times, better debug builds, and fewer optimizer surprises.

2. **Attribute precision.** Every function must carry the most precise LLVM attributes derivable from static analysis. Missing `nounwind` forces LLVM to generate unnecessary exception handling infrastructure.

## Section Dependency Graph

```
01 Nounwind ──┐
02 Dead Loads ─┼── 06 Verification
03 Sret Copy ──┤
04 Iterator ───┤
05 Range ──────┘
      ↕
  02 ←→ 04 (soft dependency: both touch pointer-forwarding/alloca patterns)
```

Sections 01–05 are mostly independent. However, Sections 02 (Dead Loads) and 04 (Iterator Wrapping) both involve the pointer-forwarding/alloca optimization path in the ARC emitter. Changes to `borrowed_param_ptrs` forwarding (Section 02) may affect how iterator elements are forwarded (Section 04). **Recommendation**: implement Section 02 first, then Section 04, or implement them together as a unified pointer-forwarding optimization. Section 06 requires all others.

## Implementation Sequence

```
Phase 1 - Independent Fixes (narrow the front: complete one fully before starting another)
  Recommended order:
  └─ 05: Range unused field extraction (smallest, self-contained in ori_arc, low risk)
  └─ 03: Sret identity copy elimination (self-contained in arc_emitter, flag-based)
  └─ 01: Nounwind propagation (investigate-first — actual blocker unknown)
  └─ 02: Dead aggregate load elimination (pointer-forwarding foundation)
  └─ 04: Iterator option wrapping overhead (depends on 02's pointer-forwarding pattern)
  Gate: timeout 150 cargo t -p ori_llvm passes (debug AND release), timeout 150 ./test-all.sh green

Phase 2 - Verification
  └─ 06: Re-run all 17 journeys, verify 10.0 scores
  Gate: All 17 journeys score 10.0/10, Valgrind clean on J07/J15/J16/J17
```

**Ordering rationale**: Section 05 is lowest risk (one crate, one file, ARC IR only). Section 03 is self-contained. Section 01 requires investigation first (the overflow hypothesis is likely wrong -- see section-01-nounwind.md for details). Section 02 should precede 04 because both touch the pointer-forwarding path and 04 may reuse 02's infrastructure.

## Current Scores (Baseline — 2026-03-19)

| Journey | Score | Issue(s) |
|---------|-------|----------|
| J07 loops | 9.8 | Range unused field extraction (LOW) |
| J15 fat-nested | 8.7 | Iterator option wrapping (MEDIUM), missing nounwind (LOW), dead loads (LOW) |
| J16 fat-ownership | 9.9 | Dead aggregate loads (LOW), sret identity copy (LOW), missing nounwind (LOW) |
| J17 fat-closure | 9.9 | Dead loads in lambda (LOW), missing nounwind (LOW) |

## Known Bugs (Pre-existing)

None — all correctness bugs fixed as of 2026-03-19. Remaining issues are optimization quality only.

## Correctness Risks

These optimizations touch code that is adjacent to correctness-critical paths. Incorrect implementation could introduce correctness bugs:

1. **Section 01 (Nounwind)**: Incorrectly classifying a function as `nounwind` when it can actually unwind would cause UB — LLVM generates code that assumes no unwind, and landing pads for RC cleanup would be dropped. **Confirmed**: `ori_panic_cstr` (`ori_rt/src/io/mod.rs:129`) is `extern "C-unwind"` and calls `_Unwind_RaiseException` — it genuinely unwinds and MUST NOT be marked nounwind. The runtime declaration (`runtime_decl/runtime_functions.rs:113-116`) has attrs `[Cold, Noreturn]` (no `Nounwind`). **Also note**: overflow `ori_panic_cstr` calls are emitted at the LLVM IR level (by `checked_ops.rs:198-204`), NOT at the ARC IR level where `is_arc_function_nounwind()` operates. The actual blocker must be identified empirically via ARC IR inspection.
2. **Section 02 (Dead Loads)**: Skipping a param load that IS needed downstream would produce poison/undef values. This manifests as runtime crashes or incorrect results, not compile errors.
3. **Section 03 (Sret Identity Copy)**: Skipping a store when the return value was NOT written to the sret pointer produces garbage in the caller's return slot. This manifests as memory corruption.
4. **Section 04 (Iterator Wrapping)**: Scratch buffer pointer aliasing or lifetime issues would produce use-after-free or data corruption. Valgrind testing is essential.

All optimizations must have both positive tests (optimization applied correctly) and negative tests (optimization NOT applied when unsafe).

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Nounwind Propagation | `section-01-nounwind.md` | Not Started |
| 02 | Dead Aggregate Load Elimination | `section-02-dead-loads.md` | Not Started |
| 03 | Sret Identity Copy Elimination | `section-03-sret-identity.md` | Not Started |
| 04 | Iterator Option Wrapping | `section-04-iterator-wrapping.md` | Not Started |
| 05 | Range Unused Field Extraction | `section-05-range-fields.md` | Not Started |
| 06 | Verification | `section-06-verification.md` | Not Started |
