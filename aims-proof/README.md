# AIMS Proof Corpus

The machine-checked calculus of AIMS (ARC Intelligent Memory System), Ori's memory model. The calculus, its soundness proofs, and the proof checker are Ori's own; prior compilers are cited as historical influences in the spec (`docs/ori_lang/v2026/spec/annex-e-system-considerations.md §AIMS`, the public algorithmic SSOT).

## Layout

- `lean/` — the compiled Lean 4 corpus (`AimsProof/*.lean`). **This is the calculus.** Verified by Lean's trusted kernel via `lake build`; on any disagreement with other artifacts, the compiled Lean wins.
- `proofs/` — human-readable `.proof` text artifacts, validated by the Ori-owned checker. Companion layer, never the authority.
- `checker/` — the Ori proof checker (Rust).
- `schemas/` — proof-artifact schemas.
- `scripts/` — verification gates (see below) and the `.proof` ↔ Lean theorem map (`proof-lean-map.json`).
- `tests/`, `test-results/` — checker harnesses and recorded runs.

## Verification

- `cd lean && lake build` — kernel-check the calculus.
- `scripts/check-proofs.sh` — run the `.proof` corpus through the Ori checker.
- `scripts/dual-discharge.sh` — enforce per-theorem agreement between the Ori checker and the Lean kernel (statement-parity prelude + verdict comparison; disagreement is a hard failure).
- `scripts/lean-no-placeholder-lint.sh` — ban `sorry` / `admit` / vacuous discharges from the compiled corpus.

CI runs the checker and the Lean cross-validation as hard gates (`.github/workflows/aims-proofs.yml`).

## Module map (`lean/AimsProof/`)

| Module | Proves |
|---|---|
| `Model` | The seven-dimension `AimsState` product, join, and canonicalization (definitions) |
| `Lattice` | L-family lattice algebra: commutativity, associativity, idempotence, partial order, finite height |
| `Canonicalization` | CN-family cross-dimensional feasibility invariants |
| `Transfer` | TF-family forward/backward transfer rules and monotonicity |
| `Decision` | DP-family decision-predicate truth tables |
| `Interprocedural` | IC-family contract algebra and SCC fixpoint convergence |
| `Pipeline` | PL-family phase-ordering laws and TRMC contracts |
| `Realization` | RL-family emission rules and reference-count balance |
| `Verification` | VF-family layered-stack coverage |
| `Coexistence` | CH-family burden/lattice coexistence handshake |

Rule labels (L / CN / TF / DP / IC / PL / RL / VF / CH) match `annex-e §AIMS`. `scripts/proof-lean-map.json` maps each `.proof` artifact to its Lean theorem.
