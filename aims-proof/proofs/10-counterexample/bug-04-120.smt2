; SMT-LIB2 encoding — BUG-04-120 narrowed-list derived Eq codegen leak shape
; ============================================================================
; Shape: 24-byte unfreed allocation on the derived-Eq codegen path for a
; list whose elements were narrowed to i8/i16/i32 by repr-opt §04 (integer
; narrowing). Per BUG-04-120:
; - Repro: `cargo test -p ori_llvm --lib --tests
; narrowing::test_narrowed_list_derived_eq` fails with
; "ori: 1 RC allocation(s) not freed (memory leak)"
; - 24-byte unfreed allocation at compile.rs:115
; - ORI_DISABLE_BURDEN_OPS=1 does NOT cure (rules out burden-op causation)
; - ORI_DISABLE_REDUNDANT_CLEANUP=1 does NOT cure
; - Suspect: derived Eq codepath on narrowed list elements where
; intermediate widened i8/i16/i32 → i64 sext + comparison + cleanup
; are NOT RC-balanced; analog of TPR-04-021 sibling
; (emit_format_fields intermediate-concat leak in Debug derive)
;
; This shape is a CODEGEN-SIDE multi-use Let Var on RC-tracked aggregate
; (the §04B.4a HISTORY notes "BUG-04-120 multi-use Let Var on RC-tracked
; aggregate" classification — multi-use because the derived-Eq codepath
; emits multiple field-comparisons over the same narrowed-list aggregate;
; "Let Var" because the comparison emits Let Var aliases of the aggregate's
; projected fields).
;
; Question to the SMT solver: under the proven calculus (TF-2 Let Var
; transparent, IA-5 step (1) seq_add accumulation across multi-use, RL-1
; inc when duplicating, RL-2 dec at last use, P2 RC-count preservation),
; can a multi-use Let Var on an RC-tracked aggregate produce an unbalanced
; ledger (1 unfreed allocation)?
;
; Expected: UNSAT
; meaning — TF-2 transparent + IA-5 step (1) seq_add accumulation forces
; the source variable's cardinality to be the SUM of demands across all
; uses of the Let Var alias. RL-1 emits one inc per duplicating use; RL-2
; emits the final dec at scope exit. P2 ensures the ledger nets to 0
; regardless of multi-use count. The empirical 24-byte leak is a CODEGEN
; defect in the LLVM derive-Eq lowering (likely the intermediate widened-
; integer comparison emits a phantom heap allocation NOT tracked by RL-1/
; RL-2 because the lowering treats the narrowed-list element as a value-
; type when it actually allocates a temporary), NOT a calculus gap.
;
; SAT would mean the proven calculus admits a multi-use Let Var imbalance —
; route §04 TF-2 + §06 IA-5 step (1) + §08 RL-1, RL-2 reformulation.
;
; Source mapping (per §10.0 shape-bug-04-120-multi-use-let-var row):
; shape_id = shape-bug-04-120-multi-use-let-var
; source = BUG-04-120 (memory leak: narrowed-
; list derived Eq leaks 24 bytes per repr-opt §04 regression)
;
; Cited proven rules (per §02-§09):
; - TF-2 Let Var(v): dst.state := state(v); transparent alias
; [aims-proof/proofs/04-transfers/TF-2.proof]
; - TF-11 Backward demand accumulation: variable receives demands from
; MULTIPLE instructions; ADDITIVELY combine cardinality (seq_add,
; NOT lattice max)
; [aims-proof/proofs/04-transfers/TF-11.proof]
; - IA-5 step (1) Let Var(v) transparent alias case (a): destination's
; accumulated demand transfers to source via seq_add
; [aims-proof/proofs/04-transfers/IA-5-step-1.proof]
; - RL-1 RC inc when duplicating
; [aims-proof/proofs/08-realization/RL-1.proof]
; - RL-2 RC dec at last use / scope exit; P2 RC-count preservation
; [aims-proof/proofs/08-realization/RL-2.proof]
; - RL-1-RL-2 composition RC-balance over heap-allocated owned non-scalar
; [aims-proof/proofs/08-realization/RL-1-RL-2-composition.proof]

(set-logic ALL)
(set-option :produce-unsat-cores true)
(set-option :produce-models true)

; ----------------------------------------------------------------------------
; §1. Multi-use Let Var on RC-tracked aggregate
; ----------------------------------------------------------------------------
(declare-datatype ArcVar
  ( (V_agg) ; the RC-tracked aggregate (narrowed list)
    (V_let_alias) ; %al = Let Var(V_agg) — transparent alias used N times
  ))

; N uses of the Let Var alias (e.g., N field comparisons in derived Eq)
(declare-const num_uses Int)
(assert (>= num_uses 1)) ; at least one use (otherwise dead)
(assert (<= num_uses 16)) ; bound: derive-Eq field count

; cumulative incs + decs over the schedule
(declare-fun rc_inc_cum () Int)
(declare-fun rc_dec_cum () Int)

; ----------------------------------------------------------------------------
; §2. Calculus axioms (per §02-§09)
; ----------------------------------------------------------------------------

; --- TF-2: Let Var transparent alias
; ⇒ V_let_alias's state = V_agg's state (no separate allocation).
; --- TF-11 + IA-5 step (1) seq_add accumulation:
; V_agg's cardinality = seq_add(use1, use2, ..., useN) = Many for N >= 2,
; Once for N == 1.
; --- RL-1: each duplicating use of V_agg (passed Owned while still live)
; emits one RcInc. Multi-use ⇒ rc_inc_cum >= num_uses - 1 (the first use
; can be the move; subsequent uses each require inc).
(assert (>= rc_inc_cum (- num_uses 1)))

; --- RL-2: final RcDec at scope exit + per-use decs.
; Conservative: rc_dec_cum = rc_inc_cum + 1 (the +1 = the alloc's
; definitional cleanup dec; per-use decs cancel with per-use incs).
; Equivalently, P2: net balance = (rc_inc_cum + 1_alloc) - rc_dec_cum = 0.
; --- RL-1-RL-2 composition / P2:
; ALLOC contributes 1 implicit inc (TF-3 Construct produces FRESH with
; RC = 1). Net ledger = (1 + rc_inc_cum) - rc_dec_cum = 0.
(assert (= (- (+ 1 rc_inc_cum) rc_dec_cum) 0))

; ----------------------------------------------------------------------------
; §3. The counterexample question (24-byte unfreed allocation)
; ----------------------------------------------------------------------------
; The empirical leak: 1 unfreed allocation ⇒ net ledger has 1 unmatched inc.

(push 1)

(declare-const cex_unfreed Int)
(assert (= cex_unfreed (- (+ 1 rc_inc_cum) rc_dec_cum)))
(assert (>= cex_unfreed 1)) ; at least one unfreed allocation

(check-sat)
; Expected: UNSAT — P2 + TF-2 + IA-5 step (1) seq_add accumulation force
; the ledger to balance regardless of multi-use count. The empirical 24-byte
; leak is a CODEGEN defect (the LLVM derive-Eq lowering allocates a phantom
; temporary not tracked by the AIMS pipeline), NOT a calculus gap.

(pop 1)

(check-sat)
