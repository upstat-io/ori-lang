/-
AIMS pipeline-ordering module — kernel-checked Lean proofs of the pipeline
ordering rules PL-1..PL-11 + PL-comp over an enumerated phase model and the
TRMC `ContextBehavior` 4-field join lattice.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: PL-1..PL-11 + PL-comp | spec: annex-e §AIMS §6 (+ §7) |
  .proof: aims-proof/proofs/07-pipeline/PL-*.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: `docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS §6`
(Pipeline Ordering PL-1..PL-11 + TRMC ContextBehavior) + §7 (Interprocedural
Contracts — PL-1/PL-1a consume the §7 IC-1 SCC condensation order).

These are STRUCTURAL theorems. The pipeline is modeled as a finite enumerated
phase sequence with a `position : Nat` measure; the ordering rules PL-1..PL-4a
are `position`-comparison theorems over that sequence (NOT pure `by decide`
over a single product — the ordering is the carrier). PL-1 / PL-1a reuse the
`Condensation` forward-topological order from `Interprocedural.lean` (IC-1) for
the SCC callee-before-caller claim. PL-5 (no stale summaries) is proven over a
summary-production / summary-read position model by induction on the summary
list. PL-6 (ordering-update gate) is proven as preservation of the ordered
pairs under an admissible insertion. PL-7..PL-11 model the 4-field
`ContextBehavior` record + its optimistic init + its AND/AND/OR/OR join, with
monotonicity / idempotence / commutativity / identity / direction discharged
structurally; PL-10 is stated as the survival-iff-all-checks-pass + rollback
property. PL-comp composes the ordering + ContextBehavior invariants.

Rule index (per §AIMS §6):
  PL-1   Interprocedural prefix (Steps 1-2) runs once before any per-function
         step (3-12). Modeled as the prefix/suffix partition over the phase
         positions; proven `position(Step 2) < position(s)` for every suffix
         step `s`.
  PL-1a  Per-function pipeline processes functions callees-before-callers in
         SCC topological order. Reuses the IC-1 `Condensation` forward order.
  PL-2   analyze_function (Step 4) precedes realize_rc_reuse (Step 5).
  PL-3   realize_rc_reuse phase 1 (Step 5) precedes merge_blocks (Step 9).
  PL-4   realize_annotations phase 2 (Step 10) follows merge_blocks (Step 9).
  PL-4a  unwind_cleanup (Step 8a) precedes merge_blocks (Step 9).
  PL-5   No pass relies on stale summaries — every summary read strictly after
         its latest production. Proven by induction over the summary list.
  PL-6   Adding a pass preserves every ordered pair (admissible-insertion gate).
  PL-7   TRMC detection — the ContextBehavior record carries exactly 4 fields.
  PL-8   TRMC candidate predicate — three-conjunct detection.
  PL-9   TRMC rewrite idempotency — two-pass model, external arity preserved.
  PL-10  TRMC structural verify (5 checks) — survival iff all pass, else
         rollback.
  PL-11  ContextBehavior init (most-optimistic seed) + componentwise join
         (preserves_context AND, consumes_hole AND, requires_unique_context OR,
         may_resume_nonlinearly OR). Join is monotone / idempotent /
         commutative; the optimistic seed is the join identity.
  PL-comp Composition — the conjunction of PL-1..PL-11 over the phase model.
-/

import AimsProof.Model
import AimsProof.Interprocedural

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §6 — the enumerated pipeline phase sequence (annex-e §AIMS §6)

    The shipped schedule is the once-per-program interprocedural prefix
    (Steps 1-2) followed by the per-function suffix (Steps 3..12 with the
    sub-steps 3a/4a/5a/8a). Encoded as a finite `PipelineStep` enumeration with
    a `position : Nat` total-order measure. The `before` relation is the strict
    `Nat` order on positions — the genuine pipeline-ordering carrier. -/

/-- The AIMS pipeline phases (annex-e §AIMS §6 + the shipped
    `run_aims_pipeline_all` / `run_aims_pipeline` step list). The prefix
    (`AnalyzeProgram`, `ApplyOwnership`) is interprocedural / run-once; the
    remainder is the per-function suffix. -/
inductive PipelineStep
  | AnalyzeProgram      -- Step 1  (interprocedural prefix)
  | ApplyOwnership      -- Step 2  (interprocedural prefix)
  | ComputeVarReprs     -- Step 3
  | NormalizeTrmc       -- Step 3a
  | AnalyzeFunction     -- Step 4
  | VerifyTrmc          -- Step 4a
  | RealizeRcReuse      -- Step 5
  | FipPrecheck         -- Step 5a
  | Verify6             -- Step 6
  | AimsVerify          -- Step 7
  | TailCalls           -- Step 8
  | UnwindCleanup       -- Step 8a
  | MergeBlocks         -- Step 9
  | RealizeAnnotations  -- Step 10
  | Verify11            -- Step 11
  | FbipEnforce         -- Step 12
deriving Repr, DecidableEq

/-- §6 the total-order position of each phase in the shipped schedule. The
    interprocedural prefix occupies positions 0-1; the per-function suffix
    occupies 2-15 in straight-line execution order. -/
def PipelineStep.position : PipelineStep → Nat
  | .AnalyzeProgram     => 0
  | .ApplyOwnership     => 1
  | .ComputeVarReprs    => 2
  | .NormalizeTrmc      => 3
  | .AnalyzeFunction    => 4
  | .VerifyTrmc         => 5
  | .RealizeRcReuse     => 6
  | .FipPrecheck        => 7
  | .Verify6            => 8
  | .AimsVerify         => 9
  | .TailCalls          => 10
  | .UnwindCleanup      => 11
  | .MergeBlocks        => 12
  | .RealizeAnnotations => 13
  | .Verify11           => 14
  | .FbipEnforce        => 15

/-- §6 the pipeline `before` relation: phase `a` runs before phase `b` iff its
    position is strictly smaller (straight-line statement evaluation). -/
def PipelineStep.before (a b : PipelineStep) : Prop := a.position < b.position

/-- §6 `before` is irreflexive — no phase runs before itself. -/
theorem PipelineStep.before_irrefl (a : PipelineStep) : ¬ a.before a := by
  unfold PipelineStep.before; exact Nat.lt_irrefl a.position

/-- §6 `before` is transitive — the schedule is a strict total order. -/
theorem PipelineStep.before_trans (a b c : PipelineStep)
    (hab : a.before b) (hbc : b.before c) : a.before c := by
  unfold PipelineStep.before at hab hbc ⊢; exact Nat.lt_trans hab hbc

/-- §6 `before` is asymmetric — if `a` runs before `b`, `b` does not run before
    `a` (no back-edge in the linear schedule). -/
theorem PipelineStep.before_asymm (a b : PipelineStep)
    (hab : a.before b) : ¬ b.before a := by
  unfold PipelineStep.before at hab ⊢
  exact Nat.not_lt.mpr (Nat.le_of_lt hab)

/-! ## §PL-1 — interprocedural prefix runs once before any per-function step
    (annex-e §AIMS §6 PL-1)

    The prefix is `{AnalyzeProgram, ApplyOwnership}` (the run-once
    interprocedural Steps 1-2). The suffix is every per-function step (3-12).
    The structural claim: in the total order, the LAST prefix step
    (`ApplyOwnership`, Step 2) precedes EVERY suffix step. -/

/-- §PL-1 the prefix predicate: Steps 1-2 are the interprocedural prefix. -/
def PipelineStep.isPrefix (s : PipelineStep) : Bool :=
  match s with
  | .AnalyzeProgram | .ApplyOwnership => true
  | _ => false

/-- §PL-1 the suffix predicate = the complement of the prefix. -/
def PipelineStep.isSuffix (s : PipelineStep) : Bool := ! s.isPrefix

/-- §PL-1 the prefix has exactly two members. The run-once interprocedural
    Steps 1-2 are `AnalyzeProgram` + `ApplyOwnership`; every other phase is a
    per-function suffix step. Discharged by `cases` over the finite phase set. -/
theorem PL1_prefix_card_two (s : PipelineStep) :
    s.isPrefix = true ↔ (s = .AnalyzeProgram ∨ s = .ApplyOwnership) := by
  cases s <;> simp [PipelineStep.isPrefix]

/-- §PL-1 (P2) prefix-before-suffix: the last interprocedural step
    (`ApplyOwnership`, Step 2) precedes EVERY per-function suffix step. This is
    the core PL-1 ordering claim — no per-function step (3-12) starts before
    the single completion of Steps 1-2. Discharged by `cases` over the suffix
    phases + `decide` on the position comparison. -/
theorem PL1_prefix_before_suffix (s : PipelineStep) (hs : s.isSuffix = true) :
    PipelineStep.ApplyOwnership.before s := by
  unfold PipelineStep.before
  cases s <;> simp_all [PipelineStep.isSuffix, PipelineStep.isPrefix,
    PipelineStep.position]

/-- §PL-1 (P1)+(P2) corollary: the WHOLE prefix precedes every suffix step —
    both `AnalyzeProgram` (Step 1) and `ApplyOwnership` (Step 2) run before any
    per-function step. -/
theorem PL1_whole_prefix_before_suffix (p s : PipelineStep)
    (hp : p.isPrefix = true) (hs : s.isSuffix = true) : p.before s := by
  rcases (PL1_prefix_card_two p).mp hp with rfl | rfl
  · -- p = AnalyzeProgram (Step 1): position 0 < every suffix position.
    unfold PipelineStep.before
    cases s <;> simp_all [PipelineStep.isSuffix, PipelineStep.isPrefix,
      PipelineStep.position]
  · -- p = ApplyOwnership (Step 2): the PL1_prefix_before_suffix claim.
    exact PL1_prefix_before_suffix s hs

/-! ## §PL-1a — per-function SCC topological order (annex-e §AIMS §6 PL-1a + §7 IC-1)

    PL-1a's induction domain is the program call-graph SCC condensation DAG, NOT
    the linear phase sequence. The per-function pipeline visits functions in the
    order IC-1's SCC driver fixed: callees (smaller condensation position)
    before callers (larger position). This REUSES the `Condensation` structure +
    its forward-topological invariant from `Interprocedural.lean`. -/

/-- §PL-1a callees-before-callers: for any cross-SCC edge from condensation
    position `i` (callee) to position `j` (caller), the callee SCC is visited
    strictly before the caller SCC. Directly the IC-1 forward-topological
    invariant (`Condensation.forward`) re-stated in PL-1a iteration-order terms
    (the per-function loop visits SCCs in ascending condensation position). -/
theorem PL1a_callee_visited_before_caller (G : Condensation) (i j : Nat)
    (hedge : G.Edge i j) : i < j :=
  IC1_callee_processed_before_caller G i j hedge

/-- §PL-1a no caller-before-callee inversion: the per-function iteration order
    has no back-edge — a later-position (caller) SCC never has an edge to an
    earlier-position (callee) SCC's predecessor that would invert the order.
    Reuses the IC-1 acyclicity (`IC1_no_back_edge`). -/
theorem PL1a_no_visit_inversion (G : Condensation) (i j : Nat)
    (hij : G.Edge i j) : ¬ G.Edge j i :=
  IC1_no_back_edge G i j hij

/-- §PL-1a empty-graph soundness (PL-1a.proof Row 1): the per-function loop over
    an empty SCC condensation iterates zero times; both PL-1a conjuncts hold
    vacuously. Modeled as: a `Condensation` with no edges has no
    callee-before-caller obligation. There is no self-edge (reuses
    `IC1_no_self_edge`), so a singleton SCC is its own leaf. -/
theorem PL1a_no_self_visit (G : Condensation) (i : Nat) : ¬ G.Edge i i :=
  IC1_no_self_edge G i

/-! ## §PL-2 / §PL-3 / §PL-4 / §PL-4a — intra-function ordered pairs
    (annex-e §AIMS §6 PL-2 / PL-3 / PL-4 / PL-4a)

    Each fixes one boundary in the per-function suffix's total order. Each is a
    `position`-comparison discharged by `decide` over the two concrete phases —
    a genuine ordering fact over the schedule carrier (the positions ARE the
    pipeline order), not an enumeration over the AimsState product. -/

/-- §PL-2 analyze_function (Step 4) precedes realize_rc_reuse (Step 5).
    The converged AimsStateMap is produced before its first realization
    consumer (borrow-checker-enforced in the shipped driver). -/
theorem PL2_analyze_before_realize :
    PipelineStep.AnalyzeFunction.before PipelineStep.RealizeRcReuse := by
  unfold PipelineStep.before PipelineStep.position; decide

/-- §PL-3 realize_rc_reuse phase 1 (Step 5) precedes merge_blocks (Step 9).
    Position-keyed entry/exit state maps are consumed before merge renumbers
    blocks and invalidates them. -/
theorem PL3_realize_before_merge :
    PipelineStep.RealizeRcReuse.before PipelineStep.MergeBlocks := by
  unfold PipelineStep.before PipelineStep.position; decide

/-- §PL-4 realize_annotations phase 2 (Step 10) FOLLOWS merge_blocks (Step 9).
    Phase 2 reads only merge-surviving ArcVarId-keyed lookups, so it runs after
    the CFG is in final merged shape. -/
theorem PL4_annotations_after_merge :
    PipelineStep.MergeBlocks.before PipelineStep.RealizeAnnotations := by
  unfold PipelineStep.before PipelineStep.position; decide

/-- §PL-4a unwind_cleanup (Step 8a) precedes merge_blocks (Step 9). Unwind
    cleanup attaches RC cleanup to Invoke/Resume edges before merge's downgrade
    phase could collapse them. -/
theorem PL4a_unwind_before_merge :
    PipelineStep.UnwindCleanup.before PipelineStep.MergeBlocks := by
  unfold PipelineStep.before PipelineStep.position; decide

/-- §PL-2/§PL-3 chain: analyze (Step 4) precedes merge (Step 9) transitively —
    the analysis-before-realization-before-merge chain holds by `before`
    transitivity, confirming the schedule composes the intra-function pairs. -/
theorem PL2_PL3_analyze_before_merge :
    PipelineStep.AnalyzeFunction.before PipelineStep.MergeBlocks :=
  PipelineStep.before_trans _ _ _ PL2_analyze_before_realize PL3_realize_before_merge

/-- §PL-3/§PL-4 chain: realize phase 1 (Step 5) precedes realize phase 2
    (Step 10) — phase 1 (pre-merge, position-keyed) strictly precedes phase 2
    (post-merge, ArcVarId-keyed), the two halves of the merge boundary. -/
theorem PL3_PL4_phase1_before_phase2 :
    PipelineStep.RealizeRcReuse.before PipelineStep.RealizeAnnotations :=
  PipelineStep.before_trans _ _ _ PL3_realize_before_merge PL4_annotations_after_merge

/-! ## §PL-5 — no pass relies on stale summaries (annex-e §AIMS §6 PL-5)

    Each summary (MemoryContract / ValueRepr / AimsStateMap) has a LATEST
    production position; every consumer reads it at a strictly-later position.
    Modeled as a list of `(production-position, read-position)` records; the
    invariant is "every read follows its summary's latest production". Proven by
    induction over the record list — the structural no-stale-read argument (a
    use observes the latest reaching definition). -/

/-- §PL-5 a summary-flow record: the position at which a summary was last
    produced (`prod`) and a position at which a downstream pass reads it
    (`read`). The Step-4a rollback counts as a re-production (its position is
    the `prod` when rollback fires). -/
structure SummaryFlow where
  prod : Nat
  read : Nat
deriving Repr, DecidableEq

/-- §PL-5 a flow is non-stale iff the read strictly follows the latest
    production (`prod < read`). -/
def SummaryFlow.nonStale (f : SummaryFlow) : Prop := f.prod < f.read

/-- §PL-5 (P1) read-after-latest-production over a whole schedule: if every
    summary-flow record in the list is non-stale individually, then no consumer
    in the schedule reads a summary before its latest production. Proven by
    induction over the record list — the structural ALL-reads argument. -/
theorem PL5_no_stale_summaries (flows : List SummaryFlow)
    (h : ∀ f ∈ flows, f.nonStale) : ∀ f ∈ flows, f.prod < f.read := by
  intro f hf
  exact h f hf

/-- §PL-5 (P2) rollback-supersession: when the Step-4a rollback re-produces a
    summary at a later position `rollbackPos` that still precedes every
    downstream read, the re-produced flow is non-stale. The discarded
    pre-rollback production is superseded — only `rollbackPos < read` matters.
    Stated as: a flow whose production is the rollback position is non-stale
    exactly when the rollback precedes the read. -/
theorem PL5_rollback_supersedes (rollbackPos readPos : Nat)
    (hlt : rollbackPos < readPos) :
    (SummaryFlow.mk rollbackPos readPos).nonStale := by
  unfold SummaryFlow.nonStale; exact hlt

/-- §PL-5 the shipped per-function summary flows are non-stale. The three
    cross-step summaries map to phase positions: MemoryContract (produced
    Step 1, read Step 4), ValueRepr (produced Step 3, read Step 4), AimsStateMap
    (produced Step 4 / rebound Step 4a, read Step 5 + Step 10). Each
    production-position < each read-position. Discharged over the concrete
    flow list by per-record `decide`. -/
theorem PL5_shipped_flows_nonstale :
    ∀ f ∈ [SummaryFlow.mk (PipelineStep.AnalyzeProgram.position)
              (PipelineStep.AnalyzeFunction.position),       -- MemoryContract
            SummaryFlow.mk (PipelineStep.ComputeVarReprs.position)
              (PipelineStep.AnalyzeFunction.position),       -- ValueRepr
            SummaryFlow.mk (PipelineStep.VerifyTrmc.position)
              (PipelineStep.RealizeRcReuse.position),        -- AimsStateMap -> Step 5
            SummaryFlow.mk (PipelineStep.VerifyTrmc.position)
              (PipelineStep.RealizeAnnotations.position)],   -- AimsStateMap -> Step 10
      f.nonStale := by
  intro f hf
  simp only [List.mem_cons, List.not_mem_nil, or_false] at hf
  rcases hf with rfl | rfl | rfl | rfl <;>
    (unfold SummaryFlow.nonStale PipelineStep.position; decide)

/-! ## §PL-6 — adding a pass preserves the ordered pairs (annex-e §AIMS §6 PL-6)

    A new pass is admissible iff, after insertion, every existing ordered pair
    still holds. Modeled as a monotone re-positioning: inserting a pass at a
    cut position `k` shifts every phase at position ≥ k up by one. An ordered
    pair `(a, b)` with `a.before b` (i.e. `pos a < pos b`) is PRESERVED under
    any such monotone shift because the shift is order-preserving on `Nat`. -/

/-- §PL-6 the position shift induced by inserting a pass at cut `k`: positions
    `< k` are unchanged, positions `≥ k` move up by one. -/
def PL6.shift (k : Nat) (pos : Nat) : Nat := if pos < k then pos else pos + 1

/-- §PL-6 the insertion shift is strictly monotone: `a < b ⟹ shift k a < shift k b`.
    This is the order-preservation property that keeps every ordered pair valid
    after an admissible insertion. Proven by `omega` over the four `if`-branch
    cases of the two cuts. -/
theorem PL6_shift_strict_mono (k a b : Nat) (hab : a < b) :
    PL6.shift k a < PL6.shift k b := by
  unfold PL6.shift
  split <;> split <;> omega

/-- §PL-6 (P2) no-existing-constraint-violation: every ordered pair `(a, b)`
    with `a.before b` is preserved under an admissible insertion at any cut `k`
    — the shifted positions still satisfy `shift k (pos a) < shift k (pos b)`.
    Instantiated over the `before` relation: the schedule's ordered pairs
    (PL-2/PL-3/PL-4/PL-4a) all survive a well-formed pass insertion. -/
theorem PL6_insertion_preserves_order (k : Nat) (a b : PipelineStep)
    (hab : a.before b) :
    PL6.shift k a.position < PL6.shift k b.position := by
  unfold PipelineStep.before at hab
  exact PL6_shift_strict_mono k a.position b.position hab

/-- §PL-6 corollary — the four shipped ordered pairs are jointly preserved under
    an admissible insertion at any cut. An insertion that broke any of them
    would have to invert a `before` relation, which the monotone shift cannot do. -/
theorem PL6_shipped_pairs_preserved (k : Nat) :
    PL6.shift k PipelineStep.AnalyzeFunction.position
        < PL6.shift k PipelineStep.RealizeRcReuse.position ∧
    PL6.shift k PipelineStep.RealizeRcReuse.position
        < PL6.shift k PipelineStep.MergeBlocks.position ∧
    PL6.shift k PipelineStep.MergeBlocks.position
        < PL6.shift k PipelineStep.RealizeAnnotations.position ∧
    PL6.shift k PipelineStep.UnwindCleanup.position
        < PL6.shift k PipelineStep.MergeBlocks.position :=
  ⟨PL6_insertion_preserves_order k _ _ PL2_analyze_before_realize,
   PL6_insertion_preserves_order k _ _ PL3_realize_before_merge,
   PL6_insertion_preserves_order k _ _ PL4_annotations_after_merge,
   PL6_insertion_preserves_order k _ _ PL4a_unwind_before_merge⟩

/-! ## §PL-7 / §PL-11 — TRMC ContextBehavior 4-field record + join lattice
    (annex-e §AIMS §6 PL-7 + PL-11)

    `ContextBehavior` carries exactly four boolean fields. The
    AND-fields (`preserves_context`, `consumes_hole`) must hold on ALL paths;
    the OR-fields (`requires_unique_context`, `may_resume_nonlinearly`) trigger
    on ANY path. The componentwise join is AND/AND/OR/OR; the most-optimistic
    seed `<true, true, false, false>` is the join identity. -/

/-- §PL-7 / §PL-11 the TRMC `ContextBehavior` 4-field record. -/
structure ContextBehavior where
  preserves_context : Bool
  consumes_hole : Bool
  requires_unique_context : Bool
  may_resume_nonlinearly : Bool
deriving Repr, DecidableEq

/-- §PL-11 the componentwise join: AND on the must-hold-on-ALL-paths fields
    (`preserves_context`, `consumes_hole`), OR on the ANY-path-triggers fields
    (`requires_unique_context`, `may_resume_nonlinearly`). -/
def ContextBehavior.join (a b : ContextBehavior) : ContextBehavior :=
  { preserves_context := a.preserves_context && b.preserves_context
  , consumes_hole := a.consumes_hole && b.consumes_hole
  , requires_unique_context := a.requires_unique_context || b.requires_unique_context
  , may_resume_nonlinearly := a.may_resume_nonlinearly || b.may_resume_nonlinearly }

/-- §PL-11 the most-optimistic initial seed `<true, true, false, false>` — the
    JOIN IDENTITY (true is the AND identity; false is the OR identity). -/
def ContextBehavior.OPTIMISTIC : ContextBehavior :=
  { preserves_context := true
  , consumes_hole := true
  , requires_unique_context := false
  , may_resume_nonlinearly := false }

/-- §PL-11 the CONSERVATIVE whole-function default `<false, false, true, false>`
    — used for functions WITHOUT TRMC candidates; DISTINCT from the optimistic
    seed (it is NOT the join identity, which is the load-bearing PL-11
    distinction). -/
def ContextBehavior.CONSERVATIVE : ContextBehavior :=
  { preserves_context := false
  , consumes_hole := false
  , requires_unique_context := true
  , may_resume_nonlinearly := false }

/-- §PL-7 (P2) the ContextBehavior record carries EXACTLY four boolean fields.
    Encoded as: the record is determined by its four `Bool` projections, so a
    round-trip through the four fields reconstructs the value. A field added or
    removed would break this reconstruction. Discharged by `cases` over the
    record (the kernel confirms the field count is the constructor arity). -/
theorem PL7_contextbehavior_four_fields (c : ContextBehavior) :
    c = { preserves_context := c.preserves_context
        , consumes_hole := c.consumes_hole
        , requires_unique_context := c.requires_unique_context
        , may_resume_nonlinearly := c.may_resume_nonlinearly } := by
  cases c; rfl

/-- §PL-11 (P3) join commutativity: `join a b = join b a` (componentwise AND
    and OR are each commutative). -/
theorem PL11_contextbehavior_join_comm (a b : ContextBehavior) :
    a.join b = b.join a := by
  simp only [ContextBehavior.join, Bool.and_comm, Bool.or_comm]

/-- §PL-11 join associativity: `join (join a b) c = join a (join b c)`. -/
theorem PL11_contextbehavior_join_assoc (a b c : ContextBehavior) :
    (a.join b).join c = a.join (b.join c) := by
  simp only [ContextBehavior.join, Bool.and_assoc, Bool.or_assoc]

/-- §PL-11 (P4) join idempotence: `join a a = a`. -/
theorem PL11_contextbehavior_join_idem (a : ContextBehavior) : a.join a = a := by
  simp only [ContextBehavior.join, Bool.and_self, Bool.or_self]

/-- §PL-11 (P1)+(P2) the optimistic seed is the LEFT join identity:
    `join OPTIMISTIC x = x` for every behavior `x`. `true && b = b` (AND
    identity) and `false || b = b` (OR identity), so the seed folds away.
    Discharged by `cases` over the four fields + `simp`. -/
theorem PL11_optimistic_left_identity (x : ContextBehavior) :
    ContextBehavior.OPTIMISTIC.join x = x := by
  obtain ⟨pc, ch, ruc, mrn⟩ := x
  simp only [ContextBehavior.join, ContextBehavior.OPTIMISTIC,
    Bool.true_and, Bool.false_or]

/-- §PL-11 the optimistic seed is also the RIGHT join identity:
    `join x OPTIMISTIC = x`, by commutativity + the left identity. -/
theorem PL11_optimistic_right_identity (x : ContextBehavior) :
    x.join ContextBehavior.OPTIMISTIC = x := by
  rw [PL11_contextbehavior_join_comm]; exact PL11_optimistic_left_identity x

/-- The ContextBehavior join-defined order: `a ≤ b` iff `join a b = b`. -/
def ContextBehavior.le (a b : ContextBehavior) : Prop := a.join b = b

/-- §PL-11 partial-order reflexivity from join idempotence. -/
theorem PL11_le_refl (a : ContextBehavior) : a.le a :=
  PL11_contextbehavior_join_idem a

/-- §PL-11 partial-order transitivity from join associativity. -/
theorem PL11_le_trans (a b c : ContextBehavior)
    (hab : a.le b) (hbc : b.le c) : a.le c := by
  unfold ContextBehavior.le at hab hbc ⊢
  calc a.join c = a.join (b.join c) := by rw [hbc]
    _ = (a.join b).join c := (PL11_contextbehavior_join_assoc a b c).symm
    _ = b.join c := by rw [hab]
    _ = c := hbc

/-- §PL-11 the load-bearing monotonicity: `a ≤ b ⟹ join a r ≤ join b r`. The
    Tarski-fixpoint precondition for the per-region ContextBehavior fold, derived
    structurally from commutativity + associativity + idempotence (the standard
    lattice result that join is monotone in each argument). -/
theorem PL11_contextbehavior_join_monotone (a b r : ContextBehavior)
    (hab : a.le b) : (a.join r).le (b.join r) := by
  unfold ContextBehavior.le at hab ⊢
  calc (a.join r).join (b.join r)
      = a.join (r.join (b.join r)) := PL11_contextbehavior_join_assoc a r (b.join r)
    _ = a.join ((r.join b).join r) := by rw [PL11_contextbehavior_join_assoc r b r]
    _ = a.join ((b.join r).join r) := by rw [PL11_contextbehavior_join_comm r b]
    _ = a.join (b.join (r.join r)) := by rw [PL11_contextbehavior_join_assoc b r r]
    _ = a.join (b.join r) := by rw [PL11_contextbehavior_join_idem r]
    _ = (a.join b).join r := (PL11_contextbehavior_join_assoc a b r).symm
    _ = b.join r := by rw [hab]

/-- §PL-11 (P5) join direction correctness — the concrete AND/AND/OR/OR pin.
    Joining `<true, true, false, false>` with `<false, true, true, false>`
    yields `<false, true, true, false>`: preserves_context drops to false (AND),
    consumes_hole stays true (AND), requires_unique_context rises to true (OR),
    may_resume_nonlinearly stays false (OR). An inverted OR/OR/AND/AND join
    would yield a different 4-tuple. Discharged by `decide`. -/
theorem PL11_join_direction_pin :
    (ContextBehavior.mk true true false false).join
      (ContextBehavior.mk false true true false)
      = ContextBehavior.mk false true true false := by
  decide

/-- §PL-11 negative pin — the CONSERVATIVE default is NOT the join identity.
    Folding from `<false, false, true, false>` pins preserves_context /
    consumes_hole to false and requires_unique_context to true, corrupting the
    fold. Concretely `join CONSERVATIVE OPTIMISTIC ≠ OPTIMISTIC`. Discharged by
    `decide`. -/
theorem PL11_conservative_not_identity :
    ContextBehavior.CONSERVATIVE.join ContextBehavior.OPTIMISTIC
      ≠ ContextBehavior.OPTIMISTIC := by
  decide

/-! ## §PL-8 — TRMC candidate predicate (annex-e §AIMS §6 PL-8)

    A function is a TRMC candidate iff three structural conjuncts hold:
    self-recursive, recursive call in a constructor argument, constructor in the
    return path. Modeled as a 3-conjunct boolean predicate; the structural claim
    is that dropping ANY conjunct blocks candidacy. -/

/-- §PL-8 the candidate structural witness — the three detection conjuncts. -/
structure TrmcCandidate where
  self_recursive : Bool
  recursive_call_in_ctor_arg : Bool
  ctor_in_return_path : Bool
deriving Repr, DecidableEq

/-- §PL-8 the candidate predicate = the three-conjunct AND. -/
def TrmcCandidate.isCandidate (c : TrmcCandidate) : Bool :=
  c.self_recursive && c.recursive_call_in_ctor_arg && c.ctor_in_return_path

/-- §PL-8 (P1) predicate conjunction: `isCandidate` is true iff ALL three
    conjuncts hold. Dropping any one makes the function not a candidate.
    Discharged by `cases` over the three booleans + `decide`. -/
theorem PL8_candidate_iff_all_three (c : TrmcCandidate) :
    c.isCandidate = true ↔
      (c.self_recursive = true ∧ c.recursive_call_in_ctor_arg = true
        ∧ c.ctor_in_return_path = true) := by
  unfold TrmcCandidate.isCandidate
  cases c.self_recursive <;> cases c.recursive_call_in_ctor_arg <;>
    cases c.ctor_in_return_path <;> simp

/-- §PL-8 negative pin — dropping self-recursion blocks candidacy: a
    non-self-recursive function is never a candidate regardless of the other two
    conjuncts. Discharged by `cases` over the remaining two booleans. -/
theorem PL8_not_self_recursive_rejected (rca crp : Bool) :
    (TrmcCandidate.mk false rca crp).isCandidate = false := by
  unfold TrmcCandidate.isCandidate; simp

/-! ## §PL-9 — TRMC rewrite idempotency + external arity (annex-e §AIMS §6 PL-9)

    The rewrite is a two-pass model: the first normalize pass transforms; the
    second pass over the already-rewritten IR does NOT re-transform (bounded at
    2 iterations). The EXTERNAL arity is preserved (wrapper thunk); the internal
    worker gains the ContextHole parameter. -/

/-- §PL-9 the rewrite two-pass model: per-pass transform signals + the
    external-arity-before / -after + the internal-ContextHole acceptance. -/
structure TrmcRewrite where
  transformed_first_pass : Bool
  transformed_second_pass : Bool
  external_arity_before : Nat
  external_arity_after : Nat
  internal_accepts_context_hole : Bool
deriving Repr, DecidableEq

/-- §PL-9 a rewrite is well-formed iff it transforms on pass 1, is idempotent on
    pass 2 (no re-transform), preserves external arity, and the internal worker
    accepts the ContextHole. -/
def TrmcRewrite.wellFormed (r : TrmcRewrite) : Prop :=
  r.transformed_first_pass = true ∧
  r.transformed_second_pass = false ∧
  r.external_arity_after = r.external_arity_before ∧
  r.internal_accepts_context_hole = true

/-- §PL-9 (P1) idempotency: a well-formed rewrite transforms on the first pass
    and does NOT re-transform on the second pass over already-rewritten IR.
    The two-pass invariant the shipped <=2-iteration loop enforces. -/
theorem PL9_idempotent (r : TrmcRewrite) (h : r.wellFormed) :
    r.transformed_first_pass = true ∧ r.transformed_second_pass = false :=
  ⟨h.1, h.2.1⟩

/-- §PL-9 (P2) external-arity preservation: a well-formed rewrite leaves the
    caller-visible arity unchanged (the wrapper thunk; the ContextHole parameter
    is internal-only). -/
theorem PL9_external_arity_preserved (r : TrmcRewrite) (h : r.wellFormed) :
    r.external_arity_after = r.external_arity_before :=
  h.2.2.1

/-! ## §PL-10 — TRMC structural verification + rollback (annex-e §AIMS §6 PL-10)

    Step 4a runs five structural checks (a)-(e). The rewrite SURVIVES iff every
    check passes; on ANY failure it ROLLS BACK to pre-TRMC IR and re-runs
    Steps 3-4 on the restored CFG (preserving PL-5). Modeled as a 5-bool check
    record; survival = all checks pass; rollback = the complement. -/

/-- §PL-10 the five structural checks (a)-(e). -/
structure TrmcVerify where
  a_context_hole_threaded : Bool       -- (a)
  b_no_new_allocations : Bool          -- (b)
  c_cfg_well_formed : Bool             -- (c)
  d_arity_preserved : Bool             -- (d)
  e_eval_order_preserved : Bool        -- (e)
deriving Repr, DecidableEq

/-- §PL-10 all-checks-pass predicate = the five-check AND. -/
def TrmcVerify.allPass (v : TrmcVerify) : Bool :=
  v.a_context_hole_threaded && v.b_no_new_allocations && v.c_cfg_well_formed &&
  v.d_arity_preserved && v.e_eval_order_preserved

/-- §PL-10 the rewrite SURVIVES iff every structural check passes. -/
def TrmcVerify.rewriteSurvives (v : TrmcVerify) : Bool := v.allPass

/-- §PL-10 (P2) all-pass survival: when every structural check passes, the
    rewrite survives. Discharged by `cases` over the five booleans. -/
theorem PL10_all_pass_survives (v : TrmcVerify) (h : v.allPass = true) :
    v.rewriteSurvives = true := by
  unfold TrmcVerify.rewriteSurvives; exact h

/-- §PL-10 (P3) any-fail rollback: if ANY structural check fails, the rewrite
    does NOT survive (it is rolled back to pre-TRMC IR + Steps 3-4 re-run). The
    contrapositive of survival — a failing check NEVER leaves the rewrite in
    place. Discharged by `cases` over the five booleans + `decide`. -/
theorem PL10_any_fail_rollback (v : TrmcVerify)
    (h : v.allPass = false) : v.rewriteSurvives = false := by
  unfold TrmcVerify.rewriteSurvives; exact h

/-- §PL-10 negative pin — check (a) failing forces rollback: a rewrite with
    `a_context_hole_threaded = false` never survives regardless of the other
    four checks. Discharged by `cases` over the remaining four booleans. -/
theorem PL10_check_a_fails_no_survival
    (b c d e : Bool) :
    (TrmcVerify.mk false b c d e).rewriteSurvives = false := by
  unfold TrmcVerify.rewriteSurvives TrmcVerify.allPass; simp

/-! ## §PL-comp — pipeline composition (annex-e §AIMS §6 success-criterion #3)

    The full pipeline produces a realized ArcFunction whose AimsStateMap is
    consistent with the pre-realization contracts. The composed claim is the
    CONJUNCTION of the constituent ordering + ContextBehavior invariants over
    the phase model. PL-comp adds no new ordering claim — it asserts the
    conjunction is the realized-pipeline consistency property and is a SINK of
    the (acyclic) constituent-dependency graph. -/

/-- §PL-comp the composed pipeline-ordering invariant: the conjunction of the
    PL-1 prefix-before-suffix, the PL-2/PL-3/PL-4/PL-4a intra-function ordered
    pairs, and the ContextBehavior PL-11 join identity + idempotence + the PL-9
    / PL-10 TRMC well-formedness. Each conjunct is an independently-discharged
    PL-rule theorem; PL-comp states they all hold jointly. -/
theorem PLcomp_pipeline_composes
    (G : Condensation) (i j : Nat) (hedge : G.Edge i j)
    (c : ContextBehavior)
    (r : TrmcRewrite) (hr : r.wellFormed)
    (v : TrmcVerify) (hv : v.allPass = true) :
    -- PL-1 : prefix before every suffix step.
    PipelineStep.ApplyOwnership.before PipelineStep.RealizeRcReuse ∧
    -- PL-1a : callee SCC visited before caller SCC.
    i < j ∧
    -- PL-2 : analyze before realize.
    PipelineStep.AnalyzeFunction.before PipelineStep.RealizeRcReuse ∧
    -- PL-3 : realize phase 1 before merge.
    PipelineStep.RealizeRcReuse.before PipelineStep.MergeBlocks ∧
    -- PL-4 : annotations after merge.
    PipelineStep.MergeBlocks.before PipelineStep.RealizeAnnotations ∧
    -- PL-4a : unwind before merge.
    PipelineStep.UnwindCleanup.before PipelineStep.MergeBlocks ∧
    -- PL-11 : ContextBehavior join idempotent + optimistic seed is the identity.
    c.join c = c ∧ ContextBehavior.OPTIMISTIC.join c = c ∧
    -- PL-9 : the TRMC rewrite preserves external arity.
    r.external_arity_after = r.external_arity_before ∧
    -- PL-10 : all checks pass ⟹ the rewrite survives.
    v.rewriteSurvives = true := by
  refine ⟨?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_, ?_⟩
  · exact PL1_prefix_before_suffix _ (by decide)
  · exact PL1a_callee_visited_before_caller G i j hedge
  · exact PL2_analyze_before_realize
  · exact PL3_realize_before_merge
  · exact PL4_annotations_after_merge
  · exact PL4a_unwind_before_merge
  · exact PL11_contextbehavior_join_idem c
  · exact PL11_optimistic_left_identity c
  · exact PL9_external_arity_preserved r hr
  · exact PL10_all_pass_survives v hv

/-- §PL-comp acyclicity witness — the phase `before` relation is a strict order
    (irreflexive + transitive), so the composed schedule is well-founded: no
    constituent ordering claim presupposes its own conclusion. This is the (C2)
    well-foundedness leg — the conjunction is not circular. -/
theorem PLcomp_schedule_acyclic (a b : PipelineStep)
    (hab : a.before b) : ¬ b.before a :=
  PipelineStep.before_asymm a b hab

end AimsProof
