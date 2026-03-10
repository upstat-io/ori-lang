---
plan: "aims"
title: "AIMS — ARC Intelligent Memory System: Exhaustive Implementation Plan"
status: not-started
reviewed: true  # 2026-03-10
references:
  - "docs/compiler/design/09-arc-system/index.md"
  - "docs/ori_lang/v2026/spec/21-memory-model.md"
  - ".claude/rules/arc.md"
---

# AIMS — ARC Intelligent Memory System: Exhaustive Implementation Plan

## Mission

Replace `ori_arc`'s sequential analysis passes (derived ownership, liveness,
uniqueness/COW annotation, RC insertion, reset/reuse detection, expansion,
RC identity propagation, RC elimination, drop hints) with a
single unified ownership analysis based on a formally-grounded lattice. AIMS
fuses these into one coherent system where all dimensions reinforce each other
— producing equal or fewer RC operations, in fewer compilation passes, with a
cleaner architecture. FBIP enforcement remains a separate read-only diagnostic
pass running on the final IR.

## Architecture

AIMS operates as a single memory-intelligence abstract interpreter with three
pipeline phases: **create opportunities** (Phase A), **prove opportunities** (Phase B),
**realize opportunities** (Phase C). These phases are distinct from the implementation
stages (Stage 1-5) in the Implementation Sequence below.

```
CanExpr
  │
  ▼ (lower — KEEP)
ArcFunction (no RC ops, no var_reprs)
  │
  ▼ (compute_var_reprs — KEEP)
ArcFunction + var_reprs + ValueRepr
  │
  ╔══════════════════════════════════════════════════════════════╗
  ║  Phase A: OPPORTUNITY CREATION (NEW — pre-analysis)         ║
  ║                                                              ║
  ║  Normalize IR into forms that expose reuse and tail-context  ║
  ║  structure before ARC analysis runs:                         ║
  ║    → TRMC normalization (self-recursive constructor contexts)║
  ║    → constructor-context metadata extraction                 ║
  ║    → collection mutation canonicalization                    ║
  ║  (Impl Stage 3 rollout — not required for initial AIMS-core)║
  ╚══════════════════════════════════════════════════════════════╝
  │
  ╔══════════════════════════════════════════════════════════════╗
  ║  Phase B: OPPORTUNITY PROVING (NEW — replaces analysis)     ║
  ║                                                              ║
  ║  Interprocedural fixed-point (SCC-based):                    ║
  ║    → MemoryContract per function (access, consumption,       ║
  ║      uniqueness, demand, locality, effects, FIP)             ║
  ║                                                              ║
  ║  Intraprocedural analysis (per-function):                    ║
  ║    → unified AimsState per (block-boundary, var)            ║
  ║      (per-instruction states re-derived during emission)    ║
  ║    → 7 dimensions: access, consumption, cardinality,         ║
  ║      uniqueness, locality, shape, effect                     ║
  ║    → single backward dataflow pass (seq_add + alt_join)      ║
  ║    → convergence produces complete AimsStateMap              ║
  ╚══════════════════════════════════════════════════════════════╝
  │
  ╔══════════════════════════════════════════════════════════════╗
  ║  Phase C: OPPORTUNITY REALIZATION (NEW — replaces emission) ║
  ║                                                              ║
  ║  Reads converged state, writes IR:                           ║
  ║    → call-site arg ownership (Apply.arg_ownership)           ║
  ║      (ArcParam.ownership is set in Phase B interprocedural, ║
  ║       step 2 — before per-function analysis)                ║
  ║    → RC emission (RcInc/RcDec at optimal points)            ║
  ║    → Reuse emission (Reset/Reuse/IsShared)                  ║
  ║    → COW annotations (StaticUnique/Dynamic/StaticShared)    ║
  ║    → Drop hints (unique-collection optimization)             ║
  ║    → FIP certification artifacts                             ║
  ║    → (future) locality / stack-allocation hints              ║
  ╚══════════════════════════════════════════════════════════════╝
  │
  ▼
ArcFunction (with RC ops, reuse ops)
  │
  ▼ (verify, tail_call, block_merge — KEEP)
  │
  ▼ (COW annotations, drop hints — NEW, post-merge)
  │
  ▼ (verify — KEEP)
ArcFunction (post-cleanup)
  │
  ▼ (fbip enforcement — KEEP, separate read-only diagnostic)
ArcFunction (final)
  │
  ▼ (ori_llvm ArcIrEmitter — UNCHANGED)
LLVM IR
```

The crucial principle:

> **No optimization gets its own independent source of truth.** If an optimization
> needs to know that a value is unique, local, single-use, and shape-compatible,
> those facts must all come from the same `AimsStateMap` and `MemoryContract`.

## Design Principles

### 1. Analysis and Emission Are Separate

The current pipeline interleaves analysis and IR mutation: `rc_insert` modifies the
IR, `reset_reuse` re-analyzes the modified IR with a fresh dominator tree and
recomputed liveness, `expand_reuse` modifies it again. AIMS separates these concerns:
analysis is pure (produces a state map), emission is a single pass that reads the map
and writes RC/reuse operations. No intermediate IR mutation, no wasted re-analysis.

**Motivated by:** The current pipeline rebuilds dominator and post-dominator trees
after RC insertion because edge cleanup can split edges and append trampoline blocks.
Liveness is computed twice (once before RC insertion, once after). Each rebuild is
wasted work.

### 2. One Lattice, One Truth

The current passes maintain independent data structures (`AnnotatedSig`,
`UniquenessSummary`, `LiveSet`, `CowAnnotations`, `DropHints`, `DerivedOwnership`)
that encode overlapping information about the same variables. AIMS uses a single
product lattice per variable at each program point. Information that was lost at
pass boundaries (e.g., "this value is unique AND borrowed AND at its last use")
is now jointly available.

**Motivated by:** RC elimination cannot undo conservative decisions made by
borrow inference. Uniqueness analysis cannot improve borrow decisions already
committed. Information walls between passes prevent cross-optimization.

### 3. Formally Grounded

The unified lattice is justified by established theory: Perceus's linear resource
calculus (RC ops = structural rules of linear logic), GHC's demand analysis
(cardinality inference), and Lean 4's borrow inference (SCC-based ownership).
These aren't ad-hoc engineering choices but projections of a single mathematical
framework onto different dimensions.

**Motivated by:** Correctness confidence. Each current pass has its own invariants
that must be manually kept in sync. A single formally-grounded lattice has one
invariant to maintain.

## Section Dependency Graph

```
  ┌──────────┐
  │ 01 Lattice│ ◄── Everything depends on the lattice design
  └─────┬─────┘
        │
   ┌────┴────┐
   ▼         ▼
┌──────┐  ┌──────┐
│02 Int-│  │03 Int-│
│ra-proc│  │er-proc│   02 and 03 are mostly independent
└───┬───┘  └───┬───┘   (03 produces MemoryContract, 02 consumes it)
    │          │
    └────┬─────┘
         ▼
   ┌───────────┐
   │04 RC Emit  │
   │05 Reuse    │   04 and 05 depend on converged analysis
   │   Emit     │
   └─────┬─────┘
         ▼
   ┌───────────┐
   │06 Pipeline │   Wires everything together
   └─────┬─────┘
         │
    ┌────┴────┐
    ▼         ▼
┌──────┐  ┌──────┐
│07 Adv-│  │08 Ver-│   Independent: advanced opts and verification
│anced │  │ify    │
└──────┘  └──────┘
```

- Sections 02 and 03 can be developed in parallel (03 produces `MemoryContract` per
  function, 02 consumes them at call sites — develop with mock contracts initially).
- Sections 04 and 05 require the analysis core (01 + 02 + 03).
- Section 06 requires 04 + 05 to wire into the pipeline.
- Sections 07 and 08 can proceed once 06 is stable.

**Cross-section interactions (must be co-implemented):**
- **Section 02 + Section 03**: The intraprocedural analysis consumes interprocedural
  contracts. If `MemoryContract` changes shape, both must update together.
- **Section 04 + Section 05**: RC emission and reuse emission read the same state map.
  They must agree on what "consumed" and "reusable" mean.
- **Section 04 + Section 06**: RC emission must populate `arg_ownership` on Apply/Invoke
  instructions AND `ArcParam.ownership` on functions. These are consumed by the LLVM
  emitter. If AIMS changes the representation, Section 06 must update the LLVM consumer.

**Critical sync points (must stay in sync with LLVM emitter):**
- `ArcFunction.cow_annotations` — consumed by `emitter_utils.rs` (keyed by `(block_idx, instr_idx)`)
- `ArcFunction.drop_hints` — consumed by `rc_ops.rs` (keyed by `(block_idx, instr_idx)`)
- `ArcFunction.var_reprs` — consumed throughout the emitter (indexed by `ArcVarId`)
- `Apply.arg_ownership` / `Invoke.arg_ownership` — consumed by RC emission in the emitter
- `ArcParam.ownership` — consumed by function prologue emission
- `RcStrategy` on `RcInc`/`RcDec` — consumed by `rc_ops.rs` for dispatch

## Implementation Sequence

```
Stage 1 — Make AIMS-core real and replace the current pass stack
  └─ Phase 0: Foundation
       └─ 01: Define AimsState lattice (7 dimensions), join, transfer functions
       └─ 01: Unit tests for lattice properties (idempotence, monotonicity, etc.)
  └─ Phase 1: Analysis Core
       └─ 02: Intraprocedural backward dataflow (with mock contracts)
       └─ 03: Interprocedural SCC fixed-point (produces MemoryContract)
       └─ 02: Connect intraprocedural to real contracts from 03
       Gate: analysis produces correct AimsState for hand-verified test cases

  └─ Stage 1A: Shadow Analysis
       └─ Run old pipeline as today
       └─ Run AIMS analysis in shadow mode alongside
       └─ Compare ONLY artifacts the old pipeline already computes:
            ArcParam.ownership, Apply/Invoke.arg_ownership,
            return uniqueness (MemoryContract → UniquenessSummary),
            cow_annotations
            (cardinality is AIMS-only — validated via AIMS-internal
             tests only; see Section 02.7 validation corpus)
       Gate: all lattice/property tests green, golden corpus green,
             diff harness shows no unexplained regressions in
             compared artifacts

  └─ Stage 1B: Metadata Cutover
       └─ Replace metadata producers: ArcParam.ownership, Apply.arg_ownership,
          Invoke.arg_ownership, ArcFunction.cow_annotations
       └─ KEEP old RC insertion/elimination and old reset/reuse
       Gate: LLVM emitter works correctly with AIMS-derived metadata,
             ./test-all.sh green

  └─ Stage 1C: RC Cutover
       └─ 04: Replace rc_insert, rc_identity, rc_elim with AIMS RC emission
       └─ Temporarily KEEP old reuse detection/expansion if needed
       Gate: emitted ArcFunctions pass ori_arc::verify, behavioral equivalence
             on full test suite, no meaningful RC regressions (exact per-test
             RC improvement is NOT required — correctness first)

  └─ Stage 1D: Reuse Cutover
       └─ 05: Replace reset_reuse, expand_reuse with AIMS reuse emission
       └─ Old analysis/emission stack fully gated off
       Gate: ./test-all.sh green with full AIMS pipeline, RC count ≤ old pipeline

  Deliverable: old ARC pipeline replaced by AIMS for standard code paths

  Stage 1 scope exclusions (NOT on the critical path):
  - FipContract inference (all functions get FipContract::Never in Stage 1)
  - TRMC normalization (normalize_function returns no-op in Stage 1)
  - Locality hint realization (Locality dimension exists but is conservative)
  - New CollectionReuse creation (existing CollectionReuse preserved, no new ones)
  - ShapeClass and EffectClass precision (conservative defaults acceptable)

Stage 2 — Add FIP-capable contracts
  └─ 03: Extend MemoryContract with FipContract
  └─ 05: Teach reuse emission to emit exact reuse fast paths where
          FIP preconditions hold
  └─ 08: Add verification counters for allocation-free execution
  Deliverable: AIMS can certify some functions as conditionally or fully in-place

Stage 3 — Add constrained TRMC normalization
  └─ NEW: aims/normalize/ — self-recursive constructor-context rewrites only
  └─ Transformed regions produce internal context metadata
  └─ Analysis reads normalized structure; no new public language feature
  
  └─ Scope bounds (v1):
       - Self-recursive functions only (no mutual recursion)
       - One recursive call per transformed region
       - Recursive call beneath a constructor or field context
       - No effectful instructions between context capture and fill
       - No polymorphic unknown-layout contexts
       - Source spans and debugability preserved
  Deliverable: more opportunities for tail-call lowering, reuse, and FIP certification

Stage 4 — Add locality realization hints
  └─ Use Locality facts to produce backend hints for stack or local allocation
  └─ Keep hint-based first; do not redesign ARC IR around stack allocation yet
  Deliverable: LLVM may consume locality hints in a later plan without changing AIMS-core

Stage 5 — Representation and runtime follow-ons (separate efforts)
  └─ 07: Representation optimization using AIMS shape/locality facts
  └─ 07: Immortal objects, SCC-based frozen-cycle RC
  └─ 07: Concurrent runtime strategies
  These should NOT block the AIMS-core replacement.
```

**Why this order:**
- Stage 1 is the core replacement — everything depends on it working end-to-end.
- Stage 2 adds FIP as a contract (not just a diagnostic), strengthening the analysis.
- Stage 3 creates better opportunities via TRMC before analysis, not after.
- Stage 4 uses locality facts already in the lattice, just adds realization hints.
- Stage 5 is independent follow-on work that reads AIMS facts.

## Metrics (Current State)

| Module | Production LOC | Test LOC | Total |
|--------|---------------|----------|-------|
| `ori_arc/` (total) | ~24,000 | ~18,000 | ~42,000 |
| — `borrow/` | ~1,050 | ~1,710 | ~2,760 |
| — `liveness/` | ~330 | ~835 | ~1,165 |
| — `rc_insert/` | ~1,420 | ~2,620 | ~4,040 |
| — `rc_elim/` | ~820 | ~1,770 | ~2,590 |
| — `rc_identity/` | ~230 | ~420 | ~650 |
| — `uniqueness/` | ~1,520 | ~2,050 | ~3,570 |
| — `reset_reuse/` | ~515 | ~800 | ~1,315 |
| — `expand_reuse/` | ~695 | ~710 | ~1,405 |
| — `fbip/` | ~300 | ~350 | ~650 |
| — `drop/` | ~420 | ~720 | ~1,140 |
| **Analysis passes (to replace)** | **~7,300** | **~12,000** | **~19,300** |

> **Note:** `ownership/` (~93 prod, ~123 test) defines data types (`Ownership`,
> `DerivedOwnership`, `AnnotatedSig`) shared across passes. These types are retained
> during migration (`MemoryContract → AnnotatedSig` conversion in Section 03) and
> may be removed once all consumers migrate to `MemoryContract`. `drop/` computes
> per-type drop info consumed by LLVM codegen — it remains independent of AIMS.

## Estimated Effort

> **File size compliance:** Sections 01 (~800), 02 (~1,200), 03 (~900), 04 (~1,100), and 05 (~800) all
> exceed the 500-line limit per file. Each section's detail file specifies the submodule split plan.
> The `aims/` module tree should be:
> 
> ```
> aims/
> ├── mod.rs              — dispatch hub, pub re-exports
> ├── normalize/          — Stage 3: opportunity creation (TRMC, context extraction)
> │   ├── mod.rs          — normalize_function() entry point
> │   ├── trmc.rs         — TRMC-eligible recursion detection + rewrite
> │   ├── context.rs      — constructor-context metadata extraction
> │   └── collections.rs  — collection mutation canonicalization
> ├── lattice.rs          — AimsState (7 dimensions), join (~350 lines)
> ├── transfer.rs         — transfer functions per ArcInstr/ArcTerminator (~300 lines)
> ├── contract.rs         — MemoryContract, ParamContract, FipContract (~250 lines)
> ├── intraprocedural/    — backward dataflow (6 files, ~1,200 lines total)
> │   ├── mod.rs          — analyze_function() entry point, worklist loop
> │   ├── state_map.rs    — AimsStateMap data structure
> │   ├── block.rs        — per-block backward analysis
> │   ├── merge.rs        — control flow join handling
> │   ├── pattern.rs      — pattern match scrutinee/binding analysis
> │   └── events.rs       — sparse event tracking (context holes, FIP gates)
> ├── interprocedural.rs  — SCC fixed-point loop (~300 lines)
> ├── builtins.rs         — builtin function MemoryContract mappings (~300 lines)
> ├── emit_rc/            — RC emission (5 files, ~1,100 lines total)
> │   ├── mod.rs          — emit_rc_ops() entry point
> │   ├── boundaries.rs   — function entry/exit/call-site RC
> │   ├── arg_ownership.rs — emit_arg_ownership()
> │   ├── cow.rs          — COW annotation computation
> │   └── drop_hints.rs   — drop hint computation
> ├── emit_reuse/         — reuse emission (4 files, ~800 lines total)
> │   ├── mod.rs          — emit_reuse() entry point
> │   ├── detect.rs       — find_reuse_opportunities() with cross-block detection
> │   ├── fip.rs          — FIP fast-path emission from FipContract
> │   └── fbip.rs         — FBIP metadata enrichment (additive, not replacement)
> └── verify/             — comparison tooling for Section 08
>     ├── mod.rs          — verify entry point
>     └── compare.rs      — old-vs-new pipeline comparison
> ```

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 Lattice Design | ~800 | High | — |
| 02 Intraprocedural | ~1,200 | High | 01 |
| 03 Interprocedural | ~900 | High | 01 |
| 04 RC Emission | ~1,100 | Medium-High | 01, 02, 03 |
| 05 Reuse Emission | ~800 | Medium | 01, 02, 03 |
| 06 Pipeline Integration | ~400 | Medium | 04, 05 |
| 07 Advanced Optimizations | ~500 | Medium | 06 |
| 08 Verification | ~500 | Low | 06 |
| normalize/ (Stage 3) | ~400 | Medium | 06 |
| **Total new** | **~6,600** | | |
| **Total replaced** | **~7,300** | | |

The unified analysis should be ~20% less code than the separate passes it replaces,
because shared infrastructure (lattice, traversal, state map) is not duplicated.

## Theoretical Foundations

AIMS draws on established PL research:

| Paper | Contribution to AIMS |
|-------|---------------------|
| **Perceus** (Reinking et al., PLDI 2021) | RC ops = structural rules of linear logic; garbage-free property |
| **FP²** (Lorenzen et al., ICFP 2023) | Reuse credits as first-class lattice element; FIP certification criterion |
| **Counting Immutable Beans** (Ullrich & de Moura, IFL 2019) | SCC-based borrow inference; reset/reuse |
| **Drop-Guided Reuse** (Lorenzen & Leijen, ICFP 2022) | Reuse after RC insertion (simpler, provably frame-limited) |
| **GHC Demand Analysis** (Sergey et al., POPL 2014) | Backward cardinality inference: {Absent, Once, Many} |
| **Substructural Interpretation** (Chirimar et al., JFP 1996) | RC = computational interpretation of linear logic |
| **Linearity ≠ Uniqueness** (Marshall et al., ESOP 2022) | Linearity (future) and uniqueness (past) are distinct dimensions |
| **Quantitative Type Theory** (Atkey, LICS 2018) | Semiring-graded usage annotations |
| **Oxidizing OCaml** (Lorenzen et al., ICFP 2024) | Modal memory management: affinity, uniqueness, locality as mode axes; safe stack allocation and in-place update. Justifies AIMS `Locality` dimension. [DOI: 10.1145/3674642](https://doi.org/10.1145/3674642) |
| **FIPTree** (Lorenzen et al., PLDI 2024) | First-class constructor contexts for O(1) top-down algorithms; compiler-generated context metadata for in-place update. Justifies AIMS opportunity-creation stage. [DOI: 10.1145/3656398](https://doi.org/10.1145/3656398) |
| **TRMC** (Leijen & Lorenzen, JFP 2025) | Tail recursion modulo context: equational approach with context laws; Perceus heap semantics. Justifies AIMS pre-analysis normalization. [DOI: 10.1017/S0956796825100117](https://doi.org/10.1017/S0956796825100117) |
| **Exploring Perceus for OCaml** (Pinto & Leijen, ML Workshop 2023) | Evaluation methodology: same compiler, same source, only switch memory-management backend. AIMS Section 08 default evaluation doctrine. |
| **Double-Ended Bit-Stealing** (Elsman, ICFP 2024) | ADT representation using both low and high pointer bits; up to 26% benchmark speedup. Future representation optimizer consuming AIMS shape/locality facts. [DOI: 10.1145/3674628](https://doi.org/10.1145/3674628) |

## Codebase Hygiene Status

The following files in `ori_arc` were scanned against hygiene rules. Files that will be
touched by AIMS but are currently clean:

| File | Lines | Status |
|------|-------|--------|
| `pipeline.rs` | 258 | Clean (7-param function is pre-existing; fix in AIMS) |
| `lib.rs` | 182 | Clean (trait default methods in lib.rs are acceptable) |
| `borrow/mod.rs` | 131 | Clean (no `#[allow]` or `#[expect]` issues) |
| `liveness/mod.rs` | 331 | Clean |
| `rc_insert/mod.rs` | 231 | Clean |
| `rc_insert/block_rc.rs` | 396 | Clean |
| `rc_insert/annotate.rs` | 250 | Clean |
| `rc_insert/edge_cleanup.rs` | 328 | Clean |
| `rc_elim/eliminate.rs` | 439 | Clean (approaching limit; will be removed) |
| `rc_elim/mod.rs` | 378 | Clean |
| `uniqueness/mod.rs` | 163 | Clean |
| `uniqueness/inter/mod.rs` | 282 | Clean |
| `uniqueness/intra/mod.rs` | 269 | Clean |
| `ownership/mod.rs` | 94 | Clean |
| `ir/mod.rs` | 431 | STYLE: 2 stale section refs (line 194 "Section 06.2", line 303 "Section 09"); approaching limit |
| `ir/instr.rs` | 396 | STYLE: 10+ stale section refs throughout method/type docs (Sections 07, 07.1, 07.6, 08, 09) |
| `graph/mod.rs` | 200 | Clean |
| `graph/call_graph/mod.rs` | 158 | Clean |
| `graph/scc/mod.rs` | 203 | Clean |
| `reset_reuse/mod.rs` | 308 | Clean |
| `reset_reuse/cross_block.rs` | 207 | Clean |
| `expand_reuse/mod.rs` | 326 | Clean |
| `fbip/mod.rs` | 301 | Clean |
| `drop/mod.rs` | 420 | Clean (approaching limit; retained by AIMS) |
| `borrow/builtins/mod.rs` | 267 | Clean |

**No BLOAT (>500 lines)** in production files. No decorative banners. No commented-out code.
No bare `#[allow(clippy)]`. One properly-formatted TODO. Overall: codebase is well-maintained.

**Findings summary:** 0 BLOAT. 0 WASTE. 0 DRIFT. 0 LEAK. **9 STYLE** (stale doc-comment
section references — see fix-along-the-way items in Section 06.3). **1 GAP** (Section 04
references non-existent `split_critical_edges`; actual function is `insert_edge_cleanup`,
`pub(super)` — corrected in Section 04.1).

The stale section references use an old internal numbering scheme that predates the
current pipeline documentation. Since AIMS replaces or removes most of these files,
the preferred fix is to update the doc comments when touching each file during
Stage 1C/1D cutover. Note: `ir/instr.rs` has stale references throughout 10+ doc
comments (not just line 1), and `ir/mod.rs` has 2 additional stale references not
previously tracked.

Files with stale section references:
| File | Stale Reference | Should Be |
|------|----------------|-----------|
| `ir/instr.rs:5,17,74,88,106,110,165,200,258,341` | "Section 07", "Section 07.1", "Section 07.6", "Section 08", "Section 09" throughout module doc, type doc, and method docs | Current pipeline pass names (rc_insert, rc_elim, reset_reuse, expand_reuse, liveness) |
| `ir/mod.rs:194,303` | "Section 06.2" on `ArcParam` doc, "Section 09" on `substitute_var` doc | Current pipeline pass names (borrow, expand_reuse) |
| `rc_elim/mod.rs:1,10,12` | "Section 08", "Section 09" | Current pipeline pass names |
| `reset_reuse/mod.rs:1` | "Section 07.6" | Current pipeline pass names |
| `expand_reuse/mod.rs:1` | "Section 09" | Current pipeline pass names |
| `uniqueness/intra/mod.rs:1` | "Section 07.2" | Current pipeline pass names |
| `uniqueness/inter/mod.rs:1` | "Section 07.3" | Current pipeline pass names |
| `drop/mod.rs:1` | "Section 07.4" | Current pipeline pass names |
| `graph/call_graph/mod.rs:9` | "Section 12" | Current pipeline module names |

(The `#[allow]` → `#[expect]` issue in `borrow/mod.rs` has been resolved.
Check remaining enforcement crates at implementation time — see Section 06.3.)

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Unified Lattice Design | `section-01-lattice.md` | Not Started |
| 02 | Intraprocedural Analysis | `section-02-intraprocedural.md` | Not Started |
| 03 | Interprocedural Analysis | `section-03-interprocedural.md` | Not Started |
| 04 | RC Emission | `section-04-rc-emission.md` | Not Started |
| 05 | Reuse Emission | `section-05-reuse-emission.md` | Not Started |
| 06 | Pipeline Integration | `section-06-pipeline.md` | Not Started |
| 07 | Advanced Optimizations | `section-07-advanced.md` | Not Started |
| 08 | Verification & Validation | `section-08-verification.md` | Not Started |
