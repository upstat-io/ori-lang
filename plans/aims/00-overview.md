---
plan: "aims"
title: "AIMS — ARC Intelligent Memory System: Exhaustive Implementation Plan"
status: in-progress
references:
  - "docs/compiler/design/09-arc-system/index.md"
  - "docs/ori_lang/v2026/spec/21-memory-model.md"
  - ".claude/rules/arc.md"
---

# AIMS — ARC Intelligent Memory System: Exhaustive Implementation Plan

## Thesis

> AIMS is not Ori's ARC optimizer; AIMS is Ori's memory semantics made executable
> as one unified analysis and realization system.

AIMS is not a collection of borrowed optimizations. It is one memory-intelligence
system with one fact model and many outputs. External research informs individual
dimensions, but AIMS is defined by the unification, not by the ingredients. The
novelty is the collapse of ownership, demand, uniqueness, locality, shape, and
effect into one abstract interpreter and one realization model.

There is no "COW pass" or "FIP pass" or "reuse pass." There is one analysis that
converges a 7-dimensional lattice, and one realization that reads the converged
state and emits all artifacts. COW mode, FIP certification, reuse tokens, RC
operations, and drop hints are projections — different views of the same proven
facts. If an optimization cannot be derived from `AimsStateMap` and
`MemoryContract` alone, it is not part of AIMS core.

## Mission

Build one unified memory intelligence system where all 7 analysis dimensions
(access, consumption, cardinality, uniqueness, locality, shape, effect) work as
one team — every dimension constrains, proves, or overrides at least one other.
COW, FIP, reuse, drop hints, and RC insertion are *outputs* of this one system's
reasoning, not separate subsystems with their own analysis logic. The system
produces equal or fewer RC operations than the legacy pipeline, in one analysis
pass and one realization pass, with a formally-grounded lattice.

**Stage 1 (complete):** Replaced `ori_arc`'s sequential analysis passes with the
AIMS unified lattice. 75% RC reduction on golden corpus, zero behavioral
regressions, zero Valgrind leaks. The 4 core dimensions (access, consumption,
cardinality, uniqueness) collaborate through canonicalize and cross-dimensional
emission decisions.

**Stage 2 (this revision):** Deepen the integration so all 7 dimensions are
active team members. Fuse transfer functions so dimensions read each other
during analysis. Merge emission passes into one realization step. COW, FIP,
reuse fall out of the converged state as views, not as separate computations.
FBIP enforcement remains a separate read-only diagnostic pass.

**Stage 3:** Create structural opportunities (TRMC normalization) that the
unified analysis can exploit — a prerequisite for FIP on recursive algorithms.

**Stage 4:** Realize locality facts as backend hints for stack allocation and
representation optimization (boxity inference, bit-stealing).

**Stage 5:** Complete the runtime with concurrent RC strategies, frozen-cycle
collection, and per-object atomicity modes — making AIMS's compile-time
intelligence effective across Ori's full concurrency model.

## AIMS Litmus Test

Every proposed optimization must answer these five questions:

1. **What dimensions does it read?** (e.g., uniqueness + locality + cardinality)
2. **What dimensions does it refine?** (e.g., tightens uniqueness from MaybeShared to Unique)
3. **What prior standalone analysis does it eliminate?** (e.g., replaces separate COW uniqueness pass)
4. **Can it be derived from `AimsStateMap` + `MemoryContract` alone?**
5. **If not, it is not part of AIMS core.** It belongs in a separate post-analysis pass (like FBIP enforcement) or in a pre-analysis normalization (like TRMC).

This test prevents AIMS from accumulating bolted-on passes that happen to share
a data structure but don't participate in the unified analysis.

## What Is Actually Unified Today

| Layer | Status | Notes |
|-------|--------|-------|
| **Lattice** | Unified | All 7 dimensions in one `AimsState` product lattice |
| **Transfer functions** | Unified | One backward pass updates all dimensions per instruction |
| **Interprocedural contracts** | Unified | One `MemoryContract` per function, one SCC fixpoint |
| **Canonicalize (cross-dim)** | Partially unified | 3 rules active; 5 more designed (Section 09) |
| **RC emission** | Separate pass | Reads state map but has own traversal |
| **Reuse emission** | Separate pass | Reads state map but has own traversal + detection scan |
| **COW annotations** | Separate post-pass | Runs after merge_blocks with own traversal |
| **Drop hints** | Separate post-pass | Runs after merge_blocks with own traversal |
| **FIP classification** | Contract layer | `MemoryContract.fip` owned by interprocedural; `FipContract::Never` for all in Stage 1; Stage 2 makes it precise via `extract_contract()` reading converged state |

**Summary:** Analysis is unified now. Realization is still partially split.
FIP classification is already in the right place (contract layer) — it needs
precision, not relocation. Section 09 activates the remaining dimensions so
all 7 participate in reasoning. Section 10 removes the remaining output-pass
boundaries so emission is one realization, not four traversals.

## Legacy Concept Collapse Table

AIMS does not combine existing passes — it replaces them with dimensional facts:

| Legacy Concept | AIMS Replacement | Dimensions Used |
|---------------|-----------------|-----------------|
| Borrow inference | `AccessClass` + interprocedural `ParamContract` | access, consumption |
| Liveness analysis | `Cardinality` + `Consumption` | cardinality, consumption |
| Uniqueness analysis | `Uniqueness` dimension (+ locality/cardinality proof) | uniqueness, locality, cardinality |
| Reuse eligibility | `ShapeClass` + `Uniqueness` + `Consumption` | shape, uniqueness, consumption |
| COW mode | Derived view of converged state | uniqueness, access, consumption |
| Drop hints | Derived view of converged state | uniqueness, shape |
| FIP certification | Derived from `EffectSummary.may_allocate` + `missed_reuses` + recursion check | effect, shape, consumption (token balance), uniqueness (Conditional preconditions) |
| RC identity normalization | Eliminated — no separate pass needed | access, cardinality |
| RC elimination | Eliminated — precise placement avoids redundant pairs | consumption, cardinality |

The left column no longer exists as separate code. The right column is what AIMS
computes in its single analysis pass. No legacy concept has its own traversal,
its own state, or its own decision procedure.

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

The unified lattice is justified by established theory — RC ops as structural
rules of linear logic, backward cardinality inference, SCC-based ownership
propagation. These aren't ad-hoc engineering choices but projections of a single
mathematical framework onto different dimensions. AIMS is defined by the
unification, not by the individual ingredients.

**Motivated by:** Correctness confidence. Each current pass has its own invariants
that must be manually kept in sync. A single formally-grounded lattice has one
invariant to maintain.

See [Research Lineage](#research-lineage) for the specific papers that inform
each dimension.

### 4. Law Before Optimization

Every rewrite in `aims/normalize/` (Stage 3 opportunity creation) must follow the
equational approach: (a) define a correctness specification, (b) identify the
algebraic laws the specification requires, (c) prove the concrete instantiation
satisfies those laws. This principle, drawn from Leijen & Lorenzen (JFP 2025),
prevents accumulating ad-hoc rewrites that work on known examples but lack
soundness arguments.
(See: [Literature Review §04 — TRMC](../aims-literature-review/section-04-trmc.md))

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
└───┬──┘  └──────┘
    │
    ▼
┌──────────┐
│09 Dimen- │   Requires all Stage 1 work (01-08)
│sional    │
│Fusion    │
└────┬─────┘
     ▼
┌──────────┐
│10 Unified│   Requires 09 (richer state to read)
│Realize   │
└────┬─────┘
     ▼
┌──────────┐
│11 Integ- │   Requires 09+10 (proves the integration works)
│ration    │
│Verify    │
└────┬─────┘
     ▼
┌──────────┐
│12 FIP    │   Requires 09+10+11 (FIP proof obligations + enforcement)
│Enforce-  │
│ment      │
└────┬─────┘
     ▼
┌──────────┐
│13 TRMC   │   Requires 09+10+11+12 (end-to-end TRMC realization)
│Realize   │
└──────────┘
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
- **Section 09 + `ParamContract`**: Locality Activation (09.2) adds `locality_bound` to
  `ParamContract`. This touches: `aims/contract/mod.rs`, `aims/interprocedural.rs`
  (extract_contract), `aims/builtins/mod.rs` (builtin defaults), `verify/mod.rs`.
  All five locations must be updated in the same commit. See Section 09.2 sync note.
- **Section 10 + edge cleanup**: `realize_rc_reuse()` (Phase 1) must call `emit_edge_cleanup()`
  at the end of its forward walk — the same edge cleanup that currently lives inside
  `emit_rc_ops()`. This must not be lost during the refactor. See Section 10.1.
- **Section 10 + COW/drop hints**: `realize_annotations()` (Phase 2) runs AFTER
  `merge_blocks()` and uses ArcVarId-keyed state lookups. This ordering is load-bearing
  (same constraint as steps 11a/12 in the current pipeline). See Section 10.1 architecture.
- **Section 10 + arg_ownership**: `emit_arg_ownership()` (current step 4) disposition
  must be decided before implementing `realize()`. See Section 10.1 disposition note.

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

  Stage 1→2 Transition Gate:
  Before beginning Stage 2 work, ALL of the following must be true:
  1. `./test-all.sh` green (old pipeline unchanged, AIMS default)
  2. `cargo test --workspace --features aims` green (zero failures)
  3. `cargo test -p ori_llvm --features aims` green (all AOT tests pass)
  4. Valgrind: 0 definite runtime leaks on all `tests/valgrind/` + `tests/aims/` programs
  5. RC count ≤ old pipeline for ALL golden corpus programs (not just net improvement)
  6. RC count ≤ old pipeline for ALL `tests/benchmarks/` programs
  7. `aims-shadow` comparison shows zero regressions on param/return/cow/arg_ownership
  8. Compilation speed within 10% of old pipeline on all representative programs
  9. Section 08 exit criteria fully met (behavioral equivalence + safety verification)
  10. Old pipeline code is still compilable (not deleted) — it remains the fallback
      until Stage 2 exits successfully.

  aims-shadow Feature Retirement Plan:
  The `aims-shadow` feature (runs both pipelines and compares results) is a
  **Stage 1 verification tool**. Its retirement follows this sequence:
  1. **Stage 1 (current):** `aims-shadow` is actively used for regression detection.
     Shadow comparison runs automatically in CI and ad-hoc via
     `diagnostics/aims-compare.sh`. All 5 comparison dimensions active.
  2. **Stage 1→2 transition:** After the Stage 1→2 gate passes, `aims-shadow` is
     demoted from CI-required to on-demand diagnostic. The feature flag remains
     compilable but is no longer run in standard CI.
  3. **Stage 2 completion:** After Section 11 exit criteria are met (integration
     verified, synergy metrics established, regression guards in place), the
     `aims-shadow` feature is deleted:
     - Remove `aims-shadow` feature from `compiler/ori_arc/Cargo.toml`
     - Delete `compiler/ori_arc/src/pipeline/shadow/` directory (mod.rs, compare.rs, tests.rs)
     - Remove `aims-shadow` references from `CLAUDE.md`, `.claude/rules/arc.md`,
       `.claude/rules/cargo.md`
     - Remove shadow-related code in `pipeline/mod.rs` (the dispatch branch)
     - Update `diagnostics/aims-compare.sh` to remove shadow mode references
  4. **Post-retirement:** The `aims` feature flag itself is also retired — AIMS
     becomes the only pipeline. Remove the feature flag, delete legacy pipeline
     code (`borrow/`, `liveness/`, `rc_insert/`, `rc_elim/`, `rc_identity/`,
     `uniqueness/`, `reset_reuse/`, `expand_reuse/`), update `run_arc_pipeline()`
     to call AIMS directly without feature dispatch. This is the final cleanup
     after Stage 2 is proven correct.

  Stage 1 scope exclusions (NOT on the critical path):
  - FipContract inference (all functions get FipContract::Never in Stage 1)
  - TRMC normalization (normalize_function returns no-op in Stage 1)
  - Locality hint realization (Locality dimension exists but is conservative)
  - New CollectionReuse creation (existing CollectionReuse preserved, no new ones)
  - ShapeClass and EffectClass precision (conservative defaults acceptable)

Stage 2 — Dimensional Fusion (one team, not separate analyses)
  └─ 09: Dimensional Fusion
       └─ 09.1: Transfer-level cross-talk (dimensions read each other during transfer)
       └─ 09.2: Active dimensions (dependency ladder: locality → effect → shape)
       └─ 09.3: Enriched canonicalize (8+ cross-dimension invariant rules, up from 3)
       └─ 09.4: Sequencing algebra extension (document/extend seq_add/alt_join)
       └─ 09.5: Convergence feedback (multi-round canonicalize, cross-dim tightening)
  └─ 10: Unified Realization
       └─ 10.1: Single realize() pass replaces emit_rc + emit_reuse + cow + drop_hints
       └─ 10.2: Per-instruction decide() reads one AimsState, makes all decisions
       └─ 10.3: COW/reuse/drop_hints as views of converged state, not separate logic
              (FIP stays in contract layer; realization consumes it, emits evidence)
  └─ 11: Integration Verification
       └─ 11.1: Cross-dimension test programs (programs only solvable by 2+ dimensions)
       └─ 11.2: Synergy metrics (quantify cross-dimensional contribution)
       └─ 11.3: Regression guards (removing any rule regresses measurably)
  Gate: every dimension influences at least one other; ≥20% of RC decisions
        require 2+ dimensions; golden corpus RC ≤ Stage 1; compilation speed ≤ 10% regression
  Deliverable: one system where COW, reuse, RC, drop hints are realization outputs
               and FIP classification falls out of contract extraction reading converged
               effect + locality state. No separate FIP pass; no separate emission passes.

  Stage 2 Exit Gate (must ALL be true before proceeding to Stage 2.5):
  1. All Section 09 exit criteria met (cross-dimension interactions ≥12)
  2. All Section 10 exit criteria met (two-phase realize, output equivalence)
  3. All Section 11 exit criteria met (synergy metrics ≥20%, regression guards)
  4. `aims-shadow` feature retired (see retirement plan in Stage 1→2 transition)
  5. `aims` feature flag retired — AIMS is the sole pipeline
  6. Legacy pipeline code deleted (borrow/, liveness/, rc_insert/, rc_elim/,
     rc_identity/, uniqueness/, reset_reuse/, expand_reuse/ — ~7,300 lines)
  7. `run_arc_pipeline()` calls AIMS directly without feature dispatch
  8. `./test-all.sh` green (now always uses AIMS, no feature flag needed)
  9. Valgrind: 0 definite runtime leaks on all test programs

Stage 2.5 — FIP Proof Obligations & Enforcement
  Completes the FIP certification story. Without this, `FipContract::Certified`
  is a metadata label, not a proven property. FP² Theorem 2 requires no
  allocation, no deallocation, AND constant stack space — Section 09.2
  only checks the first.
  └─ 12: FIP Proof Obligations & Enforcement
       └─ 12.1: Add `may_deallocate` to `EffectSummary` (FP² Theorem 2)
              + post-emission pipeline update from FipEvidence.missed_reuses
       └─ 12.2: Constant stack verification (`has_unbounded_stack`)
              + syntactic tail-position helper extraction from tail_call/mod.rs
       └─ 12.3: FIP enforcement verifier (`verify_fip_contract()`)
              + aims/verify/ module infrastructure (mod.rs + fip.rs)
       └─ 12.4: Stale documentation cleanup (contract/mod.rs banner)
  Gate: `FipContract::Certified` means provably no alloc, no dealloc,
        constant stack. Verifier rejects any contract/emission mismatch.

Stage 3 — Opportunity Creation (required for FIP on recursive algorithms)
  Stage 3 is not an optimization pass. It is a structural prerequisite. Without
  it, self-recursive constructor functions cannot be FIP or FBIP. Stage 2 makes
  the analysis *ready* to exploit contexts; Stage 3 creates the contexts to
  exploit.
  (See: [Literature Review §03 — FIPTree](../aims-literature-review/section-03-fiptree.md))
  └─ NEW: aims/normalize/ — self-recursive constructor-context rewrites only
  └─ Benefits from Stage 2: active shape dimension identifies ContextHole,
     active effect dimension verifies purity, active locality bounds scope
  └─ Scope bounds (v1):
       - Self-recursive functions only (no mutual recursion)
       - One recursive call per transformed region
       - Recursive call beneath a constructor or field context
       - No effectful instructions between context capture and fill
       - No polymorphic unknown-layout contexts
       - Source spans and debugability preserved
  └─ Proof obligations (from Leijen & Lorenzen, JFP 2025):
       - The chosen context instantiation must satisfy the two context laws
         `(appctx)` and `(appcomp)` for terminating expressions.
       - The context variable must be provably unique (AIMS `Uniqueness::Unique`)
         at every point between context creation (`ctx`) and application (`app`).
       - If the function's `EffectSummary.may_share == true`, in-place TRMC is
         unsound; fall back to non-in-place translation or skip TRMC.
       - A lifting sub-pass must run before TRMC detection to normalize
         expressions in constructor fields into let-bindings.
       (See: [Literature Review §04 — TRMC](../aims-literature-review/section-04-trmc.md))
  └─ 13: TRMC Realization & Soundness
       └─ 13.1: ContextBehavior interprocedural inference (replace default)
              + manual Default impl, context_regions threading to extract_contract
       └─ 13.2: Soundness gate reconciliation (may_share + uniqueness)
              + fixpoint edge case: first SCC iteration has no contract
       └─ 13.3: Lifting pre-pass (A-normal form for constructor args)
              + var_types extension for new variables
       └─ 13.4: 4-equation TRMC rewrite (base/tail/tlet/tmatch)
              + in-place transform vs auxiliary function decision
       └─ 13.5: Post-rewrite verification (context laws)
              + rollback via func.clone()
       └─ 13.6: Pipeline integration (event consumption, re-analysis)
              + normalize_function signature change to (&mut, Option<&MemoryContract>)
              + re-run compute_var_reprs + detect_immortals + analyze after transform
  Gate: `normalize_function()` produces rewritten functions for self-recursive
        constructor-context patterns. Both soundness gates enforced.
        ContextOpen/ContextClose events consumed by realization.
  Deliverable: FIP/FBIP eligibility for self-recursive constructor functions,
               reuse opportunities for top-down tree algorithms, tail-call
               lowering for constructor-context patterns

Stage 4 — Locality Realization + Representation
  └─ Use Locality facts to produce backend hints for stack or local allocation
  └─ Representation optimization consuming AIMS shape/locality facts
  └─ Boxity inference (Elsman ICFP 2024) as a pre-pipeline or post-analysis
     pass consuming type-level constructor metadata + AIMS uniqueness/locality facts
  └─ Reclassification feedback: repr optimizer may reclassify unboxed ADTs as
     `ArcClass::Scalar` in a second pipeline run, or via a pre-AIMS repr pass
     that adjusts `compute_var_reprs` output
  └─ Platform-specific tag-bit constants (H=16 on x86_64, alignment bits) belong
     in repr optimizer config, not in AIMS
  └─ Keep hint-based first; do not redesign ARC IR around stack allocation yet
  (See: [Literature Review §12 — Bit-Stealing](../aims-literature-review/section-12-bit-stealing.md))
  Deliverable: LLVM may consume locality hints, representation optimizer has data

Stage 5 — Runtime Intelligence
  Completes the AIMS vision by extending compile-time memory intelligence into
  the runtime. Without Stage 5, AIMS optimizes single-threaded RC but cannot
  reason about concurrency or cycles — incomplete relative to Ori's full
  concurrency and cycle-safety ambitions.

  └─ SCC-based frozen-cycle RC
       Prerequisite: A `freeze` operation or equivalent language feature that
       transitions mutable object graphs to deeply immutable.
       Prerequisite: Ori must have a mechanism for constructing cyclic graphs
       (currently impossible in safe code — no interior mutability, no `Weak`
       refs, no unsafe pointer cycles).
       These prerequisites are language features that must be designed and
       implemented before this work can begin.
       Paper contribution (Parkinson et al., ISMM 2024): SCC detection +
       union-find at freeze time; RC lifted to SCC granularity so cycles within
       a frozen SCC never leak. Only applicable after both prerequisites are met.
       (See: [Literature Review §11 — Cyclic RC](../aims-literature-review/section-11-cyclic-rc.md))
  └─ Concurrent runtime strategies: implement CIRC-style counted/uncounted
     split for `Sendable` channel-based concurrency. Requires: (a) shared-heap
     access model defined, (b) `ori_rt` RC API boundary preserved (see Section
     07.4.1), (c) EBR guard emission in LLVM codegen. Does not require
     changes to `ori_arc` analysis or AIMS lattice — consumes AIMS facts.
     (See: [Literature Review §10 — Concurrent Immediate RC](../aims-literature-review/section-10-concurrent-rc.md))
  └─ Concurrent RC progression (each sub-stage independently valuable and deployable):
       └─ Stage 5a: Lean 4-style per-object mode bits (sign-bit encoding in
          `m_rc`). No EBR, no epoch machinery. Objects default to non-atomic;
          flip to atomic when sent through a `Sendable` channel. Requires:
          header layout decision, `ori_rc_inc`/`ori_rc_dec` branch on mode,
          LLVM emitter emits "mark shared" at channel send sites.
       └─ Stage 5b: Biased RC (dual counters, owner-thread fast path). Requires:
          24-byte header, per-object owner tracking, deallocation protocol when
          both counters reach zero.
       └─ Stage 5c: CIRC-style EBR integration (only if lock-free data structures
          are added to the standard library). Requires: EBR guard emission,
          `Snapshot` type in the runtime, fundamental API changes.
  Deliverable: Ori's runtime handles concurrent RC, frozen-cycle collection,
               and per-object atomicity — AIMS compile-time facts drive runtime
               strategy selection
```

**Why this order:**
- Stage 1 is the core replacement — everything depends on it working end-to-end.
- Stage 2 deepens integration — all 7 dimensions become one team, emission unifies.
  FIP certification falls out of this integration (not bolted on as a separate pass).
- Stage 3 is a structural prerequisite for FIP on recursive algorithms. Without it,
  self-recursive constructor functions cannot be FIP or FBIP — no amount of dimensional
  fusion in Stage 2 can recover what normalization provides. Benefits from Stage 2
  because active shape/effect/locality dimensions identify TRMC candidates naturally.
- Stage 4 uses locality facts already proven precise in Stage 2, adds backend hints.
- Stage 5 completes the system — runtime concurrency and cycle handling make
  AIMS's compile-time intelligence effective across Ori's full execution model.
  Sequenced last because it has language-level prerequisites (freeze, cyclic
  graphs, shared-heap model) that must be designed first, and because Stages 1-4
  deliver value independently. But Stage 5 is committed work, not optional.

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
> The `aims/` module tree (actual, as of Stage 1 completion):
>
> ```
> aims/
> ├── mod.rs              — dispatch hub, pub re-exports
> ├── builtins/           — builtin function MemoryContract mappings
> │   ├── mod.rs          — seed_builtin_contracts()
> │   └── tests.rs
> ├── contract/           — MemoryContract, ParamContract, FipContract
> │   ├── mod.rs          — contract types + join + conversion helpers
> │   └── tests.rs
> ├── emit_rc/            — RC emission (7 files)
> │   ├── mod.rs          — emit_rc_ops() entry point
> │   ├── arg_ownership.rs — emit_arg_ownership()
> │   ├── cow.rs          — COW annotation computation
> │   ├── drop_hints.rs   — drop hint computation
> │   ├── edge_cleanup.rs — per-edge RcDec for variables dead on specific CFG edges
> │   ├── coalesce/       — static RC coalescing peephole pass
> │   │   ├── mod.rs      — coalesce adjacent RcInc/RcDec within blocks
> │   │   └── tests.rs
> │   └── tests.rs
> ├── emit_reuse/         — reuse emission (5 files)
> │   ├── mod.rs          — emit_reuse() entry point + ReuseOpportunity types
> │   ├── detect.rs       — find_reuse_opportunities() with cross-block detection
> │   ├── dynamic.rs      — MaybeShared → IsShared + Branch CFG expansion
> │   ├── fip.rs          — FIP gate records + FipGateDecision
> │   ├── planner.rs      — cross-block reuse planner (dominator/post-dominator validation)
> │   └── tests.rs
> ├── immortal/           — heap-allocated constant detection (skip RC for immortals)
> │   ├── mod.rs          — detect_immortals()
> │   └── tests.rs
> ├── interprocedural.rs  — SCC fixed-point loop (analyze_program)
> ├── interprocedural/
> │   └── tests.rs
> ├── intraprocedural/    — backward dataflow (3 implementation files)
> │   ├── mod.rs          — analyze_function() entry point, worklist loop
> │   ├── block.rs        — per-block backward analysis (exits, terminators, instructions)
> │   ├── state_map.rs    — AimsStateMap data structure + AimsEvent enum
> │   ├── state_map/
> │   │   └── tests.rs
> │   └── tests.rs
> ├── lattice/            — AimsState (7 dimensions), join, SizeClass, EffectClass
> │   ├── mod.rs          — AimsState product lattice + EffectClass + SizeClass + BorrowSource
> │   ├── dimensions.rs   — AccessClass, Consumption, Cardinality, Uniqueness, Locality, ShapeClass
> │   └── tests.rs
> └── transfer/           — transfer functions per ArcInstr/ArcTerminator
>     ├── mod.rs          — DefTransfer, UseTransfer, transfer_def, transfer_use
>     └── tests.rs
> ```
>
> **Future modules (not yet created):**
> ```
> aims/
> ├── normalize/          — Stage 3: opportunity creation (TRMC, context extraction)
> │   ├── mod.rs          — normalize_function() entry point
> │   ├── detect.rs       — TRMC-eligible recursion detection (context region extraction)
> │   ├── lift.rs         — lifting: extract expressions from ctor fields to let-bindings
> │   ├── rewrite.rs      — TRMC rewrite: apply the 4-equation algorithm
> │   ├── verify.rs       — verify context laws (appctx, appcomp) post-rewrite
> │   ├── collections.rs  — collection mutation canonicalization
> │   ├── context/        — constructor-context metadata extraction
> │   └── tests.rs        — normalization tests
> ├── verify/             — Post-realization verification passes
> │   ├── mod.rs          — verification dispatch
> │   └── fip.rs          — FIP contract vs emission consistency checks
> └── realize/            — Stage 2: unified realization (replaces emit_rc + emit_reuse)
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
| 09 Dimensional Fusion | ~800 | High | 01-07 |
|   ↳ Transfer fusion rules | ~200 | High | — |
|   ↳ Active dimensions | ~300 | High | — |
|   ↳ Enriched canonicalize | ~150 | Medium | — |
|   ↳ Convergence feedback | ~150 | Medium | — |
| 10 Unified Realization | ~600 | Medium-High | 09 |
|   ↳ realize() + decide() | ~400 | Medium-High | — |
|   ↳ Output views | ~200 | Medium | — |
| 11 Integration Verification | ~400 | Medium | 09, 10 |
|   ↳ Test programs | ~200 | Low | — |
|   ↳ Synergy metrics | ~100 | Low | — |
|   ↳ Regression guards | ~100 | Low | — |
| 12 FIP Enforcement | ~400 | Medium | 09, 10, 11 |
|   ↳ may_deallocate + stack | ~200 | Medium | — |
|   ↳ Verifier | ~150 | Medium | — |
|   ↳ Cleanup | ~50 | Low | — |
| 13 TRMC Realization | ~600 | High | 09, 10, 11, 12 |
|   ↳ ContextBehavior + soundness | ~150 | Medium | — |
|   ↳ Lifting + 4-equation rewrite | ~300 | High | — |
|   ↳ Verification + pipeline | ~150 | Medium | — |
| **Total new (Stage 1)** | **~6,600** | | |
| **Total new (Stage 2)** | **~1,800** | | |
| **Total new (Stage 2.5)** | **~400** | | |
| **Total new (Stage 3)** | **~600** | | |
| **Total replaced** | **~7,300** | | |

The unified analysis should be ~20% less code than the separate passes it replaces,
because shared infrastructure (lattice, traversal, state map) is not duplicated.

## Research Lineage

AIMS is its own system — the unification is the contribution, not any individual
ingredient. The following papers informed specific dimensions. They are listed
for rigor and traceability, not as the story of AIMS:

| Paper | Contribution to AIMS |
|-------|---------------------|
| **Perceus** (Reinking et al., PLDI 2021) | RC ops = structural rules of linear logic; garbage-free property |
| **FP²** (Lorenzen et al., ICFP 2023) | Reuse credits as first-class lattice element; FIP certification criterion. Theorem 2 (`|S|=|S'|`) establishes the proof obligation for FIP: every deallocation must be matched by a reuse (token balance). FIP/FBIP containment validates that lattice-derived classification is consistent with the formal hierarchy. Two embeddings — static uniqueness (unique bindings use `(dconru_h)` fast path, no RC check) and dynamic RC (`dropru` with runtime uniqueness test) — map directly to AIMS's `Unique` and `MaybeShared` paths respectively. (See: [Literature Review §02 — FP²](../aims-literature-review/section-02-fp2.md)) |
| **Counting Immutable Beans** (Ullrich & de Moura, IFL 2019) | SCC-based borrow inference; reset/reuse |
| **Drop-Guided Reuse** (Lorenzen & Leijen, ICFP 2022) | Reuse after RC insertion (simpler, provably frame-limited) |
| **GHC Demand Analysis** (Sergey et al., POPL 2014) | Backward cardinality inference: {Absent, Once, Many} |
| **Substructural Interpretation** (Chirimar et al., JFP 1996) | RC = computational interpretation of linear logic |
| **Linearity ≠ Uniqueness** (Marshall et al., ESOP 2022) | Linearity (future) and uniqueness (past) are distinct dimensions |
| **Quantitative Type Theory** (Atkey, LICS 2018) | QTT's 0-1-omega semiring provides the theoretical justification for AIMS Cardinality's algebraic structure: `(Cardinality, seq_add, Absent)` is a commutative monoid with absorbing element `Many`, directly analogous to QTT's resource semiring. `seq_add` corresponds to QTT's resource accumulation (+), combining usages along one execution path. `alt_join` corresponds to QTT's branch join (lub), combining usages from mutually exclusive paths. The distributivity of `seq_add` over `alt_join` is the key soundness property for fixed-point analysis over CFGs with diamonds, verified exhaustively in `lattice/tests.rs`. (See: [Literature Review §07 — QTT](../aims-literature-review/section-07-quantitative-type-theory.md)) |
| **Oxidizing OCaml** (Lorenzen et al., ICFP 2024) | Modal memory management: affinity, uniqueness, locality as independent mode axes with inference. Proves locality is **load-bearing for soundness** (not auxiliary) — the `global` modality forces both `aliased` AND `global`, establishing that heap-escaping values lose uniqueness guarantees. Locality enables safe stack allocation (90% allocation reduction) and borrowing soundness (`borrow` combinator requires `local` mode). AIMS's current treatment of locality as conservative/`Unknown` in Stage 1 is a deliberate deferral, not an architectural choice — Stage 2 must activate it. Justifies AIMS `Locality` dimension, `HeapEscaping -> not Unique` invariant, `Borrowed -> scope-bounded locality` invariant, and closure-capture-aware mode propagation. [DOI: 10.1145/3674642](https://doi.org/10.1145/3674642). (See: [Literature Review §01](../aims-literature-review/section-01-oxidizing-ocaml.md)) |
| **FIPTree** (Lorenzen et al., PLDI 2024) | First-class constructor contexts for O(1) top-down algorithms; compiler-generated context metadata for in-place update. Justifies AIMS opportunity-creation stage. [DOI: 10.1145/3656398](https://doi.org/10.1145/3656398) |
| **TRMC** (Leijen & Lorenzen, JFP 2025) | Tail recursion modulo context: equational approach with context laws; Perceus heap semantics. Justifies AIMS pre-analysis normalization. [DOI: 10.1017/S0956796825100117](https://doi.org/10.1017/S0956796825100117) |
| **Exploring Perceus for OCaml** (Pinto & Leijen, ML Workshop 2023) | Evaluation methodology: same compiler, same source, only switch memory-management backend. AIMS Section 08 default evaluation doctrine. |
| **Double-Ended Bit-Stealing** (Elsman, ICFP 2024) | ADT representation using both low and high pointer bits; up to 26% benchmark speedup. Future representation optimizer consuming AIMS shape/locality facts. [DOI: 10.1145/3674628](https://doi.org/10.1145/3674628) |

## Codebase Hygiene Status

The following files in `ori_arc` were scanned against hygiene rules. Files that will be
touched by AIMS but are currently clean:

| File | Lines | Status |
|------|-------|--------|
| `pipeline/mod.rs` | 258 | Clean (7-param function is pre-existing; fix in AIMS) |
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

**No BLOAT (>500 lines)** in OLD production files. No decorative banners. No commented-out code.
No bare `#[allow(clippy)]`. One properly-formatted TODO. Overall: codebase is well-maintained.

**Findings summary (old ori_arc files):** 0 BLOAT. 0 WASTE. 0 DRIFT. 0 LEAK. **9 STYLE**
(stale doc-comment section references — see fix-along-the-way items in Section 06.3).
**1 GAP** (Section 04 references non-existent `split_critical_edges`; actual function is
`insert_edge_cleanup`, `pub(super)` — corrected in Section 04.1).

**Findings summary (NEW aims/ implementation files, 2026-03-11 scan):**
**3 BLOAT. 0 WASTE. 0 DRIFT. 0 LEAK. 3 STYLE. 0 GAP.**

| File | Lines | Finding | Fix Location |
|------|-------|---------|--------------|
| `aims/emit_reuse/mod.rs` | 815 | **BLOAT** — 63% over 500-line limit | Section 05 cleanup checklist |
| `aims/lattice/mod.rs` | 548 | **BLOAT** — 10% over limit; partially addressed by extracting dimensions.rs | Section 06 cleanup checklist |
| `pipeline/shadow.rs` | 567 | **BLOAT** — 13% over limit; compare.rs and tests.rs extracted to shadow/ subdir | Section 06 cleanup checklist |
| `aims/emit_reuse/mod.rs` | 6 refs | **STYLE** — stale `§09.5` references | Section 05 cleanup checklist |
| `aims/emit_reuse/tests.rs` | 2 refs | **STYLE** — stale `§09.5` references | Section 05 cleanup checklist |
| Sections 02-06 frontmatter | — | **STYLE** — `status: complete` in frontmatter vs `Not Started` in body | Section 06 cleanup checklist |

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
| 01 | Unified Lattice Design | `section-01-lattice.md` | In Progress |
| 02 | Intraprocedural Analysis | `section-02-intraprocedural.md` | In Progress |
| 03 | Interprocedural Analysis | `section-03-interprocedural.md` | Complete |
| 04 | RC Emission | `section-04-rc-emission.md` | Complete |
| 05 | Reuse Emission | `section-05-reuse-emission.md` | Complete |
| 06 | Pipeline Integration | `section-06-pipeline.md` | Complete |
| 07 | Advanced Optimizations | `section-07-advanced.md` | Complete |
| 08 | Verification & Validation | `section-08-verification.md` | In Progress |
| 09 | Dimensional Fusion | `section-09-dimensional-fusion.md` | Complete |
| 10 | Unified Realization | `section-10-unified-realization.md` | Complete |
| 11 | Integration Verification | `section-11-integration-verification.md` | Complete |
| 12 | FIP Proof Obligations & Enforcement | `section-12-fip-enforcement.md` | Not Started |
| 13 | TRMC Realization & Soundness | `section-13-trmc-realization.md` | Not Started |
