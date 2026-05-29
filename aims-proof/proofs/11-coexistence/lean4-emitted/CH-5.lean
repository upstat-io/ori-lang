-- AIMS-Proof bootstrap translation
-- SSOT: aims-proof/checker/src/emit/lean4.rs (the SMT / Lean 4 emission strategy Option C)
-- Constructive-by-default per the foundational-axiom policy; classical escalation
-- requires matched commit per the foundational-axiom policy §Permitted Extensions.
-- BANNED: Classical.em, Classical.choice, funext, propext, proof
-- irrelevance, Markov's Principle — absent a matched extension entry
-- in the foundational-axiom policy.
-- Translated from proofs/11-coexistence/CH-5.proof

namespace AimsBootstrap

-- AimsState 7-tuple placeholder per Annex E §AIMS.1 through sec-1.7.
-- Each dimension is a finite constructive inductive carrier per
-- the foundational-axiom policy sec-Per-Engine-Constructive-Proof-Shape.

inductive AccessClass
 | Borrowed
 | Owned

inductive Consumption
 | Dead
 | Linear
 | Affine
 | Unrestricted

inductive Cardinality
 | Absent
 | One
 | Many

inductive Uniqueness
 | Unique
 | MaybeShared
 | Shared

inductive Locality
 | BlockLocal
 | FunctionLocal
 | ArgEscaping
 | HeapEscaping
 | Unknown

inductive Shape
 | NonReusable
 | ReusableStruct
 | ReusableEnumVariant
 | CollectionBuffer
 | ContextHole

structure EffectClass where
 may_alloc : Bool := false
 may_share : Bool := false
 may_throw : Bool := false

structure AimsState where
 access : AccessClass
 consumption : Consumption
 cardinality : Cardinality
 uniqueness : Uniqueness
 locality : Locality
 shape : Shape
 effect : EffectClass

-- Translated from proofs/11-coexistence/CH-5.proof:CH-5
-- Theorem name (verbatim from canonical-notation source):
-- Phase - ordering composition emdash PL - 1 interprocedural - first with burden - registry as Step 1 typed pre - pass
-- Preconditions (verbatim from canonical-notation source):
-- - depends - on CH - 4 ( AimsStateMap immutability under burden - registry mutation ) at aims - proof / proofs / 11 - coexistence / CH - 4 . proof emdash CH - 4 ' s acyclic BR - reads - L invariant is CH - 5 ' s pipeline - ordering precondition . CH - 5 reduces phase - ordering soundness to : ( a ) PL - 1 ' s interprocedural - first invariant preserved AND ( b ) burden - registry pre - pass sequenced AFTER L ' s IA - 7 convergence per CH - 4 .
-- - depends - on CH - 1 ( Burden - registry - lattice composition soundness ) at aims - proof / proofs / 11 - coexistence / CH - 1 . proof emdash CH - 1 ' s Part ( P2 ) no - double - counting + acyclic - dependency direction are inherited premises ( CH - 1 is the root per the sec - 11 . 1 Per - CH dependency chain ; CH - 5 transitively depends on CH - 1 via CH - 4 )
-- - PL - 1 ( Interprocedural - first ordering ) at aims - proof / proofs / 7 - pipeline / PL - 1 . proof emdash Steps 1 - 2 ( interprocedural : analyze_program + apply_ownership ) run once across all functions BEFORE any per - function step . Per - function pipeline ( Steps 3 - 12 ) processes functions in SCC topological order per PL - 1 a .
-- - Burden - registry pre - pass per the sec - 4 A . 2 design is a typed pre - pass input that lands on AimsStateMap per arc . md sec - Non - Negotiable - Invariants invariant 5 ( c ) emdash NOT a new lattice dimension ; NOT an independent RC emission path . Two viable insertion points per the sec - 4 A . 2 design : ( a ) Step 1 typed pre - pass : BEFORE analyze_program ( interprocedural - first ; per - variable burden derivation from sec - 1 - sec - 4 contract data ) ( b ) Step 4 - companion pre - pass : BETWEEN analyze_function ( Step 4 ) and realize_rc_reuse ( Step 5 ) ( per - function granularity ; the shipped insertion point per sec - 4 A success_criterion 1 emdash emit_burden_ops invoked between Step 4 and Step 5 ) CH - 5 discharges ( b ) emdash the shipped insertion . ( a ) is a target formulation per the sec - 11 success_criterion 12 depends - on sec - 7 PL - 1 . . PL - 11 binding ; both are sound under the constraints below .
-- - depends - on sec - 7 PL - 2 ( Step 4 precedes Step 5 ) at aims - proof / proofs / 7 - pipeline / PL - 2 . proof emdash analyze_function precedes realize_rc_reuse ; emit_burden_ops invoked between , consuming converged L
-- - depends - on sec - 7 PL - 5 ( no stale summaries ) at aims - proof / proofs / 7 - pipeline / PL - 5 . proof emdash burden - registry pre - pass outputs become typed inputs to subsequent pipeline steps without circular dependency
-- - depends - on sec - 7 PL - 6 ( adding - a - pass meta - rule ) at aims - proof / proofs / 7 - pipeline / PL - 6 . proof emdash adding a pass requires updating ordering + proving no constraint violation ; CH - 5 IS the constraint - no - violation proof for burden - registry pre - pass addition
-- - depends - on sec - 7 PL - 1 a ( SCC topological order ) at aims - proof / proofs / 7 - pipeline / PL - 1 a . proof emdash per - function pipeline processes functions in SCC topological order ; burden - registry pre - pass for F reads ONLY from F ' s call - graph predecessors ' computed values OR F ' s own pre - pass values
-- Soundness property (verbatim from canonical-notation source):
-- Forall function F . Forall pipeline ordering P with burden - registry pre - pass
-- inserted at the sec - 4 A . 2 - shipped Step 4 - companion position ( between Steps
-- 4 and 5 ) .
--   ( P1 ) PL - 1 interprocedural - first invariant preservation :
--     Forall function F ' . Forall pipeline step S in { 1 , 2 } .
--       S runs for F ' BEFORE Steps 3 - 12 run for F ' .
--     I . e . , interprocedural Steps 1 - 2 ( analyze_program , apply_ownership )
--     run once across ALL functions BEFORE any per - function step ( Steps
--     3 - 12 ) for ANY function . Inserting emit_burden_ops between Step 4
--     and Step 5 does NOT reorder interprocedural vs per - function phases .
--   ( P2 ) Acyclic BR - reads - L dependency :
--     Forall function F . Forall variable v in vars ( F ) .
--       BR ( F ) . burden_emitted [ v ] depends ONLY on :
--         ( i ) L ' s converged value L [ v ] for v in F ( read - only post - IA - 7
--              per CH - 4 ) , AND / OR
--         ( ii ) BR ' s pre - pass values for F ' s call - graph predecessors
--              ( read - only at F ' s processing time per SCC topological order )
--     BR ( F ) reads do NOT depend on F ' s not - yet - computed lattice values or
--     F ' s downstream per - function step outputs ( Steps 5 - 12 ) . No circular
--     dependency .
--   ( P3 ) PL - 5 no - stale - summaries preservation :
--     Forall pipeline step S downstream of emit_burden_ops ( Steps 5 , 5 a , 6 , 7 ,
--     8 , 8 a , 9 , 10 , 11 , 12 ) .
--       S consumes BR ( F ) as a fresh derivation from L ' s final post - IA - 7
--       converged state .
--     Burden - registry pre - pass outputs become typed inputs to subsequent
--     pipeline steps without staleness ; BR ( F ) is recomputed for each F at
--     Step 4 - companion entry , never reused across mutations to L .
--   ( P4 ) PL - 6 adding - a - pass meta - rule honored :
--     The pipeline ordering update ( insertion of emit_burden_ops at the
--     sec - 4 A . 2 - shipped position ) is documented in Annex E section AIMS ( the
--     pipeline definition ) + docs / ori_lang / proposals / approved / aims - burden - tracking - proposal . mdsection - 4 A -
--     minimal - lattice - adaptation . md sec - 4 A success_criterion 1 ( the shipped
--     insertion point ) ; no PL - 1 . . PL - 11 invariant is violated by the
--     addition .
-- Proof obligation (verbatim from canonical-notation source):
-- Four - part constructive discharge via interprocedural_summary ( PRIMARY
-- engine ) over the pipeline ordering specification , with structural_
-- induction for the per - function sequencing argument .
-- Part ( P1 ) emdash PL - 1 interprocedural - first invariant preservation :
--   Per PL - 1 ( Annex E section AIMS PL - 1 ) , Steps 1 - 2 SHALL run once across all
--   functions BEFORE any per - function step . The sec - 4 A . 2 - shipped insertion
--   point places emit_burden_ops at the Step 4 - companion position emdash i . e . ,
--   WITHIN the per - function pipeline ( Steps 3 - 12 ) , specifically between
--   Step 4 ( analyze_function ) and Step 5 ( realize_rc_reuse ) .
--   Steps 1 - 2 ( analyze_program , apply_ownership ) are unchanged by the
--   insertion : they still execute interprocedurally across all functions
--   before per - function pipeline begins . emit_burden_ops does NOT execute
--   during Steps 1 - 2 ; it executes within per - function processing of each
--   function F after that function ' s Step 4 has run .
--   Per PL - 1 a ( per - function SCC topological order ) , each function F is
--   processed in SCC topological order . emit_burden_ops for F runs as part
--   of F ' s per - function pipeline , between F ' s Step 4 and F ' s Step 5 ;
--   callees F ' ( SCC predecessors of F ) have already completed Steps 1 - 12
--   ( or are in the same SCC , per the within - SCC reverse - postorder
--   sub - ordering ) .
--   Conclusion ( P1 ) : the interprocedural - first invariant is preserved .
--   emit_burden_ops insertion does NOT reorder Steps 1 - 2 relative to Steps
--   3 - 12 .
-- Part ( P2 ) emdash acyclic BR - reads - L dependency :
--   Per CH - 4 Part ( P1 ) per - variable immutability , L [ v ] @ pp is invariant
--   under BR mutation . The dependency direction is therefore strictly
--   one - way : BR reads L ( post - IA - 7 convergence ) ; L does not read BR .
--   Per PL - 2 ( Step 4 precedes Step 5 ) , emit_burden_ops is invoked AFTER
--   Step 4 completes ( L converged for F ) but BEFORE Step 5 begins . BR ( F )
--   is computed from L ' s converged values for F emdash a read - only consumption .
--   Per PL - 1 a SCC topological order , when F is processed , F ' s callees
--   have already completed their per - function pipelines ( or are in the
--   same SCC ) . BR ( F ) . burden_emitted [ v ] for variables v defined in F may
--   depend on :
--     ( i ) L ' s converged value L [ v ] for v in vars ( F ) emdash read - only per CH - 4
--     ( ii ) For variables flowing in from callees ( e . g . , return values
--          with derived contracts via TF - 6 refine ) , the callee ' s
--          MemoryContract emdash computed at Step 1 - 2 per PL - 1 , available
--          before F ' s Step 4
--   Neither dependency source forms a cycle :
--     - L [ v ] is read - only after IA - 7 ( CH - 4 P1 )
--     - MemoryContract from callees is read - only after Step 1 - 2
--     - BR ( F ) itself is written exactly once per F per Step 4 - companion
--       invocation
--   Conclusion ( P2 ) : the BR - reads - L dependency is acyclic at the per -
--   function level AND at the interprocedural level . No cycle between BR
--   computation and L computation .
-- Part ( P3 ) emdash PL - 5 no - stale - summaries preservation :
--   Per PL - 5 ( no pass may rely on stale summaries ) , downstream pipeline
--   steps ( Steps 5 , 5 a , 6 , 7 , 8 , 8 a , 9 , 10 , 11 , 12 ) consume BR ( F ) only
--   AFTER emit_burden_ops has produced BR ( F ) for F . BR ( F ) is recomputed
--   for each F at emit_burden_ops entry emdash it is NOT reused across mutations
--   to L for the same F ( because L is converged + immutable post - IA - 7 per
--   CH - 4 P1 , there are no L mutations to invalidate BR ( F ) for F ' s
--   processing ) .
--   Per CH - 4 Part ( P3 ) block - boundary map immutability , L . block_entry_
--   states + L . block_exit_states remain at their post - IA - 7 converged values
--   across BR mutations ; downstream consumers reading L via ArcVarId - keyed
--   lookups ( Step 10 realize_annotations per arc . md sec - Pipeline ) see the
--   same converged L that BR ( F ) was computed from .
--   Per CH - 1 Part ( P1 ) lattice - bridge consistency , BR ( F ) . burden_emitted [ v ]
--   is a memoized DP - 2 / DP - 3 verdict on L [ v ] ; the memoization is fresh per
--   emit_burden_ops invocation . Downstream consumers see a fresh BR ( F )
--   derived from a fresh - but - stable L .
--   Conclusion ( P3 ) : PL - 5 no - stale - summaries is preserved . BR ( F ) is a
--   fresh derivation ; downstream consumers see a fresh - but - stable BR ( F )
--   paired with a fresh - but - stable L .
-- Part ( P4 ) emdash PL - 6 adding - a - pass meta - rule honored :
--   Per PL - 6 ( adding a pass requires updating ordering + proving no
--   constraint violation ) , the burden - registry pre - pass addition is :
--     ( a ) Documented : Annex E section AIMS lists the pipeline steps ; the
--         sec - 4 A . 2 insertion is documented in docs / ori_lang / proposals / approved / aims - burden - tracking - proposal . md
--         docs / ori_lang / proposals / approved / aims - burden - tracking - proposal . md sec - 4 A success_criterion
--         1
--     ( b ) Constraint - no - violation proven : Parts ( P1 ) + ( P2 ) + ( P3 ) above
--         jointly establish that no PL - 1 . . PL - 11 invariant is violated :
--           - PL - 1 ( interprocedural - first ) : preserved per ( P1 )
--           - PL - 1 a ( SCC topological order ) : preserved per ( P1 ) emdash burden -
--             registry executes within per - function processing
--           - PL - 2 ( Step 4 precedes Step 5 ) : preserved emdash burden - registry
--             inserted between , not reordering Step 4 vs Step 5
--           - PL - 3 ( Step 5 precedes Step 9 ) : preserved emdash burden - registry
--             does not move Step 5 or Step 9
--           - PL - 4 ( Step 10 follows Step 9 ) : preserved emdash burden - registry
--             does not move Step 9 or Step 10
--           - PL - 4 a ( Step 8 a precedes Step 9 ) : preserved emdash unchanged
--           - PL - 5 ( no stale summaries ) : preserved per ( P3 )
--           - PL - 6 ( adding - a - pass meta - rule ) : honored per this Part ( P4 )
--           - PL - 7 . . PL - 11 ( TRMC sub - rules ) : orthogonal to burden - registry ;
--             burden - registry is a backward - dataflow - output consumer , not
--             a TRMC participant
--   Conclusion ( P4 ) : PL - 6 is honored . The pipeline ordering update is
--   documented + the no - violation proof is constructed by composing
--   Parts ( P1 ) + ( P2 ) + ( P3 ) .
-- Coverage gate : the four Parts ( P1 , P2 , P3 , P4 ) together discharge the
-- phase - ordering composition invariant per the sec - 1 Composition . proof : 136
-- sorry obligation . A regression dropping any Part leaves CH - 5 ' s claim
-- weakened emdash Part ( P1 ) is the interprocedural - first preservation ; Part
-- ( P2 ) is the acyclic dependency ; Part ( P3 ) is the no - stale - summaries
-- corollary ; Part ( P4 ) is the PL - 6 meta - rule discharge .
-- Engines dispatched :
--   structural_induction ( CO - PRIMARY emdash per the sec - 11 . 0 Per - CH Proof - Status
--     Tracking table CH - 5 row + Composition . proof : 165 - 169 skeleton
--     dispatch ; structural induction over pipeline - step sequencing )
--   interprocedural_summary ( PRIMARY emdash per the sec - 11 . 0 table CH - 5 row +
--     Composition . proof : 165 - 169 ; SCC - level pipeline - ordering proof ; PL - 1
--     + PL - 1 a interprocedural - first preservation under burden - registry
--     insertion )
--   case_analysis ( CO - PRIMARY emdash per - PL - rule enumeration in Part ( P4 ) ;
--     enumerative coverage of PL - 1 . . PL - 11 constraint preservation )
--   lattice ( CO - PRIMARY emdash L - 6 monotonicity inherited from CH - 4 P1 ; L - 7
--     canonicalization idempotence for fresh - vs - stale BR ( F ) derivation
--     argument in ( P3 ) )
-- TODO(BUG-XX-NNN): substantive translation pending Mathlib AimsState model;
-- placeholder body is `True := by trivial` so Lean 4's parser accepts
-- the file. Semantic equivalence to the Ori claim is out of scope for
-- the §01A bootstrap per the SMT / Lean 4 emission strategy Option C consequences.
theorem CH_5_Phase_ordering_composition_emdash_PL_1_interprocedural_first_with_burden_registr : True := by trivial

end AimsBootstrap

def main : IO Unit := pure ()
