/-
AIMS transfer-function module — kernel-checked Lean proofs of the per-instruction
forward transfer functions, the backward-demand accumulation operators, and the
L-6 layer (b) monotonicity obligations over the finite `AimsState` product.

Correspondence: `docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS §4`
(Transfer Functions TF-1..TF-15a, forward + backward) + Appendix A
(Forward Transfer Matrix) + §3 ("Transfer functions shall be monotone:
`a ≤ b ⟹ f(a) ≤ f(b)`" — the L-6 layer (b) obligation, one per TF-N).

The rule IDs index the §4 obligations:
  TF-1   Let { Literal }      → SCALAR        (forward, constant)
  TF-2   Let { Var(v) }       → state(v)      (forward, identity)
  TF-2a  Let { PrimOp }       → SCALAR        (forward, constant)
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
`AimsState` inhabitant, so TF-1 / TF-2a / TF-10 / TF-10a (whose Appendix-A row is
the SCALAR sentinel) have no `AimsState`-valued forward image to model — they are
the L-9-excluded rows and are NOT modeled here (their faithful encoding is the
absence of a SCALAR `AimsState`, per `Lattice.lean` §L-9). The forward rules that
DO produce an `AimsState` (TF-3..TF-9a) plus the backward-accumulation operators
(TF-11, TF-14) plus the L-6 layer-(b) monotonicity theorems are the finite-domain
propositions modeled and proven here.

Proof strategy mirrors `Lattice.lean`: per-dimension `cases` destructure before
`decide` so each leaf is trivial (naive `decide` over the ~7200-state product
times out). Monotonicity over `AimsState.le` (defined in `Lattice.lean` via the
componentwise `rawJoin`) is destructured per-dimension, then `decide`.
-/

import AimsProof.Model
import AimsProof.Lattice

set_option maxHeartbeats 1000000

namespace AimsProof

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

/-- TF-3: every FRESH allocation is `Unique` with RC = 1 (the FRESH uniqueness
    invariant the reuse/COW layer relies on). -/
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
