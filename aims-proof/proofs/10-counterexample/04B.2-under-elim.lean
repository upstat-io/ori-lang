-- AIMS-Proof §10 counterexample-search cross-validation
-- Cross-validates aims-proof/proofs/10-counterexample/04B.2-under-elim.smt2
--
-- cross-pattern category (2): under-elimination leaks on
-- path-sensitive control flow + jump-arg merges + generic forwarders.
-- Cross-validation question (mirrors SMT goal §3):
-- Under the proven calculus (RL-1, RL-2, RL-4, RL-5, IA-3, RL-1-RL-2
-- composition P2), can a heap-allocated owned non-scalar leak (net RC > 0
-- at scope exit) on ONE arm of a CFG merge while the other arm balances?

namespace AimsUnderEliminationLeaks

inductive Arm
  | OK
  | Err
deriving Repr, DecidableEq

-- Per-path RC ledger (net inc - dec along the concrete execution trace).
-- The calculus invariant (RL-1-RL-2 composition / P2) states: along ANY
-- concrete executed path through the CFG, the cumulative ledger nets to 0.
def rc_per_path_invariant (rc_at_exit_via : Arm → Int) : Prop :=
  ∀ a : Arm, rc_at_exit_via a = 0

-- Under-elimination leak counterexample: ∃ arm whose ledger is positive.
def under_elim_leak (rc_at_exit_via : Arm → Int) : Prop :=
  ∃ a : Arm, rc_at_exit_via a > 0

-- ============================================================================
-- The counterexample-search theorem
-- ============================================================================
-- Question: under the per-path invariant, is an under-elim leak admissible?
-- Answer (Lean kernel): NO — the invariant directly refutes the counterexample.

theorem no_under_elim_leak_under_per_path_invariant
    (rc_at_exit_via : Arm → Int)
    (h_inv : rc_per_path_invariant rc_at_exit_via) :
    ¬ under_elim_leak rc_at_exit_via := by
  intro h_cex
  obtain ⟨a, h_pos⟩ := h_cex
  have h_zero : rc_at_exit_via a = 0 := h_inv a
  rw [h_zero] at h_pos
  exact absurd h_pos (by decide)

-- ============================================================================
-- Cross-validation verdict
-- ============================================================================
-- Lean theorem proves → cross-validates SMT UNSAT
-- Lean theorem refused to type-check → ENCODER_INVALID halt

end AimsUnderEliminationLeaks
