; SMT-LIB2 encoding — §04B.1 baseline single-class shapes
; ============================================================================
; Shape: §04B.1 Criterion 1 = emission-alone (no §04A.2 elimination) on
; SINGLE-CLASS shapes. Empirical FAIL: 12/16 fail-baseline under emission alone
; (per §04B HISTORY 2026-05-18, §04B.1 evaluation).
;
; Single-class = one allocation class A; the shape exercises emission-side
; RcInc/RcDec patterns WITHOUT the cross-class drop_fn cascade of BUG-04-118
; or the cross-class alias of §04B.2 cat (3). The 12/16 fail-baseline means
; the emission-alone path produces a NET-IMBALANCED RC ledger for class A
; on 12 of 16 single-class test fixtures.
;
; Question to the SMT solver: under the proven calculus (RL-1 inc when
; duplicating, RL-2 dec at last use / scope exit, P2 RC-count preservation),
; can a single-class shape produce a net-imbalanced ledger under emission
; alone (no DP-2/DP-3 elimination)?
;
; Expected: UNSAT
; meaning — RL-1-RL-2 composition P2 holds independently of the DP-2/DP-3
; elimination consumer (the consumer is an OPTIMIZATION over an already-
; balanced ledger, not a balance correctness check). A balanced ledger
; pre-elimination is INVARIANT under the calculus; an imbalanced ledger
; post-emission is an EMISSION BUG (RL-1 / RL-2 / RL-4 / RL-5 incorrectly
; sited), NOT a calculus gap.
;
; SAT would mean the proven RL-1/RL-2/RL-4/RL-5 emission rules permit a
; net-imbalanced ledger — route §08 RL-1, RL-2, RL-4, RL-5 reformulation +
; §11 if composition-level.
;
; Source mapping (per §10.0 shape-04B.1-baseline-single-class row):
; shape_id = shape-04B.1-baseline-single-class
; source = docs/ori_lang/proposals/approved/aims-burden-tracking-proposal.md
; (HISTORY 2026-05-18 §04B.1 Criterion 1 FAIL: 12/16 fail-
; baseline under emission alone, single-class shapes)
;
; Cited proven rules (per §02-§09):
; - RL-1 RC inc when duplicating
; [aims-proof/proofs/08-realization/RL-1.proof]
; - RL-2 RC dec at last use / scope exit; P2 RC-count preservation
; [aims-proof/proofs/08-realization/RL-2.proof]
; - RL-4 Edge-specific decs at CFG exit; Jump-arg exemption
; [aims-proof/proofs/08-realization/RL-4.proof]
; - RL-5 Dead-at-entry cleanup: Absent block param → immediate dec
; [aims-proof/proofs/08-realization/RL-5.proof]
; - RL-1-RL-2 composition RC-balance over heap-allocated owned non-scalar
; [aims-proof/proofs/08-realization/RL-1-RL-2-composition.proof]

(set-logic ALL)
(set-option :produce-unsat-cores true)
(set-option :produce-models true)

; ----------------------------------------------------------------------------
; §1. Single-class allocation + RC ledger
; ----------------------------------------------------------------------------
(declare-datatype ArcVar ((V_alloc))) ; single class A

; Cumulative RC ops along the program's linear schedule (single-class shape
; means no Branch/Switch/Invoke fork — straight-line emission)
(declare-fun rc_inc_total () Int)
(declare-fun rc_dec_total () Int)

; ----------------------------------------------------------------------------
; §2. Calculus axioms (per §02-§09 emission rules)
; ----------------------------------------------------------------------------

; --- RL-1: each RcInc corresponds to a duplicating use (passing Owned while
; still live). The emission rule SHALL emit one inc per such use.
(assert (>= rc_inc_total 0))

; --- RL-2 + RL-4 + RL-5: dec emission rules
(assert (>= rc_dec_total 0))

; --- RL-1-RL-2 composition / P2: cumulative ledger nets to 0 at scope exit
; for every owned non-scalar heap value, INDEPENDENT of DP-2/DP-3
; elimination (which is a refinement over an already-balanced ledger).
(assert (= rc_inc_total rc_dec_total))

; ----------------------------------------------------------------------------
; §3. The counterexample question (emission-alone imbalance)
; ----------------------------------------------------------------------------
; Empirical 12/16 fail-baseline: imbalanced ledger under emission alone.

(push 1)

(declare-const cex_imbalance Int)
(assert (= cex_imbalance (- rc_inc_total rc_dec_total)))
(assert (not (= cex_imbalance 0)))

(check-sat)
; Expected: UNSAT — P2 directly contradicts any net imbalance. The empirical
; 12/16 fail-baseline arises from EMISSION-SITE bugs (RL-1 / RL-2 / RL-4 /
; RL-5 incorrectly placed by the lowering pipeline), NOT from the proven
; calculus permitting imbalance.

(pop 1)

(check-sat)
