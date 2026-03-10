---
plan: "aims"
title: "AIMS Risk Solutions"
status: integrated
reviewed: true
references:
  - "plans/aims/00-overview.md"
  - "plans/aims/improvements.md"
  - "plans/aims/section-01-lattice.md"
  - "plans/aims/section-02-intraprocedural.md"
  - "plans/aims/section-03-interprocedural.md"
  - "plans/aims/section-05-reuse-emission.md"
  - "plans/aims/section-06-pipeline.md"
  - "plans/aims/section-08-verification.md"
---

# AIMS Risk Solutions

> **Historical Reference Document.** All five decisions in this document have been
> integrated into the section files: Decision 1 (AccessClass/Consumption split) into
> section-01, Decision 2 (cardinality semiring) into section-02, Decision 3 (Stage 1A-1D
> cutovers) into 00-overview and section-06, Decision 4 (ReusePlanner) into section-05,
> Decision 5 (stratified reduced product) into section-01. This document is retained as
> the original design rationale. For the current plan, read the section files directly.

This document resolves the five major risks identified in the AIMS plan.

It assumes the improvements from [`improvements.md`](./improvements.md) have
already been accepted:

- Opportunity Creation exists as a pre-analysis phase
- `MemoryContract` replaces `AimsSig`
- the fact domain grows beyond ownership/uniqueness/cardinality
- `FBIP` remains a post-pipeline diagnostic
- LLVM-facing ARC IR outputs stay stable in v1

The goal here is not to restate the risks. The goal is to choose designs that
will actually work and can be implemented without collapsing the schedule.

## Decision Summary

1. **Remove `Borrowed` from the ordered consumption lattice.**
   Borrowed is an alias/access property, not a consumption mode.
2. **Use a demand semiring for cardinality on ARC CFGs.**
   Sequential composition uses saturating addition; control-flow alternatives use
   `max`, not addition.
3. **Split Stage 1 into four cutover milestones.**
   AIMS replaces the old pipeline incrementally while preserving the final
   architecture.
4. **Treat cross-block reuse as semantic facts plus a structural planner.**
   The state map proves reuse safety facts; dominator/post-dominator analysis
   only validates CFG geometry.
5. **Implement AIMS as a stratified reduced product, not a flat 3,600-state table.**
   Core dimensions drive the fixed point; auxiliary dimensions are conservative
   and sparse in v1.

## 1. Borrowed Ordering

### Problem

The original lattice places `Borrowed` inside the same ordered dimension as
`Dead`, `Linear`, `Affine`, and `Unrestricted`. That does not work.

The failure case is real:

- `join(Linear, Borrowed) = Borrowed` loses the fact that one path consumes
  the value
- `join(Borrowed, Unrestricted)` becomes ambiguous: are we joining an aliasing
  fact or an RC responsibility fact?
- `Project` wants to preserve the source's uniqueness while creating a view,
  which is an aliasing property, not a consumption property

This must be fixed before implementation.

### Solution

Replace the old single "ownership mode" axis with two separate axes:

```rust
pub enum AccessClass {
    Borrowed,
    Owned,
}

pub enum Consumption {
    Dead,
    Linear,
    Affine,
    Unrestricted,
}
```

And use:

```rust
pub struct AimsState {
    pub access: AccessClass,
    pub consumption: Consumption,
    pub cardinality: Cardinality,
    pub uniqueness: Uniqueness,
    pub locality: Locality,
    pub shape: ShapeClass,
    pub effect: EffectClass,
}
```

### Join Rules

- `join(access_a, access_b)`:
  - `Owned` if either side is `Owned`
  - `Borrowed` only if both sides are `Borrowed`
- `join(consumption_a, consumption_b)`:
  - componentwise `max` with ordering
    `Dead < Linear < Affine < Unrestricted`

This gives the required result:

- `join((Owned, Linear), (Borrowed, Linear)) = (Owned, Linear)`
- consumption information survives
- RC emission remains conservative

### Borrow Provenance

Exact borrow source information is useful for optimizations such as
uniqueness-preserving borrows, but it does not belong in the finite lattice.

Use a sparse side table:

```rust
pub enum BorrowSource {
    Exact(ArcVarId),
    Unknown,
}
```

Stored only for variables currently in `AccessClass::Borrowed`.

Rules:

- `Project(dst, src, ...)` sets:
  - `dst.access = Borrowed`
  - `borrow_source[dst] = Exact(src)`
  - `dst.uniqueness = src.uniqueness`
- join of two borrowed values:
  - same source -> keep `Exact(source)`
  - different source -> promote to `Unknown`
- join of borrowed and owned:
  - `access = Owned`
  - clear borrow provenance

This keeps the lattice finite while preserving useful alias information where it
exists.

### Emission Rule

RC emission must depend on `access`, not on an overloaded "mode":

- emit `RcInc` / `RcDec` only when `access == Owned`
- borrowed values never own the RC obligation
- scalar values still short-circuit before this

### Contract Rule

`MemoryContract` should use the same split:

- parameter access requirement: borrowed vs owned
- parameter consumption: linear/affine/unrestricted

Tail-call preservation is handled here:

- if a parameter is passed to an owned callee position in tail position,
  promote that parameter's `access` requirement from `Borrowed` to `Owned`
- this is a contract inference rule, not a lattice join hack

### Why This Works

- aliasing and RC responsibility stop fighting each other inside one ordering
- `Project` becomes representable without losing source uniqueness
- the join operation becomes monotone and unsurprising
- later FIP/TRMC work can use borrowed views without corrupting ownership facts

### Required Plan Edit

Revise Section 01 so `Borrowed` is no longer a member of the ordered
substructural mode dimension. The ordered axis becomes `Consumption`; borrowed
becomes `AccessClass`.

## 2. Cardinality Analysis on Imperative ARC IR

### Problem

A direct transplant of GHC's demand analysis is not sufficient. ARC IR is a CFG
with blocks, terminators, exceptional edges, and mutation-oriented operations.

The dangerous mistake is to use saturating addition at branch joins:

- if a value is used once in the `then` branch and once in the `else` branch,
  that is still `Once` per execution, not `Many`

So the analysis must distinguish:

- **sequential composition** along one path
- **alternative control flow** where only one successor executes

### Solution

Define cardinality as the semiring:

```rust
pub enum Cardinality {
    Absent, // 0
    Once,   // 1
    Many,   // omega
}
```

With two operators:

- **Sequential composition**: `seq_add`
  - `Absent + x = x`
  - `Once + Once = Many`
  - `Many + _ = Many`
- **Alternative control-flow join**: `alt_join = max`
  - `max(Once, Once) = Once`
  - `max(Once, Many) = Many`

This is the key adaptation from functional demand analysis to imperative CFGs.

### Block Algorithm

For each block:

1. Compute per-successor edge demand states
2. Combine successor entry demands with `alt_join`
3. Walk the block backwards
4. For each instruction, add local demand with `seq_add`

The local demand of an instruction is the number of uses performed by that
instruction on each operand, before control continues to the successor.

Examples:

- `Project { value, .. }`:
  - `value` gets one read demand
- `Construct { args, .. }`:
  - each argument gets one consumption demand
- `PartialApply { args, .. }`:
  - captured args are immediately promoted to `Many` and `Owned`
    because the closure may outlive the current path and be invoked multiple times
- unknown indirect call:
  - closure and args become `Many` and `Owned` conservatively

### Loops

Do not special-case loops with ad hoc heuristics first. Let the fixed point do
the work.

The combination of:

- `alt_join = max` at block joins
- `seq_add` inside transfer functions
- iterative solution over CFG cycles

already promotes loop-carried `Once` to `Many`.

Example:

- loop body uses `x` once
- the backedge feeds a successor demand of `Once`
- walking backward through the body applies `seq_add(Once, Once) = Many`
- next iteration stabilizes at `Many`

This is the desired behavior.

### Exceptional Control Flow

`Invoke` requires edge-sensitive states. Block entry/exit alone are not enough.

Add terminator-edge states:

```rust
pub struct TerminatorEdgeState {
    pub normal: Option<StateMap>,
    pub unwind: Option<StateMap>,
}
```

Rules:

- the normal successor sees `dst` as defined
- the unwind successor does not
- successor combination for cardinality is still `alt_join`
- unwind cleanup emission reads the unwind edge state directly, not a merged
  approximation

This is the only robust way to handle exception edges.

### COW Mutation Points

COW does not need a separate cardinality semantics.

Model it as:

- one use of the receiver/value for cardinality
- uniqueness decides whether the site is `StaticUnique`, `StaticShared`, or
  `Dynamic`
- the result is always fresh/unique

This keeps cardinality orthogonal to runtime uniqueness checks.

### Validation Corpus

Before full integration, add 10 hand-traced tests covering:

1. straight-line single-use value
2. `if` with one use in each branch
3. `if` with a use in one branch and none in the other
4. simple loop with one use per iteration
5. nested loop
6. `Switch` with pattern-bound values
7. `Invoke` with live values across unwind
8. `Project` followed by source reuse
9. COW-heavy collection update
10. `PartialApply` capture

The expected cardinality for each variable at key points must be written down by
hand and asserted in tests.

### Why This Works

- branch-exclusive uses stop being misclassified as `Many`
- loops still converge to `Many` when appropriate
- exceptions are modeled explicitly rather than hidden in block joins
- the resulting cardinality is conservative enough for RC emission and precise
  enough to create real wins

### Required Plan Edit

Section 02 should explicitly define:

- `seq_add` for local/instruction composition
- `alt_join = max` for successor alternatives
- edge-sensitive states for `Invoke`

## 3. Stage 1 Delivery Risk

### Problem

Stage 1 is too large if treated as one monolithic replacement:

- new lattice
- new contracts
- new intraprocedural analysis
- new RC emission
- new reuse emission
- pipeline integration
- test parity with battle-tested code

If executed as one cutover, it can easily consume the entire branch and delay
the genuinely differentiating work: FIP contracts and TRMC-fed analysis.

### Solution

Split Stage 1 into four mandatory cutover milestones.

### Stage 1A: Shadow Analysis

Deliverables:

- `MemoryContract`
- `AimsStateMap`
- Opportunity Creation scaffolding
- no IR mutation from AIMS yet

Pipeline:

- run old pipeline as today
- run AIMS analysis in shadow mode
- compare AIMS-derived ownership, uniqueness, cardinality, and contract outputs
  against old artifacts where equivalents exist

Required gates:

- all lattice/property tests green
- golden corpus green
- diff harness shows no unexplained mismatches in ownership/COW-facing metadata

### Stage 1B: Metadata Cutover

Replace only metadata producers first:

- `ArcParam.ownership`
- `Apply.arg_ownership`
- `Invoke.arg_ownership`
- `ArcFunction.cow_annotations`

Keep:

- old RC insertion/elimination
- old reset/reuse

This proves that AIMS contracts and state maps are good enough to drive the LLVM
interface without yet taking on full RC/reuse correctness.

### Stage 1C: RC Cutover

Replace:

- `rc_insert`
- `rc_identity`
- `rc_elim`

Keep temporarily:

- old reuse detection/expansion if needed for schedule safety

This is acceptable because reuse is downstream of correct RC placement. The
pipeline still converges toward the final architecture, but the branch gets a
real working AIMS-based RC system earlier.

### Stage 1D: Reuse Cutover

Replace:

- `reset_reuse`
- `expand_reuse`

At this point Stage 1 is complete and the old analysis/emission stack can be
gated off.

### Scope Locks for Stage 1

The following are non-negotiable v1 limits:

- `Locality`, `ShapeClass`, and `EffectClass` exist in the state, but may start
  conservative (`Unknown`, `NonReusable`, `Unknown`) where precision is not yet
  needed
- no stack allocation in Stage 1
- no representation redesign in Stage 1
- no concurrency-specific RC strategy in Stage 1
- TRMC v1 is self-recursive, single-hole, effect-free between capture and fill

This prevents Stage 1 from absorbing Stage 4 and Stage 5 work.

### Why This Works

- the architecture stays intact
- each milestone produces a working system
- failures localize to one layer
- Stage 2 (FIP contracts) can begin once `MemoryContract` is real
- Stage 3 (TRMC normalization) can begin against shadow analysis and metadata
  cutover instead of waiting for the full old-pipeline deletion

### Required Plan Edit

Replace the single Stage 1 gate with the four cutover milestones above.

## 4. Cross-Block Reuse

### Problem

Cross-block reuse still needs CFG geometry:

- a dead unique value in block `B`
- a same-sized allocation in block `C`
- safe reuse only if the death dominates the allocation and the allocation
  post-dominates the death

This is not a flaw in AIMS. It is a category distinction:

- AIMS proves semantic facts
- dominance/post-dominance proves path structure

The mistake would be to pretend the state map alone can replace CFG geometry.

### Solution

Introduce a dedicated `ReusePlanner` pass after RC emission and before final
block cleanup.

The planner consumes:

- semantic candidate events from AIMS
- one dominator tree
- one post-dominator tree

### Candidate Events

The analysis/emission pipeline records two sparse event sets:

```rust
pub struct DeathEvent {
    pub var: ArcVarId,
    pub block: ArcBlockId,
    pub instr_idx: usize,
    pub uniqueness: Uniqueness,
    pub shape: ShapeClass,
    pub size_class: SizeClass,
}

pub struct AllocEvent {
    pub block: ArcBlockId,
    pub instr_idx: usize,
    pub dst: ArcVarId,
    pub shape: ShapeClass,
    pub size_class: SizeClass,
}
```

Only events relevant to reuse are stored.

### Matching Rule

For a `DeathEvent d` and `AllocEvent a`, reuse is legal only if:

1. `d.uniqueness` is `Unique` or `MaybeShared`
2. `d.shape` and `a.shape` are reuse-compatible
3. `d.size_class == a.size_class`
4. `d.block` dominates `a.block`
5. `a.block` post-dominates `d.block`
6. no earlier chosen match has already consumed the token

Selection strategy:

- prefer same-block matches
- then nearest dominated/post-dominating target by dominator depth
- prefer same-shape over merely same-size

### Dynamic vs Static Reuse

- `Unique` at death -> emit direct fast path
- `MaybeShared` at death -> emit `IsShared` split
- `Shared` -> reject as reuse source

This keeps reuse behavior grounded in AIMS facts.

### Cost Control

Do not build dominator trees for every function unconditionally.

Only run `ReusePlanner` if the function has:

- at least one death event with `shape` reusable
- at least one compatible allocation event

For the common case with no reuse candidates, there is no structural pass cost.

### Why This Works

- semantic reasoning stays unified in AIMS
- structural validity is handled by the right tool
- the planner is cheap and explicit
- the "one truth" story remains intact because dominance is not a competing
  source of semantic truth

### Required Plan Edit

Section 05 should describe cross-block reuse as:

- candidate generation from `AimsStateMap`
- structural matching by `ReusePlanner`

not as "the state map alone replaces dominator analysis."

## 5. State Space Explosion

### Problem

A naive 6-axis product domain suggests thousands of abstract states per variable
per program point. Exhaustive whole-product testing would be a mistake, and a
giant table-driven implementation would be brittle and slow.

### Solution

Implement AIMS as a **stratified reduced product**.

### Core vs Auxiliary Dimensions

Split the dimensions into:

**Core fixed-point dimensions**

- `AccessClass`
- `Consumption`
- `Cardinality`
- `Uniqueness`

**Auxiliary dimensions**

- `Locality`
- `ShapeClass`
- `EffectClass`

All seven facts still live in one `AimsState`, but they are not all equal in the
solver.

Rules:

- the worklist must always react to core changes
- auxiliary changes only trigger reprocessing of instructions that actually read
  those dimensions
- in v1, auxiliary dimensions may remain conservative for many instructions

This preserves "one truth" without forcing every axis to dominate the cost model.

### Factorized Transfer Functions

Do not define transfer by enumerating full states.

Each instruction implements small per-axis updates:

- `Project`:
  - updates `access`, borrow provenance, `shape`
  - preserves `uniqueness`
- `Construct`:
  - sets `access = Owned`
  - sets `consumption = Linear`
  - sets `uniqueness = Fresh/Unique`
  - sets `shape` from constructor kind
- `PartialApply`:
  - forces captured vars to `Owned + Many`
  - marks effect/locality conservatively

This is maintainable and testable.

### Feasibility Canonicalization

Add a `canonicalize()` step after each transfer/join.

Examples:

- borrowed values cannot carry owned-only RC obligations
- `Dead` implies `cardinality = Absent`
- definitely non-reusable values collapse `shape` to `NonReusable`
- impossible locality/effect combinations collapse to the nearest conservative
  state

This shrinks the reachable state space dramatically.

### Sparse Representation

Do not store full dense fact maps for all variables at all points.

Use:

- dense vectors for core dimensions where needed
- sparse side tables for borrow provenance and special events
- omission/defaulting for variables at the canonical bottom/top state

This matters more in practice than the theoretical state count.

### Testing Strategy

Use four layers of tests:

1. **Per-axis lattice laws**
   - exhaustive for each single dimension
2. **Pairwise interaction tests**
   - `access x consumption`
   - `consumption x cardinality`
   - `uniqueness x shape`
   - `locality x effect`
3. **Property-based full-state tests**
   - generate only feasible states
   - verify monotonicity and canonicalization invariants
4. **Golden end-to-end programs**
   - validate the real interactions that matter to codegen

The mistake is to try to exhaustively enumerate all cross-product behaviors. The
correct strategy is reduced-product testing plus end-to-end representative cases.

### Why This Works

- solver cost tracks the axes that actually matter for Stage 1 correctness
- auxiliary facts remain available for later FIP/TRMC/locality work
- transfer functions stay understandable
- testing targets real interaction surfaces instead of impossible combinations

### Required Plan Edit

Section 01 should explicitly define AIMS as a reduced product with:

- core dimensions
- auxiliary dimensions
- canonicalization after transfer/join
- sparse side tables for provenance/events

## Immediate Plan Changes

The five solutions above imply the following concrete edits to the AIMS plan:

1. `Borrowed` leaves the ordered consumption axis and becomes `AccessClass`
2. Section 02 defines cardinality with `seq_add` and `alt_join`
3. `AimsStateMap` gains edge-sensitive terminator states for `Invoke`
4. Stage 1 is rewritten as Stage 1A-1D cutovers
5. Section 05 introduces `ReusePlanner`
6. Section 01 defines a reduced-product solver model
7. borrow provenance and special events are explicit sparse side tables

## Final Position

These five decisions solve the main technical risks without weakening the AIMS
thesis.

They preserve the important claims:

- one semantic source of truth
- a coherent create/prove/realize architecture
- a realistic migration path
- room for FIP, TRMC, locality, and shape-driven optimizations later

But they remove the fragile parts:

- overloaded borrow semantics in the lattice
- incorrect branch cardinality joins
- monolithic Stage 1 scope
- hand-wavy cross-block reuse
- naive cross-product state modeling

If adopted, these solutions make the plan implementable rather than just
ambitious.
