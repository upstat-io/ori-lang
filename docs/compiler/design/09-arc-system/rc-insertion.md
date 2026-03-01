---
title: "RC Insertion"
description: "Ori Compiler Design — Perceus Algorithm RC Placement"
order: 905
section: "ARC System"
---

# RC Insertion

The RC insertion pass places `RcInc` and `RcDec` instructions precisely using liveness analysis results. This is the **Perceus algorithm**: every heap-allocated value is freed exactly once at its last use, and additional uses get `RcInc`. The pass transforms ARC IR functions in-place, inserting RC operations that the LLVM backend will later lower to runtime calls.

**Source**: `compiler/ori_arc/src/rc_insert/mod.rs`, `insert.rs`, `annotate.rs`, `edge_cleanup.rs`

## Algorithm: Perceus (Liveness-Based RC Insertion)

Entry point: `insert_rc_ops_with_ownership(func, classifier, liveness, ownership, sigs, pool)`

The algorithm processes each block with a backward scan, maintaining a running live set initialized from `live_out`:

### Step 1: Terminator Uses

Variables used in the terminator that are already in the live set get `RcInc` -- they must survive past the terminator into successor blocks. New uses are added to the live set.

Special cases:
- **Return**: The returned variable is treated as an owned position. Borrowed parameters and borrowed-derived variables at return positions get `RcInc` to transfer ownership to the caller.
- **Invoke**: Per-argument ownership is read from the pre-computed `arg_ownership` field. Borrowing arguments (external C runtime functions, or callee params marked `Borrowed`) are added to `live` but skip `RcInc` -- the callee borrows without consuming.

### Step 2: Instruction Backward Pass

For each instruction in reverse order:

**Definitions**: If the defined variable (`dst`) is not in the live set, it is dead immediately -- emit `RcDec` after the definition. Otherwise, remove it from the live set (its definition satisfies its liveness).

**Borrowing uses** (PrimOps, scalar projections, all-borrowed Apply): The operation reads its arguments without consuming them. Arguments not in the live set are at their last use -- emit `RcDec` after the borrowing operation. Arguments already in the live set need no `RcInc` because the operation does not hold a reference.

**Consuming uses** (Construct, PartialApply, Apply with owned args, ApplyIndirect): The operation takes ownership of its arguments. If an argument is already in the live set, emit `RcInc` (multi-use -- the value must survive past this consumption). Duplicate arguments in the same instruction (e.g., `Apply { args: [x, x] }`) get exactly one `RcInc` for the second occurrence.

### Step 3: Block and Function Parameters

After processing the body, any block parameter not in the live set gets `RcDec` (unused parameter -- the value was passed but never read). For the entry block, function parameters with `Ownership::Owned` that are not in the live set also get `RcDec`.

### Step 3.5: Invoke Dst Definitions

Invoke `dst` variables defined at a block's entry (via predecessor Invoke terminators) are treated like block parameters: if not in the live set, they get `RcDec`.

### Result

The new body is built in reverse during the backward walk, then reversed at the end to produce the correct forward order. Span information is preserved and aligned with the new instruction sequence.

## Borrowed Parameter Handling

Borrowed parameters (from borrow inference) and variables derived from them receive special treatment:

- **Borrowed params** completely skip all RC tracking -- no `RcInc`, no `RcDec`, not added to the live set. The only exception is at `Return`: returning a borrowed param transfers ownership to the caller, requiring `RcInc`.

- **Borrowed-derived variables** (projections or aliases of borrowed params, tracked via `DerivedOwnership::BorrowedFrom`) skip normal RC tracking but get `RcInc` at **owned positions** -- places where the value will be stored on the heap:
  - `Construct` arguments
  - `PartialApply` captures (unless the capture is at a borrowed callee position and the closure does not escape)
  - `Apply`/`ApplyIndirect` arguments at owned positions
  - `Return` values

The `needs_rc_trackable()` helper returns `false` for borrowed params, borrowed-derived vars, and scalars.

## Borrowing vs Consuming Instructions

The `is_borrowing_instr()` function classifies instructions:

**Borrowing** (read without consuming):
- `PrimOp` -- arithmetic, comparison, logical, string ops. Exception: `Binary(Add)` on list-typed operands is consuming (COW list concat).
- `Project` with scalar result -- extracts a field without consuming the parent. Non-scalar projections transfer ownership.
- `Apply` with all-borrowed `arg_ownership` -- external C runtime functions and borrowing builtins.

**Consuming** (ownership transfer):
- `Construct`, `PartialApply` -- store args on the heap.
- `Apply`, `ApplyIndirect` with owned args -- transfer to callee.
- `Project` with non-scalar result -- transfers ownership from parent to field.

## Closure Capture Analysis

When `PartialApply` captures a borrowed-derived variable, the normal rule requires `RcInc` (the closure stores the value). But this can be safely skipped when both conditions hold:

1. The callee expects the corresponding parameter as `Borrowed` (it will not store or escape the value).
2. The closure does not escape the current block (`dst` is not in `live_out`).

In this case, the captured value remains alive through its borrow root (a function parameter with lifetime spanning the entire function). This follows Lean 4's `Borrow.lean` pattern for closure captures.

## Argument Ownership Annotation

Entry point: `annotate_arg_ownership(func, sigs, interner, builtins, pool)`

Before RC insertion runs, `annotate_arg_ownership()` populates the `arg_ownership` field on every `Apply` and `Invoke` instruction. This is the single point where external-callee detection and per-param ownership lookup happen. All downstream passes read from the field rather than re-deriving ownership.

### Classification Rules

- **Ori functions with known signatures**: Per-param ownership from `AnnotatedSig` (Borrowed or Owned).
- **External C runtime** (`ori_*` prefix, not in sigs): All arguments Borrowed -- they do not participate in Perceus ownership.
- **Borrowing builtins** (`len`, `is_empty`, etc.): All arguments Borrowed -- compiled inline by the LLVM emitter.
- **COW receiver-only methods** (`remove`, `union`, etc.): Receiver Owned, other args Borrowed.
- **Unknown callees**: Conservative all-Owned.

### COW List Overrides

After initial classification, `apply_consuming_overrides()` checks whether the receiver is a list type. If so:

- **Consuming receiver methods** (`push`, `pop`, `reverse`, `sort`, etc.): arg[0] is marked Owned. The runtime handles the old buffer's RC internally.
- **Consuming second-arg methods** (`add`, `concat`): arg[1] is also marked Owned. The runtime takes ownership of list2's buffer.

These overrides are type-qualified: `"add"` and `"concat"` are shared names -- borrowing for strings, consuming for lists.

## Edge Cleanup

After per-block RC insertion, variables that are live at a predecessor's exit but not live at a successor's entry create "edge gaps" -- they need `RcDec` at the transition. The `insert_edge_cleanup()` pass handles these gaps.

### Single-predecessor blocks

`RcDec` instructions are prepended to the block's body.

### Multi-predecessor blocks with identical gaps

All predecessors have the same set of stranded variables -- `RcDec` is prepended to the block's body.

### Multi-predecessor blocks with differing gaps

Different predecessors have different stranded variable sets. A **trampoline block** is created for each edge that needs cleanup (critical edge splitting):

```text
Before:                          After:
  pred_A ──→ succ                  pred_A ──→ trampoline_A ──→ succ
  pred_B ──→ succ                  pred_B ──→ succ  (no gap)

  trampoline_A:
    RcDec v1
    RcDec v3
    Jump succ
```

If the successor has block parameters, the trampoline accepts and forwards them so the edge split is transparent to the rest of the IR.

## External Invoke Cleanup

Entry point: `insert_external_invoke_cleanup(func, classifier, liveness, pool)`

A companion post-pass for Invoke terminators with borrowing arguments. Together with the per-arg borrowing detection in `process_terminator_uses`, this implements correct RC at Invoke call sites:

1. `process_terminator_uses` identifies borrowing args and adds them to `live` (keeps them alive through the call) but skips `RcInc`.

2. `insert_external_invoke_cleanup` inserts `RcDec` at the start of the normal successor block for borrowing args whose **last use** is the Invoke. Args still in `live_out` are needed later and will be decremented at their actual last-use point by the normal Perceus logic.

This pass uses the **original** (pre-RC-insertion) liveness data, which correctly reflects user-code liveness without RC op interference.

## RcStrategy

Each `RcInc` and `RcDec` instruction carries an `RcStrategy` that tells the LLVM backend how to emit the RC operation:

- **HeapPointer**: Standard heap-allocated value -- call `ori_rc_inc`/`ori_rc_dec` on the pointer.
- **Closure**: Fat pointer (function pointer + environment) -- extract env_ptr from field 1, null-check, then inc/dec.
- **InlineEnum**: Enum with mixed scalar/ref variants -- check tag before inc/dec.
- **AggregateFields**: Struct with mixed scalar/ref fields -- inc/dec each RC field individually.

The strategy is computed from `ValueRepr` (determined by `compute_var_reprs()`) and the type pool. The `rc_strategy()` helper resolves this per-variable, with a `HeapPointer` fallback when Pool is unavailable (test-only path).

## Pipeline Integration

```text
annotate_arg_ownership()              -- populates arg_ownership on Apply/Invoke
    |
    v
insert_rc_ops_with_ownership()        -- core Perceus backward walk
    |
    v
insert_external_invoke_cleanup()      -- Invoke borrowed-arg post-cleanup
    |
    v
insert_edge_cleanup()                 -- inter-block edge gap RcDec
```

A `debug_assert!` verifies that the IR contains no existing `RcInc`/`RcDec` before insertion -- running RC insertion twice is a pipeline ordering error.

## References

- Lean 4: `src/Lean/Compiler/IR/RC.lean` -- liveness-based RC insertion
- Koka: Perceus paper (Reinking et al. 2021), Section 3.2 -- precise reference counting
- Swift: `lib/SILOptimizer/ARC/` -- bidirectional RC elimination
