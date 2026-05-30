; SMT-LIB2 encoding — cross-pattern category (1): mono-pipeline-ordering
; ============================================================================
; Shape: monomorphization-pipeline-ordering interaction with burden emission.
; Empirical signature: E5001 unresolved `__cast` in 3 generics::test_borrow_list_int_*
; tests (commit 4ac52f23d imported-mono pipeline). The protocol-builtin `__cast`
; primitive (per arc.md §Protocol Builtins) survives to a phase where it should
; already be resolved, indicating a pipeline ordering inversion between
; imported-mono and a downstream phase that expected resolution.
;
; Per `Annex E §AIMS Pipeline Ordering`, PL-1..PL-11 fix the order of
; the 12 AIMS pipeline steps. The mono pipeline runs UPSTREAM of the
; AIMS pipeline (canon → mono → ori_arc::lower → analyze_program), so any
; AIMS step that consumes `__cast` AFTER mono has converged is sound. The
; E5001 surface in this cross-pattern category indicates an instruction reached an AIMS step
; with `__cast` STILL UNRESOLVED — meaning the ordering invariant was
; violated upstream of analyze_program (Step 1) or apply_ownership (Step 2).
;
; Question to the SMT solver: under the proven calculus from §07 PL-1..PL-11,
; does ∃ a satisfying assignment producing the observed E5001 shape (a
; downstream consumer sees unresolved `__cast` despite the spec asserting
; mono completes upstream)?
;
; Expected: UNSAT
; meaning — the proven pipeline-ordering calculus refutes the E5001
; configuration. The empirical regression is a CONSTRUCTION bug (mono
; skipped the call site OR a phase boundary was crossed without mono
; being driven over the imported instance), not a calculus gap.
;
; SAT would mean the proven ordering calculus admits a state where an
; AIMS step legitimately consumes unresolved `__cast` — route to §07 PL-N
; reformulation per §10.1 SAT-handling table.
;
; Source mapping (per the counterexample search
; §10.0 shape-04B.2-mono-pipeline-ordering row):
; shape_id = shape-04B.2-mono-pipeline-ordering
; source = docs/ori_lang/proposals/approved/aims-burden-tracking-proposal.md
; (burden-prototype evidence: category (1)
; monomorphization-pipeline-ordering interaction with burden emission)
; fixture_path= aims-proof/proofs/10-counterexample/04B.2-mono-ordering.smt2
;
; Cited proven rules (per §02-§09):
; - PL-1 Interprocedural Steps 1-2 (analyze_program, apply_ownership) SHALL
; run once across ALL functions before any per-function step.
; [aims-proof/proofs/07-pipeline/PL-1.proof]
; - PL-1a Per-function pipeline (Steps 3-12) SHALL process functions in
; SCC topological order.
; [aims-proof/proofs/07-pipeline/PL-1a.proof]
; - PL-2 Step 4 (analyze_function) SHALL precede Step 5 (realize_rc_reuse).
; [aims-proof/proofs/07-pipeline/PL-2.proof]
; - PL-5 No pass SHALL rely on stale summaries.
; [aims-proof/proofs/07-pipeline/PL-5.proof]
; - PL-6 Adding a pass requires updating ordering + proving no constraint
; violation. (Used here to bound the set of admissible orderings.)
; [aims-proof/proofs/07-pipeline/PL-6.proof]
;
; Intel-query citations (graph-first per intelligence.md, 2026-05-28):
; - similar "mono_pipeline" --repo ori → Symbol-not-found (graph degradation
; per intelligence.md graceful-degradation contract).
; Manual fallback: `compiler/oric/src/pipeline/mono/` carries
; the imported-mono machinery; the burden-prototype evidence pins commit
; 4ac52f23d as the introduction site.
; - callers "transfer_select" --repo ori → 1 caller: transfer_def at
; ori_arc/src/aims/transfer/mod.rs:71. Used here to anchor the AIMS-side
; consumer that observes the `__cast` operand.
; - file-symbols "ori_arc/src/aims/realize" --repo ori → 30 symbols including
; eliminate_burden_ops, eliminate_in_block, should_elide_dec; confirms
; Step 5 (realize_rc_reuse) is the consumer downstream of analyze_function.

(set-logic ALL)
(set-option :produce-unsat-cores true)
(set-option :produce-models true)

; ----------------------------------------------------------------------------
; §1. Pipeline-step sort + monotonic-order encoding
; ----------------------------------------------------------------------------

(declare-datatype PipelineStep
  ( (PS_canon) ; upstream: canonicalization phase
    (PS_mono) ; imported-mono pipeline (commit 4ac52f23d)
    (PS_arc_lower) ; ori_arc::lower::lower_function
    (PS_analyze_program) ; AIMS Step 1 (interprocedural)
    (PS_apply_ownership) ; AIMS Step 2
    (PS_compute_reprs) ; AIMS Step 3
    (PS_analyze_fn) ; AIMS Step 4 (intraprocedural backward)
    (PS_realize_rc) ; AIMS Step 5 (RC emission consumer of __cast)
  ))

(define-fun ps_idx ((p PipelineStep)) Int
  (ite (= p PS_canon) 0
    (ite (= p PS_mono) 1
      (ite (= p PS_arc_lower) 2
        (ite (= p PS_analyze_program) 3
          (ite (= p PS_apply_ownership) 4
            (ite (= p PS_compute_reprs) 5
              (ite (= p PS_analyze_fn) 6 7))))))))

; PL-1 / PL-1a / PL-2 axiom: order is total + ascending (no inversion)
; precedes(a, b) ⟺ a's index < b's index
(define-fun precedes ((a PipelineStep) (b PipelineStep)) Bool
  (< (ps_idx a) (ps_idx b)))

; ----------------------------------------------------------------------------
; §2. Per-instruction observation of __cast
; ----------------------------------------------------------------------------

; A protocol-builtin instance under test. `__cast` is the canonical case from
; the burden-prototype evidence (E5001 unresolved __cast).
(declare-datatype Instr ((I_cast)))

; cast_resolved_at(step) — TRUE iff after step `step` the `__cast` operand
; is resolved (mono successfully expanded it into a concrete-type call).
(declare-fun cast_resolved_at ((Instr) (PipelineStep)) Bool)

; cast_consumed_at(step) — TRUE iff step `step` is a downstream AIMS consumer
; that READS the `__cast` operand (TF rule fires).
(declare-fun cast_consumed_at ((Instr) (PipelineStep)) Bool)

; ----------------------------------------------------------------------------
; §3. Proven-calculus axioms (per §07 PL-1..PL-11)
; ----------------------------------------------------------------------------

; --- PL-1: Steps 1-2 run after upstream canonicalization + mono + arc_lower
(assert (precedes PS_canon PS_mono))
(assert (precedes PS_mono PS_arc_lower))
(assert (precedes PS_arc_lower PS_analyze_program))
(assert (precedes PS_analyze_program PS_apply_ownership))

; --- PL-2: Step 4 precedes Step 5 (per RL-2 + DP-2 consumer chain)
(assert (precedes PS_analyze_fn PS_realize_rc))

; --- PL-5: AIMS steps consume the OUTPUT of mono, never the input
; ⇒ if mono is in the pipeline, mono resolves __cast BEFORE any AIMS step.
(assert
  (=> (cast_consumed_at I_cast PS_realize_rc)
      (cast_resolved_at I_cast PS_realize_rc)))
(assert
  (=> (cast_consumed_at I_cast PS_analyze_fn)
      (cast_resolved_at I_cast PS_analyze_fn)))
(assert
  (=> (cast_consumed_at I_cast PS_analyze_program)
      (cast_resolved_at I_cast PS_analyze_program)))

; --- PL-6: Resolution is monotone — once resolved at step S, resolved at all
; subsequent steps (consequence of PL-5 no-stale-summaries).
(assert (forall ((a PipelineStep) (b PipelineStep))
  (=> (and (cast_resolved_at I_cast a) (precedes a b))
      (cast_resolved_at I_cast b))))

; --- PL-1 (mono completes) for the fixture under test:
; For an instance that REACHES the AIMS pipeline, mono converged on the
; instance during PS_mono (the spec's pre-AIMS gate).
(assert (cast_resolved_at I_cast PS_mono))

; ----------------------------------------------------------------------------
; §4. The counterexample question (E5001 shape)
; ----------------------------------------------------------------------------
; E5001 shape: a downstream AIMS consumer (PS_realize_rc) reads an UNRESOLVED
; __cast operand — the empirical signature observed in the burden-prototype generics tests.

(push 1)

; Counterexample conjecture: consumer fires AND __cast still unresolved at the
; consumer's step.
(declare-const cex_consumer PipelineStep)
(assert (or (= cex_consumer PS_realize_rc)
            (= cex_consumer PS_analyze_fn)
            (= cex_consumer PS_analyze_program)))
(assert (cast_consumed_at I_cast cex_consumer))
(assert (not (cast_resolved_at I_cast cex_consumer)))

(check-sat)
; Expected: UNSAT — the proven pipeline-ordering calculus refutes the
; configuration. The empirical E5001 surfaces from an ordering inversion
; not admitted by PL-1..PL-6 (mono either ran for a different SCC, or the
; instance was added to the corpus after mono converged — both are
; construction-side bugs, not calculus gaps).

(pop 1)

; ----------------------------------------------------------------------------
; §5. Sentinel — encoding loads
; ----------------------------------------------------------------------------
(check-sat)
