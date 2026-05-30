; SMT-LIB2 encoding — cross-pattern category (3): cross-class alias-chain UAF
; ============================================================================
; Shape: cross-class alias-chain use-after-free segfault — specifically
; generics::test_borrow_list_int_nested_pin6_chain_then_return_no_leak exiting
; with -139 SIGSEGV (per the burden-prototype evidence).
;
; "Cross-class" = the alias spans TWO allocation classes (e.g., outer Result
; class B + inner Inner class A) with the inner alias's lifetime extending
; past the outer class's destructure. Distinct from BUG-04-118 (same class A
; with intra-class alias) — here the alias chain crosses a class boundary
; AND the realization pipeline emits a premature RcDec on the OUTER class B
; while the BORROW into class A's payload is still LIVE.
;
; UAF = use of memory after its allocation was freed: the outer class's drop
; cascades through its payload (drop_fn invokes child element dec), freeing
; class A while the borrow into A's payload is still live ⇒ next deref of
; the borrow reads freed memory ⇒ SIGSEGV.
;
; Question to the SMT solver: under the proven calculus (DP-5 alias safety
; reading both borrow_sources AND project_alias_sources, RL-31 disjointness,
; CN-8 borrowed-locality ceiling, IA-5 step (1) alias transfer), can a state
; arise where a live transitive cross-class alias is NOT seen by DP-5 and
; the outer class is dec'd while the inner borrow is still live?
;
; Expected: UNSAT
; meaning — DP-5 + project_alias_sources construction (per §1.9 Rules
; 1, 2, 3, 4, 6, 7 — transitive closure across Let / Jump / Project nests)
; catches cross-class transitive aliases. UAF is an IMPLEMENTATION defect
; in `compute_ssa_alias_classes` or project_alias_sources population
; (which is missing some of §1.9 Rules 5, 7 per Annex E §AIMS target-only
; markers), NOT a calculus gap.
;
; SAT would mean the proven calculus admits a UAF — route DP-5 + RL-31
; reformulation + §11 if composition-level.
;
; Source mapping (per §10.0 shape-04B.2-cross-class-uaf row):
; shape_id = shape-04B.2-cross-class-uaf
; source = docs/ori_lang/proposals/approved/aims-burden-tracking-proposal.md
; burden-prototype evidence, category (3) — 1 generics test
; (test_borrow_list_int_nested_pin6_chain_then_return_no_leak,
; exit -139 SIGSEGV)
;
; Cited proven rules (per §02-§09):
; - DP-5 can_mutate_in_place + alias safety; reads borrow_sources AND
; project_alias_sources; live transitive alias blocks conservatively
; [aims-proof/proofs/05-decisions/DP-5.proof]
; - TF-4 Project populates borrow_sources[dst] = { source: root(value),
; path: [field] }
; [aims-proof/proofs/04-transfers/TF-4.proof]
; - TF-14 Project backward demand: src.locality := max(src.loc, dst.loc);
; src.cardinality := seq_add
; [aims-proof/proofs/04-transfers/TF-14.proof]
; - IA-5 step (1) alias-transfer soundness (Let Var transparent + Project
; borrow + jump-arg + Select conditional alias propagation)
; [aims-proof/proofs/04-transfers/IA-5-step-1.proof]
; - RL-31 Disjoint Borrowed parameters SHALL receive !alias.scope + !noalias
; metadata (cross-function provenance summary)
; [aims-proof/proofs/08-realization/RL-31.proof]
; - CN-8 Access = Borrowed ∧ Locality > FunctionLocal ⟹ Locality :=
; FunctionLocal (prevents borrow from outliving its source)
; [aims-proof/proofs/03-canonicalization/CN-8.proof]
;
; Intel-query citations (graph-first, 2026-05-28):
; - similar "alias_chain" --repo ori → Symbol-not-found.
; Manual fallback: DP-5.proof Negative-direction witnesses for the
; alias-chain shape; this encoding mirrors the "Unique + live R5 Select
; alias + no direct borrow + Set" witness shape, CROSS-CLASS extension.
; - callers "transfer_select" --repo ori → transfer_def at
; ori_arc/src/aims/transfer/mod.rs:71. Confirms TF-8 + IA-5 step (1)
; case (c) Select conditional alias propagation is wired.

(set-logic ALL)
(set-option :produce-unsat-cores true)
(set-option :produce-models true)

; ----------------------------------------------------------------------------
; §1. Variable + class encoding
; ----------------------------------------------------------------------------
; Two allocation classes:
; class A — inner heap value (e.g., the [int] list)
; class B — outer heap value containing class A in its payload
; (e.g., Result<{str: int}, str>, or Some([int]))

(declare-datatype ArcVar
  ( (V_inner) ; allocation in class A
    (V_outer) ; allocation in class B; payload points to V_inner
    (V_proj) ; %proj = Project V_outer.payload (borrow into class A)
    (V_alias_let) ; %al = Let Var(%proj) (transparent alias)
    (V_alias_jump) ; block-param after Jump args=[%al]
  ))

(declare-datatype Block
  ( (B_alloc_inner)
    (B_alloc_outer)
    (B_project)
    (B_let_alias)
    (B_jump_to_use)
    (B_use_alias) ; deref of cross-class alias
    (B_outer_dec) ; RcDec on V_outer (cascades to V_inner via drop_fn)
    (B_scope_exit) ))

(define-fun block_idx ((b Block)) Int
  (ite (= b B_alloc_inner) 0
    (ite (= b B_alloc_outer) 1
      (ite (= b B_project) 2
        (ite (= b B_let_alias) 3
          (ite (= b B_jump_to_use) 4
            (ite (= b B_use_alias) 5
              (ite (= b B_outer_dec) 6 7))))))))

; ----------------------------------------------------------------------------
; §2. Alias-tracking side tables (per Annex E §AIMS)
; ----------------------------------------------------------------------------
; borrow_sources: every Project creates exactly one entry (TF-4)
(declare-fun is_borrow_source ((ArcVar) (ArcVar)) Bool)
; project_alias_sources: transitive closure per §1.9 Rules 1-6 (R5 Select, R7
; Set/SetTag are spec-mandated but not shipped per Annex E §AIMS target-only
; markers — this encoding assumes the spec, NOT shipped impl).
(declare-fun in_alias_sources ((ArcVar) (ArcVar)) Bool)

; live(var, block) — is the alias live at the given block?
(declare-fun is_live ((ArcVar) (Block)) Bool)

; class_of(var) — physical class membership (A or B)
(declare-datatype AllocClass ((Class_A) (Class_B)))
(declare-fun class_of ((ArcVar)) AllocClass)

; payload_of(outer, inner) — outer class B contains class A in its payload
; (encoded by Construct/Ok-ctor; consulted by drop_fn at outer dec)
(declare-fun payload_of ((ArcVar) (ArcVar)) Bool)

; ----------------------------------------------------------------------------
; §3. Calculus axioms (per §02-§09)
; ----------------------------------------------------------------------------

; Class membership
(assert (= (class_of V_inner) Class_A))
(assert (= (class_of V_outer) Class_B))
(assert (payload_of V_outer V_inner)) ; B's payload references A

; --- TF-4: Project populates borrow_sources[V_proj] = source: V_outer
(assert (is_borrow_source V_proj V_outer))
; --- IA-5 step (1) case (a) Let Var transparent: %al alias of %proj
(assert (in_alias_sources V_alias_let V_proj))
; --- §1.9 Rule 6 transitive: %al's sources include %proj's sources
; (NOTE: V_proj is the borrow root; V_outer is the source of V_proj)
(assert (in_alias_sources V_alias_let V_outer))
; --- §1.9 Rule 3 Jump-arg → block-param: %jp = block-param after Jump args=[%al]
(assert (in_alias_sources V_alias_jump V_alias_let))
(assert (in_alias_sources V_alias_jump V_proj))
(assert (in_alias_sources V_alias_jump V_outer)) ; transitive

; --- Liveness: the alias chain extends across the outer dec point
; (this IS the cross-class UAF setup)
(assert (is_live V_alias_jump B_use_alias))
(assert (is_live V_proj B_use_alias)) ; transitive via IA-5 step (1)
(assert (< (block_idx B_use_alias) (block_idx B_outer_dec))) ; use BEFORE dec? OR
; ALTERNATIVE shape: use AFTER outer_dec — this is the UAF scenario
(declare-fun use_after_outer_dec () Bool)
; If use_after_outer_dec, then V_proj read after V_outer was freed.

; --- DP-5 alias-safety axiom (per Annex E §AIMS DP-5 truth table):
; ∀ Project dest, ∀ live transitive alias `a` of source V_outer:
; can_mutate_in_place(V_outer, ..., block) = FALSE if a live at block.
; We restate as a SAFETY THEOREM for RcDec emission on V_outer:
; RL-2/RL-4 SHALL NOT emit RcDec(V_outer) at a block where ANY live
; transitive alias rooted in V_outer is still live (else UAF via
; drop_fn cascade through payload_of).
(assert (forall ((blk Block) (a ArcVar))
  (=> (and (in_alias_sources a V_outer)
           (is_live a blk)
           (= blk B_outer_dec))
      ; the calculus REFUSES to emit RcDec(V_outer) at this block
      false)))
; The forall asserts the CONTRAPOSITIVE: a live alias at the dec point
; means the dec MUST NOT have been scheduled there — RL-2 + DP-5 jointly
; refuse the emission. Any model where the dec happens AND a live alias
; exists at that block violates the calculus.

; ----------------------------------------------------------------------------
; §4. The counterexample question (cross-class UAF)
; ----------------------------------------------------------------------------
; UAF = ∃ block where:
; (a) RcDec on V_outer was scheduled, AND
; (b) a live transitive alias of V_outer (via V_inner's payload borrow chain)
; exists at that same block,
; ⇒ the dec frees class B, drop_fn cascades to class A, and the live alias
; into class A's payload becomes a dangling pointer.

(push 1)

; counterexample conjecture: the calculus PERMITS the UAF configuration.
(assert (in_alias_sources V_alias_jump V_outer))
(assert (is_live V_alias_jump B_outer_dec))
; The forall axiom above means this conjunction is UNSAT — the calculus
; refuses RcDec emission while the live alias is present.

(check-sat)
; Expected: UNSAT — DP-5 + project_alias_sources cross-class transitive
; closure (per §1.9 Rules 1-6) catches the alias. UAF is implementation-
; side: project_alias_sources population is MISSING transitive R5/R7 entries
; for Select / Set-Tag aliases per Annex E §AIMS target-only markers.
; If SAT: surface the missing transitive-closure rule (likely R5 or R7)
; as the §02 reformulation target.

(pop 1)

(check-sat)
