---
plan: "llvm-codegen-fixes"
title: "LLVM Codegen Fixes: Code Journey Issue Resolution"
status: complete
reviewed: false
supersedes: []
references:
  - "plans/code-journeys/overview.md"
  - "plans/code-journeys/journey1-results.md through journey12-results.md"
---

# LLVM Codegen Fixes: Code Journey Issue Resolution

## Mission

Fix all 28 issues discovered across 12 code journeys, bringing AOT correctness from 75% (9/12 journeys) to 100% (12/12), eliminating all LLVM undefined behavior, and systematically improving IR quality. The eval path is 12/12 correct and serves as the oracle — every AOT result must match.

## Architecture

```
Source (.ori)
  │
  ├─ Lexer → Parser → TypeChecker → Canonicalizer ─┐
  │         (0 issues found in these phases)        │
  │                                                 │
  │  ┌──────────────────────────────────────────────┘
  │  │
  │  ├─ Eval Path (12/12 correct — oracle)
  │  │
  │  └─ LLVM Codegen (25 of 28 issues)
  │     ├─ Section 01: Critical Correctness (C1-C4)  ← MUST FIX FIRST
  │     ├─ Section 02: UB & Soundness (M14, H2, H3, M2, M9)
  │     ├─ Section 03: Exception Handling (H1, M10, M11)
  │     ├─ Section 04: Alignment (M5)
  │     ├─ Section 05: Variant Codegen (M7, M8)
  │     ├─ Section 06: Struct & Param Codegen (M6, M13)
  │     ├─ Section 07: ARC Pipeline (M12)
  │     ├─ Section 08: Loop & Range (M4, L5, L6, L7)
  │     ├─ Section 09: IR Cleanliness (M3, L3, L4)
  │     └─ Section 10: Prelude & Startup (M1, L1, L2)
  │
  └─ Section 11: Verification (re-run all 12 journeys)
```

## Design Principles

1. **Correctness before optimization.** C1-C4 are silent miscompilations or crashes. These are fixed first. Optimization improvements (M3-M13, L1-L7) come after all correctness and soundness issues are resolved. Rationale: C3 and C4 produce *wrong answers with no visible error* — the most dangerous class of defect.

2. **Eval as oracle.** The eval path is 12/12 correct. Every AOT fix is validated by comparing AOT output against eval output using `./scripts/dual-exec-verify.sh`. No fix is complete until both paths agree.

3. **No workarounds.** Each fix addresses the root cause identified in the journey analysis. No special-casing for specific test programs — fixes must be general.

## Section Dependency Graph

```
Section 01 (Critical Correctness) ──────────────────┐
Section 02 (UB & Soundness) ─────────────────────────┤
Section 03 (Exception Handling) ─────────────────────┤
Section 04 (Alignment) ─────────────────────────────┤
Section 05 (Variant Codegen) ────────────────────────┤── Section 11 (Verification)
Section 06 (Struct & Param) ─────────────────────────┤
Section 07 (ARC Pipeline) ──────────────────────────┤
Section 08 (Loop & Range) ──────────────────────────┤
Section 09 (IR Cleanliness) ────────────────────────┤
Section 10 (Prelude & Startup) ─────────────────────┘
```

- Sections 01-10 are largely independent and can be worked in any order.
- Section 01 should be prioritized — it unblocks real programs.
- Section 02 should come next — it eliminates LLVM undefined behavior.
- Sections 03-10 can be worked in any order after 01-02.
- Section 11 requires all other sections to be complete.

**Cross-section interactions:**
- **Section 01 (C4) + Section 05 (M7)**: Option match fix (C4) and variant construction cleanup (M7) both touch variant codegen. Coordinate changes.
- **Section 03 (H1, M11) + Section 02 (H2)**: nounwind analysis and landing pad generation share the same code path in the ARC emitter. Fix together.
- **Section 04 (M5) + Section 06 (M6)**: Both touch `load_indirect_param` and struct field access codegen.

## Implementation Sequence

```
Phase 0 - Critical Correctness                    [HIGHEST PRIORITY]
  └─ 01.1: Fix C4 — Option match tag inversion
  └─ 01.2: Fix C3 — Payload sum type $eq codegen
  └─ 01.3: Fix C1 — Mixed closure argument mismatch
  └─ 01.4: Fix C2 — List indexing __index registration
  Gate: All 12 journeys produce correct results in both eval and AOT

Phase 1 - UB & Soundness
  └─ 02.1: Fix M14 — None variant uninitialized payload
  └─ 02.2: Fix H2 — Audit nounwind for runtime calls
  └─ 02.3: Fix M9 — Range overflow for ..=INT_MAX (runtime step-sign branch)
  └─ 02.4: Add H3 — noalias on proven non-aliasing parameters
  └─ 02.5: Fix M2 — Checked arithmetic (overflow panics)
  Gate: Zero LLVM UB in generated IR; nounwind analysis sound

Phase 2 - Exception Handling & Alignment          [QUICK WINS]
  └─ 03: Exception handling cleanup (H1, M10, M11)
  └─ 04: Alignment fix (M5 — DataLayout-driven, target-aware)
  Gate: No orphaned landing pads; correct alignment on all loads; `opt -passes=verify` clean

Phase 3 - Codegen Quality
  └─ 05: Variant codegen (M7, M8)
  └─ 06: Struct/param codegen (M6, M13)
  └─ 07: ARC drop dedup (M12)
  └─ 08: Loop/range optimization (M4, L5, L6, L7)
  └─ 09: IR cleanliness (M3, L3, L4)
  └─ 10: Prelude optimization (M1, L1, L2)
  Gate: IR quality improvements measurable; no regressions

Phase 4 - Verification                            [FINAL]
  └─ 11: Re-run all 12 journeys, dual-exec verify, valgrind, IR verifier
  Gate: 12/12 journeys correct, 0 dual-exec mismatches, 0 valgrind errors, opt -passes=verify clean
```

**Why this order:**
- Phase 0 fixes silent miscompilations and crashes — the most dangerous defects.
- Phase 1 eliminates UB — programs that "work" today may break with LLVM upgrades.
- Phase 2 is quick wins (M5 requires DataLayout-driven ABI alignment but is well-scoped, H1/M11 share code).
- Phase 3 is quality improvements that benefit every program but don't affect correctness.
- Phase 4 proves everything works as one system.

## Issue Summary (from 12 Code Journeys)

### By Severity

| Severity | Count | IDs |
|----------|-------|-----|
| CRITICAL | 4 | C1, C2, C3, C4 |
| HIGH | 3 | H1, H2, H3 |
| MEDIUM | 14 | M1-M14 |
| LOW | 7 | L1-L7 |
| **Total** | **28** | |

### By Phase

| Phase | Issues | Count |
|-------|--------|-------|
| LLVM Codegen | C1-C4, H1-H3, M2-M14, L3-L7 | 25 |
| Canonicalizer | L1, L2 | 2 |
| Overall | M1 | 1 |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Critical Correctness | `section-01-critical-correctness.md` | Not Started |
| 02 | UB & Soundness | `section-02-ub-soundness.md` | Not Started |
| 03 | Exception Handling | `section-03-exception-handling.md` | Not Started |
| 04 | Alignment | `section-04-alignment.md` | Not Started |
| 05 | Variant Codegen | `section-05-variant-codegen.md` | Not Started |
| 06 | Struct & Param Codegen | `section-06-struct-param-codegen.md` | Not Started |
| 07 | ARC Pipeline | `section-07-arc-pipeline.md` | Not Started |
| 08 | Loop & Range | `section-08-loop-range.md` | Not Started |
| 09 | IR Cleanliness | `section-09-ir-cleanliness.md` | Not Started |
| 10 | Prelude & Startup | `section-10-prelude-startup.md` | Not Started |
| 11 | Verification | `section-11-verification.md` | Not Started |
