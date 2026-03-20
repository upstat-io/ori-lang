---
plan: "rc-integrity"
title: "RC Integrity: Leak-Free Codegen & Matrix Regression Guard"
status: complete
references:
  - "plans/aims-10/"
  - "plans/aims-codegen-quality/"
---

# RC Integrity: Leak-Free Codegen & Matrix Regression Guard

## Mission

Eliminate all memory leaks in AOT-compiled Ori programs, harden the test infrastructure to detect leaks automatically, expand code journey coverage to exercise leak-prone patterns, and build matrix regression tests that narrow the band of acceptable behavior — making it progressively harder for future work to introduce regressions as the compiler grows in complexity.

## Context

We almost merged the AIMS branch with broken leak detection (`ORI_CHECK_LEAKS` was dead on Linux AOT — the LLVM-generated `main()` wrapper called `_ori_main()` directly without any leak check; `ori_run_main()` is only used on Windows/MSVC for SEH). The fix was adding an `ori_check_leaks()` call directly to the LLVM-generated main wrapper. This exposed 23 pre-existing RC leaks across slices, structs with heap fields, string formatting, list equality, and more. The primary loop reassignment leak (FatValue PrimOp in `is_consuming_primop`) is fixed, but many diverse leaks remain. None of the 13 code journeys exercise heap-typed loop reassignment — which is how the crash was missed.

## Architecture

```
Test Infrastructure             ARC Pipeline              Runtime
┌──────────────────┐    ┌──────────────────────┐    ┌──────────────────┐
│ test-all.sh      │    │ emit_rc/helpers.rs    │    │ ori_rt            │
│ ├─ cargo test    │    │ ├─ is_consuming_      │    │ ├─ rc/            │
│ ├─ AOT tests     │◄───│ │   primop()          │    │ │  allocate.rs    │
│ │  └─ ORI_CHECK_ │    │ ├─ is_ownership_      │    │ │  debug.rs       │
│ │     LEAKS=1    │    │ │   transfer()        │    │ ├─ string/        │
│ ├─ spec tests    │    │ └─ precompute_block_  │    │ │  ops.rs         │
│ └─ valgrind/     │    │     uses()            │    │ └─ list/          │
├──────────────────┤    │                       │    │    cow.rs         │
│ Matrix Tests     │    │ realize/walk.rs       │    └──────────────────┘
│ ├─ leak patterns │    │ ├─ emit_last_use_     │
│ ├─ struct drops  │    │ │   decs()            │    LLVM Codegen
│ ├─ loop reassign │    │ └─ emit_defined_dead()│    ┌──────────────┐
│ ├─ slice cleanup │    │                       │    │ arc_emitter/  │
│ ├─ closure caps  │    │ edge_cleanup.rs       │    │  drop_gen.rs  │
│ └─ nested RC     │    │ └─ emit_edge_cleanup()│    └──────────────┘
└──────────────────┘    │                       │
                        │ drop/mod.rs           │    Code Journeys
                        │ ├─ DropKind enum      │    ┌──────────────┐
                        │ └─ compute_drop_kind()│    │ J01-J13 (✓)  │
                        └──────────────────────┘    │ J14: string  │
                                                     │      builder │
                                                     │ J15: RC life │
                                                     │ J16: COW     │
                                                     └──────────────┘
```

## Design Principles

1. **Leak detection is mandatory, not opt-in.** Every AOT test binary calls `ori_check_leaks()` at exit. If `ORI_CHECK_LEAKS=1` is set, leaks cause exit code 2. The test harness sets this for all AOT tests. No test passes silently with leaked memory.

2. **Matrix tests narrow regression bands.** Instead of testing one example per pattern, test the cross-product of (value type × operation × context). A string in a loop, a list in a loop, a struct-with-list in a loop — each combination gets its own test. When the ARC pipeline changes, regressions surface immediately because the matrix covers the full space.

3. **Code journeys cover real-world composition.** Unit tests verify individual patterns. Code journeys verify that patterns compose correctly through the full pipeline. Adding journeys for heap-typed loops, nested RC structures, and COW patterns fills the gap that let the crash slip through.

## Section Dependency Graph

```
01 Tooling ──► 02 Leak Fixes ──┬──► 03 Journeys ───┐
                                │                    ├──► 05 Verification
                                └──► 04 Matrix Tests─┘
```

- **Section 01** (Tooling) must be first — leak detection infrastructure must work before we can verify fixes.
- **Section 02** (Leak Fixes) depends on 01. Must complete before 03 and 04 so new tests don't mask pre-existing leaks.
- **Sections 03, 04** depend on both 01 (leak detection) and 02 (leak fixes), and can proceed in parallel after both are complete.
- **Section 05** (Verification) depends on all others.

## Implementation Sequence

```
Phase 0 - Infrastructure (Section 01)
  └─ 01.1: ori_check_leaks() in main wrapper (DONE)
  └─ 01.2: test-all.sh leak check integration
  └─ 01.3: Valgrind CI script
  Gate: ORI_CHECK_LEAKS=1 detects leaks in AOT binaries

Phase 1 - Fix Leaks (Section 02)
  └─ 02.1: Categorize all 23 failing tests by root cause
  └─ 02.2: Fix ARC pipeline bugs (FatValue drops, struct drops, edge cleanup)
  └─ 02.3: Fix runtime bugs (slice RC, string formatting)
  Gate: All 1317 AOT tests pass with ORI_CHECK_LEAKS=1

Phase 2 - Expand Coverage (Sections 03, 04)
  └─ 03.1-03.3: New code journeys (J14-J16)
  └─ 04.1-04.5: Matrix tests by pattern category + journey guards
  Gate: 66+ matrix/guard tests pass, all 16 journeys score 10/10

Phase 3 - Verification (Section 05)
  └─ 05.1: Full test suite green (test-all.sh, clippy, fmt, dual-exec-verify)
  └─ 05.2: Leak verification (ORI_CHECK_LEAKS + valgrind on all journeys)
  └─ 05.3: Journey score verification (all 16 at 10/10)
  └─ 05.4: Release build (debug + release both pass)
  Gate: Zero leaks, zero regressions, zero score drops
```

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| FatValue PrimOp treated as consuming | `is_consuming_primop` checks `!= Scalar` instead of `== RcPointer` | `emit_rc/helpers.rs:288` | **FIXED** |
| `ORI_CHECK_LEAKS` dead on Linux AOT | LLVM `main()` wrapper had no leak check call (`ori_run_main` is Windows/MSVC only) | `entry_point.rs` | **FIXED** |
| 7 slice tests leak | Slice RC cleanup not dropping original buffer | `ori_rt` / ARC pipeline | Not Started |
| 4 string_sso tests leak | Heap string drops missing at end of scope | ARC pipeline | Not Started |
| 3 struct tests leak | Structs with RC fields not dropping children | ARC pipeline | Not Started |
| 4 list trait tests leak | List equality/comparison not freeing operands | ARC pipeline | Not Started |
| 5 misc tests leak | Various patterns (catch, for_iter, aims) | Mixed | Not Started |
| While-loop RC reassignment | Untested — `while` desugars to `loop+break`, edge cleanup may miss drops | ARC pipeline | Needs Investigation |
| Closure RC capture drops | Untested — `DropKind::ClosureEnv` must drop captured RC vars | ARC pipeline | Needs Investigation |
| Match arm dead variable drops | Untested — variables dead in some arms need edge cleanup | ARC pipeline | Needs Investigation |

## Codebase Hygiene Notes (Nearby Issues)

The following findings are in files adjacent to (but not directly modified by) this plan. If the implementer touches these files for any reason during leak fixes, they should be cleaned up in the same pass:

- **[STYLE]** `compiler/ori_rt/src/format/mod.rs` (477 lines) — Contains 12 decorative banners (`// =====...`). Per hygiene rules, banners should be removed if the file is touched. The file is near the 500-line limit; if edits push it over, split into submodules.

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Tooling — Leak Detection Infrastructure | `section-01-tooling.md` | In Progress |
| 02 | Fix All Pre-Existing Leaks | `section-02-leak-fixes.md` | Not Started |
| 03 | Code Journeys — Expanded Coverage | `section-03-journeys.md` | Not Started |
| 04 | Matrix Testing — Regression Guard | `section-04-matrix-testing.md` | Not Started |
| 05 | Verification & Merge Gate | `section-05-verification.md` | Not Started |
