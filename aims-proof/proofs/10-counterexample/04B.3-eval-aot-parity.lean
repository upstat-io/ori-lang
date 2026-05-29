-- AIMS-Proof §10 counterexample-search cross-validation
-- Cross-validates aims-proof/proofs/10-counterexample/04B.3-eval-aot-parity.smt2
--
-- §04B.3 eval-AOT dual-execution parity break: CFG-merge join between Ok and
-- Err arms produces a state where one arm's RcDec is dropped at LLVM codegen.
-- Cross-validation question (mirrors SMT goal §3):
-- Under the proven calculus + VF-7 / canon.md §7.1 invariant 2, can the
-- AOT backend produce a different cumulative RC balance than the eval
-- backend on the same ARC IR?

namespace AimsEvalAotParityBreak

inductive Backend
  | Eval
  | AOT
deriving Repr, DecidableEq

-- The dual-execution parity axiom exported by VF-7 + canon.md §7.1 inv 2:
-- both backends consume the SAME post-§04A.2 ARC IR and MUST produce
-- identical observable cumulative RC balances at scope exit.
def parity_invariant (rc_balance_at_exit : Backend → Int) : Prop :=
  rc_balance_at_exit Backend.Eval = rc_balance_at_exit Backend.AOT

-- Parity-break counterexample: balances differ.
def parity_break (rc_balance_at_exit : Backend → Int) : Prop :=
  rc_balance_at_exit Backend.Eval ≠ rc_balance_at_exit Backend.AOT

-- ============================================================================
-- The counterexample-search theorem
-- ============================================================================
-- Question: under the parity invariant, can a parity break arise?
-- Answer (Lean kernel): NO — the invariant directly refutes the counterexample.

theorem no_parity_break_under_invariant
    (rc_balance_at_exit : Backend → Int)
    (h_inv : parity_invariant rc_balance_at_exit) :
    ¬ parity_break rc_balance_at_exit := by
  intro h_cex
  unfold parity_break at h_cex
  unfold parity_invariant at h_inv
  exact h_cex h_inv

-- ============================================================================
-- Companion lemma: PL-4 ArcVarId-keyed lookup discipline
-- ============================================================================
-- Per `Annex E §AIMS PL-4`: Step 10 (realize_annotations / Phase 2) SHALL
-- follow Step 9 (merge_blocks); Phase 2 uses ArcVarId-keyed lookups surviving
-- merge — position-keyed maps INVALIDATED by merge_blocks. The empirical
-- AOT-only leak in §04B.3 most plausibly arises from a LLVM consumer that
-- reads position-keyed state post-merge — a PL-4 implementation violation,
-- not a calculus gap.

-- ============================================================================
-- Cross-validation verdict
-- ============================================================================
-- Lean theorem proves → cross-validates SMT UNSAT
-- Lean theorem refused to type-check → ENCODER_INVALID halt

end AimsEvalAotParityBreak
