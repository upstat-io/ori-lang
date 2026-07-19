/-
AIMS per-field release-decomposition module — kernel-checked Lean proof of the
T6 skip-set derivation theorem over the committed T2 per-class CFG-ledger
carrier: a container release decomposed per named owned field (the
`BurdenDecPartial skip_fields` shape) preserves every payload class's clause-1
exactly-once ownership IFF the skip set equals the partition's consume marks.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: PV-6 (per-field release decomposition — the skip set derived from the
    partition's consume marks is the UNIQUE clause-preserving skip set) |
  spec: IA-category posture per the IA-MF1 precedent — implementer-internal;
    Annex E §AIMS carries prose only, never PV/IA rule ids |
  .proof: aims-proof/proofs/12-provenance/T6-per-field-decomposition.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: governs the class-ledger planner's per-field aggregate
decomposition (`compiler/ori_arc/src/aims/class_ledger/` DecPartial planning):
a by-value aggregate container C holds named owned heap fields; payload P is
born (+1 on P's class) and stored into C (the Construct's owned arg — the
STORE-consume, -1: ownership enters C's recursive-release books). A payload
later extracted (Project view) and MOVED OUT (a transferring terminal use per
the committed RL-2 table — the CONSUME MARK) is double-freed by C's whole-var
release (net -1: the endangered field-view class the planner declines today).
The decomposed release `DecPartial(skip S)` walks no skipped field, so the
plan RE-BOOKS the store as non-consuming for skipped fields. The four cells:

  moved=F skipped=F : birth, store           -> net  0  (glue frees it)
  moved=T skipped=F : birth, store, move-out -> net -1  DOUBLE-FREE (reject)
  moved=T skipped=T : birth, move-out        -> net  0  (transferee owns)
  moved=F skipped=T : birth                  -> net +1  LEAK (reject)

Scope honesty: T6 governs CLAUSE 1 (exactly-once ownership) of the
decomposition; clauses 2/3 (UAF read floors, dynamic-COW sibling floors)
remain enforced by the UNCHANGED committed T2 verifier over the replanned
streams — the decomposition adds no read/mutate events, and the balanced
cells are proven safe end-to-end through the committed clauses<->safety
bridge. The container's own class books the SAME single whole-var consume
under DecPartial as under Dec (frame theorems below); classes disjoint from
the container derive verbatim-empty ledgers from the release.

Consumes (NO change to committed theorems): AimsProof.Ledger (LedgerEvent,
deriveLedger, runLedger, threeClauses, three_clauses_iff_ledger_safe);
AimsProof.Realization (rl2_use_transfers_ownership — the consume mark
classifies through the committed twelve-kind table, never free input).
-/
import AimsProof.Ledger

namespace AimsProof

/-! ## §T6 — the four-cell payload lifecycle -/

/-- §T6 the payload-class event skeleton per (moved, skipped) cell: birth at
    the payload's Construct; the STORE-consume unless the field is skipped
    (a skipped field's ownership never enters the container's release path);
    the MOVE-OUT consume iff the payload was extracted and transferred. -/
def payloadEvents (moved skipped : Bool) : List LedgerEvent :=
  .birth :: ((if skipped then [] else [.consume]) ++ (if moved then [.consume] else []))

/-- §T6 THE four-cell balance verdict: the payload class satisfies the T2
    three clauses exactly when the skip verdict matches the move mark. -/
theorem FD_cell_balanced_iff (moved skipped : Bool) :
    threeClauses (payloadEvents moved skipped) = (skipped == moved) := by
  cases moved <;> cases skipped <;> rfl

/-- §T6 under-skip named negative witness: a moved-out field NOT skipped nets
    -1 — the double free the field-view hazard declines today. -/
theorem FD_under_skip_double_free :
    (runLedger (payloadEvents true false)).count = -1 := by rfl

/-- §T6 over-skip named negative witness: an unmoved field skipped nets +1 —
    the leak (nobody releases it). -/
theorem FD_over_skip_leak :
    (runLedger (payloadEvents false true)).count = 1 := by rfl

/-- §T6 both balanced cells are safe end-to-end (the committed clauses<->safety
    bridge applied to the decomposition's streams). -/
theorem FD_balanced_cells_safe (b : Bool) :
    ledgerSafe (payloadEvents b b) := by
  have h : threeClauses (payloadEvents b b) = true := by
    rw [FD_cell_balanced_iff]; simp
  exact (three_clauses_iff_ledger_safe (payloadEvents b b)).mp h

/-! ## §T6 — the skip-set derivation theorem -/

/-- §T6 (P1) THE derivation soundness: over any named-field set, skipping
    exactly the consume-marked fields balances EVERY payload class per the
    committed threeClauses, and any deviation on any field breaks that
    field's clauses — the skip set derived from the partition's consume
    marks is the UNIQUE clause-preserving skip set. -/
theorem FD_skipset_sound {F : Type} (fields : List F) (marked skip : F → Bool) :
    (∀ f ∈ fields, threeClauses (payloadEvents (marked f) (skip f)) = true)
      ↔ (∀ f ∈ fields, skip f = marked f) := by
  constructor
  · intro h f hf
    have hc := h f hf
    rw [FD_cell_balanced_iff] at hc
    exact eq_of_beq hc
  · intro h f hf
    rw [FD_cell_balanced_iff, h f hf]
    simp

/-! ## §T6 — the expansion frame over the committed instruction carrier -/

/-- §T6 DecPartial's per-class derivation model over the committed
    `LedgerInstr` carrier: the container's own class books ONE whole-var
    consume (the burdenDec on the container var); skipped fields contribute
    NO event to any class — the skip set only silences interior field walks.
    (An unskipped field's accounting is the STORE-consume already booked at
    the container's Construct — the decomposition adds no second event.) -/
def decPartialInstrs (v : Nat) : List LedgerInstr := [.burdenDec v]

/-- §T6 (P2) container-class verbatim: DecPartial derives the SAME single
    consume on the container's class as the whole-var Dec — the skip set
    never changes the container's own books. -/
theorem FD_container_class_verbatim (classOf : Nat → Nat) (v : Nat) :
    deriveLedger classOf (classOf v) (decPartialInstrs v) = [.consume] := by
  simp [decPartialInstrs, deriveLedger]

/-- §T6 (P2) untouched-class frame (T5 style): every class other than the
    container's derives the verbatim-empty ledger from the release — the
    decomposition perturbs no disjoint class. -/
theorem FD_untouched_class_verbatim (classOf : Nat → Nat) (v : Nat) (c : Nat)
    (hc : classOf v ≠ c) :
    deriveLedger classOf c (decPartialInstrs v) = [] := by
  simp [decPartialInstrs, deriveLedger]
  intro h
  exact absurd h hc

/-! ## §T6 — end-to-end concrete instantiation (the struct_list_field shape)

    Container class 0 (by-value aggregate), two owned heap fields: payload
    class 1 (items — EXTRACTED THEN MOVED OUT, consume-marked) and payload
    class 2 (label — container-released, unmarked). -/

/-- §T6 (P4) the CURED composition, skip = {the consume-marked field}. -/
theorem FD_cured_moved_field_balanced :
    threeClauses (payloadEvents true true) = true := by rfl

/-- §T6 (P4) the CURED composition, the unmarked sibling stays on the glue. -/
theorem FD_cured_unmoved_field_balanced :
    threeClauses (payloadEvents false false) = true := by rfl

/-- §T6 (P4) the BUGGY composition: whole-var release (skip = ∅) on the same
    move — the moved field's class nets -1 and fails the clauses: the double
    free the field-view hazard declines today, now the named rejection
    witness. -/
theorem FD_buggy_whole_var_release_rejected :
    (runLedger (payloadEvents true false)).count = -1 ∧
      threeClauses (payloadEvents true false) = false :=
  ⟨rfl, rfl⟩

/-- §T6 (P4) the consume mark classifies through the COMMITTED RL-2
    twelve-kind table, never free input: a `ConstructArg` position transfers
    ownership (the extract-then-store-into-new-container shape). -/
theorem FD_moveout_is_committed_transfer :
    rl2_use_transfers_ownership .ConstructArg = true := by rfl

/-- §T6 (P4) the extraction view itself (`ApplyToBorrowedParam` read) does
    NOT consume — a merely-read (demand-endangered) view is NEVER
    consume-marked, so the derivation never skips it (over-skip = leak,
    rejected above). -/
theorem FD_read_is_not_transfer :
    rl2_use_transfers_ownership .ApplyToBorrowedParam = false := by rfl

/-! ## §T6 — per-SITE skip refinement (path-dependent move marks) -/

/-- §T6 (P5) per-SITE skip soundness: over a path universe where each path
    carries its own moved mark and passes exactly one release site, the
    per-path clause verdict holds exactly when each path's site skip verdict
    matches that path's move mark — the pathwise generalization of
    `FD_skipset_sound` for shapes whose move is path-DEPENDENT (a
    take-project source enum: the payload moves out on the extraction path
    and stays on the bypass path). -/
theorem FD_per_site_skipset_sound {P : Type} (paths : List P)
    (moved : P → Bool) (site_skip : P → Bool) :
    (∀ p ∈ paths, threeClauses (payloadEvents (moved p) (site_skip p)) = true)
      ↔ (∀ p ∈ paths, site_skip p = moved p) := by
  constructor
  · intro h p hp
    have hc := h p hp
    rw [FD_cell_balanced_iff] at hc
    exact eq_of_beq hc
  · intro h p hp
    rw [FD_cell_balanced_iff, h p hp]
    simp

/-- §T6 (P5) the site-verdict projection: when every path through a release
    site shares one move mark (the per-site arm-safety condition — a skip
    site is extraction-dominated, a whole-var site is extraction-free), a
    per-site skip assignment matching each site's uniform mark balances
    every path. -/
theorem FD_site_uniform_projection {P S : Type} (paths : List P)
    (moved : P → Bool) (site_of : P → S) (skip_at : S → Bool)
    (uniform : ∀ p ∈ paths, skip_at (site_of p) = moved p) :
    ∀ p ∈ paths, threeClauses (payloadEvents (moved p) (skip_at (site_of p))) = true := by
  intro p hp
  rw [FD_cell_balanced_iff, uniform p hp]
  simp

end AimsProof
