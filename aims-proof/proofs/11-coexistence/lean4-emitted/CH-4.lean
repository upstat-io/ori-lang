-- AIMS-Proof bootstrap translation
-- SSOT: aims-proof/checker/src/emit/lean4.rs (the SMT / Lean 4 emission strategy Option C)
-- Constructive-by-default per the foundational-axiom policy; classical escalation
-- requires matched commit per the foundational-axiom policy §Permitted Extensions.
-- BANNED: Classical.em, Classical.choice, funext, propext, proof
-- irrelevance, Markov's Principle — absent a matched extension entry
-- in the foundational-axiom policy.
-- Translated from proofs/11-coexistence/CH-4.proof

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

-- Translated from proofs/11-coexistence/CH-4.proof:CH-4
-- Theorem name (verbatim from canonical-notation source):
-- AimsStateMap immutability under burden - registry mutation
-- Preconditions (verbatim from canonical-notation source):
-- - depends - on CH - 1 ( Burden - registry - lattice composition soundness ) at aims - proof / proofs / 11 - coexistence / CH - 1 . proof emdash CH - 1 ' s Part ( P2 ) no - double - counting establishes the acyclic dependency direction ( BR reads L ; L does not read BR ) ; CH - 4 expands the immutability invariant in detail
-- - L is the converged AimsStateMap for function F per Annex E section AIMS IA - 7 ( intraprocedural backward dataflow reaches fixpoint at finite height per L - 5 + L - 6 ) ; L ' s per - variable AimsState assignments are canonical post - CN - 1 . . CN - 8
-- - BR is the burden - registry typed pre - pass output computed AFTER L ' s convergence per the sec - 4 A . 2 design ( acyclic dependency : BR reads L , L does not depend on BR ) ; BR . burden_emitted is a derived BitSet < ArcVarId > populated by emit_burden_ops at Step 4 - companion pre - pass per docs / ori_lang / proposals / approved / aims - burden - tracking - proposal . md sec - 4 A success_criterion 4
-- - consume_stack walks the instruction stream with BOTH L ( read - only after IA - 7 ) AND BR ( read - only after the sec - 4 A . 2 pre - pass write ) as inputs
-- - depends - on sec - 2 L - 2 ( associativity ) at aims - proof / proofs / 2 - lattice / L - 2 . proof emdash lattice state - map immutability under read - only access ; L - 2 is load - bearing for proving that lattice state remains canonical under BR - mutation events ( BR mutations do NOT trigger L re - joins )
-- - depends - on sec - 2 L - 6 ( monotonicity ) at aims - proof / proofs / 2 - lattice / L - 6 . proof emdash backward transfer functions are monotone ( a <= b implies f ( a ) <= f ( b ) ) ; CH - 4 uses L - 6 to prove that L ' s per - variable values are stable under BR mutations ( no L - dimension monotone update fires from BR writes , because BR is outside L ' s lattice product )
-- - depends - on sec - 7 PL - 2 ( Step 4 precedes Step 5 ) at aims - proof / proofs / 7 - pipeline / PL - 2 . proof emdash analyze_function ( Step 4 ) precedes realize_rc_ reuse ( Step 5 ) ; emit_burden_ops invoked between Step 4 and Step 5 per sec - 4 A success_criterion 1 ; therefore L is converged ( read - only ) BEFORE BR writes occur
-- - depends - on sec - 7 PL - 5 ( no stale summaries ) at aims - proof / proofs / 7 - pipeline / PL - 5 . proof emdash no pass may rely on stale summaries ; burden - registry pre - pass outputs become typed inputs to subsequent pipeline steps without circular dependency ; PL - 5 directly implies BR is a fresh derivation from L ' s final converged state
-- - depends - on arc . md sec - Non - Negotiable - Invariants invariant 5 ( c ) emdash typed pre - pass inputs that land on AimsStateMap ( as immortal detection does via the immortals : Vec < bool > bitvector ) ; BR . burden_emitted is the canonical example of a typed pre - pass input emdash it lands on AimsStateMap as a side - table , NOT as a lattice dimension
-- - Mutation events : a BR mutation event M is a write to BR . burden_emitted or to any companion side - table BR carries ( e . g . , BR . class_covered : BitSet < ClassId > ) . The set of mutation events for a function F is the finite set of emit_burden_ops calls during the sec - 4 A . 2 pre - pass ; for any post - Step - 4 program point pp , BR ' s state is THE result of replaying that finite event sequence
-- Soundness property (verbatim from canonical-notation source):
-- Forall ArcFunction F . Forall AimsStateMap L ( converged per IA - 7 ) .
-- Forall mutation_event M of BR .
--   ( P1 ) Per - variable immutability :
--     Forall variable v in vars ( F ) . Forall program point pp .
--       L [ v ] @ pp computed at pp BEFORE M = L [ v ] @ pp computed at pp AFTER M
--     I . e . , for every variable v at every program point pp , L ' s per -
--     variable AimsState value is identical before and after the mutation
--     event M .
--   ( P2 ) Canonicalization preservation :
--     After every BR mutation event M , L ' s per - variable AimsState values
--     remain canonical :
--       Forall v in vars ( F ) . canonicalize ( L [ v ] @ pp ) = L [ v ] @ pp
--     I . e . , L ' s per - variable AimsState assignments continue to satisfy
--     CN - 1 . . CN - 8 post - mutation ( CN invariants preserved ) .
--   ( P3 ) Block - boundary map immutability :
--     After every BR mutation event M , L ' s per - block_entry_states +
--     block_exit_states maps remain at their post - IA - 7 converged values :
--       Forall block b in F . L . block_entry_states [ b ] ( post - M )
--                     = L . block_entry_states [ b ] ( pre - M )
--       Forall block b in F . L . block_exit_states [ b ] ( post - M )
--                     = L . block_exit_states [ b ] ( pre - M )
--     I . e . , the converged AimsStateMap ' s block - boundary state maps are
--     side - effect - free under burden - registry mutations .
--   Composite : burden - registry mutations are SIDE - EFFECT - FREE w . r . t .
--   AimsStateMap ; no shared mutable state between BR computation and L
--   computation .
-- Proof obligation (verbatim from canonical-notation source):
-- Three - part constructive discharge by structural_induction over BR
-- mutation events ; the proof reduces to a memory - layout independence
-- argument ( BR and L are disjoint memory regions with read - only L access
-- from BR computation ) plus a pipeline - ordering invariant ( PL - 2 + PL - 5 ) .
-- Part ( P1 ) emdash per - variable immutability :
--   Per PL - 2 , the AIMS pipeline orders Step 4 ( analyze_function ) BEFORE
--   Step 5 ( realize_rc_reuse ) ; emit_burden_ops invokes between Step 4 and
--   Step 5 per sec - 4 A success_criterion 1 . Therefore at the time of any BR
--   mutation event M , L has already reached IA - 7 convergence and is no
--   longer being updated .
--   Structurally , L is stored at AimsStateMap . block_entry_states +
--   AimsStateMap . block_exit_states + AimsStateMap . events ( per shipped
--   compiler / ori_arc / src / aims / intraprocedural / mod . rs :
--   AimsStateMap struct fields ) . BR ' s storage is disjoint :
--   BR . burden_emitted : BitSet < ArcVarId > stored as a separate side - table ,
--   not embedded in AimsStateMap ' s lattice - dimension fields .
--   The disjointness is structural : BR ' s writes go to BR . burden_emitted
--   ( and BR ' s companion side - tables ) ; they do NOT alias any field of
--   AimsStateMap . Per the Rust ownership model + the sec - 4 A burden - registry
--   module boundary ( docs / ori_lang / proposals / approved / aims - burden - tracking - proposal . mdsection - 4 A - minimal -
--   lattice - adaptation . md sec - 4 A success_criterion 4 ) , BR ' s mutation API
--   takes & mut BR and & L ( immutable reference ) emdash Rust ' s borrow checker
--   mechanically prohibits a BR mutation from modifying L through the
--   same code path .
--   Per L - 6 monotonicity , even if a BR mutation were ( hypothetically ) to
--   trigger a re - evaluation of L ' s transfer functions , monotone updates
--   preserve L ' s pre - mutation values when the input lattice state has not
--   changed emdash which it has not , because BR is outside L ' s lattice product
--   per arc . md invariant 5 ( c ) .
--   Conclusion ( P1 ) : L [ v ] @ pp is invariant under BR mutation events for
--   every variable v at every program point pp . The invariance is
--   structural ( disjoint memory regions ) + algebraic ( L - 6 monotonicity
--   preserves stable inputs ) + pipeline - ordered ( PL - 2 ensures L converged
--   before BR mutates ) .
-- Part ( P2 ) emdash canonicalization preservation :
--   Per CN - 1 . . CN - 8 ( Annex E section AIMS canonicalization rules ) , L ' s per -
--   variable AimsState values are canonical post - IA - 7 ( canonicalization
--   runs after every join + every transfer function at finite - height
--   fixed point per L - 7 idempotence + L - 8 join preservation ) . Per Part
--   ( P1 ) , L [ v ] @ pp does NOT change under BR mutation ; therefore L [ v ] @ pp ' s
--   canonicalization status is preserved by the immutability of the value
--   itself .
--   Formally : if canonicalize ( L [ v ] @ pp ) = L [ v ] @ pp before M ( by IA - 7
--   convergence ) , and L [ v ] @ pp is invariant under M ( by Part P1 ) , then
--   canonicalize ( L [ v ] @ pp ) = L [ v ] @ pp after M ( by applying canonicalize to
--   an unchanged value ) .
--   Per L - 7 ( canonicalization idempotence ) , repeated canonicalization
--   queries on the same value are stable ; therefore even if a downstream
--   consumer were to re - canonicalize L [ v ] @ pp post - mutation , the result
--   remains identical .
--   Conclusion ( P2 ) : canonicalization is preserved across BR mutations ;
--   L ' s per - variable AimsState values continue to satisfy CN - 1 . . CN - 8 post -
--   mutation . CN invariants are preserved .
-- Part ( P3 ) emdash block - boundary map immutability :
--   Per Part ( P1 ) , every variable ' s per - program - point AimsState value is
--   invariant under BR mutation . The block - boundary maps
--   ( AimsStateMap . block_entry_states [ b ] and AimsStateMap . block_exit_states [ b ] )
--   are derived per - block aggregations of per - variable AimsState values
--   computed by sec - 6 IA - 2 reverse - postorder block processing + IA - 3 join .
--   Per L - 1 commutativity + L - 2 associativity + L - 3 idempotence , the
--   n - ary join at CFG merges ( per IA - 9 ) is permutation - invariant and
--   stable emdash repeated joins on the same input multiset yield the same
--   result . Per Part ( P1 ) , the input multisets to each block ' s join ( the
--   per - variable AimsState values flowing in from successors ) are
--   invariant under BR mutation ; therefore the per - block aggregation
--   results ( block_entry_states [ b ] + block_exit_states [ b ] ) are also
--   invariant .
--   Per PL - 5 ( no stale summaries ) , the converged values in block_entry_
--   states + block_exit_states are the final IA - 7 fixpoint ; they do not
--   "re-fire" under subsequent pipeline steps ( Step 5 realize_rc_reuse
--   consumes them read - only ; Step 9 merge_blocks invalidates the
--   position - keyed projection but preserves the ArcVarId - keyed view
--   block_entry_states points to via the block - index mapping per arc . md
--   sec - Pipeline - Ordering ) .
--   Conclusion ( P3 ) : block - boundary maps remain at their post - IA - 7
--   converged values across BR mutation events . The maps are side - effect -
--   free under BR writes .
-- Coverage gate : the three Parts ( P1 , P2 , P3 ) together discharge the
-- composite immutability invariant per the sec - 1 Composition . proof : 103
-- sorry obligation . A regression dropping any Part leaves CH - 4 ' s claim
-- weakened emdash Part ( P1 ) is the load - bearing per - variable invariant ; Part
-- ( P2 ) is its canonicalization corollary ; Part ( P3 ) is its block - aggregation
-- corollary . All three are required for the downstream consumers ( CH - 5
-- phase - ordering composition , CH - comp union - soundness ) to ground against .
-- Engines dispatched :
--   structural_induction ( PRIMARY emdash per the sec - 11 . 0 Per - CH Proof - Status
--     Tracking table CH - 4 row + Composition . proof : 126 - 130 skeleton
--     dispatch ; structural induction over BR mutation event sequence ;
--     per - mutation immutability check )
--   interprocedural_summary ( CO - PRIMARY emdash per Composition . proof : 126 - 130 ;
--     BR - as - typed - pre - pass + acyclic BR - reads - L dependency at SCC level )
--   case_analysis ( CO - PRIMARY emdash Appendix B post - CN - 1 . . CN - 8 canonical - state
--     enumeration for the Part ( P2 ) canonicalization preservation argument )
--   lattice ( CO - PRIMARY emdash L - 1 . . L - 3 join properties + L - 6 monotonicity
--     + L - 7 canonicalization idempotence ; substrate for the per - variable
--     + block - boundary immutability invariants )
-- TODO(BUG-XX-NNN): substantive translation pending Mathlib AimsState model;
-- placeholder body is `True := by trivial` so Lean 4's parser accepts
-- the file. Semantic equivalence to the Ori claim is out of scope for
-- the §01A bootstrap per the SMT / Lean 4 emission strategy Option C consequences.
theorem CH_4_AimsStateMap_immutability_under_burden_registry_mutation : True := by trivial

end AimsBootstrap

def main : IO Unit := pure ()
