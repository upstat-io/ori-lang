---
section: "01"
title: "Oxidizing OCaml — Modal Memory Management"
status: complete
goal: "Determine whether AIMS locality is too passive and whether access, uniqueness, and locality are cleanly separated"
paper:
  title: "Oxidizing OCaml with Modal Memory Management"
  doi: "https://doi.org/10.1145/3674642"
  venue: "ICFP 2024"
  authors: "Lorenzen, White, Dolan, Eisenberg, Lindley"
depends_on: []
sections:
  - id: "01.1"
    title: "Paper Thesis"
    status: complete
  - id: "01.2"
    title: "What AIMS Should Adopt"
    status: complete
  - id: "01.3"
    title: "What AIMS Should Not Adopt"
    status: complete
  - id: "01.4"
    title: "Plan Edits"
    status: complete
  - id: "01.5"
    title: "Code Changes (Later)"
    status: complete
  - id: "01.6"
    title: "Lens Shift"
    status: complete
  - id: "01.7"
    title: "Open Risk"
    status: complete
---

# Section 01: Oxidizing OCaml --- Modal Memory Management

**Status:** Complete
**Goal:** Determine whether AIMS locality is too passive, whether access/uniqueness/locality
are cleanly separated as mode axes, and whether contracts need stronger escape/locality
commitments.

**Paper:** Lorenzen, White, Dolan, Eisenberg, Lindley. "Oxidizing OCaml with Modal Memory
Management." Proc. ACM Program. Lang. 8, ICFP, Article 253 (August 2024), 53 pages.
[DOI: 10.1145/3674642](https://doi.org/10.1145/3674642)

**Full text accessed:** Yes. PDF retrieved from author's site (antonlorenzen.de). All 53 pages
read including appendices (proofs, mode inference, graded calculus, extended semiring).

**Why read this first:** It sharpens the mode axes and keeps you honest about locality
not being an afterthought. OxCaml treats affinity, uniqueness, and locality as independent
modal axes with inference --- exactly the decomposition AIMS claims for its lattice dimensions.

---

## 01.1 Paper Thesis

The paper's fundamental claim is that **three independent mode axes --- affinity, uniqueness,
and locality --- form a product lattice that enables safe stack allocation and in-place memory
reuse in a GC-managed functional language, without a borrow checker, and with complete
type inference**.

The decomposition choice is:

| Axis | Minimum | Maximum | Sub-moding |
|------|---------|---------|------------|
| Affinity (a) | many | once | many < once |
| Uniqueness (u) | unique | aliased | unique < aliased |
| Locality (l) | global | local | global < local |

A mode is a triple (a, u, l). Modes form a lattice ordered pointwise. The key insight
that separates this from prior work:

1. **Uniqueness is about the past** (the value has not been aliased). **Affinity is about
   the future** (the value will be consumed at most once). These are independent dimensions,
   following Marshall et al. (ESOP 2022). AIMS already encodes this separation as
   `Uniqueness` (past) and `Consumption` (future), which is correct.

2. **Locality is orthogonal to both.** A value can be unique-and-local, unique-and-global,
   aliased-and-local, etc. Locality governs *where* memory lives (stack vs heap, region
   escape). Uniqueness governs *how many* references exist. Affinity governs *how many
   future uses* are permitted.

3. **Modalities are functions on modes, not separate types.** The paper defines three
   modalities (A, M, G) that are functions from mode triples to mode triples:
   - `aliased(a, u, l) = (a, aliased, l)` --- forgets uniqueness
   - `many(a, u, l) = (many, u, l)` --- forgets affinity
   - `global(a, u, l) = (a, aliased, global)` --- forces both aliased AND global
   The critical detail: `global` affects *both* uniqueness and locality. This is not
   an artifact --- it encodes the proof obligation that a value escaping to the heap
   (becoming global) can no longer be assumed unique, because heap-allocated values
   can be reached from multiple roots.

4. **Modes can be completely inferred.** The inference works by generating inequality
   constraints on mode variables and solving via transitive closure. No programmer
   annotations are required (though they can be provided). The LAM rule applies a
   "lock" operation to the context that adjusts captured variable modes based on the
   closure's mode. This is key: closure capture is where mode information propagates
   interprocedurally.

5. **Soundness is proven via a usage-aware store semantics** (Section 4). The store
   tracks per-binding modes. Progress and preservation theorems are stated and proved
   (Lemma 4.1, Lemma 4.2). The store typing relation (WF-BASE, WF-SPACE, WF-UNUSED,
   WF-EXT) enforces the invariant that stack-allocated variables can only point to
   values in the same or earlier stack frames.

6. **The paper connects to graded modal calculi** (Section 5) by constructing a
   partially-ordered semiring whose elements encode the combined
   affinity-times-uniqueness mode space. The "naive semiring" has 7 elements; the
   extended semiring (to handle aliasable closures) has 10. This establishes that the
   mode system is not ad-hoc but a specific instantiation of a general framework.

**What the paper is NOT claiming:**
- It does not claim to replace reference counting. It is explicitly designed for a GC'd
  language. RC is mentioned only in Related Work as a complementary approach.
- It does not handle concurrency modes (sync/contention). The blog posts discuss these
  as future extensions; the paper focuses only on affinity, uniqueness, and locality.
- It does not provide runtime performance gains from in-place reuse (that is still under
  development). The demonstrated gains are from stack allocation (90% allocation reduction,
  9% runtime improvement on the ocamldep case study).

---

## 01.2 What AIMS Should Adopt

### Keep

**K1: The `global` modality must affect BOTH uniqueness and locality.**

OxCaml's `global` modality is `(a, aliased, global)` --- it forces uniqueness to `aliased`
AND locality to `global` simultaneously. This encodes the invariant: a value that escapes
to the heap cannot be assumed unique, because any heap root can be reached from multiple
paths.

AIMS currently treats locality and uniqueness as independent during transfer. Section 09.3
proposes Rule 6: `HeapEscaping + Borrowed -> force MaybeShared`. This is the right
direction but too narrow --- it applies only to borrowed values. The OxCaml insight is
stronger: **any value whose locality transitions to HeapEscaping (regardless of access
class) must have its uniqueness ceiling lowered**. An owned value stored into a heap
structure is reachable from that structure's root, and if the structure is aliased, so is
the stored value.

**Affects:** `compiler/ori_arc/src/aims/lattice/mod.rs` (canonicalize), Section 09.3 Rule 6.

**K2: Locality must be a first-class axis with its own sub-moding, not an auxiliary.**

OxCaml proves that locality with just two levels (local/global) already enables stack
allocation and safe borrowing. AIMS has four levels (BlockLocal, FunctionLocal,
HeapEscaping, Unknown) which is richer, but the extra granularity is wasted if locality
remains auxiliary. The paper's results demonstrate that even binary locality delivers
measurable value (90% allocation reduction). AIMS should commit to making locality
a full participant in Stage 2, not defer it further.

**Affects:** `plans/aims/section-09-dimensional-fusion.md` Section 09.2 (Locality Activation).

**K3: Closure capture is where mode information concentrates.**

OxCaml's LAM rule applies a "lock" operation that adjusts captured variable modes based
on the closure's own mode. A `many` closure (invokable multiple times) can only capture
`aliased` variables. A `once` closure can capture `unique` variables. This is not just a
restriction --- it is the key inference propagation mechanism.

AIMS currently handles closures conservatively: `transfer_construct` for `CtorKind::Closure`
produces `AimsState::FRESH`. The interprocedural contract extraction does not encode
closure capture constraints. This means AIMS misses the single largest source of
uniqueness refinement: knowing that a closure used once preserves the uniqueness of
its captured values.

**Affects:** `compiler/ori_arc/src/aims/transfer/mod.rs` (transfer_construct for closures),
`compiler/ori_arc/src/aims/interprocedural.rs` (contract extraction for closures).

**K4: Borrowing as a derived concept from locality + uniqueness, not a primitive.**

OxCaml's `borrow` combinator has type:
`'a @ unique -> ('a @ local -> 'b) -> ('a * 'b aliased) @ unique`

The key: borrowing is not a primitive mode. It is *derived* from the interaction of locality
and uniqueness. A borrowed reference is local (cannot escape) and aliased (multiple
references exist within the region). When the region ends, the original unique reference
is restored because all aliases were local and thus dead.

AIMS currently has `AccessClass::Borrowed` as a separate dimension from `Locality`. The
OxCaml insight suggests these should interact more tightly: `Borrowed` should imply
`FunctionLocal` (or tighter) locality, and the analysis should prove that borrowed
references do not escape their defining scope. This tighter coupling is already partially
captured by `BorrowSource` tracking, but the locality dimension does not currently
constrain access class.

**Affects:** `compiler/ori_arc/src/aims/lattice/mod.rs` (canonicalize --- new rule),
Section 09.3.

**K5: Deep mode property --- modes propagate through destructuring.**

OxCaml's modes are "deep": destructuring a unique value yields unique components.
Destructuring a local value yields local components. AIMS's `transfer_project` already
propagates uniqueness from source to projected field (`uniqueness: source.uniqueness`),
which is correct. But it does NOT propagate locality: a field projected from a
`BlockLocal` source gets `Locality::Unknown` (from `AimsState::TOP` defaults).

**Affects:** `compiler/ori_arc/src/aims/transfer/mod.rs` (transfer_project --- add locality
propagation).

### New Invariants

**I1: HeapEscaping -> not Unique (unless proven otherwise).**

If `locality >= HeapEscaping` then `uniqueness` must be `>= MaybeShared` unless the
analysis can prove the heap structure itself is unique (Unique + HeapEscaping is only
valid when the containing structure is also Unique). This strengthens Section 09.3 Rule 6
and mirrors OxCaml's `global` modality affecting uniqueness.

This invariant belongs in `canonicalize()` in `lattice/mod.rs`.

**I2: Borrowed implies scope-bounded locality.**

If `access == Borrowed` then `locality <= FunctionLocal`. A borrowed reference by
definition cannot escape its defining function (it is a temporary view). If the analysis
ever produces `Borrowed + HeapEscaping`, this is an error in the analysis, not a valid
state.

This invariant belongs in `canonicalize()` in `lattice/mod.rs`.

**I3: Once-closure capture preserves uniqueness of captured values.**

When a value is captured by a closure that will be invoked at most once (the closure's
`consumption <= Linear`), the captured value's uniqueness should be preserved through
the closure. This is the "lock" mechanism from OxCaml. AIMS does not currently have
closure-consumption-aware transfer, but this invariant should be documented as a
requirement for Section 09.1 Transfer Fusion.

This invariant belongs in `transfer/mod.rs` (closure construct transfer) and
`interprocedural.rs` (contract extraction).

**I4: Locality propagates through projections (deepness).**

`Project(dst, src, field)` must set `dst.locality <= src.locality`. A field of a
block-local value is at most block-local. This is a monotonicity requirement that
mirrors OxCaml's deep mode property.

This invariant belongs in `transfer/mod.rs` (transfer_project).

---

## 01.3 What AIMS Should Not Adopt

### Reject

**R1: Mode annotations on types / type qualifiers.**

OxCaml attaches modes to types via `@ unique`, `@ local`, `@@ aliased` (modality on
fields). This is a surface-language design choice for a language that wants programmer
control over modes. Ori does not expose memory modes to programmers --- AIMS is a
compiler-internal analysis. Adding mode syntax to Ori's type system would violate Ori's
"no borrow checker" design pillar. AIMS should infer everything; the programmer should
never write mode annotations.

**R2: Regions as first-class constructs (exclave, borrow construct).**

OxCaml introduces `exclave` to end a region prematurely and a `borrow` construct to
create scoped aliased references. These are surface-language features that make sense
for a GC'd language where the programmer manually controls allocation location. Ori's
ARC system handles deallocation automatically --- regions are an implementation detail
of the compiler, not a user-visible concept. AIMS's BlockLocal/FunctionLocal levels
capture region-like scope information without exposing it.

**R3: The specific sub-moding direction `global < local`.**

OxCaml uses `global < local` (global is the *weaker* mode --- a global value can always be
used where a local is expected). This is the correct direction for a GC language where
"local" means "restricted to this scope." In AIMS, the lattice direction is reversed:
`BlockLocal < FunctionLocal < HeapEscaping < Unknown`, where `BlockLocal` is the
*tightest* (most optimistic) and `Unknown` is the widest (most conservative). This is
correct for a backward analysis where we are computing how far a value *actually* escapes.
Do not change AIMS's lattice ordering to match OxCaml's.

**R4: The graded modal semiring formalization.**

The paper's Section 5 constructs a 7-element (extended to 10-element) semiring that
models the combined affinity-uniqueness space. This is theoretically elegant and
important for the soundness proof, but AIMS does not need it. AIMS's product lattice
with componentwise join already provides the same expressiveness. The semiring structure
matters for type-system-level soundness; AIMS operates at the IR level where soundness
is validated by the verify pass, not by type rules. Adopting the semiring formalization
would add complexity without actionable benefit.

**R5: Weak mode polymorphism / mode-polymorphic functions.**

OxCaml's Section 6.6 discusses "weakly polymorphic" modes where functions get different
mode instantiations at different call sites. This is a type-system concern. AIMS already
handles this through interprocedural contracts: `MemoryContract` per function with
`ParamContract` per parameter. The SCC-based fixpoint already computes the most
permissive contract that is sound across all callers. No additional polymorphism mechanism
is needed at the IR level.

**R6: Currying-aware mode propagation.**

Section 6.4 discusses complications from currying: a partially applied `f @ local` captures
local values, so the partial application itself must be local. Ori does not have implicit
currying --- `PartialApply` in ARC IR is an explicit construct. AIMS already handles
partial application via `transfer_construct` (CtorKind::Closure). The currying-specific
complications are OCaml-specific.

---

## 01.4 Plan Edits

### `plans/aims/section-09-dimensional-fusion.md`

**Section 09.2 Locality Activation:**
- Add explicit note that OxCaml's `global` modality forces BOTH `aliased` AND `global`
  simultaneously. The current plan treats locality-to-uniqueness interaction as one rule
  (`BlockLocal + Owned + <=Once -> Unique`). It should also include the *reverse*
  direction: `HeapEscaping -> ceiling on Uniqueness`. The current Rule 6 in Section 09.3
  (`HeapEscaping + Borrowed -> force MaybeShared`) is too narrow; extend it to cover
  owned values stored into heap structures.
  <!-- reviewed: completeness fix — Rule 6 strengthening conflicts with Section 02 (FP2)
  which wants Rule 7 moved OUT of canonicalize to contract extraction. Both sections
  propose expanding 09.3 rules, but in different directions. Section 01 wants a broader
  Rule 6; Section 02 wants FIP logic removed from canonicalize. These are compatible
  (broadening Rule 6 and moving FIP to contract extraction can coexist). -->

- Add locality propagation through `Project` as an explicit rule alongside the existing
  uniqueness propagation rule that is already implemented. Reference OxCaml's deepness
  property.
  <!-- reviewed: completeness fix — Section 09.1 already has "Unique source projection
  preserves uniqueness" as an [x] implemented rule. This proposes a parallel locality
  rule. The target location (09.1 or 09.2) should be specified: locality propagation
  through Project is a transfer fusion rule (09.1), not an active dimension item (09.2).
  -->

- Add closure-capture-aware locality: when a value is captured by a closure, its locality
  must be at least `FunctionLocal` (it escapes the block where it was defined, into the
  closure's scope). If the closure itself escapes, the captured value's locality becomes
  `HeapEscaping`.
  <!-- reviewed: completeness fix — Already partially present in AIMS plan. Section 01.5
  (transfer functions) already documents: "PartialApply ... Captured args' locality
  promoted to HeapEscaping (closure may outlive the defining function)." This edit refines
  it by distinguishing FunctionLocal (non-escaping closure) from HeapEscaping (escaping
  closure). The existing transfer function is more conservative (always HeapEscaping).
  -->

**Section 09.3 Enriched Canonicalize:**
- Strengthen Rule 6 from `HeapEscaping + Borrowed -> MaybeShared` to:
  `HeapEscaping -> uniqueness >= MaybeShared` (regardless of access class), with the
  exception that `HeapEscaping + Unique` is valid when the containing structure is itself
  provably Unique. This requires checking the `BorrowSource` or containing structure's
  state.
  <!-- reviewed: completeness fix — The exception clause ("unless containing structure is
  provably Unique") requires canonicalize to inspect BorrowSource, which is a side table,
  not a lattice dimension. This violates the stated constraint that "canonicalize() must be
  a pure function on AimsState" (Section 01.1, line 188). Either the exception must be
  handled in transfer functions (not canonicalize), or the constraint must be relaxed.
  Cross-dependency: Section 04 (TRMC) also proposes strengthening ContextHole requirements
  in 09.2, which interacts with this Rule 6 extension. -->

- Add new Rule 9: `Borrowed -> locality <= FunctionLocal`. If canonicalize finds
  `Borrowed + HeapEscaping`, force `locality = FunctionLocal` (borrows cannot escape to
  the heap by definition). Alternatively, this could be treated as a verify-time error
  rather than a canonicalize rule.
  <!-- reviewed: completeness fix — This is a sound rule. Borrows are temporary views that
  cannot outlive their defining scope, so HeapEscaping locality is indeed contradictory for
  a Borrowed value. Recommend canonicalize (not verify-time) since it is derivable purely
  from state fields. Chain height analysis: forces locality DOWN (HeapEscaping -> at most
  FunctionLocal), which is a tightening (toward bottom). No interaction with Rule 4 or Rule
  6 because those require specific access/locality combinations that Rule 9 would prevent.
  -->

**Section 09.1 Transfer Fusion:**
- Add closure-capture transfer rule: when processing `Construct` with `CtorKind::Closure`,
  each captured variable's locality should be widened to at least `FunctionLocal` (the
  value now lives in the closure, which may outlive the block). If the closure itself is
  determined to be `once` (from its consumption context), captured values preserve
  uniqueness.
  <!-- reviewed: completeness fix — This should target PartialApply, not Construct with
  CtorKind::Closure. In ARC IR, closures are created via PartialApply (not Construct).
  The existing transfer function in Section 01.5 already handles PartialApply with
  HeapEscaping. This edit refines it to FunctionLocal when the closure doesn't escape.
  -->

### `plans/aims/00-overview.md`

**Research Lineage table:**
- The OxCaml entry should be expanded to note that the paper proves locality is NOT
  auxiliary --- it is load-bearing for both stack allocation and borrowing soundness.
  Current text says "Justifies AIMS Locality dimension" which undersells the finding.
  The paper proves locality interacts with uniqueness (global modality forces aliased),
  which means AIMS's current treatment of locality as "auxiliary/conservative" is a
  deliberate deferral, not an architectural choice.
  <!-- reviewed: completeness fix — The existing research lineage entry already says:
  "Modal memory management: affinity, uniqueness, locality as mode axes; safe stack
  allocation and in-place update. Justifies AIMS Locality dimension." The proposed edit
  would expand this. The framing "deliberate deferral, not architectural choice" is
  accurate — Section 01.4a explicitly says "This dimension does not affect RC emission
  in v1 but provides the architectural foundation for future stack allocation hints."
  -->

### `plans/aims/section-02-intraprocedural.md`

- Note that locality should propagate through `Project` instructions (deepness), not just
  uniqueness. This is a transfer function change but affects the intraprocedural analysis
  state.
  <!-- reviewed: completeness fix — The target location is transfer/mod.rs (Section 01.5),
  not intraprocedural analysis. The intraprocedural analysis calls transfer functions; it
  does not define them. Cross-reference with Code Change C1 in 01.5 below. -->

### `plans/aims/section-03-interprocedural.md`

- Note that `ParamContract` will need `locality_bound` (already planned in Section 09.2)
  and that this is not just an optimization hint but a soundness requirement for the
  `HeapEscaping -> not Unique` invariant. If a callee stores a parameter into a heap
  structure, the caller must know, because the caller's uniqueness reasoning depends on it.
  <!-- reviewed: completeness fix — ALREADY PRESENT in AIMS plan. ParamContract already
  has locality_bound (Section 03.1, line 109: "pub locality_bound: Locality"). Section
  09.2 already has a sync point note listing all 5 locations to update. The 00-overview
  cross-section interactions (line 295) also documents this. What is NEW here is the
  soundness argument: locality_bound is not just an optimization hint but is required for
  the HeapEscaping->not-Unique invariant. This framing should be added to Section 09.2's
  sync point note. -->

---

## 01.5 Code Changes (Later)

These are implementation items for after the full literature review is complete.
Each references specific files in `compiler/ori_arc/src/aims/`.

**C1: Add locality propagation to `transfer_project` in `transfer/mod.rs`.**

Currently `transfer_project` sets `uniqueness: source.uniqueness` but does not propagate
locality. Add `locality: source.locality` (or `min(source.locality, FunctionLocal)` if
projecting from a different function's return value). Requires reading the source
variable's current state during transfer.

**C2: Strengthen `canonicalize()` in `lattice/mod.rs` with two new rules.**

Rule: `HeapEscaping + Unique -> MaybeShared` (unless containing structure is provably
Unique). This is a generalization of the planned Rule 6.

Rule: `Borrowed + HeapEscaping -> force locality = FunctionLocal` (or flag as error).

**C3: Add closure-capture-aware transfer in `transfer/mod.rs`.**

When `transfer_construct` processes `CtorKind::Closure`, each captured variable should
have its locality widened to at least `FunctionLocal`. This interacts with the existing
`PartialApply` handling.

**C4: Add `locality_bound` to `ParamContract` in `contract.rs`.**

Already planned in Section 09.2. The OxCaml review confirms this is a soundness
requirement, not just an optimization. The `extract_contract` function in
`interprocedural.rs` must compute `locality_bound` from the converged state map.

**C5: Verify `BorrowSource` tracking is complete for locality reasoning.**

The `BorrowSource` side table in `lattice/mod.rs` tracks where borrowed values come from.
For the `HeapEscaping -> not Unique` invariant to work correctly, we need to know whether
a value stored in a heap structure is reachable only through a unique path. This may
require extending `BorrowSource` or adding a parallel "containment source" tracker.

---

## 01.6 Lens Shift

### How to read Paper 02 (FP2) differently

FP2 (Lorenzen et al., ICFP 2023) introduces "reuse credits" as first-class lattice
elements for FIP certification. After reading OxCaml:

1. **FP2's reuse credits should be interpreted through the lens of mode decomposition.**
   OxCaml shows that uniqueness and affinity are separate axes with a formal product
   structure. FP2's reuse credits conflate "the value is unique" (past) with "the value
   will be consumed" (future). When reading FP2, track which reuse credit operations
   depend on uniqueness and which depend on consumption/affinity. AIMS already separates
   these into `Uniqueness` and `Consumption` dimensions, which is the right factoring.

2. **FP2's FIP certification should be re-read as a property of the converged mode state.**
   OxCaml demonstrates that memory properties (stack allocation eligibility, in-place
   update safety) are derivable from the converged mode lattice. FP2 has a separate FIP
   certification pass. AIMS Section 09.2 Effect Activation already plans to make FIP
   certification a read of the converged state. OxCaml validates this direction: if modes
   are correctly decomposed and inference is complete, FIP properties fall out
   automatically.

3. **Watch for where FP2 needs locality but does not model it.** OxCaml proves locality
   is necessary for safe borrowing (the `borrow` combinator requires `local` mode to be
   sound). FP2 does not have a locality axis. Where FP2 assumes values don't escape (e.g.,
   for reuse credit tracking), that is an implicit locality assumption that AIMS should
   make explicit.

### How to read subsequent papers differently

- **Paper 03 (FIPTree):** Read through the "global modality forces aliased" lens.
  Constructor contexts that escape to the heap lose uniqueness. AIMS's `ContextHole`
  shape class should interact with locality.

- **Paper 06 (Marshall et al., Linearity != Uniqueness):** OxCaml is a direct application
  of this paper's thesis. AIMS's separation of `Consumption` (linearity/affinity) from
  `Uniqueness` is validated. Read Paper 06 for the proof obligations, not the design ---
  the design is already embodied in AIMS.

- **Paper 08 (Lean 4 Borrow):** Read with attention to how Lean 4 handles locality
  implicitly. Lean 4's borrow inference does not have an explicit locality dimension
  but achieves similar results through scope-limited borrowing. Compare Lean 4's implicit
  locality reasoning to AIMS's explicit `Locality` dimension.

- **Paper 09 (GHC Demand):** GHC's cardinality analysis {Absent, Once, Many} maps
  directly to AIMS's `Cardinality` dimension. OxCaml confirms that cardinality
  (demand/usage count) is orthogonal to uniqueness and locality. Read GHC demand analysis
  for sequential composition (`seq_add`) and alternative join (`alt_join`) patterns, which
  AIMS already uses.

---

## 01.7 Open Risk

**Risk 1: AIMS Locality is genuinely too passive and Stage 2 may not fix it.**

OxCaml demonstrates that locality is load-bearing for soundness (borrowing requires it)
and for the primary optimization target (stack allocation). AIMS currently defaults
Locality to `Unknown` for all values in Stage 1. Section 09.2 plans to activate it, but
the plan treats locality activation as an optimization (reducing RC operations) rather
than as a soundness requirement. If AIMS ever adds borrowing-dependent optimizations
(e.g., RC-skip for function-local linear values, as planned in Section 09.2), the
correctness of those optimizations depends on locality being precise, not just
conservative. A conservative locality (everything is `Unknown`) is safe but forecloses
the optimization. A *wrong* locality (claiming `FunctionLocal` when the value actually
escapes) would be unsound. The risk is that making locality precise is harder than
anticipated and blocks the entire Stage 2 optimization payoff.

**Risk 2: The global-forces-aliased invariant exposes a gap in canonicalize.**

AIMS Section 09.3 plans 8 canonicalize rules. None of them currently encode the
OxCaml-proven invariant that heap-escaping values lose uniqueness guarantees (Rule 6
covers only borrowed values). If this invariant is not added, AIMS could incorrectly
assume a heap-stored value is unique, leading to unsound in-place reuse. The risk is
that the invariant is more complex than a simple canonicalize rule: whether
`HeapEscaping + Unique` is valid depends on the uniqueness of the *containing* structure,
which requires inter-variable reasoning that canonicalize (which operates on a single
`AimsState`) cannot express.

**Risk 3: Closure capture is a blind spot in AIMS's current transfer functions.**

OxCaml's LAM rule with the lock mechanism is the primary way modes propagate through
closures. AIMS's `transfer_construct` for closures currently produces `AimsState::FRESH`
for the closure itself, which is correct for the closure as an allocation. But it does
NOT adjust the captured variables' states. A unique value captured by a many-times closure
is no longer unique --- AIMS does not encode this. This is not a future optimization; it
is a correctness gap. If AIMS ever optimizes based on a captured variable's uniqueness
without accounting for how many times the capturing closure is invoked, the result would
be unsound.

The mitigation is already partially in place: AIMS's backward analysis propagates
`Cardinality::Many` through loop-carried uses, which covers closures invoked in loops.
But a closure stored in a data structure and invoked multiple times at different call
sites may not be detected as `Many` by the intraprocedural analysis alone.

**Risk 4: AIMS has no mode inference --- it has mode analysis.**

OxCaml performs forward mode inference during type checking, generating constraints that
are solved globally. AIMS performs backward mode analysis on the IR, computing a fixed
point. These are fundamentally different approaches:

- OxCaml's inference works on pre-RC-insertion code and informs allocation decisions.
- AIMS's analysis works on post-lowering IR and informs RC insertion and reuse decisions.

The risk is that by the time AIMS runs, the IR has already committed to certain allocation
patterns (heap-allocated closures, boxed constructors) that a forward mode inference
could have avoided. AIMS cannot undo heap allocations --- it can only optimize the RC
operations around them. OxCaml's 90% allocation reduction comes from *avoiding*
allocations entirely (stack allocation), which AIMS cannot achieve at the IR level.

This is an architectural limitation, not a bug. But it means AIMS's Locality dimension,
even when fully activated, cannot deliver the same class of optimization that OxCaml's
locality mode delivers. AIMS can use locality to skip RC operations and enable reuse,
but it cannot use locality to stack-allocate values. Stack allocation would require a
pre-lowering pass or changes to the ARC IR itself.

**Risk 5: ParamContract without locality_bound is unsound for RC-skip.**

Section 09.2 plans `FunctionLocal + Owned + Linear -> RC-skip eligible`. This optimization
skips RcInc at entry and RcDec at last use for function-local linear values. The soundness
of this optimization depends on the callee not escaping the value. If `ParamContract` does
not carry a `locality_bound`, the caller has no way to know whether the callee will escape
the value. The SCC fixpoint must converge on locality bounds before RC-skip can be enabled.
Until `locality_bound` is in `ParamContract`, RC-skip is blocked --- not deferred, blocked.
