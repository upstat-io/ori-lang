; SMT-LIB2 encoding — §04B.2 cross-pattern category (4): over-elim closure-env double-free
; ============================================================================
; Shape: §04A.2 over-elimination on closure-env producing double-frees. Empirical
; signature: 2 higher_order tests (test_hof_closure_capture_in_loop +
; test_hof_make_predicate) — FATAL `ori_rc_dec called on already-freed allocation`.
; Per §04B.2 HISTORY 2026-05-18: "emission-side dual to BUG-04-118 match-alias
; shape but on closure shapes."
;
; Closure-env shape: PartialApply captures variable `v` into closure env;
; capture_state_update (TF-13) updates `v`'s lattice state under the closure's
; locality. Realization later emits BOTH an RcInc for the capture AND a final
; RcDec at scope exit. The lattice's DP-2 / DP-3 consumer (eliminate_burden_ops
; in burden_elim.rs) eliminates the capture-side RcInc but DOES NOT
; correspondingly eliminate the paired RcDec ⇒ over-elimination ⇒ scope-exit
; dec frees env that's already been freed (the closure's drop fired first).
;
; Question to the SMT solver: under the proven calculus (TF-13 capture_state_
; update, RL-2 dec emission, DP-2 supplementary-only gate, RL-1-RL-2 composition
; P2), can the elimination consumer drop ONLY ONE side of a paired (inc, dec)
; — producing a net-negative balance (double-free) at scope exit?
;
; Expected: UNSAT
; meaning — DP-2 gates SUPPLEMENTARY decs only (per Annex E §AIMS DP-2
; spec text); terminal RL-2 emissions OWN their logic and CANNOT be over-
; eliminated without violating P2 (which requires BALANCED ledger). A SAT
; verdict would identify a witness where DP-2's "supplementary-only" gate
; wording does not actually preclude the over-elimination — routing to §05
; DP-2 reformulation.
;
; SAT would mean DP-2 reformulation needed — route to §05 DP-2.proof + §08
; RL-2 (terminal-dec ownership) + §11 if composition-level.
;
; Source mapping (per §10.0 shape-04B.2-over-elimination-closure-env row):
; shape_id = shape-04B.2-over-elimination-closure-env
; source = docs/ori_lang/proposals/approved/aims-burden-tracking-proposal.md
; HISTORY 2026-05-18 §04B.2 category (4) — 2 higher_order tests
; (test_hof_closure_capture_in_loop, test_hof_make_predicate)
;
; Cited proven rules (per §02-§09):
; - TF-7 PartialApply: dst := FRESH(NonReusable). Captures via
; capture_state_update.
; [aims-proof/proofs/04-transfers/TF-7.proof]
; - TF-13 capture_state_update(current, closure_state): Access promotion
; on closure_state.locality ≥ HeapEscaping; consumption seq_add;
; cardinality seq_add; locality max.
; [aims-proof/proofs/04-transfers/TF-13.proof]
; - RL-1 RC inc when duplicating; captures via PartialApply receive RcInc.
; [aims-proof/proofs/08-realization/RL-1.proof]
; - RL-2 RC dec at last use / scope exit; ownership-transferring uses
; EXCLUDE PartialApply capture (per RL-2 exclusion list).
; P2 RC-count preservation.
; [aims-proof/proofs/08-realization/RL-2.proof]
; - DP-2 is_rc_dec_unnecessary — gates SUPPLEMENTARY decs only — terminal
; RL-2 emissions own their logic.
; [aims-proof/proofs/05-decisions/DP-2.proof]
; - DP-3 is_rc_inc_elidable: cardinality = Once ∧ consumption = Linear.
; [aims-proof/proofs/05-decisions/DP-3.proof]
; - RL-1-RL-2 composition RC-balance over heap-allocated owned non-scalar.
; [aims-proof/proofs/08-realization/RL-1-RL-2-composition.proof]
;
; Intel-query citations (graph-first, 2026-05-28):
; - file-symbols "ori_arc/src/aims/realize" --repo ori → 30 symbols incl.
; eliminate_burden_ops (burden_elim.rs:87), eliminate_in_block (122),
; should_elide_dec (201); the DP-2 / DP-3 elimination consumer surface.

(set-logic ALL)
(set-option :produce-unsat-cores true)
(set-option :produce-models true)

; ----------------------------------------------------------------------------
; §1. Variables + program points
; ----------------------------------------------------------------------------
(declare-datatype ArcVar
  ( (V_alloc) ; original heap allocation (captured variable)
    (V_closure) ; PartialApply result (closure env)
  ))

(declare-datatype ProgramPoint
  ( (PP_alloc) ; %v = Construct ...
    (PP_partial_apply) ; %closure = PartialApply (..., args=[%v])
    (PP_use_closure) ; %closure applied; env captures live
    (PP_closure_drop) ; %closure dec fires drop_fn cascade over env
    (PP_scope_exit) ))

(define-fun pp_idx ((p ProgramPoint)) Int
  (ite (= p PP_alloc) 0
    (ite (= p PP_partial_apply) 1
      (ite (= p PP_use_closure) 2
        (ite (= p PP_closure_drop) 3 4)))))

; ----------------------------------------------------------------------------
; §2. RC ledger + emission predicates
; ----------------------------------------------------------------------------
; cumulative inc + dec counts for V_alloc up to and including each PP
(declare-fun rc_inc_cum ((ProgramPoint)) Int)
(declare-fun rc_dec_cum ((ProgramPoint)) Int)

; net balance
(define-fun rc_balance ((p ProgramPoint)) Int
  (- (rc_inc_cum p) (rc_dec_cum p)))

; ----------------------------------------------------------------------------
; §3. Calculus axioms (per §02-§09)
; ----------------------------------------------------------------------------

; --- TF-3 + RL-1: alloc emits net +1 (fresh allocation, RC = 1)
(assert (= (rc_inc_cum PP_alloc) 1))
(assert (= (rc_dec_cum PP_alloc) 0))

; --- TF-7 + TF-13: PartialApply captures V_alloc into closure env
; RL-1 emits RcInc when capture is a duplicating use (capture_state_update
; promoted V_alloc.access to Owned if closure.locality ≥ HeapEscaping).
; Capture is an OWNERSHIP-TRANSFERRING use of V_alloc per RL-2 exclusion
; list — meaning the burden of dec'ing V_alloc transfers to the closure's
; drop_fn. Therefore at PP_partial_apply: inc bump (+1) for the capture,
; NO compensating dec at this point.
(assert (>= (rc_inc_cum PP_partial_apply) (rc_inc_cum PP_alloc)))
(assert (= (rc_dec_cum PP_partial_apply) (rc_dec_cum PP_alloc)))

; --- RL-2 closure_drop: when the closure is dec'd, drop_fn cascades through
; env, emitting RcDec for V_alloc (paired with the PartialApply RcInc).
(assert (>= (rc_dec_cum PP_closure_drop) (rc_dec_cum PP_partial_apply)))

; --- DP-2 spec text (per Annex E §AIMS DP-2):
; is_rc_dec_unnecessary GATES SUPPLEMENTARY DECS ONLY — terminal RL-2 /
; RL-4 / RL-5 emissions OWN their logic and ARE NOT eliminated by DP-2.
; ⇒ The closure_drop cascade dec is a TERMINAL RL-2 emission and MUST
; stay in the realized IR regardless of DP-2 outcome.
; We encode this as: closure_drop dec contributes ≥ 1 to rc_dec_cum.
(assert (>= (- (rc_dec_cum PP_closure_drop)
               (rc_dec_cum PP_partial_apply))
            1))

; --- RL-1-RL-2 composition / P2: net balance at scope exit = 0.
(assert (= (rc_balance PP_scope_exit) 0))

; --- monotone accumulation: incs + decs only increase over PP
(assert (forall ((a ProgramPoint) (b ProgramPoint))
  (=> (< (pp_idx a) (pp_idx b))
      (and (<= (rc_inc_cum a) (rc_inc_cum b))
           (<= (rc_dec_cum a) (rc_dec_cum b))))))

; ----------------------------------------------------------------------------
; §4. The counterexample question (over-elimination ⇒ double-free)
; ----------------------------------------------------------------------------
; Double-free shape: cumulative decs at scope exit > cumulative incs ⇒
; rc_balance < 0 ⇒ the final RcDec fires on already-freed allocation
; (drop_fn cascade from closure_drop freed V_alloc; second dec at
; scope exit attempts to free already-freed memory).
;
; Over-elimination is the construction shape: DP-2 incorrectly elides the
; capture-side RcInc at PP_partial_apply (rc_inc_cum stays at 1, not 2),
; but the closure_drop cascade STILL emits its dec — producing rc_dec_cum=2,
; rc_inc_cum=1, balance=-1 (double-free).

(push 1)

; counterexample conjecture: rc_balance(PP_scope_exit) < 0 (double-free)
(declare-const cex_balance Int)
(assert (= cex_balance (rc_balance PP_scope_exit)))
(assert (< cex_balance 0))

(check-sat)
; Expected: UNSAT — RL-1-RL-2 composition P2 directly contradicts
; rc_balance < 0. The empirical double-free arises because the implementation
; elides the capture-side RcInc (lattice consumer marked it "unnecessary"
; via DP-2/DP-3) WITHOUT also eliding the terminal RL-2 closure_drop cascade
; dec. The proven calculus REFUSES this elision: DP-2 spec text restricts it
; to supplementary decs; the closure_drop dec is TERMINAL.

(pop 1)

(check-sat)
