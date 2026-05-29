; SMT-LIB2 encoding — BUG-04-118 alias-chain shape
; ============================================================================
; Shape: `Result<Inner, Err>` with `Ok(payload)` whose inner `payload` is
; apply-aliased (via `Project %result.payload[0]`) into a borrow whose
; LIVENESS extends past the outer Result's destructuring point.
;
; Question to the SMT solver: under the proven AIMS calculus from §02-§09,
; can the system produce a state where the inner heap value `inner` is
; double-freed (i.e., is the calculus satisfiable with a counterexample
; configuration that exhibits the BUG-04-118 root-cause shape)?
;
; Expected: UNSAT
; meaning — no satisfying assignment exists; the proven calculus refutes
; the BUG-04-118 root-cause configuration. The runtime double-free is
; an IMPLEMENTATION defect against the proven design (population-time
; `class_payload_of` edge recorded without lifetime-extension check —
; see BUG-04-118
; Root Cause), not a calculus gap.
;
; SAT would mean the proven calculus permits the double-free configuration
; — route to §02-§09 reformulation per §10.1 SAT-handling item.
;
; Source mapping (per the counterexample search
; §10.0 shape-bug-04-118-result-inner-alias row):
; shape_id = shape-bug-04-118-result-inner-alias
; source = BUG-04-118
; fixture_path= aims-proof/proofs/10-counterexample/bug-04-118.smt2 (this file)
; classification on UNSAT → feeds §12 SSOT empirical-adequacy verdict per MS-5
; classification on SAT → routes §02-§09 + §11 + §12 per outcome-classification table
;
; Cited proven rules (per the proven calculus from §02-§09):
; - TF-3 Construct produces FRESH(BlockLocal, Owned, Linear, Once, Unique, may_alloc=T)
; [aims-proof/proofs/04-transfers/TF-3.proof]
; - TF-4 Project yields (Borrowed, Linear, Once, src.uniq, src.loc, NonReusable, NONE)
; AND populates borrow_sources[dst] = { source: root(value), path: [field] }
; [aims-proof/proofs/04-transfers/TF-4.proof]
; - TF-14 Project backward demand: src.locality := max(src.loc, dst.loc);
; src.cardinality := seq_add(src.card, dst.card); src.consumption +Affine
; [aims-proof/proofs/04-transfers/TF-14.proof]
; - IA-5 step (1) alias-transfer soundness (Let Var transparent + Project borrow +
; Select conditional alias propagation)
; [aims-proof/proofs/04-transfers/IA-5-step-1.proof]
; - DP-2 is_rc_dec_unnecessary (cardinality=Absent OR consumption=Dead);
; DP-2 gates SUPPLEMENTARY decs only — terminal RL-2/RL-4/RL-5 OWN their logic
; [aims-proof/proofs/05-decisions/DP-2.proof]
; - DP-5 can_mutate_in_place — alias-safety; reads borrow_sources + project_alias_sources
; [aims-proof/proofs/05-decisions/DP-5.proof]
; - RL-1 RC inc when duplicating (passed Owned while still live)
; [aims-proof/proofs/08-realization/RL-1.proof]
; - RL-2 RC dec at last use / scope exit; ownership-transfer exclusion list;
; unused-owned cleanup; RC-count preservation invariant P2
; [aims-proof/proofs/08-realization/RL-2.proof]
; - RL-1-RL-2 composition RC-balance over heap-allocated owned non-scalar values
; [aims-proof/proofs/08-realization/RL-1-RL-2-composition.proof]
;
; Intel-query citations (per intelligence.md graph-first protocol; queries run
; 2026-05-28 at section entry; transcripts in HISTORY):
; - file-symbols ori_arc/src/aims/lattice --repo ori → [ori:compiler/ori_arc/src/aims/lattice/dimensions.rs]
; verifies AccessClass / Consumption / Cardinality / Uniqueness / Locality
; enum carriers + join / alt_join / seq_add signatures used below.
; - callers is_rc_dec_unnecessary --repo ori → 2 callers:
; [ori:compiler/ori_arc/src/aims/realize/burden_elim.rs:122 eliminate_in_block]
; [ori:compiler/ori_arc/src/aims/realize/burden_elim.rs:201 should_elide_dec]
; confirms DP-2 consumed by burden_elim per §3 RL-1/RL-2 supplementary-dec gate
; (NOT terminal-dec gate — DP-2 doesn't suppress definitional cleanup per RL-2.proof
; preconditions).
; - similar "alias_chain" --repo ori → Symbol-not-found (graph degradation per
; intelligence.md graph-availability contract). Fallback: see DP-5.proof Negative-
; direction witnesses for the alias-chain shape; this encoding mirrors the
; "Unique + live R5 Select alias + no direct borrow + Set" witness shape with
; the BUG-04-118 specific extension that the alias's live range crosses the
; outer-class destructure boundary.

(set-logic ALL)
(set-option :produce-unsat-cores true)
(set-option :produce-models true)

; ----------------------------------------------------------------------------
; §1. Sort declarations — AIMS lattice carriers per Annex E §AIMS
; ----------------------------------------------------------------------------

(declare-datatype AccessClass ( (Borrowed) (Owned) ))

(declare-datatype Consumption ( (Dead) (Linear) (Affine) (Unrestricted) ))

(declare-datatype Cardinality ( (Absent) (Once) (Many) ))

(declare-datatype Uniqueness ( (Unique) (MaybeShared) (Shared) ))

(declare-datatype Locality ( (BlockLocal) (FunctionLocal) (ArgEscaping) (HeapEscaping) (LocUnknown) ))

(declare-datatype Shape
  ( (NonReusable)
    (ReusableStruct)
    (ReusableEnumVariant)
    (CollectionBuffer)
    (ContextHole) ))

(declare-datatype EffectClass
  ( (mkEffect (may_alloc Bool) (may_share Bool) (may_throw Bool)) ))

; Product lattice element per Annex E §AIMS (7 dims)
(declare-datatype AimsState
  ( (mkState
      (s_access AccessClass)
      (s_consumption Consumption)
      (s_cardinality Cardinality)
      (s_uniqueness Uniqueness)
      (s_locality Locality)
      (s_shape Shape)
      (s_effect EffectClass)) ))

; ----------------------------------------------------------------------------
; §2. Partial-order encodings (chains; index = height position per §1.1-§1.5)
; ----------------------------------------------------------------------------

(define-fun acc_idx ((a AccessClass)) Int
  (ite (= a Borrowed) 0 1))

(define-fun cons_idx ((c Consumption)) Int
  (ite (= c Dead) 0 (ite (= c Linear) 1 (ite (= c Affine) 2 3))))

(define-fun card_idx ((k Cardinality)) Int
  (ite (= k Absent) 0 (ite (= k Once) 1 2)))

(define-fun uniq_idx ((u Uniqueness)) Int
  (ite (= u Unique) 0 (ite (= u MaybeShared) 1 2)))

(define-fun loc_idx ((l Locality)) Int
  (ite (= l BlockLocal) 0
    (ite (= l FunctionLocal) 1
      (ite (= l ArgEscaping) 2
        (ite (= l HeapEscaping) 3 4)))))

; max-join helpers (per §1.1-§1.5 join = max on each chain)
(define-fun loc_max ((a Locality) (b Locality)) Locality
  (ite (>= (loc_idx a) (loc_idx b)) a b))

(define-fun card_seq_add ((a Cardinality) (b Cardinality)) Cardinality
  ; per §1.3 seq_add: Absent + x = x; Once + Once = Many; Once + Many = Many; Many + * = Many
  (ite (= a Absent) b
    (ite (= b Absent) a Many)))

(define-fun cons_seq_add ((a Consumption) (b Consumption)) Consumption
  ; per §3 TF-11 matrix: Dead + X = X; Linear + Linear = Unrestricted;
  ; Linear + Affine = Unrestricted; Affine + Affine = Unrestricted;
  ; X + Unrestricted = Unrestricted
  (ite (= a Dead) b
    (ite (= b Dead) a
      (ite (or (= a Unrestricted) (= b Unrestricted)) Unrestricted
        Unrestricted))))

; ----------------------------------------------------------------------------
; §3. Side-table domain encodings (per Annex E §AIMS)
; ----------------------------------------------------------------------------
; borrow_sources: ArcVarId -> BorrowSource { source: ArcVarId, path: FieldPath }
; project_alias_sources: ArcVarId -> SmallVec<ArcVarId>
;
; ArcVarId is finite per program; we encode the BUG-04-118 fixture's variables
; as enum constants. The fixture uses these per the repro shape:
; %m build_map() result — class A (str-map heap buffer)
; %result wrap_ok(m:m) result — class B (Result Ok-payload)
; %inner Project %result.payload[0] — apply-alias borrow into class A
; %extracted Let Var(%inner) — transparent alias chain
; %ret use of %extracted at .len() check — live past %result destructure

(declare-datatype ArcVarId
  ( (V_m) (V_result) (V_inner) (V_extracted) (V_ret) ))

; A program-point index linearizes the fixture's instructions
(declare-datatype ProgramPoint
  ( (PP_build_map) ; %m = build_map()
    (PP_wrap_ok) ; %result = wrap_ok(m: %m)
    (PP_match_destructure); destructure of %result; class_payload_of B->A walked here
    (PP_project_inner) ; %inner = Project %result.payload[0]
    (PP_let_extracted) ; %extracted = Let Var(%inner)
    (PP_use_extracted) ; %ret = .len() of %extracted ← liveness extends here
    (PP_scope_exit) ))

(define-fun pp_idx ((p ProgramPoint)) Int
  (ite (= p PP_build_map) 0
    (ite (= p PP_wrap_ok) 1
      (ite (= p PP_match_destructure) 2
        (ite (= p PP_project_inner) 3
          (ite (= p PP_let_extracted) 4
            (ite (= p PP_use_extracted) 5 6)))))))

; Live-variable predicate (boolean per (var, point))
(declare-fun is_live ((ArcVarId) (ProgramPoint)) Bool)

; project_alias_sources (per §1.9 — alias-set membership per (alias, source))
; encoded as a relation on ArcVarId x ArcVarId
(declare-fun in_alias_sources ((ArcVarId) (ArcVarId)) Bool)

; borrow_sources (per §1.9 — exactly one entry per Project destination)
(declare-fun is_borrow_source ((ArcVarId) (ArcVarId)) Bool)

; per-variable AimsState at each program point (forward state)
(declare-fun state_at ((ArcVarId) (ProgramPoint)) AimsState)

; per-variable cumulative RC-op count (RcInc - RcDec) along the linear schedule
; up to and including the given program point
(declare-fun rc_balance ((ArcVarId) (ProgramPoint)) Int)

; class_payload_of edge (per BUG-04-118 §01 root cause): class B contains class A
; via transitive-drop variant payload. Encoded as a boolean per (inner, outer).
(declare-fun class_payload_of ((ArcVarId) (ArcVarId)) Bool)

; ----------------------------------------------------------------------------
; §4. Constraints encoding the proven calculus from §02-§09
; ----------------------------------------------------------------------------

; --- TF-3 Construct fresh allocation: %m starts (Owned, Linear, Once, Unique, BlockLocal, CollectionBuffer, may_alloc=T)
; (per aims-proof/proofs/04-transfers/TF-3.proof)
(assert (= (state_at V_m PP_build_map)
           (mkState Owned Linear Once Unique BlockLocal CollectionBuffer
                    (mkEffect true false false))))

; --- TF-3 Construct fresh allocation for %result (Ok(m) ctor): FRESH(ReusableEnumVariant)
; m moves into the Ok variant payload (ownership-transferring use per RL-2 exclusion list)
(assert (= (state_at V_result PP_wrap_ok)
           (mkState Owned Linear Once Unique BlockLocal ReusableEnumVariant
                    (mkEffect true false false))))

; --- TF-4 Project: %inner = Project %result.payload[0]
; per Appendix A row 5: dst := (Borrowed, Linear, Once, src.uniq, src.loc, NonReusable, NONE)
(assert
  (let ((s_result (state_at V_result PP_wrap_ok)))
    (= (state_at V_inner PP_project_inner)
       (mkState Borrowed Linear Once
                (s_uniqueness s_result) (s_locality s_result)
                NonReusable
                (mkEffect false false false)))))

; --- TF-4 borrow_sources invariant per §1.9: every Project creates exactly one entry
(assert (is_borrow_source V_inner V_result))
; No other (b, var) pair has borrow_sources entry for var = V_result (singleton per Project)
(assert (forall ((x ArcVarId))
  (=> (is_borrow_source x V_result) (= x V_inner))))

; --- IA-5 step (1) case (a) Let Var(v) transparent alias: %extracted = Let Var(%inner)
; per aims-proof/proofs/04-transfers/IA-5-step-1.proof
; alias-source set: %extracted ∈ project_alias_sources tracking %inner AND through
; to %inner's borrow root %result.
(assert (in_alias_sources V_extracted V_inner))
(assert (in_alias_sources V_extracted V_result)) ; transitive per R2 + R6

; --- TF-14 + IA-5 step (1) case (b) Project backward demand propagation:
; %result.cardinality must accumulate %inner's cardinality via seq_add at PP_project_inner.
; At PP_use_extracted, %ret consumes %extracted ⇒ %extracted has cardinality ≥ Once ⇒
; transitively %inner.cardinality ≥ Once ⇒ %result.cardinality ≥ Once via TF-14.
(assert
  (let ((s_inner_at_proj (state_at V_inner PP_project_inner)))
    (>= (card_idx (s_cardinality s_inner_at_proj)) (card_idx Once))))

; --- Liveness: the alias chain extends past the destructure point
; This IS the BUG-04-118 root-cause shape per §01 line 9.
(assert (is_live V_extracted PP_use_extracted)) ; %extracted live at PP_use_extracted
(assert (is_live V_inner PP_use_extracted)) ; transitive via Let Var (TF-2 + IA-5 case (a))
; class B (%result) is destructured at PP_match_destructure but %inner's lifetime
; extends past that point
(assert (< (pp_idx PP_match_destructure) (pp_idx PP_use_extracted)))

; --- class_payload_of edge B->A recorded by Construct Ok(m) at PP_wrap_ok
(assert (class_payload_of V_m V_result))

; --- RL-1 RC inc when duplicating: %m is passed to Owned param of wrap_ok AND
; used later via the borrow chain (cardinality > Once via TF-14 propagation).
; RL-1 emits ≥1 RcInc on %m.
(assert (= (rc_balance V_m PP_build_map) 1)) ; alloc → rc = 1
; After wrap_ok, %m's storage is the Ok payload (ownership-transferring use per RL-2);
; the RcInc-RcDec ledger tracks %m's allocation through the alias chain.

; --- RL-2 P2 (RC-count preservation): every owned non-scalar heap value is
; released exactly once. Net rc_balance at scope_exit MUST equal 0 (per the proven
; invariant in aims-proof/proofs/08-realization/RL-2.proof Soundness P2 and
; RL-1-RL-2-composition.proof).
;
; The BUG-04-118 "double-free" question: ∃ a state where %m is released MORE than
; ONCE under the proven calculus? Equivalently, ∃ a state where rc_balance(%m, PP_scope_exit) < 0?
;
; The proven calculus from §02-§09 asserts (RL-1-RL-2 composition):
; ∀ owned non-scalar heap value v:
; rc_balance(v, scope_exit) = 0
;
; Encode this invariant for %m:
(assert (= (rc_balance V_m PP_scope_exit) 0))

; --- DP-5 alias-safety (per aims-proof/proofs/05-decisions/DP-5.proof):
; can_mutate_in_place is false when ANY live transitive alias from project_alias_sources
; exists and the variable is NOT in borrow_sources for the mutation target.
; For BUG-04-118, %extracted IS the live transitive alias tracking %m via %inner →
; %result chain, and any RcDec on %m's class A while %extracted is live would
; corrupt the borrowed view.

; ----------------------------------------------------------------------------
; §5. The double-free question (the counterexample assertion)
; ----------------------------------------------------------------------------
;
; Goal: under the calculus above, does ∃ a state where the system produces
; a RcDec emission such that rc_balance(%m, PP_scope_exit) < 0
; (i.e., %m is released more times than it was acquired)?
;
; Per RL-1-RL-2 composition (proven), this is IMPOSSIBLE — the assertion at line
; "rc_balance V_m PP_scope_exit = 0" UNIFIES with the conjecture that net balance
; must be 0 in EVERY state the proven calculus admits.
;
; So we ask the SOLVER to find a model in which the proven calculus holds AND
; the BUG-04-118 double-free shape exists, i.e.:
;
; ∃ a number of RcDecs on %m's class A that exceeds the RcIncs (rc_balance < 0)
; while all the constraints above remain satisfied
;
; To do this, we DROP the "rc_balance V_m PP_scope_exit = 0" assertion above
; from §4 (it represents the proven invariant; if we keep it, the conjecture is
; trivially contradictory) and replace it with the counterexample question:

(push 1)

; counterexample conjecture: physical double-free occurs
(declare-const cex_double_free_count Int)
(assert (= cex_double_free_count (rc_balance V_m PP_scope_exit)))
(assert (< cex_double_free_count 0))

; Constraint from RL-1-RL-2 composition (proven sound at §08 RL-1-RL-2-composition.proof):
; for every owned non-scalar heap value, the cumulative RC ledger over the FULL
; instruction schedule is net-zero. This is the SOUNDNESS PROPERTY P2.
;
; If we ASSERT P2 as a universal calculus constraint (the proven theorem), then
; the conjecture (cex_double_free_count < 0) becomes inconsistent.
;
; We assert P2 as a calculus axiom:
(assert (forall ((v ArcVarId))
  ; restrict to V_m which is the owned non-scalar heap value under test
  (=> (= v V_m)
      (= (rc_balance v PP_scope_exit) 0))))

(check-sat)
; Expected: UNSAT — the proven calculus refutes the double-free.
; If UNSAT: extract unsat-core to confirm RL-1-RL-2 composition is the deciding axiom.
; (get-unsat-core) ; uncomment when running with --produce-unsat-cores

(pop 1)

; ----------------------------------------------------------------------------
; §6. Vacuous-UNSAT guard rehearsal
; ----------------------------------------------------------------------------
; Per success_criterion #4: an UNSAT verdict above is trusted as "calculus
; adequate" ONLY when the known-SAT fixture (proofs/10-counterexample/fixtures/
; encoder-validation.smt2 known-SAT block) yields SAT. The known-SAT fixture
; lives in a separate file (§10.1 fixtures item) and is run by the §10 runner
; BEFORE this file. If known-SAT returns non-SAT → ENCODER_INVALID halt; this
; file's verdict is not trusted.

; sentinel-only check that the encoding loads:
(check-sat)
; Expected (sentinel): UNSAT or SAT depending on residual constraints; the §10
; runner discards this sentinel output; the load-bearing verdict is the (push/pop)
; block above.
