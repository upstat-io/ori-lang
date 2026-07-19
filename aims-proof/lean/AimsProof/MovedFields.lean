/-
AIMS moved-out-fields convergence module — kernel-checked Lean proof of the
bounded MUST-move INTERSECT fixpoint convergence rule IA-MF1 over the Phase-5
burden-lowering moved-out-fields per-block lattice.

Evidence-tie (4-anchor evidence cross-tie — rule <-> spec <-> .proof <-> Lean):
  rules: IA-MF1 (moved-out-fields INTERSECT-fixpoint bounded convergence) |
  spec: IA-category (implementer-internal analysis rule) — no Annex E §AIMS
    clause; IA-MF1 is not part of the public algorithmic contract |
  .proof: aims-proof/proofs/04-transfers/IA-MF1-moved-fields-convergence.proof |
  map: aims-proof/scripts/proof-lean-map.json (theorem -> rule/spec/proof/lean).

Correspondence: rule IA-MF1 governs the bounded MUST-move INTERSECT fixpoint
convergence — a finite per-block lattice height bounds the round count.
IA-category (implementer-internal); no Annex E §AIMS anchor. The governing
source file, the `.proof` artifact, and the proof-lean map are cited in the
evidence-tie above.

Governs `compiler/ori_arc/src/lower/burden_lower/moved_fields/dataflow.rs`
`propagate_moved_out_fields`: the forward INTERSECT-at-entry / union-with-local
dataflow over per-block moved-field sets. The analysis is monotone-DESCENDING
from the ⊤ (universe) seed at non-entry blocks; each non-fixpoint round strictly
removes at least one `(var, field)` pair from some block's exit set. The global
lattice height is `n_blocks × universe_pair_count` (universe_pair_count = the
count of distinct `(project_src, field)` pairs any block can move), so a
round-robin sweep reaches a fixpoint within that many descending rounds. The
release-active guard's abort (`changed == true` at the derived cap) is therefore
UNREACHABLE on valid input — only a compiler bug / synthetic input can trip it,
which is what makes the release-active hard-failure crash-safe (Q3).

Reuses `AimsProof.IC7_bounded_monotone_converges` (the abstract ascending
bounded-monotone convergence lemma) via the complement-measure transform
(`measure := bound - size`), turning the descending chain into an ascending one.
-/
import AimsProof.Interprocedural

namespace AimsProof

/-! ## §MF-1 — moved-out-fields INTERSECT-fixpoint bounded convergence

    A round-robin monotone-DESCENDING dataflow over a finite product lattice of
    height `bound` reaches a fixpoint within `bound + 1` rounds; at the derived
    cap the round observes no change. The `bound` is the concrete finite lattice
    height `n_blocks × universe_pair_count` (grounded by
    `MF1_lattice_height_realizable`). -/

/-- `iterate` pushes one more step OUT the front:
    `iterate f (n+1) s = f (iterate f n s)`. -/
theorem MF1_iterate_succ_comm {α : Type} (f : α → α) (n : Nat) (s : α) :
    iterate f (n + 1) s = f (iterate f n s) := by
  induction n generalizing s with
  | zero => rfl
  | succ n ih => simpa [iterate] using ih (f s)

/-- §MF-1 descending-fixpoint convergence. A step with a `Nat` `size` bounded by
    `bound` that, at each point, either is a fixpoint OR strictly shrinks `size`
    reaches a fixpoint within `bound + 1` iterations. Proven by the complement
    measure `bound - size` + `IC7_bounded_monotone_converges` (each strict shrink
    strictly increases the complement, which is bounded by `bound`). -/
theorem MF1_descending_converges
    {S : Type} (step : S → S) (size : S → Nat) (bound : Nat)
    (sizeBounded : ∀ s, size s ≤ bound)
    (shrinkOrFixed : ∀ s, step s = s ∨ size (step s) < size s) :
    ∀ s, ∃ k, k ≤ bound + 1 ∧ step (iterate step k s) = iterate step k s := by
  intro s
  have measBounded : ∀ t, (bound - size t) ≤ bound := by intro t; omega
  have measStrictOrFixed :
      ∀ t, step t = t ∨ (bound - size t) < (bound - size (step t)) := by
    intro t
    rcases shrinkOrFixed t with hfix | hstrict
    · exact Or.inl hfix
    · have hb := sizeBounded t
      exact Or.inr (by omega)
  obtain ⟨k, hk, hfix⟩ :=
    IC7_bounded_monotone_converges step (fun t => bound - size t) bound
      measBounded measStrictOrFixed s
  exact ⟨k, by omega, hfix⟩

/-- A reached fixpoint persists: iterating past it does not move. -/
theorem MF1_fixpoint_persists {α : Type} (f : α → α) (k : Nat) (s : α)
    (hfix : f (iterate f k s) = iterate f k s) :
    ∀ d, iterate f (k + d) s = iterate f k s := by
  intro d
  induction d with
  | zero => rfl
  | succ d ih =>
      have hcomm : iterate f (k + d + 1) s = f (iterate f (k + d) s) :=
        MF1_iterate_succ_comm f (k + d) s
      show iterate f (k + d + 1) s = iterate f k s
      rw [hcomm, ih]; exact hfix

/-- §MF-1 THE load-bearing theorem. At the DERIVED cap `bound + 1` (where
    `bound` is the finite lattice height `n_blocks × universe_pair_count`), the
    moved-out-fields INTERSECT fixpoint has already reached a fixpoint — running
    the cap-th round observes NO change (`changed == false`). The release-active
    guard's abort is therefore unreachable on any valid input; only a genuine
    bug / synthetic input can trip it (crash-safety, Q3). -/
theorem MF1_no_change_at_derived_cap
    {S : Type} (step : S → S) (size : S → Nat) (bound : Nat)
    (sizeBounded : ∀ s, size s ≤ bound)
    (shrinkOrFixed : ∀ s, step s = s ∨ size (step s) < size s) :
    ∀ s, step (iterate step (bound + 1) s) = iterate step (bound + 1) s := by
  intro s
  obtain ⟨k, hk, hfix⟩ := MF1_descending_converges step size bound sizeBounded shrinkOrFixed s
  have hpersist := MF1_fixpoint_persists step k s hfix (bound + 1 - k)
  have hcap : k + (bound + 1 - k) = bound + 1 := by omega
  rw [hcap] at hpersist
  have hstep : step (iterate step (bound + 1) s) = step (iterate step k s) := by
    rw [hpersist]
  rw [hstep, hfix, hpersist]

/-- §MF-1 concrete lattice-height realizability. The total moved-pair count
    across `n` per-block slots, each holding at most `u` pairs, is at most
    `n * u`. Grounds the `sizeBounded` hypothesis of `MF1_no_change_at_derived_cap`
    with `bound = n_blocks × universe_pair_count`: the finite lattice height is a
    real fact of the per-block-subset model, not an axiom. -/
theorem MF1_lattice_height_realizable (u : Nat) :
    ∀ (sizes : List Nat), (∀ x ∈ sizes, x ≤ u) →
      sizes.foldr (· + ·) 0 ≤ sizes.length * u := by
  intro sizes
  induction sizes with
  | nil => intro _; simp
  | cons hd tl ih =>
      intro hall
      have hhd : hd ≤ u := hall hd (List.mem_cons.mpr (Or.inl rfl))
      have htl : ∀ x ∈ tl, x ≤ u := fun x hx => hall x (List.mem_cons.mpr (Or.inr hx))
      have hsum := ih htl
      simp only [List.foldr_cons, List.length_cons]
      calc hd + tl.foldr (· + ·) 0
          ≤ u + tl.length * u := by omega
        _ = (tl.length + 1) * u := by rw [Nat.add_mul, Nat.one_mul]; omega

end AimsProof
