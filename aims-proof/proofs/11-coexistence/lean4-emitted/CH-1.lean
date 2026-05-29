-- AIMS-Proof bootstrap translation
-- SSOT: aims-proof/checker/src/emit/lean4.rs (the SMT / Lean 4 emission strategy Option C)
-- Constructive-by-default per the foundational-axiom policy; classical escalation
-- requires matched commit per the foundational-axiom policy §Permitted Extensions.
-- BANNED: Classical.em, Classical.choice, funext, propext, proof
-- irrelevance, Markov's Principle — absent a matched extension entry
-- in the foundational-axiom policy.
-- Translated from proofs/11-coexistence/CH-1.proof

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

-- Translated from proofs/11-coexistence/CH-1.proof:CH-1
-- Theorem name (verbatim from canonical-notation source):
-- Burden - registry - lattice composition soundness
-- Preconditions (verbatim from canonical-notation source):
-- - BR is the burden - registry typed pre - pass output for function F , with the side tables described in docs / ori_lang / proposals / approved / aims - burden - tracking - proposal . md sec - 4 A success_criterion 4 : BR . burden_emitted : BitSet < ArcVarId > ( populated by emit_burden_ops ) BR . class_covered : BitSet < ClassId > ( derived intraprocedurally in aims / intraprocedural / post_convergence . rs )
-- - L is the lattice analysis converged AimsStateMap for F per Annex E section AIMS IA - 7 ( intraprocedural backward dataflow reaches fixpoint at finite height per L - 5 + L - 6 + IA - 7 ' s iteration bound )
-- - consume_stack ( F , BR , L ) is the sec - 4 A . 2 elimination consumer as defined in aims - proof / proofs / 11 - coexistence / Handshake . proof Function 1 ( burden_emission_path ) , terminating at the shipped ` pub ( crate ) fn eliminate_burden_ops ` at compiler / ori_arc / src / aims / realize / burden_elim . rs : 87 ( visibility - qualifier confirmed via intel - query . sh callers eliminate_burden_ops 2026 - 5 - 28 emdash 29 callers , all under . . . / burden_elim / tests . rs ; no production caller - site above eliminate_burden_ops exists , confirming the elimination consumer ' s terminal - position invariant )
-- - Lattice bridge : burden - owned ( s ) per Handshake . proof Predicate 1 iff s . access = Owned and s . consumption in { Linear , Affine } and s . uniqueness = Unique and is_rc_dec_unnecessary ( s ) ( per Annex E section AIMS DP - 2 )
-- - depends - on sec - 4 TF - 3 ( Construct FRESH initialization ) at aims - proof / proofs / 4 - transfers / TF - 3 . proof emdash every Construct allocates a FRESH ( Owned , Linear , Once , Unique , BlockLocal , shape , { may_alloc = T } ) state ; this is the entry - point for the burden - owned lattice bridge ( Construct sites are the canonical source of class_covered = true variables under TF - 3 ' s monotone L - 6 layer ( b ) per - ctor constant - function proof )
-- - depends - on sec - 4 TF - 4 ( Project borrow propagation ) at aims - proof / proofs / 4 - transfers / TF - 4 . proof emdash projections inherit Uniqueness from source ; field - grain dispatch per Handshake . proof Predicate 2 mixed - coverage rule consumes this monotonicity
-- - depends - on sec - 4 TF - 6 / TF - 6 a ( Apply / Invoke refine contract ) at aims - proof / proofs / 4 - transfers / TF - 6 . proof + TF - 6 a . proof emdash refine narrows Uniqueness from CONSERVATIVE MaybeShared to contract . uniqueness ; Owned access stays at CONSERVATIVE ( per Annex E section AIMS TF - 6 NON - narrow list ) ; the refinement preserves the lattice - bridge predicate ( CH - 1 depends on this for class identification under callee return contracts )
-- - depends - on sec - 4 TF - 11 ( backward demand propagation ) at aims - proof / proofs / 4 - transfers / TF - 11 . proof emdash TF - 11 emits ( operand , Once , Linear ) demands ; seq_add accumulation reaches Many at multi - use sites , preventing premature burden - owned classification on values whose effective Cardinality > Once ( DP - 3 inc - elidable false at Many )
-- - depends - on sec - 4 TF - 14 ( Project source demand propagation ) at aims - proof / proofs / 4 - transfers / TF - 14 . proof emdash seq_add cardinality propagation through Project is QTT - consistent ; alias - source demand is not under - counted ( this is the proof that CH - 1 ' s "no double-counting" claim does not silently miss alias sites )
-- - depends - on sec - 4 TF - 15 / TF - 15 a ( Set / SetTag in - place mutation backward demand ) at aims - proof / proofs / 4 - transfers / TF - 15 . proof + TF - 15 a . proof emdash ` Set { base , field , value } ` and ` SetTag { base , tag } ` produce no ` dst ` forward state ; backward demand promotes base . access := Owned + base . locality := max ( base . locality , value . locality ) ; these promotions preserve burden - owned ' s Owned access invariant
-- - depends - on sec - 5 DP - 2 truth table at aims - proof / proofs / 5 - decisions / DP - 2 . proof emdash ` is_rc_dec_unnecessary ( s ) iff s . cardinality = Absent or s . consumption = Dead ` ( canonical states post - CN - 1 ; the disjunction is bidirectionally implied on canonical states ) . DP - 2 gates SUPPLEMENTARY RC ops ONLY per Annex E section AIMS DP - 2 + sec - 8 RL - 2 : terminal RL - 2 / RL - 4 / RL - 5 emissions own their own logic , NOT suppressed by DP - 2 ' s pre - pass false - positive
-- - depends - on sec - 5 DP - 3 truth table at aims - proof / proofs / 5 - decisions / DP - 3 . proof emdash ` is_rc_inc_elidable ( s ) iff s . cardinality = Once and s . consumption = Linear ` ( canonical states ; moved - once , no inc ) . Direct truth - table consequence of the lattice ' s LinearxOnce cell .
-- - depends - on sec - 2 L - 1 . . L - 8 lattice properties at aims - proof / proofs / 2 - lattice / L - 1 . proof . . L - 8 . proof emdash substrate for the burden - owned conjunction ( each conjunct is a per - dimension query on the lattice ; per L - 3 idempotence + L - 7 canonicalization idempotence , repeated burden - registry reads are stable ; per L - 6 monotonicity , the conjunction is preserved under transfer functions ; per L - 2 associativity , the conjunction is well - defined under N - ary join at CFG merges )
-- Soundness property (verbatim from canonical-notation source):
-- Forall ArcFunction F . Forall BurdenSpec BR . Forall AimsStateMap L ( converged ) .
-- Forall variable v in vars ( F ) .
--   ( P1 ) Lattice - bridge consistency :
--     consume_stack ( F , BR , L ) classifies v ' s burden eligibility iff
--     v ' s converged lattice state L [ v ] satisfies the burden - owned
--     predicate ; equivalently , BR ' s elimination decision for v
--     ( DP - 2 dec unnecessary OR DP - 3 inc elidable ) is consistent with
--     L [ v ] ' s converged AimsState . Symbolically :
--       v in BR . burden_emitted and BR . eliminates ( v )
--         iff
--       burden - owned ( L [ v ] ) and ( is_rc_dec_unnecessary ( L [ v ] )
--                             or is_rc_inc_elidable ( L [ v ] ) )
--   ( P2 ) No double - counting :
--     Burden - registry writes do NOT race - invalidate the lattice ' s
--     elimination decisions . Formally :
--       Forall v in BR . burden_emitted . Forall pp in program_points ( F ) .
--         L [ v ] @ pp computed at pp = L [ v ] @ pp computed under any BR mutation
--         event ( BR . write ( v ' , . . . ) for v ' in vars ( F ) ) .
--     Equivalently , consume_stack emits EXACTLY ONE elimination decision
--     per ( v , pp ) pair emdash burden - derived AND lattice - derived emissions
--     are THE SAME decision , not stacked . This is the canonical " single
--     elimination decision " invariant the sec - 11 coexistence handshake binds .
--   ( P3 ) Per - class coexistence well - formedness :
--     For each AIMS class C ( identified by IC - 3 ParamContract dimensions
--     per the burden - tracking sec - 4 A success_criterion 4 class taxonomy ) ,
--     the lattice ' s per - variable claim AND the burden registry ' s per - class
--     annotation agree on class membership :
--       Forall v in class_members ( C ) . class_of ( v ) = C
--         iff
--       ( Forall dim in { access , consumption , cardinality , uniqueness , locality ,
--                 shape , effect } . L [ v ] . dim in C . allowed_values ( dim ) )
--     Lattice classes + burden classes do NOT overlap in their elimination
--     claims emdash the class taxonomy is total + disjoint per CH - 3 ( the
--     partition discharge ) .
-- Proof obligation (verbatim from canonical-notation source):
-- Three - part constructive discharge , mirroring the sec - 9 VF - 1 proof shape
-- ( aims - proof / proofs / 9 - verification / VF - 1 . proof Parts ( P1 ) + ( P2 ) +
-- coverage - grid ) :
-- Part ( P1 ) emdash lattice - bridge consistency :
--   For each variable v in vars ( F ) , the converged AimsStateMap L assigns
--   a canonical AimsState L [ v ] ( per IA - 7 + CN - 1 . . CN - 8 canonicalization ) .
--   The burden - owned predicate is the conjunction of four per - dimension
--   queries on L [ v ] :
--     burden - owned ( L [ v ] ) iff
--         L [ v ] . access = Owned ( Access dim , height 1 )
--       and L [ v ] . consumption in { Linear , Affine } ( Consumption dim , height 3 )
--       and L [ v ] . uniqueness = Unique ( Uniqueness dim , height 2 )
--       and is_rc_dec_unnecessary ( L [ v ] ) ( DP - 2 truth table , 4 rows )
--   Per L - 5 finite height , the conjunction is bounded over a finite
--   product subspace ( Access x Consumption x Uniqueness x DP - 2 - applicable
--   = 2 x 4 x 3 x 2 = 48 raw combinations ; CN - 1 + CN - 3 + CN - 6 + CN - 8 prune
--   to canonical subset per Appendix B ; effective burden - owned subspace
--   derived below in Part ( P3 ) ' s class - grid ) .
--   Per L - 6 monotonicity , the conjunction is preserved under each
--   forward transfer function ( TF - 3 produces FRESH ( Owned , Linear , Once ,
--   Unique , BlockLocal , shape ) which trivially satisfies burden - owned
--   under DP - 2 false - at - Once - Linear ; subsequent backward demand via
--   TF - 11 / TF - 14 may PROMOTE consumption to Unrestricted via seq_add ,
--   escaping burden - owned to non - burden - owned emdash preserving consistency
--   with the lattice ' s claim of multi - use Cardinality ) .
--   Per the DP - 2 truth - table proof ( aims - proof / proofs / 5 - decisions / DP - 2 . proof
--   Part ( a ) Appendix C row partition + Part ( c ) soundness equality on
--   canonical rows ) : is_rc_dec_unnecessary ( L [ v ] ) = true iff canonical row
--   ( Dead , Absent ) ; false on canonical rows { ( Linear , Once ) , ( Linear ,
--   Many ) , ( Affine , Once ) , ( Affine , Many ) , ( Unrestricted , Once ) ,
--   ( Unrestricted , Many ) } . DP - 3 false except on canonical row ( Linear ,
--   Once ) . These per - row decisions are pure - function on L [ v ] .
--   Verifier : consume_stack walks F ' s instruction stream ; for each v
--   with BR . burden_emitted [ v ] = true , queries DP - 2 ( L [ v ] ) or DP - 3 ( L [ v ] )
--   and removes the burden op iff the predicate fires . The lattice - bridge
--   consistency claim P1 reduces to : the burden registry ' s class_covered
--   annotation IS computed by the same predicate ( the class - covered rule
--   consumes burden - owned per Handshake . proof Predicate 2 ) . Per the
--   shipped pipeline ordering ( Annex E section AIMS PL - 2 Step 4 precedes
--   Step 5 ; emit_burden_ops invoked between Step 4 analyze_function and
--   Step 5 realize_rc_reuse per docs / ori_lang / proposals / approved / aims - burden - tracking - proposal . md
--   sec - 4 A success_criterion 1 ) , BR . class_covered is computed from L
--   AFTER L converges ; therefore burden - owned ( L [ v ] ) is a function of
--   L ' s converged state , computed once and consumed by consume_stack at
--   Step 5 / 5 a / 10 burden - elimination sites .
--   Conclusion ( P1 ) : the bridge is constructively definable as a pure
--   function on L ' s converged AimsStateMap ; consume_stack ' s elimination
--   decisions are derived from this pure function ; therefore the
--   iff - equivalence claim holds by construction .
-- Part ( P2 ) emdash no double - counting :
--   The dependency direction is acyclic :
--     BR reads L ( after IA - 7 convergence )
--     L does NOT read BR
--   This is the load - bearing invariant CH - 4 discharges in detail
--   ( AimsStateMap immutability under burden - registry mutation ; the sec - 11
--   success_criterion 12 depends - on sec - 2 L - 2 lattice algebra state - map
--   immutability under read - only access ) . Per CH - 4 , Forall mutation event M
--   of BR . L is NOT modified by M ; L ' s per - variable AimsState values
--   remain canonical ( CN invariants preserved ) ; L ' s per - block_entry_states
--   + block_exit_states maps remain at their post - IA - 7 converged values .
--   Given the acyclic dependency , consume_stack reads BOTH L ( read - only
--   after Step 4 ) AND BR ( read - only after Step 5 pre - emission ) . The
--   elimination decision for variable v at program point pp is :
--     eliminate ( v , pp ) =
--       BR . burden_emitted [ v ]
--       and ( is_rc_dec_unnecessary ( L [ v ] @ pp ) or is_rc_inc_elidable ( L [ v ] @ pp ) )
--   For each ( v , pp ) , this is a pure function with EXACTLY ONE Boolean
--   output . There is no shared mutable state between BR ' s per - variable
--   annotation and L ' s per - variable AimsState ( CH - 4 invariant ) . Therefore
--   consume_stack emits exactly one elimination decision per ( v , pp ) ;
--   the burden - derived AND lattice - derived emissions are THE SAME
--   decision , not stacked .
--   Negative - direction witness ( the "no double-counting" claim has
--   teeth emdash a regression that stacks two elimination decisions is
--   REJECTED ) : a hypothetical implementation that reads BR before L
--   converges ( violating PL - 2 Step 4 precedes Step 5 ) would produce
--   eliminate ( v , pp ) with stale L [ v ] @ pp , potentially differing from the
--   post - convergence value ; the sec - 8 RL - 2 emission would then emit a
--   definitional cleanup dec the burden - registry ' s stale read claimed
--   eligible - to - elide . CH - 5 ( phase - ordering composition ) discharges this
--   negative witness emdash PL - 1 interprocedural - first with burden - registry as
--   Step 1 typed pre - pass preserves the no - double - counting invariant
--   iff the BR computation is sequenced AFTER L ' s IA - 7 convergence .
-- Part ( P3 ) emdash per - class coexistence well - formedness :
--   The AIMS class taxonomy per the sec - 11 . 0 Per - CH Proof - Status Tracking
--   table CH - 3 row is total + disjoint over three sub - classes ( CH - 3
--   discharges this in detail ; CH - 1 cites the partition ' s well - formedness
--   as a precondition for the lattice - bridge consistency to be
--   well - defined ) :
--     sub - class A : Owned x Linear x Once x Unique ( RL - 2 + RL - 14 candidate )
--     sub - class B : Borrowed x Linear x Once x Unique ( RL - 14 headerless
--                                                    candidate )
--     sub - class C : MaybeShared x Many ( RL - 7 dynamic COW candidate )
--   For each variable v in vars ( F ) , L [ v ] assigns each lattice dimension
--   a value in the dimension ' s finite chain . The three sub - classes
--   partition the per - variable lattice state space exhaustively over
--   the ( Access x Uniqueness x Cardinality ) sub - product :
--     Class A : L [ v ] . access = Owned and L [ v ] . uniqueness = Unique
--              and L [ v ] . cardinality = Once and L [ v ] . consumption = Linear
--     Class B : L [ v ] . access = Borrowed and L [ v ] . uniqueness = Unique
--              and L [ v ] . cardinality = Once and L [ v ] . consumption = Linear
--     Class C : L [ v ] . uniqueness = MaybeShared and L [ v ] . cardinality = Many
--   Lattice - bridge predicate burden - owned holds ONLY on Class A ( the
--   Owned x Linear x Unique conjunction ; DP - 2 false at Once , DP - 3 TRUE
--   at Linear x Once - > burden - owned eligible for inc - elision ; per RL - 2
--   composition for the dec - side , the cleanup dec at last use is
--   OWNED by the lattice not the burden registry ; therefore burden - owned
--   on Class A means " the lattice ' s DP - 3 inc - elision claim agrees with
--   the burden registry ' s burden - emitted annotation " ) .
--   For Class B , burden - owned fails on the Access = Owned conjunct ;
--   the burden registry MUST NOT classify Class B variables as
--   burden - emitted ( RL - 14 headerless : no RC emission ; the burden walk
--   skips Borrowed variables per the shipped burden_lower . rs
--   DerivedOwnership side - table consumption ) .
--   For Class C , burden - owned fails on the Uniqueness = Unique conjunct ;
--   the burden registry MUST NOT classify Class C variables as
--   burden - emitted ( RL - 7 dynamic COW : IsShared runtime check ; burden
--   elimination would race - invalidate the runtime check ) .
--   Therefore the three sub - classes are total + disjoint w . r . t . their
--   elimination claims : Class A claims burden - eligible ; Classes B + C
--   claim non - burden - eligible . The lattice ' s per - variable AimsState
--   assigns each v to exactly one class via the AccessClass + Uniqueness
--   + Cardinality dimensions ; burden registry ' s class_covered annotation
--   classifies the same v via the burden - owned predicate ; the two
--   classifications agree by Part ( P1 ) ' s lattice - bridge consistency .
--   Conclusion ( P3 ) : per - class coexistence well - formedness holds ; the
--   class taxonomy is total + disjoint at the lattice level ; burden
--   registry annotations agree with lattice claims on class membership ;
--   no class - overlap in elimination claims .
-- Coverage gate : the three Parts ( P1 , P2 , P3 ) together discharge the
-- three soundness - property clauses ( lattice - bridge consistency ,
-- no double - counting , per - class coexistence well - formedness ) per the
-- sec - 1 Composition . proof : 1 sorry obligation . A regression dropping any
-- Part ( e . g . , omitting P3 ' s class partition disjointness ) leaves a
-- soundness - property clause unverified emdash the joined CH - 1 claim is no
-- stronger than its weakest part .
-- Engines dispatched :
--   structural_induction ( PRIMARY emdash per the sec - 11 . 0 Per - CH Proof - Status
--     Tracking table CH - 1 row + Composition . proof : 28 - 32 skeleton dispatch ;
--     per - instruction structural - check soundness over ( P1 ) lattice - bridge
--     consistency + ( P2 ) acyclic - dependency invariant + ( P3 ) per - class
--     partition disjointness )
--   interprocedural_summary ( CO - PRIMARY emdash per Composition . proof : 28 - 32 ;
--     SCC - level burden - registry - vs - lattice agreement under IC - 3 + IC - 4
--     contract dimensions ; load - bearing for class - identification under
--     callee return contracts via TF - 6 / TF - 6 a )
--   case_analysis ( CO - PRIMARY emdash Appendix C truth - table enumeration for
--     DP - 2 + DP - 3 truth tables consumed by burden - owned predicate ;
--     per - class - row enumeration in Part ( P3 ) )
--   lattice ( CO - PRIMARY emdash L - 1 . . L - 8 substrate ; L - 6 monotonicity for
--     burden - owned preservation under transfer functions ; L - 7
--     canonicalization idempotence for repeated burden - registry reads ;
--     L - 2 associativity for N - ary join at CFG merges )
-- TODO(BUG-XX-NNN): substantive translation pending Mathlib AimsState model;
-- placeholder body is `True := by trivial` so Lean 4's parser accepts
-- the file. Semantic equivalence to the Ori claim is out of scope for
-- the §01A bootstrap per the SMT / Lean 4 emission strategy Option C consequences.
theorem CH_1_Burden_registry_lattice_composition_soundness : True := by trivial

end AimsBootstrap

def main : IO Unit := pure ()
