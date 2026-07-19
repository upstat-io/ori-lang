---
title: "Backward Dataflow Analysis"
description: "Ori Compiler Design — AIMS 7-Dimensional Backward Dataflow Analysis"
order: 904
section: "AIMS"
---

# Backward Dataflow Analysis

## What Does the Analysis Compute?

Traditional ARC optimizers use liveness analysis — a single boolean per variable answering "will this value be read in the future?" AIMS replaces this with a richer question: "what is the complete memory fate of this variable?" The answer is an `AimsState` — a 7-dimensional product lattice that captures ownership, consumption pattern, usage count, uniqueness proof, escape scope, reuse shape, and effect classification simultaneously.

Where traditional liveness computes one bit per variable, AIMS computes seven
interacting dimensions. Instead of feeding separate RC-insertion and reuse
passes, its unified state feeds one logical realization step that freezes
ownership events, cleanup, COW/reuse admissibility, and effect obligations
together. Physical planners act only after those facts are fixed.

The analysis is a textbook backward dataflow analysis (Appel, "Modern Compiler Implementation," Section 10.1) generalized to a product lattice. Information flows backward through the control flow graph: how a variable is used in future blocks constrains its state at earlier blocks. The analysis computes a fixed point by iterating until the `AimsState` at each block boundary stabilizes.

## The AimsState Lattice

Each variable at each program point has an `AimsState` — a product of seven dimensions:

| # | Dimension | Values | Ordering | Height |
|---|-----------|--------|----------|--------|
| 1 | **AccessClass** | `Borrowed < Owned` | Borrowed is more optimistic | 1 |
| 2 | **Consumption** | `Dead < Linear < Affine < Unrestricted` | Dead is most optimistic | 3 |
| 3 | **Cardinality** | `Absent < Once < Many` | Absent is most optimistic | 2 |
| 4 | **Uniqueness** | `Unique < MaybeShared < Shared` | Unique is most optimistic | 2 |
| 5 | **Locality** | `BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping < Unknown` | BlockLocal is most optimistic | 4 |
| 6 | **ShapeClass** | `NonReusable \| ReusableCtor(kind) \| CollectionBuffer \| ContextHole` | Flat lattice | 1 |
| 7 | **EffectClass** | `{ may_alloc, may_share, may_throw }` | Per-bit boolean | 3 |

**Target-calculus height: 16.** The shipped `ori_arc` carrier temporarily
omits `ArgEscaping` under the documented ArgEscaping-family carve-out and has
height 15.

That implementation-status carve-out does not make AIMS an executor-specific
phase: VM, LLVM/native, compiled-WebAssembly, and JIT projections are all
governed by the same five-value target contract.

### Key Constants

| Constant | Value | Use |
|----------|-------|-----|
| `AimsState::TOP` | Most conservative state | Unconstrained variables, join identity for empty sets |
| `AimsState::BOTTOM` | Most optimistic state | Starting point for all variables |
| `AimsState::SCALAR` | Sentinel for types with no managed ownership obligation | Scalars excluded from analysis |
| `AimsState::FRESH` | Base for newly constructed values | Constructions start here |

### Operations

**Join** (`a.join(b)`) — componentwise maximum. Idempotent, associative, commutative. Used at control flow merge points to combine information from different predecessors. If one path says a variable is `Once` and another says `Many`, the result is `Many`.

**Canonicalize** — enforces cross-dimension feasibility rules. Some dimension combinations are impossible or imply stronger facts. For example:
- `Unique` requires freshness or alias-proven one-owner multiplicity; locality alone proves only a lifetime bound
- `Dead + Unique` implies `Absent` cardinality (a dead unique variable has zero future uses)

Canonicalization runs in multiple rounds to catch cross-dimension chains where updating one dimension triggers another. The `cross_dimension_detected` flag tracks whether any additional rounds were needed.

## Algorithm

### Step 1: Initialization

1. Mark all variables as `BOTTOM` (most optimistic)
2. Identify scalar variables (from `ArcClass`) — these are set to `SCALAR` and excluded from analysis
3. Identify static-lifetime values whose frozen lifetime and
   `OwnershipObservationFacts` require no dynamic additional-credit or
   sharing-observation events — also excluded. The shipped compiled projection
   realizes some of these as heap objects with `MAX_REFCOUNT`; that storage and
   saturation mechanism is not part of AIMS.
4. Compute postorder traversal of the CFG

### Step 2: Backward Iteration

Process blocks in postorder (successors before predecessors) until convergence:

```
repeat:
    changed = false
    for block in postorder:
        // 1. Compute exit state from successor entry states
        exit_state[var] = join(entry_state[succ][var]) for each successor succ

        // 2. Handle invoke edges: normal vs unwind may have different demands
        if terminator is Invoke:
            record per-edge demand (InvokeEdgeState)

        // 3. Walk instructions BACKWARD through block
        for instr in block.body.reversed():
            apply transfer function to update state

        // 4. Compute entry state
        entry_state[block][var] = state after walking all instructions

        if any state changed: changed = true
until not changed
```

### Step 3: Post-Convergence

After the fixed point is reached:

1. **Verify canonical fixed point** — run multi-round canonicalize to check cross-dimension chains
2. **Populate borrow sources** — provenance tracking for borrowed variables
3. **Populate sparse events**:
   - `ContextOpen`/`ContextClose` — TRMC context regions
   - `ReusableAllocation` — candidates for reuse
   - `LifetimeBound` / `PlacementEligibilityCandidate` — neutral
     lifetime evidence that a physical planner may use for placement; the
     current event carrier is named `PlacementEligibilityCandidate`
   - `FipGate` — conditional FIP gates
   - `AllocCreditBalance` — per-branch FIP balance
4. **Compute per-variable shapes** — from `ShapeClass` dimension
5. **Compute FIP balance** — construct count vs consumed count

## Transfer Functions

Each instruction type has a backward transfer function that updates one or more dimensions of the `AimsState` for the variables it defines and uses.

### Core Patterns

**`Let(Var(v))`** — alias: inherit `v`'s state. The defined variable gets whatever state `v` has from future uses.

**`Construct`** — new allocation: state starts at `FRESH` with shape derived from the constructor kind (`ReusableCtor(Struct)`, `ReusableCtor(EnumVariant)`, `CollectionBuffer`, etc.) and `may_alloc` effect set.

**`Project`** — field extraction: produces a `Borrowed` view with the source's uniqueness. The projected variable inherits locality from the source.

**`Apply`** — function call: if the callee has a known `MemoryContract`, apply per-argument demands from the contract. Unknown callees get `TOP` (most conservative). Each argument's state is updated based on the callee's `ParamContract` for that position.

**`RcInc` / `RcDec`** — RC operations: not present during analysis (inserted during realization). Transfer functions for these are not needed.

**`PartialApply`** — closure capture: captured variables become owned by the
closure environment and inherit at least the environment's lifetime. The
current lattice records an escaping capture with the legacy `HeapEscaping`
label; AIMS does not require the environment to use heap storage.

### Dimension-Specific Updates

Each transfer function updates specific dimensions:

| Instruction | Access | Consumption | Cardinality | Uniqueness | Locality | Shape | Effect |
|-------------|--------|-------------|-------------|------------|----------|-------|--------|
| Use as operand | — | ↑ | +1 | — | — | — | — |
| Return | Owned | Linear | Once | — | escaping lifetime (`HeapEscaping` legacy label) | — | — |
| Construct arg | Owned | — | +1 | — | — | — | may_alloc |
| Project | Borrowed | — | +1 | inherit | inherit | — | — |
| Apply (owned param) | Owned | — | +1 | — | per contract | — | per contract |
| Apply (borrowed param) | Borrowed | — | +1 | — | per contract | — | per contract |
| PartialApply capture | Owned | — | +1 | — | closure-environment lifetime (`HeapEscaping` when escaping) | — | — |

The "—" entries mean the dimension is unchanged by that instruction. "+1" means cardinality is incremented (Absent → Once → Many).

## Convergence

Convergence is guaranteed by the lattice structure:

1. **Finite height**: Target lattice height is 16 (sum of per-dimension heights); the shipped four-value-locality carrier is 15
2. **Monotonic**: Each transfer function can only raise state (toward TOP), never lower it
3. **Join is monotonic**: `join(a, b) ≥ a` and `join(a, b) ≥ b`

In practice, convergence is fast — typically 2-3 iterations for acyclic
control flow, and a few more for loops. The target theoretical bound is 16
ascents per variable; the current four-value shipped carrier uses its exact
height-15 bound.

## Output: AimsStateMap

The converged analysis produces an `AimsStateMap` with the following fields:

| Field | Type | Purpose |
|-------|------|---------|
| `block_exit_states` | `Vec<FxHashMap<ArcVarId, AimsState>>` | State after each block's terminator |
| `block_entry_states` | `Vec<FxHashMap<ArcVarId, AimsState>>` | State before each block's first instruction |
| `invoke_edge_states` | `FxHashMap<ArcBlockId, InvokeEdgeState>` | Per-edge demand for Invoke (normal vs unwind) |
| `borrow_sources` | `FxHashMap<ArcVarId, BorrowSource>` | Provenance of borrowed variables |
| `events` | `FxHashMap<ArcBlockId, Vec<AimsEvent>>` | Sparse special-interest points per block |
| `scalars` | `BitVec` | Variables excluded from analysis (scalar types) |
| `immortals` | `BitVec` | Variables excluded from analysis (immortal constants) |
| `effect_summary` | `EffectSummary` | Accumulated function-level effects |
| `fip_construct_count` | `usize` | FIP token: constructions |
| `fip_consumed_count` | `usize` | FIP token: consumptions |
| `var_shapes` | `FxHashMap<ArcVarId, ShapeClass>` | Per-variable shape classification |
| `cross_dimension_detected` | `bool` | Whether canonicalization needed multiple rounds |

The realization step reads this map to make all logical ownership-event, reuse, COW, and drop decisions. After `merge_blocks()`, position-keyed fields (`block_entry_states`, `block_exit_states`) may be stale — Phase 2 realization uses `ArcVarId`-keyed lookups via `var_state_at_block_entry()` instead.

## Relationship to Traditional Liveness

AIMS's backward analysis subsumes traditional liveness:

| Traditional Concept | AIMS Equivalent |
|--------------------|-----------------|
| Live/dead | `Cardinality`: `Absent` = dead, `Once`/`Many` = live |
| Live-for-use vs live-for-drop | `Consumption` + `Cardinality`: `Dead + Once` = live-for-drop only |
| Last-use point | Where `Cardinality` transitions from `Once` to `Absent` |
| Multi-use detection | `Cardinality` = `Many` (requires an additional owner-credit event; current carrier: `RcInc`) |

The additional dimensions (Access, Uniqueness, Locality, Shape, Effect) provide information that traditional liveness does not — enabling reuse, COW, FIP, and TRMC decisions in the same realization step.

## Prior Art

**Appel's "Modern Compiler Implementation"** (Section 10.1) is the standard reference for backward dataflow analysis. AIMS's algorithm follows the textbook structure (postorder iteration, fixed-point convergence) but generalizes from boolean liveness to a 7-dimensional product lattice.

**[Lean 4](https://github.com/leanprover/lean4)** (`src/Lean/Compiler/IR/LiveVars.lean`) uses backward liveness for RC insertion via the Perceus algorithm — the historical influence on the backward direction. AIMS's analysis is Ori's own multi-dimensional formulation: liveness is one facet of the seven-dimension product, enabling unified realization rather than separate passes (proven independently; see Spec: Annex E §AIMS).

**[GHC](https://github.com/ghc/ghc)** (`compiler/GHC/Core/Opt/DmdAnal.hs`) uses a backward demand analysis with a product lattice (usage × strictness × unboxing) that influenced AIMS's multi-dimensional approach. GHC's `Demand` type is a product of `SubDemand` and `Card` (cardinality), analogous to AIMS's product of Consumption and Cardinality.

**[Koka](https://github.com/koka-lang/koka)** (Perceus paper, Section 3.2) describes liveness-based RC insertion as the foundation of precise reference counting. AIMS computes the same information (and more) through its Cardinality and Consumption dimensions.

## Design Tradeoffs

**7-dimensional product vs separate analyses.** AIMS computes all dimensions simultaneously rather than running separate liveness, uniqueness, and escape analyses. The unified approach lets lifetime, freshness, and alias-proven owner-multiplicity facts constrain one another without treating locality as uniqueness. The target lattice height is 16; the shipped four-value-locality carrier has height 15. In practice, most dimensions converge in 2-3 iterations.

**Sparse state maps vs dense arrays.** The implementation uses `FxHashMap<ArcVarId, AimsState>` for per-block states. Dense arrays indexed by `ArcVarId::raw()` would be faster for large functions but waste memory when most variables are scalar (excluded from analysis). The sparse representation is a good fit because typically only ~50% of variables need tracking (the non-scalar ones).

**Post-convergence event computation vs inline.** AIMS computes sparse events (TRMC regions, reuse candidates, FIP gates) in a post-convergence pass rather than inline during iteration. This is slightly more work but keeps the main iteration loop clean and avoids recomputing events that change during iteration.

**Scalar and immortal exclusion.** Excluding scalar and immortal variables from the analysis reduces map sizes significantly but requires classification and immortal detection before the analysis begins. This is a clear win — smaller maps mean faster convergence and less memory — and it couples the analysis to the type classification system and immortal detection pass.
