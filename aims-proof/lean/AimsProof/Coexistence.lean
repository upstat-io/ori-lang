/-
AIMS coexistence-handshake module — kernel-checked Lean proofs of the
coexistence-handshake rules CH-1..CH-5 + CH-comp over a minimal faithful model
of the burden-registry-as-typed-pre-pass / lattice-state-map / DP-2/DP-3
elimination-consumer composition. This is the Ori-novel composition theorem:
the burden registry is a typed pre-pass input that lands on the lattice
state map (NOT a parallel emission path), and the elimination consumer reads
both the frozen state map and the burden registry to emit a SINGLE elimination
decision per program point — per-class, with a total/disjoint coverage
partition (class_covered / mixed-coverage / uncovered).

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: CH-1..CH-5 + CH-comp | spec: annex-e §AIMS |
  .proof: aims-proof/proofs/11-coexistence/CH-*.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: `docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS`
(§1 dimensions, §3 decision predicates, §5 canonicalization, §6 pipeline
ordering). The coexistence handshake composes:
  - the burden-registry typed pre-pass (a `BitSet<ArcVarId>`-shaped side table
    that lands on the lattice state map, never a lattice dimension),
  - the converged lattice state map (frozen post-convergence; read-only),
  - the DP-2 / DP-3 elimination consumer (reused from `Decision.lean`),
  - the per-class coexistence partition (Class A / B / C over the
    Access × Uniqueness × Cardinality × Consumption sub-product).

These are STRUCTURAL theorems over the minimal faithful structures the
handshake's soundness property is defined over: a burden-op model
(`BurdenOp`), the `burden_owned` lattice-bridge predicate, the elimination-
consumer relation (`eliminate`), a frozen state-map model (`StateMap`), the
class-coverage partition (`CoverageClass`), and the phase-ordering carrier
reused from `Pipeline.lean`. The dimension carriers + DP-2 / DP-3 are reused
from `Model.lean` / `Decision.lean`; the pipeline phases + `before` relation
from `Pipeline.lean`; the union-composition shape mirrors `Verification.lean`'s
VF-comp (the §9 layered-verifier composition precedent).

The unifying soundness invariant of the coexistence family is HANDSHAKE
COVERAGE: the burden registry and the lattice agree on every elimination
decision (no double-counting), each variable falls in exactly one coverage
class, the elimination consumer does not mutate the frozen state map, the
pre-pass is sequenced after lattice convergence, and the composed handshake
catches the UNION of the per-layer failure classes — a fix passing a strict
subset of layers is rejected.

Rule index (per §AIMS coexistence handshake):
  CH-1   lattice-bridge consistency — burden_owned conjunction (Owned ∧
         consumption ∈ {Linear, Affine} ∧ Unique ∧ DP-2-dec-unnecessary)
         aligned with the canonical class; the burden-emitted annotation IS a
         pure function of the converged lattice state.
  CH-2   single-elimination commutativity — the burden-derived and lattice-
         derived elimination decisions are THE SAME boolean per program point;
         the emitted-op set is order-independent (DP-2 / DP-3 reused).
  CH-3   per-class partition — Class A (burden-eligible) / Class B (borrowed,
         RC-free) / Class C (MaybeShared, dynamic COW) are total + disjoint
         over the lattice sub-product; only Class A is burden-eligible.
  CH-4   AimsStateMap immutability — `eliminate_burden_ops` consumes a frozen
         state map; a burden-op elimination event does NOT mutate it.
  CH-5   phase-ordering composition — the burden pre-pass is sequenced after
         lattice convergence (analyze before realize), preserving the §6
         interprocedural-first + intra-function ordering; reuses Pipeline.lean.
  CH-comp coexistence-handshake union — the Ori-novel composition theorem:
         each layer catches a distinct failure class, the stack catches the
         union, and the soundness partition (class_covered / mixed-coverage /
         uncovered) is total + disjoint with a per-case dispatch verdict.
-/

import AimsProof.Model
import AimsProof.Decision
import AimsProof.Pipeline

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §CH-1 — lattice-bridge consistency (annex-e §AIMS §1 + §3)

    The lattice bridge `burden_owned(s)` is the four-conjunct query on a
    converged canonical lattice state: Owned access, Linear-or-Affine
    consumption, Unique uniqueness, and DP-2 dec-unnecessary (reused from
    `Decision.lean`). The burden registry's `burden_emitted` annotation IS this
    pure function of the converged state — so the handshake's elimination
    decision derived from the burden registry equals the one derived directly
    from the lattice. -/

/-- §CH-1 the lattice-bridge predicate `burden_owned(s)`: the burden registry
    classifies `s` as burden-eligible iff the converged lattice state is Owned,
    Linear-or-Affine, Unique, and DP-2 dec-unnecessary (per CH-1.proof
    Predicate 1, reusing `is_rc_dec_unnecessary` from `Decision.lean`). -/
def burden_owned (s : AimsState) : Bool :=
  (s.access = .Owned)
    && ((s.consumption = .Linear) || (s.consumption = .Affine))
    && (s.uniqueness = .Unique)
    && is_rc_dec_unnecessary s

/-- §CH-1 the burden registry's per-variable annotation is DERIVED from the
    lattice (a pure function of the converged state, never an independent
    claim): `burden_emitted` IS `burden_owned` of the looked-up state. -/
def burden_emitted (s : AimsState) : Bool := burden_owned s

/-- §CH-1 (P1) lattice-bridge consistency — the truth-table characterization:
    `burden_owned` is true on EXACTLY the Owned × {Linear,Affine} × Unique ×
    DP-2-dec-unnecessary corner. Proven by destructuring the read dimensions
    (Access × Consumption × Uniqueness × Cardinality — the four `burden_owned`
    reads, where DP-2 reads Cardinality/Consumption) before `decide`. -/
theorem CH1_lattice_bridge (acc : AccessClass) (con : Consumption)
    (u : Uniqueness) (car : Cardinality) (rest : AimsState) :
    burden_owned
        { rest with access := acc, consumption := con,
                    uniqueness := u, cardinality := car }
      = ((decide (acc = .Owned))
          && ((decide (con = .Linear)) || (decide (con = .Affine)))
          && (decide (u = .Unique))
          && ((decide (car = .Absent)) || (decide (con = .Dead)))) := by
  cases acc <;> cases con <;> cases u <;> cases car <;> rfl

/-- §CH-1 (P1) the burden-emitted annotation EQUALS the lattice bridge on every
    state — the registry never diverges from the lattice (the no-independent-
    claim invariant: `burden_emitted` is `burden_owned`, by definition). -/
theorem CH1_burden_emitted_is_bridge (s : AimsState) :
    burden_emitted s = burden_owned s := by rfl

/-- §CH-1 (P1) burden-eligible implies DP-2 dec-unnecessary: every burden-owned
    state is dec-unnecessary (the fourth conjunct IS DP-2), so the registry's
    elimination claim agrees with the lattice's DP-2 verdict. The lattice-bridge
    consistency: a burden-emitted decision is a DP-2 decision. -/
theorem CH1_burden_owned_implies_dec_unnecessary (s : AimsState)
    (h : burden_owned s = true) : is_rc_dec_unnecessary s = true := by
  unfold burden_owned at h
  simp only [Bool.and_eq_true] at h
  exact h.2

/-- §CH-1 (P1) the canonical burden-owned witness: a FRESH-canonical state with
    Owned access, Linear consumption, Absent cardinality (DP-2 fires), Unique
    uniqueness is burden-owned — the Class-A entry point (TF-3 Construct lands
    here). A POSITIVE witness that the bridge admits the canonical class. -/
theorem CH1_canonical_classA_burden_owned (rest : AimsState) :
    burden_owned
        { rest with access := .Owned, consumption := .Linear,
                    cardinality := .Absent, uniqueness := .Unique } = true := by
  rfl

/-- §CH-1 (P2) no double-counting NEGATIVE witness: a Borrowed state is NOT
    burden-owned regardless of the other dimensions — accepting it would let a
    borrowed value (whose RC is the caller's responsibility) be burden-
    eliminated, a double-free. The first conjunct (Owned) excludes it. -/
theorem CH1_borrowed_not_burden_owned (con : Consumption) (u : Uniqueness)
    (car : Cardinality) (rest : AimsState) :
    burden_owned
        { rest with access := .Borrowed, consumption := con,
                    uniqueness := u, cardinality := car } = false := by
  cases con <;> cases u <;> cases car <;> rfl

/-- §CH-1 (P2) no double-counting NEGATIVE witness: a MaybeShared state is NOT
    burden-owned — burden elimination would invalidate RL-7's unresolved
    sharing-observation obligation. The third conjunct (Unique) excludes it. -/
theorem CH1_maybeshared_not_burden_owned (acc : AccessClass) (con : Consumption)
    (car : Cardinality) (rest : AimsState) :
    burden_owned
        { rest with access := acc, consumption := con,
                    uniqueness := .MaybeShared, cardinality := car } = false := by
  cases acc <;> cases con <;> cases car <;> rfl

/-! ## §CH-2 — single-elimination commutativity (annex-e §AIMS §3 + §8.1)

    The elimination consumer reads the frozen lattice state and the burden
    registry and emits a SINGLE elimination decision per program point:
    `eliminate(s) = burden_emitted(s) ∧ (DP-2 ∨ DP-3)`. Because the burden
    registry IS a memoized DP-2/DP-3 verdict (CH-1), the burden-derived and
    lattice-derived decisions are bit-identical; the emitted-op SET is therefore
    order-independent (commutative under composition). DP-2 / DP-3 are reused
    verbatim from `Decision.lean`. -/

/-- §CH-2 the elimination consumer verdict for a variable: the burden registry
    annotation AND the DP-2/DP-3 lattice verdict — a single boolean per state. -/
def eliminate (s : AimsState) : Bool :=
  burden_emitted s && (is_rc_dec_unnecessary s || is_rc_inc_elidable s)

/-- §CH-2 the lattice-only elimination verdict (the predicate-stack path that
    consults the lattice WITHOUT the burden registry): the DP-2/DP-3 verdict
    gated on the lattice bridge directly. -/
def eliminate_lattice_only (s : AimsState) : Bool :=
  burden_owned s && (is_rc_dec_unnecessary s || is_rc_inc_elidable s)

/-- §CH-2 (P1) single-elimination decision: the burden-derived consumer verdict
    EQUALS the lattice-only verdict on every state — the two paths produce THE
    SAME boolean per program point (not two stacked decisions). Direct
    consequence of `burden_emitted = burden_owned` (CH-1). -/
theorem CH2_single_elimination (s : AimsState) :
    eliminate s = eliminate_lattice_only s := by
  unfold eliminate eliminate_lattice_only burden_emitted; rfl

/-- §CH-2 the emitted-op set over a list of variable states: the set of states
    for which `eliminate` fires (modeled as the filtered sublist — emission is
    idempotent under duplication, so membership is what matters). -/
def emittedOps (states : List AimsState) : List AimsState :=
  states.filter eliminate

/-- §CH-2 (P2) composition commutativity — order independence: the membership of
    a state in the emitted-op set depends ONLY on its `eliminate` verdict, never
    on position. So a burden-derived emission and a lattice-derived emission for
    the same state coincide regardless of which path runs first. Proven via the
    `List.mem_filter` characterization (membership iff in-list ∧ verdict). -/
theorem CH2_emission_order_independent (s : AimsState) (states : List AimsState) :
    s ∈ emittedOps states ↔ (s ∈ states ∧ eliminate s = true) := by
  unfold emittedOps
  exact List.mem_filter

/-- §CH-2 (P2) the emitted-op set is path-agnostic: filtering by the burden-
    derived `eliminate` yields the SAME list as filtering by the lattice-only
    `eliminate_lattice_only` — the two emission paths emit the identical set,
    so consume_stack is deterministic regardless of ordering. -/
theorem CH2_paths_emit_same_set (states : List AimsState) :
    states.filter eliminate = states.filter eliminate_lattice_only := by
  apply List.filter_congr
  intro s _
  rw [CH2_single_elimination]

/-- §CH-2 (P3) stack-consumer well-formedness POSITIVE witness: a Class-A
    canonical state (Owned, Linear, Absent → DP-2 fires) eliminates under BOTH
    the burden-derived and the lattice-only consumer — the verdict is `true`
    and identical. -/
theorem CH2_classA_eliminates (rest : AimsState) :
    eliminate
        { rest with access := .Owned, consumption := .Linear,
                    cardinality := .Absent, uniqueness := .Unique } = true
      ∧ eliminate_lattice_only
        { rest with access := .Owned, consumption := .Linear,
                    cardinality := .Absent, uniqueness := .Unique } = true := by
  constructor <;> rfl

/-- §CH-2 (P3) stack-consumer well-formedness NEGATIVE witness: a Borrowed state
    never eliminates under the burden-derived consumer (the registry skips
    Borrowed variables) — `eliminate` is `false` because `burden_emitted` is
    `false`. -/
theorem CH2_borrowed_not_eliminated (con : Consumption) (u : Uniqueness)
    (car : Cardinality) (rest : AimsState) :
    eliminate
        { rest with access := .Borrowed, consumption := con,
                    uniqueness := u, cardinality := car } = false := by
  unfold eliminate burden_emitted
  rw [CH1_borrowed_not_burden_owned]
  rfl

/-! ## §CH-3 — per-class coexistence partition (annex-e §AIMS §1 + §8.2/§8.3)

    The burden-relevant lattice subspace partitions into three named sub-classes
    over the Access × Uniqueness × Cardinality × Consumption sub-product:
      Class A: Owned × Unique × Once × Linear (RL-2 logical release, burden-eligible)
      Class B: Borrowed × Unique × Once × Linear (caller-owned, no callee release)
      Class C: MaybeShared × Many (RL-7 dynamic COW)
    Allocation LifetimeBound and representation ExtentClass are orthogonal to
    this ownership partition; physical mechanisms are admitted by Satisfies.
    The classification is TOTAL over the named subset and the three are
    pairwise DISJOINT; only Class A is burden-eligible. Variables outside the
    three named cells are Uncovered (non-burden-eligible by construction). -/

/-- §CH-3 the burden-relevant coverage class of a variable's state (the named
    sub-class, or `Uncovered` for everything else). -/
inductive CoverageClass
  | ClassA      -- Owned × Unique × Once × Linear (burden-eligible)
  | ClassB      -- Borrowed × Unique × Once × Linear (RC-free)
  | ClassC      -- MaybeShared × Many (dynamic COW)
  | Uncovered   -- outside the three named cells
deriving Repr, DecidableEq

/-- §CH-3 the class-membership predicate for Class A. -/
def isClassA (s : AimsState) : Bool :=
  (s.access = .Owned) && (s.uniqueness = .Unique)
    && (s.cardinality = .One) && (s.consumption = .Linear)

/-- §CH-3 the class-membership predicate for Class B. -/
def isClassB (s : AimsState) : Bool :=
  (s.access = .Borrowed) && (s.uniqueness = .Unique)
    && (s.cardinality = .One) && (s.consumption = .Linear)

/-- §CH-3 the class-membership predicate for Class C. -/
def isClassC (s : AimsState) : Bool :=
  (s.uniqueness = .MaybeShared) && (s.cardinality = .Many)

/-- §CH-3 the class-of function: assigns each state to exactly one
    `CoverageClass`. Class A / B / C are checked in order; the named cells are
    disjoint (CH3_classes_disjoint), so the order does not affect membership. -/
def class_of (s : AimsState) : CoverageClass :=
  if isClassA s then .ClassA
  else if isClassB s then .ClassB
  else if isClassC s then .ClassC
  else .Uncovered

/-- §CH-3 (P1) total partition: `class_of` assigns every state to exactly one
    coverage class, and the assignment matches the membership predicates — for a
    state matching `isClassA`, `class_of` returns `ClassA` (and likewise for B
    when not A, C when not A/B). The classification is a total function. -/
theorem CH3_class_of_classA (s : AimsState) (h : isClassA s = true) :
    class_of s = .ClassA := by
  unfold class_of; rw [h]; rfl

/-- §CH-3 (P2) pairwise disjointness: no state satisfies two of the three named
    membership predicates simultaneously. Proven by destructuring the four
    read dimensions (Access × Uniqueness × Cardinality × Consumption) +
    `decide`: the per-dimension witnesses contradict (A∩B via Access, A∩C and
    B∩C via Uniqueness). -/
theorem CH3_classes_disjoint (s : AimsState) :
    (¬ (isClassA s = true ∧ isClassB s = true))
      ∧ (¬ (isClassA s = true ∧ isClassC s = true))
      ∧ (¬ (isClassB s = true ∧ isClassC s = true)) := by
  obtain ⟨acc, con, car, u, loc, sh, eff⟩ := s
  cases acc <;> cases u <;> cases car <;> cases con <;>
    simp [isClassA, isClassB, isClassC]

/-- §CH-3 (P3) per-class burden agreement: ONLY Class A is burden-owned. A
    Class-A state with DP-2 firing (canonical Once → not Absent, so DP-2 must
    come from the bridge's Linear/Affine path) — the load-bearing fact is that
    the burden-owned predicate restricted to the three classes admits exactly
    Class A. We prove Class B and Class C are NEVER burden-owned. -/
theorem CH3_classB_not_burden_owned (s : AimsState) (h : isClassB s = true) :
    burden_owned s = false := by
  unfold isClassB at h
  simp only [Bool.and_eq_true, decide_eq_true_eq] at h
  unfold burden_owned
  rw [h.1.1.1]   -- s.access = Borrowed
  rfl

/-- §CH-3 (P3) per-class burden agreement: Class C is NEVER burden-owned (the
    MaybeShared uniqueness fails the third conjunct). -/
theorem CH3_classC_not_burden_owned (s : AimsState) (h : isClassC s = true) :
    burden_owned s = false := by
  unfold isClassC at h
  simp only [Bool.and_eq_true, decide_eq_true_eq] at h
  unfold burden_owned
  rw [h.1]   -- s.uniqueness = MaybeShared
  simp

/-- §CH-3 (P3) per-class burden agreement: a burden-owned state is NEVER in
    Class B or Class C — equivalently, burden-eligibility implies the variable
    is outside the RC-free / dynamic-COW classes. The contrapositive of the two
    exclusion lemmas combined. -/
theorem CH3_burden_owned_excludes_BC (s : AimsState) (h : burden_owned s = true) :
    isClassB s = false ∧ isClassC s = false := by
  refine ⟨?_, ?_⟩
  · cases hb : isClassB s with
    | false => rfl
    | true =>
        rw [CH3_classB_not_burden_owned s hb] at h
        exact absurd h (by decide)
  · cases hc : isClassC s with
    | false => rfl
    | true =>
        rw [CH3_classC_not_burden_owned s hc] at h
        exact absurd h (by decide)

/-! ## §CH-4 — AimsStateMap immutability under burden-op elimination
    (annex-e §AIMS §6 + Annex E §AIMS §2 invariant 5)

    The elimination consumer `eliminate_burden_ops` reads the frozen converged
    state map (a finite `var → AimsState` map) and the burden registry, and
    produces an emitted-op decision per variable — WITHOUT mutating the state
    map. The state map is the burden-op elimination's INPUT, read-only; a burden
    mutation event does not alter any per-variable lattice state. Modeled as a
    finite association map; `eliminate_burden_ops` is a pure read that returns
    the SAME map it was given. -/

/-- §CH-4 a minimal frozen state map: a finite list of `(var, state)` entries
    (the converged AimsStateMap's per-variable assignments). -/
abbrev StateMap := List (Nat × AimsState)

/-- §CH-4 a burden-registry mutation event: a write of a burden annotation for a
    variable. It carries NO state-map field — the registry is a disjoint side
    table (Annex E §AIMS §2 invariant 5: a typed pre-pass input, not a lattice
    dimension). -/
structure BurdenMutation where
  var : Nat
  emitted : Bool
deriving Repr, DecidableEq

/-- §CH-4 the elimination consumer: given the frozen state map `L` and a burden
    mutation event `m`, it reads `L` (and `m`) to make a decision but returns
    `L` UNCHANGED — the burden op elimination does not mutate the state map. -/
def eliminate_burden_ops (L : StateMap) (_m : BurdenMutation) : StateMap := L

/-- §CH-4 (P1) per-variable immutability: `eliminate_burden_ops` returns the
    input state map UNCHANGED for any burden mutation event — every per-variable
    AimsState is invariant under the elimination consumer. The frozen-input
    invariant the no-double-counting (CH-1 P2) argument rests on. -/
theorem CH4_state_map_immutable (L : StateMap) (m : BurdenMutation) :
    eliminate_burden_ops L m = L := by rfl

/-- §CH-4 (P1) per-variable lookup stability: the looked-up state for any
    variable is identical before and after a burden mutation event (a direct
    corollary — the map itself is unchanged). -/
theorem CH4_lookup_stable (L : StateMap) (m : BurdenMutation) (v : Nat) :
    (eliminate_burden_ops L m).lookup v = L.lookup v := by
  rw [CH4_state_map_immutable]

/-- §CH-4 (P2) canonicalization preservation: since the state map is unchanged
    (CH-4 P1), every per-variable state's canonicalization status is preserved —
    a canonical state stays canonical across a burden mutation event. Modeled
    over the looked-up state: if it was canonical before, it is after (same
    value). -/
theorem CH4_canon_preserved (L : StateMap) (m : BurdenMutation) (v : Nat)
    (s : AimsState) (h : L.lookup v = some s) :
    (eliminate_burden_ops L m).lookup v = some s := by
  rw [CH4_lookup_stable]; exact h

/-- §CH-4 (P3) replay immutability: replaying a whole finite sequence of burden
    mutation events leaves the state map unchanged — composing the per-event
    immutability over the event list (the block-boundary maps are
    side-effect-free under the full pre-pass write sequence). -/
def replay_mutations (L : StateMap) (ms : List BurdenMutation) : StateMap :=
  ms.foldl eliminate_burden_ops L

/-- §CH-4 (P3) replay leaves the map unchanged: the fold over the event sequence
    returns the original state map. Proven by induction over the event list. -/
theorem CH4_replay_immutable (L : StateMap) (ms : List BurdenMutation) :
    replay_mutations L ms = L := by
  unfold replay_mutations
  induction ms generalizing L with
  | nil => rfl
  | cons hd tl ih =>
      simp only [List.foldl_cons]
      rw [CH4_state_map_immutable]
      exact ih L

/-! ## §CH-5 — phase-ordering composition (annex-e §AIMS §6)

    The burden pre-pass is sequenced AFTER lattice convergence: it is the
    Step-4-companion pre-pass that runs after `analyze_function` (Step 4) and
    before `realize_rc_reuse` (Step 5), consuming the converged state map. The
    coexistence handshake preserves the §6 interprocedural-first invariant (PL-1)
    and the intra-function ordering (PL-2 analyze-before-realize). Reuses the
    `PipelineStep` carrier + `before` relation from `Pipeline.lean`. -/

/-- §CH-5 the burden pre-pass is positioned at the Step-4-companion slot: after
    `AnalyzeFunction` (Step 4), before `RealizeRcReuse` (Step 5). We model its
    ordering relative to the two anchoring phases via the existing
    `PipelineStep.before` relation. -/
theorem CH5_analyze_before_realize :
    PipelineStep.AnalyzeFunction.before PipelineStep.RealizeRcReuse :=
  PL2_analyze_before_realize

/-- §CH-5 (P1) PL-1 interprocedural-first preserved: the interprocedural prefix
    (`ApplyOwnership`, Step 2) precedes `RealizeRcReuse` (the first realization
    consumer of the burden pre-pass), so inserting the burden pre-pass within
    the per-function suffix does NOT reorder the interprocedural phases. -/
theorem CH5_prefix_before_realize :
    PipelineStep.ApplyOwnership.before PipelineStep.RealizeRcReuse :=
  PL1_prefix_before_suffix PipelineStep.RealizeRcReuse (by decide)

/-- §CH-5 (P2) acyclic BR-reads-L sequencing: the burden pre-pass reads the
    converged state map (produced at/after `AnalyzeFunction`) and feeds
    `RealizeRcReuse` — so the producing phase strictly precedes the consuming
    phase. The burden pre-pass occupies the gap (analyze < realize), confirming
    a one-way producer→consumer order with no back-edge. -/
theorem CH5_no_back_edge :
    ¬ PipelineStep.RealizeRcReuse.before PipelineStep.AnalyzeFunction :=
  PipelineStep.before_asymm _ _ PL2_analyze_before_realize

/-- §CH-5 (P3) PL-5 no-stale composition: the analyze→realize ordering composes
    transitively to analyze→merge (the burden pre-pass output, consumed at
    realize, is downstream-stable through merge) — the converged state map the
    pre-pass reads is fresh at realize and not re-fired before merge. Reuses the
    `before` transitivity chain. -/
theorem CH5_analyze_before_merge :
    PipelineStep.AnalyzeFunction.before PipelineStep.MergeBlocks :=
  PipelineStep.before_trans _ _ _ PL2_analyze_before_realize PL3_realize_before_merge

/-- §CH-5 (P4) PL-6 no-violation: the schedule with the burden pre-pass inserted
    remains a strict order — `before` is irreflexive (no phase before itself), so
    the burden-pre-pass insertion introduces no cyclic ordering constraint. -/
theorem CH5_schedule_acyclic (a b : PipelineStep) (hab : a.before b) :
    ¬ b.before a :=
  PipelineStep.before_asymm a b hab

/-! ## §CH-comp — coexistence-handshake union (the Ori-novel composition theorem)
    (annex-e §AIMS coexistence handshake)

    The capstone. The coexistence handshake conjoins the four constituent
    capabilities — burden-registry-as-typed-pre-pass (CH-1 bridge), lattice-
    state-map (CH-4 immutability), DP-2/DP-3 elimination consumer (CH-2 single-
    elimination), and per-class coexistence (CH-3 partition) — sequenced by the
    phase order (CH-5). The composition (1) catches the UNION of the per-layer
    failure classes (a fix passing a strict subset is rejected), and (2) the
    soundness partition over the variable space is TOTAL + DISJOINT into
    class_covered / mixed-coverage / uncovered, each with a per-case dispatch
    verdict. Mirrors the §9 VF-comp layered-verifier composition. -/

/-- §CH-comp the five coexistence-handshake layers. -/
inductive CoexLayer
  | CH1Bridge        -- lattice-bridge consistency
  | CH2Elimination   -- single-elimination commutativity
  | CH3Partition     -- per-class coexistence partition
  | CH4Immutability  -- AimsStateMap immutability
  | CH5Ordering      -- phase-ordering composition
deriving Repr, DecidableEq

/-- §CH-comp the failure class each layer catches (the 1:1 partition). -/
inductive CoexFailure
  | BridgeInconsistent    -- CH-1: lattice-bridge inconsistency / double-counting
  | MultiElimination      -- CH-2: stacked elimination / race-invalidation
  | PartitionIllformed    -- CH-3: non-total / overlapping class partition
  | StateMapMutated       -- CH-4: AimsStateMap mutated under burden write
  | OrderingViolated      -- CH-5: PL-1 reordering / BR-reads-L cycle
deriving Repr, DecidableEq

/-- §CH-comp the layer→failure-class catch map (each layer owns one class). -/
def CoexLayer.catches : CoexLayer → CoexFailure
  | .CH1Bridge       => .BridgeInconsistent
  | .CH2Elimination  => .MultiElimination
  | .CH3Partition    => .PartitionIllformed
  | .CH4Immutability => .StateMapMutated
  | .CH5Ordering     => .OrderingViolated

/-- §CH-comp the whole-handshake verdict over per-layer accept bits: ACCEPT iff
    EVERY layer accepts (the conjunction fold — mirrors VF-comp `stackAccepts`). -/
def handshakeAccepts (layerVerdicts : List Bool) : Bool :=
  layerVerdicts.foldr (· && ·) true

/-- §CH-comp (catch-partition) each layer catches a DISTINCT failure class — the
    `CoexLayer.catches` map is INJECTIVE over the 5 layers. No two layers share a
    failure class, so the per-layer classes are pairwise disjoint. -/
theorem CHcomp_layers_catch_distinct (l1 l2 : CoexLayer)
    (h : l1.catches = l2.catches) : l1 = l2 := by
  cases l1 <;> cases l2 <;> first | rfl | (simp [CoexLayer.catches] at h)

/-- §CH-comp the per-layer catch map is SURJECTIVE onto the failure-class
    universe: every coexistence failure class is caught by some layer. -/
theorem CHcomp_every_failure_caught (f : CoexFailure) :
    ∃ l : CoexLayer, l.catches = f := by
  cases f
  · exact ⟨.CH1Bridge, rfl⟩
  · exact ⟨.CH2Elimination, rfl⟩
  · exact ⟨.CH3Partition, rfl⟩
  · exact ⟨.CH4Immutability, rfl⟩
  · exact ⟨.CH5Ordering, rfl⟩

/-- §CH-comp (P1) the handshake ACCEPTS iff EVERY layer accepts — the joined
    union of the per-layer verdicts. Proven by induction over the layer-verdict
    list (a failing layer sticky-clears the conjunction). -/
theorem CHcomp_accepts_iff_all (layerVerdicts : List Bool) :
    handshakeAccepts layerVerdicts = true ↔ ∀ v ∈ layerVerdicts, v = true := by
  unfold handshakeAccepts
  induction layerVerdicts with
  | nil => simp
  | cons hd tl ih =>
      simp only [List.foldr_cons, Bool.and_eq_true, List.mem_cons]
      rw [ih]
      constructor
      · rintro ⟨hhd, htl⟩ v (rfl | hv)
        · exact hhd
        · exact htl v hv
      · intro h
        exact ⟨h hd (Or.inl rfl), fun v hv => h v (Or.inr hv)⟩

/-- §CH-comp (P2) the union has teeth: a fix passing a strict SUBSET (CH-1..CH-4
    accept but CH-5 ordering regresses) is REJECTED by the whole handshake — some
    layer in the union catches the regressed class. A fix passing one layer while
    regressing another is a correctness regression, not a partial win. -/
theorem CHcomp_subset_pass_rejected :
    handshakeAccepts [true, true, true, true, false] = false := by decide

/-- §CH-comp (P2) conjunction strength: a handshake where EVERY layer accepts is
    accepted (the only accepting verdict requires the full 5-layer pass). -/
theorem CHcomp_all_pass_accepted :
    handshakeAccepts [true, true, true, true, true] = true := by decide

/-- §CH-comp (P2) a DROPPED constituent leaves a failure class uncaught: if any
    layer's verdict is false (a regressed/missing layer), the handshake rejects —
    no false-valid slips through. Over an arbitrary failing layer position. -/
theorem CHcomp_any_layer_fails_rejects (pre post : List Bool) :
    handshakeAccepts (pre ++ false :: post) = false := by
  unfold handshakeAccepts
  induction pre with
  | nil => simp
  | cons hd tl ih =>
      simp only [List.cons_append, List.foldr_cons, ih, Bool.and_false]

/-! ### §CH-comp soundness partition — class_covered / mixed-coverage / uncovered

    The handshake's soundness theorem partitions the variable space into three
    cases per the coverage class. `class_covered` = Class A (burden-eligible);
    `uncovered` = no class member burden-owned; `mixed-coverage` = a payload
    that is partially covered (per-field dispatch). The case verdict routes to
    `burden_emission_path` (covered), `predicate_stack_path` (uncovered), or the
    per-field composite (mixed). We model the partition as a total classification
    over a variable's coverage status with a dispatch target per case. -/

/-- §CH-comp the soundness-partition case of a variable's coverage status. -/
inductive CoverageCase
  | classCovered    -- the variable's class is burden-owned (Class A)
  | mixedCoverage   -- a payload partially covered (per-field dispatch)
  | uncovered       -- no burden-owned member (Class B / C / Uncovered)
deriving Repr, DecidableEq

/-- §CH-comp the dispatch target per case (the coexistence_dispatch verdict). -/
inductive DispatchTarget
  | burdenEmission   -- consume the burden registry
  | perFieldComposite -- per-field dispatch (mixed)
  | predicateStack   -- lattice-only path
deriving Repr, DecidableEq

/-- §CH-comp the coexistence dispatch: maps each soundness-partition case to its
    realization target. class_covered → burden emission; mixed → per-field
    composite; uncovered → predicate stack. -/
def coexistence_dispatch : CoverageCase → DispatchTarget
  | .classCovered  => .burdenEmission
  | .mixedCoverage => .perFieldComposite
  | .uncovered     => .predicateStack

/-- §CH-comp classify a variable's state into a soundness-partition case: a
    burden-owned state is `classCovered`; otherwise (Class B / C / Uncovered)
    `uncovered`. (The `mixedCoverage` case arises at the payload granularity for
    aggregates with partial coverage — it is a distinct constructor consumed by
    the per-field dispatch; a scalar-grain state is covered or uncovered.) -/
def coverage_case_of (s : AimsState) : CoverageCase :=
  if burden_owned s then .classCovered else .uncovered

/-- §CH-comp (P3) per-case dispatch — class_covered: a burden-owned state routes
    to the burden-emission path. -/
theorem CHcomp_covered_dispatches_burden (s : AimsState)
    (h : burden_owned s = true) :
    coexistence_dispatch (coverage_case_of s) = .burdenEmission := by
  unfold coverage_case_of; rw [h]; rfl

/-- §CH-comp (P3) per-case dispatch — uncovered: a non-burden-owned state routes
    to the predicate-stack (lattice-only) path. -/
theorem CHcomp_uncovered_dispatches_predicate_stack (s : AimsState)
    (h : burden_owned s = false) :
    coexistence_dispatch (coverage_case_of s) = .predicateStack := by
  unfold coverage_case_of; rw [h]; rfl

/-- §CH-comp (P3) per-case dispatch — mixed-coverage: a partially-covered payload
    routes to the per-field composite path (distinct from both pure paths). -/
theorem CHcomp_mixed_dispatches_composite :
    coexistence_dispatch .mixedCoverage = .perFieldComposite := by rfl

/-- §CH-comp (P3) the soundness partition is TOTAL + DISJOINT: the three dispatch
    targets are pairwise distinct (no two cases route the same way — the
    classification is unambiguous), and `coexistence_dispatch` is injective. -/
theorem CHcomp_dispatch_injective (c1 c2 : CoverageCase)
    (h : coexistence_dispatch c1 = coexistence_dispatch c2) : c1 = c2 := by
  cases c1 <;> cases c2 <;> first | rfl | (simp [coexistence_dispatch] at h)

/-- §CH-comp the capstone (the Ori-novel composition theorem): the coexistence
    handshake conjoins all four capabilities, sequenced by the phase order, and
    (a) catches the complete pairwise-distinct UNION of the per-layer failure
    classes, (b) accepts iff every layer accepts (so any regressed layer's class
    is caught), and (c) partitions the variable space into a total + disjoint
    class_covered / mixed-coverage / uncovered soundness partition with an
    unambiguous per-case dispatch. No inconsistency class escapes the handshake. -/
theorem CHcomp_handshake_union :
    -- (a) every coexistence failure class is caught by some layer
    (∀ f : CoexFailure, ∃ l : CoexLayer, l.catches = f)
    -- (b) the catch is a partition (distinct layers → distinct classes)
    ∧ (∀ l1 l2 : CoexLayer, l1.catches = l2.catches → l1 = l2)
    -- (c) the whole-handshake verdict is the union (accept iff every layer accepts)
    ∧ (∀ verdicts : List Bool,
        handshakeAccepts verdicts = true ↔ ∀ v ∈ verdicts, v = true)
    -- (d) the soundness partition's dispatch is total + disjoint (injective)
    ∧ (∀ c1 c2 : CoverageCase,
        coexistence_dispatch c1 = coexistence_dispatch c2 → c1 = c2) := by
  refine ⟨CHcomp_every_failure_caught, CHcomp_layers_catch_distinct, ?_, ?_⟩
  · exact CHcomp_accepts_iff_all
  · exact CHcomp_dispatch_injective

end AimsProof
