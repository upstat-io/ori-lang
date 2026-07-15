## §11 Coexistence Handshake Proofs — Corpus Overview

Discharges the burden-prototype minimal-lattice-consumer coexistence handshake — Ori's
novel composition that no reference compiler has. Contributes to the
Ori-novel coexistence-proof mission
via 6 individually-discharged CH theorems (CH-1..CH-5 + CH-comp).

- Per-CH proof_search_cap: 5
- PRIMARY engines per CH: structural_induction + interprocedural_summary +
  case_analysis + lattice (per aims-proof/proofs/01-theorems/Composition.proof
  lines 28-32, 59-63, 93-97, 126-130, 165-169)

### Corpus inventory

| File | Subject | Skeleton | proof_status | Discharged-in-this-dispatch |
|---|---|---|---|---|
| Handshake.proof | Formal specification SSOT (Predicates 1-2 + Functions 1-3 + Partition) | — | definition_only | YES |
| CH-1.proof | Burden-registry-lattice composition soundness | Composition.proof:1 | valid (attempted) | YES |
| CH-2.proof | DP-2/DP-3 elimination consumer composition with predicate-stack-derived ops | Composition.proof:38 | pending | NO (subsequent §11.1 dispatch) |
| CH-3.proof | Per-class coexistence three sub-classes (Owned×Linear×Once×Unique RL-2 logical release, Borrowed×Linear×Once×Unique caller-owned, MaybeShared×Many RL-7 dynamic COW); allocation facts are orthogonal | Composition.proof:69 | pending | NO (subsequent §11.1 dispatch) |
| CH-4.proof | AimsStateMap immutability under burden-registry mutation | Composition.proof:103 | pending | NO (subsequent §11.1 dispatch) |
| CH-5.proof | Phase-ordering composition (PL-1 interprocedural-first + burden-registry as Step 1 typed pre-pass) | Composition.proof:136 | pending | NO (subsequent §11.1 dispatch) |
| CH-comp.proof | CH-1..CH-5 union-soundness composition (mirror §09 VF-comp pattern) | (to be authored at §11.1 execution) | pending | NO (subsequent §11.1 dispatch) |

### Per-CH dependency chain (per §11.1 body table)

| CH-N | Depends on |
|---|---|
| CH-1 | (root) |
| CH-2 | CH-1 |
| CH-3 | CH-1 |
| CH-4 | CH-1 |
| CH-5 | CH-4 (transitively CH-1) |
| CH-comp | CH-1, CH-2, CH-3, CH-4, CH-5 |

### Cross-references

- Handshake.proof — formal specification SSOT consumed by every CH-N
- aims-proof/proofs/01-theorems/Composition.proof lines 1, 38, 69, 103, 136 —
  the sorry-bearing skeletons CH-1..CH-5 discharge
- aims-proof/proofs/09-verification/VF-comp.proof — the §09 layered-verifier
  composition pattern CH-comp mirrors (union-coverage shape; a fix passing a
  strict subset of layers is rejected; coverage gate prevents a dropped layer)
- the coexistence-handshake proofs §11.0
  Per-CH Proof-Status Tracking — SSOT for proof_status / reformulations columns
- aims-proof/lean/AimsProof/Coexistence.lean — kernel-checked Lean proofs of
  CH-1..CH-5 + CH-comp, cross-validated per-theorem by the dual-discharge gate
  (aims-proof/scripts/dual-discharge.sh). Replaces the retired
  placeholder-mirror emitter umbrella (the former
  cross-validation/coexistence-handshake.lean) under the dual-prover design

### Downstream items (NOT discharged in this dispatch)

Per the dispatch boundary in the coexistence-handshake proofs
§11.1, the following items are out of scope for this batch:

- CH-2 / CH-3 / CH-4 / CH-5 / CH-comp proof authoring
- Lean 4 cross-validation (now hand-authored at
  aims-proof/lean/AimsProof/Coexistence.lean, gated by dual-discharge.sh)
- §11 close-out runner authoring
  (aims-proof/scripts/run-section-11-proofs.sh)
- Compiler-conformance cross-walk Tier 2 behavioral harness
  (compiler/ori_arc/tests/aims/coexistence/ch4_behavioral_conformance.rs)
- §11 exit_reasons registration
  (scripts/plan_corpus/exit_reasons.py)
- Coverage-manifest CH-shape flip
  (aims-proof/proofs/00-coverage-corpus/CH-shape.proof + coverage-manifest.json)
- the CI gate critical-proof cross-validation slot wiring
- §11.0 Per-CH Proof-Status Tracking table row flips
- §11.1 checkbox flips

Subsequent §11.1 execution dispatches will discharge each item in
dependency-respecting order per the §11.1 Per-CH dependency chain table.
