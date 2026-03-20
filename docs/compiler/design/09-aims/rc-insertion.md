---
title: "Unified Realization"
description: "Ori Compiler Design — AIMS Unified RC, Reuse, COW, and Drop Emission"
order: 905
section: "AIMS"
---

# Unified Realization

## The Emission Problem

Given a converged `AimsStateMap` that describes the complete memory fate of every variable at every program point, how do you translate those abstract lattice states into concrete IR instructions? This is the realization problem — reading the analysis results and emitting all memory management operations.

Traditional ARC optimizers solve this through a sequence of passes: RC insertion, reuse detection, reuse expansion, RC elimination, COW annotation, drop hints. Each pass runs its own analysis, makes its own decisions, and produces its own output. AIMS replaces all of these with a **single realization step** that reads the converged lattice state and emits RC operations, reuse tokens, COW annotations, and drop hints in one coordinated walk.

The benefit is consistency — all decisions come from one data source, so RC placement, reuse, COW, and drops cannot disagree. It also eliminates the need for a separate RC elimination pass, because precise placement based on full lattice information avoids creating the redundant pairs that elimination would remove.

## Two-Phase Realization

Realization proceeds in two phases, separated by `merge_blocks()`:

### Phase 1: RC + Reuse + Arg Ownership (Pre-Merge)

`realize_rc_reuse()` walks the IR forward, reading the converged `AimsStateMap` at each instruction:

1. **Arg ownership**: For each `Apply` and `Invoke`, determine per-argument ownership from the callee's `MemoryContract`. Write `arg_ownership` on the call instruction.

2. **RC decisions**: For each variable definition and use, consult the lattice state:
   - **Cardinality = Many**: Variable is used more than once — emit `RcInc` with count = uses - 1
   - **Cardinality = Once**: Variable is used exactly once — no `RcInc` needed
   - **Cardinality = Absent**: Variable is never used after this point — emit `RcDec`
   - **Access = Borrowed**: Skip RC entirely (caller manages lifetime)

3. **Reuse decisions**: For each `RcDec`/`Construct` proximity where `ShapeClass` matches and `Uniqueness` allows, emit `Reset`/`Reuse` pairs. See [Reuse Emission](reset-reuse.md) for details.

4. **Strategy assignment**: Each `RcInc` and `RcDec` gets an embedded `RcStrategy` computed from `ValueRepr`:

| Strategy | What It Does |
|----------|-------------|
| `HeapPointer` | Direct `ori_rc_inc`/`ori_rc_dec` on the pointer |
| `FatPointer` | Extract data pointer from field 1, then inc/dec |
| `Closure` | Extract env_ptr, null-check (bare functions have no env), then inc/dec |
| `AggregateFields` | Traverse RC fields recursively |
| `InlineEnum` | Tag-switch, per-variant field handling |

### Phase 2: COW + Drop Hints (Post-Merge)

After `merge_blocks()` cleans up the CFG, `realize_annotations()` walks the post-merge IR to emit annotations:

1. **COW annotations**: For each mutation site, determine the COW mode from the lattice:
   - **`StaticUnique`**: Uniqueness dimension proves RC == 1 — emit only the fast path (no runtime check)
   - **`StaticShared`**: Uniqueness proves RC > 1 — always use the slow path (copy + mutate)
   - **`Dynamic`**: Uniqueness is `MaybeShared` — emit the full COW pattern with `IsShared` check

2. **Drop hints**: For each `RcDec`, check if the target is proven unique — if so, the LLVM backend can call `ori_buffer_drop_unique` (skips the refcount check) instead of `ori_buffer_rc_dec`.

**Critical ordering**: Phase 2 uses `ArcVarId`-keyed lookups via `var_state_at_block_entry()`, not position-keyed state maps. Position-keyed maps (`block_entry_states`, `block_exit_states`) are invalidated by `merge_blocks()`. The `ArcVarId`-keyed lookups work because `merge_blocks()` preserves entry block IDs.

## Decision Functions

All realization decisions flow through a unified `decide()` function that reads the `AimsState` for a variable and returns the appropriate action. The decision logic for each output:

### RC Placement

```
if state.access == Borrowed:
    skip entirely (caller manages lifetime)
elif var is scalar or immortal:
    skip (no RC needed)
elif state.cardinality == Absent:
    emit RcDec (variable is dead)
elif state.cardinality == Many:
    emit RcInc with count = uses - 1
else:
    no RC op needed (single use, naturally consumed)
```

### COW Mode

```
if state.uniqueness == Unique:
    StaticUnique (skip runtime check)
elif state.uniqueness == Shared:
    StaticShared (always copy)
else:
    Dynamic (runtime IsShared check)
```

### Drop Hint

```
if state.uniqueness == Unique:
    use ori_buffer_drop_unique (skip RC check)
else:
    use ori_buffer_rc_dec (full check)
```

## Borrowed Parameter Handling

Borrowed parameters receive special treatment:

- **Borrowed params** skip all RC tracking — no `RcInc`, no `RcDec`. The only exception is at `Return`: returning a borrowed param transfers ownership to the caller, requiring `RcInc`.

- **Borrowed-derived variables** (projections or aliases of borrowed params) skip normal RC tracking but get `RcInc` at **owned positions** — places where the value will be stored on the heap:
  - `Construct` arguments
  - `PartialApply` captures (unless the capture is at a borrowed callee position and the closure does not escape)
  - `Apply`/`ApplyIndirect` arguments at owned positions
  - `Return` values

### Closure Capture Optimization

When `PartialApply` captures a borrowed-derived variable, the normal rule requires `RcInc` (the closure stores the value). But this can be safely skipped when both conditions hold:

1. The callee expects the corresponding parameter as `Borrowed`
2. The closure does not escape the current block (`dst` is not in `live_out`)

## Edge Cleanup

After per-block realization, variables that are live at a predecessor's exit but not live at a successor's entry create "edge gaps" — they need `RcDec` at the transition. Three cases arise:

**Single-predecessor blocks**: `RcDec` instructions are prepended to the successor block's body.

**Multi-predecessor with identical gaps**: All predecessors strand the same variables. `RcDec` is prepended to the successor block.

**Multi-predecessor with differing gaps**: Different predecessors strand different variables. A **trampoline block** (critical edge splitting) is created for each edge that needs cleanup.

## Why No Separate RC Elimination

Traditional pipelines need a separate RC elimination pass because their sequential architecture creates redundant pairs:
- Perceus insertion creates pairs that borrow inference later makes unnecessary
- Reuse expansion creates slow-path pairs that interact with surrounding operations
- Identity normalization creates root-level pairs from projected ones

AIMS avoids most of these redundancies because:
- **Contracts are known before realization**: Borrow decisions are already incorporated into the lattice state
- **Reuse decisions are coordinated**: The same state map drives both RC and reuse, so placement does not create unnecessary pairs around reuse sites
- **No identity normalization needed**: The lattice tracks variables through projections, so RC decisions are already made on the correct roots

The result is a minimal set of RC operations that requires no further cleanup.

## Prior Art

**[Koka](https://github.com/koka-lang/koka)** and the [Perceus paper](https://www.microsoft.com/en-us/research/publication/perceus-garbage-free-reference-counting-with-reuse/) (Reinking et al., 2021) formalize liveness-based RC insertion and prove that it produces the minimum number of RC operations for a given ownership assignment. AIMS's realization produces equivalent or better results because it has richer information (7 dimensions vs binary liveness) when making placement decisions.

**[Lean 4](https://github.com/leanprover/lean4)** (`src/Lean/Compiler/IR/RC.lean`) implements liveness-based RC insertion independently. AIMS differs by combining RC, reuse, COW, and drop decisions into one realization step rather than running them as separate passes.

**[Swift](https://github.com/swiftlang/swift)** (`lib/SILOptimizer/ARC/`) takes the opposite approach: Swift inserts RC eagerly during SILGen and then optimizes them away using bidirectional dataflow analysis ("insert liberally, eliminate aggressively"). AIMS is "analyze comprehensively, emit precisely" — no separate elimination needed.

## Design Tradeoffs

**Unified realization vs separate passes.** AIMS emits all outputs (RC, reuse, COW, drops) from one data source in one walk. The benefit is consistency and minimal output. The cost is a more complex realization step and the requirement that the `AimsStateMap` is complete and correct before any output is emitted. A bug in the lattice analysis affects all outputs simultaneously.

**Two-phase split at merge_blocks.** Phase 1 (RC + reuse) runs before block merging, Phase 2 (COW + drops) runs after. This is necessary because `merge_blocks()` invalidates position-keyed state maps, but COW and drop decisions need the simplified post-merge CFG. The split adds complexity but is structurally required.

**Strategy embedding.** Each `RcInc`/`RcDec` carries its `RcStrategy` embedded in the instruction. This makes the LLVM emitter a stateless pattern-match loop but increases IR size. The alternative (computing strategy at emission time from the variable's type) would keep the IR simpler but force Pool queries during LLVM emission.
