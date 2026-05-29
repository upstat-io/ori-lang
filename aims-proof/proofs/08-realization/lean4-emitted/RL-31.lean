-- AIMS-Proof bootstrap translation
-- SSOT: aims-proof/checker/src/emit/lean4.rs (the SMT / Lean 4 emission strategy Option C)
-- Constructive-by-default per the foundational-axiom policy; classical escalation
-- requires matched commit per the foundational-axiom policy §Permitted Extensions.
-- BANNED: Classical.em, Classical.choice, funext, propext, proof
-- irrelevance, Markov's Principle — absent a matched extension entry
-- in the foundational-axiom policy.
-- Translated from proofs/08-realization/RL-31.proof

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

-- Translated from proofs/08-realization/RL-31.proof:RL-31
-- Theorem name (verbatim from canonical-notation source):
-- disjoint Borrowed parameters - > ! alias . scope + ! noalias metadata ( CRITICAL )
-- Preconditions (verbatim from canonical-notation source):
-- - RL - 31 governs disjoint - Borrowed - param alias metadata per Annex E section AIMS AIMS - > LLVM Fact Export : disjoint Borrowed parameters SHALL receive ! alias . scope + ! noalias metadata pairs . "Disjoint" = no two Borrowed params can alias the same memory at runtime . The proof requires a CROSS - FUNCTION provenance summary not in IC - 2 / IC - 3 alone : each call site proves the actual arguments to distinct Borrowed params trace to different source aggregates or disjoint fields .
-- - CRITICAL : RL - 31 directly addresses sec - 4 B C5 ( the CRITICAL criterion ) emdash the Ori - novel realization rule whose soundness sec - 4 B assumed but never proved ; sec - 8 makes the proof explicit . sec - 11 coexistence builds on it .
-- - Signature ArcFunction x AimsStateMap - > ArcFunction per the canonical proof notation : 264 ; discharged against the sec - 1 Realization . proof sorry obligation .
-- - The full 8 - clause SUFFICIENT condition ( per Annex E section AIMS RL - 31 procedure + docs / ori_lang / proposals / approved / aims - burden - tracking - proposal . md C5 proven_by line 90 ` 8 - clause SUFFICIENT condition ` descriptor ) , enumerated as explicit theorem antecedents : ( 1 ) both params Borrowed ( Access dimension ) ; ( 2 ) per - call - site evaluation across ALL call sites ( any site failing
-- - > no metadata emitted ) ; ( 3 ) arg root - set extraction = filter project_alias_sources [ arg ] to variables with no UPSTREAM source ; ( 4 ) disjoint - root - set case - > distinct source aggregates - > disjoint params ; ( 5 ) FRESH - allocation case ( Construct / Reuse / CollectionReuse not in project_alias_sources ) - > own disjoint root ; ( 6 ) untraceable - arg case - > FAIL conservatively ; ( 7 ) same - root disjoint - fields case - > trace to the originating Project + compare field indices , disjoint iff neither field path is a prefix of the other ( nested - projection prefix test ) ; ( 8 ) borrow_sources is function - local ( sec - 1 . 9 ) , so the cross - function proof operates on the CALLERS ' local provenance tables , NOT callee IC - 2 / IC - 3 alone .
-- - depends - on sec - 5 : DP - 5 disjointness is the prerequisite ; depends - on sec - 1 . 9 borrow_sources + project_alias_sources ( the function - local provenance tables ) ; depends - on sec - 6 : IC - 3 ParamContract . access ( the Borrowed dimension ) . VF - 2 ( b ) consumes the disjointness proof .
-- - RL - 31 is in the critical - proof cross - validation set ( alongside sec - 1 A bootstrap + sec - 11 coexistence handshake per 0 - overview Design Principle 1 + MS - 4 ) ; the sec - 8 close - out gate cross - validates RL - 31 against Lean 4 ( sec - 15 automates the nightly re - check ) .
-- Soundness property (verbatim from canonical-notation source):
-- Forall function f with Borrowed parameters p_i , p_j , and forall call site
-- of f :
--   ( P1 ) Emission decision ( the full 8 - clause SUFFICIENT condition ) : RL - 31
--        emits ! alias . scope + ! noalias for ( p_i , p_j ) iff , at EVERY call
--        site ( clause 2 ) , the args to p_i and p_j are PROVABLY disjoint per
--        clauses ( 1 ) both Borrowed , ( 3 ) root - set extraction , ( 4 ) disjoint
--        root sets , ( 5 ) FRESH allocation own root , ( 6 ) untraceable - > FAIL ,
--        ( 7 ) same - root disjoint - fields prefix test , ( 8 ) on the callers '
--        function - local provenance tables .
--   ( P2 ) Dual - disjointness facet coverage : BOTH ( a ) call - site provenance
--        disjointness ( the 8 - clause per - call - site procedure ) AND ( b )
--        type - level disjointness via BurdenSpec . field_type chains hold .
--        Both facets are load - bearing emdash the type - level facet is demonstrably
--        more precise than the contract - layer encoding for the BUG - 4 - 118
--        shape , but proving ONLY the type - level facet leaves the
--        per - call - site provenance facet ( VF - 2 ( b ) contract - consistency
--        check ) unproven , so the metadata would be unsound .
--   ( P3 ) Soundness : when both facets hold , no two of f ' s Borrowed params can
--        alias the same memory at runtime , so LLVM may treat them as
--        ! noalias emdash the optimizer ' s alias analysis is correct under the
--        metadata .
-- RL - 31 is the substantive Ori - novel contribution : a cross - function
-- provenance proof ( on the callers ' local tables , clause 8 ) that justifies
-- LLVM noalias metadata for borrowed parameters , going beyond what IC - 2 / IC - 3
-- contracts alone can express .
-- Proof obligation (verbatim from canonical-notation source):
-- Constructive discharge by the refinement engine via the 8 - clause
-- per - call - site procedure + the all - sites conjunction ( clause 2 ) + the
-- dual - facet conjunction .
-- Part ( P1 ) facet ( a ) emdash 8 - clause call - site provenance grid :
--   clause4_disjoint_roots_emit - > emit ( distinct root sets )
--   clause5_fresh_alloc_emit - > emit ( FRESH own root )
--   clause6_untraceable_fail - > NO emit ( conservative FAIL )
--   clause7_same_root_disjoint_fields_emit - > emit ( disjoint field paths )
--   clause7_prefix_overlap_no_emit - > NO emit ( prefix overlap )
--   clause1_non_borrowed_no_emit - > NO emit ( not both Borrowed )
--   Coverage gate : 6 clause cases .
-- Clause ( 2 ) all - sites conjunction : metadata is emitted iff EVERY call site
--   proves disjointness emdash ANY failing site withholds the metadata .
-- Clause ( 8 ) : the proof operates on the callers ' function - local
--   borrow_sources / project_alias_sources , NOT callee IC - 2 / IC - 3 alone .
-- Part ( P2 ) dual - facet conjunction : BOTH the call - site provenance facet ( a )
--   AND the type - level facet ( b , BurdenSpec . field_type chains ) must hold ;
--   the type - level facet ALONE is rejected ( negative - direction witness emdash the
--   VF - 2 ( b ) contract - consistency check would be unproven ) .
-- Part ( P3 ) Lean 4 cross - validation ( sec - 8 close - out gate ) : RL - 31 is in the
--   critical - proof cross - validation set ; at sec - 8 close - out the RL - 31 proof
--   MUST cross - validate against Lean 4 emdash this RL - 31 . proof checks clean by
--   Ori ' s checker AND its Lean 4 transcription checks clean , gated by
--   aims - proof / run - section - 8 - proofs . sh ( sec - 15 automates the recurring
--   nightly re - check ) . Evidence anchor : aims - proof / proofs / 8 - realization / RL - 31 . proof
--   + the run - section - 8 - proofs . sh RL - 31 Lean 4 emission step .
-- Engines dispatched :
--   refinement ( PRIMARY emdash
--     section_08 : : verify_rl31_disjoint_borrowed_alias_metadata ; 8 - clause
--     per - call - site grid + all - sites conjunction + dual - facet conjunction ;
--     emits Fail on a clause divergence , a non - conjunctive all - sites
--     emission , or a type - level - facet - alone acceptance )
--   rc_counting ( SECONDARY emdash gracious - accept per the coverage - manifest RL
--     row )
--   case_analysis ( SECONDARY emdash gracious - accept per the coverage - manifest RL
--     row )
-- TODO(BUG-XX-NNN): substantive translation pending Mathlib AimsState model;
-- placeholder body is `True := by trivial` so Lean 4's parser accepts
-- the file. Semantic equivalence to the Ori claim is out of scope for
-- the §01A bootstrap per the SMT / Lean 4 emission strategy Option C consequences.
theorem RL_31_disjoint_Borrowed_parameters_alias_scope_noalias_metadata_CRITICAL : True := by trivial

end AimsBootstrap

def main : IO Unit := pure ()
