---
plan: "llvm-verification-tooling"
title: "LLVM & AIMS Verification Tooling: Exhaustive Implementation Plan"
status: in-progress
supersedes: []
references:
  - "plans/llvm-verification-tooling/research.md"
  - ".claude/rules/arc.md"
  - ".claude/rules/compiler.md"
  - ".claude/rules/llvm.md"
  - ".claude/rules/impl-hygiene.md"
  - ".claude/rules/tests.md"
  - "~/projects/reference_repos/verification_tools/"
---

# LLVM & AIMS Verification Tooling: Exhaustive Implementation Plan

## Mission

Build world-class verification tooling for Ori's AIMS memory system and LLVM backend — exploiting Ori's unique dual-layer architecture (ARC IR + LLVM IR + Interpreter differential oracle) to create verification capabilities that exceed what Rust, Swift, Lean4, or Koka have. Make verification failures *blocking gates* in test/CI, not informational warnings. The AIMS pipeline's 7-dimensional product lattice, 12-step observable pipeline, and contract-realization coherence requirements demand purpose-built tooling that no other compiler needs — because no other compiler has this architecture.

## Mission Success Criteria

- [ ] **AIMS snapshot regression detection**: Running `cargo test -p oric --test aims_snapshots` catches pass-level regressions in `realize_rc_reuse`, `merge_blocks`, `realize_annotations`, `normalize_function`, and `tail_calls` via `lowered.arc` baseline + per-pass `.after.arc` artifacts (Section 03)
- [x] **Lattice property verification**: `timeout 150 cargo test -p ori_arc -- lattice::prop_tests` runs 36 property tests (35 pass, 1 O(n^3) manual-only `#[ignore]`). Join commutativity, associativity, idempotence, partial-order axioms, canonicalization idempotence/convergence/dimension-guarantees, 8 decision predicate semantic contracts, capture_state_update monotonicity, fixpoint convergence, and permutation invariance all verified. BUG-04-057 fixed (3f7cf7c2): removed Rule 4, widened Rule 6. Section 04 complete (2026-04-12). TPR found BUG-04-059 (realization-layer unsound proofs, filed in bug tracker).
- [x] **Contract coherence oracle**: After AIMS pipeline completion, an independent contract re-derivation from realized ARC IR (walking actual `RcInc`/`RcDec`/`Reuse` instructions) matches inferred `MemoryContract` — discrepancies are blocking errors under `ORI_VERIFY_ARC=1` (Section 05) — Completed 2026-04-12. Oracle walks post-pipeline ARC IR with aliasing-aware tracking, handles batched RcInc, PartialApply effects, directional tolerance. 29 unit tests, 17,142 full-suite tests pass with zero false positives. TPR found alias fixpoint bug + 2 others, all fixed. Enriched diagnostic with per-mismatch details.
- [ ] **Protocol builtin ownership pinned**: Every `ProtocolBuiltin` variant's per-argument expected ownership is pinned at all three layers: IR definition (existing tests audited), ori_arc consumers (MemoryContract field-level + negative pins + consistency pins + dispatch order bug fixed), and LLVM codegen (AOT type x pattern matrix with `ORI_CHECK_LEAKS=1` for `str`, `[int]`, `{str:int}`, `{int:str}`, looped indexing). `Set<str>` verified via existing tests; `Set<int>` blocked by BUG-04-065 (iteration crashes in AOT). Exhaustiveness guard with compile-time + test-time coverage. 3 bugs filed (BUG-04-061/062/065). (Section 06) — In Progress (close-out: TPR + hygiene + tooling sweep pending).
- [x] **LLVM verification gates active**: `ORI_VERIFY_EACH=1` runs LLVM IR verifier after every optimization pass in `test-all.sh` and CI; function-level `fn_val.verify()` runs after each function's codegen; `opt -lint` runs in the codegen audit pipeline (Section 01) — Completed 2026-04-10. Both `ORI_VERIFY_ARC=1` and `ORI_VERIFY_EACH=1` enabled globally (54s, 36% of budget). `function(lint)` integrated. All 16,978 tests pass.
- [ ] **FileCheck IR assertions**: `compiler/ori_llvm/tests/codegen/` contains ≥30 directive-based IR pattern tests covering RC emission, COW patterns, closure codegen, ABI, and iterator patterns, with revision support for debug/release/no-repr-opt configurations (Section 07)
- [ ] **Sanitizer integration**: ASan/UBSan instrumentation on generated AOT binaries via LLVM pipeline; separate CI job with smoke subset on PRs and full sweep nightly (Section 08)
- [ ] **Alive2 refinement checking**: Curated subset of pure/arithmetic-heavy functions verified via Alive2 `alive-tv` for pre-opt → post-opt LLVM IR refinement, running nightly (Section 09)
- [ ] **Differential oracle fuzzing**: `fuzz/fuzz_targets/ori_differential.rs` generates random Ori programs, executes via eval AND LLVM, compares stdout + `ORI_CHECK_LEAKS` results; ≥24h cumulative fuzzing with zero unresolved divergences (Section 10)
- [ ] **CI fully integrated**: `.github/workflows/ci.yml` runs `verify_each`, function-level verify, FileCheck tests, and LLVM backend spec tests; nightly job runs sanitizers, Alive2, and differential fuzzing (Section 11)
- [ ] **IR regression tracking**: Baseline IR captured for ≥20 key programs; `scripts/ir-baseline.sh --compare` detects any IR shape change; `--bless` updates baselines (Section 12)
- [ ] `./test-all.sh` green — no regressions
- [ ] All section success criteria met

## Architecture

```
                    ┌─────────────────────────────────────────────────┐
                    │           Verification Tooling Stack            │
                    └──────────┬──────────────────────────────────────┘
                               │
    ┌──────────────────────────┼──────────────────────────┐
    │                          │                           │
┌───▼────────────┐   ┌────────▼─────────┐   ┌────────────▼──────────┐
│  Phase A:      │   │  Phase B:        │   │  Phase C:             │
│  Foundation    │   │  AIMS Verifiers  │   │  LLVM Verification    │
│  & Gates       │   │  (Ori-Unique)    │   │  (Industry Standard)  │
├────────────────┤   ├──────────────────┤   ├───────────────────────┤
│ §01 Verifier   │   │ §03 Pass-Level   │   │ §07 FileCheck IR      │
│     Gates &    │   │     Snapshots    │   │     Assertions        │
│     Quick Wins │   │ §04 Lattice      │   │ §08 Sanitizers        │
│ §02 Shared     │   │     Properties   │   │     (ASan/UBSan)      │
│     Harness    │   │ §05 Contract     │   │ §09 Alive2 Formal     │
│                │   │     Coherence    │   │     Verification      │
│                │   │ §06 Protocol     │   │                       │
│                │   │     Builtins     │   │                       │
└───────┬────────┘   └────────┬─────────┘   └───────────┬───────────┘
        │                     │                          │
        └─────────────────────┼──────────────────────────┘
                              │
               ┌──────────────▼──────────────────┐
               │  Phase D: Continuous             │
               │  Verification (Going Beyond)     │
               ├──────────────────────────────────┤
               │ §10 Differential Oracle Fuzzing  │
               │ §11 CI Integration & ARC Parity  │
               │ §12 Regression Dashboard         │
               └──────────────────────────────────┘
```

### Data Flow

```
Ori Source → Parser → Type Checker → CanExpr
                                        │
                    ┌───────────────────┤
                    │                   │
              ┌─────▼──────┐    ┌──────▼──────────┐
              │ Interpreter │    │ ARC Lowering     │
              │ (ori_eval)  │    │ CanExpr→ArcFunc  │
              │  [Oracle]   │    └──────┬───────────┘
              └─────┬───────┘           │
                    │            ┌──────▼──────────────────────────┐
                    │            │ AIMS Pipeline (12 steps)        │
                    │            │  §03: Snapshot at each step     │
                    │            │  §04: Lattice properties hold   │
                    │            │  §05: Contracts match realized  │
                    │            │  §06: Protocol builtins pinned  │
                    │            └──────┬──────────────────────────┘
                    │                   │
                    │            ┌──────▼──────────────────────────┐
                    │            │ LLVM Codegen                    │
                    │            │  §01: verify_each after passes  │
                    │            │  §01: fn_val.verify() per fn    │
                    │            │  §07: FileCheck IR assertions   │
                    │            │  §08: ASan/UBSan instrumentation│
                    │            │  §09: Alive2 refinement check   │
                    │            └──────┬──────────────────────────┘
                    │                   │
                    │            ┌──────▼─────┐
                    │            │ AOT Binary  │
                    │            └──────┬──────┘
                    │                   │
              ┌─────▼───────────────────▼──────┐
              │ §10: Differential Oracle        │
              │ eval output == LLVM output       │
              │ + ORI_CHECK_LEAKS on both       │
              └────────────────────────────────┘
```

## Design Principles

### 1. Multi-Layer Verification (Ori's Unique Advantage)

Ori has **two IR levels** (ARC IR and LLVM IR) plus an **interpreter** — three views of the same program. No other compiler has this. A bug that produces correct output from bad IR (LLVM optimizer papers over codegen bugs) is invisible to behavioral tests but caught by IR-level verification. A lattice bug that causes unsound RC elision passes all structural checks but fails the contract coherence oracle. Each verification layer catches a distinct class of bugs:

| Layer | Catches | Tools |
|-------|---------|-------|
| ARC IR snapshots | Pass regressions, optimization quality drift | §03 |
| Lattice properties | Unsound analysis, non-monotone transfers | §04 |
| Contract coherence | Inferred vs realized mismatch | §05 |
| LLVM IR assertions | Codegen bugs masked by optimizer | §07 |
| Sanitizers | Memory errors in generated code | §08 |
| Formal verification | Optimization incorrectness (mathematical proof) | §09 |
| Differential oracle | Behavioral divergence between eval and LLVM | §10 |

### 2. Verification Failures Are Blocking Gates

Current state: `run_verify()` and `run_aims_verify()` log warnings via `tracing::warn!` (`pipeline/mod.rs:128,144`). FIP structural checks use `debug_assert!` that disappear in release. This violates `.claude/rules/arc.md` §Non-Negotiable Invariant #4: "Every active subsystem needs implementation + invariant enforcement + verification."

This plan makes verification failures **hard errors** under verification mode (`ORI_VERIFY_ARC=1` / `ORI_VERIFY_EACH=1`), and those modes are **ON by default** in `test-all.sh` and CI. Behavioral tests that pass while a verifier fails are false confidence.

### 3. Shared Harness, Not Fragmented Tools

Directive parsing, artifact naming, `--bless` mode, and revision expansion must live in ONE place — a workspace library consumed by both AIMS snapshot tests and FileCheck IR tests. Rust's compiletest (`src/tools/compiletest/`) is the reference pattern. Building a standalone `ori-check` binary would create a second source of truth for compiler behavior (DRIFT per `impl-hygiene.md` §SSOT). Instead: workspace library + `oric` subcommand.

## Section Dependency Graph

```
§01 Verifier Gates ──────────┬──► §06 Protocol Builtins (needs blocking audit)
                              │
§02 Shared Harness ──────────┼──► §03 AIMS Snapshots ──┐
                              │                          ├──► §05 Contract Oracle
                              │   §04 Lattice Props ────┘    (needs lattice; snapshots optional)
                              │
                              ├──► §07 FileCheck IR ──────► §09 Alive2
                              │
§01 ─────────────────────────┼──► §08 Sanitizers
                              │
§01 ─────────────────────────┼──► §10 Differential Fuzzing
                              │
                              §11 CI Integration ◄──── all above (incremental)
                                        │
                              §12 Regression Dashboard ◄── §11
```

- §01 and §02 are foundation — everything depends on at least one.
- §04 (lattice properties) and §06 (protocol builtins) can be done early — they mostly use existing Rust tests and canonical sources without the full harness.
- §05 depends on BOTH §03 AND §04 — the contract oracle should not be built until lattice properties are pinned and snapshot infrastructure exists.
- §06 depends on §01 — protocol builtin RC balance tests need blocking codegen audit behavior.
- §08 (sanitizers) and §10 (fuzzing) depend on §01 (verifier gates active).
- §09 depends on §07 (Alive2 uses FileCheck-style test corpus as input).
- §11 is incremental — each section should add its own CI gate; §11 consolidates and adds ARC IR parity.
- §12 requires §11.

**Cross-section interactions (must be co-implemented):**
- **§01 + §02**: Verifier gates define what "failure" means; the shared harness captures failures. If §01 makes verifiers blocking but §02 isn't ready to capture the output, test runs produce raw panic output instead of structured findings.

## Implementation Sequence

```
Phase A - Foundation & Gates
  └─ §01: Verifier gates, verify_each wiring, function-level verify, opt -lint
  └─ §02: Shared test harness library (directive parser, bless, revisions, artifacts)
  Gate: ORI_VERIFY_EACH=1 cargo test -p ori_llvm passes without regressions

Phase B - AIMS Verification (Ori-Unique)  [CRITICAL PATH]
  └─ §03: AIMS pass-level snapshot tests (realize_rc_reuse, merge_blocks, etc.)
  └─ §04: AIMS lattice property verification (proptest, 7D lattice axioms)
  └─ §05: Contract coherence oracle (re-derive from realized IR)
  └─ §06: Protocol builtin verification matrix
  Gate: cargo test -p ori_arc aims_ passes; ORI_VERIFY_ARC=1 tests green

Phase C - LLVM Verification (Industry Standard + Formal)
  └─ §07: FileCheck-style IR pattern matching (compiler/ori_llvm/tests/codegen/, 30+ tests)
  └─ §08: Sanitizer integration (ASan/UBSan on AOT, separate CI job)
  └─ §09: Alive2 formal verification (curated subset, nightly)
  Gate: compiler/ori_llvm/tests/codegen/ passes; ORI_SANITIZE=1 smoke passes; alive-tv nightly green

Phase D - Continuous Verification (Going Beyond)
  └─ §10: Differential oracle fuzzing (eval vs LLVM, cargo-fuzz)
  └─ §11: CI integration (workflow updates, ARC IR parity, opt-bisect)
  └─ §12: Verification dashboard & regression tracking (IR baselines, trends)
  Gate: 24h+ fuzzing clean; CI workflow passes; baselines captured
```

**Why this order:**
- Phase A is **not purely additive** — §01 changes `run_verify()`/`run_aims_verify()` from warnings to hard errors, which WILL surface existing latent bugs. Gate the blocking behavior behind the explicit `ORI_VERIFY_ARC=1` flag and prove no false positives before enabling by default. `verify_each` and function-level verify are pure additions.
- Phase B is the critical path because AIMS is Ori's unique verification surface — no off-the-shelf tools exist; everything must be purpose-built. §04 and §06 can be done early (existing tests + canonical sources); §03 needs the harness; §05 needs both §03 and §04.
- Phase C can proceed in parallel with Phase B after §02 completes — they touch different layers (ARC IR vs LLVM IR).
- Phase D requires stable verification infrastructure from A+B+C. Fuzzing needs all verifier gates active to detect divergences. CI integration is incremental throughout — each section adds its own CI gate; §11 consolidates.

**Known failing tests (expected during implementation):**

Enabling `ORI_VERIFY_ARC=1` and `ORI_VERIFY_EACH=1` is specifically designed to find latent bugs in the AIMS pipeline and LLVM codegen. The plan assumes failures WILL be found. Triage path:

- **Verification failures in existing code**: These are pre-existing bugs that verification surfaced. Each gets filed via `/add-bug` and fixed before the section proceeds (per CLAUDE.md §Zero Deferral).
- **Snapshot mismatches after optimization changes**: Expected during active development. Use `--bless` to update baselines when changes are intentional.
- **Alive2 false positives**: Expected due to memory operation limitations. Managed via suppression file with categories (runtime-call, memory-model, loop-bound).
- **Fuzzing crashes**: Expected — that's the point. Each gets triaged: parser crashes → fix parser; codegen crashes → file via `/add-bug`; eval/LLVM divergences → investigate.

## Metrics (Current State)

| Component | Production LOC | Test LOC | Total |
|-----------|---------------|----------|-------|
| `ori_llvm/src/verify/` | ~1,438 | ~1,015 | ~2,453 |
| `ori_arc/src/aims/verify/` | ~210 | — | ~210 |
| `ori_arc/src/aims/normalize/verify.rs` | ~599 | — | ~599 |
| `ori_arc/src/aims/lattice/` | ~835 | ~2,365 | ~3,200 |
| `ori_arc/src/aims/interprocedural/` | ~537 | ~1,243 | ~1,780 |
| `ori_arc/src/aims/intraprocedural/` | ~971 | ~4,012 | ~4,983 |
| `ori_arc/src/aims/transfer/` | ~524 | — | ~524 |
| `diagnostics/` (scripts) | ~4,323 | — | ~4,323 |
| `ori_ir/builtin_constants/protocol/` | — | ~79 | ~79 |
| **Total existing** | **~9,437** | **~8,714** | **~18,151** |

**Estimated new code**: ~15,000-25,000 lines (library + tests + scripts + fuzz targets + CI + external tool integration + corpus + documentation). The lower range assumes smooth integration; the upper range accounts for false-positive triage (Alive2), typed program generation (fuzzing), and the inevitable bugs surfaced by enabling verification gates.

## Estimated Effort

| Section | Est. Lines | Complexity | Depends On |
|---------|-----------|------------|------------|
| 01 Verifier Gates & Quick Wins | ~600 | Medium | — |
| 02 Shared Test Harness | ~2,000 | High | — |
| 03 AIMS Pass-Level Snapshots | ~1,500 | Medium | 02 |
| 04 AIMS Lattice Properties | ~1,200 | Medium | — |
| 05 Contract Coherence Oracle | ~1,500 | High | 03, 04 |
| 06 Protocol Builtin Matrix | ~800 | In Progress | 01 |
| 07 FileCheck IR Assertions | ~2,500 | High | 02 |
| 08 Sanitizer Integration | ~1,200 | Medium | 01 |
| 09 Alive2 Formal Verification | ~2,000 | Very High | 07 |
| 10 Differential Oracle Fuzzing | ~2,500 | Very High | 01 |
| 11 CI Integration & ARC Parity | ~1,500 | Medium | all |
| 12 Regression Dashboard | ~1,000 | Medium | 11 |
| **Total new** | **~18,300** | | |
| **Total deleted** | **~0** | | |

Note: §09 (Alive2) and §10 (fuzzing) estimates include tool installation, corpus management, typed program generation, false-positive triage, CI sharding, and documentation — not just the integration code.

## Known Bugs (Pre-existing)

| Bug | Root Cause | Fix Location | Status |
|-----|-----------|-------------|--------|
| `run_verify()` logs warnings, not errors | Verification failures non-blocking | Section 01 | Not Started |
| `run_aims_verify()` logs warnings, not errors | Same as above | Section 01 | Not Started |
| FIP checks use `debug_assert!` (disappear in release) | Compile-out in release builds | Section 01 | Not Started |
| `ORI_VERIFY_EACH` has no canonical home in `debug_flags.rs` | Flag not registered | Section 01 | Not Started |
| CI missing `ori test --backend=llvm tests/` | LLVM backend not tested in CI | Section 11 | Not Started |
| `test-all.sh` masks LLVM backend crash | Escape hatch in test runner | Owned by `plans/llvm-worker-isolation/` | Blocked |
| Research.md dependency story stale | Lists diagnostic-tooling as blocking | Update research.md | Fixed (2026-04-10) |

## Quick Reference

| ID | Title | File | Status |
|----|-------|------|--------|
| 01 | Verifier Gates & Quick Wins | `section-01-verifier-gates.md` | Complete |
| 02 | Shared Test Harness Infrastructure | `section-02-shared-harness.md` | Complete |
| 03 | AIMS Pass-Level Snapshot Tests | `section-03-aims-snapshots.md` | Complete |
| 04 | AIMS Lattice Property Verification | `section-04-lattice-properties.md` | Complete |
| 05 | Contract Coherence Oracle | `section-05-contract-oracle.md` | Complete |
| 06 | Protocol Builtin Verification Matrix | `section-06-protocol-builtins.md` | In Progress |
| 07 | FileCheck-Style IR Pattern Matching | `section-07-filecheck.md` | Not Started |
| 08 | Sanitizer Integration | `section-08-sanitizers.md` | Not Started |
| 09 | Alive2 Formal Verification | `section-09-alive2.md` | Not Started |
| 10 | Differential Oracle Fuzzing | `section-10-differential-fuzzing.md` | Not Started |
| 11 | CI Integration & ARC IR Parity | `section-11-ci-integration.md` | Not Started |
| 12 | Verification Dashboard & Regression Tracking | `section-12-regression-dashboard.md` | Not Started |
