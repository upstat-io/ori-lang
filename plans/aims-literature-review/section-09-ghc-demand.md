---
section: "09"
title: "GHC Demand Analysis"
status: complete
goal: "Verify seq_add and branch joins are documented strongly enough and that control flow composition is precise"
paper:
  title: "GHC Demand Analysis (DmdAnal.hs and related)"
  source_files:
    - "compiler/GHC/Core/Opt/DmdAnal.hs"
    - "compiler/GHC/Types/Demand.hs"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08"]
sections:
  - id: "09.1"
    title: "Source Analysis"
    status: complete
  - id: "09.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "09.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "09.4"
    title: "Plan Edits"
    status: complete
  - id: "09.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "09.6"
    title: "Lens Shift"
    status: complete
  - id: "09.7"
    title: "Open Risk"
    status: complete
---

# Section 09: GHC Demand Analysis

**Status:** Complete
**Goal:** Verify that `seq_add` and branch joins are documented strongly enough, that
alternative control flow is distinguished from sequential composition everywhere, and
that loops and exceptional edges have adequate documentation and tests.

**Sources:** GHC compiler:
- `compiler/GHC/Core/Opt/DmdAnal.hs` -- demand analysis pass
- `compiler/GHC/Types/Demand.hs` -- demand types and operations

**Why read this ninth:** Mostly about methodology for backward reasoning, joins, and what
"once" means across control flow. GHC's demand analysis is the most mature implementation
of backward cardinality inference.

**Pause questions:**
- Are `seq_add` and branch joins documented strongly enough?
- Are you distinguishing alternative control flow from sequential composition everywhere?
- Do loops and exceptional edges need stronger documentation or tests?

**AIMS context:**
- `Cardinality::seq_add()` composes sequential uses (Once + Once = Many)
- `AimsState::alt_join()` joins at control flow merge points
- Backward dataflow in `intraprocedural/block.rs` processes instructions in reverse
- `InvokeEdgeState` handles exceptional edges (normal vs unwind)
- Loop handling: worklist iterates to fixpoint over back edges

---

## 09.1 Source Analysis

### GHC's Demand Representation

GHC represents demands as a product of **cardinality** (how many times a value
is evaluated) and **sub-demand** (how deeply a value is used once evaluated).
This is implemented in `compiler/GHC/Types/Demand.hs`.

**Card (Cardinality)** is a set-based interval `[lower..upper]` encoded as a
3-bit vector:

| Pattern | Meaning | Strictness | Usage |
|---------|---------|------------|-------|
| `C_10`  | `{}` bottom, diverges | strict | absent |
| `C_00`  | `{0}` absent | lazy | absent |
| `C_11`  | `{1}` strict+once | strict | once |
| `C_01`  | `{0,1}` lazy+once | lazy | once |
| `C_1N`  | `{1,n}` strict+many | strict | many |
| `C_0N`  | `{0,1,n}` top | lazy | many |

The bit-vector encoding (`0b[n_bit][1_bit][0_bit]`) enables all lattice
operations to be implemented as bitwise operations:

- **lub (alternative join):** `lubCard a b = a .|. b` (bitwise OR)
- **glb (meet):** `glbCard a b = a .&. b` (bitwise AND)

GHC distinguishes three binary operations on cardinalities:

1. **`lubCard`** -- alternative composition (case branches, if/else). Bitwise
   OR. `lubCard C_11 C_11 = C_11`. Key property: `lub(Once, Once) = Once`
   because only one branch executes per dynamic run.

2. **`plusCard`** -- sequential composition (let body + RHS, application + argument).
   Counts combine: `plusCard C_11 C_11 = C_1N` because both sides execute.
   Algebraic specification from Note [Algebraic specification for plusCard and
   multCard]:
   - `0 in (a + b)` iff `0 in a AND 0 in b`
   - `1 in (a + b)` iff `1 in a OR 1 in b`
   - `n in (a + b)` iff `n in a OR n in b OR (1 in a AND 1 in b)` (carry)

3. **`multCard`** -- scaling (lambda under call demand, nested evaluation context).
   Models "if the outer context evaluates N times, and the inner expression
   evaluates M times per outer evaluation, the combined is N*M":
   - `0 in (a * b)` iff `0 in a OR 0 in b`
   - `1 in (a * b)` iff `1 in a AND 1 in b`
   - `n in (a * b)` iff `(1 in result) AND (n in a OR n in b)`

**SubDemand** describes what happens *conditional on evaluation occurring*:

```haskell
data SubDemand
  = Poly !Boxity !CardNonOnce   -- uniform demand at all depths
  | Call !CardNonAbs !SubDemand -- function called N times, result has sd
  | Prod !Boxity ![Demand]      -- product with per-field demands
```

The key design decision (Note [SubDemand denotes at least one evaluation]) is
that sub-demands describe usage depth *conditional* on at least one evaluation.
This allows `L :* P(S,L)` to mean "if evaluated, the first field is always
strict" even though the outer demand is lazy.

**Demand** itself is:
```haskell
data Demand = BotDmd | AbsDmd | D !CardNonAbs !SubDemand
```

### How GHC Does Backward Analysis

`DmdAnal.hs` performs backward demand analysis. The main function:

```haskell
dmdAnal :: AnalEnv -> SubDemand -> CoreExpr -> WithDmdType CoreExpr
```

takes an environment and an incoming sub-demand (from the use context), walks
the expression tree, and returns a `DmdType` mapping free variables to their
demands. The analysis is compositional -- each expression form has a specific
composition strategy:

**Application (sequential):** `f x` analyzes `f` under a `CallDmd` wrapper,
extracts the argument demand from the result, analyzes `x` under that demand,
then combines with `plusDmdType` (sequential composition -- both `f` and `x`
are evaluated):

```haskell
dmdAnal' env dmd (App fun arg) =
  let call_dmd = mkCalledOnceDmd dmd
      WithDmdType fun_ty fun' = dmdAnal env call_dmd fun
      (arg_dmd, res_ty) = splitDmdTy fun_ty
      (arg_ty, arg') = dmdAnalStar env arg_dmd arg
  in  WithDmdType (res_ty `plusDmdType` arg_ty) (App fun' arg')
```

**Case alternatives (alternative):** Multiple case branches are joined with
`lubDmdType` because only one executes per dynamic run:

```haskell
dmdAnalSumAlts env dmd case_bndr (alt:alts) =
  let WithDmdType cur_ty  alt'  = dmdAnalSumAlt env dmd case_bndr alt
      WithDmdType rest_ty alts' = dmdAnalSumAlts env dmd case_bndr alts
  in  WithDmdType (lubDmdType cur_ty rest_ty) (alt':alts')
```

**Let bindings (sequential):** The body's demands and the RHS's demands are
combined with `plusDmdType`. Two strategies exist:
- **LetUp**: analyze body first, extract demand on the let-bound variable, then
  analyze RHS under that demand (more precise for non-recursive bindings).
- **LetDown**: analyze RHS first to compute a demand signature, add to
  environment, then analyze body (required for recursive bindings).

**Lambda (scaling):** Peels one `Call` demand layer, analyzes body, then
scales the result with `multDmdType n` where `n` is the call cardinality.
A lambda called zero times has no demands on its free variables; called many
times multiplies demands.

### How GHC Handles Loops and Back-Edges

Recursive `let` bindings use `dmdFix`, which performs fixed-point iteration:

1. **Initialization:** All recursive bindings start with `botSig` (bottom
   demand signature -- strict in all arguments, diverging). This is the most
   pessimistic starting point, ensuring the iteration finds a *least* fixed
   point.

2. **Iteration:** Each round re-analyzes all RHS bodies with the current
   signatures, producing new signatures. The `reuseEnv` function stabilizes
   free-variable demands for recursive functions to prevent infinite iteration
   on cardinality alone.

3. **Convergence check:** Iteration stops when signatures stabilize (or after
   a maximum of 10 iterations -- a safety net).

4. **Weak demands:** `splitWeakDmds` separates lazy, used-at-most-once demands
   from the signature. These "weak" demands are less useful for inter-procedural
   optimization in recursive contexts and are excluded from the fixed-point
   comparison to improve convergence speed.

### What GHC Tests About Its Algebra

GHC's approach to algebraic correctness is primarily through:

1. **Specification in comments:** Note [Algebraic specification for plusCard
   and multCard] provides the definitive set-theoretic specification.

2. **Semantic clarity through encoding:** The bitwise representation makes lub
   and glb obviously correct (OR and AND on sets). plusCard and multCard have
   more complex bit manipulations but are derived from the algebraic spec.

3. **Test suite coverage:** GHC's test suite includes hundreds of demand
   analysis tests that verify end-to-end behavior (strictness analysis results,
   worker-wrapper transformations) rather than testing algebraic properties of
   the operations in isolation.

4. **Invariant documentation:** Key invariants are documented as Notes
   (e.g., Note [SubDemand denotes at least one evaluation], Note [Boxity for
   bottoming functions]) with references from the code.

---

## 09.2 What AIMS Should Adopt

### Keep

**1. The Three-Operation Discipline**

GHC distinguishes three conceptually different operations on cardinalities:
- `lub` (alternative): only one branch executes
- `plus` (sequential): both sides execute
- `mult` (scaling): nested evaluation context

AIMS currently has two: `alt_join` (= GHC's `lub`) and `seq_add` (= GHC's
`plus`). This is correct for the strict evaluation model -- AIMS does not
need `mult` because:

- In a lazy language, `\x -> body` may evaluate `body` zero or many times
  depending on the call context, requiring demand scaling.
- In a strict language, every function body executes exactly once per call.
  There is no laziness-induced multiplication.

**However, AIMS should explicitly document this decision.** The `Cardinality`
documentation in `lattice/dimensions.rs` should state that `mult` is not
needed because Ori evaluates strictly, and that `seq_add` already handles the
sequential case that GHC splits between `plus` and `mult`.

**Keep also:** AIMS's existing separation of `seq_add` from `alt_join` is
exactly the right discipline. The test
`branch_value_used_in_both_arms_is_once` in `intraprocedural/tests.rs`
correctly verifies that `alt_join(Once, Once) = Once`, and
`sequential_uses_in_same_block_are_many` verifies `seq_add(Once, Once) = Many`.
This is the core invariant.

**2. Algebraic Specification as Comments**

GHC documents its operations with set-theoretic specifications:
```
0 in (a + b) iff 0 in a AND 0 in b
1 in (a + b) iff 1 in a OR 1 in b
n in (a + b) iff n in a OR n in b OR (1 in a AND 1 in b)
```

AIMS should adopt this documentation style for `seq_add`. The current doc
comment says:

```rust
/// `Absent + x = x`, `Once + Once = Many`, `Many + _ = Many`.
```

This is a truth table, not a specification. A specification would state:
- `seq_add` is the monoid operation for sequential composition
- Identity: `Absent`
- Absorbing element: `Many`
- `seq_add(a, b)` counts the total number of uses when `a` uses happen
  before `b` uses

**3. Bottom-Starting Fixed-Point for Recursion**

GHC starts recursive bindings at `botSig` (bottom). AIMS starts all variables
at `BOTTOM` (most optimistic). Both approaches find the least fixed point
because:

- GHC's `botSig` = strict in all arguments (pessimistic for laziness analysis)
- AIMS's `BOTTOM` = `(Borrowed, Dead, Absent, Unique, BlockLocal, NonReusable, NoEffect)`

In a backward analysis, BOTTOM means "no demand from successors yet" --
this is the correct starting point because demand can only increase (monotone
transfer functions). The worklist iteration discovers demands by propagating
backward from uses.

AIMS's approach is correct and matches GHC's methodology for the backward
direction. The convergence documentation in `intraprocedural/mod.rs` line 63
("lattice has finite chain height 15") is a stronger guarantee than GHC's
iteration-count safety net.

**4. Separate Treatment of Alternative vs Sequential in Block Analysis**

GHC's `dmdAnalSumAlts` uses `lubDmdType` for case alternatives, while
`dmdAnalBindLetUp` uses `plusDmdType` for let body + RHS. AIMS mirrors
this cleanly:

- `compute_block_exit_state` uses `AimsState::join` (= alt_join) for
  combining successor entry states at branch/switch points
- `add_backward_demand` uses `Cardinality::seq_add` for accumulating
  demand within a block

This is the correct decomposition. The block boundary is the transition point:
within a block, composition is sequential; across successor edges at a
branch/switch, composition is alternative.

### New Invariants

**Invariant 1: Distributivity of seq_add over alt_join**

GHC's operations satisfy distributivity. AIMS already tests this
(`distributivity_seq_add_over_alt_join` in `lattice/tests.rs`):

```rust
a.seq_add(b.alt_join(c)) == a.seq_add(b).alt_join(a.seq_add(c))
```

This is critical for correctness when a block adds demand on a variable that
also has demand from multiple successor branches. The test exists and passes.
This is a strength of the AIMS test suite that should be documented as a
required algebraic property, not just a test.

**Invariant 2: Monotonicity of seq_add with respect to the lattice order**

If `a <= b` (in the lattice order), then `seq_add(a, x) <= seq_add(b, x)`.
This is required for the backward dataflow to converge: if successor demands
only increase (via join at block boundaries), accumulated demands within the
block must also only increase. AIMS should add an exhaustive test for this:

```rust
for a in all_cardinality() {
    for b in all_cardinality() {
        if a <= b {
            for x in all_cardinality() {
                assert!(a.seq_add(x) <= b.seq_add(x));
            }
        }
    }
}
```

**Invariant 3: The "Premise" for Sub-Demands**

GHC's Note [SubDemand denotes at least one evaluation] establishes that
sub-demand structure is conditional on evaluation. AIMS has an analogous
property: the `Consumption` and `Cardinality` dimensions in `AimsState`
describe usage conditional on the variable being live. The `Dead` +
`Absent` bidirectional sync in `canonicalize` enforces this -- a dead
variable has no usage count, and an absent variable is dead. This invariant
should be elevated to a documented design principle in
`lattice/dimensions.rs`, not just an implementation detail in `canonicalize`.

**Invariant 4: Loop back-edge demands must be monotone**

At a loop back-edge, the block's exit state joins the loop header's entry
state (from the previous iteration) with the loop body's contribution.
Because `alt_join` = `max` and `seq_add` = saturating-add-to-Many, the
combined demand can only stay the same or increase. This guarantees
convergence for loops. AIMS should document this in
`compute_block_exit_state` as a loop convergence argument, referencing GHC's
`dmdFix` as prior art.

---

## 09.3 What AIMS Should Not Adopt

### Reject

**1. Call Demands and Strictness Analysis (SubDemand, Call constructor)**

GHC's `SubDemand` type has a `Call !CardNonAbs !SubDemand` constructor that
tracks how many times a function is called and what demand is placed on its
result. This is essential for Haskell's lazy evaluation -- it enables
strictness analysis (determining that a lazy thunk is always forced).

Ori evaluates strictly. Every function argument is evaluated before the call.
There is no laziness to "discover" through demand analysis. AIMS does not
need:
- `Call` demands (function call context)
- `Prod` demands with per-field depth tracking (product scrutiny)
- `Poly` demands (uniform demand at all depths)
- Strictness predicates (`isStrict`)
- The lazy/strict lower-bound axis of GHC's `Card`

AIMS's flat `Cardinality` enum (`Absent | Once | Many`) captures everything
needed for RC optimization in a strict language. The sub-demand structure
would add complexity without benefit.

**2. Boxity Analysis**

GHC tracks `Boxity` (whether to unbox a value) as part of `SubDemand`.
This enables the worker-wrapper transformation where strict arguments are
unboxed. Ori's ARC system works at the allocation level, not the boxing
level. Values are either scalars (no allocation, no RC) or heap-allocated
(RC-tracked). The `ArcClass` classification (`Scalar | DefiniteRef |
PossibleRef`) already handles this at a coarser granularity. GHC's
fine-grained boxity would not provide additional optimization opportunities
in Ori's memory model.

**3. The Lower Bound of Card (Strictness Dimension)**

GHC's `Card` encodes a two-dimensional interval: `[lower, upper]` where lower
is the strictness bit and upper is the usage bound. In GHC, `C_10` (strict
but absent = bottom/diverges) vs `C_00` (lazy and absent = truly unused) is
a critical distinction for strictness analysis.

AIMS does not need the strictness dimension. In a strict language:
- Every evaluated expression is strict by definition
- The question is not "is this strict?" but "how many times is this used?"
- `Absent` in AIMS means "zero future uses" (= GHC's `C_00`), not "lazy"
- There is no equivalent of GHC's `C_10` (divergent) in AIMS because
  divergence is handled by the effect dimension (`may_throw`)

**4. Weak Free Variables and `reuseEnv`**

GHC's `splitWeakDmds` separates lazy, used-at-most-once demands from demand
signatures for recursive functions. `reuseEnv` stabilizes free-variable
demands to prevent oscillation during fixed-point iteration.

AIMS does not need these because:
- Strict evaluation means there are no "weak" (lazy) demands
- AIMS's worklist uses monotone lattice operations that guarantee convergence
  without demand stabilization tricks
- The convergence bound (`CHAIN_HEIGHT * vars * blocks`) provides a
  mathematical guarantee that GHC's empirical `n > 10` safety net does not

**5. `anticipateANF` and Memoization Heuristics**

GHC's `dmdAnalStar` applies `anticipateANF` to adjust cardinality for
expressions that will become let-bindings during ANF conversion. This is
because lazy evaluation + memoization means a let-bound thunk is evaluated
at most once, even if referenced multiple times.

AIMS operates on basic-block IR that is already in SSA form. There are no
thunks, no memoization, and no ANF conversion to anticipate. The SSA
property guarantees that each variable is defined exactly once, making the
memoization question moot.

**6. `multDmd` / `multCard` (Demand Scaling)**

As noted in 09.2, GHC's multiplicative composition (`multCard`) models
nested evaluation contexts in lazy evaluation. `multCard C_01 d` says
"if evaluated at most once, the inner demand `d` applies at most once."

In strict evaluation, every call evaluates its body exactly once. There is
no outer cardinality to multiply by. AIMS's `seq_add` handles all the
composition that Ori needs.

---

## 09.4 Plan Edits

### Section 09 (Dimensional Fusion) -- 09.4 Sequencing Algebra

The current plan text in `plans/aims/section-09-dimensional-fusion.md` at
09.4 says:

> Currently only cardinality has `seq_add` and `alt_join`. The other dimensions
> use plain `join` (pointwise max/widening).

This is correct. GHC's analysis confirms that for boolean/flag dimensions
(like `EffectClass`), `seq_add` = `alt_join` = bitwise OR, which is exactly
what AIMS already does with `join`. The plan items for Locality and Effect
`seq_add` are correctly marked as "document that this is intentional, not
accidental." No plan changes needed here -- the GHC review validates the
existing plan.

**Add to 09.4:** A new documentation item: explicitly state that `mult`
(GHC's `multCard`) is not needed for strict evaluation and that `seq_add`
subsumes the sequential composition role. This should go in the
`Cardinality` doc comment in `lattice/dimensions.rs` as a design note, and
in the sequencing algebra section of the plan as a "Verified Correct" item.

### Section 02 (Intraprocedural Documentation)

The current `intraprocedural/mod.rs` references GHC demand analysis in its
module-level docs (line 22). The `block.rs` documentation explains the
backward direction and the use of `alt_join` vs `seq_add`. These are
adequate but should be strengthened:

**Edit `intraprocedural/block.rs`:**
- Add a doc comment to `compute_block_exit_state` explaining that successor
  combination is *alternative* composition because at a branch/switch, only
  ONE successor executes per dynamic run. Currently the doc comment says
  this (line 33-35), but it should also cite the invariant:
  `alt_join(Once, Once) = Once` (not Many).
- Add a doc comment to `add_backward_demand` stating explicitly that this
  is GHC's `plus` operation adapted for a strict language (no `mult`
  needed), with the algebraic identity `Absent` and absorbing element
  `Many`.

**Edit `intraprocedural/mod.rs`:**
- The convergence documentation is strong (line 63: chain height 15,
  iteration limit formula). Add a note that this is stronger than GHC's
  approach (GHC uses `n > 10` empirical cutoff; AIMS has a mathematical
  bound derived from the product lattice height).
- Document the loop convergence argument: at back-edges,
  `alt_join` = `max` ensures monotonicity, so the demand on loop-carried
  variables can only increase or stay the same across iterations.

### Plans Overview

No changes needed to `plans/aims/00-overview.md`. The GHC reference is
already listed in the research lineage.

---

## 09.5 Code Changes (Later)

### `lattice/dimensions.rs` -- Cardinality Documentation

**File:** `compiler/ori_arc/src/aims/lattice/dimensions.rs`

Add to the `Cardinality` enum doc comment (after line 57):

```rust
/// # Design Notes
///
/// AIMS uses two composition operations on cardinality, following GHC demand
/// analysis (Sergey et al., POPL 2014):
///
/// - [`seq_add`](Self::seq_add): sequential composition along one execution
///   path. Corresponds to GHC's `plusCard`. Identity: `Absent`. Absorbing:
///   `Many`. `seq_add(Once, Once) = Many` because both uses happen.
///
/// - [`alt_join`](Self::alt_join): alternative composition at control-flow
///   merge points. Corresponds to GHC's `lubCard`. `alt_join(Once, Once) =
///   Once` because only one branch executes per dynamic run.
///
/// GHC has a third operation, `multCard` (demand scaling), which models
/// nested evaluation contexts in lazy evaluation. AIMS does not need this
/// because Ori evaluates strictly: every function body executes exactly once
/// per call, so there is no outer cardinality to multiply by.
///
/// # Algebraic Properties (tested exhaustively in `lattice/tests.rs`)
///
/// - `seq_add` is commutative, associative, with identity `Absent`
/// - `alt_join` is commutative, associative, idempotent
/// - `seq_add` distributes over `alt_join`
/// - Both operations are monotone with respect to the lattice order
```

### `lattice/tests.rs` -- Monotonicity Tests

**File:** `compiler/ori_arc/src/aims/lattice/tests.rs`

Add monotonicity tests for `seq_add`:

```rust
#[test]
fn seq_add_monotone_left() {
    for a in all_cardinality() {
        for b in all_cardinality() {
            if a <= b {
                for x in all_cardinality() {
                    assert!(
                        a.seq_add(x) <= b.seq_add(x),
                        "seq_add monotone left: {a:?} <= {b:?} => \
                         {a:?}.seq_add({x:?}) = {:?} <= {b:?}.seq_add({x:?}) = {:?}",
                        a.seq_add(x), b.seq_add(x)
                    );
                }
            }
        }
    }
}

#[test]
fn seq_add_monotone_right() {
    for a in all_cardinality() {
        for b in all_cardinality() {
            if a <= b {
                for x in all_cardinality() {
                    assert!(
                        x.seq_add(a) <= x.seq_add(b),
                        "seq_add monotone right: {a:?} <= {b:?} => \
                         {x:?}.seq_add({a:?}) = {:?} <= {x:?}.seq_add({b:?}) = {:?}",
                        x.seq_add(a), x.seq_add(b)
                    );
                }
            }
        }
    }
}
```

### `intraprocedural/block.rs` -- Documentation Strengthening

**File:** `compiler/ori_arc/src/aims/intraprocedural/block.rs`

Strengthen the doc comment on `add_backward_demand` (around line 192):

```rust
/// Add backward demand to a variable in the current state.
///
/// Uses `seq_add` for sequential composition: within a basic block,
/// instructions execute sequentially, so each instruction's demand on
/// an operand adds to the total demand on that operand.
///
/// This corresponds to GHC's `plusCard` (not `lubCard`). The critical
/// distinction:
/// - Two `Project` instructions reading the same source in the same block:
///   `seq_add(Once, Once) = Many` (both reads happen).
/// - The same source read in two alternative branches of a `Branch`:
///   `alt_join(Once, Once) = Once` (only one branch executes).
///
/// Algebraic properties (see `lattice/tests.rs`):
/// - Identity: `seq_add(Absent, x) = x` (no prior demand changes nothing)
/// - Absorbing: `seq_add(Many, x) = Many` (already saturated)
/// - Monotone: if `a <= b`, then `seq_add(a, x) <= seq_add(b, x)`
///   (increasing prior demand never decreases total demand)
```

### `intraprocedural/block.rs` -- Loop Back-Edge Documentation

Add a doc comment to `compute_block_exit_state` explaining loop convergence
(enhancement to existing comment around line 27):

```rust
/// # Loop Convergence
///
/// At a loop back-edge (successor = loop header), this function joins the
/// loop header's current entry state with the loop body's exit contribution.
/// Because `alt_join` = `max` on each dimension, and all dimensions are
/// finite-height lattices, the demand on loop-carried variables can only
/// increase or stay the same across worklist iterations. This guarantees
/// convergence without the demand-stabilization tricks that GHC uses for
/// lazy evaluation (`reuseEnv`, weak free variables). The convergence bound
/// is `CHAIN_HEIGHT * num_variables * num_blocks` (see `AimsState::iteration_limit`).
```

### `intraprocedural/tests.rs` -- Loop Demand Test

**File:** `compiler/ori_arc/src/aims/intraprocedural/tests.rs`

The existing `analysis_converges_for_simple_loop` test verifies that the
analysis terminates but does not verify the *demand values* in a loop.
Add a test that verifies a variable used inside a loop body gets `Many`
cardinality (because the loop body may execute multiple times):

```rust
#[test]
fn loop_body_use_escalates_to_many() {
    // Block 0: let v0 = construct; let v1 = construct(bool); jump block1
    // Block 1: let v2 = project(v0, 0); branch v1 -> block1, block2
    // Block 2: return v0
    //
    // v0 is used in block 1 (Project) which loops back. The loop
    // means v0 may be projected many times, so cardinality should
    // escalate to Many at the loop header's exit state.
    // ... (construct the ArcFunction and verify cardinality)
}
```

### Test Coverage for Exceptional Edges

**File:** `compiler/ori_arc/src/aims/intraprocedural/tests.rs`

The existing Invoke test (around line 398) verifies that `dst` is defined
only in the normal successor. Add a test verifying that variables live
across an Invoke have correct demand on both the normal and unwind edges:

```rust
#[test]
fn invoke_live_var_demand_on_both_edges() {
    // A variable live before an Invoke should have demand recorded
    // on both the normal and unwind edges (via InvokeEdgeState),
    // because exceptional control flow also needs RC cleanup.
    // ... (construct the ArcFunction and verify InvokeEdgeState)
}
```

---

## 09.6 Lens Shift

### What Changes for Paper 10 (Concurrent RC)

GHC's demand analysis operates in a single-threaded evaluation model. The
`plusCard` operation assumes sequential execution: "both sides happen in
sequence." When reading Paper 10 (Concurrent Immediate Reference Counting),
the key question becomes: **does concurrent execution change the composition
algebra?**

In concurrent execution, two references to the same object may be used
"simultaneously" rather than sequentially. From the RC perspective:
- Sequential `Once + Once = Many` (GHC's `plusCard`) means two sequential
  uses may need the value to stay alive across both.
- Concurrent `Once || Once` is different: both uses happen at the "same time,"
  but the value still needs to survive both. The cardinality is still `Many`
  from the RC perspective (refcount must be at least 2 during concurrent use).

So `seq_add` may still be correct for concurrent composition, but the
**atomicity** of RC operations becomes the concern, not the algebra. Read
Paper 10 looking for whether the demand algebra changes under concurrency
or whether it is only the RC implementation that changes (atomic inc/dec).

### What Changes for Paper 11 (Cyclic RC)

GHC does not deal with cycles (Haskell uses GC). AIMS's backward demand
analysis assumes acyclic data flow (SSA form). Cyclic references would
mean a variable's demand depends on itself -- a self-referential fixed
point that the current worklist cannot express. Read Paper 11 looking for
whether cyclic RC requires changes to the demand algebra or only to the
RC runtime (cycle detection/collection).

### What Changes for Paper 12 (Bit-Stealing)

No change. GHC's demand analysis is orthogonal to representation
optimization. Bit-stealing changes the physical layout of RC metadata,
not the demand algebra.

---

## 09.7 Open Risk

### Risk 1: `seq_add` Monotonicity Is Not Tested

The existing lattice tests verify associativity, commutativity, identity,
and absorbing element for `seq_add`. They also verify distributivity of
`seq_add` over `alt_join`. **But they do not test monotonicity** -- the
property that if `a <= b` then `seq_add(a, x) <= seq_add(b, x)`.

Monotonicity is required for the backward dataflow to converge: if demands
at block boundaries only increase (via `join` = `max`), then accumulated
demands within the block (via `seq_add`) must also only increase. Without
this property, the worklist could oscillate.

**Impact:** Low -- AIMS's `Cardinality` is a 3-element chain (`Absent <
Once < Many`) where `seq_add` is equivalent to saturating addition. Monotonicity
holds trivially for such a small domain. But the test should exist to guard
against future changes to the lattice (e.g., adding finer cardinality
distinctions like `Twice`).

**Mitigation:** Add the exhaustive monotonicity tests described in 09.5.

### Risk 2: Loop Back-Edge Demands Are Under-Tested

The test `analysis_converges_for_simple_loop` verifies termination but does
not verify the *demand values* produced by a loop. There is no test that a
variable used inside a loop body gets `Many` cardinality, which is the
critical optimization decision (loop body variables need full RC, not linear
move optimization).

**Impact:** Medium -- a bug in loop demand propagation could cause AIMS to
emit `Once` cardinality for a variable used in a loop, leading to a missing
`RcInc` and a use-after-free at runtime.

**Mitigation:** Add the loop demand escalation test described in 09.5.

### Risk 3: Invoke Exceptional Edge Demand Is Under-Tested

`InvokeEdgeState` separates normal and unwind demands, which is necessary for
correct RC cleanup on exception paths. The existing test verifies that `dst` is
defined only in the normal successor, but there is no test verifying that a
variable live across an Invoke has correct demand on both edges.

**Impact:** Medium -- incorrect unwind demand could lead to missing `RcDec` on
the exception path (memory leak) or spurious `RcDec` (use-after-free).

**Mitigation:** Add the invoke edge demand test described in 09.5.

### Risk 4: Documentation Relies on Tests, Not Specification

GHC documents its demand algebra with set-theoretic specifications
(Note [Algebraic specification for plusCard and multCard]) that exist
independently of the implementation. AIMS documents `seq_add` with a
truth-table comment (`Absent + x = x`, `Once + Once = Many`,
`Many + _ = Many`). This is complete for a 3-element domain but would
not scale if the lattice grows.

**Impact:** Low -- the 3-element domain is small enough that the truth
table IS the specification. But if AIMS ever adds finer cardinality
(e.g., `Twice` for exactly-two-uses optimization), the truth-table style
would be error-prone.

**Mitigation:** Add the algebraic specification as a doc comment (09.5)
and reference the GHC Note as prior art.

### Risk 5: No Cross-Dimension Monotonicity Test

AIMS tests join laws per dimension and for the full `AimsState` product
lattice. But there is no test that `seq_add` on `Cardinality`, when
embedded in the product (via `add_backward_demand`), is monotone with
respect to the product order. A bug where `add_backward_demand` accidentally
widens a non-cardinality dimension could break convergence.

**Impact:** Low -- `add_backward_demand` currently only modifies
`cardinality` (via `seq_add`) and `consumption` (bumping to `Affine`).
Both are monotone operations. But the function is a natural place for
future changes that could violate this.

**Mitigation:** Add a property test that `add_backward_demand` is monotone:
if `state_a <= state_b` (product order), then
`add_backward_demand(state_a, var, card) <= add_backward_demand(state_b, var, card)`.
This is an integration-level test for the block analysis, not a lattice-level test.
