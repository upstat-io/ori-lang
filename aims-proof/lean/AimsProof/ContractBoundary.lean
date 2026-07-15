/-
AIMS contract-boundary module — kernel-checked Lean proofs of the T4
cross-function contract-boundary composition theorem and the T5 frame-limited
robustness theorem over the T2 CFG-ledger model.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: T4 (contract-boundary composition — per-param contracts classify the
    boundary events; per-function three-clause ledgers compose end-to-end
    without callee re-derivation) + T5 (frame-limited robustness — a new
    tier-1 partition edge perturbs only the merged classes' ledgers) |
  spec: annex-e §AIMS §7 (interprocedural contracts — the per-param access
    verdict the boundary consumes) + §8 (the RL-2 twelve-kind terminal-use
    table, the RL-34 no-post-tail-call-dec law) |
  .proof: aims-proof/proofs/12-provenance/T4-contract-boundary-composition.proof
    + aims-proof/proofs/12-provenance/T5-frame-limited-robustness.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: T4 governs how a caller's per-class ledger meets a call site.
The callee's per-param contract — the access verdict (Owned / Borrowed per the
RL-32/RL-34 fixpoint), the iter-consume flag (the RL-2
`ApplyToIterConsumingParam` transfer kind), the transfers-through-return flag
(the RL-34 `transferOwnership` tail-call action / RL-2 Return-overlap), and
the sharing-view-producer flag (a read-only co-owner view result) — classifies
the boundary event for each argument's partition class, COMPUTED through the
committed RL-2 twelve-kind table (`rl2_use_transfers_ownership`,
AimsProof.Realization), never free input:
  owned param (callee side)                  = BIRTH (the callee ledger opens
                                               with the transferred-in ref);
  owned arg (caller side)                    = CONSUME (the caller hands off);
  iter_consumes && !transfers_through_return = CONSUME (the callee's iterator
                                               machinery is the release);
  borrowed                                   = READ (caller retains);
  transfers_through_return                   = PASSTHROUGH, modeled as
                                               CONSUME-at-call + CREDIT-at-
                                               return netting zero (the form
                                               stated explicitly below);
  sharing-view producer                      = CREDIT (the view result mints a
                                               credit fact on the source class).
Composing a caller ledger satisfying the three clauses WITH the contract-
classified summary in place, and a callee ledger satisfying its OWN three
clauses under the contract-mandated opening, yields a three-clause-satisfying
inlined ledger — the contract IS the interface; the callee is never re-derived.

T5 governs partition evolution: introducing a NEW tier-1 same-allocation edge
(a T1 `PartitionAdm.tier1` admission merging two classes) is FRAME-LIMITED —
every class whose membership is untouched derives a VERBATIM-identical ledger
(same events, same verdict), the merged class's ledger is exactly the
positional interleaving of the two prior classes' events (mutate-free,
non-bridging fragment; a class-bridging jump-arg handoff collapses into the
RL-4 exemption, net-preservingly), clause 1 composes unconditionally (net
additivity), and clauses 2/3 compose under two honest side conditions — both
prior running counts stay nonnegative at every prefix, and the stream is
dynamic-COW-mutate-free — each shown NECESSARY by a negative witness (two
individually-safe ledgers interleave into a READ-at-0 shape; a merge turns a
formerly-cross-class suffix reader into a live sibling and raises a mutate
floor past the running count).

Structure:
  Part A — the T4 boundary carrier `BoundaryContract` (access = the committed
    `AccessClass`; iter-consume / transfers-through-return / sharing-view
    flags the committed corpus lacks, documented as the T4 carrier) and the
    COMPUTED boundary classification: `boundaryUseKind` selects the RL-2
    terminal-use kind; `boundaryInstrs` expands the call site to
    instruction-shaped input; the classification-table row theorems.
  Part B — the callee side: `calleeOpening` (BIRTH iff the boundary use kind
    transfers ownership in — the same committed table lookup), `calleeLedger`,
    and the callee's own invariant `calleeConforms` (three clauses under the
    opening; the borrow assumption for non-transfer contracts).
  Part C — floor monotonicity, the summary-split lemma, the event-level
    substitution lemma, and THE T4 composition theorem
    `T4_contract_boundary_composition_sound` (+ the `ledgerSafe` corollary).
  Part D — T4 negative witnesses: the owned-arg-misclassified-borrowed
    double-release (clause 1 rejects the composed ledger at count -1) and the
    live-at-call side-condition necessity witness (a dead call-site count
    composes into a use-after-free the summary alone cannot see).
  Part E — the T5 class-map redirect `mergeClasses` (the class-level
    projection of a tier-1 `PartitionUF.union` — root redirect), membership
    lemmas, the derivation-congruence lemma, and THE frame theorem
    `T5_frame_untouched_class_ledger_verbatim` (+ verbatim-verdict corollary
    + the retired-class-empties lemma).
  Part F — T5 net additivity: the merged class's net ledger is EXACTLY the
    sum of the two prior classes' nets, for EVERY instruction stream
    (bridging jump-args collapse net-preservingly) — clause 1 composes
    unconditionally.
  Part G — T5 bounded impact: on the mutate-free non-bridging fragment the
    merged ledger IS the positional interleaving (`filterMap` over the
    per-instruction event of whichever prior class the instruction touched);
    the bridging-jump-arg collapse lemma states the one shape that folds.
  Part H — T5 robustness: the floors-composition lemma (both prior counts
    threaded; nonnegativity carries each class's floor across the other's
    events) and the robustness corollary; the two necessity witnesses.
  Part I — T1 grounding: the new semantic edge is a `PartitionAdm.tier1`
    admission; the concrete `PartitionUF.union` instance computes exactly the
    `mergeClasses` redirect on the witness universe.
-/

import AimsProof.Ledger
import AimsProof.Interprocedural

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §T4 Part A — the boundary carrier + the computed classification

    The T4 carrier: `access` is the COMMITTED `AccessClass` (Model.lean §1.2;
    the RL-32 fixpoint / RL-34 tail-call verdict). The three flags are fields
    the committed corpus lacks a carrier for, defined here as the T4 carrier:
    `iterConsumes` grounds the RL-2 `ApplyToIterConsumingParam` transfer kind;
    `transfersThroughReturn` grounds the RL-34 `transferOwnership` action and
    the RL-2 iter-consume-Return overlap; `sharingViewProducer` grounds the
    read-only co-owner view result (RL-17 sharing-bound family). -/

/-- §T4 the boundary contract — the per-param interface the caller composes
    through. `access` is the committed `AccessClass`; the flags are the T4
    carrier fields. -/
structure BoundaryContract where
  access : AccessClass
  iterConsumes : Bool
  transfersThroughReturn : Bool
  sharingViewProducer : Bool
deriving Repr, DecidableEq

/-- §T4 grounding constructor: the boundary access IS the IC-2/IC-3
    `ParamContract`'s access dimension (annex-e §AIMS §7). -/
def BoundaryContract.ofParamContract (pc : ParamContract)
    (iter ttr view : Bool) : BoundaryContract :=
  ⟨pc.access, iter, ttr, view⟩

/-- §T4 the COMPUTED terminal-use kind of the argument at the call site — a
    function of the contract fields into the committed RL-2 twelve-kind
    vocabulary. An Owned param receives the reference (transfer); a Borrowed
    param that iter-consumes WITHOUT transferring through the return is the
    RL-2 iter-consume transfer; every other Borrowed param is the borrow-read
    kind. -/
def boundaryUseKind (bc : BoundaryContract) : TerminalUse :=
  match bc.access with
  | .Owned => .ApplyToOwnedParam
  | .Borrowed =>
      if bc.iterConsumes && !bc.transfersThroughReturn then
        .ApplyToIterConsumingParam
      else
        .ApplyToBorrowedParam

/-- §T4 whether the boundary transfers ownership INTO the callee — looked up
    in the COMMITTED RL-2 table, never restated. -/
def boundaryTransfersIn (bc : BoundaryContract) : Bool :=
  rl2_use_transfers_ownership (boundaryUseKind bc)

/-- §T4 the caller-side result-binding instructions: a transfers-through-
    return contract re-acquires the same allocation through the return value;
    a sharing-view producer's result co-owns the source allocation. Both mint
    the CREDIT on the source class (`ret` is tier-1-unified with `arg` — the
    same allocation flows back / the view shares it). -/
def boundaryResultInstrs (ret : Nat) (bc : BoundaryContract) : List LedgerInstr :=
  if bc.transfersThroughReturn || bc.sharingViewProducer then
    [.dup ret]
  else
    []

/-- §T4 the two exits of an unwind-capable call. The argument boundary event
    happens before either exit is selected; a result binding exists only on the
    normal exit. -/
inductive BoundaryExit
  | normal
  | unwind
deriving Repr, DecidableEq

/-- §T4 the event image of the result binding. A passthrough or sharing view
    credits the source class; every other result has no event on that class. -/
def boundaryResultEvents (bc : BoundaryContract) : List LedgerEvent :=
  if bc.transfersThroughReturn || bc.sharingViewProducer then [.credit] else []

/-- §T4 the COMPUTED call-site expansion: the argument's terminal use (the
    contract-selected RL-2 kind) followed by the result binding. This is the
    instruction-shaped input the caller's ledger derives through the
    committed classification. -/
def boundaryInstrs (arg ret : Nat) (bc : BoundaryContract) : List LedgerInstr :=
  .escapeUse arg (boundaryUseKind bc) :: boundaryResultInstrs ret bc

/-- §T4 the caller-side summary event of the argument: CONSUME exactly when
    the contract-selected kind transfers ownership (the committed table's
    verdict), READ otherwise. -/
def boundaryArgEvent (bc : BoundaryContract) : LedgerEvent :=
  if boundaryTransfersIn bc then .consume else .read

/-- §T4 the boundary events visible on one Invoke successor. The argument
    event is common to both successors. Only normal return binds a result. -/
def invokeBoundaryEvents (bc : BoundaryContract) : BoundaryExit → List LedgerEvent
  | .normal => boundaryArgEvent bc :: boundaryResultEvents bc
  | .unwind => [boundaryArgEvent bc]

/-- §T4 the derived caller-side argument event IS the summary event — through
    the committed RL-2 bridge (`terminal_event_matches_rl2`). -/
theorem boundary_arg_event_derived (classOf : Nat → Nat) (c arg : Nat)
    (bc : BoundaryContract) (harg : (classOf arg == c) = true) :
    deriveLedger classOf c [.escapeUse arg (boundaryUseKind bc)]
      = [boundaryArgEvent bc] := by
  rw [terminal_event_matches_rl2 classOf c arg harg (boundaryUseKind bc)]
  rfl

/-- §T4 the summary event's net: -1 on a transfer-in boundary (the obligation
    hands off), 0 on a borrow (the caller retains). -/
theorem boundaryArgEvent_net (bc : BoundaryContract) :
    ledgerNet [boundaryArgEvent bc]
      = (if boundaryTransfersIn bc then (-1 : Int) else 0) := by
  unfold boundaryArgEvent
  cases boundaryTransfersIn bc <;> rfl

/-! ### §T4 Part A.1 — the classification-table row theorems (P1)

    Each row of the fixed table is a COMPUTED fact about `boundaryInstrs` /
    `deriveLedger` — the classification flows through the committed RL-2
    table, never a stipulated event list. -/

/-- §T4 (P1) OWNED ARG = CONSUME: an Owned-access contract selects the RL-2
    `ApplyToOwnedParam` transfer kind, and the caller's derived boundary
    event is the consume — the caller hands off its reference at the call. -/
theorem T4_owned_arg_consumes (classOf : Nat → Nat) (c arg : Nat)
    (iter ttr view : Bool) (harg : (classOf arg == c) = true) :
    boundaryUseKind ⟨.Owned, iter, ttr, view⟩ = .ApplyToOwnedParam
    ∧ deriveLedger classOf c
        [.escapeUse arg (boundaryUseKind ⟨.Owned, iter, ttr, view⟩)]
      = [.consume] := by
  refine ⟨rfl, ?_⟩
  rw [boundary_arg_event_derived classOf c arg _ harg]
  rfl

/-- §T4 (P1) ITER-CONSUME (no return transfer) = CONSUME: a Borrowed-access
    contract with `iterConsumes && !transfersThroughReturn` selects the RL-2
    `ApplyToIterConsumingParam` transfer kind — the callee's iterator
    machinery is the release, so the caller's boundary event is the consume
    (mirrors `RL2_iter_consuming_no_caller_dec`). -/
theorem T4_iter_consume_arg_consumes (classOf : Nat → Nat) (c arg : Nat)
    (view : Bool) (harg : (classOf arg == c) = true) :
    boundaryUseKind ⟨.Borrowed, true, false, view⟩ = .ApplyToIterConsumingParam
    ∧ deriveLedger classOf c
        [.escapeUse arg (boundaryUseKind ⟨.Borrowed, true, false, view⟩)]
      = [.consume] := by
  refine ⟨rfl, ?_⟩
  rw [boundary_arg_event_derived classOf c arg _ harg]
  rfl

/-- §T4 (P1) BORROWED = READ: a plain Borrowed contract (no iter-consume, no
    return transfer) selects the RL-2 borrow-read kind; the caller retains
    its reference and the boundary event is the read the placed release must
    follow (mirrors `RL2_borrowed_param_emits_caller_dec`). -/
theorem T4_borrowed_arg_reads (classOf : Nat → Nat) (c arg : Nat)
    (view : Bool) (harg : (classOf arg == c) = true) :
    boundaryUseKind ⟨.Borrowed, false, false, view⟩ = .ApplyToBorrowedParam
    ∧ deriveLedger classOf c
        [.escapeUse arg (boundaryUseKind ⟨.Borrowed, false, false, view⟩)]
      = [.read] := by
  refine ⟨rfl, ?_⟩
  rw [boundary_arg_event_derived classOf c arg _ harg]
  rfl

/-- §T4 (P1) PASSTHROUGH = CONSUME-at-call + CREDIT-at-return, NETTING ZERO —
    the explicitly-chosen passthrough form. An Owned contract transferring
    through the return hands the reference off at the call (the RL-2 transfer
    verdict) and re-acquires the SAME allocation through the result binding
    (`ret` tier-1-unified with `arg`): the caller-visible boundary is
    `[consume, credit]` and its net is zero — no net boundary event. -/
theorem T4_passthrough_consume_credit_nets_zero (classOf : Nat → Nat)
    (c arg ret : Nat) (iter view : Bool)
    (harg : (classOf arg == c) = true) (hret : (classOf ret == c) = true) :
    deriveLedger classOf c (boundaryInstrs arg ret ⟨.Owned, iter, true, view⟩)
      = [.consume, .credit]
    ∧ ledgerNet (deriveLedger classOf c
        (boundaryInstrs arg ret ⟨.Owned, iter, true, view⟩)) = 0 := by
  have hderiv : deriveLedger classOf c
      (boundaryInstrs arg ret ⟨.Owned, iter, true, view⟩)
      = [.consume, .credit] := by
    show deriveLedger classOf c
        (.escapeUse arg .ApplyToOwnedParam :: [.dup ret]) = _
    show (if classOf arg == c then
            (if rl2_use_transfers_ownership .ApplyToOwnedParam then
              LedgerEvent.consume else .read) :: deriveLedger classOf c [.dup ret]
          else deriveLedger classOf c [.dup ret]) = _
    rw [harg]
    show LedgerEvent.consume ::
        (if classOf ret == c then LedgerEvent.credit :: deriveLedger classOf c []
         else deriveLedger classOf c []) = _
    rw [hret]
    rfl
  refine ⟨hderiv, ?_⟩
  rw [hderiv]
  rfl

/-- §T4 (P1) SHARING-VIEW PRODUCER = CREDIT: the view result mints a credit
    fact on the SOURCE class (`ret` shares the source allocation — the T1
    tier-1 view admission), on top of the borrow-read of the source. The
    credit component in isolation: the result binding derives exactly
    `[credit]`. -/
theorem T4_sharing_view_credits_source (classOf : Nat → Nat)
    (c arg ret : Nat) (harg : (classOf arg == c) = true)
    (hret : (classOf ret == c) = true) :
    deriveLedger classOf c
        (boundaryResultInstrs ret ⟨.Borrowed, false, false, true⟩)
      = [.credit]
    ∧ deriveLedger classOf c
        (boundaryInstrs arg ret ⟨.Borrowed, false, false, true⟩)
      = [.read, .credit] := by
  constructor
  · show deriveLedger classOf c [.dup ret] = _
    show (if classOf ret == c then
            LedgerEvent.credit :: deriveLedger classOf c []
          else deriveLedger classOf c []) = _
    rw [hret]
    rfl

  · show deriveLedger classOf c
        (.escapeUse arg .ApplyToBorrowedParam :: [.dup ret]) = _
    show (if classOf arg == c then
            (if rl2_use_transfers_ownership .ApplyToBorrowedParam then
              LedgerEvent.consume else .read) :: deriveLedger classOf c [.dup ret]
          else deriveLedger classOf c [.dup ret]) = _
    rw [harg]
    show LedgerEvent.read ::
        (if classOf ret == c then LedgerEvent.credit :: deriveLedger classOf c []
         else deriveLedger classOf c []) = _
    rw [hret]
    rfl

/-- §T4 the result-event abstraction is derived from the same instruction
    expansion as `boundaryResultInstrs`, not stipulated independently. -/
theorem boundary_result_events_derived (classOf : Nat → Nat) (c ret : Nat)
    (bc : BoundaryContract) (hret : (classOf ret == c) = true) :
    deriveLedger classOf c (boundaryResultInstrs ret bc) = boundaryResultEvents bc := by
  unfold boundaryResultInstrs boundaryResultEvents
  split <;> simp_all [deriveLedger]

/-- §T4 Invoke argument classification precedes successor selection: both
    the normal and unwind event streams begin with the same computed argument
    event. -/
theorem T4_invoke_arg_event_precedes_both_exits (bc : BoundaryContract) :
    (invokeBoundaryEvents bc .normal).head? = some (boundaryArgEvent bc)
    ∧ (invokeBoundaryEvents bc .unwind).head? = some (boundaryArgEvent bc) := by
  constructor <;> rfl

/-- §T4 an Owned Invoke argument transfers on both exits. Treating the
    consume as normal-only omits the handoff from the unwind path. -/
theorem T4_invoke_owned_arg_consumes_on_both_exits :
    (invokeBoundaryEvents ⟨.Owned, false, false, false⟩ .normal).head?
        = some .consume
    ∧ (invokeBoundaryEvents ⟨.Owned, false, false, false⟩ .unwind).head?
        = some .consume := by
  decide

/-- §T4 negative witness: the unwind stream of an Owned argument is not
    empty, so a normal-only argument transfer contradicts the boundary model. -/
theorem T4_invoke_unwind_transfer_omission_rejected :
    invokeBoundaryEvents ⟨.Owned, false, false, false⟩ .unwind ≠ [] := by
  decide

/-- §T4 result credit is path-asymmetric: a sharing-view result credits the
    source only on normal return, while the argument read precedes both exits. -/
theorem T4_invoke_result_credit_is_normal_only :
    invokeBoundaryEvents ⟨.Borrowed, false, false, true⟩ .normal
        = [.read, .credit]
    ∧ invokeBoundaryEvents ⟨.Borrowed, false, false, true⟩ .unwind
        = [.read] := by
  decide

/-! ### §T4 / PV-4 borrowed indirect-call adapter

    Indirect-call explicit operands use one caller-borrowed logical contract:
    the caller keeps its ownership obligation, so each explicit operand
    contributes a READ on its partition class. Each backend projects that
    contract into its own calling convention. The closed target identity freezes the exact
    inward-owner demand derived from the final parameter contract. Ordinary
    Owned access, borrowed iterator consumption, and borrowed COW consumption
    demand a whole-value owner; projected-field iterator consumption demands
    only that field's owner. A plain borrow demands none. Contradictory whole-
    value plus projected-field demands fail closed. CREDIT/CONSUME are logical
    AIMS events; this model selects no physical retain instruction, counter,
    object layout, or discharge mechanism. -/

/-- Exact topology of the independent owner demanded by one target parameter.
    This is a logical AIMS carrier: `projectedField` identifies a semantic
    partition, not a physical byte offset or representation. -/
inductive CalleeOwnerDemand
  | borrow
  | wholeValue
  | projectedField (field : Nat)
deriving Repr, DecidableEq

/-- Whether the normalized demand needs one logical owner on the selected
    partition class. -/
def CalleeOwnerDemand.requiresCredit : CalleeOwnerDemand → Bool
  | .borrow => false
  | .wholeValue | .projectedField _ => true

/-- Stable semantic identity of one target parameter. Function and parameter
    identities are part of the frozen fact so result-side evidence cannot be
    replayed across callables or parameter slots. -/
structure TargetOwnershipFactIdentity where
  function : Nat
  parameter : Nat
deriving Repr, DecidableEq

/-- Frozen ownership for one explicit target parameter. `identity` binds the
    fact to its callable and parameter slot, `contract` preserves the exact
    final semantic input, and `demand` is computed once at freeze time and is
    the sole adapter oracle thereafter. -/
structure FrozenTargetOwnershipFact where
  identity : TargetOwnershipFactIdentity
  contract : BoundaryContract
  demand : CalleeOwnerDemand
deriving Repr, DecidableEq

/-- Freeze the production `ParamContract::callee_owner_demand` rule. The
    existing RL-2 table supplies ordinary Owned / borrowed-iterator transfer;
    borrowed COW consumption adds the other whole-value case; a projected field
    is admitted only when no whole-value demand is present. `none` is the
    fail-closed contradictory row. -/
def FrozenTargetOwnershipFact.freeze (identity : TargetOwnershipFactIdentity)
    (contract : BoundaryContract)
    (borrowedCowConsumed : Bool) (iterConsumesProjectedField : Option Nat) :
    Option FrozenTargetOwnershipFact :=
  match boundaryTransfersIn contract || borrowedCowConsumed,
      iterConsumesProjectedField with
  | true, some _ => none
  | true, none => some ⟨identity, contract, .wholeValue⟩
  | false, some field => some ⟨identity, contract, .projectedField field⟩
  | false, none => some ⟨identity, contract, .borrow⟩

/-- Ground the freeze operation in the final IC parameter contract plus its
    committed exceptional-transfer facets. Return aliases and sharing-view
    facts remain result-side accounting and do not add pre-entry owner credit. -/
def FrozenTargetOwnershipFact.ofParamContract
    (identity : TargetOwnershipFactIdentity) (pc : ParamContract)
    (iter transfersThroughReturn sharingViewProducer borrowedCowConsumed : Bool)
    (iterConsumesProjectedField : Option Nat) :
    Option FrozenTargetOwnershipFact :=
  FrozenTargetOwnershipFact.freeze identity
    (BoundaryContract.ofParamContract pc iter transfersThroughReturn
      sharingViewProducer)
    borrowedCowConsumed iterConsumesProjectedField

/-- The per-partition callee interface induced by the already-frozen demand.
    Whole-value and exact projected-field owners both transfer one credit on the
    partition currently being checked; a borrow transfers none. This mapping is
    logical normalization, not backend ABI or representation selection. -/
def FrozenTargetOwnershipFact.boundaryContract
    (fact : FrozenTargetOwnershipFact) : BoundaryContract :=
  if fact.demand.requiresCredit then
    ⟨.Owned, false, false, false⟩
  else
    ⟨.Borrowed, false, false, false⟩

/-- One logical adapter credit exactly when the frozen target demand requires
    an independent owner on the selected partition. This is a semantic owner
    obligation, not a physical opcode. -/
def indirectAdapterCreditEvents
    (fact : FrozenTargetOwnershipFact) : List LedgerEvent :=
  if fact.demand.requiresCredit then [.credit] else []

/-- Target-side ownership discharge is path-total: every demanded owner is
    consumed or transferred onward on both normal and unwind; a borrow
    discharges nothing. -/
def indirectTargetDischargeEvents
    (fact : FrozenTargetOwnershipFact) : BoundaryExit → List LedgerEvent
  | .normal | .unwind =>
      if fact.demand.requiresCredit then [.consume] else []

/-- Complete per-class summary for one caller-borrowed explicit operand. -/
def borrowedIndirectOperandEvents
    (fact : FrozenTargetOwnershipFact) (exit : BoundaryExit) : List LedgerEvent :=
  [.read] ++ indirectAdapterCreditEvents fact
    ++ indirectTargetDischargeEvents fact exit

/-- The exact full-contract freeze and adapter tables. Ordinary Owned,
    borrowed iter-consume, and borrowed COW-consume freeze to `wholeValue`;
    an exact projected-field consume freezes to that field; plain Borrowed and
    iter-through-return remain borrows; whole + projected conflict fails closed.
    Both owner-demand shapes receive one credit paired with one discharge on both
    exits, while Borrow remains a read. -/
theorem T4_PV4_borrowed_indirect_exact_adapter_rows :
    let identity : TargetOwnershipFactIdentity := ⟨100, 0⟩
    let owned : BoundaryContract := ⟨.Owned, false, false, false⟩
    let borrowed : BoundaryContract := ⟨.Borrowed, false, false, false⟩
    let iterConsume : BoundaryContract := ⟨.Borrowed, true, false, false⟩
    let iterThroughReturn : BoundaryContract := ⟨.Borrowed, true, true, false⟩
    FrozenTargetOwnershipFact.freeze identity owned false none =
        some ⟨identity, owned, .wholeValue⟩
    ∧ FrozenTargetOwnershipFact.freeze identity borrowed false none =
        some ⟨identity, borrowed, .borrow⟩
    ∧ FrozenTargetOwnershipFact.freeze identity iterConsume false none =
        some ⟨identity, iterConsume, .wholeValue⟩
    ∧ FrozenTargetOwnershipFact.freeze identity iterThroughReturn false none =
        some ⟨identity, iterThroughReturn, .borrow⟩
    ∧ FrozenTargetOwnershipFact.freeze identity borrowed true none =
        some ⟨identity, borrowed, .wholeValue⟩
    ∧ FrozenTargetOwnershipFact.freeze identity borrowed false (some 3) =
        some ⟨identity, borrowed, .projectedField 3⟩
    ∧ FrozenTargetOwnershipFact.freeze identity owned false (some 3) = none
    ∧ borrowedIndirectOperandEvents ⟨identity, owned, .wholeValue⟩ .normal =
        [.read, .credit, .consume]
    ∧ borrowedIndirectOperandEvents ⟨identity, owned, .wholeValue⟩ .unwind =
        [.read, .credit, .consume]
    ∧ borrowedIndirectOperandEvents
        ⟨identity, borrowed, .projectedField 3⟩ .normal =
        [.read, .credit, .consume]
    ∧ borrowedIndirectOperandEvents
        ⟨identity, borrowed, .projectedField 3⟩ .unwind =
        [.read, .credit, .consume]
    ∧ borrowedIndirectOperandEvents ⟨identity, borrowed, .borrow⟩ .normal = [.read]
    ∧ borrowedIndirectOperandEvents ⟨identity, borrowed, .borrow⟩ .unwind = [.read] := by
  decide

/-- Each exact adapter row preserves the caller's retained obligation and its
    read floor on both target exits. -/
theorem T4_PV4_borrowed_indirect_adapter_paths_conform
    (fact : FrozenTargetOwnershipFact) (exit : BoundaryExit) :
    clauseNetZero (borrowedIndirectOperandEvents fact exit) = true
      ∧ clauseFloors 1 (borrowedIndirectOperandEvents fact exit) = true := by
  cases fact with
  | mk identity contract demand =>
      cases demand <;> cases exit <;>
        simp [borrowedIndirectOperandEvents, indirectAdapterCreditEvents,
          indirectTargetDischargeEvents, CalleeOwnerDemand.requiresCredit] <;> decide

/-- Negative witness: target ownership discharge without the exact preceding
    adapter credit consumes the caller's retained obligation. -/
theorem T4_PV4_owner_demand_missing_credit_rejected :
    clauseNetZero [.read, .consume] = false := by
  decide

/-- Negative witness: omitting target discharge on the unwind path leaks the
    adapter credit even though the operand itself was only borrowed. -/
theorem T4_PV4_owner_demand_missing_unwind_discharge_rejected :
    clauseNetZero [.read, .credit] = false := by
  decide

/-- IC-8a integration: a non-enumerable target's CONSERVATIVE access freezes to
    whole-value owner demand, therefore the borrowed indirect adapter must take
    the exact credit/discharge row on normal and unwind. -/
theorem IC8a_borrowed_indirect_adapter_uses_whole_value_target_fact :
    let identity : TargetOwnershipFactIdentity := ⟨200, 0⟩
    let contract := BoundaryContract.ofParamContract
      ParamContract.CONSERVATIVE false false false
    let fact : FrozenTargetOwnershipFact := ⟨identity, contract, .wholeValue⟩
    FrozenTargetOwnershipFact.ofParamContract identity ParamContract.CONSERVATIVE
        false false false false none = some fact
      ∧ fact.demand = .wholeValue
      ∧ borrowedIndirectOperandEvents fact .normal = [.read, .credit, .consume]
      ∧ borrowedIndirectOperandEvents fact .unwind = [.read, .credit, .consume] := by
  decide

/-! ### §T4 / PV-4 per-return-site owner provenance

    Entry adaptation and result ownership are separate questions. A closed
    target freezes one result relation per normal return site, together with an
    exact proof of where the returned owner comes from. `EntryCredit` transfers
    the already-created entry owner; `TargetFunded` is certified by the target's
    logical class ledger at that return site; `NeedsResultCredit` asks the
    adapter to create the returned owner on the normal edge only. No result
    owner exists on unwind. These are logical sources and actions, not a choice
    of counter, layout, instruction, or calling convention. -/

/-- Relationship between one normal return site and a target parameter. The
    parameter slot and semantic field index are stable contract identities, not
    physical offsets. -/
inductive ReturnOwnerRelation
  | independent
  | direct (parameter : Nat)
  | projectedField (parameter field : Nat)
  | contained (parameter : Nat)
deriving Repr, DecidableEq

/-- Whether a related-result relation names this exact target parameter.
    Independent results name no parameter and therefore pass this check. -/
def ReturnOwnerRelation.referencesParameter
    (relation : ReturnOwnerRelation) (parameter : Nat) : Bool :=
  match relation with
  | .independent => true
  | .direct relatedParameter
  | .contained relatedParameter => relatedParameter == parameter
  | .projectedField relatedParameter _ => relatedParameter == parameter

/-- Result topology that one normalized target demand may fund. A whole-value
    owner may fund a direct or contained result, while a projected owner may
    fund only the same semantic field. Borrow has no entry owner to transfer,
    so its related result may be funded independently in any supported shape. -/
def CalleeOwnerDemand.acceptsReturnRelation
    (demand : CalleeOwnerDemand) (relation : ReturnOwnerRelation) : Bool :=
  match demand, relation with
  | _, .independent => true
  | .borrow, .direct _
  | .borrow, .projectedField _ _
  | .borrow, .contained _ => true
  | .wholeValue, .direct _
  | .wholeValue, .contained _ => true
  | .projectedField demandedField, .projectedField _ returnedField =>
      demandedField == returnedField
  | _, _ => false

/-- Stable semantic identity of one result fact. Both function and normal
    return site are in the identity, making cross-function and cross-site fact
    replay structurally visible to every consumer. -/
structure ResultFactIdentity where
  function : Nat
  returnSite : Nat
deriving Repr, DecidableEq

/-- Function and related-parameter compatibility between a frozen entry fact
    and one result relation. Topology equality is checked specifically when the
    result claims `EntryCredit`: a target-funded result may legitimately reshape
    after consuming the entry owner. -/
def FrozenTargetOwnershipFact.matchesResultIdentity
    (entry : FrozenTargetOwnershipFact) (identity : ResultFactIdentity)
    (relation : ReturnOwnerRelation) : Bool :=
  (entry.identity.function == identity.function)
    && relation.referencesParameter entry.identity.parameter

/-- Exclusive logical source of one owned normal result. Target-backed variants
    carry the stable fact identity that funds the result. -/
inductive ResultOwnerSource
  | independentTargetBirth (fact : Nat)
  | entryCredit
  | targetFunded (fact : Nat)
  | needsResultCredit
deriving Repr, DecidableEq

/-- Exact normal-site certificate projected from verified target facts. The two
    optional identities distinguish a fresh target birth from class-ledger
    funding of a result related to an input.
    `targetOwnedRoot` and `targetOwnedPayloadEdges` jointly certify the complete
    returned ownership topology. `entryCreditTransfers` identifies a transfer
    of the adapter-created owner into the result. Entry credit must close exactly
    once on unwind; result absence there is structural in `FunctionExit`. -/
structure ReturnSiteFundingEvidence where
  independentTargetBirthFact : Option Nat
  targetFundingFact : Option Nat
  entryCreditTransfers : Bool
  targetOwnedRoot : Bool
  targetOwnedPayloadEdges : Bool
  entryCreditDischargedOnUnwind : Bool
deriving Repr, DecidableEq

def ReturnSiteFundingEvidence.targetFullyFunds
    (evidence : ReturnSiteFundingEvidence) : Bool :=
  evidence.targetFundingFact.isSome
    && evidence.targetOwnedRoot
    && evidence.targetOwnedPayloadEdges

/-- One normalized result-owner fact keyed by stable function + return-site
    identity. -/
structure FrozenResultOwnerFact where
  identity : ResultFactIdentity
  relation : ReturnOwnerRelation
  source : ResultOwnerSource
deriving Repr, DecidableEq

/-- Freeze one return site's exclusive owner source. Independent target births
    require no entry fact (supporting zero-parameter functions); every related
    result requires exactly one function/parameter-bound entry fact. Multiple
    simultaneous source proofs fail closed rather than minting two owners. -/
def FrozenResultOwnerFact.freeze (identity : ResultFactIdentity)
    (entry : Option FrozenTargetOwnershipFact) (relation : ReturnOwnerRelation)
    (evidence : ReturnSiteFundingEvidence) : Option FrozenResultOwnerFact :=
  match relation, entry with
  | .independent, none =>
      match evidence.independentTargetBirthFact with
      | some fact =>
          if evidence.targetFundingFact.isNone
              && evidence.targetOwnedRoot
              && evidence.targetOwnedPayloadEdges
              && !evidence.entryCreditTransfers
              && !evidence.entryCreditDischargedOnUnwind then
            some ⟨identity, relation, .independentTargetBirth fact⟩
          else
            none
      | none => none
  | .independent, some _ => none
  | _, none => none
  | relation, some entry =>
      if !entry.matchesResultIdentity identity relation then
        none
      else if evidence.independentTargetBirthFact.isSome then
        none
      else
        let entryClaim := evidence.entryCreditTransfers
          || evidence.entryCreditDischargedOnUnwind
        let entrySource := entry.demand.requiresCredit
          && entry.demand.acceptsReturnRelation relation
          && evidence.entryCreditTransfers
          && evidence.entryCreditDischargedOnUnwind
        let targetClaim := evidence.targetFundingFact.isSome
          || evidence.targetOwnedRoot || evidence.targetOwnedPayloadEdges
        let targetSource := evidence.targetFullyFunds
        if entryClaim && !entrySource then
          none
        else if targetClaim && !targetSource then
          none
        else if entrySource && targetSource then
          none
        else if entrySource then
          some ⟨identity, relation, .entryCredit⟩
        else if targetSource then
          match evidence.targetFundingFact with
          | some fact => some ⟨identity, relation, .targetFunded fact⟩
          | none => none
        else
          match relation, entry.demand with
          | .direct _, .borrow
          | .projectedField _ _, .borrow =>
              some ⟨identity, relation, .needsResultCredit⟩
          | _, _ => none

/-! #### Function-total result plans

    A result-owner plan is frozen against an authoritative projection of the
    function CFG's exits and return-ownership analysis. Every normal site has
    exactly one row: either a fact produced by `FrozenResultOwnerFact.freeze`
    or an explicit ownerless proof carrying its stable proof identity and
    reason. Unwind exits have no result-evidence constructor. -/

/-- Stable semantic reason that one normal result requires no owner. -/
inductive OwnerlessResultReason
  | scalarValue
  | borrowedView
  | uninhabited
deriving Repr, DecidableEq

/-- The authoritative requirement for one normal return site. Ownerless rows
    carry the exact proof identity and reason expected from semantic analysis. -/
inductive NormalReturnRequirementKind
  | ownerless (proofIdentity : Nat) (reason : OwnerlessResultReason)
  | owned (relation : ReturnOwnerRelation)
deriving Repr, DecidableEq

def NormalReturnRequirementKind.requiresOwner :
    NormalReturnRequirementKind → Bool
  | .ownerless _ _ => false
  | .owned _ => true

/-- Explicit proof row for a normal result that requires no owner. The proof is
    tied to the analyzed requirement itself rather than a caller-written Bool;
    plan freezing then matches that requirement to the CFG-owned site row. -/
structure OwnerlessResultProof where
  identity : ResultFactIdentity
  requirement : NormalReturnRequirementKind
  proofIdentity : Nat
  reason : OwnerlessResultReason
  requirementIsOwnerless : requirement = .ownerless proofIdentity reason
  provenOwnerless : requirement.requiresOwner = false

structure NormalReturnRequirement where
  identity : ResultFactIdentity
  kind : NormalReturnRequirementKind
deriving Repr, DecidableEq

/-- One frozen normal-return row. The sum makes an Owned/Ownerless category
    mismatch explicit and rejectable rather than encoding absence as a null
    owner fact. -/
inductive FrozenNormalReturnResult
  | ownerless (proof : OwnerlessResultProof)
  | owned (fact : FrozenResultOwnerFact)

/-- Raw per-normal-site evidence consumed by the function-plan freezer. Owned
    evidence cannot inject a preconstructed fact: it must pass the per-site
    source freezer before entering the plan. -/
inductive NormalReturnEvidence
  | ownerless (proof : OwnerlessResultProof)
  | owned (entry : Option FrozenTargetOwnershipFact)
      (relation : ReturnOwnerRelation) (funding : ReturnSiteFundingEvidence)
      (claimed : FrozenResultOwnerFact)

def NormalReturnRequirementKind.isValid :
    NormalReturnRequirementKind → Bool
  | .ownerless proofIdentity _ => !(proofIdentity == 0)
  | .owned _ => true

def NormalReturnRequirementKind.ownerlessProofIdentity? :
    NormalReturnRequirementKind → Option Nat
  | .ownerless proofIdentity _ => some proofIdentity
  | .owned _ => none

/-- Freeze one raw evidence row against its authoritative normal-return
    requirement. Owned rows are certified only through the per-site freezer. -/
def NormalReturnRequirement.freezeEvidence
    (requirement : NormalReturnRequirement)
    (evidence : NormalReturnEvidence) : Option FrozenNormalReturnResult :=
  match requirement.kind, evidence with
  | .ownerless proofIdentity reason, .ownerless proof =>
      if (requirement.identity == proof.identity)
          && (proof.requirement == requirement.kind)
          && (proof.proofIdentity == proofIdentity)
          && !(proofIdentity == 0)
          && (proof.reason == reason) then
        some (.ownerless proof)
      else
        none
  | .owned relation, .owned entry evidenceRelation funding claimed =>
      if relation == evidenceRelation then
        match FrozenResultOwnerFact.freeze requirement.identity entry relation funding with
        | some certified =>
            if certified == claimed then some (.owned certified) else none
        | none => none
      else
        none
  | _, _ => none

/-- Duplicate-free site/proof-identity check used by the frozen plan. -/
def identifiersUnique : List Nat → Bool
  | [] => true
  | identifier :: rest =>
      !(rest.contains identifier) && identifiersUnique rest

/-- The canonical exit projection supplied by the verified function CFG.
    Normal exits carry the authoritative ownership requirement; unwind exits
    deliberately carry no result requirement. -/
inductive FunctionExit
  | normalReturn (site : Nat) (requirement : NormalReturnRequirementKind)
  | unwind (site : Nat)
deriving Repr, DecidableEq

def FunctionExit.site : FunctionExit → Nat
  | .normalReturn site _ | .unwind site => site

structure FunctionExitInventory where
  function : Nat
  exits : List FunctionExit
deriving Repr, DecidableEq

def FunctionExitInventory.exitSites
    (inventory : FunctionExitInventory) : List Nat :=
  inventory.exits.map FunctionExit.site

def FunctionExitInventory.normalReturnRequirements
    (inventory : FunctionExitInventory) : List NormalReturnRequirement :=
  inventory.exits.filterMap fun
    | .normalReturn site requirement =>
        some ⟨⟨inventory.function, site⟩, requirement⟩
    | .unwind _ => none

def FunctionExitInventory.normalReturnSites
    (inventory : FunctionExitInventory) : List Nat :=
  inventory.normalReturnRequirements.map
    (fun requirement => requirement.identity.returnSite)

/-- Freeze every normal row one-for-one. Missing, extra, wrong-kind, and
    forged-owned-source evidence all fail closed. -/
def freezeNormalReturnRows :
    List NormalReturnRequirement → List NormalReturnEvidence →
      Option (List FrozenNormalReturnResult)
  | [], [] => some []
  | requirement :: requirements, evidence :: evidences =>
      match requirement.freezeEvidence evidence,
          freezeNormalReturnRows requirements evidences with
      | some row, some rows => some (row :: rows)
      | _, _ => none
  | _, _ => none

/-- Successful row freezing is exact coverage: the frozen row count equals the
    CFG-derived normal-return requirement count. -/
theorem freezeNormalReturnRows_length
    (requirements : List NormalReturnRequirement)
    (evidences : List NormalReturnEvidence)
    (rows : List FrozenNormalReturnResult)
    (hfreeze : freezeNormalReturnRows requirements evidences = some rows) :
    rows.length = requirements.length := by
  induction requirements generalizing evidences rows with
  | nil =>
      cases evidences <;> simp [freezeNormalReturnRows] at hfreeze
      simp_all
  | cons requirement requirements ih =>
      cases evidences with
      | nil => simp [freezeNormalReturnRows] at hfreeze
      | cons evidence evidences =>
          simp only [freezeNormalReturnRows] at hfreeze
          split at hfreeze <;> simp_all
          rename_i row frozenRows hrow hrows
          subst rows
          rw [List.length_cons, ih evidences frozenRows hrows]

/-- One complete result plan for the CFG-owned exit inventory. -/
structure FrozenFunctionResultPlan where
  function : Nat
  normalReturnSites : List Nat
  requirements : List NormalReturnRequirement
  rows : List FrozenNormalReturnResult

/-- Freeze only an exact, duplicate-free, function-bound normal-return plan.
    Normal sites and requirements are computed from the CFG exit inventory;
    unwind result evidence is unrepresentable. -/
def FrozenFunctionResultPlan.freeze (inventory : FunctionExitInventory)
    (evidences : List NormalReturnEvidence) :
    Option FrozenFunctionResultPlan :=
  let requirements := inventory.normalReturnRequirements
  let proofIdentities := requirements.filterMap
    (fun requirement => requirement.kind.ownerlessProofIdentity?)
  if identifiersUnique inventory.exitSites
      && identifiersUnique proofIdentities
      && requirements.all (fun requirement => requirement.kind.isValid) then
    match freezeNormalReturnRows requirements evidences with
    | some rows =>
        some ⟨inventory.function, inventory.normalReturnSites, requirements, rows⟩
    | none => none
  else
    none

/-- Universal totality barrier: every accepted plan is tied to the exact
    function, canonical normal-site list, and requirements computed from the
    authoritative CFG exit inventory; exit identities are duplicate-free and
    the certified row count covers every normal requirement exactly. -/
theorem FrozenFunctionResultPlan.freeze_preserves_inventory
    (inventory : FunctionExitInventory) (evidences : List NormalReturnEvidence)
    (plan : FrozenFunctionResultPlan)
    (hfreeze : FrozenFunctionResultPlan.freeze inventory evidences = some plan) :
    plan.function = inventory.function
      ∧ plan.normalReturnSites = inventory.normalReturnSites
      ∧ plan.requirements = inventory.normalReturnRequirements
      ∧ identifiersUnique inventory.exitSites = true
      ∧ plan.rows.length = inventory.normalReturnRequirements.length := by
  unfold FrozenFunctionResultPlan.freeze at hfreeze
  dsimp only at hfreeze
  split at hfreeze
  · rename_i hvalid
    split at hfreeze
    · rename_i rows hrows
      simp only [Option.some.injEq] at hfreeze
      subst plan
      have hlength := freezeNormalReturnRows_length
        inventory.normalReturnRequirements evidences rows hrows
      simp_all
    · simp at hfreeze
  · simp at hfreeze

/-- A frozen function plan exposes rows only on normal return. -/
def FrozenFunctionResultPlan.results
    (plan : FrozenFunctionResultPlan) : BoundaryExit → List FrozenNormalReturnResult
  | .normal => plan.rows
  | .unwind => []

theorem T4_PV4_function_result_plan_has_no_unwind_result
    (plan : FrozenFunctionResultPlan) : plan.results .unwind = [] := by
  rfl

/-- The only adapter-owned result action is a logical normal-edge credit when
    neither entry nor target facts fund a Direct/Projected result. -/
inductive LogicalResultOwnerAction
  | none
  | creditReturnedValue
deriving Repr, DecidableEq

def ResultOwnerSource.logicalAction :
    ResultOwnerSource → BoundaryExit → LogicalResultOwnerAction
  | .needsResultCredit, .normal => .creditReturnedValue
  | _, _ => .none

/-- Source accounting for the returned owner itself. Exactly one of these four
    mutually exclusive counts is one for every owned normal result. -/
def ResultOwnerSource.entryTransferCount : ResultOwnerSource → Nat
  | .entryCredit => 1
  | _ => 0

def ResultOwnerSource.independentTargetBirthCount : ResultOwnerSource → Nat
  | .independentTargetBirth _ => 1
  | _ => 0

def ResultOwnerSource.targetFundingCount : ResultOwnerSource → Nat
  | .targetFunded _ => 1
  | _ => 0

def ResultOwnerSource.resultAdapterCreditCount : ResultOwnerSource → BoundaryExit → Nat
  | .needsResultCredit, .normal => 1
  | _, _ => 0

/-- Entry credits exist before the path split and therefore require an exact
    unwind discharge. Target and result-adapter funding are normal-site facts
    and mint nothing on unwind. -/
def ResultOwnerSource.unwindEntryDischargeCount : ResultOwnerSource → Nat
  | .entryCredit => 1
  | _ => 0

/-- Exact freeze rows and exclusive-source arithmetic for Direct, Project, and
    containment. In particular, Project/containment target funding requires
    both the owned root and every owned payload edge; ambiguous double-source
    evidence and unfunded containment fail closed. -/
theorem T4_PV4_result_owner_sources_are_exact :
    let borrowed : FrozenTargetOwnershipFact :=
      ⟨⟨100, 0⟩, ⟨.Borrowed, false, false, false⟩, .borrow⟩
    let whole : FrozenTargetOwnershipFact :=
      ⟨⟨100, 0⟩, ⟨.Owned, false, false, false⟩, .wholeValue⟩
    let noFunding : ReturnSiteFundingEvidence :=
      ⟨none, none, false, false, false, false⟩
    let entryFunding : ReturnSiteFundingEvidence :=
      ⟨none, none, true, false, false, true⟩
    let projectFunding : ReturnSiteFundingEvidence :=
      ⟨none, some 1200, false, true, true, false⟩
    let containmentFunding : ReturnSiteFundingEvidence :=
      ⟨none, some 1400, false, true, true, false⟩
    let doubleFunding : ReturnSiteFundingEvidence :=
      ⟨none, some 1700, true, true, true, true⟩
    let independentFunding : ReturnSiteFundingEvidence :=
      ⟨some 1900, none, false, true, true, false⟩
    FrozenResultOwnerFact.freeze ⟨100, 10⟩ (some whole) (.direct 0) entryFunding =
        some ⟨⟨100, 10⟩, .direct 0, .entryCredit⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 11⟩ (some borrowed) (.direct 0) noFunding =
        some ⟨⟨100, 11⟩, .direct 0, .needsResultCredit⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 12⟩ (some borrowed) (.projectedField 0 3)
        projectFunding = some ⟨⟨100, 12⟩, .projectedField 0 3, .targetFunded 1200⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 13⟩ (some borrowed) (.projectedField 0 3)
        noFunding = some ⟨⟨100, 13⟩, .projectedField 0 3, .needsResultCredit⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 14⟩ (some borrowed) (.contained 0)
        containmentFunding = some ⟨⟨100, 14⟩, .contained 0, .targetFunded 1400⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 15⟩ (some borrowed) (.contained 0) noFunding = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 16⟩ (some whole) (.contained 0) entryFunding =
        some ⟨⟨100, 16⟩, .contained 0, .entryCredit⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 17⟩ (some whole) (.direct 0) doubleFunding = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 18⟩ (some whole) (.direct 0)
        ⟨none, none, false, false, false, true⟩ = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 19⟩ none .independent independentFunding =
        some ⟨⟨100, 19⟩, .independent, .independentTargetBirth 1900⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 20⟩ none .independent noFunding = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 21⟩ none .independent
        ⟨some 2100, none, false, true, false, false⟩ = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 22⟩ (some borrowed) (.direct 0)
        independentFunding = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 23⟩ (some borrowed) (.projectedField 0 3)
        ⟨none, some 2300, false, true, false, false⟩ = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 24⟩ (some borrowed) (.direct 0)
        ⟨none, none, true, false, false, true⟩ = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 25⟩ (some whole) (.direct 0)
        ⟨none, none, true, false, false, false⟩ = none := by
  decide

/-- Entry-credit funding is tied to the exact target demand topology and to
    stable function/parameter identities. Projected fields cannot fund another
    field or a direct result, whole-value owners cannot masquerade as projected
    owners, and facts cannot cross parameter, site, or function identities. -/
theorem T4_PV4_result_relation_topology_and_identity_are_exact :
    let whole : FrozenTargetOwnershipFact :=
      ⟨⟨100, 0⟩, ⟨.Owned, false, false, false⟩, .wholeValue⟩
    let projected : FrozenTargetOwnershipFact :=
      ⟨⟨100, 0⟩, ⟨.Borrowed, false, false, false⟩,
        .projectedField 3⟩
    let entryFunding : ReturnSiteFundingEvidence :=
      ⟨none, none, true, false, false, true⟩
    let targetFunding : ReturnSiteFundingEvidence :=
      ⟨none, some 4100, false, true, true, false⟩
    let independentFunding : ReturnSiteFundingEvidence :=
      ⟨some 4200, none, false, true, true, false⟩
    FrozenResultOwnerFact.freeze ⟨100, 30⟩ (some whole) (.direct 0) entryFunding =
        some ⟨⟨100, 30⟩, .direct 0, .entryCredit⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 31⟩ (some whole) (.contained 0)
        entryFunding = some ⟨⟨100, 31⟩, .contained 0, .entryCredit⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 32⟩ (some projected)
        (.projectedField 0 3) entryFunding =
          some ⟨⟨100, 32⟩, .projectedField 0 3, .entryCredit⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 33⟩ (some projected)
        (.projectedField 0 4) entryFunding = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 34⟩ (some projected) (.direct 0)
        entryFunding = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 35⟩ (some whole)
        (.projectedField 0 3) entryFunding = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 36⟩ (some whole) (.direct 1)
        entryFunding = none
    ∧ FrozenResultOwnerFact.freeze ⟨101, 37⟩ (some whole) (.direct 0)
        entryFunding = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 38⟩ (some whole) (.direct 0)
        entryFunding ≠ FrozenResultOwnerFact.freeze ⟨100, 39⟩ (some whole)
          (.direct 0) entryFunding
    ∧ FrozenResultOwnerFact.freeze ⟨100, 40⟩ (some whole)
        (.projectedField 0 3) targetFunding =
          some ⟨⟨100, 40⟩, .projectedField 0 3, .targetFunded 4100⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 41⟩ (some projected)
        (.direct 0) targetFunding =
          some ⟨⟨100, 41⟩, .direct 0, .targetFunded 4100⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 42⟩ (some projected)
        (.projectedField 0 4) targetFunding =
          some ⟨⟨100, 42⟩, .projectedField 0 4, .targetFunded 4100⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 43⟩ none .independent
        independentFunding =
          some ⟨⟨100, 43⟩, .independent, .independentTargetBirth 4200⟩
    ∧ FrozenResultOwnerFact.freeze ⟨100, 44⟩ (some whole) .independent
        independentFunding = none
    ∧ FrozenResultOwnerFact.freeze ⟨100, 45⟩ none (.direct 0)
        entryFunding = none := by
  decide

/-- A production-shaped two-return function (one ownerless scalar and one
    owned related result) freezes exactly. Every malformed coverage, identity,
    category, proof, or unwind row fails closed. This matrix pins the plan's
    totality rather than proving only an isolated owned site. -/
theorem T4_PV4_function_result_plan_is_total_and_exact :
    let ownerlessKind : NormalReturnRequirementKind :=
      .ownerless 7001 .scalarValue
    let ownedKind : NormalReturnRequirementKind := .owned (.direct 0)
    let inventory : FunctionExitInventory :=
      ⟨100, [.normalReturn 10 ownerlessKind, .normalReturn 11 ownedKind,
        .unwind 12]⟩
    let duplicateInventory : FunctionExitInventory :=
      ⟨100, [.normalReturn 10 ownerlessKind, .normalReturn 10 ownerlessKind]⟩
    let zeroProofInventory : FunctionExitInventory :=
      ⟨100, [.normalReturn 10 (.ownerless 0 .scalarValue)]⟩
    let borrowed : FrozenTargetOwnershipFact :=
      ⟨⟨100, 0⟩, ⟨.Borrowed, false, false, false⟩, .borrow⟩
    let targetFunding : ReturnSiteFundingEvidence :=
      ⟨none, some 9000, false, true, true, false⟩
    let ownerlessProof : OwnerlessResultProof :=
      ⟨⟨100, 10⟩, ownerlessKind, 7001, .scalarValue, rfl, rfl⟩
    let ownerlessEvidence : NormalReturnEvidence := .ownerless ownerlessProof
    let ownedFact : FrozenResultOwnerFact :=
      ⟨⟨100, 11⟩, .direct 0, .targetFunded 9000⟩
    let ownedEvidence : NormalReturnEvidence :=
      .owned (some borrowed) (.direct 0) targetFunding ownedFact
    let extraNormalEvidence : NormalReturnEvidence :=
      .ownerless ⟨⟨100, 13⟩, .ownerless 7003 .scalarValue, 7003,
        .scalarValue, rfl, rfl⟩
    let unwindEvidence : NormalReturnEvidence :=
      .ownerless ⟨⟨100, 12⟩, .ownerless 7004 .scalarValue, 7004,
        .scalarValue, rfl, rfl⟩
    let wrongFunctionOwned : NormalReturnEvidence :=
      .owned (some borrowed) (.direct 0) targetFunding
        ⟨⟨101, 11⟩, .direct 0, .targetFunded 9000⟩
    let wrongSiteOwned : NormalReturnEvidence :=
      .owned (some borrowed) (.direct 0) targetFunding
        ⟨⟨100, 12⟩, .direct 0, .targetFunded 9000⟩
    let ownerlessAtOwnedSite : NormalReturnEvidence :=
      .ownerless ⟨⟨100, 11⟩, .ownerless 7002 .borrowedView, 7002,
        .borrowedView, rfl, rfl⟩
    let ownedAtOwnerlessSite : NormalReturnEvidence :=
      .owned (some borrowed) (.direct 0) targetFunding
        ⟨⟨100, 10⟩, .direct 0, .targetFunded 9000⟩
    let zeroProofEvidence : NormalReturnEvidence :=
      .ownerless ⟨⟨100, 10⟩, .ownerless 0 .scalarValue, 0,
        .scalarValue, rfl, rfl⟩
    let wrongProofIdentityEvidence : NormalReturnEvidence :=
      .ownerless ⟨⟨100, 10⟩, .ownerless 7002 .scalarValue, 7002,
        .scalarValue, rfl, rfl⟩
    let wrongReasonEvidence : NormalReturnEvidence :=
      .ownerless ⟨⟨100, 10⟩, .ownerless 7001 .borrowedView, 7001,
        .borrowedView, rfl, rfl⟩
    let forgedSourceEvidence : NormalReturnEvidence :=
      .owned (some borrowed) (.direct 0) targetFunding
        ⟨⟨100, 11⟩, .direct 0, .entryCredit⟩
    (FrozenFunctionResultPlan.freeze inventory
        [ownerlessEvidence, ownedEvidence]).isSome = true
    ∧ (FrozenFunctionResultPlan.freeze inventory
        [ownerlessEvidence]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze inventory
        [ownerlessEvidence, ownedEvidence, extraNormalEvidence]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze duplicateInventory
        [ownerlessEvidence, ownerlessEvidence]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze inventory
        [ownerlessEvidence, ownerlessEvidence]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze inventory
        [ownerlessEvidence, wrongFunctionOwned]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze inventory
        [ownerlessEvidence, wrongSiteOwned]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze inventory
        [ownerlessEvidence, ownerlessAtOwnedSite]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze inventory
        [ownedAtOwnerlessSite, ownedEvidence]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze zeroProofInventory
        [zeroProofEvidence]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze inventory
        [wrongProofIdentityEvidence, ownedEvidence]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze inventory
        [wrongReasonEvidence, ownedEvidence]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze inventory
        [ownerlessEvidence, ownedEvidence, unwindEvidence]).isSome = false
    ∧ (FrozenFunctionResultPlan.freeze inventory
        [ownerlessEvidence, forgedSourceEvidence]).isSome = false := by
  decide

/-- Every owned result has exactly one logical normal-path owner source, no
    result-adapter credit on unwind, and an unwind discharge exactly when the
    source was the entry credit. -/
theorem T4_PV4_result_owner_source_accounting_is_exact
    (source : ResultOwnerSource) :
    source.independentTargetBirthCount + source.entryTransferCount
          + source.targetFundingCount
          + source.resultAdapterCreditCount .normal = 1
      ∧ source.resultAdapterCreditCount .unwind = 0
      ∧ (source.logicalAction .normal = .creditReturnedValue ↔
          source.resultAdapterCreditCount .normal = 1)
      ∧ source.logicalAction .unwind = .none
      ∧ source.unwindEntryDischargeCount = source.entryTransferCount := by
  cases source <;> simp [ResultOwnerSource.independentTargetBirthCount,
    ResultOwnerSource.entryTransferCount,
    ResultOwnerSource.targetFundingCount,
    ResultOwnerSource.resultAdapterCreditCount,
    ResultOwnerSource.logicalAction,
    ResultOwnerSource.unwindEntryDischargeCount]

/-- §T4 (P1) the boundary expansion never places a release: no
    `burdenDec` appears in `boundaryInstrs` for ANY contract — the RL-34
    no-post-tail-call-dec law at the boundary level (the transfer rows hand
    the obligation off; the borrow rows leave the caller's placed release
    where the caller's own three clauses govern it). -/
theorem T4_boundary_places_no_release (arg ret : Nat) (bc : BoundaryContract) :
    (boundaryInstrs arg ret bc).all
      (fun i => match i with | .burdenDec _ => false | _ => true) = true := by
  unfold boundaryInstrs boundaryResultInstrs
  cases bc.transfersThroughReturn || bc.sharingViewProducer <;> rfl

/-- §T4 (P1) the RL-34 grounding: an Owned callee param is the
    `transferOwnership` tail-call action (never a post-call dec), and the
    Owned boundary contract computes transfer-in through the SAME committed
    RL-2 table verdict. -/
theorem T4_owned_boundary_matches_rl34 (iter ttr view : Bool) :
    rl34_action .Owned = TailCallAction.transferOwnership
    ∧ boundaryTransfersIn ⟨.Owned, iter, ttr, view⟩ = true :=
  ⟨rfl, rfl⟩

/-! ## §T4 Part B — the callee side: contract-mandated opening + conformance

    The callee's ledger for the param's class opens with the transferred-in
    reference EXACTLY when the boundary use kind transfers ownership in — the
    same committed RL-2 table lookup that classified the caller side. The
    OWNED-PARAM = BIRTH row: the opening is a `construct` of the param (the
    allocation enters the callee's scope funding its ledger). A borrowed
    contract has no opening; the callee body is checked under the BORROW
    ASSUMPTION (entry count 1 — the caller's retained live reference) and
    must net zero (the callee releases nothing it does not own). -/

/-- §T4 the contract-mandated callee-side opening: the transferred-in
    reference births the class (owned / iter-consume rows); a borrow opens
    nothing. -/
def calleeOpening (p : Nat) (bc : BoundaryContract) : List LedgerInstr :=
  if boundaryTransfersIn bc then [.construct p] else []

/-- §T4 the callee's derived per-param-class ledger under the opening. -/
def calleeLedger (cc : Nat → Nat) (p : Nat) (bc : BoundaryContract)
    (body : List LedgerInstr) : List LedgerEvent :=
  deriveLedger cc (cc p) (calleeOpening p bc ++ body)

/-- §T4 the callee's OWN invariant at the boundary: a transfer-in contract
    demands the three clauses over the opened ledger (birth + body); a
    borrowed contract demands net zero + the floor clauses from the borrow
    assumption count 1. -/
def calleeConforms (cc : Nat → Nat) (p : Nat) (bc : BoundaryContract)
    (body : List LedgerInstr) : Bool :=
  if boundaryTransfersIn bc then
    threeClauses (calleeLedger cc p bc body)
  else
    clauseNetZero (calleeLedger cc p bc body)
      && clauseFloors 1 (calleeLedger cc p bc body)

/-- §T4 (P1) OWNED PARAM = BIRTH: on a transfer-in contract the callee ledger
    opens with the birth of the transferred-in reference. -/
theorem T4_owned_param_births_callee_ledger (cc : Nat → Nat) (p : Nat)
    (bc : BoundaryContract) (body : List LedgerInstr)
    (htr : boundaryTransfersIn bc = true) :
    calleeLedger cc p bc body = .birth :: deriveLedger cc (cc p) body := by
  unfold calleeLedger calleeOpening
  rw [htr]
  show deriveLedger cc (cc p) (.construct p :: body) = _
  show (if cc p == cc p then
          LedgerEvent.birth :: deriveLedger cc (cc p) body
        else deriveLedger cc (cc p) body) = _
  rw [beq_self_eq_true (cc p)]
  rfl

/-- §T4 a borrowed contract's callee ledger is the body's derivation (no
    opening). -/
theorem calleeLedger_borrowed (cc : Nat → Nat) (p : Nat)
    (bc : BoundaryContract) (body : List LedgerInstr)
    (htr : boundaryTransfersIn bc = false) :
    calleeLedger cc p bc body = deriveLedger cc (cc p) body := by
  unfold calleeLedger calleeOpening
  rw [htr]
  rfl

/-- §T4 (P2) the INTERFACE-COMPLEMENT extraction: a conforming callee's body
    ledger nets EXACTLY what the caller-side summary event nets (transfer-in:
    -1, the handed-off obligation discharged inside; borrowed: 0, nothing
    released), and its floor clauses hold from the entry count 1 (the
    transferred-in reference / the borrow assumption). The contract is a
    SOUND SUMMARY: net and floors of the callee body are pinned by the
    callee's OWN invariant — no callee re-derivation at the composition. -/
theorem calleeConforms_interface (cc : Nat → Nat) (p : Nat)
    (bc : BoundaryContract) (body : List LedgerInstr)
    (h : calleeConforms cc p bc body = true) :
    ledgerNet (deriveLedger cc (cc p) body) = ledgerNet [boundaryArgEvent bc]
    ∧ clauseFloors 1 (deriveLedger cc (cc p) body) = true := by
  unfold calleeConforms at h
  rw [boundaryArgEvent_net bc]
  cases htr : boundaryTransfersIn bc with
  | true =>
      rw [htr] at h
      simp only [reduceIte] at h ⊢
      rw [T4_owned_param_births_callee_ledger cc p bc body htr] at h
      have h' : (clauseNetZero (LedgerEvent.birth :: deriveLedger cc (cc p) body)
          && clauseFloors 0 (LedgerEvent.birth :: deriveLedger cc (cc p) body)) = true := h
      rw [Bool.and_eq_true] at h'
      obtain ⟨hnet, hfloors⟩ := h'
      constructor
      · have hnetv : (ledgerNet (LedgerEvent.birth :: deriveLedger cc (cc p) body) == 0)
            = true := hnet
        have hnete : ledgerNet (LedgerEvent.birth :: deriveLedger cc (cc p) body) = 0 :=
          beq_iff_eq.mp hnetv
        have hcons : ledgerNet (LedgerEvent.birth :: deriveLedger cc (cc p) body)
            = 1 + ledgerNet (deriveLedger cc (cc p) body) := by
          show ((LedgerEvent.birth :: _).map eventDelta).foldr (· + ·) 0 = _
          rw [List.map_cons, List.foldr_cons]
          rfl
        rw [hcons] at hnete
        omega
      · have hfl : (eventFloor 0 .birth
            && clauseFloors (0 + eventDelta .birth) (deriveLedger cc (cc p) body))
            = true := hfloors
        rw [Bool.and_eq_true] at hfl
        have harith : (0 : Int) + eventDelta .birth = 1 := by
          show (0 : Int) + 1 = 1
          omega
        rw [harith] at hfl
        exact hfl.2
  | false =>
      rw [htr] at h
      rw [if_neg (by decide : ¬(false = true))] at h
      rw [if_neg (by decide : ¬(false = true))]
      rw [calleeLedger_borrowed cc p bc body htr] at h
      rw [Bool.and_eq_true] at h
      obtain ⟨hnet, hfloors⟩ := h
      constructor
      · have hnetv : (ledgerNet (deriveLedger cc (cc p) body) == 0) = true := hnet
        have hnete : ledgerNet (deriveLedger cc (cc p) body) = 0 :=
          beq_iff_eq.mp hnetv
        rw [hnete]
      · exact hfloors

/-! ## §T4 floor monotonicity

    This is declared before both direct and indirect adapter composition so
    each boundary can lift a target's entry-count-1 floor proof to the actual
    caller-side count. -/

/-- §T4 the floor clauses are monotone in the start count: every floor an
    event demands (`1 ≤ n` at a READ, `1 + sibs ≤ n` at a MUTATE) is upward-
    closed, and the running count shifts uniformly. -/
theorem clauseFloors_mono :
    ∀ (es : List LedgerEvent) (m n : Int), m ≤ n →
      clauseFloors m es = true → clauseFloors n es = true := by
  intro es
  induction es with
  | nil => intro m n _ _; rfl
  | cons e rest ih =>
      intro m n hmn h
      have h' : (eventFloor m e && clauseFloors (m + eventDelta e) rest) = true := h
      rw [Bool.and_eq_true] at h'
      obtain ⟨hf, hrest⟩ := h'
      show (eventFloor n e && clauseFloors (n + eventDelta e) rest) = true
      rw [Bool.and_eq_true]
      refine ⟨?_, ih (m + eventDelta e) (n + eventDelta e) (by omega) hrest⟩
      cases e with
      | read =>
          have h1 : (1 : Int) ≤ m := of_decide_eq_true hf
          exact decide_eq_true (by omega)
      | mutate sibs =>
          have h1 : (1 : Int) + (sibs : Int) ≤ m := of_decide_eq_true hf
          exact decide_eq_true (by omega)
      | birth => rfl
      | credit => rfl
      | consume => rfl

/-! ### §T4 / PV-4 borrowed indirect-call composition

    The exact adapter row above is also a genuine contract boundary. The
    target body is consumed through `calleeConforms` using the boundary
    carrier normalized from the frozen owner demand. The caller-side READ stays
    live; a whole-value or exact projected-field demand receives one adapter
    CREDIT before its body, while a borrow executes directly. -/

/-- Caller-visible events for one borrowed explicit operand composed with the
    target's realized per-param body ledger. -/
def borrowedIndirectComposedEvents
    (cc : Nat → Nat) (p : Nat) (fact : FrozenTargetOwnershipFact)
    (body : List LedgerInstr) : List LedgerEvent :=
  [.read] ++ indirectAdapterCreditEvents fact
    ++ deriveLedger cc (cc p) body

/-- The local adapter segment preserves the caller's incoming reference: net
    zero and every read/mutate floor holds from the borrowed-entry count 1. -/
def borrowedIndirectSegmentConforms (events : List LedgerEvent) : Bool :=
  clauseNetZero events && clauseFloors 1 events

/-- PV-4 adapter composition over frozen IC ownership facts. The target body
    is never re-derived: `calleeConforms_interface` supplies its exact net and
    floors. The adapter adds exactly the complementary credit for either
    whole-value or exact projected-field owner demand and no credit for Borrow,
    preserving the caller-borrowed segment. -/
theorem T4_PV4_borrowed_indirect_adapter_composition_sound
    (fact : FrozenTargetOwnershipFact) (cc : Nat → Nat) (p : Nat)
    (body : List LedgerInstr)
    (hcallee : calleeConforms cc p
      fact.boundaryContract body = true) :
    borrowedIndirectSegmentConforms
      (borrowedIndirectComposedEvents cc p fact body) = true := by
  have hinterface := calleeConforms_interface cc p
    fact.boundaryContract body hcallee
  obtain ⟨hnet, hfloors⟩ := hinterface
  obtain ⟨identity, contract, demand⟩ := fact
  cases demand with
  | borrow =>
      have hnet0 : ledgerNet (deriveLedger cc (cc p) body) = 0 := by
        simpa [FrozenTargetOwnershipFact.boundaryContract,
          CalleeOwnerDemand.requiresCredit, boundaryArgEvent,
          boundaryTransfersIn, boundaryUseKind,
          rl2_use_transfers_ownership, ledgerNet] using hnet
      unfold borrowedIndirectSegmentConforms
      rw [Bool.and_eq_true]
      constructor
      · apply beq_iff_eq.mpr
        change eventDelta .read + ledgerNet (deriveLedger cc (cc p) body) = 0
        rw [hnet0]
        rfl
      · change (eventFloor 1 .read
          && clauseFloors (1 + eventDelta .read)
            (deriveLedger cc (cc p) body)) = true
        rw [Bool.and_eq_true]
        exact ⟨by decide, by simpa using hfloors⟩
  | wholeValue =>
      have hnetNeg : ledgerNet (deriveLedger cc (cc p) body) = -1 := by
        simpa [FrozenTargetOwnershipFact.boundaryContract,
          CalleeOwnerDemand.requiresCredit, boundaryArgEvent,
          boundaryTransfersIn, boundaryUseKind,
          rl2_use_transfers_ownership, ledgerNet] using hnet
      unfold borrowedIndirectSegmentConforms
      rw [Bool.and_eq_true]
      constructor
      · apply beq_iff_eq.mpr
        change eventDelta .read +
          (eventDelta .credit + ledgerNet (deriveLedger cc (cc p) body)) = 0
        rw [hnetNeg]
        rfl
      · have hfloors2 := clauseFloors_mono
          (deriveLedger cc (cc p) body) 1 2 (by omega) hfloors
        change (eventFloor 1 .read
          && (eventFloor (1 + eventDelta .read) .credit
          && clauseFloors (1 + eventDelta .read + eventDelta .credit)
            (deriveLedger cc (cc p) body))) = true
        rw [Bool.and_eq_true, Bool.and_eq_true]
        exact ⟨by decide, by decide, by simpa using hfloors2⟩
  | projectedField field =>
      have hnetNeg : ledgerNet (deriveLedger cc (cc p) body) = -1 := by
        simpa [FrozenTargetOwnershipFact.boundaryContract,
          CalleeOwnerDemand.requiresCredit, boundaryArgEvent,
          boundaryTransfersIn, boundaryUseKind,
          rl2_use_transfers_ownership, ledgerNet] using hnet
      unfold borrowedIndirectSegmentConforms
      rw [Bool.and_eq_true]
      constructor
      · apply beq_iff_eq.mpr
        change eventDelta .read +
          (eventDelta .credit + ledgerNet (deriveLedger cc (cc p) body)) = 0
        rw [hnetNeg]
        rfl
      · have hfloors2 := clauseFloors_mono
          (deriveLedger cc (cc p) body) 1 2 (by omega) hfloors
        change (eventFloor 1 .read
          && (eventFloor (1 + eventDelta .read) .credit
          && clauseFloors (1 + eventDelta .read + eventDelta .credit)
            (deriveLedger cc (cc p) body))) = true
        rw [Bool.and_eq_true, Bool.and_eq_true]
        exact ⟨by decide, by decide, by simpa using hfloors2⟩

/-- The same frozen target fact and composition theorem govern both exit
    paths. Each path supplies its own target conformance proof; no normal-only
    ownership assumption can discharge the unwind obligation. -/
theorem T4_PV4_borrowed_indirect_adapter_composes_normal_and_unwind
    (fact : FrozenTargetOwnershipFact) (cc : Nat → Nat) (p : Nat)
    (normalBody unwindBody : List LedgerInstr)
    (hnormal : calleeConforms cc p
      fact.boundaryContract normalBody = true)
    (hunwind : calleeConforms cc p
      fact.boundaryContract unwindBody = true) :
    borrowedIndirectSegmentConforms
      (borrowedIndirectComposedEvents cc p fact normalBody) = true
    ∧ borrowedIndirectSegmentConforms
      (borrowedIndirectComposedEvents cc p fact unwindBody) = true :=
  ⟨T4_PV4_borrowed_indirect_adapter_composition_sound fact cc p normalBody hnormal,
   T4_PV4_borrowed_indirect_adapter_composition_sound fact cc p unwindBody hunwind⟩

/-- Every frozen owner source presents one owned result at a normal return and
    no result at unwind. For `NeedsResultCredit` this credit is the adapter's
    logical action; for the other rows it records the certified owner crossing
    the boundary rather than minting another owner. -/
def resultOwnerArrivalEvents
    (_source : ResultOwnerSource) : BoundaryExit → List LedgerEvent
  | .normal => [.credit]
  | .unwind => []

/-- Result arrival for the complete normal-row sum. Ownerless rows add no
    logical owner; owned rows add exactly the certified result owner. Neither
    row kind can produce an unwind result. -/
def normalReturnResultArrivalEvents
    (row : FrozenNormalReturnResult) (exit : BoundaryExit) : List LedgerEvent :=
  match row with
  | .ownerless _ => []
  | .owned fact => resultOwnerArrivalEvents fact.source exit

def FrozenNormalReturnResult.ownerCount : FrozenNormalReturnResult → Nat
  | .ownerless _ => 0
  | .owned _ => 1

theorem T4_PV4_normal_return_row_arrival_is_exact
    (row : FrozenNormalReturnResult) :
    ledgerNet (normalReturnResultArrivalEvents row .normal) = row.ownerCount
      ∧ normalReturnResultArrivalEvents row .unwind = [] := by
  cases row <;> constructor <;> rfl

/-- Full borrowed indirect-call path: entry adaptation, the target's verified
    body ledger, and the frozen return site's owner arrival. This semantic event
    stream is independent of any backend instruction or counter realization. -/
def borrowedIndirectCallableEvents
    (cc : Nat → Nat) (p : Nat) (entry : FrozenTargetOwnershipFact)
    (result : FrozenResultOwnerFact) (body : List LedgerInstr)
    (exit : BoundaryExit) : List LedgerEvent :=
  borrowedIndirectComposedEvents cc p entry body
    ++ resultOwnerArrivalEvents result.source exit

/-- Callable composition over the complete Ownerless/Owned result-row sum. -/
def borrowedIndirectCallableRowEvents
    (cc : Nat → Nat) (p : Nat) (entry : FrozenTargetOwnershipFact)
    (row : FrozenNormalReturnResult) (body : List LedgerInstr)
    (exit : BoundaryExit) : List LedgerEvent :=
  borrowedIndirectComposedEvents cc p entry body
    ++ normalReturnResultArrivalEvents row exit

/-- Every certified row composes with the balanced borrowed-indirect segment:
    ownerless normal results add net zero, owned normal results add net one,
    and unwind always adds zero. Floor safety is preserved on both exits. -/
theorem T4_PV4_normal_return_row_callable_composition_sound
    (entry : FrozenTargetOwnershipFact) (row : FrozenNormalReturnResult)
    (cc : Nat → Nat) (p : Nat) (normalBody unwindBody : List LedgerInstr)
    (hnormal : calleeConforms cc p entry.boundaryContract normalBody = true)
    (hunwind : calleeConforms cc p entry.boundaryContract unwindBody = true) :
    ledgerNet (borrowedIndirectCallableRowEvents cc p entry row
        normalBody .normal) = row.ownerCount
      ∧ clauseFloors 1 (borrowedIndirectCallableRowEvents cc p entry row
          normalBody .normal) = true
      ∧ ledgerNet (borrowedIndirectCallableRowEvents cc p entry row
          unwindBody .unwind) = 0
      ∧ clauseFloors 1 (borrowedIndirectCallableRowEvents cc p entry row
          unwindBody .unwind) = true := by
  have hnormalSegment :=
    T4_PV4_borrowed_indirect_adapter_composition_sound
      entry cc p normalBody hnormal
  have hunwindSegment :=
    T4_PV4_borrowed_indirect_adapter_composition_sound
      entry cc p unwindBody hunwind
  unfold borrowedIndirectSegmentConforms at hnormalSegment hunwindSegment
  rw [Bool.and_eq_true] at hnormalSegment hunwindSegment
  obtain ⟨hnormalNetBool, hnormalFloors⟩ := hnormalSegment
  obtain ⟨hunwindNetBool, hunwindFloors⟩ := hunwindSegment
  have hnormalNet : ledgerNet
      (borrowedIndirectComposedEvents cc p entry normalBody) = 0 :=
    beq_iff_eq.mp hnormalNetBool
  have hunwindNet : ledgerNet
      (borrowedIndirectComposedEvents cc p entry unwindBody) = 0 :=
    beq_iff_eq.mp hunwindNetBool
  cases row with
  | ownerless proof =>
      simpa [borrowedIndirectCallableRowEvents,
        normalReturnResultArrivalEvents, FrozenNormalReturnResult.ownerCount]
        using And.intro hnormalNet
          (And.intro hnormalFloors (And.intro hunwindNet hunwindFloors))
  | owned fact =>
      refine ⟨?_, ?_, ?_, ?_⟩
      · unfold borrowedIndirectCallableRowEvents normalReturnResultArrivalEvents
          resultOwnerArrivalEvents FrozenNormalReturnResult.ownerCount
        rw [ledgerNet_append, hnormalNet]
        rfl
      · unfold borrowedIndirectCallableRowEvents normalReturnResultArrivalEvents
          resultOwnerArrivalEvents
        rw [clauseFloors_append, Bool.and_eq_true]
        refine ⟨hnormalFloors, ?_⟩
        rw [hnormalNet]
        rfl
      · simpa [borrowedIndirectCallableRowEvents,
          normalReturnResultArrivalEvents, resultOwnerArrivalEvents]
          using hunwindNet
      · simpa [borrowedIndirectCallableRowEvents,
          normalReturnResultArrivalEvents, resultOwnerArrivalEvents]
          using hunwindFloors

/-- PV-4 end-to-end callable composition from one complete frozen contract.
    A valid return-site freeze preserves its stable identity, every normal path
    exports exactly one owned result, and every unwind path exports none. Entry,
    target, fresh-target-birth, and adapter funding stay mutually exclusive;
    only the entry-funded row carries an unwind discharge obligation. -/
theorem T4_PV4_borrowed_indirect_callable_composition_sound
    (entry : FrozenTargetOwnershipFact) (identity : ResultFactIdentity)
    (relation : ReturnOwnerRelation) (evidence : ReturnSiteFundingEvidence)
    (result : FrozenResultOwnerFact) (cc : Nat → Nat) (p : Nat)
    (normalBody unwindBody : List LedgerInstr)
    (hresult : FrozenResultOwnerFact.freeze identity (some entry) relation evidence =
      some result)
    (hnormal : calleeConforms cc p
      entry.boundaryContract normalBody = true)
    (hunwind : calleeConforms cc p
      entry.boundaryContract unwindBody = true) :
    FrozenResultOwnerFact.freeze identity (some entry) relation evidence = some result
      ∧ ledgerNet (borrowedIndirectCallableEvents cc p entry result
          normalBody .normal) = 1
      ∧ clauseFloors 1 (borrowedIndirectCallableEvents cc p entry result
          normalBody .normal) = true
      ∧ ledgerNet (borrowedIndirectCallableEvents cc p entry result
          unwindBody .unwind) = 0
      ∧ clauseFloors 1 (borrowedIndirectCallableEvents cc p entry result
          unwindBody .unwind) = true
      ∧ result.source.independentTargetBirthCount
          + result.source.entryTransferCount
          + result.source.targetFundingCount
          + result.source.resultAdapterCreditCount .normal = 1
      ∧ result.source.resultAdapterCreditCount .unwind = 0
      ∧ (result.source.logicalAction .normal = .creditReturnedValue ↔
          result.source.resultAdapterCreditCount .normal = 1)
      ∧ result.source.logicalAction .unwind = .none
      ∧ result.source.unwindEntryDischargeCount =
          result.source.entryTransferCount := by
  have hnormalSegment :=
    T4_PV4_borrowed_indirect_adapter_composition_sound
      entry cc p normalBody hnormal
  have hunwindSegment :=
    T4_PV4_borrowed_indirect_adapter_composition_sound
      entry cc p unwindBody hunwind
  unfold borrowedIndirectSegmentConforms at hnormalSegment hunwindSegment
  rw [Bool.and_eq_true] at hnormalSegment hunwindSegment
  obtain ⟨hnormalNetBool, hnormalFloors⟩ := hnormalSegment
  obtain ⟨hunwindNetBool, hunwindFloors⟩ := hunwindSegment
  have hnormalNet : ledgerNet
      (borrowedIndirectComposedEvents cc p entry normalBody) = 0 :=
    beq_iff_eq.mp hnormalNetBool
  have hunwindNet : ledgerNet
      (borrowedIndirectComposedEvents cc p entry unwindBody) = 0 :=
    beq_iff_eq.mp hunwindNetBool
  obtain ⟨hsource, hnoUnwindCredit, hnormalAction, hnoUnwindAction,
      hentryDischarge⟩ :=
    T4_PV4_result_owner_source_accounting_is_exact result.source
  refine ⟨hresult, ?_, ?_, ?_, ?_, hsource, hnoUnwindCredit,
    hnormalAction, hnoUnwindAction, hentryDischarge⟩
  · unfold borrowedIndirectCallableEvents
    rw [ledgerNet_append, hnormalNet]
    rfl
  · unfold borrowedIndirectCallableEvents
    rw [clauseFloors_append, Bool.and_eq_true]
    refine ⟨hnormalFloors, ?_⟩
    rw [hnormalNet]
    rfl
  · simpa [borrowedIndirectCallableEvents, resultOwnerArrivalEvents]
      using hunwindNet
  · simpa [borrowedIndirectCallableEvents, resultOwnerArrivalEvents]
      using hunwindFloors

/-! ## §T4 Part C — the summary split and THE
    composition theorem -/

/-- §T4 the boundary expansion is dynamic-COW-mutate-free (an escape use and
    an optional result dup) — the left-segment condition the committed
    derivation-split lemma needs. -/
theorem boundaryInstrs_mutate_free (arg ret : Nat) (bc : BoundaryContract) :
    (boundaryInstrs arg ret bc).all (fun i => !(LedgerInstr.isMutate i)) = true := by
  unfold boundaryInstrs boundaryResultInstrs
  cases bc.transfersThroughReturn || bc.sharingViewProducer <;> rfl

/-- §T4 the caller's summary-composed ledger splits at the call site: prefix
    events, the contract-classified argument summary, the result-binding
    events, the suffix events — via the committed mutate-free derivation
    split (`deriveLedger_append_mutate_free`) and the committed RL-2 bridge.
    The mutate-free prefix hypothesis is the committed corpus's split
    discipline (a dynamic-COW mutate's sibling floor reads its path suffix). -/
theorem caller_summary_split (classOf : Nat → Nat) (c arg ret : Nat)
    (bc : BoundaryContract) (preI postI : List LedgerInstr)
    (harg : (classOf arg == c) = true)
    (hpremutfree : preI.all (fun i => !(LedgerInstr.isMutate i)) = true) :
    deriveLedger classOf c (preI ++ boundaryInstrs arg ret bc ++ postI)
      = deriveLedger classOf c preI
          ++ ([boundaryArgEvent bc]
          ++ (deriveLedger classOf c (boundaryResultInstrs ret bc)
          ++ deriveLedger classOf c postI)) := by
  rw [List.append_assoc,
      deriveLedger_append_mutate_free classOf c preI _ hpremutfree,
      deriveLedger_append_mutate_free classOf c (boundaryInstrs arg ret bc) postI
        (boundaryInstrs_mutate_free arg ret bc),
      show boundaryInstrs arg ret bc
          = [LedgerInstr.escapeUse arg (boundaryUseKind bc)]
              ++ boundaryResultInstrs ret bc from rfl,
      deriveLedger_append_mutate_free classOf c
        [LedgerInstr.escapeUse arg (boundaryUseKind bc)]
        (boundaryResultInstrs ret bc) (by rfl),
      boundary_arg_event_derived classOf c arg bc harg,
      List.append_assoc]

/-- §T4 the event-level substitution core: replacing the contract-classified
    argument summary `S` with a callee segment `B` of the SAME net whose
    floors hold from the call-site count preserves the three clauses of the
    whole composed ledger. The prefix/result/suffix segments are untouched;
    clause 1 composes by net additivity, clauses 2/3 by the floor
    decomposition (the committed `clauseFloors_append`). -/
theorem threeClauses_substitute_segment (P S B R Q : List LedgerEvent)
    (hcaller : threeClauses (P ++ (S ++ (R ++ Q))) = true)
    (hnet : ledgerNet B = ledgerNet S)
    (hfloors : clauseFloors (ledgerNet P) B = true) :
    threeClauses (P ++ (B ++ (R ++ Q))) = true := by
  have hcaller' : (clauseNetZero (P ++ (S ++ (R ++ Q)))
      && clauseFloors 0 (P ++ (S ++ (R ++ Q)))) = true := hcaller
  rw [Bool.and_eq_true] at hcaller'
  obtain ⟨hnet0, hfl⟩ := hcaller'
  show (clauseNetZero (P ++ (B ++ (R ++ Q)))
      && clauseFloors 0 (P ++ (B ++ (R ++ Q)))) = true
  rw [Bool.and_eq_true]
  constructor
  · -- Clause 1: net additivity + the net-equal substitution.
    have hnetv : (ledgerNet (P ++ (S ++ (R ++ Q))) == 0) = true := hnet0
    have hnete : ledgerNet (P ++ (S ++ (R ++ Q))) = 0 := beq_iff_eq.mp hnetv
    rw [ledgerNet_append, ledgerNet_append, ledgerNet_append] at hnete
    show (ledgerNet (P ++ (B ++ (R ++ Q))) == 0) = true
    rw [ledgerNet_append, ledgerNet_append, ledgerNet_append, hnet]
    exact beq_iff_eq.mpr hnete
  · -- Clauses 2 + 3: floor decomposition; B's floors from the call-site
    -- count; the suffix re-enters at the SAME count (net B = net S).
    rw [clauseFloors_append, Bool.and_eq_true] at hfl
    obtain ⟨hP, hSRQ⟩ := hfl
    rw [clauseFloors_append, Bool.and_eq_true] at hSRQ
    obtain ⟨_, hRQ⟩ := hSRQ
    rw [clauseFloors_append, Bool.and_eq_true]
    refine ⟨hP, ?_⟩
    rw [clauseFloors_append, Bool.and_eq_true]
    constructor
    · have harith : (0 : Int) + ledgerNet P = ledgerNet P := by omega
      rw [harith]
      exact hfloors
    · rw [hnet]
      exact hRQ

/-- §T4 (P3) THE contract-boundary composition theorem. A caller whose
    per-class per-path ledger satisfies the three clauses WITH the contract-
    classified call-site summary in place (`boundaryInstrs` — the argument's
    RL-2 kind + the result binding, all computed from the contract), and a
    callee whose OWN per-param ledger conforms (`calleeConforms` — the three
    clauses under the contract-mandated BIRTH opening, or net-zero + borrow-
    assumption floors), compose: the inlined end-to-end ledger — caller
    prefix, callee body events, result binding, caller suffix — satisfies
    the three clauses. The callee is consumed ONLY through its contract
    conformance: no callee re-derivation.

    The `hlive` side condition (the caller's running count is at least 1 at
    the call site) is honest and NECESSARY: the summary CONSUME event carries
    no floor of its own, so the caller's clauses alone do not force a live
    reference at a transfer-in handoff — `T4_live_at_call_side_condition_necessary`
    exhibits the use-after-free the composition admits without it. On the
    borrowed rows the READ summary's own floor already forces it. -/
theorem T4_contract_boundary_composition_sound
    (classOf cc : Nat → Nat) (c arg ret p : Nat) (bc : BoundaryContract)
    (preI postI body : List LedgerInstr)
    (harg : (classOf arg == c) = true)
    (hpremutfree : preI.all (fun i => !(LedgerInstr.isMutate i)) = true)
    (hcaller : threeClauses (deriveLedger classOf c
        (preI ++ boundaryInstrs arg ret bc ++ postI)) = true)
    (hcallee : calleeConforms cc p bc body = true)
    (hlive : 1 ≤ ledgerNet (deriveLedger classOf c preI)) :
    threeClauses
      (deriveLedger classOf c preI
        ++ (deriveLedger cc (cc p) body
        ++ (deriveLedger classOf c (boundaryResultInstrs ret bc)
        ++ deriveLedger classOf c postI))) = true := by
  rw [caller_summary_split classOf c arg ret bc preI postI harg hpremutfree]
    at hcaller
  obtain ⟨hnet, hfl1⟩ := calleeConforms_interface cc p bc body hcallee
  exact threeClauses_substitute_segment _ _ _ _ _ hcaller hnet
    (clauseFloors_mono _ 1 _ hlive hfl1)

/-- §T4 primary composition statement including Invoke path semantics. The
    callee-summary substitution is sound, the argument event precedes both
    successors, and only normal return carries result-binding events. -/
theorem T4_contract_boundary_invoke_composition_sound
    (classOf cc : Nat → Nat) (c arg ret p : Nat) (bc : BoundaryContract)
    (preI postI body : List LedgerInstr)
    (harg : (classOf arg == c) = true)
    (hpremutfree : preI.all (fun i => !(LedgerInstr.isMutate i)) = true)
    (hcaller : threeClauses (deriveLedger classOf c
        (preI ++ boundaryInstrs arg ret bc ++ postI)) = true)
    (hcallee : calleeConforms cc p bc body = true)
    (hlive : 1 ≤ ledgerNet (deriveLedger classOf c preI)) :
    threeClauses
      (deriveLedger classOf c preI
        ++ (deriveLedger cc (cc p) body
        ++ (deriveLedger classOf c (boundaryResultInstrs ret bc)
        ++ deriveLedger classOf c postI))) = true
      ∧ invokeBoundaryEvents bc .normal
          = boundaryArgEvent bc :: boundaryResultEvents bc
      ∧ invokeBoundaryEvents bc .unwind = [boundaryArgEvent bc] := by
  refine ⟨T4_contract_boundary_composition_sound classOf cc c arg ret p bc
      preI postI body harg hpremutfree hcaller hcallee hlive, ?_, ?_⟩
  · rfl
  · rfl

/-- §T4 (P3) the operational corollary: the composed inlined ledger is
    balanced-and-safe — no leak, no use-after-free, no COW corruption — via
    the committed clauses-safety equivalence. -/
theorem T4_contract_boundary_composition_safe
    (classOf cc : Nat → Nat) (c arg ret p : Nat) (bc : BoundaryContract)
    (preI postI body : List LedgerInstr)
    (harg : (classOf arg == c) = true)
    (hpremutfree : preI.all (fun i => !(LedgerInstr.isMutate i)) = true)
    (hcaller : threeClauses (deriveLedger classOf c
        (preI ++ boundaryInstrs arg ret bc ++ postI)) = true)
    (hcallee : calleeConforms cc p bc body = true)
    (hlive : 1 ≤ ledgerNet (deriveLedger classOf c preI)) :
    ledgerSafe
      (deriveLedger classOf c preI
        ++ (deriveLedger cc (cc p) body
        ++ (deriveLedger classOf c (boundaryResultInstrs ret bc)
        ++ deriveLedger classOf c postI))) :=
  (three_clauses_iff_ledger_safe _).mp
    (T4_contract_boundary_composition_sound classOf cc c arg ret p bc
      preI postI body harg hpremutfree hcaller hcallee hlive)

/-! ## §T4 Part D — negative witnesses

    Values live in identity classes computed by the REAL T1 union-find over
    the empty edge list (every value its own representative) — nothing
    hardcoded. Caller values 700-region; callee values 710-region (their own
    namespace, bridged only by the contract). -/

def t4UF : PartitionUF := buildPartitionUF []

def t4ClassOf : Nat → Nat := fun v => t4UF.find v

/-- §T4 the TRUE contract of the witness callee: an Owned param. -/
def t4BcOwned : BoundaryContract := ⟨.Owned, false, false, false⟩

/-- §T4 the MISCLASSIFIED boundary: the same callee declared Borrowed. -/
def t4BcMis : BoundaryContract := ⟨.Borrowed, false, false, false⟩

/-- §T4 the witness callee body: reads its param, then releases it (the
    owned-param obligation discharged inside — `[read, consume]` after the
    BIRTH opening). -/
def t4CalleeBody : List LedgerInstr := [.projRead 710, .burdenDec 710]

def t4PreI : List LedgerInstr := [.construct 700]

/-- §T4 under the misclassification the caller keeps the release obligation
    (a Borrowed arg survives the call, so the caller's placed dec follows). -/
def t4PostMis : List LedgerInstr := [.burdenDec 700]

/-- §T4 (P4) the MISCLASSIFIED BOUNDARY is REJECTED. The callee truly owns
    (conforms to the Owned contract: birth + read + consume, three clauses
    hold) and does NOT conform to the Borrowed misclassification (its body
    nets -1 where a borrow demands 0). The caller's summary-composed ledger
    under the misclassification looks fine in isolation — birth, READ
    summary, placed dec — but the INLINED composition double-releases:
    the callee's real release plus the caller's retained release drive the
    class net to -1; clause 1 computes false and the operational count lands
    at -1. Correctly classified (Owned: CONSUME summary, no caller dec), the
    same callee composes safely. -/
theorem T4_owned_arg_misclassified_borrowed_rejected :
    -- The caller's own invariant ACCEPTS the misclassified summary:
    threeClauses (deriveLedger t4ClassOf (t4ClassOf 700)
        (t4PreI ++ boundaryInstrs 700 701 t4BcMis ++ t4PostMis)) = true
    -- The callee conforms to its TRUE (Owned) contract:
    ∧ calleeConforms t4ClassOf 710 t4BcOwned t4CalleeBody = true
    -- and does NOT conform to the misclassified (Borrowed) contract:
    ∧ calleeConforms t4ClassOf 710 t4BcMis t4CalleeBody = false
    -- The inlined composition under the misclassification violates clause 1:
    ∧ threeClauses
        (deriveLedger t4ClassOf (t4ClassOf 700) t4PreI
          ++ (deriveLedger t4ClassOf (t4ClassOf 710) t4CalleeBody
          ++ (deriveLedger t4ClassOf (t4ClassOf 700)
                (boundaryResultInstrs 701 t4BcMis)
          ++ deriveLedger t4ClassOf (t4ClassOf 700) t4PostMis))) = false
    -- ... the double-release shape (operational count -1):
    ∧ (runLedger
        (deriveLedger t4ClassOf (t4ClassOf 700) t4PreI
          ++ (deriveLedger t4ClassOf (t4ClassOf 710) t4CalleeBody
          ++ (deriveLedger t4ClassOf (t4ClassOf 700)
                (boundaryResultInstrs 701 t4BcMis)
          ++ deriveLedger t4ClassOf (t4ClassOf 700) t4PostMis)))).count = -1
    -- The CORRECT classification composes safely (caller places no dec):
    ∧ threeClauses (deriveLedger t4ClassOf (t4ClassOf 700)
        (t4PreI ++ boundaryInstrs 700 701 t4BcOwned ++ [])) = true
    ∧ threeClauses
        (deriveLedger t4ClassOf (t4ClassOf 700) t4PreI
          ++ (deriveLedger t4ClassOf (t4ClassOf 710) t4CalleeBody
          ++ (deriveLedger t4ClassOf (t4ClassOf 700)
                (boundaryResultInstrs 701 t4BcOwned)
          ++ deriveLedger t4ClassOf (t4ClassOf 700) []))) = true := by
  decide

/-- §T4 (P4) the LIVE-AT-CALL side condition is NECESSARY. A degenerate
    caller stream hands off a reference it never held (call-site count 0) and
    re-credits afterward: its summary-composed ledger passes the three
    clauses (a CONSUME event carries no floor), the callee conforms to its
    Owned contract — yet the inlined composition READS at count 0: the
    operational machine observes the use-after-free. Dropping `hlive` from
    the composition theorem would be an overclaim. -/
theorem T4_live_at_call_side_condition_necessary :
    threeClauses (deriveLedger t4ClassOf (t4ClassOf 720)
        (([] : List LedgerInstr)
          ++ boundaryInstrs 720 721 t4BcOwned ++ [.dup 720])) = true
    ∧ calleeConforms t4ClassOf 730 t4BcOwned
        [.projRead 730, .burdenDec 730] = true
    ∧ ¬ (1 ≤ ledgerNet (deriveLedger t4ClassOf (t4ClassOf 720)
        ([] : List LedgerInstr)))
    ∧ (runLedger
        (deriveLedger t4ClassOf (t4ClassOf 720) ([] : List LedgerInstr)
          ++ (deriveLedger t4ClassOf (t4ClassOf 730)
                [.projRead 730, .burdenDec 730]
          ++ (deriveLedger t4ClassOf (t4ClassOf 720)
                (boundaryResultInstrs 721 t4BcOwned)
          ++ deriveLedger t4ClassOf (t4ClassOf 720) [.dup 720])))).uaf = true := by
  decide

/-! ## §T5 Part E — the class-map redirect + THE frame theorem

    Introducing a NEW tier-1 same-allocation edge is, at the class-map level
    the T2 engine consumes (the partition AS GIVEN input), a ROOT REDIRECT:
    the retired representative's members re-map to the absorbing
    representative; every other value's class is untouched. `mergeClasses` is
    that redirect — the exact class-level projection of `PartitionUF.union`'s
    parent-level redirect (`fun x => if x == ra then rb else parent x`),
    witnessed computationally on a concrete union-find in Part I. -/

/-- §T5 the tier-1-merge class-map redirect: members of class `c1` re-map to
    class `c2`; every other value keeps its class. -/
def mergeClasses (classOf : Nat → Nat) (c1 c2 : Nat) : Nat → Nat :=
  fun v => if classOf v == c1 then c2 else classOf v

/-- §T5 (P1) untouched-class membership is VERBATIM: for any class `c`
    distinct from both merge participants, every value's membership test is
    unchanged by the redirect. -/
theorem mergeClasses_untouched_membership (classOf : Nat → Nat) (c1 c2 c : Nat)
    (hne1 : c ≠ c1) (hne2 : c ≠ c2) (v : Nat) :
    (mergeClasses classOf c1 c2 v == c) = (classOf v == c) := by
  unfold mergeClasses
  cases hv : classOf v == c1 with
  | true =>
      simp only [reduceIte]
      have h2 : (c2 == c) = false := by
        cases hb : c2 == c with
        | false => rfl
        | true => exact absurd (beq_iff_eq.mp hb).symm hne2
      have h3 : (classOf v == c) = false := by
        cases hb : classOf v == c with
        | false => rfl
        | true =>
            exact absurd ((beq_iff_eq.mp hb).symm.trans (beq_iff_eq.mp hv)) hne1
      rw [h2, h3]
  | false =>
      rw [if_neg (by decide : ¬(false = true))]

/-- §T5 (P1) merged-class membership is the DISJUNCTION of the two prior
    memberships: the absorbing class `c2` now holds exactly the old `c1`
    members and the old `c2` members. -/
theorem mergeClasses_merged_membership (classOf : Nat → Nat) (c1 c2 v : Nat) :
    (mergeClasses classOf c1 c2 v == c2)
      = (classOf v == c1 || classOf v == c2) := by
  unfold mergeClasses
  cases hv : classOf v == c1 with
  | true =>
      simp only [reduceIte]
      rw [beq_self_eq_true, Bool.true_or]
  | false =>
      rw [if_neg (by decide : ¬(false = true)), Bool.false_or]

/-- §T5 (P1) the retired class is EMPTY after the redirect: no value maps to
    `c1` any more (its members re-mapped to `c2`; everyone else never was
    `c1`). -/
theorem mergeClasses_retired_membership (classOf : Nat → Nat) (c1 c2 v : Nat)
    (hne : c1 ≠ c2) :
    (mergeClasses classOf c1 c2 v == c1) = false := by
  unfold mergeClasses
  cases hv : classOf v == c1 with
  | true =>
      simp only [reduceIte]
      cases hb : c2 == c1 with
      | false => rfl
      | true => exact absurd (beq_iff_eq.mp hb).symm hne
  | false =>
      rw [if_neg (by decide : ¬(false = true))]
      exact hv

/-- §T5 the dynamic-COW live-sibling count depends on the classifier ONLY
    through the membership test — pointwise-equal memberships count the same
    siblings. -/
theorem sibReadCount_congr (f g : Nat → Nat) (cf cg v : Nat)
    (h : ∀ w, (f w == cf) = (g w == cg)) (rest : List LedgerInstr) :
    sibReadCount f cf v rest = sibReadCount g cg v rest := by
  unfold sibReadCount
  have hfilter : ((rest.filterMap LedgerInstr.readsValue).filter
        (fun w => !(w == v) && f w == cf))
      = ((rest.filterMap LedgerInstr.readsValue).filter
        (fun w => !(w == v) && g w == cg)) :=
    List.filter_congr (fun w _ => by rw [h w])
  rw [hfilter]

/-- §T5 (P2) derivation congruence: the derived ledger depends on the
    classifier ONLY through the membership test — pointwise-equal
    memberships derive VERBATIM-identical event lists (every case of the
    computed classification, the jump-arg partition routing and the
    suffix-computed sibling floor included). -/
theorem deriveLedger_congr (f g : Nat → Nat) (cf cg : Nat)
    (h : ∀ w, (f w == cf) = (g w == cg)) :
    ∀ instrs, deriveLedger f cf instrs = deriveLedger g cg instrs := by
  intro instrs
  induction instrs with
  | nil => rfl
  | cons i rest ih =>
      cases i with
      | construct v =>
          show (if f v == cf then LedgerEvent.birth :: deriveLedger f cf rest
                else deriveLedger f cf rest)
            = (if g v == cg then LedgerEvent.birth :: deriveLedger g cg rest
                else deriveLedger g cg rest)
          rw [h v, ih]
      | dup v =>
          show (if f v == cf then LedgerEvent.credit :: deriveLedger f cf rest
                else deriveLedger f cf rest)
            = (if g v == cg then LedgerEvent.credit :: deriveLedger g cg rest
                else deriveLedger g cg rest)
          rw [h v, ih]
      | projRead v =>
          show (if f v == cf then LedgerEvent.read :: deriveLedger f cf rest
                else deriveLedger f cf rest)
            = (if g v == cg then LedgerEvent.read :: deriveLedger g cg rest
                else deriveLedger g cg rest)
          rw [h v, ih]
      | cowMutate v =>
          show (if f v == cf then
                  LedgerEvent.mutate (sibReadCount f cf v rest)
                    :: deriveLedger f cf rest
                else deriveLedger f cf rest)
            = (if g v == cg then
                  LedgerEvent.mutate (sibReadCount g cg v rest)
                    :: deriveLedger g cg rest
                else deriveLedger g cg rest)
          rw [h v, sibReadCount_congr f g cf cg v h rest, ih]
      | escapeUse v u =>
          show (if f v == cf then
                  (if rl2_use_transfers_ownership u then LedgerEvent.consume
                   else .read) :: deriveLedger f cf rest
                else deriveLedger f cf rest)
            = (if g v == cg then
                  (if rl2_use_transfers_ownership u then LedgerEvent.consume
                   else .read) :: deriveLedger g cg rest
                else deriveLedger g cg rest)
          rw [h v, ih]
      | jumpArg v p =>
          show (if f v == cf then
                  (if f p == cf then deriveLedger f cf rest
                   else .consume :: deriveLedger f cf rest)
                else
                  if f p == cf then LedgerEvent.credit :: deriveLedger f cf rest
                  else deriveLedger f cf rest)
            = (if g v == cg then
                  (if g p == cg then deriveLedger g cg rest
                   else .consume :: deriveLedger g cg rest)
                else
                  if g p == cg then LedgerEvent.credit :: deriveLedger g cg rest
                  else deriveLedger g cg rest)
          rw [h v, h p, ih]
      | burdenDec v =>
          show (if f v == cf then LedgerEvent.consume :: deriveLedger f cf rest
                else deriveLedger f cf rest)
            = (if g v == cg then LedgerEvent.consume :: deriveLedger g cg rest
                else deriveLedger g cg rest)
          rw [h v, ih]
      | holeFill v hole =>
          have hsib : sibReadCount f cf hole rest = sibReadCount g cg hole rest := by
            unfold sibReadCount
            have hfilter :
                ((rest.filterMap LedgerInstr.readsValue).filter
                    (fun w => !(w == hole) && f w == cf))
                  = ((rest.filterMap LedgerInstr.readsValue).filter
                    (fun w => !(w == hole) && g w == cg)) := by
              apply List.filter_congr
              intro w _
              rw [h w]
            rw [hfilter]
          show (if f hole == cf then
                  (if f v == cf then
                    .mutate (sibReadCount f cf hole rest) :: .consume
                      :: deriveLedger f cf rest
                   else
                    .mutate (sibReadCount f cf hole rest) :: deriveLedger f cf rest)
                else
                  if f v == cf then LedgerEvent.consume :: deriveLedger f cf rest
                  else deriveLedger f cf rest)
            = (if g hole == cg then
                  (if g v == cg then
                    .mutate (sibReadCount g cg hole rest) :: .consume
                      :: deriveLedger g cg rest
                   else
                    .mutate (sibReadCount g cg hole rest) :: deriveLedger g cg rest)
                else
                  if g v == cg then LedgerEvent.consume :: deriveLedger g cg rest
                  else deriveLedger g cg rest)
          rw [h v, h hole, hsib, ih]

/-- §T5 (P2) THE FRAME THEOREM. For EVERY class `c` distinct from both merge
    participants and EVERY instruction stream, the post-merge derived ledger
    is VERBATIM the pre-merge derived ledger: `deriveLedger (mergeClasses
    classOf c1 c2) c instrs = deriveLedger classOf c instrs`. A new tier-1
    partition edge cannot perturb a distant class's events — same list, same
    clauses, same verdict. Unconditional in the instruction stream. -/
theorem T5_frame_untouched_class_ledger_verbatim (classOf : Nat → Nat)
    (c1 c2 c : Nat) (hne1 : c ≠ c1) (hne2 : c ≠ c2)
    (instrs : List LedgerInstr) :
    deriveLedger (mergeClasses classOf c1 c2) c instrs
      = deriveLedger classOf c instrs :=
  deriveLedger_congr (mergeClasses classOf c1 c2) classOf c c
    (fun v => mergeClasses_untouched_membership classOf c1 c2 c hne1 hne2 v)
    instrs

/-- §T5 (P2) the frame corollary at the verdict level: an untouched class's
    three-clause verdict and its balanced-and-safe status are UNCHANGED
    verbatim by the merge. -/
theorem T5_frame_verdict_verbatim (classOf : Nat → Nat) (c1 c2 c : Nat)
    (hne1 : c ≠ c1) (hne2 : c ≠ c2) (instrs : List LedgerInstr) :
    threeClauses (deriveLedger (mergeClasses classOf c1 c2) c instrs)
      = threeClauses (deriveLedger classOf c instrs)
    ∧ (ledgerSafe (deriveLedger (mergeClasses classOf c1 c2) c instrs)
        ↔ ledgerSafe (deriveLedger classOf c instrs)) := by
  rw [T5_frame_untouched_class_ledger_verbatim classOf c1 c2 c hne1 hne2 instrs]
  exact ⟨rfl, Iff.rfl⟩

/-- §T5 the retired class derives the EMPTY ledger after the merge — its
    members moved to the absorbing class; nothing is double-tracked. -/
theorem T5_retired_class_ledger_empty (classOf : Nat → Nat) (c1 c2 : Nat)
    (hne : c1 ≠ c2) (instrs : List LedgerInstr) :
    deriveLedger (mergeClasses classOf c1 c2) c1 instrs = [] := by
  apply deriveLedger_untouched
  have hret : ∀ v, (mergeClasses classOf c1 c2 v == c1) = false :=
    fun v => mergeClasses_retired_membership classOf c1 c2 v hne
  rw [List.all_eq_true]
  intro i _
  cases i <;> simp [LedgerInstr.touchesClass, hret]

/-! ## §T5 Part F — net additivity: clause 1 composes unconditionally

    The merged class's net ledger is EXACTLY the sum of the two prior
    classes' nets, for EVERY instruction stream — mutates contribute delta 0
    whatever their sibling floors, and a class-bridging jump-arg handoff
    collapses net-preservingly (the prior consume (-1) + credit (+1) pair
    becomes the RL-4 exemption's no-event). -/

/-- §T5 the cons-step of the net ledger. -/
theorem ledgerNet_cons (e : LedgerEvent) (es : List LedgerEvent) :
    ledgerNet (e :: es) = eventDelta e + ledgerNet es := by
  show ((e :: es).map eventDelta).foldr (· + ·) 0 = _
  rw [List.map_cons, List.foldr_cons]
  rfl

/-- §T5 with the merge participants distinct, membership is mutually
    exclusive: a value in `c1` is not in `c2`. -/
theorem beq_second_false_of_first (x c1 c2 : Nat) (hne : c1 ≠ c2)
    (h1 : (x == c1) = true) : (x == c2) = false := by
  cases hb : x == c2 with
  | false => rfl
  | true => exact absurd ((beq_iff_eq.mp h1).symm.trans (beq_iff_eq.mp hb)) hne

/-- §T5 the per-instruction net contribution to one class — the delta the
    computed classification assigns (birth/credit +1, consume -1, read/mutate
    0, the jump-arg partition routing). -/
def instrNetAt (f : Nat → Nat) (c : Nat) : LedgerInstr → Int
  | .construct v => if f v == c then 1 else 0
  | .dup v => if f v == c then 1 else 0
  | .projRead _ => 0
  | .cowMutate _ => 0
  | .escapeUse v u =>
      if f v == c then
        (if rl2_use_transfers_ownership u then -1 else 0)
      else 0
  | .jumpArg v p =>
      if f v == c then (if f p == c then 0 else -1)
      else if f p == c then 1 else 0
  | .burdenDec v => if f v == c then -1 else 0
  | .holeFill v _ => if f v == c then -1 else 0

/-- §T5 the derived net decomposes per instruction: cons an instruction, add
    its net contribution. -/
theorem ledgerNet_deriveLedger_cons (f : Nat → Nat) (c : Nat)
    (i : LedgerInstr) (rest : List LedgerInstr) :
    ledgerNet (deriveLedger f c (i :: rest))
      = instrNetAt f c i + ledgerNet (deriveLedger f c rest) := by
  cases i with
  | construct v =>
      cases hv : f v == c <;>
        simp [deriveLedger, instrNetAt, hv, ledgerNet_cons, eventDelta] <;> omega
  | dup v =>
      cases hv : f v == c <;>
        simp [deriveLedger, instrNetAt, hv, ledgerNet_cons, eventDelta] <;> omega
  | projRead v =>
      cases hv : f v == c <;>
        simp [deriveLedger, instrNetAt, hv, ledgerNet_cons, eventDelta] <;> omega
  | cowMutate v =>
      cases hv : f v == c <;>
        simp [deriveLedger, instrNetAt, hv, ledgerNet_cons, eventDelta] <;> omega
  | escapeUse v u =>
      cases hv : f v == c <;> cases hu : rl2_use_transfers_ownership u <;>
        simp [deriveLedger, instrNetAt, hv, hu, ledgerNet_cons, eventDelta] <;> omega
  | jumpArg v p =>
      cases hv : f v == c <;> cases hp : f p == c <;>
        simp [deriveLedger, instrNetAt, hv, hp, ledgerNet_cons, eventDelta] <;> omega
  | burdenDec v =>
      cases hv : f v == c <;>
        simp [deriveLedger, instrNetAt, hv, ledgerNet_cons, eventDelta] <;> omega
  | holeFill v hole =>
      cases hv : f v == c <;> cases hh : f hole == c <;>
        simp [deriveLedger, instrNetAt, hv, hh, ledgerNet_cons, eventDelta] <;> omega

/-- §T5 (P3) the per-instruction contribution is ADDITIVE under the merge:
    the merged class's contribution is the sum of the two prior classes'
    contributions, for every instruction — the bridging jump-arg rows sum
    the prior (-1) + (+1) to the exemption's 0. -/
theorem instrNetAt_merged_additive (classOf : Nat → Nat) (c1 c2 : Nat)
    (hne : c1 ≠ c2) (i : LedgerInstr) :
    instrNetAt (mergeClasses classOf c1 c2) c2 i
      = instrNetAt classOf c1 i + instrNetAt classOf c2 i := by
  cases i with
  | construct v =>
      simp only [instrNetAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
  | dup v =>
      simp only [instrNetAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
  | projRead v => rfl
  | cowMutate v => rfl
  | escapeUse v u =>
      simp only [instrNetAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        cases hu : rl2_use_transfers_ownership u <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
  | jumpArg v p =>
      simp only [instrNetAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v,
          mergeClasses_merged_membership classOf c1 c2 p]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        cases hp1 : classOf p == c1 <;> cases hp2 : classOf p == c2 <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
          | exact absurd ((beq_iff_eq.mp hp1).symm.trans (beq_iff_eq.mp hp2)) hne
  | burdenDec v =>
      simp only [instrNetAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
  | holeFill v hole =>
      simp only [instrNetAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne

/-- §T5 (P3) NET ADDITIVITY — the bounded-impact theorem at the net level,
    UNCONDITIONAL in the instruction stream: the merged class's net ledger is
    exactly the sum of the two prior classes' nets. -/
theorem T5_merged_net_additive (classOf : Nat → Nat) (c1 c2 : Nat)
    (hne : c1 ≠ c2) :
    ∀ instrs : List LedgerInstr,
      ledgerNet (deriveLedger (mergeClasses classOf c1 c2) c2 instrs)
        = ledgerNet (deriveLedger classOf c1 instrs)
          + ledgerNet (deriveLedger classOf c2 instrs) := by
  intro instrs
  induction instrs with
  | nil =>
      show (0 : Int) = 0 + 0
      omega
  | cons i rest ih =>
      rw [ledgerNet_deriveLedger_cons, ledgerNet_deriveLedger_cons,
          ledgerNet_deriveLedger_cons, ih,
          instrNetAt_merged_additive classOf c1 c2 hne i]
      omega

/-- §T5 (P3) the clause-1 robustness corollary, UNCONDITIONAL: two prior
    classes each netting zero merge into a class netting zero — a new tier-1
    edge can never manufacture a leak or a double-free at the net level. -/
theorem T5_merged_clause1_net_zero (classOf : Nat → Nat) (c1 c2 : Nat)
    (hne : c1 ≠ c2) (instrs : List LedgerInstr)
    (h1 : clauseNetZero (deriveLedger classOf c1 instrs) = true)
    (h2 : clauseNetZero (deriveLedger classOf c2 instrs) = true) :
    clauseNetZero (deriveLedger (mergeClasses classOf c1 c2) c2 instrs) = true := by
  have h1' : ledgerNet (deriveLedger classOf c1 instrs) = 0 := beq_iff_eq.mp h1
  have h2' : ledgerNet (deriveLedger classOf c2 instrs) = 0 := beq_iff_eq.mp h2
  show (ledgerNet (deriveLedger (mergeClasses classOf c1 c2) c2 instrs) == 0) = true
  rw [T5_merged_net_additive classOf c1 c2 hne instrs, h1', h2']
  rfl

/-! ## §T5 Part G — bounded impact: the merged ledger is the positional
    interleave

    On the mutate-free non-bridging fragment the merged class's ledger is
    EXACTLY the positional interleaving of the two prior classes' events: at
    every instruction, the event that instruction contributed to whichever
    prior class it touched (at most one, by disjointness). The dynamic-COW
    mutate is excluded because its event reads the path suffix (the sibling
    floor can RISE under the merge — Part H's second witness); a
    class-bridging jump-arg is excluded because its prior consume/credit
    pair COLLAPSES into the RL-4 exemption (the collapse lemma below states
    the one folding shape — net-preserving, so Part F stays unconditional). -/

/-- §T5 the per-instruction event image of one class (mutate-free fragment;
    the dynamic-COW mutate reads the path suffix and has no per-instruction
    image). -/
def instrEventAt (f : Nat → Nat) (c : Nat) : LedgerInstr → Option LedgerEvent
  | .construct v => if f v == c then some .birth else none
  | .dup v => if f v == c then some .credit else none
  | .projRead v => if f v == c then some .read else none
  | .cowMutate _ => none
  | .escapeUse v u =>
      if f v == c then
        some (if rl2_use_transfers_ownership u then .consume else .read)
      else none
  | .jumpArg v p =>
      if f v == c then (if f p == c then none else some .consume)
      else if f p == c then some .credit else none
  | .burdenDec v => if f v == c then some .consume else none
  | .holeFill _ _ => none

/-- §T5 the positional pair image: the event the instruction contributed to
    whichever prior class it touched (`c1` first; disjointness makes the
    order immaterial off the bridging shapes). -/
def pairEventAt (classOf : Nat → Nat) (c1 c2 : Nat) (i : LedgerInstr) :
    Option LedgerEvent :=
  match instrEventAt classOf c1 i with
  | some e => some e
  | none => instrEventAt classOf c2 i

/-- §T5 a jump-arg handoff BRIDGING the two merge participants (source in
    one, receiving block-param in the other). -/
def isBridgingJump (classOf : Nat → Nat) (c1 c2 : Nat) : LedgerInstr → Bool
  | .jumpArg v p =>
      (classOf v == c1 && classOf p == c2) || (classOf v == c2 && classOf p == c1)
  | _ => false

/-- §T5 on the mutate-free fragment the derived ledger IS the filterMap of
    the per-instruction event image. -/
theorem deriveLedger_eq_filterMap (f : Nat → Nat) (c : Nat) :
    ∀ instrs : List LedgerInstr,
      instrs.all (fun i => !(LedgerInstr.isMutate i)) = true →
      deriveLedger f c instrs = instrs.filterMap (instrEventAt f c) := by
  intro instrs
  induction instrs with
  | nil => intro _; rfl
  | cons i rest ih =>
      intro h
      rw [List.all_cons, Bool.and_eq_true] at h
      obtain ⟨hi, hrest⟩ := h
      have htail := ih hrest
      cases i with
      | cowMutate v => simp [LedgerInstr.isMutate] at hi
      | holeFill v hole => simp [LedgerInstr.isMutate] at hi
      | construct v =>
          cases hv : f v == c <;>
            simp [deriveLedger, instrEventAt, hv, htail]
      | dup v =>
          cases hv : f v == c <;>
            simp [deriveLedger, instrEventAt, hv, htail]
      | projRead v =>
          cases hv : f v == c <;>
            simp [deriveLedger, instrEventAt, hv, htail]
      | escapeUse v u =>
          cases hv : f v == c <;>
            simp [deriveLedger, instrEventAt, hv, htail]
      | jumpArg v p =>
          cases hv : f v == c <;> cases hp : f p == c <;>
            simp [deriveLedger, instrEventAt, hv, hp, htail]
      | burdenDec v =>
          cases hv : f v == c <;>
            simp [deriveLedger, instrEventAt, hv, htail]

/-- §T5 filterMap congruence over a list (pointwise-equal images filter the
    same). -/
theorem filterMap_congr_local {α β : Type} (f g : α → Option β) :
    ∀ l : List α, (∀ a ∈ l, f a = g a) → l.filterMap f = l.filterMap g := by
  intro l
  induction l with
  | nil => intro _; rfl
  | cons a rest ih =>
      intro h
      rw [List.filterMap_cons, List.filterMap_cons, h a (List.mem_cons_self ..),
          ih (fun x hx => h x (List.mem_cons_of_mem _ hx))]

/-- §T5 (P4) the per-instruction merged image, TOTAL over the non-mutate
    instruction space: either the merged image IS the positional pair image
    (with at most one prior class firing — disjointness), or the instruction
    is a bridging jump-arg and the prior (consume, credit) / (credit,
    consume) pair collapses to NO merged event (the RL-4 exemption — the
    handoff became class-internal). -/
theorem instrEventAt_merged_cases (classOf : Nat → Nat) (c1 c2 : Nat)
    (hne : c1 ≠ c2) (i : LedgerInstr) :
    (instrEventAt (mergeClasses classOf c1 c2) c2 i
        = pairEventAt classOf c1 c2 i
      ∧ (instrEventAt classOf c1 i = none ∨ instrEventAt classOf c2 i = none))
    ∨ (instrEventAt classOf c1 i = some .consume
        ∧ instrEventAt classOf c2 i = some .credit
        ∧ instrEventAt (mergeClasses classOf c1 c2) c2 i = none)
    ∨ (instrEventAt classOf c1 i = some .credit
        ∧ instrEventAt classOf c2 i = some .consume
        ∧ instrEventAt (mergeClasses classOf c1 c2) c2 i = none) := by
  cases i with
  | construct v =>
      unfold pairEventAt
      simp only [instrEventAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
  | dup v =>
      unfold pairEventAt
      simp only [instrEventAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
  | projRead v =>
      unfold pairEventAt
      simp only [instrEventAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
  | cowMutate v => exact Or.inl ⟨rfl, Or.inl rfl⟩
  | escapeUse v u =>
      unfold pairEventAt
      simp only [instrEventAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        cases hu : rl2_use_transfers_ownership u <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
  | jumpArg v p =>
      unfold pairEventAt
      simp only [instrEventAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v,
          mergeClasses_merged_membership classOf c1 c2 p]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        cases hp1 : classOf p == c1 <;> cases hp2 : classOf p == c2 <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
          | exact absurd ((beq_iff_eq.mp hp1).symm.trans (beq_iff_eq.mp hp2)) hne
  | burdenDec v =>
      unfold pairEventAt
      simp only [instrEventAt]
      rw [mergeClasses_merged_membership classOf c1 c2 v]
      cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
        first
          | decide
          | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
  | holeFill v hole => exact Or.inl ⟨rfl, Or.inl rfl⟩

/-- §T5 (P4) BOUNDED IMPACT — the positional-interleave theorem. On a
    mutate-free stream with no jump-arg bridging the two merge participants,
    the merged class's derived ledger is EXACTLY the positional interleaving
    of the two prior classes' events. -/
theorem T5_merged_ledger_is_positional_interleave (classOf : Nat → Nat)
    (c1 c2 : Nat) (hne : c1 ≠ c2) (instrs : List LedgerInstr)
    (hmutfree : instrs.all (fun i => !(LedgerInstr.isMutate i)) = true)
    (hnobridge : instrs.all (fun i => !(isBridgingJump classOf c1 c2 i)) = true) :
    deriveLedger (mergeClasses classOf c1 c2) c2 instrs
      = instrs.filterMap (pairEventAt classOf c1 c2) := by
  rw [deriveLedger_eq_filterMap _ _ instrs hmutfree]
  apply filterMap_congr_local
  intro i hi
  rcases instrEventAt_merged_cases classOf c1 c2 hne i with
    ⟨hM, _⟩ | ⟨he1, he2, _⟩ | ⟨he1, he2, _⟩
  · exact hM
  · -- A bridging consume/credit shape contradicts the no-bridging hypothesis:
    -- only a jump-arg with source in c1 and receiver in c2 yields it, and
    -- exactly that combo refutes `hnobridge`; every other combo refutes the
    -- pinned event equalities.
    exfalso
    have hall := List.all_eq_true.mp hnobridge i hi
    cases i with
    | jumpArg v p =>
        cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
          cases hp1 : classOf p == c1 <;> cases hp2 : classOf p == c2 <;>
          first
            | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
            | exact absurd ((beq_iff_eq.mp hp1).symm.trans (beq_iff_eq.mp hp2)) hne
            | (rw [show isBridgingJump classOf c1 c2 (.jumpArg v p)
                    = ((classOf v == c1 && classOf p == c2)
                        || (classOf v == c2 && classOf p == c1)) from rfl,
                  hv1, hv2, hp1, hp2] at hall
               exact absurd hall (by decide))
            | simp [instrEventAt, hv1, hv2, hp1, hp2] at he1 he2
    | construct v => cases hv : classOf v == c1 <;> simp [instrEventAt, hv] at he1
    | dup v => cases hv : classOf v == c1 <;> simp [instrEventAt, hv] at he1
    | projRead v => cases hv : classOf v == c1 <;> simp [instrEventAt, hv] at he1
    | cowMutate v => simp [instrEventAt] at he1
    | escapeUse v u =>
        cases hv2 : classOf v == c2 <;>
          cases hu : rl2_use_transfers_ownership u <;>
          simp [instrEventAt, hv2, hu] at he2
    | burdenDec v => cases hv2 : classOf v == c2 <;> simp [instrEventAt, hv2] at he2
    | holeFill v hole => simp [instrEventAt] at he1
  · -- Mirrored bridging shape (source in c2, receiver in c1); same scheme.
    exfalso
    have hall := List.all_eq_true.mp hnobridge i hi
    cases i with
    | jumpArg v p =>
        cases hv1 : classOf v == c1 <;> cases hv2 : classOf v == c2 <;>
          cases hp1 : classOf p == c1 <;> cases hp2 : classOf p == c2 <;>
          first
            | exact absurd ((beq_iff_eq.mp hv1).symm.trans (beq_iff_eq.mp hv2)) hne
            | exact absurd ((beq_iff_eq.mp hp1).symm.trans (beq_iff_eq.mp hp2)) hne
            | (rw [show isBridgingJump classOf c1 c2 (.jumpArg v p)
                    = ((classOf v == c1 && classOf p == c2)
                        || (classOf v == c2 && classOf p == c1)) from rfl,
                  hv1, hv2, hp1, hp2] at hall
               exact absurd hall (by decide))
            | simp [instrEventAt, hv1, hv2, hp1, hp2] at he1 he2
    | construct v => cases hv : classOf v == c1 <;> simp [instrEventAt, hv] at he1
    | dup v => cases hv2 : classOf v == c2 <;> simp [instrEventAt, hv2] at he2
    | projRead v => cases hv : classOf v == c1 <;> simp [instrEventAt, hv] at he1
    | cowMutate v => simp [instrEventAt] at he1
    | escapeUse v u =>
        cases hv1 : classOf v == c1 <;>
          cases hu : rl2_use_transfers_ownership u <;>
          simp [instrEventAt, hv1, hu] at he1
    | burdenDec v => cases hv : classOf v == c1 <;> simp [instrEventAt, hv] at he1
    | holeFill v hole => simp [instrEventAt] at he1

/-- §T5 (P4) the BRIDGING-JUMP COLLAPSE — the one folding shape, stated
    honestly: a jump-arg handing off from the retired class to the absorbing
    class was a cross-class handoff (consume from the source, credit into
    the receiver — the committed `jump_arg_cross_class_handoff`); after the
    merge both ends share the class and the handoff is the RL-4 exemption
    (NO event). Net-preserving: the collapsed pair summed to zero. -/
theorem T5_bridging_jump_collapses (classOf : Nat → Nat) (c1 c2 v p : Nat)
    (hne : c1 ≠ c2)
    (hv : (classOf v == c1) = true) (hp : (classOf p == c2) = true) :
    deriveLedger classOf c1 [.jumpArg v p] = [.consume]
    ∧ deriveLedger classOf c2 [.jumpArg v p] = [.credit]
    ∧ deriveLedger (mergeClasses classOf c1 c2) c2 [.jumpArg v p] = []
    ∧ ledgerNet [LedgerEvent.consume] + ledgerNet [LedgerEvent.credit]
        = ledgerNet ([] : List LedgerEvent) := by
  have hvc2 : (classOf v == c2) = false :=
    beq_second_false_of_first (classOf v) c1 c2 hne hv
  have hpc1 : (classOf p == c1) = false :=
    beq_second_false_of_first (classOf p) c2 c1 (Ne.symm hne) hp
  have hcross := jump_arg_cross_class_handoff classOf c1 c2 v p hv hpc1 hvc2 hp
  refine ⟨hcross.1, hcross.2, ?_, by decide⟩
  have hmv : (mergeClasses classOf c1 c2 v == c2) = true := by
    rw [mergeClasses_merged_membership classOf c1 c2 v, hv]
    rfl
  have hmp : (mergeClasses classOf c1 c2 p == c2) = true := by
    rw [mergeClasses_merged_membership classOf c1 c2 p, hpc1, hp]
    rfl
  exact jump_arg_same_class_exempt (mergeClasses classOf c1 c2) c2 v p hmv hmp

/-! ## §T5 Part H — robustness: clauses 2 + 3 under the merge

    The merged running count at every prefix is the SUM of the two prior
    running counts (Part F per instruction). A read belonging to one prior
    class carries that class's own floor (>= 1); the sum meets the merged
    floor exactly when the OTHER class's count is nonnegative there. Hence
    the two HONEST side conditions:

    * running-nonnegativity — each prior class's running count stays >= 0 at
      every prefix (`countNeverNegative`). NECESSARY: the three clauses
      alone tolerate an interior negative dip (a consume path-ordered before
      its funding birth, floors silent absent reads), and two individually-
      clause-satisfying ledgers then interleave into a READ-at-0 shape —
      `T5_unconditional_floor_preservation_false` exhibits it.
    * mutate-freeness — a dynamic-COW mutate's sibling floor is computed
      from the path suffix, and the merge can RAISE it (a formerly-cross-
      class suffix reader becomes a live sibling) past a running count that
      satisfied the pre-merge floor — `T5_mutate_floor_rises_under_merge_rejected`
      exhibits it. Clause-3 preservation genuinely requires re-deriving the
      sibling floors at the merged partition.

    Clause 1 needs NEITHER (Part F is unconditional). -/

/-- §T5 running-nonnegativity: no prefix of the ledger drives the running
    count below zero. -/
def countNeverNegative : Int → List LedgerEvent → Bool
  | _, [] => true
  | n, e :: rest =>
      decide (0 ≤ n + eventDelta e) && countNeverNegative (n + eventDelta e) rest

/-- §T5 split a cons floor check. -/
theorem clauseFloors_cons_split (n : Int) (e : LedgerEvent)
    (es : List LedgerEvent) (h : clauseFloors n (e :: es) = true) :
    eventFloor n e = true ∧ clauseFloors (n + eventDelta e) es = true := by
  have h' : (eventFloor n e && clauseFloors (n + eventDelta e) es) = true := h
  rw [Bool.and_eq_true] at h'
  exact h'

/-- §T5 split a cons nonnegativity check. -/
theorem countNeverNegative_cons_split (n : Int) (e : LedgerEvent)
    (es : List LedgerEvent) (h : countNeverNegative n (e :: es) = true) :
    0 ≤ n + eventDelta e ∧ countNeverNegative (n + eventDelta e) es = true := by
  have h' : (decide (0 ≤ n + eventDelta e)
      && countNeverNegative (n + eventDelta e) es) = true := h
  rw [Bool.and_eq_true] at h'
  exact ⟨of_decide_eq_true h'.1, h'.2⟩

/-- §T5 build a cons floor check. -/
theorem clauseFloors_cons_build (n : Int) (e : LedgerEvent)
    (es : List LedgerEvent) (hf : eventFloor n e = true)
    (hrest : clauseFloors (n + eventDelta e) es = true) :
    clauseFloors n (e :: es) = true := by
  show (eventFloor n e && clauseFloors (n + eventDelta e) es) = true
  rw [Bool.and_eq_true]
  exact ⟨hf, hrest⟩

/-- §T5 the derivation cons-step through the per-instruction image
    (non-mutate head): cons the image event when the instruction touches the
    class, pass through otherwise. -/
theorem deriveLedger_cons_eventAt (f : Nat → Nat) (c : Nat) (i : LedgerInstr)
    (hmut : LedgerInstr.isMutate i = false) (rest : List LedgerInstr) :
    deriveLedger f c (i :: rest)
      = (match instrEventAt f c i with
          | some e => e :: deriveLedger f c rest
          | none => deriveLedger f c rest) := by
  cases i with
  | cowMutate v => simp [LedgerInstr.isMutate] at hmut
  | holeFill v hole => simp [LedgerInstr.isMutate] at hmut
  | construct v => cases hv : f v == c <;> simp [deriveLedger, instrEventAt, hv]
  | dup v => cases hv : f v == c <;> simp [deriveLedger, instrEventAt, hv]
  | projRead v => cases hv : f v == c <;> simp [deriveLedger, instrEventAt, hv]
  | escapeUse v u => cases hv : f v == c <;> simp [deriveLedger, instrEventAt, hv]
  | jumpArg v p =>
      cases hv : f v == c <;> cases hp : f p == c <;>
        simp [deriveLedger, instrEventAt, hv, hp]
  | burdenDec v => cases hv : f v == c <;> simp [deriveLedger, instrEventAt, hv]

/-- §T5 (P5) the floors-composition lemma: with both prior running counts
    threaded (starting nonnegative, kept nonnegative by the running-
    nonnegativity checks) and both prior floor checks in force, the merged
    class's floor check holds from the SUM count — each firing event's floor
    is met by its own class's floor plus the other class's nonnegativity;
    a bridging jump-arg moves one unit between the two counts, leaving the
    sum (and the merged ledger) untouched. Mutate-free streams. -/
theorem T5_merged_floors_of_prior (classOf : Nat → Nat) (c1 c2 : Nat)
    (hne : c1 ≠ c2) :
    ∀ (instrs : List LedgerInstr) (n1 n2 : Int),
      instrs.all (fun i => !(LedgerInstr.isMutate i)) = true →
      0 ≤ n1 → 0 ≤ n2 →
      clauseFloors n1 (deriveLedger classOf c1 instrs) = true →
      clauseFloors n2 (deriveLedger classOf c2 instrs) = true →
      countNeverNegative n1 (deriveLedger classOf c1 instrs) = true →
      countNeverNegative n2 (deriveLedger classOf c2 instrs) = true →
      clauseFloors (n1 + n2)
        (deriveLedger (mergeClasses classOf c1 c2) c2 instrs) = true := by
  intro instrs
  induction instrs with
  | nil => intro n1 n2 _ _ _ _ _ _ _; rfl
  | cons i rest ih =>
      intro n1 n2 hmut h0n1 h0n2 hf1 hf2 hnn1 hnn2
      rw [List.all_cons, Bool.and_eq_true] at hmut
      obtain ⟨hmi, hmrest⟩ := hmut
      have hmi' : LedgerInstr.isMutate i = false := by
        cases hm : LedgerInstr.isMutate i
        · rfl
        · rw [hm] at hmi; exact absurd hmi (by decide)
      rw [deriveLedger_cons_eventAt classOf c1 i hmi' rest] at hf1 hnn1
      rw [deriveLedger_cons_eventAt classOf c2 i hmi' rest] at hf2 hnn2
      rw [deriveLedger_cons_eventAt (mergeClasses classOf c1 c2) c2 i hmi' rest]
      rcases instrEventAt_merged_cases classOf c1 c2 hne i with
        ⟨hM, hnone⟩ | ⟨he1, he2, hM⟩ | ⟨he1, he2, hM⟩
      · rcases hnone with h1n | h2n
        · -- c1 contributes nothing here; only c2 may fire.
          rw [h1n] at hf1 hnn1
          have hMp : instrEventAt (mergeClasses classOf c1 c2) c2 i
              = instrEventAt classOf c2 i := by
            rw [hM]
            unfold pairEventAt
            rw [h1n]
          rw [hMp]
          cases h2 : instrEventAt classOf c2 i with
          | none =>
              rw [h2] at hf2 hnn2
              exact ih n1 n2 hmrest h0n1 h0n2 hf1 hf2 hnn1 hnn2
          | some e =>
              rw [h2] at hf2 hnn2
              obtain ⟨hfe, hf2'⟩ := clauseFloors_cons_split _ _ _ hf2
              obtain ⟨hge, hnn2'⟩ := countNeverNegative_cons_split _ _ _ hnn2
              apply clauseFloors_cons_build
              · cases e with
                | read =>
                    have h1le : (1 : Int) ≤ n2 := of_decide_eq_true hfe
                    exact decide_eq_true (by omega)
                | mutate sibs =>
                    have h1le : (1 : Int) + (sibs : Int) ≤ n2 :=
                      of_decide_eq_true hfe
                    exact decide_eq_true (by omega)
                | birth => rfl
                | credit => rfl
                | consume => rfl
              · have harith : n1 + n2 + eventDelta e = n1 + (n2 + eventDelta e) := by
                  omega
                rw [harith]
                exact ih n1 (n2 + eventDelta e) hmrest h0n1 (by omega)
                  hf1 hf2' hnn1 hnn2'
        · -- c2 contributes nothing here; only c1 may fire.
          rw [h2n] at hf2 hnn2
          cases h1 : instrEventAt classOf c1 i with
          | none =>
              rw [h1] at hf1 hnn1
              have hMp : instrEventAt (mergeClasses classOf c1 c2) c2 i = none := by
                rw [hM]
                unfold pairEventAt
                rw [h1, h2n]
              rw [hMp]
              exact ih n1 n2 hmrest h0n1 h0n2 hf1 hf2 hnn1 hnn2
          | some e =>
              rw [h1] at hf1 hnn1
              have hMp : instrEventAt (mergeClasses classOf c1 c2) c2 i
                  = some e := by
                rw [hM]
                unfold pairEventAt
                rw [h1]
              rw [hMp]
              obtain ⟨hfe, hf1'⟩ := clauseFloors_cons_split _ _ _ hf1
              obtain ⟨hge, hnn1'⟩ := countNeverNegative_cons_split _ _ _ hnn1
              apply clauseFloors_cons_build
              · cases e with
                | read =>
                    have h1le : (1 : Int) ≤ n1 := of_decide_eq_true hfe
                    exact decide_eq_true (by omega)
                | mutate sibs =>
                    have h1le : (1 : Int) + (sibs : Int) ≤ n1 :=
                      of_decide_eq_true hfe
                    exact decide_eq_true (by omega)
                | birth => rfl
                | credit => rfl
                | consume => rfl
              · have harith : n1 + n2 + eventDelta e = (n1 + eventDelta e) + n2 := by
                  omega
                rw [harith]
                exact ih (n1 + eventDelta e) n2 hmrest (by omega) h0n2
                  hf1' hf2 hnn1' hnn2
      · -- Bridging handoff c1 -> c2: one unit moves between the counts; the
        -- sum and the merged ledger are untouched.
        rw [he1] at hf1 hnn1
        rw [he2] at hf2 hnn2
        rw [hM]
        obtain ⟨_, hf1'⟩ := clauseFloors_cons_split _ _ _ hf1
        obtain ⟨hge1, hnn1'⟩ := countNeverNegative_cons_split _ _ _ hnn1
        obtain ⟨_, hf2'⟩ := clauseFloors_cons_split _ _ _ hf2
        obtain ⟨hge2, hnn2'⟩ := countNeverNegative_cons_split _ _ _ hnn2
        have h0n2' : (0 : Int) ≤ n2 + eventDelta .credit := by
          show (0 : Int) ≤ n2 + 1
          omega
        have harith : n1 + n2
            = (n1 + eventDelta .consume) + (n2 + eventDelta .credit) := by
          show n1 + n2 = (n1 + (-1)) + (n2 + 1)
          omega
        rw [harith]
        exact ih (n1 + eventDelta .consume) (n2 + eventDelta .credit) hmrest
          hge1 h0n2' hf1' hf2' hnn1' hnn2'
      · -- Mirrored bridging handoff c2 -> c1.
        rw [he1] at hf1 hnn1
        rw [he2] at hf2 hnn2
        rw [hM]
        obtain ⟨_, hf1'⟩ := clauseFloors_cons_split _ _ _ hf1
        obtain ⟨hge1, hnn1'⟩ := countNeverNegative_cons_split _ _ _ hnn1
        obtain ⟨_, hf2'⟩ := clauseFloors_cons_split _ _ _ hf2
        obtain ⟨hge2, hnn2'⟩ := countNeverNegative_cons_split _ _ _ hnn2
        have h0n1' : (0 : Int) ≤ n1 + eventDelta .credit := by
          show (0 : Int) ≤ n1 + 1
          omega
        have harith : n1 + n2
            = (n1 + eventDelta .credit) + (n2 + eventDelta .consume) := by
          show n1 + n2 = (n1 + 1) + (n2 + (-1))
          omega
        rw [harith]
        exact ih (n1 + eventDelta .credit) (n2 + eventDelta .consume) hmrest
          h0n1' hge2 hf1' hf2' hnn1' hnn2'

/-- §T5 (P5) THE ROBUSTNESS COROLLARY. Both prior classes satisfying the
    three clauses, both running counts staying nonnegative at every prefix
    (the READ-ordering-hazard guard), and the stream mutate-free: the merged
    class satisfies all three clauses. Clause 1 rides the unconditional net
    additivity; clauses 2/3 ride the floors composition. The two side
    conditions are each NECESSARY — the two witnesses below reject the
    theorem forms without them. -/
theorem T5_merged_class_clauses_robust (classOf : Nat → Nat) (c1 c2 : Nat)
    (hne : c1 ≠ c2) (instrs : List LedgerInstr)
    (hmutfree : instrs.all (fun i => !(LedgerInstr.isMutate i)) = true)
    (h1 : threeClauses (deriveLedger classOf c1 instrs) = true)
    (h2 : threeClauses (deriveLedger classOf c2 instrs) = true)
    (hnn1 : countNeverNegative 0 (deriveLedger classOf c1 instrs) = true)
    (hnn2 : countNeverNegative 0 (deriveLedger classOf c2 instrs) = true) :
    threeClauses (deriveLedger (mergeClasses classOf c1 c2) c2 instrs) = true := by
  have h1' : (clauseNetZero (deriveLedger classOf c1 instrs)
      && clauseFloors 0 (deriveLedger classOf c1 instrs)) = true := h1
  have h2' : (clauseNetZero (deriveLedger classOf c2 instrs)
      && clauseFloors 0 (deriveLedger classOf c2 instrs)) = true := h2
  rw [Bool.and_eq_true] at h1' h2'
  show (clauseNetZero (deriveLedger (mergeClasses classOf c1 c2) c2 instrs)
      && clauseFloors 0 (deriveLedger (mergeClasses classOf c1 c2) c2 instrs))
      = true
  rw [Bool.and_eq_true]
  constructor
  · exact T5_merged_clause1_net_zero classOf c1 c2 hne instrs h1'.1 h2'.1
  · exact T5_merged_floors_of_prior classOf c1 c2 hne instrs 0 0 hmutfree
      (by omega) (by omega) h1'.2 h2'.2 hnn1 hnn2

/-! ### §T5 Part H.1 — the two necessity witnesses (computed derivations
    over the REAL empty-edge union-find; identity classes) -/

def t5wUF : PartitionUF := buildPartitionUF []

def t5wClassOf : Nat → Nat := fun v => t5wUF.find v

/-- §T5 the READ-at-0 stream: class 601's release is path-ordered BEFORE its
    funding birth (net still zero — the three clauses are silent on the dip
    absent reads); class 600 lives entirely inside the dip. -/
def t5wReadAt0Instrs : List LedgerInstr :=
  [.burdenDec 601, .construct 600, .projRead 600, .burdenDec 600, .construct 601]

/-- §T5 (P6) the UNCONDITIONAL clause-2/3 preservation is FALSE: both prior
    classes satisfy the three clauses, yet the merged class READS at count 0
    (the use-after-free the operational machine observes). The violated side
    condition is pinpointed: class 601's running count dips negative
    (`countNeverNegative` false) — the READ-before-birth ordering hazard the
    robustness corollary's guard exists for. Clause 1 still composes (the
    merged net is zero) — exactly the Part-F unconditional slice. -/
theorem T5_unconditional_floor_preservation_false :
    threeClauses (deriveLedger t5wClassOf (t5wClassOf 600) t5wReadAt0Instrs) = true
    ∧ threeClauses (deriveLedger t5wClassOf (t5wClassOf 601) t5wReadAt0Instrs) = true
    ∧ countNeverNegative 0
        (deriveLedger t5wClassOf (t5wClassOf 601) t5wReadAt0Instrs) = false
    ∧ threeClauses
        (deriveLedger (mergeClasses t5wClassOf (t5wClassOf 600) (t5wClassOf 601))
          (t5wClassOf 601) t5wReadAt0Instrs) = false
    ∧ (runLedger
        (deriveLedger (mergeClasses t5wClassOf (t5wClassOf 600) (t5wClassOf 601))
          (t5wClassOf 601) t5wReadAt0Instrs)).uaf = true
    ∧ clauseNetZero
        (deriveLedger (mergeClasses t5wClassOf (t5wClassOf 600) (t5wClassOf 601))
          (t5wClassOf 601) t5wReadAt0Instrs) = true := by
  decide

/-- §T5 the mutate stream: class 800's dynamic-COW mutate satisfies its
    pre-merge sibling floor (no same-class suffix reader), and class 801's
    reader sits in the suffix — cross-class before the merge, a live sibling
    after it. -/
def t5wMutInstrs : List LedgerInstr :=
  [.construct 800, .cowMutate 800, .construct 801, .projRead 801,
   .burdenDec 801, .burdenDec 800]

/-- §T5 (P6) MUTATE-FREENESS is a necessary hypothesis: both prior classes
    satisfy the three clauses AND both running counts stay nonnegative — the
    other side condition holds — yet the merge RAISES the mutate's sibling
    floor (the formerly-cross-class suffix reader of 801 became a live
    sibling of 800's class) past the running count: clause 3 computes false
    and the operational machine observes the COW hazard. Sibling floors must
    be re-derived at the merged partition. -/
theorem T5_mutate_floor_rises_under_merge_rejected :
    threeClauses (deriveLedger t5wClassOf (t5wClassOf 800) t5wMutInstrs) = true
    ∧ threeClauses (deriveLedger t5wClassOf (t5wClassOf 801) t5wMutInstrs) = true
    ∧ countNeverNegative 0
        (deriveLedger t5wClassOf (t5wClassOf 800) t5wMutInstrs) = true
    ∧ countNeverNegative 0
        (deriveLedger t5wClassOf (t5wClassOf 801) t5wMutInstrs) = true
    ∧ threeClauses
        (deriveLedger (mergeClasses t5wClassOf (t5wClassOf 800) (t5wClassOf 801))
          (t5wClassOf 801) t5wMutInstrs) = false
    ∧ (runLedger
        (deriveLedger (mergeClasses t5wClassOf (t5wClassOf 800) (t5wClassOf 801))
          (t5wClassOf 801) t5wMutInstrs)).cowHazard = true := by
  decide

/-! ## §T5 Part I — T1 grounding: the new edge is a tier-1 admission; the
    union computes the redirect

    A newly-introduced semantic same-allocation edge enters the T1 partition
    ONLY through the admission rule; equal birth-sites make it a
    `PartitionAdm.tier1` edge, so `samerep_birthsite_sound` keeps the merge
    birth-site-sound. At the class-map level the T1 `PartitionUF.union` IS
    the `mergeClasses` redirect — witnessed computationally on a concrete
    union-find over the touched value universe (the T1 Part-C discipline:
    concrete representatives are COMPUTED through the real fuelled
    find/union, never hardcoded). -/

/-- §T5 (P1) the introduced semantic edge is a tier-1 ADMISSION: equal
    birth-sites license it, so the merge unifies only same-birth-site
    classes (T1 `samerep_birthsite_sound` binds every downstream
    consequence). -/
theorem T5_new_edge_is_tier1_admission {ν β : Type} {birthSite : ν → β}
    {u v : ν} (h : birthSite u = birthSite v) : PartitionAdm birthSite u v :=
  .tier1 h

/-- §T5 the pre-merge witness union-find: {900, 901} one class (a tier-1
    view edge), 902 / 903 / 904 singletons. -/
def t5UF : PartitionUF := buildPartitionUF [(901, 900)]

def t5ClassOfU : Nat → Nat := fun v => t5UF.find v

/-- §T5 the post-merge union-find: the NEW tier-1 edge (902, 900) folded in
    through the REAL `PartitionUF.union`. -/
def t5UFMerged : PartitionUF := t5UF.union 902 900

/-- §T5 (P1) the union IS the redirect on the witness universe: for every
    touched value, the post-merge COMPUTED representative equals the
    `mergeClasses` redirect of the pre-merge representative (retired rep =
    the computed class of 902; absorbing rep = the computed class of 900). -/
theorem T5_union_is_mergeClasses_on_witness :
    ([900, 901, 902, 903, 904].all (fun v =>
      t5UFMerged.find v
        == mergeClasses t5ClassOfU (t5ClassOfU 902) (t5ClassOfU 900) v)) = true := by
  decide

/-- §T5 (P1) the merge is genuine on the witness: pre-merge 902 and 900 hold
    DISTINCT computed representatives; post-merge they share one; the
    distant class 903 keeps its representative verbatim. -/
theorem T5_witness_merge_shape :
    t5UF.sameRep 902 900 = false
    ∧ t5UFMerged.sameRep 902 900 = true
    ∧ t5UFMerged.find 903 = t5UF.find 903 := by
  decide

/-- §T5 (P2 instantiated) the frame theorem applied at the concrete witness:
    the distant class 903's derived ledger is VERBATIM unchanged by the
    902-900 tier-1 merge, for an arbitrary instruction stream over 903. -/
theorem T5_distant_class_untouched_on_witness (instrs : List LedgerInstr) :
    deriveLedger (mergeClasses t5ClassOfU (t5ClassOfU 902) (t5ClassOfU 900))
        (t5ClassOfU 903) instrs
      = deriveLedger t5ClassOfU (t5ClassOfU 903) instrs :=
  T5_frame_untouched_class_ledger_verbatim t5ClassOfU
    (t5ClassOfU 902) (t5ClassOfU 900) (t5ClassOfU 903)
    (by decide) (by decide) instrs

/-! ## §T4 / §T5 conclusion bundles -/

/-- §T4 full boundary/result-plan bundle. Direct Invoke composition, CFG-bound
    result-plan totality, and Ownerless/Owned callable accounting are one
    backend-neutral theorem surface; VM and LLVM projections consume the same
    semantic facts rather than forking ownership calculus. -/
theorem T4_contract_boundary_and_result_plan_sound :
    (∀ (classOf cc : Nat → Nat) (c arg ret p : Nat) (bc : BoundaryContract)
        (preI postI body : List LedgerInstr),
      (classOf arg == c) = true →
      preI.all (fun i => !(LedgerInstr.isMutate i)) = true →
      threeClauses (deriveLedger classOf c
          (preI ++ boundaryInstrs arg ret bc ++ postI)) = true →
      calleeConforms cc p bc body = true →
      1 ≤ ledgerNet (deriveLedger classOf c preI) →
      threeClauses
          (deriveLedger classOf c preI
            ++ (deriveLedger cc (cc p) body
            ++ (deriveLedger classOf c (boundaryResultInstrs ret bc)
            ++ deriveLedger classOf c postI))) = true
        ∧ invokeBoundaryEvents bc .normal
            = boundaryArgEvent bc :: boundaryResultEvents bc
        ∧ invokeBoundaryEvents bc .unwind = [boundaryArgEvent bc])
    ∧ (∀ (inventory : FunctionExitInventory)
        (evidences : List NormalReturnEvidence) (plan : FrozenFunctionResultPlan),
      FrozenFunctionResultPlan.freeze inventory evidences = some plan →
      plan.function = inventory.function
        ∧ plan.normalReturnSites = inventory.normalReturnSites
        ∧ plan.requirements = inventory.normalReturnRequirements
        ∧ identifiersUnique inventory.exitSites = true
        ∧ plan.rows.length = inventory.normalReturnRequirements.length)
    ∧ (∀ (entry : FrozenTargetOwnershipFact)
        (row : FrozenNormalReturnResult) (cc : Nat → Nat) (p : Nat)
        (normalBody unwindBody : List LedgerInstr),
      calleeConforms cc p entry.boundaryContract normalBody = true →
      calleeConforms cc p entry.boundaryContract unwindBody = true →
      ledgerNet (borrowedIndirectCallableRowEvents cc p entry row
          normalBody .normal) = row.ownerCount
        ∧ clauseFloors 1 (borrowedIndirectCallableRowEvents cc p entry row
            normalBody .normal) = true
        ∧ ledgerNet (borrowedIndirectCallableRowEvents cc p entry row
            unwindBody .unwind) = 0
        ∧ clauseFloors 1 (borrowedIndirectCallableRowEvents cc p entry row
            unwindBody .unwind) = true) := by
  refine ⟨?_, FrozenFunctionResultPlan.freeze_preserves_inventory, ?_⟩
  · intro classOf cc c arg ret p bc preI postI body harg hpre hcaller
      hcallee hlive
    exact T4_contract_boundary_invoke_composition_sound classOf cc c arg ret p
      bc preI postI body harg hpre hcaller hcallee hlive
  · intro entry row cc p normalBody unwindBody hnormal hunwind
    exact T4_PV4_normal_return_row_callable_composition_sound entry row cc p
      normalBody unwindBody hnormal hunwind

/-- §T4 the contract-boundary bundle: the composition theorem's operational
    corollary, the classification-table rows (owned-arg consume, iter-consume
    consume, borrowed read, passthrough netting zero, sharing-view credit,
    owned-param callee birth), the no-boundary-release law, and the two
    rejections (misclassified boundary; dead call-site handoff). -/
theorem T4_contract_boundary_bundle :
    (∀ (classOf cc : Nat → Nat) (c arg ret p : Nat) (bc : BoundaryContract)
        (preI postI body : List LedgerInstr),
      (classOf arg == c) = true →
      preI.all (fun i => !(LedgerInstr.isMutate i)) = true →
      threeClauses (deriveLedger classOf c
          (preI ++ boundaryInstrs arg ret bc ++ postI)) = true →
      calleeConforms cc p bc body = true →
      1 ≤ ledgerNet (deriveLedger classOf c preI) →
      ledgerSafe
        (deriveLedger classOf c preI
          ++ (deriveLedger cc (cc p) body
          ++ (deriveLedger classOf c (boundaryResultInstrs ret bc)
          ++ deriveLedger classOf c postI))))
    ∧ (∀ (cc : Nat → Nat) (p : Nat) (bc : BoundaryContract)
        (body : List LedgerInstr), boundaryTransfersIn bc = true →
      calleeLedger cc p bc body = .birth :: deriveLedger cc (cc p) body)
    ∧ (∀ arg ret : Nat, ∀ bc : BoundaryContract,
        (boundaryInstrs arg ret bc).all
          (fun i => match i with | .burdenDec _ => false | _ => true) = true)
    ∧ threeClauses
        (deriveLedger t4ClassOf (t4ClassOf 700) t4PreI
          ++ (deriveLedger t4ClassOf (t4ClassOf 710) t4CalleeBody
          ++ (deriveLedger t4ClassOf (t4ClassOf 700)
                (boundaryResultInstrs 701 t4BcMis)
          ++ deriveLedger t4ClassOf (t4ClassOf 700) t4PostMis))) = false :=
  ⟨fun classOf cc c arg ret p bc preI postI body harg hpre hcaller hcallee hlive =>
      T4_contract_boundary_composition_safe classOf cc c arg ret p bc
        preI postI body harg hpre hcaller hcallee hlive,
   fun cc p bc body htr => T4_owned_param_births_callee_ledger cc p bc body htr,
   T4_boundary_places_no_release,
   T4_owned_arg_misclassified_borrowed_rejected.2.2.2.1⟩

/-- §T5 the frame-limited-robustness bundle: the verbatim frame for every
    untouched class, the retired class emptied, the unconditional net
    additivity + clause-1 composition, the mutate-free non-bridging
    positional interleave, the honest-side-condition robustness corollary,
    and the two rejections (READ-at-0 interleave; merge-raised sibling
    floor). -/
theorem T5_frame_limited_robustness_bundle :
    (∀ (classOf : Nat → Nat) (c1 c2 c : Nat), c ≠ c1 → c ≠ c2 →
      ∀ instrs : List LedgerInstr,
        deriveLedger (mergeClasses classOf c1 c2) c instrs
          = deriveLedger classOf c instrs)
    ∧ (∀ (classOf : Nat → Nat) (c1 c2 : Nat), c1 ≠ c2 →
        ∀ instrs : List LedgerInstr,
          deriveLedger (mergeClasses classOf c1 c2) c1 instrs = [])
    ∧ (∀ (classOf : Nat → Nat) (c1 c2 : Nat), c1 ≠ c2 →
        ∀ instrs : List LedgerInstr,
          ledgerNet (deriveLedger (mergeClasses classOf c1 c2) c2 instrs)
            = ledgerNet (deriveLedger classOf c1 instrs)
              + ledgerNet (deriveLedger classOf c2 instrs))
    ∧ (∀ (classOf : Nat → Nat) (c1 c2 : Nat), c1 ≠ c2 →
        ∀ instrs : List LedgerInstr,
          instrs.all (fun i => !(LedgerInstr.isMutate i)) = true →
          threeClauses (deriveLedger classOf c1 instrs) = true →
          threeClauses (deriveLedger classOf c2 instrs) = true →
          countNeverNegative 0 (deriveLedger classOf c1 instrs) = true →
          countNeverNegative 0 (deriveLedger classOf c2 instrs) = true →
          threeClauses
            (deriveLedger (mergeClasses classOf c1 c2) c2 instrs) = true)
    ∧ threeClauses
        (deriveLedger (mergeClasses t5wClassOf (t5wClassOf 600) (t5wClassOf 601))
          (t5wClassOf 601) t5wReadAt0Instrs) = false
    ∧ threeClauses
        (deriveLedger (mergeClasses t5wClassOf (t5wClassOf 800) (t5wClassOf 801))
          (t5wClassOf 801) t5wMutInstrs) = false :=
  ⟨fun classOf c1 c2 c hne1 hne2 instrs =>
      T5_frame_untouched_class_ledger_verbatim classOf c1 c2 c hne1 hne2 instrs,
   fun classOf c1 c2 hne instrs =>
      T5_retired_class_ledger_empty classOf c1 c2 hne instrs,
   fun classOf c1 c2 hne instrs =>
      T5_merged_net_additive classOf c1 c2 hne instrs,
   fun classOf c1 c2 hne instrs hmut h1 h2 hnn1 hnn2 =>
      T5_merged_class_clauses_robust classOf c1 c2 hne instrs hmut h1 h2 hnn1 hnn2,
   T5_unconditional_floor_preservation_false.2.2.2.1,
   T5_mutate_floor_rises_under_merge_rejected.2.2.2.2.1⟩

end AimsProof
