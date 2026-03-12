---
section: "04"
title: "TRMC — Tail Recursion Modulo Context: An Equational Approach"
status: complete
goal: "Determine whether AIMS context laws are explicit enough and whether soundness criteria are defined (not just profitability)"
paper:
  title: "Tail Recursion Modulo Context: An Equational Approach"
  doi: "https://doi.org/10.1017/S0956796825100117"
  venue: "JFP 2025"
  authors: "Leijen & Lorenzen"
depends_on: ["01", "02", "03"]
sections:
  - id: "04.1"
    title: "Paper Thesis"
    status: complete
  - id: "04.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "04.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "04.4"
    title: "Plan Edits"
    status: complete
  - id: "04.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "04.6"
    title: "Lens Shift"
    status: complete
  - id: "04.7"
    title: "Open Risk"
    status: complete
---

# Section 04: TRMC — Tail Recursion Modulo Context: An Equational Approach

**Status:** Complete
**Goal:** Determine whether AIMS context laws are explicit enough, whether the plan
defines when TRMC is *sound* (not just when it seems profitable), and whether AIMS
should adopt a "law before optimization" rule for all opportunity-creation rewrites.

**Paper:** Leijen & Lorenzen, "Tail Recursion Modulo Context: An Equational Approach,"
JFP 2025. [DOI: 10.1017/S0956796825100117](https://doi.org/10.1017/S0956796825100117)
MSR-TR-2022-18 (v1, July 2022). Full text: 27 pages + 12 pages appendices (proofs,
benchmarks, typing rules).

**Why read this fourth:** This is the strongest *methodology* paper in the set. It is
about how to *calculate* a transformation from laws, not just how to implement a pass.
The equational approach provides soundness proofs grounded in Perceus heap semantics.

**Pause questions:**
- Are your context laws explicit enough?
- Does the plan define when TRMC is sound, or only when it seems profitable?
- Should AIMS have a "law before optimization" rule for all opportunity-creation rewrites?

**AIMS context:**
- Stage 3 (Opportunity Creation) includes TRMC normalization
- `aims/normalize/trmc.rs` is planned (self-recursive constructor-context rewrites)
- Scope bounds defined: single self-recursive call, under constructor/field context, no effects
- Currently a post-Stage-2 enhancement, not core architecture

---

## 04.1 Paper Thesis

This is a **methodology** paper, not merely an optimization paper. The central claim is:

> TRMC transformations can be **calculated** from their specification using equational
> reasoning, producing an algorithm that is *parameterized by abstract context
> operations* and *correct by construction* as long as two context laws hold.

The paper generalizes tail-recursion modulo *cons* (TRMc) to modulo *contexts* (TRMC)
by abstracting over the concrete context implementation. The resulting generic TRMC
algorithm has only 4 equations (Figure 2 in the paper: `base`, `tail`, `tlet`, `tmatch`)
and is parameterized by three operations (`ctx`, `comp`/bullet, `app`) plus one
applicability condition (star). Any instantiation that satisfies the two context laws
is a correct TRMC implementation.

**The two context laws** (Section 3.1):

1. **(appctx)** `app (ctx E) e = E[e]` — applying a context to an expression is the
   same as filling the hole in the evaluation context.

2. **(appcomp)** `app (k1 bullet k2) e = app k1 (app k2 e)` — applying a composed
   context is the same as sequentially applying the inner then the outer.

These are not implementation details; they are the *soundness contract*. Any
instantiation must prove these two laws hold for terminating expressions. The
paper then shows how to **derive** (not design) efficient implementations from
these laws.

**Key insight for AIMS:** The paper demonstrates that the correctness of TRMC does
not come from ad-hoc pattern matching on recursive call positions. It comes from
proving that context operations satisfy algebraic laws. The optimization is a
*consequence* of the algebra, not the other way around.

**Five instantiations proven correct:**

1. **Evaluation contexts** (Section 4.1): `ctx E = lambda x. E[x]` — CPS translation.
   Most general but allocates closures. Subsumes classical CPS.

2. **Defunctionalized evaluation contexts** (Section 4.2): Finite set of accumulator
   constructors `A_i` representing each possible E shape. The accumulator is a
   zipper structure. Enables reuse: the `A_i` node used to accumulate is reused
   for the `Cons` cell in the result (Section 4.2.1).

3. **Associative operator contexts** (Section 4.3): For any monoid `(tau, odot, unit)`,
   context is just an element of `tau`. Fold reduces context to a value. Derives
   textbook accumulator versions (e.g., `length` with `+`, `reverse` with `++`).

4. **Monoid contexts** (Section 4.4): Handles non-commutative monoids with
   left-and-right accumulation as `(l, r)` pairs.

5. **Semiring contexts** (Section 4.5): Two monoids where one distributes over the
   other. Derives hash-function accumulator.

6. **Exponent contexts** (Section 4.6): Repeated function application counted as an
   integer. Derives McCarthy's 91-function optimization.

**The modulo *cons* instantiation** (Section 5) is the most important for compilers.
It uses Minamide's hole calculus for functional correctness (Theorem 1: TRMC uses
contexts linearly, proven via linear type discipline) and Perceus heap semantics
for in-place update soundness (Section 5.2).

**Perceus heap semantics role** (Section 5.2):

The paper uses the explicit heap semantics of Perceus (Reinking et al., PLDI 2021)
to reason about *when in-place mutation is safe*. The key derivation:

- When an object `x` is **unique** (refcount = 1), `x.i as y` (field update) reduces
  to: alpha-rename `x` to reuse the original address, then drop the old field value
  and store the new one. This is the *(assign)* rule.
- A constructor context always evaluates to a **unique linear chain** (Lemma 2):
  all objects along the path from the top of the context to the hole are unique
  and not reachable from elsewhere.
- Therefore context composition `(ucomp)` and application `(uapp)` can be
  performed in-place: update the hole pointer, no copying needed.

The full in-place update rules (Section 5.2.6):

- **(uapp)** `H | app <x, y@i> z  -->_r  H | subst <x, y@i> z`
  (application = in-place substitution at the hole)
- **(ucomp)** `H | <x1, y1@i> bullet <x2, y2@j>  -->_r  H | <app <x1, y1@i> x2, y2@j>`
  (composition = update the hole in the first context to point to the second)

**Non-linear control flow** (Section 5.3): The efficient in-place implementation
**breaks** when non-linear control operations (call/cc, shift/reset, algebraic effect
handlers) can resume a continuation more than once, because the "linear" context `k`
may be captured in a lambda and used multiple times. The paper identifies this
precisely: the `(handle)` rule captures `k` in a non-linear lambda via the
`resume` operation.

**The hybrid approach** (Section 5.4): Instead of choosing between safe-but-slow CPS
and fast-but-fragile in-place, the paper proposes:

1. Track **context paths** at runtime using an 8-bit field index in each object header.
2. When composing contexts, check uniqueness of the dominator.
3. If unique: fast in-place update (normal path). If shared (due to non-linear
   control): fall back to copying the context chain via `append`.
4. Static type information can eliminate the runtime check when the function is
   guaranteed to use only linear effects.

This hybrid approach is calculated from the context laws extended with `(subapp)`
for non-unique substitution. The context laws are proven to still hold (Appendix C.10).

**Benchmarks** (Section 6): On map, tmap, rbtree, knapsack benchmarks (100M elements),
TRMC is always as fast or faster than manual accumulator alternatives for linear
control. For the knapsack benchmark (non-linear via backtracking effects), the hybrid
approach is about 25% slower than the accumulator version in the worst case, because
the context is copied at each backtracking choice point. Still, Koka prefers hybrid
to avoid code duplication.

---

## 04.2 What AIMS Should Adopt

### Keep

**K1. The two context laws as explicit proof obligations.**
AIMS Stage 3 (`aims/normalize/trmc.rs`) must document and verify the two context
laws for its instantiation. Currently, the plan says "self-recursive constructor contexts"
but does not name the laws or require proving them. The laws are:
- `app (ctx K) e = K[e]` (applying context = filling hole)
- `app (k1 bullet k2) e = app k1 (app k2 e)` (composition distributes over application)

Any AIMS TRMC rewrite that does not satisfy these two laws for the target context type
is unsound. The plan must make this explicit.

**K2. The "law before optimization" principle.**
The paper's methodology is: (a) define a specification, (b) calculate the algorithm
from the specification using equational laws, (c) instantiate with specific context
types. AIMS should adopt this as a general rule for all Stage 3 opportunity-creation
rewrites:

> Every rewrite in `aims/normalize/` must have (1) a specification that defines
> correctness, (2) a set of laws that the specification satisfies, and (3) a proof
> (or test-based demonstration) that the concrete instantiation satisfies those laws.

This prevents accumulating ad-hoc pattern-matching rewrites that happen to work on
known examples but lack a soundness argument.

**K3. Uniqueness as precondition for in-place TRMC.**
The paper proves that in-place context update requires the linear chain property
(Lemma 2): all nodes from top to hole are unique and unreachable from elsewhere.
This maps directly to AIMS:

- `Uniqueness::Unique` on the context variable is a **precondition** for in-place TRMC.
- If uniqueness cannot be proven statically, AIMS must either emit the hybrid
  path (runtime check) or fall back to the CPS translation.
- This is NOT just a profitability decision — it is a correctness decision.

**K4. The lifting transformation as a separate pre-pass.**
The paper separates TRMC from a **lifting** transformation (Section 5.6) that
extracts expressions from constructor fields into let-bindings before TRMC matching.
This is important because:
- Without lifting, `Cons(f(x), map(xx, f))` is not a K context (because `f(x)` is
  an expression in a field position).
- With lifting, it becomes `let y = f(x) in Cons(y, map(xx, f))` which exposes
  the constructor context `Cons(y, [])`.
- AIMS should similarly define lifting as a pre-pass to TRMC detection, not
  interleave them.

**K5. The hybrid path as architecture, not afterthought.**
The paper shows that non-linear control (which maps to Ori's effect handlers and
`with...in` blocks) breaks in-place TRMC. The hybrid approach is not optional for
correctness in the presence of effects. AIMS must plan for this from the start:

- `EffectClass::may_share` (or the function-level `EffectSummary`) determines
  whether the hybrid path is needed.
- If `may_share == false` (pure function or no handler resumption), the fast
  in-place path is sound.
- If `may_share == true`, AIMS must emit either the hybrid check or fall back.

**K6. Defunctionalized contexts as the reuse sweet spot.**
The paper shows (Section 4.2.1) that defunctionalized evaluation contexts naturally
create reuse opportunities: the accumulator constructor `A_i` has the same arity as
the result constructor, so Perceus reuse analysis can reuse it in-place. This means
AIMS's `ShapeClass::ContextHole` should track not just "there is a hole" but also
the shape of the accumulator for reuse matching.

### New Invariants

**I1. TRMC soundness = context laws + uniqueness, not pattern matching.**
The plan currently defines TRMC eligibility by structural patterns (scope bounds in
`00-overview.md` Stage 3): self-recursive, one recursive call, under constructor
context, no effects, no polymorphic layouts. These are necessary conditions but the
plan does not state the *sufficient* condition: the context operations satisfy
`(appctx)` and `(appcomp)` for the chosen instantiation. Add this.

**I2. "No effectful instructions between context capture and fill" is necessary but
not sufficient.** The paper shows the precise failure mode: it is not about effects
*between* capture and fill, but about the context variable `k` being captured in a
non-linear lambda (via effect handler resumption). AIMS must check:
- The context variable is used linearly (exactly once on each path).
- No handler between context creation and application can resume more than once.

**I3. Constructor context = unique linear chain.** Every AIMS TRMC rewrite must
maintain the invariant that the context evaluates to a unique linear chain (Lemma 2
in the paper). This means:
- All objects on the path from `res` to `hole` have refcount 1.
- No object on the path is reachable from outside the chain.
- The `hole` field must be a pointer that can be updated in-place.

This is verifiable at the AIMS level: the context variable must have
`Uniqueness::Unique` at every point between creation and application.

**I4. Lifting must precede TRMC detection.** AIMS Stage 3 must include a lifting
sub-pass that normalizes expressions in constructor field positions into let-bindings
before scanning for TRMC candidates. Without lifting, functions like `map` where
`f(x)` appears as a sibling field to the recursive call will not be detected.

---

## 04.3 What AIMS Should Not Adopt

### Reject

**R1. The CPS/evaluation-context instantiation (Section 4.1).**
This instantiation allocates closures for every context — exactly the allocation
overhead AIMS exists to eliminate. It is useful as a theoretical baseline and for
languages like Scheme where closures are cheap, but is inappropriate for Ori where
the goal is zero-overhead RC-managed memory. AIMS should only implement the
modulo-cons instantiation with in-place update.

**R2. General monoid/semiring/exponent instantiations as compiler transforms.**
The associative, monoid, semiring, and exponent instantiations (Sections 4.3-4.6)
derive elegant accumulator versions but they require *recognizing algebraic structure*
in user code (e.g., "this is a monoid with these operations"). This is a program
analysis problem beyond AIMS's scope — it requires semantic knowledge about
user-defined operations. These instantiations are better served by library-level
annotations or manual programmer optimization.

Exception: if Ori adds a `#[trmc_associative]` annotation in the future, the
associative instantiation could be implemented. But this is not Stage 3 scope.

**R3. The Koka-specific effect monadic compilation pipeline.**
The paper describes how Koka translates effect handlers through a multi-prompt
control monad `eff` before TRMC (Section 5.3). This is deeply tied to Koka's
compilation of algebraic effects to lambda calculus with `Pure`/`Yield` cases. Ori's
capability system (`uses`/`with...in`) has a different compilation strategy. AIMS
should not adopt Koka's specific monadic desugaring — instead, use `EffectClass`
to determine when the fast path is available.

**R4. Runtime context-path tracking via object header bits.**
The hybrid approach uses an 8-bit field index in each heap object's header to track
the "next link" in the context chain (Section 5.4). Ori's `ori_rt` runtime does not
have a spare 8-bit field in object headers (the header is `{ refcount: i64 }`). Adding
one would require a runtime layout change with broad consequences. For Stage 3,
AIMS should use static analysis (uniqueness + effect purity) to determine the fast
path, and fall back to CPS (not hybrid) when static analysis is insufficient. The
hybrid approach can be revisited if runtime header space becomes available.

**R5. Multi-hole product contexts (Appendix A.1).**
The paper extends TRMC to functions returning tuples (e.g., `partition` returning
`(list, list)`) via multi-hole tail contexts `T`. This is an elegant generalization but
significantly increases the complexity of the normalize pass. AIMS Stage 3 should
start with single-hole constructor contexts only.

---

## 04.4 Plan Edits

**P1. `plans/aims/00-overview.md` Stage 3 scope bounds — add proof obligations.**

Current scope bounds (lines 395-401) list structural conditions. Add after line 401:

> **Proof obligations (from Leijen & Lorenzen, JFP 2025):**
> - The chosen context instantiation must satisfy the two context laws
>   `(appctx)` and `(appcomp)` for terminating expressions.
> - The context variable must be provably unique (AIMS `Uniqueness::Unique`)
>   at every point between context creation (`ctx`) and application (`app`).
> - If the function's `EffectSummary.may_share == true`, in-place TRMC is
>   unsound; fall back to non-in-place translation or skip TRMC.
> - A lifting sub-pass must run before TRMC detection to normalize
>   expressions in constructor fields into let-bindings.
<!-- reviewed: completeness fix — Cross-dependency: Section 03 (FIPTree) also proposes
reframing Stage 3 in 00-overview.md. Both edits target the same text block. P1 adds proof
obligations; Section 03 reframes the deliverable description. Both are compatible and
should be applied together. The existing Stage 3 scope bounds (lines 395-401) already
include "No effectful instructions between context capture and fill" which partially
overlaps with the may_share check proposed here. The proof obligations add formal rigor
beyond the existing structural conditions. -->

**P2. `plans/aims/00-overview.md` module tree — expand `normalize/`.**

The current module tree (lines 457-461) shows: <!-- reviewed: accuracy fix, was lines 457-460 -->
```
normalize/
  mod.rs          -- normalize_function() entry point
  trmc.rs         -- TRMC-eligible recursion detection + rewrite
  context.rs      -- constructor-context metadata extraction
  collections.rs  -- collection mutation canonicalization
```

Expand to reflect the paper's decomposition:
```
normalize/
  mod.rs          -- normalize_function() entry point
  lift.rs         -- lifting: extract expressions from ctor fields to let-bindings
  trmc.rs         -- TRMC detection: identify eligible recursive calls under K contexts
  rewrite.rs      -- TRMC rewrite: apply the 4-equation algorithm (base/tail/tlet/tmatch)
  context.rs      -- constructor-context representation (Minamide tuple: res + hole ptr)
  verify.rs       -- verify context laws hold for each rewrite site
  collections.rs  -- collection mutation canonicalization (unchanged)
```

The key addition is separating detection from rewriting and adding a verification step.
<!-- reviewed: completeness fix — Cross-dependency: Section 03 (FIPTree) proposes a
context/ subdirectory expansion (context/mod.rs, detect.rs, validate.rs, multi.rs).
These two proposals must be reconciled. Section 04's top-level structure (lift, trmc,
rewrite, context, verify) is the pipeline decomposition. Section 03's context/ is a
deeper decomposition of the context module. Both are needed. See reconciliation note in
Section 03.4. -->

**P3. `plans/aims/section-09-dimensional-fusion.md` Section 09.2 Shape Activation —
strengthen ContextHole.**

Current (lines 399-402):
> `ContextHole` shape means the value has a hole to be filled by a recursive
> call. When shape analysis identifies `ContextHole + FunctionLocal`, the
> function is a TRMC candidate.

Strengthen to:
> `ContextHole` shape means the value has a hole to be filled by a recursive
> call. TRMC candidacy requires `ContextHole + FunctionLocal + Unique +
> (EffectClass::may_share == false OR hybrid path available)`. The uniqueness
> requirement is a **soundness** condition (Lemma 2, Leijen & Lorenzen JFP 2025),
> not merely a profitability hint. The effect purity check guards against
> non-linear control flow that would break the linear chain property.
<!-- reviewed: completeness fix — Cross-dependency: Section 03 (FIPTree) proposes
enriching ContextHole with ContextMeta (hole position, depth, accumulator count) at
the same location in 09.2. These two edits are complementary: Section 03 adds structural
metadata, Section 04 adds soundness conditions. Both should be applied together. The
combined requirement becomes: ContextHole(ContextMeta) + FunctionLocal + Unique +
may_share==false. -->

**P4. `plans/aims/section-09-dimensional-fusion.md` Section 09.2 Effect Activation —
add TRMC interaction.**

After the "Effect -> FIP natural detection" item (line 363), add:

> **Effect -> TRMC soundness gate.**
> `may_share == false` is a PRECONDITION for in-place TRMC, not just a
> profitability signal. When `may_share == true`, the context variable `k` may
> be captured by an effect handler's resumption and used non-linearly, breaking
> the unique linear chain invariant. AIMS must gate in-place TRMC behind this
> check. Stage 3 `normalize/verify.rs` must query `EffectSummary` to determine
> the path.

**P5. Add "law before optimization" principle to `00-overview.md` Design Principles.**

After "3. Formally Grounded" (line 225), add a new design principle:

> ### 4. Law Before Optimization
>
> Every rewrite in `aims/normalize/` (Stage 3 opportunity creation) must follow
> the equational approach: (a) define a correctness specification, (b) identify
> the algebraic laws the specification requires, (c) prove the concrete
> instantiation satisfies those laws. This principle, drawn from Leijen &
> Lorenzen (JFP 2025), prevents accumulating ad-hoc rewrites that work on
> known examples but lack soundness arguments. The AIMS litmus test (above)
> covers analysis dimensions; this principle covers pre-analysis rewrites.
<!-- reviewed: completeness fix — This is a genuinely new design principle not present
in the existing AIMS plan. The current three design principles (Analysis/Emission
Separate, One Lattice One Truth, Formally Grounded) cover analysis. This fourth
principle covers pre-analysis rewrites. Good addition. Note: this principle applies
only to Stage 3+ (normalize/ module), not to Stage 1-2. The AIMS plan should clarify
that this principle is forward-looking and does not retroactively apply to existing
Stage 1 code. -->

---

## 04.5 Code Changes (Later)

**C1. `compiler/ori_arc/src/aims/contract/mod.rs` — ContextBehavior (lines 322-343).**

The current `ContextBehavior` has two boolean fields (`preserves_context`,
`consumes_hole`). After this review, it should additionally track:
- `requires_unique_context: bool` — whether the function requires the context to be
  unique for correctness (always true for in-place TRMC, false for CPS fallback).
- `may_resume_nonlinearly: bool` — whether effect handlers in scope can resume more
  than once (determines hybrid vs fast path).

These should be derived from `EffectSummary` during contract extraction, not set
independently.

**C2. `compiler/ori_arc/src/aims/contract/mod.rs` — ContextRegion (lines 397-405).**

The current `ContextRegion` is a placeholder with `_private: ()`. It should carry:
- `context_var: ArcVarId` — the variable holding the context (Minamide tuple).
- `hole_field: usize` — which field of the constructor is the hole.
- `recursive_callee: Name` — which function is being called recursively.
- `context_shape: ShapeClass` — shape of the accumulator constructor (for reuse matching).
- `requires_lifting: bool` — whether the lifting sub-pass had to extract expressions.

**C3. `compiler/ori_arc/src/aims/lattice/dimensions.rs` — ShapeClass::ContextHole.**

Currently `ContextHole` is a bare variant (line 181). It should carry metadata:
```rust
ContextHole {
    /// The constructor kind forming the context.
    ctor_kind: ReuseCtorKind,
    /// Which field is the hole (index into constructor fields).
    hole_field: usize,
}
```
This enables reuse matching: the defunctionalized accumulator has the same shape
as the result constructor, so AIMS can match them for in-place reuse (Paper
Section 4.2.1).

Note: this changes `ShapeClass` from `Copy` to potentially non-`Copy` depending on
field sizes. Since `ReuseCtorKind` is `Copy` and `usize` is `Copy`, the struct
remains `Copy`. The `join` implementation needs updating: two `ContextHole` values
join to `ContextHole` only if `ctor_kind` and `hole_field` match.

**C4. `aims/normalize/lift.rs` — new file.**

Implement the lifting transformation from Section 5.6 of the paper:
- Walk each function body looking for constructors in tail position.
- For each constructor field that is an expression (not a variable or value),
  extract it into a let-binding immediately before the constructor.
- This exposes the pure constructor context `K` for TRMC detection.

The lifting is simple and can be tested independently of TRMC.

**C5. `aims/normalize/verify.rs` — new file.**

For each TRMC rewrite site, verify:
1. The context variable is used linearly (exactly once per control-flow path).
2. `Uniqueness::Unique` holds for the context variable at every use.
3. `EffectSummary.may_share == false` for the enclosing function (or hybrid
   path is emitted).
4. No polymorphic constructor with unknown layout is used as context.

These checks should be run after the rewrite and should be `debug_assert!` in
release builds (the detection pass should not produce invalid rewrites).

**C6. `aims/normalize/trmc.rs` — the 4-equation algorithm.**

Implement Figure 2 from the paper as the core rewrite:
- **(base)** `[[e]]_{f,k} = app k e` — non-tail expression: apply context.
- **(tail)** `[[E[f e1 ... en]]]_{f,k} = f_hat e1 ... en (k bullet (ctx E))` iff (star)
  — tail call under context E: compose context and recurse.
- **(tlet)** `[[let x = e' in e]]_{f,k} = let x = e' in [[e]]_{f,k}` — let: recurse
  into body.
- **(tmatch)** `[[match e' { pi -> ei }]]_{f,k} = match e' { pi -> [[ei]]_{f,k} }` —
  match: recurse into each arm.

The (star) condition gates which evaluation contexts E are valid constructor contexts.
For AIMS, (star) is: E is a K context (constructor with variable/value fields and
one hole at a recursive call position).

---

## 04.6 Lens Shift

Reading Paper 05 (Perceus for OCaml, evaluation methodology):

1. **The Perceus heap semantics is not just about RC insertion.** This paper shows
   that the explicit heap with reference counts is a *proof framework* for calculating
   in-place update rules. When reviewing Paper 05, look for whether the evaluation
   methodology accounts for this — measuring RC operations alone misses the TRMC
   dimension (where RC operations are *avoided* by proving uniqueness statically,
   not by eliminating redundant pairs).

2. **Same-compiler-different-backend evaluation is necessary but not sufficient.**
   TRMC introduces a structural rewrite that changes the *shape* of execution (from
   stack-consuming recursion to tail-recursive accumulation). Comparing RC counts
   before/after TRMC would show fewer operations, but the real win is stack space
   elimination. Paper 05's evaluation methodology should account for memory
   footprint (stack + heap), not just throughput.

3. **Non-linear control flow is a cross-cutting concern.** Paper 05 evaluates Perceus
   on OCaml which has GC-based memory management. The TRMC hybrid approach is
   specific to precise reference counting. When comparing evaluation methodologies,
   note that OCaml's GC does not have the non-linear control problem (shared
   contexts just work because GC handles them). AIMS's evaluation must separately
   measure the hybrid path cost.

4. **Reuse and TRMC reinforce each other.** The defunctionalized context approach
   creates reuse opportunities (Section 4.2.1). When evaluating AIMS's reuse
   emission, TRMC-eligible functions should show *more* reuse opportunities than
   non-TRMC versions of the same code. This is a measurable synergy between
   Stage 3 (opportunity creation) and Stage 1D (reuse cutover).

---

## 04.7 Open Risk

**Risk 1: AIMS Stage 3 defines soundness by structural pattern, not by algebraic law.**

The current scope bounds (`00-overview.md` lines 395-401) are:
- Self-recursive functions only (no mutual recursion)
- One recursive call per transformed region
- Recursive call beneath a constructor or field context
- No effectful instructions between context capture and fill
- No polymorphic unknown-layout contexts
- Source spans and debugability preserved

These are *necessary conditions* for the modulo-cons instantiation, but they are not
stated as consequences of the context laws. The risk is that someone relaxes a
condition (e.g., allowing two recursive calls) without checking whether the context
laws still hold. **Mitigation:** P1 above (add proof obligations). Also, `verify.rs`
(C5) should independently check the laws for each rewrite site.

**Risk 2: The effect check is too coarse.**

AIMS plans to gate TRMC on `EffectClass.may_share == false`. But the paper shows
the precise failure mode is more specific: it is the *resumption* of an effect handler
that uses the continuation more than once. A function that uses effects but whose
handlers all resume at most once (one-shot continuations) is still safe for in-place
TRMC. AIMS's current `EffectClass` does not distinguish one-shot from multi-shot
resumption.

**Short-term mitigation:** Conservative — gate on `may_share == false`. This is sound
but rejects some safe cases.

**Long-term mitigation:** Add a `may_resume_multiple: bool` field to `EffectSummary`
(or a `ResumeCardinality` enum: `AtMostOnce | Unrestricted`). This requires tracking
handler resumption cardinality through the capability system, which is significant
work. Defer to a later stage.

**Risk 3: The plan does not define what "fall back" means when TRMC is unsound.**

When in-place TRMC cannot be applied (non-linear effects, non-unique context, mutual
recursion), what does AIMS do? Options:
- **Skip:** Leave the function unmodified. This is safe but loses the opportunity.
- **CPS fallback:** Apply the evaluation-context instantiation (Section 4.1). This
  preserves tail recursion but allocates closures.
- **Hybrid:** Runtime uniqueness check (Section 5.4). Requires runtime support.

The plan currently says "no-op in Stage 1" and "self-recursive constructor-context
rewrites only" for Stage 3. It does not specify the fallback strategy. This should
be decided before implementation.

**Recommendation:** Skip (leave unmodified) as the Stage 3 default. The CPS fallback
is complex and the hybrid approach requires runtime changes. Skipping is safe —
the function still works correctly, just without the TRMC optimization. Document
this as an explicit choice, not an oversight.

**Risk 4: Stage 3 may be too late in the pipeline.**

The paper shows that TRMC + reuse together achieve FIP-level performance for tree
algorithms (Section 4.2.1: the accumulator constructor is reused for the result
constructor). If TRMC normalization runs after Stage 2 (dimensional fusion + unified
realization), the reuse analysis has already converged without seeing TRMC
opportunities. AIMS would need to re-run analysis after normalization.

The current architecture (Phase A: opportunity creation, Phase B: analysis,
Phase C: realization) already handles this correctly — TRMC runs in Phase A
*before* analysis. The risk is that someone reorders these phases. The plan should
state this ordering as a hard constraint, not just a current implementation choice.

**Risk 5: `ContextRegion` is a placeholder that will accumulate ad-hoc fields.**

The current `ContextRegion` (contract/mod.rs line 402) is `{ _private: () }`. When
Stage 3 is implemented, there is a risk of growing this struct incrementally without
designing the representation up front. C2 above proposes the initial fields. The
design should be finalized before implementation begins, informed by the paper's
Minamide tuple representation `<x, y@i>` (pointer to context, address of hole as
i-th field of object y).
