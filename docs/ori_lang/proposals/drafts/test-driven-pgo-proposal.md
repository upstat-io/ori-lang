# Proposal: Test-Driven Profile-Guided Optimization

**Status:** Draft
**Author:** Eric (with Claude)
**Created:** 2026-02-28

---

## Summary

Leverage Ori's mandatory test infrastructure as an automatic source of PGO (Profile-Guided Optimization) training data. Since every function requires tests and tests live in the dependency graph, `ori test` already exercises representative code paths. Feed these execution profiles into LLVM's PGO pipeline and Meta's BOLT binary optimizer to produce faster binaries with a single flag.

```bash
ori build program.ori --pgo          # instrument → test → optimize
ori build program.ori --pgo --bolt   # + post-link binary optimization
```

No separate profiling step. No synthetic benchmarks. No production instrumentation. The tests *are* the profile.

---

## Motivation

### The Problem

PGO delivers 10-20% speedup on real workloads. BOLT adds another 10-15% on top. But adoption is abysmal because the workflow is painful:

1. **Build instrumented binary** — special compiler flags, different build config
2. **Run representative workload** — what counts as "representative"? Nobody knows
3. **Collect profile data** — custom tooling, file management, format conversion
4. **Rebuild with profiles** — second full compilation pass
5. **Hope the profiles match production** — if they don't, PGO can make things *worse*

Most teams skip it entirely. The ones that don't (Chrome, Firefox, Meta) have dedicated infrastructure teams maintaining the pipeline.

### Why Ori Is Different

Ori has something no other language has: **guaranteed, comprehensive execution profiles that are always available.**

- **Every function has tests** — mandatory, compiler-enforced
- **Tests are in the dependency graph** — they exercise real call paths, not synthetic ones
- **Tests cover hot paths** — developers test what matters; edge cases are covered too
- **Tests are fast** — capabilities make mocking trivial; no real I/O in unit tests
- **Tests are always up to date** — change a function, tests re-run automatically

An `ori test` run is implicitly a PGO training session. The compiler just isn't using the data yet.

### The Fortran Parallel

This is analogous to how Fortran's array semantics let compilers auto-vectorize without programmer intervention. Other languages *can* vectorize, but the programmer has to work for it (`restrict`, SIMD intrinsics). Ori's mandatory tests mean PGO "just works" — the data exists by construction.

---

## Design

### Phase 1: LLVM PGO Integration

Use LLVM's built-in instrumentation PGO (`-fprofile-generate` / `-fprofile-use`):

1. `ori build --pgo` triggers a three-step pipeline:
   - **Instrument**: Compile with LLVM IR-level instrumentation (`InstrProfiling` pass)
   - **Profile**: Run `ori test` on the instrumented binary, collecting `.profraw` files
   - **Optimize**: Merge profiles (`llvm-profdata merge`), rebuild with profile data (`-fprofile-use`)
2. Profile data is cached in `.ori/pgo/` alongside Salsa's incremental cache
3. Incremental: only re-profile functions whose tests changed (dependency graph already tracks this)

**LLVM passes activated by PGO data:**
- Branch probability refinement (hot/cold path layout)
- Function inlining threshold adjustment (inline hot callees more aggressively)
- Basic block reordering (hot blocks contiguous for instruction cache)
- Loop unrolling heuristic refinement
- Switch case reordering

### Phase 2: BOLT Post-Link Optimization

After PGO produces an optimized binary, optionally run BOLT for function-level layout:

1. `ori build --pgo --bolt` adds a post-link step:
   - Run the PGO-optimized binary through `ori test` with `perf record` (Linux) or hardware counters
   - Feed the `perf.data` to `llvm-bolt` for function reordering
2. BOLT optimizations:
   - Function reordering (hot functions adjacent in memory)
   - Basic block splitting (cold blocks moved to end of binary)
   - ICF (Identical Code Folding) for generated drop functions
   - PLT optimization for runtime calls

### Phase 3: Test-Weighted Profiles

Not all tests are equal. A test that exercises a tight inner loop is more valuable for PGO than a test that checks an error message.

- **Weight by execution count**: Functions called millions of times in tests get stronger profile signal
- **Weight by attached vs floating**: `@test tests @target` tests exercise the exact function; floating `tests _` tests are integration-level
- **Dependency depth weighting**: Tests deep in the dependency graph exercise more call paths

This is future work — Phases 1-2 use unweighted profiles, which are already better than no profiles.

### Profile Staleness

- Profiles are keyed by function content hash (Salsa already computes this)
- Changed function → profile invalidated → re-instrumented on next `--pgo` build
- Unchanged functions keep their cached profiles
- Full re-profile: `ori build --pgo --fresh`

---

## Interaction With Other Optimizations

| Optimization | PGO Interaction |
|-------------|-----------------|
| **ARC elision** | PGO data shows which RC operations are hot → prioritize elision |
| **Static uniqueness (VSO §07)** | Uniqueness proofs eliminate branches; PGO data confirms the branch bias |
| **FBIP reset/reuse** | PGO confirms reuse path is hot → LLVM inlines the fast path more aggressively |
| **Bump allocation (repr-opt §08.4)** | PGO shows allocation-heavy functions → bump alloc candidates |
| **noalias (codegen-fixes H3)** | PGO + noalias compound: LLVM has both aliasing guarantees AND branch probabilities |

The key insight: Ori's semantic optimizations (noalias, uniqueness, FBIP) tell LLVM *what's legal*. PGO tells LLVM *what's likely*. Together, the compiler knows both what it *can* do and what it *should* do.

---

## Prior Art

- **Rust**: `cargo pgo` (third-party crate) — requires manual workload selection; no automatic profile source
- **Go**: PGO since Go 1.20 — feeds production CPU profiles back to compiler; requires deployed service
- **GCC**: `-fprofile-generate` / `-fprofile-use` — same manual two-step as LLVM
- **Meta BOLT**: Post-link optimizer — 10-15% speedup on data center binaries; requires `perf.data`
- **AutoFDO** (Google): Samples production with `perf`, feeds to compiler — requires production deployment
- **Swift**: No built-in PGO workflow despite LLVM backend

None of these have an automatic, always-available profile source. They all require either production instrumentation or manually curated benchmark suites.

---

## Open Questions

1. **Test coverage vs production profile**: Tests may not perfectly represent production hot paths. Is test-driven PGO better than no PGO but worse than production PGO? (Likely yes — even imperfect profiles help.)
2. **Compilation time**: Three-pass compilation (instrument → test → optimize) is ~3x slower. Acceptable for release builds?
3. **Platform support**: BOLT is Linux-only. macOS/Windows need alternatives (or PGO-only, no BOLT).
4. **Profile format stability**: LLVM profile formats change between versions. Cache invalidation on LLVM upgrade?
5. **Capability interaction**: Should profiling runs use real capabilities or mocked ones? Mocked = faster but may miss I/O-bound hot paths. Real = slower but more representative.

---

## Scope

**In scope:** LLVM PGO integration, BOLT integration, incremental profile caching, `--pgo` flag
**Out of scope:** Production profile ingestion, sampling-based profiling, custom profiling passes, JIT compilation
**Future extensions:** Test-weighted profiles, adaptive profile merging, cross-compilation PGO

---

## Implementation Estimate

- **Phase 1 (LLVM PGO)**: Moderate — plumbing between `ori build`, LLVM instrumentation passes, and `llvm-profdata`. Core logic is LLVM's; Ori provides the workflow.
- **Phase 2 (BOLT)**: Small — shell out to `llvm-bolt` with the right flags. Platform-gated (Linux only).
- **Phase 3 (Weighted profiles)**: Research — needs experimentation to determine optimal weighting.

---

## Decision

Deferred — this proposal documents the opportunity for future implementation. The mandatory test infrastructure is the prerequisite, and it already exists. Implementation should wait until the core optimization pipeline (VSO, codegen fixes, repr-opt) is complete, so PGO has maximally optimized code to profile.
