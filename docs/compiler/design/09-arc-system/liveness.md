---
title: "Liveness"
description: "Ori Compiler Design — Backward Dataflow Liveness Analysis"
order: 904
section: "ARC System"
---

# Liveness Analysis

## What Does "Live" Mean?

A variable is **live** at a program point if its value will be read at some point in the future. If a variable is not live — if no future instruction will ever read it — then its value is dead and any resources it holds can be released.

Liveness analysis is one of the most fundamental algorithms in compiler construction. It was first formalized in the context of register allocation (Chaitin, 1981) — a register holding a dead value can be reused for something else. In the ARC system, liveness serves an analogous purpose: a dead reference-counted value can be freed. The `RcDec` goes at the point where a variable transitions from live to dead — its **last use**.

The algorithm is a textbook backward dataflow analysis (Appel, "Modern Compiler Implementation," Section 10.1; Aho, Lam, Sethi, and Ullman, "Compilers: Principles, Techniques, and Tools," Section 9.2.5). Information flows backward through the control flow graph: a variable used in block B is live at the exit of every block that can reach B. The analysis computes a fixed point by iterating until the live-in and live-out sets stabilize.

## Algorithm

### Step 1: Compute Gen/Kill Sets

For each basic block, a forward scan computes two sets:

- **gen(B)** = variables used in B before being defined. These are the variables that B "needs" from its predecessors.
- **kill(B)** = variables defined in B (including block parameters). These are definitions that shadow any prior liveness.

Only RC-requiring variables are tracked — scalars (int, float, bool, etc.) are excluded because they never need `RcInc`/`RcDec`. The classifier makes this determination, keeping the sets small.

`Invoke` terminators require special handling: their `dst` variable is defined at the normal successor block's entry (not at the invoking block), since the definition only materializes if the call succeeds. A precomputed `invoke_defs` map associates each normal successor with its Invoke-defined variables.

### Step 2: Postorder Traversal

Compute a postorder traversal of the CFG. For backward dataflow, processing in postorder visits successors before predecessors, which provides good convergence — typically reaching the fixed point in 2-3 iterations for acyclic control flow, and a few more for loops.

### Step 3: Fixed-Point Iteration

Iterate until no sets change:

```
repeat:
    changed = false
    for block in postorder:
        live_out(B) = ∪ live_in(S) for each successor S of B
        live_in(B)  = gen(B) ∪ (live_out(B) - kill(B))

        if live_in or live_out changed:
            changed = true
until not changed
```

Block parameter flow is handled implicitly: `Jump` arguments are uses in the predecessor (captured by `gen` via `used_vars()`), and block params are definitions in the successor (in `kill`). No explicit parameter substitution is needed.

### Output

```
struct BlockLiveness {
    live_in:  Vec<LiveSet>,  // variables live at block entry, indexed by ArcBlockId
    live_out: Vec<LiveSet>,  // variables live at block exit, indexed by ArcBlockId
}

type LiveSet = FxHashSet<ArcVarId>;
```

## Refined Liveness

Standard liveness answers "is variable X live here?" but does not distinguish **why** it is live. This distinction is critical for reset/reuse optimization.

Consider a variable `x` that has been decremented (`RcDec`) but is still "live" because a later block also decrements it (dead code, or the `RcDec` is on a different control flow path). Standard liveness says `x` is live, so it cannot be reused. But `x` is only live because it needs cleanup — no instruction will ever *read* its value again. It is safe to reset `x` and reuse its allocation.

Refined liveness splits the live set into two categories:

| Category | Meaning | Reuse Safety |
|----------|---------|-------------|
| `live_for_use` | Variable will be read as an operand | Cannot reuse — value is needed |
| `live_for_drop` | Variable only needs `RcDec` | Safe to reuse — only cleanup pending |

### Refinement Algorithm

After computing standard liveness, a second backward pass per block classifies why each variable is live:

1. **Seed**: All `live_out` variables start as `live_for_drop` (conservative assumption).

2. **Terminator scan**: Variables used by the terminator as operands are promoted from `live_for_drop` to `live_for_use`.

3. **Backward body walk**: For each instruction in reverse:
   - `RcDec { var }`: The variable stays `live_for_drop` (unless already promoted).
   - Any other instruction using the variable as an operand: promote to `live_for_use`.

At join points (blocks with multiple predecessors), `live_for_use` wins conservatively — if any successor path reads the variable, it is treated as `live_for_use` at the join.

## Usage in the Pipeline

Liveness is computed **twice** in the ARC pipeline, serving different purposes each time:

### First Call: Before RC Insertion

Drives placement of `RcInc` and `RcDec`. The Perceus algorithm uses `live_out` to initialize a running live set per block, then walks instructions backward. Variables not in the live set at their definition point are dead immediately and get `RcDec`. Variables already in the live set at a use point need `RcInc` (multi-use).

### Second Call: After RC Insertion

Re-computes liveness on the post-RC CFG (which now contains `RcInc`/`RcDec` instructions). The refined liveness from this second pass drives reset/reuse detection: only variables classified as `live_for_drop` (not `live_for_use`) are candidates for in-place reuse.

## Prior Art

**Appel's "Modern Compiler Implementation"** (Section 10.1) is the standard reference for backward dataflow liveness analysis. Ori's implementation follows the textbook algorithm directly, with the optimization of pre-computing gen/kill sets to avoid recomputation during iteration.

**[Lean 4](https://github.com/leanprover/lean4)** (`src/Lean/Compiler/IR/LiveVars.lean`) uses liveness for the same purpose — driving RC insertion via the Perceus algorithm. Lean's implementation is functionally equivalent to Ori's standard liveness, though Lean does not implement refined liveness (Lean's reuse detection uses a different safety criterion).

**[Koka](https://github.com/koka-lang/koka)** (Perceus paper, Section 3.2) describes liveness-based RC insertion as the foundation of precise reference counting. The paper formalizes the relationship between liveness and RC placement that Ori implements.

## Design Tradeoffs

**HashSet vs bitset.** The implementation uses `FxHashSet<ArcVarId>` for live sets. A bitset indexed by `ArcVarId::raw()` would be faster for large functions (O(1) per union/intersection/difference on word-aligned chunks), but adds complexity. The hash set is simpler and performs well for typical function sizes. This is a straightforward optimization point if profiling identifies liveness as a bottleneck.

**Two-pass refinement vs single-pass.** Refined liveness runs as a second pass after standard liveness, rather than being integrated into the main fixed-point iteration. This is slightly more work (two passes instead of one), but keeps the standard liveness implementation clean and reusable — the first call (before RC insertion) does not need refined liveness, so computing it there would be wasted work.

**Scalar exclusion.** Excluding scalar variables from liveness tracking reduces the set sizes (roughly halving them for typical programs) but requires the classifier at every liveness computation. This is a clear win — smaller sets mean faster convergence and less memory — but it couples liveness to the type classification system.
