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
  Part A — a REAL executable union-find (functional parent map, fuelled find
    with the fuel carried on the structure and grown by one per link-by-root
    union, an edge-list fold). Every concrete representative in Part C is
    COMPUTED through it; nothing is a hardcoded representative table.
  Part B — the parametric core, over the node universe and the birth-site map:
    admission (`PartitionAdm`) + the union-find's connected-components relation
    (`SameRep`, the reflexive-symmetric-transitive closure of admitted edges);
    `samerep_birthsite_sound` (RST-closure induction) and
    `distinct_birthsite_no_phi_admission` (the kill-criterion guard —
    over-unification of distinct birth-sites is unrepresentable).
  Part B′ — the kernel-checked executable ↔ closure correspondence:
    `buildPartitionUF_sameRep_iff_edgeConn` (the general theorem — the
    computed `sameRep` verdict holds EXACTLY on the RST closure of the folded
    edge list) and `buildPartitionUF_birthsite_sound` (its composition with
    `samerep_birthsite_sound` through the admitted-edge lift).
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

    Nodes are `Nat` ids; the union-find is a functional parent map paired with
    a fuel bound carried on the structure. `find` follows parents to a root
    within `fuel` steps; `union` links root-to-root and grows the fuel by one
    (link-by-root adds at most one non-trivial parent edge per union, so every
    parent chain stays within bound — the fact Part B′ proves);
    `buildPartitionUF` FOLDS an admitted edge list. Every representative claim
    in Part C is computed through these definitions. -/

/-- Functional parent map + fuel bound; `empty` makes every node its own
    parent under fuel 1. -/
structure PartitionUF where
  parent : Nat → Nat
  fuel : Nat

def PartitionUF.empty : PartitionUF := ⟨id, 1⟩

/-- Fuelled parent walk: follow `parent` to a fixpoint within `fuel` steps. -/
def findAux (parent : Nat → Nat) : Nat → Nat → Nat
  | 0,      x => x
  | fuel+1, x => if parent x = x then x else findAux parent fuel (parent x)

/-- `find`: the fuelled walk under the structure's own fuel bound. -/
def PartitionUF.find (uf : PartitionUF) (x : Nat) : Nat :=
  findAux uf.parent uf.fuel x

/-- The link-by-root redirect map: `ra ↦ rb`, every other node unchanged. -/
def redirect (parent : Nat → Nat) (ra rb : Nat) (y : Nat) : Nat :=
  if y = ra then rb else parent y

/-- `union a b`: point `find a`'s root at `find b`'s root (link-by-root) and
    grow the fuel by one (at most one new parent edge per union). -/
def PartitionUF.union (uf : PartitionUF) (a b : Nat) : PartitionUF :=
  ⟨redirect uf.parent (uf.find a) (uf.find b), uf.fuel + 1⟩

/-- Fold an admitted edge list into the union-find (consume the edges). -/
def buildPartitionUF (edges : List (Nat × Nat)) : PartitionUF :=
  edges.foldl (fun uf e => uf.union e.1 e.2) PartitionUF.empty

/-- The COMPUTED representatives coincide. -/
def PartitionUF.sameRep (uf : PartitionUF) (a b : Nat) : Bool :=
  uf.find a == uf.find b

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

    `SameRep` is the reflexive-symmetric-transitive closure of the admitted
    edges — the object the soundness theorems below govern. The executable
    `buildPartitionUF` fold computes EXACTLY that closure, and the
    correspondence IS kernel-checked (Part B′):
    `buildPartitionUF_sameRep_iff_edgeConn` proves — as a general theorem over
    every edge list, by induction, never by instance `decide` — that the
    computed `sameRep` verdict holds iff the RST closure of the folded edge
    list connects the nodes, and `buildPartitionUF_birthsite_sound` composes
    it end-to-end with `samerep_birthsite_sound`. Consumers may rely on the
    executable partition as a proof surface. `class_eq_iff_sameRep`
    (Ledger.lean) bridges the executable's find-equality to its own `sameRep`
    Boolean. -/

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

/-! ## §T1 Part B′ — executable ↔ closure correspondence (kernel-checked)

    The bridge discharging the executable-to-closure obligation as a GENERAL
    theorem. The fuelled walk is characterized by an inductive root-walk
    certificate (`WalksTo`: every intermediate node is a non-fixpoint, so a
    walk never passes THROUGH a root mid-walk); the certificate transports
    across a link-by-root `redirect` with AT MOST one extra step — matching
    the one-fuel growth per `union` — and an invariant threaded through the
    edge-list fold yields BOTH directions of
    `buildPartitionUF_sameRep_iff_edgeConn`. -/

/-- The reflexive-symmetric-transitive closure of a concrete edge list. -/
inductive EdgeConn (edges : List (Nat × Nat)) : Nat → Nat → Prop
  | refl (x : Nat) : EdgeConn edges x x
  | edge {u v : Nat} (h : (u, v) ∈ edges) : EdgeConn edges u v
  | symm {u v : Nat} (h : EdgeConn edges u v) : EdgeConn edges v u
  | trans {u v w : Nat} (h1 : EdgeConn edges u v) (h2 : EdgeConn edges v w) :
      EdgeConn edges u w

/-- Closure monotonicity in the edge list. -/
theorem edgeConn_mono {edges edges' : List (Nat × Nat)}
    (hsub : ∀ e ∈ edges, e ∈ edges') {u v : Nat}
    (h : EdgeConn edges u v) : EdgeConn edges' u v := by
  induction h with
  | refl x => exact .refl x
  | edge h => exact .edge (hsub _ h)
  | symm _ ih => exact .symm ih
  | trans _ _ ih1 ih2 => exact .trans ih1 ih2

/-- Root-walk certificate: `x` reaches fixpoint `r` in exactly `n` proper
    steps, every intermediate node a non-fixpoint. -/
inductive WalksTo (parent : Nat → Nat) : Nat → Nat → Nat → Prop
  | root {x : Nat} (h : parent x = x) : WalksTo parent 0 x x
  | step {n x r : Nat} (hne : parent x ≠ x)
      (h : WalksTo parent n (parent x) r) : WalksTo parent (n+1) x r

/-- A certified walk ends at a fixpoint. -/
theorem WalksTo.root_fix {parent : Nat → Nat} {n x r : Nat}
    (h : WalksTo parent n x r) : parent r = r := by
  induction h with
  | root h => exact h
  | step _ _ ih => exact ih

/-- The fuelled walk realizes any certificate within its fuel. -/
theorem findAux_eq_of_walksTo {parent : Nat → Nat} {n x r : Nat}
    (h : WalksTo parent n x r) : ∀ f, n ≤ f → findAux parent f x = r := by
  induction h with
  | @root x hfix =>
    intro f _
    cases f with
    | zero => rfl
    | succ f =>
      simp only [findAux]
      rw [if_pos hfix]
  | @step n x r hne hwalk ih =>
    intro f hf
    cases f with
    | zero => exact absurd hf (Nat.not_succ_le_zero n)
    | succ f =>
      simp only [findAux]
      rw [if_neg hne]
      exact ih f (Nat.le_of_succ_le_succ hf)

theorem redirect_eq (parent : Nat → Nat) (ra rb : Nat) :
    redirect parent ra rb ra = rb := if_pos rfl

theorem redirect_ne {parent : Nat → Nat} {ra rb y : Nat} (h : y ≠ ra) :
    redirect parent ra rb y = parent y := if_neg h

/-- Certificate transport across a link-by-root redirect `ra ↦ rb` (both old
    fixpoints): the walk survives with AT MOST one extra step, landing on the
    redirect of its old root. Intermediate nodes are non-fixpoints, hence
    never `ra`, so only the endpoint case extends the walk. -/
theorem WalksTo.redirect_transport {parent : Nat → Nat} {ra rb : Nat}
    (hra : parent ra = ra) (hrb : parent rb = rb) {n x r : Nat}
    (h : WalksTo parent n x r) :
    ∃ m, m ≤ n + 1 ∧
      WalksTo (redirect parent ra rb) m x (if r = ra then rb else r) := by
  induction h with
  | @root x hfix =>
    by_cases hxa : x = ra
    · rw [hxa, if_pos rfl]
      by_cases hba : rb = ra
      · refine ⟨0, Nat.zero_le _, ?_⟩
        rw [hba]
        exact WalksTo.root (redirect_eq parent ra ra)
      · refine ⟨1, Nat.le_refl _, ?_⟩
        have h1 : redirect parent ra rb ra = rb := redirect_eq parent ra rb
        have h2 : redirect parent ra rb rb = rb := by
          rw [redirect_ne hba]; exact hrb
        exact WalksTo.step (by rw [h1]; exact hba)
          (by rw [h1]; exact WalksTo.root h2)
    · rw [if_neg hxa]
      exact ⟨0, Nat.zero_le _,
        WalksTo.root (by rw [redirect_ne hxa]; exact hfix)⟩
  | @step n x r hne hwalk ih =>
    have hxa : x ≠ ra := fun hcontra => hne (by rw [hcontra]; exact hra)
    obtain ⟨m, hm, hw'⟩ := ih
    have hpx : redirect parent ra rb x = parent x := redirect_ne hxa
    exact ⟨m + 1, Nat.succ_le_succ hm,
      WalksTo.step (by rw [hpx]; exact hne) (by rw [hpx]; exact hw')⟩

/-- Well-fuelled: every node's parent chain reaches its root within fuel. -/
def PartitionUF.Wf (uf : PartitionUF) : Prop :=
  ∀ x, ∃ n r, n ≤ uf.fuel ∧ WalksTo uf.parent n x r

theorem PartitionUF.find_eq_of_walksTo {uf : PartitionUF} {n x r : Nat}
    (hn : n ≤ uf.fuel) (h : WalksTo uf.parent n x r) : uf.find x = r :=
  findAux_eq_of_walksTo h uf.fuel hn

/-- Under `Wf`, every node holds a certificate ending at its `find`. -/
theorem PartitionUF.walksTo_find {uf : PartitionUF} (hwf : uf.Wf) (x : Nat) :
    ∃ n, n ≤ uf.fuel ∧ WalksTo uf.parent n x (uf.find x) := by
  obtain ⟨n, r, hn, hw⟩ := hwf x
  rw [uf.find_eq_of_walksTo hn hw]
  exact ⟨n, hn, hw⟩

/-- Under `Wf`, every computed representative is a parent fixpoint. -/
theorem PartitionUF.parent_find_fix {uf : PartitionUF} (hwf : uf.Wf)
    (x : Nat) : uf.parent (uf.find x) = uf.find x := by
  obtain ⟨n, hn, hw⟩ := uf.walksTo_find hwf x
  exact hw.root_fix

/-- Fuel growth keeps the union well-fuelled: the transported certificate's
    one extra step rides the union's one extra fuel. -/
theorem PartitionUF.union_wf {uf : PartitionUF} (hwf : uf.Wf) (a b : Nat) :
    (uf.union a b).Wf := by
  intro x
  obtain ⟨n, hn, hw⟩ := uf.walksTo_find hwf x
  obtain ⟨m, hm, hw'⟩ := hw.redirect_transport
    (uf.parent_find_fix hwf a) (uf.parent_find_fix hwf b)
  exact ⟨m, _, Nat.le_trans hm (Nat.succ_le_succ hn), hw'⟩

/-- THE central lemma: a link-by-root union redirects exactly the old root
    class of `a` onto the old root of `b` and leaves every other computed
    representative unchanged. -/
theorem PartitionUF.find_union {uf : PartitionUF} (hwf : uf.Wf)
    (a b x : Nat) :
    (uf.union a b).find x =
      if uf.find x = uf.find a then uf.find b else uf.find x := by
  obtain ⟨n, hn, hw⟩ := uf.walksTo_find hwf x
  obtain ⟨m, hm, hw'⟩ := hw.redirect_transport
    (uf.parent_find_fix hwf a) (uf.parent_find_fix hwf b)
  exact PartitionUF.find_eq_of_walksTo (uf := uf.union a b)
    (Nat.le_trans hm (Nat.succ_le_succ hn)) hw'

/-- The fold invariant over the processed edge prefix: well-fuelled; every
    parent step is edge-connected; every processed edge's endpoints share a
    computed representative. -/
structure UFInv (edges : List (Nat × Nat)) (uf : PartitionUF) : Prop where
  wf : uf.Wf
  parent_conn : ∀ x, EdgeConn edges x (uf.parent x)
  edges_merged : ∀ e ∈ edges, uf.find e.1 = uf.find e.2

/-- A certified walk is edge-connected end-to-end. -/
theorem walksTo_conn {edges : List (Nat × Nat)} {parent : Nat → Nat}
    (hconn : ∀ y, EdgeConn edges y (parent y)) {n x r : Nat}
    (h : WalksTo parent n x r) : EdgeConn edges x r := by
  induction h with
  | root _ => exact .refl _
  | step _ _ ih => exact .trans (hconn _) ih

/-- Under the invariant, every node is edge-connected to its representative. -/
theorem UFInv.find_conn {edges : List (Nat × Nat)} {uf : PartitionUF}
    (hinv : UFInv edges uf) (x : Nat) : EdgeConn edges x (uf.find x) := by
  obtain ⟨n, _, hw⟩ := uf.walksTo_find hinv.wf x
  exact walksTo_conn hinv.parent_conn hw

/-- One union step: processing edge `(a, b)` extends the invariant by it. -/
theorem UFInv.union_step {edges : List (Nat × Nat)} {uf : PartitionUF}
    (hinv : UFInv edges uf) (a b : Nat) :
    UFInv (edges ++ [(a, b)]) (uf.union a b) := by
  have hmono : ∀ e ∈ edges, e ∈ edges ++ [(a, b)] :=
    fun e he => List.mem_append.mpr (Or.inl he)
  refine ⟨PartitionUF.union_wf hinv.wf a b, ?_, ?_⟩
  · intro x
    show EdgeConn (edges ++ [(a, b)]) x
      (redirect uf.parent (uf.find a) (uf.find b) x)
    by_cases hxa : x = uf.find a
    · rw [hxa, redirect_eq]
      have hab : EdgeConn (edges ++ [(a, b)]) a b :=
        .edge (List.mem_append.mpr (Or.inr (List.Mem.head _)))
      have ha : EdgeConn (edges ++ [(a, b)]) a (uf.find a) :=
        edgeConn_mono hmono (hinv.find_conn a)
      have hb : EdgeConn (edges ++ [(a, b)]) b (uf.find b) :=
        edgeConn_mono hmono (hinv.find_conn b)
      exact .trans (.symm ha) (.trans hab hb)
    · rw [redirect_ne hxa]
      exact edgeConn_mono hmono (hinv.parent_conn x)
  · intro e he
    obtain ⟨u, v⟩ := e
    show (uf.union a b).find u = (uf.union a b).find v
    rw [PartitionUF.find_union hinv.wf, PartitionUF.find_union hinv.wf]
    rcases List.mem_append.mp he with hmem | hmem
    · rw [show uf.find u = uf.find v from hinv.edges_merged (u, v) hmem]
    · cases hmem with
      | head =>
        rw [if_pos rfl]
        by_cases hb : uf.find b = uf.find a
        · rw [if_pos hb]
        · rw [if_neg hb]
      | tail _ h => cases h

/-- The fold preserves and extends the invariant across a suffix of edges. -/
theorem foldl_union_inv (rest : List (Nat × Nat)) :
    ∀ (processed : List (Nat × Nat)) (uf : PartitionUF), UFInv processed uf →
      UFInv (processed ++ rest)
        (rest.foldl (fun uf e => uf.union e.1 e.2) uf) := by
  induction rest with
  | nil =>
    intro processed uf hinv
    simpa using hinv
  | cons e rest ih =>
    intro processed uf hinv
    obtain ⟨a, b⟩ := e
    have := ih (processed ++ [(a, b)]) (uf.union a b) (hinv.union_step a b)
    simpa [List.foldl_cons, List.append_assoc] using this

/-- The built union-find satisfies the invariant over the FULL edge list. -/
theorem buildPartitionUF_inv (edges : List (Nat × Nat)) :
    UFInv edges (buildPartitionUF edges) := by
  have h0 : UFInv [] PartitionUF.empty :=
    ⟨fun x => ⟨0, x, Nat.zero_le _, WalksTo.root rfl⟩,
     fun x => EdgeConn.refl x,
     fun _ he => by cases he⟩
  simpa [buildPartitionUF] using foldl_union_inv edges [] PartitionUF.empty h0

/-- Soundness half: a shared computed representative yields closure
    membership (walk down to the shared root, back up the other side). -/
theorem edgeConn_of_sameRep {edges : List (Nat × Nat)} {a b : Nat}
    (h : (buildPartitionUF edges).sameRep a b = true) : EdgeConn edges a b := by
  have hinv := buildPartitionUF_inv edges
  have hfind : (buildPartitionUF edges).find a = (buildPartitionUF edges).find b := by
    simpa [PartitionUF.sameRep, beq_iff_eq] using h
  have ha := hinv.find_conn a
  have hb := hinv.find_conn b
  rw [hfind] at ha
  exact ha.trans hb.symm

/-- Completeness half: closure membership yields a shared computed
    representative (RST-closure induction over the merged-edges invariant). -/
theorem find_eq_of_edgeConn {edges : List (Nat × Nat)} {a b : Nat}
    (h : EdgeConn edges a b) :
    (buildPartitionUF edges).find a = (buildPartitionUF edges).find b := by
  have hinv := buildPartitionUF_inv edges
  induction h with
  | refl x => rfl
  | edge h => exact hinv.edges_merged _ h
  | symm _ ih => exact ih.symm
  | trans _ _ ih1 ih2 => exact ih1.trans ih2

/-- §T1 (P2′) THE executable ↔ closure correspondence, kernel-checked as a
    GENERAL theorem: for EVERY edge list and EVERY node pair, the executable
    fold's computed `sameRep` verdict holds EXACTLY on the reflexive-
    symmetric-transitive closure of the admitted edge list. -/
theorem buildPartitionUF_sameRep_iff_edgeConn (edges : List (Nat × Nat))
    (a b : Nat) :
    (buildPartitionUF edges).sameRep a b = true ↔ EdgeConn edges a b := by
  constructor
  · exact edgeConn_of_sameRep
  · intro h
    simpa [PartitionUF.sameRep, beq_iff_eq] using find_eq_of_edgeConn h

/-- Lift each concrete admitted edge through the parametric admission: the
    closure of an admitted edge list lands inside `SameRep`. -/
theorem sameRep_of_edgeConn {β : Type} {birthSite : Nat → β}
    {edges : List (Nat × Nat)}
    (hadm : ∀ e ∈ edges, PartitionAdm birthSite e.1 e.2)
    {u v : Nat} (h : EdgeConn edges u v) : SameRep birthSite u v := by
  induction h with
  | refl x => exact .refl x
  | edge h => exact .edge (hadm _ h)
  | symm _ ih => exact .symm ih
  | trans _ _ ih1 ih2 => exact .trans ih1 ih2

/-- §T1 (P2″) the executable partition is birth-site-sound END-TO-END: fold
    ANY admitted edge list; a shared COMPUTED representative implies a shared
    birth-site (`buildPartitionUF_sameRep_iff_edgeConn` composed with the
    admitted-edge lift and `samerep_birthsite_sound`). -/
theorem buildPartitionUF_birthsite_sound {β : Type} (birthSite : Nat → β)
    (edges : List (Nat × Nat))
    (hadm : ∀ e ∈ edges, PartitionAdm birthSite e.1 e.2)
    {a b : Nat} (h : (buildPartitionUF edges).sameRep a b = true) :
    birthSite a = birthSite b :=
  samerep_birthsite_sound
    (sameRep_of_edgeConn hadm
      ((buildPartitionUF_sameRep_iff_edgeConn edges a b).mp h))

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
