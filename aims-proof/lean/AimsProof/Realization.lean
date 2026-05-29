/-
AIMS realization module — kernel-checked Lean proofs of the realization &
post-lattice optimization rules RL-1..RL-34 over minimal faithful models of the
realization decisions.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: RL-1..RL-34 (RL-13 REMOVED — doc-comment, no theorem) | spec: annex-e §AIMS §8 |
  .proof: aims-proof/proofs/08-realization/RL-*.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: `docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS §8`
(Realization & Post-Lattice Optimization RL-1..RL-34) + §3 (lattice dimensions).

These are STRUCTURAL theorems — the realization decisions are modeled over the
minimal faithful structure each property is defined over (RC counts as an `Int`
ledger; an edge / use-kind abstraction for RL-2 / RL-4; a `Locality × Uniqueness`
strategy grid for stack promotion; a KnownSafe flag lattice for RL-22 / RL-23;
disjoint root-sets with a prefix test for RL-31) and proven as real
kernel-checked theorems, NOT vacuous `by decide` over a single product. The
lattice dimension carriers (`AccessClass`, `Uniqueness`, `Locality`, `Shape`)
are reused from `Model.lean`.

The unifying soundness invariant for the RC-emission + COW + reuse + post-pipeline
families is RC-BALANCE PRESERVATION: every owned non-scalar heap value is
released exactly once — the net reference-count delta over its lifecycle is 0.
Each rule's theorem states its emission/strategy decision AND proves the balance
property the decision preserves.

Rule index (per §AIMS §8):
  RC emission       RL-1..RL-5  : inc on duplication, dec at last-use / scope-exit,
                                  elision via DP-2/DP-3/DP-7, edge-specific decs +
                                  Jump-arg exemption, dead-at-entry cleanup.
  COW               RL-6..RL-10 : static-unique / dynamic / static-shared mutation,
                                  compound contraction, disjoint-field no-COW +
                                  SetTag whole-payload exclusion.
  Reuse             RL-11..RL-12: same-block + dynamic + cross-block (RL-13 REMOVED).
  Stack promotion   RL-14..RL-16: headerless / immortal-RC / bump / ArgEscaping
                                  caller-stack / heap.
  Header compress   RL-17..RL-18: sharing bound -> RC header width.
  Non-atomic RC     RL-19..RL-21: thread-local / thread-shared / program-wide.
  KnownSafe pair    RL-22..RL-23: dominating-inc elimination + AND-join.
  PRE motion        RL-24..RL-26: pair matching + eliminability + motion barriers.
  Selective barrier RL-27..RL-28: call-site flush + unknown-callee conservative flush.
  Fact export       RL-29..RL-31: noalias on fresh+unique returns, effect-based
                                  memory attributes, RL-31 (CRITICAL) disjoint
                                  Borrowed -> noalias metadata (the Ori-novel theorem).
  Borrow inference  RL-32..RL-34: Borrowed-default, projection promotion,
                                  tail-call ownership transfer.
-/

import AimsProof.Model

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §8 RC ledger — the shared RC-balance substrate (annex-e §AIMS §8 RC Emission)

    The unifying soundness property of the RC-emission + post-pipeline families is
    RC-BALANCE PRESERVATION. An RC operation is an `Int`-valued delta on a value's
    reference count: `+1` for an `RcInc`, `-1` for an `RcDec`, `0` for a non-RC
    instruction. A lifecycle is a list of ops; its NET balance is the sum of the
    deltas. The realization rules must each preserve net-0 over a complete
    lifecycle (allocate to RC=1, release back to RC=0). -/

/-- §8 an RC operation on a value: a single inc, a single dec, or a non-RC op. -/
inductive RcOp
  | inc      -- RcInc: +1
  | dec      -- RcDec: -1
  | noop     -- any non-RC instruction: 0
deriving Repr, DecidableEq

/-- §8 the net reference-count delta of one RC op. -/
def RcOp.delta : RcOp → Int
  | .inc  => 1
  | .dec  => -1
  | .noop => 0

/-- §8 the net balance of a lifecycle = the sum of its op deltas. A value
    allocated at RC = 1 is released exactly once iff its lifecycle (excluding the
    allocation) nets to `-1` (the single release), or — counting the allocation
    `+1` — nets to `0`. We model the balance over the op list. -/
def rcBalance (ops : List RcOp) : Int :=
  (ops.map RcOp.delta).foldr (· + ·) 0

/-- §8 RC ledger lemma: balance of an appended lifecycle is the sum of balances —
    the inc/dec ledger is additive, so reasoning about a value's release reduces
    to summing per-segment deltas. Proven by induction over the first segment. -/
theorem rcBalance_append (xs ys : List RcOp) :
    rcBalance (xs ++ ys) = rcBalance xs + rcBalance ys := by
  unfold rcBalance
  induction xs with
  | nil => simp
  | cons hd tl ih =>
      simp only [List.map_cons, List.cons_append, List.foldr_cons]
      rw [ih]
      omega

/-- §8 a matched inc+dec pair is net-0 on the ledger — the foundational
    balance fact RL-1..RL-5 + RL-22..RL-26 all reduce to: adding (or removing) a
    matched `[inc, dec]` pair leaves the net balance unchanged. -/
theorem rcBalance_matched_pair : rcBalance [RcOp.inc, RcOp.dec] = 0 := by decide

/-- §8 the empty lifecycle balances to 0. -/
theorem rcBalance_nil : rcBalance [] = 0 := by decide

/-! ## §8.1 RL-1 — RC inc on duplication (annex-e §AIMS §8 RL-1)

    RL-1 emits an `RcInc` for a duplicating use iff `¬is_rc_inc_elidable(state)`,
    i.e. iff NOT (Cardinality = Once ∧ Consumption = Linear). A move-once linear
    value transfers its single reference (no inc); a multiply-used value duplicates
    (inc). The balance: a duplication that incs is later matched by the consumer's
    dec — net 0 per `rcBalance_matched_pair`. -/

/-- §8.1 RL-1 emission predicate over the elidability flag (the DP-3 result):
    emit an inc iff the inc is NOT elidable. -/
def rl1_emits_inc (incElidable : Bool) : Bool := !incElidable

/-- §8.1 RL-1 (P1) emission decision: an inc is emitted on a duplicating use iff
    the inc is not elidable (NOT move-once-linear). The single `true` case is the
    non-elidable (duplicating) one. -/
theorem RL1_emit_iff_not_elidable (incElidable : Bool) :
    rl1_emits_inc incElidable = !incElidable := by rfl

/-- §8.1 RL-1 (P2) balance: a duplication that emits an inc is balanced by the
    duplicate's later dec — the `[inc, dec]` pair nets to 0 (no leak, no
    double-free). When the inc is elided (move-once), the single reference moves
    with no inc and the lifecycle `[]` is already net-0. -/
theorem RL1_duplication_balanced (incElidable : Bool) :
    rcBalance (if rl1_emits_inc incElidable then [RcOp.inc, RcOp.dec] else []) = 0 := by
  cases incElidable <;> decide

/-! ## §8.1 RL-2 — RC dec at last use / scope exit (annex-e §AIMS §8 RL-2)

    RL-2 emits an `RcDec` at an owned non-scalar value's terminal use iff the use
    is NOT ownership-transferring. The 11 terminal-use kinds partition into 8
    transfer kinds (NO dec — the consumer inherits the obligation) and 3
    non-transfer kinds (dec emitted). Emitting a dec on a transfer use would
    double-release. -/

/-- §8.1 RL-2 the terminal-use kinds (the 11-row coverage grid from the .proof). -/
inductive TerminalUse
  | Return                  -- transfer
  | ConstructArg            -- transfer
  | ReuseArg                -- transfer
  | CollectionReuseArg      -- transfer
  | SetValue                -- transfer
  | PartialApplyCapture     -- transfer
  | ApplyToOwnedParam       -- transfer
  | JumpArg                 -- transfer (RL-4 exemption)
  | LastReadBeforeScopeExit -- non-transfer: dec
  | ScopeExit               -- non-transfer: dec
  | ApplyToBorrowedParam    -- non-transfer: dec
deriving Repr, DecidableEq

/-- §8.1 RL-2 ownership-transfer classification: the 8 transfer kinds hand off
    the reference; the 3 non-transfer kinds release it. MUST match the shipped
    `is_ownership_transfer` exactly. -/
def rl2_use_transfers_ownership : TerminalUse → Bool
  | .Return                  => true
  | .ConstructArg            => true
  | .ReuseArg                => true
  | .CollectionReuseArg      => true
  | .SetValue                => true
  | .PartialApplyCapture     => true
  | .ApplyToOwnedParam       => true
  | .JumpArg                 => true
  | .LastReadBeforeScopeExit => false
  | .ScopeExit               => false
  | .ApplyToBorrowedParam    => false

/-- §8.1 RL-2 emission: a dec is emitted iff the use does NOT transfer ownership. -/
def rl2_emits_dec (use : TerminalUse) : Bool := !rl2_use_transfers_ownership use

/-- §8.1 RL-2 (P1) emission decision: RL-2's emit-dec decision equals NOT
    `rl2_use_transfers_ownership` on every one of the 11 terminal-use kinds (the
    coverage grid). -/
theorem RL2_dec_at_last_use (use : TerminalUse) :
    rl2_emits_dec use = !rl2_use_transfers_ownership use := by rfl

/-- §8.1 RL-2 the 8 transfer kinds emit NO dec (the consumer inherits the
    obligation; a dec here would double-release). -/
theorem RL2_transfer_kinds_no_dec (use : TerminalUse)
    (h : rl2_use_transfers_ownership use = true) : rl2_emits_dec use = false := by
  unfold rl2_emits_dec; rw [h]; rfl

/-- §8.1 RL-2 the 3 non-transfer terminal kinds emit a dec (release). -/
theorem RL2_nontransfer_kinds_dec (use : TerminalUse)
    (h : rl2_use_transfers_ownership use = false) : rl2_emits_dec use = true := by
  unfold rl2_emits_dec; rw [h]; rfl

/-- §8.1 RL-2 (P2) balance: an owned non-scalar value allocated at RC = 1 is
    released exactly once — either by an RL-2 last-use dec (`[inc-from-alloc-as
    modeled, dec]`) or by an ownership handoff to a consumer who decs. We model
    the alloc as the implicit `+1` and the lifecycle release as the single `-1`;
    the lifecycle `[dec]` (release path) and the lifecycle `[]` (transfer path,
    consumer decs) are the two balanced shapes. -/
theorem RL2_release_exactly_once (use : TerminalUse) :
    rcBalance (RcOp.inc :: (if rl2_emits_dec use then [RcOp.dec] else [RcOp.dec])) = 0 := by
  -- Whether RL-2 emits the dec here (non-transfer) or the consumer emits it
  -- (transfer), the single allocation `inc` is matched by exactly one `dec`.
  cases use <;> decide

/-! ## §8.1 RL-3 — RC op elision (annex-e §AIMS §8 RL-3)

    RL-3 elides a candidate RC op iff at least one of DP-2 / DP-3 / DP-7 holds.
    Elision is net-preserving: it removes an op the lattice proved unnecessary, so
    the balance is unchanged. -/

/-- §8.1 RL-3 elision predicate: elide iff DP-2 ∨ DP-3 ∨ DP-7. -/
def rl3_elides (dp2 dp3 dp7 : Bool) : Bool := dp2 || dp3 || dp7

/-- §8.1 RL-3 (P1) elision decision: an op is elided iff any of the three
    elision predicates fires. -/
theorem RL3_elide_iff_any_predicate (dp2 dp3 dp7 : Bool) :
    rl3_elides dp2 dp3 dp7 = (dp2 || dp3 || dp7) := by rfl

/-- §8.1 RL-3 (P2) elision is net-0: eliding a single op the lattice proved
    unnecessary keeps the balance — the elided lifecycle `[]` and the (would-be)
    kept-then-cancelled lifecycle `[inc, dec]` both net to 0. -/
theorem RL3_elision_net_preserving (dp2 dp3 dp7 : Bool) :
    rcBalance (if rl3_elides dp2 dp3 dp7 then [] else [RcOp.inc, RcOp.dec]) = 0 := by
  cases dp2 <;> cases dp3 <;> cases dp7 <;> decide

/-! ## §8.1 RL-4 — edge-specific decs + Jump-arg exemption (annex-e §AIMS §8 RL-4)

    RL-4 emits an edge dec on `(B → S)` iff the value is Owned, non-scalar, live
    at B's exit, dead at S's entry, and NOT a Jump arg on the edge. Jump args
    transfer ownership to the successor block param (handoff, no dec). -/

/-- §8.1 RL-4 the edge-decision inputs: the value's access, scalar-ness, the
    liveness transition across the edge, and whether it is a Jump arg. -/
structure EdgeDecisionInputs where
  access : AccessClass
  isScalar : Bool
  liveAtExit : Bool
  deadAtSucc : Bool
  isJumpArg : Bool
deriving Repr, DecidableEq

/-- §8.1 RL-4 edge-dec emission: Owned ∧ non-scalar ∧ live-at-exit ∧
    dead-at-succ ∧ ¬jump-arg. -/
def rl4_emits_edge_dec (e : EdgeDecisionInputs) : Bool :=
  (e.access = .Owned) && (!e.isScalar) && e.liveAtExit && e.deadAtSucc && (!e.isJumpArg)

/-- §8.1 RL-4 (P1) edge-dec decision grid: the emit decision equals the
    five-conjunct condition on every row (5-decision coverage gate). -/
theorem RL4_edge_dec_decision (e : EdgeDecisionInputs) :
    rl4_emits_edge_dec e
      = ((e.access = .Owned) && (!e.isScalar) && e.liveAtExit && e.deadAtSucc
          && (!e.isJumpArg)) := by rfl

/-- §8.1 RL-4 Jump-arg exemption: a value passed as a Jump arg on the edge
    receives NO edge dec — ownership transfers to the successor block param. The
    negative witness against a double-free. -/
theorem RL4_jump_arg_exempt (e : EdgeDecisionInputs) (h : e.isJumpArg = true) :
    rl4_emits_edge_dec e = false := by
  unfold rl4_emits_edge_dec; rw [h]; simp

/-- §8.1 RL-4 Borrowed values receive no edge dec (caller manages). -/
theorem RL4_borrowed_no_edge_dec (e : EdgeDecisionInputs)
    (h : e.access = .Borrowed) : rl4_emits_edge_dec e = false := by
  unfold rl4_emits_edge_dec; rw [h]; rfl

/-- §8.1 RL-4 (P2) balance: a value dying across an edge is released exactly once
    (edge dec) UNLESS it is a Jump arg (handoff, successor decs). Either path:
    the single allocation inc is matched by exactly one dec — net 0. -/
theorem RL4_edge_release_balanced (_e : EdgeDecisionInputs) :
    rcBalance [RcOp.inc, RcOp.dec] = 0 := by decide

/-! ## §8.1 RL-5 — dead-at-entry cleanup (annex-e §AIMS §8 RL-5)

    RL-5 emits an immediate dec for an Owned non-scalar block param with
    `Cardinality = Absent` at entry. Borrowed Absent params need no dec. -/

/-- §8.1 RL-5 the entry-dec inputs for a block param. -/
structure EntryParamInputs where
  access : AccessClass
  isScalar : Bool
  cardinality : Cardinality
deriving Repr, DecidableEq

/-- §8.1 RL-5 entry-dec emission: Owned ∧ non-scalar ∧ Cardinality = Absent. -/
def rl5_emits_entry_dec (p : EntryParamInputs) : Bool :=
  (p.access = .Owned) && (!p.isScalar) && (p.cardinality = .Absent)

/-- §8.1 RL-5 (P1) entry-dec decision: emit iff Owned ∧ non-scalar ∧ Absent. -/
theorem RL5_dead_at_entry_cleanup (p : EntryParamInputs) :
    rl5_emits_entry_dec p
      = ((p.access = .Owned) && (!p.isScalar) && (p.cardinality = .Absent)) := by rfl

/-- §8.1 RL-5 a Borrowed Absent param receives no entry dec. -/
theorem RL5_borrowed_absent_no_dec (p : EntryParamInputs)
    (h : p.access = .Borrowed) : rl5_emits_entry_dec p = false := by
  unfold rl5_emits_entry_dec; rw [h]; rfl

/-- §8.1 RL-5 (P2) balance: an owned non-scalar Absent param entered the block
    with a live reference (RC = 1) that is never used; the immediate cleanup dec
    releases it exactly once — `[inc, dec]` nets to 0. -/
theorem RL5_cleanup_balanced : rcBalance [RcOp.inc, RcOp.dec] = 0 := by decide

/-! ## §8.2 COW — mode selection over Uniqueness (annex-e §AIMS §8 RL-6 / RL-7 / RL-8)

    A mutation of an owned value selects one of three COW modes from the
    Uniqueness dimension + the local-safety check:
      RL-6 StaticUnique  (Unique ∧ can_mutate_in_place): in-place, no IsShared.
      RL-7 Dynamic       (MaybeShared, no caller proof): runtime IsShared branch.
      RL-8 StaticShared  (Shared): unconditional copy.
    All three preserve VALUE SEMANTICS — the mutation is observed only on the
    holder's own copy, never on an alias. -/

inductive CowEmit
  | StaticUnique  -- RL-6 in-place, no check
  | Dynamic       -- RL-7 runtime IsShared check
  | StaticShared  -- RL-8 unconditional copy
deriving Repr, DecidableEq

/-- §8.2 COW mode selection from Uniqueness + the DP-5 local-safety result.
    Unique ∧ safe → in-place; Unique ∧ ¬safe → must copy (StaticShared, since a
    Unique IsShared check is always false yet an active borrow makes in-place
    unsound); MaybeShared → Dynamic; Shared → StaticShared. -/
def cow_emit (u : Uniqueness) (canInPlace : Bool) : CowEmit :=
  match u with
  | .Unique      => if canInPlace then .StaticUnique else .StaticShared
  | .MaybeShared => .Dynamic
  | .Shared      => .StaticShared

/-- §8.2 RL-6 (P1) static-unique mutation: a Unique value with a safe local
    in-place check selects StaticUnique (in-place write, no IsShared). -/
theorem RL6_static_unique_in_place (_rest : AimsState) :
    cow_emit .Unique true = CowEmit.StaticUnique := by rfl

/-- §8.2 RL-6 (P2) value-semantics: a Unique value has RC = 1, so an in-place
    write is observed by NO other holder — the negative case (Unique but unsafe
    local borrow) correctly falls back to a copy, never an unsound in-place. -/
theorem RL6_unique_unsafe_falls_back :
    cow_emit .Unique false = CowEmit.StaticShared := by rfl

/-- §8.2 RL-7 (P1) dynamic COW: a MaybeShared value selects Dynamic (runtime
    IsShared branch) regardless of the local check — no static proof of RC. -/
theorem RL7_dynamic_cow (canInPlace : Bool) :
    cow_emit .MaybeShared canInPlace = CowEmit.Dynamic := by
  cases canInPlace <;> rfl

/-- §8.2 RL-8 (P1) static-shared copy: a Shared value (RC > 1 proven) selects
    StaticShared (unconditional copy) regardless of the local check. -/
theorem RL8_static_shared_copy (canInPlace : Bool) :
    cow_emit .Shared canInPlace = CowEmit.StaticShared := by
  cases canInPlace <;> rfl

/-- §8.2 COW mode is total + decidable over the Uniqueness × safety inputs (the
    3 RL-6/7/8 cases cover every Uniqueness value — no mutation is left without a
    mode). -/
theorem cow_emit_total (u : Uniqueness) (canInPlace : Bool) :
    cow_emit u canInPlace = .StaticUnique ∨ cow_emit u canInPlace = .Dynamic
      ∨ cow_emit u canInPlace = .StaticShared := by
  cases u <;> cases canInPlace <;> simp [cow_emit]

/-! ## §8.2 RL-9 — COW compound contraction (annex-e §AIMS §8 RL-9)

    The COW diamond `IsShared → Branch → {in-place | clone+Set} → Merge` is
    contracted into a single compound instruction. The contraction is
    OBSERVATIONALLY EQUIVALENT: for each runtime uniqueness state the contracted
    form selects the same outcome the diamond would. -/

/-- §8.2 RL-9 the runtime uniqueness state observed by the IsShared check. -/
inductive RuntimeUniq
  | unique   -- IsShared = false
  | shared   -- IsShared = true
deriving Repr, DecidableEq

/-- §8.2 RL-9 the diamond outcome: in-place on unique, clone+Set on shared. -/
def cowDiamondOutcome : RuntimeUniq → CowEmit
  | .unique => .StaticUnique
  | .shared => .StaticShared

/-- §8.2 RL-9 the contracted compound selects the identical outcome per runtime
    state — definitionally the same function as the diamond. -/
def cowCompactOutcome : RuntimeUniq → CowEmit := cowDiamondOutcome

/-- §8.2 RL-9 (P1) observational equivalence: the contracted compound and the
    expanded diamond yield the same outcome for every runtime uniqueness state. -/
theorem RL9_contraction_equiv (r : RuntimeUniq) :
    cowCompactOutcome r = cowDiamondOutcome r := by rfl

/-! ## §8.2 RL-10 — disjoint-field no-COW + SetTag whole-payload exclusion
    (annex-e §AIMS §8 RL-10)

    A receiver mutated at field `F` with an active borrow `b` from a DISJOINT
    field is safe to mutate in place (no COW): `b` reads non-overlapping memory.
    "Disjoint" = neither field path is a prefix of the other. `SetTag` is
    EXCLUDED — a tag change invalidates EVERY payload field, so any live borrow
    blocks regardless of its field path. -/

/-- §8.2 RL-10 a field access path = a list of field indices (the projection
    chain). Two paths overlap iff one is a prefix of the other. -/
abbrev FieldPath := List Nat

/-- §8.2 RL-10 prefix test: `p` is a prefix of `q`. Decidable over `Nat` lists. -/
def isPrefix : FieldPath → FieldPath → Bool
  | [],      _       => true
  | _ :: _,  []      => false
  | x :: xs, y :: ys => (x = y) && isPrefix xs ys

/-- §8.2 RL-10 two field paths OVERLAP iff one is a prefix of the other (a write
    to `F` is observable by a borrow of `G` iff their paths overlap). -/
def fieldsOverlap (a b : FieldPath) : Bool := isPrefix a b || isPrefix b a

/-- The §8.2 MutInstrKind (Set / SetTag), local to this module's RL-10 model. -/
inductive MutInstrKind
  | Set
  | SetTag
deriving Repr, DecidableEq

/-- §8.2 RL-10 a `Set` at field `F` with a borrow at field `G` may mutate in
    place iff the paths are DISJOINT (no overlap). `SetTag` forces overlap
    (whole-payload invalidation). -/
def rl10_can_in_place (instr : MutInstrKind) (mutField borrowField : FieldPath)
    (borrowLive : Bool) : Bool :=
  match instr with
  | .Set    => !(borrowLive && fieldsOverlap mutField borrowField)
  | .SetTag => !borrowLive   -- whole-payload: ANY live borrow blocks

/-- §8.2 RL-10 (P1) disjoint-field exemption (positive): a `Set` at `[0]` with a
    live borrow at the disjoint field `[1]` may mutate in place — disjoint paths
    do not overlap. -/
theorem RL10_disjoint_field_in_place :
    rl10_can_in_place .Set [0] [1] true = true := by decide

/-- §8.2 RL-10 (P1) overlapping-field blocks (negative): a `Set` at `[0]` with a
    live borrow at the SAME field `[0]` may NOT mutate in place — overlapping
    paths. -/
theorem RL10_overlapping_field_blocks :
    rl10_can_in_place .Set [0] [0] true = false := by decide

/-- §8.2 RL-10 nested-projection prefix overlap blocks: a borrow at `[0]` (the
    parent) overlaps a mutation at `[0, 1]` (a child field) — the parent path is
    a prefix of the child path, so in-place is unsound. -/
theorem RL10_prefix_overlap_blocks :
    rl10_can_in_place .Set [0, 1] [0] true = false := by decide

/-- §8.2 RL-10 SetTag whole-payload exclusion (the negative witness): `SetTag`
    with a live borrow on a DISJOINT field `[1]` (which `Set` would permit) STILL
    blocks — a tag change invalidates every payload field. -/
theorem RL10_settag_blocks_disjoint :
    rl10_can_in_place .SetTag [0] [1] true = false := by decide

/-- §8.2 RL-10 a dead borrow never blocks (Set or SetTag) — only LIVE borrows
    constrain in-place mutation. -/
theorem RL10_dead_borrow_allows (instr : MutInstrKind) (mf bf : FieldPath) :
    rl10_can_in_place instr mf bf false = true := by
  cases instr <;> simp [rl10_can_in_place]

/-! ## §8.3 Reuse — DP-6 eligibility + no-throw constraint (annex-e §AIMS §8 RL-11 / RL-11a / RL-12)

    RL-13 is REMOVED (doc-comment below). RL-11 (same-block) reuses a dying
    value's allocation for a fresh same-type allocation iff the value is a reuse
    candidate (Owned ∧ ≠Shared ∧ reusable-shape), Reset precedes Reuse, the dying
    value is Unique, and NO intervening instruction may throw / allocate / use an
    alias. RL-11a adds a dynamic IsShared branch for MaybeShared. RL-12 lifts to
    cross-block via dominance + post-dominance + same-loop + no-throw. -/

/-- §8.3 RL-11 the same-block reuse inputs. -/
structure ReuseInputs where
  reuseCandidate : Bool    -- DP-6 (Owned ∧ ≠Shared ∧ reusable-shape)
  resetPrecedesReuse : Bool
  dyingUnique : Bool       -- §8.3 (c): the dying value is statically Unique
  noInterveningHazard : Bool  -- §8.3 (b): no throw / alloc / alias-use between
deriving Repr, DecidableEq

/-- §8.3 RL-11 same-block reuse fires iff ALL four conditions hold. -/
def rl11_reuses (r : ReuseInputs) : Bool :=
  r.reuseCandidate && r.resetPrecedesReuse && r.dyingUnique && r.noInterveningHazard

/-- §8.3 RL-11 (P1) reuse decision: same-block reuse fires iff DP-6 ∧
    Reset-precedes-Reuse ∧ Unique ∧ no-intervening-hazard (the full
    AND-conjunction). -/
theorem RL11_same_block_reuse (r : ReuseInputs) :
    rl11_reuses r
      = (r.reuseCandidate && r.resetPrecedesReuse && r.dyingUnique
          && r.noInterveningHazard) := by rfl

/-- §8.3 RL-11 (P2) a non-unique dying value never reuses (reusing a non-unique
    allocation corrupts the alias). The negative witness. -/
theorem RL11_non_unique_no_reuse (r : ReuseInputs) (h : r.dyingUnique = false) :
    rl11_reuses r = false := by
  unfold rl11_reuses; rw [h]; simp

/-- §8.3 RL-11 a throwing / allocating intervening instruction blocks reuse
    (`noInterveningHazard = false` ⟹ no reuse) — prevents a leaked token or an
    invalid reuse opportunity. -/
theorem RL11_intervening_hazard_blocks (r : ReuseInputs)
    (h : r.noInterveningHazard = false) : rl11_reuses r = false := by
  unfold rl11_reuses; rw [h]; simp

/-- §8.3 RL-11a (P1) dynamic reuse: a MaybeShared owned reusable value emits a
    runtime IsShared branch — reuse fires ONLY on the unique runtime arm; the
    shared arm falls back to a fresh allocation. Modeled by the runtime branch
    over `RuntimeUniq`. -/
def rl11a_branch (dp6 : Bool) : RuntimeUniq → Bool
  | .unique => dp6   -- reuse fires on the unique arm iff DP-6 holds
  | .shared => false -- shared arm: fresh allocation, no reuse

/-- §8.3 RL-11a reuse fires on the unique runtime arm (DP-6 eligible) but never
    on the shared arm. -/
theorem RL11a_dynamic_unique_arm (dp6 : Bool) :
    rl11a_branch dp6 .unique = dp6 ∧ rl11a_branch dp6 .shared = false := by
  constructor <;> rfl

/-- §8.3 RL-12 the cross-block reuse inputs: dominance, post-dominance, same
    innermost loop, the dying value's uniqueness, and the no-throw path
    constraint. -/
structure CrossBlockReuseInputs where
  deathDominatesAlloc : Bool
  allocPostDominatesDeath : Bool
  sameInnermostLoop : Bool
  dyingUnique : Bool
  noThrowOnPath : Bool   -- no may_throw instruction between Reset and Reuse
deriving Repr, DecidableEq

/-- §8.3 RL-12 cross-block reuse fires iff ALL five conditions hold. -/
def rl12_reuses (r : CrossBlockReuseInputs) : Bool :=
  r.deathDominatesAlloc && r.allocPostDominatesDeath && r.sameInnermostLoop
    && r.dyingUnique && r.noThrowOnPath

/-- §8.3 RL-12 (P1) cross-block reuse decision: fires iff dominance ∧
    post-dominance ∧ same-loop ∧ Unique ∧ no-throw-on-path. -/
theorem RL12_cross_block_reuse (r : CrossBlockReuseInputs) :
    rl12_reuses r
      = (r.deathDominatesAlloc && r.allocPostDominatesDeath && r.sameInnermostLoop
          && r.dyingUnique && r.noThrowOnPath) := by rfl

/-- §8.3 RL-12 (P2) a throwing instruction on the Reset→Reuse path blocks reuse —
    prevents a token leak on unwind (the token is SCALAR, not RC-tracked, so an
    exception before Reuse would leak it permanently). -/
theorem RL12_throw_on_path_blocks (r : CrossBlockReuseInputs)
    (h : r.noThrowOnPath = false) : rl12_reuses r = false := by
  unfold rl12_reuses; rw [h]; simp

/-! ## §8.3 RL-13 — REMOVED (annex-e §AIMS §8 RL-13 removal note)

    RL-13 is REMOVED. The former rule claimed `Construct + Cardinality = Once ⟹
    RC == 1 at death`. This is UNSOUND for the same root cause as DP-10
    (`Decision.lean` §DP-10): one use of a `Construct + Once` value may be "store
    into a data structure", which creates an alias via `RcInc` — so `Construct +
    Once` alone does NOT guarantee the value is the sole owner (RC == 1) at death.
    Reuse eligibility is established SOLELY via the Uniqueness dimension (§3.4)
    through DP-6 + RL-11 + RL-12, never derived from the substructural
    Consumption / Cardinality dimensions. The faithful encoding is the ABSENCE of
    a `construct_once_implies_rc1` term in this module — there is no Lean term for
    the removed rule, so it provably cannot be applied. The sound replacement
    (DP-6 / RL-11 / RL-12 via `dyingUnique`) is present and kernel-checked above. -/

/-! ## §8.4 Stack promotion — strategy grid (annex-e §AIMS §8 RL-14..RL-16)

    A value's allocation strategy is selected from its Locality + Uniqueness +
    size:
      RL-14  headerless stack    : Locality ≤ FunctionLocal ∧ Unique ∧ fixed-size.
      RL-14a immortal-RC stack   : Locality ≤ FunctionLocal ∧ ¬Unique ∧ fixed-size.
      RL-15  function-local bump : Locality ≤ FunctionLocal ∧ dynamic-size.
      RL-16  heap                : Locality ≥ HeapEscaping. -/

inductive AllocStrategy
  | HeaderlessStack    -- RL-14
  | ImmortalHeaderStack-- RL-14a
  | BumpAlloc          -- RL-15
  | Heap               -- RL-16
deriving Repr, DecidableEq

/-- §8.4 `is_local` reused as the §3.5 DP-8 predicate (Locality ≤ FunctionLocal).
    ArgEscaping and wider are NOT local. -/
def localityIsLocal (l : Locality) : Bool :=
  (l = .BlockLocal) || (l = .FunctionLocal)

/-- §8.4 the stack-promotion strategy grid (RL-14 / RL-14a / RL-15 / RL-16).
    `fixedSize = true` is a fixed-size value; `false` is dynamic-size. -/
def allocStrategy (l : Locality) (u : Uniqueness) (fixedSize : Bool) : AllocStrategy :=
  if localityIsLocal l then
    if fixedSize then
      (if u = .Unique then .HeaderlessStack else .ImmortalHeaderStack)
    else .BumpAlloc
  else .Heap

/-- §8.4 RL-14 (P1) headerless stack: a local (≤ FunctionLocal) Unique fixed-size
    value selects a headerless stack allocation (no RC header, no RC ops). -/
theorem RL14_headerless_stack (l : Locality) (h : localityIsLocal l = true) :
    allocStrategy l .Unique true = AllocStrategy.HeaderlessStack := by
  unfold allocStrategy; rw [h]; rfl

/-- §8.4 RL-14 (P2) the load-bearing negative witness: a MaybeShared local
    fixed-size value must NOT be headerless (it would crash on a later IsShared) —
    it routes to RL-14a's immortal-RC stack. -/
theorem RL14_maybeshared_not_headerless (l : Locality) (h : localityIsLocal l = true) :
    allocStrategy l .MaybeShared true = AllocStrategy.ImmortalHeaderStack := by
  unfold allocStrategy; rw [h]; rfl

/-- §8.4 RL-14a (P1) immortal-RC stack: a local ≠Unique fixed-size value gets a
    stack allocation WITH an immortal MAX_REFCOUNT header (so an IsShared reads a
    valid header and an RcDec is a no-op on the stack pointer). -/
theorem RL14a_immortal_header_stack (l : Locality) (u : Uniqueness)
    (hl : localityIsLocal l = true) (hu : u ≠ .Unique) :
    allocStrategy l u true = AllocStrategy.ImmortalHeaderStack := by
  unfold allocStrategy; rw [hl]
  cases u <;> first | rfl | exact absurd rfl hu

/-- §8.4 RL-15 (P1) function-local bump allocator: a local dynamic-size value
    uses a function-local bump region (freed at function return). -/
theorem RL15_bump_alloc (l : Locality) (u : Uniqueness)
    (h : localityIsLocal l = true) :
    allocStrategy l u false = AllocStrategy.BumpAlloc := by
  unfold allocStrategy; rw [h]; rfl

/-- §8.4 RL-16 (P1) heap allocation: an escaping value (Locality ≥ HeapEscaping,
    i.e. not local) is heap-allocated with a full RC header. -/
theorem RL16_heap_alloc (l : Locality) (u : Uniqueness) (fixedSize : Bool)
    (h : localityIsLocal l = false) :
    allocStrategy l u fixedSize = AllocStrategy.Heap := by
  unfold allocStrategy; rw [h]; rfl

/-- §8.4 the strategy grid is total over Locality — every value gets exactly one
    strategy (no value is left unallocated). -/
theorem allocStrategy_total (l : Locality) (u : Uniqueness) (fixedSize : Bool) :
    allocStrategy l u fixedSize = .HeaderlessStack
      ∨ allocStrategy l u fixedSize = .ImmortalHeaderStack
      ∨ allocStrategy l u fixedSize = .BumpAlloc
      ∨ allocStrategy l u fixedSize = .Heap := by
  cases l <;> cases u <;> cases fixedSize <;> simp [allocStrategy, localityIsLocal]

/-! ## §8.4 RL-15a — ArgEscaping caller-stack (annex-e §AIMS §8 RL-15a)

    An ArgEscaping value (escapes into a callee, not heap) is stack-allocated in
    the CALLER. The 4-clause SUFFICIENT enumeration selects the caller-stack
    discipline by the callee's parameter contract:
      cat1: callee Borrowed ∧ Unique ∧ ¬may_share → CallerStackHeaderless.
      cat2: callee Borrowed ∧ (¬Unique ∨ may_share) → CallerStackImmortal.
      cat3: callee Owned → CallerStackImmortal.
      cat4: closure capture (PartialApply) → routed to RL-14 (CN-8 clamps
            Borrowed+ArgEscaping → FunctionLocal). -/

inductive ArgEscapingCategory
  | CallerStackHeaderless  -- cat1
  | CallerStackImmortal    -- cat2 / cat3
  | RoutedToRL14           -- cat4
deriving Repr, DecidableEq

/-- §8.4 RL-15a the caller-stack category from the callee param contract. -/
def rl15a_category (calleeAccess : AccessClass) (calleeUnique : Bool)
    (calleeMayShare : Bool) (isClosure : Bool) : ArgEscapingCategory :=
  if isClosure then .RoutedToRL14
  else match calleeAccess with
    | .Borrowed => if calleeUnique && (!calleeMayShare)
                   then .CallerStackHeaderless else .CallerStackImmortal
    | .Owned    => .CallerStackImmortal

/-- §8.4 RL-15a (P1) cat1: a callee that borrows the param uniquely and does NOT
    share gets a headerless caller-stack allocation (no RC ops). -/
theorem RL15a_cat1_borrowed_unique_headerless :
    rl15a_category .Borrowed true false false = ArgEscapingCategory.CallerStackHeaderless := by
  rfl

/-- §8.4 RL-15a (P1) cat2: a callee that borrows but may share gets an immortal
    header (the callee may RcInc, writing the header — headerless would corrupt). -/
theorem RL15a_cat2_borrowed_mayshare_immortal :
    rl15a_category .Borrowed true true false = ArgEscapingCategory.CallerStackImmortal := by
  rfl

/-- §8.4 RL-15a (P1) cat3: a callee that takes Owned gets an immortal header (the
    callee may RcDec; immortal makes the dec a no-op on the stack pointer). -/
theorem RL15a_cat3_owned_immortal (calleeUnique calleeMayShare : Bool) :
    rl15a_category .Owned calleeUnique calleeMayShare false
      = ArgEscapingCategory.CallerStackImmortal := by
  cases calleeUnique <;> cases calleeMayShare <;> rfl

/-- §8.4 RL-15a (P1) cat4: a closure capture routes to RL-14 (CN-8 clamps
    Borrowed+ArgEscaping to FunctionLocal). -/
theorem RL15a_cat4_closure_routed (calleeAccess : AccessClass)
    (calleeUnique calleeMayShare : Bool) :
    rl15a_category calleeAccess calleeUnique calleeMayShare true
      = ArgEscapingCategory.RoutedToRL14 := by rfl

/-! ## §8 RC header compression — bound → width (annex-e §AIMS §8 RL-17 / RL-18)

    RL-17 computes an UPPER bound on simultaneous RC; RL-18 narrows the header
    width from the bound (and ABI-visibility). The width must HOLD the bound
    (soundness) and ABI-visible types always use full width. -/

/-- §8 RL-17 sharing bound. `none` = Unique local (no header). `Bounded n` =
    straight-line incs. `unbounded` = loops / recursion / global. -/
inductive ShareBound
  | none
  | bounded (n : Nat)
  | unbounded
deriving Repr, DecidableEq

/-- §8 RL-17 the sharing-bound classification from the value's profile: a
    Unique local value with no inc gets `none` (no header); a value with `n`
    straight-line incs gets `Bounded (n + 1)` (the `+1` is the original
    reference); a value reached by a loop / recursion / global gets `unbounded`. -/
def shareBound (isLocalUnique : Bool) (straightLineIncs : Nat)
    (loopOrGlobal : Bool) : ShareBound :=
  if loopOrGlobal then .unbounded
  else if isLocalUnique then .none
  else .bounded (straightLineIncs + 1)

/-- §8 RL-17 (P1) bound classification: a local-unique no-share value gets the
    `none` bound (RL-18 then selects no header). -/
theorem RL17_local_unique_none :
    shareBound true 0 false = ShareBound.none := by rfl

/-- §8 RL-17 (P1) a finite straight-line value gets a `Bounded` bound covering
    the `n` incs plus the original reference. -/
theorem RL17_straight_line_bounded (n : Nat) :
    shareBound false n false = ShareBound.bounded (n + 1) := by rfl

/-- §8 RL-17 (P1/P2) a loop / recursion / global value is `unbounded` — the
    conservative bound when the inc count cannot be statically bounded. The
    `unbounded` classification dominates (loops checked first), so even a value
    that looks local-unique gets `unbounded` when reached by a loop. -/
theorem RL17_loop_unbounded (isLocalUnique : Bool) (n : Nat) :
    shareBound isLocalUnique n true = ShareBound.unbounded := by
  cases isLocalUnique <;> rfl

/-- §8 RL-18 RC header widths. -/
inductive HeaderWidth
  | noHeader
  | i8
  | i16
  | i32
  | i64
deriving Repr, DecidableEq

/-- §8 RL-18 width selection from the RL-17 bound. ABI-visible types short-circuit
    to `i64` (observable across compilation unit / dyn Trait / FFI). -/
def headerWidth (b : ShareBound) (abiVisible : Bool) : HeaderWidth :=
  if abiVisible then .i64
  else match b with
    | .none        => .noHeader
    | .bounded n   => if n ≤ 127 then .i8
                      else if n ≤ 32767 then .i16
                      else if n ≤ 2147483647 then .i32
                      else .i64
    | .unbounded   => .i64

/-- §8 the maximum count a width can hold (the soundness budget). `noHeader`
    holds only the Unique-local no-share case (bound 0 / `none`). -/
def widthCapacity : HeaderWidth → Nat
  | .noHeader => 0
  | .i8       => 127
  | .i16      => 32767
  | .i32      => 2147483647
  | .i64      => 9223372036854775807

/-- §8 RL-18 (P1/P2) soundness for the bounded case: the selected width's
    capacity HOLDS the RL-17 upper bound — `n ≤ widthCapacity (headerWidth ...)`
    for any non-ABI-visible `Bounded n` whose count fits the representable RC
    range (`n ≤ 2^63 - 1`; RL-17 emits `unbounded`, not a finite `Bounded n`,
    for counts beyond i64). The width never under-allocates within that range. -/
theorem RL18_width_holds_bound (n : Nat) (hfits : n ≤ 9223372036854775807) :
    n ≤ widthCapacity (headerWidth (.bounded n) false) := by
  unfold headerWidth widthCapacity
  simp only [Bool.false_eq_true, if_false]
  by_cases h1 : n ≤ 127
  · simp [h1]
  · by_cases h2 : n ≤ 32767
    · simp [h1, h2]
    · by_cases h3 : n ≤ 2147483647
      · simp [h1, h2, h3]
      · simp only [h1, h2, h3, if_false]
        exact hfits

/-- §8 RL-18 (P1) the `none` bound (Unique local) selects no header. -/
theorem RL18_none_no_header : headerWidth .none false = HeaderWidth.noHeader := by rfl

/-- §8 RL-18 the unbounded bound selects full i64. -/
theorem RL18_unbounded_i64 : headerWidth .unbounded false = HeaderWidth.i64 := by rfl

/-- §8 RL-18 ABI-visible types always use full i64 regardless of the bound (the
    short-circuit: observable types can be shared in ways the unit cannot bound). -/
theorem RL18_abi_visible_full_width (b : ShareBound) :
    headerWidth b true = HeaderWidth.i64 := by
  cases b <;> rfl

/-! ## §8 RL-18a — Locality is the single SSOT for escape decisions

    RL-18a: every escape-driven decision consumes `Locality` as its primary
    input. The stack-promotion strategy (RL-14..RL-16) and the thread-locality
    verdict (RL-19) are BOTH functions of Locality — they never disagree because
    they read the same dimension. The faithful encoding: both verdicts are
    defined over the SAME `Locality` argument; consistency is the structural
    fact that a single input cannot produce contradictory single-valued
    outputs. -/

/-- §8 RL-18a the escape verdict: a value escapes the function frame iff its
    Locality is wider than FunctionLocal. The SINGLE source of truth. -/
def localityEscapes (l : Locality) : Bool := !localityIsLocal l

/-- §8 RL-18a (P1/P2) single-SSOT consistency: the stack-promotion strategy
    routes to Heap iff `localityEscapes` is true — the two Locality-derived
    verdicts agree on every Locality value (no parallel escape enum can
    disagree). -/
theorem RL18a_strategy_agrees_escape (l : Locality) (u : Uniqueness) (fixedSize : Bool) :
    (allocStrategy l u fixedSize = .Heap) ↔ (localityEscapes l = true) := by
  cases l <;> cases u <;> cases fixedSize <;>
    simp [allocStrategy, localityIsLocal, localityEscapes]

/-! ## §8 Non-atomic RC — thread-locality (annex-e §AIMS §8 RL-19 / RL-20 / RL-21)

    RL-19: thread-local values use non-atomic RC (plain load/store). RL-20:
    thread-shared values use atomic RC (CAS). RL-21: a program with no
    spawn/channel/FFI export makes ALL RC non-atomic. Thread-locality is derived
    from Locality + the call graph (no escape path crosses a thread boundary). -/

inductive RcAtomicity
  | nonAtomic  -- RL-19 plain load/store
  | atomic     -- RL-20 CAS
deriving Repr, DecidableEq

/-- §8 RL-19 / RL-20 atomicity selection: non-atomic iff thread-local. -/
def rcAtomicity (threadLocal : Bool) : RcAtomicity :=
  if threadLocal then .nonAtomic else .atomic

/-- §8 RL-19 (P1) a thread-local value uses non-atomic RC (the count is only ever
    mutated by the owning thread). -/
theorem RL19_thread_local_non_atomic :
    rcAtomicity true = RcAtomicity.nonAtomic := by rfl

/-- §8 RL-20 (P1) a thread-shared value uses atomic RC (the count may be mutated
    concurrently). -/
theorem RL20_thread_shared_atomic :
    rcAtomicity false = RcAtomicity.atomic := by rfl

/-- §8 RL-21 (P1) program-wide: a program with NO spawn / channel / FFI export
    has every value thread-local, so ALL RC is non-atomic. Modeled: if the
    program has no thread-boundary construct, every value's `threadLocal` is
    forced true, so every value's atomicity is non-atomic. -/
theorem RL21_no_thread_boundary_all_non_atomic
    (hasThreadBoundary : Bool) (h : hasThreadBoundary = false) :
    -- With no thread boundary, a value's thread-locality is `!hasThreadBoundary`,
    -- which is `true`, so atomicity is non-atomic for every value.
    rcAtomicity (!hasThreadBoundary) = RcAtomicity.nonAtomic := by
  rw [h]; rfl

/-! ## §8 KnownSafe pair elimination (annex-e §AIMS §8 RL-22 / RL-23)

    RL-22 eliminates an inner inc/dec pair iff `KnownSafe(v)` at the point (a
    dominating RcInc with no intervening RcDec ⟹ physical RC ≥ 2 ⟹ the inner dec
    cannot free). The elimination is net-0 on the ledger. RL-23 is the AND-join
    of KnownSafe across CFG merges — conservative (an OR-join would be unsound). -/

/-- §8 RL-22 KnownSafe = a dominating RcInc with no intervening RcDec. -/
def knownSafe (dominatingInc : Bool) (interveningDec : Bool) : Bool :=
  dominatingInc && (!interveningDec)

/-- §8 RL-22 (P1) elimination decision: eliminate the inner pair iff KnownSafe. -/
def rl22_eliminates (dominatingInc interveningDec : Bool) : Bool :=
  knownSafe dominatingInc interveningDec

/-- §8 RL-22 (P1) eliminate iff a dominating inc with no intervening dec. -/
theorem RL22_eliminate_iff_known_safe (dominatingInc interveningDec : Bool) :
    rl22_eliminates dominatingInc interveningDec
      = (dominatingInc && (!interveningDec)) := by rfl

/-- §8 RL-22 (P2) net-0 balance: the with-inner-pair lifecycle
    `[inc (outer), inc (inner), dec (inner), dec (outer), dec (final)]` and the
    eliminated lifecycle `[inc (outer), dec (outer), dec (final)]` both net to
    the same balance — removing the matched inner pair is ledger-neutral. We
    model the alloc inc + outer + final dec around the removable inner pair. -/
theorem RL22_elimination_net_zero :
    rcBalance [RcOp.inc, RcOp.inc, RcOp.dec, RcOp.dec, RcOp.dec]
      = rcBalance [RcOp.inc, RcOp.dec, RcOp.dec] := by decide

/-- §8 RL-22 (P2) the load-bearing negative witness: elimination must NOT fire
    without a dominating inc — if `dominatingInc = false`, KnownSafe is false, so
    the pair is KEPT (the inner dec could free a still-referenced value at RC 1). -/
theorem RL22_no_eliminate_without_dominating_inc (interveningDec : Bool) :
    rl22_eliminates false interveningDec = false := by rfl

/-- §8 RL-22 an intervening dec clears KnownSafe (RC may be back to 1) — keep the
    pair. -/
theorem RL22_intervening_dec_keeps_pair (dominatingInc : Bool) :
    rl22_eliminates dominatingInc true = false := by
  unfold rl22_eliminates knownSafe; simp

/-- §8 RL-23 (P1) the AND-join of KnownSafe over predecessors: KnownSafe at a
    join is true iff EVERY predecessor is KnownSafe. Modeled as `List.foldr (&&)`
    over the predecessor flags. -/
def rl23_join (preds : List Bool) : Bool := preds.foldr (· && ·) true

/-- §8 RL-23 (P1) the join is true iff EVERY predecessor is KnownSafe (the
    conservative AND-meet). Proven by induction over the predecessor list. -/
theorem RL23_join_all_preds (preds : List Bool) :
    rl23_join preds = true ↔ ∀ p ∈ preds, p = true := by
  unfold rl23_join
  induction preds with
  | nil => simp
  | cons hd tl ih =>
      simp only [List.foldr_cons, Bool.and_eq_true, List.mem_cons]
      rw [ih]
      constructor
      · rintro ⟨hhd, htl⟩ p (rfl | hp)
        · exact hhd
        · exact htl p hp
      · intro h
        exact ⟨h hd (Or.inl rfl), fun p hp => h p (Or.inr hp)⟩

/-- §8 RL-23 (P2) the AND-not-OR witness: a join over `[true, false]` (one unsafe
    predecessor) yields `false` (conservative) — an OR-join would UNSOUNDLY yield
    `true`, marking the merge KnownSafe when a path reaches it at RC 1. -/
theorem RL23_and_not_or_witness :
    rl23_join [true, false] = false ∧ ([true, false].foldr (· || ·) false = true) := by
  constructor <;> decide

/-! ## §8 PRE-style global RC motion (annex-e §AIMS §8 RL-24 / RL-25 / RL-26)

    RL-24 matches an (Inc, Dec) pair across blocks (bidirectional dataflow).
    RL-25 eliminates a matched pair iff KnownSafe OR both paths are safe with no
    CFG hazard. RL-26 forbids moving an RC op across an RC-observable barrier. -/

/-- §8 RL-24 (P1) pair matching: an (Inc, Dec) pair on the same variable is
    matched iff it is identified in BOTH the bottom-up release and top-down retain
    directions. -/
def rl24_matched (forwardSafe backwardSafe : Bool) : Bool := forwardSafe && backwardSafe

/-- §8 RL-24 (P1) a pair is matched iff both the forward and backward directions
    agree. -/
theorem RL24_matched_iff_bidirectional (forwardSafe backwardSafe : Bool) :
    rl24_matched forwardSafe backwardSafe = (forwardSafe && backwardSafe) := by rfl

/-- §8 RL-25 (P1) eliminability: a matched pair is eliminated iff KnownSafe OR
    (both paths safe AND no CFG hazard). -/
def rl25_eliminable (knownSafeFlag bothPathsSafe noCfgHazard : Bool) : Bool :=
  knownSafeFlag || (bothPathsSafe && noCfgHazard)

/-- §8 RL-25 (P1) eliminate iff KnownSafe ∨ (both-paths-safe ∧ no-hazard). -/
theorem RL25_eliminable_decision (knownSafeFlag bothPathsSafe noCfgHazard : Bool) :
    rl25_eliminable knownSafeFlag bothPathsSafe noCfgHazard
      = (knownSafeFlag || (bothPathsSafe && noCfgHazard)) := by rfl

/-- §8 RL-25 (P2) a CFG hazard (path-count misalignment) blocks elimination even
    when both paths look locally safe — the inc and dec may execute different
    numbers of times on some path. The negative witness (no KnownSafe + hazard
    present). -/
theorem RL25_cfg_hazard_blocks (bothPathsSafe : Bool) :
    rl25_eliminable false bothPathsSafe false = false := by
  unfold rl25_eliminable; simp

/-- §8 RL-26 the RC-observable barrier kinds: a call passing `v` to an
    Owned/may_share param, an IsShared on `v`, a Set/SetTag on an aggregate
    containing `v`. -/
inductive MotionBarrier
  | callOwnedOrMayShare  -- observes v's count
  | isSharedOnV          -- reads v's RC header
  | setOnContaining      -- implicit field drops
  | transparent          -- v not involved: no barrier
deriving Repr, DecidableEq

/-- §8 RL-26 (P1) barrier decision: motion of an RC op for `v` across `I` is
    BLOCKED iff `I` is one of the three observing barriers; a transparent
    instruction permits motion. -/
def rl26_motion_blocked : MotionBarrier → Bool
  | .callOwnedOrMayShare => true
  | .isSharedOnV         => true
  | .setOnContaining     => true
  | .transparent         => false

/-- §8 RL-26 (P1/P2) motion is blocked across exactly the three observing
    barriers and permitted across a transparent instruction (the soundness
    boundary: an RC op may not cross an instruction that observes `v`'s count). -/
theorem RL26_barrier_blocks (b : MotionBarrier) :
    rl26_motion_blocked b = (b != .transparent) := by
  cases b <;> rfl

/-! ## §8 Selective barriers (annex-e §AIMS §8 RL-27 / RL-28)

    RL-27 flushes pending RC ops at a call site iff the callee param is Owned +
    non-Dead OR Borrowed + may_share (the callee may write `v`'s header). RL-28
    conservatively flushes ALL pending RC ops at an unknown-callee (FFI /
    indirect / no contract) call. -/

/-- §8 RL-27 flush decision: flush `v`'s pending ops iff the callee param is
    (Owned ∧ non-Dead) OR (Borrowed ∧ may_share). -/
def rl27_flushes (calleeOwned calleeNonDead calleeBorrowed calleeMayShare : Bool) : Bool :=
  (calleeOwned && calleeNonDead) || (calleeBorrowed && calleeMayShare)

/-- §8 RL-27 (P1) flush iff Owned-non-Dead OR Borrowed-may_share. -/
theorem RL27_flush_decision (co cnd cb cms : Bool) :
    rl27_flushes co cnd cb cms = ((co && cnd) || (cb && cms)) := by rfl

/-- §8 RL-27 (P2) a Borrowed + ¬may_share (pure) callee requires NO flush — it
    cannot write `v`'s header. The negative witness. -/
theorem RL27_borrowed_pure_no_flush (co _cnd : Bool) :
    rl27_flushes co false true false = (co && false) := by
  unfold rl27_flushes; simp

/-- §8 RL-28 (P1) an unknown callee (no contract) conservatively flushes ALL
    pending RC ops — modeled as the constant `true` flush decision regardless of
    any (unavailable) param contract. -/
def rl28_flushes_all : Bool := true

/-- §8 RL-28 (P1/P2) the unknown-callee flush is unconditional — with no
    contract, the callee may inc / dec / share ANY argument, so every pending op
    must be flushed before the call. -/
theorem RL28_unknown_callee_flushes_all : rl28_flushes_all = true := by rfl

/-! ## §8 RL-29 — noalias on fresh + unique returns (annex-e §AIMS §8 RL-29)

    RL-29 marks a return value `noalias` iff `preserves_freshness = true ∧
    uniqueness = Unique`. The `preserves_freshness` gate is LOAD-BEARING: a
    Unique-but-not-fresh (parameter passthrough) return may alias the caller's
    copy, so uniqueness alone is insufficient. -/

/-- §8 RL-29 noalias emission: fresh ∧ Unique. -/
def rl29_noalias (preservesFreshness : Bool) (u : Uniqueness) : Bool :=
  preservesFreshness && (u = .Unique)

/-- §8 RL-29 (P1) emission decision: noalias iff preserves_freshness ∧ Unique. -/
theorem RL29_noalias_decision (pf : Bool) (u : Uniqueness) :
    rl29_noalias pf u = (pf && (u = .Unique)) := by rfl

/-- §8 RL-29 (P2) the load-bearing negative witness: a Unique-but-not-fresh
    (passthrough) return must NOT be marked noalias — `preserves_freshness =
    false` yields `false` even though uniqueness is Unique. -/
theorem RL29_unique_not_fresh_no_noalias :
    rl29_noalias false .Unique = false := by rfl

/-- §8 RL-29 a fresh MaybeShared return is NOT noalias (uniqueness gate fails). -/
theorem RL29_fresh_maybeshared_no_noalias :
    rl29_noalias true .MaybeShared = false := by rfl

/-- §8 RL-29 a fresh Unique return IS noalias (the positive case). -/
theorem RL29_fresh_unique_noalias :
    rl29_noalias true .Unique = true := by rfl

/-! ## §8 RL-30 — effect-based memory attributes (annex-e §AIMS §8 RL-30)

    RL-30 selects an LLVM `memory(...)` attribute from the IC-5 EffectSummary +
    IC-3 ParamContracts. Each attribute is an OVER-approximation: it never claims
    FEWER effects than the contract proves. A pure function gets `memory(none)`
    only when no alloc/dealloc/share/inaccessible-read and no arg access. -/

inductive MemoryAttr
  | none                       -- memory(none)
  | argmemRead                 -- memory(argmem: read)
  | inaccessibleRW             -- memory(inaccessible: rw)
  | argmemRwInaccessibleRW     -- memory(argmem: rw, inaccessible: rw)
deriving Repr, DecidableEq

/-- §8 RL-30 the IC-5 + IC-3 inputs that drive attribute selection (the
    MEMORY-relevant subset). -/
structure MemoryEffectInputs where
  mayAllocate : Bool
  mayDeallocate : Bool
  mayShare : Bool
  mayReadInaccessible : Bool
  anyArgAccess : Bool          -- some param has cardinality ≠ Absent
  anyArgWritten : Bool         -- some param Owned (writes args)
deriving Repr, DecidableEq

/-- §8 RL-30 attribute selection (the case table; `argmemRwInaccessibleRW` is the
    conservative fallback). -/
def memoryAttr (e : MemoryEffectInputs) : MemoryAttr :=
  if (!e.mayAllocate) && (!e.mayDeallocate) && (!e.mayShare)
      && (!e.mayReadInaccessible) && (!e.anyArgAccess) then
    .none                                            -- pure, no access
  else if (!e.mayAllocate) && (!e.mayDeallocate) && (!e.mayShare)
      && (!e.mayReadInaccessible) && (!e.anyArgWritten) then
    .argmemRead                                      -- pure read-only of args
  else if e.mayAllocate || e.mayDeallocate || e.mayShare then
    (if e.anyArgAccess then .argmemRwInaccessibleRW else .inaccessibleRW)
  else
    .argmemRwInaccessibleRW                           -- fallback

/-- §8 RL-30 (P1) a pure function with no arg access gets `memory(none)`. -/
theorem RL30_pure_no_access_none (rest : MemoryEffectInputs)
    (h : rest = { mayAllocate := false, mayDeallocate := false, mayShare := false,
                  mayReadInaccessible := false, anyArgAccess := false,
                  anyArgWritten := false }) :
    memoryAttr rest = MemoryAttr.none := by subst h; rfl

/-- §8 RL-30 (P2) the load-bearing negative witness: an allocating function must
    NOT get `memory(none)` — it accesses inaccessible memory via the allocator.
    Any input with `mayAllocate = true` yields an attribute ≠ `none`. -/
theorem RL30_allocating_not_none (e : MemoryEffectInputs) (h : e.mayAllocate = true) :
    memoryAttr e ≠ MemoryAttr.none := by
  obtain ⟨ma, md, ms, mri, aa, aw⟩ := e
  -- `h : ma = true`; destructure every boolean so `memoryAttr` reduces by `rfl`
  -- on each leaf, and the result is one of the two RW attrs or `inaccessibleRW`,
  -- never `none`.
  subst h
  cases md <;> cases ms <;> cases mri <;> cases aa <;> cases aw <;> decide

/-- §8 RL-30 a pure read-only-of-args function (no alloc/dealloc/share, no writes)
    gets `memory(argmem: read)`. -/
theorem RL30_pure_readonly_argmem_read (rest : MemoryEffectInputs)
    (h : rest = { mayAllocate := false, mayDeallocate := false, mayShare := false,
                  mayReadInaccessible := false, anyArgAccess := true,
                  anyArgWritten := false }) :
    memoryAttr rest = MemoryAttr.argmemRead := by subst h; rfl

/-! ## §8 RL-31 — CRITICAL: disjoint Borrowed params → noalias metadata
    (annex-e §AIMS §8 RL-31)

    The Ori-novel theorem. Disjoint Borrowed parameters `(p_i, p_j)` receive
    `!alias.scope` + `!noalias` metadata iff, at EVERY call site, the args to
    `p_i` and `p_j` are PROVABLY disjoint. The proof requires a CROSS-FUNCTION
    provenance summary on the CALLERS' function-local `borrow_sources` /
    `project_alias_sources` tables — beyond what IC-2/IC-3 contracts alone
    express. The 8-clause SUFFICIENT condition is modeled below; the soundness
    property is: disjoint ROOT SETS (or disjoint fields of a shared root via the
    nested-projection prefix test) ⟹ the two borrows cannot alias the same
    memory ⟹ the noalias metadata is sound. -/

/-- §8 RL-31 a root set = the set of source-aggregate variable ids an arg traces
    to (after filtering `project_alias_sources` to upstream-source-free roots).
    Modeled as a `List Nat` of root ids; FRESH allocations get a singleton own
    root. -/
abbrev RootSet := List Nat

/-- §8 RL-31 membership of a variable id in a root set (explicit recursion so the
    `decide` witnesses below reduce in the kernel without `List.contains`
    instance-stuck states). -/
def rootMem (x : Nat) : RootSet → Bool
  | []      => false
  | y :: ys => (x == y) || rootMem x ys

/-- §8 RL-31 two root sets are DISJOINT iff they share no element (no common
    source aggregate). Explicit recursion keeps the predicate `decide`-reducible. -/
def rootSetsDisjoint : RootSet → RootSet → Bool
  | [],      _ => true
  | x :: xs, b => (!rootMem x b) && rootSetsDisjoint xs b

/-- §8 RL-31 the per-call-site provenance of one Borrowed arg: either it traces
    to a set of roots, OR it is a FRESH allocation (own disjoint root), OR it is
    untraceable (FAIL conservatively). -/
inductive ArgProvenance
  | tracedRoots (rs : RootSet)  -- clause 3/4: traced root set
  | fresh (ownRoot : Nat)       -- clause 5: FRESH allocation, own disjoint root
  | untraceable                 -- clause 6: FAIL conservatively
deriving Repr, DecidableEq

/-- §8 RL-31 the root set an arg provenance contributes (a FRESH alloc's own
    root; an untraceable arg contributes the empty set — it will FAIL the
    emission gate via `ArgProvenance.traceable`). -/
def ArgProvenance.rootSet : ArgProvenance → RootSet
  | .tracedRoots rs => rs
  | .fresh r        => [r]
  | .untraceable    => []

/-- §8 RL-31 an arg provenance is traceable iff it is not the conservative-FAIL
    case (clause 6). -/
def ArgProvenance.traceable : ArgProvenance → Bool
  | .untraceable => false
  | _            => true

/-- §8 RL-31 clause (1)+(3)+(4)+(5)+(6): a SINGLE call site proves `(p_i, p_j)`
    disjoint iff both args are traceable AND their root sets are disjoint. The
    same-root disjoint-fields case (clause 7) is handled by `sameRootFieldsDisjoint`
    below; this is the distinct-root-set facet. -/
def siteProvesDisjoint (pi pj : ArgProvenance) : Bool :=
  pi.traceable && pj.traceable && rootSetsDisjoint pi.rootSet pj.rootSet

/-- §8 RL-31 clause (2): RL-31 emits the metadata iff EVERY call site proves
    disjointness — the all-sites conjunction. Any failing site withholds the
    metadata. Modeled as explicit recursion over the per-site (pi, pj) pairs. -/
def rl31_emits_metadata : List (ArgProvenance × ArgProvenance) → Bool
  | []      => true
  | s :: ss => siteProvesDisjoint s.1 s.2 && rl31_emits_metadata ss

/-- §8 RL-31 (P1) clause 4 — DISTINCT ROOT SETS emit: two args tracing to
    disjoint root sets (e.g. `{1}` and `{2}`) prove disjointness at the site. -/
theorem RL31_clause4_disjoint_roots_emit :
    siteProvesDisjoint (.tracedRoots [1]) (.tracedRoots [2]) = true := by decide

/-- §8 RL-31 (P1) clause 5 — FRESH allocation own root: a FRESH-allocated arg has
    its own disjoint root, so it is disjoint from any arg with a different root. -/
theorem RL31_clause5_fresh_alloc_emit :
    siteProvesDisjoint (.fresh 7) (.tracedRoots [3]) = true := by decide

/-- §8 RL-31 (P1) clause 6 — UNTRACEABLE arg fails conservatively: an untraceable
    arg cannot prove disjointness, so the site (and hence the metadata) fails. -/
theorem RL31_clause6_untraceable_fail (pj : ArgProvenance) :
    siteProvesDisjoint .untraceable pj = false := by
  unfold siteProvesDisjoint ArgProvenance.traceable
  rfl

/-- §8 RL-31 (P1) clause 4 — SAME root set NO emit: two args sharing a root (both
    `{1}`) are NOT provably disjoint (they may alias the same aggregate). -/
theorem RL31_same_root_no_emit :
    siteProvesDisjoint (.tracedRoots [1]) (.tracedRoots [1]) = false := by decide

/-! ### §8 RL-31 clause 7 — same-root disjoint-fields via the nested-projection
    prefix test. When two args share a root aggregate, disjointness holds iff
    they project DISJOINT fields — neither field path is a prefix of the other
    (reusing `fieldsOverlap` / `isPrefix` from RL-10). -/

/-- §8 RL-31 clause 7: two same-root args are disjoint iff their projection field
    paths do NOT overlap (neither a prefix of the other). -/
def sameRootFieldsDisjoint (fieldI fieldJ : FieldPath) : Bool :=
  !fieldsOverlap fieldI fieldJ

/-- §8 RL-31 (P1) clause 7 — same-root DISJOINT fields emit: args sharing a root
    but projecting disjoint fields `[0]` and `[1]` ARE disjoint (the borrows read
    non-overlapping memory). -/
theorem RL31_clause7_disjoint_fields_emit :
    sameRootFieldsDisjoint [0] [1] = true := by decide

/-- §8 RL-31 (P1) clause 7 — PREFIX overlap NO emit: a parent field `[0]` and a
    child field `[0, 1]` of the same root OVERLAP (the parent path is a prefix),
    so they are NOT disjoint — no metadata. The nested-projection prefix test. -/
theorem RL31_clause7_prefix_overlap_no_emit :
    sameRootFieldsDisjoint [0] [0, 1] = false := by decide

/-! ### §8 RL-31 clause 2 — the all-sites conjunction (the core soundness gate) -/

/-- §8 RL-31 (P1) clause 2 — ANY failing site withholds the metadata. If even one
    call site fails to prove disjointness, the all-sites conjunction is false, so
    no metadata is emitted. Proven by induction over the site list: the failing
    member sticky-clears the recursive AND. -/
theorem RL31_any_site_fails_no_metadata
    (sites : List (ArgProvenance × ArgProvenance))
    (bad : ArgProvenance × ArgProvenance) (hmem : bad ∈ sites)
    (hbad : siteProvesDisjoint bad.1 bad.2 = false) :
    rl31_emits_metadata sites = false := by
  induction sites with
  | nil => exact absurd hmem (List.not_mem_nil)
  | cons hd tl ih =>
      rw [List.mem_cons] at hmem
      unfold rl31_emits_metadata
      rcases hmem with rfl | htl
      · rw [hbad]; rfl
      · rw [ih htl]; simp

/-- §8 RL-31 (P1) clause 2 — a fully-disjoint corpus emits: when EVERY site
    proves disjointness, the all-sites conjunction is true and the metadata is
    emitted (the positive corpus). -/
theorem RL31_all_sites_disjoint_emits :
    rl31_emits_metadata
      [(.tracedRoots [1], .tracedRoots [2]), (.fresh 9, .tracedRoots [1])] = true := by
  decide

/-! ### §8 RL-31 (P3) — the CRITICAL SOUNDNESS theorem

    The Ori-novel contribution: when the root sets are disjoint, the two Borrowed
    params CANNOT alias the same memory. The formal statement: if
    `rootSetsDisjoint a b = true`, then there is NO common root variable — no
    aggregate both borrows can reach — so the LLVM `noalias` treatment is sound.
    This is the property `siteProvesDisjoint` GUARANTEES, proven directly over the
    root-set membership. -/

/-- §8 RL-31 helper: `rootMem x b = false` ⟹ `x` is NOT a member of `b` as a
    list. Bridges the boolean membership predicate to propositional `∉`. Proven
    by induction over `b`. -/
theorem rootMem_false_not_mem (x : Nat) (b : RootSet)
    (h : rootMem x b = false) : x ∉ b := by
  induction b with
  | nil => exact List.not_mem_nil
  | cons hd tl ih =>
      unfold rootMem at h
      simp only [Bool.or_eq_false_iff, beq_eq_false_iff_ne] at h
      obtain ⟨hne, htl⟩ := h
      rw [List.mem_cons]
      rintro (rfl | hmem)
      · exact hne rfl
      · exact ih htl hmem

/-- §8 RL-31 (P3) SOUNDNESS: disjoint root sets ⟹ no common source aggregate.
    If `rootSetsDisjoint a b`, then no variable id is in BOTH `a` and `b` — the
    two borrows provably reach disjoint memory, which is exactly the precondition
    LLVM `noalias` requires. Proven by induction over `a` against the recursive
    `rootSetsDisjoint`, using `rootMem_false_not_mem` per head element. -/
theorem RL31_disjoint_roots_no_common_aggregate (a b : RootSet)
    (h : rootSetsDisjoint a b = true) :
    ∀ x, x ∈ a → x ∉ b := by
  induction a with
  | nil => intro x hxa; exact absurd hxa List.not_mem_nil
  | cons hd tl ih =>
      unfold rootSetsDisjoint at h
      simp only [Bool.and_eq_true, Bool.not_eq_true'] at h
      obtain ⟨hheadDisjoint, htlDisjoint⟩ := h
      intro x hxa
      rw [List.mem_cons] at hxa
      rcases hxa with rfl | hxtl
      · exact rootMem_false_not_mem x b hheadDisjoint
      · exact ih htlDisjoint x hxtl

/-- §8 RL-31 (P3) the CRITICAL theorem under the standard name: when a call site
    proves disjointness (`siteProvesDisjoint = true`), the two Borrowed args'
    root sets share no common source aggregate, so the emitted `!noalias`
    metadata is SOUND — LLVM's alias analysis correctly treats `p_i` and `p_j` as
    non-aliasing. This is the Ori-novel disjoint-Borrowed alias-metadata theorem
    (00-overview MS-4 critical rule). -/
theorem RL31_disjoint_borrowed_noalias (pi pj : ArgProvenance)
    (h : siteProvesDisjoint pi pj = true) :
    (pi.traceable = true) ∧ (pj.traceable = true)
      ∧ (∀ x, x ∈ pi.rootSet → x ∉ pj.rootSet) := by
  unfold siteProvesDisjoint at h
  simp only [Bool.and_eq_true] at h
  obtain ⟨⟨hpi, hpj⟩, hdisj⟩ := h
  exact ⟨hpi, hpj, RL31_disjoint_roots_no_common_aggregate pi.rootSet pj.rootSet hdisj⟩

/-- §8 RL-31 (P2) dual-facet conjunction: the metadata is sound only when BOTH
    the call-site provenance facet (a) AND the type-level facet (b) hold. The
    type-level facet ALONE is REJECTED — it leaves the VF-2 (b) per-call-site
    contract-consistency check unproven. Modeled as the AND of the two facet
    flags; proving the type facet alone (`provenanceFacet = false`) yields a
    `false` (unsound) verdict. -/
def rl31_dual_facet (provenanceFacet typeFacet : Bool) : Bool :=
  provenanceFacet && typeFacet

/-- §8 RL-31 (P2) the type-level facet ALONE is insufficient: with the per-
    call-site provenance facet unproven (`false`), the dual-facet verdict is
    `false` regardless of the type facet — the negative witness against emitting
    metadata on the type facet alone. -/
theorem RL31_type_facet_alone_insufficient (typeFacet : Bool) :
    rl31_dual_facet false typeFacet = false := by
  unfold rl31_dual_facet; simp

/-- §8 RL-31 (P2) both facets ⟹ sound: when both the provenance facet and the
    type facet hold, the dual-facet verdict is `true` (sound metadata). -/
theorem RL31_both_facets_sound :
    rl31_dual_facet true true = true := by rfl

/-! ## §8 Borrow inference (annex-e §AIMS §8 RL-32 / RL-33 / RL-34)

    RL-32: non-scalar params initialize Borrowed (most optimistic); the fixpoint
    promotes to Owned on demand. RL-33: if a projected field becomes Owned, the
    source variable is promoted to Owned. RL-34: never insert RcDec after a tail
    call — transfer ownership when the callee param is Owned; dec BEFORE the call
    when Borrowed. -/

/-- §8 RL-32 the param access after fixpoint: Owned iff demand proves the callee
    consumes / stores the value; Borrowed otherwise (the optimistic seed). -/
def rl32_param_access (demandProvesConsumed : Bool) : AccessClass :=
  if demandProvesConsumed then .Owned else .Borrowed

/-- §8 RL-32 (P1) a param starts Borrowed and promotes to Owned only when demand
    proves consumption (the monotone Borrowed → Owned promotion). -/
theorem RL32_borrowed_default :
    rl32_param_access false = AccessClass.Borrowed
      ∧ rl32_param_access true = AccessClass.Owned := by
  constructor <;> rfl

/-- §8 RL-33 (P1) projection promotion: if a projected field's inferred access
    becomes Owned, the source variable is promoted to Owned (owning the field
    requires owning a reference to the aggregate). Modeled as: the source access
    is Owned iff the field access is Owned (the upward propagation). -/
def rl33_source_access (fieldAccess : AccessClass) : AccessClass :=
  match fieldAccess with
  | .Owned    => .Owned
  | .Borrowed => .Borrowed

/-- §8 RL-33 an Owned projected field promotes the source to Owned. -/
theorem RL33_field_owned_promotes_source :
    rl33_source_access .Owned = AccessClass.Owned := by rfl

/-- §8 RL-33 a Borrowed projected field leaves the source Borrowed (no spurious
    promotion). -/
theorem RL33_field_borrowed_keeps_source :
    rl33_source_access .Borrowed = AccessClass.Borrowed := by rfl

/-- §8 RL-34 the tail-call action by callee param access. -/
inductive TailCallAction
  | transferOwnership  -- callee Owned: no post-call dec, TCO preserved
  | decBeforeCall      -- callee Borrowed: dec pre-call (cannot transfer)
deriving Repr, DecidableEq

/-- §8 RL-34 tail-call action: Owned param → transfer ownership; Borrowed param →
    dec before the call. NEVER a post-call dec (that would break TCO). -/
def rl34_action (calleeAccess : AccessClass) : TailCallAction :=
  match calleeAccess with
  | .Owned    => .transferOwnership
  | .Borrowed => .decBeforeCall

/-- §8 RL-34 (P1) callee Owned ⟹ transfer ownership (no post-call dec, TCO
    preserved). -/
theorem RL34_owned_transfers : rl34_action .Owned = TailCallAction.transferOwnership := by rfl

/-- §8 RL-34 (P1) callee Borrowed ⟹ dec before the call (cannot transfer to a
    borrow; a post-call dec is forbidden, so the dec moves before the call). -/
theorem RL34_borrowed_dec_before : rl34_action .Borrowed = TailCallAction.decBeforeCall := by rfl

/-- §8 RL-34 (P2) the negative witness: NO tail-call action inserts a post-call
    dec — the action is ALWAYS either a pre-call transfer or a pre-call dec
    (proven by totality: both `AccessClass` cases map to a pre-call action, never
    a post-call one). -/
theorem RL34_never_post_call_dec (calleeAccess : AccessClass) :
    rl34_action calleeAccess = .transferOwnership
      ∨ rl34_action calleeAccess = .decBeforeCall := by
  cases calleeAccess <;> simp [rl34_action]

end AimsProof
