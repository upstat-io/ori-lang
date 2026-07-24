/-
AIMS transfer-function module — kernel-checked Lean proofs of the per-instruction
forward transfer functions, the backward-demand accumulation operators, and the
L-6 layer (b) monotonicity obligations over the finite `AimsState` product.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: TF-3..TF-9a (forward), TF-11 / TF-14 (backward), TF-8 (Select), L-6 layer-b |
  spec: annex-e §AIMS §4 + Appendix A |
  .proof: aims-proof/proofs/04-transfers/TF-*.proof + IA-5-step-1.proof + Composition.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).
  Note: SCALAR-sentinel rows (TF-1/10/10a) are not modeled as `AimsState`
  images — SCALAR is the L-9 absence-of-inhabitant sentinel. TF-2a is modeled
  through `PrimitiveTransferResult`, whose `scalar` case stays outside
  `AimsState`; TF-2b is the typed owned-result primitive case.

Correspondence: `docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS §4`
(Transfer Functions TF-1..TF-15a, forward + backward) + Appendix A
(Forward Transfer Matrix) + §3 ("Transfer functions shall be monotone:
`a ≤ b ⟹ f(a) ≤ f(b)`" — the L-6 layer (b) obligation, one per TF-N).

The rule IDs index the §4 obligations:
  TF-1   Let { Literal }      → SCALAR        (forward, constant)
  TF-2   Let { Var(v) }       → state(v)      (forward, identity)
  TF-2a  scalar PrimOp        → SCALAR        (forward, constant)
  TF-2b  owned-result PrimOp  → FRESH(shape)  (forward, descriptor-directed)
  TF-3   Construct            → FRESH(shape)  (forward, per-ctor constant)
  TF-4   Project              → Borrowed view of source (forward)
  TF-5   Apply (no contract)  → CONSERVATIVE  (forward, constant)
  TF-5a  ApplyIndirect        → CONSERVATIVE  (forward, constant)
  TF-6   Apply (contract)     → refine(CONSERVATIVE, return_contract)
  TF-6a  Invoke (contract)    → same as TF-6
  TF-6b  Invoke (no contract) → CONSERVATIVE  (forward, constant)
  TF-6c  InvokeIndirect       → CONSERVATIVE  (forward, constant)
  TF-7   PartialApply         → FRESH(NonReusable) (forward, constant)
  TF-8   Select               → componentwise join w/ uniqueness downgrade
  TF-9   Reuse                → FRESH(inherited shape) (forward, constant)
  TF-9a  CollectionReuse      → FRESH(CollectionBuffer) (forward, constant)
  TF-10  IsShared             → SCALAR        (forward, constant)
  TF-10a Reset                → SCALAR        (forward, constant)
  TF-11  backward demand      → seq_add accumulation over Consumption × Cardinality
  TF-14  Project backward     → propagate_project_source_demand (seq_add + max)
  L-6    layer (b)            → per-TF-N monotonicity `a ≤ b ⟹ f a ≤ f b`

The model SCALAR exclusion is L-9 (see `Lattice.lean` §L-9): SCALAR is not an
`AimsState` inhabitant. `PrimitiveTransferResult` makes that exclusion explicit:
TF-2a returns its `scalar` case while TF-2b returns a tracked `AimsState`. TF-1 /
TF-10 / TF-10a remain L-9-excluded rows without an `AimsState`-valued image. The
forward rules that produce an `AimsState`, the backward-accumulation operators
(TF-11, TF-14), and the L-6 layer-(b) monotonicity theorems are the finite-domain
propositions modeled and proven here.

TF-N/A first classifies the neutral logical effects `ownerCredit`, `release`,
and `cleanup`: none has a destination, forward `AimsState` image, or TF-11
backward demand (Appendix A "—" across every dimension). The current MIR
carriers (`RcInc` / `RcDec` / `BurdenInc` / `BurdenDec`) are enumerated
separately and refine that classification; they are not calculus vocabulary.
Burden-op ELIMINATION soundness
is the §11 coexistence family (`Coexistence.lean` `CH1_burden_emitted_is_bridge`
`burden_emitted = burden_owned`; `CH2_single_elimination` = lattice DP-2/DP-3
verdict), and the lowered (BurdenInc → RcInc / BurdenDec → RcDec) form's
RC-balance is `Realization.lean` `RL3_elision_net_preserving` (annex-e §AIMS §8).

Proof strategy mirrors `Lattice.lean`: per-dimension `cases` destructure before
`decide` so each leaf is trivial (naive `decide` over the ~7200-state product
times out). Monotonicity over `AimsState.le` (defined in `Lattice.lean` via the
componentwise `rawJoin`) is destructured per-dimension, then `decide`.
-/

import AimsProof.Model
import AimsProof.Lattice

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §TF-N/A — destination-free logical ownership effects

    This is the calculus-level classification. Transitional MIR carrier names
    are modeled separately below so adding or replacing a carrier cannot change
    the logical transfer rule. -/

/-- Neutral ownership effects that do not define an SSA destination. -/
inductive LogicalOwnershipEffect
  | ownerCredit
  | release
  | cleanup
deriving Repr, DecidableEq

def logicalEffectHasDestination (_ : LogicalOwnershipEffect) : Bool := false
def logicalEffectCreatesBackwardDemand (_ : LogicalOwnershipEffect) : Bool := false

/-- TF-N/A: logical ownership effects have neither a destination nor backward
    lattice demand. -/
theorem TFNA_logical_events_no_destination_or_demand
    (event : LogicalOwnershipEffect) :
    logicalEffectHasDestination event = false
      ∧ logicalEffectCreatesBackwardDemand event = false := by
  cases event <;> constructor <;> rfl

/-- Current transitional instruction carriers. This enumeration establishes
    coverage only; it does not promote carrier names into the calculus. -/
inductive TransitionalOwnershipCarrier
  | rcInc
  | rcDec
  | burdenInc
  | burdenDec
deriving Repr, DecidableEq

def carrierHasDestination (_ : TransitionalOwnershipCarrier) : Bool := false
def carrierCreatesBackwardDemand (_ : TransitionalOwnershipCarrier) : Bool := false

/-- Every current carrier refines the destination-free TF-N/A shape. -/
theorem TFNA_transitional_carriers_refine_logical_shape
    (carrier : TransitionalOwnershipCarrier) :
    carrierHasDestination carrier = false
      ∧ carrierCreatesBackwardDemand carrier = false := by
  cases carrier <;> constructor <;> rfl

/-! ## §1 Forward transfer-function images (annex-e §AIMS §4 + Appendix A)

    The forward transfers that produce an `AimsState` post-state. TF-3 / TF-7 /
    TF-9 / TF-9a are FRESH allocations parameterized by shape; TF-5 / TF-5a /
    TF-6b / TF-6c are the CONSERVATIVE constant; TF-4 is the Project borrow view;
    TF-6 / TF-6a are the `refine` of CONSERVATIVE by a return contract. -/

/-- `FRESH(shape)` = `(Owned, Linear, Once, Unique, BlockLocal, shape,
    {may_alloc=true})` per §4 TF-3 + Appendix A row 4. -/
def fresh (shape : Shape) : AimsState :=
  { access := .Owned
  , consumption := .Linear
  , cardinality := .One
  , uniqueness := .Unique
  , locality := .BlockLocal
  , shape := shape
  , effect := { may_alloc := true, may_share := false, may_throw := false } }

/-! ## §1.1 Typed primitive transfer descriptors (TF-2a / TF-2b)

    A primitive's ownership interface is semantic and backend-neutral. Result
    ownership is separate from physical allocation: a logically independent
    owned result may take over storage from a consumed input or allocate new
    storage. This distinction permits COW realization without claiming that a
    `FRESH` lattice result always performs a physical allocation. -/

/-- How a primitive uses one operand's ownership obligation. -/
inductive PrimitiveOperandUse
  | borrow
  | consume
deriving Repr, DecidableEq

/-- The semantic ownership origin of a primitive result.

    `ownedFromConsumedOrIndependent` means the result has one independent
    ownership obligation. A physical realization may source its storage from
    one of the named consumed inputs or allocate independently. -/
inductive PrimitiveResultOwnership
  | scalar
  | independentOwned
  | ownedFromConsumedOrIndependent (eligibleInputs : List Nat)
  | alias (operand : Nat)
deriving Repr, DecidableEq

/-- Physical allocation possibility, kept separate from result ownership. -/
inductive AllocationEffect
  | none
  | mayAllocate
  | strategyDependent
deriving Repr, DecidableEq

/-- Typed ownership descriptor attached to a primitive operation. -/
structure PrimitiveDescriptor where
  result : PrimitiveResultOwnership
  operandUses : List PrimitiveOperandUse
  allocation : AllocationEffect
deriving Repr, DecidableEq

/-- A descriptor is well formed for an arity when every operand has one use,
    aliases are in range, and every eligible storage-takeover source is both in
    range and consumed. Empty takeover sets are rejected. -/
def primitiveDescriptorValid (arity : Nat) (descriptor : PrimitiveDescriptor) : Bool :=
  descriptor.operandUses.length == arity &&
    match descriptor.result, descriptor.allocation with
    | .scalar, .none => true
    | .independentOwned, .mayAllocate => true
    | .alias operand, .none => operand < arity
    | .ownedFromConsumedOrIndependent eligible, .strategyDependent =>
        !eligible.isEmpty && eligible.all fun operand =>
          operand < arity && descriptor.operandUses[operand]? == some .consume
    | _, _ => false

/-- TF-2's result carrier extended with the L-9 SCALAR sentinel. -/
inductive PrimitiveTransferResult
  | scalar
  | tracked (state : AimsState)
deriving Repr, DecidableEq

/-- Descriptor-directed primitive transfer. `none` is the fail-closed image for
    missing/malformed ownership metadata. Owned-result primitives receive the
    canonical `FRESH(shape)` lattice state; `may_alloc` means may allocate and
    does not require every physical realization row to allocate. -/
def transferPrimitive
    (shape : Shape)
    (operandStates : List AimsState)
    (descriptor : PrimitiveDescriptor) : Option PrimitiveTransferResult :=
  if !primitiveDescriptorValid operandStates.length descriptor then none
  else match descriptor.result with
    | .scalar => some .scalar
    | .independentOwned | .ownedFromConsumedOrIndependent _ =>
        some (.tracked (fresh shape))
    | .alias operand => operandStates[operand]?.map .tracked

/-- Abstract physical strategies for a two-input ownership operation. These
    names describe storage provenance only; they carry no operator or backend
    identity. -/
inductive PrimitiveStrategy
  | takeOperandZero
  | takeOperandOne
  | allocateIndependent
deriving Repr, DecidableEq

inductive StorageSource
  | consumedOperand (operand : Nat)
  | independent
deriving Repr, DecidableEq

structure PrimitiveStrategyRow where
  consumedInputs : List Nat
  producedOwnerCount : Nat
  storageSource : StorageSource
  allocated : Bool
deriving Repr, DecidableEq

/-- The three physical realization rows admitted by a dual-consuming,
    one-owned-result primitive interface. -/
def realizeDualConsume : PrimitiveStrategy → PrimitiveStrategyRow
  | .takeOperandZero =>
      { consumedInputs := [0, 1]
      , producedOwnerCount := 1
      , storageSource := .consumedOperand 0
      , allocated := false }
  | .takeOperandOne =>
      { consumedInputs := [0, 1]
      , producedOwnerCount := 1
      , storageSource := .consumedOperand 1
      , allocated := false }
  | .allocateIndependent =>
      { consumedInputs := [0, 1]
      , producedOwnerCount := 1
      , storageSource := .independent
      , allocated := true }

/-- A realized storage source is funded either by a consumed input obligation
    or by the row's independent allocation. -/
def primitiveSourceIsFunded (row : PrimitiveStrategyRow) : Bool :=
  match row.storageSource with
  | .consumedOperand operand => operand ∈ row.consumedInputs
  | .independent => row.allocated

def dualConsumeDescriptor : PrimitiveDescriptor :=
  { result := .ownedFromConsumedOrIndependent [0, 1]
  , operandUses := [.consume, .consume]
  , allocation := .strategyDependent }

/-- TF-2a: a well-formed scalar primitive has the SCALAR sentinel image, not an
    `AimsState` inhabitant. -/
theorem TF2a_scalar_primitive_transfer
    (shape : Shape)
    (operandStates : List AimsState)
    (operandUses : List PrimitiveOperandUse)
    (arity : operandUses.length = operandStates.length) :
    transferPrimitive shape operandStates
      { result := .scalar
      , operandUses := operandUses
      , allocation := .none } = some .scalar := by
  simp [transferPrimitive, primitiveDescriptorValid, arity]

/-- TF-2b: the generic dual-consuming owned-result descriptor is valid. -/
theorem TF2b_dual_consume_descriptor_valid :
    primitiveDescriptorValid 2 dualConsumeDescriptor = true := by decide

/-- TF-2b: the generic dual-consuming primitive produces one logically owned
    `FRESH(shape)` result. The theorem states no physical allocation choice. -/
theorem TF2b_dual_consume_transfer_is_logically_owned
    (shape : Shape) (left right : AimsState) :
    transferPrimitive shape [left, right] dualConsumeDescriptor =
      some (.tracked (fresh shape)) := by
  rfl

/-- TF-2b: every admitted physical strategy consumes both input obligations,
    produces exactly one output obligation, and funds its storage source. This
    is the no-duplicate-obligation interface theorem. -/
theorem TF2b_dual_consume_rows_preserve_one_result_owner
    (strategy : PrimitiveStrategy) :
    let row := realizeDualConsume strategy
    row.consumedInputs = [0, 1] ∧
      row.producedOwnerCount = 1 ∧
      primitiveSourceIsFunded row = true := by
  cases strategy <;> decide

/-- TF-2b: storage takeover is not a physical allocation; only the independent
    row necessarily allocates in this abstract realization table. -/
theorem TF2b_storage_takeover_does_not_imply_allocation :
    (realizeDualConsume .takeOperandZero).allocated = false ∧
    (realizeDualConsume .takeOperandOne).allocated = false ∧
    (realizeDualConsume .allocateIndependent).allocated = true := by decide

/-- TF-2b fail-closed pin: an out-of-range takeover candidate that is not
    consumed has no transfer image. -/
theorem TF2b_malformed_descriptor_fails_closed (shape : Shape) (state : AimsState) :
    transferPrimitive shape [state]
      { result := .ownedFromConsumedOrIndependent [1]
      , operandUses := [.borrow]
      , allocation := .strategyDependent } = none := by
  rfl

/-- `shape_from_ctor` (§4 TF-3 + Appendix A): the seven constructor kinds map to
    four distinct shapes. Encoded over the `Shape` carrier directly (Struct ↦
    ReusableStruct, EnumVariant ↦ ReusableEnumVariant, the three collection
    literals ↦ CollectionBuffer, Tuple/Closure ↦ NonReusable). -/
inductive Ctor
  | Struct
  | EnumVariant
  | ListLiteral
  | SetLiteral
  | MapLiteral
  | Tuple
  | Closure
deriving Repr, DecidableEq

def shapeFromCtor : Ctor → Shape
  | .Struct      => .ReusableStruct
  | .EnumVariant => .ReusableEnumVariant
  | .ListLiteral => .CollectionBuffer
  | .SetLiteral  => .CollectionBuffer
  | .MapLiteral  => .CollectionBuffer
  | .Tuple       => .NonReusable
  | .Closure     => .NonReusable

/-- TF-3 forward: `Construct { ctor }` → `FRESH(shape_from_ctor(ctor))`. -/
def tfConstruct (c : Ctor) : AimsState := fresh (shapeFromCtor c)

/-- TF-7 forward: `PartialApply` → `FRESH(NonReusable)`. -/
def tfPartialApply : AimsState := fresh .NonReusable

/-- TF-9a forward: `CollectionReuse` → `FRESH(CollectionBuffer)`. -/
def tfCollectionReuse : AimsState := fresh .CollectionBuffer

/-- TF-9 forward: `Reuse` → `FRESH(inherited shape)` from the Reset token's
    original Construct shape. -/
def tfReuse (inheritedShape : Shape) : AimsState := fresh inheritedShape

/-- `CONSERVATIVE` = `(Owned, Unrestricted, Many, MaybeShared, Unknown,
    NonReusable, ALL)` per §4 TF-5 + Appendix A legend. NOT lattice TOP
    (`MaybeShared` < `Shared` to enable dynamic COW). -/
def conservative : AimsState :=
  { access := .Owned
  , consumption := .Unrestricted
  , cardinality := .Many
  , uniqueness := .MaybeShared
  , locality := .Unknown
  , shape := .NonReusable
  , effect := { may_alloc := true, may_share := true, may_throw := true } }

/-- TF-5 / TF-5a / TF-6b / TF-6c forward: unknown / indirect calls → CONSERVATIVE. -/
def tfApplyNoContract : AimsState := conservative

/-- TF-4 forward: `Project { value }` → `(Borrowed, Linear, Once, src.uniqueness,
    src.locality, NonReusable, NONE)` per §4 TF-4 + Appendix A row 5. Uniqueness
    + Locality inherit from the source; Access is Borrowed; Shape NonReusable. -/
def tfProject (src : AimsState) : AimsState :=
  { access := .Borrowed
  , consumption := .Linear
  , cardinality := .One
  , uniqueness := src.uniqueness
  , locality := src.locality
  , shape := .NonReusable
  , effect := { may_alloc := false, may_share := false, may_throw := false } }

/-- A return contract narrows the three dimensions §4 TF-6 `refine` touches:
    Uniqueness, Locality, Shape (Access / Consumption / Cardinality / Effect stay
    at CONSERVATIVE per the §4 TF-6 "Dimensions NOT narrowed by refine" list). -/
structure ReturnContract where
  uniqueness : Uniqueness
  locality : Locality
  shape : Shape
deriving Repr, DecidableEq

/-- TF-6 / TF-6a forward: `refine(CONSERVATIVE, contract)` — narrow Uniqueness,
    Locality, Shape from the callee's return contract; keep the other four
    dimensions at CONSERVATIVE per §4 TF-6. -/
def refine (base : AimsState) (rc : ReturnContract) : AimsState :=
  { base with
    uniqueness := rc.uniqueness
  , locality := rc.locality
  , shape := rc.shape }

def tfApplyContract (rc : ReturnContract) : AimsState := refine conservative rc

/-! ## §2 TF-3 / TF-7 / TF-9 / TF-9a forward-state determinism (annex-e §AIMS §4)

    Each FRESH-producing forward rule is a per-token constant function: the
    post-state is determined entirely by the static constructor/shape token, not
    by any input operand state. These are the §4 forward "well-definedness"
    theorems — the post-state equals the canonical FRESH(shape) for the token. -/

/-- TF-3: the Construct post-state is exactly FRESH(shape_from_ctor(ctor)). -/
theorem TF3_construct_fresh (c : Ctor) :
    tfConstruct c = fresh (shapeFromCtor c) := rfl

/-- TF-3: per-ctor shape mapping is exactly the Appendix A row-4 table. -/
theorem TF3_shape_mapping :
    shapeFromCtor .Struct = .ReusableStruct ∧
    shapeFromCtor .EnumVariant = .ReusableEnumVariant ∧
    shapeFromCtor .ListLiteral = .CollectionBuffer ∧
    shapeFromCtor .SetLiteral = .CollectionBuffer ∧
    shapeFromCtor .MapLiteral = .CollectionBuffer ∧
    shapeFromCtor .Tuple = .NonReusable ∧
    shapeFromCtor .Closure = .NonReusable := by decide

/-- TF-3: every FRESH logical allocation is `Unique` with one owner credit (the
    FRESH uniqueness invariant the reuse/COW layer relies on). -/
theorem TF3_fresh_unique (c : Ctor) : (tfConstruct c).uniqueness = .Unique := by
  cases c <;> rfl

/-- TF-3: every FRESH allocation carries `may_alloc = true` (Effect 0b001). -/
theorem TF3_fresh_may_alloc (c : Ctor) : (tfConstruct c).effect.may_alloc = true := by
  cases c <;> rfl

/-- TF-7: PartialApply produces FRESH(NonReusable). -/
theorem TF7_partial_apply_fresh : tfPartialApply = fresh .NonReusable := rfl

/-- TF-9a: CollectionReuse produces FRESH(CollectionBuffer). -/
theorem TF9a_collection_reuse_fresh : tfCollectionReuse = fresh .CollectionBuffer := rfl

/-- TF-9: Reuse produces FRESH(inherited shape) — shape determined by the token. -/
theorem TF9_reuse_fresh (sh : Shape) : tfReuse sh = fresh sh := rfl

/-! ## §3 TF-5 / TF-5a / TF-6b / TF-6c forward determinism (annex-e §AIMS §4)

    The unknown/indirect-call forward rules are the CONSERVATIVE constant. -/

/-- TF-5: Apply-no-contract is CONSERVATIVE. -/
theorem TF5_apply_no_contract_conservative : tfApplyNoContract = conservative := rfl

/-- TF-5: CONSERVATIVE is `MaybeShared` (NOT `Shared`) — enables dynamic COW
    per §4 TF-5 ("NOT lattice TOP for Uniqueness"). -/
theorem TF5_conservative_maybeshared : conservative.uniqueness = .MaybeShared := rfl

/-! ## §4 TF-6 refine well-definedness (annex-e §AIMS §4 TF-6)

    `refine` narrows exactly Uniqueness / Locality / Shape and leaves the other
    four dimensions at the base (CONSERVATIVE) value. -/

/-- TF-6: refine narrows exactly the three contract dimensions and preserves
    Access / Consumption / Cardinality / Effect from the base. -/
theorem TF6_refine_narrows (base : AimsState) (rc : ReturnContract) :
    (refine base rc).uniqueness = rc.uniqueness ∧
    (refine base rc).locality = rc.locality ∧
    (refine base rc).shape = rc.shape ∧
    (refine base rc).access = base.access ∧
    (refine base rc).consumption = base.consumption ∧
    (refine base rc).cardinality = base.cardinality ∧
    (refine base rc).effect = base.effect := by
  refine ⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩

/-! ## §5 Backward-demand accumulation operators (annex-e §AIMS §4 TF-11)

    `seq_add` over Consumption and Cardinality — additive (QTT `+`) combination
    of demand from multiple instructions, NOT lattice `max`. -/

/-- TF-11 Consumption `seq_add` matrix (annex-e §AIMS §4 TF-11):
      Dead + X = X, Linear + Linear = Unrestricted, Linear + Affine = Unrestricted,
      Affine + Affine = Unrestricted, X + Unrestricted = Unrestricted. -/
def Consumption.seqAdd : Consumption → Consumption → Consumption
  | .Dead,         x            => x
  | x,             .Dead        => x
  | .Unrestricted, _            => .Unrestricted
  | _,             .Unrestricted => .Unrestricted
  | .Linear,       .Linear      => .Unrestricted
  | .Linear,       .Affine      => .Unrestricted
  | .Affine,       .Linear      => .Unrestricted
  | .Affine,       .Affine      => .Unrestricted

/-- TF-11 Cardinality `seq_add` matrix (annex-e §AIMS §1.3 + §4 TF-11):
      Absent + X = X, Once + Once = Many, Once + Many = Many, Many + X = Many. -/
def Cardinality.seqAdd : Cardinality → Cardinality → Cardinality
  | .Absent, x       => x
  | x,       .Absent => x
  | .Many,   _       => .Many
  | _,       .Many   => .Many
  | .One,    .One    => .Many

/-! ## §6 TF-11 backward-demand `seq_add` soundness (annex-e §AIMS §4 TF-11)

    The Consumption 4×4 + Cardinality 3×3 matrices match the §4 TF-11 statement,
    are commutative, and are monotone in each argument (L-6 layer (b) for the
    backward-demand accumulation). -/

/-! ### TF-11 part (b) — Consumption seq_add matrix rows -/

theorem TF11_seq_add_consumption_dead_left (x : Consumption) :
    Consumption.seqAdd .Dead x = x := by cases x <;> rfl

theorem TF11_seq_add_consumption_dead_right (x : Consumption) :
    Consumption.seqAdd x .Dead = x := by cases x <;> rfl

theorem TF11_seq_add_consumption_linear_linear :
    Consumption.seqAdd .Linear .Linear = .Unrestricted := rfl

theorem TF11_seq_add_consumption_linear_affine :
    Consumption.seqAdd .Linear .Affine = .Unrestricted := rfl

theorem TF11_seq_add_consumption_affine_affine :
    Consumption.seqAdd .Affine .Affine = .Unrestricted := rfl

theorem TF11_seq_add_consumption_unrestricted_absorb (x : Consumption) :
    Consumption.seqAdd .Unrestricted x = .Unrestricted ∧
    Consumption.seqAdd x .Unrestricted = .Unrestricted := by
  cases x <;> exact ⟨rfl, rfl⟩

/-- TF-11 part (b): `seq_add` over Consumption is commutative (the matrix is
    symmetric per §1.2). -/
theorem TF11_seq_add_consumption_comm (a b : Consumption) :
    Consumption.seqAdd a b = Consumption.seqAdd b a := by
  cases a <;> cases b <;> rfl

/-- TF-11 part (b): `seq_add` over Consumption is associative. Sequential
    demand may therefore be accumulated independently of fold grouping. -/
theorem TF11_seq_add_consumption_assoc (a b c : Consumption) :
    Consumption.seqAdd (Consumption.seqAdd a b) c =
      Consumption.seqAdd a (Consumption.seqAdd b c) := by
  cases a <;> cases b <;> cases c <;> rfl

/-! ### TF-11 part (c) — Cardinality seq_add matrix rows -/

theorem TF11_seq_add_cardinality_absent_left (x : Cardinality) :
    Cardinality.seqAdd .Absent x = x := by cases x <;> rfl

theorem TF11_seq_add_cardinality_absent_right (x : Cardinality) :
    Cardinality.seqAdd x .Absent = x := by cases x <;> rfl

theorem TF11_seq_add_cardinality_once_once :
    Cardinality.seqAdd .One .One = .Many := rfl

theorem TF11_seq_add_cardinality_once_many :
    Cardinality.seqAdd .One .Many = .Many := rfl

theorem TF11_seq_add_cardinality_many_absorb (x : Cardinality) :
    Cardinality.seqAdd .Many x = .Many ∧ Cardinality.seqAdd x .Many = .Many := by
  cases x <;> exact ⟨rfl, rfl⟩

/-- TF-11 part (c): `seq_add` over Cardinality is commutative. -/
theorem TF11_seq_add_cardinality_comm (a b : Cardinality) :
    Cardinality.seqAdd a b = Cardinality.seqAdd b a := by
  cases a <;> cases b <;> rfl

/-- TF-11 part (c): `seq_add` over Cardinality is associative. -/
theorem TF11_seq_add_cardinality_assoc (a b c : Cardinality) :
    Cardinality.seqAdd (Cardinality.seqAdd a b) c =
      Cardinality.seqAdd a (Cardinality.seqAdd b c) := by
  cases a <;> cases b <;> cases c <;> rfl

/-! ### TF-11 part (e) -- raw demand accumulation before CN-1 observation

    Cardinality and Consumption are independent evidence while the backward
    walk accumulates demand. `RawDemand.seqAdd` never canonicalizes either
    dimension. `RawDemand.observe` applies the existing product
    canonicalization exactly once at the observation boundary.

    Delaying observation is load-bearing. A dead projection contributes
    `(Absent, Affine)`: it observes as `(Absent, Dead)` when it is the only
    demand, but its Affine evidence must survive long enough to compose with a
    separate `(Once, Linear)` demand, yielding `(Once, Unrestricted)`. -/

/-- Backend-neutral, pre-canonicalization demand evidence. -/
structure RawDemand where
  cardinality : Cardinality
  consumption : Consumption
deriving Repr, DecidableEq

namespace RawDemand

/-- Identity demand for a block-entry backward fold. -/
def zero : RawDemand :=
  { cardinality := .Absent, consumption := .Dead }

/-- Independent componentwise sequential composition. Canonicalization is not
    part of this algebra. -/
def seqAdd (left right : RawDemand) : RawDemand :=
  { cardinality := Cardinality.seqAdd left.cardinality right.cardinality
  , consumption := Consumption.seqAdd left.consumption right.consumption }

/-- Extract the two demand dimensions from a product state without observing
    or canonicalizing them. -/
def ofState (state : AimsState) : RawDemand :=
  { cardinality := state.cardinality, consumption := state.consumption }

/-- Embed raw demand in a neutral product state so the single existing
    `canonicalize` definition remains the observation authority. -/
def toObservationState (demand : RawDemand) : AimsState :=
  { access := .Owned
  , consumption := demand.consumption
  , cardinality := demand.cardinality
  , uniqueness := .Unique
  , locality := .BlockLocal
  , shape := .NonReusable
  , effect := {} }

/-- Observe accumulated demand once through the canonical product rules. The
    neutral dimensions make CN-1 the only rule able to change this projection. -/
def observe (demand : RawDemand) : RawDemand :=
  ofState (canonicalize (toObservationState demand))

/-- Right-associated raw sum, used as the declarative fold. -/
def sum : List RawDemand -> RawDemand
  | [] => zero
  | demand :: demands => seqAdd demand (sum demands)

theorem seqAdd_left_zero (demand : RawDemand) :
    seqAdd zero demand = demand := by
  cases demand <;> rfl

theorem seqAdd_right_zero (demand : RawDemand) :
    seqAdd demand zero = demand := by
  cases demand
  simp [seqAdd, zero, TF11_seq_add_cardinality_absent_right,
    TF11_seq_add_consumption_dead_right]

theorem seqAdd_comm (left right : RawDemand) :
    seqAdd left right = seqAdd right left := by
  cases left
  cases right
  simp [seqAdd, TF11_seq_add_cardinality_comm,
    TF11_seq_add_consumption_comm]

theorem seqAdd_assoc (first second third : RawDemand) :
    seqAdd (seqAdd first second) third =
      seqAdd first (seqAdd second third) := by
  cases first
  cases second
  cases third
  simp [seqAdd, TF11_seq_add_cardinality_assoc,
    TF11_seq_add_consumption_assoc]

/-- Imperative left-fold accumulation equals the declarative raw sum for every
    initial demand; observation is absent from both sides. -/
theorem foldl_seqAdd_eq (initial : RawDemand) (demands : List RawDemand) :
    demands.foldl seqAdd initial = seqAdd initial (sum demands) := by
  induction demands generalizing initial with
  | nil =>
      exact (seqAdd_right_zero initial).symm
  | cons demand demands ih =>
      rw [List.foldl_cons, ih]
      exact seqAdd_assoc initial demand (sum demands)

theorem foldl_zero_eq_sum (demands : List RawDemand) :
    demands.foldl seqAdd zero = sum demands := by
  rw [foldl_seqAdd_eq, seqAdd_left_zero]

/-- Raw demand sum is invariant under every permutation before observation. -/
theorem sum_perm {left right : List RawDemand} (permutation : left.Perm right) :
    sum left = sum right := by
  induction permutation with
  | nil => rfl
  | cons demand permutation ih =>
      simp only [sum]
      rw [ih]
  | swap first second demands =>
      simp only [sum]
      calc
        seqAdd second (seqAdd first (sum demands)) =
            seqAdd (seqAdd second first) (sum demands) :=
          (seqAdd_assoc second first (sum demands)).symm
        _ = seqAdd (seqAdd first second) (sum demands) := by
          rw [seqAdd_comm second first]
        _ = seqAdd first (seqAdd second (sum demands)) :=
          seqAdd_assoc first second (sum demands)
  | trans _ _ leftToMiddle middleToRight =>
      exact leftToMiddle.trans middleToRight

/-- Observing after a permutation-invariant raw fold preserves that equality. -/
theorem observe_sum_perm {left right : List RawDemand}
    (permutation : left.Perm right) :
    observe (sum left) = observe (sum right) := by
  exact congrArg observe (sum_perm permutation)

/-- IA-5 Project's fixed Affine contribution, kept independent of destination
    cardinality until the block fold is observed. -/
def projectContribution (destination : AimsState) : RawDemand :=
  { cardinality := destination.cardinality, consumption := .Affine }

def deadProjection : RawDemand :=
  { cardinality := .Absent, consumption := .Affine }

/-- A scalar Project has no destination `AimsState`. Its source contribution is
    therefore selected only by whether the scalar copy-out is live: one live
    copy-out contributes one Affine occurrence, while a dead copy-out retains
    the ordinary dead-projection evidence until observation. -/
def scalarProjectContribution (live : Bool) : RawDemand :=
  if live then
    { cardinality := .One, consumption := .Affine }
  else
    deadProjection

/-- A live scalar Project contributes exactly one Affine source occurrence. -/
theorem scalarProjectContribution_live_eq_once_affine :
    scalarProjectContribution true =
      { cardinality := .One, consumption := .Affine } := by
  rfl

/-- A live scalar copy-out agrees with ordinary Project propagation whenever
    the managed destination has exactly one accumulated use. -/
theorem scalarProjectContribution_live_eq_projectContribution
    (destination : AimsState) (once : destination.cardinality = .One) :
    scalarProjectContribution true = projectContribution destination := by
  simp [scalarProjectContribution, projectContribution, once]

/-- A dead scalar Project is the same pending Affine evidence as every other
    dead projection; CN-1 may erase it only at the observation boundary. -/
theorem scalarProjectContribution_dead_eq_deadProjection :
    scalarProjectContribution false = deadProjection := by
  rfl

/-- Copy-out caps a live scalar Project at one source occurrence. Replacing any
    downstream scalar demand with the one-copy managed witness yields the same
    contribution, so downstream scalar reuse cannot promote it to `Many`. -/
theorem scalarProjectContribution_live_copy_out_cap
    (downstream : AimsState) :
    scalarProjectContribution true =
      projectContribution { downstream with cardinality := .One } := by
  rfl

def directLinearUse : RawDemand :=
  { cardinality := .One, consumption := .Linear }

/-- The eager, incorrect timing loses the pending Affine obligation. -/
theorem eager_observation_yields_once_linear :
    observe (seqAdd (observe deadProjection) directLinearUse) =
      { cardinality := .One, consumption := .Linear } := by
  decide

/-- Concrete non-commutation witness: observing CN-1 between contributions
    erases the Affine evidence and differs from observing once after the sum. -/
theorem eager_CN1_observation_does_not_commute_with_seqAdd :
    observe (seqAdd (observe deadProjection) directLinearUse) !=
      observe (seqAdd deadProjection directLinearUse) := by
  decide

/-- The correct block timing: fold raw evidence, then observe once. -/
theorem raw_fold_then_observe_once_unrestricted :
    observe (sum [deadProjection, directLinearUse]) =
      { cardinality := .One, consumption := .Unrestricted } := by
  decide

/-- A lone dead projection retains no live demand after the one observation. -/
theorem lone_dead_projection_observes_dead :
    observe (sum [deadProjection]) = zero := by
  decide

/-- TF-11 timing theorem: the raw algebra is associative and commutative,
    left- and right-associated folds agree, permutations are irrelevant before
    observation, and the concrete CN-1 timing witnesses have the required
    outcomes. -/
theorem TF11_raw_demand_timing_sound :
    (forall first second third : RawDemand,
      seqAdd (seqAdd first second) third =
        seqAdd first (seqAdd second third)) /\
    (forall left right : RawDemand, seqAdd left right = seqAdd right left) /\
    (forall (initial : RawDemand) (demands : List RawDemand),
      demands.foldl seqAdd initial = seqAdd initial (sum demands)) /\
    (forall (left right : List RawDemand), left.Perm right ->
      sum left = sum right) /\
    observe (seqAdd (observe deadProjection) directLinearUse) =
      { cardinality := .One, consumption := .Linear } /\
    observe (seqAdd (observe deadProjection) directLinearUse) !=
      observe (seqAdd deadProjection directLinearUse) /\
    observe (sum [deadProjection, directLinearUse]) =
      { cardinality := .One, consumption := .Unrestricted } /\
    observe (sum [deadProjection]) = zero := by
  refine ⟨seqAdd_assoc, seqAdd_comm, foldl_seqAdd_eq, ?_,
    eager_observation_yields_once_linear,
    eager_CN1_observation_does_not_commute_with_seqAdd,
    raw_fold_then_observe_once_unrestricted,
    lone_dead_projection_observes_dead⟩
  intro left right permutation
  exact sum_perm permutation

end RawDemand

/-! ### TF-11 part (d) — `seq_add` monotonicity (L-6 layer (b))

    `a1 ≤ a2 ⟹ seq_add(a1, c) ≤ seq_add(a2, c)` for fixed `c`, over the
    Consumption + Cardinality chain orders (rank comparison). The chain order is
    the rank `≤` (per §1.2 / §1.3). -/

theorem TF11_seq_add_consumption_monotone (a1 a2 c : Consumption)
    (h : a1.rank ≤ a2.rank) :
    (Consumption.seqAdd a1 c).rank ≤ (Consumption.seqAdd a2 c).rank := by
  cases a1 <;> cases a2 <;> cases c <;> simp_all [Consumption.seqAdd, Consumption.rank]

theorem TF11_seq_add_cardinality_monotone (a1 a2 c : Cardinality)
    (h : a1.rank ≤ a2.rank) :
    (Cardinality.seqAdd a1 c).rank ≤ (Cardinality.seqAdd a2 c).rank := by
  cases a1 <;> cases a2 <;> cases c <;> simp_all [Cardinality.seqAdd, Cardinality.rank]

/-! ### TF-14 scalar-liveness producer domain

    The sparse scalar-liveness side table represents one Boolean coordinate per
    L-9-excluded scalar SSA value. This carrier is not an `AimsState` dimension:
    `dead < live`, alternative paths join by Boolean OR, and the only output
    consumed by TF-14 is the final dead/live choice passed to
    `RawDemand.scalarProjectContribution`.

    A reverse transfer first joins destination liveness into its source and
    then kills the destination definition. A Jump edge applies the same rule
    positionally from each live successor parameter to its predecessor
    argument, then removes the successor parameter. -/

inductive ScalarLiveness
  | dead
  | live
deriving Repr, DecidableEq

namespace ScalarLiveness

def toBool : ScalarLiveness → Bool
  | .dead => false
  | .live => true

def ofBool : Bool → ScalarLiveness
  | false => .dead
  | true => .live

def rank : ScalarLiveness → Nat
  | .dead => 0
  | .live => 1

def below (left right : ScalarLiveness) : Prop :=
  rank left ≤ rank right

def strictBelow (left right : ScalarLiveness) : Prop :=
  below left right ∧ left ≠ right

/-- Alternative-path join for one sparse-set membership coordinate. -/
def join : ScalarLiveness → ScalarLiveness → ScalarLiveness
  | .live, _ => .live
  | _, .live => .live
  | .dead, .dead => .dead

/-- Removing an SSA definition from the predecessor-visible environment. -/
def kill (_ : ScalarLiveness) : ScalarLiveness := .dead

structure ReverseStep where
  source : ScalarLiveness
  destination : ScalarLiveness
deriving Repr, DecidableEq

/-- Transfer destination liveness to its source before killing the definition. -/
def reverseTransfer (source destination : ScalarLiveness) : ReverseStep :=
  { source := join source destination
  , destination := kill destination }

structure JumpEdgeState where
  argument : ScalarLiveness
  parameter : ScalarLiveness
deriving Repr, DecidableEq

/-- One positional pair from `Jump(args)` to the target block's parameters. -/
def jumpParamSubstitution
    (argument parameter : ScalarLiveness) : JumpEdgeState :=
  { argument := join argument parameter
  , parameter := kill parameter }

def Monotone (function : ScalarLiveness → ScalarLiveness) : Prop :=
  ∀ {left right}, below left right → below (function left) (function right)

/-- Iteration of one producer coordinate from the sparse-set bottom. -/
def iterateFromDead
    (function : ScalarLiveness → ScalarLiveness) : Nat → ScalarLiveness
  | 0 => .dead
  | step + 1 => function (iterateFromDead function step)

/-- The sole seam from scalar liveness into TF-14 raw demand. -/
def projectContribution (liveness : ScalarLiveness) : RawDemand :=
  RawDemand.scalarProjectContribution (toBool liveness)

theorem toBool_ofBool (value : Bool) :
    toBool (ofBool value) = value := by
  cases value <;> rfl

theorem ofBool_toBool (liveness : ScalarLiveness) :
    ofBool (toBool liveness) = liveness := by
  cases liveness <;> rfl

/-- Sparse-set union is exactly Boolean OR on each coordinate. -/
theorem join_is_bool_or (left right : ScalarLiveness) :
    toBool (join left right) = (toBool left || toBool right) := by
  cases left <;> cases right <;> rfl

theorem join_comm (left right : ScalarLiveness) :
    join left right = join right left := by
  cases left <;> cases right <;> rfl

theorem join_assoc (first second third : ScalarLiveness) :
    join (join first second) third = join first (join second third) := by
  cases first <;> cases second <;> cases third <;> rfl

theorem join_idem (liveness : ScalarLiveness) :
    join liveness liveness = liveness := by
  cases liveness <;> rfl

theorem join_dead_left (liveness : ScalarLiveness) :
    join .dead liveness = liveness := by
  cases liveness <;> rfl

theorem below_iff_join_eq_right {left right : ScalarLiveness} :
    below left right ↔ join left right = right := by
  cases left <;> cases right <;> simp [below, rank, join]

theorem dead_below (liveness : ScalarLiveness) :
    below .dead liveness := by
  cases liveness <;> simp [below, rank]

theorem below_live (liveness : ScalarLiveness) :
    below liveness .live := by
  cases liveness <;> simp [below, rank]

theorem dead_strictly_below_live :
    strictBelow .dead .live := by
  simp [strictBelow, below, rank]

theorem strictBelow_iff_dead_live {left right : ScalarLiveness} :
    strictBelow left right ↔ left = .dead ∧ right = .live := by
  cases left <;> cases right <;> simp [strictBelow, below, rank]

/-- The carrier has height one: two consecutive strict rises are impossible. -/
theorem no_two_strict_rises
    (first second third : ScalarLiveness)
    (firstRise : strictBelow first second)
    (secondRise : strictBelow second third) : False := by
  cases first <;> cases second <;> cases third <;>
    simp_all [strictBelow, below, rank]

theorem join_monotone
    {left1 left2 right1 right2 : ScalarLiveness}
    (leftBelow : below left1 left2)
    (rightBelow : below right1 right2) :
    below (join left1 right1) (join left2 right2) := by
  cases left1 <;> cases left2 <;> cases right1 <;> cases right2 <;>
    simp_all [below, rank, join]

theorem reverseTransfer_dead_destination (source : ScalarLiveness) :
    reverseTransfer source .dead =
      { source := source, destination := .dead } := by
  cases source <;> rfl

theorem reverseTransfer_live_destination (source : ScalarLiveness) :
    reverseTransfer source .live =
      { source := .live, destination := .dead } := by
  cases source <;> rfl

theorem reverseTransfer_monotone
    {source1 source2 destination1 destination2 : ScalarLiveness}
    (sourceBelow : below source1 source2)
    (destinationBelow : below destination1 destination2) :
    below (reverseTransfer source1 destination1).source
        (reverseTransfer source2 destination2).source ∧
      below (reverseTransfer source1 destination1).destination
        (reverseTransfer source2 destination2).destination := by
  constructor
  · exact join_monotone sourceBelow destinationBelow
  · simp [reverseTransfer, kill, below, rank]

/-- Jump substitution is the same transfer-before-kill rule under edge names. -/
theorem jumpParamSubstitution_matches_reverseTransfer
    (argument parameter : ScalarLiveness) :
    (jumpParamSubstitution argument parameter).argument =
        (reverseTransfer argument parameter).source ∧
      (jumpParamSubstitution argument parameter).parameter =
        (reverseTransfer argument parameter).destination := by
  exact ⟨rfl, rfl⟩

theorem jumpParamSubstitution_dead_parameter (argument : ScalarLiveness) :
    jumpParamSubstitution argument .dead =
      { argument := argument, parameter := .dead } := by
  cases argument <;> rfl

theorem jumpParamSubstitution_live_parameter (argument : ScalarLiveness) :
    jumpParamSubstitution argument .live =
      { argument := .live, parameter := .dead } := by
  cases argument <;> rfl

theorem jumpParamSubstitution_monotone
    {argument1 argument2 parameter1 parameter2 : ScalarLiveness}
    (argumentBelow : below argument1 argument2)
    (parameterBelow : below parameter1 parameter2) :
    below (jumpParamSubstitution argument1 parameter1).argument
        (jumpParamSubstitution argument2 parameter2).argument ∧
      below (jumpParamSubstitution argument1 parameter1).parameter
        (jumpParamSubstitution argument2 parameter2).parameter := by
  constructor
  · exact join_monotone argumentBelow parameterBelow
  · simp [jumpParamSubstitution, kill, below, rank]

/-- Every monotone producer, seeded at dead, reaches a fixed point after its
    only possible strict rise. -/
theorem monotone_from_dead_reaches_fixed_point
    (function : ScalarLiveness → ScalarLiveness)
    (monotone : Monotone function) :
    function (function .dead) = function .dead := by
  have ordered : below (function .dead) (function .live) :=
    monotone (dead_below .live)
  cases deadImage : function .dead <;>
    cases liveImage : function .live <;>
    simp_all [below, rank]

/-- Every positive iteration is the same fixed point. This is the pointwise
    convergence theorem for the sparse scalar-liveness set. -/
theorem height_one_convergence
    (function : ScalarLiveness → ScalarLiveness)
    (monotone : Monotone function)
    (step : Nat) :
    iterateFromDead function (Nat.succ step) = function .dead := by
  have fixed := monotone_from_dead_reaches_fixed_point function monotone
  induction step with
  | zero => rfl
  | succ step inductionHypothesis =>
      change function (iterateFromDead function (Nat.succ step)) = function .dead
      rw [inductionHypothesis]
      exact fixed

theorem projectContribution_exact_seam (liveness : ScalarLiveness) :
    projectContribution liveness =
      RawDemand.scalarProjectContribution (toBool liveness) := by
  rfl

theorem projectContribution_dead :
    projectContribution .dead = RawDemand.deadProjection := by
  rfl

theorem projectContribution_live :
    projectContribution .live =
      { cardinality := .One, consumption := .Affine } := by
  rfl

/-- Complete laws for the pointwise producer domain consumed by TF-14. -/
structure ProducerDomainLaws : Prop where
  joinIsBoolOr : ∀ left right, toBool (join left right) =
    (toBool left || toBool right)
  joinCommutative : ∀ left right, join left right = join right left
  joinAssociative : ∀ first second third,
    join (join first second) third = join first (join second third)
  joinIdempotent : ∀ liveness, join liveness liveness = liveness
  deadIsBottom : ∀ liveness, join .dead liveness = liveness
  orderInducedByJoin : ∀ {left right},
    below left right ↔ join left right = right
  deadBelowLive : strictBelow .dead .live
  heightOne : ∀ first second third,
    strictBelow first second → strictBelow second third → False
  reverseDead : ∀ source, reverseTransfer source .dead =
    { source := source, destination := .dead }
  reverseLive : ∀ source, reverseTransfer source .live =
    { source := .live, destination := .dead }
  reverseMonotone : ∀ {source1 source2 destination1 destination2},
    below source1 source2 → below destination1 destination2 →
      below (reverseTransfer source1 destination1).source
          (reverseTransfer source2 destination2).source ∧
        below (reverseTransfer source1 destination1).destination
          (reverseTransfer source2 destination2).destination
  jumpMatchesReverse : ∀ argument parameter,
    (jumpParamSubstitution argument parameter).argument =
        (reverseTransfer argument parameter).source ∧
      (jumpParamSubstitution argument parameter).parameter =
        (reverseTransfer argument parameter).destination
  jumpDead : ∀ argument, jumpParamSubstitution argument .dead =
    { argument := argument, parameter := .dead }
  jumpLive : ∀ argument, jumpParamSubstitution argument .live =
    { argument := .live, parameter := .dead }
  jumpMonotone : ∀ {argument1 argument2 parameter1 parameter2},
    below argument1 argument2 → below parameter1 parameter2 →
      below (jumpParamSubstitution argument1 parameter1).argument
          (jumpParamSubstitution argument2 parameter2).argument ∧
        below (jumpParamSubstitution argument1 parameter1).parameter
          (jumpParamSubstitution argument2 parameter2).parameter
  reachesFixedPoint : ∀ function, Monotone function →
    function (function .dead) = function .dead
  converges : ∀ function, Monotone function → ∀ step,
    iterateFromDead function (Nat.succ step) = function .dead
  exactProjectSeam : ∀ liveness,
    projectContribution liveness =
      RawDemand.scalarProjectContribution (toBool liveness)

theorem producer_domain_sound : ProducerDomainLaws where
  joinIsBoolOr := join_is_bool_or
  joinCommutative := join_comm
  joinAssociative := join_assoc
  joinIdempotent := join_idem
  deadIsBottom := join_dead_left
  orderInducedByJoin := below_iff_join_eq_right
  deadBelowLive := dead_strictly_below_live
  heightOne := no_two_strict_rises
  reverseDead := reverseTransfer_dead_destination
  reverseLive := reverseTransfer_live_destination
  reverseMonotone := reverseTransfer_monotone
  jumpMatchesReverse := jumpParamSubstitution_matches_reverseTransfer
  jumpDead := jumpParamSubstitution_dead_parameter
  jumpLive := jumpParamSubstitution_live_parameter
  jumpMonotone := jumpParamSubstitution_monotone
  reachesFixedPoint := monotone_from_dead_reaches_fixed_point
  converges := height_one_convergence
  exactProjectSeam := projectContribution_exact_seam

end ScalarLiveness

/-! ## §7 TF-14 Project backward-demand propagation (annex-e §AIMS §4 TF-14)

    `propagate_project_source_demand(src, dst)` mutates the source:
      locality    := max(src.locality, dst.locality)
      cardinality := seq_add(src.cardinality, dst.cardinality)   -- QTT-consistent
      consumption := seq_add(src.consumption, Affine)
    No Access promotion; Uniqueness / Shape / Effect unchanged. -/

def propagateProjectSourceDemand (src dst : AimsState) : AimsState :=
  { src with
    locality := src.locality.join dst.locality
  , cardinality := Cardinality.seqAdd src.cardinality dst.cardinality
  , consumption := Consumption.seqAdd src.consumption .Affine }

/-- TF-14 contributes its two demand dimensions through the raw algebra. The
    full product state is not canonicalized inside the Project transfer. -/
theorem TF14_uses_raw_demand_composition (src dst : AimsState) :
    RawDemand.ofState (propagateProjectSourceDemand src dst) =
      RawDemand.seqAdd (RawDemand.ofState src)
        (RawDemand.projectContribution dst) := by
  rfl

/-- TF-14 part (a): the three mutations exactly per §4 TF-14, and the
    no-propagation dimensions (access / uniqueness / shape / effect) unchanged. -/
theorem TF14_propagation_spec (src dst : AimsState) :
    (propagateProjectSourceDemand src dst).locality
      = src.locality.join dst.locality ∧
    (propagateProjectSourceDemand src dst).cardinality
      = Cardinality.seqAdd src.cardinality dst.cardinality ∧
    (propagateProjectSourceDemand src dst).consumption
      = Consumption.seqAdd src.consumption .Affine ∧
    (propagateProjectSourceDemand src dst).access = src.access ∧
    (propagateProjectSourceDemand src dst).uniqueness = src.uniqueness ∧
    (propagateProjectSourceDemand src dst).shape = src.shape ∧
    (propagateProjectSourceDemand src dst).effect = src.effect := by
  refine ⟨rfl, rfl, rfl, rfl, rfl, rfl, rfl⟩

/-- TF-14's scalar-destination case. L-9 excludes a scalar destination from
    `AimsState`, so liveness supplies its complete source-demand contribution:
    dead preserves the existing dead-projection evidence, live agrees with an
    ordinary Once destination, and the live copy-out remains capped at Once
    for every downstream scalar demand. -/
theorem TF14_scalar_project_copy_out_sound :
    RawDemand.scalarProjectContribution true =
      { cardinality := .One, consumption := .Affine } /\
    RawDemand.scalarProjectContribution false = RawDemand.deadProjection /\
    (forall destination : AimsState, destination.cardinality = .One ->
      RawDemand.scalarProjectContribution true =
        RawDemand.projectContribution destination) /\
    (forall downstream : AimsState,
      RawDemand.scalarProjectContribution true =
        RawDemand.projectContribution
          { downstream with cardinality := .One }) := by
  exact ⟨RawDemand.scalarProjectContribution_live_eq_once_affine,
    RawDemand.scalarProjectContribution_dead_eq_deadProjection,
    RawDemand.scalarProjectContribution_live_eq_projectContribution,
    RawDemand.scalarProjectContribution_live_copy_out_cap⟩

/-- Complete TF-14 theorem surface: the scalar-liveness producer, managed
    Project propagation, and the L-9-excluded scalar copy-out rule are
    discharged together. -/
theorem TF14_project_demand_sound (src dst : AimsState) :
    ScalarLiveness.ProducerDomainLaws /\
    (((propagateProjectSourceDemand src dst).locality
        = src.locality.join dst.locality /\
      (propagateProjectSourceDemand src dst).cardinality
        = Cardinality.seqAdd src.cardinality dst.cardinality /\
      (propagateProjectSourceDemand src dst).consumption
        = Consumption.seqAdd src.consumption .Affine /\
      (propagateProjectSourceDemand src dst).access = src.access /\
      (propagateProjectSourceDemand src dst).uniqueness = src.uniqueness /\
      (propagateProjectSourceDemand src dst).shape = src.shape /\
      (propagateProjectSourceDemand src dst).effect = src.effect) /\
    (RawDemand.scalarProjectContribution true =
        { cardinality := .One, consumption := .Affine } /\
      RawDemand.scalarProjectContribution false = RawDemand.deadProjection /\
      (forall destination : AimsState, destination.cardinality = .One ->
        RawDemand.scalarProjectContribution true =
          RawDemand.projectContribution destination) /\
      (forall downstream : AimsState,
        RawDemand.scalarProjectContribution true =
          RawDemand.projectContribution
            { downstream with cardinality := .One }))) := by
  exact ⟨ScalarLiveness.producer_domain_sound,
    TF14_propagation_spec src dst,
    TF14_scalar_project_copy_out_sound⟩

/-- TF-14 part (c): NO Access promotion — `out.access = src.access` regardless of
    `src.access` or `dst.access` (the negative witness in TF-14.proof part (c)). -/
theorem TF14_no_access_promotion (src dst : AimsState) :
    (propagateProjectSourceDemand src dst).access = src.access := rfl

/-- TF-14 part (b): QTT-consistency — `src.cardinality = Once` and
    `dst.cardinality = Once` accumulate to `Many` via `seq_add` (the `max`
    alternative would undercount to `Once`). -/
theorem TF14_qtt_witness (src dst : AimsState)
    (hs : src.cardinality = .One) (hd : dst.cardinality = .One) :
    (propagateProjectSourceDemand src dst).cardinality = .Many := by
  simp only [propagateProjectSourceDemand, hs, hd, Cardinality.seqAdd]

/-- TF-14 part (d): monotone in `dst.locality` for fixed `src` — the locality
    mutation `max(src.locality, dst.locality)` is monotone (chain `max`). -/
theorem TF14_locality_monotone (src : AimsState) (l1 l2 : Locality)
    (h : l1.rank ≤ l2.rank) :
    (src.locality.join l1).rank ≤ (src.locality.join l2).rank := by
  cases hsrc : src.locality <;> cases l1 <;> cases l2 <;>
    simp_all [Locality.join, Locality.rank]

/-- TF-14 part (d): monotone in `dst.cardinality` for fixed `src` — the
    cardinality mutation `seq_add(src.cardinality, dst.cardinality)` is monotone. -/
theorem TF14_cardinality_monotone (src : AimsState) (k1 k2 : Cardinality)
    (h : k1.rank ≤ k2.rank) :
    (Cardinality.seqAdd src.cardinality k1).rank
      ≤ (Cardinality.seqAdd src.cardinality k2).rank := by
  have := TF11_seq_add_cardinality_monotone k1 k2 src.cardinality h
  rwa [TF11_seq_add_cardinality_comm src.cardinality k1,
       TF11_seq_add_cardinality_comm src.cardinality k2]

/-! ## §7a IA-5 step (1): backward transfer of forward definitions

    IA-5 step (1) is the instruction-sensitive bridge between accumulated
    destination demand and source-side state. It is backend-neutral: the
    carrier below contains only AIMS lattice states and logical instruction
    shapes. Operational TF-11 demand is intentionally excluded and remains
    step (2).

    `primary` is the alias source, projection source, first Select arm,
    aggregate argument, or Set value. `secondary` is the second Select arm or
    Set base. Aggregate transfer is per argument; applying the same case to
    every argument gives the n-ary rule. -/

/-- Whether Select's two syntactic arms name distinct variables or the same
    variable. The same-variable case must compose both arm transfers
    sequentially rather than collapse them with an alternative-path join. -/
inductive IA5SelectOperands
  | distinct
  | same
deriving Repr, DecidableEq

/-- Aggregate-building instruction families governed by one IA-5 rule. -/
inductive IA5AggregateKind
  | construct
  | reuse
  | collectionReuse
deriving Repr, DecidableEq

/-- Non-aliasing definitions whose IA-5 step (1) image is the identity.
    Their separate TF-11, TF-12, or TF-13 demand remains step (2). -/
inductive IA5NonAliasKind
  | apply
  | applyIndirect
  | invoke
  | invokeIndirect
  | partialApply
  | rcInc
  | rcDec
  | isShared
  | reset
deriving Repr, DecidableEq

/-- Exhaustive logical instruction shapes for IA-5 step (1). -/
inductive IA5Step1Instr
  | letVar
  | project
  | select (operands : IA5SelectOperands)
  | aggregate (kind : IA5AggregateKind)
  | set
  | setTag
  | nonAlias (kind : IA5NonAliasKind)
deriving Repr, DecidableEq

/-- The two state slots an IA-5 instruction shape can update. -/
structure IA5Step1Operands where
  primary : AimsState
  secondary : AimsState
deriving Repr, DecidableEq

/-- Transparent-alias demand transfer: cardinality and consumption compose
    sequentially; locality widens; all other dimensions stay on the source. -/
def ia5FullAliasDemand (source destination : AimsState) : AimsState :=
  { source with
    consumption := Consumption.seqAdd source.consumption destination.consumption
  , cardinality := Cardinality.seqAdd source.cardinality destination.cardinality
  , locality := source.locality.join destination.locality }

/-- IA-5 transparent/conditional aliases compose their demand dimensions in
    the same raw algebra; locality remains the independent max component. -/
theorem IA5_full_alias_uses_raw_demand_composition
    (source destination : AimsState) :
    RawDemand.ofState (ia5FullAliasDemand source destination) =
      RawDemand.seqAdd (RawDemand.ofState source)
        (RawDemand.ofState destination) := by
  rfl

/-- A block walk may use its imperative left fold without observing between
    instruction contributions. Permuting contributions cannot change the raw
    block-entry demand. -/
theorem IA5_block_walk_raw_fold_permutation
    {left right : List RawDemand} (permutation : left.Perm right) :
    left.foldl RawDemand.seqAdd RawDemand.zero =
      right.foldl RawDemand.seqAdd RawDemand.zero := by
  rw [RawDemand.foldl_zero_eq_sum, RawDemand.foldl_zero_eq_sum]
  exact RawDemand.sum_perm permutation

/-- An aggregate takes ownership of each argument and inherits the aggregate
    destination's locality. Destination multiplicity is not transferred. -/
def ia5AggregateArgument (argument destination : AimsState) : AimsState :=
  { argument with
    access := .Owned
  , locality := argument.locality.join destination.locality }

/-- Set takes ownership of its value and widens that value to the base's
    locality. The base itself receives only its direct TF-11 demand in step (2). -/
def ia5SetValue (value base : AimsState) : AimsState :=
  { value with
    access := .Owned
  , locality := value.locality.join base.locality }

/-- IA-5 step (1), independent of any physical executor. -/
def ia5Step1
    (instruction : IA5Step1Instr)
    (operands : IA5Step1Operands)
    (destination : AimsState) : IA5Step1Operands :=
  match instruction with
  | .letVar =>
      { operands with primary := ia5FullAliasDemand operands.primary destination }
  | .project =>
      { operands with
        primary := propagateProjectSourceDemand operands.primary destination }
  | .select .distinct =>
      { primary := ia5FullAliasDemand operands.primary destination
      , secondary := ia5FullAliasDemand operands.secondary destination }
  | .select .same =>
      { operands with
        primary := ia5FullAliasDemand
          (ia5FullAliasDemand operands.primary destination) destination }
  | .aggregate _ =>
      { operands with
        primary := ia5AggregateArgument operands.primary destination }
  | .set =>
      { operands with primary := ia5SetValue operands.primary operands.secondary }
  | .setTag | .nonAlias _ => operands

/-- Independent semantic relation for transparent and conditional aliases. -/
def ia5FullAliasConforms
    (source destination result : AimsState) : Prop :=
  result.consumption
      = Consumption.seqAdd source.consumption destination.consumption ∧
    result.cardinality
      = Cardinality.seqAdd source.cardinality destination.cardinality ∧
    result.locality = source.locality.join destination.locality ∧
    result.access = source.access ∧
    result.uniqueness = source.uniqueness ∧
    result.shape = source.shape ∧
    result.effect = source.effect

/-- Independent semantic relation for one aggregate argument. The unchanged
    cardinality and consumption clauses reject destination-demand transfer. -/
def ia5AggregateArgumentConforms
    (argument destination result : AimsState) : Prop :=
  result.access = .Owned ∧
    result.locality = argument.locality.join destination.locality ∧
    result.consumption = argument.consumption ∧
    result.cardinality = argument.cardinality ∧
    result.uniqueness = argument.uniqueness ∧
    result.shape = argument.shape ∧
    result.effect = argument.effect

/-- Independent semantic relation for Set's value-side transfer. -/
def ia5SetValueConforms (value base result : AimsState) : Prop :=
  result.access = .Owned ∧
    result.locality = value.locality.join base.locality ∧
    result.consumption = value.consumption ∧
    result.cardinality = value.cardinality ∧
    result.uniqueness = value.uniqueness ∧
    result.shape = value.shape ∧
    result.effect = value.effect

/-- The complete IA-5 step (1) relation. Select with the same variable in both
    arms applies the full destination demand twice to that one source state. -/
def ia5Step1Conforms
    (instruction : IA5Step1Instr)
    (before : IA5Step1Operands)
    (destination : AimsState)
    (after : IA5Step1Operands) : Prop :=
  match instruction with
  | .letVar =>
      ia5FullAliasConforms before.primary destination after.primary ∧
        after.secondary = before.secondary
  | .project =>
      after.primary = propagateProjectSourceDemand before.primary destination ∧
        after.secondary = before.secondary
  | .select .distinct =>
      ia5FullAliasConforms before.primary destination after.primary ∧
        ia5FullAliasConforms before.secondary destination after.secondary
  | .select .same =>
      let once := ia5FullAliasDemand before.primary destination
      ia5FullAliasConforms before.primary destination once ∧
        ia5FullAliasConforms once destination after.primary ∧
        after.secondary = before.secondary
  | .aggregate _ =>
      ia5AggregateArgumentConforms before.primary destination after.primary ∧
        after.secondary = before.secondary
  | .set =>
      ia5SetValueConforms before.primary before.secondary after.primary ∧
        after.secondary = before.secondary
  | .setTag | .nonAlias _ => after = before

/-- IA-5 step (1): the backend-neutral function satisfies every instruction
    case in the independent semantic relation. -/
theorem IA5_step1_sound
    (instruction : IA5Step1Instr)
    (operands : IA5Step1Operands)
    (destination : AimsState) :
    ia5Step1Conforms instruction operands destination
      (ia5Step1 instruction operands destination) := by
  cases instruction with
  | letVar =>
      simp [ia5Step1Conforms, ia5Step1, ia5FullAliasConforms,
        ia5FullAliasDemand]
  | project => simp [ia5Step1Conforms, ia5Step1]
  | select mode =>
      cases mode <;>
        simp [ia5Step1Conforms, ia5Step1, ia5FullAliasConforms,
          ia5FullAliasDemand]
  | aggregate kind =>
      cases kind <;>
        simp [ia5Step1Conforms, ia5Step1, ia5AggregateArgumentConforms,
          ia5AggregateArgument]
  | set =>
      simp [ia5Step1Conforms, ia5Step1, ia5SetValueConforms, ia5SetValue]
  | setTag => simp [ia5Step1Conforms, ia5Step1]
  | nonAlias kind =>
      cases kind <;> simp [ia5Step1Conforms, ia5Step1]

/-- IA-5 composition theorem: the exhaustive instruction relation and the raw
    block-walk timing obligation hold together. This is the kernel-checked
    bridge from instruction transfer to observe-once block accumulation. -/
theorem IA5_step1_and_raw_block_walk_sound :
    (forall (instruction : IA5Step1Instr)
      (operands : IA5Step1Operands) (destination : AimsState),
      ia5Step1Conforms instruction operands destination
        (ia5Step1 instruction operands destination)) /\
    (forall source destination : AimsState,
      RawDemand.ofState (ia5FullAliasDemand source destination) =
        RawDemand.seqAdd (RawDemand.ofState source)
          (RawDemand.ofState destination)) /\
    (forall source destination : AimsState,
      RawDemand.ofState (propagateProjectSourceDemand source destination) =
        RawDemand.seqAdd (RawDemand.ofState source)
          (RawDemand.projectContribution destination)) /\
    (RawDemand.scalarProjectContribution true =
        { cardinality := .One, consumption := .Affine } /\
      RawDemand.scalarProjectContribution false = RawDemand.deadProjection /\
      (forall destination : AimsState, destination.cardinality = .One ->
        RawDemand.scalarProjectContribution true =
          RawDemand.projectContribution destination) /\
      (forall downstream : AimsState,
        RawDemand.scalarProjectContribution true =
          RawDemand.projectContribution
            { downstream with cardinality := .One })) /\
    (forall (left right : List RawDemand), left.Perm right ->
      left.foldl RawDemand.seqAdd RawDemand.zero =
        right.foldl RawDemand.seqAdd RawDemand.zero) := by
  refine ⟨IA5_step1_sound, IA5_full_alias_uses_raw_demand_composition,
    TF14_uses_raw_demand_composition, TF14_scalar_project_copy_out_sound, ?_⟩
  intro left right permutation
  exact IA5_block_walk_raw_fold_permutation permutation

/-- Let Var transfers the full accumulated destination demand to its source. -/
theorem IA5_let_var_transfers_full_demand
    (operands : IA5Step1Operands) (destination : AimsState) :
    ia5FullAliasConforms operands.primary destination
      (ia5Step1 .letVar operands destination).primary := by
  have sound := IA5_step1_sound .letVar operands destination
  exact sound.1

/-- Project step (1) is exactly TF-14 and emits no independent TF-11 demand. -/
theorem IA5_project_composes_TF14
    (operands : IA5Step1Operands) (destination : AimsState) :
    (ia5Step1 .project operands destination).primary
      = propagateProjectSourceDemand operands.primary destination := by
  have sound := IA5_step1_sound .project operands destination
  exact sound.1

/-- Select with distinct variables transfers full demand to both arms. -/
theorem IA5_select_distinct_transfers_both_arms
    (operands : IA5Step1Operands) (destination : AimsState) :
    ia5FullAliasConforms operands.primary destination
        (ia5Step1 (.select .distinct) operands destination).primary ∧
      ia5FullAliasConforms operands.secondary destination
        (ia5Step1 (.select .distinct) operands destination).secondary := by
  simpa [ia5Step1Conforms] using
    IA5_step1_sound (.select .distinct) operands destination

/-- Every aggregate family uses the same per-argument Owned/locality rule and
    does not transfer destination cardinality or consumption. -/
theorem IA5_aggregate_argument_rule
    (kind : IA5AggregateKind)
    (operands : IA5Step1Operands)
    (destination : AimsState) :
    ia5AggregateArgumentConforms operands.primary destination
      (ia5Step1 (.aggregate kind) operands destination).primary := by
  have sound := IA5_step1_sound (.aggregate kind) operands destination
  exact sound.1

/-- Set promotes its value to Owned at the base's locality. -/
theorem IA5_set_value_rule
    (operands : IA5Step1Operands) (destination : AimsState) :
    ia5SetValueConforms operands.primary operands.secondary
      (ia5Step1 .set operands destination).primary := by
  have sound := IA5_step1_sound .set operands destination
  exact sound.1

/-- SetTag and every enumerated non-aliasing definition are step-(1) no-ops. -/
theorem IA5_no_alias_definitions_are_noops
    (kind : IA5NonAliasKind)
    (operands : IA5Step1Operands)
    (destination : AimsState) :
    ia5Step1 .setTag operands destination = operands ∧
      ia5Step1 (.nonAlias kind) operands destination = operands := by
  exact ⟨rfl, rfl⟩

/-! ### IA-5 executable witnesses and negative pins -/

def ia5WitnessSource : AimsState :=
  { access := .Borrowed
  , consumption := .Dead
  , cardinality := .Absent
  , uniqueness := .Unique
  , locality := .BlockLocal
  , shape := .ReusableStruct
  , effect := {} }

def ia5WitnessDemand : AimsState :=
  { access := .Owned
  , consumption := .Linear
  , cardinality := .One
  , uniqueness := .MaybeShared
  , locality := .HeapEscaping
  , shape := .NonReusable
  , effect := {} }

def ia5WitnessOperands : IA5Step1Operands :=
  { primary := ia5WitnessSource, secondary := ia5WitnessSource }

/-- Same-variable Select is two sequential uses: Once + Once becomes Many and
    Linear + Linear becomes Unrestricted. -/
theorem IA5_select_same_operand_sequential_witness :
    let result := ia5Step1 (.select .same) ia5WitnessOperands ia5WitnessDemand
    result.primary.cardinality = .Many ∧
      result.primary.consumption = .Unrestricted := by
  decide

/-- Negative pin: alternative-path join would leave the same-variable Select at
    Once/Linear and therefore undercount the two syntactic arm transfers. -/
theorem IA5_select_same_operand_rejects_alternative_join :
    let result := ia5Step1 (.select .same) ia5WitnessOperands ia5WitnessDemand
    result.primary.cardinality ≠
        ia5WitnessSource.cardinality.join ia5WitnessDemand.cardinality ∧
      result.primary.consumption ≠
        ia5WitnessSource.consumption.join ia5WitnessDemand.consumption := by
  decide

/-- Negative pin: even a Many/Unrestricted destination does not transfer those
    dimensions into an aggregate argument; only Owned and locality transfer. -/
theorem IA5_aggregate_does_not_transfer_destination_demand
    (kind : IA5AggregateKind) :
    let result := ia5Step1 (.aggregate kind) ia5WitnessOperands
      { ia5WitnessDemand with
        consumption := .Unrestricted
      , cardinality := .Many }
    result.primary.access = .Owned ∧
      result.primary.locality = .HeapEscaping ∧
      result.primary.consumption = .Dead ∧
      result.primary.cardinality = .Absent := by
  cases kind <;> decide

/-- Set follows the same no-multiplicity-transfer boundary while using the
    base's locality rather than a destination state. -/
theorem IA5_set_uses_base_locality_without_demand_transfer :
    let operands : IA5Step1Operands :=
      { primary := ia5WitnessSource
      , secondary := { ia5WitnessSource with locality := .HeapEscaping } }
    let result := ia5Step1 .set operands ia5WitnessDemand
    result.primary.access = .Owned ∧
      result.primary.locality = .HeapEscaping ∧
      result.primary.consumption = .Dead ∧
      result.primary.cardinality = .Absent := by
  decide

/-! ## §8 TF-8 Select uniqueness downgrade (annex-e §AIMS §4 TF-8)

    The one-SCALAR Select case downgrades the surviving operand's uniqueness via
    `max(MaybeShared, u)` — preserves Shared, downgrades Unique to MaybeShared.
    Modeled over the `Uniqueness` chain (`join = max` on rank, per Model.lean). -/

/-- TF-8 case 2: `max(MaybeShared, u)` table — Unique → MaybeShared (downgrade),
    MaybeShared → MaybeShared (preserve), Shared → Shared (preserve). -/
theorem TF8_uniqueness_downgrade_table :
    Uniqueness.join .MaybeShared .Unique = .MaybeShared ∧
    Uniqueness.join .MaybeShared .MaybeShared = .MaybeShared ∧
    Uniqueness.join .MaybeShared .Shared = .Shared := by decide

/-- TF-8 part (d): the uniqueness downgrade `max(MaybeShared, ·)` is monotone over
    the Uniqueness chain (L-6 layer (b) for the Select scalar-exclusion case). -/
theorem TF8_select_downgrade_monotone (u1 u2 : Uniqueness) (h : u1.rank ≤ u2.rank) :
    (Uniqueness.join .MaybeShared u1).rank ≤ (Uniqueness.join .MaybeShared u2).rank := by
  cases u1 <;> cases u2 <;> simp_all [Uniqueness.join, Uniqueness.rank]

/-! ## §8a TF-13 capture_state_update (annex-e §AIMS §4 TF-13, OxCaml LAM pattern)

    `capture_state_update(current, closure_state, closure_card)` mutates the
    captured-argument state over the §4 TF-13 OxCaml Locality-And-Multiplicity
    pattern, splitting on the closure's cardinality:
      Branch 1 (closure_card ≤ Once):
        consumption := seq_add(current.consumption, Affine)
        cardinality := seq_add(current.cardinality, Once)
        locality    := max(current.locality, closure_state.locality)
      Branch 2 (closure_card > Once, i.e. Many):
        consumption := Unrestricted
        cardinality := Many
        locality    := max(current.locality, closure_state.locality)
    Access-promotion clause (cross-cuts both branches): when
    `closure_state.locality ≥ HeapEscaping` the access is promoted to Owned (per
    TF-13 + CN-8 interaction); otherwise it inherits `current.access`. The L-6
    obligation is monotonicity in `closure_state` for fixed `current`. -/

/-- TF-13: promote access to Owned when the closure escapes via the heap
    (`closure_state.locality ≥ HeapEscaping`); else inherit `current.access`. -/
def promoteAccessForCapture (current closureState : AimsState) : AccessClass :=
  if closureState.locality.rank ≥ Locality.HeapEscaping.rank
  then .Owned else current.access

/-- TF-13: the `capture_state_update` forward mutation over the existing
    AimsState model. `closureCard ≤ Once` selects branch 1 (seq_add accumulation);
    `closureCard > Once` selects branch 2 (Unrestricted / Many constants). The
    locality update is `max(current.locality, closure_state.locality)` in both
    branches; the access-promotion clause is applied in both. Uniqueness / Shape /
    Effect are unchanged (no propagation per §4 TF-13). -/
def captureStateUpdate (current closureState : AimsState)
    (closureCard : Cardinality) : AimsState :=
  if closureCard.rank ≤ Cardinality.One.rank then
    { current with
      access := promoteAccessForCapture current closureState
    , consumption := Consumption.seqAdd current.consumption .Affine
    , cardinality := Cardinality.seqAdd current.cardinality .One
    , locality := current.locality.join closureState.locality }
  else
    { current with
      access := promoteAccessForCapture current closureState
    , consumption := .Unrestricted
    , cardinality := .Many
    , locality := current.locality.join closureState.locality }

/-- TF-13 part (a): branch-1 (`closureCard ≤ Once`) updates exactly per §4 TF-13 —
    `consumption := seq_add(current.consumption, Affine)`,
    `cardinality := seq_add(current.cardinality, Once)`,
    `locality := max(current.locality, closure_state.locality)`. -/
theorem TF13_branch1_spec (current closureState : AimsState)
    (k : Cardinality) (hk : k.rank ≤ Cardinality.One.rank) :
    (captureStateUpdate current closureState k).consumption
      = Consumption.seqAdd current.consumption .Affine ∧
    (captureStateUpdate current closureState k).cardinality
      = Cardinality.seqAdd current.cardinality .One ∧
    (captureStateUpdate current closureState k).locality
      = current.locality.join closureState.locality := by
  unfold captureStateUpdate
  rw [if_pos hk]
  exact ⟨rfl, rfl, rfl⟩

/-- TF-13 part (b): branch-2 (`closureCard > Once`) assigns the TOP constants —
    `consumption := Unrestricted`, `cardinality := Many` — and keeps the
    `locality := max` mutation. -/
theorem TF13_branch2_spec (current closureState : AimsState)
    (k : Cardinality) (hk : ¬ k.rank ≤ Cardinality.One.rank) :
    (captureStateUpdate current closureState k).consumption = .Unrestricted ∧
    (captureStateUpdate current closureState k).cardinality = .Many ∧
    (captureStateUpdate current closureState k).locality
      = current.locality.join closureState.locality := by
  unfold captureStateUpdate
  rw [if_neg hk]
  exact ⟨rfl, rfl, rfl⟩

/-- TF-13 part (c): Access-promotion clause enumeration. When the closure
    locality is `≥ HeapEscaping` (`HeapEscaping` or `Unknown`), the output access
    is `Owned` regardless of `current.access` or the branch taken. -/
theorem TF13_access_promotion (current closureState : AimsState)
    (k : Cardinality) (hloc : closureState.locality.rank ≥ Locality.HeapEscaping.rank) :
    (captureStateUpdate current closureState k).access = .Owned := by
  -- The access field is `promoteAccessForCapture` in both branches.
  have hacc : (captureStateUpdate current closureState k).access
      = promoteAccessForCapture current closureState := by
    unfold captureStateUpdate
    split <;> rfl
  rw [hacc]
  simp only [promoteAccessForCapture, hloc, if_true]

/-- TF-13 part (c) negative side: when the closure locality is strictly below
    `HeapEscaping` (`BlockLocal` / `FunctionLocal` / `ArgEscaping`), there is NO
    promotion — the output access inherits `current.access`. -/
theorem TF13_no_access_promotion (current closureState : AimsState)
    (k : Cardinality) (hloc : ¬ closureState.locality.rank ≥ Locality.HeapEscaping.rank) :
    (captureStateUpdate current closureState k).access = current.access := by
  have hacc : (captureStateUpdate current closureState k).access
      = promoteAccessForCapture current closureState := by
    unfold captureStateUpdate
    split <;> rfl
  rw [hacc]
  simp only [promoteAccessForCapture, hloc, if_false]

/-- TF-13 L-6 layer (b): `capture_state_update` is MONOTONE in `closure_state`
    for fixed `current` and fixed `closure_card`. The closure_state feeds the
    output through exactly two channels — the `locality := max(current, ·)` join
    (monotone over the §1.5 Locality chain) and the access-promotion clause
    (monotone: a higher closure locality can only promote access from
    `current.access` up to `Owned`, never down). The proof establishes the
    join-defined product order `out1 ≤ out2` (componentwise `rawJoin`) by
    destructuring per dimension and discharging by `decide` per leaf.

    Monotonicity is stated over `s1 ≤ s2` (the product order); the only
    dimension of `closureState` the output reads is `locality`, so the relevant
    hypothesis is `s1.locality ≤ s2.locality` (extracted from the product order),
    and the locality + access channels are the only output dimensions that vary
    with `closureState`. -/
theorem TF13_capture_state_update_monotone
    (current s1 s2 : AimsState) (k : Cardinality)
    (h : AimsState.le s1 s2) :
    AimsState.le (captureStateUpdate current s1 k)
                 (captureStateUpdate current s2 k) := by
  -- From the product order, the closure-state localities are ≤ on rank.
  have hjoin : s1.rawJoin s2 = s2 := h
  have hl : s1.locality.join s2.locality = s2.locality := by
    have := congrArg AimsState.locality hjoin; simpa [AimsState.rawJoin] using this
  -- locality.join is `max` on rank, so `s1.locality ≤ s2.locality` (rank order).
  -- `hl` says the join equals `s2.locality`; rewriting with it gives the rank ≤.
  have hlrank : s1.locality.rank ≤ s2.locality.rank := by
    have hjr := congrArg Locality.rank hl
    -- `(s1.locality.join s2.locality).rank = s2.locality.rank`.
    simp only [Locality.join] at hjr
    -- Case on the `if s1.rank ≥ s2.rank` guard inside the join.
    by_cases hge : s1.locality.rank ≥ s2.locality.rank
    · rw [if_pos hge] at hjr
      -- hjr : s1.locality.rank = s2.locality.rank
      exact Nat.le_of_eq hjr
    · exact Nat.le_of_lt (Nat.lt_of_not_le hge)
  -- Now show the rawJoin-order equation field by field on the two outputs.
  show (captureStateUpdate current s1 k).rawJoin (captureStateUpdate current s2 k)
       = captureStateUpdate current s2 k
  -- The outputs agree on every dimension except `access` and `locality`, which
  -- track `closureState`. Split on the branch + handle both varying dimensions.
  -- locality channel: max(current.locality, s1.locality) ≤ max(current.locality,
  -- s2.locality) since s1.locality ≤ s2.locality (rank-monotone max).
  have hlocchan : (current.locality.join s1.locality).join
      (current.locality.join s2.locality) = current.locality.join s2.locality := by
    cases hc : current.locality <;> cases h1 : s1.locality <;> cases h2 : s2.locality <;>
      first
        | rfl
        | (exfalso; rw [h1, h2] at hlrank; simp only [Locality.rank] at hlrank; omega)
  -- access channel: promoteAccessForCapture is monotone in closureState.locality.
  have hacc : (promoteAccessForCapture current s1).join
      (promoteAccessForCapture current s2) = promoteAccessForCapture current s2 := by
    simp only [promoteAccessForCapture]
    cases h1 : s1.locality <;> cases h2 : s2.locality <;>
      first
        | (cases ha : current.access <;> decide)
        | (exfalso; rw [h1, h2] at hlrank; simp only [Locality.rank] at hlrank; omega)
  -- Every output dimension except `access` and `locality` depends only on
  -- `current` and `k` — identical between the two outputs — so its componentwise
  -- join is idempotent. `access` uses `hacc`, `locality` uses `hlocchan`.
  unfold captureStateUpdate
  split <;>
    (simp only [AimsState.rawJoin, hacc, hlocchan, Consumption.join_idem,
      Cardinality.join_idem, Uniqueness.join_idem, Shape.join_idem,
      EffectClass.join_idem])

/-! ## §9 L-6 layer (b) — per-TF-N transfer-function monotonicity

    The §3 L-6 obligation `a ≤ b ⟹ f(a) ≤ f(b)` over the join-defined product
    order `AimsState.le` (`Lattice.lean`, via componentwise `rawJoin`). Modeled
    for every forward TF that takes an `AimsState` operand (TF-4 Project) or is a
    constant function (TF-3 / TF-5 / TF-6 / TF-7 / TF-9 / TF-9a — vacuously
    monotone, image is independent of input → equal images, `le` reflexive).
    The Composition theorem (annex-e §AIMS §4 Composition) lifts these per-TF
    monotonicities across a block via L-2 associativity (function composition). -/

/-- Constant transfer functions (TF-3 / TF-5 / TF-7 / TF-9 / TF-9a / TF-6) are
    monotone: the image is independent of the input, so `(fun _ => k) a = k =
    (fun _ => k) b` and `le k k` holds by L-4 reflexivity. This is the L-6 layer
    (b) obligation, in general form, for every per-token constant forward rule. -/
theorem L6_const_monotone (k : AimsState) (a b : AimsState) (_ : AimsState.le a b) :
    AimsState.le ((fun _ => k) a) ((fun _ => k) b) :=
  L4_le_refl k

/-- TF-3 monotonicity (L-6 layer (b)): `tfConstruct` ignores its operand state, so
    for any `a ≤ b`, `f a = f b`, hence `f a ≤ f b` (reflexivity). Stated as: the
    image is a fixed FRESH state, `le`-comparable to itself. -/
theorem TF3_monotone (c : Ctor) : AimsState.le (tfConstruct c) (tfConstruct c) :=
  L4_le_refl (tfConstruct c)

/-- TF-5 monotonicity (L-6 layer (b)): CONSERVATIVE constant → reflexive `le`. -/
theorem TF5_monotone : AimsState.le tfApplyNoContract tfApplyNoContract :=
  L4_le_refl tfApplyNoContract

/-- TF-7 monotonicity (L-6 layer (b)): FRESH(NonReusable) constant → reflexive. -/
theorem TF7_monotone : AimsState.le tfPartialApply tfPartialApply :=
  L4_le_refl tfPartialApply

/-- TF-9a monotonicity (L-6 layer (b)): FRESH(CollectionBuffer) → reflexive. -/
theorem TF9a_monotone : AimsState.le tfCollectionReuse tfCollectionReuse :=
  L4_le_refl tfCollectionReuse

/-- TF-6 monotonicity (L-6 layer (b)): for a fixed return contract, `tfApplyContract`
    is a constant function of the call operand → reflexive `le`. -/
theorem TF6_monotone (rc : ReturnContract) :
    AimsState.le (tfApplyContract rc) (tfApplyContract rc) :=
  L4_le_refl (tfApplyContract rc)

/-- TF-4 monotonicity (L-6 layer (b)): `tfProject` inherits Uniqueness + Locality
    from the source and fixes the other dimensions to constants. It is monotone in
    the source: `src1 ≤ src2 ⟹ tfProject src1 ≤ tfProject src2`. Proven by
    destructuring the componentwise `rawJoin`-order on every dimension and
    discharging by `decide` per leaf. -/
theorem TF4_project_monotone (src1 src2 : AimsState)
    (h : AimsState.le src1 src2) : AimsState.le (tfProject src1) (tfProject src2) := by
  -- Extract the per-dimension equalities the componentwise rawJoin order gives.
  have hjoin : src1.rawJoin src2 = src2 := h
  have hu : src1.uniqueness.join src2.uniqueness = src2.uniqueness := by
    have := congrArg AimsState.uniqueness hjoin; simpa [AimsState.rawJoin] using this
  have hl : src1.locality.join src2.locality = src2.locality := by
    have := congrArg AimsState.locality hjoin; simpa [AimsState.rawJoin] using this
  -- The projected states differ only in uniqueness + locality (inherited); the
  -- other five dimensions are equal constants, so their componentwise join is
  -- reflexive. Build the rawJoin-order equation field by field.
  show (tfProject src1).rawJoin (tfProject src2) = tfProject src2
  simp only [tfProject, AimsState.rawJoin, AccessClass.join, Consumption.join,
    Cardinality.join, Shape.join, EffectClass.join, hu, hl, ite_self, Bool.or_self]

end AimsProof
