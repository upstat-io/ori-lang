---
title: "RC Elimination"
description: "Ori Compiler Design — Redundant RC Pair Removal"
order: 907
section: "ARC System"
---

# RC Elimination

## Why RC Operations Cancel Out

The Perceus algorithm inserts RC operations at precise last-use points, but "precise" does not mean "minimal." Several sources create redundant pairs:

**Multi-use variables.** When a variable is used twice — once consumed by a `Construct` and once by a later `Apply` — the Perceus algorithm inserts an `RcInc` at the first use (to keep the value alive) and eventually an `RcDec` at the second use (the actual last use). If the `Apply` turns out to be borrowing (not consuming), the increment and subsequent decrement cancel out.

**Reuse expansion.** The reset/reuse expansion pass generates both fast and slow paths. The slow path decrements the old value and constructs a new one, which can produce `RcInc`/`RcDec` pairs that interact with surrounding operations.

**Identity normalization.** After RC identity propagation rewrites `RcInc(projected_field)` to `RcInc(root)`, new canceling pairs between root-level operations become visible.

**Ownership refinement.** When borrow inference marks a parameter as Borrowed, call sites no longer need RC for that argument. But the Perceus algorithm operates on the post-borrow IR, so some of its insertion decisions were made based on ownership that was later refined.

RC elimination is the cleanup pass that sweeps up all of these redundancies.

## Algorithm

The algorithm combines three strategies, iterated to fixpoint:

### Intra-Block: Forward Pass

Scan each block's instructions forward. When an `RcInc { var: x }` is found at position `i`, look ahead for an `RcDec { var: x }` at position `j` with no intervening use of `x`. If found, eliminate both.

### Intra-Block: Backward Pass

Scan each block's instructions backward. When an `RcDec { var: x }` is found at position `j`, look backward for an `RcInc { var: x }` at position `i` with no intervening use. If found, eliminate both.

The two directions are complementary — the forward pass catches inc-then-dec patterns visible from the top, while the backward pass catches patterns that intervening instructions hide from the forward scan.

### Cross-Block Elimination

After intra-block passes, examine block boundaries:

- If a block ends with an `RcInc { var: x }` (trailing increment) and its unique successor begins with an `RcDec { var: x }` (leading decrement), eliminate both across the edge.
- This only applies when the successor has a **single predecessor**. With multiple predecessors, the increment might not be present on all incoming edges, making elimination unsound.

### Ownership-Aware Elimination

When ownership information is available, additional pairs can be removed:

- **Borrowed variables**: If a variable is `BorrowedFrom(source)` and the source is still live, the borrowed variable's RC operations are redundant — the source keeps the value alive.
- **Multi-predecessor join**: When an `RcInc { var: x }` is available on ALL incoming edges of a join block, it can be paired with an `RcDec { var: x }` at the join point.

## The Safety Invariant

The algorithm **only** eliminates pairs in the order `RcInc; ...; RcDec` (increment before decrement in program order). The reverse — `RcDec; ...; RcInc` — is **never** eliminated.

This is not an arbitrary restriction. An increment followed by a decrement is a net no-op: the refcount goes up and then back down. A decrement followed by an increment means the refcount drops first (potentially reaching zero, triggering deallocation) and then goes back up — but on a value that may have already been freed. Eliminating a dec-then-inc pair would cause use-after-free.

The "no intervening use" check is the second safety condition. If `x` is read between the increment and decrement, removing the pair would leave the refcount too low during that read — another instruction might observe an incorrect refcount or trigger premature deallocation.

## Cascading

Eliminating one pair can expose new pairs. Consider:

```
RcInc { var: a }
  RcInc { var: b }     // pair (b): inner
  RcDec { var: b }     // pair (b): inner
RcDec { var: a }
```

After eliminating the `b` pair, the `a` pair becomes adjacent and can be eliminated:

```
RcInc { var: a }
RcDec { var: a }       // now visible
```

The algorithm iterates until fixpoint — when no more pairs are found. In practice, cascading rarely exceeds two or three iterations, since Perceus insertion does not produce deeply nested redundant pairs.

## Pipeline Position

RC elimination runs as the **last** optimization pass in the ARC pipeline:

```mermaid
flowchart LR
    Insert["RC Insertion
    (Perceus)"] --> Reuse["Reset/Reuse
    Detection + Expansion"]
    Reuse --> Identity["RC Identity
    Normalization"]
    Identity --> Elim["RC Elimination
    (this pass)"]
    Elim --> Hints["Drop Hints"]

    classDef native fill:#5c3a1e,stroke:#f59e0b,color:#fef3c7

    class Insert,Reuse,Identity,Elim,Hints native
```

This ordering is deliberate — each preceding pass can create new redundant pairs:
- Reset/reuse expansion creates slow-path pairs
- Identity normalization creates root-level pairs from projected ones
- The elimination pass runs last and catches everything

## Prior Art

**[Lean 4](https://github.com/leanprover/lean4)** performs similar RC elimination after its RC insertion pass. Lean's approach is simpler — primarily forward-pass matching — because Lean's IR structure produces fewer nested patterns that require bidirectional analysis.

**[Swift](https://github.com/swiftlang/swift)** (`lib/SILOptimizer/ARC/`) uses bidirectional dataflow analysis for ARC optimization, which is a more general version of the same idea. Swift's analysis computes matching retain/release sets and eliminates them as a unit, which can handle more complex patterns than pair-by-pair elimination but is also more expensive.

**Dead code elimination** in general-purpose optimizers (LLVM, GCC) performs analogous work — removing operations whose effects cancel out. RC elimination is a domain-specific instance of this general principle, specialized for the increment/decrement pattern.

## Design Tradeoffs

**Pair-by-pair vs set-based.** Ori eliminates RC operations one pair at a time with cascading, while Swift eliminates matching sets simultaneously. Pair-by-pair is simpler to implement and reason about correctness, but may miss patterns that set-based analysis would catch (e.g., three increments followed by three decrements in different orders). In practice, Perceus insertion produces clean pair patterns that the simpler approach handles well.

**Iterative cascading vs single-pass.** The cascading approach runs multiple passes until fixpoint. A single-pass approach with a more sophisticated data structure (e.g., tracking pending increments in a stack) could find all cascading opportunities in one pass. The iterative approach was chosen for simplicity — the pass count is small, and the per-pass cost is low (linear in block size).

**Ownership awareness.** Using ownership information for additional elimination is optional — the base algorithm (direction + cross-block) is correct without it. Adding ownership awareness increases the elimination count but couples the pass to the ownership analysis output.
