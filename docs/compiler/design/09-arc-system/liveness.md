---
title: "Liveness"
description: "Ori Compiler Design — Backward Dataflow Liveness Analysis"
order: 904
section: "ARC System"
---

# Liveness Analysis

Liveness analysis computes which variables are **live** (will be read in the future) at every basic block boundary. This information drives RC insertion: a variable's last use is where its `RcDec` goes, and additional uses require `RcInc`.

The implementation is standard backward dataflow with fixed-point iteration.

**Source**: `compiler/ori_arc/src/liveness/mod.rs`

## Algorithm

Entry point: `compute_liveness(func, classifier) -> BlockLiveness`

### Step 1: Precompute Gen/Kill

For each block, compute gen and kill sets with a forward scan:

- **gen(B)** = variables used before being defined in B. These are the variables that block B "needs" from its predecessors.
- **kill(B)** = variables defined in B, including block parameters. These are the variables whose definitions in B shadow any prior liveness.

Block parameters are treated as definitions (they go into `kill`). Invoke `dst` variables are treated as definitions at the normal successor's entry, not at the invoking block -- a precomputed `invoke_defs` map handles this.

Only RC-requiring variables are tracked. Scalar variables (int, float, bool, etc.) are excluded because they never need `RcInc`/`RcDec`. The classifier (`ArcClassification::needs_rc()`) makes this determination.

```
fn compute_gen_kill(block, func, classifier, invoke_defs) -> (gen, kill):
    kill = {block params} ∪ {invoke dsts defined at this block's entry}
    gen = {}

    for instr in block.body (forward):
        for var in instr.used_vars():
            if needs_rc(var) and var not in kill:
                gen.insert(var)
        if instr defines dst and needs_rc(dst):
            kill.insert(dst)

    for var in terminator.used_vars():
        if needs_rc(var) and var not in kill:
            gen.insert(var)
```

### Step 2: Postorder Iteration

Compute a postorder traversal of the CFG. For backward dataflow, postorder processes successors before predecessors, which provides good convergence behavior.

### Step 3: Fixed-Point Iteration

Iterate until no sets change:

```
repeat:
    changed = false
    for block_idx in postorder:
        live_out(B) = union of live_in(S) for each successor S
        live_in(B)  = gen(B) ∪ (live_out(B) - kill(B))

        if live_in or live_out changed:
            changed = true
until not changed
```

Block parameter flow is handled implicitly: `Jump` arguments are uses in the predecessor (captured by `gen` via `ArcTerminator::used_vars()`), and block params are definitions in the successor (in `kill`). No explicit parameter substitution is needed.

## Output

```
struct BlockLiveness {
    live_in:  Vec<LiveSet>,  // variables live at block entry, indexed by ArcBlockId
    live_out: Vec<LiveSet>,  // variables live at block exit, indexed by ArcBlockId
}

type LiveSet = FxHashSet<ArcVarId>;
```

`live_in[b]` is the set of variables live at the entry of block `b`. `live_out[b]` is the set of variables live at the exit of block `b`. Both are indexed by `ArcBlockId::index()`.

The implementation uses `FxHashSet` for simplicity. A bitset indexed by `ArcVarId::raw()` would be faster for large functions but adds complexity -- this can be optimized later if profiling shows it matters.

## Refined Liveness

Standard liveness says "variable X is live here" but does not distinguish between "X is live because it will be read" and "X is live only because it needs an `RcDec`." This distinction is critical for reset/reuse optimization: a variable that is only live-for-drop can be safely reset without risking a use-after-free, whereas a variable that is live-for-use cannot.

Entry point: `compute_refined_liveness(func, classifier) -> (Vec<RefinedLiveness>, BlockLiveness)`

```
struct RefinedLiveness {
    live_for_use:  LiveSet,  // variable will be read as an operand
    live_for_drop: LiveSet,  // variable only needs RcDec (not read)
}
```

### Algorithm

After computing standard liveness, a second backward pass per block classifies *why* each variable is live:

1. **Seed**: All `live_out` variables start in `live_for_drop` (conservative).

2. **Terminator scan**: Variables used by the terminator as operands are promoted from `live_for_drop` to `live_for_use`.

3. **Backward body walk**: For each instruction in reverse:
   - `RcDec { var }`: The variable stays in `live_for_drop` (unless already promoted to `live_for_use`).
   - Any other instruction: Variables used as operands are promoted from `live_for_drop` to `live_for_use`.

At join points (blocks with multiple predecessors), `live_for_use` wins conservatively -- if any successor path reads the variable, it is treated as live-for-use at the join.

The function returns both the refined classification and the standard `BlockLiveness`, avoiding a redundant fixed-point iteration when callers need both.

## Usage in the Pipeline

Liveness is computed **twice** in the ARC pipeline:

### First call: Before RC insertion

Drives placement of `RcInc` and `RcDec`. The RC insertion pass (Perceus algorithm) uses `live_out` to initialize a running live set per block, then walks instructions backward. Variables not in the live set at their definition point get `RcDec` (dead immediately). Variables already in the live set at a use point get `RcInc` (multi-use).

### Second call: After RC insertion

Re-computes liveness on the post-RC CFG (which now contains `RcInc`/`RcDec` instructions). The refined liveness from this second pass drives reset/reuse detection: only variables classified as `live_for_drop` (not `live_for_use`) are candidates for in-place reuse.

## Invoke Handling

The `Invoke` terminator is a function call that may throw (similar to LLVM's `invoke`). Its `dst` variable is defined at the normal successor's entry, not at the invoking block. A precomputed `invoke_defs` map (`collect_invoke_defs()`) associates each normal successor block with the Invoke `dst` variables defined there, so that gen/kill correctly accounts for these definitions.

## References

- Lean 4: `src/Lean/Compiler/IR/LiveVars.lean` -- liveness for RC insertion
- Koka: Perceus paper, Section 3.2 -- liveness-based RC insertion
- Appel: "Modern Compiler Implementation", Section 10.1 -- backward dataflow analysis
- Swift: `lib/SILOptimizer/ARC/` -- ownership SSA and liveness
