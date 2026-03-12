---
section: "06"
title: "Linearity and Uniqueness: An Entente Cordiale"
status: complete
goal: "Verify the lattice distinction between Consumption and Uniqueness is sharp everywhere and no transfer rules conflate them"
paper:
  title: "Linearity and Uniqueness: An Entente Cordiale"
  doi: "https://doi.org/10.1007/978-3-030-99336-8_13"
  venue: "ESOP 2022"
  authors: "Marshall, Vollmer, Orchard"
depends_on: ["01", "02", "03", "04", "05"]
sections:
  - id: "06.1"
    title: "Paper Thesis"
    status: complete
  - id: "06.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "06.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "06.4"
    title: "Plan Edits"
    status: complete
  - id: "06.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "06.6"
    title: "Lens Shift"
    status: complete
  - id: "06.7"
    title: "Open Risk"
    status: complete
---

# Section 06: Linearity and Uniqueness: An Entente Cordiale

**Status:** Complete
**Goal:** Verify that the AIMS lattice distinction between `Consumption` (future demand --
how many more times will this be used?) and `Uniqueness` (past aliasing -- is this the
only reference?) is sharp everywhere, that no transfer rules conflate them, and identify
comments/docs that should be rewritten to prevent drift.

**Paper:** Marshall, Vollmer, and Orchard, "Linearity and Uniqueness: An Entente Cordiale,"
ESOP 2022.
[DOI: 10.1007/978-3-030-99336-8_13](https://doi.org/10.1007/978-3-030-99336-8_13)

**Why read this sixth:** AIMS needs a crisp theory for why "used once" (linearity/consumption)
and "not shared" (uniqueness) are different facts that still interact. This paper provides
exactly that framework.

**Pause questions:**
- Is the lattice distinction between Consumption and Uniqueness sharp enough everywhere?
- Are any transfer rules still conflating future demand with past aliasing guarantees?
- Which comments/docs should be rewritten to stop that drift?

**AIMS context (corrected):**
- `AccessClass`: Borrowed | Owned (aliasing mode -- is this variable a view or an owner?)
- `Consumption`: Dead < Linear < Affine < Unrestricted (substructural discipline -- what
  structural rules does this variable obey?)
- `Cardinality`: Absent < Once < Many (forward usage count -- how many future uses?)
- `Uniqueness`: Unique < MaybeShared < Shared (past aliasing -- is this the only reference?)
- The paper's linearity maps to AIMS `Consumption` + `Cardinality` (future demand)
- The paper's uniqueness maps to AIMS `Uniqueness` (past aliasing guarantee)
- `AccessClass` has no direct analogue in the paper -- it is Ori's mechanism for tracking
  whether a variable owns its RC obligation, which is orthogonal to both linearity and
  uniqueness
- Transfer functions in `transfer/mod.rs` update these dimensions simultaneously but
  they derive from DIFFERENT source facts (instructions vs control flow vs provenance)

---

## 06.1 Paper Thesis

Marshall et al. formalize a long-standing confusion in the substructural type systems
literature: **linearity and uniqueness are not the same property, they are not dual, and
they are not interchangeable -- but they interact in precise, useful ways.**

### Core distinction

**Linearity** is a restriction on FUTURE use. A linear type guarantees that a value will
be consumed exactly once going forward. It restricts the structural rules of contraction
(cannot duplicate) and weakening (cannot discard). The information flows forward: given
a linear value, we know that substituting it into an expression will preserve linearity,
because there is no way to transform a linear value into a non-linear one. Linear types
are about what can be DONE WITH a value.

**Uniqueness** is a guarantee about the PAST. A unique type guarantees that a value has
not been duplicated -- there is exactly one reference to it right now. The information
flows backward: given a unique expression, we know that any values substituted into it
will not affect the uniqueness guarantee, because there is no way to transform a
non-unique value into a unique one. Unique types are about what HAS BEEN DONE TO a value.

### The "cake and coffee" examples

The paper crystallizes the distinction with two canonical examples that are structurally
identical but fail in opposite directions:

**Linearity (Granule):** `impossible : Cake -> (Happy, Cake)` -- ill-typed because the
linear value `cake` would need to be used twice (contraction is forbidden). You can't
have your cake and eat it too. Linearity says: "this value will be consumed once in the
future."

**Uniqueness (Clean):** `impossible :: *Coffee -> (*Awake, *Coffee)` -- ill-typed because
after duplicating `coffee`, we can no longer guarantee either copy is unique (the
argument was unique on entry, but after use in `drink`, the second occurrence `keep`
cannot claim uniqueness). Uniqueness says: "this value has not been duplicated in the
past."

### The interaction: when they diverge

In a system with only linear and unrestricted values, linearity and uniqueness are
equivalent -- if nothing can ever be duplicated, then everything has exactly one
reference. The crucial divergence arises when UNRESTRICTED values exist alongside
restricted ones:

- **Unique to non-unique (losing uniqueness):** Given a unique value `*A`, we can
  "borrow" it to produce a non-unique `A`. The uniqueness guarantee is forgotten
  (the value may now have multiple references). This is the BORROW rule:
  `Gamma |- t : *A / Gamma |- &t : !A`.

- **Non-linear from linear (gaining linearity):** Given a non-linear `!A`, we can
  produce a linear `A` by eliminating the `!` modality. The linearity restriction
  is imposed. This is the `!E` rule (dereliction):
  `Gamma, x : A |- t : B / Gamma, x : [A] |- t : B`.

- **Linear to non-linear is IMPOSSIBLE:** We cannot promote a linear value to
  non-linear. This is the asymmetry that makes linearity a comonad and
  non-uniqueness a monad, not a simple duality.

- **Non-unique to unique is IMPOSSIBLE:** We cannot claim a non-unique value is
  unique. The uniqueness modality `*` can only be introduced via `NEC` when
  there are no dependencies: `emptyset |- t : A / [Gamma] |- *t : *A`.

### The formal calculus (LCU)

The paper builds the Linear-Cartesian-Unique (LCU) calculus on a linear basis
(multiplicative linear lambda calculus with `!` modality for non-linearity and `*`
modality for uniqueness). Key typing rules:

- **VAR:** `[Gamma], x : A |- x : A` -- using a linear variable marks the rest of
  the context as non-linear.
- **BORROW:** `Gamma |- t : *A / Gamma |- &t : !A` -- a unique value can be
  borrowed into a non-linear (unrestricted) value.
- **COPY:** `Gamma_1 |- t_1 : !A, Gamma_2, x : *A |- t_2 : !B / Gamma_1 + Gamma_2 |-
  copy t_1 as x in t_2 : !B` -- a non-linear value of type `A` can be copied
  to produce a unique `*A` (the copy is deep).
- **NEC (necessitation):** `emptyset |- t : A / [Gamma] |- *t : *A` -- values
  with no dependencies can be assumed unique.

### Key theorems

- **Theorem 4 (Conservation = Linearity):** For a well-typed term, a reduction
  preserves the typing context and resource usage is approximated by the heap.
  "If a variable is linear then it must always be used in a linear way."

- **Theorem 5 (Uniqueness):** For a well-typed term with a unique type `*A`,
  array references contributing to the final term that are unique in the incoming
  heap stay unique in the resulting heap, and new array references are also unique.
  "If a variable is unique then it has only one reference."

### The "entente cordiale" (takeaway, p.8)

> *Linearity and uniqueness behave dually with respect to composition, but
> identically with respect to structural rules, i.e., their internal plumbing.*

Both non-linearity (`!`) and non-uniqueness (circle) are comonoidal internally --
they allow the same contraction and weakening rules. The duality is in how values
enter and exit these modalities. This is why they look similar (same structural rules)
but are genuinely different (opposite information flow).

---

## 06.2 What AIMS Should Adopt

### Keep

**1. The existing dimension separation is correct and well-designed.** The AIMS
lattice already encodes exactly the distinction Marshall et al. formalize:

| Paper concept | AIMS dimension(s) | Direction | Structural rules |
|---|---|---|---|
| Linearity (future demand) | `Consumption` + `Cardinality` | Backward (what WILL happen) | `rc_inc` = contraction, `rc_dec` = weakening |
| Uniqueness (past aliasing) | `Uniqueness` | Forward-derived (what HAS happened) | COW, reset/reuse eligibility |
| Unrestricted (no restrictions) | `Consumption::Unrestricted` + `Cardinality::Many` | N/A | Full RC |

The decision to place `Borrowed` in `AccessClass` rather than `Consumption`
(solutions.md Decision 1) is vindicated by the paper: borrowing is not a consumption
mode (not about future use count) but an aliasing state (about current reference
structure). The paper's BORROW rule maps a unique value to a non-linear one,
changing the access modality without altering the substructural discipline.

**2. The `is_rc_inc_elidable` and `is_rc_dec_unnecessary` predicates are
correctly factored.** In `transfer/mod.rs`:

- `is_rc_inc_elidable(state)`: checks `cardinality == Once && consumption == Linear`.
  This is purely about FUTURE demand (linearity). Correct.
- `is_rc_dec_unnecessary(state)`: checks `cardinality == Absent || consumption == Dead`.
  This is purely about FUTURE demand (the value has no remaining uses). Correct.
- `cow_mode_from_uniqueness(uniqueness)`: checks ONLY the `Uniqueness` dimension.
  This is purely about PAST aliasing. Correct.
- `can_mutate_in_place(state)`: checks `access == Owned && uniqueness == Unique`.
  This combines ownership (access) with past aliasing (uniqueness). Correct --
  mutation requires both that you own the RC obligation AND that no other
  references exist.

None of these predicates mix linearity facts with uniqueness facts. The separation
is clean.

**3. The `transfer_project` rule correctly preserves uniqueness through borrows.**
In `transfer/mod.rs` line ~144: `uniqueness: source.uniqueness`. A `Project`
(field extraction) produces a borrowed view that inherits the source's uniqueness.
This matches the paper's BORROW rule: borrowing does not duplicate the reference,
so the source's uniqueness is preserved.

**4. The `Consumption` ordering is correct for Ori's semantics.** The paper
works with a binary linear/non-linear split. AIMS refines this into a 4-point
chain: `Dead < Linear < Affine < Unrestricted`. This is strictly more expressive
than the paper's system and correctly captures Ori's operational semantics where
values can be:
- Dead (no future use)
- Linear (exactly one future use, no duplication or discard)
- Affine (at most one future use, may be discarded)
- Unrestricted (any number of future uses)

The paper's Theorem 4 (Conservation) applies to the `Linear` level; the `Affine`
level corresponds to systems with built-in weakening (the paper discusses affine
types in Section 2.1, noting they "allow discarding behaviour by adding back in
weakening").

### New Invariants

**Invariant 1: No transfer function may derive `Uniqueness` from `Consumption`
or `Cardinality` alone.** Uniqueness is about the PAST (has this been duplicated?),
while consumption/cardinality are about the FUTURE (how will this be used?).
A value can be `Linear + Once` (will be used once) but `Shared` (multiple
references exist from prior aliasing). Conversely, a value can be `Unique`
(sole reference) but `Unrestricted + Many` (will be used many times via `rc_inc`).

This invariant is currently satisfied in all transfer functions. It should be
documented as a design invariant in `transfer/mod.rs`.

**Exception (sound):** The proposed canonicalize Rule 4 in Section 09.3
(`BlockLocal + Owned + <=Once -> Unique`) does derive uniqueness from cardinality
+ locality, but this is sound because it also requires `BlockLocal` locality -- a
value that has not escaped its defining block AND is owned AND has at most one use
CANNOT have been aliased. The locality dimension provides the past-aliasing
evidence that bridges the gap.

**Invariant 2: `Consumption` and `Cardinality` must always agree on liveness.**
This is already enforced by canonicalize (`Dead <-> Absent`), but the invariant
should be stated as: "the linearity dimensions form a consistent picture of future
demand." The paper's linearity is a single concept; AIMS splits it into two
dimensions (structural discipline + usage count) for precision, but they must
never contradict.

**Invariant 3: `Uniqueness` changes ONLY from these sources:**
1. A `Construct` instruction (fresh allocation = `Unique`)
2. A COW operation result (both paths produce unique output)
3. An `rc_inc` instruction or sharing event (= `Shared` or `MaybeShared`)
4. Join at control flow merge (= `max(branch_a, branch_b)`)
5. Interprocedural contract (callee's return uniqueness)
6. Canonicalize Rule 4 (locality-mediated, sound per exception above)

`Uniqueness` must NEVER be derived from: the number of future uses alone, the
consumption mode alone, or the cardinality alone. These are all FUTURE facts;
uniqueness is a PAST fact.

---

## 06.3 What AIMS Should Not Adopt

### Reject

**1. Full substructural type system at the surface level.** The LCU calculus
requires explicit `!` and `*` type annotations, `borrow`/`copy` terms, and
tracking of linear vs non-linear contexts. Ori has no surface-level linear types.
AIMS operates entirely at the ARC IR level as an optimization analysis, not as a
type discipline. The paper's contribution to AIMS is conceptual clarity, not type
system features.

**2. The BORROW/COPY term-level constructs.** In LCU, `&t` (borrow) and
`copy t as x in t2` (deep copy) are explicit syntactic constructs. In AIMS,
borrowing is an emergent property of `Project` instructions (field extraction
produces a borrowed view), and copying is an emergent property of `rc_inc`
(contraction). AIMS does not need term-level constructs for these because
they are derived from the IR's operational semantics.

**3. The relative monad/comonad structure.** The paper shows `*` (uniqueness)
forms a relative monad over `!` (non-linearity). This is theoretically elegant
but AIMS does not need the categorical structure -- it needs the operational
consequences (which transfer functions already encode). The monad/comonad
relationship would matter if AIMS had higher-order transformations on its lattice,
but it does not.

**4. Fractional permissions (Future Work, Section 6).** The paper sketches
`*_n P` where `0 < n <= 1` for fractional uniqueness. This is interesting for
Rust-style mutable borrows but Ori's ARC-based system does not need it -- the
`MaybeShared` lattice point already handles "don't know the exact refcount"
without quantifying it.

**5. Graded modalities for linearity.** The paper implements linearity in
Granule's graded modal framework with semiring grades `{0, 1, omega}`. AIMS
already has a more refined cardinality system (`Absent < Once < Many`) with
sequencing algebra (`seq_add`, `alt_join`). The GHC-inspired demand analysis
approach is better suited to an optimization analysis than a graded type system.

---

## 06.4 Plan Edits

### Section 01 (Lattice)

**Fix the AIMS context block in this file (already done above).** The original
template stated `Consumption: Dead < Borrowed < Owned` which conflates the access
dimension with the consumption dimension -- exactly the confusion Marshall et al.
warn against. The corrected version above separates `AccessClass` from `Consumption`
and maps each to the paper's concepts.

**Add to section-01-lattice.md Section 01.3 (Uniqueness Dimension):** After the
existing Marshall et al. citation (line ~108 of `dimensions.rs`), add a doc-level
note explaining the full mapping:

> The three-way split of resource properties in AIMS corresponds to Marshall et al.'s
> analysis: `Consumption` + `Cardinality` encode linearity (future demand -- what
> structural rules does this value obey going forward?), while `Uniqueness` encodes
> uniqueness (past aliasing -- has this value been duplicated?). These are different
> type-theoretic properties with different information flow: linearity restricts what
> CAN be done with a value, uniqueness guarantees what HAS been done to a value.
> They interact (a unique value used linearly needs no RC at all) but must never be
> conflated (a shared value used linearly still needs `rc_dec` at its single use
> point; a unique value used many times needs `rc_inc` at each additional use).

**Add to section-01-lattice.md Section 01.3a (Dimension Interactions):** The
existing interaction rules are all correct but should be annotated with the
Marshall et al. framework:

- **Consumption x Uniqueness interaction:** "This cross-product encodes the paper's
  central insight. A value can be `(Linear, Unique)` = single use of sole reference
  (no RC at all), `(Linear, Shared)` = single use but other refs exist (need
  `rc_dec` at use), `(Unrestricted, Unique)` = many uses of sole reference (need
  `rc_inc` at each extra use, COW is static-unique), `(Unrestricted, Shared)` =
  many uses with sharing (full ARC, COW is always-copy). Each combination is
  meaningful and drives a different optimization."

### Section 09 (Dimensional Fusion)

**Add a "Marshall et al. design invariant" to Section 09.1 (Transfer Fusion):**
Before any transfer fusion rule, state the design invariant: "No fusion rule may
derive uniqueness from consumption or cardinality alone. Uniqueness is about the
past; consumption and cardinality are about the future. A fusion rule that crosses
this boundary must also involve a past-facing dimension (locality, shape, or an
interprocedural contract) to bridge the gap."

This invariant should be a GATE for all proposed fusion rules in 09.1. Specifically:

- Rule "Pure callee preserves caller uniqueness" -- SOUND because it uses the
  callee's `EffectSummary.may_share` (a past-facing fact about what the callee
  HAS done).
- Rule "Linear consumption at call site enables callee reuse" -- needs scrutiny.
  This propagates `uniqueness = Unique` into the callee based on `consumption ==
  Linear + cardinality == Once`. But `Linear + Once` is a FUTURE fact (caller
  will use this once). The bridge is: if the caller will use the value once AND
  owns it (`access == Owned`), then at the moment of the call, the caller's
  reference is the only live reference that will consume the value. This is
  sound but the reasoning crosses the linearity/uniqueness boundary and MUST be
  documented.

---

## 06.5 Code Changes (Later)

### `lattice/dimensions.rs` -- Comment enhancement

**File:** `compiler/ori_arc/src/aims/lattice/dimensions.rs`

**Lines 28-33 (`Consumption` doc comment):** The existing comment says "Substructural
consumption mode" which is correct but does not explain the directionality. Rewrite to:

```rust
/// Substructural consumption discipline: what structural rules does this
/// value obey going forward?
///
/// This is a FUTURE-facing dimension (Marshall et al., ESOP 2022: "linearity
/// is a restriction on what can be done with a value in the future"). RC
/// operations are the operational realization of structural rules:
/// `rc_inc` = contraction (duplication), `rc_dec` = weakening (discard).
///
/// NOT to be confused with [`Uniqueness`], which is a PAST-facing dimension
/// about aliasing history. A value can be `Linear` (one future use) but
/// `Shared` (multiple current references). These are independent facts.
///
/// Ordered: `Dead < Linear < Affine < Unrestricted`. Chain height: 3.
/// Based on Chirimar et al.: `rc_inc` = contraction, `rc_dec` = weakening.
```

**Lines 102-108 (`Uniqueness` doc comment):** The existing comment already cites
Marshall et al. and states the distinction. It is correct. Add one sentence of
operational consequence:

```rust
/// Runtime reference count knowledge.
///
/// Ordered: `Unique < MaybeShared < Shared`. Chain height: 2.
/// Uniqueness is a PAST guarantee ("not duplicated"), distinct from linearity
/// which is FUTURE ("consumed once") -- Marshall et al., ESOP 2022.
///
/// A value can be `Unique` but `Unrestricted` (sole reference, used many times
/// via `rc_inc`). Conversely, a value can be `Linear` but `Shared` (one future
/// use, but other references exist). These combinations drive different RC
/// strategies: unique+linear = no RC; unique+unrestricted = static COW;
/// shared+linear = dec only; shared+unrestricted = full ARC.
```

### `transfer/mod.rs` -- Module-level doc enhancement

**File:** `compiler/ori_arc/src/aims/transfer/mod.rs`

**Lines 1-16 (module doc):** Add a design invariant section:

```rust
//! # Design invariant (Marshall et al., ESOP 2022)
//!
//! Transfer functions update `Consumption`/`Cardinality` (future demand) and
//! `Uniqueness` (past aliasing) simultaneously, but they derive these updates
//! from DIFFERENT source facts:
//!
//! - **Future demand** (Consumption, Cardinality): derived from how many times
//!   an instruction's operands appear in future instructions. Computed by
//!   backward demand propagation.
//! - **Past aliasing** (Uniqueness): derived from allocation provenance
//!   (Construct = Unique), sharing events (rc_inc = Shared), and control flow
//!   merges (join = max).
//!
//! No transfer function may derive Uniqueness from Consumption or Cardinality
//! alone. The `canonicalize()` rule `BlockLocal + Owned + <=Once -> Unique`
//! (Section 09.3) is the one exception, and it requires `BlockLocal` locality
//! (a past-facing escape analysis fact) to bridge the gap.
```

### `transfer/mod.rs` -- `is_rc_inc_elidable` comment

**File:** `compiler/ori_arc/src/aims/transfer/mod.rs`, line ~367

The existing comment says "the value has a single consumer that moves it -- no inc
needed." Clarify the directionality:

```rust
/// Whether an RC increment can be elided at a use site.
///
/// If cardinality is `Once` and consumption is `Linear`, the value has
/// a single consumer that moves it -- no inc needed. This is purely a
/// FUTURE-demand decision (linearity): the value will be used once, so no
/// contraction (duplication) occurs. The `Uniqueness` dimension is
/// irrelevant here -- even a `Shared` value used once needs no inc
/// (though it may need a dec if this is the last reference).
```

### `lattice/mod.rs` -- `canonicalize` comment

**File:** `compiler/ori_arc/src/aims/lattice/mod.rs`, lines 220-253

The canonicalize rules enforce cross-dimension consistency. Add a framing comment:

```rust
/// # Invariant categories (Marshall et al., ESOP 2022)
///
/// Rules 1-3 enforce consistency within the FUTURE-demand dimensions
/// (Consumption <-> Cardinality). These dimensions jointly encode the
/// paper's notion of linearity.
///
/// Rule 3 (Shared + ReusableCtor -> NonReusable) crosses the future/past
/// boundary: it uses the PAST fact (Shared = aliased) to constrain a
/// reuse decision. This is sound because reuse requires sole ownership
/// of the memory, which is a past-aliasing property.
///
/// Future rules from Section 09.3 must document which boundary they cross.
```

### No changes to `intraprocedural/mod.rs` or `intraprocedural/block.rs`

The backward analysis correctly computes FUTURE demand via `seq_add` and `alt_join`.
The module doc in `block.rs` correctly states "demand from successors" and "total
demand placed on variables flowing into this block." No rewriting needed.

### No changes to `emit_rc/mod.rs`

The RC emission logic correctly reads `Consumption`/`Cardinality` for inc/dec
decisions and `Uniqueness` for COW decisions. The separation is clean.

#### Fix along the way (when touching this file)

- [ ] **[BLOAT]** `aims/emit_rc/mod.rs:970` — 970 lines, nearly 2x the 500-line limit. Extract submodules: `emit_block_rc` + phases A/B/C helpers into `phases.rs`, ownership transfer/consuming primop detection into `ownership_transfer.rs`, `collect_all_borrowed_defs` + `collect_rc_incremented_vars` into `borrowed_tracking.rs`.

---

## 06.6 Lens Shift

### Reading Paper 07 (Quantitative Type Theory, Atkey LICS 2018)

Marshall et al. frame linearity as a binary restriction on structural rules
(contraction/weakening). QTT generalizes this to a semiring of usage quantities
where 0 = irrelevant (erased), 1 = linear, omega = unrestricted, and the semiring
structure allows compositional reasoning.

**Key shift for reading QTT through this lens:**

1. **QTT's quantities encode FUTURE demand only.** QTT does not have a uniqueness
   modality. When QTT says a variable has quantity 1, it means "used once going
   forward" -- this is linearity, not uniqueness. AIMS should map QTT quantities
   to `Cardinality` (and the semiring operations to `seq_add`/`alt_join`), NOT to
   `Uniqueness`.

2. **QTT's 0 quantity maps to `Absent`/`Dead`, not to "no aliasing."** A QTT
   variable with quantity 0 is erased/unused. In AIMS this is `Cardinality::Absent`
   with `Consumption::Dead`. It says nothing about how many references exist --
   just that zero future uses will occur.

3. **QTT does not capture the past/future distinction.** The paper's main
   contribution for AIMS was sharpening the boundary between linearity (future)
   and uniqueness (past). QTT lives entirely on the future side. When reading
   QTT, do not expect it to inform the `Uniqueness` dimension. QTT will inform
   the `Cardinality` dimension's algebraic properties (semiring laws, which AIMS
   already tests in `section-01-lattice.md` Section 01.4).

4. **The interaction is where AIMS adds value over QTT.** QTT cannot express "this
   value is used once AND is the sole reference" -- it can only express the first
   half. AIMS's product lattice captures both halves simultaneously, enabling
   optimizations (no-RC-at-all for unique+linear) that QTT alone cannot derive.

---

## 06.7 Open Risk

### Risk 1: The `(Linear, Shared)` transient state during fixed-point iteration

Section 01.3a of the lattice plan (line ~203) documents that `(Owned, Linear, *,
Shared)` is a transient state during iteration that "should not appear in converged
output." The reasoning is: if a value is `Linear` (one future use) but `Shared`
(multiple references), then another reference must exist that is NOT represented in
the current function's demand -- either an interprocedural escape or a bug.

Marshall et al. confirm this is a valid concern: linearity restricts future use, but
shared uniqueness means the past included duplication. The combination IS meaningful
(a shared value that this function will use once does need an `rc_dec` but not an
`rc_inc`). However, the plan's comment that this state "should not appear in converged
output" is too strong. **It CAN appear in converged output legitimately** when a
function receives a shared parameter but only uses it once. The `rc_dec` without
`rc_inc` pattern is correct for this case.

**Action:** Revise the comment in section-01-lattice.md (line ~204-212) to say:
"This state is valid in converged output when a function receives a shared value
(e.g., parameter with `Uniqueness::Shared` from interprocedural contract) but uses
it at most once. The emission logic correctly handles it: `rc_dec` at last use, no
`rc_inc`. The previous note calling this 'infeasible' was based on conflating
linearity with uniqueness (the error Marshall et al. warn against)."

### Risk 2: `capture_state_update` sets both future AND past dimensions

In `transfer/mod.rs`, `capture_state_update()` (line ~419) sets:
- `consumption = Unrestricted` (future: closure may invoke captured value many times)
- `cardinality = Many` (future: multiple invocations)
- `locality = HeapEscaping` (past/present: closure escapes to heap)

But it does NOT change `uniqueness`. This is correct per Marshall et al.: capturing
a value in a closure changes its future demand profile (it may be used many times)
but does not change its past aliasing history (if it was unique before capture, it
is still unique -- the closure holds the sole reference to the captured value, unless
the closure itself is shared). The `rc_inc` at capture time will change uniqueness
to `Shared` if the closure is invoked -- but that happens at the call site, not at
the capture site.

**Status:** No action needed. The current code is correct. Document the reasoning.

### Risk 3: Section 09 fusion rules crossing the linearity/uniqueness boundary

The proposed fusion rule "Linear consumption at call site enables callee reuse"
(Section 09.1) derives uniqueness from consumption. As analyzed in Section 06.4 above,
this is sound but requires careful reasoning that crosses the boundary. The risk is
that future fusion rules will make similar crossings without documenting the
bridge fact.

**Action:** Add the design invariant from Section 06.4 as a mandatory gate for all
Section 09 fusion rules. Any rule that derives a PAST-facing dimension value from
FUTURE-facing dimensions must explicitly identify the bridge fact (locality, contract,
or instruction semantics) that makes the crossing sound.

### Risk 4: Backward analysis direction creates a subtle terminology trap

AIMS runs backward analysis: it discovers FUTURE demand by walking from exits to
entries. The `Uniqueness` dimension is also computed during this backward walk. This
creates a terminology hazard: someone reading the code might think "backward analysis
= past-facing," but backward analysis direction is about the DATAFLOW DIRECTION
(exits to entries), not about the TEMPORAL DIRECTION of the property being computed.

- `Consumption` and `Cardinality`: backward dataflow direction, FUTURE temporal direction
  ("what will happen to this value going forward?")
- `Uniqueness`: backward dataflow direction, PAST temporal direction ("has this value
  been aliased?") -- but computed during the backward walk by tracking which instructions
  CREATE uniqueness (Construct) vs DESTROY it (sharing events)

**Action:** Add a comment in `intraprocedural/mod.rs` module doc clarifying:
"The analysis direction is backward (exits to entries) but the dimensions have
different temporal semantics: Consumption/Cardinality track FUTURE demand (linearity),
while Uniqueness tracks PAST aliasing history. Both are computed in the same backward
pass for efficiency, but they answer different questions."

### No conflation found in existing transfer rules

After reviewing all transfer functions in `transfer/mod.rs`:

- `transfer_construct`: sets `Unique` from instruction semantics (fresh allocation),
  not from demand. Correct.
- `transfer_project`: inherits uniqueness from source, not from projected field's
  demand. Correct.
- `transfer_apply_conservative`: sets `MaybeShared` conservatively, not derived
  from demand. Correct.
- `transfer_partial_apply`: uses `FRESH` (Unique), from instruction semantics.
  Correct.
- `backward_demands`: returns only `Cardinality` contributions. Does not touch
  `Uniqueness`. Correct.
- `capture_state_update`: changes demand dimensions only, preserves uniqueness.
  Correct.
- `consumed_state`: sets `Unique` (consumed value = sole reference about to be freed).
  Correct -- this is instruction semantics, not demand derivation.

**Conclusion:** The existing transfer rules maintain a clean separation between
future-demand dimensions and past-aliasing dimensions. The AIMS lattice correctly
implements the Marshall et al. distinction. The primary risk areas are in FUTURE
fusion rules (Section 09) that might cross the boundary without documenting the
bridge fact.
