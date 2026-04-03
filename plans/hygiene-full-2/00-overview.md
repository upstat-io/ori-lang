---
plan: "hygiene-full-2"
title: "Implementation Hygiene Full Sweep #2"
status: not-started
references:
  - ".claude/rules/impl-hygiene.md"
  - ".claude/rules/compiler.md"
---

# Implementation Hygiene Full Sweep #2

## Mission

Achieve a cohesive, well-architected compiler where every phase has clear boundaries, every piece of knowledge lives in exactly one canonical home, and every solution is the correct one. This sweep addresses the runtime COW protocol — where cow_mode checks are scattered with no canonical dispatch point; evaluator and codegen method dispatch — where iterator consumers duplicate the same algorithmic skeletons and cross-backend routing tables drift independently; type resolution — where parallel ParsedType matching has no shared resolver; and structural cleanliness — oversized files that mix responsibilities, stale plan annotations from completed work, and undocumented unsafe blocks. The standard is `.claude/rules/impl-hygiene.md`.

## Architecture

```
Hygiene fixes touch ALL compiler phases but do NOT change behavior:

  ori_rt (COW protocol)     ─── Section 01: centralize cow_mode checks
  ori_eval (dispatch)       ─── Section 02: extract shared iterator/method skeletons
  ori_eval ↔ ori_llvm       ─── Section 03: registry-driven enforcement tests
  ori_types (type resolver) ─── Section 04: TypeResolver trait for ParsedType
  ori_llvm (codegen DRY)    ─── Section 05: extract shared codegen helpers
  ori_lexer/ori_parse       ─── Section 06: parameterize cooking/parsing functions
  ALL crates                ─── Section 07: remove stale annotations + banners
  ALL crates                ─── Section 08: split oversized files
  ori_rt                    ─── Section 09: add SAFETY comments to ~80 unsafe blocks
```

## Design Principles

1. **Zero behavioral change.** Every fix is structural — renaming, extracting, centralizing, documenting. No test should change behavior. `./test-all.sh` must pass identically before and after each section.

2. **LEAKs first, separately.** Side logic is how clean architectures decay. Every LEAK creates a second source of truth that WILL drift. Fix LEAKs before DRIFTs, DRIFTs before GAPs, GAPs before BLOAT.

3. **Registry as canonical home.** When multiple backends maintain parallel dispatch tables, the fix is NOT to merge the backends — it's to add enforcement tests that verify both backends cover the same method set as `ori_registry`. The backends need different implementations, but the same coverage.

## Section Dependency Graph

```
  01 (rt COW)     02 (eval DRY)     04 (type DRY)     06 (lexer DRY)
       |               |                  |                  |
       v               v                  v                  v
  09 (SAFETY)     03 (cross-backend)      |             07 (annotations)
                       |                  |                  |
                       v                  v                  v
                  05 (LLVM DRY)      08 (file sizes)       10 (cleanup)
                       |                  |
                       v                  v
                      10 (cleanup) <──────┘
```

- Sections 01, 02, 04, 06 are independent — can be worked in any order.
- Section 03 benefits from 02 (eval dispatch is cleaner after DRY extraction).
- Section 05 is independent but logically follows 03 (both touch ori_llvm).
- Section 07 is independent (pure annotation cleanup).
- Section 08 (file splitting) is best done last before cleanup — other sections may change file sizes.
- Section 09 depends on 01 (COW code is reorganized before documenting it).
- Section 10 runs after all others.

## Implementation Sequence

```
Phase 1 - Independent LEAKs (any order)
  ├─ Section 01: Runtime COW protocol centralization
  ├─ Section 02: Evaluator algorithmic DRY
  ├─ Section 04: Type resolution DRY
  └─ Section 06: Lexer/parser DRY
  Gate: ./test-all.sh green, no behavior changes

Phase 2 - Cross-crate LEAKs
  ├─ Section 03: Cross-backend dispatch enforcement
  └─ Section 05: LLVM codegen internal DRY
  Gate: ./test-all.sh green, enforcement tests catch drift

Phase 3 - DRIFTs + EXPOSURE
  ├─ Section 07: Stale annotations and decorative banners
  └─ Section 09: SAFETY comments for ori_rt
  Gate: zero stale annotations, all unsafe blocks documented

Phase 4 - BLOAT
  └─ Section 08: File size violations
  Gate: zero files >500 lines (production, excluding exempt data tables)

Phase 5 - Verification + Cleanup
  └─ Section 10: Final verification and plan deletion
  Gate: ./test-all.sh green, ./clippy-all.sh clean, plan directory deleted
```

## Metrics (Current State)

| Category | Count |
|----------|-------|
| LEAK findings | 38 |
| GAP findings | 10 |
| DRIFT findings | 14 |
| WASTE findings | 5 |
| EXPOSURE findings (missing SAFETY) | 8 (covering ~80 unsafe blocks) |
| Files >500 lines | 58 |
| Functions >100 lines | 31+ |

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Depends On |
|---------|-------------------|------------|------------|
| 01 RT COW Protocol | ~200 | Medium | -- |
| 02 Eval DRY | ~300 | Medium | -- |
| 03 Cross-Backend Dispatch | ~150 (tests) | Low | 02 |
| 04 Type Resolution DRY | ~400 | High | -- |
| 05 LLVM DRY | ~250 | Medium | -- |
| 06 Lexer/Parser DRY | ~200 | Low | -- |
| 07 Stale Annotations | ~100 (deletions) | Low | -- |
| 08 File Sizes | ~500 (splits) | Medium | 01-07 |
| 09 SAFETY Comments | ~200 (comments) | Low | 01 |
| 10 Cleanup | ~10 | Low | all |
| **Total** | **~2300** | | |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Runtime COW Protocol Centralization | `section-01-rt-cow-protocol.md` | Not Started |
| 02 | Evaluator Algorithmic DRY | `section-02-eval-dry.md` | Not Started |
| 03 | Cross-Backend Dispatch Unification | `section-03-cross-backend-dispatch.md` | Not Started |
| 04 | Type Resolution DRY | `section-04-type-resolution-dry.md` | Not Started |
| 05 | LLVM Codegen Internal DRY | `section-05-llvm-dry.md` | Not Started |
| 06 | Lexer/Parser DRY | `section-06-lexer-parser-dry.md` | Not Started |
| 07 | Stale Annotations and Decorative Banners | `section-07-stale-annotations.md` | Not Started |
| 08 | File Size Violations | `section-08-file-size.md` | Not Started |
| 09 | SAFETY Comments for ori_rt | `section-09-safety-comments.md` | Not Started |
| 10 | Cleanup | `section-10-cleanup.md` | Not Started |
