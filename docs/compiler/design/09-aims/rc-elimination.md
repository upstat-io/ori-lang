---
title: "RC Optimization"
description: "Ori Compiler Design — AIMS RC Redundancy Avoidance and Remaining Elimination"
order: 907
section: "AIMS"
---

# RC Optimization

## Why AIMS Avoids Redundancies

Traditional ARC optimizers generate redundant RC operation pairs through their sequential architecture. Each pass operates independently, creating artifacts that later passes must clean up:

- **Perceus insertion** creates pairs when borrow inference later makes them unnecessary
- **Reuse expansion** generates slow-path pairs that interact with surrounding operations
- **Identity normalization** creates root-level pairs from projected ones

Traditional pipelines solve this with a separate **RC elimination pass** that sweeps up redundancies. AIMS takes a different approach: by computing all memory facts in a single unified analysis and emitting all outputs from one converged lattice state, most redundancies never arise.

### How AIMS Prevents Each Source

**Contract-informed placement.** In a traditional pipeline, RC insertion runs before borrow inference feedback is fully propagated, creating pairs that later become unnecessary. AIMS computes interprocedural contracts first, then reads them during realization — borrowed parameters never get RC operations in the first place.

**Coordinated reuse.** Traditional reuse expansion creates pairs at the slow-path/fast-path boundary that interact with surrounding RC operations. AIMS emits reuse and RC from the same state map in the same walk, so placement accounts for reuse boundaries from the start.

**Projection-aware decisions.** Traditional pipelines need an identity normalization pass because RC operations on projected fields and their parent structs reference different variable IDs. AIMS's backward analysis tracks variables through projections via the `BorrowSource` provenance map, making RC decisions on the correct roots directly.

**Precise cardinality.** Traditional Perceus insertion uses binary liveness (live vs dead) to determine last-use points, which can be imprecise at join points. AIMS's `Cardinality` dimension (`Absent` / `Once` / `Many`) provides finer-grained demand information, avoiding unnecessary increments when a variable is used exactly once on both branches.

## Remaining Elimination

While AIMS avoids the majority of redundancies, some patterns can still produce canceling pairs after realization:

- **Reuse expansion slow paths**: The `Reset`/`Reuse` → `IsShared` expansion generates both fast and slow paths, and the slow path may contain `RcInc`/`RcDec` pairs that the expansion's projection-increment erasure does not fully resolve.
- **Edge cleanup trampolines**: Critical edge splitting can create blocks where an increment immediately precedes a decrement on the same variable.

For these remaining cases, a lightweight cleanup pass can eliminate obvious `RcInc`/`RcDec` pairs:

### Safety Invariant

RC pair elimination **only** removes pairs in the order `RcInc; ...; RcDec` (increment before decrement in program order). The reverse — `RcDec; ...; RcInc` — is **never** eliminated.

An increment followed by a decrement is a net no-op: the refcount goes up and then back down. A decrement followed by an increment means the refcount drops first (potentially reaching zero, triggering deallocation) and then goes back up on a value that may have been freed. Eliminating a dec-then-inc pair would cause use-after-free.

The "no intervening use" check is the second safety condition — if `x` is read between the increment and decrement, removing the pair would leave the refcount too low during that read.

### Intra-Block Elimination

When present, a bidirectional scan finds adjacent or near-adjacent `RcInc(x)`/`RcDec(x)` pairs with no intervening use of `x` and removes both. The pass cascades — removing one pair may expose another — but rarely needs more than 2-3 iterations.

### Cross-Block Elimination

If a block ends with `RcInc(x)` and its unique successor begins with `RcDec(x)`, both are removed. This only applies to single-predecessor successors — with multiple predecessors, the increment might not be present on all incoming edges.

## Measurement

AIMS's `SynergyMetrics` tracks the quality of RC emission:

- `rc_ops_inserted`: Total `RcInc` + `RcDec` operations emitted
- `reuse_ops_inserted`: Total `Reset` + `Reuse` + `IsShared` operations
- `cow_annotations`: COW modes assigned (StaticUnique vs Dynamic vs StaticShared)
- `drop_hints`: Unique drop eligibility count

These metrics are reported during tracing (`ORI_LOG=ori_arc=debug`) and used by the AIMS verification passes to validate that contracts and realization agree.

## Prior Art

**[Lean 4](https://github.com/leanprover/lean4)** performs RC elimination after its sequential RC insertion pass. Lean's approach uses forward-pass matching because its pipeline produces straightforward cascading patterns.

**[Swift](https://github.com/swiftlang/swift)** (`lib/SILOptimizer/ARC/`) uses bidirectional dataflow analysis for ARC optimization — a more general version that computes matching retain/release sets and eliminates them as a unit. This handles more complex patterns than pair-by-pair elimination but is also more expensive.

**Dead code elimination** in general-purpose optimizers (LLVM, GCC) performs analogous work — removing operations whose effects cancel out. RC optimization is a domain-specific instance of this principle.

## Design Tradeoffs

**Prevention vs cleanup.** AIMS primarily prevents redundancies through precise, lattice-informed emission rather than cleaning them up after the fact. This inverts the traditional "insert liberally, eliminate aggressively" strategy (used by Swift). Prevention produces better results (fewer operations survive to reach the backend) but requires a more sophisticated analysis.

**Lightweight cleanup vs full elimination.** The remaining elimination is intentionally lightweight — bidirectional pair matching with cascading. A full dataflow-based elimination pass (like Swift's) would catch more patterns but adds complexity that AIMS's precise emission makes largely unnecessary. If profiling identifies remaining redundancies, the cleanup can be strengthened without changing the overall architecture.
