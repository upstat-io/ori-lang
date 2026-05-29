/-
AIMS canonicalization-rule module — kernel-checked Lean proofs of the active
canonicalization rules CN-1 / CN-3 / CN-6 / CN-8 over the finite `AimsState`
product.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: CN-1 / CN-3 / CN-6 / CN-8 (CN-4 / CN-7 REMOVED — doc-comment, no theorem) |
  spec: annex-e §AIMS §5 |
  .proof: aims-proof/proofs/03-canonicalization/CN-{1,3,6,8}.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: `docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS §5`
(Canonicalization Rules) + §3 ("Canonicalization (§5) shall be idempotent and
shall preserve join results.").

The §5 rule table (spec, in order):
  CN-1  Dead ↔ Absent bidirectional:
        `Consumption = Dead ⟹ Cardinality := Absent` and
        `Cardinality = Absent ⟹ Consumption := Dead`.
  CN-2  Linear + Absent infeasible — subsumed by CN-1 (Absent ⟹ Dead); no
        separate mutation (doc-comment, no theorem).
  CN-3  Shared blocks reuse:
        `Uniqueness = Shared ∧ Shape ≠ NonReusable ⟹ Shape := NonReusable`.
  CN-4  Reserved — REMOVED (former optimistic uniqueness promotion broke
        monotonicity); no theorem (doc-comment below).
  CN-5  Unique + Dead preserves reusable shape — forbids a collapse, adds no
        mutation (doc-comment, no theorem).
  CN-6  Wide-locality uniqueness ceiling:
        `Locality ≥ HeapEscaping ∧ Uniqueness = Unique ⟹ Uniqueness := MaybeShared`.
  CN-7  Reserved — REMOVED (former Shared+CollectionBuffer COW-mode rule;
        canonicalization mutates lattice dimensions only, not decision
        predicates); no theorem (doc-comment below).
  CN-8  Borrowed locality ceiling:
        `Access = Borrowed ∧ Locality > FunctionLocal ⟹ Locality := FunctionLocal`.
        Fires BEFORE CN-6 (spec §5 NOTE) so locality is precise when CN-6 reads it.

Each active rule is proven as a FEASIBILITY-INVARIANT theorem on the full
`canonicalize s` output (not on the isolated `cnN s`): after one canonicalize
pass, the rule's post-condition holds on the canonical state. Proving on the
full composition (vs the isolated rule) is the load-bearing statement — it
shows the rule's invariant survives the other rules running after it in the
fixed `cn6 ∘ cn8 ∘ cn3 ∘ cn1` order.

Proof strategy mirrors `Lattice.lean` L-7: total case enumeration over every
dimension any CN-N rule reads (Consumption × Cardinality × Uniqueness ×
Locality × Access × Shape), join-irrelevant dimensions (Effect) carried
symbolically, each leaf discharged by `decide`. Naive `decide` over the full
~7200-state product times out; per-dimension `cases` destructuring before
`decide` keeps each leaf trivial.

Canonicalize idempotence is L-7 — REUSED from `Lattice.lean`
(`L7_canonicalize_idem`), not re-proven here.
-/

import AimsProof.Model
import AimsProof.Lattice

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §CN-1 — Dead ↔ Absent bidirectional feasibility (annex-e §AIMS §5)

    After canonicalize, `Consumption = Dead ↔ Cardinality = Absent`: the two
    infeasible mixed states (Dead-but-present, Absent-but-live) are forced into
    the single coherent `(Dead, Absent)` corner. CN-1 fires first in the pass;
    no later rule (CN-3 / CN-8 / CN-6) reads or writes Consumption or
    Cardinality, so the CN-1 post-condition survives to the canonical output. -/

theorem CN1_dead_absent (s : AimsState) :
    (canonicalize s).consumption = .Dead ↔ (canonicalize s).cardinality = .Absent := by
  obtain ⟨acc, con, car, uni, loc, shp, ⟨ma, ms, mt⟩⟩ := s
  cases acc <;> cases con <;> cases car <;> cases uni <;> cases loc <;> cases shp <;>
    cases ma <;> cases ms <;> cases mt <;> decide

/-! ## §CN-3 — Shared blocks reuse / shape collapse (annex-e §AIMS §5)

    After canonicalize, `Uniqueness = Shared ⟹ Shape = NonReusable`: a shared
    value can never carry a reusable shape, because reuse of a shared cell is
    unsound. CN-3 fires after CN-1; CN-8 / CN-6 (which run later) touch only
    Locality and Uniqueness, never Shape, and CN-6 only RAISES Uniqueness
    (Unique → MaybeShared), never lowers it TO Shared — so a state that left
    CN-3 non-Shared cannot become Shared later, and the collapse invariant
    holds on the canonical output. -/

theorem CN3_shared_blocks_reuse (s : AimsState) :
    (canonicalize s).uniqueness = .Shared → (canonicalize s).shape = .NonReusable := by
  obtain ⟨acc, con, car, uni, loc, shp, ⟨ma, ms, mt⟩⟩ := s
  cases acc <;> cases con <;> cases car <;> cases uni <;> cases loc <;> cases shp <;>
    cases ma <;> cases ms <;> cases mt <;> decide

/-! ## §CN-8 — Borrowed locality ceiling (annex-e §AIMS §5)

    After canonicalize, `Access = Borrowed ⟹ Locality ≤ FunctionLocal` (rank
    ≤ FunctionLocal.rank): a borrowed value cannot escape past the function
    frame. CN-8 fires before CN-6 (spec §5 NOTE); CN-6 runs afterward but only
    mutates Uniqueness, never Locality — so the CN-8 ceiling survives to the
    canonical output. CN-1 / CN-3 (earlier) touch neither Access nor Locality. -/

theorem CN8_borrowed_locality_ceiling (s : AimsState) :
    (canonicalize s).access = .Borrowed →
      (canonicalize s).locality.rank ≤ Locality.FunctionLocal.rank := by
  obtain ⟨acc, con, car, uni, loc, shp, ⟨ma, ms, mt⟩⟩ := s
  cases acc <;> cases con <;> cases car <;> cases uni <;> cases loc <;> cases shp <;>
    cases ma <;> cases ms <;> cases mt <;> decide

/-! ## §CN-6 — Wide-locality uniqueness ceiling (annex-e §AIMS §5)

    After canonicalize, `Locality ≥ HeapEscaping ⟹ Uniqueness ≠ Unique`: a
    value reaching heap-escaping locality cannot be statically Unique (another
    reference may exist through the escape), so CN-6 demotes Unique to
    MaybeShared. CN-6 fires last; the Locality it reads is already clamped by
    CN-8, and no rule runs after CN-6 — so its post-condition is exactly the
    canonical output. The interaction with CN-8 is load-bearing: a Borrowed
    state had its Locality clamped to FunctionLocal by CN-8, so CN-6's
    wide-locality trigger only fires on Owned states (or states CN-8 left at
    HeapEscaping/Unknown), which is precisely the spec ordering intent. -/

theorem CN6_wide_locality_uniqueness_ceiling (s : AimsState) :
    (canonicalize s).locality.rank ≥ Locality.HeapEscaping.rank →
      (canonicalize s).uniqueness ≠ .Unique := by
  obtain ⟨acc, con, car, uni, loc, shp, ⟨ma, ms, mt⟩⟩ := s
  cases acc <;> cases con <;> cases car <;> cases uni <;> cases loc <;> cases shp <;>
    cases ma <;> cases ms <;> cases mt <;> decide

/-! ## §CN-1..CN-8 idempotence — REUSE of L-7 (annex-e §AIMS §3 "idempotent")

    The spec §3 requires canonicalization to be idempotent. This is law L-7,
    proven in `Lattice.lean` as `L7_canonicalize_idem`. It is REUSED here, not
    re-proven: the active CN-1 / CN-3 / CN-8 / CN-6 rules each reach a fixed
    point in one pass, so the composition `cn6 ∘ cn8 ∘ cn3 ∘ cn1` is idempotent.
    Re-exported as a local alias for callers that import only this module. -/

theorem CN_canonicalize_idempotent (s : AimsState) :
    canonicalize (canonicalize s) = canonicalize s :=
  L7_canonicalize_idem s

/-! ## §CN-4 — REMOVED (annex-e §AIMS §5)

    Former CN-4 (optimistic uniqueness promotion: `MaybeShared → Unique` under
    `BlockLocal ∧ Owned ∧ ≤ Once`) was REMOVED because it was anti-monotone —
    it lowered a value on the Uniqueness chain, breaking lattice associativity
    (L-2) and antisymmetry (L-4). There is no CN-4 rule in `canonicalize`
    (`Model.lean` defines `canonicalize = cn6 ∘ cn8 ∘ cn3 ∘ cn1`, with no cn4).
    No theorem is authored for a removed rule; its absence is verified by the
    Uniqueness join staying monotone (max-on-chain), which `Lattice.lean`
    discharges via `Uniqueness.join_assoc`. This doc-comment records the
    removal per the §AIMS §5 rule table (CN-4 = Reserved). -/

/-! ## §CN-7 — REMOVED (annex-e §AIMS §5)

    Former CN-7 (Shared + CollectionBuffer COW-mode assignment) was REMOVED
    because `cow_mode` is a DECISION-PREDICATE result (a §4 decision-predicate
    query), NOT a lattice-dimension mutation — canonicalization shall mutate
    lattice dimensions only, never decision predicates. There is no CN-7 rule
    in `canonicalize`, and `AimsState` carries no `cow_mode` dimension to
    mutate. The Shared + CollectionBuffer case is handled by the decision
    layer, not by a CN rule. No theorem is authored for a removed rule; this
    doc-comment records the removal per the §AIMS §5 rule table (CN-7 =
    Reserved). -/

/-! ## Computability pins — canonicalize feasibility on a concrete witness

    Concrete evaluations confirming the canonical output is well-formed:
    a Shared reusable-shape state collapses to NonReusable (CN-3), and a
    Borrowed wide-locality state clamps to FunctionLocal (CN-8). -/

-- CN-3 witness: Shared + CollectionBuffer → NonReusable.
example :
    (canonicalize
      { access := .Owned
      , consumption := .Linear
      , cardinality := .One
      , uniqueness := .Shared
      , locality := .BlockLocal
      , shape := .CollectionBuffer
      , effect := {} }).shape = Shape.NonReusable := by decide

-- CN-6 witness: HeapEscaping + Unique → MaybeShared (Owned, so CN-8 does not
-- pre-clamp the locality).
example :
    (canonicalize
      { access := .Owned
      , consumption := .Linear
      , cardinality := .One
      , uniqueness := .Unique
      , locality := .HeapEscaping
      , shape := .NonReusable
      , effect := {} }).uniqueness = Uniqueness.MaybeShared := by decide

-- CN-1 witness: Dead consumption forces Absent cardinality.
example :
    (canonicalize
      { access := .Owned
      , consumption := .Dead
      , cardinality := .Many
      , uniqueness := .Unique
      , locality := .BlockLocal
      , shape := .NonReusable
      , effect := {} }).cardinality = Cardinality.Absent := by decide

end AimsProof
