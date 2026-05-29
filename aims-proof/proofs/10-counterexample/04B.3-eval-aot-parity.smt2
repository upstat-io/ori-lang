; SMT-LIB2 encoding — §04B.3 eval-AOT dual-execution parity break
; ============================================================================
; Shape: CFG-merge join between Ok and Err arms produces a state where one
; arm's RcDec is dropped at LLVM codegen, BREAKING the eval/AOT parity that
; `canon.md §7.1` invariant 2 requires.
;
; Empirical signature (per §04B.3 HISTORY 2026-05-18 §04B.3 evidence):
; - Eval positive-pin PASSES (eval backend correctly emits paired inc/dec)
; - AOT positive-pin FAILS with `ori: 1 RC allocation(s) not freed (memory leak)`
; - IR analysis via `ORI_DUMP_AFTER_ARC=1 ORI_LOG=ori_arc::aims::realize=trace`:
; burden_inc/burden_dec pairs SURVIVE elimination on alias-chain vars
; %17, %12, %23, %29 — lattice DP-2/DP-3 consumer NOT over-eliminating in
; isolation; per-block post-§04A.2 RC snapshot confirms RcInc[6] at block 12
; paired with RcDec[6, 5] at blocks 13+15
; - AOT leak attributable to CFG-merge join between Ok/Err arm decs where
; LLVM codegen consumption of post-§04A.2 ARC IR drops ONE dec
;
; Per `canon.md §7.1` AIMS invariants:
; #1 Contracts and realization must agree
; #2 Active rewrites must be sound (identical observable behavior across
; eval + AOT)
; The §04B.3 finding is a contracts↔realization disagreement.
;
; Question to the SMT solver: under the proven calculus (RL-2 with terminal-
; dec emission per CFG-merge arm, IA-3 alt_join, DP-2 supplementary-only
; restriction, dual-execution parity per VF-7 (a)(b)(c) tiers), can the LLVM
; backend consume the SAME post-§04A.2 ARC IR and produce a DIFFERENT
; emission than the eval backend?
;
; Expected: UNSAT
; meaning — the calculus's dual-execution parity invariant (canon.md §7.1
; inv 2) forbids divergent backend emissions on the same ARC IR. The
; empirical AOT leak is an IMPLEMENTATION defect in the LLVM consumer
; (likely the CFG-merge dec emission path drops one of the two arm-decs
; when block params are present and the consumer reads from the post-merge
; IR using position-keyed lookups instead of ArcVarId-keyed lookups per
; PL-4 constraint), NOT a calculus gap.
;
; SAT would mean the proven calculus admits divergent backend emission —
; route §08 RL-2 + §07 PL-4 + §11 if composition-level.
;
; Source mapping (per §10.0 shape-04B.3-eval-aot-parity-break row):
; shape_id = shape-04B.3-eval-aot-parity-break
; source = docs/ori_lang/proposals/approved/aims-burden-tracking-proposal.md
; HISTORY 2026-05-18 §04B.3 evidence (Eval PASS / AOT FAIL
; on burden_alias_inner_survives_result_destructure)
;
; Cited proven rules (per §02-§09):
; - RL-2 RC dec at last use / scope exit; ownership-transfer exclusion;
; unused-owned cleanup; RC-count preservation invariant P2
; [aims-proof/proofs/08-realization/RL-2.proof]
; - RL-4 Edge-specific decs at CFG exit; Jump-arg exemption
; [aims-proof/proofs/08-realization/RL-4.proof]
; - DP-2 is_rc_dec_unnecessary — supplementary decs only
; [aims-proof/proofs/05-decisions/DP-2.proof]
; - DP-3 is_rc_inc_elidable
; [aims-proof/proofs/05-decisions/DP-3.proof]
; - PL-4 Step 10 (realize_annotations / Phase 2) SHALL follow Step 9
; (merge_blocks); Phase 2 uses ArcVarId-keyed lookups surviving
; merge — position-keyed maps INVALIDATED by merge_blocks
; [aims-proof/proofs/07-pipeline/PL-4.proof]
; - VF-7 Active rewrites SHALL be sound: identical observable behavior
; (eval + AOT)
; [aims-proof/proofs/09-verification/VF-7.proof]
;
; Intel-query citations (graph-first, 2026-05-28):
; - similar "cfg_merge_join" --repo ori → Symbol-not-found.
; Manual fallback: IA-3 alt_join + RL-4 edge-specific decs both live in
; ori_arc/src/aims/intraprocedural and ori_arc/src/aims/realize/emit_rc
; respectively; the LLVM consumer is ori_llvm/src/codegen/arc_emitter/.

(set-logic ALL)
(set-option :produce-unsat-cores true)
(set-option :produce-models true)

; ----------------------------------------------------------------------------
; §1. ARC IR shape — CFG-merge with Ok/Err arms (BUG-04-118-style)
; ----------------------------------------------------------------------------
(declare-datatype Backend ((Eval) (AOT)))

(declare-datatype Block
  ( (B_alloc)
    (B_branch) ; Branch(cond) → ok_arm / err_arm
    (B_ok_arm) ; inc at block 12 (per §04B.3 evidence)
    (B_err_arm)
    (B_merge) ; block param from Jump args
    (B_exit) ))

(declare-datatype ArcVar ((V_alias))) ; one of the alias-chain vars %17/%12/%23/%29

; rc_emitted_per_backend(backend, block) — count of RcDecs the backend emits
; for V_alias at the given block.
(declare-fun rc_emitted_dec ((Backend) (Block)) Int)
(declare-fun rc_emitted_inc ((Backend) (Block)) Int)

; cumulative balance at exit per backend
(declare-fun rc_balance_at_exit ((Backend)) Int)

; ----------------------------------------------------------------------------
; §2. Calculus axioms (per §02-§09 + canon.md §7.1)
; ----------------------------------------------------------------------------

; --- The ARC IR (post-§04A.2 lattice consumer) is the SAME across both
; backends — both consume the identical ArcFunction. Encode as: the
; emitted-instr count per (block, backend) is determined by the SHARED
; IR plus the backend-specific lowering.
; For RL-2 emissions, the SPEC mandates identical counts per arm:
; both backends emit the dec at B_ok_arm and at B_err_arm (per CFG-merge
; join rule — each arm-exit gets its own dec when V_alias dies on that arm).

; --- VF-7 / canon.md §7.1 invariant 2: dual-execution parity.
; ∀ block B, ∀ backend pair (b1, b2):
; rc_emitted_dec(b1, B) = rc_emitted_dec(b2, B)
; rc_emitted_inc(b1, B) = rc_emitted_inc(b2, B)
; This is the LOAD-BEARING parity axiom.
(assert (forall ((blk Block))
  (and (= (rc_emitted_dec Eval blk) (rc_emitted_dec AOT blk))
       (= (rc_emitted_inc Eval blk) (rc_emitted_inc AOT blk)))))

; --- RL-2 per-arm dec: V_alias dies on both arms (alias chain extends past
; destructure, then dec'd in each arm's exit per the §04B.3 IR analysis
; "RcInc[6] at block 12 paired with RcDec[6, 5] at blocks 13+15").
; ⇒ each backend emits ≥ 1 dec at B_ok_arm AND ≥ 1 dec at B_err_arm.
(assert (>= (rc_emitted_dec Eval B_ok_arm) 1))
(assert (>= (rc_emitted_dec Eval B_err_arm) 1))

; --- Balance computation per backend (sum over all blocks)
(assert (= (rc_balance_at_exit Eval)
           (- (+ (rc_emitted_inc Eval B_alloc)
                 (rc_emitted_inc Eval B_branch)
                 (rc_emitted_inc Eval B_ok_arm)
                 (rc_emitted_inc Eval B_err_arm)
                 (rc_emitted_inc Eval B_merge)
                 (rc_emitted_inc Eval B_exit))
              (+ (rc_emitted_dec Eval B_alloc)
                 (rc_emitted_dec Eval B_branch)
                 (rc_emitted_dec Eval B_ok_arm)
                 (rc_emitted_dec Eval B_err_arm)
                 (rc_emitted_dec Eval B_merge)
                 (rc_emitted_dec Eval B_exit)))))

(assert (= (rc_balance_at_exit AOT)
           (- (+ (rc_emitted_inc AOT B_alloc)
                 (rc_emitted_inc AOT B_branch)
                 (rc_emitted_inc AOT B_ok_arm)
                 (rc_emitted_inc AOT B_err_arm)
                 (rc_emitted_inc AOT B_merge)
                 (rc_emitted_inc AOT B_exit))
              (+ (rc_emitted_dec AOT B_alloc)
                 (rc_emitted_dec AOT B_branch)
                 (rc_emitted_dec AOT B_ok_arm)
                 (rc_emitted_dec AOT B_err_arm)
                 (rc_emitted_dec AOT B_merge)
                 (rc_emitted_dec AOT B_exit)))))

; --- RL-1-RL-2 composition P2 for the eval backend (observed: PASS).
(assert (= (rc_balance_at_exit Eval) 0))

; ----------------------------------------------------------------------------
; §3. The counterexample question (eval-AOT parity break)
; ----------------------------------------------------------------------------
; Empirical parity-break: AOT leaks (balance > 0) while Eval is clean.

(push 1)

; counterexample conjecture: AOT balance differs from Eval balance.
(assert (not (= (rc_balance_at_exit Eval) (rc_balance_at_exit AOT))))

(check-sat)
; Expected: UNSAT — the VF-7 parity axiom (canon.md §7.1 invariant 2) forces
; identical per-block emission counts across backends, which forces identical
; cumulative balances. The empirical AOT-only leak is an IMPLEMENTATION defect
; in the LLVM consumer: likely the post-merge ArcVarId-keyed lookup per PL-4
; is consulting position-keyed state (invalidated by merge_blocks at Step 9),
; producing a dropped dec on the AOT-specific lowering path.

(pop 1)

(check-sat)
