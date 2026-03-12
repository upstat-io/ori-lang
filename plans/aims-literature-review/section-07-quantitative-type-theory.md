---
section: "07"
title: "The Syntax and Semantics of Quantitative Type Theory"
status: complete
goal: "Determine whether Cardinality is rich enough and whether usage facts are treated algebraically and compositionally"
paper:
  title: "The Syntax and Semantics of Quantitative Type Theory"
  doi: "https://doi.org/10.1145/3209108.3209189"
  venue: "LICS 2018"
  authors: "Atkey"
depends_on: ["01", "02", "03", "04", "05", "06"]
sections:
  - id: "07.1"
    title: "Paper Thesis"
    status: complete
  - id: "07.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "07.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "07.4"
    title: "Plan Edits"
    status: complete
  - id: "07.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "07.6"
    title: "Lens Shift"
    status: complete
  - id: "07.7"
    title: "Open Risk"
    status: complete
---

# Section 07: The Syntax and Semantics of Quantitative Type Theory

**Status:** Complete
**Goal:** Determine whether AIMS `Cardinality` is rich enough, whether usage facts are
treated algebraically and compositionally (semiring-like), and whether AIMS needs stronger
language around resource semirings in the plan documentation.

**Paper:** Atkey, "The Syntax and Semantics of Quantitative Type Theory," LICS 2018.
[DOI: 10.1145/3209108.3209189](https://doi.org/10.1145/3209108.3209189)

**Why read this seventh:** This is not a direct blueprint, but it can tighten the semantics
of demand/cardinality and the way you talk about resource usage. QTT's semiring framework
provides a principled foundation for composing usage annotations.

**Pause questions:**
- Is `Cardinality` rich enough? (Currently just Absent/Once/Many)
- Are you treating usage facts algebraically and compositionally enough?
- Does AIMS need stronger language around resource semirings in the plan?

**AIMS context:**
- `Cardinality`: Absent < Once < Many
- `seq_add`: sequential composition of cardinality (Once + Once = Many)
- `alt_join`: alternative composition (branch join)
- These operations are defined but their algebraic properties aren't documented as semiring laws
- Transfer functions compose cardinality updates implicitly

---

## 07.1 Paper Thesis

QTT parameterizes dependent type theory by a **semiring of usage annotations**. Every
variable in a typing judgement carries an element of a semiring R that records how the
variable is used. The two semiring operations have precise compositional meanings:

- **Addition (+)**: combining the usage of a variable from two sub-computations that are
  *both* active. In the App rule, the function M needs the variable with usage from
  context Gamma_1, and the argument N needs it with usage from Gamma_2. The combined
  context is Gamma_1 + pi * Gamma_2, where pi is the function's declared usage of its
  parameter. This is "I use x here, and I also use x there."

- **Multiplication (*)**: scaling usage through nesting. When a function with parameter
  usage pi is applied to an argument that itself uses variable x with usage rho, the
  total usage of x is pi * rho. This is "I use the argument pi times, and within the
  argument, x is used rho times."

The paper's canonical example is the **0-1-omega semiring** {0, 1, omega}:

| +     | 0 | 1     | omega |
|-------|---|-------|-------|
| 0     | 0 | 1     | omega |
| 1     | 1 | omega | omega |
| omega | omega | omega | omega |

| *     | 0 | 1 | omega |
|-------|---|---|-------|
| 0     | 0 | 0 | 0     |
| 1     | 0 | 1 | omega |
| omega | 0 | omega | omega |

Key properties:
- 0 is the additive identity and multiplicative annihilator (0 * rho = 0)
- 1 is the multiplicative identity
- omega + anything-nonzero = omega (saturation)
- omega * omega = omega

The 0 element has a dual role that resolves the dependency/linearity conflict: a variable
with usage 0 has no runtime presence but is still available for type formation. This is
how QTT threads the needle between erasure and linearity.

**Substitution lemma (Lemma 2.5):** The critical property. When substituting a term N
(with context Gamma_2) for variable x (with usage rho) into a term M (with context
Gamma_1), the resulting context is Gamma_1 + rho * Gamma_2. This is only admissible
when the semiring is positive (rho + pi = 0 implies rho = pi = 0) and has the
zero-product property (rho * pi = 0 implies rho = 0 or pi = 0). The original McBride
system had a bug where allowing arbitrary usage annotations on the output (not just 0 or 1)
made substitution inadmissible.

The paper then constructs Quantitative Categories with Families (QCwFs) as the categorical
semantics, with realisability models over R-Linear Combinatory Algebras that concretely
track resource consumption.

---

## 07.2 What AIMS Should Adopt

### Keep

**1. The semiring perspective on Cardinality composition is already present and correct.**

AIMS Cardinality {Absent, Once, Many} maps directly to QTT's {0, 1, omega}:

| AIMS      | QTT   | Meaning                    |
|-----------|-------|----------------------------|
| Absent    | 0     | Not used (dead)            |
| Once      | 1     | Used exactly once (linear) |
| Many      | omega | Used multiple times         |

The two operations map as follows:

| QTT operation      | QTT meaning                          | AIMS operation | AIMS meaning                                |
|--------------------|--------------------------------------|---------------|---------------------------------------------|
| Addition (+)       | Combining usages from sub-terms      | `seq_add`     | Sequential composition along one path       |
| Max / join         | Combining usages from branches       | `alt_join`    | Control-flow merge (only one branch runs)   |

This is the correct decomposition. QTT uses addition for "both sub-computations are
active" which, in a backward dataflow analysis, is exactly the sequential case: walking
backward through a block, each instruction that demands a variable adds demand via
`seq_add`. QTT's branching (ElimBool) uses the "additive" style from linear logic where
both branches share the same context -- this is AIMS's `alt_join` (max/lub) at control-flow
merge points.

**The existing tests already verify the key algebraic laws** (in
`compiler/ori_arc/src/aims/lattice/tests.rs`):

- `seq_add` associativity (lines 236-243)
- `seq_add` commutativity (lines 251-257)
- `seq_add` identity: Absent is two-sided identity (lines 264-267)
- `seq_add` absorbing: Many absorbs everything (lines 272-275)
- `alt_join` idempotence (lines 280-282)
- `alt_join` associativity (lines 287-294)
- Distributivity: `a.seq_add(b.alt_join(c)) == a.seq_add(b).alt_join(a.seq_add(c))` (lines 302-313)

This distributivity test is exactly the semiring law -- `seq_add` distributes over
`alt_join`. The tests are exhaustive (all 27 combinations for the 3-element set).

**2. The backward analysis correctly uses `seq_add` for sequential demand.**

In `compiler/ori_arc/src/aims/intraprocedural/block.rs`, the `add_backward_demand`
function (line 196) explicitly uses `seq_add` to accumulate cardinality as instructions
are walked in reverse. This is the right operation: within a single execution path,
multiple demands on the same variable compose additively.

**3. The `alt_join` at control-flow merge is correct.**

In `compute_block_exit_state` (block.rs line 37), successor entry states are combined
via `join` (which delegates to `alt_join` for cardinality). This correctly models QTT's
branching semantics: at an if/match, only one branch executes, so the demand is the
maximum of the two branches, not their sum.

### New Invariants

**1. Document `seq_add` and `alt_join` as semiring operations in the source.**

The operations ARE a semiring (with `alt_join` as the additive operation and `seq_add` as
the multiplicative one -- note the reversal relative to QTT's naming). The doc comments
should state this explicitly. Currently `seq_add` is documented as "sequential composition"
and `alt_join` as "alternative control-flow join" but the connection to semiring algebra
is only implicit.

Specifically, the algebraic structure is:

- (Cardinality, `alt_join`, Absent) is a commutative idempotent monoid (join-semilattice)
- (Cardinality, `seq_add`, Absent) is a commutative monoid with identity Absent
- `seq_add` distributes over `alt_join`
- Many is an absorbing element for `seq_add` (Many.seq_add(x) = Many for all x)

This is actually a **bounded distributive lattice with an additional monoid operation**,
not a plain semiring (because `alt_join` is idempotent, unlike general semiring addition).
The idempotence of `alt_join` is what makes it a lub operation rather than a counting
operation. QTT's addition is NOT idempotent (1 + 1 = omega), which corresponds to
AIMS's `seq_add` (Once.seq_add(Once) = Many). The correspondence is:

| QTT semiring (+, *)        | AIMS operations         |
|----------------------------|-------------------------|
| + (resource accumulation)  | `seq_add` (along path)  |
| lub (branch join)          | `alt_join` (at merge)   |

QTT does not have a separate lub because it does branching via the "additive" linear logic
connectives. AIMS, as a dataflow analysis, naturally distinguishes sequential (along a path)
from alternative (at a merge point).

**2. Test the missing annihilation law.**

The existing tests verify that Many absorbs for `seq_add`, but do not test that Absent
is an annihilator for a hypothetical multiplicative scaling operation. AIMS does not
currently have a "scaling" operation (QTT's multiplication), because scaling arises from
interprocedural composition (a callee's parameter usage rho scaled by the caller's
argument count pi). The interprocedural analysis in `analyze_program()` handles this
implicitly through contract propagation rather than explicit semiring multiplication.
This is acceptable for now -- see 07.7 for whether it should change.

**3. Test the "positive semiring" property.**

QTT requires positivity: `a + b = 0 implies a = 0 and b = 0`. For AIMS:
`a.seq_add(b) = Absent implies a = Absent and b = Absent`. This is true by inspection
of the match arms in `seq_add`, but should have an explicit exhaustive test to guard
against future changes.

---

## 07.3 What AIMS Should Not Adopt

### Reject

**1. Full dependent type theory and type-level resource tracking.**

QTT's primary contribution is resolving the dependency/linearity conflict: how to let
variables appear in types (where they are "used" for type formation) while tracking their
computational usage. Ori is not dependently typed. There is no conflict to resolve --
types and values occupy separate phases. The entire QCwF categorical semantics (Section 3)
and the realisability models (Section 4) are irrelevant to AIMS.

**2. The sigma = 0 / sigma = 1 fragment split.**

QTT splits the theory into an erased fragment (sigma = 0, for type formation) and a
present fragment (sigma = 1, for computation). This is a solution to the dependent types
problem. In Ori, erasure is handled by the type checker (type parameters are erased before
ARC IR), not by usage annotations.

**3. Explicit multiplicative scaling as a user-visible operation.**

QTT's context scaling (pi * Gamma) is a formal device for tracking nested usage through
dependent function application. AIMS handles interprocedural scaling implicitly through
`MemoryContract` propagation: a callee's `ParamContract.cardinality` already encodes
how many times each parameter is used, and this flows back through the SCC-based
fixpoint in `interprocedural.rs`. Adding an explicit `scale` operation to `Cardinality`
would add complexity without benefit -- the current approach of propagating contracts
achieves the same result.

**4. The tensor product type and its resource-aware elimination.**

QTT's dependent tensor product (pi : S) tensor T with its forced pattern-matching
elimination (no independent fst/snd in the computational fragment) is a linear logic
construction for ensuring both components of a pair are consumed. AIMS handles this at the
ARC IR level: `Project` is the elimination for struct fields, and the backward demand
analysis naturally tracks which fields are used without requiring forced elimination.

**5. Natural number or boolean semiring variants.**

QTT mentions {0, 1} (erased/present) and N (natural numbers) as alternative semirings.
AIMS's {Absent, Once, Many} already saturates at the right level for ARC optimization:
the distinction between "used 3 times" and "used 7 times" does not affect RC insertion
strategy. Both are Many (need RcInc). Finer counting would add lattice height without
improving optimization decisions.

---

## 07.4 Plan Edits

### Section 09 (Dimensional Fusion) -- 09.4 Sequencing Algebra Extension

The current plan (09.4) correctly identifies that Locality and Effect have natural
sequencing semantics and notes that `seq_add` and `alt_join` should be documented as
intentionally equivalent to `join` for those dimensions. QTT's framework strengthens the
case for this: the plan should add a subsection explicitly naming the algebraic structure.

**Specific edit to `plans/aims/section-09-dimensional-fusion.md`, Section 09.4:**

Add a preamble paragraph before the dimension-specific items:

> **Algebraic foundation.** AIMS Cardinality operations form a bounded distributive
> lattice with semiring-like structure, directly analogous to QTT's 0-1-omega semiring
> (Atkey, LICS 2018). `seq_add` corresponds to QTT's resource accumulation (+): combining
> usages along one execution path. `alt_join` corresponds to QTT's branch join (lub):
> combining usages from mutually exclusive paths. The key properties -- associativity,
> commutativity, identity (Absent), absorption (Many), distributivity of `seq_add` over
> `alt_join` -- are verified exhaustively in `lattice/tests.rs`. For Locality and Effect,
> `seq_add` coincides with `join` because these dimensions track properties that widen
> monotonically (a value that escapes in one instruction stays escaped). This coincidence
> should be documented as intentional, not accidental. If future dimensions need
> non-idempotent sequential composition (e.g., allocation counts), `seq_add` would diverge
> from `join` and the tests would need updating.

### Section 09.4 -- Add documentation task

Add a task item:

> - [ ] **Document the QTT semiring correspondence in `dimensions.rs` doc comments.**
>   On `Cardinality`: note that `(Cardinality, seq_add, Absent)` is a commutative monoid
>   and `seq_add` distributes over `alt_join`, forming a structure analogous to QTT's
>   0-1-omega semiring. On `alt_join`: note that it is the lattice lub (idempotent), not
>   semiring addition (which would be `seq_add`). On Locality and Effect: note that
>   `seq_add` = `join` for these dimensions is a design choice, not a limitation.

### Plans overview -- Research lineage

The AIMS plans' research lineage (`plans/aims/00-overview.md`) should mention QTT
alongside the existing GHC demand analysis citation as the theoretical justification for
the semiring structure of `seq_add`/`alt_join`.

---

## 07.5 Code Changes (Later)

### `compiler/ori_arc/src/aims/lattice/dimensions.rs` -- Cardinality doc comments

Update the module-level and per-operation doc comments to name the algebraic structure:

1. **`Cardinality` enum doc:** Add a paragraph explaining the QTT correspondence:
   `Absent = 0, Once = 1, Many = omega` in Atkey's 0-1-omega semiring.

2. **`seq_add` doc:** Add: "This operation corresponds to QTT's semiring addition (+):
   combining usage counts along one execution path. Forms a commutative monoid with
   identity `Absent` and absorbing element `Many`. Distributes over `alt_join`."

3. **`alt_join` doc:** Add: "This operation is the lattice lub (idempotent, commutative,
   associative). It corresponds to the join at branching points, not QTT's semiring
   addition. `alt_join(Once, Once) = Once` because only one branch executes."

### `compiler/ori_arc/src/aims/lattice/tests.rs` -- Two additional algebraic tests

1. **Positivity test:** Verify that `a.seq_add(b) == Absent` implies `a == Absent` and
   `b == Absent`. Exhaustive over all 9 pairs. This guards the property that QTT requires
   for substitution admissibility and that AIMS relies on for backward demand soundness
   (if combined demand is zero, each individual demand must be zero).

2. **Right-distributivity test:** The existing distributivity test checks left-distribution
   (`a.seq_add(b.alt_join(c)) == a.seq_add(b).alt_join(a.seq_add(c))`). Add
   right-distribution: `(a.alt_join(b)).seq_add(c) == a.seq_add(c).alt_join(b.seq_add(c))`.
   Since `seq_add` is commutative for Cardinality, this is derivable, but an explicit test
   prevents regressions if future changes break commutativity.

### `compiler/ori_arc/src/aims/intraprocedural/block.rs` -- Comment enhancement

In `add_backward_demand` (line 196), the comment says "Uses `seq_add` for sequential
composition: within a block, each instruction adds demand on its operands sequentially."
Add: "This is the analogue of QTT's semiring addition: `x` is used once here AND once
there, so the total is `Once.seq_add(Once) = Many`."

In `compute_block_exit_state` (line 37), the doc comment says "Uses `alt_join` ... for
successor combination." Add: "This is the lattice lub, not semiring addition: at a
branch, only ONE successor executes per dynamic run, so `Once.alt_join(Once) = Once`."

---

## 07.6 Lens Shift

**For Paper 08 (Lean 4 borrow inference):**

QTT establishes that the {0, 1, omega} grading is the natural granularity for resource
tracking in a compiler. Lean 4's borrow inference operates on a similar domain -- it
distinguishes unused, borrowed (used once without ownership transfer), and owned (used
in a way that requires RC) variables. When reading Lean 4's `updateLiveVars` and
`addInc`/`addDec`, look for:

1. **Whether Lean 4 has an explicit semiring structure on its usage annotations.** QTT
   says the answer should be yes. If Lean 4's operations are ad-hoc rather than algebraic,
   that's a warning sign for subtle bugs (McBride's substitution bug came from not
   respecting the semiring laws).

2. **Whether Lean 4's interprocedural composition is multiplicative.** QTT says that when
   a function uses its parameter omega times, and the argument itself uses variable x
   once, the total usage of x is omega * 1 = omega. Check whether Lean 4's `Borrow.lean`
   propagates ownership demands multiplicatively through call edges, or whether it uses
   a different composition scheme.

3. **Whether Lean 4 distinguishes sequential and alternative composition.** AIMS already
   does (seq_add vs alt_join). If Lean 4 uses a single "join" operation for both, it
   may be losing precision at branch points (treating `Once` in both branches as `Many`
   instead of `Once`).

**For Paper 09 (GHC demand analysis):**

QTT confirms that GHC's demand analysis (which AIMS cites as a primary reference) is
using the correct algebraic framework. GHC's `lubDmd`/`plusDmd` correspond to AIMS's
`alt_join`/`seq_add`. The QTT paper gives this structure a formal name (semiring) and
proves that the laws are necessary for compositional substitution. When reading GHC,
verify that its demand types also satisfy positivity and the zero-product property.

---

## 07.7 Open Risk

**Is Cardinality {Absent, Once, Many} rich enough?**

**Yes, for ARC optimization decisions.** QTT's 0-1-omega semiring is the same three-element
structure. Atkey explicitly presents it as the canonical "useful" semiring for resource
tracking, alongside the trivial {0,1} and the full natural numbers. The three elements
map to distinct optimization strategies:

| Cardinality | RC strategy             | COW strategy             |
|-------------|-------------------------|--------------------------|
| Absent      | No operations needed    | N/A (dead variable)      |
| Once        | Move (no inc/dec)       | Static unique (no check) |
| Many        | Full RC (inc at use,    | Dynamic check needed     |
|             | dec at last use)        |                          |

No finer grading within "Many" would change these decisions. "Used 3 times" and "used 7
times" both require the same RcInc/RcDec pattern. The natural number semiring would add
precision without actionable benefit.

**Risk: interprocedural multiplicative composition is implicit, not algebraic.**

The one area where AIMS departs from QTT's algebraic discipline is interprocedural
composition. QTT multiplies usages: if function f uses parameter p omega times, and at
the call site the argument for p uses variable x once, then x's total usage is omega * 1
= omega. AIMS handles this through `MemoryContract` propagation in the SCC-based
fixpoint (`interprocedural.rs`), where callee contracts flow backward to caller demand
states via `apply_callee_contract` in `block.rs` (line 222). The contract's
`param_contract.cardinality` is merged into the caller's demand map via `merge_demand`
(join), not explicitly multiplied.

This is currently sound because the contract's cardinality already represents the callee's
total demand on that parameter (the fixpoint converges to the correct value). But it
means that a subtle bug in contract extraction could silently violate the multiplicative
composition law without any algebraic test catching it. Consider adding a property test:
for a function with known usage (e.g., `@f(x) = x + x`, where x has cardinality Many),
verify that all callers of f see their argument to x with cardinality >= Many after
contract propagation. This would catch the class of bugs that QTT's substitution lemma
is designed to prevent.

**Risk: the lattice/tests.rs tests verify Cardinality in isolation, not composition
through the analysis pipeline.**

The exhaustive algebraic tests in `lattice/tests.rs` verify that `seq_add` and `alt_join`
satisfy semiring laws on the bare `Cardinality` type. But the actual analysis composes
these operations through `add_backward_demand` (which modifies a `HashMap<ArcVarId,
AimsState>`) and `compute_block_exit_state` (which merges states across successors).
The composition happens at the `AimsState` level, where canonicalize may change cardinality
based on other dimensions (e.g., Absent forces Dead). An integration test that constructs
a small ARC IR function, runs `analyze_function`, and checks that the converged cardinality
at each program point matches the expected semiring computation would close this gap.
