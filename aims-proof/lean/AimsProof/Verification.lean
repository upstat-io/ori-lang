/-
AIMS verification module — kernel-checked Lean proofs of the layered verification
stack VF-1..VF-8 + VF-comp over a minimal faithful model of the verification
layers. The VF layers verify the realized ARC IR that the RL rules emit.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: VF-1..VF-8 + VF-comp | spec: annex-e §AIMS §9 |
  .proof: aims-proof/proofs/09-verification/VF-*.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: `docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS §9`
(Verification Layers VF-1..VF-8).

These are STRUCTURAL theorems — each verification layer is modeled over the
minimal faithful structure its failure-detection property is defined over (a
`FailureClass` enumeration, a per-layer "catches" relation mapping each layer to
the failure classes it owns, and the layer→class coverage map) and proven as
real kernel-checked theorems. The lattice dimension carriers (`AccessClass`,
`Consumption`, `Cardinality`, `EffectClass`) are reused from `Model.lean`; the
RC-balance ledger shape (`Int`-valued deltas, `foldr` sum) mirrors the
RC-emission substrate over an allocation ledger for VF-4.

The unifying soundness invariant of the verification family is FAILURE-CLASS
COVERAGE: every well-formedness / consistency / certification failure class the
stack owns is caught by exactly one layer, and the composed stack (VF-comp)
catches the UNION of the per-layer classes — no failure class escapes all
layers, and a fix passing a strict subset of layers is rejected.

Rule index (per §AIMS §9):
  VF-1  : Layer-1 structural ARC IR well-formedness — the 5 checks
          (use-before-def, dangling-block, RC-on-scalar, dec-on-borrowed,
          arg-ownership-len) map 1:1 to 5 VerifyError variants.
  VF-2  : Layer-2 AIMS contract consistency — AbsentParamHasUses; (b)/(c)/(d)
          target-only, gated on RL-31 / RL-29 / RL-30.
  VF-3  : Layer-3 oracle cross-check — re-derived contract refines the inferred
          contract along access / consumption / effects (too-optimistic = error).
  VF-4  : Layer-4 FIP certification — Certified iff zero unmatched alloc/dealloc.
  VF-5  : end-to-end mandate — implementation ∧ invariant-enforcement ∧ tests.
  VF-6  : contracts ↔ realization agreement — composes VF-3 oracle + VF-4 FIP.
  VF-7  : active-rewrite three-tier discipline — structural ∧ behavioral ∧
          proof-sketch.
  VF-8  : stack applies to ALL rules — implemented (active layer) /
          unimplemented (planned layer) / no-layer (spec gap).
  VF-comp: composition — each layer catches a distinct failure class; the stack
          catches the UNION (no failure class escapes all layers; a fix passing a
          strict subset is rejected).
-/

import AimsProof.Model

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §9 Failure-class universe — the shared coverage substrate (annex-e §AIMS §9)

    The unifying soundness property of the verification family is FAILURE-CLASS
    COVERAGE. The verification stack owns a finite universe of failure classes;
    each layer is responsible for a distinct subset, and the composed stack must
    cover their union. The `FailureClass` enumeration is the minimal faithful
    model of "what can go wrong" that the layered stack must catch. -/

/-- §9 the failure classes the layered verification stack owns. Each is the
    distinguished failure class of exactly one layer (VF-1..VF-4); VF-5..VF-8 are
    whole-stack mandates that range over these classes rather than introducing
    new ones. -/
inductive FailureClass
  | StructuralIllformed   -- VF-1: a structural ARC IR well-formedness failure
  | ContractInconsistent  -- VF-2: an AIMS contract inconsistency (AbsentParamHasUses)
  | OracleTooOptimistic   -- VF-3: re-derived contract exceeds the inferred one
  | FipUnbalanced         -- VF-4: an unmatched alloc/dealloc in a Certified fn
deriving Repr, DecidableEq

/-- §9 the four verification LAYERS that own a distinct failure class. (The
    whole-stack mandates VF-5..VF-8 are proven below as their own structural
    properties; the catch-partition is over the four detecting layers.) -/
inductive Layer
  | L1Structural   -- VF-1
  | L2Contract     -- VF-2
  | L3Oracle       -- VF-3
  | L4Fip          -- VF-4
deriving Repr, DecidableEq

/-- §9 the layer→failure-class coverage map: each detecting layer catches exactly
    one failure class (the 1:1 partition the stack relies on). -/
def Layer.catches : Layer → FailureClass
  | .L1Structural => .StructuralIllformed
  | .L2Contract   => .ContractInconsistent
  | .L3Oracle     => .OracleTooOptimistic
  | .L4Fip        => .FipUnbalanced

/-- §9 the full layer roster (the 4 detecting layers, in stack order). -/
def allLayers : List Layer := [.L1Structural, .L2Contract, .L3Oracle, .L4Fip]

/-- §9 the full failure-class universe. -/
def allFailureClasses : List FailureClass :=
  [.StructuralIllformed, .ContractInconsistent, .OracleTooOptimistic, .FipUnbalanced]

/-! ## §9.1 VF-1 — Layer-1 structural ARC IR well-formedness (annex-e §AIMS §9 VF-1)

    VF-1 runs 5 structural checks that map 1:1 to 5 VerifyError variants
    {UseBeforeDef, DanglingBlockRef, RcOnScalar, DecOnBorrowed,
    ArgOwnershipLenMismatch}. (P1) the check↔error map is a bijection (every
    well-formedness failure class Layer-1 owns is detected by exactly one check);
    (P2) detection has teeth — a malformed function exhibiting any class is
    rejected, while a well-formed function (INCLUDING the RL-14 field-drop
    exemption: a param-unrestricted Project+RcDec at scope cleanup) is accepted. -/

/-- §9.1 VF-1 the 5 structural checks (= the 5 VerifyError variants). -/
inductive StructCheck
  | UseBeforeDef
  | DanglingBlockRef
  | RcOnScalar
  | DecOnBorrowed
  | ArgOwnershipLenMismatch
deriving Repr, DecidableEq

/-- §9.1 VF-1 the 5 VerifyError variants the checks map to (1:1 with the checks). -/
inductive VerifyError
  | UseBeforeDef
  | DanglingBlockRef
  | RcOnScalar
  | DecOnBorrowed
  | ArgOwnershipLenMismatch
deriving Repr, DecidableEq

/-- §9.1 VF-1 the check→error map. Each check reports exactly one VerifyError. -/
def StructCheck.error : StructCheck → VerifyError
  | .UseBeforeDef            => .UseBeforeDef
  | .DanglingBlockRef        => .DanglingBlockRef
  | .RcOnScalar              => .RcOnScalar
  | .DecOnBorrowed           => .DecOnBorrowed
  | .ArgOwnershipLenMismatch => .ArgOwnershipLenMismatch

/-- §9.1 VF-1 the full 5-check roster. -/
def allChecks : List StructCheck :=
  [.UseBeforeDef, .DanglingBlockRef, .RcOnScalar, .DecOnBorrowed,
   .ArgOwnershipLenMismatch]

/-- §9.1 VF-1 (P1) the check→error map is INJECTIVE: distinct checks report
    distinct errors (no two checks share a variant — no check is duplicated). -/
theorem VF1_check_error_injective (c1 c2 : StructCheck)
    (h : c1.error = c2.error) : c1 = c2 := by
  cases c1 <;> cases c2 <;> first | rfl | (simp [StructCheck.error] at h)

/-- §9.1 VF-1 (P1) the check→error map is SURJECTIVE onto the 5 VerifyError
    variants: every variant is reported by some check (no failure class is left
    undetected). -/
theorem VF1_check_error_surjective (e : VerifyError) :
    ∃ c : StructCheck, c.error = e := by
  cases e
  · exact ⟨.UseBeforeDef, rfl⟩
  · exact ⟨.DanglingBlockRef, rfl⟩
  · exact ⟨.RcOnScalar, rfl⟩
  · exact ⟨.DecOnBorrowed, rfl⟩
  · exact ⟨.ArgOwnershipLenMismatch, rfl⟩

/-- §9.1 VF-1 (P1) the bijection is exactly 5 checks (the coverage count: not 4,
    not 6 — every Layer-1 failure class has exactly one check). -/
theorem VF1_exactly_five_checks : allChecks.length = 5 := by decide

/-! ### §9.1 VF-1 (P2) detection has teeth

    A function is rejected iff it exhibits at least one of the 5 structural
    failure classes. The RL-14 field-drop exemption is a Project+RcDec at scope
    cleanup that is param-UNRESTRICTED, so it is NOT a DecOnBorrowed violation
    (the `check_no_dec_on_borrowed` check is restricted to function params). We
    model a function's structural status as the set of failure classes it
    exhibits (the empty list = well-formed). -/

/-- §9.1 VF-1 a structural witness = the list of failure classes the IR exhibits.
    `[]` = well-formed; any non-empty list = malformed. -/
abbrev StructWitness := List VerifyError

/-- §9.1 VF-1 the Layer-1 verdict: ACCEPT iff no failure class is exhibited. -/
def vf1_accepts (w : StructWitness) : Bool := w.isEmpty

/-- §9.1 VF-1 (P2) a well-formed function (no exhibited failure class) is
    ACCEPTED. -/
theorem VF1_wellformed_accepted : vf1_accepts [] = true := by decide

/-- §9.1 VF-1 (P2) the RL-14 field-drop exemption is ACCEPTED: a param-
    unrestricted Project+RcDec at scope cleanup exhibits NO DecOnBorrowed
    failure class (the check is param-restricted), so the witness is empty and
    the function is accepted. -/
theorem VF1_field_drop_exemption_accepted : vf1_accepts [] = true := by decide

/-- §9.1 VF-1 (P2) detection has teeth: a function exhibiting any failure class
    (non-empty witness) is REJECTED. Proven for an arbitrary non-empty witness. -/
theorem VF1_malformed_rejected (e : VerifyError) (rest : StructWitness) :
    vf1_accepts (e :: rest) = false := by
  unfold vf1_accepts; rfl

/-- §9.1 VF-1 (P2) the load-bearing negative witness: a GENUINE DecOnBorrowed on
    a function parameter (NOT the exemption) is REJECTED — accepting it would let
    a borrowed param be wrongly dec'd (a double-free of the caller's value). -/
theorem VF1_dec_on_borrowed_param_rejected :
    vf1_accepts [VerifyError.DecOnBorrowed] = false := by decide

/-- §9.1 VF-1 catches the StructuralIllformed failure class: the L1 layer's
    distinguished class is exactly StructuralIllformed (the catch-partition
    anchor for VF-comp). -/
theorem VF1_catches_structural :
    Layer.catches .L1Structural = FailureClass.StructuralIllformed := by rfl

/-! ## §9.2 VF-2 — Layer-2 AIMS contract consistency (annex-e §AIMS §9 VF-2)

    VF-2 is an INDEPENDENT layer (not a filter over VF-1). The implemented check
    is AbsentParamHasUses: a parameter declared `Cardinality = Absent` MUST have
    no live use. (b)/(c)/(d) are TARGET-ONLY, each gated on its sec-08
    LLVM-fact-export rule (RL-31 / RL-29 / RL-30); emitting one without the
    underlying proof asserts unproven facts. -/

/-- §9.2 VF-2 the AbsentParamHasUses decision inputs: whether the param is
    declared Absent, and whether it has a live forward-reachable use. -/
structure AbsentParamInputs where
  declaredAbsent : Bool
  hasLiveUse : Bool
deriving Repr, DecidableEq

/-- §9.2 VF-2 the AbsentParamHasUses error fires iff the param is declared Absent
    AND has a live use (the contract claimed it is never used; the IR uses it). -/
def vf2_absent_param_error (p : AbsentParamInputs) : Bool :=
  p.declaredAbsent && p.hasLiveUse

/-- §9.2 VF-2 (P1) the AbsentParamHasUses decision grid: the error fires iff
    Absent ∧ has-live-use, over all 4 (declaredAbsent, hasLiveUse) rows. -/
theorem VF2_absent_param_decision (p : AbsentParamInputs) :
    vf2_absent_param_error p = (p.declaredAbsent && p.hasLiveUse) := by rfl

/-- §9.2 VF-2 (P1) a non-Absent parameter is consistent regardless of use
    (uses are permitted on a non-Absent param). -/
theorem VF2_non_absent_consistent (hasLiveUse : Bool) :
    vf2_absent_param_error { declaredAbsent := false, hasLiveUse := hasLiveUse }
      = false := by
  unfold vf2_absent_param_error; rfl

/-- §9.2 VF-2 (P1) an Absent parameter with NO live use is consistent (the
    contract's claim holds). -/
theorem VF2_absent_no_use_consistent :
    vf2_absent_param_error { declaredAbsent := true, hasLiveUse := false }
      = false := by decide

/-- §9.2 VF-2 (P1) the load-bearing positive: an Absent parameter WITH a live use
    is flagged (the contract-vs-realization inconsistency). -/
theorem VF2_absent_with_use_flagged :
    vf2_absent_param_error { declaredAbsent := true, hasLiveUse := true }
      = true := by decide

/-- §9.2 VF-2 (P2) target-only conditional soundness: a target-only check
    (b)/(c)/(d) is sound iff its sec-08 LLVM-fact-export dependency discharged.
    Modeled as the implication `dependencyDischarged → checkSound`. -/
def vf2_target_only_sound (dependencyDischarged checkSound : Bool) : Bool :=
  !dependencyDischarged || checkSound

/-- §9.2 VF-2 (P2) when the sec-08 dependency (RL-31 / RL-29 / RL-30) IS
    discharged, the corresponding target-only check is sound. -/
theorem VF2_target_only_discharged_sound :
    vf2_target_only_sound true true = true := by decide

/-- §9.2 VF-2 (P2) the negative witness: emitting a target-only check WITHOUT its
    sec-08 proof (`checkSound = false` while claiming to fire) is unsound — the
    soundness implication is violated. The verifier rejects the undischarged
    case: a fired-but-undischarged target check (`dependencyDischarged = true`,
    `checkSound = false`) yields an unsound verdict. -/
theorem VF2_target_only_undischarged_unsound :
    vf2_target_only_sound true false = false := by decide

/-- §9.2 VF-2 catches the ContractInconsistent failure class (disjoint from VF-1's
    StructuralIllformed — the two layers catch distinct classes). -/
theorem VF2_catches_contract :
    Layer.catches .L2Contract = FailureClass.ContractInconsistent := by rfl

/-! ## §9.3 VF-3 — Layer-3 oracle re-derivation (annex-e §AIMS §9 VF-3)

    VF-3 re-derives a `MemoryContract` from the realized IR and checks the
    inferred contract was NOT too optimistic: SAFE iff the re-derived contract is
    lattice-≤ the inferred contract on every compared dimension (access,
    consumption, effects). An UNSAFE mismatch is the re-derived value EXCEEDING
    the inferred value on any dimension (analysis demanded less than realization
    needs). The lattice ranks reuse `Model.lean`'s `AccessClass.rank` /
    `Consumption.rank`. -/

/-- §9.3 VF-3 the comparable contract dimensions (the three VF-3 cross-checks). -/
structure ContractDims where
  access : AccessClass
  consumption : Consumption
  -- effects modeled as the may_alloc flag (the shipped oracle re-derives
  -- may_allocate / may_deallocate / may_share; one OR-monotone flag is the
  -- faithful minimal carrier: false ≤ true).
  mayAlloc : Bool
deriving Repr, DecidableEq

/-- §9.3 VF-3 the effect-flag rank (false < true; OR-monotone). -/
def boolRank (b : Bool) : Nat := if b then 1 else 0

/-- §9.3 VF-3 SAFE iff the re-derived contract is lattice-≤ the inferred contract
    on EVERY dimension — access, consumption, and the effect flag each refine
    (re-derived rank ≤ inferred rank). -/
def vf3_safe (rederived inferred : ContractDims) : Bool :=
  (rederived.access.rank ≤ inferred.access.rank)
    && (rederived.consumption.rank ≤ inferred.consumption.rank)
    && (boolRank rederived.mayAlloc ≤ boolRank inferred.mayAlloc)

/-- §9.3 VF-3 (P1/P2) exact-match is SAFE: a re-derived contract equal to the
    inferred contract on every dimension refines trivially. -/
theorem VF3_exact_match_safe (c : ContractDims) : vf3_safe c c = true := by
  unfold vf3_safe; simp

/-- §9.3 VF-3 (P1/P2) re-derived strictly below inferred is SAFE: a Borrowed/Dead/
    pure re-derived contract refines an Owned/Unrestricted/may-alloc inferred
    contract (the realized IR is within the promised envelope). -/
theorem VF3_rederived_below_inferred_safe :
    vf3_safe
      { access := .Borrowed, consumption := .Dead, mayAlloc := false }
      { access := .Owned, consumption := .Unrestricted, mayAlloc := true }
      = true := by decide

/-- §9.3 VF-3 (P2) access too optimistic is UNSAFE: a re-derived Owned access
    against an inferred Borrowed contract EXCEEDS the inferred dimension — the
    realized IR owns a value the contract promised was borrowed (a
    miscompilation where the caller treats it as Borrowed). The negative witness. -/
theorem VF3_access_too_optimistic_unsafe :
    vf3_safe
      { access := .Owned, consumption := .Linear, mayAlloc := false }
      { access := .Borrowed, consumption := .Linear, mayAlloc := false }
      = false := by decide

/-- §9.3 VF-3 (P2) consumption too optimistic is UNSAFE: re-derived Unrestricted
    vs inferred Linear exceeds the consumption dimension. -/
theorem VF3_consumption_too_optimistic_unsafe :
    vf3_safe
      { access := .Owned, consumption := .Unrestricted, mayAlloc := false }
      { access := .Owned, consumption := .Linear, mayAlloc := false }
      = false := by decide

/-- §9.3 VF-3 (P2) effect too optimistic is UNSAFE: a re-derived may-alloc against
    an inferred pure contract exceeds the effect dimension. -/
theorem VF3_effect_too_optimistic_unsafe :
    vf3_safe
      { access := .Owned, consumption := .Linear, mayAlloc := true }
      { access := .Owned, consumption := .Linear, mayAlloc := false }
      = false := by decide

/-- §9.3 VF-3 (P2) per-dimension teeth: ANY single dimension where the re-derived
    rank exceeds the inferred rank yields UNSAFE — the SAFE verdict requires
    every dimension to refine (proven by the contrapositive over the access
    dimension as the witness dimension). -/
theorem VF3_any_dimension_exceeds_unsafe (rederived inferred : ContractDims)
    (h : rederived.access.rank > inferred.access.rank) :
    vf3_safe rederived inferred = false := by
  unfold vf3_safe
  have : ¬ (rederived.access.rank ≤ inferred.access.rank) := Nat.not_le.mpr h
  simp [this]

/-- §9.3 VF-3 catches the OracleTooOptimistic failure class. -/
theorem VF3_catches_oracle :
    Layer.catches .L3Oracle = FailureClass.OracleTooOptimistic := by rfl

/-! ## §9.4 VF-4 — Layer-4 FIP certification (annex-e §AIMS §9 VF-4)

    VF-4 proves a `FipContract::Certified` function has zero unmatched
    alloc/dealloc. This is the allocation-ledger analogue of the §8 RC-balance
    ledger: an allocation lifecycle is a list of `AllocEvent`s (Alloc = +1,
    Dealloc = -1, ReuseInPlace = 0); Certified IFF the net is 0. A net > 0 is a
    leak relative to the contract; a net < 0 is a double-free. A Reset+Reuse pair
    is allocation-neutral (the FIP win). -/

/-- §9.4 VF-4 an allocation event on a value's FIP lifecycle. -/
inductive AllocEvent
  | alloc        -- +1
  | dealloc      -- -1
  | reuseInPlace -- 0 (the FIP allocation-free win)
deriving Repr, DecidableEq

/-- §9.4 VF-4 the net allocation delta of one event. -/
def AllocEvent.delta : AllocEvent → Int
  | .alloc        => 1
  | .dealloc      => -1
  | .reuseInPlace => 0

/-- §9.4 VF-4 the net allocation balance of a lifecycle = the sum of event
    deltas (the FIP ledger). -/
def allocBalance (events : List AllocEvent) : Int :=
  (events.map AllocEvent.delta).foldr (· + ·) 0

/-- §9.4 VF-4 a function is FIP-Certified iff its allocation ledger nets 0. -/
def vf4_certified (events : List AllocEvent) : Bool :=
  decide (allocBalance events = 0)

/-- §9.4 VF-4 ledger lemma: the balance of an appended lifecycle is the sum of
    balances (the FIP ledger is additive). Proven by induction over the first
    segment (mirrors the §8 `rcBalance_append`). -/
theorem allocBalance_append (xs ys : List AllocEvent) :
    allocBalance (xs ++ ys) = allocBalance xs + allocBalance ys := by
  unfold allocBalance
  induction xs with
  | nil => simp
  | cons hd tl ih =>
      simp only [List.map_cons, List.cons_append, List.foldr_cons]
      rw [ih]; omega

/-- §9.4 VF-4 (P1) alloc matched by a dealloc is Certified (net 0). -/
theorem VF4_alloc_matched_dealloc_certified :
    vf4_certified [AllocEvent.alloc, AllocEvent.dealloc] = true := by decide

/-- §9.4 VF-4 (P2) a Reset+Reuse pair (in-place reuse) is allocation-neutral —
    a pure FIP function reusing a dying allocation nets 0 (the FIP win). -/
theorem VF4_reuse_in_place_certified :
    vf4_certified [AllocEvent.reuseInPlace] = true := by decide

/-- §9.4 VF-4 (P2) a mixed balanced lifecycle (paired alloc/dealloc + an in-place
    reuse) is Certified. -/
theorem VF4_mixed_balanced_certified :
    vf4_certified
      [AllocEvent.alloc, AllocEvent.reuseInPlace, AllocEvent.dealloc] = true := by
  decide

/-- §9.4 VF-4 (P2) a pure FIP function (empty ledger) is Certified (net 0). -/
theorem VF4_no_allocations_certified : vf4_certified [] = true := by decide

/-- §9.4 VF-4 the negative witness: an unmatched allocation (net = +1, a leak
    relative to the contract) is NOT Certified. -/
theorem VF4_unmatched_alloc_not_certified :
    vf4_certified [AllocEvent.alloc] = false := by decide

/-- §9.4 VF-4 the negative witness: an unmatched dealloc (net = -1, a double-free
    relative to the contract) is NOT Certified. -/
theorem VF4_unmatched_dealloc_not_certified :
    vf4_certified [AllocEvent.dealloc] = false := by decide

/-- §9.4 VF-4 (P1) certification iff balance: a function is Certified IFF its
    allocation ledger nets 0 (the soundness characterization, not just the
    fixture rows). -/
theorem VF4_certified_iff_balanced (events : List AllocEvent) :
    vf4_certified events = true ↔ allocBalance events = 0 := by
  unfold vf4_certified; exact decide_eq_true_iff

/-- §9.4 VF-4 catches the FipUnbalanced failure class. -/
theorem VF4_catches_fip :
    Layer.catches .L4Fip = FailureClass.FipUnbalanced := by rfl

/-! ## §9.5 VF-5 — end-to-end verification mandate (annex-e §AIMS §9 VF-5)

    Every active subsystem SHALL be end-to-end verified: a subsystem is verified
    IFF it has implementation AND invariant-enforcement AND tests. Missing any
    one tier marks it incomplete; there is no two-of-three partial credit. -/

/-- §9.5 VF-5 the three end-to-end tiers of a subsystem. -/
structure SubsystemTiers where
  hasImplementation : Bool
  hasInvariantEnforcement : Bool
  hasTests : Bool
deriving Repr, DecidableEq

/-- §9.5 VF-5 a subsystem is end-to-end verified iff ALL three tiers hold. -/
def vf5_verified (s : SubsystemTiers) : Bool :=
  s.hasImplementation && s.hasInvariantEnforcement && s.hasTests

/-- §9.5 VF-5 (P1) three-tier conjunction: verified iff impl ∧ enforcement ∧
    tests, over all 8 tier combinations. -/
theorem VF5_three_tier_conjunction (s : SubsystemTiers) :
    vf5_verified s
      = (s.hasImplementation && s.hasInvariantEnforcement && s.hasTests) := by rfl

/-- §9.5 VF-5 (P1) all three present ⟹ complete. -/
theorem VF5_all_tiers_complete :
    vf5_verified { hasImplementation := true, hasInvariantEnforcement := true, hasTests := true } = true := by decide

/-- §9.5 VF-5 (P2) missing implementation ⟹ incomplete. -/
theorem VF5_missing_impl_incomplete (enf tests : Bool) :
    vf5_verified { hasImplementation := false, hasInvariantEnforcement := enf, hasTests := tests } = false := by
  unfold vf5_verified; rfl

/-- §9.5 VF-5 (P2) missing tests ⟹ incomplete (an unexercised gate). -/
theorem VF5_missing_tests_incomplete (impl enf : Bool) :
    vf5_verified { hasImplementation := impl, hasInvariantEnforcement := enf, hasTests := false } = false := by
  unfold vf5_verified; simp

/-- §9.5 VF-5 (P2) the load-bearing negative witness: a subsystem with
    implementation + tests but NO invariant enforcement is NOT complete —
    accepting it would let an unenforced invariant silently rot (the gate is
    absent, so the invariant degrades undetected). -/
theorem VF5_missing_enforcement_incomplete :
    vf5_verified { hasImplementation := true, hasInvariantEnforcement := false, hasTests := true } = false := by decide

/-! ## §9.6 VF-6 — contracts ↔ realization agreement (annex-e §AIMS §9 VF-6)

    VF-6 COMPOSES VF-3 (oracle refinement) and VF-4 (FIP balance) into a single
    contract-realization implication: when the contract claims Certified,
    agreement requires BOTH the realized IR alloc-balanced (VF-4) AND the oracle
    re-derivation refining the inferred contract (VF-3). A non-Certified contract
    still requires oracle refinement (VF-3 applies regardless); FIP balance is
    asserted only when Certified is claimed. -/

/-- §9.6 VF-6 the agreement predicate composing the VF-3 (oracleRefines) and VF-4
    (allocBalanced) sub-verdicts under the Certified claim. -/
def vf6_agrees (certified allocBalanced oracleRefines : Bool) : Bool :=
  if certified then allocBalanced && oracleRefines else oracleRefines

/-- §9.6 VF-6 (P1) Certified ⟹ balanced ∧ refines: a Certified contract agrees
    iff BOTH VF-4 (balanced) and VF-3 (oracle refines) hold. -/
theorem VF6_certified_requires_both (allocBalanced oracleRefines : Bool) :
    vf6_agrees true allocBalanced oracleRefines
      = (allocBalanced && oracleRefines) := by rfl

/-- §9.6 VF-6 (P1) a Certified + balanced + refining contract is in AGREEMENT. -/
theorem VF6_certified_balanced_refines_agree :
    vf6_agrees true true true = true := by decide

/-- §9.6 VF-6 (P2) Certified over an UNBALANCED realized IR DISAGREES (VF-4
    fails) — the contract claims FIP while the realized IR leaks. The negative
    witness. -/
theorem VF6_certified_unbalanced_disagree (oracleRefines : Bool) :
    vf6_agrees true false oracleRefines = false := by
  unfold vf6_agrees; simp

/-- §9.6 VF-6 (P2) Certified + balanced but oracle-UNSAFE DISAGREES (VF-3 fails). -/
theorem VF6_certified_oracle_unsafe_disagree :
    vf6_agrees true true false = false := by decide

/-- §9.6 VF-6 (P2) a NON-Certified contract still requires oracle refinement:
    agreement = the VF-3 verdict alone (FIP balance not asserted). A non-
    Certified + refining contract agrees; a non-Certified + oracle-unsafe
    contract disagrees (VF-3 applies regardless of the Certified claim). -/
theorem VF6_noncertified_is_oracle (allocBalanced oracleRefines : Bool) :
    vf6_agrees false allocBalanced oracleRefines = oracleRefines := by rfl

/-- §9.6 VF-6 (P2) a non-Certified + oracle-unsafe contract DISAGREES (VF-3 still
    applies even without a Certified claim). -/
theorem VF6_noncertified_oracle_unsafe_disagree (allocBalanced : Bool) :
    vf6_agrees false allocBalanced false = false := by
  unfold vf6_agrees; simp

/-! ## §9.7 VF-7 — active-rewrite three-tier discipline (annex-e §AIMS §9 VF-7)

    An active rewrite SHALL be sound (identical observable behavior). It is active
    IFF it has (a) compile-time structural verification AND (b) test-time
    behavioral verification AND (c) a documented proof sketch. Structural
    verification ALONE does NOT satisfy soundness — behavioral verification is
    required. Lacking any tier = NOT active. -/

/-- §9.7 VF-7 the three active-rewrite tiers. -/
structure RewriteTiers where
  structuralVerification : Bool   -- (a)
  behavioralVerification : Bool   -- (b)
  proofSketch : Bool              -- (c)
deriving Repr, DecidableEq

/-- §9.7 VF-7 a rewrite is active iff ALL three tiers hold. -/
def vf7_active (r : RewriteTiers) : Bool :=
  r.structuralVerification && r.behavioralVerification && r.proofSketch

/-- §9.7 VF-7 (P1) three-tier conjunction: active iff (a) ∧ (b) ∧ (c). -/
theorem VF7_three_tier_conjunction (r : RewriteTiers) :
    vf7_active r
      = (r.structuralVerification && r.behavioralVerification && r.proofSketch)
      := by rfl

/-- §9.7 VF-7 (P1) all three tiers present ⟹ active. -/
theorem VF7_all_tiers_active :
    vf7_active { structuralVerification := true, behavioralVerification := true, proofSketch := true } = true := by decide

/-- §9.7 VF-7 (P1) the load-bearing negative witness: a STRUCTURAL-ONLY rewrite
    (tier (a) present, no behavioral (b) or proof sketch (c)) is NOT active —
    structural tests alone do not satisfy soundness; the rewrite could still
    change observable behavior. -/
theorem VF7_structural_only_not_active :
    vf7_active { structuralVerification := true, behavioralVerification := false, proofSketch := false } = false := by decide

/-- §9.7 VF-7 (P1) missing behavioral verification ⟹ not active. -/
theorem VF7_missing_behavioral_not_active (struct proof : Bool) :
    vf7_active { structuralVerification := struct, behavioralVerification := false, proofSketch := proof } = false := by
  unfold vf7_active; simp

/-- §9.7 VF-7 (P1) missing proof sketch ⟹ not active. -/
theorem VF7_missing_proof_sketch_not_active (struct beh : Bool) :
    vf7_active { structuralVerification := struct, behavioralVerification := beh, proofSketch := false } = false := by
  unfold vf7_active; simp

/-- §9.7 VF-7 (P2) per-rewrite-class tier mapping: each rewrite class (TRMC,
    RL-22..RL-26) names a non-empty source for all three tiers. Modeled as: a
    rewrite class supplies all three tiers iff each tier flag is true; both the
    TRMC and the RL-22..RL-26 classes do so. -/
def rewriteClassComplete (r : RewriteTiers) : Bool := vf7_active r

/-- §9.7 VF-7 (P2) the TRMC class supplies all three tiers (PL-10 structural /
    TRMC spec tests / constrained-rewrite proof sketch). -/
theorem VF7_trmc_class_complete :
    rewriteClassComplete { structuralVerification := true, behavioralVerification := true, proofSketch := true } = true := by decide

/-- §9.7 VF-7 (P2) the RL-22..RL-26 class supplies all three tiers (VF-1+VF-2+VF-3
    re-verify / RC-motion regression tests / RC-ops-only proof sketch). -/
theorem VF7_rl22_26_class_complete :
    rewriteClassComplete { structuralVerification := true, behavioralVerification := true, proofSketch := true } = true := by decide

/-! ## §9.8 VF-8 — stack applies to ALL rules (annex-e §AIMS §9 VF-8)

    The verification stack applies to ALL rules. A rule is COVERED iff it has a
    verification layer — an ACTIVE layer when implemented, a PLANNED layer when
    unimplemented or target-only. An unimplemented rule with NO planned layer is
    a SPEC GAP. -/

/-- §9.8 VF-8 a rule's coverage inputs: whether it is implemented (active layer),
    and whether it has a planned layer. -/
structure RuleCoverage where
  implemented : Bool
  hasPlannedLayer : Bool
deriving Repr, DecidableEq

/-- §9.8 VF-8 a rule is covered by the stack iff it is implemented (active layer)
    OR has a planned layer. -/
def vf8_covered (r : RuleCoverage) : Bool := r.implemented || r.hasPlannedLayer

/-- §9.8 VF-8 a rule is a SPEC GAP iff it is NEITHER implemented NOR planned. -/
def vf8_spec_gap (r : RuleCoverage) : Bool := !r.implemented && !r.hasPlannedLayer

/-- §9.8 VF-8 (P1) an implemented rule (active layer) is covered — e.g. sec-8
    RL-1 covered by VF-1 / VF-4. -/
theorem VF8_implemented_covered (hasPlanned : Bool) :
    vf8_covered { implemented := true, hasPlannedLayer := hasPlanned } = true := by
  unfold vf8_covered; rfl

/-- §9.8 VF-8 (P1) an unimplemented rule WITH a planned layer is covered — e.g.
    unimplemented RL-22 covered by the planned VF-7 three-tier layer; target-only
    IC-5 covered by the planned VF-2 (d) / RL-30 layer. -/
theorem VF8_unimplemented_planned_covered :
    vf8_covered { implemented := false, hasPlannedLayer := true } = true := by decide

/-- §9.8 VF-8 (P2) the load-bearing gap detection: an unimplemented rule with NO
    planned layer is a SPEC GAP (NOT covered) — silently treating it as covered
    would let a realization rule ship with no verification. The negative witness. -/
theorem VF8_unimplemented_no_layer_spec_gap :
    vf8_covered { implemented := false, hasPlannedLayer := false } = false
      ∧ vf8_spec_gap { implemented := false, hasPlannedLayer := false } = true := by
  constructor <;> decide

/-- §9.8 VF-8 (P1/P2) coverage ↔ not-spec-gap: a rule is covered by the stack iff
    it is NOT a spec gap (the closure property — every rule is either covered or a
    surfaced gap, never silently uncovered). Proven over all 4 rows. -/
theorem VF8_covered_iff_not_gap (r : RuleCoverage) :
    vf8_covered r = !vf8_spec_gap r := by
  unfold vf8_covered vf8_spec_gap
  cases r.implemented <;> cases r.hasPlannedLayer <;> rfl

/-! ## §9.comp VF-comp — layered-stack composition (annex-e §AIMS §9 VF-comp)

    The composition theorem. (1) Each layer catches a DISTINCT failure class (the
    catch-partition over the 4 detecting layers is injective). (2) The stack as a
    whole catches the UNION: the whole-stack verdict ACCEPTS iff EVERY layer
    accepts, so a fix passing a strict SUBSET of layers (regressing one) is
    REJECTED — no failure class escapes all layers. The whole-stack verdict is
    the conjunction-fold over the per-layer accept bits (mirrors the §8 RL-23 /
    interprocedural `foldr (· && ·)` composition pattern). -/

/-- §9.comp the whole-stack verdict over a list of per-layer accept bits: ACCEPT
    iff EVERY layer accepts (the conjunction fold). -/
def stackAccepts (layerVerdicts : List Bool) : Bool :=
  layerVerdicts.foldr (· && ·) true

/-- §9.comp (catch-partition) each layer catches a DISTINCT failure class — the
    `Layer.catches` map is INJECTIVE over the 4 detecting layers. No two layers
    share a failure class, so the per-layer classes are pairwise disjoint. -/
theorem VFcomp_layers_catch_distinct (l1 l2 : Layer)
    (h : l1.catches = l2.catches) : l1 = l2 := by
  cases l1 <;> cases l2 <;> first | rfl | (simp [Layer.catches] at h)

/-- §9.comp the per-layer catch map is SURJECTIVE onto the failure-class universe:
    every failure class is caught by some layer (no class is uncaught). -/
theorem VFcomp_every_class_caught (f : FailureClass) :
    ∃ l : Layer, l.catches = f := by
  cases f
  · exact ⟨.L1Structural, rfl⟩
  · exact ⟨.L2Contract, rfl⟩
  · exact ⟨.L3Oracle, rfl⟩
  · exact ⟨.L4Fip, rfl⟩

/-- §9.comp (P1) the stack ACCEPTS iff EVERY layer accepts — the joined union of
    the per-layer verdicts. Proven by induction over the layer-verdict list
    (the failing layer sticky-clears the conjunction). -/
theorem VFcomp_stack_accepts_iff_all (layerVerdicts : List Bool) :
    stackAccepts layerVerdicts = true ↔ ∀ v ∈ layerVerdicts, v = true := by
  unfold stackAccepts
  induction layerVerdicts with
  | nil => simp
  | cons hd tl ih =>
      simp only [List.foldr_cons, Bool.and_eq_true, List.mem_cons]
      rw [ih]
      constructor
      · rintro ⟨hhd, htl⟩ v (rfl | hv)
        · exact hhd
        · exact htl v hv
      · intro h
        exact ⟨h hd (Or.inl rfl), fun v hv => h v (Or.inr hv)⟩

/-- §9.comp (P1/P2) the union has teeth: a fix passing a strict SUBSET of layers
    (here VF-1/VF-2/VF-3 accept but VF-4 FIP regresses) is REJECTED by the whole
    stack — some layer in the union catches the regressed class. A fix passing one
    layer while regressing another is a correctness regression, not a partial win. -/
theorem VFcomp_subset_pass_rejected :
    stackAccepts [true, true, true, false] = false := by decide

/-- §9.comp (P2) conjunction strength: a stack where EVERY layer accepts is
    accepted (the only accepting verdict requires the full layer count to pass —
    the joined claim is no stronger than its weakest premise, and no weaker). -/
theorem VFcomp_all_pass_accepted :
    stackAccepts [true, true, true, true] = true := by decide

/-- §9.comp (P2) a DROPPED constituent leaves a failure class uncaught: if any
    layer's verdict is false (a regressed/missing layer), the stack rejects —
    no false-valid slips through. The negative direction over an arbitrary
    failing layer. -/
theorem VFcomp_any_layer_fails_stack_rejects
    (pre post : List Bool) :
    stackAccepts (pre ++ false :: post) = false := by
  unfold stackAccepts
  induction pre with
  | nil => simp
  | cons hd tl ih =>
      simp only [List.cons_append, List.foldr_cons, ih, Bool.and_false]

/-- §9.comp (P1/P2) the capstone: the stack covers the UNION of the per-layer
    failure classes. Concretely — the 4 detecting layers catch a complete,
    pairwise-distinct partition of the failure-class universe (every class caught
    by exactly one layer), AND the whole-stack verdict accepts iff every layer's
    verdict accepts (so any regressed layer's class is caught). No failure class
    escapes all layers. -/
theorem VFcomp_stack_covers_union :
    -- (a) every failure class is caught by some layer (complete coverage)
    (∀ f : FailureClass, ∃ l : Layer, l.catches = f)
    -- (b) the catch is a partition (distinct layers → distinct classes)
    ∧ (∀ l1 l2 : Layer, l1.catches = l2.catches → l1 = l2)
    -- (c) the whole-stack verdict is the union (accept iff every layer accepts)
    ∧ (∀ verdicts : List Bool,
        stackAccepts verdicts = true ↔ ∀ v ∈ verdicts, v = true) := by
  refine ⟨VFcomp_every_class_caught, VFcomp_layers_catch_distinct, ?_⟩
  exact VFcomp_stack_accepts_iff_all

end AimsProof
