/-
AIMS shared model module — the Lean ENCODING of the AIMS product lattice.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: shared ENCODING of the §1 dimensions + §5 canonicalize (consumed by
    every L/CN/TF/DP/IC/PL/RL/VF/CH theorem; not a single rule-ID) |
  spec: annex-e §AIMS §1 + §5 |
  .proof: aims-proof/proofs/02-lattice/Lattice.proof + Product.proof (the
    lattice-foundation derivation this model must match) |
  map: aims-proof/scripts/proof-lean-map.json (every theorem's `lean` anchor
    imports this model; the model row records the encoding tie).

Correspondence: `docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS`
(§1 dimensions + orderings, §5 canonicalization rules). The seven dimension
carriers + EffectClass + AimsState are lifted from the verified prototype at
`proofs/10-counterexample/bug-04-118.lean` §1.

Cardinalities: Access 2 × Consumption 4 × Cardinality 3 × Uniqueness 3 ×
Locality 5 × Shape 5 × Effect 8.

This module is the encoding of the calculus; it does NOT claim the shipped
Rust implementation matches the calculus (implementation fidelity is gated
separately by shipped-conformance + dual-execution parity).
-/

-- A finite resource bound so a runaway tactic search over the finite product
-- fails rather than hanging the build gate.
set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §1 Lattice dimension carriers (annex-e §AIMS §1.1–§1.7) -/

/-- §1.1 Access — `Borrowed < Owned`, height 1. -/
inductive AccessClass
  | Borrowed
  | Owned
deriving Repr, DecidableEq

/-- §1.2 Consumption — `Dead < Linear < Affine < Unrestricted`, height 3. -/
inductive Consumption
  | Dead
  | Linear
  | Affine
  | Unrestricted
deriving Repr, DecidableEq

/-- §1.3 Cardinality — `Absent < Once < Many`, height 2.
    The carrier `One` is the prototype name for `Once`. -/
inductive Cardinality
  | Absent
  | One
  | Many
deriving Repr, DecidableEq

/-- §1.4 Uniqueness — `Unique < MaybeShared < Shared`, height 2. -/
inductive Uniqueness
  | Unique
  | MaybeShared
  | Shared
deriving Repr, DecidableEq

/-- §1.5 Locality — `BlockLocal < FunctionLocal < ArgEscaping < HeapEscaping < Unknown`,
    height 4. -/
inductive Locality
  | BlockLocal
  | FunctionLocal
  | ArgEscaping
  | HeapEscaping
  | Unknown
deriving Repr, DecidableEq

/-- §1.6 Shape — flat lattice: equal stays, unequal joins to `NonReusable`,
    height 1. -/
inductive Shape
  | NonReusable
  | ReusableStruct
  | ReusableEnumVariant
  | CollectionBuffer
  | ContextHole
deriving Repr, DecidableEq

/-- §1.7 Effect — three independent boolean flags, join = componentwise OR,
    height 3 (8 inhabitants). -/
structure EffectClass where
  may_alloc : Bool := false
  may_share : Bool := false
  may_throw : Bool := false
deriving Repr, DecidableEq

/-- The AIMS product lattice element. -/
structure AimsState where
  access : AccessClass
  consumption : Consumption
  cardinality : Cardinality
  uniqueness : Uniqueness
  locality : Locality
  shape : Shape
  effect : EffectClass
deriving Repr, DecidableEq

/-! ## §1 Per-dimension rank (totally-ordered chains) + join

    For the five chain dimensions, `join = max` over the chain order
    (annex-e §AIMS §1.1–§1.5). Each rank assigns the chain position. -/

def AccessClass.rank : AccessClass → Nat
  | .Borrowed => 0
  | .Owned    => 1

def Consumption.rank : Consumption → Nat
  | .Dead         => 0
  | .Linear       => 1
  | .Affine       => 2
  | .Unrestricted => 3

def Cardinality.rank : Cardinality → Nat
  | .Absent => 0
  | .One    => 1
  | .Many   => 2

def Uniqueness.rank : Uniqueness → Nat
  | .Unique      => 0
  | .MaybeShared => 1
  | .Shared      => 2

def Locality.rank : Locality → Nat
  | .BlockLocal    => 0
  | .FunctionLocal => 1
  | .ArgEscaping   => 2
  | .HeapEscaping  => 3
  | .Unknown       => 4

/-- §1.1 Access join = max over the chain. -/
def AccessClass.join (a b : AccessClass) : AccessClass :=
  if a.rank ≥ b.rank then a else b

/-- §1.2 Consumption join = max over the chain. -/
def Consumption.join (a b : Consumption) : Consumption :=
  if a.rank ≥ b.rank then a else b

/-- §1.3 Cardinality join (`alt_join`) = max over the chain. -/
def Cardinality.join (a b : Cardinality) : Cardinality :=
  if a.rank ≥ b.rank then a else b

/-- §1.4 Uniqueness join = max over the chain. -/
def Uniqueness.join (a b : Uniqueness) : Uniqueness :=
  if a.rank ≥ b.rank then a else b

/-- §1.5 Locality join = max over the chain. -/
def Locality.join (a b : Locality) : Locality :=
  if a.rank ≥ b.rank then a else b

/-- §1.6 Shape flat-lattice join: equal stays, unequal → `NonReusable`. -/
def Shape.join (a b : Shape) : Shape :=
  if a = b then a else Shape.NonReusable

/-- §1.7 Effect join = componentwise OR. -/
def EffectClass.join (a b : EffectClass) : EffectClass :=
  { may_alloc := a.may_alloc || b.may_alloc
  , may_share := a.may_share || b.may_share
  , may_throw := a.may_throw || b.may_throw }

/-! ## §1 Product join = componentwise join + canonicalization (annex-e §AIMS §1) -/

/-- Raw componentwise product join, before canonicalization. -/
def AimsState.rawJoin (a b : AimsState) : AimsState :=
  { access      := a.access.join b.access
  , consumption := a.consumption.join b.consumption
  , cardinality := a.cardinality.join b.cardinality
  , uniqueness  := a.uniqueness.join b.uniqueness
  , locality    := a.locality.join b.locality
  , shape       := a.shape.join b.shape
  , effect      := a.effect.join b.effect }

/-! ## §5 Canonicalization (annex-e §AIMS §5; rule ordering CN-8 before CN-6) -/

/-- CN-1 Dead ↔ Absent bidirectional:
    `Dead ⟹ Absent` and `Absent ⟹ Dead`. -/
def cn1 (s : AimsState) : AimsState :=
  match s.consumption, s.cardinality with
  | .Dead, _      => { s with cardinality := .Absent }
  | _,     .Absent => { s with consumption := .Dead }
  | _,     _       => s

/-- CN-3 Shared blocks reuse:
    `Uniqueness = Shared ∧ Shape ≠ NonReusable ⟹ Shape := NonReusable`. -/
def cn3 (s : AimsState) : AimsState :=
  match s.uniqueness with
  | .Shared => if s.shape = .NonReusable then s else { s with shape := .NonReusable }
  | _       => s

/-- CN-8 Borrowed locality ceiling:
    `Access = Borrowed ∧ Locality > FunctionLocal ⟹ Locality := FunctionLocal`.
    Fires BEFORE CN-6 so locality is precise when CN-6 evaluates. -/
def cn8 (s : AimsState) : AimsState :=
  match s.access with
  | .Borrowed =>
      if s.locality.rank > Locality.FunctionLocal.rank
      then { s with locality := .FunctionLocal } else s
  | _ => s

/-- CN-6 Wide-locality uniqueness ceiling:
    `Locality ≥ HeapEscaping ∧ Uniqueness = Unique ⟹ Uniqueness := MaybeShared`. -/
def cn6 (s : AimsState) : AimsState :=
  if s.locality.rank ≥ Locality.HeapEscaping.rank ∧ s.uniqueness = .Unique
  then { s with uniqueness := .MaybeShared } else s

/-- One canonicalization pass: CN-1, CN-3, then CN-8 before CN-6 (annex-e §5).
    CN-2/CN-5 are documentation/no-op (CN-2 subsumed by CN-1; CN-5 forbids a
    collapse, so it adds no mutation); CN-4/CN-7 are reserved/removed. The
    active rules reach a fixed point in one pass. -/
def canonicalize (s : AimsState) : AimsState :=
  cn6 (cn8 (cn3 (cn1 s)))

/-- The product join: componentwise join followed by canonicalization
    (annex-e §AIMS §1 "Product join = componentwise join + canonicalization"). -/
def AimsState.join (a b : AimsState) : AimsState :=
  canonicalize (a.rawJoin b)

/-! ## Computability pins — `join` is decidable and computable, so downstream
    `by decide` over the finite product is viable. -/

/-- A canonical fresh-allocation state (TF-3 shape) used by the pins. -/
def freshState : AimsState :=
  { access := .Owned
  , consumption := .Linear
  , cardinality := .One
  , uniqueness := .Unique
  , locality := .BlockLocal
  , shape := .CollectionBuffer
  , effect := { may_alloc := true, may_share := false, may_throw := false } }

-- `join` is computable: it evaluates to a concrete `AimsState`.
#eval (freshState.join freshState)

-- Idempotence on the fresh state, discharged by `decide` (the derived
-- `DecidableEq` makes the goal decidable → the finite-domain proof tactic works).
example : freshState.join freshState = freshState := by decide

-- A canonicalization pin: joining a Borrowed wide-locality state clamps
-- locality to FunctionLocal (CN-8), so the result's locality is FunctionLocal.
example :
    (canonicalize
      { access := .Borrowed
      , consumption := .Linear
      , cardinality := .One
      , uniqueness := .MaybeShared
      , locality := .HeapEscaping
      , shape := .NonReusable
      , effect := {} }).locality = Locality.FunctionLocal := by decide

end AimsProof
