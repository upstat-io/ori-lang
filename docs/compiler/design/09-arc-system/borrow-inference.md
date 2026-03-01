---
title: "Borrow Inference"
description: "Ori Compiler Design — Global Ownership Inference"
order: 903
section: "ARC System"
---

# Borrow Inference

Borrow inference determines whether each function parameter is **borrowed** (the callee does not retain the value -- no RC operations needed at the call site) or **owned** (the callee may retain the value -- the caller must `rc_inc` before the call, and the callee must `rc_dec` when done).

This is a **per-module global pass** that analyzes all functions simultaneously via fixed-point iteration on strongly connected components (SCCs). It runs once before the per-function pipeline and produces an `AnnotatedSig` for every function, which downstream passes (derived ownership, RC insertion) consume.

The implementation follows Lean 4's approach (`src/Lean/Compiler/IR/Borrow.lean`).

**Source**: `compiler/ori_arc/src/borrow/mod.rs`, `per_scc.rs`, `derived.rs`, `callees.rs`, `builtins/mod.rs`

## Algorithm: SCC-Based with Fixed-Point

Entry point: `infer_borrows_scc(functions, classifier, borrowing_builtins) -> FxHashMap<Name, AnnotatedSig>`

### Step 1: Decompose

Build an inter-procedural call graph from all functions in the module. Extract direct callees from `Apply`, `PartialApply`, and `Invoke` instructions (indirect calls via `ApplyIndirect` are excluded -- their callees are unknown at compile time). Compute SCCs using Tarjan's algorithm.

### Step 2: Initialize

All non-scalar parameters start as `Borrowed`. Scalar parameters (int, float, bool, etc.) start as `Owned` because they have no reference count -- there is nothing to borrow. The classifier (`ArcClassification::is_scalar()`) makes this determination.

```
fn initialize_single_borrowed(func, classifier) -> AnnotatedSig {
    for each param:
        if classifier.is_scalar(param.ty):
            ownership = Owned    // no RC needed
        else:
            ownership = Borrowed // optimistic default
}
```

### Step 3: Per-SCC Analysis (Topological Order)

SCCs are processed in topological order -- callees before callers. This ensures that when analyzing a function, all of its non-recursive callees already have finalized signatures.

- **Non-recursive SCCs** (single function, no self-call): Single-pass analysis via `infer_borrow_single()`. The function's body is scanned once; any parameter usage that requires ownership triggers promotion from Borrowed to Owned.

- **Recursive SCCs** (mutual recursion): Fixed-point iteration within the SCC via `infer_borrow_fixed_point()`. All SCC members are initialized as Borrowed, then repeatedly scanned until no parameter changes ownership. Each iteration sees the latest in-progress signatures of co-members via `local_sigs`, while external callees use their stable final sigs from `external_sigs`.

### Step 4: Convergence

Ownership is **monotonic**: parameters can only move from Borrowed to Owned, never backwards. With N total parameters in an SCC, convergence is guaranteed in at most N + 1 iterations. Each iteration must promote at least one parameter; when none change, the analysis has reached its fixed point. A `debug_assert!` verifies this bound.

## Parameter Promotion Rules

The core analysis (`update_ownership_inner`) scans all instructions and terminators in a function. A parameter is promoted from Borrowed to Owned when any of the following holds:

### Returned from the function

If a parameter (or an alias of it) appears as the value in a `Return` terminator, the caller transfers ownership to its caller -- the parameter must be Owned.

### Passed to an owned parameter at a call site

If a parameter is passed as an argument to another function at a position where the callee's `AnnotatedSig` marks the parameter as Owned, the caller must transfer ownership. Unknown callees (not in the sigs map and not in the borrowing builtins set) conservatively mark all arguments as Owned.

### Stored in a Construct (owned position)

`Construct` instructions build structs, enums, and closures. All arguments are stored in heap-allocated memory and must be Owned -- the constructed value takes ownership.

### Passed to a PartialApply

`PartialApply` captures arguments into a closure environment. Captured values are stored on the heap, requiring Owned semantics.

### Passed to an ApplyIndirect

Indirect calls through closures have unknown callee signatures. All arguments are conservatively promoted to Owned.

### Projected and then used in an owned position

When `Project { dst, value }` extracts a field from a value and the extracted `dst` is itself used in an owned position (returned, stored, etc.), the source `value` must also be Owned. Otherwise the caller might free the struct while the projected field is still live. This propagation is transitive and handled naturally by the fixed-point iteration.

### Passed to a tail call at an owned position

When a function tail-calls another function and passes a currently-Borrowed parameter to an Owned position, the parameter must be promoted. Without this, RC insertion would need to insert an `RcDec` after the tail call, which would break the tail call optimization (the caller's stack frame must not exist after the tail call).

## Alias Resolution

Parameters may be aliased via `Let { dst, value: Var(src) }` instructions. Before checking whether a variable is a parameter, the analysis resolves alias chains: `v2 -> v1 -> v0 (param)`. This ensures that code like `let v1 = param; Construct([v1])` correctly promotes `param` to Owned.

A defensive 64-step limit guards against pathological or cyclic alias maps. In practice, alias chains are 1-3 deep.

## Borrowing vs Consuming Instructions

The borrow analysis distinguishes two categories of instructions based on how they use their arguments:

### Borrowing (read-only)

These instructions read their arguments without taking ownership:

- **PrimOp** (arithmetic, comparison, logical, string ops) -- except list `Add`, which is a COW operation
- **Project** with scalar result -- extracts a field without consuming the parent
- **Apply** with all-borrowed args -- external C runtime functions and borrowing builtins

### Consuming (ownership transfer)

These instructions take ownership of their arguments:

- **Construct** -- stores args in heap-allocated struct/enum
- **PartialApply** -- captures args into closure environment
- **Apply** with owned args -- transfers to callee
- **ApplyIndirect** -- unknown callee, conservative owned

## Derived Ownership (`infer_derived_ownership`)

After parameter-level borrow inference determines which function parameters are Borrowed vs Owned, derived ownership extends this tracking to **all local variables** in a function. This is a single forward pass over SSA blocks -- no fixed-point iteration needed because SSA form guarantees each variable is defined exactly once.

**Source**: `compiler/ori_arc/src/borrow/derived.rs`

### Classification Rules

- **Owned** -- function call results (`Apply`, `ApplyIndirect`), literals, block parameters (which receive values via jump arguments), `Select` results
- **BorrowedFrom(ArcVarId)** -- projection (`Project`) or alias (`Let { value: Var(x) }`) of a borrowed variable. Transitively resolved: if `value` borrows from X, the projection also borrows from X
- **Fresh** -- newly constructed values (`Construct`, `PartialApply`) with refcount = 1. This enables more aggressive reset/reuse pairing because the first `RcDec` is guaranteed to deallocate

### Cross-Block Propagation

The per-block `compute_borrows()` helper tracks borrowed-derived variables within a single block. The global `infer_derived_ownership()` provides cross-block coverage: when a variable derived from a borrowed parameter flows across a block boundary (defined in B0, used in B1), the per-block approach loses track, but the global `DerivedOwnership` vector correctly identifies it as `BorrowedFrom`.

## Output Types

```
enum Ownership {
    Borrowed,  // callee will not retain -- no RC at call site
    Owned,     // callee may retain -- caller must rc_inc
}

enum DerivedOwnership {
    Owned,                    // call result, literal, block param
    BorrowedFrom(ArcVarId),   // projection/alias of borrowed variable
    Fresh,                    // refcount = 1 (Construct, PartialApply)
}

struct AnnotatedParam {
    name: Name,       // interned parameter name
    ty: Idx,          // type pool index
    ownership: Ownership,
}

struct AnnotatedSig {
    params: Vec<AnnotatedParam>,  // per-param ownership
    return_type: Idx,             // return type pool index
}
```

## Built-in Ownership

The borrow analysis must know about built-in methods that the LLVM backend compiles inline (they do not appear as user functions and have no entries in the sigs map).

**Source**: `compiler/ori_arc/src/borrow/builtins/mod.rs`

### `borrowing_builtin_names()`

Methods that always borrow their receiver: `len`, `is_empty`, `contains`, `first`, `last`, `equals`, `compare`, `hash`, `clone`, `to_str`, `split`, `trim`, `starts_with`, `ends_with`, and others (~40 total). These are read-only operations that produce independent results.

### `consuming_receiver_builtin_names()`

COW list methods that consume the receiver: `push`, `pop`, `insert`, `remove`, `reverse`, `sort`, `sort_stable`, `add`, `concat`. The runtime handles the old buffer's RC lifecycle internally -- the ARC pipeline must NOT emit an additional `RcDec` for the receiver.

### `consuming_second_arg_builtin_names()`

COW list methods that also consume their second argument (list2): `add`, `concat`. The runtime takes ownership of list2's buffer.

### `consuming_receiver_only_builtin_names()`

Map/Set COW methods where only the receiver is consumed; other args (comparison keys, read-only collections) are borrowed: `remove`, `difference`, `intersection`, `union`.

### `BuiltinOwnershipSets`

Pre-computed struct grouping all four interned name sets. Constructed once per session to avoid redundant `intern()` work across multiple function compilations.

## Pipeline Integration

```text
infer_borrows_scc()              -- global, per-module
    |
    v
apply_borrows()                  -- writes ownership back to ArcFunction params
    |
    v
infer_derived_ownership()        -- per-function, single forward pass
    |
    v
annotate_arg_ownership()         -- populates per-arg ownership on Apply/Invoke
    |
    v
insert_rc_ops_with_ownership()   -- RC insertion reads all of the above
```

Borrow inference results are cached per session. When a function body has not changed, its cached `AnnotatedSig` is reused without re-running inference.

## References

- Lean 4: `src/Lean/Compiler/IR/Borrow.lean` -- SCC-based borrow inference
- Koka: Perceus paper (Reinking et al. 2021) -- liveness-based RC insertion
- Swift: `lib/SILOptimizer/ARC/` -- bidirectional RC elimination, ownership SSA
