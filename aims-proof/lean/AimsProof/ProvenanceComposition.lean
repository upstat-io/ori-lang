/-
AIMS provenance-composition module — kernel-checked Lean proofs extending the
committed composition theorems CH-comp (`Coexistence.lean`), PL-comp
(`Pipeline.lean`), and VF-comp (`Verification.lean`) to cover the provenance
PARTITION as a TYPED SIDE-TABLE input — Annex E §AIMS §2 invariant 5,
admission avenue (c): a typed pre-pass input landing beside the state map
(like the immortals bitvector), never a new lattice dimension, never a
parallel emission path.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: CH-comp-PV (partition-as-side-table composition extension over the
    committed CH-comp / PL-comp / VF-comp) |
  spec: annex-e §AIMS §2 invariant 5 (typed pre-pass inputs) + §3 (decision
    predicates) + §6 (pipeline ordering) + §9 (verification layers) |
  .proof: aims-proof/proofs/12-provenance/comp-partition-side-table.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: the provenance partition is the T1 birth-site union-find class
map (`AimsProof.Partition`) computed by a Phase-5 pre-pass and consumed at
burden-op sites (`AimsProof.Ledger` takes it AS GIVEN as `classOf`). The
committed composition theorems were proven WITHOUT the partition input; this
module discharges the extension obligation: each composition still holds when
the partition side table is threaded as a typed input, because the table is
READ-ONLY relative to the composed handshake / pipeline / verification
structure — it refines elimination and verification decisions at class grain,
never mutates the state map, never spawns an emission path beside the lattice.

Structure:
  Part A — the side-table carrier: `PartitionTable` (the `Nat → Nat` class map
    `deriveLedger` already consumes), `partitionTableOf` over the executable T1
    union-find, and the identity tying table-class equality to the computed T1
    `sameRep` verdict (the committed `class_eq_iff_sameRep`).
  Part B — CH extension `CHcomp_partition_side_table`: the handshake
    composition is parametric over the table + a class-grain keep function.
    The gated refined verdict `eliminateAtClass` preserves CH-2's
    single-elimination (refined burden path = refined lattice path per program
    point), its verdict set is a SUBSET of the lattice verdict set (the
    lattice stays the optimizer authority; DP-2/DP-3 reused through the
    committed `eliminate`), the refined emitted set selects WITHIN the
    committed `emittedOps`, the table-consulting consumer preserves CH-4's
    state-map immutability, and the extended handshake keeps the CH-comp
    accept-iff-all union with the partition layer appended. A general
    subset-HYPOTHESIS form covers any refined eliminator. Instantiated at the
    committed T1 loop-CFG union-find (`pfUF`).
  Part C — negative witnesses (the invariant-5 parallel-path ban,
    machine-checked): a rogue class-grain eliminator that consults ONLY the
    partition fires on a Borrowed state the committed CH-2 verdict refuses —
    it violates the subset discipline and DIVERGES from the lattice verdict at
    a program point (a second elimination authority); a WRITING consumer that
    inserts into the state map provably breaks the CH-4 leg.
  Part D — PL extension `PLcomp_partition_side_table`: the committed PL-comp
    conjunction re-discharges with the partition pre-pass inserted at its slot
    (the analyze -> realize gap, mirroring CH-5's sequencing shape); every
    committed ordered pair survives the PL-6 admissible-insertion shift at the
    partition cut; every partition-table summary flow is non-stale under the
    committed PL-5 form; a pre-pass slotted AFTER its consumer is provably a
    stale read.
  Part E — VF extension `VFcomp_partition_side_table`: the partition-aware
    conformance layer (class-grain net-zero, T2's clause 1 over `deriveLedger`)
    layered onto the committed stack preserves the composed verdict — the
    extended stack accepts iff every layer accepts (through the committed
    VF-comp characterization), the new layer only ever REJECTS MORE, a
    base-layer failure still rejects with the layer appended, the extended
    catch map stays a complete pairwise-distinct partition, and the layer has
    teeth on the committed K1 past-merge double-free witness.
-/

import AimsProof.Partition
import AimsProof.Ledger
import AimsProof.Coexistence
import AimsProof.Pipeline
import AimsProof.Verification

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §PV Part A — the partition side-table carrier

    The provenance partition enters the composed structures as a TYPED,
    READ-ONLY side table: the per-value class map (a value's T1 union-find
    representative). It is exactly the `classOf : Nat → Nat` carrier the T2
    ledger engine consumes — no new carrier, no parallel partition notion. -/

/-- §PV the partition side table: the per-value same-allocation class map the
    T1 pre-pass computes (a typed pre-pass input per invariant 5 avenue (c)). -/
abbrev PartitionTable := Nat → Nat

/-- §PV the side table OF a built T1 union-find: value ↦ its COMPUTED
    representative through the executable fuelled `find`. -/
def partitionTableOf (uf : PartitionUF) : PartitionTable :=
  fun v => uf.find partitionFuel v

/-- §PV table-class equality IS the computed T1 `sameRep` verdict — the side
    table carries no partition notion of its own (directly the committed
    `class_eq_iff_sameRep`). -/
theorem partitionTableOf_eq_iff_sameRep (uf : PartitionUF) (u v : Nat) :
    partitionTableOf uf u = partitionTableOf uf v ↔ uf.sameRep u v = true :=
  class_eq_iff_sameRep uf u v

/-- Helper: the ALL-membership form over a single appended verdict — the list
    logic both extended accept-iff-all characterizations (CH + VF) reduce to. -/
theorem forall_mem_append_singleton (verdicts : List Bool) (pv : Bool) :
    (∀ x ∈ verdicts ++ [pv], x = true)
      ↔ ((∀ x ∈ verdicts, x = true) ∧ pv = true) := by
  constructor
  · intro h
    exact ⟨fun x hx => h x (List.mem_append.mpr (Or.inl hx)),
           h pv (List.mem_append.mpr (Or.inr (List.mem_cons_self ..)))⟩
  · rintro ⟨hall, hpv⟩ x hx
    rcases List.mem_append.mp hx with h1 | h2
    · exact hall x h1
    · rcases List.mem_cons.mp h2 with rfl | hnil
      · exact hpv
      · cases hnil

/-! ## §PV Part B — CH extension: class-grain refinement over the handshake

    The refined elimination verdict is the committed CH-2 consumer verdict
    GATED on the partition class's keep bit. The gating shape IS the subset
    discipline: the refinement can only KEEP (`classKeep` true) or DECLINE
    (`classKeep` false) an elimination the lattice already licensed — it can
    never fire outside the committed `eliminate` set, so the lattice stays
    the optimizer authority (the AIMS conflict rule). -/

/-- §PV-CH the class-grain refined elimination verdict over the burden-derived
    path: the committed `eliminate` gated on the value's partition class. -/
def eliminateAtClass (part : PartitionTable) (classKeep : Nat → Bool)
    (v : Nat) (s : AimsState) : Bool :=
  eliminate s && classKeep (part v)

/-- §PV-CH the class-grain refined verdict over the lattice-only
    (predicate-stack) path — the CH-2 twin under the same gate. -/
def eliminateLatticeAtClass (part : PartitionTable) (classKeep : Nat → Bool)
    (v : Nat) (s : AimsState) : Bool :=
  eliminate_lattice_only s && classKeep (part v)

/-- §PV-CH (CH-2 preserved) the refined burden-derived and refined
    lattice-derived verdicts are THE SAME boolean per (var, state) program
    point — class-grain refinement commutes with the committed CH-2
    single-elimination identity; still one decision, never two stacked ones. -/
theorem CHext_single_elimination_refined (part : PartitionTable)
    (classKeep : Nat → Bool) (v : Nat) (s : AimsState) :
    eliminateAtClass part classKeep v s
      = eliminateLatticeAtClass part classKeep v s := by
  unfold eliminateAtClass eliminateLatticeAtClass
  rw [CH2_single_elimination]

/-- §PV-CH the gated refinement satisfies the subset discipline BY
    CONSTRUCTION: a refined fire is a committed `eliminate` fire. -/
theorem CHext_gated_satisfies_subset (part : PartitionTable)
    (classKeep : Nat → Bool) (v : Nat) (s : AimsState)
    (h : eliminateAtClass part classKeep v s = true) : eliminate s = true := by
  have h' : (eliminate s && classKeep (part v)) = true := h
  rw [Bool.and_eq_true] at h'
  exact h'.1

/-- §PV-CH (subset — lattice authority) the refined verdict set is a SUBSET of
    the lattice verdict set: every refined fire is a lattice-only fire
    (through the committed CH-2 identity). -/
theorem CHext_refined_subset_of_lattice (part : PartitionTable)
    (classKeep : Nat → Bool) (v : Nat) (s : AimsState)
    (h : eliminateAtClass part classKeep v s = true) :
    eliminate_lattice_only s = true := by
  rw [← CH2_single_elimination]
  exact CHext_gated_satisfies_subset part classKeep v s h

/-- §PV-CH the refined verdict lands INSIDE the DP-2/DP-3 verdict-eliminable
    set: a refined elimination carries the lattice's DP-2-or-DP-3 verdict
    (reused verbatim from `Decision.lean` through the committed `eliminate`
    conjunction) — the refinement keeps-or-declines WITHIN that set only. -/
theorem CHext_refined_within_dp_verdicts (part : PartitionTable)
    (classKeep : Nat → Bool) (v : Nat) (s : AimsState)
    (h : eliminateAtClass part classKeep v s = true) :
    (is_rc_dec_unnecessary s || is_rc_inc_elidable s) = true := by
  have h1 : eliminate s = true := CHext_gated_satisfies_subset part classKeep v s h
  have h2 : (burden_emitted s
      && (is_rc_dec_unnecessary s || is_rc_inc_elidable s)) = true := h1
  rw [Bool.and_eq_true] at h2
  exact h2.2

/-- §PV-CH the refined emitted-op set over (var, state) pairs: the pairs whose
    refined verdict fires — the partition-aware sibling of the committed
    `emittedOps` (which is var-blind; the table needs the var). -/
def refinedEmittedOps (part : PartitionTable) (classKeep : Nat → Bool)
    (pairs : List (Nat × AimsState)) : List (Nat × AimsState) :=
  pairs.filter (fun p => eliminateAtClass part classKeep p.1 p.2)

/-- §PV-CH refined-emission membership characterization (mirrors the committed
    `CH2_emission_order_independent`): membership depends only on the refined
    verdict, never on position — refinement keeps CH-2's order-independence. -/
theorem CHext_refined_emission_order_independent (part : PartitionTable)
    (classKeep : Nat → Bool) (p : Nat × AimsState)
    (pairs : List (Nat × AimsState)) :
    p ∈ refinedEmittedOps part classKeep pairs
      ↔ (p ∈ pairs ∧ eliminateAtClass part classKeep p.1 p.2 = true) := by
  unfold refinedEmittedOps
  exact List.mem_filter

/-- §PV-CH (no parallel emission) the refined emitted set selects WITHIN the
    committed emitted-op set: every refined-emitted pair's state is a member
    of the committed `emittedOps` over the projected states — discharged
    through the committed CH-2 membership characterization. Class-grain
    refinement never emits BESIDE the lattice path. -/
theorem CHext_refined_ops_within_committed (part : PartitionTable)
    (classKeep : Nat → Bool) (p : Nat × AimsState)
    (pairs : List (Nat × AimsState))
    (h : p ∈ refinedEmittedOps part classKeep pairs) :
    p.2 ∈ emittedOps (pairs.map Prod.snd) := by
  obtain ⟨hmem, hfire⟩ :=
    (CHext_refined_emission_order_independent part classKeep p pairs).mp h
  exact (CH2_emission_order_independent p.2 (pairs.map Prod.snd)).mpr
    ⟨List.mem_map.mpr ⟨p, hmem, rfl⟩,
     CHext_gated_satisfies_subset part classKeep p.1 p.2 hfire⟩

/-- §PV-CH the general subset-HYPOTHESIS form: ANY class-grain refined verdict
    whose fires are within the committed `eliminate` set (the subset
    hypothesis) never fires outside the lattice-only set, and its emitted
    pairs project into the committed emitted-op set. The gated
    `eliminateAtClass` is the canonical instance
    (`CHext_gated_satisfies_subset` discharges its hypothesis). -/
theorem CHext_subset_eliminator_within_lattice
    (elimR : Nat → AimsState → Bool)
    (hsub : ∀ v s, elimR v s = true → eliminate s = true) :
    (∀ v s, elimR v s = true → eliminate_lattice_only s = true)
    ∧ (∀ (p : Nat × AimsState) (pairs : List (Nat × AimsState)),
        p ∈ pairs.filter (fun q => elimR q.1 q.2) →
          p.2 ∈ emittedOps (pairs.map Prod.snd)) := by
  constructor
  · intro v s h
    rw [← CH2_single_elimination]
    exact hsub v s h
  · intro p pairs hp
    obtain ⟨hmem, hfire⟩ := List.mem_filter.mp hp
    exact (CH2_emission_order_independent p.2 (pairs.map Prod.snd)).mpr
      ⟨List.mem_map.mpr ⟨p, hmem, rfl⟩, hsub p.1 p.2 hfire⟩

/-- §PV-CH the table-consulting elimination consumer: reads the frozen state
    map AND the partition side table, returns the map UNCHANGED (the committed
    CH-4 consumer is its map component) plus its refined per-event verdict —
    the side table refines the DECISION, never the state. -/
def eliminate_burden_ops_with_table (part : PartitionTable)
    (classKeep : Nat → Bool) (L : StateMap) (m : BurdenMutation) :
    StateMap × Bool :=
  ( eliminate_burden_ops L m
  , match L.lookup m.var with
    | some s => eliminateAtClass part classKeep m.var s
    | none => false )

/-- §PV-CH (CH-4 preserved) the table-consulting consumer returns the frozen
    state map unchanged — directly the committed CH-4 immutability threaded
    through the pair's map component. -/
theorem CHext_state_map_immutable_with_table (part : PartitionTable)
    (classKeep : Nat → Bool) (L : StateMap) (m : BurdenMutation) :
    (eliminate_burden_ops_with_table part classKeep L m).1 = L :=
  CH4_state_map_immutable L m

/-- §PV-CH per-variable lookup stability under the table-consulting consumer
    (the committed CH-4 corollary carried through the extension). -/
theorem CHext_lookup_stable_with_table (part : PartitionTable)
    (classKeep : Nat → Bool) (L : StateMap) (m : BurdenMutation) (v : Nat) :
    ((eliminate_burden_ops_with_table part classKeep L m).1).lookup v
      = L.lookup v := by
  rw [CHext_state_map_immutable_with_table]

/-- §PV-CH replaying the table-consulting consumer over a whole burden-event
    sequence (threading only its map component). -/
def replay_with_table (part : PartitionTable) (classKeep : Nat → Bool)
    (L : StateMap) (ms : List BurdenMutation) : StateMap :=
  ms.foldl (fun acc m => (eliminate_burden_ops_with_table part classKeep acc m).1) L

/-- §PV-CH replay immutability with the table consulted: the whole-sequence
    fold returns the original state map — the committed CH-4 replay theorem
    discharged through the map-component identity. -/
theorem CHext_replay_immutable_with_table (part : PartitionTable)
    (classKeep : Nat → Bool) (L : StateMap) (ms : List BurdenMutation) :
    replay_with_table part classKeep L ms = L := by
  have hfun : (fun (acc : StateMap) (m : BurdenMutation) =>
      (eliminate_burden_ops_with_table part classKeep acc m).1)
      = eliminate_burden_ops := by
    funext acc m
    rfl
  show ms.foldl
    (fun acc m => (eliminate_burden_ops_with_table part classKeep acc m).1) L = L
  rw [hfun]
  exact CH4_replay_immutable L ms

/-- §PV-CH the extended handshake accept-iff-all: appending the partition
    layer's verdict keeps the committed CH-comp union characterization — the
    extended handshake accepts iff every committed layer AND the partition
    layer accept. -/
theorem CHext_handshake_append_iff (verdicts : List Bool) (pv : Bool) :
    handshakeAccepts (verdicts ++ [pv]) = true
      ↔ ((∀ x ∈ verdicts, x = true) ∧ pv = true) := by
  rw [CHcomp_accepts_iff_all]
  exact forall_mem_append_singleton verdicts pv

/-- §PV-CH a regressed partition layer is caught by the union: appending a
    FALSE partition verdict rejects the whole handshake even when every
    committed layer accepts — the committed any-layer-fails theorem at the
    appended position. -/
theorem CHext_partition_layer_failure_rejected (verdicts : List Bool) :
    handshakeAccepts (verdicts ++ [false]) = false :=
  CHcomp_any_layer_fails_rejects verdicts []

/-- §PV-CH a committed-layer regression is STILL caught with the partition
    layer appended — extending the handshake weakens no committed catch. -/
theorem CHext_base_layer_failure_still_rejected (pre post : List Bool)
    (pv : Bool) :
    handshakeAccepts (pre ++ false :: (post ++ [pv])) = false :=
  CHcomp_any_layer_fails_rejects pre (post ++ [pv])

/-- §PV-CH THE coexistence-handshake extension (invariant 5 avenue (c)): the
    handshake composition is parametric over the partition side table — for
    EVERY table and EVERY class-grain keep function, (a) the refined
    burden-derived and lattice-derived verdicts remain a SINGLE decision per
    program point (CH-2 preserved), (b) the refined verdict set is a SUBSET of
    the lattice verdict set (the lattice stays the optimizer authority), (c)
    the refined emitted set selects WITHIN the committed emitted-op set (no
    parallel emission path), (d) the table-consulting consumer leaves the
    frozen state map untouched (CH-4 preserved), and (e) the extended
    handshake still accepts iff every layer INCLUDING the partition layer
    accepts (the CH-comp union keeps its teeth). -/
theorem CHcomp_partition_side_table (part : PartitionTable)
    (classKeep : Nat → Bool) :
    (∀ (v : Nat) (s : AimsState),
        eliminateAtClass part classKeep v s
          = eliminateLatticeAtClass part classKeep v s)
    ∧ (∀ (v : Nat) (s : AimsState),
        eliminateAtClass part classKeep v s = true →
          eliminate_lattice_only s = true)
    ∧ (∀ (p : Nat × AimsState) (pairs : List (Nat × AimsState)),
        p ∈ refinedEmittedOps part classKeep pairs →
          p.2 ∈ emittedOps (pairs.map Prod.snd))
    ∧ (∀ (L : StateMap) (m : BurdenMutation),
        (eliminate_burden_ops_with_table part classKeep L m).1 = L)
    ∧ (∀ (verdicts : List Bool) (pv : Bool),
        handshakeAccepts (verdicts ++ [pv]) = true
          ↔ ((∀ x ∈ verdicts, x = true) ∧ pv = true)) :=
  ⟨CHext_single_elimination_refined part classKeep,
   CHext_refined_subset_of_lattice part classKeep,
   CHext_refined_ops_within_committed part classKeep,
   CHext_state_map_immutable_with_table part classKeep,
   CHext_handshake_append_iff⟩

/-- §PV-CH the canonical Class-A entry state (the committed CH-1/CH-2
    positive-witness corner over `freshState`: Owned, Linear, Absent — DP-2
    fires — Unique). -/
def classAState : AimsState :=
  { freshState with access := .Owned, consumption := .Linear,
                    cardinality := .Absent, uniqueness := .Unique }

/-- §PV-CH instantiation witness over the COMMITTED T1 loop-CFG partition
    (`pfUF`, the real edge-fold union-find): with the side table = the
    computed T1 representatives and the keep bit pinned to the items class,
    the refined verdict (i) FIRES on a lattice-eliminable state at the
    loop-header node the T1 singleton witness unified into the items class,
    (ii) DECLINES the same state at the label-merge node the T1
    kill-criterion kept split — while the lattice verdict still fires
    (keep-or-decline strictly WITHIN the eliminable set), and (iii) the
    table's class equality IS the computed T1 `sameRep` verdict. -/
theorem CHext_instantiated_at_T1_partition :
    eliminateAtClass (partitionTableOf pfUF)
        (fun c => c == partitionTableOf pfUF 10) 12 classAState = true
    ∧ eliminateAtClass (partitionTableOf pfUF)
        (fun c => c == partitionTableOf pfUF 10) 22 classAState = false
    ∧ eliminate classAState = true
    ∧ (∀ u v : Nat, partitionTableOf pfUF u = partitionTableOf pfUF v
        ↔ pfUF.sameRep u v = true) :=
  ⟨by decide, by decide, by decide,
   fun u v => partitionTableOf_eq_iff_sameRep pfUF u v⟩

/-! ## §PV Part C — negative witnesses: the invariant-5 discipline has teeth

    The extension holds BECAUSE the side table is subset-disciplined and
    read-only. Both violations are machine-checked to break it: a class-grain
    eliminator consulting ONLY the partition (a second emission authority)
    fires outside the lattice set and diverges from the lattice verdict; a
    writing consumer mutates the frozen state map CH-4 protects. -/

/-- §PV-NEG a rogue class-grain "eliminator" that consults ONLY the partition:
    it fires on every member of a trusted class, ignoring the lattice verdict
    — the parallel-emission-path shape invariant 5 bans. -/
def rogueEliminateAtClass (part : PartitionTable) (trusted : Nat)
    (v : Nat) (_s : AimsState) : Bool :=
  part v == trusted

/-- §PV-NEG the canonical outside-the-set victim: a Borrowed state (whose RC
    is the caller's responsibility — the committed CH-2 negative witness). -/
def borrowedVictim : AimsState :=
  { freshState with access := .Borrowed, consumption := .Linear,
                    uniqueness := .Unique, cardinality := .One }

/-- §PV-NEG the rogue eliminator provably BREAKS the extended composition:
    (a) it fires on the Borrowed victim while the committed lattice-only
    verdict refuses it — its verdict set is NOT a subset of the lattice set
    (the `CHcomp_partition_side_table` subset leg fails for it, so the
    general subset-hypothesis form rejects it), and (b) its verdict DIVERGES
    from the lattice verdict at that program point — two different
    elimination answers, the stacked-decision failure class CH-2 exists to
    refuse. Eliminating a borrowed value's release obligation is the
    double-free the discipline makes unrepresentable. -/
theorem CHext_rogue_breaks_extended_composition :
    rogueEliminateAtClass id 7 7 borrowedVictim = true
    ∧ eliminate_lattice_only borrowedVictim = false
    ∧ ¬ (∀ (v : Nat) (s : AimsState),
          rogueEliminateAtClass id 7 v s = true → eliminate s = true)
    ∧ rogueEliminateAtClass id 7 7 borrowedVictim
        ≠ eliminate_lattice_only borrowedVictim := by
  have hfired : rogueEliminateAtClass id 7 7 borrowedVictim = true := by decide
  have helim : eliminate borrowedVictim = false :=
    CH2_borrowed_not_eliminated .Linear .Unique .One freshState
  have hlat : eliminate_lattice_only borrowedVictim = false := by
    rw [← CH2_single_elimination]
    exact helim
  refine ⟨hfired, hlat, ?_, ?_⟩
  · intro hsub
    have hcontra := hsub 7 borrowedVictim hfired
    rw [helim] at hcontra
    exact absurd hcontra (by decide)
  · intro heq
    rw [hfired, hlat] at heq
    exact absurd heq (by decide)

/-- §PV-NEG a WRITING "consumer" — one that inserts a synthesized entry into
    the state map instead of returning it verbatim. -/
def rogueWritingConsumer (L : StateMap) (m : BurdenMutation) : StateMap :=
  (m.var, classAState) :: L

/-- §PV-NEG the writing consumer provably breaks the CH-4 leg: the map
    changes and a per-variable lookup diverges — while the committed
    (read-only) consumer at the same instance returns the map verbatim. The
    read-only discipline is load-bearing, not stylistic. -/
theorem CHext_writing_consumer_breaks_immutability :
    rogueWritingConsumer [] ⟨0, true⟩ ≠ ([] : StateMap)
    ∧ (rogueWritingConsumer [] ⟨0, true⟩).lookup 0 ≠ ([] : StateMap).lookup 0
    ∧ eliminate_burden_ops ([] : StateMap) ⟨0, true⟩ = ([] : StateMap) := by
  refine ⟨?_, ?_, CH4_state_map_immutable ([] : StateMap) ⟨0, true⟩⟩ <;> decide

/-! ## §PV Part D — PL extension: the partition pre-pass at its schedule slot

    The pre-pass computes the side table AFTER `AnalyzeFunction` (it reads the
    converged state map) and BEFORE `RealizeRcReuse` (its first consumer) —
    the CH-5 burden-pre-pass sequencing shape. Modeled over the committed
    PL-6 admissible-insertion shift at the realize cut, with the committed
    PL-5 summary-flow non-staleness form over the table's flows. -/

/-- §PV-PL the partition pre-pass insertion cut: the realize position — the
    pre-pass occupies the analyze -> realize gap. -/
def partitionPrePassCut : Nat := PipelineStep.RealizeRcReuse.position

/-- §PV-PL the inserted pre-pass position under the PL-6 shift (the pass at
    the cut; every phase at-or-after it moves up by one). -/
def partitionPrePassPos : Nat := partitionPrePassCut

/-- §PV-PL the partition-table summary flows: the analyze output feeds the
    pre-pass; the pre-pass output feeds the (shifted) realize + annotations
    consumers. -/
def partitionSummaryFlows : List SummaryFlow :=
  [ SummaryFlow.mk
      (PL6.shift partitionPrePassCut PipelineStep.AnalyzeFunction.position)
      partitionPrePassPos
  , SummaryFlow.mk partitionPrePassPos
      (PL6.shift partitionPrePassCut PipelineStep.RealizeRcReuse.position)
  , SummaryFlow.mk partitionPrePassPos
      (PL6.shift partitionPrePassCut PipelineStep.RealizeAnnotations.position) ]

/-- §PV-PL every partition-table summary flow is non-stale — each read
    strictly follows its production — through the committed PL-5 ALL-reads
    form over the concrete flow list (downstream consumers see the COMPUTED
    table, never a stale one). -/
theorem PLext_partition_flows_nonstale :
    ∀ f ∈ partitionSummaryFlows, f.prod < f.read := by
  apply PL5_no_stale_summaries
  intro f hf
  simp only [partitionSummaryFlows, List.mem_cons, List.not_mem_nil,
    or_false] at hf
  rcases hf with rfl | rfl | rfl <;>
    (unfold SummaryFlow.nonStale partitionPrePassPos partitionPrePassCut
      PL6.shift PipelineStep.position; decide)

/-- §PV-PL NEGATIVE a pre-pass slotted AFTER its consumer is a STALE read: the
    flow whose production is the (shifted) realize position and whose read is
    the pre-pass slot fails the committed PL-5 non-staleness — the
    compute-after-analyze-before-realize sequencing is load-bearing. -/
theorem PLext_consumer_before_prepass_stale :
    ¬ (SummaryFlow.mk
        (PL6.shift partitionPrePassCut PipelineStep.RealizeRcReuse.position)
        partitionPrePassPos).nonStale := by
  unfold SummaryFlow.nonStale partitionPrePassPos partitionPrePassCut
    PL6.shift PipelineStep.position
  decide

/-- §PV-PL the pre-pass insertion preserves EVERY committed ordered pair: any
    `before` pair survives the shift — the committed PL-6
    admissible-insertion gate instantiated at the partition cut. -/
theorem PLext_insertion_preserves_committed_pairs (a b : PipelineStep)
    (hab : a.before b) :
    PL6.shift partitionPrePassCut a.position
      < PL6.shift partitionPrePassCut b.position :=
  PL6_insertion_preserves_order partitionPrePassCut a b hab

/-- §PV-PL producer-before-consumer around the slot: analyze (before the cut,
    unshifted) strictly precedes the pre-pass, and the pre-pass strictly
    precedes the shifted realize — the committed PL-2 analyze-before-realize
    pair (CH-5's sequencing anchor) carried through the insertion with the
    pre-pass in the gap; no back-edge. -/
theorem PLext_prepass_in_analyze_realize_gap :
    PL6.shift partitionPrePassCut PipelineStep.AnalyzeFunction.position
        < partitionPrePassPos
    ∧ partitionPrePassPos
        < PL6.shift partitionPrePassCut PipelineStep.RealizeRcReuse.position
    ∧ PipelineStep.AnalyzeFunction.before PipelineStep.RealizeRcReuse := by
  refine ⟨?_, ?_, PL2_analyze_before_realize⟩ <;>
    (unfold partitionPrePassPos partitionPrePassCut PL6.shift
      PipelineStep.position; decide)

/-- §PV-PL THE pipeline-composition extension: the committed PL-comp
    conjunction RE-DISCHARGES with the partition pre-pass inserted at its slot
    (the pre-pass adds no constraint on the constituent invariants — the
    committed `PLcomp_pipeline_composes` supplies the whole conjunction
    verbatim), every committed ordered pair survives the insertion shift, the
    pre-pass sits in the analyze -> realize gap (computed before consumption,
    after analyze — the CH-5 sequencing mirror), and every partition-table
    summary flow reads strictly after its production (no stale-summary
    violation). -/
theorem PLcomp_partition_side_table
    (G : Condensation) (i j : Nat) (hedge : G.Edge i j)
    (c : ContextBehavior)
    (r : TrmcRewrite) (hr : r.wellFormed)
    (v : TrmcVerify) (hv : v.allPass = true) :
    (PipelineStep.ApplyOwnership.before PipelineStep.RealizeRcReuse ∧
      i < j ∧
      PipelineStep.AnalyzeFunction.before PipelineStep.RealizeRcReuse ∧
      PipelineStep.RealizeRcReuse.before PipelineStep.MergeBlocks ∧
      PipelineStep.MergeBlocks.before PipelineStep.RealizeAnnotations ∧
      PipelineStep.UnwindCleanup.before PipelineStep.MergeBlocks ∧
      c.join c = c ∧ ContextBehavior.OPTIMISTIC.join c = c ∧
      r.external_arity_after = r.external_arity_before ∧
      v.rewriteSurvives = true)
    ∧ (∀ a b : PipelineStep, a.before b →
        PL6.shift partitionPrePassCut a.position
          < PL6.shift partitionPrePassCut b.position)
    ∧ (PL6.shift partitionPrePassCut PipelineStep.AnalyzeFunction.position
          < partitionPrePassPos
        ∧ partitionPrePassPos
          < PL6.shift partitionPrePassCut PipelineStep.RealizeRcReuse.position)
    ∧ (∀ f ∈ partitionSummaryFlows, f.prod < f.read) :=
  ⟨PLcomp_pipeline_composes G i j hedge c r hr v hv,
   fun a b hab => PLext_insertion_preserves_committed_pairs a b hab,
   ⟨PLext_prepass_in_analyze_realize_gap.1,
    PLext_prepass_in_analyze_realize_gap.2.1⟩,
   PLext_partition_flows_nonstale⟩

/-! ## §PV Part E — VF extension: the partition-aware conformance layer

    The new layer checks class-grain net-zero (T2's clause 1 over the derived
    per-class ledger) — checkable ONLY with the partition side table in hand.
    Layered onto the committed stack it preserves the composed verdict: it
    only ever REJECTS MORE, never accepts a shape a prior layer rejects. -/

/-- §PV-VF the partition-aware conformance verdict: EVERY class in the checked
    set satisfies the T2 clause-1 net-zero over the derived per-class ledger. -/
def partitionLayerVerdict (part : PartitionTable) (classes : List Nat)
    (instrs : List LedgerInstr) : Bool :=
  classes.all (fun c => clauseNetZero (deriveLedger part c instrs))

/-- §PV-VF clause 1 extracted from a committed three-clause discharge (the
    first conjunct of the committed `threeClauses`). -/
theorem clauseNetZero_of_threeClauses (es : List LedgerEvent)
    (h : threeClauses es = true) : clauseNetZero es = true := by
  have h' : (clauseNetZero es && clauseFloors 0 es) = true := h
  rw [Bool.and_eq_true] at h'
  exact h'.1

/-- §PV-VF the extended stack accept-iff-all: appending the partition layer's
    verdict keeps the committed VF-comp union characterization. -/
theorem VFext_stack_append_iff (verdicts : List Bool) (pv : Bool) :
    stackAccepts (verdicts ++ [pv]) = true
      ↔ ((∀ x ∈ verdicts, x = true) ∧ pv = true) := by
  rw [VFcomp_stack_accepts_iff_all]
  exact forall_mem_append_singleton verdicts pv

/-- §PV-VF the new layer only REJECTS MORE: an extended-stack accept implies
    the base-stack accept — the partition layer can never accept a shape a
    prior layer rejects (composition soundness preserved). -/
theorem VFext_layer_only_rejects_more (verdicts : List Bool) (pv : Bool)
    (h : stackAccepts (verdicts ++ [pv]) = true) :
    stackAccepts verdicts = true := by
  obtain ⟨hall, _⟩ := (VFext_stack_append_iff verdicts pv).mp h
  exact (VFcomp_stack_accepts_iff_all verdicts).mpr hall

/-- §PV-VF a committed-layer failure is STILL caught with the partition layer
    appended — extending the stack weakens no committed catch. -/
theorem VFext_base_failure_still_rejected (pre post : List Bool) (pv : Bool) :
    stackAccepts (pre ++ false :: (post ++ [pv])) = false :=
  VFcomp_any_layer_fails_stack_rejects pre (post ++ [pv])

/-- §PV-VF the extended layer roster: the four committed detecting layers plus
    the partition-conformance layer. -/
inductive LayerPv
  | base (l : Layer)
  | partitionConformance
deriving Repr, DecidableEq

/-- §PV-VF the extended failure-class universe: the committed four plus the
    partition-grain imbalance class (a class-grain net violation no committed
    class names). -/
inductive FailureClassPv
  | base (f : FailureClass)
  | partitionImbalance
deriving Repr, DecidableEq

/-- §PV-VF the extended catch map: committed layers keep their committed
    classes (through the committed `Layer.catches`); the partition layer
    catches the partition-grain imbalance. -/
def LayerPv.catches : LayerPv → FailureClassPv
  | .base l => .base l.catches
  | .partitionConformance => .partitionImbalance

/-- §PV-VF the extended catch map stays INJECTIVE — distinct extended layers
    catch distinct extended classes: the committed VF-comp injectivity lifted
    through the embedding; the new layer's class collides with no committed
    class by constructor disjointness. -/
theorem VFext_catches_injective (l1 l2 : LayerPv)
    (h : l1.catches = l2.catches) : l1 = l2 := by
  cases l1 with
  | base b1 =>
      cases l2 with
      | base b2 =>
          have hb : b1.catches = b2.catches := by
            simpa only [LayerPv.catches, FailureClassPv.base.injEq] using h
          exact congrArg LayerPv.base (VFcomp_layers_catch_distinct b1 b2 hb)
      | partitionConformance => simp [LayerPv.catches] at h
  | partitionConformance =>
      cases l2 with
      | base b2 => simp [LayerPv.catches] at h
      | partitionConformance => rfl

/-- §PV-VF the extended catch map stays SURJECTIVE — every extended failure
    class is caught by some extended layer: committed classes through the
    committed VF-comp surjectivity; the imbalance class by the new layer. -/
theorem VFext_every_class_caught (f : FailureClassPv) :
    ∃ l : LayerPv, l.catches = f := by
  cases f with
  | base fb =>
      obtain ⟨l, hl⟩ := VFcomp_every_class_caught fb
      exact ⟨.base l, by simp only [LayerPv.catches, hl]⟩
  | partitionImbalance => exact ⟨.partitionConformance, rfl⟩

/-- §PV-VF the partition layer ACCEPTS the committed K1 CURED placement: the
    class-grain net-zero holds on both K1 walks, extracted from the committed
    three-clause discharge — the layer rejects only genuinely-imbalanced
    shapes. -/
theorem VFext_partition_layer_accepts_k1_cured :
    partitionLayerVerdict k1ClassOf [k1Class]
        (walkInstrs k1Cured k1NormalWalk) = true
    ∧ partitionLayerVerdict k1ClassOf [k1Class]
        (walkInstrs k1Cured k1UnwindWalk) = true := by
  constructor
  · simp only [partitionLayerVerdict, List.all_cons, List.all_nil,
      Bool.and_true]
    exact clauseNetZero_of_threeClauses _ T2_K1_cured_clauses.1
  · simp only [partitionLayerVerdict, List.all_cons, List.all_nil,
      Bool.and_true]
    exact clauseNetZero_of_threeClauses _ T2_K1_cured_clauses.2

/-- §PV-VF the partition layer's TEETH over a COMMITTED kill-criterion
    instance: the K1 past-merge relocation (the committed T2 double-free
    witness — the unwind path nets -1) fails the class-grain net-zero, so the
    partition layer verdict is FALSE — and the extended stack REJECTS it even
    when every committed layer accepts (the committed 4-true verdict list
    alone ACCEPTS; the appended partition layer is what catches the
    partition-grain imbalance). -/
theorem VFext_partition_layer_catches_k1_double_free :
    partitionLayerVerdict k1ClassOf [k1Class]
        (walkInstrs k1PastMerge k1UnwindWalk) = false
    ∧ stackAccepts [true, true, true, true] = true
    ∧ stackAccepts ([true, true, true, true]
        ++ [partitionLayerVerdict k1ClassOf [k1Class]
              (walkInstrs k1PastMerge k1UnwindWalk)]) = false := by
  have hpv : partitionLayerVerdict k1ClassOf [k1Class]
      (walkInstrs k1PastMerge k1UnwindWalk) = false := by decide
  refine ⟨hpv, VFcomp_all_pass_accepted, ?_⟩
  rw [hpv]
  exact VFcomp_any_layer_fails_stack_rejects [true, true, true, true] []

/-- §PV-VF THE verification-stack extension: layering the partition-aware
    conformance check onto the committed stack (a) keeps the accept-iff-all
    union (through the committed VF-comp characterization), (b) only ever
    REJECTS MORE (an extended accept implies the base accept), (c) still
    catches every committed-layer failure with the layer appended, (d) keeps
    the catch map a complete pairwise-distinct partition over the EXTENDED
    failure-class universe, and (e) has teeth on the committed K1 past-merge
    double-free: the class-grain net-zero fails and the extended stack rejects
    a shape every committed layer accepted. -/
theorem VFcomp_partition_side_table :
    (∀ (verdicts : List Bool) (pv : Bool),
        stackAccepts (verdicts ++ [pv]) = true
          ↔ ((∀ x ∈ verdicts, x = true) ∧ pv = true))
    ∧ (∀ (verdicts : List Bool) (pv : Bool),
        stackAccepts (verdicts ++ [pv]) = true → stackAccepts verdicts = true)
    ∧ (∀ (pre post : List Bool) (pv : Bool),
        stackAccepts (pre ++ false :: (post ++ [pv])) = false)
    ∧ (∀ l1 l2 : LayerPv, l1.catches = l2.catches → l1 = l2)
    ∧ (∀ f : FailureClassPv, ∃ l : LayerPv, l.catches = f)
    ∧ (partitionLayerVerdict k1ClassOf [k1Class]
          (walkInstrs k1PastMerge k1UnwindWalk) = false
        ∧ stackAccepts ([true, true, true, true]
            ++ [partitionLayerVerdict k1ClassOf [k1Class]
                  (walkInstrs k1PastMerge k1UnwindWalk)]) = false) :=
  ⟨VFext_stack_append_iff,
   VFext_layer_only_rejects_more,
   VFext_base_failure_still_rejected,
   VFext_catches_injective,
   VFext_every_class_caught,
   ⟨VFext_partition_layer_catches_k1_double_free.1,
    VFext_partition_layer_catches_k1_double_free.2.2⟩⟩

/-! ## §PV conclusion bundle -/

/-- §PV the extension bundle: one load-bearing leg per extended composition
    (CH-2 single-elimination preserved under class-grain refinement; the
    partition pre-pass flows non-stale at its schedule slot; the partition
    verification layer only rejects more) plus the two discipline-violation
    rejections (a subset-violating eliminator and a state-map-writing
    consumer both provably break the extension). -/
theorem PV_composition_extension_bundle :
    (∀ (part : PartitionTable) (classKeep : Nat → Bool) (v : Nat)
        (s : AimsState),
        eliminateAtClass part classKeep v s
          = eliminateLatticeAtClass part classKeep v s)
    ∧ (∀ f ∈ partitionSummaryFlows, f.prod < f.read)
    ∧ (∀ (verdicts : List Bool) (pv : Bool),
        stackAccepts (verdicts ++ [pv]) = true → stackAccepts verdicts = true)
    ∧ ¬ (∀ (v : Nat) (s : AimsState),
          rogueEliminateAtClass id 7 v s = true → eliminate s = true)
    ∧ rogueWritingConsumer [] ⟨0, true⟩ ≠ ([] : StateMap) :=
  ⟨CHext_single_elimination_refined,
   PLext_partition_flows_nonstale,
   VFext_layer_only_rejects_more,
   CHext_rogue_breaks_extended_composition.2.2.1,
   CHext_writing_consumer_breaks_immutability.1⟩

end AimsProof
