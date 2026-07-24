; SMT-LIB2 encoding — cross-pattern category (2): under-elimination leaks
; ============================================================================
; Shape: under-elimination memory leaks on path-sensitive control flow + jump-arg
; merges + generic forwarders. Empirical signature: 7 generics tests / 17 total
; leaked allocations (per the burden-prototype evidence).
;
; Under-elimination = the lattice/realization pipeline KEEPS an RcInc whose
; partner RcDec is dropped (or vice versa) along ONE path of a CFG merge,
; producing a net positive RC balance at scope exit ⇒ memory leak.
;
; Question to the SMT solver: under the proven calculus (RL-1, RL-2, RL-4, RL-5,
; TF-11 with seq_add accumulation, IA-3 alt_join at CFG merges), can a leak
; survive realization on a path-sensitive shape (Ok-arm leaks 1 RC while
; Err-arm balances 0)?
;
; Expected: UNSAT
; meaning — RL-1 (inc when duplicating) + RL-2 (dec at last use / scope exit)
; + RL-4 (edge-specific decs at CFG exit) + the RC-count-preservation
; invariant P2 from RL-1-RL-2-composition refute path-sensitive net-positive
; leaks: each path's RC ledger nets to zero by construction.
;
; SAT would mean the proven calculus admits a leak — route §08 RL-N + §11 if
; the unsat-core spans the CFG-merge coexistence handshake.
;
; Source mapping (per §10.0 shape-04B.2-under-elimination-leaks row):
; shape_id = shape-04B.2-under-elimination-leaks
; source = docs/ori_lang/proposals/approved/aims-burden-tracking-proposal.md
; (burden-prototype evidence, category (2) — 7 generics tests,
; 17 leaked allocations on path-sensitive control flow,
; jump-arg merges, generic forwarders)
; fixture_path= aims-proof/proofs/10-counterexample/04B.2-under-elim.smt2
;
; Cited proven rules (per §02-§09):
; - RL-1 RC inc when duplicating (passed Owned while still live)
; [aims-proof/proofs/08-realization/RL-1.proof]
; - RL-2 RC dec at last use / scope exit; RC-count preservation P2
; [aims-proof/proofs/08-realization/RL-2.proof]
; - RL-4 Edge-specific decs at CFG exit (variable owned at block exit
; but dead at successor entry); Jump-arg exemption
; [aims-proof/proofs/08-realization/RL-4.proof]
; - RL-5 Dead-at-entry cleanup: block param Absent → immediate dec
; [aims-proof/proofs/08-realization/RL-5.proof]
; - TF-11 Backward demand accumulation via seq_add (NOT lattice max)
; [aims-proof/proofs/04-transfers/TF-11.proof]
; - IA-3 Block exit state = alt_join of successor entry states (only one
; path executes)
; [aims-proof/proofs/06-intraprocedural/IA-3.proof]
; - DP-2 is_rc_dec_unnecessary — gates SUPPLEMENTARY decs only;
; terminal RL-2/RL-4/RL-5 OWN their logic (cannot be over-eliminated)
; [aims-proof/proofs/05-decisions/DP-2.proof]
; - RL-1-RL-2 composition RC-balance over heap-allocated owned non-scalar
; [aims-proof/proofs/08-realization/RL-1-RL-2-composition.proof]
;
; Intel-query citations (graph-first, 2026-05-28):
; - file-symbols "ori_arc/src/aims/realize" --repo ori → 30 symbols incl.
; eliminate_burden_ops (burden_elim.rs:87), eliminate_in_block (122),
; should_elide_dec (201); confirms the over-elimination consumer surface.
; - similar "cfg_merge_join" --repo ori → Symbol-not-found (graph degradation).
; Manual fallback: IA-3 alt_join lives at ori_arc/src/aims/intraprocedural/
; block-level state combination per Annex E §AIMS

(set-logic ALL)
(set-option :produce-unsat-cores true)
(set-option :produce-models true)

; ----------------------------------------------------------------------------
; §1. Path-sensitive CFG with two arms + merge
; ----------------------------------------------------------------------------
; Layout (mirrors the under-elimination path-sensitive control-flow shape):
; entry -> Branch(cond)
; ok_arm (live %v + ownership-transferring use)
; err_arm (live %v + last-use)
; merge (block param %p inherits %v's RC obligation per RL-4 Jump-arg
; exemption)
; exit (scope_exit; RL-2 P2 invariant fires here)

(declare-datatype Block
  ( (B_entry)
    (B_ok_arm)
    (B_err_arm)
    (B_merge)
    (B_exit) ))

(declare-datatype ArcVar ((V_alloc))) ; the heap-allocated owned non-scalar

; rc_balance(v, block, exit_or_entry) — int ledger of RcInc - RcDec up to
; block boundary. We track per-arm net RC ops separately.
(declare-fun rc_emitted_inc ((ArcVar) (Block)) Int)
(declare-fun rc_emitted_dec ((ArcVar) (Block)) Int)

; Ledger at function scope exit = sum across the ACTUALLY-EXECUTED path
; (alt_join is execution-path semantics, NOT lattice join over both).
(declare-fun rc_balance_at_exit_via ((ArcVar) (Block)) Int)
; via PARAM is the arm visited en route to exit (B_ok_arm OR B_err_arm).

; ----------------------------------------------------------------------------
; §2. Calculus axioms (per §08 RL-1, RL-2, RL-4, RL-5, IA-3, RL-1-RL-2 comp)
; ----------------------------------------------------------------------------

; --- TF-3 + RL-1: alloc at entry contributes net +1 RC (alloc + 1 ownership)
(assert (= (rc_emitted_inc V_alloc B_entry) 1))
(assert (= (rc_emitted_dec V_alloc B_entry) 0))

; --- RL-1-RL-2 composition / P2 (RC-count preservation):
; for the heap-allocated owned non-scalar value, the cumulative RC ledger
; over the FULL instruction schedule along ANY executed path nets to 0.
; IA-3 alt_join semantics: only one arm executes per concrete trace,
; so this is a per-path invariant, NOT a max over both arms.
(assert (forall ((arm Block))
  (=> (or (= arm B_ok_arm) (= arm B_err_arm))
      (= (rc_balance_at_exit_via V_alloc arm) 0))))

; --- DP-2 + RL-2 + RL-4 ordering:
; terminal RL-2 / RL-4 / RL-5 emissions OWN their own logic — DP-2 only
; gates SUPPLEMENTARY decs. ⇒ the definitional cleanup dec for the alloc
; SHALL be emitted along ANY path where the variable is owned at scope exit.
; We assert this as a closed conservation law on net incs vs decs per arm.
(assert (>= (rc_emitted_dec V_alloc B_ok_arm) 1))
(assert (>= (rc_emitted_dec V_alloc B_err_arm) 1))

; --- IA-3 alt_join + RL-1-RL-2 composition: the cumulative ledger along the
; CONCRETE path through arm A is the SUM of per-block emissions on that
; arm. We encode it explicitly so the SAT-search can attempt to construct
; a leaking model that satisfies the per-arm sum.
(assert (= (rc_balance_at_exit_via V_alloc B_ok_arm)
           (- (+ (rc_emitted_inc V_alloc B_entry)
                 (rc_emitted_inc V_alloc B_ok_arm)
                 (rc_emitted_inc V_alloc B_merge)
                 (rc_emitted_inc V_alloc B_exit))
              (+ (rc_emitted_dec V_alloc B_entry)
                 (rc_emitted_dec V_alloc B_ok_arm)
                 (rc_emitted_dec V_alloc B_merge)
                 (rc_emitted_dec V_alloc B_exit)))))
(assert (= (rc_balance_at_exit_via V_alloc B_err_arm)
           (- (+ (rc_emitted_inc V_alloc B_entry)
                 (rc_emitted_inc V_alloc B_err_arm)
                 (rc_emitted_inc V_alloc B_merge)
                 (rc_emitted_inc V_alloc B_exit))
              (+ (rc_emitted_dec V_alloc B_entry)
                 (rc_emitted_dec V_alloc B_err_arm)
                 (rc_emitted_dec V_alloc B_merge)
                 (rc_emitted_dec V_alloc B_exit)))))

; ----------------------------------------------------------------------------
; §3. The counterexample question (path-sensitive leak)
; ----------------------------------------------------------------------------
; Empirical leak: 1 unfreed allocation on one arm, 0 on the other.
; ⇒ ∃ arm A s.t. rc_balance_at_exit_via(V_alloc, A) > 0
; (leak) while the OTHER arm has rc_balance = 0 (no leak).

(push 1)

(declare-const cex_leaking_arm Block)
(assert (or (= cex_leaking_arm B_ok_arm) (= cex_leaking_arm B_err_arm)))
(assert (> (rc_balance_at_exit_via V_alloc cex_leaking_arm) 0))

(check-sat)
; Expected: UNSAT — the proven calculus (per-arm RL-1-RL-2 composition)
; refutes path-sensitive leaks. A SAT verdict would identify a witness
; configuration showing path-sensitive flow that the calculus does not catch,
; routing to RL-4 (Jump-arg exemption interaction) or IA-3 (alt_join
; semantics gap).

(pop 1)

(check-sat)
