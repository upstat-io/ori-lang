/-
AIMS decision-predicate module — kernel-checked Lean proofs of the decision
predicates DP-1..DP-9 over the finite `AimsState` product.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: DP-1..DP-9 (DP-10 REMOVED — doc-comment, no theorem) |
  spec: annex-e §AIMS §3 (dimensions) + §8 (the realization rules each DP gates) |
  .proof: aims-proof/proofs/05-decisions/DP-{1..9}.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: `docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS`.
The decision predicates map a canonical lattice state (§3 dimensions) to an
RC / COW / reuse / stack decision; each predicate gates a §8 realization rule:
  DP-1 is_rc_needed        gates §8.1 RL-1 / RL-2 reference-count emission
  DP-2 is_rc_dec_unnecessary gates the §8.1 supplementary-dec elision
  DP-3 is_rc_inc_elidable  gates the §8.1 move-once inc elision
  DP-4 needs_cow_check     gates §8.2 RL-6 / RL-7 / RL-8 (Unique / MaybeShared / Shared)
  DP-5 can_mutate_in_place gates §8.2 RL-6 in-place mutation + §8.2 RL-10 disjoint-field
  DP-6 is_reuse_candidate  gates §8.3 RL-11 / RL-11a allocation reuse
  DP-7 is_rc_skip_eligible gates the §8.1 caller-inc / callee-dec pair elision
  DP-8 is_local            gates §8.4 RL-14 / RL-15 stack promotion (Locality ≤ FunctionLocal)
  DP-9 cow_mode            gates §8.2 RL-6 / RL-7 / RL-8 mode selection (Static / Dynamic)
  DP-10 REMOVED            unsound caller→callee uniqueness derivation; see doc-comment.

Each predicate is defined as a Lean function over `AimsState` (DP-1..DP-4, DP-6,
DP-8 pure) plus the small extra inputs DP-5 / DP-7 / DP-9 need:
  - DP-1 / DP-7 take an `is_scalar : Bool`. SCALAR is NOT an `AimsState`
    inhabitant (the §AIMS §3 SCALAR-exclusion invariant — see `Lattice.lean`
    §L-9); the §3 dimensions model only the non-scalar carrier. The truth
    table's `is_scalar` column ranges over a value that is either the SCALAR
    sentinel or a real `AimsState`, so the faithful encoding is an explicit
    `Bool` input (matching the DP-1 / DP-7 truth-table enumeration axis).
  - DP-5 takes the MINIMAL structural model of the §8.2 borrow / alias / liveness
    inputs the truth table is defined over: an `InstrKind` (Set / SetTag), a
    `directBorrowLiveOverlap : Bool` (a live direct borrow whose field path
    overlaps the mutation field per §8.2 RL-10), and an `aliasLive : Bool` (a
    live transitive non-borrow alias). The §8.2 RL-10 SetTag whole-payload
    invalidation clause (a tag change invalidates every payload field) is
    encoded by forcing overlap whenever `InstrKind = SetTag` and any direct
    borrow is live, regardless of field path.
  - DP-9 takes the DP-5 result plus the §7 IC-3 `ParamContract.uniqueness` input.

The proof strategy mirrors `Lattice.lean` / `Transfer.lean`: per-dimension
`cases` destructure before `decide` so each leaf is trivial (naive `decide` over
the ~7200-state product times out). Each truth-table theorem name encodes the
rule ID (`DP1_is_rc_needed_table`, …, `DP9_cow_mode_table`).
-/

import AimsProof.Model

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §DP-1 — `is_rc_needed` (annex-e §AIMS §3 + §8.1 RL-1 / RL-2)

    `is_rc_needed(s) ⟺ s.access = Owned ∧ s.consumption ≠ Dead ∧ ¬is_scalar`.
    Gates §8.1 reference-count emission: an owned, live, non-scalar value carries
    RC responsibility. -/

/-- DP-1: RC is needed iff the value is Owned, live (≠ Dead), and not a scalar. -/
def is_rc_needed (s : AimsState) (is_scalar : Bool) : Bool :=
  (s.access = .Owned) && (s.consumption != .Dead) && (!is_scalar)

/-- DP-1 truth table (4 rows, exhaustive over Access × Consumption × is_scalar):
      Borrowed → false; Owned ∧ Dead → false; Owned ∧ ≠Dead ∧ scalar → false;
      Owned ∧ ≠Dead ∧ non-scalar → true. -/
theorem DP1_is_rc_needed_table (s : AimsState) (is_scalar : Bool) :
    is_rc_needed s is_scalar
      = ((s.access = .Owned) && (s.consumption != .Dead) && (!is_scalar)) := by
  rfl

/-- DP-1 row partition by destructuring the read dimensions: the predicate is
    `true` on EXACTLY the Owned × non-Dead × non-scalar corner. -/
theorem DP1_is_rc_needed_rows (acc : AccessClass) (con : Consumption)
    (rest : AimsState) (is_scalar : Bool) :
    is_rc_needed { rest with access := acc, consumption := con } is_scalar
      = (decide (acc = .Owned) && decide (con ≠ .Dead) && !is_scalar) := by
  cases acc <;> cases con <;> cases is_scalar <;> rfl

/-! ## §DP-2 — `is_rc_dec_unnecessary` (annex-e §AIMS §3 + §8.1)

    `is_rc_dec_unnecessary(s) ⟺ s.cardinality = Absent ∨ s.consumption = Dead`.
    Under CN-1 (§5) the two clauses are bidirectionally implied on canonical
    states. Gates the §8.1 supplementary-dec elision. -/

def is_rc_dec_unnecessary (s : AimsState) : Bool :=
  (s.cardinality = .Absent) || (s.consumption = .Dead)

/-- DP-2 truth table (Cardinality × Consumption): Absent → true; Dead → true;
      Once ∧ ≠Dead → false; Many ∧ ≠Dead → false. -/
theorem DP2_is_rc_dec_unnecessary_table (car : Cardinality) (con : Consumption)
    (rest : AimsState) :
    is_rc_dec_unnecessary { rest with cardinality := car, consumption := con }
      = (decide (car = .Absent) || decide (con = .Dead)) := by
  cases car <;> cases con <;> rfl

/-! ## §DP-3 — `is_rc_inc_elidable` (annex-e §AIMS §3 + §8.1)

    `is_rc_inc_elidable(s) ⟺ s.cardinality = Once ∧ s.consumption = Linear`.
    Move-once: a value used exactly once and consumed linearly needs no inc. -/

def is_rc_inc_elidable (s : AimsState) : Bool :=
  (s.cardinality = .One) && (s.consumption = .Linear)

/-- DP-3 truth table (Cardinality × Consumption): Once ∧ Linear → true;
      Once ∧ ≠Linear → false; ≠Once → false. -/
theorem DP3_is_rc_inc_elidable_table (car : Cardinality) (con : Consumption)
    (rest : AimsState) :
    is_rc_inc_elidable { rest with cardinality := car, consumption := con }
      = (decide (car = .One) && decide (con = .Linear)) := by
  cases car <;> cases con <;> rfl

/-! ## §DP-4 — `needs_cow_check` (annex-e §AIMS §3.4 + §8.2 RL-6 / RL-7 / RL-8)

    `needs_cow_check(s) ⟺ s.uniqueness = MaybeShared`. Unique = §8.2 RL-6 static
    fast path; Shared = §8.2 RL-8 static slow path; MaybeShared = §8.2 RL-7
    runtime IsShared check. -/

def needs_cow_check (s : AimsState) : Bool :=
  s.uniqueness = .MaybeShared

/-- DP-4 truth table (Uniqueness): Unique → false; MaybeShared → true;
      Shared → false — true on EXACTLY the MaybeShared row. -/
theorem DP4_needs_cow_check_table (u : Uniqueness) (rest : AimsState) :
    needs_cow_check { rest with uniqueness := u } = decide (u = .MaybeShared) := by
  cases u <;> rfl

/-! ## §DP-6 — `is_reuse_candidate` (annex-e §AIMS §3 + §8.3 RL-11 / RL-11a)

    `is_reuse_candidate(s) ⟺ s.access = Owned ∧ s.uniqueness ≠ Shared ∧
      s.shape ≠ NonReusable`. Gates §8.3 allocation reuse. -/

def is_reuse_candidate (s : AimsState) : Bool :=
  (s.access = .Owned) && (s.uniqueness != .Shared) && (s.shape != .NonReusable)

/-- DP-6 truth table (Access × Uniqueness × Shape): Borrowed → false;
      Owned ∧ Shared → false; Owned ∧ NonReusable → false;
      Owned ∧ ≠Shared ∧ reusable-shape → true. -/
theorem DP6_is_reuse_candidate_table (acc : AccessClass) (u : Uniqueness)
    (sh : Shape) (rest : AimsState) :
    is_reuse_candidate { rest with access := acc, uniqueness := u, shape := sh }
      = (decide (acc = .Owned) && decide (u ≠ .Shared) && decide (sh ≠ .NonReusable)) := by
  cases acc <;> cases u <;> cases sh <;> rfl

/-! ## §DP-8 — `is_local` (annex-e §AIMS §3.5 + §8.4 RL-14 / RL-15)

    `is_local(s) ⟺ s.locality ∈ {BlockLocal, FunctionLocal}`. ArgEscaping is NOT
    local — it escapes into the callee; its optimization is §8.4 RL-15a caller
    stack promotion, not RC elision. Gates §8.4 stack promotion. -/

def is_local (s : AimsState) : Bool :=
  (s.locality = .BlockLocal) || (s.locality = .FunctionLocal)

/-- DP-8 truth table (Locality): BlockLocal / FunctionLocal → true;
      ArgEscaping / HeapEscaping / Unknown → false — true on EXACTLY the two
      local rows. -/
theorem DP8_is_local_table (l : Locality) (rest : AimsState) :
    is_local { rest with locality := l }
      = (decide (l = .BlockLocal) || decide (l = .FunctionLocal)) := by
  cases l <;> rfl

/-! ## §DP-7 — `is_rc_skip_eligible` (annex-e §AIMS §3 + §8.1)

    `is_rc_skip_eligible(s) ⟺ is_local(s) ∧ s.access = Owned ∧
      s.consumption = Linear ∧ s.cardinality = Once ∧ s.uniqueness = Unique ∧
      ¬is_scalar`. Gates the §8.1 caller-inc / callee-dec pair elision; evaluated
    on the callee's parameter state. -/

def is_rc_skip_eligible (s : AimsState) (is_scalar : Bool) : Bool :=
  (is_local s)
    && (s.access = .Owned)
    && (s.consumption = .Linear)
    && (s.cardinality = .One)
    && (s.uniqueness = .Unique)
    && (!is_scalar)

/-- DP-7 truth table (7-row): false unless is_local ∧ Owned ∧ Linear ∧ Once ∧
      Unique ∧ non-scalar; the single `true` row is the move-once local unique
      non-scalar owned corner. Proven by destructuring the six read dimensions
      (Locality × Access × Consumption × Cardinality × Uniqueness × is_scalar). -/
theorem DP7_is_rc_skip_eligible_table (l : Locality) (acc : AccessClass)
    (con : Consumption) (car : Cardinality) (u : Uniqueness)
    (rest : AimsState) (is_scalar : Bool) :
    is_rc_skip_eligible
        { rest with locality := l, access := acc, consumption := con,
                    cardinality := car, uniqueness := u } is_scalar
      = ((decide (l = .BlockLocal) || decide (l = .FunctionLocal))
          && decide (acc = .Owned)
          && decide (con = .Linear)
          && decide (car = .One)
          && decide (u = .Unique)
          && !is_scalar) := by
  cases l <;> cases acc <;> cases con <;> cases car <;> cases u <;>
    cases is_scalar <;> rfl

/-! ## §DP-5 — `can_mutate_in_place` (annex-e §AIMS §3 + §8.2 RL-6 / RL-10)

    `can_mutate_in_place(s, var, field, point, instr) ⟺ s.access = Owned ∧
      s.uniqueness = Unique ∧ no_active_overlapping_borrows(var, field, point, instr)`.

    `no_active_overlapping_borrows` decomposes (§8.2 RL-10 + the §3 SCALAR /
    borrow / alias model) into:
      Step 1 (direct borrow): a live direct borrow whose field path OVERLAPS the
        mutation field blocks. For `InstrKind = SetTag` the tag change
        invalidates EVERY payload field (the variant layout may differ), so any
        live direct borrow blocks regardless of field path (§8.2 RL-10 SetTag
        whole-payload invalidation).
      Step 2 (transitive alias): any live transitive non-borrow alias blocks
        conservatively (aliases lack field-level granularity).

    MINIMAL faithful structural model: the truth table IS defined over these
    boolean conditions, so booleans for (a) `InstrKind`, (b) a live direct
    borrow's field-overlap, and (c) a live transitive alias capture the table
    exactly. `anyDirectBorrowLive` distinguishes "a direct borrow is live" from
    "its path overlaps" so SetTag's force-overlap clause is faithful. -/

inductive InstrKind
  | Set
  | SetTag
deriving Repr, DecidableEq

/-- §8.2 RL-10 SetTag whole-payload invalidation: a live direct borrow OVERLAPS
    the mutation field iff the borrow is live AND (the instruction is SetTag —
    invalidating every field — OR the borrow path overlaps the mutation field). -/
def directBorrowOverlaps (instr : InstrKind) (anyDirectBorrowLive : Bool)
    (borrowPathOverlapsField : Bool) : Bool :=
  anyDirectBorrowLive && (decide (instr = .SetTag) || borrowPathOverlapsField)

/-- The §8.2 RL-10 alias-safety side condition: no live overlapping direct
    borrow (Step 1) AND no live transitive alias (Step 2). -/
def noActiveOverlappingBorrows (instr : InstrKind) (anyDirectBorrowLive : Bool)
    (borrowPathOverlapsField : Bool) (aliasLive : Bool) : Bool :=
  (!directBorrowOverlaps instr anyDirectBorrowLive borrowPathOverlapsField)
    && (!aliasLive)

/-- DP-5: in-place mutation is licensed iff Owned, Unique, and no active
    overlapping borrow or alias. Consults the §8.2 minimal borrow / alias /
    liveness model + the `InstrKind` (Set / SetTag) per §8.2 RL-10. -/
def can_mutate_in_place (s : AimsState) (instr : InstrKind)
    (anyDirectBorrowLive : Bool) (borrowPathOverlapsField : Bool)
    (aliasLive : Bool) : Bool :=
  (s.access = .Owned)
    && (s.uniqueness = .Unique)
    && noActiveOverlappingBorrows instr anyDirectBorrowLive borrowPathOverlapsField aliasLive

/-- DP-5 truth table (5-row partition over Access × Uniqueness × borrow-overlap
      × alias-live, with the InstrKind axis):
      ≠Owned → false (Row 1); Owned ∧ ≠Unique → false (Row 2);
      Owned ∧ Unique ∧ live-overlapping-direct-borrow → false (Row 3);
      Owned ∧ Unique ∧ no-overlap ∧ live-alias → false (Row 4);
      Owned ∧ Unique ∧ no-overlap ∧ no-live-alias → true (Row 5). -/
theorem DP5_can_mutate_in_place_table (acc : AccessClass) (u : Uniqueness)
    (rest : AimsState) (instr : InstrKind) (anyDirectBorrowLive : Bool)
    (borrowPathOverlapsField : Bool) (aliasLive : Bool) :
    can_mutate_in_place { rest with access := acc, uniqueness := u }
        instr anyDirectBorrowLive borrowPathOverlapsField aliasLive
      = (decide (acc = .Owned)
          && decide (u = .Unique)
          && (!directBorrowOverlaps instr anyDirectBorrowLive borrowPathOverlapsField)
          && (!aliasLive)) := by
  cases acc <;> cases u <;> cases instr <;> cases anyDirectBorrowLive <;>
    cases borrowPathOverlapsField <;> cases aliasLive <;> rfl

/-- DP-5 §8.2 RL-10 SetTag whole-payload clause: for `InstrKind = SetTag`, ANY
    live direct borrow blocks in-place mutation regardless of its field path —
    even a borrow that would be disjoint under `Set`. The negative witness:
    Owned ∧ Unique ∧ SetTag ∧ a live direct borrow on a DISJOINT field
    (`borrowPathOverlapsField = false`) still yields `false`. -/
theorem DP5_settag_blocks_all (rest : AimsState) :
    can_mutate_in_place { rest with access := .Owned, uniqueness := .Unique }
        .SetTag (anyDirectBorrowLive := true)
        (borrowPathOverlapsField := false) (aliasLive := false) = false := by
  rfl

/-- DP-5 §8.2 RL-10 disjoint-field exemption (positive witness): for
    `InstrKind = Set`, a live direct borrow on a DISJOINT field
    (`borrowPathOverlapsField = false`) does NOT block — Owned ∧ Unique with no
    overlapping borrow and no live alias yields `true`. -/
theorem DP5_set_disjoint_field_allows (rest : AimsState) :
    can_mutate_in_place { rest with access := .Owned, uniqueness := .Unique }
        .Set (anyDirectBorrowLive := true)
        (borrowPathOverlapsField := false) (aliasLive := false) = true := by
  rfl

/-! ## §DP-9 — `cow_mode` (annex-e §AIMS §3.4 + §7 IC-3 + §8.2 RL-6 / RL-7 / RL-8)

    `cow_mode` classifies a mutation into one of three modes. It composes DP-5
    (the local in-place safety check) and the §7 IC-3 `ParamContract.uniqueness`
    (the interprocedural past-guarantee). The 6-row table (post row (c) /
    (c-prime) refinement):
      (a) Unique ∧ DP-5 = true            → StaticUnique  (§8.2 RL-6)
      (b) Unique ∧ DP-5 = false           → StaticShared  (§8.2 RL-8)
      (c) MaybeShared ∧ DP-5 ∧ IC-3 = Unique → StaticUnique (caller proved RC == 1)
      (c-prime) MaybeShared ∧ ¬DP-5 ∧ IC-3 = Unique → Dynamic (local borrow blocks)
      (d) MaybeShared ∧ IC-3 ≠ Unique     → Dynamic  (§8.2 RL-7)
      (e) Shared                          → StaticShared  (§8.2 RL-8). -/

inductive CowMode
  | StaticUnique
  | StaticShared
  | Dynamic
deriving Repr, DecidableEq

/-- DP-9: classify the mutation mode from the state's Uniqueness, the DP-5 local
    in-place result, and the §7 IC-3 parameter-contract uniqueness. -/
def cow_mode (s : AimsState) (dp5 : Bool) (ic3Uniqueness : Uniqueness) : CowMode :=
  match s.uniqueness with
  | .Unique      => if dp5 then .StaticUnique else .StaticShared           -- rows (a) / (b)
  | .Shared      => .StaticShared                                          -- row (e)
  | .MaybeShared =>
      match ic3Uniqueness with
      | .Unique => if dp5 then .StaticUnique else .Dynamic                 -- rows (c) / (c-prime)
      | _       => .Dynamic                                                -- row (d)

/-- DP-9 truth table (6-row, exhaustive over Uniqueness × DP-5-result ×
      IC-3-uniqueness — 3 × 2 × 3 = 18 cases): matches the row partition
      (a) / (b) / (c) / (c-prime) / (d) / (e). -/
theorem DP9_cow_mode_table (u : Uniqueness) (rest : AimsState)
    (dp5 : Bool) (ic3Uniqueness : Uniqueness) :
    cow_mode { rest with uniqueness := u } dp5 ic3Uniqueness
      = (match u with
         | .Unique      => if dp5 then CowMode.StaticUnique else CowMode.StaticShared
         | .Shared      => CowMode.StaticShared
         | .MaybeShared =>
             match ic3Uniqueness with
             | .Unique => if dp5 then CowMode.StaticUnique else CowMode.Dynamic
             | _       => CowMode.Dynamic) := by
  cases u <;> cases dp5 <;> cases ic3Uniqueness <;> rfl

/-- DP-9 row (c): MaybeShared ∧ DP-5 = true ∧ IC-3 = Unique → StaticUnique. The
    caller-proved RC == 1 (IC-3) composes with local borrow safety (DP-5). -/
theorem DP9_row_c_refinement (rest : AimsState) :
    cow_mode { rest with uniqueness := .MaybeShared } true .Unique
      = CowMode.StaticUnique := by rfl

/-- DP-9 row (c-prime): MaybeShared ∧ DP-5 = false ∧ IC-3 = Unique → Dynamic.
    IC-3 proves not-shared at entry but the local DP-5 check forces fall-through
    to the §8.2 RL-7 runtime IsShared path (the joint-DP-5 refinement). -/
theorem DP9_row_c_prime_fallthrough (rest : AimsState) :
    cow_mode { rest with uniqueness := .MaybeShared } false .Unique
      = CowMode.Dynamic := by rfl

/-- DP-9 row (d): MaybeShared ∧ IC-3 ≠ Unique → Dynamic regardless of DP-5.
    No interprocedural guarantee → §8.2 RL-7 runtime check required. -/
theorem DP9_row_d_dynamic (rest : AimsState) (dp5 : Bool) (ic3 : Uniqueness)
    (h : ic3 ≠ .Unique) :
    cow_mode { rest with uniqueness := .MaybeShared } dp5 ic3 = CowMode.Dynamic := by
  cases ic3 <;> cases dp5 <;> first | rfl | exact absurd rfl h

/-- DP-9 row (e): Shared → StaticShared regardless of DP-5 / IC-3 (RC > 1 proved
    statically → §8.2 RL-8 unconditional copy). -/
theorem DP9_row_e_shared (rest : AimsState) (dp5 : Bool) (ic3 : Uniqueness) :
    cow_mode { rest with uniqueness := .Shared } dp5 ic3 = CowMode.StaticShared := by
  rfl

/-! ## §DP-10 — REMOVED (unsound)

    DP-10 formerly claimed `Owned ∧ Linear ∧ Once ⟹ RC == 1`. This is FALSE:
    backward analysis proves "no future duplication" but NOT "no existing
    aliases". Uniqueness is established ONLY via the Uniqueness dimension
    (§3.4) or a fresh allocation (TF-3 §4), never derived from the substructural
    Consumption / Cardinality dimensions. The removal is recorded here as a
    doc-comment with NO theorem — there is no sound proposition to prove, and
    asserting one would be the unsound rule the removal eliminates. -/

end AimsProof
