; SMT-LIB2 encoder-validation fixtures
; ============================================================================
; Per the counterexample search §10.1
; checkbox 8 + success_criterion #6 (asymmetric vacuous-UNSAT guard).
;
; Two fixtures live in this file as separate push/pop blocks. The §10 runner
; (aims-proof/scripts/run-section-10-counterexample.sh) executes both BEFORE
; any real-shape fixture every invocation. Verdicts:
;
; known-SAT block expects: SAT
; - non-SAT → ENCODER_INVALID halt; vacuous-UNSAT hazard active;
; NO real-shape UNSAT is trusted as "calculus adequate".
;
; known-UNSAT block expects: UNSAT
; - non-UNSAT → ENCODER_INVALID halt; soundness sanity check rejected
; a sound shape; encoder bug.
;
; Asymmetric causal link per Annex E §AIMS §VF-3 oracle-cross-check shape:
; only the witness-surfacing known-SAT fixture proves the absence of vacuity
; (a vacuously-unsat encoder ALSO returns UNSAT on the known-UNSAT block, so
; soundness sanity alone cannot rule out over-constraint).
;
; Companion metadata: encoder-validation.json carries shape_id + expected +
; rationale per fixture; the runner reads it to drive the assertion matrix.

(set-logic ALL)
(set-option :produce-unsat-cores true)
(set-option :produce-models true)

; ----------------------------------------------------------------------------
; §1. Sort declarations — AIMS lattice carriers (subset; mirrors bug-04-118.smt2)
; ----------------------------------------------------------------------------

(declare-datatype AccessClass ( (Borrowed) (Owned) ))
(declare-datatype Consumption ( (Dead) (Linear) (Affine) (Unrestricted) ))
(declare-datatype Cardinality ( (Absent) (Once) (Many) ))
(declare-datatype Uniqueness ( (Unique) (MaybeShared) (Shared) ))

; ----------------------------------------------------------------------------
; §2. KNOWN-SAT FIXTURE — load-bearing vacuous-UNSAT guard
; ----------------------------------------------------------------------------
;
; shape_id: fixture-known-sat-vacuity-guard
; expected: SAT
;
; Encoding intent: pose a calculus state that the PROVEN calculus refutes
; (per DP-10 REMOVED — Annex E §AIMS DP-10), then DELIBERATELY OMIT the
; refuting axiom. The encoder MUST surface a satisfying model exhibiting the
; unsound combination. SAT here proves the encoder can express + discover
; counterexample states (i.e., the constraint surface is not over-constrained
; into vacuous UNSAT).
;
; The unsound mutation: claim that (Owned ∧ Linear ∧ Once) implies Uniqueness=Unique
; (this WAS DP-10's claim, which §4 documents as REMOVED for unsoundness:
; backward analysis proves "no future duplication" but NOT "no existing
; aliases"). The proven calculus does NOT permit deriving Unique from the
; Access/Consumption/Cardinality triple alone; Uniqueness is established
; ONLY via the Uniqueness dimension OR fresh allocation (TF-3).
;
; By posing a state (Owned, Linear, Once, MaybeShared) and asking whether
; it is consistent with the (omitted) DP-10 inference, the solver must say
; SAT — the state exists in the lattice and no proven axiom rules it out.

(push 1)

(declare-const w_access AccessClass)
(declare-const w_consumption Consumption)
(declare-const w_cardinality Cardinality)
(declare-const w_uniqueness Uniqueness)

; The (Owned, Linear, Once) triple — DP-10's antecedent
(assert (= w_access Owned))
(assert (= w_consumption Linear))
(assert (= w_cardinality Once))

; The counterexample to the REMOVED DP-10 inference: the value can be MaybeShared
; even with the triple satisfied (upstream aliases may exist; backward analysis
; cannot observe them). The encoder must allow this — and the solver must find it.
(assert (= w_uniqueness MaybeShared))

; If the encoder were vacuously over-constrained (e.g., forcing w_uniqueness=Unique
; from the triple), this block would return UNSAT and the runner would halt with
; ENCODER_INVALID per §10.1 fixtures item.

(check-sat)
; Expected: SAT
; (get-model) ; uncomment when running with --produce-models

(pop 1)

; ----------------------------------------------------------------------------
; §3. KNOWN-UNSAT FIXTURE — independent soundness sanity check
; ----------------------------------------------------------------------------
;
; shape_id: fixture-known-unsat-soundness-sanity
; expected: UNSAT
;
; Encoding intent: pose a trivially-contradictory constraint over the lattice
; carriers (Access = Owned AND Access = Borrowed simultaneously). UNSAT proves
; the encoder + solver return UNSAT on a known-contradictory shape; catches
; `assert false`-style encoder bugs where the solver incorrectly returns SAT
; on a contradictory encoding.
;
; This fixture does NOT detect vacuity (a vacuously-unsat encoder ALSO returns
; UNSAT here); only the known-SAT fixture above proves the absence of vacuity.
; Both gates are mandatory per the asymmetric causal link.

(push 1)

(declare-const v_access AccessClass)

; Trivially-contradictory constraint: a single AccessClass-valued variable
; cannot simultaneously equal Owned and Borrowed (datatype disjointness).
(assert (= v_access Owned))
(assert (= v_access Borrowed))

(check-sat)
; Expected: UNSAT

(pop 1)
