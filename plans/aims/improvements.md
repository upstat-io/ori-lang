---
plan: "aims"
title: "AIMS Improvements: Coherent Research Integration"
status: integrated
reviewed: true
references:
  - "plans/aims/00-overview.md"
  - "plans/aims/section-01-lattice.md"
  - "plans/aims/section-02-intraprocedural.md"
  - "plans/aims/section-03-interprocedural.md"
  - "plans/aims/section-04-rc-emission.md"
  - "plans/aims/section-05-reuse-emission.md"
  - "plans/aims/section-06-pipeline.md"
  - "plans/aims/section-07-advanced.md"
  - "plans/aims/section-08-verification.md"
---

# AIMS Improvements: Coherent Research Integration

> **Historical Reference Document.** All changes proposed in this document have been
> integrated into the section files (section-01 through section-08) and the overview
> (00-overview.md). This document is retained as the original rationale and research
> survey. For the current plan, read the section files directly.

## Purpose

This document is a correction and expansion of the current AIMS plan. The goal
is to integrate the strongest relevant research ideas into **one coherent ARC
memory-intelligence system**, rather than accumulating independent optimizations.

The key architectural change is:

> **AIMS should be treated as a single memory-intelligence abstract interpreter
> with three stages: create opportunities, prove opportunities, realize opportunities.**

That means:

- `TRMC`, constructor contexts, and normalization belong to **opportunity creation**
- ownership, cardinality, uniqueness, locality, and effect facts belong to
  **opportunity proving**
- RC emission, reuse emission, COW elimination, drop hints, and later stack or
  layout decisions belong to **opportunity realization**

This is the cleanest way to absorb the recent literature without making AIMS a
bag of unrelated tricks.

## Core Thesis

The current AIMS plan is already moving in the right direction: one lattice,
one state map, one emission system. However, it still treats some of the most
valuable ideas as optional add-ons:

- `TRMC` and constructor contexts are deferred to Section 07 as stretch goals
- modal memory management ideas are not yet reflected in the lattice
- "fully in-place" certification is referenced, but not built into the plan as
  a first-class output
- representation work is outside the architecture entirely

That is too weak if the branch goal is "novel ARC memory intelligence with
extreme performance."

The coherent version of AIMS should be:

1. **Opportunity Creation**
   - Normalize the IR into forms that expose reuse and tail-context structure.
   - Apply a constrained `TRMC` pass before ARC analysis.
   - Represent constructor-context opportunities explicitly enough that later
     phases can reason about them.

2. **Opportunity Proving**
   - Compute one `MemoryContract` per function and one `AimsStateMap` per
     function body.
   - The same facts drive RC placement, reuse, COW elimination, FIP
     certification, and later stack or layout hints.

3. **Opportunity Realization**
   - Emit only the RC and reuse operations justified by the proven facts.
   - Attach stable, final-layout metadata for LLVM.
   - Produce certification artifacts such as "fully in-place under these
     preconditions."

## Research Inputs

This section records the exact papers and the precise ideas worth integrating.
The intention is not to "copy papers into Ori", but to extract the parts that
strengthen the AIMS architecture.

### 1. Oxidizing OCaml with Modal Memory Management

- **Authors:** Anton Lorenzen, Leo White, Stephen Dolan, Richard A. Eisenberg,
  Sam Lindley
- **Venue:** Proceedings of the ACM on Programming Languages, Vol. 8, ICFP
- **Article:** 253
- **Pages:** 485-514
- **Published:** August 15, 2024
- **DOI:** `10.1145/3674642`
- **Source:** <https://doi.org/10.1145/3674642>
- **Exact extracted idea:**
  - The design centers on **three mode axes: affinity, uniqueness, and
    locality**.
  - Modes are **backwards compatible** and **fully inferable**.
  - The practical payoff is safe **stack allocation** and safe **in-place
    update** of immutable structures.
- **AIMS integration:**
  - Add a **locality dimension** to the AIMS fact domain.
  - Do **not** attempt full OxCaml surface syntax or type-system machinery.
  - Use the paper as the justification for tracking:
    - whether a value is heap-escaping
    - whether a value remains local to the current function
    - whether a value can be materialized with stack or local-allocation hints
  - This belongs in **Section 01 lattice** and **Section 03 interprocedural
    contracts**, not in a later ad hoc optimization pass.

### 2. FP2: Fully in-Place Functional Programming

- **Authors:** Anton Lorenzen, Daan Leijen, Wouter Swierstra
- **Venue:** Proceedings of the ACM on Programming Languages, Vol. 7, ICFP
- **Pages:** 275-304
- **Published:** September 2023
- **DOI:** `10.1145/3607840`
- **Source:** <https://doi.org/10.1145/3607840>
- **Exact extracted idea:**
  - A linear **fully in-place (FIP) calculus** characterizes when functions can
    run with **no allocation**, **no deallocation**, and **constant stack
    space**, provided arguments are unique.
  - The paper also shows generic derivation of fully in-place `map` for
    polynomial data types.
  - The implementation uses **Perceus** at runtime, but the important static
    idea is the **certification criterion**.
- **AIMS integration:**
  - Add an explicit **FIP certification output** to AIMS.
  - AIMS should be able to say:
    - this function is FIP-capable under uniqueness preconditions
    - this call site satisfies those preconditions
    - therefore allocation must be zero on the fast path
  - This is not just "FBIP enforcement." It is a stronger, more exact contract.
  - This belongs in **Section 03** and **Section 05**, with verification in
    **Section 08**.

### 3. The Functional Essence of Imperative Binary Search Trees

- **Authors:** Anton Lorenzen, Daan Leijen, Wouter Swierstra, Sam Lindley
- **Venue:** Proceedings of the ACM on Programming Languages, Vol. 8, PLDI
- **Pages:** 518-542
- **Published:** June 2024
- **DOI:** `10.1145/3656398`
- **Source:** <https://doi.org/10.1145/3656398>
- **Exact extracted idea:**
  - The paper presents top-down algorithms using a **novel first-class
    constructor context primitive**.
  - It shows functional algorithms with performance on par with imperative
    counterparts.
  - The important compiler-level idea is that **constructor contexts can encode
    in-place update opportunities structurally** rather than discovering them
    late and locally.
- **AIMS integration:**
  - Constructor contexts should not live as an isolated future experiment.
  - They should be treated as the **opportunity-creation front-end** for AIMS:
    expose "build around a hole" structure before ARC analysis runs.
  - Ori does not need user-visible constructor contexts immediately.
  - A practical first step is compiler-generated **internal constructor-context
    metadata** for specific rewrites, not a new user language feature.

### 4. Tail Recursion Modulo Context: An Equational Approach

- **Authors:** Daan Leijen, Anton Lorenzen
- **Conference version:** POPL 2023
- **Extended version:** Journal of Functional Programming, Vol. 35, 2025, e22
- **Published online:** October 24, 2025
- **DOI:** `10.1017/S0956796825100117`
- **Source:** <https://doi.org/10.1017/S0956796825100117>
- **Exact extracted idea:**
  - Generalizes tail recursion modulo `cons` to **tail recursion modulo
    contexts (TRMC)**.
  - Introduces **context laws** and abstract context operations
    (`ctx`, `app`, composition) as the semantic basis for the transformation.
  - Includes specific treatment of **constructor contexts** and a Perceus heap
    semantics instantiation to reason about in-place update.
- **AIMS integration:**
  - `TRMC` should move **out of "future stretch goals"** and become a
    pre-analysis normalization pass.
  - Not full generality at first. Use a staged rollout:
    - self-recursive functions only
    - one recursive call under a constructor or field context
    - no effectful instructions between context capture and fill
  - This belongs before the AIMS fixed-point, because it **creates** better
    memory opportunities; it is not a late optimization.

### 5. Exploring Perceus for OCaml

- **Authors:** Elton Pinto, Daan Leijen
- **Venue:** ML Family Workshop 2023, co-located with ICFP 2023
- **Presented:** September 8, 2023
- **Source:** <https://www.microsoft.com/en-us/research/publication/exploring-perceus-for-ocaml/>
- **Exact extracted idea:**
  - The important contribution is the **evaluation methodology**, not a new
    abstract analysis.
  - Same compiler, same source language, same benchmarks, **only switch the
    memory-management backend**.
  - The workshop report explicitly frames this as a direct comparison between
    precise RC and generational GC in the same system.
- **AIMS integration:**
  - The AIMS branch should be evaluated the same way:
    - same compiler
    - same frontend
    - same optimizer
    - same LLVM backend
    - only switch old ARC pipeline vs AIMS pipeline
  - This should become the **Section 08 default evaluation doctrine**.

### 6. Concurrent Immediate Reference Counting

- **Authors:** Jaehwang Jung, Jeonghyeon Kim, Matthew J. Parkinson, Jeehoon Kang
- **Venue:** Proceedings of the ACM on Programming Languages, Vol. 8, PLDI
- **Article:** 153
- **Pages:** 24 pages
- **Published:** June 20, 2024
- **DOI:** `10.1145/3656383`
- **Source:** <https://doi.org/10.1145/3656383>
- **Exact extracted idea:**
  - `CIRC` combines SMR-style deferral with RC but **immediately applies
    decrements** and only defers reclamation.
  - It specifically targets highly concurrent data structures, avoiding memory
    growth seen in deferred RC designs.
- **AIMS integration:**
  - This is **not core to current AIMS** unless Ori commits to concurrent shared
    heaps in the near term.
  - Keep as a future runtime branch:
    - preserve `RcStrategy` / runtime hook abstraction so the compiler can later
      target concurrent RC primitives
    - do not let this complexity leak into the current single-threaded AIMS core

### 7. Reference Counting Deeply Immutable Data Structures with Cycles

- **Authors:** Matthew J. Parkinson, Sylvan Clebsch, Tobias Wrigstad
- **Venue:** ISMM 2024
- **Pages:** 131-141
- **Published:** June 2024
- **Source:** <https://www.microsoft.com/en-us/research/publication/reference-counting-deeply-immutable-data-structures-with-cycles-an-intellectual-abstract/>
- **Exact extracted idea:**
  - For deeply immutable frozen graphs, reference counting can be lifted to the
    level of **strongly connected components (SCCs)**.
  - Since the graph is immutable after freeze, SCCs can be computed once and
    then RC can operate over the SCC DAG rather than individual cyclic nodes.
- **AIMS integration:**
  - This is **not immediate AIMS-core work**.
  - It becomes relevant only if Ori later adds:
    - explicit freeze
    - immutable cyclic heaps
    - graph-like runtime structures beyond trees/lists/closures
  - Keep as a future extension note in Section 07 or a separate plan.

### 8. Double-Ended Bit-Stealing for Algebraic Data Types

- **Author:** Martin Elsman
- **Venue:** Proceedings of the ACM on Programming Languages, Vol. 8, ICFP
- **Article:** 239
- **Pages:** 88-120
- **Published:** August 2024
- **DOI:** `10.1145/3674628`
- **Source:** <https://doi.org/10.1145/3674628>
- **Exact extracted idea:**
  - Uses both low and high unused pointer bits to represent more ADTs unboxed
    while retaining a uniform value representation.
  - Implemented in MLKit, with reported benchmark speedups from `0%` to `26%`
    where the representation matters, and around `9%` compiler speedups for
    compiling MLton and MLKit.
- **AIMS integration:**
  - This should remain **representation work**, not be forced into the AIMS
    fixed-point itself.
  - However, AIMS should expose enough **shape/layout hints** that a future
    representation optimizer can use AIMS facts:
    - not shared
    - not escaping
    - constructor-only usage
    - hot allocation site
  - This is a sister project to AIMS, not AIMS-core.

## Revised AIMS Architecture

The correct shape for AIMS is:

```text
Source / CanExpr
  -> lower
  -> ARC IR (allocation-neutral form)
  -> Opportunity Creation
       - TRMC normalization
       - constructor-context extraction
       - tail-context canonicalization
       - explicit collection mutation normalization
  -> Opportunity Proving
       - interprocedural MemoryContract fixed-point
       - intraprocedural AimsStateMap fixed-point
  -> Opportunity Realization
       - arg ownership
       - RcInc/RcDec
       - Reset/Reuse/IsShared/Set/SetTag
       - CowAnnotations
       - DropHints
       - FIP / FBIP certification artifacts
       - future locality / stack-allocation hints
  -> final ARC cleanup + verification
  -> LLVM IR
```

The crucial principle is:

> **No optimization gets its own independent source of truth.**

If an optimization needs to know that a value is unique, local, single-use, and
shape-compatible, those facts must all come from the same `AimsStateMap` and
`MemoryContract`.

## Exact Plan Changes

This section describes how to weave the research ideas into the **existing**
plan in a working fashion.

### Change 1: Add a new pre-analysis phase to the plan

**Current problem:** `TRMC` and constructor contexts are deferred to Section 07
as optional future work.

**Correction:** add a new section before the current Section 01.

Suggested section:

- **Section 00A: Opportunity Creation**
  - `aims/normalize/mod.rs`
  - `aims/normalize/trmc.rs`
  - `aims/normalize/context.rs`
  - `aims/normalize/collections.rs`

Responsibilities:

- detect `TRMC`-eligible self recursion
- rewrite simple constructor-context recursion into a canonical form
- normalize collection-mutating idioms into explicit ARC-friendly forms
- preserve source spans and debugability

Initial rollout constraints:

- self-recursive only
- one recursive call per transformed region
- recursive call beneath a constructor or field context
- no captured exceptions/effects across the context
- no polymorphic unknown layout contexts in v1

This keeps the first implementation tractable and correct.

### Change 2: Expand Section 01 from a 3-axis lattice to a modal fact domain

**Current problem:** the plan focuses mainly on ownership, uniqueness, and
cardinality.

**Correction:** Section 01 should define a product domain closer to:

```rust
pub struct AimsState {
    pub ownership: OwnershipMode,
    pub cardinality: Cardinality,
    pub uniqueness: Uniqueness,
    pub locality: Locality,
    pub shape: ShapeClass,
    pub effect: EffectClass,
}
```

Where:

- `OwnershipMode`
  - `Dead`, `Borrowed`, `Affine`, `Linear`, `Unrestricted`
- `Cardinality`
  - `Absent`, `Once`, `Many`
- `Uniqueness`
  - `Fresh`, `Unique`, `MaybeShared`, `Shared`
- `Locality`
  - `HeapEscaping`, `FunctionLocal`, `BlockLocal`, `Unknown`
- `ShapeClass`
  - `NonReusable`, `ReusableCtor(CtorKind)`, `CollectionBuffer`, `ContextHole`
- `EffectClass`
  - `Pure`, `MayAlloc`, `MayShare`, `MayThrow`, `Unknown`

Important note:

- `shape` and `effect` do **not** need to be perfect for v1.
- They may begin as coarse conservative tags.
- The value is architectural: all downstream consumers read one unified fact
  structure.

### Change 3: Replace "AimsSig" with "MemoryContract"

**Current problem:** `AimsSig` in Section 03 is still too narrow. It is a better
`AnnotatedSig`, but not yet the single contract object the rest of the system
should consume.

**Correction:** rename the core concept from `AimsSig` to `MemoryContract`
(keeping `type AimsSig = MemoryContract` as a migration alias if needed).

Suggested structure:

```rust
pub struct MemoryContract {
    pub params: Vec<ParamContract>,
    pub return_info: ReturnContract,
    pub effects: EffectSummary,
    pub context_behavior: ContextBehavior,
    pub fip: FipContract,
}
```

Where:

- `ParamContract`
  - ownership requirement
  - cardinality
  - may_escape
  - may_share
  - locality lower bound
- `ReturnContract`
  - uniqueness
  - freshness
  - locality
  - shape class
- `EffectSummary`
  - may_allocate
  - may_share
  - may_throw
- `ContextBehavior`
  - preserves constructor context?
  - consumes context hole?
- `FipContract`
  - `Never`
  - `Conditional { requires_unique_params: BitSet }`
  - `Certified`

This contract should drive:

- `ArcParam.ownership`
- `Apply.arg_ownership` / `Invoke.arg_ownership`
- call-site RC transfer decisions
- locality hints
- FIP checks

### Change 4: Make Section 02 emit more than "state at points"

**Current problem:** Section 02 frames the state map mainly as the analysis data
needed for RC and reuse.

**Correction:** Section 02 should also record:

- context-hole provenance for rewritten `TRMC` regions
- local allocation eligibility
- shape compatibility for reuse
- exact fast-path predicates for FIP-capable calls

Practical rule:

- keep `block_entry_states` / `block_exit_states`
- add a sparse per-event table for special events:
  - constructor-context open/close
  - candidate reusable allocation sites
  - FIP gate points

This prevents the state map from becoming an unstructured dump.

### Change 5: Keep LLVM-facing artifacts unchanged in v1

**Current problem:** the plan is ambitious enough that it risks forcing major IR
surface changes too early.

**Correction:** the first coherent AIMS integration should preserve these stable
outputs:

- `ArcParam.ownership`
- `Apply.arg_ownership`
- `Invoke.arg_ownership`
- `ArcFunction.cow_annotations`
- `ArcFunction.drop_hints`
- `ArcFunction.tail_calls`

New AIMS outputs should be added as internal analysis artifacts first, not new
mandatory fields on `ArcFunction`.

If new fields are later needed, they must be:

- optional or derived
- `#[serde(skip)]` when cache-incompatible
- invisible to old consumers until wired

### Change 6: Keep FBIP as a post-pipeline diagnostic; add FIP as a contract

**Current problem:** the current plan tends to blur reuse emission and FBIP
enforcement.

**Correction:**

- `FBIP` should remain a **post-pipeline read-only diagnostic** over final IR.
- `FIP` should become an **analysis-time contract** computed by AIMS.

That yields a clean split:

- `FIP` = what the analysis proves can run fully in-place under preconditions
- `FBIP` = what the final emitted function actually achieves in the final ARC IR

This is cleaner, stronger, and easier to verify.

### Change 7: Move TRMC and constructor contexts out of Section 07 stretch goals

**Current problem:** Section 07 treats `TRMC` and constructor contexts as future
nice-to-haves.

**Correction:** split them:

- `TRMC` and internal constructor-context extraction move to the new
  **Opportunity Creation** phase
- advanced whole-program generalizations remain in Section 07

Section 07 should then focus on genuinely advanced follow-ons:

- immortal objects
- static RC coalescing
- whole-program mutability
- SCC-frozen cyclic RC
- concurrency-specific runtime strategies

### Change 8: Strengthen Section 06 integration around real entry points

The current plan underestimates how many places consume ARC pipeline APIs.

Section 06 should explicitly account for:

- `run_arc_pipeline_all`
- `run_arc_pipeline`
- `run_uniqueness_analysis`
- direct `annotate_arg_ownership` use in LLVM codegen

Required migration rule:

1. Introduce new AIMS-backed implementations behind compatibility wrappers.
2. Keep old public names valid during migration.
3. Convert all call sites only after both backends are feature-selectable.

Required feature plumbing:

- add `aims` feature to `ori_arc`
- add forwarding `aims` feature to `ori_llvm`
- add forwarding `aims` feature to `oric`
- update `test-all.sh` and related scripts to support AIMS feature selection

Without this, verification instructions in Section 08 are not executable.

### Change 9: Tighten Section 08 to measure the right things

Section 08 should not stop at "same output" and "RC count lower."

It should explicitly record:

- old pipeline vs AIMS pipeline behavioral equivalence
- old pipeline vs AIMS pipeline RC op count
- old pipeline vs AIMS pipeline allocation count
- `FIP` certification coverage
- `FBIP` achieved vs missed reuse opportunities
- compile-time overhead of normalization + analysis

Suggested benchmark categories:

- list map / reverse / concat
- tree rebalance / insert / rotate-heavy code
- closure-heavy higher-order code
- pattern-match-heavy code
- collection COW stress
- deep recursive constructor contexts

This is where the `Exploring Perceus for OCaml` methodology matters most:
switch only the memory-management strategy, hold everything else constant.

## Working Integration Strategy

To keep this branch implementable, the research ideas should be staged.

### Stage 1: Make AIMS-core real and replace the current pass stack

Scope:

- Sections 01-06 as already planned
- add locality/effect/shape to the fact domain, but conservatively
- no stack allocation yet
- no user-visible constructor contexts
- no concurrent RC

Deliverable:

- old ARC pipeline replaced by AIMS for standard code paths

### Stage 2: Add FIP-capable contracts

Scope:

- extend `MemoryContract` with `FipContract`
- teach Section 05 to emit exact reuse fast paths where preconditions hold
- add verification counters for allocation-free execution

Deliverable:

- AIMS can certify some functions as conditionally or fully in-place

### Stage 3: Add constrained TRMC normalization

Scope:

- self-recursive constructor-context rewrites only
- transformed regions produce internal context metadata
- analysis reads normalized structure; no new public language feature

Deliverable:

- more opportunities for tail-call lowering, reuse, and FIP certification

### Stage 4: Add locality realization hints

Scope:

- use `Locality` facts to produce backend hints for stack or local allocation
- keep this hint-based first; do not redesign ARC IR around stack allocation yet

Deliverable:

- LLVM may consume locality hints in a later plan without changing AIMS-core

### Stage 5: Representation and runtime follow-ons

Separate but related efforts:

- representation optimization using AIMS shape/locality facts
- immortal objects
- SCC-based frozen-cycle RC
- concurrent runtime strategies

These should not block the AIMS-core replacement.

## Concrete Edits to the Existing AIMS Plan

This is the shortest exact patch list for the current plan documents.

### `00-overview.md`

Add:

- a new phase called **Opportunity Creation**
- revised architecture diagram with create/prove/realize stages
- updated dependency graph showing normalization before the lattice
- an expanded theoretical foundations table including:
  - Oxidizing OCaml
  - FIPTree
  - TRMC
  - Exploring Perceus for OCaml
  - Double-Ended Bit-Stealing

Change:

- move `TRMC` from future-only status to staged near-term work

### `section-01-lattice.md`

Add:

- `Locality`
- `ShapeClass`
- `EffectClass`

Clarify:

- these are conservative abstract facts
- not all axes are used equally in v1

### `section-02-intraprocedural.md`

Add:

- event tracking for constructor-context regions
- local-allocation eligibility flags
- FIP gate tracking

Clarify:

- `AimsStateMap` is the sole fact source for RC, reuse, COW, and FIP

### `section-03-interprocedural.md`

Change:

- evolve `AimsSig` into `MemoryContract`

Add:

- `FipContract`
- locality summaries
- effect summaries
- context-preservation summaries

### `section-04-rc-emission.md`

Add:

- read `Locality` and `EffectClass`, but in v1 only emit hints, not stack
  allocation
- preserve compatibility with current LLVM-visible outputs

### `section-05-reuse-emission.md`

Add:

- distinction between dynamic reuse, static reuse, and certified FIP fast paths
- explicit relation between reuse emission and `FipContract`

Clarify:

- `FBIP` remains read-only diagnostic over final IR

### `section-06-pipeline.md`

Add:

- feature propagation across `ori_arc`, `ori_llvm`, `oric`
- migration of all real entry points, not only `run_arc_pipeline_all`
- script changes needed to run verification with AIMS enabled

### `section-07-advanced.md`

Change:

- remove `TRMC` and constructor contexts from "future stretch" status
- keep only the broader follow-ons here

### `section-08-verification.md`

Add:

- old-vs-new allocation count comparison
- FIP certification coverage
- direct old-vs-new same-compiler comparison methodology

## Recommended Module Layout

If the plan is updated accordingly, the `aims/` tree should evolve toward:

```text
aims/
├── mod.rs
├── normalize/
│   ├── mod.rs
│   ├── trmc.rs
│   ├── context.rs
│   └── collections.rs
├── lattice.rs
├── transfer.rs
├── contract.rs
├── intraprocedural/
│   ├── mod.rs
│   ├── state_map.rs
│   ├── block.rs
│   ├── merge.rs
│   ├── pattern.rs
│   └── events.rs
├── interprocedural.rs
├── builtins.rs
├── emit_rc/
│   ├── mod.rs
│   ├── boundaries.rs
│   ├── arg_ownership.rs
│   ├── cow.rs
│   └── drop_hints.rs
├── emit_reuse/
│   ├── mod.rs
│   ├── detect.rs
│   ├── fip.rs
│   └── fbip.rs
└── verify/
    ├── mod.rs
    └── compare.rs
```

This is a better fit for the full architecture than the current tree because it
separates:

- creation
- proving
- realization
- validation

## What Not to Do

To keep the plan coherent, avoid these failure modes:

- Do **not** add a separate "locality pass" later.
- Do **not** add FIP as a standalone checker disconnected from the lattice.
- Do **not** bolt `TRMC` onto final ARC IR after RC insertion.
- Do **not** let representation work mutate the AIMS-core abstractions too early.
- Do **not** fold `FBIP` into mutating reuse emission; keep it diagnostic.
- Do **not** introduce new LLVM-visible fields until the old outputs remain stable.

## Final Recommendation

The best coherent version of AIMS is:

- **Create opportunities** with normalization and constrained `TRMC`
- **Prove opportunities** with a richer modal fact domain and `MemoryContract`
- **Realize opportunities** through one emission system and stable LLVM-facing
  artifacts

That architecture is stronger than the current plan, closer to the recent
literature, and still implementable in stages without losing the existing
pipeline migration path.

## Sources

- Oxidizing OCaml with Modal Memory Management:
  <https://doi.org/10.1145/3674642>
- FP2: Fully in-Place Functional Programming:
  <https://doi.org/10.1145/3607840>
- The Functional Essence of Imperative Binary Search Trees:
  <https://doi.org/10.1145/3656398>
- Tail Recursion Modulo Context: An Equational Approach (extended version):
  <https://doi.org/10.1017/S0956796825100117>
- Exploring Perceus for OCaml:
  <https://www.microsoft.com/en-us/research/publication/exploring-perceus-for-ocaml/>
- Concurrent Immediate Reference Counting:
  <https://doi.org/10.1145/3656383>
- Reference Counting Deeply Immutable Data Structures with Cycles:
  <https://www.microsoft.com/en-us/research/publication/reference-counting-deeply-immutable-data-structures-with-cycles-an-intellectual-abstract/>
- Double-Ended Bit-Stealing for Algebraic Data Types:
  <https://doi.org/10.1145/3674628>
