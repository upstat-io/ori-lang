---
title: "Reset/Reuse"
description: "Ori Compiler Design — In-Place Constructor Reuse"
order: 906
section: "ARC System"
---

# Reset/Reuse

## Overview

Reset/Reuse identifies opportunities for in-place constructor reuse. When a value's
reference count is decremented and a new value of the same type is constructed shortly
after, the old allocation can be reused instead of being freed and re-allocated. This
eliminates a free+malloc pair on the fast path and improves cache locality.

Functional programs frequently deconstruct a value and immediately construct a replacement of the same type — the classic example being a linked-list map that walks down a list, dropping each old node and allocating a new one with a transformed payload. With reset/reuse, each old node's memory is handed directly to the new node.

The transformation proceeds in two phases:

1. **Detection** — identify RcDec/Construct pairs that can be linked
2. **Expansion** — lower each Reset/Reuse pair into conditional fast/slow paths

After expansion, no Reset or Reuse instructions remain in the IR. They are fully
replaced by IsShared checks, branches, and concrete Set/Construct instructions.

## Detection Algorithm

Entry: `detect_reset_reuse_cfg(func, classifier, dom_tree, post_dom_tree, refined, pool)`

Detection operates at two granularities: within a single basic block (the common case)
and across block boundaries (for patterns like linked-list traversal).

### Intra-Block Detection

The function `detect_reset_reuse` scans each basic block forward:

1. For each instruction at position `i`, check if it is an `RcDec { var: x }`.
2. Look ahead from position `i+1` for a matching `Construct { dst, ty, ctor, args }`
   at position `j`.
3. If a candidate is found, verify all constraints (see below).
4. If valid, rewrite: replace the `RcDec` with `Reset { var: x, token: t }` and the
   `Construct` with `Reuse { token: t, dst, ty, ctor, args }`.
5. Continue scanning forward from `j+1`.

The forward scan finds the nearest eligible Construct, minimizing the live range of
the reuse token.

### Cross-Block Detection

Cross-block detection extends the algorithm to patterns where the decrement and
construction occur in different basic blocks — the canonical example being a recursive
function that decrements a list node in block B0 and constructs a replacement in a
dominated block B2.

The algorithm uses:

- **Dominator tree**: The block containing the RcDec must dominate the block containing
  the Construct. This ensures the reuse token is available on all paths reaching the
  Construct.
- **Refined liveness**: The decremented variable must be "only live-for-drop" in all
  intermediate blocks between the RcDec and the Construct. If the variable is used
  for anything other than cleanup in those blocks, reuse is unsafe.
- **Post-dominator tree**: Used to verify that the Construct block post-dominates the
  Reset block within the relevant subgraph, ensuring the token is always consumed.

When cross-block detection succeeds, the same Reset/Reuse rewrite is applied, but the
two instructions reside in different blocks. The reuse token flows through block
parameters as needed.

## Constraints

A Reset/Reuse pairing is only valid when all of the following hold:

1. **Type match**: The type of the decremented variable must equal the type of the
   Construct. The old allocation's size and layout must be compatible with the new
   value. This is a conservative structural check — no subtyping or coercion.

2. **No intervening use**: The decremented variable `x` must not be used between the
   RcDec at position `i` and the Construct at position `j`. Any read of `x` in that
   window would observe a value whose refcount has already been decremented, which is
   unsound.

3. **Needs RC**: The type must be reference-counted (heap-allocated). Stack-allocated
   values have no allocation to reuse. The `classifier` determines this.

4. **Not a collection constructor**: Collection types (lists, maps, sets) use a
   separate buffer allocation whose layout is incompatible with the in-place Set
   strategy. Their internal buffer size depends on capacity, not just element count,
   so reuse at the constructor level is not meaningful.

## Transformation

The detection phase produces paired Reset/Reuse instructions:

```
Before:
  RcDec { var: x }
  ...
  Construct { dst: y, ty: T, ctor: C, args: [a, b, c] }

After:
  Reset { var: x, token: t }
  ...
  Reuse { token: t, dst: y, ty: T, ctor: C, args: [a, b, c] }
```

The reuse token `t` is a fresh variable that represents the potential to reuse `x`'s
allocation. It has no runtime representation of its own — it is a compile-time artifact
that links the Reset to the Reuse and is eliminated during expansion.

## Reuse Expansion

Entry: `expand_reset_reuse(func, classifier, pool)`

Expansion lowers each Reset/Reuse pair into a conditional structure with fast and slow
paths. After expansion, the IR contains only concrete operations — no Reset or Reuse
instructions remain.

### Expansion Steps

For each Reset/Reuse pair:

1. **Analyze projections**: Scan instructions before the Reset to find all
   `Project { src: x, field: i }` instructions that extract fields from the value
   being reset. Build a projection map recording which fields have been projected
   and into which variables.

2. **Projection-increment erasure**: Identify RcInc instructions on projected field
   variables that appear between the Reset and Reuse. On the fast path (unique
   ownership), these increments are redundant — the projected fields are implicitly
   owned by the unique parent, so their refcounts need not be bumped. Mark these
   increments for erasure on the fast path.

3. **Generate IsShared check**: Emit an `IsShared { var: x }` instruction that tests
   whether the refcount of `x` is greater than 1. This produces a boolean that
   drives the branch.

4. **Fast path (unique, RC == 1)**: When the value is uniquely owned:
   - The allocation is reused in place.
   - For each constructor argument, emit a `Set { base: x, field: i, value: arg }`
     instruction that writes the new field value directly into the existing allocation.
   - Apply **self-set elimination**: if a Set instruction writes a field back to the
     same position it was projected from (i.e., `Set { base: x, field: i, value: v }`
     where `v` was produced by `Project { src: x, field: i }`), the Set is a no-op
     and is omitted.
   - Erased RcInc instructions from step 2 are not emitted on this path.
   - The result variable points to the same allocation as `x`.

5. **Slow path (shared, RC > 1)**: When the value is shared:
   - Emit `RcDec { var: x }` to release this reference's claim.
   - Emit a fresh `Construct { dst, ty, ctor, args }` to allocate a new value.
   - Any RcInc instructions that were erased on the fast path are restored here,
     since the slow path does not inherit implicit ownership of projected fields.

6. **Merge block**: A join block receives the result from whichever path was taken.
   Both paths assign to the same destination variable, and control merges after the
   conditional.

### Sub-Optimizations

#### Projection-Increment Erasure

When the fast path reuses a unique allocation, any fields projected out of that
allocation before the Reset are implicitly retained — their parent's refcount is 1,
meaning the parent (and by extension its fields) are exclusively owned. Incrementing
a projected field's refcount before storing it back into a sibling slot is unnecessary
on this path.

The erased increments are recorded and restored on the slow path, where the original
value is shared and projected fields need proper refcount management.

This sub-optimization is particularly effective for constructor transformations that
pass most fields through unchanged (e.g., updating one field of a record).

#### Self-Set Elimination

A Set instruction that writes a value back to the same field it was projected from
is a no-op:

```
v = Project { src: x, field: 2 }
Set { base: x, field: 2, value: v }   // eliminated — writes v back where it came from
```

This arises naturally when a match expression deconstructs a value, transforms one
field, and reconstructs the same variant with the other fields unchanged. Self-set
elimination avoids the redundant store.

## FBIP Diagnostics

"Functional But In-Place" (FBIP) is a read-only diagnostic pass that reports on the
effectiveness of the reset/reuse optimization. It does not modify the IR.

The diagnostic reports two categories:

- **Achieved reuse**: Reset/Reuse pairs that were successfully detected and will use
  in-place mutation on the fast path. Each report includes the source location, the
  type being reused, and the constructor involved.

- **Missed reuse**: Cases where an RcDec and a Construct of the same type exist in
  proximity but could not be paired. Each report includes the reason the pairing
  failed — for example, an intervening use of the decremented variable, a type
  mismatch, or a collection constructor.

FBIP diagnostics are intended for compiler developers and advanced users tuning
allocation behavior. They are emitted when the `ORI_DUMP_AFTER_ARC=1` flag is set.

## Reference Implementations

- Lean 4: `src/Lean/Compiler/IR/ExpandResetReuse.lean` — token-based reset/reuse
- Koka: "Perceus: Garbage Free Reference Counting with Reuse" (Reinking et al., 2021) — FBIP
- Swift: `lib/SILOptimizer/ARC/` — unique-reference fast path for COW containers
