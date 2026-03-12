---
section: "08"
title: "Lean 4 Borrow Inference Sources"
status: complete
goal: "Verify contract extraction and SCC iteration are shaped correctly; identify where Lean is deliberately conservative"
paper:
  title: "Lean 4 Compiler IR — Borrow Inference"
  url: "https://github.com/leanprover/lean4"
  source_files:
    - "src/Lean/Compiler/IR/Borrow.lean"
    - "src/Lean/Compiler/IR/RC.lean"
    - "src/Lean/Compiler/IR/ExpandResetReuse.lean"
depends_on: ["01", "02", "03", "04", "05", "06", "07"]
sections:
  - id: "08.1"
    title: "Source Analysis"
    status: complete
  - id: "08.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "08.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "08.4"
    title: "Plan Edits"
    status: complete
  - id: "08.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "08.6"
    title: "Lens Shift"
    status: complete
  - id: "08.7"
    title: "Open Risk"
    status: complete
---

# Section 08: Lean 4 Borrow Inference Sources

**Status:** Complete
**Goal:** Verify that AIMS contract extraction and SCC iteration are shaped correctly by
studying Lean 4's actual implementation. Identify where Lean is deliberately conservative
and whether AIMS should be too. Document invariants as monotonicity constraints.

**Sources:** Lean 4 compiler repository: [github.com/leanprover/lean4](https://github.com/leanprover/lean4)
- `src/Lean/Compiler/IR/Borrow.lean` — borrow inference (~330 lines)
- `src/Lean/Compiler/IR/RC.lean` — RC insertion (~480 lines)
- `src/Lean/Compiler/IR/ExpandResetReuse.lean` — reset/reuse expansion (~290 lines)

---

## 08.1 Source Analysis

### Borrow.lean: Borrow Inference

Lean 4's borrow inference is a **forward monotonic set-expansion** algorithm over mutually
recursive function blocks. The core data structures:

**`OwnedSet`**: A `HashMap<(FunId, Index), ()>` keyed by `(function, variable_index)`. This
is the single monotonic state: once a variable is added to `OwnedSet`, it is never removed.
The set grows from empty toward all variables. This is the key monotonicity invariant.

**`ParamMap`**: A `HashMap<Key, Array Param>` mapping function IDs and join-point IDs to
parameter arrays. Each `Param` carries a `borrow: Bool` field. Parameters start with
`borrow := true` (most optimistic, corresponding to AIMS's `all_borrowed` initialization),
and are promoted to `borrow := false` (owned) when the variable appears in `OwnedSet`.

**Initialization** (`initBorrow`, lines 59-60): Every parameter whose type `isPossibleRef`
starts with `borrow := true`. Scalar parameters (non-ref types) keep their default
`borrow := false`. Exported functions skip borrow initialization entirely — all params
stay owned. This is a conservative choice for C++ interop.

**The `collectExpr` function** (lines 234-260) is the core transfer function. For each
variable declaration `let z := expr`, it determines what variables must be owned:

| Expression | Owned variables | Notes |
|-----------|----------------|-------|
| `reset _ x` | `z`, `x` | Both destination and source must be owned |
| `reuse x _ _ ys` | `z`, `x`, plus `ys` if params | Destination and token owned; args owned if they are function parameters (heuristic) |
| `ctor _ xs` | `z`, plus `xs` if params | Constructor always owns destination; args owned if params (packing heuristic) |
| `proj _ x` | `z` if `x` owned; `x` if `z` owned | **Bidirectional**: ownership propagates through projections in both directions |
| `fap g xs` | `z`, plus args matching owned callee params | Destination always owned; args follow callee's param ownership |
| `ap x ys` | `z`, `x`, all `ys` | Indirect call: everything owned (fully conservative) |
| `pap _ xs` | `z`, all `xs` | Partial application: everything owned |

**Critical observation — bidirectional projection ownership** (line 246-248): Lean does
something AIMS does not — if a projection destination `z` is owned (demanded by later code),
the source `x` is retroactively promoted to owned. Conversely, if `x` is owned, `z` inherits
ownership. This is a fixed-point interaction that can propagate through multiple iterations.

**`ownArgsIfParam` heuristic** (lines 218-232): When constructor args are also function
parameters, Lean forces them owned. The comment explicitly says this is a **heuristic** and
not related to reset/reuse effectiveness. It handles patterns like:
```
def f (x y : obj) :=
  let z := ctor_1 x y
  ret z
```
Without this, `x` and `y` would be borrowed, requiring inc/dec around the constructor.

**`preserveTailCall`** (lines 262-270): After collecting a declaration `let x := fap g ys`
followed by `ret x`, if this is a self-recursive tail call, Lean runs `ownParamsUsingArgs` —
promoting function parameters to owned if the corresponding argument at the tail-call site is
owned. This prevents breaking tail calls with post-call decrements.

**Important limitation**: Lean only preserves tail calls to the **same function** (self-calls,
line 267: `ctx.currFn == g`). Mutual tail calls between SCC members are not handled.

**`collectFnBody`** (lines 275-292): Traverses the function body. Join points are analyzed
with updated param sets (`withReader (fun ctx => updateParamSet ctx ys)`). At each vdecl,
it processes the *continuation first* (`collectFnBody b`), then the expression
(`collectExpr x v`), then the tail call check (`preserveTailCall x v b`). This bottom-up
order means ownership demanded by later code propagates backward through the body.

**Join points** (`.jmp j ys`, lines 286-290): Two symmetric operations:
1. `ownArgsUsingParams ys ps` — if join point param `ps[i]` is owned, mark arg `ys[i]` owned
2. `ownParamsUsingArgs ys ps` — if arg `ys[i]` is owned, mark param `ps[i]` owned

This bidirectional propagation ensures join-point calls can be compiled as direct jumps
(analogous to AIMS's tail-call preservation rule, but applied to all join points, not just
tail calls).

**Fixed-point iteration** (`whileModifying`, lines 301-309): The outermost loop. Each
iteration processes all declarations in the SCC block. If any variable was added to
`OwnedSet` (tracked by the `modified` flag), the iteration repeats. Convergence is
guaranteed because `OwnedSet` only grows and is bounded by the total number of variables
across all SCC functions.

**`infer`** (line 316-317): Runs the fixed point on a block of mutually recursive declarations.
The `ParamMap` is initialized once, then refined by the fixed-point loop.

### RC.lean: RC Insertion

RC insertion is a **single-pass backward traversal** that uses borrow annotations from
`Borrow.lean` and live-variable analysis computed inline. Key structures:

**`DerivedValInfo`** (lines 21-27): Tracks parent-child relationships between variables.
A variable `z` is a "derived value" of `x` if `z := proj _ x`. The `DerivedValMap` records
these relationships. When a parent is live, its borrowed descendants are also considered live
(through `addDescendants`). When a reset breaks the parent-child link, `removeFromParent`
is called. This is Lean's mechanism for **RC identity through projections** — projections
from a borrowed parent do not need separate inc/dec.

**`LiveVars`** (lines 108-111): Two sets: `vars` (live variables) and `borrows` (variables
that are merely borrowed — derived from borrowed params). The `borrows` set is critical:
if a variable is borrowed (in `borrows`) rather than owned, incrementing it is unnecessary.

**`mkRetLiveVars`** (lines 148-151): At return points, all borrowed parameters and their
descendants are placed in the `borrows` set. This means returning a borrowed parameter
doesn't require an inc — the caller still holds the reference.

**`addIncBeforeAux`** (lines 256-272): The core inc-insertion logic for function calls.
For each argument `x` at a consuming position:
- Count how many times `x` appears at consuming positions (`numConsumptions`)
- If `x` is live after the call OR borrowed at another position, `numIncs = numConsumptions`
- Otherwise `numIncs = numConsumptions - 1` (one consumption is "free" — the last use)

This is the **"last use is free"** optimization. AIMS achieves the same effect through
`Cardinality::Once` + `Consumption::Linear` detecting that a single-use variable's inc
can be elided (`is_rc_inc_elidable` in `transfer/mod.rs`).

**`addDecForAlt`** (lines 216-225): At case/match branches, inserts decs for variables live
at the case entry but dead in a specific alternative. Also inserts incs when a variable is
borrowed at the case level but not in the alternative.

**`processVDecl`** (lines 351-383): The main per-instruction handler. Notable special cases:
- Projection (`.proj`): if `z` is not borrowed, add `inc z`; then add `dec x` if `x` dies.
  If `z` IS borrowed, skip the inc entirely (RC identity through projection).
- Function call (`.fap`): uses `addIncBefore` (parameter-aware) and `addDecAfterFullApp`
  (adds decs for consumed-but-still-alive args).
- `Array.getInternal`: if the result is borrowed, Lean rewrites to a `Borrowed` variant
  of the array access function (lines 369-373). This is a codegen-level optimization specific
  to Lean's array representation.

### ExpandResetReuse.lean: Reset/Reuse Expansion

This pass expands the abstract `reset`/`reuse` instructions into concrete conditional code.

**`consumed` predicate** (lines 47-54): Checks whether a variable `x` is consumed in all
branches. A reset token is only expanded if it is consumed — otherwise it is dead code.

**`eraseProjIncFor`** (lines 59-93): Before expanding a reset, this function scans backward
from the reset instruction looking for `proj[i] y` + `inc z` pairs where `y` is the reset
source. These pairs represent projections that took fields before the reset. The function
removes the corresponding `inc` instructions and records which fields were "reclaimed" in a
`Mask` array.

**`mkSlowPath`** (lines 130-135): When reuse fails at runtime (object is shared), the slow
path: inc the reclaimed fields, dec the source, replace `reuse` with `ctor` (fresh alloc).

**`mkFastPath`** (lines 236-239): When reuse succeeds (object is unique), the fast path:
`del y` (no-op free), replace `reuse x ctor_i ws` with `set x i ws[i]` (in-place mutation),
and release any fields not reclaimed by the code.

**`expand`** (lines 242-253): The top-level expansion:
```
x := reset[n] y; b
```
becomes:
```
let c := isShared y;
if c then mkSlowPath(...) else mkFastPath(...)
```

**`removeSelfSet`** (lines 173-192): After expansion, removes redundant `set` instructions
where `set x[i] := proj[i] x` — i.e., writing a field that already has the same value.

**`reuseToSet`** (lines 194-215): In the fast path, replaces `reuse x ctor ys` with a
sequence of `set x[i] := ys[i]` instructions and `setTag x ctor.cidx` if the constructor
tag changed.

### Three-Pass Architecture Summary

Lean's ARC optimization is three clean sequential passes, each with a clear input/output contract:

1. **Borrow.lean**: Monotonic set-expansion fixed point. Input: IR. Output: `ParamMap`
   (borrow annotations on all function and join-point parameters). Determines WHO owns WHAT.
2. **RC.lean**: Single backward pass using borrow annotations + inline liveness. Input: IR +
   `ParamMap`. Output: IR with `inc`/`dec` instructions inserted. Determines WHERE to inc/dec.
3. **ExpandResetReuse.lean**: Per-function forward pass. Input: IR with RC ops. Output: IR with
   `reset`/`reuse` expanded into `isShared`/`set`/`del` sequences. Determines HOW to reuse.

---

## 08.2 What AIMS Should Adopt

### Keep

**K1. Monotone-only set growth for ownership.**
Lean's `OwnedSet` only grows. Once a variable is marked owned, it stays owned forever. The
`markModified` flag (line 147) triggers re-iteration only when the set actually grows. AIMS
mirrors this correctly: `ParamContract.access` can only promote from `Borrowed` to `Owned`,
never demote. The `join` function in `MemoryContract` (`contract/mod.rs` line 87) is
componentwise max, ensuring monotonic convergence. **AIMS is correct here.**

**K2. Initialize parameters to borrow, promote to owned.**
Lean initializes all `isPossibleRef` parameters to `borrow := true` (line 60). AIMS's
`MemoryContract::all_borrowed` (line 57 of `contract/mod.rs`) does the same with
`ParamContract::OPTIMISTIC`. Both start optimistic and promote toward conservative. The
direction is identical and correct.

**K3. Tail call preservation as post-inference fixup.**
Lean's `preserveTailCall` (line 262) runs after `collectExpr` but within the same iteration.
AIMS's plan describes this as a "post-inference fixup step" (section-03-interprocedural.md
lines 345-369). Both apply the same rule: if a tail-call argument is owned but the callee's
parameter is borrowed, promote the parameter to owned. Both are monotonic (Borrowed -> Owned).

**K4. Join-point bidirectional ownership propagation.**
Lean's `jmp j ys` handler (lines 286-290) runs both `ownArgsUsingParams` AND
`ownParamsUsingArgs`. This bidirectional flow is essential for ensuring jump-to-join-point
operations don't break by requiring inc/dec around them. AIMS models join points as block
parameters. The backward demand analysis (`backward_terminator_demands` in `transfer/mod.rs`
line 331: `Jump { args } => args once`) correctly propagates demand from join points to
arguments. However, the reverse direction — promoting block parameters when arguments are
owned — happens implicitly through the state-map merge at block entries. **Verify this
bidirectionality is preserved in the block-level analysis.**

**K5. Indirect calls and partial applications are fully conservative.**
Lean marks `ap x ys` with everything owned (line 253-256) and `pap _ xs` with everything
owned (line 257-259). AIMS does the same: `ApplyIndirect` gets `AimsState::TOP` in
`transfer_def` (line 80), and `PartialApply` captured args get `(Owned, Unrestricted, Many)`
via `capture_state_update` (line 419-430). This conservatism is correct — unknown callees
offer no contract.

**K6. Scalar exclusion from borrow inference.**
Lean's `initBorrow` (line 60) only marks `isPossibleRef` parameters as borrowable. AIMS's
`extract_contract` (line 212-217 of `interprocedural.rs`) returns `ParamContract::CONSERVATIVE`
for scalar parameters and the `AimsState::SCALAR` sentinel (line 169 of `lattice/mod.rs`)
short-circuits the entire analysis. Both correctly exclude scalars.

**K7. Exported functions stay conservative.**
Lean's `initBorrowIfNotExported` (lines 68-70) keeps exported functions fully owned for C++
interop. AIMS should adopt this for FFI functions. Currently, FFI functions get conservative
contracts via the fallback path (line 84-90 of `interprocedural.rs`), which is correct but
not explicitly documented as matching Lean's rationale.

### New Invariants

**I1. Monotonicity invariant for `OwnedSet` analog.**
Document explicitly in `interprocedural.rs`: "The `all_sigs` map is append-only with respect
to contract conservatism. Once a function's contract is finalized (after its SCC is processed),
it is never weakened. SCC-local contracts (`local_sigs`) may only grow toward conservative
via `join`."

The existing `analyze_scc_fixpoint` code (line 152-158 of `interprocedural.rs`) correctly
implements this via `old_contract.join(&new_contract)`, but the invariant should be stated
as a formal comment.

**I2. Projection bidirectionality invariant.**
Lean's `proj` handling (line 246-248) propagates ownership bidirectionally through projections.
AIMS's `transfer_project` (`transfer/mod.rs` line 134-151) only propagates in the forward
direction (source uniqueness flows to destination). The backward direction — if a projection
result is owned/consumed, promote the source to owned — is handled by the backward demand
analysis (`backward_demands` for `Project` returns `(value, Once)`, line 265-267). This
should be documented as an invariant: "Backward demand from a consumed projection promotes
the source variable toward Owned. This is the AIMS analog of Lean's bidirectional proj rule."

**I3. Modified-flag convergence invariant.**
Lean uses an explicit `modified: Bool` flag (line 136) reset at each iteration start (line 303).
AIMS uses `old_contract != new_contract` for the same purpose (line 151 of `interprocedural.rs`).
Document: "Convergence detection is by structural equality of contracts across iterations.
This is equivalent to Lean's `modified` flag but more robust — it detects any dimension change,
not just ownership promotion."

**I4. Constructor-args-if-param heuristic should be documented as optional.**
Lean's `ownArgsIfParam` (lines 218-232) is explicitly called a "heuristic" that is "not related
with the effectiveness of the reset/reuse optimization." AIMS does not implement this heuristic.
Document in `interprocedural.rs`: "AIMS does not implement Lean's `ownArgsIfParam` heuristic
(packing function params into constructors forces ownership). The backward demand analysis
naturally promotes constructor args to Owned when the constructor result is consumed, achieving
the same effect without a special case."

---

## 08.3 What AIMS Should Not Adopt

### Reject

**R1. Single-dimension `OwnedSet` representation.**
Lean tracks only one bit per variable: owned or borrowed. AIMS's `ParamContract` carries six
dimensions (access, consumption, cardinality, may_escape, may_share, locality_bound), and the
per-variable `AimsState` has seven dimensions. The single-bit approach is appropriate for Lean's
pure functional semantics (no mutation, no COW, no partial consumption) but would lose critical
information for Ori's imperative features (COW mutations, closure captures, collection reuse).
**Reject: AIMS correctly uses a richer lattice.**

**R2. Forward-only borrow inference with backward heuristics.**
Lean's `collectFnBody` traverses forward through the body (lines 275-292), then applies backward
rules as special cases (`preserveTailCall`, `ownArgsIfParam`). AIMS uses a proper backward
dataflow analysis with worklist iteration. The backward approach is more principled for demand
analysis — it naturally computes "what does the future code need?" rather than Lean's approach
of "mark things owned when we see them used in ownership-demanding positions." AIMS's approach
handles complex control flow (loops, nested branches) more uniformly. **Reject: AIMS's backward
analysis is the better design for a non-pure language.**

**R3. `DerivedValInfo` parent-child tracking for RC identity.**
Lean's `DerivedValMap` (RC.lean lines 21-97) tracks projection chains to identify variables
whose RC operations can be elided because they share identity with a parent. This is a
per-function side table maintained during RC insertion. AIMS achieves the equivalent through
`BorrowSource` (lattice/mod.rs lines 342-410): projections get `BorrowSource::exact_field`
tracking, and the emission pass uses this to identify RC-identity relationships. AIMS's
approach is cleaner because provenance is part of the analysis state rather than a separate
tracking structure. **Reject the separate tracking structure; keep AIMS's integrated approach.**

**R4. Self-call-only tail call preservation.**
Lean's `preserveTailCall` (line 267) only handles tail calls where `ctx.currFn == g` — calls
to the same function. AIMS's plan describes tail-call preservation for any syntactic tail call
(section-03-interprocedural.md lines 351-368), including calls to other SCC members or non-SCC
callees. **AIMS is more general here and should stay so.** However, note that Lean's restriction
is intentional — it only performs TCO for self-recursion. If AIMS also only performs TCO for
self-recursion, the broader tail-call ownership preservation may be unnecessary in v1.

**R5. Array accessor specialization.**
Lean rewrites `Array.getInternal` to `Array.getInternalBorrowed` when the result is borrowed
(RC.lean lines 369-373). This is specific to Lean's array representation and runtime. Ori's
runtime handles this through COW — borrowed projections from lists don't need special accessors.
**Reject: not applicable to Ori's COW model.**

**R6. Lean's treatment of exported functions.**
Lean never infers borrow annotations for exported (`@[export]`) functions because C++ wrappers
need to know ownership statically. Ori's FFI model is different — Ori controls both sides of
the boundary (compiler + `ori_rt`). AIMS should infer borrow for all functions, including those
called from LLVM-generated code, since the codegen is aware of the annotations.

---

## 08.4 Plan Edits

**P1. `section-03-interprocedural.md` Section 03.2: Add monotonicity documentation requirement.**

Add to the SCC fixed-point checklist (after line 285): "Document the monotonicity invariant:
`all_sigs` entries for completed SCCs are never weakened. `local_sigs` within an SCC iteration
only grow via `join`. This mirrors Lean's `OwnedSet` grow-only property and guarantees
convergence."

**P2. `section-03-interprocedural.md` Section 03.3: Clarify projection ownership propagation.**

Add a note to the "A parameter must be `access == Owned`" rules (around line 298): "Projection
uses propagate ownership bidirectionally (Lean Borrow.lean line 246-248). In AIMS, the forward
direction is handled by `transfer_project` (inherited uniqueness), and the backward direction
by backward demand (`backward_demands` for `Project` returns `(source, Once)`). When the
projection result is later consumed at an owned position, the backward demand naturally
promotes the source."

**P3. `section-03-interprocedural.md` Section 03.3: Document `ownArgsIfParam` non-adoption.**

Add a note after the constructor ownership rules (around line 301): "Note: Lean's
`ownArgsIfParam` heuristic (force constructor args to owned when they are function parameters)
is deliberately not implemented. AIMS's backward demand analysis achieves the same effect:
when a constructor result is consumed (returned, stored), backward demand propagates to the
args, promoting parameters to Owned as needed. No special case required."

**P4. `section-03-interprocedural.md` Section 03.3: Clarify tail-call scope.**

Strengthen the tail-call preservation note (around line 363): "Unlike Lean (which preserves
tail calls only for self-recursion, Borrow.lean line 267), AIMS preserves tail calls for any
direct callee in syntactic tail position. This is sound because AIMS's contract system makes
the ownership requirement explicit at call sites regardless of callee identity."

---

## 08.5 Code Changes (Later)

**C1. `interprocedural.rs`: Add formal monotonicity comment.**

At the top of `analyze_scc_fixpoint` (around line 117), add a doc comment block:
```
/// # Monotonicity Invariants (verified against Lean 4 Borrow.lean)
///
/// 1. `external_sigs` entries are finalized — never weakened after their SCC completes.
/// 2. `local_sigs` entries only grow via `join` (componentwise max toward conservative).
/// 3. The `modified` state is captured by `old_contract != joined` — equivalent to
///    Lean's explicit `modified: Bool` flag (Borrow.lean line 136).
/// 4. Convergence bound: each iteration must promote at least one lattice dimension.
///    Total dimensions per function = params x 6 + return x 4 + effects x 4 = O(params).
```

File: `compiler/ori_arc/src/aims/interprocedural.rs`

**C2. `interprocedural.rs`: Verify `all_sigs` is never weakened post-SCC.**

Add a `debug_assert!` after `all_sigs.extend(scc_sigs)` (around line 83) verifying that
no previously-finalized contract was weakened:
```rust
#[cfg(debug_assertions)]
for (name, new_contract) in &scc_sigs {
    if let Some(old) = external_sigs.get(name) {
        debug_assert!(
            old == &old.join(new_contract),
            "AIMS: SCC result weakened a finalized contract for {name:?}"
        );
    }
}
```
This catches a class of bugs where SCC processing accidentally overwrites a callee's
already-converged contract with a weaker one.

**C3. `transfer/mod.rs`: Document projection bidirectionality.**

Add a comment to `backward_demands` for `Project` (around line 265):
```rust
// Project: one read of the source.
// This is the backward half of Lean's bidirectional projection ownership
// (Borrow.lean line 246-248). When a projection result is consumed at an
// owned position, this demand propagates backward to the source, eventually
// promoting it from Borrowed to Owned.
```

**C4. `interprocedural.rs` `extract_contract`: Document scalar handling rationale.**

Add a comment at line 212:
```rust
// Scalar parameters don't participate in RC — matching Lean's
// initBorrow (Borrow.lean line 60) which only marks isPossibleRef
// parameters as borrowable. Conservative access avoids confusion
// in downstream passes that might misinterpret Borrowed+Scalar.
```

**C5. Consider adding join-point ownership bidirectional assertion.**

In `intraprocedural/block.rs` or wherever block-parameter states are merged, add a
`debug_assert!` verifying that when an argument to a `Jump` terminator is `Owned`, the
corresponding block parameter is not `Borrowed`. This is the AIMS analog of Lean's symmetric
`ownArgsUsingParams`/`ownParamsUsingArgs` at join points (Borrow.lean lines 286-290).

---

## 08.6 Lens Shift

**For Paper 09 (GHC Demand Analysis):**

Lean's source analysis reshapes how to read GHC's demand analysis in three ways:

1. **Cardinality is doing the heavy lifting, not access class.** Lean's borrow inference is
   fundamentally about one bit (owned vs borrowed). All the nuance in Lean's system comes from
   WHERE ownership is demanded (constructors, calls, returns, closures). GHC's demand analysis
   adds cardinality (usage counts) as a first-class dimension. When reading GHC, focus on how
   `seq_add` and `alt_join` interact with the fixed point — AIMS already has these operations
   (`Cardinality::seq_add` and `Cardinality::alt_join` in `dimensions.rs`), but GHC may reveal
   edge cases around loop bodies and recursive calls.

2. **Lean's `modified` flag is a cheap convergence check that GHC avoids.** GHC uses structural
   comparison of demand signatures across iterations (like AIMS's `old_contract != new_contract`).
   Lean uses a boolean flag. The GHC approach is more robust but more expensive. AIMS already
   uses the GHC-style approach. When reading GHC, focus on whether they have optimizations for
   the comparison (e.g., hash-based fast path, or dimension-count-based early termination).

3. **The "packing" heuristic is a demand-analysis gap.** Lean's `ownArgsIfParam` forces
   ownership when function params are packed into constructors. GHC's demand analysis should
   handle this naturally through usage analysis — if a parameter is used once (packed into a
   ctor), its demand is `Once`. If the ctor result escapes, the param's demand escalates.
   When reading GHC, look for how "transitive demand" through constructor packing works and
   whether AIMS's backward analysis already captures it correctly.

4. **DerivedValInfo is a demand-analysis side effect.** Lean's RC pass tracks projection chains
   separately from borrow inference. GHC folds this into strictness/absence analysis. When
   reading GHC, look for how projection chains interact with demand transformers — this may
   suggest improvements to AIMS's `BorrowSource` tracking.

---

## 08.7 Open Risk

**Risk 1: Bidirectional projection ownership may not fully converge in one backward pass.**

Lean's `collectExpr` for `proj` (line 246-248) explicitly propagates ownership in both
directions through projections. AIMS relies on the backward demand analysis to propagate
demand from consumed projections to their sources. In a single backward pass, this works:
consumed projection result -> demand on source. But consider:

```
let a = param.field1     // Project
let b = f(a)             // Apply consuming a
let c = param.field2     // Project
return c
```

Here, `a` is consumed by `f`, which demands `param` be Owned (via backward demand from
`Project`). But `c` is a projection of `param` too. If `param` becomes Owned, does `c`
inherit ownership correctly? In AIMS, `transfer_project` gives `c` `AccessClass::Borrowed`
(line 140 of `transfer/mod.rs`). The question is whether `c` being consumed (by Return)
causes its own backward demand to promote `param` independently.

**Mitigation**: The backward analysis should handle this naturally — `Return { value: c }`
creates demand on `c`, which creates demand on `param` via `Project`'s `backward_demands`.
But verify this with a test case in `interprocedural/tests.rs`.

**Risk 2: SCC iteration count may exceed the theoretical bound for pathological cases.**

AIMS's convergence bound (line 171-183 of `interprocedural.rs`) is `sum(params * 6 + 4 + 4)`.
Lean has no explicit bound — it relies on the finite variable set and monotonicity. AIMS's
bound is tighter (dimension-based) but may be violated if a bug causes non-monotonic updates.
The existing `debug_assert!` (line 180) catches this in debug builds, but consider logging a
`tracing::warn!` instead of asserting in release builds to avoid crashing the compiler on
edge cases.

**Risk 3: Join-point ownership preservation may be incomplete.**

Lean explicitly handles join points (`jmp j ys`, lines 286-290) with bidirectional ownership
propagation. AIMS models join points as regular blocks with parameters. The question is whether
the backward analysis at `Jump { target, args }` terminators correctly propagates ownership
requirements from block parameters to arguments. If block `B` has a parameter `p` that is
`Owned` (because code in `B` consumes it), the backward demand at the `Jump` to `B` must
promote the corresponding argument to `Owned`.

**Mitigation**: Add a test in `interprocedural/tests.rs` with a function containing a join
point (block with multiple predecessors) where one predecessor provides an owned value and
the other provides a borrowed one. Verify the backward analysis promotes both to `Owned`.

**Risk 4: `preserves_freshness` may be more fragile than Lean's approach.**

Lean doesn't have a `preserves_freshness` concept — it infers ownership at the parameter
level and relies on the RC pass to insert the right inc/dec. AIMS's `preserves_freshness`
(in `extract_return_info`, line 249 of `interprocedural.rs`) traces return values to their
definitions and checks whether all paths produce fresh values or pass through parameters.
This recursive tracing (via `var_uniqueness`) is not part of the fixed point — it runs once
after intraprocedural analysis. If the definition map is incomplete (e.g., Invoke-defined
variables not in `def_map`), the result defaults to `MaybeShared, false`, which is safe
but may miss optimization opportunities. The `invoke_defs` map (line 259) handles this
case, but verify coverage for all definition sites.
