---
title: "Ownership-Event Optimization"
description: "Ori Compiler Design — AIMS Ownership-Event Redundancy Avoidance and Physical Projection"
order: 907
section: "AIMS"
---

# Ownership-Event Optimization

## Why AIMS Avoids Redundancies

Traditional ARC optimizers generate redundant retain/release pairs through sequential passes. AIMS instead optimizes the logical owner-credit plan before a physical mechanism is selected; the current `RcInc`/`RcDec` carrier is a transitional spelling of those events.

- **Perceus insertion** creates pairs when borrow inference later makes them unnecessary
- **Reuse expansion** generates slow-path pairs that interact with surrounding operations
- **Identity normalization** creates root-level pairs from projected ones

Traditional pipelines solve this with a separate physical RC-elimination pass. AIMS computes all ownership facts in one analysis and freezes one minimal logical event plan, so most redundancies never arise in any projection.

### How AIMS Prevents Each Source

**Contract-informed placement.** AIMS computes interprocedural contracts before realization, so borrowed parameters never receive transfer/release event pairs. A counter-based projection therefore has no pair to emit, while a counter-free projection receives the same minimal logical contract.

**Coordinated reuse.** AIMS derives reuse eligibility and ownership events from the same state map, so event placement accounts for slow/fast-path boundaries from the start.

**Projection-aware decisions.** AIMS tracks projected fields and parents through `BorrowSource` provenance, so logical ownership decisions bind the correct allocation roots directly.

**Precise cardinality.** Traditional Perceus insertion uses binary liveness (live vs dead) to determine last-use points, which can be imprecise at join points. AIMS's `Cardinality` dimension (`Absent` / `Once` / `Many`) provides finer-grained demand information, avoiding unnecessary owner-credit events when a variable is used exactly once on both branches.

## Remaining Elimination

While AIMS avoids the majority of redundancies, some patterns can still produce canceling pairs after realization:

- **Reuse expansion slow paths**: The `Reset`/`Reuse` → `IsShared` expansion generates both fast and slow paths, and the slow path may contain `RcInc`/`RcDec` pairs that the expansion's projection-increment erasure does not fully resolve.
- **Edge cleanup trampolines**: Critical edge splitting can create blocks where an increment immediately precedes a decrement on the same variable.

For these remaining cases, the logical pass may eliminate whole matched credit/debit pairs. If a physical plan selects counters and creates additional mechanism-local redundancy, a separate post-layout optimizer may eliminate it without changing the frozen AIMS trace.

### Safety Invariant

Logical pair elimination removes only a credit followed by its matched debit. The reverse order is never eliminable because it may discharge the final owner before a later credit exists.

A running logical owner-credit count witnesses the proof: a matched credit/debit pair nets zero, while debit-before-credit can reach zero and trigger logical cleanup prematurely. A counter-based projection obtains the familiar physical refcount corollary; the theorem does not mandate a counter.

The second condition forbids removal when an intervening use relies on the additional owner. Physical optimizers must also preserve their selected mechanism and synchronization identities.

### Intra-Block Elimination

When present, a bidirectional scan finds adjacent or near-adjacent `RcInc(x)`/`RcDec(x)` pairs with no intervening use of `x` and removes both. The pass cascades — removing one pair may expose another — but rarely needs more than 2-3 iterations.

### Cross-Block Elimination in the Transitional Carrier

If a block ends with `RcInc(x)` and its unique successor begins with `RcDec(x)`, both are removed. This only applies to single-predecessor successors — with multiple predecessors, the increment might not be present on all incoming edges.

## Measurement

The transitional `SynergyMetrics` tracks the current counter projection and
must be reported separately from logical AIMS event counts:

- `rc_ops_inserted`: Total `RcInc` + `RcDec` operations emitted
- `reuse_ops_inserted`: Total `Reset` + `Reuse` + `IsShared` operations
- `cow_annotations`: COW modes assigned (StaticUnique vs Dynamic vs StaticShared)
- `drop_hints`: Unique drop eligibility count

These metrics are reported during tracing (`ORI_LOG=ori_arc=debug`). Logical
verification validates contract/event agreement; physical RC counts diagnose
one selected plan and do not define AIMS correctness.

## Prior Art

**[Lean 4](https://github.com/leanprover/lean4)** performs RC elimination after its sequential RC insertion pass. Lean's approach uses forward-pass matching because its pipeline produces straightforward cascading patterns.

**[Swift](https://github.com/swiftlang/swift)** (`lib/SILOptimizer/ARC/`) uses bidirectional dataflow analysis for ARC optimization — a more general version that computes matching retain/release sets and eliminates them as a unit. This handles more complex patterns than pair-by-pair elimination but is also more expensive.

**Dead code elimination** in general-purpose optimizers (LLVM, GCC) performs analogous work — removing operations whose effects cancel out. RC optimization is a domain-specific instance of this principle.

## Design Tradeoffs

**Prevention vs cleanup.** AIMS primarily prevents logical redundancies through precise, lattice-informed event freezing rather than cleaning them up after the fact. This inverts the traditional "insert liberally, eliminate aggressively" strategy (used by Swift). Prevention produces a smaller shared obligation plan before physical projection but requires a more sophisticated analysis.

**Lightweight logical cleanup vs physical optimization.** The remaining logical elimination is intentionally lightweight — bidirectional pair matching with cascading. A full dataflow-based logical elimination pass (like Swift's) would catch more patterns but adds complexity that AIMS's precise event plan makes largely unnecessary. A selected physical plan may separately optimize mechanism-local redundancy without changing the frozen AIMS trace.
