---
plan: "aims-literature-review"
title: "AIMS Literature Review: Invariants, Decomposition, and Evaluation Discipline"
status: complete
references:
  - "plans/aims/00-overview.md"
  - "plans/aims/section-01-lattice.md"
  - "plans/aims/section-09-dimensional-fusion.md"
  - "plans/aims/section-10-unified-realization.md"
  - "docs/compiler/design/09-arc-system/index.md"
---

# AIMS Literature Review: Invariants, Decomposition, and Evaluation Discipline

## Mission

Mine 12 papers and compiler sources for **invariants, decomposition choices, proof
obligations, and evaluation discipline** — not features. Each paper is reviewed against
the current AIMS codebase and plan to produce actionable items (changes to plan or code)
and non-actionable items (rejected with reasoning). The output sharpens AIMS's theoretical
foundations, identifies gaps in the lattice/transfer/realization design, and sets explicit
boundaries for what AIMS should *not* attempt.

## Review Discipline

Every paper review follows the same gated output format:

1. **What the paper is actually claiming** — core thesis, not feature list
2. **What AIMS should adopt** — invariants, decomposition choices, proof obligations
3. **What AIMS should explicitly not adopt** — with reasoning
4. **What changes belong in the plan** — specific AIMS plan sections/files to edit
5. **What changes belong in code later** — implementation items for after review
6. **What this paper changes about how we read the next one** — cumulative lens shift

Standard output categories per paper:

- **Keep**: what should be pulled into AIMS
- **Reject**: what is not appropriate for Ori
- **Plan edits**: exact AIMS plan sections/files to change
- **New invariants**: what must become an explicit rule
- **Open risk**: what the paper exposes as still weak

## Paper Ordering Rationale

The 12 papers are organized in three tiers:

### Tier 1: Architecture-Defining (Papers 1-5)

These papers challenge or validate fundamental AIMS design choices — lattice dimensions,
FIP certification, constructor contexts, TRMC normalization, evaluation methodology.

```
Paper 1 (OxCaml) → Paper 2 (FP²) → Paper 3 (FIPTree) → Paper 4 (TRMC) → Paper 5 (Perceus/OCaml)
  │                    │                  │                    │                    │
  │ mode axes,         │ FIP theorem      │ constructor        │ context laws,      │ evaluation
  │ locality           │ conditions       │ contexts           │ soundness          │ methodology
  └────────────────────┴──────────────────┴────────────────────┴────────────────────┘
```

### Tier 2: Theory-Tightening (Papers 6-9)

These papers refine AIMS's formal foundations — linearity vs uniqueness, resource
semirings, borrow monotonicity, demand algebra.

```
Paper 6 (Lin≠Uniq) → Paper 7 (QTT) → Paper 8 (Lean 4) → Paper 9 (GHC Demand)
  │                    │                  │                    │
  │ consumption vs     │ semiring         │ monotonicity       │ backward join
  │ uniqueness         │ grading          │ contracts          │ algebra
  └────────────────────┴──────────────────┴────────────────────┘
```

### Tier 3: Boundary-Setting (Papers 10-12)

These papers define what AIMS should *not* attempt yet, while preserving future
extension points.

```
Paper 10 (Concurrent RC) → Paper 11 (Cyclic RC) → Paper 12 (Bit-Stealing)
  │                          │                       │
  │ runtime boundary         │ frozen-cycle           │ repr downstream
  └──────────────────────────┴───────────────────────┘
```

## Pause Questions Per Paper

Each review must address paper-specific pause questions (documented in the section
files) that probe whether AIMS's current design holds up under the paper's claims.

## Section Dependency Graph

```
Papers are reviewed sequentially. Each review informs the next.

01 OxCaml ──► 02 FP² ──► 03 FIPTree ──► 04 TRMC ──► 05 Perceus/OCaml
                                                            │
06 Lin≠Uniq ◄──────────────────────────────────────────────┘
  │
  ▼
07 QTT ──► 08 Lean 4 ──► 09 GHC Demand
                                │
10 Concurrent RC ◄─────────────┘
  │
  ▼
11 Cyclic RC ──► 12 Bit-Stealing
```

Each paper carries a "cumulative lens" — insights from earlier papers that change
how we read later ones. This is tracked in the "Lens Shift" section of each review.

## Current AIMS State (Context for Reviewers)

### What exists
- 7D product lattice: AccessClass × Consumption × Cardinality × Uniqueness × Locality × ShapeClass × EffectClass
- Backward dataflow intraprocedural analysis
- SCC-based interprocedural fixed-point (MemoryContract per function)
- RC emission from state map
- Reuse emission (detect + plan + expand)
- COW annotations and drop hints as post-merge passes
- Shadow comparison pipeline (aims-shadow feature)

### What is planned but not built (Stage 2)
- Dimensional fusion (Section 09): transfer-level cross-talk, active locality/effect/shape
- Unified realization (Section 10): single realize() pass
- Integration verification (Section 11): cross-dimension tests
- TRMC normalization (Stage 3)
- Locality realization (Stage 4)

### Key files
- `compiler/ori_arc/src/aims/lattice/mod.rs` — AimsState product lattice
- `compiler/ori_arc/src/aims/transfer/mod.rs` — transfer functions
- `compiler/ori_arc/src/aims/intraprocedural/mod.rs` — backward dataflow
- `compiler/ori_arc/src/aims/interprocedural.rs` — SCC fixed-point
- `compiler/ori_arc/src/aims/emit_rc/mod.rs` — RC emission
- `compiler/ori_arc/src/aims/emit_reuse/mod.rs` — reuse emission
- `plans/aims/00-overview.md` — AIMS plan overview

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Oxidizing OCaml (Modal Memory) | `section-01-oxidizing-ocaml.md` | Complete |
| 02 | FP² (Fully in-Place) | `section-02-fp2.md` | Complete |
| 03 | FIPTree (Constructor Contexts) | `section-03-fiptree.md` | Complete |
| 04 | TRMC (Tail Recursion Modulo Context) | `section-04-trmc.md` | Complete |
| 05 | Perceus for OCaml (Evaluation Methodology) | `section-05-perceus-ocaml.md` | Complete |
| 06 | Linearity and Uniqueness | `section-06-linearity-uniqueness.md` | Complete |
| 07 | Quantitative Type Theory | `section-07-quantitative-type-theory.md` | Complete |
| 08 | Lean 4 Borrow Inference | `section-08-lean4-borrow.md` | Complete |
| 09 | GHC Demand Analysis | `section-09-ghc-demand.md` | Complete |
| 10 | Concurrent Immediate RC | `section-10-concurrent-rc.md` | Complete |
| 11 | Cyclic RC for Immutable Data | `section-11-cyclic-rc.md` | Complete |
| 12 | Double-Ended Bit-Stealing | `section-12-bit-stealing.md` | Complete |

---

## Unification Analysis

This section verifies that all 12 reviews integrate as a coherent whole. Each check
corresponds to a specific cross-section concern.

### Check A: Cumulative Lens Chain Verification

Each section's Lens Shift must form a coherent chain where later papers build on
insights from earlier ones, not contradict them. The chain is verified below.

**01 -> 02**: OxCaml establishes mode decomposition (affinity/uniqueness/locality as
independent axes). Section 02 is read asking: "Does FP2 conflate uniqueness and
consumption in its reuse credits?" AIMS correctly separates them. The lens shift
focuses on reading FP2's reuse credits through the mode-decomposition lens. **Coherent.**

**02 -> 03**: FP2 establishes allocation balance as a theorem (|S|=|S'|). Section 03
is read asking: "What does AIMS need beyond allocation balance to handle constructor
contexts?" The answer maps to `ShapeClass::ContextHole` and Stage 3 normalization.
The prior lens (FP2's FIP containment hierarchy) is used to evaluate FIPTree's claims.
**Coherent.**

**03 -> 04**: FIPTree reveals that TRMC is not the whole story (dual-context patterns,
context-as-return-value). Section 04 is read asking: "Does the equational approach
cover multi-accumulator patterns, or is that a separate extension?" The lens correctly
narrows focus to equational laws rather than just optimization. **Coherent.**

**04 -> 05**: TRMC establishes "law before optimization" and Perceus heap semantics as
a proof framework. Section 05 is read asking: "Does evaluation methodology account for
TRMC-style structural rewrites (not just RC count)?" The lens correctly shifts focus
from algorithm to methodology. **Coherent.**

**05 -> 06**: Perceus/OCaml establishes isolation discipline (same-compiler, feature-flag).
Section 06 is read asking: "Can the linearity/uniqueness improvement be isolated to a
single mechanism via feature-flag comparison?" The lens from Section 05 (skepticism
toward cross-system benchmarks) is correctly applied. **Coherent.**

**06 -> 07**: Marshall et al. sharpen the linearity/uniqueness boundary. Section 07 is
read asking: "QTT quantities encode FUTURE demand only, not uniqueness." The lens
correctly predicts that QTT will inform `Cardinality` (not `Uniqueness`), which is
confirmed. **Coherent.**

**07 -> 08**: QTT establishes semiring structure for usage tracking. Section 08 is read
asking three questions: whether Lean has explicit semiring structure, whether
interprocedural composition is multiplicative, and whether Lean distinguishes
sequential/alternative composition. The lens correctly frames what to look for in
Lean's implementation. **Coherent.**

**08 -> 09**: Lean's source analysis reveals that cardinality (not just access class)
does the heavy lifting. Section 09 is read asking about `seq_add`/`alt_join` edge
cases in loops and exceptional edges. The lens correctly shifts from Lean's
two-category (owned/borrowed) to GHC's richer cardinality domain. **Coherent.**

**09 -> 10**: GHC confirms the demand algebra is correct. Section 10 is read asking:
"Does concurrent execution change the composition algebra?" The answer (from CIRC
Section 10.1): no, the demand algebra is unchanged; only the RC implementation
changes. **Coherent.**

**10 -> 11**: CIRC establishes the runtime boundary (counted/uncounted split). Section
11 is read asking: "Does SCC-frozen cycle handling require deferred decrements?"
The lens from Section 10 (reject deferred decrements for Drop ordering) correctly
constrains how cycle support should be designed. **Coherent.**

**11 -> 12**: Cyclic RC establishes header-layout preservation constraints. Section 12
is read asking: "Do bit-stealing representation changes extend into the RC header?"
The lens correctly identifies that bit-stealing is downstream of both AIMS and RC,
not feeding back. **Coherent.**

**Overall chain verdict**: The 12-section lens chain is coherent. Each section's lens
shift accurately predicts what to look for in the next paper, and no later section
contradicts an earlier lens. The cumulative lens at Section 12 (AIMS is a fact-proving
system; data flow is one-directional; lattice is stable at 7D/height-15) is a
natural culmination of the chain.

### Check B: Master Keep Registry

All Keep items across the 12 sections, compiled for cross-reference. Sections 01-05,
08, 10, 11 use K-prefixed notation; Sections 06, 07, 09, 12 use numbered notation.

| Section | ID | Summary |
|---------|-----|---------|
| 01 | K1 | `global` modality forces BOTH uniqueness AND locality |
| 01 | K2 | Locality as first-class axis with own sub-moding |
| 01 | K3 | Closure capture is where mode information concentrates |
| 01 | K4 | Borrowing derived from locality + uniqueness, not primitive |
| 01 | K5 | Deep mode property (locality propagates through projections) |
| 02 | K1 | Allocation balance as derived property of converged state |
| 02 | K2 | Token-level tracking by constructor arity |
| 02 | K3 | Per-branch token linearity (not probability weighting) |
| 02 | K4 | FIP requires no deallocation, not just no allocation |
| 02 | K5 | `FipContract::Conditional` with `requires_unique_params` |
| 02 | K6 | Atoms as zero-credit constructors |
| 02 | K7 | FIP/FBIP/lambda-fip containment validates AIMS architecture |
| 03 | K1 | Context representation as Minamide tuple with runtime path |
| 03 | K2 | Context laws (identity, associativity, distributivity) as proof obligations |
| 03 | K3 | FIP check as reuse credit accounting |
| 03 | K4 | Dual accumulator pattern (multi-context accumulation) |
| 03 | K5 | Bounded FIP contract from allocation balance (compiler-inferred) |
| 04 | K1 | Two context laws (appctx, appcomp) as explicit proof obligations |
| 04 | K2 | "Law before optimization" principle |
| 04 | K3 | Uniqueness as precondition for in-place TRMC |
| 04 | K4 | Lifting transformation as separate pre-pass |
| 04 | K5 | Hybrid path as architecture (not afterthought) for non-linear effects |
| 04 | K6 | Defunctionalized contexts as reuse sweet spot |
| 05 | K1 | Same-compiler feature-flag isolation (AIMS already does this) |
| 05 | K2 | Behavioral equivalence as non-negotiable hard gate |
| 05 | K3 | Multiple optimization tiers measured independently |
| 05 | K4 | Peak memory (RSS) as tracked metric |
| 05 | K5 | Mean-of-N-runs with explicit hardware context |
| 05 | K6 | Benchmarks must be allocation-intensive |
| 06 | 1 | Existing dimension separation (Consumption/Cardinality vs Uniqueness) correct |
| 06 | 2 | `is_rc_inc_elidable` / `is_rc_dec_unnecessary` correctly factored |
| 06 | 3 | `transfer_project` correctly preserves uniqueness through borrows |
| 06 | 4 | `Consumption` ordering correct for Ori's semantics |
| 07 | 1 | Semiring perspective on Cardinality already present and correct |
| 07 | 2 | Backward analysis correctly uses `seq_add` for sequential demand |
| 07 | 3 | `alt_join` at control-flow merge correct |
| 08 | K1 | Monotone-only set growth for ownership |
| 08 | K2 | Initialize parameters to borrow, promote to owned |
| 08 | K3 | Tail call preservation as post-inference fixup |
| 08 | K4 | Join-point bidirectional ownership propagation |
| 08 | K5 | Indirect calls and partial applications fully conservative |
| 08 | K6 | Scalar exclusion from borrow inference |
| 08 | K7 | Exported functions stay conservative |
| 09 | 1 | Three-operation discipline (lub/plus/mult, AIMS needs first two) |
| 09 | 2 | Algebraic specification as comments (GHC style) |
| 09 | 3 | Bottom-starting fixed-point for recursion |
| 09 | 4 | Separate treatment of alternative vs sequential in block analysis |
| 10 | K1 | `ori_rt` function-call boundary is correct abstraction point |
| 10 | K2 | `RcStrategy` correctly separates "what to RC" from "how to RC" |
| 10 | K3 | AIMS avoids embedding refcount-value assumptions |
| 10 | K4 | `single-threaded` feature flag is right mechanism for now |
| 10 | K5 | `drop_fn` parameter is recursive-reclamation hook |
| 11 | K1 | Do not hardcode acyclicity into RC header layout |
| 11 | K2 | Keep `ori_rc_inc`/`ori_rc_dec` as thin dispatch points |
| 11 | K3 | Preserve `drop_fn` callback architecture |
| 11 | K4 | Keep `ori_rc_is_unique` cycle-unaware |
| 11 | K5 | Preserve spare header capacity for future metadata |
| 12 | 1 | Constructor arity and payload classification for repr optimizer |
| 12 | 2 | Allocation-site hotness (Locality + Cardinality proxies) |
| 12 | 3 | Return uniqueness at type boundaries |
| 12 | 4 | Enum-variant-count metadata (type-level, not AIMS) |

**Cross-section consistency check**: No Keep items contradict each other. Key
reinforcements:

- 01.K1 (global forces aliased) reinforced by 06.Inv-3 (Uniqueness changes only from
  specific sources) -- heap-escaping values lose uniqueness through canonicalize, not
  through future-demand derivation.
- 02.K7 (FIP/FBIP/lambda-fip containment) reinforced by 07.1 (semiring validates
  cardinality structure) -- the lattice classification is algebraically grounded.
- 04.K2 (law before optimization) reinforced by 08.K1 (monotone-only set growth) --
  both demand formal foundations, not ad-hoc patterns.
- 10.K1 (runtime boundary) reinforced by 12.boundary-invariant (repr-opt consumes
  AIMS, never feeds back) -- the data flow is strictly one-directional.
- 03.K5 (bounded FIP, compiler-inferred) and 02.R5 (reject `fbip(n)` as programmer
  tier) are reconciled: Section 03 keeps the compiler-inferred `Bounded(n)` contract,
  Section 02 rejects the user-facing annotation. No conflict.

### Check C: Master Reject Registry with Contradiction Check

Critical Reject items that set boundaries, grouped by theme:

**Surface-language exposure (consistent "AIMS infers, never annotates"):**
- 01.R1: Reject mode annotations on types
- 02.R1: Reject separate FIP type system / syntax annotation
- 03.R1: Reject `fip` keyword as user-facing annotation
- 03.R4: Reject constructor context as first-class language feature
- 06.R1: Reject full substructural type system at surface level

**Implementation specifics from other systems (consistent "learn invariants, not code"):**
- 01.R4: Reject graded modal semiring formalization (complexity without benefit)
- 02.R2: Reject Koka's `AllocTree` structure
- 02.R3: Reject Koka's fractional probability weights
- 04.R1: Reject CPS/evaluation-context instantiation (allocates closures)
- 04.R3: Reject Koka-specific effect monadic pipeline
- 08.R1: Reject single-dimension `OwnedSet` representation
- 08.R2: Reject forward-only borrow inference
- 08.R3: Reject `DerivedValInfo` parent-child tracking (AIMS uses `BorrowSource`)

**Premature runtime complexity (consistent "Stage 5, not now"):**
- 10.R1: Reject epoch-based reclamation
- 10.R2: Reject uncounted `Snapshot`-style references in IR
- 10.R3: Reject deferred-decrement buffering (violates Drop ordering)
- 10.R7: Reject biased RC in Stage 1-4 (header layout change)
- 10.R8: Reject per-object mode bits in Stage 1-4
- 11.R1-R5: Reject all cycle-detection machinery (no cycles in safe Ori)

**Representation in AIMS lattice (consistent "downstream, not analysis"):**
- 12.R1: Reject boxity inference inside AIMS
- 12.R3: Reject constructor-kind refinement in `ShapeClass`
- 12.R5: Reject unboxing as Consumption/Uniqueness refinement

**Contradiction check**: No contradictions found. All Reject items are consistent
with the principle that AIMS is a compiler-internal analysis that proves facts for
downstream consumption. The boundary between "AIMS territory" and "downstream
territory" is consistently drawn at the same line across all 12 sections.

### Check D: Unified Invariant Registry

All invariants declared across the 12 sections, with their enforcement location.

**Lattice/Canonicalize Invariants:**

| Source | ID | Invariant | Location |
|--------|-----|-----------|----------|
| 01 | I1 | HeapEscaping -> not Unique (unless container proven Unique) | `canonicalize()` |
| 01 | I2 | Borrowed implies scope-bounded locality (<= FunctionLocal) | `canonicalize()` |
| 01 | I4 | Locality propagates through projections (deepness) | `transfer_project` |
| 06 | Inv-1 | No transfer derives Uniqueness from Consumption/Cardinality alone | `transfer/mod.rs` design rule |
| 06 | Inv-2 | Consumption and Cardinality must agree on liveness (Dead <-> Absent) | `canonicalize()` |
| 06 | Inv-3 | Uniqueness changes only from: Construct, COW, rc_inc/sharing, join, contract, canonicalize Rule 4 | `transfer/mod.rs` design rule |

**FIP Certification Invariants:**

| Source | ID | Invariant | Location |
|--------|-----|-----------|----------|
| 02 | N1 | FIP Certification: may_allocate==false + missed_reuses==0 + recursion check + per-arm balance | `contract/mod.rs`, `interprocedural.rs` |
| 02 | N2 | Token Balance: sum(produced) == sum(consumed) for FIP functions | `verify/mod.rs` |
| 02 | N3 | Per-Arm Token Balance: each match arm individually balances credits | `verify/mod.rs` |
| 03 | I4 | Allocation credit balance per match arm | `intraprocedural/block.rs` |

**Context/TRMC Invariants:**

| Source | ID | Invariant | Location |
|--------|-----|-----------|----------|
| 03 | I1 | Context region purity (EffectClass::NONE between capture and fill) | `intraprocedural/mod.rs` |
| 03 | I2 | Context hole type compatibility | `normalize/trmc.rs` |
| 03 | I3 | Context uniqueness at all composition/application points | `Uniqueness` dimension |
| 04 | I1 | TRMC soundness = context laws + uniqueness, not pattern matching | `normalize/verify.rs` |
| 04 | I2 | Context variable used linearly + no multi-shot handler resumption | `normalize/verify.rs` |
| 04 | I3 | Constructor context = unique linear chain (Lemma 2) | `Uniqueness` dimension |
| 04 | I4 | Lifting must precede TRMC detection | `normalize/lift.rs` |

**Interprocedural/SCC Invariants:**

| Source | ID | Invariant | Location |
|--------|-----|-----------|----------|
| 08 | I1 | Monotonicity: `all_sigs` never weakened after SCC completes | `interprocedural.rs` |
| 08 | I2 | Projection bidirectionality: backward demand promotes source to Owned | `transfer/mod.rs` |
| 08 | I3 | Modified-flag convergence: structural equality across iterations | `interprocedural.rs` |
| 08 | I4 | `ownArgsIfParam` heuristic non-adoption documented | `interprocedural.rs` |

**Demand Algebra Invariants:**

| Source | ID | Invariant | Location |
|--------|-----|-----------|----------|
| 07 | 1 | Document `seq_add`/`alt_join` as semiring operations | `dimensions.rs` docs |
| 07 | 2 | Test positivity: `a.seq_add(b) == Absent => a == Absent && b == Absent` | `lattice/tests.rs` |
| 07 | 3 | Test missing annihilation law (for hypothetical mult) | informational |
| 09 | 1 | Distributivity of `seq_add` over `alt_join` | `lattice/tests.rs` |
| 09 | 2 | Monotonicity of `seq_add` w.r.t. lattice order | `lattice/tests.rs` (proposed) |
| 09 | 3 | Conditional usage (Dead + Absent bidirectional sync) | `canonicalize()` |
| 09 | 4 | Loop back-edge demands must be monotone | `compute_block_exit_state` |

**Evaluation Invariants:**

| Source | ID | Invariant | Location |
|--------|-----|-----------|----------|
| 05 | N1 | Confounding-variable isolation (same commit, same profile, same LLVM) | `aims-compare.sh` |
| 05 | N2 | Optimization-tier tracking (attribute RC improvements to mechanisms) | `shadow/compare.rs` |
| 05 | N3 | Benchmark stability contract (golden corpus frozen) | Section 08 |
| 05 | N4 | Compilation-speed isolation (phase-level breakdown) | `aims_pipeline.rs` |
| 05 | N5 | Distinguish static from dynamic metrics | Section 08 |

**Runtime Boundary Invariants:**

| Source | ID | Invariant | Location |
|--------|-----|-----------|----------|
| 10 | N1 | AIMS must not assume synchronous reclamation ordering | `emit_reuse/` docs |
| 10 | N2 | `ori_rc_is_unique` returning true != "no other thread can see this" | `ori_rt` docs |
| 10 | N3 | `MAX_REFCOUNT` immortal sentinel compatible with any future RC scheme | `ori_rt` docs |
| 11 | I1 | Document acyclicity assumption in AIMS analysis | `aims/mod.rs` |
| 11 | I2 | Document drop-chain acyclicity in `ori_rt` | `ori_rt/rc/mod.rs` |
| 11 | I3 | Ori value semantics as primary cycle prevention mechanism | language-level |
| 12 | B1 | AIMS analysis is representation-agnostic (no boxity in lattice) | design rule |
| 12 | B2 | Repr-opt consumes AIMS, never feeds back (one-directional) | design rule |

**Cross-reference check**: All invariants referenced in later sections are traceable
to their origin section. Key cross-section invariant dependencies:

- Section 06.Inv-1 (no Uniqueness from Consumption alone) is referenced by Section 09
  discussion of fusion rules crossing the linearity/uniqueness boundary. **Verified.**
- Section 02.N1 (FIP certification conditions) is referenced by Section 03.K3 (FIP as
  reuse credit accounting) and Section 03.I4 (per-arm balance). **Verified.**
- Section 04.I1 (context laws) builds on Section 03.K2 (context laws as proof
  obligations). **Verified, consistent.**
- Section 10.N1 (no synchronous reclamation assumption) constrains Section 02's reuse
  tokens -- reuse tokens require sole counted ownership. **Verified, compatible.**
- Section 12.B2 (one-directional data flow) is the culmination of boundary invariants
  from Sections 10, 11. **Verified, consistent.**

### Check E: Plan Edits Consistency

All Plan Edits target specific AIMS plan sections. Verified no two sections propose
contradictory changes to the same target.

**`plans/aims/section-09-dimensional-fusion.md`** (most-targeted file):
- 01.PE: Add HeapEscaping->Unique ceiling, locality propagation through Project,
  Borrowed->FunctionLocal rule, closure-capture-aware locality
- 02.PE: Tighten FIP-natural detection, move FIP from canonicalize to contract extraction
- 03.PE: Enrich ContextHole metadata, expand `normalize/context.rs` scope
- 04.PE: Strengthen ContextHole TRMC candidacy requirements, add effect->TRMC gate
- 06.PE: Add Marshall et al. design invariant to Section 09.1
- 07.PE: Add semiring algebra preamble to Section 09.4
- 09.PE: Document `mult` non-adoption, strengthen block.rs docs

**Consistency verdict**: All proposed edits to Section 09 are additive (adding rules,
strengthening requirements, adding documentation). No two sections propose removing or
weakening the same rule. The FIP-related edits from 02 and 03 are complementary (02
adds deallocation tracking, 03 adds per-branch balance). The canonicalize rule edits
from 01 (add HeapEscaping ceiling) and 06 (add Marshall et al. gate) address different
rules and are compatible.

**`plans/aims/00-overview.md`**:
- 01.PE: Expand OxCaml entry in Research Lineage
- 02.PE: Update FP2 entry and Legacy Concept Collapse table
- 03.PE: Reframe Stage 3 as mandatory architecture
- 04.PE: Add proof obligations to Stage 3 scope, add "law before optimization" principle,
  expand normalize/ module tree
- 05.PE: (no direct edits)
- 11.PE: Expand Stage 5 scope with prerequisites
- 12.PE: Add boxity/repr bullets to Stage 4

**Consistency verdict**: All edits are additive to different subsections of the
overview. No conflicts.

**`plans/aims/section-03-interprocedural.md`**:
- 01.PE: Note `locality_bound` in ParamContract
- 08.PE: Add monotonicity documentation, projection bidirectionality, `ownArgsIfParam`
  non-adoption, tail-call scope clarification

**Consistency verdict**: Compatible -- 01 adds a field, 08 adds documentation about
existing behavior. No conflicts.

### Check F: Boundary-Setting Sections Respect Prior Decisions

Sections 10-12 (Tier 3: Boundary-Setting) must respect the architectural decisions
from Sections 01-09.

**Section 10 (Concurrent RC) respects:**
- 01-05 (Architecture): K3 confirms AIMS avoids embedding refcount assumptions, which
  is consistent with the lattice being abstract (01.K2 locality as first-class, 06.Inv-1
  no Uniqueness from Consumption). The runtime boundary (K1) preserves the one-directional
  data flow established by 04.K2 (law before optimization) and 05.K1 (feature-flag isolation).
- 06-09 (Theory): Section 10.1 explicitly answers Section 09's lens question ("Does
  concurrent execution change the demand algebra?") with "No." This confirms that
  `seq_add`/`alt_join` (validated in 07 and 09) are thread-model-independent.

**Section 11 (Cyclic RC) respects:**
- 01-05 (Architecture): K1 (don't hardcode acyclicity) preserves future extensibility
  without contaminating AIMS core. I1-I3 document the acyclicity assumption that the
  current lattice relies on, consistent with AIMS being a "fact-proving system" (12.L1).
- 06-09 (Theory): The backward dataflow convergence argument (lattice height, not graph
  acyclicity) is explicitly noted as sound even in the presence of cycles (I1). This is
  consistent with 09.Inv-4 (loop back-edge demands monotone) and 07's semiring structure.
- 10 (Concurrent RC): L1 checks compatibility with immediate-decrement invariant from
  Section 10. L3 identifies `pop_edges()`/`drop_fn` as shared primitive. **Consistent.**

**Section 12 (Bit-Stealing) respects:**
- 01-05 (Architecture): The "AIMS is representation-agnostic" boundary invariant (B1)
  is the strongest statement of AIMS's scope. It follows directly from 02.K7 (lattice
  classifies functions into FIP/FBIP/lambda-fip fragments without needing repr info),
  03.R2 (reject header encoding details), and 04.R4 (reject runtime context-path bits).
- 06-09 (Theory): The one-directional data flow (B2) is consistent with 06's design
  invariant (no Uniqueness from Consumption alone) and 08's monotonicity invariant
  (contracts never weakened). All information flows forward from analysis to consumers.
- 10-11 (Boundary): L4 explicitly coordinates with Section 10 (tag-bit budgets for
  CIRC vs bit-stealing). Section 11's header-layout preservation (K5) is compatible
  with bit-stealing targeting data pointers (not RC headers).

**Overall boundary verdict**: Sections 10-12 consistently treat AIMS as a producer of
facts and the runtime/repr as consumers. No boundary-setting section introduces
requirements that would force changes to the AIMS lattice, transfer functions, or
convergence properties. The only feedback mechanisms are pre-AIMS (repr reclassification
before analysis, per 12) and post-AIMS (runtime API preservation, per 10-11), both
of which are explicitly documented as outside the AIMS analysis loop.

### Formatting Note: Keep Item Numbering Convention

Sections 01-05, 08, 10, 11 use K-prefixed notation (K1, K2, etc.) for Keep items and
I/N-prefixed notation for invariants. Sections 06, 07, 09, 12 use plain numbered
notation (1, 2, 3, etc.). Both are acceptable within the review discipline defined in
this overview (the output format specifies "Keep" as a category, not a numbering
scheme). The registry above normalizes by using section-qualified identifiers
(e.g., "06.1" or "09.3") for unambiguous cross-reference.

## Codebase Hygiene Audit

Audit of all codebase files referenced by the plan's Code Changes and Plan Edits
sections. 23 source files scanned; findings listed below. Each finding is woven into
the appropriate section file for "fix along the way" during implementation.

### Findings

- **[BLOAT]** `aims/emit_rc/mod.rs` (970 lines) — nearly 2x the 500-line limit.
  Should be split into submodules (e.g., extract `emit_block_rc` + helpers, phase A/B/C
  emission, ownership transfer detection). Woven into Section 06 (Linearity/Uniqueness,
  which has the existing `emit_rc/mod.rs` subsection).

- **[NOTE]** `aims/emit_reuse/mod.rs` (508 lines) — marginally over 500-line limit.
  Recently created; will grow as Stage 2+ features land. Proactive split at ~450 lines
  recommended when next touched. Woven into Section 10 (Concurrent RC, which has the
  existing `emit_reuse/` code changes subsection).

### Clean Files (no findings)

All other scanned files are within limits and well-structured:

| File | Lines | Status |
|------|-------|--------|
| `aims/lattice/mod.rs` | 411 | Clean |
| `aims/lattice/dimensions.rs` | 237 | Clean |
| `aims/transfer/mod.rs` | 461 | Clean |
| `aims/interprocedural.rs` | 413 | Clean |
| `aims/intraprocedural/mod.rs` | 308 | Clean |
| `aims/intraprocedural/block.rs` | 245 | Clean |
| `aims/intraprocedural/state_map.rs` | 428 | Clean |
| `aims/contract/mod.rs` | 420 | Clean |
| `aims/emit_reuse/fip.rs` | 115 | Clean |
| `aims/emit_reuse/detect.rs` | 251 | Clean |
| `aims/emit_reuse/dynamic.rs` | 318 | Clean |
| `aims/emit_reuse/planner.rs` | 157 | Clean |
| `aims/immortal/mod.rs` | 83 | Clean |
| `aims/mod.rs` | 27 | Clean |
| `pipeline/aims_pipeline.rs` | 249 | Clean |
| `pipeline/shadow.rs` | 355 | Clean |
| `pipeline/shadow/compare.rs` | 408 | Clean |
| `verify/mod.rs` | 375 | Clean |
| `ori_rt/src/rc/mod.rs` | 373 | Clean |
| `aims/emit_rc/coalesce/mod.rs` | 189 | Clean |
