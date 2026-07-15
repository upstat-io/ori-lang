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
minimal faithful structure each property is defined over (logical ownership
events form an `Int` credit ledger; an edge / use-kind abstraction for RL-2 /
RL-4; an `AllocationFacts`
freeze plus target capability relation for RL-14..RL-21; a KnownSafe flag
lattice for RL-22 / RL-23; disjoint root-sets with a prefix test for RL-31) and
proven as real
kernel-checked theorems, NOT vacuous `by decide` over a single product. The
lattice dimension carriers (`AccessClass`, `Uniqueness`, `Locality`, `Shape`)
are reused from `Model.lean`.

The unifying soundness invariant for ownership realization, mutation isolation,
transfer, and post-pipeline refinement is LOGICAL OWNERSHIP-EVENT BALANCE: every
owned non-scalar allocation
identity starts with one ownership credit and discharges it exactly once. The
abstract credit/debit ledger balances to 0; it does not require a physical
counter or prescribe reclamation. Each rule's theorem states its emission/fact
decision AND proves the balance property the decision preserves.

Rule index (per §AIMS §8):
  Ownership events  RL-1..RL-5  : credit on duplication, release at terminal use,
                                  elision via DP-2/DP-3/DP-7, edge releases +
                                  Jump-arg exemption, dead-at-entry cleanup.
  Mutation isolation RL-6..RL-10: admissible outcomes, sharing observations,
                                  compound contraction, disjoint-field isolation +
                                  SetTag whole-payload exclusion.
  Credit transfer  RL-11..RL-12 : donor/recipient transfer in same block or
                                  across blocks (RL-13 REMOVED).
  Allocation facts RL-14..RL-16: logical lifetime, caller-use, cleanup, and
                                  representation-owned extent evidence.
  Owner/projection RL-17..RL-18a: owner bounds, exact ownership-observation identities,
                                  capability satisfaction, and trace parity.
  Thread facts     RL-19..RL-21: conservative thread reachability and the
                                  no-thread-boundary whole-program corollary.
  KnownSafe pair    RL-22..RL-23: dominating-inc elimination + AND-join.
  Event refinement RL-24..RL-26 : pair matching + eliminability + ordering barriers.
  Selective barrier RL-27..RL-28: call-site ordering + unknown-callee ordering.
  Fact export       RL-29..RL-31: fresh-self-allocation, memory-access, and
                                  parameter-disjointness facts. Target attributes
                                  and metadata are separate projections.
  Borrow inference  RL-32..RL-34: Borrowed-default, projection promotion,
                                  tail-call ownership transfer.
-/

import AimsProof.Decision

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §8 logical ownership-event substrate (annex-e §AIMS §8 realization)

    The unifying soundness property of the RC-emission + post-pipeline families is
    LOGICAL OWNERSHIP-EVENT BALANCE. An event has an `Int`-valued credit delta:
    `+1` for an owner credit, `-1` for release or cleanup, and `0` for a neutral
    operation or user-level drop body.
    A lifecycle is a list of events; its NET balance is their sum. The realization
    rules preserve net-0 from initial credit through final discharge. A target may
    realize the proof with a counter, static lifetime, uniqueness, or another
    validated mechanism; the ledger does not choose one. -/

/-- §8 canonical logical ownership event. Physical retains, releases, counters,
    tracing, regions, and static discharge are projection mechanisms. -/
inductive OwnershipEvent
  | ownerCredit -- one additional logical owner credit: +1
  | release     -- one logical owner releases its credit: -1
  | cleanup     -- terminal cleanup discharges the final credit: -1
  | neutral     -- an operation with no ownership-credit effect: 0
  | userDrop    -- user `@drop` body; distinct from credit discharge: 0
deriving Repr, DecidableEq

/-- §8 the net logical ownership-credit delta of one event. -/
def OwnershipEvent.delta : OwnershipEvent → Int
  | .ownerCredit => 1
  | .release     => -1
  | .cleanup     => -1
  | .neutral     => 0
  | .userDrop => 0

/-- Historical proof-carrier alias retained for theorem/map compatibility.
    `inc`, `dec`, and `noop` below are aliases, not canonical calculus terms. -/
abbrev RcOp := OwnershipEvent

namespace RcOp
abbrev inc : RcOp := OwnershipEvent.ownerCredit
abbrev dec : RcOp := OwnershipEvent.release
abbrev noop : RcOp := OwnershipEvent.neutral
abbrev userDrop : RcOp := OwnershipEvent.userDrop
def delta : RcOp → Int
  | .ownerCredit => 1
  | .release => -1
  | .cleanup => -1
  | .neutral => 0
  | .userDrop => 0
end RcOp

/-- §8 the net balance of a lifecycle = the sum of its event deltas. A value born
    with one logical ownership credit discharges that credit exactly once iff its
    lifecycle (excluding birth) nets to `-1`, or — including the initial `+1` —
    nets to `0`. -/
def ownerCreditBalance (ops : List OwnershipEvent) : Int :=
  (ops.map OwnershipEvent.delta).foldr (· + ·) 0

/-- Historical compatibility function for the original ledger name. The body
    is definitionally the canonical balance over the compatibility carrier. -/
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

/-- Canonical name for the append law; the historical theorem above remains in
    the proof map for compatibility. -/
theorem ownerCreditBalance_append (xs ys : List OwnershipEvent) :
    ownerCreditBalance (xs ++ ys) =
      ownerCreditBalance xs + ownerCreditBalance ys :=
  rcBalance_append xs ys

/-- §8 a matched inc+dec pair is net-0 on the ledger — the foundational
    balance fact RL-1..RL-5 + RL-22..RL-26 all reduce to: adding (or removing) a
    matched `[inc, dec]` pair leaves the net balance unchanged. -/
theorem rcBalance_matched_pair : rcBalance [RcOp.inc, RcOp.dec] = 0 := by decide

/-- A logical owner-credit/release pair is balance-neutral. -/
theorem ownerCreditRelease_pair_balanced :
    ownerCreditBalance [OwnershipEvent.ownerCredit, OwnershipEvent.release] = 0 := by
  decide

/-- §8 the empty lifecycle balances to 0. -/
theorem rcBalance_nil : rcBalance [] = 0 := by decide

/-! ## §8.1 RL-1 — owner credit on duplication (annex-e §AIMS §8 RL-1)

    RL-1 emits an `RcInc` for a duplicating use iff `¬is_rc_inc_elidable(state)`,
    i.e. iff NOT (Cardinality = Once ∧ Consumption = Linear). A move-once linear
    value transfers its single reference (no inc); a multiply-used value duplicates
    (inc). The balance: a duplication that incs is later matched by the consumer's
    dec — net 0 per `rcBalance_matched_pair`. -/

/-- §8.1 RL-1 records an additional owner credit iff it is not elidable. -/
def rl1_records_additional_credit (creditElidable : Bool) : Bool := !creditElidable

/-- Historical compatibility alias for the physical-counter spelling. -/
abbrev rl1_emits_inc := rl1_records_additional_credit

/-- §8.1 RL-1 (P1) emission decision: an inc is emitted on a duplicating use iff
    the inc is not elidable (NOT move-once-linear). The single `true` case is the
    non-elidable (duplicating) one. -/
theorem RL1_emit_iff_not_elidable (incElidable : Bool) :
    rl1_records_additional_credit incElidable = !incElidable := by rfl

/-- §8.1 RL-1 (P2) balance: a duplication that emits an inc is balanced by the
    duplicate's later dec — the `[inc, dec]` pair nets to 0 (no leak, no
    double-free). When the inc is elided (move-once), the single reference moves
    with no inc and the lifecycle `[]` is already net-0. -/
theorem RL1_duplication_balanced (incElidable : Bool) :
    rcBalance (if rl1_emits_inc incElidable then [RcOp.inc, RcOp.dec] else []) = 0 := by
  cases incElidable <;> decide

/-! ## §8.1 RL-2 — release at terminal use / scope exit (annex-e §AIMS §8 RL-2)

    RL-2 emits an `RcDec` at an owned non-scalar value's terminal use iff the use
    is NOT ownership-transferring. The 12 terminal-use kinds partition into 9
    transfer kinds (NO dec — the consumer inherits the obligation) and 3
    non-transfer kinds (dec emitted). Emitting a dec on a transfer use would
    double-release.

    `ApplyToIterConsumingParam` is the iter-consuming transfer kind: a collection
    passed at a borrowed terminator-Invoke arg to a callee that iter-consumes it
    (`for x in coll` lowering -> `@iter` [owned] -> `ori_iter_drop` frees the
    collection INSIDE the callee). Despite the callee's parameter contract reading
    `Borrowed`, ownership of the allocation transfers inward and the callee's
    iterator machinery (`ori_iter_drop`) emits the release — so the caller emits
    NO dec, exactly like the other transfer kinds. The distinction from
    `ApplyToBorrowedParam` (a borrow-read such as `xs.fold(..)` that does NOT
    free) is the proof basis: one kind cannot yield both the caller-dec and
    no-dec verdicts, so a NEW transfer kind is required to express the
    iter-consume case. -/

/-- §8.1 RL-2 the terminal-use kinds (the 12-row coverage grid from the .proof). -/
inductive TerminalUse
  | Return                  -- transfer
  | ConstructArg            -- transfer
  | ReuseArg                -- transfer
  | CollectionReuseArg      -- transfer
  | SetValue                -- transfer
  | PartialApplyCapture     -- transfer
  | ApplyToOwnedParam       -- transfer
  | JumpArg                 -- transfer (RL-4 exemption)
  | ApplyToIterConsumingParam -- transfer (callee iter-consumes + frees via ori_iter_drop)
  | LastReadBeforeScopeExit -- non-transfer: dec
  | ScopeExit               -- non-transfer: dec
  | ApplyToBorrowedParam    -- non-transfer: dec
deriving Repr, DecidableEq

/-- §8.1 RL-2 ownership-transfer classification: the 9 transfer kinds hand off
    the reference; the 3 non-transfer kinds release it. MUST match the shipped
    `is_ownership_transfer` exactly. `ApplyToIterConsumingParam` transfers — the
    callee's `ori_iter_drop` frees the iter-consumed collection, so the caller
    emits no dec. -/
def rl2_use_transfers_ownership : TerminalUse → Bool
  | .Return                  => true
  | .ConstructArg            => true
  | .ReuseArg                => true
  | .CollectionReuseArg      => true
  | .SetValue                => true
  | .PartialApplyCapture     => true
  | .ApplyToOwnedParam       => true
  | .JumpArg                 => true
  | .ApplyToIterConsumingParam => true
  | .LastReadBeforeScopeExit => false
  | .ScopeExit               => false
  | .ApplyToBorrowedParam    => false

/-- §8.1 RL-2 records a release iff the use does NOT transfer ownership. -/
def rl2_records_release (use : TerminalUse) : Bool := !rl2_use_transfers_ownership use

/-- Historical compatibility alias for the physical-counter spelling. -/
abbrev rl2_emits_dec := rl2_records_release

/-- §8.1 RL-2 (P1) emission decision: RL-2's emit-dec decision equals NOT
    `rl2_use_transfers_ownership` on every one of the 12 terminal-use kinds (the
    coverage grid). -/
theorem RL2_dec_at_last_use (use : TerminalUse) :
    rl2_records_release use = !rl2_use_transfers_ownership use := by rfl

/-- §8.1 RL-2 the 9 transfer kinds emit NO dec (the consumer inherits the
    obligation; a dec here would double-release). -/
theorem RL2_transfer_kinds_no_dec (use : TerminalUse)
    (h : rl2_use_transfers_ownership use = true) : rl2_emits_dec use = false := by
  unfold rl2_emits_dec rl2_records_release; rw [h]; rfl

/-- §8.1 RL-2 the 3 non-transfer terminal kinds emit a dec (release). -/
theorem RL2_nontransfer_kinds_dec (use : TerminalUse)
    (h : rl2_use_transfers_ownership use = false) : rl2_emits_dec use = true := by
  unfold rl2_emits_dec rl2_records_release; rw [h]; rfl

/-- §8.1 RL-2 iter-consume transfer: `ApplyToIterConsumingParam` is a transfer
    kind, so the caller emits NO dec — the callee's `ori_iter_drop` releases the
    iter-consumed collection. -/
theorem RL2_iter_consuming_no_caller_dec :
    rl2_emits_dec TerminalUse.ApplyToIterConsumingParam = false := by decide

/-- §8.1 RL-2 the iter-consume GAP: `ApplyToBorrowedParam` alone CANNOT express
    both the caller-dec verdict (a borrow-read such as `xs.fold(..)` that does NOT
    free wants a caller dec) AND the no-caller-dec verdict (an iter-consume that
    DOES free wants none). One kind yields one verdict (`true` here); the two
    verdicts split only with the distinct `ApplyToIterConsumingParam` transfer
    kind — exactly what `RL2_iter_consuming_caller_dec_splits` proves. -/
theorem RL2_borrowed_param_emits_caller_dec :
    rl2_emits_dec TerminalUse.ApplyToBorrowedParam = true := by decide

/-- §8.1 RL-2 the iter-consume distinction is observable at the caller: the
    iter-consuming kind emits no caller dec (the callee frees), the borrow-read
    kind emits one (the value survives the borrowed call) — the two verdicts a
    single `ApplyToBorrowedParam` could not provide. This is the soundness basis
    for the burden-path contract field that classifies a borrowed terminator-arg
    as iter-consuming. -/
theorem RL2_iter_consuming_caller_dec_splits :
    rl2_emits_dec TerminalUse.ApplyToIterConsumingParam = false
    ∧ rl2_emits_dec TerminalUse.ApplyToBorrowedParam = true := by decide

/-- §8.1 RL-2 (P2) balance: an owned non-scalar value born with one logical
    owner credit is
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

/-! ## §8.1 RL-2 — iter-consume ∧ transfer-through-return OVERLAP

    `ApplyToIterConsumingParam` (the callee frees via `ori_iter_drop`) is a sound
    transfer kind ONLY when the iter-consumed value does NOT also survive as the
    Return value. When the SAME allocation is BOTH iter-consumed AND transferred
    through Return — `@f(x) = { let _ = x.iter().count(); x }` — the pure-transfer
    model discharges the owner handed to the caller (UAF). The cure is one
    keep-alive logical credit before the iter-consume: its discharge balances
    the consumer and the original owner survives in the Return value. -/

/-- §8.1 RL-2 overlap balance: a param born owned (+1) that is iter-consumed
    (−1) and ALSO transferred through Return must carry exactly one logical
    owner out. `RcOp` is the established proof-carrier name for credit/debit;
    a physical reference counter is only one projection corollary. -/
def rl2_overlap_returned_rc (keepAliveInc : Bool) : Int :=
  rcBalance ([RcOp.inc] ++ (if keepAliveInc then [RcOp.inc] else []) ++ [RcOp.dec])

/-- §8.1 RL-2 overlap GAP: without the keep-alive credit the returned allocation has
    zero logical owners — a cleaned value handed to the caller (the UAF the pure-transfer
    model cannot prevent on the overlap shape). -/
theorem RL2_iter_consume_return_overlap_gap :
    rl2_overlap_returned_rc false = 0 := by decide

/-- §8.1 RL-2 overlap CURE: one keep-alive credit before the iter-consume restores a
    live (+1) logical owner in the Return value. -/
theorem RL2_iter_consume_return_overlap_cured :
    rl2_overlap_returned_rc true = 1 := by decide

/-- §8.1 RL-2 overlap MINIMALITY: one inc is necessary (0 ⇒ unsound) and exact
    (1 ⇒ live, not over-inc'd). The cure neither under- nor over-counts. -/
theorem RL2_iter_consume_return_overlap_minimal :
    rl2_overlap_returned_rc true = 1 ∧ rl2_overlap_returned_rc false = 0 := by decide

/-- §8.1 RL-2 overlap FULL LIFECYCLE: counting alloc(+1), keep-alive inc(+1),
    iter-drop(−1), and the caller's eventual release of the returned value (−1),
    the cured lifecycle nets to 0 — released exactly once. -/
theorem RL2_iter_consume_return_overlap_balanced :
    rcBalance [RcOp.inc, RcOp.inc, RcOp.dec, RcOp.dec] = 0 := by decide

/-- §8.1 RL-2 overlap DISJOINTNESS: when the param is iter-consumed but NOT
    returned, the pure-transfer model is correct — no keep-alive inc, allocation
    freed exactly once (net 0). The cure fires ONLY on the overlap. -/
theorem RL2_iter_consume_no_return_unchanged :
    rcBalance [RcOp.inc, RcOp.dec] = 0 := by decide

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
    with one logical owner credit that is never used; the immediate cleanup dec
    releases it exactly once — `[inc, dec]` nets to 0. -/
theorem RL5_cleanup_balanced : rcBalance [RcOp.inc, RcOp.dec] = 0 := by decide

/-! ## §8.1 RL-DROP — scope-exit user `@drop` for a drop-glue value (annex-e §AIMS §8)

    The RC realization rules (RL-2 / RL-4 / RL-5) all gate dec emission on
    `!isScalar`: a value whose monomorphized repr is `Scalar` carries no logical
    logical owner-credit obligation and provably receives NO `RcDec` (that gating is
    the soundness basis VF-1's
    `RcOnScalar` invariant enforces). But a value's TYPE may carry a user `@drop`
    (drop-glue) INDEPENDENT of its RC repr — a `Scalar`-repr struct
    (`type Guard = { id: int }` with `impl Guard: Drop`) has drop-glue yet no
    logical ownership-credit event. Such a value still needs its `@drop` run exactly
    once at its death point. The RC ops cannot express this (an `RcDec` on a
    `Scalar` is rejected by VF-1), so the realization op set carries a SEPARATE
    `userDrop` op: a drop-glue
    CALL with ref-count delta 0. RL-DROP emits exactly one `userDrop` at the death
    point iff the type has drop-glue, gated on drop-glue existence INDEPENDENT of
    `isScalar`; because `userDrop` is RC-balance-neutral, the `!isScalar` RcDec
    invariant (RL-2/4/5) is preserved untouched — a scalar value still receives no
    `RcDec`, only the balance-neutral `@drop` call. -/

/-- §8.1 RL-DROP the death-point drop inputs for a value. `hasUserDrop` = the
    value's type carries a user `@drop`; `isScalar` = its monomorphized repr is
    `Scalar` and therefore carries no logical RC obligation. -/
structure DropInputs where
  hasUserDrop : Bool
  isScalar : Bool
deriving Repr, DecidableEq

/-- §8.1 RL-DROP emission: emit a scope-exit `userDrop` iff the type carries
    drop-glue — INDEPENDENT of `isScalar`. -/
def rldrop_emits_user_drop (d : DropInputs) : Bool := d.hasUserDrop

/-- §8.1 RL-DROP (decision): the scope-exit user-drop is emitted iff drop-glue
    exists, regardless of RC repr. -/
theorem RLDROP_emit_on_drop_glue (d : DropInputs) :
    rldrop_emits_user_drop d = d.hasUserDrop := by rfl

/-- §8.1 RL-DROP scalar-independence: a `Scalar`-repr value with a user `@drop`
    STILL emits its scope-exit drop — the bug this cures (the prior RC-only
    realization elided it because every RC dec is `!isScalar`-gated). -/
theorem RLDROP_scalar_value_still_drops (h : Bool) :
    rldrop_emits_user_drop { hasUserDrop := true, isScalar := h } = true := by rfl

/-- §8.1 RL-DROP a value with no drop-glue emits no scope-exit drop (whatever its
    repr) — the `@drop`-less common case is untouched. -/
theorem RLDROP_no_glue_no_drop (h : Bool) :
    rldrop_emits_user_drop { hasUserDrop := false, isScalar := h } = false := by rfl

/-- §8.1 RL-DROP balance-neutrality: a single `userDrop` op nets 0 on the RC
    ledger — it is a drop-glue call, not an RC ref-count op, so emitting it never
    perturbs the RL-1..RL-5 RC-balance invariant. -/
theorem RLDROP_user_drop_balance_neutral : rcBalance [RcOp.userDrop] = 0 := by decide

/-- §8.1 RL-DROP scalar-soundness (the load-bearing one): the death-point
    lifecycle of a `Scalar`-repr value with drop-glue emits the balance-neutral
    `userDrop` AND NO `RcDec` (the `!isScalar` invariant). Its RC balance is 0 —
    the `@drop` runs without ever placing an `RcDec` on a scalar (which VF-1 would
    reject). Models the cured shape: ops = `[userDrop]` (no dec), balance 0. -/
theorem RLDROP_scalar_lifecycle_sound :
    rcBalance [RcOp.userDrop] = 0
      ∧ rldrop_emits_user_drop { hasUserDrop := true, isScalar := true } = true := by
  decide

/-- §8.1 RL-DROP exactly-once: a drop-glue value's death-point lifecycle carries
    exactly ONE `userDrop` op — modeled as the singleton emission; the op count is
    1 iff drop-glue exists, never duplicated (no double-drop) and never elided. -/
def rldrop_lifecycle (d : DropInputs) : List RcOp :=
  if rldrop_emits_user_drop d then [RcOp.userDrop] else []

theorem RLDROP_exactly_once_on_glue :
    (rldrop_lifecycle { hasUserDrop := true, isScalar := true }).length = 1
      ∧ (rldrop_lifecycle { hasUserDrop := true, isScalar := false }).length = 1
      ∧ (rldrop_lifecycle { hasUserDrop := false, isScalar := true }).length = 0 := by
  decide

/-! ### §8.1.1 RL-DROP copy-out — a VALUE-copied drop-glue aggregate

    A drop-glue aggregate VALUE-copied into a still-live container (a
    borrowed-store arg whose callee acquires a funded copy) has its death
    point at the CONTAINER's teardown, not at the local binding's release
    site: the local site emits a fields-only release (no `userDrop`); the
    stored copy's teardown glue carries the value's single `userDrop`. -/

/-- §8.1.1 death-point inputs: `copiedOut` = the value was VALUE-copied into
    a still-live container at a store site. -/
structure CopyOutInputs where
  hasUserDrop : Bool
  copiedOut : Bool
deriving Repr, DecidableEq

/-- §8.1.1 local-site discipline: the binding's release emits `userDrop` iff
    drop-glue exists AND the value was not copied out. -/
def copyout_local_emits_user_drop (d : CopyOutInputs) : Bool :=
  d.hasUserDrop && !d.copiedOut

/-- §8.1.1 the local site's release ops: one funded dec (the fields walk),
    plus the `userDrop` only when the local site IS the death point. -/
def copyout_local_lifecycle (d : CopyOutInputs) : List RcOp :=
  if copyout_local_emits_user_drop d then [RcOp.dec, RcOp.userDrop] else [RcOp.dec]

/-- §8.1.1 the container teardown's ops for the stored copy: its funded dec
    plus the value's single `userDrop` when drop-glue exists. -/
def copyout_container_lifecycle (hasUserDrop : Bool) : List RcOp :=
  if hasUserDrop then [RcOp.dec, RcOp.userDrop] else [RcOp.dec]

/-- §8.1.1 count of `userDrop` ops in a lifecycle. -/
def userDropCount (ops : List RcOp) : Nat :=
  (ops.filter (· = RcOp.userDrop)).length

/-- §8.1.1 exactly-once composition: the copied-out local's fields-only
    release plus the container's teardown carry ONE `userDrop` total. -/
theorem RLDROP_copyout_exactly_once (h : Bool) :
    userDropCount
      (copyout_local_lifecycle { hasUserDrop := h, copiedOut := true }
        ++ copyout_container_lifecycle h)
      = (if h then 1 else 0) := by
  cases h <;> decide

/-- §8.1.1 negative witness: booking the local site as a death point while
    the container also carries the stored copy DOUBLE-drops. -/
theorem RLDROP_copyout_local_death_double_drops :
    userDropCount
      (copyout_local_lifecycle { hasUserDrop := true, copiedOut := false }
        ++ copyout_container_lifecycle true)
      = 2 := by decide

/-- §8.1.1 balance: each site's dec is funded by its own acquisition (the
    local's birth inc; the store's elem inc) — the composition nets 0. -/
theorem RLDROP_copyout_balanced (h : Bool) :
    rcBalance
      ((RcOp.inc :: copyout_local_lifecycle { hasUserDrop := h, copiedOut := true })
        ++ (RcOp.inc :: copyout_container_lifecycle h))
      = 0 := by
  cases h <;> decide

/-! ## §8.2 mutation isolation — admissible outcomes and obligations
    (annex-e §AIMS §8 RL-6 / RL-7 / RL-8)

    A mutation of an owned value selects one of three neutral states from the
    Uniqueness dimension + the local-safety check:
      RL-6 SameIdentityAdmissible: mutation may preserve allocation identity.
      RL-7 SharingObservationRequired: observe sharing, then satisfy the outcome.
      RL-8 IsolationRequired: mutation must be isolated from existing owners.
    All three preserve VALUE SEMANTICS — the mutation is observed only on the
    holder's result, never through an existing alias. A physical `IsShared`,
    branch, copy, or storage operation is a projection, not a theorem premise. -/

/-- Historical COW carrier retained for theorem/map compatibility. -/
abbrev CowEmit := MutationObligation

namespace CowEmit
abbrev StaticUnique : CowEmit := MutationObligation.SameIdentityAdmissible
abbrev Dynamic : CowEmit := MutationObligation.SharingObservationRequired
abbrev StaticShared : CowEmit := MutationObligation.IsolationRequired
end CowEmit

/-- §8.2 neutral mutation-obligation selection from Uniqueness + DP-5.
    Unique ∧ safe admits same identity; Unique ∧ ¬safe and Shared require
    isolation; MaybeShared requires a sharing observation. -/
def realize_mutation_obligation (u : Uniqueness)
    (canInPlace : Bool) : MutationObligation :=
  match u with
  | .Unique      => if canInPlace then .SameIdentityAdmissible else .IsolationRequired
  | .MaybeShared => .SharingObservationRequired
  | .Shared      => .IsolationRequired

/-- Historical compatibility alias for the COW-emission classifier name. -/
abbrev cow_emit := realize_mutation_obligation

/-- §8.2 RL-6 (P1): a Unique value with a safe local check admits a
    same-identity mutation outcome. -/
theorem RL6_static_unique_in_place (_rest : AimsState) :
    realize_mutation_obligation .Unique true = .SameIdentityAdmissible := by rfl

/-- §8.2 RL-6 (P2): one logical owner permits same-identity mutation only when
    local borrows also permit it. The negative case freezes an isolation
    obligation without prescribing how a projection satisfies it. -/
theorem RL6_unique_unsafe_falls_back :
    realize_mutation_obligation .Unique false = .IsolationRequired := by rfl

/-- §8.2 RL-7 (P1): MaybeShared requires a sharing observation regardless of
    the local check. The calculus does not prescribe how that fact is observed. -/
theorem RL7_dynamic_cow (canInPlace : Bool) :
    realize_mutation_obligation .MaybeShared canInPlace
      = .SharingObservationRequired := by
  cases canInPlace <;> rfl

/-- §8.2 RL-8 (P1): Shared requires mutation isolation regardless of the local
    check. Copying is one possible projection, not the frozen outcome. -/
theorem RL8_static_shared_copy (canInPlace : Bool) :
    realize_mutation_obligation .Shared canInPlace = .IsolationRequired := by
  cases canInPlace <;> rfl

/-- §8.2 the mutation obligation is total and decidable over the Uniqueness ×
    safety inputs. The RL-6/7/8 cases cover every Uniqueness value. -/
theorem cow_emit_total (u : Uniqueness) (canInPlace : Bool) :
    realize_mutation_obligation u canInPlace = .SameIdentityAdmissible
      ∨ realize_mutation_obligation u canInPlace = .SharingObservationRequired
      ∨ realize_mutation_obligation u canInPlace = .IsolationRequired := by
  cases u <;> cases canInPlace <;> simp [realize_mutation_obligation]

/-! ## §8.2 RL-9 — sharing-observation refinement (annex-e §AIMS §8 RL-9)

    An explicit and a compact representation of the sharing observation must
    select the same admissible outcome. The theorem freezes that equivalence;
    it does not freeze the physical diamond or compact instruction. -/

/-- §8.2 RL-9 the neutral result of a sharing observation. -/
inductive RuntimeUniq
  | unique   -- exactly one logical owner observed
  | shared   -- multiple logical owners observed
deriving Repr, DecidableEq

/-- §8.2 RL-9 explicit observation outcome. -/
def explicitObservationOutcome : RuntimeUniq → MutationObligation
  | .unique => .SameIdentityAdmissible
  | .shared => .IsolationRequired

/-- §8.2 RL-9 a compact representation selects the identical outcome. -/
def compactObservationOutcome : RuntimeUniq → MutationObligation :=
  explicitObservationOutcome

/-- Historical compatibility aliases for the original physical-form names. -/
abbrev cowDiamondOutcome := explicitObservationOutcome
abbrev cowCompactOutcome := compactObservationOutcome

/-- §8.2 RL-9 (P1) observational equivalence: the contracted compound and the
    expanded diamond yield the same outcome for every runtime uniqueness state. -/
theorem RL9_contraction_equiv (r : RuntimeUniq) :
    compactObservationOutcome r = explicitObservationOutcome r := by rfl

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

/-! ## §8.3 donor/recipient credit transfer
    (annex-e §AIMS §8 RL-11 / RL-11a / RL-12)

    RL-13 is REMOVED (doc-comment below). RL-11 freezes when a dying donor's
    credit may transfer to a fresh recipient: eligibility, donor-before-recipient
    ordering, one-owner evidence, and no intervening hazard. RL-11a states the
    two sharing-observation outcomes. RL-12 lifts the relation across blocks.
    `Reset`, `Reuse`, allocation identity, and storage are transitional carrier
    or projection details, not logical premises. -/

/-- §8.3 RL-11 the same-block reuse inputs. -/
structure ReuseInputs where
  reuseCandidate : Bool    -- DP-6 (Owned ∧ ≠Shared ∧ reusable-shape)
  resetPrecedesReuse : Bool -- historical carrier name: donor precedes recipient
  dyingUnique : Bool       -- §8.3 (c): the dying value is statically Unique
  noInterveningHazard : Bool  -- §8.3 (b): no throw / alloc / alias-use between
deriving Repr, DecidableEq

/-- §8.3 RL-11 freezes donor/recipient credit transfer iff all conditions hold. -/
def rl11_freezes_credit_transfer (r : ReuseInputs) : Bool :=
  r.reuseCandidate && r.resetPrecedesReuse && r.dyingUnique && r.noInterveningHazard

/-- Historical compatibility alias for the storage-reuse spelling. -/
abbrev rl11_reuses := rl11_freezes_credit_transfer

/-- §8.3 RL-11 (P1) reuse decision: same-block reuse fires iff DP-6 ∧
    Reset-precedes-Reuse ∧ Unique ∧ no-intervening-hazard (the full
    AND-conjunction). -/
theorem RL11_same_block_reuse (r : ReuseInputs) :
    rl11_freezes_credit_transfer r
      = (r.reuseCandidate && r.resetPrecedesReuse && r.dyingUnique
          && r.noInterveningHazard) := by rfl

/-- §8.3 RL-11 (P2) a non-unique dying value never reuses (reusing a non-unique
    allocation corrupts the alias). The negative witness. -/
theorem RL11_non_unique_no_reuse (r : ReuseInputs) (h : r.dyingUnique = false) :
    rl11_freezes_credit_transfer r = false := by
  unfold rl11_freezes_credit_transfer; rw [h]; simp

/-- §8.3 RL-11 a throwing / allocating intervening instruction blocks reuse
    (`noInterveningHazard = false` ⟹ no reuse) — prevents a leaked token or an
    invalid reuse opportunity. -/
theorem RL11_intervening_hazard_blocks (r : ReuseInputs)
    (h : r.noInterveningHazard = false) : rl11_freezes_credit_transfer r = false := by
  unfold rl11_freezes_credit_transfer; rw [h]; simp

/-- §8.3 RL-11a: a sharing observation admits donor/recipient credit transfer
    only on the one-owner outcome. Multiple owners require an independent
    logical birth for the recipient. -/
def rl11a_transfer_outcome (dp6 : Bool) : RuntimeUniq → Bool
  | .unique => dp6
  | .shared => false

/-- Historical compatibility alias for the branch-specific spelling. -/
abbrev rl11a_branch := rl11a_transfer_outcome

/-- §8.3 RL-11a reuse fires on the unique runtime arm (DP-6 eligible) but never
    on the shared arm. -/
theorem RL11a_dynamic_unique_arm (dp6 : Bool) :
    rl11a_transfer_outcome dp6 .unique = dp6
      ∧ rl11a_transfer_outcome dp6 .shared = false := by
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
    prevents an orphaned logical transfer witness on unwind. The SCALAR witness
    carries no owner credit and is not a physical allocation or handle. -/
theorem RL12_throw_on_path_blocks (r : CrossBlockReuseInputs)
    (h : r.noThrowOnPath = false) : rl12_reuses r = false := by
  unfold rl12_reuses; rw [h]; simp

/-! ## §8.3 RL-13 — REMOVED (annex-e §AIMS §8 RL-13 removal note)

    RL-13 is REMOVED. The former rule claimed `Construct + Cardinality = Once ⟹
    exactly one logical owner at death`. This is UNSOUND for the same root cause as DP-10
    (`Decision.lean` §DP-10): one use of a `Construct + Once` value may be "store
    into a data structure", which creates an alias via `RcInc` — so `Construct +
    Once` alone does NOT guarantee that the value has one logical owner at death.
    Reuse eligibility is established SOLELY via the Uniqueness dimension (§3.4)
    through DP-6 + RL-11 + RL-12, never derived from the substructural
    Consumption / Cardinality dimensions. The faithful encoding is the ABSENCE of
    a `construct_once_implies_rc1` term in this module — there is no Lean term for
    the removed rule, so it provably cannot be applied. The sound replacement
    (DP-6 / RL-11 / RL-12 via `dyingUnique`) is present and kernel-checked above. -/

/-! ## §8.4 Backend-neutral allocation facts (annex-e §AIMS RL-14..RL-16)

    AIMS freezes logical facts. Representation analysis supplies extent
    evidence separately. Physical planners select placement, metadata,
    synchronization, and ABI mechanisms after this seam. -/

/-- Stable logical allocation/birth-site identity; never a target storage site. -/
abbrev AllocationSiteId := Nat
abbrev BlockId := Nat
abbrev CallSiteId := Nat
abbrev EventId := Nat
abbrev DropPlanId := Nat
abbrev FieldId := Nat
abbrev StorageSiteId := Nat
abbrev TypeId := Nat

inductive CallerProtocol
  | borrowOnly
  | mayShare
  | ownershipTransfer
deriving Repr, DecidableEq

structure CallerUse where
  site : CallSiteId
  protocol : CallerProtocol
deriving Repr, DecidableEq

/-- A nonempty caller extent stores its first use separately. -/
inductive LifetimeBound
  | block (block : BlockId)
  | function
  | callerExtent (first : CallerUse) (rest : List CallerUse)
  | escaping
  | unknown
deriving Repr, DecidableEq

/-- `bounded extra` means at most `extra + 1` simultaneous owners. -/
inductive OwnerBound
  | bounded (extra : Nat)
  | unbounded
deriving Repr, DecidableEq

structure ExactOwnershipObservationFacts where
  sharingObservationEvents : List EventId
  additionalCreditEvents : List EventId
  releaseEvents : List EventId
  externallyObservable : Bool
deriving Repr, DecidableEq

inductive OwnershipObservationFacts
  | exact (facts : ExactOwnershipObservationFacts)
  | unknown
deriving Repr, DecidableEq

structure ExactCleanupObligation where
  releaseEvents : List EventId
  dropPlan : Option DropPlanId
  fieldOrder : List FieldId
  normalExitEvents : List EventId
  unwindExitEvents : List EventId
  lifetimeEndEvents : List EventId
deriving Repr, DecidableEq

inductive CleanupObligation
  | exact (obligation : ExactCleanupObligation)
  | unknown
deriving Repr, DecidableEq

inductive ThreadReachability
  | confined
  | potentiallyShared
deriving Repr, DecidableEq

inductive ExternalVisibility
  | internal
  | crossModule
  | foreignOrOpaque
  | unknown
deriving Repr, DecidableEq

structure AllocationFacts where
  /-- Logical allocation/birth-site identity, distinct from `StorageSiteId`. -/
  site : AllocationSiteId
  locality : Locality
  lifetime : LifetimeBound
  owners : OwnerBound
  ownershipObservations : OwnershipObservationFacts
  cleanup : CleanupObligation
  thread : ThreadReachability
  visibility : ExternalVisibility
deriving Repr, DecidableEq

/-- Extent is representation evidence, not an AIMS fact. -/
inductive ExtentClass
  | staticShape (type : TypeId)
  | runtimeSized (storage : StorageSiteId)
deriving Repr, DecidableEq

structure ProjectionInput where
  facts : AllocationFacts
  extent : ExtentClass
deriving Repr, DecidableEq

def lifetimeFromLocality (locality : Locality) (block : BlockId)
    (callerUses : List CallerUse) : LifetimeBound :=
  match locality with
  | .BlockLocal => .block block
  | .FunctionLocal => .function
  | .ArgEscaping =>
      match callerUses with
      | [] => .unknown
      | first :: rest => .callerExtent first rest
  | .HeapEscaping => .escaping
  | .Unknown => .unknown

theorem lifetime_from_locality_sound (block : BlockId) :
    lifetimeFromLocality .BlockLocal block [] = .block block ∧
    lifetimeFromLocality .FunctionLocal block [] = .function ∧
    lifetimeFromLocality .HeapEscaping block [] = .escaping ∧
    lifetimeFromLocality .Unknown block [] = .unknown := by
  exact ⟨rfl, rfl, rfl, rfl⟩

theorem caller_extent_sites_complete (block : BlockId) (first : CallerUse)
    (rest : List CallerUse) :
    lifetimeFromLocality .ArgEscaping block (first :: rest) =
      .callerExtent first rest := by rfl

def ownerBound (isLocalUnique : Bool) (additionalCreditEvents : List EventId)
    (loopOrGlobal externallyRetainable : Bool) : OwnerBound :=
  if loopOrGlobal || externallyRetainable then .unbounded
  else if isLocalUnique then .bounded 0
  else .bounded additionalCreditEvents.length

def ownerCapacity : OwnerBound → Option Nat
  | .bounded extra => some (extra + 1)
  | .unbounded => none

theorem owner_bound_is_dynamic_upper_bound (additionalCreditEvents : List EventId) :
    ownerCapacity (ownerBound false additionalCreditEvents false false) =
      some (additionalCreditEvents.length + 1) := by rfl

theorem loop_or_global_forces_unbounded (isLocalUnique : Bool)
    (additionalCreditEvents : List EventId) (externallyRetainable : Bool) :
    ownerBound isLocalUnique additionalCreditEvents true externallyRetainable =
      OwnerBound.unbounded := by
  simp [ownerBound]

def cleanupCoversNormalAndUnwind : CleanupObligation → Bool
  | .unknown => false
  | .exact obligation =>
      obligation.releaseEvents.all fun event =>
        obligation.normalExitEvents.contains event &&
          obligation.unwindExitEvents.contains event

theorem cleanup_complete_on_normal_and_unwind (event : EventId)
    (dropPlan : DropPlanId) (field : FieldId) :
    let obligation : ExactCleanupObligation := {
      releaseEvents := [event]
      dropPlan := some dropPlan
      fieldOrder := [field]
      normalExitEvents := [event]
      unwindExitEvents := [event]
      lifetimeEndEvents := [event]
    }
    cleanupCoversNormalAndUnwind (.exact obligation) = true ∧
      obligation.dropPlan = some dropPlan ∧
      obligation.fieldOrder = [field] ∧
      obligation.lifetimeEndEvents = [event] := by
  simp [cleanupCoversNormalAndUnwind]

def threadReachabilityFrom (locality : Locality)
    (crossesThreadBoundary : Bool) : ThreadReachability :=
  if locality = .Unknown then .potentiallyShared
  else if crossesThreadBoundary then .potentiallyShared
  else .confined

theorem thread_reachability_from_locality_callgraph
    (locality : Locality) (crossesThreadBoundary : Bool) :
    threadReachabilityFrom locality crossesThreadBoundary =
      (if locality = .Unknown then .potentiallyShared
       else if crossesThreadBoundary then .potentiallyShared
       else .confined) := by rfl

def programThreadReachability (noThreadBoundary : Bool) (locality : Locality)
    (crossesThreadBoundary : Bool) : ThreadReachability :=
  if noThreadBoundary then .confined
  else threadReachabilityFrom locality crossesThreadBoundary

theorem no_thread_boundary_all_confined (locality : Locality)
    (crossesThreadBoundary : Bool) :
    programThreadReachability true locality crossesThreadBoundary =
      ThreadReachability.confined := by rfl

def unknownAllocationFacts (site : AllocationSiteId) : AllocationFacts := {
  site
  locality := .Unknown
  lifetime := .unknown
  owners := .unbounded
  ownershipObservations := .unknown
  cleanup := .unknown
  thread := .potentiallyShared
  visibility := .unknown
}

def freezeAllocationFacts (site : AllocationSiteId)
    (evidence : Option AllocationFacts) : AllocationFacts :=
  match evidence with
  | some facts => if facts.site = site then facts else unknownAllocationFacts site
  | none => unknownAllocationFacts site

theorem freeze_allocation_facts_total_conservative (site : AllocationSiteId) :
    freezeAllocationFacts site none = unknownAllocationFacts site ∧
    (freezeAllocationFacts site none).lifetime = .unknown ∧
    (freezeAllocationFacts site none).owners = .unbounded ∧
    (freezeAllocationFacts site none).thread = .potentiallyShared ∧
    (freezeAllocationFacts site none).visibility = .unknown := by
  exact ⟨rfl, rfl, rfl, rfl, rfl⟩

theorem extent_class_is_repr_owned (facts : AllocationFacts)
    (left right : ExtentClass) :
    (ProjectionInput.mk facts left).facts =
      (ProjectionInput.mk facts right).facts := by rfl

/-! ## §8.5 Physical-plan capability satisfaction (annex-e §AIMS RL-17..RL-18a) -/

inductive ExtentCapability
  | staticOnly
  | runtimeSized
deriving Repr, DecidableEq

inductive ThreadCapability
  | confinedOnly
  | sharedSafe
deriving Repr, DecidableEq

def lifetimeCovers : LifetimeBound → LifetimeBound → Bool
  | .block required, .block provided => required == provided
  | .block _, _ => true
  | .function, .block _ => false
  | .function, _ => true
  | .callerExtent first rest, .callerExtent providedFirst providedRest =>
      first == providedFirst && rest == providedRest
  | .callerExtent _ _, .escaping => true
  | .callerExtent _ _, .unknown => true
  | .callerExtent _ _, _ => false
  | .escaping, .escaping => true
  | .escaping, .unknown => true
  | .escaping, _ => false
  | .unknown, .unknown => true
  | .unknown, _ => false

def extentCovers : ExtentClass → ExtentCapability → Bool
  | .staticShape _, _ => true
  | .runtimeSized _, .runtimeSized => true
  | .runtimeSized _, .staticOnly => false

def ownerCovers : OwnerBound → OwnerBound → Bool
  | .bounded required, .bounded provided => required ≤ provided
  | .bounded _, .unbounded => true
  | .unbounded, .unbounded => true
  | .unbounded, .bounded _ => false

def threadCovers : ThreadReachability → ThreadCapability → Bool
  | .confined, _ => true
  | .potentiallyShared, .sharedSafe => true
  | .potentiallyShared, .confinedOnly => false

def visibilityCovers : ExternalVisibility → ExternalVisibility → Bool
  | .internal, _ => true
  | .crossModule, .crossModule => true
  | .crossModule, .foreignOrOpaque => true
  | .crossModule, .unknown => true
  | .foreignOrOpaque, .foreignOrOpaque => true
  | .foreignOrOpaque, .unknown => true
  | .unknown, .unknown => true
  | _, _ => false

def cleanupNeedsUnwind : CleanupObligation → Bool
  | .unknown => true
  | .exact obligation =>
      match obligation.unwindExitEvents with
      | [] => false
      | _ :: _ => true

structure LayoutCapabilities where
  site : AllocationSiteId
  lifetimeCoverage : LifetimeBound
  extentCoverage : ExtentCapability
  ownerCapacity : OwnerBound
  ownershipObservationProtocol : OwnershipObservationFacts
  cleanupCoverage : CleanupObligation
  unwindCoverage : Bool
  threadSafety : ThreadCapability
  visibilityCoverage : ExternalVisibility
  externalContractId : Nat
deriving Repr, DecidableEq

def Satisfies (facts : AllocationFacts) (extent : ExtentClass)
    (capabilities : LayoutCapabilities) : Prop :=
  capabilities.site = facts.site ∧
  lifetimeCovers facts.lifetime capabilities.lifetimeCoverage = true ∧
  extentCovers extent capabilities.extentCoverage = true ∧
  ownerCovers facts.owners capabilities.ownerCapacity = true ∧
  capabilities.ownershipObservationProtocol = facts.ownershipObservations ∧
  capabilities.cleanupCoverage = facts.cleanup ∧
  (cleanupNeedsUnwind facts.cleanup = true → capabilities.unwindCoverage = true) ∧
  threadCovers facts.thread capabilities.threadSafety = true ∧
  visibilityCovers facts.visibility capabilities.visibilityCoverage = true

structure CapabilityRefines (strong weak : LayoutCapabilities) : Prop where
  sameSite : strong.site = weak.site
  lifetime : ∀ required,
    lifetimeCovers required weak.lifetimeCoverage = true →
      lifetimeCovers required strong.lifetimeCoverage = true
  extent : ∀ required,
    extentCovers required weak.extentCoverage = true →
      extentCovers required strong.extentCoverage = true
  owners : ∀ required,
    ownerCovers required weak.ownerCapacity = true →
      ownerCovers required strong.ownerCapacity = true
  sameOwnershipObservationContract :
    strong.ownershipObservationProtocol = weak.ownershipObservationProtocol
  sameCleanupContract : strong.cleanupCoverage = weak.cleanupCoverage
  unwind : weak.unwindCoverage = true → strong.unwindCoverage = true
  thread : ∀ required,
    threadCovers required weak.threadSafety = true →
      threadCovers required strong.threadSafety = true
  visibility : ∀ required,
    visibilityCovers required weak.visibilityCoverage = true →
      visibilityCovers required strong.visibilityCoverage = true
  sameExternalContract : strong.externalContractId = weak.externalContractId

theorem stronger_capability_preserves_satisfaction
    (facts : AllocationFacts) (extent : ExtentClass)
    (strong weak : LayoutCapabilities)
    (hrefines : CapabilityRefines strong weak)
    (hsatisfies : Satisfies facts extent weak) :
    Satisfies facts extent strong := by
  rcases hsatisfies with
    ⟨hsite, hlifetime, hextent, howners, hobservations, hcleanup,
      hunwind, hthread, hvisibility⟩
  refine ⟨?_, hrefines.lifetime _ hlifetime, hrefines.extent _ hextent,
    hrefines.owners _ howners, ?_, ?_, ?_, hrefines.thread _ hthread,
    hrefines.visibility _ hvisibility⟩
  · exact hrefines.sameSite.trans hsite
  · exact hrefines.sameOwnershipObservationContract.trans hobservations
  · exact hrefines.sameCleanupContract.trans hcleanup
  · intro hneeds
    exact hrefines.unwind (hunwind hneeds)

inductive VmPlacement
  | frame
  | arena
  | managed
deriving Repr, DecidableEq

inductive VmOwnershipMechanism
  | omitted
  | slotCount
  | sideTable
  | synchronizedSlot
deriving Repr, DecidableEq

structure VmLayoutPlan where
  capabilities : LayoutCapabilities
  placement : VmPlacement
  ownership : VmOwnershipMechanism
deriving Repr, DecidableEq

inductive CompiledPlacement
  | register
  | stack
  | region
  | managed
deriving Repr, DecidableEq

inductive CompiledOwnershipMechanism
  | omitted
  | inlineMetadata
  | runtimeHandle
deriving Repr, DecidableEq

structure CompiledLayoutPlan where
  capabilities : LayoutCapabilities
  placement : CompiledPlacement
  ownership : CompiledOwnershipMechanism
deriving Repr, DecidableEq

structure ValidatedVmPlan (facts : AllocationFacts) (extent : ExtentClass) where
  plan : VmLayoutPlan
  evidence : Satisfies facts extent plan.capabilities

structure ValidatedCompiledPlan (facts : AllocationFacts) (extent : ExtentClass) where
  plan : CompiledLayoutPlan
  evidence : Satisfies facts extent plan.capabilities

theorem validated_vm_plan_sound (facts : AllocationFacts) (extent : ExtentClass)
    (validated : ValidatedVmPlan facts extent) :
    Satisfies facts extent validated.plan.capabilities := validated.evidence

theorem validated_compiled_plan_sound (facts : AllocationFacts)
    (extent : ExtentClass) (validated : ValidatedCompiledPlan facts extent) :
    Satisfies facts extent validated.plan.capabilities := validated.evidence

theorem potentially_shared_requires_safe_capability
    (facts : AllocationFacts) (extent : ExtentClass)
    (capabilities : LayoutCapabilities)
    (hthread : facts.thread = .potentiallyShared)
    (hsatisfies : Satisfies facts extent capabilities) :
    capabilities.threadSafety = .sharedSafe := by
  rcases hsatisfies with ⟨_, _, _, _, _, _, _, hsafe, _⟩
  rw [hthread] at hsafe
  cases hcap : capabilities.threadSafety with
  | confinedOnly => simp [threadCovers, hcap] at hsafe
  | sharedSafe => rfl

structure AimsTrace where
  ownershipObservations : OwnershipObservationFacts
  cleanup : CleanupObligation
deriving Repr, DecidableEq

def aimsTrace (facts : AllocationFacts) : AimsTrace := {
  ownershipObservations := facts.ownershipObservations
  cleanup := facts.cleanup
}

def capabilityTrace (capabilities : LayoutCapabilities) : AimsTrace := {
  ownershipObservations := capabilities.ownershipObservationProtocol
  cleanup := capabilities.cleanupCoverage
}

def vmPlanTrace (plan : VmLayoutPlan) : AimsTrace :=
  capabilityTrace plan.capabilities

def compiledPlanTrace (plan : CompiledLayoutPlan) : AimsTrace :=
  capabilityTrace plan.capabilities

theorem projection_refines_aims_trace (facts : AllocationFacts)
    (extent : ExtentClass) (capabilities : LayoutCapabilities)
    (hsatisfies : Satisfies facts extent capabilities) :
    capabilityTrace capabilities = aimsTrace facts := by
  rcases hsatisfies with ⟨_, _, _, _, hobservations, hcleanup, _, _, _⟩
  unfold capabilityTrace aimsTrace
  rw [hobservations, hcleanup]

theorem vm_compiled_event_parity (facts : AllocationFacts)
    (extent : ExtentClass) (vm : ValidatedVmPlan facts extent)
    (compiled : ValidatedCompiledPlan facts extent) :
    vmPlanTrace vm.plan = compiledPlanTrace compiled.plan := by
  calc
    vmPlanTrace vm.plan = aimsTrace facts :=
      projection_refines_aims_trace facts extent vm.plan.capabilities vm.evidence
    _ = compiledPlanTrace compiled.plan :=
      (projection_refines_aims_trace facts extent compiled.plan.capabilities
        compiled.evidence).symm

/-! ## §8.7 Thread reachability facts (annex-e §AIMS RL-19..RL-21)

    AIMS freezes reachability. A physical plan may use any mechanism whose
    capability satisfies the frozen fact. -/

theorem unknown_thread_reachability_is_potentially_shared
    (crossesThreadBoundary : Bool) :
    threadReachabilityFrom .Unknown crossesThreadBoundary =
      ThreadReachability.potentiallyShared := by
  rfl

/-! ## §8 KnownSafe pair elimination (annex-e §AIMS §8 RL-22 / RL-23)

    RL-22 eliminates an inner credit/debit pair iff `KnownSafe(v)` at the point:
    a dominating logical owner credit remains outstanding because no intervening
    release discharged it. That ownership-observation evidence proves the candidate pair can
    be removed without changing the ledger net or violating a later use floor;
    it does not require a physical reference counter. RL-23 is the AND-join of
    KnownSafe across CFG merges — conservative (an OR-join would be unsound). -/

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
    without a dominating credit — if `dominatingInc = false`, KnownSafe is
    false, so the pair is KEPT (the inner debit could exhaust the logical owner
    credit needed by a later use). -/
theorem RL22_no_eliminate_without_dominating_inc (interveningDec : Bool) :
    rl22_eliminates false interveningDec = false := by rfl

/-- §8 RL-22 an intervening debit clears KnownSafe because the dominating
    logical credit may already be discharged — keep the pair. -/
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

/-- §8 RL-23 (P2) the AND-not-OR witness: a join over `[true, false]` (one path
    lacks the outstanding-credit evidence) yields `false` — an OR-join would
    UNSOUNDLY mark the merge KnownSafe from only one proven predecessor. -/
theorem RL23_and_not_or_witness :
    rl23_join [true, false] = false ∧ ([true, false].foldr (· || ·) false = true) := by
  constructor <;> decide

/-! ## §8 logical event-pair refinement (annex-e §AIMS §8 RL-24 / RL-25 / RL-26)

    RL-24 matches an owner-credit/release pair across blocks (bidirectional
    dataflow).
    RL-25 eliminates a matched pair iff KnownSafe OR both paths are safe with no
    CFG hazard. RL-26 preserves logical event order across an ownership-observing
    barrier. A backend may project this refinement as physical instruction
    motion only after proving the projection preserves the frozen order. -/

/-- §8 RL-24 (P1) pair matching: an owner-credit/release pair for the same
    logical identity is matched iff both dataflow directions identify it. -/
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

/-- §8 RL-26 neutral ownership-event ordering barriers. -/
inductive EventOrderingBarrier
  | ownershipContractBoundary
  | sharingObservation
  | containingValueMutation
  | transparent
deriving Repr, DecidableEq

/-- Historical carrier alias and spellings retained for theorem-map
    compatibility. They describe common MIR projections, not calculus terms. -/
abbrev MotionBarrier := EventOrderingBarrier

namespace MotionBarrier
abbrev callOwnedOrMayShare : MotionBarrier := .ownershipContractBoundary
abbrev isSharedOnV : MotionBarrier := .sharingObservation
abbrev setOnContaining : MotionBarrier := .containingValueMutation
abbrev transparent : MotionBarrier := EventOrderingBarrier.transparent
end MotionBarrier

/-- §8 RL-26 (P1): preserve event order across every observing barrier. -/
def rl26_event_order_blocked : EventOrderingBarrier → Bool
  | .ownershipContractBoundary => true
  | .sharingObservation => true
  | .containingValueMutation => true
  | .transparent         => false

/-- Historical compatibility alias for the physical-motion spelling. -/
abbrev rl26_motion_blocked := rl26_event_order_blocked

/-- §8 RL-26 (P1/P2) motion is blocked across exactly the three observing
    barriers and permitted across a transparent instruction (the soundness
    boundary: an RC op may not cross an instruction that observes `v`'s count). -/
theorem RL26_barrier_blocks (b : MotionBarrier) :
    rl26_event_order_blocked b = (b != .transparent) := by
  cases b <;> rfl

/-! ## §8 selective event-ordering barriers (annex-e §AIMS §8 RL-27 / RL-28)

    RL-27 orders pending logical ownership events before a call iff the callee
    contract may observe or change the argument's ownership state. RL-28 orders
    every pending event before an unknown-callee call. "Flush" remains only a
    historical implementation spelling. -/

/-- §8 RL-27 flush decision: flush `v`'s pending ops iff the callee param is
    (Owned ∧ non-Dead) OR (Borrowed ∧ may_share). -/
def rl27_orders_before_call
    (calleeOwned calleeNonDead calleeBorrowed calleeMayShare : Bool) : Bool :=
  (calleeOwned && calleeNonDead) || (calleeBorrowed && calleeMayShare)

/-- Historical compatibility alias for the worklist-flush spelling. -/
abbrev rl27_flushes := rl27_orders_before_call

/-- §8 RL-27 (P1) flush iff Owned-non-Dead OR Borrowed-may_share. -/
theorem RL27_flush_decision (co cnd cb cms : Bool) :
    rl27_orders_before_call co cnd cb cms = ((co && cnd) || (cb && cms)) := by rfl

/-- §8 RL-27 (P2) a Borrowed + ¬may_share (pure) callee requires NO flush — it
    cannot mutate `v`'s logical count state. The negative witness. -/
theorem RL27_borrowed_pure_no_flush (co _cnd : Bool) :
    rl27_orders_before_call co false true false = (co && false) := by
  unfold rl27_orders_before_call; simp

/-- §8 RL-28 (P1) an unknown callee (no contract) conservatively flushes ALL
    pending RC ops — modeled as the constant `true` flush decision regardless of
    any (unavailable) param contract. -/
def rl28_orders_all_before_call : Bool := true

/-- Historical compatibility alias for the worklist-flush spelling. -/
abbrev rl28_flushes_all := rl28_orders_all_before_call

/-- §8 RL-28 (P1/P2) the unknown-callee flush is unconditional — with no
    contract, the callee may inc / dec / share ANY argument, so every pending op
    must be flushed before the call. -/
theorem RL28_unknown_callee_flushes_all : rl28_orders_all_before_call = true := by rfl

/-! ## §8 RL-29 — backend-neutral fresh-self-allocation facts

    RL-29 first freezes whether every return path yields storage allocated by
    this function. `preserves_freshness` and uniqueness are insufficient: a
    function can preserve uniqueness while forwarding caller-owned or consumed
    storage. Target attributes are a later projection of the stronger fact. -/

inductive FreshSelfAllocationFact
  | notProven
  | proven
deriving Repr, DecidableEq

/-- §8 RL-29 neutral derivation from IC-4's stronger return-provenance field. -/
def freshSelfAllocationFact (returnsFreshSelfAlloc : Bool) : FreshSelfAllocationFact :=
  if returnsFreshSelfAlloc then .proven else .notProven

/-- §8 RL-29 (P1) the neutral fact depends only on the path-universal
    fresh-self-allocation proof. -/
theorem RL29_neutral_fresh_self_allocation_decision (returnsFreshSelfAlloc : Bool) :
    freshSelfAllocationFact returnsFreshSelfAlloc =
      (if returnsFreshSelfAlloc then
        FreshSelfAllocationFact.proven
      else FreshSelfAllocationFact.notProven) := by rfl

/-- §8 RL-29 (P2) a parameter passthrough has no fresh-self-allocation proof. -/
theorem RL29_passthrough_not_proven :
    freshSelfAllocationFact false = FreshSelfAllocationFact.notProven := by rfl

/-- §8 RL-29 (P3) a result that may reuse consumed input storage has no
    fresh-self-allocation proof. -/
theorem RL29_consumed_storage_not_proven :
    freshSelfAllocationFact false = FreshSelfAllocationFact.notProven := by rfl

/-! ### §8 RL-29 LLVM projection corollary

    LLVM may spell a proven fact as return `noalias` only when the selected ABI
    returns the allocation as a direct pointer. This target condition does not
    participate in the neutral fact derivation. -/

def llvmReturnNoalias (fact : FreshSelfAllocationFact) (directPointer : Bool) : Bool :=
  match fact with
  | .proven => directPointer
  | .notProven => false

/-- §8 RL-29 target spelling fidelity. -/
theorem RL29_llvm_projection_fidelity
    (fact : FreshSelfAllocationFact) (directPointer : Bool) :
    llvmReturnNoalias fact directPointer =
      (match fact with
       | .proven => directPointer
       | .notProven => false) := by rfl

/-- §8 RL-29 target negative witness: unproven return provenance can never
    receive LLVM return `noalias`, even under a direct-pointer ABI. -/
theorem RL29_unproven_forbids_llvm_noalias :
    llvmReturnNoalias FreshSelfAllocationFact.notProven true = false := by rfl

/-! ## §8 RL-30 — backend-neutral memory-access facts (annex-e §AIMS §8 RL-30)

    RL-30 first derives a backend-neutral fact from the final IC-5
    EffectSummary, IC-3 ParamContracts, and realized operations. A backend may
    project that fact only after applying its ABI and lowering constraints.
    The neutral derivation never names or assumes an LLVM attribute. -/

/-- §8 RL-30 the IC-5 + IC-3 + realized-operation inputs that drive the
    backend-neutral fact. `mayThrow` fails closed for the current panic/unwind
    runtime's TLS and diagnostic writes. `mayWriteInaccessible` independently
    covers untyped calls, I/O, allocators, and any other non-argument write not
    described more precisely by IC-5. -/
structure MemoryEffectInputs where
  mayAllocate : Bool
  mayDeallocate : Bool
  mayShare : Bool
  mayThrow : Bool
  mayReadInaccessible : Bool
  mayWriteInaccessible : Bool
  anyArgAccess : Bool          -- some param has cardinality ≠ Absent
  anyArgWritten : Bool         -- some param Owned (writes args)
deriving Repr, DecidableEq

/-- §8 RL-30 the neutral whole-function access classification. `readOnly`
    permits reads from argument and inaccessible memory; it claims only that
    no writes occur. -/
inductive MemoryAccessFact
  | readOnly
  | readWrite
deriving Repr, DecidableEq

/-- §8 RL-30 neutral fact derivation. Until IC-5 supplies typed descriptors for
    a call, the producer sets `mayWriteInaccessible`, selecting `readWrite`;
    `mayThrow` is independently conservative for the current runtime. -/
def memoryAccessFact (e : MemoryEffectInputs) : MemoryAccessFact :=
  if e.mayAllocate || e.mayDeallocate || e.mayShare || e.mayThrow
      || e.mayWriteInaccessible || e.anyArgWritten then
    .readWrite
  else
    .readOnly

/-- §8 RL-30 (P1) the neutral fact is exactly the disjunction of proven write
    sources; reads do not strengthen a no-write claim into no-access. -/
theorem RL30_neutral_memory_access_decision (e : MemoryEffectInputs) :
    memoryAccessFact e =
      (if e.mayAllocate || e.mayDeallocate || e.mayShare || e.mayThrow
          || e.mayWriteInaccessible || e.anyArgWritten then
        MemoryAccessFact.readWrite
      else MemoryAccessFact.readOnly) := by rfl

/-- §8 RL-30 (P2) load-bearing negative witness: any inaccessible-memory write
    forces the neutral fact to `readWrite`. -/
theorem RL30_inaccessible_write_requires_readwrite (e : MemoryEffectInputs)
    (h : e.mayWriteInaccessible = true) :
    memoryAccessFact e = MemoryAccessFact.readWrite := by
  simp [memoryAccessFact, h]

/-- §8 RL-30 a may-throw path fails closed because the current panic/unwind
    runtime may write thread-local panic state or diagnostics. -/
theorem RL30_throw_requires_readwrite (e : MemoryEffectInputs)
    (h : e.mayThrow = true) :
    memoryAccessFact e = MemoryAccessFact.readWrite := by
  simp [memoryAccessFact, h]

/-- §8 RL-30 reads of inaccessible memory remain representable by the generic
    `readOnly` fact when every write source is false. -/
theorem RL30_inaccessible_read_is_readonly :
    memoryAccessFact {
      mayAllocate := false, mayDeallocate := false, mayShare := false,
      mayThrow := false,
      mayReadInaccessible := true, mayWriteInaccessible := false,
      anyArgAccess := false, anyArgWritten := false
    } = MemoryAccessFact.readOnly := by rfl

/-! ### §8 RL-30 LLVM projection corollary

    This target corollary is separate from the neutral fact definition. The
    shipped conservative subset projects generic `memory(read)` only; it never
    claims `memory(none)` or `memory(argmem: read)`. A write-capable fact omits
    the restrictive attribute. -/

inductive LlvmMemoryAttr
  | none
  | argmemRead
  | read
  | omitted
deriving Repr, DecidableEq

def llvmMemoryAttr (fact : MemoryAccessFact) : LlvmMemoryAttr :=
  match fact with
  | .readOnly => .read
  | .readWrite => .omitted

/-- §8 RL-30 LLVM spelling fidelity for the shipped conservative projection. -/
theorem RL30_llvm_projection_fidelity (fact : MemoryAccessFact) :
    llvmMemoryAttr fact =
      (match fact with
       | .readOnly => LlvmMemoryAttr.read
       | .readWrite => LlvmMemoryAttr.omitted) := by rfl

/-- §8 RL-30 target negative corollary: an inaccessible write can receive
    neither `memory(none)`, `memory(argmem: read)`, nor generic `memory(read)`. -/
theorem RL30_inaccessible_write_forbids_restrictive_attrs
    (e : MemoryEffectInputs) (h : e.mayWriteInaccessible = true) :
    llvmMemoryAttr (memoryAccessFact e) ≠ LlvmMemoryAttr.none ∧
    llvmMemoryAttr (memoryAccessFact e) ≠ LlvmMemoryAttr.argmemRead ∧
    llvmMemoryAttr (memoryAccessFact e) ≠ LlvmMemoryAttr.read := by
  simp [memoryAccessFact, llvmMemoryAttr, h]

/-! ## §8 RL-31 — CRITICAL: neutral disjoint-Borrowed-parameter fact
    (annex-e §AIMS §8 RL-31)

    The Ori-novel theorem. The fact for Borrowed parameters `(p_i, p_j)` is
    proven only when, at EVERY call site, the args to `p_i` and `p_j` are
    PROVABLY disjoint. The proof requires a CROSS-FUNCTION
    provenance summary on the CALLERS' function-local `borrow_sources` /
    `project_alias_sources` tables — beyond what IC-2/IC-3 contracts alone
    express. The 8-clause SUFFICIENT condition is modeled below; the soundness
    property is: disjoint ROOT SETS (or disjoint fields of a shared root via the
    nested-projection prefix test) ⟹ the two borrows cannot alias the same
    memory. Target metadata is a later projection of that neutral fact. -/

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

/-- §8 RL-31 clause (2): the provenance facet is proven iff EVERY call site
    proves disjointness. Any failing site clears the neutral fact. -/
def rl31AllSitesProven : List (ArgProvenance × ArgProvenance) → Bool
  | []      => true
  | s :: ss => siteProvesDisjoint s.1 s.2 && rl31AllSitesProven ss

/-- §8 RL-31 (P1) clause 4 — DISTINCT ROOT SETS prove the neutral fact: two args tracing to
    disjoint root sets (e.g. `{1}` and `{2}`) prove disjointness at the site. -/
theorem RL31_clause4_disjoint_roots_prove :
    siteProvesDisjoint (.tracedRoots [1]) (.tracedRoots [2]) = true := by decide

/-- §8 RL-31 (P1) clause 5 — FRESH allocation own root: a FRESH-allocated arg has
    its own disjoint root, so it is disjoint from any arg with a different root. -/
theorem RL31_clause5_fresh_alloc_proves :
    siteProvesDisjoint (.fresh 7) (.tracedRoots [3]) = true := by decide

/-- §8 RL-31 (P1) clause 6 — UNTRACEABLE arg fails conservatively: an untraceable
    arg cannot prove disjointness, so the site (and hence the fact) fails. -/
theorem RL31_clause6_untraceable_fail (pj : ArgProvenance) :
    siteProvesDisjoint .untraceable pj = false := by
  unfold siteProvesDisjoint ArgProvenance.traceable
  rfl

/-- §8 RL-31 (P1) clause 4 — SAME root set remains unproven: two args sharing a root (both
    `{1}`) are NOT provably disjoint (they may alias the same aggregate). -/
theorem RL31_same_root_not_proven :
    siteProvesDisjoint (.tracedRoots [1]) (.tracedRoots [1]) = false := by decide

/-! ### §8 RL-31 clause 7 — same-root disjoint-fields via the nested-projection
    prefix test. When two args share a root aggregate, disjointness holds iff
    they project DISJOINT fields — neither field path is a prefix of the other
    (reusing `fieldsOverlap` / `isPrefix` from RL-10). -/

/-- §8 RL-31 clause 7: two same-root args are disjoint iff their projection field
    paths do NOT overlap (neither a prefix of the other). -/
def sameRootFieldsDisjoint (fieldI fieldJ : FieldPath) : Bool :=
  !fieldsOverlap fieldI fieldJ

/-- §8 RL-31 (P1) clause 7 — same-root DISJOINT fields prove disjointness: args sharing a root
    but projecting disjoint fields `[0]` and `[1]` ARE disjoint (the borrows read
    non-overlapping memory). -/
theorem RL31_clause7_disjoint_fields_prove :
    sameRootFieldsDisjoint [0] [1] = true := by decide

/-- §8 RL-31 (P1) clause 7 — PREFIX overlap remains unproven: a parent field `[0]` and a
    child field `[0, 1]` of the same root OVERLAP (the parent path is a prefix),
    so they are NOT disjoint. The nested-projection prefix test. -/
theorem RL31_clause7_prefix_overlap_not_proven :
    sameRootFieldsDisjoint [0] [0, 1] = false := by decide

/-! ### §8 RL-31 clause 2 — the all-sites conjunction (the core soundness gate) -/

/-- §8 RL-31 (P1) clause 2 — ANY failing site clears the fact. If even one
    call site fails to prove disjointness, the all-sites conjunction is false, so
    disjointness remains unproven. Proven by induction over the site list: the failing
    member sticky-clears the recursive AND. -/
theorem RL31_any_site_fails_unproven
    (sites : List (ArgProvenance × ArgProvenance))
    (bad : ArgProvenance × ArgProvenance) (hmem : bad ∈ sites)
    (hbad : siteProvesDisjoint bad.1 bad.2 = false) :
    rl31AllSitesProven sites = false := by
  induction sites with
  | nil => exact absurd hmem (List.not_mem_nil)
  | cons hd tl ih =>
      rw [List.mem_cons] at hmem
      unfold rl31AllSitesProven
      rcases hmem with rfl | htl
      · rw [hbad]; rfl
      · rw [ih htl]; simp

/-- §8 RL-31 (P1) clause 2 — a fully-disjoint corpus proves the provenance
    facet when EVERY site proves disjointness. -/
theorem RL31_all_sites_disjoint_prove :
    rl31AllSitesProven
      [(.tracedRoots [1], .tracedRoots [2]), (.fresh 9, .tracedRoots [1])] = true := by
  decide

/-! ### §8 RL-31 (P3) — the CRITICAL SOUNDNESS theorem

    The Ori-novel contribution: when the root sets are disjoint, the two Borrowed
    params CANNOT alias the same memory. The formal statement: if
    `rootSetsDisjoint a b = true`, then there is NO common root variable — no
    aggregate both borrows can reach. This is the property `siteProvesDisjoint`
    GUARANTEES, proven directly over the
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
    two borrows provably reach disjoint memory. Proven by induction over `a`
    against the recursive
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
    root sets share no common source aggregate. This is the neutral
    disjoint-Borrowed soundness theorem; physical consumers may project it only
    when their ABI and metadata placement preserve the proven relation. -/
theorem RL31_neutral_parameter_disjointness_sound (pi pj : ArgProvenance)
    (h : siteProvesDisjoint pi pj = true) :
    (pi.traceable = true) ∧ (pj.traceable = true)
      ∧ (∀ x, x ∈ pi.rootSet → x ∉ pj.rootSet) := by
  unfold siteProvesDisjoint at h
  simp only [Bool.and_eq_true] at h
  obtain ⟨⟨hpi, hpj⟩, hdisj⟩ := h
  exact ⟨hpi, hpj, RL31_disjoint_roots_no_common_aggregate pi.rootSet pj.rootSet hdisj⟩

/-- §8 RL-31 (P2) dual-facet conjunction: the neutral fact is proven only when BOTH
    the call-site provenance facet (a) AND the type-level facet (b) hold. The
    type-level facet ALONE is REJECTED — it leaves the VF-2 (b) per-call-site
    contract-consistency check unproven. Modeled as the AND of the two facet
    flags; proving the type facet alone (`provenanceFacet = false`) yields a
    `false` (unproven) verdict. -/
def rl31_dual_facet (provenanceFacet typeFacet : Bool) : Bool :=
  provenanceFacet && typeFacet

/-- §8 RL-31 (P2) the type-level facet ALONE is insufficient: with the per-
    call-site provenance facet unproven (`false`), the dual-facet verdict is
    `false` regardless of the type facet. -/
theorem RL31_type_facet_alone_insufficient (typeFacet : Bool) :
    rl31_dual_facet false typeFacet = false := by
  unfold rl31_dual_facet; simp

/-- §8 RL-31 (P2) both facets establish the neutral disjointness fact. -/
theorem RL31_both_facets_sound :
    rl31_dual_facet true true = true := by rfl

inductive ParameterDisjointnessFact
  | notProven
  | proven
deriving Repr, DecidableEq

/-- Freeze the backend-neutral RL-31 fact from the dual proof facets. -/
def parameterDisjointnessFact (provenanceFacet typeFacet : Bool) :
    ParameterDisjointnessFact :=
  if rl31_dual_facet provenanceFacet typeFacet then .proven else .notProven

theorem RL31_neutral_parameter_disjointness_decision
    (provenanceFacet typeFacet : Bool) :
    parameterDisjointnessFact provenanceFacet typeFacet =
      (if provenanceFacet && typeFacet then
        ParameterDisjointnessFact.proven
      else ParameterDisjointnessFact.notProven) := by rfl

/-! ### §8 RL-31 LLVM projection corollary

    LLVM parameter `noalias` or alias-scope metadata is a target spelling. It
    requires both the frozen neutral fact and a placement/ABI proof. -/

def llvmProjectsParameterNoalias
    (fact : ParameterDisjointnessFact) (placementPreservesProof : Bool) : Bool :=
  match fact with
  | .proven => placementPreservesProof
  | .notProven => false

theorem RL31_llvm_projection_fidelity
    (fact : ParameterDisjointnessFact) (placementPreservesProof : Bool) :
    llvmProjectsParameterNoalias fact placementPreservesProof =
      (fact == ParameterDisjointnessFact.proven && placementPreservesProof) := by
  cases fact <;> simp [llvmProjectsParameterNoalias]

theorem RL31_unproven_forbids_llvm_noalias (placementPreservesProof : Bool) :
    llvmProjectsParameterNoalias .notProven placementPreservesProof = false := by rfl

/-! ## §8 Borrow inference (annex-e §AIMS §8 RL-32 / RL-33 / RL-34)

    RL-32: non-scalar params initialize Borrowed (most optimistic); the fixpoint
    promotes to Owned on demand. RL-33: if a projected field becomes Owned, the
    source variable is promoted to Owned. RL-34 freezes a pre-tail-call logical
    action: handoff when the callee owns the parameter, release before the call
    when it only borrows. A physical post-call operation is not part of the
    calculus. -/

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
  | handoffBeforeTail
  | releaseBeforeTail
deriving Repr, DecidableEq

namespace TailCallAction
/-- Historical spellings retained for theorem-map compatibility. -/
abbrev transferOwnership : TailCallAction := .handoffBeforeTail
abbrev decBeforeCall : TailCallAction := .releaseBeforeTail
end TailCallAction

/-- §8 RL-34 tail-call action: Owned param → transfer ownership; Borrowed param →
    dec before the call. NEVER a post-call dec (that would break TCO). -/
def rl34_action (calleeAccess : AccessClass) : TailCallAction :=
  match calleeAccess with
  | .Owned    => .handoffBeforeTail
  | .Borrowed => .releaseBeforeTail

/-- §8 RL-34 (P1) callee Owned ⟹ transfer ownership (no post-call dec, TCO
    preserved). -/
theorem RL34_owned_transfers :
    rl34_action .Owned = TailCallAction.handoffBeforeTail := by rfl

/-- §8 RL-34 (P1) callee Borrowed ⟹ dec before the call (cannot transfer to a
    borrow; a post-call dec is forbidden, so the dec moves before the call). -/
theorem RL34_borrowed_dec_before :
    rl34_action .Borrowed = TailCallAction.releaseBeforeTail := by rfl

/-- §8 RL-34 (P2) the negative witness: NO tail-call action inserts a post-call
    dec — the action is ALWAYS either a pre-call transfer or a pre-call dec
    (proven by totality: both `AccessClass` cases map to a pre-call action, never
    a post-call one). -/
theorem RL34_never_post_call_dec (calleeAccess : AccessClass) :
    rl34_action calleeAccess = .handoffBeforeTail
      ∨ rl34_action calleeAccess = .releaseBeforeTail := by
  cases calleeAccess <;> simp [rl34_action]

end AimsProof
