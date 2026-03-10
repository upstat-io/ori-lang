---
plan: "repr-opt"
title: "Representation Optimization & ARC Intelligence: Exhaustive Implementation Plan"
status: not-started
reviewed: false
supersedes:
  - "docs/ori_lang/proposals/approved/representation-optimization-proposal.md (implements)"
references:
  - "docs/ori_lang/v2026/spec/annex-e-system-considerations.md"
  - "docs/ori_lang/proposals/approved/representation-optimization-proposal.md"
  - "plans/value-semantics-optimization/"
  - "compiler/ori_arc/src/lib.rs"
  - "compiler/ori_llvm/src/codegen/type_info/"
---

# Representation Optimization & ARC Intelligence: Exhaustive Implementation Plan

## Mission

Complete the representation optimization system as one cohesive machine: from abstract types through range analysis and escape analysis to optimally-narrowed LLVM IR, with ARC header compression and thread-local fast paths — making Ori's generated code competitive with hand-written C while the programmer never sees a bit width.

This plan implements the full 3-tier framework from the approved representation-optimization proposal, plus ARC-specific optimizations that no other language combines in one system: Lean 4's reuse analysis, Swift's retain/release pairing, Koka's FBIP verification, and Zig's comptime narrowing — unified under Ori's deterministic ARC model.

## Architecture

```
                        ┌──────────────────────────────────┐
                        │         ori_types (Pool)         │
                        │  Tag::Int, Tag::Float, Tag::Str  │
                        │  Semantic contracts only          │
                        └──────────────┬───────────────────┘
                                       │
                                       ▼
                    ┌──────────────────────────────────────┐
                    │     Section 01: Representation IR    │
                    │  ReprPlan — the decision document    │
                    │  Maps every Idx → MachineRepr        │
                    └──────────┬───────────────────────────┘
                               │
              ┌────────────────┼────────────────────────┐
              ▼                ▼                         ▼
    ┌─────────────────┐ ┌────────────────┐  ┌───────────────────┐
    │  §02 Triviality │ │  §03 Range     │  │  §08 Escape       │
    │  Transitive ARC │ │  Analysis      │  │  Analysis         │
    │  elision        │ │  Framework     │  │  Stack promotion   │
    └────────┬────────┘ └───────┬────────┘  └─────────┬─────────┘
             │                  │                      │
             │          ┌───────┴────────┐             │
             │          ▼                ▼             │
             │  ┌──────────────┐ ┌──────────────┐     │
             │  │ §04 Integer  │ │ §05 Float    │     │
             │  │ Narrowing    │ │ Narrowing    │     │
             │  └──────┬───────┘ └──────┬───────┘     │
             │         │                │              │
             ▼         ▼                ▼              ▼
    ┌──────────────────────────────────────────────────────────┐
    │                 ReprPlan (populated)                      │
    │  int→i32, float→f32, Option<int>→niche, struct→reordered│
    └──────────────┬───────────────────────────────────────────┘
                   │
       ┌───────────┼────────────┬────────────┐
       ▼           ▼            ▼            ▼
┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐
│ §06      │ │ §07 Enum │ │ §09 ARC  │ │ §10 Thread   │
│ Struct   │ │ Repr     │ │ Header   │ │ Local ARC    │
│ Layout   │ │ Niche    │ │ Compress │ │ Non-atomic   │
└─────┬────┘ └────┬─────┘ └────┬─────┘ └──────┬───────┘
      │           │            │               │
      └───────────┴────────────┴───────────────┘
                          │
                          ▼
              ┌───────────────────────┐
              │  §11 Collection       │
              │  Specialization       │
              │  SSO, SVO, packed     │
              └───────────┬───────────┘
                          │
                          ▼
              ┌───────────────────────┐
              │  §12 Verification     │
              │  Dual-exec, Valgrind  │
              │  Benchmarks, safety   │
              └───────────────────────┘
```


## Design Principles

### 1. Semantic Contract Inviolability

The programmer's mental model must NEVER break. `int` is always [-2⁶³, 2⁶³-1]. `float` is always IEEE 754 double. No optimization may produce a different result than the canonical representation for any conforming program. This is enforced by the spec (Annex E — System Considerations) and tested by dual-execution verification (§12).

**Why this matters:** If narrowing ever changes semantics, the entire premise of hidden representations collapses. Every optimization must include a proof obligation (either formal or test-based) that semantic equivalence holds.

### 2. ARC Determinism as Information Advantage

ARC's compile-time visibility into every retain/release is a **strictly more powerful** basis for optimization than tracing GC. The compiler can compute exact lifetime bounds, sharing cardinality, and thread-locality — information that's unavailable to GC-based systems. Every ARC optimization in this plan exploits this determinism.

**Why this matters:** This is Ori's competitive edge. Swift proved ARC can match GC throughput; Lean proved RC can enable allocation reuse; Koka proved FBIP can eliminate allocations entirely. Ori combines all three because its ARC pipeline already has the infrastructure (borrow inference, reset/reuse, RC elimination).

### 3. Optimization as a Separate Phase

All narrowing decisions are recorded in a `ReprPlan` data structure between type checking and codegen. The type checker never sees machine representations. The codegen never makes narrowing decisions. This keeps both phases simple and makes every optimization independently testable.

**Why this matters:** Mixing analysis with codegen creates bugs that are impossible to diagnose. Swift's SIL optimizer is a separate phase for this reason. Lean's LCNF pipeline is a separate phase. The cost of an extra data structure is negligible compared to the debugging cost of entangled phases.

```
Information flow — each stage enriches the ReprPlan:

  Phase A — Early type-level decisions (before ARC lowering, on Pool only):
    Pool (semantic) → ReprPlan (empty)
      → §01 Canonical (populates canonical representations for all types)
        → §02 Triviality (marks trivial types — recursive Pool walk)

  Phase B — Function-level analysis (after ARC lowering, on ArcFunction):
    ArcFunction → §03 Range analysis (adds interval bounds per ArcVarId)
      → §04 Integer narrowing (sets MachineInt variants for struct fields/locals)
        → §05 Float narrowing (sets MachineFloat variants)

  Phase C — Layout decisions (uses narrowed types from Phase B):
    ReprPlan (with narrowed types) →
      → §06 Struct layout (computes field order + padding — uses narrowed field sizes)
        → §07 Enum repr (fills niches, computes tag type — uses narrowed discriminants)
          → §11 Collection (sets backing store strategy — uses narrowed element types)

  Phase D — ARC intelligence (after ARC lowering, on ArcFunction):
    ArcFunction →
      → §08 Escape analysis (marks stack-promotable allocations)
        → §09 ARC header (sets RC width per allocation)
          → §10 Thread-local (marks non-atomic RC)
            → ori_llvm reads final ReprPlan
```

## Section Dependency Graph

```
§01 ReprPlan IR ──────────────────────────────────────────────┐
  │                                                           │
  ├──→ §02 Transitive Triviality ─────────────────────────┐   │
  │                                                       │   │
  ├──→ §03 Range Analysis Framework ──┬──→ §04 Int Narrow │   │
  │                                   └──→ §05 Float      │   │
  │                                                       │   │
  ├──→ §06 Struct Layout ◄────────────── (§04, §05)       │   │
  │                                                       │   │
  ├──→ §07 Enum Repr ◄───────────────── (§04)             │   │
  │                                                       │   │
  ├──→ §08 Escape Analysis ◄─────────── (§02)             │   │
  │                                                       │   │
  ├──→ §09 ARC Header ◄──────────────── (§02, §08)        │   │
  │                                                       │   │
  ├──→ §10 Thread-Local ARC ◄────────── (§08, §09)        │   │
  │                                                       │   │
  └──→ §11 Collection Specialization ◄─ (§04, §06)        │   │
                                                          │   │
  §12 Verification ◄─────────────────── (ALL)             │   │
```

- **§01** is the foundation — everything depends on it
- **§02, §03** are independent of each other and can be developed in parallel
- **§04, §05** both depend on §03 (range analysis) and can be developed in parallel
- **§06** depends on §04/§05 (needs to know narrowed field types for layout)
- **§07** depends on §04 (integer narrowing affects discriminant sizing)
- **§08** depends on §02 (triviality affects escape classification)
- **§09** depends on §02, §08 (triviality + escape determine RC width)
- **§10** depends on §08, §09 (escape analysis + header decisions)
- **§11** depends on §04, §06 (element narrowing + layout knowledge)
- **§12** depends on all (verification is last)

**Cross-section interactions (must be co-implemented):**
- **§04 + §06**: Integer narrowing changes field sizes, which changes struct layout. If narrowing lands without layout update, struct padding wastes the savings.
- **§08 + §09**: Escape analysis determines which allocations are stack-local (no RC header) vs heap (need RC header). If escape analysis lands without header compression, heap allocations still use i64 headers unnecessarily.
- **§02 + ori_arc pipeline**: Transitive triviality must agree with `ori_arc::ArcClassifier` (defined in `ori_arc/src/classify/mod.rs`, re-exported at crate root). If they disagree, codegen either emits unnecessary RC ops or skips needed ones. Both must use the same classification — `ori_types::triviality::classify_triviality()` is the single source of truth.
- **§01.7 + §06**: `#repr` attributes (c, packed, transparent, aligned) are stored in ReprPlan by §01 but consumed by §06's layout algorithm. The layout pass must check `repr_attrs` before reordering fields. If §01 stores attrs but §06 ignores them, C-ABI structs get silently reordered → FFI bugs.
- **§02 + ori_eval**: The evaluator (`ori_eval`) does NOT use triviality classification and is NOT affected by §02 or any other section in this plan. `ori_eval` uses Rust-native reference counting (no `ori_rc_*` calls). All representation optimizations are confined to the `ori_types → ori_arc → ori_repr → ori_llvm` pipeline.
- **§02 standalone viability**: The core algorithm (`classify_triviality()` in `ori_types`) and the `ArcClassifier` delegation can be implemented and tested before §01 (ReprPlan) is complete. Only the `TypeInfoStore::is_trivial()` delegation to `ReprPlan` blocks on §01. This means §02 can begin implementation in parallel with §01.

## Implementation Sequence

```
Phase 0 — Prerequisites
  └─ §01: ReprPlan IR data structure + empty pass integration

Phase 1 — Foundation (parallel)
  ├─ §02: Transitive triviality → ARC elision for compound trivial types
  └─ §03: Range analysis framework (abstract interpretation engine)
  Gate: `./test-all.sh` green, no behavioral changes, ReprPlan populated with triviality + ranges

Phase 2 — Core Narrowing (parallel)
  ├─ §04: Integer narrowing (i64 → i32/i16/i8 where safe)
  └─ §05: Float narrowing (f64 → f32 where safe)
  Gate: narrowed types visible in LLVM IR, dual-exec shows identical results

Phase 3 — Layout Optimization (partially parallel)
  ├─ §06: Struct field reordering + padding minimization  ─┐
  ├─ §07: Enum niche filling + discriminant narrowing       │ (§06 ∥ §07)
  └─ §11: Collection specialization (SSO, SVO, packed arrays) ← after §06
  Gate: sizeof() measurements show reduced footprint, Valgrind clean

Phase 4 — ARC Intelligence  [CRITICAL PATH — sequential, NOT parallel]
  §08: Escape analysis → stack promotion
    → §09: ARC header compression (i64 → i32/i16/i8 refcounts) [depends on §08]
      → §10: Thread-local non-atomic RC [depends on §08, §09]
  Gate: benchmark programs show measurable speedup, Valgrind clean, no leaks

Phase 5 — Verification
  └─ §12: Full verification (dual-exec, Valgrind, benchmarks, code journeys)
  Gate: all benchmarks baselined, zero regressions, perf targets met
```

**Why this order:**
- Phase 0 is pure infrastructure — no behavioral changes, just adds the ReprPlan data structure
- Phase 1 must precede Phase 2 because narrowing decisions consume range analysis results and triviality info
- Phase 2 must precede Phase 3 because struct layout needs narrowed field types
- Within Phase 3: §06 and §07 can be parallel, but §11 must come after §06 (§11 uses layout info from §06)
- Phase 4 is the critical path because ARC optimizations have the highest performance impact but are the most dangerous (incorrect RC = use-after-free or leak)
- Phase 5 gates the release — nothing ships without full verification

**Known failing tests (expected until plan completion):**

None expected. Each section is additive — the current system works correctly with all types at canonical width. Narrowing is pure optimization; no tests should break unless there's a semantic equivalence bug (which must be caught in §12).

## Metrics (Current State)

| Crate | Production LOC | Test LOC | Total |
|-------|---------------|----------|-------|
| `ori_arc` | ~17,700 | ~21,100 | ~38,800 |
| `ori_llvm` (type_info) | ~1,160 | ~1,360 | ~2,520 |
| `ori_llvm` (arc_emitter) | ~12,070 | ~1,890 | ~13,960 |
| `ori_rt` | ~9,620 | ~9,710 | ~19,330 |
| **Total existing** | **~40,550** | **~34,060** | **~74,610** |

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 ReprPlan IR | ~1,130 (6 files, all <500 lines) | Medium-High | — |
| 02 Transitive Triviality | ~500 | Medium | §01 |
|   ↳ 02.1 Unify triviality classification | ~100 | Low | §01 |
|   ↳ 02.2 Transitive walk with cycle detection | ~150 | Medium | §01 |
|   ↳ 02.3 ARC elision in ori_arc pipeline | ~100 | Medium | §01 |
|   ↳ 02.4 Drop function elision | ~50 | Low | §01 |
|   ↳ 02.5 Newtype & FFI types | ~30 | Low | §01 |
|   ↳ 02.6 Generic type interaction | ~20 | Low | §01 |
|   ↳ 02.7 Completion checklist & tests | ~50 | Low | §01 |
| 03 Range Analysis | ~1,400 (5 files in `range/` submodule) | **High** | §01 |
|   ↳ 03.1 Interval lattice | ~250 | Medium | §01 |
|   ↳ 03.2 Transfer functions | ~450 | **High** — mul/div corner cases | §01 |
|   ↳ 03.3 Widening/narrowing + fixpoint | ~350 | **High** — block params, terminators, termination | §01 |
|   ↳ 03.4 Conditional refinement | ~150 | Medium — 6 comparison operators | §01 |
|   ↳ 03.5 Interprocedural (DEFERRABLE) | ~150 | **High** — SCC fixpoint | §01 |
|   ↳ 03.6 Config, tests, integration | ~50 | Low | §01 |
| 04 Integer Narrowing | ~800 | High | §03 |
|   ↳ 04.1 Width selection | ~200 | Medium | §03 |
|   ↳ 04.2 ABI boundary widening | ~150 | Medium | §03 |
|   ↳ 04.3 Overflow guards | ~250 | High | §03 |
| 05 Float Narrowing | ~500 | High | §03 |
| 06 Struct Layout | ~700 | Medium | §04, §05 |
|   ↳ 06.1 Field reordering | ~300 | Medium | §04 |
|   ↳ 06.2 Padding minimization | ~200 | Medium | §04 |
| 07 Enum Repr | ~900 | High | §04 |
|   ↳ 07.1 Niche filling | ~400 | High | §04 |
|   ↳ 07.2 Discriminant narrowing | ~200 | Medium | §04 |
|   ↳ 07.3 Tagged pointers | ~300 | High | §04 |
| 08 Escape Analysis | ~1,500 (5+ files in `escape/` submodule) | Very High | §02 |
|   ↳ 08.1 Intraprocedural escape | ~500 | High | §02 |
|   ↳ 08.2 Interprocedural escape | ~600 | Very High | §02 |
|   ↳ 08.3 Stack promotion codegen | ~400 | High | §02 |
| 09 ARC Header Compression | ~600 | High | §02, §08 |
| 10 Thread-Local ARC | ~500 | High | §08, §09 |
| 11 Collection Specialization | ~1,000 (SSO already exists in `ori_rt/src/string/`) | High | §04, §06 |
|   ↳ 11.1 Small string optimization | ~400 | High | — |
|   ↳ 11.2 Small vector optimization | ~300 | High | §04 |
|   ↳ 11.3 Packed bool arrays | ~300 | Medium | — |
| 12 Verification | ~800 | Medium | ALL |
| **Total new** | **~10,330** | | |
| **Total deleted** | **~200** | | |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Representation IR & Decision Framework | `section-01-repr-ir.md` | Not Started |
| 02 | Transitive Triviality & ARC Elision | `section-02-transitive-triviality.md` | Not Started |
| 03 | Value Range Analysis Framework | `section-03-range-analysis.md` | Not Started |
| 04 | Integer Narrowing Pipeline | `section-04-integer-narrowing.md` | Not Started |
| 05 | Float Narrowing Pipeline | `section-05-float-narrowing.md` | Not Started |
| 06 | Struct & Tuple Layout Optimization | `section-06-struct-layout.md` | Not Started |
| 07 | Enum Representation Optimization | `section-07-enum-repr.md` | Not Started |
| 08 | Escape Analysis & Stack Promotion | `section-08-escape-analysis.md` | Not Started |
| 09 | ARC Header Compression | `section-09-arc-header.md` | Not Started |
| 10 | Thread-Local Non-Atomic ARC | `section-10-thread-local-arc.md` | Not Started |
| 11 | Collection Specialization | `section-11-collection-spec.md` | Not Started |
| 12 | Verification & Benchmarks | `section-12-verification.md` | Not Started |
