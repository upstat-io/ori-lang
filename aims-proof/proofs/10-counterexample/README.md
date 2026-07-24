# §10 Counterexample Search — SMT/Lean Fixture Corpus

- Owning plan: `the counterexample search`
- Per-shape tracking SSOT: §10.0 table in the owning section file
- Runner SSOT (when shipped): `aims-proof/scripts/run-section-10-counterexample.sh`
- Result cache (when shipped): `aims-proof/proofs/10-counterexample/results.json`

## Purpose

- Encode the Ori-specific burden-prototype failure shapes + BUG-04-118 + BUG-04-120 as SMT constraints over the proven calculus from §02-§09.
- Ask the SMT solver per shape: under the proven calculus, does a counterexample state exist?
- UNSAT → calculus adequate for this shape → feeds the empirical-adequacy SSOT verdict.
- SAT → concrete counterexample → routes §02-§09 owning rule + §11 if composition-level + the empirical-adequacy SSOT per outcome-classification table in §10.1.

## Shape Inventory

Per §10.0 per-shape result tracking + §10.1 SC #7 expanded SMT input coverage.

| Shape | Fixture | Source | Status |
|---|---|---|---|
| `shape-bug-04-118-result-inner-alias` | `bug-04-118.smt2` + `bug-04-118.lean` | BUG-04-118 root-cause (Result<Inner, Err> Ok-payload inner-alias chain crossing destructure boundary) | authored — first §10.1 checkbox |
| `shape-04B.1-baseline-single-class` | `04B.1-baseline.smt2` + `.lean` | baseline single-class shapes (12/16 fail-baseline under emission alone) — encodes P2 RC-count preservation independent of DP-2/DP-3 elimination | authored — §10.1 SC #7 |
| `shape-04B.2-mono-pipeline-ordering` | `04B.2-mono-ordering.smt2` + `.lean` | mono-pipeline-ordering (E5001 `__cast` unresolved) — encodes PL-1, PL-1a, PL-2, PL-5, PL-6 ordering | authored — §10.1 SC #7 |
| `shape-04B.2-under-elimination-leaks` | `04B.2-under-elim.smt2` + `.lean` | under-elimination leaks (path-sensitive control flow + jump-arg merges + generic forwarders) — encodes RL-1, RL-2, RL-4, RL-5, IA-3 alt_join, RL-1-RL-2 composition P2 | authored — §10.1 SC #7 |
| `shape-04B.2-cross-class-uaf` | `04B.2-cross-class-uaf.smt2` + `.lean` | cross-class alias-chain UAF segfault — encodes DP-5 + project_alias_sources transitive closure (R1–R6) + TF-4 + IA-5 step (1) + RL-31 disjointness | authored — §10.1 SC #7 |
| `shape-04B.2-over-elimination-closure-env` | `04B.2-over-elim-closure.smt2` + `.lean` | over-elimination closure-env double-frees — encodes TF-7, TF-13 capture_state_update, RL-1, RL-2 with PartialApply exclusion, DP-2 supplementary-only restriction, P2 | authored — §10.1 SC #7 |
| `shape-04B.3-eval-aot-parity-break` | `04B.3-eval-aot-parity.smt2` + `.lean` | eval-AOT dual-execution parity break (CFG-merge RcDec drop at LLVM codegen) — encodes VF-7 / canon.md §7.1 inv 2 parity axiom + PL-4 ArcVarId-keyed lookup discipline | authored — §10.1 SC #7 |
| `shape-04B.4a-cross-pattern-categories` | excluded — see `excluded-shapes.json` | cross-pattern evaluation criterion outcome was BUILD BREAK (not novel calculus shape); coverage anchor VF-5 + VF-1 | excluded — §10.1 SC #7 |
| `shape-04B.4b-cross-pattern-categories` | excluded — see `excluded-shapes.json` | cross-pattern evaluation criterion outcome was transitive BUILD BREAK; coverage anchor VF-5 | excluded — §10.1 SC #7 |
| `shape-bug-04-120-multi-use-let-var` | `bug-04-120.smt2` + `.lean` | BUG-04-120 narrowed-list derived Eq codegen leak (24 bytes; multi-use Let Var on RC-tracked aggregate) — encodes TF-2, TF-11, IA-5 step (1) seq_add accumulation, RL-1, RL-2, P2 | authored — §10.1 SC #7 |

## Encoder-Validation Fixtures

Per §10.1 fixtures item — load-bearing vacuous-UNSAT guard.

| Fixture | Path | Expected | Failure mode |
|---|---|---|---|
| `fixture-known-sat-vacuity-guard` | `fixtures/encoder-validation.smt2` (known-SAT block) | `SAT` | non-SAT → ENCODER_INVALID halt; vacuous-UNSAT hazard active; NO real-shape verdict is trusted |
| `fixture-known-unsat-soundness-sanity` | `fixtures/encoder-validation.smt2` (known-UNSAT block) | `UNSAT` | non-UNSAT → ENCODER_INVALID halt; soundness sanity check rejected a sound shape |

Asymmetric causal link per `Annex E §AIMS §VF-3` oracle-cross-check shape: only the witness-surfacing known-SAT fixture proves the absence of vacuity. Both fixtures live in `fixtures/encoder-validation.{smt2|json}`.

## Excluded Shapes

`excluded-shapes.json` records intentionally-omitted shapes with the §02-§09 rule already proving coverage. Current entries:

- `shape-04B.4a-cross-pattern-categories` — cross-pattern evaluation criterion surfaced BUILD BREAKs in the burden-construction machinery (E0061, E0425). Build breaks are CONSTRUCTION-side defects; no semantic IR for SMT evaluation. Coverage anchor: VF-5 (end-to-end verification requires buildable impl) + VF-1 (structural well-formedness presupposes buildable compiler).
- `shape-04B.4b-cross-pattern-categories` — './test-all.sh' criterion transitively blocked by the build breaks above. Same exclusion rationale. The four cross-pattern categories already encoded cover the calculus surface; the excluded criteria would re-execute their empirical reproductions, not introduce new shapes.

## Outcome-Classification Routing (per §10.1 closed enum)

| `smt_result` | `route_target` |
|---|---|
| `UNSAT` | record "adequate for this shape" in `results.json`; feeds the empirical-adequacy SSOT verdict; does NOT flip the proven_by gate entries |
| `SAT` | route §02-§09 owning rule (engine-local core) + §11 if composition-level + the empirical-adequacy SSOT on non-recovery |
| `TIMEOUT` | bounded retry: 2× budget per retry, max 3 retries (300s/600s/1200s/2400s) |
| `TIMEOUT_EXHAUSTED` | `route_target: manual_review`; results.json row carries retry trace + `inconclusive` classification |
| `ENCODER_INVALID` | halt the run; no UNSAT verdict trusted; fix the encoding first |

## Cited Proven Rules (§02-§09)

Each fixture cites the proven calculus rules it depends on in its preamble comment block. The first-shipped fixture (`bug-04-118.smt2`) cites: TF-3, TF-4, TF-14, IA-5 step (1), DP-2, DP-5, RL-1, RL-2, RL-1-RL-2-composition.

## Runner Contract (when shipped — §10.1 checkbox 9)

- Single runner: `aims-proof/scripts/run-section-10-counterexample.sh`
- Initial per-shape solver timeout: 300s wall-clock (overridable via `--initial-timeout-seconds=<N>`)
- Bounded-retry on TIMEOUT: 2× budget per retry, max 3 retries; cap fires `TIMEOUT_EXHAUSTED`
- Per-shape result normalized to `{UNSAT, SAT, TIMEOUT, TIMEOUT_EXHAUSTED, ENCODER_INVALID}`
- Output: `aims-proof/proofs/10-counterexample/results.json`
- Routing assertions tested in `aims-proof/proofs/10-counterexample/tests/test_routing.py`
- NEVER reads from stdin / interactive prompts (per `skill-control-contract.md §Autopilot Mode`)

## Cross-Validation (Lean 4)

Each `.smt2` fixture carries a paired `.lean` file transcribing the same theorem against the proven calculus. The Lean kernel check is the cross-validation peer per the CI gate (when shipped). Cross-validation divergence → ENCODER_INVALID halt per §10.1 SC #4 asymmetric guard.

## HISTORY

- **2026-05-28 — First fixture landed**: `bug-04-118.smt2` + `bug-04-118.lean` + this README. Encodes the BUG-04-118 alias-chain shape (Result<Inner, Err> Ok-payload `inner` lifetime survives Result destructure) per `BUG-04-118`. SMT goal expected UNSAT (the proven calculus refutes the double-free; the runtime defect is implementation-side at `compute_ssa_alias_classes` population-time per BUG-04-118 §01 root cause, not a calculus gap). Verdict pending §10.1 checkbox 4 (runner SMT-solver dispatch) — encoding ships now; verdict captured once `aims-proof/scripts/run-section-10-counterexample.sh` lands.
- **2026-05-28 — §10.1 checkboxes 2-4 landed**: authored eight new fixture files mirroring the `bug-04-118.smt2` style — baseline single-class (`04B.1-baseline.{smt2,lean}`); four cross-pattern categories (`04B.2-mono-ordering.{smt2,lean}`, `04B.2-under-elim.{smt2,lean}`, `04B.2-cross-class-uaf.{smt2,lean}`, `04B.2-over-elim-closure.{smt2,lean}`); eval-AOT parity break (`04B.3-eval-aot-parity.{smt2,lean}`); BUG-04-120 multi-use Let Var on RC-tracked aggregate (`bug-04-120.{smt2,lean}`). The two cross-pattern build-break criteria recorded as exclusions in `excluded-shapes.json` (BUILD BREAK outcomes, not novel calculus shapes; coverage anchored on VF-5 + VF-1; the four cross-pattern categories already cover their empirical reproductions). Each fixture cites the proven calculus rules it depends on in its preamble comment block + carries an intel-query citation block per `intelligence.md` graph-first protocol. All SMT goals expected UNSAT per §10 hypothesis ("calculus adequate for the failure surface"); verdicts captured once the §10 runner lands (§10.1 checkboxes 5-10, outside this dispatch's scope).
