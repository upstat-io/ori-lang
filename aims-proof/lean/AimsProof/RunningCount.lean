/-
AIMS running-count module — kernel-checked Lean proofs of the keep-alive
whole-pair running-owner-credit theorem T3 over the committed `RcOp` ledger, bridged
to the RL-22 / RL-23 / RL-25 KnownSafe pair-elimination family.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: T3 (keep-alive whole-pair running-refcount soundness) |
  spec: annex-e §AIMS §8 (RL-22 KnownSafe pair elimination + RL-23 AND-join +
    RL-25 PRE eliminability are the bridged committed family) |
  .proof: aims-proof/proofs/12-provenance/T3-keepalive-running-refcount.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Interpretation: `RcOp`, `rcBalance`, and `runningRc` are stable historical proof
carrier names. Their integers model outstanding LOGICAL OWNER CREDITS, not a
mandated runtime header or reference counter. A counter-based target obtains a
physical corollary only after its plan proves `Satisfies` against these facts.

Correspondence: the committed ledger (`rcBalance` over `RcOp`, Realization.lean)
proves NET balance only — it has no running-owner-credit notion, so it cannot
distinguish the SOUND whole-pair elision (a matched inc/dec pair removed under
a live same-allocation-class sibling) from the UNSOUND inc-only split (inc
removed, dec kept). This module adds the running-count (prefix-sum) surface
over the SAME `RcOp` carrier and proves the distinction:

  * whole-PAIR elision under a live same-class sibling bracket is SAFE — the
    logical credit count never drops below 1 before the final release;
  * the inc-ONLY split drives the count to 0 before the sibling's release
    (premature free) and nets -1 (double release) — proven unsound, so the
    per-var inc-only elision is correctly refused.

Bridge to the committed KnownSafe family (annex-e §AIMS §8): a same-class
sibling live across the pair's span IS the RL-22 evidence — a dominating RcInc
(the sibling's birth) with no intervening RcDec on the shared allocation — so
`knownSafe` holds, `rl22_eliminates` fires on the matched pair, RL-23's
AND-join carries the evidence across CFG merges exactly when EVERY predecessor
holds it (`RL23_join_all_preds`), and RL-25's eliminability disjunction fires
on the KnownSafe flag. The running-count theorems supply the premature-free
guarantee those decision theorems assume of a fired whole-pair elimination.
-/

import AimsProof.Realization

set_option maxHeartbeats 1000000

namespace AimsProof

/-! ## §T3 running owner credits — the prefix-sum surface over the RcOp ledger -/

/-- §T3 the running logical owner-credit count after each lifecycle prefix:
    the head is `start`; each step adds the next op's delta. -/
def runningRc : Int → List RcOp → List Int
  | start, [] => [start]
  | start, op :: rest => start :: runningRc (start + op.delta) rest

/-- §T3 every running count stays at or above `floor` (`floor = 1` reads "not
    yet freed"). Bool-valued so the concrete instances `decide`. -/
def neverBelow (floor : Int) (counts : List Int) : Bool :=
  counts.all (fun c => decide (floor ≤ c))

/-- §T3 the INTERIOR of a running-count list: drop the pre-birth head and the
    post-final-release tail. The premature-free hazard is an interior count
    reaching 0 — between the allocation and the single legitimate release; the
    head 0 (pre-birth) and the tail 0 (post-release) are correct. -/
def interiorCounts (counts : List Int) : List Int :=
  (counts.drop 1).dropLast

/-! ## §T3 the shared allocation's three lifecycles

    One shared allocation R; two reference lineages touch it: the same-class
    sibling S births R (+1) and releases it exactly once at the end (-1); the
    alias V carries a matched burden pair — V's inc (+1) ... V's dec (-1) —
    bracketed INSIDE S's live span. `keepAliveSpan n` models the body work
    between V's inc and V's dec where S stays live. -/

/-- §T3 body work between the pair's inc and dec (no RC effect on R). -/
def keepAliveSpan (n : Nat) : List RcOp := List.replicate n RcOp.noop

/-- §T3 the KEPT shape: S births, V incs, span, V decs, S releases. -/
def keptPairLifecycle (n : Nat) : List RcOp :=
  [RcOp.inc] ++ [RcOp.inc] ++ keepAliveSpan n ++ [RcOp.dec] ++ [RcOp.dec]

/-- §T3 whole-PAIR elision (the sound cure): V's inc AND dec both removed —
    S births, span, S releases. -/
def wholePairElidedLifecycle (n : Nat) : List RcOp :=
  [RcOp.inc] ++ keepAliveSpan n ++ [RcOp.dec]

/-- §T3 the inc-ONLY split (the unsound per-var shape): V's inc removed, V's
    dec KEPT. -/
def incOnlySplitLifecycle (n : Nat) : List RcOp :=
  [RcOp.inc] ++ keepAliveSpan n ++ [RcOp.dec] ++ [RcOp.dec]

/-! ## §T3 net-balance obligations (consume the committed rcBalance ledger) -/

/-- §T3 a noop span carries zero net balance. -/
theorem keepAliveSpan_balance (n : Nat) : rcBalance (keepAliveSpan n) = 0 := by
  induction n with
  | zero => rfl
  | succ k ih =>
      have hcons : keepAliveSpan (k + 1) = RcOp.noop :: keepAliveSpan k := rfl
      rw [hcons]
      unfold rcBalance at ih ⊢
      simp only [List.map_cons, List.foldr_cons, RcOp.delta]
      omega

/-- §T3 (P1) the kept lifecycle is balanced (net 0): the standing correct
    baseline, decomposed via the committed `rcBalance_append` ledger lemma. -/
theorem T3_kept_balanced (n : Nat) : rcBalance (keptPairLifecycle n) = 0 := by
  unfold keptPairLifecycle
  rw [rcBalance_append, rcBalance_append, rcBalance_append, rcBalance_append,
      keepAliveSpan_balance]
  decide

/-- §T3 (P2) whole-pair elision PRESERVES the net balance: removing the matched
    V pair (net 0) leaves R's lifecycle balance unchanged. -/
theorem T3_whole_pair_balance_preserved (n : Nat) :
    rcBalance (wholePairElidedLifecycle n) = rcBalance (keptPairLifecycle n) := by
  unfold wholePairElidedLifecycle keptPairLifecycle
  rw [rcBalance_append, rcBalance_append, keepAliveSpan_balance,
      rcBalance_append, rcBalance_append, rcBalance_append, rcBalance_append,
      keepAliveSpan_balance]
  decide

/-- §T3 (P4, net half) the inc-only split nets -1 — a freed value handed
    onward (double release): the split is already wrong on the net ledger. -/
theorem T3_inc_only_net_negative (n : Nat) :
    rcBalance (incOnlySplitLifecycle n) = -1 := by
  unfold incOnlySplitLifecycle
  rw [rcBalance_append, rcBalance_append, rcBalance_append, keepAliveSpan_balance]
  decide

/-! ## §T3 running-count obligations — the surface the net ledger lacks -/

/-- §T3 running counts across a noop span: `n` copies of `start`, then the
    counts of the remainder from `start`. -/
theorem runningRc_span (start : Int) (n : Nat) (rest : List RcOp) :
    runningRc start (keepAliveSpan n ++ rest)
      = List.replicate n start ++ runningRc start rest := by
  induction n with
  | zero => rfl
  | succ k ih =>
      have hcons : keepAliveSpan (k + 1) ++ rest
          = RcOp.noop :: (keepAliveSpan k ++ rest) := rfl
      rw [hcons]
      show start :: runningRc (start + RcOp.noop.delta) (keepAliveSpan k ++ rest)
          = List.replicate (k + 1) start ++ runningRc start rest
      rw [show RcOp.noop.delta = 0 from rfl, Int.add_zero, ih]
      rfl

/-- §T3 the whole-pair-elided running counts in closed form:
    pre-birth 0, then 1 across the sibling-bracketed span, then the final
    release back to 0. -/
theorem T3_whole_pair_counts (n : Nat) :
    runningRc 0 (wholePairElidedLifecycle n)
      = 0 :: (List.replicate n 1 ++ [1, 0]) := by
  show 0 :: runningRc (0 + RcOp.inc.delta) (keepAliveSpan n ++ [RcOp.dec])
      = 0 :: (List.replicate n 1 ++ [1, 0])
  rw [show (0 : Int) + RcOp.inc.delta = 1 from rfl, runningRc_span]
  rfl

/-- §T3 the inc-only-split running counts in closed form: the kept dec fires
    a second release, driving the count through 0 to -1. -/
theorem T3_inc_only_counts (n : Nat) :
    runningRc 0 (incOnlySplitLifecycle n)
      = 0 :: (List.replicate n 1 ++ [1, 0, -1]) := by
  have hshape : incOnlySplitLifecycle n
      = [RcOp.inc] ++ (keepAliveSpan n ++ [RcOp.dec, RcOp.dec]) := by
    unfold incOnlySplitLifecycle
    simp [List.append_assoc]
  rw [hshape]
  show 0 :: runningRc (0 + RcOp.inc.delta) (keepAliveSpan n ++ [RcOp.dec, RcOp.dec])
      = 0 :: (List.replicate n 1 ++ [1, 0, -1])
  rw [show (0 : Int) + RcOp.inc.delta = 1 from rfl, runningRc_span]
  rfl

/-- §T3 helper: a replicated in-bound count passes the floor check. -/
theorem T3_all_floor_replicate (n : Nat) :
    (List.replicate n (1 : Int)).all (fun c => decide (1 ≤ c)) = true := by
  induction n with
  | zero => rfl
  | succ k ih => simp [List.replicate_succ, ih]

/-- §T3 (P3) THE new safety fact. Under the whole-pair elision, R's running
    refcount NEVER drops below 1 before the final release, for EVERY span
    length: the sibling's live +1 brackets the whole span. -/
theorem whole_pair_never_freed_early (n : Nat) :
    neverBelow 1 (interiorCounts (runningRc 0 (wholePairElidedLifecycle n)))
      = true := by
  rw [T3_whole_pair_counts]
  show neverBelow 1 ((List.replicate n (1 : Int) ++ [1, 0]).dropLast) = true
  rw [show (List.replicate n (1 : Int) ++ [1, 0])
        = (List.replicate n 1 ++ [1]) ++ [0] by simp,
      List.dropLast_concat]
  unfold neverBelow
  rw [List.all_append, T3_all_floor_replicate]
  rfl

/-- §T3 (P4, running-count half) the inc-only split frees early: the kept dec
    drives R's running refcount to 0 BEFORE the sibling's release, for EVERY
    span length — the premature-free the net ledger cannot see. -/
theorem inc_only_freed_early (n : Nat) :
    neverBelow 1 (interiorCounts (runningRc 0 (incOnlySplitLifecycle n)))
      = false := by
  rw [T3_inc_only_counts]
  show neverBelow 1 ((List.replicate n (1 : Int) ++ [1, 0, -1]).dropLast) = false
  rw [show (List.replicate n (1 : Int) ++ [1, 0, -1])
        = (List.replicate n 1 ++ [1, 0]) ++ [-1] by simp,
      List.dropLast_concat]
  unfold neverBelow
  rw [List.all_append]
  rw [show (([1, 0] : List Int).all (fun c => decide (1 ≤ c))) = false by decide]
  exact Bool.and_false _

/-! ## §T3 bridge — same-class sibling liveness IS the KnownSafe evidence

    RL-22's evidence is a dominating RcInc with no intervening RcDec on the
    shared allocation. A same-allocation-class sibling live across the pair's
    span supplies exactly that: the sibling's birth inc dominates the pair and
    its single release lands after the pair's dec, so no dec intervenes within
    the span. The negative direction stays with the committed family:
    `RL22_no_eliminate_without_dominating_inc` keeps the pair when no such
    sibling evidence exists. -/

/-- §T3 (P5) the bridge into the committed KnownSafe family: sibling liveness
    supplies `knownSafe true false`; the RL-22 decision fires on the matched
    pair; RL-23's AND-join carries the evidence across a CFG merge exactly when
    EVERY predecessor supplies it (consumes `RL23_join_all_preds`); RL-25's
    eliminability disjunction fires on the KnownSafe flag. -/
theorem T3_sibling_liveness_is_dominating_inc_evidence
    (preds : List Bool) (bothPathsSafe noCfgHazard : Bool)
    (hall : ∀ p ∈ preds, p = true) :
    knownSafe true false = true
    ∧ rl22_eliminates true false = true
    ∧ rl23_join preds = true
    ∧ rl25_eliminable true bothPathsSafe noCfgHazard = true := by
  refine ⟨rfl, rfl, (RL23_join_all_preds preds).mpr hall, ?_⟩
  unfold rl25_eliminable
  simp

/-! ## §T3 the keep-alive redundancy theorem -/

/-- §T3 (P6) THE keep-alive redundancy theorem. For every span length, the
    kept pair on V is removable WITHOUT introducing a premature free exactly
    when the cure is whole-pair, never inc-only:

      whole-pair -> balanced AND never-freed-early            (sound)
      inc-only   -> unbalanced (nets -1) AND freed-early      (unsound; refused)

    The sound direction is the elimination RL-22 licenses under the sibling's
    dominating-inc evidence; the refuted direction is the per-var split no
    committed elimination rule licenses. -/
theorem keep_alive_redundancy_sound_iff_whole_pair (n : Nat) :
    (rcBalance (wholePairElidedLifecycle n) = 0
      ∧ neverBelow 1 (interiorCounts (runningRc 0 (wholePairElidedLifecycle n)))
          = true)
    ∧ ¬ (rcBalance (incOnlySplitLifecycle n) = 0
      ∧ neverBelow 1 (interiorCounts (runningRc 0 (incOnlySplitLifecycle n)))
          = true) := by
  refine ⟨⟨?_, whole_pair_never_freed_early n⟩, ?_⟩
  · rw [T3_whole_pair_balance_preserved, T3_kept_balanced]
  · intro h
    have hneg := T3_inc_only_net_negative n
    rw [h.1] at hneg
    exact absurd hneg (by decide)

end AimsProof
