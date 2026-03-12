---
section: "02"
title: "FP² — Fully in-Place Functional Programming"
status: complete
goal: "Determine whether FIP in AIMS is an output or half a side-analysis, and what preconditions MemoryContract should expose"
paper:
  title: "FP²: Fully in-Place Functional Programming"
  doi: "https://doi.org/10.1145/3607840"
  venue: "ICFP 2023"
  authors: "Lorenzen, Leijen, Swierstra"
depends_on: ["01"]
sections:
  - id: "02.1"
    title: "Paper Thesis"
    status: complete
  - id: "02.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "02.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "02.4"
    title: "Plan Edits"
    status: complete
  - id: "02.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "02.6"
    title: "Lens Shift"
    status: complete
  - id: "02.7"
    title: "Open Risk"
    status: complete
---

# Section 02: FP2 -- Fully in-Place Functional Programming

**Status:** Complete
**Goal:** Determine whether FIP in AIMS is a true output (derived from converged state) or
still half a side-analysis, what exact preconditions `MemoryContract` should expose for FIP
certification, and which current "reuse" ideas should be reframed as certification conditions.

**Paper:** Lorenzen, Leijen, Swierstra, "FP2: Fully in-Place Functional Programming," ICFP 2023.
[DOI: 10.1145/3607840](https://doi.org/10.1145/3607840) — 30 pages, Open Access (CC-BY).
Local copy: `3607840.pdf`.

**Why read this second:** This gives the strongest criterion for when "in-place" is a
theorem instead of an optimization guess. It defines what must be true for a function to
be certified FIP -- allocation balance, frame-limited reuse, no net heap growth.

**Pause questions:**
- Is FIP in AIMS an output or still half a side-analysis?
- What exact preconditions should `MemoryContract` expose?
- Which current "reuse" ideas should be reframed as certification conditions?

**AIMS context:**
- `FipContract` exists but is `Never` for all functions in Stage 1
- `EffectClass` tracks `may_alloc`, `may_share`, `may_throw`
- `ShapeClass` tracks reuse compatibility (ReusableCtor, CollectionBuffer)
- Section 05 handles reuse emission; Section 09 plans FIP as a view of converged state
- FBIP enforcement is a separate read-only diagnostic pass

---

## 02.1 Paper Thesis

FP² establishes a **certification criterion** -- not an optimization heuristic -- for when
a purely functional program can execute fully in-place. The core claim (from the abstract):

> A wide class of purely functional programs can be executed safely using in-place updates
> without requiring allocation, provided that the function's arguments are not shared
> elsewhere.

The paper introduces a **linear fully in-place (FIP) calculus** with a central theorem
(Theorem 2, p.12): for any well-formed FIP program, the store size never changes during
evaluation (`|S| = |S'|`). This is a provable property of the program, not an optimization
that the compiler tries harder at.

### Paper Structure

The paper presents four increasingly permissive calculi, each a strict subset of the next:

```
FIP ⊂ FBIP ⊂ λ^fip
 │      │       │
 │      │       └─ Full Perceus RC (dup, dropru, alloc, lambdas) — any program
 │      └───────── FIP + deallocation (drop, free) — can only shrink store
 └──────────────── Pure in-place — store size never changes
```

Plus a stack-safe variant FIP^S that constrains recursion for bounded stack.

### Formal Concepts

**1. Owned (Γ) vs Borrowed (Δ) Environments (Fig. 4, p.10).**
The FIP calculus distinguishes two environments:
- **Γ (owned)**: a *multiset* of variables `x` and reuse credits `◇_k`. Variables in Γ are
  used linearly — consumed exactly once. This is the key formal device: linearity of Γ
  ensures every resource is accounted for.
- **Δ (borrowed)**: a *set* of variables that can be freely inspected (via BMATCH, BAPP,
  CALL) but never consumed, stored in data structures, or destructively matched.
- Borrowed parameters come from function signatures (`f(ȳ; x̄) = e` where ȳ is borrowed,
  x̄ is owned) or from the LET rule which can borrow Γ₂ in the derivation of e₁.

AIMS mapping: `AccessClass::Borrowed` ≈ Δ; `AccessClass::Owned` ≈ Γ.

**2. Reuse Credits ◇_k from Destructive Match (DMATCH! rule, Fig. 4).**
When `match! x { C_i x̄_i → e_i }` destructs an *owned* variable `x`, each branch receives:
- The matched fields `x̄_i` as owned variables
- A reuse credit `◇_k` where `k = |x̄_i|` (the constructor's *arity* — number of fields)
The credit represents the memory cell of the destructed constructor. It is consumed by
the REUSE rule to allocate a new constructor of the same arity k. The linearity of Γ
ensures each credit is consumed exactly once.

Key: `k` is constructor *arity* (field count), not byte size. A 3-field struct produces
`◇_3`. A 2-field constructor cannot consume `◇_3` (arity mismatch). This is how the
paper tracks allocation balance without needing types — just field counts.

**3. Atoms and Unboxed Tuples — Critical FIP Enablers (Section 1, 2.2).**
Two features are essential for FIP to be practical:
- **Atoms**: Constructors with 0 fields (`Nil`, `True`, `False`, integers, floats). The ATOM
  rule: `Δ | ∅ ⊢ C` — atoms need no owned resources, hence zero allocation. Without atoms,
  even `Nil` at the end of a list would need allocation, breaking FIP for most programs.
- **Unboxed tuples**: Multi-value returns `(v₁,...,vₙ)` are syntactically *expressions*, not
  *values*. They cannot be stored in constructors or passed to functions, so they never
  cause heap allocation. The TUPLE rule splits Γ for each component. In Koka's
  implementation, tuples are register-passed value types.

AIMS implication: `ShapeClass` should recognize atom constructors as zero-credit (no token
needed for reuse). Unboxed tuples map naturally to Ori's existing tuple-return convention.

**4. Store Semantics and the In-Place Theorem (Section 2.3, Fig. 5, p.11-12).**
The paper defines *store semantics* with a fixed-size store S containing:
- Bindings: `x ↦ C^k x₁...x_k` (constructor with its fields)
- Reuse credits: `◇_k`

The store is *linear*: every variable in dom(S) occurs at most once in the free variables
of rng(S). Linearity ensures mutation is safe when a binding has exactly one reference.

**Theorem 1** (Soundness, p.12): If `Δ | Γ ⊢ e` and given disjoint stores S₁ (borrowed,
sound) and S₂ (owned, linear) with appropriate conditions, then functional evaluation
`[S₁,S₂]x̄ = v̄` implies store evaluation `S₁,S₂ | e ↦*_s S₁,S₃ | x̄` where S₃ is linear
and borrowed values in S₁ are unchanged.

**Theorem 2** (In-place, p.12): For any `S | e ↦*_s S' | e'`, we have `|S| = |S'|`.
*Store size never changes.* This is the core certification: FIP programs can run on a
fixed pre-allocated store with zero dynamic (de)allocation.

**Corollary 1** (p.12): If `∅ | ◇̄_k ⊢ e`, then we can evaluate on a store containing
exactly those reuse credits — no more memory is ever needed.

**5. FBIP: Allowing Deallocation (Section 2.4, Fig. 6, p.13).**
FBIP extends FIP with two new forms:
- `drop x; e` — consumes owned variable x from Γ (allows freeing memory)
- `free k; e` — discards a reuse credit ◇_k (allows not reusing a cell)

**Theorem 3** (p.13): FBIP can only deallocate: `|S| ≥ |S'|`. The store can shrink but
never grow. In Koka, the `fbip` keyword checks for well-formed FBIP.

Key distinction from FIP: FBIP allows *unused* destructor credits to be freed rather than
requiring they be consumed. This is strictly more permissive — `FIP ⊂ FBIP`.

**6. Stack-Safe FIP^S (Section 2.5, p.13-14).**
FIP guarantees bounded heap but not bounded stack. FIP^S adds two constraints:
- Every function belongs to a mutually recursive group f̄
- All recursive calls within f̄ must be in tail position
- Σ may only contain functions defined before f̄, or in f̄ itself

**Theorem 4** (p.14): At any intermediate step, the evaluation context size |E| is bounded
by `|e_max| · |Σ|²` — constant stack per function, bounded by program size.

AIMS implication: FIP certification should include a recursion classification (none / tail /
non-tail) derived from SCC analysis. `FipContract::Certified` should require no recursion
or tail-only; non-tail recursion blocks `Certified` even if allocation-balanced.

**7. The λ^fip Calculus — Dynamic Embedding (Section 5, Fig. 8-9, p.21-25).**
The paper's deepest contribution: unifying the FIP calculus with Perceus RC into a single
formal system λ^fip. This extends FBIP with:
- `dup x; e` — duplicates an owned or borrowed variable (increments RC)
- `dropru x; e` — drops x, creates ◇_k where k = size(x) (RC-aware drop-with-reuse)
- `alloc k; e` — creates a fresh credit ◇_k (allocates new memory)
- Full lambda expressions `λ^z̄ x̄. e` with explicit free variables

The key insight (p.22): **FIP is exactly the subset of λ^fip that excludes rules requiring
dynamic reference counting.** The strict containment `FIP ⊂ FBIP ⊂ λ^fip` means the FIP
calculus is not a separate system — it is the *non-RC fragment* of full Perceus.

The `dropru` rule has two heap semantics (p.23):
- **Unique** (rc=1): `(dconru_h)` — creates ◇_k, drops constructor fields
- **Shared** (rc>1): `(dropru_h)` — decrements rc, allocates *fresh* ◇_k

This is precisely the `is-unique` branch that AIMS already emits via `IsShared` + `Branch`.
The paper proves (Theorem 6, p.25) that this heap semantics is sound for well-formed
λ^fip programs: correct RC counts, no premature drops, no garbage at evaluation end.

**8. TRMReC: Tail Recursion Modulo Reusable Contexts (Section 3, p.14-17).**
A major technical contribution: a *general transformation* from direct-style recursive
functions over polynomial inductive datatypes into tail-recursive fully in-place versions.

The transformation (Fig. 7, p.16):
1. Identify evaluation contexts E_i in the recursive function body
2. Create zipper constructors Z_i carrying free variables z̄_i and a link to parent
3. Transform `f` into `f'(ȳ; x̄, z)` where z is the current zipper
4. Provide `app(ȳ, z, x̄')` to apply the zipper on the way back up
5. Recursive calls in tail position → direct tail calls to f'
6. Recursive calls in evaluation contexts → convert to tail calls with Z_i

**Theorem 5** (p.16): The TRMReC transformation is sound — if the side-condition holds
(E_i does not depend on borrowed variables, and a credit of the right size exists), the
result is a tail-recursive, fully in-place program.

Key insight: the defunctionalized CPS contexts (the zipper type) correspond to the
*derivative* of the original datatype. For a `tree` with `Bin(left, right)`, the derivative
has `BinL(up, right)` and `BinR(left, up)` — exactly a Schorr-Waite traversal (Section 3.2).

AIMS implication: TRMReC is an *opportunity creation* transform (Stage 3). AIMS's job is
to certify the result; TRMReC creates the right structure for certification to succeed.

**9. Padding and Buffers for Size-Mismatched Constructors (Section 4.3, p.20-21).**
When constructors of a datatype have different arities, reuse credits don't match up.
Two techniques:
- **Padding**: Add dummy `Pad` atom fields to make all constructors the same arity.
  Example: finger tree `More(One(...), ...)` and `More(Triple(...), ...)` — pad smaller
  variants to match the largest.
- **Credit buffers**: Pair a data structure with a buffer that stores excess credits.
  The buffer has an invariant (e.g., for finger trees: `n₁ + 2n₂ + n₃` credits for
  n₁ Triple, n₂ Three, n₃ Two constructors). The buffer is never empty.

AIMS implication: `ShapeClass` should eventually support padded constructors (all variants
same size) as a reuse-enabling transformation. This is a Stage 3+ concern.

**10. Benchmarks (Section 6, Fig. 10, p.25-26).**
Five benchmarks (AMD 7950X, Koka v2.4.2, 100 iterations over N=100000 elements):
- **rbtree**: FIP rivals C++ `std::map`. `fip` 1.03x of std-reuse; C++ 2.47x.
- **ftree**: Finger tree cons/snoc. `fip` 1.07x of std-reuse.
- **msort**: In-place merge sort. `fip` 0.98x of std-reuse (faster!).
- **qsort**: In-place quicksort. `fip` 1.19x of std-reuse. C++ 1.69x.
- **tmap**: Map over shared tree. `fip` *slower* (0.74x) — Schorr-Waite pointer reversal
  costs more than stack recursion when the tree is shared (no reuse possible anyway).

Key observations:
- `std-reuse` (Koka with dynamic reuse, no `fip` annotation) is already very effective.
  The `fip` annotation primarily adds a *guarantee*, not necessarily more performance.
- The dynamic reuse check (`is-unique`) has negligible overhead (~1% for tmap).
- FIP shines when arguments are unique; on shared data, the guarantee is maintained but
  the fast path (in-place) is not taken.

---

## 02.2 What AIMS Should Adopt

### Keep

**K1. Allocation balance as a derived property of converged state.**
FP²'s in-place theorem (Theorem 2: `|S| = |S'|`) maps directly to what AIMS can derive
from the converged `AimsStateMap`: count variables that transition `Alive -> Dead` with
`ShapeClass::ReusableCtor` (token sources) and count `Construct` instructions (token
consumers). If the counts balance per-function, the function is FIP-eligible. This is
exactly what Section 09.3 Rule 7 proposes: `EffectClass::NONE + alloc-balanced ->
FIP-natural`. The key insight from FP²: this is not a heuristic. It is a proof obligation
backed by Theorem 2.

**K2. Token-level tracking by constructor arity, not just count-level.**
FP² tracks reuse credits by *constructor arity* `k` (number of fields), not by type or
byte size. A `match!` on a 3-field constructor produces `◇_3`; the REUSE rule requires
`◇_k` to build a k-field constructor. A 2-field constructor cannot consume `◇_3` (arity
mismatch). AIMS currently matches reuse by *same type* (Section 05.1: `d.ty == a.ty`),
which is stricter than FP²'s arity-class matching — same type implies same arity, but
not vice versa. This is correct for Stage 1 but should be documented as a conservative
approximation. The `SizeClass` type already exists in `lattice/mod.rs` — Stage 2+ should
activate arity-class matching for cross-type reuse (e.g., reusing a `Node` cell for a
`NodeL` zipper element of the same arity).

**K3. Per-branch token linearity, not probability weighting.**
The paper handles branch divergence through its formal system: the DMATCH! rule (Fig. 4)
places `◇_k` into the owned environment Γ *within each branch arm*. The linearity of Γ
ensures each credit is consumed exactly once on its branch's control flow path. There is
no branch probability tracking in the paper's formalism — that is a Koka implementation
detail (`gammaDia` with `Ratio Int` weights in `CheckFBIP.hs`).

For FIP certification, AIMS needs the structural invariant from the paper: a reuse credit
is valid only on the control flow path where it was produced. AIMS currently treats branch
divergence via `ShapeClass::join`, which collapses mismatched shapes to `NonReusable` at
merge points. This is sound (conservative). For FIP certification, the key check is: on
every branch arm of a destructive match, is the produced credit consumed by a constructor
on that same arm? This is a per-arm balance check, not a cross-branch join.

**K4. FIP requires no deallocation, not just no allocation.**
FP²'s Theorem 2 (`|S| = |S'|`) means the store *never changes size* — neither allocation
nor deallocation. The FBIP extension (Theorem 3: `|S| ≥ |S'|`) is strictly weaker, allowing
shrinkage via `drop` and `free`. A function that destructs a value and frees it (without
reusing the cell) is FBIP, not FIP. AIMS needs to distinguish these: `missed_reuses == 0`
means all destructions are reused (FIP); `missed_reuses > 0` means some destructions are
freed (FBIP at best). The current `missed_reuses` counter in `EmitReuseResult` is exactly
this signal.

**K5. The `FipContract::Conditional` requires_unique_params precondition.**
FP²'s λ^fip calculus (Section 5) formalizes the dynamic embedding: `dropru x` checks
uniqueness at runtime. If unique, in-place reuse via `(dconru_h)`; if shared, fresh
allocation via `(dropru_h)`. AIMS's `FipContract::Conditional { requires_unique_params }`
already models this. The paper adds the formal obligation: the caller must ensure owned
parameters are unique for the FIP fast path. This proof comes from the caller's converged
`AimsState`. When all `requires_unique_params` are satisfied, the caller can assume the
callee's `EffectSummary` is `{ may_allocate: false, may_share: false }` — a call-site
contract override, not code specialization.

**K6. Atoms as zero-credit constructors.**
FP²'s ATOM rule (`Δ | ∅ ⊢ C`) is crucial: zero-field constructors need no owned resources.
This means `Nil`, `True`, `False`, enum variants without payloads, etc. are "free" in the
FIP accounting. AIMS's `ShapeClass` should mark atom constructors distinctly — they
produce `◇_0` which can only be consumed by the EMPTY rule or another atom allocation, and
atoms need no credit to construct. This prevents false negatives in FIP certification where
atomic constructors would otherwise appear as unmatched allocations.

**K7. The FIP ⊂ FBIP ⊂ λ^fip containment as architectural validation.**
The paper proves these three calculi form a strict containment hierarchy (p.22). The FIP
fragment is exactly the non-RC subset of full Perceus. This validates AIMS's architecture:
the lattice analysis determines *which fragment each function falls into*:
- `Uniqueness::Unique` everywhere → FIP fragment (static guarantee)
- `Uniqueness::MaybeShared` somewhere → λ^fip fragment (dynamic RC needed)
- Token-balanced → FIP/FBIP; unbalanced → general λ^fip
The lattice doesn't need to "implement FIP" — it naturally classifies functions into these
fragments, and the classification determines what guarantees can be certified.

### New Invariants

**N1. FIP Certification Invariant (must be documented in `contract/mod.rs`).**
Grounded in Theorem 2 and the DEFFUN rule (Fig. 4, p.10):
```
A function f has FipContract::Certified iff:
  (a) f.effects.may_allocate == false   (or alloc_only_on_slow_path == true)
  (b) f has zero missed_reuses (all destructions matched by constructions)
  (c) f has no recursive calls (fip) OR only tail-recursive calls (fbip)
  (d) For every match arm that destructs a constructor of arity k,
      a constructor of arity k is built on that arm's control flow path
```
Note: condition (d) is per-arm, not per-function. The paper's formal system ensures this
structurally through the linearity of Γ within each branch of DMATCH!.

The original version listed "total/empty effects (no panics, no IO)" as a condition. The
paper's calculus is simply pure — it has no concept of effects. For Ori, which has capability
effects, the relevant check is `may_allocate` and `may_share`, not a blanket effect ban.

**N2. Token Balance Invariant.**
For every function with `FipContract != Never`:
```
  sum(reuse_tokens_produced) == sum(reuse_tokens_consumed)
```
Where token production = `RcDec` that feeds a `ReuseOpportunity`, and token consumption =
`Construct` replaced by in-place `Set` instructions. This corresponds to Theorem 2's
`|S| = |S'|`. Currently implicit in `EmitReuseResult` counters but should be an explicit
assertion in the verification pass.

**N3. Per-Arm Token Balance Invariant.**
From the paper's DMATCH! rule: each branch arm of a destructive match receives `◇_k` in
its own Γ. The linearity of Γ ensures it is consumed within that arm. The AIMS structural
equivalent: for FIP certification, check that each match arm that destructs a constructor
of arity k has a constructor allocation of arity k on its dominated control flow path.
This is stronger than global count balance — it ensures structural correspondence between
destructions and constructions.

---

## 02.3 What AIMS Should Not Adopt

### Reject

**R1. Separate FIP type system / syntax annotation.**
Koka adds `fip` and `fbip` keywords to function declarations, making FIP a source-level
annotation that the programmer must apply. Ori should NOT require this. FP²'s own thesis
supports this rejection: the certification is a *derivable property* of the code, not a
programmer declaration. AIMS computes FIP certification from converged analysis state. The
programmer should not need to annotate functions as `fip` -- the compiler should infer it.
If Ori later adds a `fip` annotation, it should be an assertion (like `pre()`/`post()`), not
a type system feature -- the compiler checks the assertion against its own analysis, it does
not use the annotation to guide the analysis.

**R2. Koka's `AllocTree` structure for allocation tracking.**
The Koka implementation (`CheckFBIP.hs`) builds an `AllocTree` data structure to track
allocation/deallocation patterns across branches. This is an *implementation choice* for
Koka's checker, not a concept from the paper's formal system. The paper's calculus tracks
allocation balance structurally through the linearity of Γ in the typing rules — no
separate tree is needed. AIMS should follow the paper's approach: derive allocation balance
from the already-converged state (count `ShapeClass::ReusableCtor` deaths matched by
`Construct` instructions). The `AimsStateMap` already contains this information. Building
a separate `AllocTree` would violate the AIMS litmus test (Question 4: "Can it be derived
from AimsStateMap + MemoryContract alone?").

**R3. Koka's fractional probability weights for token tracking.**
The Koka implementation uses `Ratio Int` (rational numbers) to track token availability
across branches with fractional weights. This is a Koka implementation choice, not from the
paper's formal system. The paper handles branches purely through the DMATCH! rule placing
`◇_k` into the branch-local Γ — the linearity of Γ does the rest. AIMS should follow the
paper: per-arm balance checking (each arm individually balances its credits). The structural
equivalent — dominator/post-dominator validation that the token is consumed on its producing
arm's control flow — is already approximated by `ReusePlanner`. Binary per-arm balance is
sufficient for certification.

**R4. Blanket effect restriction for FIP.**
The paper's FIP calculus is simply a pure functional calculus — it has no concept of effects
at all. The Koka implementation adds an effect restriction (total/empty effects only)
because Koka has algebraic effects where this is natural. Ori has capability effects
(`uses Http`, `uses Suspend`). Restricting FIP to "no capabilities" would be too coarse.
AIMS should gate FIP on its own `EffectSummary`: `may_allocate` and `may_share` are the
relevant checks. A function using `uses Print` for logging should be FIP-eligible if it
doesn't allocate. `may_throw` may optionally block FIP certification (a panic allocates
an error value), but this should be configurable, not bundled with effect purity.

**R5. `fbip(n)` as a separate certification tier with per-call allocation bounds.**
FP² distinguishes `fip` (zero allocation, Theorem 2) from `fbip` (deallocation-only,
Theorem 3). Koka extends this with `fip(n)` and `fbip(n)` for bounded allocation. AIMS
should not model per-call allocation bounds as a first-class concept. Instead, AIMS's
`FipContract::Conditional` already captures the useful distinction: "FIP when preconditions
hold, standard when they don't." The per-call bound adds complexity without clear benefit
for Ori's use cases. FBIP remains a post-pipeline diagnostic.

**R6. Unboxed tuples as a syntactic restriction.**
The paper enforces that tuples are expressions (not values) via a syntactic restriction —
tuples cannot appear in constructor arguments. This prevents tuple values from escaping to
the heap. Ori already handles this differently: tuples are value types passed in registers,
and the compiler controls tuple representation. AIMS does not need a syntactic restriction
to achieve the same effect.

---

## 02.4 Plan Edits

### Section 09 (Dimensional Fusion)

**09.2 Effect Activation -- tighten FIP-natural detection.**
Current plan (09.2): "may_alloc == false AND all Consume matched by Construct -> FIP-natural."
**Revision:** Add the FP²-derived conditions from invariant N1:
- `missed_reuses == 0` (no unmatched destructions -- no deallocation)
- No recursive calls (for `Certified`), or only tail-recursive calls (for `Conditional`)
- Per-arm token balance (not just global count balance)
The `fip_alloc_balanced` tracking proposed in 09.3 Rule 7 should be renamed to
`fip_token_balanced` and should check both allocation AND deallocation balance, not just
allocation absence.
<!-- reviewed: completeness fix — The AIMS plan Section 09.2 "Effect -> FIP natural
detection" (line 359) currently says: "may_alloc == false AND all Consume matched by
Construct (allocation balance = 0) -> function is naturally FIP." The proposed revision
adds missed_reuses and recursion checks. These are sound additions. Note: missed_reuses
is an emission-phase quantity (from EmitReuseResult), not an analysis-phase quantity.
This creates a dependency: FIP certification at contract extraction time (interprocedural)
cannot know missed_reuses because emission hasn't run yet. Resolution: FIP certification
must be split into two phases — tentative (analysis-time, from effect+balance) and
confirmed (emission-time, from missed_reuses). Cross-dependency: Section 03 (FIPTree)
also proposes FipContract expansion (Fip/Fbip/Bounded variants). -->

**09.3 Enriched Canonicalize -- add Rule 7 detail.**
Rule 7 currently says: `EffectClass::NONE + alloc-balanced -> FIP-natural`. Revise to:
`EffectSummary.may_allocate == false && missed_reuses == 0 && no_recursion -> FipContract::Certified`.
This makes FIP certification a formal function-level property derived from the converged
state, not a per-variable canonicalize rule. (FIP is inherently function-level, not
per-variable. It should NOT be in canonicalize. Move to contract extraction in
`interprocedural.rs`.)
<!-- reviewed: completeness fix — ALREADY ACKNOWLEDGED in AIMS plan. Section 09.3 Rule 7
(line 457) already says: "FIP-natural is a function-level property (not per-variable
state)" and "Track allocation balance in the function-level accumulation (same mechanism
as EffectSummary)." The plan already places this in analyze_function() return value and
MemoryContract, not in canonicalize(). The edit here is confirmatory: it validates the
AIMS plan's existing decision. The specific formula revision (adding missed_reuses and
no_recursion) is genuinely new content from FP2. -->

### Section 05 (Reuse Emission)

**05.4 FIP as Contract -- add deallocation tracking.**
Current text says "FIP drives reuse emission; FBIP validates the result." **Add:** FIP
certification requires `EmitReuseResult.missed_reuses == 0`. The existing
`tracing::warn!` when a FIP-certified function has unmatched deaths should be upgraded to
a hard verification error in `verify/mod.rs` -- a function claiming `FipContract::Certified`
that has missed reuses is unsound (violates Theorem 2).
<!-- reviewed: completeness fix — PARTIALLY PRESENT. Section 05.4 already has:
"EmitReuseResult.missed_reuses tracks death events with no compatible allocation.
emit_reuse() emits tracing::warn! when a FIP-certified function has unmatched deaths."
What is NEW: upgrading warn to hard error in verify/mod.rs. This is a sound proposal but
requires Stage 2 (FipContract inference must be active first — Stage 1 uses Never for all
functions, so missed_reuses + Never is not a contradiction). -->

### Section 07 (Advanced Optimizations)

**07.3 Cross-Optimization Synergies -- add FIP call-site specialization.**
When calling a `FipContract::Conditional` function where the caller proves all
`requires_unique_params` are `Unique`, the caller should use the callee's FIP-optimized
contract (`may_allocate: false, may_share: false`) instead of the conservative contract.
This corresponds to FP²'s dynamic embedding (Section 5): `dropru` on a unique binding
uses the `(dconru_h)` fast path.
<!-- reviewed: completeness fix — PARTIALLY PRESENT. Section 05.4 already documents this
mechanism: "FIP benefits are realized at call sites: when the caller's AIMS analysis
proves that the arguments corresponding to requires_unique_params are Unique, the caller
knows the callee will hit all fast paths (no allocation)." What is NEW: using the
FIP-optimized contract (may_allocate: false, may_share: false) for the caller's own
analysis — this is a contract-switching optimization at call sites. The existing plan
describes the downstream effect but not the mechanism of swapping contracts. -->

### plans/aims/00-overview.md

**Research Lineage table -- update FP² entry.**
Current: "Reuse credits as first-class lattice element; FIP certification criterion."
**Revision:** "In-place theorem (Theorem 2: |S|=|S'|) as proof obligation; reuse credit
linearity (consumed exactly once per branch arm); FIP/FBIP containment validates
AIMS's lattice-derived classification; atoms/unboxed-tuples as zero-credit enablers;
TRMReC as generic opportunity creation for polynomial datatypes; two embeddings (static
uniqueness, dynamic RC) map to AIMS Unique/MaybeShared paths."
<!-- reviewed: completeness fix — The existing entry is concise. The proposed expansion
is significantly more detailed. Recommend a middle ground: keep it under 2 lines to match
other entries' style, but add the Theorem 2 proof obligation and two-embedding mapping.
-->

**Legacy Concept Collapse Table -- update FIP row.**
Current: "FIP certification | Derived view of EffectClass + allocation balance | effect, locality, shape."
**Revision:** "FIP certification | Derived from EffectSummary.may_allocate + missed_reuses + recursion check | effect, shape, consumption (for token balance), uniqueness (for Conditional preconditions)."
<!-- reviewed: completeness fix — The existing FIP row in the collapse table (line 97)
says: "Derived view of EffectClass + allocation balance | effect, locality, shape". The
proposed revision adds dimensions (consumption, uniqueness). This is accurate: FIP does
depend on more dimensions than the current row lists. Sound edit. -->

### EffectSummary in contract/mod.rs

**Add `may_deallocate: bool` field** (or derive from emission results). This is the missing
half of FP²'s in-place theorem. `may_allocate == false` alone gives FBIP (Theorem 3:
|S| >= |S'|). Full FIP (Theorem 2: |S| = |S'|) additionally requires `may_deallocate ==
false` -- every destructor's memory must be reused, not freed. Compute as
`missed_reuses > 0` at emission time.
<!-- reviewed: completeness fix — EffectSummary already exists in contract.rs (Section
03.1) with fields: may_allocate, alloc_only_on_slow_path, may_share, may_throw. Adding
may_deallocate is a structural change. Note the circular dependency: may_deallocate is
computed from emission results (missed_reuses), but EffectSummary is set during
interprocedural analysis (before emission). Resolution: may_deallocate must be set in a
post-emission update to the contract, not during initial contract extraction. This is the
same issue noted in the 09.2 review comment above. -->

---

## 02.5 Code Changes (Later)

### `compiler/ori_arc/src/aims/contract/mod.rs`

**C1. Add `may_deallocate` to `EffectSummary`.**
```rust
pub struct EffectSummary {
    pub may_allocate: bool,
    pub alloc_only_on_slow_path: bool,
    pub may_share: bool,
    pub may_throw: bool,
    pub may_deallocate: bool,  // NEW: unmatched destructions exist
}
```
Update `CONSERVATIVE` (true), `OPTIMISTIC` (false), `join` (OR). Populate from
`EmitReuseResult.missed_reuses > 0` during pipeline step 7. This is a post-emission
fact, so it flows backward: emission computes it, then it is stored on the contract
for verification and cross-function reasoning.

### `compiler/ori_arc/src/aims/interprocedural.rs`

**C2. Compute `FipContract` from converged analysis state.**
In `extract_contract()`, after the intraprocedural analysis converges, derive `FipContract`:
```
if effects.may_allocate == false
   && effects.may_deallocate == false
   && !has_recursive_calls
   -> FipContract::Certified

if effects.alloc_only_on_slow_path == true
   && !has_non_tail_recursive_calls
   -> FipContract::Conditional { requires_unique_params: <params where uniqueness gates slow path> }

else -> FipContract::Never
```
This replaces the current `FipContract::Never` default in Stage 1.

### `compiler/ori_arc/src/aims/verify/mod.rs`

**C3. Add FIP certification verification.**
New check in `run_aims_verify()` (pipeline step 9a):
- If `contract.fip == FipContract::Certified`, verify:
  - `emit_reuse_result.missed_reuses == 0` (Theorem 2: no deallocation)
  - Per-arm token balance (DMATCH! invariant)
  - Function has no recursive calls (for Certified) or only tail calls (for Conditional)
- Error variant: `FipCertificationViolation { function, reason }`

### `compiler/ori_arc/src/aims/emit_reuse/fip.rs`

**C4. Enrich `FipGateRecord` with token balance information.**
Add fields to `FipGateRecord`:
```rust
pub struct FipGateRecord {
    pub source_var: ArcVarId,
    pub block: ArcBlockId,
    pub decision: FipGateDecision,
    pub token_arity: u32,           // NEW: constructor arity k (not byte size)
    pub consumed_by: Option<ArcVarId>,  // NEW: which Construct consumed this token
}
```
This enables the verifier to check arity-level balance, not just count-level balance.
Note: the paper uses arity k (field count), not byte size, for credit matching.

### `compiler/ori_arc/src/aims/lattice/dimensions.rs`

**C5. No changes to lattice dimensions.**
FP² does not require new lattice dimensions. The existing 7 dimensions are sufficient.
FIP certification is a function-level property derived from dimension values, not a new
dimension. This is a key architectural validation: FIP falls out of the existing lattice
as a view, confirming the AIMS thesis. The `FIP ⊂ FBIP ⊂ λ^fip` containment hierarchy
maps to lattice-derived classifications, not new dimensions.

---

## 02.6 Lens Shift

### For Paper 03 (FIPTree)

FP² establishes the baseline: FIP certification requires allocation balance + token linearity +
bounded frame. FIPTree (Lorenzen et al., PLDI 2024) extends this with **first-class constructor
contexts** -- holes in partially-constructed values that enable O(1) in-place algorithms for
data structures that FP² alone cannot certify (e.g., top-down tree traversals, appending to
the end of a list while building from the front).

Read FIPTree asking: "What does AIMS need beyond allocation balance to handle constructor
contexts?" The answer should map to `ShapeClass::ContextHole` and the Stage 3 TRMC
normalization.

FP²'s reuse credit system is *constructor-level* -- one credit per destructed value. FIPTree
introduces *field-level* contexts -- a hole at a specific field position within a partially
constructed value. AIMS's `ShapeClass` must distinguish between "whole constructor reusable"
(FP² token) and "field within constructor is a fillable hole" (FIPTree context). This is
already sketched in `ShapeClass::ContextHole` but needs FIPTree's formal semantics to be
implemented correctly.

### For Paper 04 (TRMC)

**Important correction:** FP² already contains TRMReC (Tail Recursion Modulo Reusable
Contexts) in Section 3, with Theorem 5 proving soundness. This is the *same* transformation
framework that TRMC (Leijen & Lorenzen, POPL 2023) establishes equationally. Paper 04
provides the equational *laws* (context laws, soundness criterion); FP² Section 3 provides
the *FIP-specific* realization of those laws (TRMReC = defunctionalized CPS contexts +
reuse credit accounting + side-condition for in-place guarantee).

Read TRMC asking: "What equational laws justify TRMReC, and what additional conditions
does the general framework expose that FP²'s side-condition might miss?" The pipeline is:
TRMC provides the algebraic foundation → TRMReC is the FIP-specialized application →
AIMS Stage 3 implements the transformation → the lattice certifies the result.

### For Paper 05 (Perceus for OCaml)

FP²'s benchmarks (Section 6) show that `std-reuse` (dynamic reuse without `fip`) is already
very effective — within 7% of `fip` on most benchmarks. The dynamic `is-unique` check has
~1% overhead (tmap benchmark). Read Perceus/OCaml asking: "Does the OCaml evaluation
confirm that dynamic reuse is sufficient in practice, making FIP certification primarily
a *proof* (for reasoning) rather than a *performance* tool?" FP²'s benchmarks suggest yes.

### Cumulative Lens After Paper 02

After OxCaml (Paper 01) and FP² (Paper 02), the cumulative lens for reading subsequent
papers is:

> AIMS dimensions serve two masters: (1) RC optimization (fewer inc/dec operations) and
> (2) FIP certification (proving zero allocation). These are not the same goal. RC
> optimization tolerates conservative approximations (a missed optimization is just slower
> code). FIP certification demands precision (a missed token balance is a certification
> failure). When reading subsequent papers, ask whether a proposed improvement helps
> RC optimization, FIP certification, or both. Improvements that help only RC optimization
> are Stage 1 refinements. Improvements that help FIP certification are Stage 2 requirements.

Additional lens from FP²'s formal structure:

> The `FIP ⊂ FBIP ⊂ λ^fip` containment means the question is not "does AIMS implement
> FIP?" but "which fragment does each function land in?" The lattice naturally classifies
> functions along this hierarchy. Functions where all parameters are `Unique` and all
> destructions are reused are in the FIP fragment. Functions with `MaybeShared` parameters
> that pass the `is-unique` check at runtime are in FBIP/λ^fip with the FIP fast path.
> This classification is a *derived property* of converged state — not a separate analysis.

---

## 02.7 Open Risk

**Risk 1: FIP certification is currently decoupled from reuse emission.**
FP² says FIP is a property of the program's token balance (Theorem 2). AIMS computes
tokens (reuse opportunities) during emission (step 7), but `FipContract` is set during
interprocedural analysis (step 2, before emission). This is a chicken-and-egg problem:
interprocedural analysis needs to know whether a function is FIP to set `FipContract`, but
FIP certification requires knowing whether all tokens are consumed, which is only known
after emission.

**Mitigation:** Two-phase FIP computation:
1. Interprocedural analysis computes *optimistic* FIP (based on effect state: no allocation,
   no sharing). This is `FipContract::Certified` as the starting point.
2. After emission, verify that token balance holds. If not, downgrade to `Conditional` or
   `Never`. Store the verified result back on the contract for downstream consumers.
This mirrors FP²'s own approach: the typing rules (Fig. 4) certify well-formedness
structurally, then the store semantics theorems (1, 2) prove the consequences. The
structure is checked first; the consequences follow.

**Risk 2: `ShapeClass` join loses token identity at control flow merges.**
The paper's DMATCH! rule places `◇_k` into each branch arm's *local* Γ. The linearity of
Γ ensures the credit is consumed within that arm — there is no need to "join" credits
across branches. AIMS's `ShapeClass::join` is a flat lattice: two different `ReusableCtor`
values join to `NonReusable`. This means at a control flow merge, token identity is lost.
A value that is `ReusableCtor(Struct)` on one branch and `ReusableCtor(EnumVariant)` on
another becomes `NonReusable` — even though each branch individually has a valid token.

**Mitigation:** FIP certification should track token balance *per branch arm*, not globally.
The paper's formal system handles this naturally through branch-local Γ. For AIMS, this
means: instead of checking global token count balance, check that each match arm individually
achieves token balance (destructions matched by constructions on that arm's dominated path).
The `ReusePlanner` already does dominator/post-dominator analysis per reuse opportunity.
For FIP, add a per-arm assertion: every `◇_k` produced in a match arm is consumed in that
arm's control flow before the merge point.

**Risk 3: Tail-call recursion and FIP.**
FP² distinguishes FIP (Theorem 2, no recursion constraint on heap) from FIP^S (Theorem 4,
tail-recursive for bounded stack). AIMS's `FipContract` currently has no notion of recursion
style. `FipContract::Certified` should imply bounded stack (FIP^S), which requires:
- Non-recursive functions: always fine
- Self-recursive: must be tail-recursive (stack frames reused)
- Mutually recursive SCC: all calls within the group must be in tail position
AIMS should use SCC analysis (`graph/scc/mod.rs`) to determine recursion membership and
`tail_call` detection (pipeline step 10) to classify tail vs. non-tail.

**Risk 4: `FipContract` join semantics may be too aggressive.**
`FipContract::join` currently: `Certified.join(Conditional) = Conditional`. But in SCC
fixed-point iteration, a function that calls itself (recursion) should NOT start as
`Certified` -- it should start as `Never` (or at least `Conditional`). The current
`all_borrowed()` constructor takes `fip_initial` as a parameter, defaulting to `Never`
in Stage 1. When Stage 2 activates FIP inference, the initial value for recursive functions
must be `Never` (not `Certified`), because non-tail recursion blocks `Certified` by
FIP^S's definition. Only non-recursive functions should start as `Certified`.

**Risk 5: No formal connection between `EffectClass` (per-variable dimension) and
`EffectSummary` (per-function contract).**
`EffectClass` is a per-variable, per-program-point lattice dimension tracking whether
accessing that variable causes effects. `EffectSummary` is a per-function summary. The
aggregation from `EffectClass` values to `EffectSummary` is described in Section 09.2 as
"forward accumulation even in a backward pass" but is not yet implemented. Until this
aggregation is precise, `EffectSummary` uses conservative defaults (`CONSERVATIVE`),
making FIP certification impossible. This is the critical-path blocker for FIP: without
precise effect accumulation, `FipContract` remains `Never` for all functions.

**Risk 6: Atom handling in AIMS.**
FP² relies heavily on atoms (zero-field constructors) being allocation-free. The ATOM rule
(`Δ | ∅ ⊢ C`) produces no credit and consumes no credit. If AIMS treats atom constructors
the same as non-atom constructors in `ShapeClass`, it will produce phantom reuse credits
from atom destructions and require phantom credits for atom constructions, falsely
unbalancing the token accounting. AIMS's `ShapeClass` must recognize atoms as a distinct
case that neither produces nor consumes credits.
