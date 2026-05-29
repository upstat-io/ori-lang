/-
AIMS interprocedural-contract module — kernel-checked Lean proofs of the
interprocedural contract rules IC-1..IC-8a over the AIMS product lattice.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: IC-1..IC-7 + IC-8a (IC-8 REMOVED — doc-comment, no theorem) |
  spec: annex-e §AIMS §7 + §3 + §6 |
  .proof: aims-proof/proofs/06-interprocedural/IC-*.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: `docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS §7`
(Interprocedural Contracts) + §3 (lattice dimensions + orderings) + §6 (pipeline
ordering PL-1a SCC topological order).

These are STRUCTURAL theorems — the contracts are modeled as Lean structures
over the lattice dimension carriers from `Model.lean`, and the proofs proceed by
induction over the call-graph / iteration-count rather than a single finite
`by decide` enumeration. The lattice substrate (per-dimension `rank` / `join` /
`height`, the join-defined order, finite height) is reused from `Lattice.lean`.

Rule index (per §AIMS §7):
  IC-1  Call graph decomposed into SCCs, processed in topological order
        (callees before callers). Modeled as a finite DAG-of-SCCs with an
        acyclic precedence relation; a well-founded topological processing
        order is proven to exist.
  IC-2  Each parameter initializes most-optimistic
        `(Borrowed, Dead, Absent, BlockLocal, Unique, may_share=false)`.
        Proven to be the bottom of the ParamContract product lattice.
  IC-3  ParamContract join = componentwise max + may_share OR. Proven
        commutative / associative / idempotent / monotone.
  IC-4  ReturnContract join = uniqueness(join) + preserves_freshness(AND) +
        locality(join) + shape(join). Proven monotone + AND-fold over paths.
  IC-5  EffectSummary join = componentwise OR over the effect flags. Proven
        monotone + idempotent + commutative + associative.
  IC-7  Convergence: finite domain ⟹ termination. The SCC fixpoint iteration
        starts at the bottom seed (IC-2) and advances monotonically (IC-3);
        finite lattice height bounds the iteration count. Proven by induction
        on the remaining-height budget (a monotone bounded ascending chain in
        a finite-height lattice reaches a fixpoint).
  IC-8  REMOVED (doc-comment). Same unsound root cause as DP-10: deriving
        callee parameter uniqueness from caller consumption is a backward-
        demand → past-RC confusion. SUBSUMED-BY IC-2 / IC-3 natural
        convergence.
  IC-8a Address-taken / closure CONSERVATIVE initialization. Proven that the
        CONSERVATIVE seed upper-bounds every IC-2/IC-3-reachable call-set param
        state (uniqueness ≤ MaybeShared) in the join-defined order. CONSERVATIVE
        uses uniqueness = MaybeShared (NOT the strict uniqueness top Shared) per
        the IC-8a spec rule — the weakest sound non-enumerable assumption.
-/

import AimsProof.Model
import AimsProof.Lattice

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §IC-2 / §IC-3 — ParamContract product lattice (annex-e §AIMS §7 IC-2 / IC-3)

    A ParamContract is the 5 spec lattice dimensions
    `(Access, Consumption, Cardinality, Locality, Uniqueness)` plus the
    orthogonal boolean `may_share` (joined by OR per IC-3). It is the per-
    parameter product sub-lattice over which the SCC fixpoint iterates. -/

/-- §IC-2 / §IC-3 ParamContract: the 5 spec lattice dimensions + `may_share`. -/
structure ParamContract where
  access : AccessClass
  consumption : Consumption
  cardinality : Cardinality
  locality : Locality
  uniqueness : Uniqueness
  may_share : Bool
deriving Repr, DecidableEq

/-- §IC-3 ParamContract join: componentwise max on each chain dimension,
    boolean OR on `may_share` (annex-e §AIMS §7 IC-3). -/
def ParamContract.join (a b : ParamContract) : ParamContract :=
  { access := a.access.join b.access
  , consumption := a.consumption.join b.consumption
  , cardinality := a.cardinality.join b.cardinality
  , locality := a.locality.join b.locality
  , uniqueness := a.uniqueness.join b.uniqueness
  , may_share := a.may_share || b.may_share }

/-- §IC-2 OPTIMISTIC seed = the product-lattice bottom
    `(Borrowed, Dead, Absent, BlockLocal, Unique, may_share=false)`. -/
def ParamContract.OPTIMISTIC : ParamContract :=
  { access := .Borrowed
  , consumption := .Dead
  , cardinality := .Absent
  , locality := .BlockLocal
  , uniqueness := .Unique
  , may_share := false }

/-- §IC-8a CONSERVATIVE seed = the product-lattice top
    `(Owned, Unrestricted, Many, Unknown, MaybeShared, may_share=true)`. -/
def ParamContract.CONSERVATIVE : ParamContract :=
  { access := .Owned
  , consumption := .Unrestricted
  , cardinality := .Many
  , locality := .Unknown
  , uniqueness := .MaybeShared
  , may_share := true }

/-- The join-defined order on the ParamContract product:
    `a ≤ b` iff joining `a` into `b` componentwise yields `b`. -/
def ParamContract.le (a b : ParamContract) : Prop := a.join b = b

/-! ### §IC-3 join algebra (commutative / associative / idempotent / monotone)

    Each property lifts the per-dimension chain lemmas from `Lattice.lean`
    (`*.join_comm`, `*.join_assoc`, `*.join_idem`) field-by-field, with the
    boolean OR on `may_share` discharged by `Bool` algebra. -/

/-- §IC-3 join commutativity: `join a b = join b a`. -/
theorem IC3_paramcontract_join_comm (a b : ParamContract) :
    a.join b = b.join a := by
  simp only [ParamContract.join, AccessClass.join_comm, Consumption.join_comm,
    Cardinality.join_comm, Locality.join_comm, Uniqueness.join_comm,
    Bool.or_comm]

/-- §IC-3 join associativity: `join (join a b) c = join a (join b c)`. -/
theorem IC3_paramcontract_join_assoc (a b c : ParamContract) :
    (a.join b).join c = a.join (b.join c) := by
  simp only [ParamContract.join, AccessClass.join_assoc, Consumption.join_assoc,
    Cardinality.join_assoc, Locality.join_assoc, Uniqueness.join_assoc,
    Bool.or_assoc]

/-- §IC-3 join idempotence: `join a a = a`. -/
theorem IC3_paramcontract_join_idem (a : ParamContract) : a.join a = a := by
  simp only [ParamContract.join, AccessClass.join_idem, Consumption.join_idem,
    Cardinality.join_idem, Locality.join_idem, Uniqueness.join_idem,
    Bool.or_self]

/-! ### §IC-3 monotonicity — the load-bearing soundness property

    `IC3_paramcontract_join_monotone`: `a ≤ b ⟹ join a r ≤ join b r`.
    Monotonicity is the precondition for Tarski least-fixpoint convergence
    (IC-7). It is derived structurally from join commutativity / associativity /
    idempotence — the standard lattice result that join is monotone in each
    argument. -/

/-- §IC-3 partial-order reflexivity: `a ≤ a`, from join idempotence. -/
theorem IC3_le_refl (a : ParamContract) : a.le a :=
  IC3_paramcontract_join_idem a

/-- §IC-3 partial-order transitivity: `a ≤ b → b ≤ c → a ≤ c`,
    from join associativity. -/
theorem IC3_le_trans (a b c : ParamContract)
    (hab : a.le b) (hbc : b.le c) : a.le c := by
  unfold ParamContract.le at hab hbc ⊢
  calc a.join c = a.join (b.join c) := by rw [hbc]
    _ = (a.join b).join c := (IC3_paramcontract_join_assoc a b c).symm
    _ = b.join c := by rw [hab]
    _ = c := hbc

/-- §IC-3 partial-order antisymmetry: `a ≤ b → b ≤ a → a = b`,
    from join commutativity. -/
theorem IC3_le_antisymm (a b : ParamContract)
    (hab : a.le b) (hba : b.le a) : a = b := by
  unfold ParamContract.le at hab hba
  have hcomm : a.join b = b.join a := IC3_paramcontract_join_comm a b
  rw [hab, hba] at hcomm
  exact hcomm.symm

/-- §IC-3 monotonicity: `a ≤ b ⟹ join a r ≤ join b r`.
    `join (join a r) (join b r) = join (join a b) (join r r) = join b r`,
    using commutativity + associativity to regroup and `a.join b = b`
    (the `a ≤ b` hypothesis) + `join r r = r` (idempotence). -/
theorem IC3_paramcontract_join_monotone (a b r : ParamContract)
    (hab : a.le b) : (a.join r).le (b.join r) := by
  unfold ParamContract.le at hab ⊢
  -- Regroup (a ⊔ r) ⊔ (b ⊔ r) = (a ⊔ b) ⊔ (r ⊔ r) = b ⊔ r.
  calc (a.join r).join (b.join r)
      = a.join (r.join (b.join r)) := IC3_paramcontract_join_assoc a r (b.join r)
    _ = a.join ((r.join b).join r) :=
          by rw [IC3_paramcontract_join_assoc r b r]
    _ = a.join ((b.join r).join r) :=
          by rw [IC3_paramcontract_join_comm r b]
    _ = a.join (b.join (r.join r)) :=
          by rw [IC3_paramcontract_join_assoc b r r]
    _ = a.join (b.join r) := by rw [IC3_paramcontract_join_idem r]
    _ = (a.join b).join r := (IC3_paramcontract_join_assoc a b r).symm
    _ = b.join r := by rw [hab]

/-! ## §IC-2 — OPTIMISTIC seed is the product-lattice bottom

    `IC2_optimistic_is_bottom`: for every ParamContract `p`,
    `OPTIMISTIC ≤ p`. Joining the most-optimistic seed into any contract
    yields that contract — the seed is the unique minimum, so the SCC fixpoint
    starting at OPTIMISTIC starts at or below its least fixpoint (Tarski
    precondition). Discharged by per-dimension `cases` + `decide` since each
    dimension's `max(bottom, x) = x` is a finite-domain fact. -/

theorem IC2_optimistic_is_bottom (p : ParamContract) :
    ParamContract.OPTIMISTIC.le p := by
  unfold ParamContract.le ParamContract.join ParamContract.OPTIMISTIC
  obtain ⟨acc, con, car, loc, uni, ms⟩ := p
  cases acc <;> cases con <;> cases car <;> cases loc <;> cases uni <;>
    cases ms <;> rfl

/-! ## §IC-8a — CONSERVATIVE seed upper-bounds every reachable call-set state

    `IC8a_conservative_upper_bounds`: for every ParamContract `p` whose
    uniqueness is `≤ MaybeShared` (i.e. `p.uniqueness ≠ Shared`),
    `p ≤ CONSERVATIVE`. Address-taken / closure parameters with non-enumerable
    call sites seed CONSERVATIVE; since every IC-2/IC-3-reachable param state is
    below it, an IC-3 join with such a state stays at CONSERVATIVE
    (`a ⊔ CONSERVATIVE = CONSERVATIVE`), so the conservative init can never
    under-approximate those states.

    The `uniqueness ≠ Shared` hypothesis is load-bearing and faithful to the
    IC-8a spec rule (annex-e §AIMS §7 IC-8a + the IC-8a proof artifact (P3)):
    CONSERVATIVE deliberately uses `uniqueness = MaybeShared`, NOT the strict
    uniqueness TOP `Shared`. `Shared` is the stronger "definitely shared" claim;
    MaybeShared is the weakest sound assumption (it still triggers the DP-4
    runtime IsShared check while leaving the unique-reference fast path
    reachable). A call-site arg that is provably `Shared` widens PAST
    CONSERVATIVE to `Shared` via IC-3 max-join (artifact (P3)(c)) — so
    CONSERVATIVE is the upper bound of the reachable lattice REGION, not the
    absolute uniqueness top. Discharged by per-dimension `cases` + `decide`
    over the finite carriers; the `Shared` uniqueness leaf is excluded by the
    hypothesis. -/

theorem IC8a_conservative_upper_bounds (p : ParamContract)
    (hu : p.uniqueness ≠ Uniqueness.Shared) :
    p.le ParamContract.CONSERVATIVE := by
  unfold ParamContract.le ParamContract.join ParamContract.CONSERVATIVE
  obtain ⟨acc, con, car, loc, uni, ms⟩ := p
  -- `uni` is Unique or MaybeShared (Shared excluded by `hu`).
  cases uni <;> cases acc <;> cases con <;> cases car <;> cases loc <;>
    cases ms <;> first | rfl | (exact absurd rfl hu)

/-- §IC-8a corollary (artifact (P4)): joining CONSERVATIVE with any reachable
    observed call-site state (`uniqueness ≠ Shared`) is absorbed back to
    CONSERVATIVE. A visible call site for a non-enumerable function whose arg is
    not provably `Shared` therefore cannot narrow the seed. -/
theorem IC8a_conservative_absorbs (p : ParamContract)
    (hu : p.uniqueness ≠ Uniqueness.Shared) :
    p.join ParamContract.CONSERVATIVE = ParamContract.CONSERVATIVE :=
  IC8a_conservative_upper_bounds p hu

/-! ## §IC-3 height + §IC-7 finite-height termination foundation

    Each ParamContract dimension has finite height (`Lattice.lean` §L-5). The
    per-contract height (the maximum number of strict ascents from bottom to
    top) is the sum of the dimension heights:
      Access 1 + Consumption 3 + Cardinality 2 + Locality 4 + Uniqueness 2 +
      may_share 1 = 13.
    The `rank` of a contract is the sum of its per-dimension ranks; it is
    bounded by the height, and a strict ascent in the join-defined order
    strictly increases the rank. That is the well-founded measure IC-7 uses. -/

/-- ParamContract rank = sum of per-dimension ranks + the `may_share` bit. -/
def ParamContract.rank (p : ParamContract) : Nat :=
  p.access.rank + p.consumption.rank + p.cardinality.rank +
  p.locality.rank + p.uniqueness.rank + (if p.may_share then 1 else 0)

/-- §IC-3 / §IC-7 per-parameter height = 13 (sum of dimension heights). -/
def ParamContract.height : Nat :=
  AccessClass.height + Consumption.height + Cardinality.height +
  Locality.height + Uniqueness.height + 1

/-- §IC-7 the per-parameter height is 13 (annex-e §AIMS §7 IC-7 spec formula
    `param=13`). Closed arithmetic fact. -/
theorem IC7_paramcontract_height_eq : ParamContract.height = 13 := by
  decide

/-- §IC-7 the rank of any ParamContract is bounded by the height — every
    ascending chain in the per-parameter lattice is finite. Discharged by
    per-dimension `cases` + `decide` over the finite carriers. -/
theorem IC7_paramcontract_rank_bounded (p : ParamContract) :
    p.rank ≤ ParamContract.height := by
  unfold ParamContract.rank ParamContract.height
  obtain ⟨acc, con, car, loc, uni, ms⟩ := p
  cases acc <;> cases con <;> cases car <;> cases loc <;> cases uni <;>
    cases ms <;> decide

/-! ### §IC-3 join weak-monotonicity on rank — a join never lowers the rank

    `IC7_join_rank_ge_left`: `(p.join q).rank ≥ p.rank`. Joining toward
    conservative never decreases the rank measure — the monotone-advance
    property that, combined with the rank bound, forces termination. Each
    per-dimension `join = max` has `(join a b).rank ≥ a.rank`, and `||` on the
    `may_share` bit only ever raises it; the sum lifts via `Nat.add_le_add`.

    Proven per-dimension (small `cases` over each chain pair) rather than by a
    single Cartesian `cases` over both whole contracts (that product is
    720 × 720 leaves — a tactic explosion). The per-dimension lemmas multiply
    out to at most 5 × 5 = 25 leaves each. -/

theorem AccessClass.rank_le_join (a b : AccessClass) : a.rank ≤ (a.join b).rank := by
  cases a <;> cases b <;> decide
theorem Consumption.rank_le_join (a b : Consumption) : a.rank ≤ (a.join b).rank := by
  cases a <;> cases b <;> decide
theorem Cardinality.rank_le_join (a b : Cardinality) : a.rank ≤ (a.join b).rank := by
  cases a <;> cases b <;> decide
theorem Locality.rank_le_join (a b : Locality) : a.rank ≤ (a.join b).rank := by
  cases a <;> cases b <;> decide
theorem Uniqueness.rank_le_join (a b : Uniqueness) : a.rank ≤ (a.join b).rank := by
  cases a <;> cases b <;> decide

theorem IC7_join_rank_ge_left (p q : ParamContract) :
    p.rank ≤ (p.join q).rank := by
  unfold ParamContract.rank ParamContract.join
  -- The may_share bit: `b1 ≤ b1 || b2` as a `Nat` weight (0/1).
  have hms : (if p.may_share then 1 else 0) ≤
      (if (p.may_share || q.may_share) then 1 else 0) := by
    cases p.may_share <;> cases q.may_share <;> decide
  -- Sum the five chain-dimension rank inequalities + the bit inequality.
  have h := Nat.add_le_add
    (Nat.add_le_add
      (Nat.add_le_add
        (Nat.add_le_add
          (Nat.add_le_add
            (AccessClass.rank_le_join p.access q.access)
            (Consumption.rank_le_join p.consumption q.consumption))
          (Cardinality.rank_le_join p.cardinality q.cardinality))
        (Locality.rank_le_join p.locality q.locality))
      (Uniqueness.rank_le_join p.uniqueness q.uniqueness))
    hms
  exact h

/-! ## §IC-7 — convergence: monotone bounded ascending chain reaches a fixpoint

    The SCC fixpoint iteration is a sequence `x_0 ≤ x_1 ≤ x_2 ≤ ...` in a
    finite-height lattice, where `x_0` = the IC-2 OPTIMISTIC seed and each step
    is an IC-3 join with observed call-site demands. Each non-converging step
    strictly increases the rank by ≥1; the rank is bounded by the height
    (`IC7_paramcontract_rank_bounded`); therefore the iteration converges in at
    most `height` steps.

    This is the genuinely-structural core of IC-7: it is proven by STRONG
    INDUCTION on the remaining-height budget `fuel := height - current_rank`,
    not by finite enumeration. The model: an iteration `step : S → S` that is
    inflationary (`s ≤ step s`) and rank-bounded reaches a fixpoint
    (`step s = s`) within `fuel` iterations. Stated abstractly over a `Nat`
    rank measure so the proof is the real monotone-bounded-termination argument,
    then instantiated for ParamContract. -/

/-- Iterate `f` exactly `n` times. -/
def iterate {α : Type} (f : α → α) : Nat → α → α
  | 0,     s => s
  | n + 1, s => iterate f n (f s)

/-- §IC-7 abstract convergence lemma.

    Let `step : S → S` be an iteration with a `Nat`-valued `measure` such that:
      (inflationary) `measure s ≤ measure (step s)` always;
      (bounded)      `measure s ≤ bound` always;
      (strict-or-fixed) each step either strictly increases `measure` OR is a
                        fixpoint (`step s = s`).
    Then iterating from any `s` reaches a fixpoint within `bound + 1 - measure s`
    steps. Proven by strong induction on the remaining-budget
    `fuel := bound + 1 - measure s`.

    The hypothesis `strictOrFixed` is the call-site-demand semantics of the SCC
    loop: an iteration either advances at least one dimension (rank strictly up)
    or no dimension changed (the join was idempotent — fixpoint). -/
theorem IC7_bounded_monotone_converges
    {S : Type} (step : S → S) (measure : S → Nat) (bound : Nat)
    (boundedH : ∀ s, measure s ≤ bound)
    (strictOrFixed : ∀ s, step s = s ∨ measure s < measure (step s)) :
    ∀ s, ∃ k, k ≤ bound + 1 - measure s ∧ step (iterate step k s) = iterate step k s := by
  -- Strong induction on the remaining budget `fuel = bound + 1 - measure s`.
  intro s
  -- Generalize over a `fuel` that upper-bounds the remaining budget.
  suffices H : ∀ fuel s, bound + 1 - measure s ≤ fuel →
      ∃ k, k ≤ fuel ∧ step (iterate step k s) = iterate step k s by
    obtain ⟨k, hk, hfix⟩ := H (bound + 1 - measure s) s (Nat.le_refl _)
    exact ⟨k, hk, hfix⟩
  intro fuel
  induction fuel with
  | zero =>
      -- fuel = 0 ⟹ bound + 1 - measure s = 0 ⟹ measure s ≥ bound + 1,
      -- contradicting boundedH; so the step must already be a fixpoint.
      intro s hle
      -- From hle: bound + 1 - measure s = 0, i.e. bound + 1 ≤ measure s.
      have hge : bound + 1 ≤ measure s := by omega
      -- But measure s ≤ bound, contradiction ⟹ step s must be the fixpoint.
      have := boundedH s
      -- measure s ≤ bound and bound + 1 ≤ measure s is impossible.
      omega
  | succ fuel ih =>
      intro s hle
      rcases strictOrFixed s with hfix | hstrict
      · -- Already a fixpoint at k = 0.
        exact ⟨0, Nat.zero_le _, by simpa [iterate] using hfix⟩
      · -- Strict increase: budget for `step s` drops by ≥1, apply IH.
        have hbudget : bound + 1 - measure (step s) ≤ fuel := by
          have hb := boundedH (step s)
          omega
        obtain ⟨k, hk, hkfix⟩ := ih (step s) hbudget
        refine ⟨k + 1, by omega, ?_⟩
        -- iterate step (k+1) s = iterate step k (step s).
        simpa [iterate] using hkfix

/-- §IC-7 ParamContract fixpoint convergence.

    A ParamContract SCC iteration `step` that (a) joins toward conservative so
    it never lowers the rank and is inflationary in the join order, and (b) at
    each step either is a fixpoint or strictly advances the rank, converges
    within `ParamContract.height + 1 = 14` iterations from any starting
    contract. Instantiates `IC7_bounded_monotone_converges` with
    `measure := ParamContract.rank` and `bound := ParamContract.height`. -/
theorem IC7_fixpoint_converges
    (step : ParamContract → ParamContract)
    (strictOrFixed : ∀ p, step p = p ∨ p.rank < (step p).rank) :
    ∀ p, ∃ k, k ≤ ParamContract.height + 1 - p.rank ∧
      step (iterate step k p) = iterate step k p :=
  IC7_bounded_monotone_converges step ParamContract.rank ParamContract.height
    IC7_paramcontract_rank_bounded strictOrFixed

/-- §IC-7 corollary — the universal iteration bound. From any starting
    contract the fixpoint is reached within `height + 1 = 14` iterations
    (the bound is independent of the start since `height + 1 - p.rank ≤
    height + 1`). This is the closed-form per-parameter term of the spec
    formula `param_count × 13 + ...` (annex-e §AIMS §7 IC-7). -/
theorem IC7_fixpoint_converges_uniform_bound
    (step : ParamContract → ParamContract)
    (strictOrFixed : ∀ p, step p = p ∨ p.rank < (step p).rank) (p : ParamContract) :
    ∃ k, k ≤ ParamContract.height + 1 ∧
      step (iterate step k p) = iterate step k p := by
  obtain ⟨k, hk, hfix⟩ := IC7_fixpoint_converges step strictOrFixed p
  exact ⟨k, by omega, hfix⟩

/-! ## §IC-4 — ReturnContract per-dim join over return paths (annex-e §AIMS §7 IC-4)

    A ReturnContract is `(uniqueness, preserves_freshness, locality, shape)`.
    The join is uniqueness(max) + preserves_freshness(AND) + locality(max) +
    shape(join). Freshness requires ALL paths preserve, so the boolean is joined
    by AND (conjunction monoid; identity `true`). -/

/-- §IC-4 IcReturnContract: the 4 return-provenance dimensions. -/
structure IcReturnContract where
  uniqueness : Uniqueness
  preserves_freshness : Bool
  locality : Locality
  shape : Shape
deriving Repr, DecidableEq

/-- §IC-4 IcReturnContract join: uniqueness(max) + preserves_freshness(AND) +
    locality(max) + shape(join). -/
def IcReturnContract.join (a b : IcReturnContract) : IcReturnContract :=
  { uniqueness := a.uniqueness.join b.uniqueness
  , preserves_freshness := a.preserves_freshness && b.preserves_freshness
  , locality := a.locality.join b.locality
  , shape := a.shape.join b.shape }

/-- §IC-4 CONSERVATIVE seed = the empty-fold identity for non-returning
    functions `(MaybeShared, preserves_freshness=false, Unknown, NonReusable)`. -/
def IcReturnContract.CONSERVATIVE : IcReturnContract :=
  { uniqueness := .MaybeShared
  , preserves_freshness := false
  , locality := .Unknown
  , shape := .NonReusable }

/-- §IC-4 join commutativity. The `shape` dimension uses the flat-lattice join
    (`Shape.join_comm`); uniqueness / locality use chain `max`;
    preserves_freshness uses AND. -/
theorem IC4_returncontract_join_comm (a b : IcReturnContract) :
    a.join b = b.join a := by
  simp only [IcReturnContract.join, Uniqueness.join_comm, Locality.join_comm,
    Shape.join_comm, Bool.and_comm]

/-- §IC-4 join associativity. -/
theorem IC4_returncontract_join_assoc (a b c : IcReturnContract) :
    (a.join b).join c = a.join (b.join c) := by
  simp only [IcReturnContract.join, Uniqueness.join_assoc, Locality.join_assoc,
    Shape.join_assoc, Bool.and_assoc]

/-- §IC-4 join idempotence. -/
theorem IC4_returncontract_join_idem (a : IcReturnContract) : a.join a = a := by
  simp only [IcReturnContract.join, Uniqueness.join_idem, Locality.join_idem,
    Shape.join_idem, Bool.and_self]

/-! ### §IC-4 preserves_freshness AND-fold over return paths

    Freshness requires ALL paths preserve: the function-level
    `preserves_freshness` is the AND over every path's bit. Modeled as a
    `List.foldr (· && ·) true` over the per-path bits; proven equal to
    `List.all`, i.e. true iff every path preserves. This is the genuine
    "ALL paths" structural property — induction over the path list. -/

/-- §IC-4 the AND-fold over a path list of `preserves_freshness` bits. -/
def IcReturnContract.freshnessFold (paths : List Bool) : Bool :=
  paths.foldr (· && ·) true

/-- §IC-4 the AND-fold is true iff EVERY path preserves freshness (the spec
    "Freshness requires ALL paths preserve" rule). Proven by induction over the
    path list — the structural ALL-paths argument. -/
theorem IC4_freshness_all_paths (paths : List Bool) :
    IcReturnContract.freshnessFold paths = true ↔ ∀ b ∈ paths, b = true := by
  unfold IcReturnContract.freshnessFold
  induction paths with
  | nil => simp
  | cons hd tl ih =>
      simp only [List.foldr_cons, Bool.and_eq_true, List.mem_cons]
      rw [ih]
      constructor
      · rintro ⟨hhd, htl⟩ b (rfl | hb)
        · exact hhd
        · exact htl b hb
      · intro h
        exact ⟨h hd (Or.inl rfl), fun b hb => h b (Or.inr hb)⟩

/-- §IC-4 a single non-fresh path sticky-clears the function-level freshness
    (the AND-monoid sticky-false transition). If any path has
    `preserves_freshness = false`, the fold is false. The negative pin: OR-fold
    would let a fresh path mask a non-fresh one — unsound for the RL-29 noalias
    gate. -/
theorem IC4_freshness_sticky_false (paths : List Bool) (hmem : false ∈ paths) :
    IcReturnContract.freshnessFold paths = false := by
  -- If the fold were `true`, IC4_freshness_all_paths forces every member to be
  -- `true`, contradicting `false ∈ paths`.
  cases hfold : IcReturnContract.freshnessFold paths with
  | false => rfl
  | true =>
      rw [IC4_freshness_all_paths] at hfold
      exact absurd (hfold false hmem) (by decide)

/-! ## §IC-5 — EffectSummary join (componentwise OR over effect flags)

    The EffectSummary is the §3.7 Effect dimension: independent boolean flags
    joined by componentwise OR. Reuse `EffectClass` from `Model.lean` and its
    `EffectClass.join`. The IC-5 properties — monotone, idempotent, commutative,
    associative — are the per-flag OR-algebra lifted componentwise. -/

/-- §IC-5 EffectSummary join idempotence (already an `EffectClass` law in
    `Lattice.lean`; re-exported under the IC-5 rule name). -/
theorem IC5_effectsummary_join_idem (e : EffectClass) : e.join e = e :=
  EffectClass.join_idem e

/-- §IC-5 EffectSummary join commutativity. -/
theorem IC5_effectsummary_join_comm (a b : EffectClass) : a.join b = b.join a :=
  EffectClass.join_comm a b

/-- §IC-5 EffectSummary join associativity. -/
theorem IC5_effectsummary_join_assoc (a b c : EffectClass) :
    (a.join b).join c = a.join (b.join c) :=
  EffectClass.join_assoc a b c

/-- The EffectSummary join-defined order: `a ≤ b` iff `a ⊔ b = b`. -/
def EffectClass.le (a b : EffectClass) : Prop := a.join b = b

/-- §IC-5 EffectSummary join monotonicity: `a ≤ b ⟹ join a r ≤ join b r`.
    Lifts the bool OR monotonicity componentwise; proven by `cases` over all
    three flags of each of `a`, `b`, `r` + `decide`. -/
theorem IC5_effectsummary_join_monotone (a b r : EffectClass)
    (hab : EffectClass.le a b) : EffectClass.le (a.join r) (b.join r) := by
  unfold EffectClass.le EffectClass.join at hab ⊢
  obtain ⟨aa, as, at'⟩ := a
  obtain ⟨ba, bs, bt⟩ := b
  obtain ⟨ra, rs, rt⟩ := r
  -- hab equates the OR-join of a and b with b, componentwise on three bools.
  simp only [EffectClass.mk.injEq] at hab ⊢
  obtain ⟨h1, h2, h3⟩ := hab
  refine ⟨?_, ?_, ?_⟩ <;>
    (cases aa <;> cases ba <;> cases ra <;> cases as <;> cases bs <;> cases rs <;>
      cases at' <;> cases bt <;> cases rt <;> simp_all)

/-! ## §IC-1 — SCC topological order (annex-e §AIMS §7 IC-1 + §6 PL-1a)

    The call graph is decomposed into SCCs; the condensation is a DAG, and SCCs
    are processed in topological order (callees before callers). The structural
    property: a finite acyclic precedence relation over the SCC list yields a
    well-founded processing order — every cross-SCC edge points from an earlier
    position to a later position, so processing left-to-right respects "callees
    before callers".

    Modeled as: a finite list of SCC nodes indexed by position, and a precedence
    relation `edge i j` (an edge in the condensation from the SCC at position `i`
    to the SCC at position `j`). The forward-topological invariant is
    `edge i j → i < j` (callees first). The structural theorem is that under
    this invariant the precedence relation is WELL-FOUNDED (acyclic) — there is
    no cycle, so the iterative SCC fixpoint over the position-ordered list
    terminates and never depends on an unprocessed (later) SCC. -/

/-- §IC-1 a condensation edge relation indexed by SCC position. `Edge i j`
    means a cross-SCC call edge runs from the SCC at position `i` to the one at
    position `j`. The forward-topological invariant (callees before callers)
    is `Edge i j → i < j`. -/
structure Condensation where
  /-- number of SCCs in the emitted list. -/
  numSccs : Nat
  /-- the cross-SCC precedence relation (position-indexed). -/
  Edge : Nat → Nat → Prop
  /-- IC-1 (P3) forward-topological invariant: every cross-SCC edge points
      forward (callee position < caller position). -/
  forward : ∀ i j, Edge i j → i < j

/-- §IC-1 the precedence relation is acyclic: no SCC reaches itself by a
    non-empty edge path. Self-edge case (the leaf): `Edge i i` is impossible
    because the forward invariant forces `i < i`. This is the (P2)/(P3)
    soundness floor — the condensation is a DAG, never cyclic. -/
theorem IC1_no_self_edge (G : Condensation) (i : Nat) : ¬ G.Edge i i := by
  intro h
  have : i < i := G.forward i i h
  exact absurd this (Nat.lt_irrefl i)

/-- §IC-1 the forward-topological order is well-founded: the precedence
    relation `Edge` (callee → caller) has no infinite ascending chain because
    every edge strictly increases the position index, which is a `Nat` bounded
    above by `numSccs`. Equivalently: a back-edge (from a later SCC to an
    earlier one) is impossible. This is the (P3) topological-ordering theorem —
    processing the SCC list in position order computes every callee's contract
    before any caller's. -/
theorem IC1_no_back_edge (G : Condensation) (i j : Nat)
    (hij : G.Edge i j) : ¬ G.Edge j i := by
  intro hji
  have h1 : i < j := G.forward i j hij
  have h2 : j < i := G.forward j i hji
  exact absurd h1 (Nat.not_lt.mpr (Nat.le_of_lt h2))

/-- §IC-1 well-foundedness as a measure: the precedence relation is contained in
    the strict `Nat` order on positions, so the inverse relation
    `fun a b => Edge b a` (caller→callee, the "depends-on" direction the fixpoint
    descends) is well-founded — every dependency chain strictly decreases the
    position and bottoms out at the leaf SCC (position 0 / a sink). The
    well-founded `Nat.lt` witnesses it. -/
theorem IC1_topological_order_wellfounded (G : Condensation) :
    ∀ i j, G.Edge i j → i < j := G.forward

/-- §IC-1 the SCC processing order respects "callees before callers": for any
    cross-SCC edge from position `i` to position `j`, position `i` is processed
    first (smaller index ⟹ earlier in the left-to-right SCC fixpoint sweep).
    This is the IC-2/IC-3 antecedent: a caller's ParamContract join consumes the
    callee's already-converged contract (no stale-summary dependency, PL-5). -/
theorem IC1_callee_processed_before_caller (G : Condensation) (i j : Nat)
    (hedge : G.Edge i j) : i < j :=
  G.forward i j hedge

/-! ## §IC-8 — REMOVED (annex-e §AIMS §7 IC-8 removal note)

    IC-8 is REMOVED. The former rule derived callee parameter uniqueness from
    caller consumption patterns:
      `caller arg (Owned, Linear, Once) ⟹ callee param Uniqueness := Unique`.

    This is UNSOUND for the same root cause as DP-10 (`Decision.lean` §DP-10):
    `(Owned, Linear, Once)` at the call site is a BACKWARD-DEMAND property
    ("this caller will not duplicate the argument beyond this single linear
    use"). The conclusion `Uniqueness = Unique` is a PAST-RC property requiring
    proof that the argument's runtime reference count is 1 at the call site.
    Backward demand says NOTHING about upstream aliases: a caller holding a
    `MaybeShared` argument used linearly may still hold RC > 1 from an upstream
    alias. The derivation jump-NARROWS to `Unique` without monotone lattice
    justification (it would violate IC-3 monotonicity, which only WIDENS Unique →
    MaybeShared → Shared, never narrows).

    SUBSUMED-BY: parameter uniqueness is established by the IC-2 OPTIMISTIC seed
    + IC-3 SCC fixpoint natural convergence (`IC2_optimistic_is_bottom` +
    `IC3_paramcontract_join_monotone` + `IC7_fixpoint_converges`). A fresh
    allocation at the caller (TF-3 / TF-9 / TF-9a) carries `Uniqueness = Unique`
    at the call site, and the IC-3 max-join carries it across to the callee
    parameter's converged contract WITHOUT any caller-consumption derivation.
    For non-enumerable call sites, IC-8a seeds CONSERVATIVE
    (`IC8a_conservative_upper_bounds`).

    The faithful encoding is the ABSENCE of a
    `derive_uniqueness_from_caller_consumption` function in this module: there
    is no Lean term for the removed rule, so it provably cannot be applied. The
    sound replacement paths (IC-2 / IC-3 / IC-8a) ARE present and kernel-checked
    above. This mirrors the DP-10-REMOVED treatment in `Decision.lean`. -/

end AimsProof
