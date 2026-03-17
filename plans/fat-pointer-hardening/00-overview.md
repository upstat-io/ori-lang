---
plan: "fat-pointer-hardening"
title: "Fat Pointer Hardening: All 17 Journeys to 10/10"
status: not-started
references:
  - "plans/code-journeys/overview.md"
  - "plans/code-journeys/14-fat-string-sharing-results.md"
  - "plans/code-journeys/15-fat-nested-collections-results.md"
  - "plans/code-journeys/16-fat-ownership-transfer-results.md"
  - "plans/code-journeys/17-fat-closure-capture-results.md"
---

# Fat Pointer Hardening: All 17 Journeys to 10/10

## Mission

Fix 3 CRITICAL bugs and 1 HIGH codegen issue discovered by the fat pointer code journeys (J14-J17), then harden the compiler with a combinatorial test matrix that crosses fat pointer types with every language feature. The goal: all 17 code journeys score 10.0/10, and the test matrix prevents future regressions at **any** intersection of fat pointers with other features.

**Constraint**: Every fix must be a **system-level solution** that applies generally to the type/feature category — not a patch for the specific journey code. The journeys are symptoms; the plan fixes root causes.

## Architecture

```
Root Causes (3 systemic issues)         Symptoms (journey failures)
─────────────────────────────           ───────────────────────────
1. Iterator takes element ownership     → J15: double-free on [str]
   but collection destructor also       → Affects ANY [T] where T
   frees elements                         has Drop (str, [T], closures)

2. Monomorphization doesn't propagate   → J17: codegen crash on
   concrete types through closure         closure capturing str
   capture environments                 → Affects ANY closure capturing
                                          non-scalar type (str, [T],
                                          structs, closures)

3. Value emission always uses           → J16: 3-6x instruction bloat
   field-by-field copy for fat          → J14: duplicate ptrtoint
   aggregates (GEP+load+insertvalue)    → Affects ALL fat pointer
   instead of aggregate load/store        operations everywhere
```

## Design Principles

1. **Fix the contract, not the symptom.** The iterator/collection double-free is an ownership contract violation — who owns the elements? Fix the contract (iterator borrows, collection owns) and every `[T]` where T has Drop works. Don't special-case `[str]`.

2. **Type completeness over type specificity.** The monomorphization bug isn't "closures can't capture strings" — it's "the type propagation path has a gap for non-scalar capture environments." Fix the propagation, and closures capturing any type work.

3. **Combinatorial testing catches intersection bugs.** The original 13 journeys all scored 10/10 because they tested features in isolation. The bugs live at feature intersections. The test matrix must cover `{type categories} × {language features}` systematically.

## Section Dependency Graph

```
Section 01 (Iterator Ownership)  ──┐
Section 02 (Monomorphization)    ──┼──→ Section 04 (Test Matrix) ──→ Section 05 (Verification)
Section 03 (Aggregate Emission)  ──┘
```

- Sections 01, 02, 03 are **independent** — different compiler subsystems, no shared code paths.
- Section 04 requires all fixes landed — it validates the cross-product.
- Section 05 requires the test matrix — it re-runs all 17 journeys and validates 10/10.

## Implementation Sequence

```
Phase 1 — Bug Fixes (independent, parallelizable)
  ├─ Section 01: Fix iterator/collection ownership contract
  │   Gate: [str], [[int]], [ClosureType] all drop without double-free
  ├─ Section 02: Fix monomorphization type propagation for captures
  │   Gate: closure capturing str/[T]/struct all compile and run correctly in AOT
  └─ Section 03: Aggregate load/store for fat pointer values
      Gate: instruction count for str operations matches ideal

Phase 2 — Test Matrix
  └─ Section 04: Build combinatorial test matrix
      Gate: all test matrix cells covered, all pass

Phase 3 — Verification
  └─ Section 05: Re-run all 17 journeys, validate 10/10
      Gate: overall score 10.0/10, ./test-all.sh green
```

**Why this order:**
- Phase 1 fixes are independent — they touch different crates (`ori_rt` + `ori_arc` + `ori_llvm` vs `ori_types` + `ori_arc` + `ori_llvm` vs `ori_llvm`). While Sections 01 and 02 both touch `ori_arc`, they operate on different subsystems within it (control_flow/for_loops vs lower/calls/lambda).

> **Warning: Section 01 is the highest-risk section.** It spans 3 crates (`ori_rt`, `ori_arc`, `ori_llvm`) and 4 subsystems (runtime `IterState::Drop`, ARC lowering `for_iterator.rs`, AIMS RC emission `emit_rc/`, and LLVM arc_emitter landing pads). The ownership contract change must be consistent across all 4 — a mismatch between any pair causes either double-free or leak. Implement with one crate at a time: trace first (01.1), then fix runtime (01.2), verify ARC IR agrees (01.2 last items), fix unwind (01.3), then generalize (01.4).

- Phase 2 validates the fixes work in combination, not just isolation.
- Phase 3 proves the system is complete.

**Note on `ori_arc`**: All three root causes flow through `ori_arc`:
1. `ori_arc/src/lower/control_flow/for_loops/` emits ARC IR for iteration, and `ori_arc/src/aims/emit_rc/` places RcDec on iterator elements -- this is where the double-free originates if both iterator and collection emit RcDec.
2. `ori_arc/src/lower/calls/lambda.rs` lowers closure captures into ARC IR, receiving types from `ori_types` -- if types are unresolved, ARC IR is wrong.
3. `ori_arc/src/classify/` determines ArcClass for fat pointers (DefiniteRef vs Scalar), which drives how `arc_emitter` materializes values (field-by-field vs aggregate).

Implementers MUST trace through `ori_arc` when debugging any of these root causes.

## Known Bugs (Pre-existing)

| Bug | Journey | Root Cause | Fix Location | Status |
|-----|---------|-----------|-------------|--------|
| Double-free on `[str]` elements | J15 | Iterator and collection destructor both free elements | Section 01 (`ori_rt`) | Not Started |
| Double `ori_buffer_rc_dec` in unwind | J15 | Landing pad emits two buffer drops | Section 01 (`ori_llvm` arc_emitter) | Not Started |
| Closure capturing str crashes AOT | J17 | Unresolved type variable leaks to codegen | Section 02 (`ori_types` mono + `ori_llvm` monomorphize) | Not Started |
| Field-by-field aggregate copy | J16, J14 | Value emission uses GEP+load+insertvalue instead of aggregate ops | Section 03 (`ori_llvm` value_emission) | Not Started |
| Duplicate ptrtoint in SSO guard | J14 | Same pointer converted twice per SSO check | Section 03 (`ori_llvm` rc_buffer_ops) | Not Started |
| Redundant unconditional branches in string functions | J14 | CFG simplification doesn't merge single-predecessor blocks after SSO guard emission | Section 03 (`ori_llvm` cfg_simplify) | Not Started |
| Dead landing pads for nounwind callees | J16 | `invoke` used instead of `call` for nounwind callees | Section 03 (`ori_llvm` terminators + dead_unwind) | Not Started |

## Estimated Effort

| Section | Est. Lines Changed | Complexity | Crates Touched | Depends On |
|---------|-------------------|------------|----------------|------------|
| 01 Iterator Ownership | ~50-100 | Medium | `ori_rt`, `ori_arc`, `ori_llvm` | — |
| 02 Monomorphization | ~30-80 | High | `ori_types`, `ori_llvm`, `ori_arc` | — |
| 03 Aggregate Emission | ~50-100 | Medium | `ori_llvm` | — |
| 04 Test Matrix | ~500-800 (tests) | Medium | `ori_llvm` (tests), test `.ori` files | 01, 02, 03 |
| 05 Verification | ~50 (scripts) | Low | scripts, plan docs | 04 |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Iterator–Collection Ownership Contract | `section-01-iterator-ownership.md` | Not Started |
| 02 | Monomorphization of Captured Types | `section-02-monomorphization.md` | Not Started |
| 03 | Aggregate Value Emission | `section-03-aggregate-emission.md` | Not Started |
| 04 | Combinatorial Test Matrix | `section-04-test-matrix.md` | Not Started |
| 05 | Verification | `section-05-verification.md` | Not Started |
