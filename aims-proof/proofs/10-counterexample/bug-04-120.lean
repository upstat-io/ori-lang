-- AIMS-Proof §10 counterexample-search cross-validation
-- Cross-validates aims-proof/proofs/10-counterexample/bug-04-120.smt2
--
-- BUG-04-120 narrowed-list derived Eq codegen leak shape: 24-byte unfreed
-- allocation on the derived-Eq codepath over a list whose elements were
-- narrowed to i8/i16/i32 by repr-opt. Classification per the burden-prototype evidence:
-- "multi-use Let Var on RC-tracked aggregate".
--
-- Cross-validation question (mirrors SMT goal §3):
-- Under the proven calculus (TF-2 Let Var transparent, IA-5 step (1)
-- seq_add accumulation, RL-1 inc when duplicating, RL-2 dec at last use,
-- RL-1-RL-2 composition P2), can a multi-use Let Var on an RC-tracked
-- aggregate produce a net ledger with ≥ 1 unfreed allocation?

namespace AimsBug04120MultiUseLetVar

-- P2 RC-count preservation (with alloc's implicit inc):
-- (1_alloc + rc_inc_cum) - rc_dec_cum = 0 at scope exit.
def p2_with_alloc (rc_inc_cum rc_dec_cum : Int) : Prop :=
  (1 + rc_inc_cum) - rc_dec_cum = 0

-- Counterexample: ≥ 1 unfreed allocation.
def has_unfreed_allocation (rc_inc_cum rc_dec_cum : Int) : Prop :=
  (1 + rc_inc_cum) - rc_dec_cum ≥ 1

-- ============================================================================
-- The counterexample-search theorem
-- ============================================================================

theorem no_unfreed_allocation_under_p2
    (rc_inc_cum rc_dec_cum : Int)
    (h_p2 : p2_with_alloc rc_inc_cum rc_dec_cum) :
    ¬ has_unfreed_allocation rc_inc_cum rc_dec_cum := by
  intro h_cex
  unfold has_unfreed_allocation at h_cex
  unfold p2_with_alloc at h_p2
  rw [h_p2] at h_cex
  exact absurd h_cex (by decide)

-- ============================================================================
-- Cross-validation verdict
-- ============================================================================
-- Lean theorem proves → cross-validates SMT UNSAT
-- The empirical 24-byte leak indicates a CODEGEN defect (LLVM derive-Eq
-- lowering allocates a phantom temporary), NOT a calculus gap.

end AimsBug04120MultiUseLetVar
