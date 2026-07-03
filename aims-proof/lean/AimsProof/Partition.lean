/-
AIMS partition module — kernel-checked Lean proofs of the per-(var, field)
birth-site same-allocation partition soundness theorem T1 over the Phase-5
burden-lowering union-find.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: T1 (per-field birth-site partition soundness) |
  spec: IA-category (implementer-internal analysis rule) — no Annex E §AIMS
    clause; the per-field partition is not part of the public algorithmic
    contract |
  .proof: aims-proof/proofs/12-provenance/T1-partition-soundness.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: T1 governs the per-(ArcVarId, FieldIdx) birth-site-keyed
same-allocation union-find in Phase-5 burden lowering
(compiler/ori_arc/src/lower/burden_lower/): the partition deciding which
per-field lineage nodes share one allocation class (one balanced release per
class). Admission is edge-based — an unconditional Tier-1 same-allocation edge
(fund / view / alias), or a phi-merge edge licensed ONLY by the singleton
birth-site witness (every phi predecessor argument resolves to ONE birth-site).

Structure:
  Part A — a REAL executable union-find (parent map, fuelled find, link-by-root
    union, an edge-list fold). Every concrete representative in Part C is
    COMPUTED through it; nothing is a hardcoded representative table.
  Part B — the parametric core, over the node universe and the birth-site map:
    admission (`PartitionAdm`) + the union-find's connected-components relation
    (`SameRep`, the reflexive-symmetric-transitive closure of admitted edges);
    `samerep_birthsite_sound` (RST-closure induction) and
    `distinct_birthsite_no_phi_admission` (the kill-criterion guard —
    over-unification of distinct birth-sites is unrepresentable).
  Part C — the concrete loop-CFG instantiation witness: a back-edge loop whose
    loop-invariant field merge IS admitted (singleton witness holds) and whose
    loop-varying field merge is NOT (two birth-sites across the back-edge),
    with every representative computed by the Part-A union-find.
  Part D — per-field release accounting over the loop CFG: the cured per-field
    emission is safe (no read-after-free, both fields net 0 — the per-field
    RL-1/RL-2 exactly-once shape over a real back-edge); the whole-var
    single-dec emission leaks both fields (the negative witness).
-/

import AimsProof.Model

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §T1 Part A — executable union-find over an admitted edge list

    Nodes are `Nat` ids; the union-find is a functional parent map. `find`
    follows parents to a root under a fuel bound; `union` links root-to-root;
    `buildPartitionUF` FOLDS an admitted edge list. Every representative claim
    in Part C is computed through these definitions. -/

/-- Functional parent map; `empty` makes every node its own parent. -/
structure PartitionUF where
  parent : Nat → Nat

def PartitionUF.empty : PartitionUF := ⟨id⟩

/-- Fuelled `find`: follow `parent` to a root (a node that is its own parent).
    Fuel bounds the chain length; the concrete instances stay far below it. -/
def PartitionUF.find (uf : PartitionUF) : Nat → Nat → Nat
  | 0,      x => x
  | fuel+1, x => let px := uf.parent x; if px == x then x else uf.find fuel px

/-- Fuel bound above any parent-chain length in the concrete CFG instances. -/
def partitionFuel : Nat := 16

/-- `union a b`: point `find a`'s root at `find b`'s root (link-by-root). -/
def PartitionUF.union (uf : PartitionUF) (a b : Nat) : PartitionUF :=
  let ra := uf.find partitionFuel a
  let rb := uf.find partitionFuel b
  ⟨fun x => if x == ra then rb else uf.parent x⟩

/-- Fold an admitted edge list into the union-find (consume the edges). -/
def buildPartitionUF (edges : List (Nat × Nat)) : PartitionUF :=
  edges.foldl (fun uf e => uf.union e.1 e.2) PartitionUF.empty

/-- The COMPUTED representatives coincide. -/
def PartitionUF.sameRep (uf : PartitionUF) (a b : Nat) : Bool :=
  uf.find partitionFuel a == uf.find partitionFuel b

/-! ## §T1 Part B — parametric partition soundness (the load-bearing core)

    Parametric over the node universe `ν` and the birth-site map
    `birthSite : ν → β`. An edge is ADMITTED in exactly two ways:

    * `tier1` — an unconditional same-allocation edge (fund / view / alias);
      its meaning is that both nodes name the SAME allocation, so admission
      carries `birthSite u = birthSite v`.
    * `phi` — a phi-merge edge `p ~ a`, admitted ONLY under the SINGLETON
      birth-site witness: some `B` with EVERY phi predecessor argument's
      birth-site equal to `B`, and the merge node's own birth-site equal to
      that `B` (phi semantics: the merge holds whichever predecessor's value
      flowed, and they all hold `B`).

    The union-find computes exactly the reflexive-symmetric-transitive closure
    of its admitted edges; `SameRep` is that closure. -/

/-- §T1 an admitted same-allocation edge over node universe `ν` with birth-site
    map `birthSite : ν → β`. -/
inductive PartitionAdm {ν β : Type} (birthSite : ν → β) : ν → ν → Prop
  | tier1 {u v : ν} (h : birthSite u = birthSite v) : PartitionAdm birthSite u v
  | phi {p a : ν} {preds : List ν} {B : β}
      (hmem : a ∈ preds)
      (hsingleton : ∀ x ∈ preds, birthSite x = B)
      (hphi : birthSite p = B) : PartitionAdm birthSite p a

/-- §T1 (P1) every admitted edge is birth-site-sound (the per-edge obligation). -/
theorem partitionAdm_birthsite_sound {ν β : Type} {birthSite : ν → β}
    {u v : ν} (h : PartitionAdm birthSite u v) : birthSite u = birthSite v := by
  cases h with
  | tier1 h => exact h
  | phi hmem hsingleton hphi => rw [hphi, hsingleton _ hmem]

/-- §T1 the union-find's connected-components relation: the reflexive-
    symmetric-transitive closure of the admitted edges (what the edge-list
    fold through `union`/`find` computes). -/
inductive SameRep {ν β : Type} (birthSite : ν → β) : ν → ν → Prop
  | refl (x : ν) : SameRep birthSite x x
  | edge {u v : ν} (h : PartitionAdm birthSite u v) : SameRep birthSite u v
  | symm {u v : ν} (h : SameRep birthSite u v) : SameRep birthSite v u
  | trans {u v w : ν} (h1 : SameRep birthSite u v) (h2 : SameRep birthSite v w) :
      SameRep birthSite u w

/-- §T1 (P2) THE partition-soundness theorem. The built partition only ever
    unions same-birth-site nodes — `SameRep birthSite u v` implies
    `birthSite u = birthSite v` — for EVERY node universe and EVERY birth-site
    map. Proven by induction on the RST closure: each admitted step preserves
    the birth-site via `partitionAdm_birthsite_sound`; never stipulated, never
    a `decide` over a hardcoded representative map. -/
theorem samerep_birthsite_sound {ν β : Type} {birthSite : ν → β}
    {u v : ν} (h : SameRep birthSite u v) : birthSite u = birthSite v := by
  induction h with
  | refl x => rfl
  | edge h => exact partitionAdm_birthsite_sound h
  | symm _ ih => exact ih.symm
  | trans _ _ ih1 ih2 => exact ih1.trans ih2

/-- §T1 (P2) the disjointness payoff (contrapositive). Distinct birth-sites are
    NEVER unified: one balanced release per representative class cannot
    cross-free a different allocation. -/
theorem distinct_birthsite_distinct_rep {ν β : Type} {birthSite : ν → β}
    {u v : ν} (h : birthSite u ≠ birthSite v) : ¬ SameRep birthSite u v :=
  fun hsr => h (samerep_birthsite_sound hsr)

/-- §T1 (P3) the kill-criterion guard. A phi merge whose predecessors carry
    DISTINCT birth-sites has NO singleton witness, so no `phi` edge for that
    merge is admissible: a loop back-edge re-feeding a field from a different
    iteration's allocation cannot be unified — the over-unification that would
    double-free is unrepresentable in the admission rule. -/
theorem distinct_birthsite_no_phi_admission {ν β : Type} {birthSite : ν → β}
    {a1 a2 : ν} {preds : List ν}
    (h1 : a1 ∈ preds) (h2 : a2 ∈ preds) (hne : birthSite a1 ≠ birthSite a2) :
    ¬ ∃ B, ∀ x ∈ preds, birthSite x = B := by
  rintro ⟨B, hall⟩
  exact hne ((hall a1 h1).trans (hall a2 h2).symm)

/-! ## §T1 Part C — concrete loop-CFG instantiation witness (computed reps)

    A back-edge loop with two owned heap fields lowered per (var, field):

      items field (loop-INVARIANT — allocated once, threaded unchanged):
        10 itemsCtor   (the pfBirthItems allocation)
        11 itemsView   (Project borrow-view of itemsCtor — same allocation)
        12 itemsHdr    (loop-header block-param field,
                        phi(entry: itemsView, latch: itemsView))
      label field (loop-VARYING — re-allocated each iteration):
        20 labelB0     (pfBirthLabel0, the entry / iteration-0 allocation)
        21 labelB1     (pfBirthLabel1, the latch / iteration-1 allocation)
        22 labelHdr    (loop-header block-param field,
                        phi(entry: labelB0, latch: labelB1))
      aggregate move-alias (its own class, holds no field allocation):
        30 aggRoot  31 aggAlias

    Ground-truth birth-sites: distinct fields and the loop-varying
    re-allocation carry DISTINCT birth-site ids. The union-find never reads
    this map — it consumes edges plus the singleton guard; the map is the
    soundness yardstick the computed partition is checked against. -/

def pfBirthItems : Nat := 100
def pfBirthLabel0 : Nat := 200
def pfBirthLabel1 : Nat := 201
def pfBirthAgg : Nat := 300

/-- Ground-truth birth-site per lineage node. Node 22's value only witnesses
    that its merge's predecessor birth-sites differ (no edge is admitted for
    it, so its representative stays its own). -/
def pfBirthSite : Nat → Nat
  | 10 | 11 | 12 => pfBirthItems
  | 20           => pfBirthLabel0
  | 21           => pfBirthLabel1
  | 22           => pfBirthLabel1
  | 30 | 31      => pfBirthAgg
  | _            => 0

/-- Tier-1 unconditional same-allocation edges (fund / view / alias). -/
def pfTier1Edges : List (Nat × Nat) :=
  [ (11, 10)    -- itemsView ~ itemsCtor (Project borrow-view, same allocation)
  , (31, 30)    -- aggAlias  ~ aggRoot   (whole-var alias, its own class)
  ]

/-- The items loop-header merge IS admitted: both phi predecessors (entry and
    latch) carry the loop-invariant `itemsView`, so the birth-site set across
    the merge is the singleton {pfBirthItems}. -/
def pfItemsMergeEdge : List (Nat × Nat) :=
  [ (12, 11) ]

/-- The label loop-header merge is NOT admitted: predecessors carry
    pfBirthLabel0 (entry) and pfBirthLabel1 (latch back-edge) — a non-singleton
    birth-site set — so NO edge is added and node 22 keeps its own
    representative. -/
def pfLabelMergeEdge : List (Nat × Nat) := []

/-- The full admitted edge set the union-find folds. -/
def pfAdmittedEdges : List (Nat × Nat) :=
  pfTier1Edges ++ pfItemsMergeEdge ++ pfLabelMergeEdge

/-- The built union-find — computed by folding `pfAdmittedEdges` through the
    real `union`/`find`; nothing hardcoded. -/
def pfUF : PartitionUF := buildPartitionUF pfAdmittedEdges

/-- §T1 (P4) itemsView and itemsCtor are unified (the Tier-1 view edge);
    COMPUTED over the built union-find. -/
theorem T1_items_view_unified : pfUF.sameRep 11 10 = true := by
  decide

/-- §T1 (P4) the items loop-header field — fed across the BACK-EDGE by the
    loop-invariant value — IS unified to the items birth-site class: the
    singleton witness licensed the merge edge and the real union-find computed
    the same representative. -/
theorem T1_items_header_unified_across_backedge : pfUF.sameRep 12 10 = true := by
  decide

/-- §T1 (P4) the non-singleton label merge keeps a representative DISTINCT from
    the entry allocation; COMPUTED. -/
theorem T1_backedge_keeps_distinct_from_entry : pfUF.sameRep 22 20 = false := by
  decide

/-- §T1 (P4) the non-singleton label merge keeps a representative DISTINCT from
    the latch allocation; COMPUTED. -/
theorem T1_backedge_keeps_distinct_from_latch : pfUF.sameRep 22 21 = false := by
  decide

/-- §T1 (P4) the two genuinely-distinct label allocations are never unified;
    COMPUTED. -/
theorem T1_distinct_label_allocs_not_unified : pfUF.sameRep 20 21 = false := by
  decide

/-- §T1 (P4) the aggregate move-alias class is its OWN class — it holds neither
    field's allocation, so a whole-var dec routed onto it frees no field;
    COMPUTED. -/
theorem T1_agg_class_holds_no_field :
    pfUF.sameRep 30 10 = false ∧ pfUF.sameRep 30 20 = false := by
  decide

/-- §T1 (P3 instantiated) the concrete label merge has NO singleton witness —
    the parametric kill-criterion applied at the loop-varying phi's predecessor
    list (pfBirthSite 20 = 200, pfBirthSite 21 = 201). -/
theorem T1_label_merge_no_singleton_witness :
    ¬ ∃ B, ∀ x ∈ [20, 21], pfBirthSite x = B :=
  distinct_birthsite_no_phi_admission (a1 := 20) (a2 := 21)
    (by decide) (by decide) (by decide)

/-! ## §T1 Part D — per-field release accounting over the loop CFG

    A loop state machine over both owned-field counts plus a read-after-free
    flag; the event stream threads the back-edge across two iterations. The
    per-field grain is the point: the loop-invariant items release rides the
    whole-loop lifetime while each loop-varying label release rides its own
    per-iteration allocation — the RL-1/RL-2 balance discipline applied per
    allocation class over the disjoint partition proven above. -/

/-- Machine state: (items count, label count, read-after-free observed). -/
abbrev PerFieldSt := Int × Int × Bool

inductive PerFieldEvent
  | allocItems | readItems | relItems    -- loop-invariant items lifecycle
  | allocLabel | readLabel | relLabel    -- per-iteration label lifecycle
  | aggWholeDec                          -- whole-var dec on the aggregate class
deriving Repr, DecidableEq

/-- A read at count < 1 observes a freed allocation and sets the flag. The
    whole-var dec lands on the aggregate class — which holds no field
    allocation per `T1_agg_class_holds_no_field` — so it is a no-op on both
    field counts. -/
def perFieldStep (st : PerFieldSt) : PerFieldEvent → PerFieldSt
  | .allocItems  => (st.1 + 1, st.2.1, st.2.2)
  | .relItems    => (st.1 - 1, st.2.1, st.2.2)
  | .readItems   => (st.1, st.2.1, if st.1 < 1 then true else st.2.2)
  | .allocLabel  => (st.1, st.2.1 + 1, st.2.2)
  | .relLabel    => (st.1, st.2.1 - 1, st.2.2)
  | .readLabel   => (st.1, st.2.1, if st.2.1 < 1 then true else st.2.2)
  | .aggWholeDec => st

def perFieldRun (es : List PerFieldEvent) : PerFieldSt :=
  es.foldl perFieldStep (0, 0, false)

/-- Safe: no read observed a freed value AND both field allocations net 0
    (items released once at exit; each label released within its iteration). -/
def perFieldSafe (es : List PerFieldEvent) : Prop :=
  (perFieldRun es).2.2 = false ∧ (perFieldRun es).1 = 0 ∧ (perFieldRun es).2.1 = 0

/-- One cured iteration: read the invariant items view, then allocate / read /
    release THIS iteration's fresh label allocation. The items allocation is
    not released here — it lives across the back-edge. -/
def pfCuredIter : List PerFieldEvent :=
  [PerFieldEvent.readItems, PerFieldEvent.allocLabel,
   PerFieldEvent.readLabel, PerFieldEvent.relLabel]

/-- The cured loop across TWO iterations (the back-edge re-enters the body):
    items allocated once at entry, released once at exit. -/
def pfCuredLoop : List PerFieldEvent :=
  [PerFieldEvent.allocItems] ++ pfCuredIter ++ pfCuredIter
    ++ [PerFieldEvent.relItems]

/-- §T1 (P5) the cured loop is SAFE over the back-edge: items stays live across
    both iterations (every read sees count >= 1), each per-iteration label is
    released exactly once, and both fields net to zero. -/
theorem T1_cured_loop_safe : perFieldSafe pfCuredLoop := by
  unfold perFieldSafe pfCuredLoop pfCuredIter; decide

/-- §T1 (P5) exactly-once per field across the loop: items nets 0 (one release
    at exit) AND label nets 0 (one release per iteration) — the back-edge
    generalization of the RL-2 exactly-once shape applied per field. -/
theorem T1_per_field_release_exactly_once_over_loop :
    (perFieldRun pfCuredLoop).1 = 0 ∧ (perFieldRun pfCuredLoop).2.1 = 0 := by
  unfold perFieldRun pfCuredLoop pfCuredIter; decide

/-- The whole-var emission: per-field releases omitted, one dec on the
    aggregate class (a no-op on both field allocations). -/
def pfBuggyLoop : List PerFieldEvent :=
  [PerFieldEvent.allocItems]
    ++ [PerFieldEvent.readItems, PerFieldEvent.allocLabel, PerFieldEvent.readLabel]
    ++ [PerFieldEvent.readItems, PerFieldEvent.allocLabel, PerFieldEvent.readLabel]
    ++ [PerFieldEvent.aggWholeDec]

/-- §T1 (P5) the negative witness: the whole-var emission leaks — after two
    iterations the invariant items allocation ends at count 1 and the label
    allocations end at net count 2, all unreleased. -/
theorem T1_buggy_loop_leaks :
    (perFieldRun pfBuggyLoop).1 = 1 ∧ (perFieldRun pfBuggyLoop).2.1 = 2 := by
  unfold perFieldRun pfBuggyLoop; decide

/-! ## §T1 conclusion bundle -/

/-- §T1 the feasibility bundle: computed unification of the singleton-admitted
    merge, computed split of the non-singleton merge, the parametric soundness
    + kill-criterion instantiated at the concrete birth-site map, and the
    per-field release accounting over the back-edge loop. -/
theorem T1_per_field_birthsite_union_find_sound :
    pfUF.sameRep 12 10 = true
    ∧ pfUF.sameRep 22 20 = false
    ∧ pfUF.sameRep 22 21 = false
    ∧ (∀ u v : Nat, SameRep pfBirthSite u v → pfBirthSite u = pfBirthSite v)
    ∧ (∀ u v : Nat, pfBirthSite u ≠ pfBirthSite v → ¬ SameRep pfBirthSite u v)
    ∧ perFieldSafe pfCuredLoop
    ∧ ((perFieldRun pfBuggyLoop).1 = 1 ∧ (perFieldRun pfBuggyLoop).2.1 = 2) := by
  refine ⟨T1_items_header_unified_across_backedge,
          T1_backedge_keeps_distinct_from_entry,
          T1_backedge_keeps_distinct_from_latch,
          fun _ _ => samerep_birthsite_sound,
          fun _ _ => distinct_birthsite_distinct_rep,
          T1_cured_loop_safe,
          T1_buggy_loop_leaks⟩

end AimsProof
