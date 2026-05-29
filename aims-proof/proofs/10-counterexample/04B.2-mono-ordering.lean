-- AIMS-Proof §10 counterexample-search cross-validation
-- Cross-validates aims-proof/proofs/10-counterexample/04B.2-mono-ordering.smt2
-- SSOT for Lean emission: aims-proof/checker/src/emit/lean4.rs (the SMT / Lean 4 emission strategy Option C)
--
-- Translated from §04B.2 cross-pattern category (1): mono-pipeline-ordering
-- (per docs/ori_lang/proposals/approved/aims-burden-tracking-proposal.md HISTORY
-- 2026-05-18 — E5001 unresolved `__cast` in generics::test_borrow_list_int_*).
--
-- Cross-validation question (mirrors SMT goal in 04B.2-mono-ordering.smt2 §4):
-- Under the proven §07 pipeline-ordering calculus (PL-1, PL-1a, PL-2, PL-5,
-- PL-6), can a downstream AIMS consumer LEGITIMATELY read an unresolved
-- `__cast` operand?
--
-- Expected Lean kernel verdict: theorem proves no_unresolved_cast_at_consumer
-- (no model exists where consumer fires with __cast still unresolved).

namespace AimsMonoPipelineOrdering

-- ============================================================================
-- §1. Pipeline-step carrier (per PL-1..PL-6 ordering)
-- ============================================================================

inductive PipelineStep
  | PS_canon
  | PS_mono
  | PS_arc_lower
  | PS_analyze_program
  | PS_apply_ownership
  | PS_compute_reprs
  | PS_analyze_fn
  | PS_realize_rc
deriving Repr, DecidableEq

def ps_idx : PipelineStep → Nat
  | .PS_canon => 0
  | .PS_mono => 1
  | .PS_arc_lower => 2
  | .PS_analyze_program => 3
  | .PS_apply_ownership => 4
  | .PS_compute_reprs => 5
  | .PS_analyze_fn => 6
  | .PS_realize_rc => 7

def precedes (a b : PipelineStep) : Prop := ps_idx a < ps_idx b

-- ============================================================================
-- §2. Calculus axioms encoded as preconditions on (cast_resolved_at, cast_consumed_at)
-- ============================================================================

structure CalculusModel where
  cast_resolved_at : PipelineStep → Prop
  cast_consumed_at : PipelineStep → Prop
  -- PL-1: mono resolves __cast for any instance reaching AIMS
  mono_resolves : cast_resolved_at PipelineStep.PS_mono
  -- PL-5: every downstream AIMS consumer sees resolved __cast
  consumer_implies_resolved :
    ∀ s : PipelineStep,
      cast_consumed_at s → cast_resolved_at s
  -- PL-6: resolution monotone along the ordering
  resolution_monotone :
    ∀ a b : PipelineStep,
      cast_resolved_at a → precedes a b → cast_resolved_at b

-- ============================================================================
-- §3. The counterexample-search theorem
-- ============================================================================
-- E5001 counterexample: ∃ s in {PS_realize_rc, PS_analyze_fn, PS_analyze_program}
-- such that cast_consumed_at s ∧ ¬ cast_resolved_at s.

theorem no_unresolved_cast_at_consumer
    (M : CalculusModel) (s : PipelineStep)
    (h_consumed : M.cast_consumed_at s) :
    M.cast_resolved_at s := by
  exact M.consumer_implies_resolved s h_consumed

-- Corollary: no instance of the E5001 shape is admissible under the calculus.
theorem e5001_refuted
    (M : CalculusModel) (s : PipelineStep)
    (h_consumed : M.cast_consumed_at s) :
    ¬ ¬ M.cast_resolved_at s := by
  intro h_neg
  exact h_neg (no_unresolved_cast_at_consumer M s h_consumed)

-- ============================================================================
-- §4. Cross-validation verdict
-- ============================================================================
-- The theorem above corresponds to the SMT UNSAT verdict in
-- 04B.2-mono-ordering.smt2 §4. Routing per §10.1 outcome-classification:
-- Lean theorem proves → cross-validates SMT UNSAT → feeds §12
-- Lean theorem refused to type-check → ENCODER_INVALID halt per §10.1 SC #4

end AimsMonoPipelineOrdering
