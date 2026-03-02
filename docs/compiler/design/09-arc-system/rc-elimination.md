---
title: "RC Elimination"
description: "Ori Compiler Design — Redundant RC Pair Removal"
order: 907
section: "ARC System"
---

# RC Elimination

## Overview

RC elimination removes redundant reference-count increment/decrement pairs. After the
initial ARC insertion pass conservatively inserts RcInc and RcDec instructions, many
of these operations cancel each other out. This pass identifies and removes such pairs,
reducing runtime overhead without changing program semantics.

The algorithm combines bidirectional intra-block dataflow, cross-block analysis along
single-predecessor edges, and ownership-aware elimination for borrowed variables.

Entry: `eliminate_rc_ops_dataflow(func, ownership) -> usize`

Returns the total number of pairs eliminated.

## Algorithm

### V1: Intra-Block Elimination

Each basic block is analyzed independently with two directional passes:

**Top-down (forward) pass:**

1. Scan the block's instructions in forward order.
2. When an `RcInc { var: x }` is encountered at position `i`, scan forward from
   `i+1` looking for an `RcDec { var: x }`.
3. If a matching `RcDec` is found at position `j` with no intervening use of `x`
   between positions `i` and `j`, eliminate both the RcInc and RcDec.
4. Continue scanning from the position after the eliminated pair.

**Bottom-up (backward) pass:**

1. Scan the block's instructions in reverse order.
2. When an `RcDec { var: x }` is encountered at position `j`, scan backward from
   `j-1` looking for an `RcInc { var: x }`.
3. If a matching `RcInc` is found at position `i` with no intervening use of `x`
   between positions `i` and `j`, eliminate both.
4. Continue scanning backward.

The two passes are complementary. The forward pass catches patterns where the increment
comes first and the decrement follows soon after. The backward pass catches patterns
where intervening instructions prevented the forward pass from finding a match, but
the same pair is visible when scanning from the other direction.

### Cross-Block Elimination

After intra-block passes, the algorithm examines block boundaries:

- If a block ends with an `RcInc { var: x }` (trailing increment) and its unique
  successor begins with an `RcDec { var: x }` (leading decrement), the pair can be
  eliminated across the edge.
- This only applies when the successor has a single predecessor. With multiple
  predecessors, the increment might not be present on all incoming edges, making
  elimination unsound.

### Ownership-Aware Elimination

When ownership analysis information is available, additional pairs can be removed:

- **Borrowed variables**: If a variable is `BorrowedFrom(source)` and the source
  variable is still live, the borrowed variable's refcount operations are redundant.
  The source keeps the value alive, so the borrow does not need its own increment
  and decrement.

- **Multi-predecessor join**: When an `RcInc { var: x }` is available on ALL incoming
  edges of a join block, it can be paired with an `RcDec { var: x }` at the join
  point. The increment is moved (conceptually) past the join and eliminated with the
  decrement.

## Safety Invariant

The algorithm only eliminates pairs in the order `RcInc; ...; RcDec` (increment before
decrement in program order). The reverse order — `RcDec; ...; RcInc` — is never
eliminated, because removing such a pair would mean the refcount drops to zero (and
the value is freed) before the subsequent increment, resulting in use-after-free.

This invariant is fundamental: an increment followed by a decrement means the refcount
goes up and then back down, a net no-op. A decrement followed by an increment means
the refcount goes down (potentially to zero, triggering deallocation) and then back up
on a possibly-freed value.

The "no intervening use" check is the second safety condition. If `x` is used between
the increment and decrement, removing the pair could cause the refcount to be too low
during that use. "Use" includes any instruction that reads `x` — loads, copies,
function calls with `x` as an argument, or any operation that might observe `x`'s
refcount.

## Cascading

Eliminating one pair can expose new pairs that were previously separated by the removed
instructions. For example:

```
RcInc { var: a }
RcInc { var: b }     // pair 1: RcInc(b)
RcDec { var: b }     // pair 1: RcDec(b)
RcDec { var: a }
```

After eliminating the `b` pair, the `a` pair becomes adjacent:

```
RcInc { var: a }
RcDec { var: a }
```

The algorithm iterates until no more pairs are found (fixpoint). In practice, cascading
rarely exceeds two or three iterations, since the initial ARC insertion pass does not
produce deeply nested redundant pairs.

## Interaction with Other Passes

RC elimination runs after initial ARC insertion and before reset/reuse detection. This
ordering is important:

- **Before reset/reuse**: Eliminating redundant pairs first simplifies the IR, making
  reset/reuse detection more precise. Fewer spurious RcDec instructions mean fewer
  false candidates.

- **After reuse expansion**: A second round of RC elimination runs after reuse
  expansion, since the expansion can introduce new redundant pairs (particularly on
  the slow path, where a fresh RcDec + Construct may produce pairs with surrounding
  increments).

The pass is also run after borrow inference, which provides the ownership information
needed for ownership-aware elimination.
