-- AIMS-Proof sec-11 coexistence-handshake cross-validation umbrella.
-- SSOT: aims-proof/checker/src/emit/lean4.rs (the SMT / Lean 4 emission strategy Option C).
-- Constructive-by-default per the foundational-axiom policy; classical escalation requires a
-- matched commit per the foundational-axiom policy sec-Permitted-Extensions.
-- Auto-concatenated by aims-proof/scripts/run-section-11-proofs.sh from the
-- per-CH emitted Lean 4 transcriptions at
-- aims-proof/proofs/11-coexistence/lean4-emitted/CH-*.lean.
--
-- Consumed by sec-15 nightly CI per 00-overview.md MS-4 (sec-11 is in the
-- critical-proof cross-validation set alongside sec-01A bootstrap + sec-08
-- RL-31).


-- ============================================================
-- CH-1 (auto-included from proofs/11-coexistence/lean4-emitted/CH-1.lean)
-- ============================================================
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

-- ============================================================
-- CH-2 (auto-included from proofs/11-coexistence/lean4-emitted/CH-2.lean)
-- ============================================================
-- AIMS-Proof bootstrap translation
-- SSOT: aims-proof/checker/src/emit/lean4.rs (the SMT / Lean 4 emission strategy Option C)
-- Constructive-by-default per the foundational-axiom policy; classical escalation
-- requires matched commit per the foundational-axiom policy §Permitted Extensions.
-- BANNED: Classical.em, Classical.choice, funext, propext, proof
-- irrelevance, Markov's Principle — absent a matched extension entry
-- in the foundational-axiom policy.
-- Translated from proofs/11-coexistence/CH-2.proof

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

-- Translated from proofs/11-coexistence/CH-2.proof:CH-2
-- Theorem name (verbatim from canonical-notation source):
-- DP - 2 / DP - 3 elimination consumer composition with predicate - stack - derived ops
-- Preconditions (verbatim from canonical-notation source):
-- - depends - on CH - 1 ( Burden - registry - lattice composition soundness ) at aims - proof / proofs / 11 - coexistence / CH - 1 . proof emdash CH - 1 ' s lattice - bridge consistency ( P1 ) + acyclic BR - reads - L dependency ( P2 ) + per - class coexistence well - formedness ( P3 ) are load - bearing premises this CH - 2 proof grounds against ( CH - 1 root per the sec - 11 . 1 Per - CH dependency chain table )
-- - DP - 2 ( ` is_rc_dec_unnecessary ` ) is the canonical - state truth - table predicate discharged at aims - proof / proofs / 5 - decisions / DP - 2 . proof : is_rc_dec_unnecessary ( s ) iff s . cardinality = Absent or s . consumption = Dead ( post - CN - 1 canonical states ; the disjunction is bidirectionally implied )
-- - DP - 3 ( ` is_rc_inc_elidable ` ) is the canonical - state truth - table predicate discharged at aims - proof / proofs / 5 - decisions / DP - 3 . proof : is_rc_inc_elidable ( s ) iff s . cardinality = Once and s . consumption = Linear ( canonical states ; moved - once , no inc required )
-- - Predicate - stack - derived ops are the sec - 3 - sec - 4 lattice - only emission path operating purely on L without consulting BR per Handshake . proof Function 2 ( ` predicate_stack_path ` ) ; realization rules per Annex E section AIMS RL - 1 . . RL - 34
-- - consume_stack ( F , BR , L ) is the sec - 4 A . 2 elimination consumer per Handshake . proof Function 1 ( ` burden_emission_path ` ) , terminating at ` pub ( crate ) fn eliminate_burden_ops ` at compiler / ori_arc / src / aims / realize / burden_elim . rs : 87
-- - depends - on sec - 2 L - 6 ( monotonicity ) at aims - proof / proofs / 2 - lattice / L - 6 . proof emdash backward transfer functions SHALL be monotone ( a <= b implies f ( a ) <= f ( b ) ) ; monotonicity is the substrate for proving that adding the burden - registry pre - pass writes does NOT race - invalidate the lattice ' s converged L per CH - 4 ( which CH - 2 reduces to as the acyclic - dependency precondition )
-- - depends - on sec - 7 PL - 2 ( Step 4 precedes Step 5 ) at aims - proof / proofs / 7 - pipeline / PL - 2 . proof emdash analyze_function ( Step 4 ) precedes realize_rc_reuse ( Step 5 ) ; AimsStateMap converges BEFORE emit_burden_ops invocation per docs / ori_lang / proposals / approved / aims - burden - tracking - proposal . md sec - 4 A success_criterion 1 ; therefore DP - 2 / DP - 3 evaluations against L ' s converged state are read - only at consume_stack invocation time
-- - depends - on sec - 8 RL - 1 + RL - 2 composition at aims - proof / proofs / 8 - realization / RL - 1 - RL - 2 - composition . proof emdash the lattice - only emission path ' s RC inc / dec emission is RC - count - preserving ( every emitted inc balanced by an emitted dec at last use ) ; predicate_stack_path inherits this composition
-- Soundness property (verbatim from canonical-notation source):
-- Forall ArcFunction F . Forall BurdenSpec BR . Forall AimsStateMap L ( converged ) .
-- Forall variable v in vars ( F ) . Forall program point pp .
--   ( P1 ) Single - elimination decision per ( v , pp ) :
--     consume_stack emits EXACTLY ONE elimination decision for v at pp :
--       eliminate ( v , pp ) =
--         BR . burden_emitted [ v ]
--         and ( is_rc_dec_unnecessary ( L [ v ] @ pp ) or is_rc_inc_elidable ( L [ v ] @ pp ) )
--     The burden - derived elimination decision AND the lattice - derived
--     elimination decision are THE SAME Boolean for each ( v , pp ) pair ;
--     the burden registry ' s per - variable annotation IS computed from the
--     same DP - 2 / DP - 3 truth - table evaluations that the lattice consumer
--     applies at Step 5 / 5 a / 10 burden - elimination sites .
--   ( P2 ) No race - invalidation :
--     Predicate - stack ops ( the sec - 3 - sec - 4 lattice - only emission path operating
--     on L ) do NOT race - invalidate the lattice ' s DP - 2 / DP - 3 elimination
--     claim . Formally :
--       Forall predicate - stack op pi ( RC inc / RC dec / Reset / Reuse / IsShared /
--                               Set / SetTag emitted from L without BR ) .
--       Forall burden op beta ( RC inc / RC dec emitted from BR ) .
--         predicate_stack_path ( F , L ) o burden_emission_path ( F , L , BR ) equivalent_to
--         burden_emission_path ( F , L , BR ) o predicate_stack_path ( F , L )
--     Composition is commutative because pi and beta operate on the same L
--     ( read - only after IA - 7 ) with the SAME elimination decision per ( v , pp )
--     per ( P1 ) ; the union of emitted ops is a SET ( idempotent under
--     duplication ) , so consume_stack ' s emission is deterministic regardless
--     of ordering between burden - derived and lattice - derived emissions .
--   ( P3 ) Stack consumer well - formedness :
--     For each variable v with active stack burden ( i . e . , v in BR . burden_emitted
--     AND BR . eliminates ( v ) under DP - 2 / DP - 3 ) :
--       consume_stack honors the single - elimination invariant by reading
--       BR ' s per - variable annotation AS the lattice - derived DP - 2 / DP - 3 verdict
--       ( not re - deriving it ) ; the burden - tracking pre - pass ( sec - 4 A . 2 )
--       materializes the verdict into BR . burden_emitted , which consume_stack
--       consumes verbatim .
-- Proof obligation (verbatim from canonical-notation source):
-- Three - part constructive discharge , mirroring CH - 1 ' s three - part shape
-- ( Parts P1 / P2 / P3 ) ; composition of CH - 1 ' s lattice - bridge + acyclic
-- dependency premises with DP - 2 / DP - 3 truth - table consequence .
-- Part ( P1 ) emdash single - elimination decision :
--   Per CH - 1 Part ( P1 ) lattice - bridge consistency , BR . burden_emitted [ v ] is
--   derived from a pure function on L ' s converged AimsStateMap ; equivalently :
--     BR . burden_emitted [ v ] iff burden - owned ( L [ v ] ) per Handshake . proof Predicate 1
--   Per CH - 1 Part ( P1 ) coverage gate , the lattice - bridge predicate
--   burden - owned ( s ) implies ( is_rc_dec_unnecessary ( s ) or is_rc_inc_elidable ( s ) )
--   on canonical states post - CN - 1 . . CN - 8 , because :
--     - burden - owned ( s ) implies s . access = Owned and s . consumption in { Linear , Affine }
--       and s . uniqueness = Unique and is_rc_dec_unnecessary ( s )
--     - The fourth conjunct IS is_rc_dec_unnecessary ( s ) ; therefore the
--       disjunction holds trivially via the right disjunct
--     - When the third conjunct holds ( Unique ) AND consumption = Linear ,
--       DP - 3 fires ( Linear and Once on canonical states ; Linear demands at
--       single use site reach cardinality Once ) ; the disjunction holds via
--       the left disjunct as well
--   Therefore eliminate ( v , pp ) computed from BR is bit - identical to
--   eliminate ( v , pp ) computed from L ' s DP - 2 / DP - 3 truth tables ; the burden -
--   derived AND lattice - derived elimination decisions are THE SAME Boolean ,
--   not two decisions stacked .
--   Conclusion ( P1 ) : single - elimination decision per ( v , pp ) holds by
--   construction ; consume_stack reads BR . burden_emitted [ v ] ( which is a
--   memoized DP - 2 / DP - 3 verdict on L [ v ] ) instead of re - evaluating DP - 2 / DP - 3
--   on L [ v ] ; the verdict is bit - identical either way .
-- Part ( P2 ) emdash no race - invalidation ( composition commutativity ) :
--   Per CH - 1 Part ( P2 ) no - double - counting , the dependency direction is
--   acyclic : BR reads L ( after IA - 7 convergence ) ; L does NOT read BR . Per
--   CH - 4 ( AimsStateMap immutability under burden - registry mutation ; the sec - 11
--   success_criterion 12 depends - on sec - 2 L - 2 lattice algebra state - map
--   immutability under read - only access ) , Forall mutation event M of BR . L is
--   NOT modified by M .
--   Given L is read - only after IA - 7 convergence , predicate_stack_path ( F , L )
--   ( which reads L only ) and burden_emission_path ( F , L , BR ) ( which reads L
--   + BR , where BR was derived from L ) are both PURE FUNCTIONS of L ( BR is
--   a memoized derivation per CH - 1 Part P1 ) . Pure functions of the same
--   input commute trivially under composition :
--     predicate_stack_path ( F , L ) o burden_emission_path ( F , L , BR )
--       = predicate_stack_path ( F , L ) o burden_emission_path ( F , L , BR ( L ) )
--       = ( read - only L , deterministic output ops )
--       = burden_emission_path ( F , L , BR ( L ) ) o predicate_stack_path ( F , L )
--   The union of emitted ops is a SET ( each ( v , pp , op_kind ) triple emitted
--   at most once per L per the single - elimination invariant from Part P1 ) ,
--   so consume_stack ' s final emission is deterministic regardless of which
--   path runs first .
--   Per L - 6 monotonicity , backward transfer functions in sec - 3 IA - 5 step ( 1 )
--   preserve the conjunction burden - owned ( s ) under alias transfer ; therefore
--   the predicate_stack_path ' s RC emissions are consistent with the burden_
--   emission_path ' s RC emissions on every variable for which BR . burden_
--   emitted [ v ] = true .
--   Conclusion ( P2 ) : no race - invalidation ; the two emission paths commute
--   under composition ; consume_stack emits a deterministic single union
--   of RC ops regardless of which path runs first .
-- Part ( P3 ) emdash stack consumer well - formedness :
--   Per CH - 1 Part ( P3 ) per - class coexistence well - formedness , the AIMS
--   class taxonomy is total + disjoint over the three sub - classes ( A : Owned
--   x Linear x Once x Unique ; B : Borrowed x Linear x Once x Unique ; C :
--   MaybeShared x Many ) . The burden registry classifies a variable v as
--   burden_emitted ONLY when v ' s converged L [ v ] satisfies the burden - owned
--   conjunction ( Class A per the sec - 11 success_criterion 12 IC - 3 binding ) .
--   Therefore consume_stack ' s stack - consumer well - formedness reduces to :
--   for each v in BR . burden_emitted , consume_stack treats BR ' s per - variable
--   annotation AS the lattice - derived DP - 2 / DP - 3 verdict , NOT a second
--   independent elimination claim . The burden - tracking pre - pass ( sec - 4 A . 2 )
--   materializes the lattice ' s DP - 2 / DP - 3 verdict into BR . burden_emitted as
--   a typed - side - table memoization per Annex E section AIMS . 7 EffectSummary +
--   arc . md sec - Non - Negotiable - Invariants invariant 5 ( c ) " feed the lattice - driven
--   analysis as a typed pre - pass input that lands on AimsStateMap ( as
--   immortal detection does via the immortals : Vec < bool > bitvector ) " .
--   Conclusion ( P3 ) : stack consumer well - formedness holds ; the burden
--   registry materializes ( not re - derives ) the lattice ' s DP - 2 / DP - 3 verdict ;
--   consume_stack honors the single - elimination invariant .
-- Coverage gate : the three Parts ( P1 , P2 , P3 ) together discharge the
-- soundness - property clauses ( single - elimination decision , no race -
-- invalidation , stack consumer well - formedness ) per the sec - 1
-- Composition . proof : 38 sorry obligation . A regression dropping any
-- Part leaves a soundness - property clause unverified emdash the joined CH - 2
-- claim is no stronger than its weakest part .
-- Engines dispatched :
--   structural_induction ( PRIMARY emdash per the sec - 11 . 0 Per - CH Proof - Status
--     Tracking table CH - 2 row + Composition . proof : 59 - 63 skeleton dispatch ;
--     per - instruction structural - check soundness over ( P1 ) single -
--     elimination decision + ( P2 ) composition commutativity + ( P3 ) stack
--     consumer well - formedness )
--   interprocedural_summary ( CO - PRIMARY emdash per Composition . proof : 59 - 63 ;
--     SCC - level burden - registry - vs - lattice agreement under IC - 3 + IC - 4
--     contract dimensions ; load - bearing for class - identification under
--     callee return contracts )
--   case_analysis ( CO - PRIMARY emdash DP - 2 + DP - 3 canonical - state truth - table
--     enumeration per Appendix C ; per - class - row enumeration in ( P3 ) )
--   lattice ( CO - PRIMARY emdash L - 6 monotonicity for burden - owned preservation
--     under backward transfer ; L - 7 canonicalization idempotence for
--     repeated burden - registry reads )
-- TODO(BUG-XX-NNN): substantive translation pending Mathlib AimsState model;
-- placeholder body is `True := by trivial` so Lean 4's parser accepts
-- the file. Semantic equivalence to the Ori claim is out of scope for
-- the §01A bootstrap per the SMT / Lean 4 emission strategy Option C consequences.
theorem CH_2_DP_2_DP_3_elimination_consumer_composition_with_predicate_stack_derived_ops : True := by trivial

end AimsBootstrap

def main : IO Unit := pure ()

-- ============================================================
-- CH-3 (auto-included from proofs/11-coexistence/lean4-emitted/CH-3.lean)
-- ============================================================
-- AIMS-Proof bootstrap translation
-- SSOT: aims-proof/checker/src/emit/lean4.rs (the SMT / Lean 4 emission strategy Option C)
-- Constructive-by-default per the foundational-axiom policy; classical escalation
-- requires matched commit per the foundational-axiom policy §Permitted Extensions.
-- BANNED: Classical.em, Classical.choice, funext, propext, proof
-- irrelevance, Markov's Principle — absent a matched extension entry
-- in the foundational-axiom policy.
-- Translated from proofs/11-coexistence/CH-3.proof

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

-- Translated from proofs/11-coexistence/CH-3.proof:CH-3
-- Theorem name (verbatim from canonical-notation source):
-- Per - class coexistence with active stack burden emdash three sub - classes
-- Preconditions (verbatim from canonical-notation source):
-- - depends - on CH - 1 ( Burden - registry - lattice composition soundness ) at aims - proof / proofs / 11 - coexistence / CH - 1 . proof emdash CH - 1 ' s Part ( P3 ) per - class coexistence well - formedness skeleton is the partition - discharge premise CH - 3 expands here in detail ( CH - 1 cites the partition ' s well - formedness as a precondition for the lattice - bridge consistency ; CH - 3 discharges the partition itself )
-- - Three AIMS variable classes per sec - 11 . 0 Per - CH Proof - Status Tracking table CH - 3 row + the sec - 11 success_criterion 12 IC - 3 binding : sub - class A : Owned x Linear x Once x Unique emdash RL - 2 + RL - 14 candidate ( burden - emission - eligible ; last - use dec + headerless stack promotion ) sub - class B : Borrowed x Linear x Once x Unique emdash RL - 14 headerless candidate ( no RC emission ; borrowed locality clamped to FunctionLocal per CN - 8 ) sub - class C : MaybeShared x Many emdash RL - 7 dynamic COW candidate ( IsShared runtime check ; alloc - and - copy slow path )
-- - depends - on sec - 6 IC - 3 ParamContract at aims - proof / proofs / 6 - interprocedural / IC - 3 - param - join . proof emdash class identification per callee - parameter contract dimensions ( Access , Consumption , Cardinality , Uniqueness , Locality , may_share ) ; the burden registry ' s class_covered annotation consumes the IC - 3 contract dimensions to classify a variable ' s AIMS class
-- - depends - on sec - 2 L - 9 ( SCALAR exclusion ) at aims - proof / proofs / 2 - lattice / L - 9 . proof emdash SCALAR is a sentinel , NOT a lattice element ; analysis excludes scalars from state map ; CH - 3 ' s class partition operates only on non - SCALAR variables
-- - depends - on sec - 2 L - 5 ( finite height ) at aims - proof / proofs / 2 - lattice / L - 5 . proof emdash every lattice dimension has bounded height ; the ( Access x Uniqueness x Cardinality x Consumption ) sub - product has finite cardinality ( 2 x 3 x 3 x 4 = 72 raw rows ; CN - 1 . . CN - 8 prune to canonical subset ) , enabling exhaustive case - analysis enumeration
-- - depends - on sec - 2 L - 7 ( canonicalization idempotence ) at aims - proof / proofs / 2 - lattice / L - 7 . proof emdash class membership predicates are computed on canonical states post - CN - 1 . . CN - 8 ; repeated class - membership queries are stable
-- - depends - on sec - 8 RL - 2 ( last - use dec ) at aims - proof / proofs / 8 - realization / RL - 2 . proof emdash RC dec at last use or scope exit ; sub - class A ' s realization invariant
-- - depends - on sec - 8 RL - 14 ( headerless stack promotion ) at aims - proof / proofs / 8 - realization / RL - 14 . proof emdash non - escaping allocations with Locality <= FunctionLocal and Uniqueness = Unique SHALL be stack - allocated via alloca with no RC header ; sub - classes A + B ' s realization invariant
-- - depends - on sec - 8 RL - 7 ( dynamic COW ) at aims - proof / proofs / 8 - realization / RL - 7 . proof emdash MaybeShared values emit IsShared check + branch to in - place / copy ; sub - class C ' s realization invariant
-- - depends - on CN - 8 ( Borrowed locality ceiling ) at aims - proof / proofs / 3 - canonicalization / CN - 8 . proof emdash Access = Borrowed and Locality > FunctionLocal implies Locality := FunctionLocal ; sub - class B ' s locality is canonically bounded
-- - depends - on CN - 6 ( wide - locality uniqueness ceiling ) at aims - proof / proofs / 3 - canonicalization / CN - 6 . proof emdash Locality >= HeapEscaping and Uniqueness = Unique implies Uniqueness := MaybeShared ; sub - class C variables with HeapEscaping locality are automatically demoted to MaybeShared ( the class C entry condition )
-- Soundness property (verbatim from canonical-notation source):
-- Forall ArcFunction F . Forall AimsStateMap L ( converged ) . Forall variable v in vars ( F )
-- with L [ v ] ! = SCALAR .
--   ( P1 ) Total partition : every non - SCALAR variable v belongs to exactly
--     one of the three sub - classes :
--       class_of ( v ) = A iff L [ v ] . access = Owned
--                       and L [ v ] . uniqueness = Unique
--                       and L [ v ] . cardinality = Once
--                       and L [ v ] . consumption = Linear
--       class_of ( v ) = B iff L [ v ] . access = Borrowed
--                       and L [ v ] . uniqueness = Unique
--                       and L [ v ] . cardinality = Once
--                       and L [ v ] . consumption = Linear
--       class_of ( v ) = C iff L [ v ] . uniqueness = MaybeShared
--                       and L [ v ] . cardinality = Many
--     Variables that do not match any of A / B / C are non - burden -
--     eligible ( the burden registry ' s class_covered annotation excludes
--     them ) ; the three named classes carve out the burden - relevant subset
--     of the lattice state space .
--   ( P2 ) Disjointness : Forall v in vars ( F ) with L [ v ] ! = SCALAR .
--     not ( class_of ( v ) = A and class_of ( v ) = B )
--     and not ( class_of ( v ) = A and class_of ( v ) = C )
--     and not ( class_of ( v ) = B and class_of ( v ) = C )
--     The three sub - classes are pairwise disjoint via mutually - exclusive
--     lattice - dimension witnesses ( A disjoint B via Access ; A disjoint C via Uniqueness
--     + Cardinality ; B disjoint C via Uniqueness + Cardinality ) .
--   ( P3 ) Per - class realization invariant + burden agreement : for each
--     sub - class , the lattice ' s claim AND the burden registry ' s annotation
--     agree :
--       class_of ( v ) = A implies consume_stack honors RL - 2 last - use dec AND
--                           RL - 14 stack - promotion field - level RcDec ;
--                           burden registry annotation MUST agree
--                           ( BR . burden_emitted [ v ] = true )
--       class_of ( v ) = B implies consume_stack honors RL - 14 headerless invariant
--                           ( no RC emission ) ; burden registry annotation
--                           MUST agree ( BR . burden_emitted [ v ] = false emdash
--                           Borrowed values never enter burden_emission_path )
--       class_of ( v ) = C implies consume_stack honors RL - 7 dynamic COW slow - path
--                           ( IsShared runtime check ) ; burden registry
--                           annotation MUST flag MaybeShared ( BR . burden_
--                           emitted [ v ] = false emdash MaybeShared values are
--                           outside the burden - owned conjunction per
--                           Handshake . proof Predicate 1 )
-- Proof obligation (verbatim from canonical-notation source):
-- Three - part constructive discharge via case_analysis ( PRIMARY engine )
-- enumerating canonical states post - CN - 1 . . CN - 8 in Appendix B + Appendix C
-- of Annex E section AIMS , mirroring the sec - 9 VF - 3 oracle re - derivation shape .
-- Part ( P1 ) emdash total partition :
--   Per L - 5 finite height , the ( Access x Uniqueness x Cardinality x
--   Consumption ) sub - product has finite cardinality :
--     | Access | = 2 ( Borrowed , Owned )
--     | Uniqueness | = 3 ( Unique , MaybeShared , Shared )
--     | Cardinality | = 3 ( Absent , Once , Many )
--     | Consumption | = 4 ( Dead , Linear , Affine , Unrestricted )
--     Raw product = 2 x 3 x 3 x 4 = 72 rows .
--   Per CN - 1 ( Dead iff Absent ) , the ( Dead , ! = Absent ) and ( ! = Dead , Absent ) rows
--   collapse to ( Dead , Absent ) , reducing 24 raw rows to 6 canonical rows
--   with Consumption = Dead and Cardinality = Absent ( one per Access x
--   Uniqueness combination ) . The non - Dead non - Absent rows total
--   2 x 3 x 2 x 3 = 36 .
--   Per CN - 3 ( Shared implies NonReusable ) , Shared is preserved on the input but
--   Shape is forced NonReusable ; Shape is orthogonal to the ( Access x
--   Uniqueness x Cardinality x Consumption ) sub - product , so CN - 3 does NOT
--   reduce row count for this enumeration .
--   Per CN - 6 ( HeapEscaping and Unique implies MaybeShared ) , variables with wide
--   locality demote Unique - > MaybeShared in their lattice state ; the demoted
--   states naturally enter sub - class C ( Uniqueness = MaybeShared ) .
--   Per CN - 8 ( Borrowed and Locality > FunctionLocal implies FunctionLocal ) , sub -
--   class B ' s locality is canonically bounded to FunctionLocal ; sub - class
--   B never coexists with wide locality .
--   Class A row exactly matches the ( Owned , Unique , Once , Linear ) cell of
--   the canonical sub - product :
--     Owned x Unique x Once x Linear = exactly 1 canonical row , satisfies
--     burden - owned per Handshake . proof Predicate 1 .
--   Class B row exactly matches the ( Borrowed , Unique , Once , Linear ) cell :
--     Borrowed x Unique x Once x Linear = exactly 1 canonical row , fails
--     burden - owned ' s first conjunct ( Access = Owned ) , excluded from
--     burden_emission_path .
--   Class C row matches the ( * , MaybeShared , Many , * ) slice :
--     any Access x MaybeShared x Many x any Consumption = 2 x 1 x 1 x 4 = 8
--     canonical rows , fail burden - owned ' s third conjunct ( Uniqueness =
--     Unique ) , excluded from burden_emission_path .
--   Conclusion ( P1 ) : every non - SCALAR variable ' s canonical L [ v ] falls into
--   exactly one of three carved subsets of the sub - product . Burden
--   registry ' s class_covered predicate identifies sub - class A as the
--   burden - eligible class ; sub - classes B + C are non - burden - eligible by
--   construction .
-- Part ( P2 ) emdash disjointness :
--   Pairwise disjointness reduces to per - dimension witness contradictions :
--     A disjoint B : L [ v ] . access = Owned ! = Borrowed = L [ v ] . access ( Access dim
--     witness contradiction ; canonical states have a single Access value
--     per L - 1 idempotence of join ) .
--     A disjoint C : L [ v ] . uniqueness = Unique ! = MaybeShared = L [ v ] . uniqueness
--     ( Uniqueness dim witness contradiction ; per L - 1 + L - 9 , scalars are
--     excluded and remaining Uniqueness values are pairwise distinct ) .
--     Additional witness : L [ v ] . cardinality = Once ! = Many = L [ v ] . cardinality
--     ( Cardinality dim ) .
--     B disjoint C : L [ v ] . uniqueness = Unique ! = MaybeShared ( Uniqueness dim ) ;
--     L [ v ] . cardinality = Once ! = Many ( Cardinality dim ) . Two witnesses
--     amplify confidence ; either alone suffices .
--   Per L - 9 SCALAR exclusion , all non - class - eligible variables ( variables
--   whose L [ v ] does not match A / B / C ) are either SCALAR ( excluded from
--   analysis per sec - 6 IA - 1 ) or fall into the non - burden - eligible complement
--   ( Shared , Dead , partial states , etc . ) emdash outside CH - 3 ' s discharge scope .
--   Conclusion ( P2 ) : the three sub - classes are pairwise disjoint at the
--   lattice - dimension level ; no variable v with non - SCALAR L [ v ] can satisfy
--   two distinct class membership predicates simultaneously .
-- Part ( P3 ) emdash per - class realization invariant + burden agreement :
--   Per CH - 1 Part ( P3 ) per - class coexistence well - formedness , the burden
--   registry ' s class_covered annotation IS derived from the same lattice -
--   bridge predicate ( burden - owned per Handshake . proof Predicate 1 ) that
--   identifies sub - class A . Therefore burden agreement is automatic for
--   classes A / B / C :
--     Class A : burden - owned holds ( the full conjunction satisfied ) ;
--       BR . burden_emitted [ v ] = true per the lattice - bridge consistency
--       ( CH - 1 P1 ) ; consume_stack reads BR ' s annotation and emits the
--       single elimination decision per CH - 2 ( P1 ) . RL - 2 last - use dec is
--       emitted from L ' s converged state per sec - 8 RL - 2 . proof ; RL - 14
--       stack - promotion is selected based on Locality <= FunctionLocal
--       ( per DP - 8 is_local + RL - 14 candidate predicate ) emdash Class A is
--       a CANDIDATE for RL - 14 ( the candidate is realized only when the
--       Locality <= FunctionLocal precondition additionally holds , which
--       is independent of the ( Access x Uniqueness x Cardinality x
--       Consumption ) sub - product ) . When realized , RL - 14 emits field - level
--       RcDec for Owned reference - type fields in reverse declaration order
--       at scope exit per RL - 14 ' s heap - children rule . burden - emission and
--       predicate - stack - derived ops both honor this invariant ( per CH - 2 P2
--       composition commutativity ) .
--     Class B : burden - owned fails on the Access = Owned conjunct ( L [ v ] .
--       access = Borrowed ) ; BR . burden_emitted [ v ] = false ( the burden walk
--       skips Borrowed variables per the shipped burden_lower . rs
--       DerivedOwnership side - table consumption per
--       docs / ori_lang / proposals / approved / aims - burden - tracking - proposal . md
--       sec - 4 A success_criterion 4 ) ; RL - 14 headerless invariant holds ( no
--       RC emission ) ; consume_stack does NOT enter burden_emission_path
--       for Class B variables .
--     Class C : burden - owned fails on the Uniqueness = Unique conjunct
--       ( L [ v ] . uniqueness = MaybeShared ) ; BR . burden_emitted [ v ] = false ;
--       RL - 7 dynamic COW IsShared runtime check is emitted per sec - 8
--       RL - 7 . proof ; consume_stack does NOT enter burden_emission_path
--       for Class C variables . Burden elimination would race - invalidate
--       the runtime check ( the IsShared check reads the RC header field ,
--       which burden elimination assumes is unnecessary emdash a contradiction ) .
--   Conclusion ( P3 ) : per - class realization invariant + burden agreement
--   holds across all three sub - classes . Class A is the burden - emission -
--   eligible class ; Classes B + C are non - burden - eligible by lattice - bridge
--   predicate exclusion ( CH - 1 P1 inherited ) ; consume_stack ' s per - class
--   emission honors the lattice ' s realization invariants AND the burden
--   registry ' s annotation simultaneously .
-- Coverage gate : the three Parts ( P1 , P2 , P3 ) together discharge the
-- partition ' s totality + disjointness invariant + per - class realization
-- invariant per the sec - 1 Composition . proof : 69 sorry obligation . A regression
-- dropping any Part leaves the partition unverified emdash the joined CH - 3
-- claim is no stronger than its weakest part .
-- Engines dispatched :
--   structural_induction ( CO - PRIMARY emdash per the sec - 11 . 0 Per - CH Proof - Status
--     Tracking table CH - 3 row + Composition . proof : 93 - 97 skeleton dispatch ;
--     per - instruction structural - check soundness over ( P3 ) per - class
--     realization invariant emission )
--   interprocedural_summary ( CO - PRIMARY emdash per Composition . proof : 93 - 97 ;
--     SCC - level class identification under IC - 3 ParamContract dimensions )
--   case_analysis ( PRIMARY emdash per the sec - 11 . 0 table CH - 3 row + the partition ' s
--     canonical - state enumeration in Part ( P1 ) over Appendix B + Appendix
--     C truth tables ; pairwise disjointness in ( P2 ) via lattice - dimension
--     witness contradictions )
--   lattice ( CO - PRIMARY emdash L - 5 finite height for the sub - product
--     enumeration ; L - 1 join idempotence for the canonical - state witness
--     argument ; L - 9 SCALAR exclusion gating the analysis subspace )
-- TODO(BUG-XX-NNN): substantive translation pending Mathlib AimsState model;
-- placeholder body is `True := by trivial` so Lean 4's parser accepts
-- the file. Semantic equivalence to the Ori claim is out of scope for
-- the §01A bootstrap per the SMT / Lean 4 emission strategy Option C consequences.
theorem CH_3_Per_class_coexistence_with_active_stack_burden_emdash_three_sub_classes : True := by trivial

end AimsBootstrap

def main : IO Unit := pure ()

-- ============================================================
-- CH-4 (auto-included from proofs/11-coexistence/lean4-emitted/CH-4.lean)
-- ============================================================
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

-- ============================================================
-- CH-5 (auto-included from proofs/11-coexistence/lean4-emitted/CH-5.lean)
-- ============================================================
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

-- ============================================================
-- CH-comp (auto-included from proofs/11-coexistence/lean4-emitted/CH-comp.lean)
-- ============================================================
-- AIMS-Proof bootstrap translation
-- SSOT: aims-proof/checker/src/emit/lean4.rs (the SMT / Lean 4 emission strategy Option C)
-- Constructive-by-default per the foundational-axiom policy; classical escalation
-- requires matched commit per the foundational-axiom policy §Permitted Extensions.
-- BANNED: Classical.em, Classical.choice, funext, propext, proof
-- irrelevance, Markov's Principle — absent a matched extension entry
-- in the foundational-axiom policy.
-- Translated from proofs/11-coexistence/CH-comp.proof

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

-- Translated from proofs/11-coexistence/CH-comp.proof:CH-comp
-- Theorem name (verbatim from canonical-notation source):
-- layered - handshake composition emdash the handshake catches the UNION of CH - 1 . . CH - 5 failure classes
-- Preconditions (verbatim from canonical-notation source):
-- - This is the sec - 11 composition theorem per the sec - 11 success_criterion 4 + sec - 11 . 0 Per - CH Proof - Status Tracking table CH - comp row : the layered coexistence handshake catches the UNION of the failure classes its constituent CH - N theorems catch ; a fix passing one CH but regressing another is a correctness regression , NOT a partial win ( per Annex E section AIMS + arc . md Verification Surface : " a fix that passes one layer but regresses another is a correctness regression " ) .
-- - Mirror precedent : aims - proof / proofs / 9 - verification / VF - comp . proof emdash the sec - 9 layered - verifier composition theorem the sec - 11 CH - comp pattern mirrors ( union - coverage shape ; a fix passing a strict subset of layers is rejected ; coverage gate prevents a dropped layer ) . Per sec - 11 success_criterion 4 , CH - comp ' s proof shape mirrors VF - comp ' s structural_ induction over the constituent enumeration .
-- - Each constituent CH - N is proved by its own theorem : CH - 1 . proof emdash Burden - registry - lattice composition soundness ( root ) CH - 2 . proof emdash DP - 2 / DP - 3 elimination consumer composition ( depends - on CH - 1 ) CH - 3 . proof emdash Per - class coexistence three sub - classes ( depends - on CH - 1 ) CH - 4 . proof emdash AimsStateMap immutability under BR mutation ( depends - on CH - 1 ) CH - 5 . proof emdash Phase - ordering composition ( depends - on CH - 4 ; transitively CH - 1 )
-- - depends - on every CH - N ( the constituent coexistence - handshake layers ) : CH - 1 ( lattice - bridge + acyclic dependency ) , CH - 2 ( single - elimination decision + composition commutativity ) , CH - 3 ( class partition totality + disjointness + per - class realization ) , CH - 4 ( AimsStateMap immutability ) , CH - 5 ( phase - ordering composition ) .
-- - Signature : ArcFunction x AimsStateMap x BurdenSpec x class C - > ArcFunction ( the whole - handshake verdict per Handshake . proof Function 3 ` coexistence_dispatch ` ) .
-- Soundness property (verbatim from canonical-notation source):
-- Forall ArcFunction F . Forall AimsStateMap L ( converged ) . Forall BurdenSpec BR . Forall class C .
--   ( P1 ) Joined union : the layered handshake ACCEPTS ( F , L , BR , C ) iff
--     EVERY constituent layer ( CH - 1 . . CH - 5 ) accepts . The handshake catches
--     the UNION of the per - layer failure classes :
--       CH - 1 catches : lattice - bridge inconsistency , double - counting ,
--                     per - class non - well - formedness
--       CH - 2 catches : multi - elimination per ( v , pp ) , race - invalidation ,
--                     stack - consumer ill - formedness
--       CH - 3 catches : non - total partition , class overlap , per - class
--                     realization invariant violation
--       CH - 4 catches : AimsStateMap mutation under BR , canonicalization
--                     loss , block - boundary map drift
--       CH - 5 catches : PL - 1 reordering , BR - reads - L cycle , PL - 5 staleness ,
--                     PL - 6 meta - rule violation
--     A fix passing a strict SUBSET of CH - N ( e . g . , passes CH - 1 , CH - 2 , CH - 3 ,
--     CH - 4 but regresses CH - 5 by inadvertently reordering PL - 1 vs Step 4 -
--     companion ) is REJECTED by the handshake , because some CH - N in the
--     union catches the regressed class .
--   ( P2 ) Conjunction strength : CH - comp is EXACTLY as strong as the
--     conjunction of its constituents emdash it holds iff every constituent
--     CH - N discharges . It never gracious - accepts over a failing or missing
--     premise . A dropped constituent leaves a failure class uncaught ( a
--     false - valid risk ) , so the coverage gate asserts the full 5 - layer
--     count .
--   ( P3 ) Per - case soundness witnessing emdash the case partition from
--     Handshake . proof ' s soundness theorem ( case 1 class_covered ; case 2
--     mixed - coverage ; case 3 uncovered ) is exhaustively covered by the
--     composition of CH - 1 . . CH - 5 :
--       case 1 ( class_covered ) : CH - 1 lattice - bridge + CH - 2 single -
--              elimination + CH - 3 partition + CH - 4 immutability + CH - 5
--              phase - ordering jointly establish coexistence_dispatch equivalent_to
--              burden_emission_path AND equivalent_to predicate_stack_path
--       case 2 ( mixed - coverage ) : CH - 3 ' s per - class disjointness combined
--              with TF - 4 ( Project ) field - grain inheritance per
--              Handshake . proof Predicate 2 mixed - coverage dispatch rule
--              establishes per - field dispatch composing via sec - 7 PL - 3
--              emission - order
--       case 3 ( uncovered ) : CH - 1 lattice - bridge predicate exclusion
--              ( burden - owned fails ) trivially reduces coexistence_dispatch
--              equivalent_to predicate_stack_path by definition
-- CH - comp is the capstone of the sec - 11 contribution to MS - 4 ( Ori - novel
-- coexistence - proof per 0 - overview . md MS - 4 ) : the proof that the AIMS
-- coexistence handshake , layer by layer and composed , catches every
-- inconsistency class emdash lattice - bridge consistency , single - elimination
-- determinism , per - class partition well - formedness , AimsStateMap
-- immutability under BR mutation , phase - ordering composition emdash and that
-- no fix passing one layer while regressing another slips through .
-- Proof obligation (verbatim from canonical-notation source):
-- Constructive discharge by the structural_induction engine via composition
-- of all 5 discharged CH constituents ( mirrors sec - 9 VF - comp ' s 8 - constituent
-- composition + sec - 8 RL - comp ' s whole - suite composition ; per the sec - 11
-- success_criterion 4 VF - comp mirror ) .
-- Part ( P1 / P2 ) emdash constituent composition :
--   The handshake re - runs each of the 5 CH constituent theorems ( CH - 1
--   through CH - 5 ) , asserts each returns Valid , AND models the union - of -
--   failure - classes property : a fix passing a strict subset of CH - N ( 4 of
--   5 , regressing CH - 5 ) is REJECTED by the whole - handshake verdict , and a
--   fix passing every CH - N is accepted .
--   Constituent re - run :
--     CH - 1 ( lattice - bridge consistency ) : re - run ; must return Valid . Catches
--       lattice - bridge inconsistency , double - counting , per - class non - well -
--       formedness in its 3 - part discharge .
--     CH - 2 ( single - elimination decision ) : re - run ; must return Valid .
--       Catches multi - elimination per ( v , pp ) , race - invalidation , stack -
--       consumer ill - formedness in its 3 - part discharge .
--     CH - 3 ( per - class partition ) : re - run ; must return Valid . Catches non -
--       total partition , class overlap , per - class realization invariant
--       violation in its 3 - part discharge .
--     CH - 4 ( AimsStateMap immutability ) : re - run ; must return Valid . Catches
--       per - variable AimsState mutation , canonicalization loss , block -
--       boundary map drift in its 3 - part discharge .
--     CH - 5 ( phase - ordering composition ) : re - run ; must return Valid .
--       Catches PL - 1 reordering , BR - reads - L cycle , PL - 5 staleness , PL - 6
--       meta - rule violation in its 4 - part discharge .
--   Coverage gate : exactly 5 CH constituents discharged . A failing or
--   missing constituent fails CH - comp ( the joined claim is no stronger
--   than its weakest premise ) .
--   Union - semantics check : a synthetic regression that flips CH - 5 ' s PL - 1
--   preservation to false ( a hypothetical implementation that reorders
--   interprocedural Steps 1 - 2 after Step 4 - companion emit_burden_ops )
--   causes CH - 5 to fail ; CH - comp ' s whole - handshake verdict rejects the
--   composition because the union catches the regression even though
--   CH - 1 . . CH - 4 still discharge .
-- Part ( P3 ) emdash case - partition exhaustive coverage :
--   Per Handshake . proof ' s soundness theorem ( the case 1 / case 2 / case 3
--   partition ) , the coexistence handshake ' s observable equivalence claim
--   decomposes into three cases . CH - comp proves each case is exhaustively
--   covered by composing CH - 1 . . CH - 5 :
--     case 1 ( class_covered = true ) : the burden - owned conjunction holds
--       for every variable in class C ; CH - 1 lattice - bridge consistency
--       establishes that BR . burden_emitted [ v ] = true iff burden - owned ( L [ v ] ) ;
--       CH - 2 single - elimination guarantees burden_emission_path AND
--       predicate_stack_path produce the same RC ops per ( v , pp ) ; CH - 3
--       per - class disjointness confirms class C contains only burden -
--       eligible members ( sub - class A ) ; CH - 4 AimsStateMap immutability
--       ensures L ' s state is stable across BR ' s mutation ; CH - 5 phase -
--       ordering composition ensures BR ( F ) is fresh when consumed .
--       Joint : coexistence_dispatch ( F , L , BR , C ) equivalent_to burden_emission_path ( F ,
--       L , BR ) AND equivalent_to predicate_stack_path ( F , L ) .
--     case 2 ( mixed - coverage ) : class C contains at least one transitive
--       payload that fails class_covered ; per Handshake . proof Predicate 2 ' s
--       mixed - coverage dispatch rule , per - field dispatch routes per - field
--       to burden_emission_path or predicate_stack_path . CH - 3 ' s per - class
--       partition disjointness ( over the ( Access x Uniqueness x Cardinality
--       x Consumption ) sub - product ) extends to per - field granularity via
--       TF - 4 Project ' s borrow - source propagation ( each Project inherits
--       source ' s uniqueness ; CN - 6 demotes wide - locality projections ) . CH - 2
--       single - elimination guarantees per - field decisions compose without
--       race ; CH - 4 immutability ensures field - grain state is stable .
--       Joint : coexistence_dispatch ( F , L , BR , C ) equivalent_to predicate_stack_path ( F ,
--       L ) under the per - field dispatch rule ( sec - 7 PL - 3 emission - order
--       composes per - field decisions ) .
--     case 3 ( uncovered = both class C AND all transitive payloads fail
--       class_covered ) : CH - 1 ' s lattice - bridge predicate excludes class C
--       from burden - emission entirely ; coexistence_dispatch routes
--       unconditionally to predicate_stack_path . CH - 2 . . CH - 5 are vacuously
--       satisfied for case 3 ( no burden_emission_path branch taken ) .
--       Joint : coexistence_dispatch ( F , L , BR , C ) equivalent_to predicate_stack_path ( F ,
--       L ) by definition .
--   Exhaustive coverage : the three cases partition the variable space
--   vars ( F ) per Handshake . proof ' s BurdenCovered disjoint_union MixedCoverage disjoint_union Uncovered
--   partition ( CH - 3 P1 + P2 discharge totality + disjointness at the
--   variable - space level ) ; the case - by - case soundness witnessing in ( P3 )
--   ensures every variable ' s coexistence_dispatch decision is observably
--   equivalent to the case - appropriate target per the soundness theorem .
-- Engines dispatched :
--   structural_induction ( PRIMARY emdash sec - 11 success_criterion 4 + VF - comp
--     precedent at aims - proof / proofs / 9 - verification / VF - comp . proof : 42 - 55 ;
--     constituent re - run + Valid - assertion + union - semantics check +
--     5 - count coverage gate ; emits Fail on any non - discharging constituent ,
--     a union - semantics violation , or a count mismatch )
--   interprocedural_summary ( SECONDARY emdash inherited from CH - 1 + CH - 2 + CH - 5
--     interprocedural premises ; SCC - level composition gracious - accept )
--   case_analysis ( CO - SECONDARY emdash case 1 / case 2 / case 3 partition
--     enumeration in Part ( P3 ) )
--   lattice ( CO - SECONDARY emdash L - 1 + L - 2 + L - 3 + L - 6 + L - 7 inherited from
--     CH - 1 . . CH - 5 lattice premises )
--   refinement ( SECONDARY emdash gracious - accept per the coverage - manifest CH
--     row ' s full 8 - engine spectrum )
--   rc_counting ( SECONDARY emdash gracious - accept ; inherited from CH - 1 ' s
--     RL - 2 / RL - 7 / RL - 14 realization - rule consequences )
--   monotonicity ( SECONDARY emdash gracious - accept ; L - 6 inherited from CH - 4 )
--   fixpoint ( SECONDARY emdash gracious - accept ; IA - 7 + IC - 7 inherited from
--     CH - 1 + CH - 4 )
-- TODO(BUG-XX-NNN): substantive translation pending Mathlib AimsState model;
-- placeholder body is `True := by trivial` so Lean 4's parser accepts
-- the file. Semantic equivalence to the Ori claim is out of scope for
-- the §01A bootstrap per the SMT / Lean 4 emission strategy Option C consequences.
theorem CH_comp_layered_handshake_composition_emdash_the_handshake_catches_the_UNION_of_CH_1_CH : True := by trivial

end AimsBootstrap

def main : IO Unit := pure ()
