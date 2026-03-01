---
title: "ARC System Overview"
description: "Ori Compiler Design — ARC Pipeline Overview"
order: 900
section: "ARC System"
---

# ARC System Overview

Ori uses value semantics — every assignment is a logical copy, every variable owns its data, there is no shared mutable state. Naively, this means every list push copies the entire list. Every struct update allocates a fresh struct. Every function call that passes a collection duplicates it.

The ARC system makes value semantics fast. It is a compiler pipeline (`ori_arc` crate) that transforms canonical IR into memory-managed code with explicit reference counting, then systematically eliminates the overhead that value semantics would normally impose. The crate is **backend-independent** — it has no LLVM dependency. The separate `arc_emitter` in `ori_llvm` translates ARC IR to LLVM IR.

ARC IR is the **sole codegen path** — all LLVM codegen flows through ARC IR.

## The Problem

Without optimization, value semantics requires:

- **RC at every call site** — passing a list to a function means incrementing its refcount, then decrementing when the function returns. Even if the function only reads the list.
- **Copy on every mutation** — `list.push(x)` must allocate a new list, copy all elements, append `x`, and free the old list. O(n) per push.
- **Allocation churn on pattern matching** — `match node { Leaf(v) -> Leaf(f(v)) }` deconstructs a node, frees the old allocation, and allocates a new one of the same type and size.
- **Redundant RC pairs** — conservative RC insertion generates inc/dec pairs that cancel out, wasting cycles on atomic operations.

Each of these problems has been solved individually in other systems. Ori's contribution is combining all the solutions into a single pipeline where each pass feeds the next, and the compounding effect eliminates overhead that any single technique would leave on the table.

## How the Pipeline Eliminates Each Cost

### 1. Type Classification — skip RC entirely for value types

Every type is classified as `Scalar` (int, bool, small structs — no RC needed), `DefiniteRef` (str, collections, closures — always needs RC), or `PossibleRef` (unresolved generics — conservative fallback). Classification is monomorphized: `Option<int>` is Scalar (tag + int, no heap pointer), `Option<str>` is DefiniteRef. This means roughly half of all variables in a typical program need zero RC operations.

See [ARC IR](arc-ir.md) for the full classification system.

### 2. Borrow Inference — eliminate RC at call sites

A global SCC-based fixed-point analysis classifies each function parameter as **Borrowed** (callee only reads it) or **Owned** (callee may store, return, or transfer it). Borrowed parameters skip RC entirely at the call site — no increment on call, no decrement on return. The analysis handles mutually recursive functions via Tarjan's SCC decomposition, iterating within each SCC until ownership converges. Promotion is monotonic (Borrowed → Owned, never backward), guaranteeing termination.

The result: a function like `len(list)` borrows its argument. The caller never touches the refcount. In a language without borrow inference, every `len()` call costs two atomic operations.

See [Borrow Inference](borrow-inference.md) for the algorithm.

### 3. Perceus RC Insertion — place RC at precise last-use points

Once ownership is known, RC operations are placed mechanically via backward liveness analysis. Each variable gets an `RcDec` at its last use, and an `RcInc` at each additional use beyond the first. Borrowed parameters skip RC entirely. This is deterministic — no heuristics, no surprises.

The precision matters because it sets up the later passes. Conservative placement (increment early, decrement late) would be correct but would hide optimization opportunities from reset/reuse and elimination.

See [RC Insertion](rc-insertion.md) for the Perceus algorithm.

### 4. Reset/Reuse — turn free+alloc into in-place mutation

When an `RcDec` (about to free a value) is followed by a `Construct` of the same type (about to allocate), the pipeline detects this and replaces both with a reset/reuse pair. At runtime, if the old value is uniquely owned (RC == 1), the allocation is reused in-place — no free, no alloc, no copy. If it's shared (RC > 1), the slow path decrements the old ref and allocates fresh.

This is the optimization that makes functional pattern matching competitive with imperative mutation. A list map that walks a linked list, dropping each old node and constructing a new one, reuses every node allocation on the fast path.

The expansion phase generates both code paths (unique vs shared) with an `IsShared` guard, and also performs sub-optimizations: projection-increment erasure (projected fields of a unique parent are implicitly owned, so their RcInc is unnecessary) and self-set elimination (skip writing a field back to its original value).

See [Reset/Reuse](reset-reuse.md) for detection and expansion.

### 5. RC Identity Normalization — enable elimination across projections

When `v1 = Project(v0, field)`, both `v1` and `v0` share the same RC identity — incrementing `v1` is the same as incrementing `v0`. The identity pass normalizes all RC operations to target canonical roots, so `RcInc(v1)` followed by `RcDec(v0)` can be recognized as a canceling pair.

Without this, struct field manipulation leaves orphaned RC operations that the eliminator can't match.

See [RC Elimination](rc-elimination.md).

### 6. RC Elimination — remove redundant pairs

A bidirectional intra-block dataflow pass finds `RcInc(x)` / `RcDec(x)` pairs with no intervening use and removes both. A top-down scan catches inc-then-dec; a bottom-up scan catches pairs the forward pass misses. Cross-block elimination handles pairs split across basic block boundaries along single-predecessor edges. The pass cascades until fixpoint — removing one pair can expose another.

This is a cleanup pass. Perceus insertion is deliberately precise rather than minimal, because the downstream passes (reset/reuse, identity normalization) can create new cancellation opportunities. Elimination runs last and sweeps up everything that became redundant.

See [RC Elimination](rc-elimination.md).

### 7. COW Collections — O(1) mutation for unique owners

At runtime, every collection mutation (list push, map insert, set remove) checks `ori_rc_is_unique()`. If the collection has RC == 1, it mutates in-place — O(1) amortized for push, O(n) only when the buffer grows. If shared (RC > 1), it copies the buffer and decrements the old ref.

This is what makes value semantics practical for collections. Without COW, `list.push(x)` in a loop is O(n^2). With COW, it's O(n) — identical to imperative mutation when the list is uniquely owned, which is the common case after borrow inference has eliminated unnecessary sharing.

See [Collections & COW](../11-runtime/collections-cow.md).

### 8. Static Uniqueness Analysis — eliminate runtime checks

The final layer proves at compile time that certain values are uniquely owned (RC == 1), allowing codegen to emit only the fast path — no `IsShared` check, no branch, no slow-path code. The analysis uses a forward dataflow lattice (`Unique` / `MaybeShared` / `Shared`) with the key insight that COW operations always produce unique results (whether the fast path mutated in-place or the slow path copied, the result has RC == 1).

When the compiler can prove uniqueness, a tight loop mutating a list compiles to straight-line code with no branches — identical to what a C programmer would write by hand.

## The Compounding Effect

Each technique handles a different dimension of the problem, and they multiply:

| Without | Cost | With | Residual |
|---------|------|------|----------|
| Classification | RC ops on ints, bools | Classification | Zero RC on ~50% of variables |
| Borrow inference | RC at every call site | + Borrow inference | Zero RC for read-only params |
| Precise insertion | RC at scope boundaries | + Perceus | RC only at true last-use |
| Reset/reuse | alloc+free per constructor | + Reset/reuse | In-place reuse when unique |
| RC elimination | Redundant inc/dec pairs | + Elimination | Minimal surviving RC ops |
| Identity normalization | Orphaned ops on projections | + Normalization | No leaked projection RC |
| COW runtime | O(n) per collection mutation | + COW | O(1) when unique |
| Static uniqueness | Branch per mutation | + Uniqueness | Branchless fast path |

The net result: a program written in pure value semantics — no mutation syntax, no ownership annotations, no lifetime parameters — compiles to code that mutates in-place, reuses allocations, skips RC operations on scalars and borrowed params, and eliminates branches on provably-unique values. The programmer writes functional code; the compiler generates imperative performance.

## Pipeline Position

```text
Source → Lex → Parse → Type Check → Canonicalize ─┬─→ ori_eval  (interprets CanExpr)
                                      (ori_canon)  └─→ ori_arc   (lowers CanExpr → ARC IR)
                                                           │
                                                      LLVM codegen (ori_llvm/arc_emitter)
```

## Pipeline Passes

The ARC pipeline runs in a strict, load-bearing order. Do NOT reorder or skip passes.

```text
CanExpr → lower_function_can() → ArcFunction
  │
  │── Global pass (per-module) ──────────────────────────────
  │  1. infer_borrows()          — fixed-point SCC-based ownership
  │  2. apply_borrows()          — write ownership back to params
  │
  │── Per-function pipeline (run_arc_pipeline) ─────────────
  │  1. compute_var_reprs()               — value representations
  │  2. infer_derived_ownership()         — per-variable ownership
  │  3. compute_refined_liveness()        — liveness + aliasing
  │  4. insert_rc_ops_with_ownership()    — RC insertion (Perceus)
  │  5. insert_external_invoke_cleanup()  — invoke edge cleanup
  │  6. DominatorTree::build()            — dominator tree (post-RC)
  │  7. PostDominatorTree::build()        — post-dominator tree
  │  8. compute_refined_liveness()        — re-liveness (post-RC CFG)
  │  9. detect_reset_reuse_cfg()          — reset/reuse detection
  │ 10. expand_reset_reuse()              — reuse expansion
  │ 11. propagate_rc_identity()           — RC identity normalization
  │ 12. eliminate_rc_ops_dataflow()       — RC elimination
  └──────────────────────────────────────────────────────────
```

Lowering (`lower_function_can`) converts `CanExpr` into ARC IR before the pipeline runs — it is not a pipeline step itself. Borrow inference runs **once** for the entire module before the per-function pipeline.

Each pass depends on prior passes:
- Borrow inference needs classification (to skip scalars)
- RC insertion needs borrow ownership (to skip borrowed params)
- Reset/reuse needs RC insertion (to find RcDec/Construct pairs)
- Reuse expansion needs detection (to know what to expand)
- RC elimination needs identity normalization (to match projected pairs)
- Static uniqueness needs COW paths (to prove results are unique)

## Entry Points

| Function | Purpose |
|----------|---------|
| `run_arc_pipeline()` | Single function — runs passes 1-12 |
| `run_arc_pipeline_all()` | Batch — infers borrows globally, then runs per-function pipeline on each |

Consumers always use these entry points, never manual pass sequencing.

## Debugging

| Tool | Command |
|------|---------|
| **Tracing** | `ORI_LOG=ori_arc=debug ori build file.ori` |
| **Verbose tracing** | `ORI_LOG=ori_arc=trace ORI_LOG_TREE=1 ori build file.ori` |
| **ARC IR dump** | `ORI_DUMP_AFTER_ARC=1 ori build file.ori` |
| **RC trace (runtime)** | `ORI_TRACE_RC=1 ./binary` |
| **Leak check** | `ORI_CHECK_LEAKS=1 ./binary` |
| **RC balance** | `diagnostics/rc-stats.sh file.ori` |
| **Codegen audit** | `diagnostics/codegen-audit.sh --strict file.ori` |
| **Full diagnosis** | `diagnostics/diagnose-aot.sh --valgrind file.ori` |
| **Interpreter vs AOT** | `diagnostics/dual-exec-debug.sh file.ori` |

## Related Documents

- [ARC IR](arc-ir.md) — IR definitions and type classification
- [Lowering](lowering.md) — CanExpr → ARC IR
- [Borrow Inference](borrow-inference.md) — Global ownership inference
- [Liveness](liveness.md) — Backward dataflow analysis
- [RC Insertion](rc-insertion.md) — Perceus algorithm
- [Reset/Reuse](reset-reuse.md) — In-place constructor reuse
- [RC Elimination](rc-elimination.md) — Redundant pair removal
- [Drop Descriptors](drop-descriptors.md) — Per-type drop generation
- [Decision Trees](decision-trees.md) — Pattern compilation in ARC IR
- [ARC Emitter](../10-llvm-backend/arc-emitter.md) — ARC IR → LLVM IR (in LLVM backend)
- [Runtime RC](../11-runtime/reference-counting.md) — Runtime RC implementation
