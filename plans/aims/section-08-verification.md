---
section: "08"
title: "Verification & Validation"
status: not-started
reviewed: true  # 2026-03-10
goal: "Prove AIMS correctness via behavioral equivalence, performance comparison, and safety verification"
depends_on: ["06"]
sections:
  - id: "08.1"
    title: "Behavioral Equivalence"
    status: not-started
  - id: "08.2"
    title: "RC Operation Count Comparison"
    status: not-started
  - id: "08.2a"
    title: "Allocation Count Comparison"
    status: not-started
  - id: "08.2b"
    title: "FIP Certification Coverage"
    status: not-started
  - id: "08.3"
    title: "Test Matrix"
    status: not-started
  - id: "08.4"
    title: "Safety Verification"
    status: not-started
  - id: "08.5"
    title: "Performance Validation"
    status: not-started
  - id: "08.6"
    title: "Documentation"
    status: not-started
  - id: "08.7"
    title: "Completion Checklist"
    status: not-started
---

# Section 08: Verification & Validation

**Status:** Not Started

**Goal:** Prove that AIMS is correct (same behavior as current pipeline), produces
equal or fewer RC operations, achieves equal or fewer allocations, compiles at
least as fast, and generates correct code under stress testing and Valgrind.
Additionally, measure FIP certification coverage and FBIP achieved-vs-missed reuse.

**Context:** AIMS is a rewrite of the analysis core. Correctness is non-negotiable.
The existing test suite and the diagnostic scripts (`diagnostics/dual-exec-verify.sh`,
`diagnostics/valgrind-aot.sh`, etc.) provide the verification infrastructure.

**Evaluation doctrine** (from "Exploring Perceus for OCaml", Pinto & Leijen, ML
Workshop 2023): Same compiler, same frontend, same optimizer, same LLVM backend —
only switch old ARC pipeline vs AIMS pipeline. This is the default evaluation
methodology for AIMS. Differences in output must be attributable solely to the
memory-management strategy change.

**Depends on:** Section 06 (working AIMS pipeline).

---

## 08.1 Behavioral Equivalence

Compare AIMS output against the current pipeline for every test case.

- [ ] Build a comparison script using compile-time feature flag (matches Section 06.1):
  ```bash
  #!/bin/bash
  # Compare old vs AIMS pipelines.
  # Two separate passes: (1) behavioral equivalence, (2) RC count comparison.
  # Behavioral regressions are hard failures. RC count regressions are
  # informational during initial migration (see Stage 1C gate).
  set -euo pipefail
  shopt -s globstar  # required for **/*.ori glob expansion in bash

  cargo build -p oric 2>/dev/null
  cp target/debug/ori /tmp/ori_old

  cargo build -p oric --features aims 2>/dev/null
  cp target/debug/ori /tmp/ori_aims

  semantic_failures=0
  rc_improvements=0
  rc_regressions=0
  rc_matches=0

  for test in tests/spec/**/*.ori; do
      # === Pass 1: Behavioral equivalence (hard failure) ===
      # Build failures are hard failures, not masked.
      if ! /tmp/ori_old build "$test" -o /tmp/bin_old 2>/dev/null; then
          echo "SKIP (old build failed): $test"
          continue
      fi
      if ! /tmp/ori_aims build "$test" -o /tmp/bin_new 2>/dev/null; then
          echo "SEMANTIC FAILURE (AIMS build failed, old succeeded): $test"
          ((semantic_failures++))
          continue
      fi

      old_exit=0
      /tmp/bin_old > /tmp/out_old.txt 2>&1 || old_exit=$?
      new_exit=0
      /tmp/bin_new > /tmp/out_new.txt 2>&1 || new_exit=$?

      if [ "$old_exit" != "$new_exit" ] || \
         ! diff /tmp/out_old.txt /tmp/out_new.txt > /dev/null 2>&1; then
          echo "SEMANTIC FAILURE (different output/exit): $test"
          ((semantic_failures++))
          continue
      fi

      # === Pass 2: RC count comparison (informational) ===
      ORI_DUMP_AFTER_ARC=1 /tmp/ori_old build "$test" 2> /tmp/arc_old.txt
      ORI_DUMP_AFTER_ARC=1 /tmp/ori_aims build "$test" 2> /tmp/arc_new.txt
      if diff /tmp/arc_old.txt /tmp/arc_new.txt > /dev/null; then
          ((rc_matches++))
      else
          old_rc=$(grep -c "RcInc\|RcDec" /tmp/arc_old.txt || echo 0)
          new_rc=$(grep -c "RcInc\|RcDec" /tmp/arc_new.txt || echo 0)
          if [ "$new_rc" -le "$old_rc" ]; then
              echo "RC IMPROVEMENT ($old_rc -> $new_rc): $test"
              ((rc_improvements++))
          else
              echo "RC REGRESSION ($old_rc -> $new_rc): $test"
              ((rc_regressions++))
          fi
      fi
  done

  echo ""
  echo "=== Behavioral: $semantic_failures failures ==="
  echo "=== RC counts: $rc_matches matches, $rc_improvements improvements, $rc_regressions regressions ==="
  # Hard-fail on semantic regressions only. RC regressions are logged
  # but do not fail the harness during Stage 1C migration.
  [ "$semantic_failures" -gt 0 ] && exit 1
  ```

- [ ] Run `diagnostics/dual-exec-verify.sh` with AIMS binary — every spec test
  must produce identical output between eval and AOT paths

- [ ] Run `diagnostics/dual-exec-debug.sh` on edge cases (pattern match, closures,
  loops with collections, COW-heavy code) — auto-dumps on mismatch

- [ ] **Verify arg_ownership fields match**:
  Compare `Apply.arg_ownership` and `Invoke.arg_ownership` between old and new
  pipelines for every function. Mismatches here cause subtle RC behavior
  differences that won't show up as output differences but will cause memory
  errors under stress. Only Valgrind or leak checks catch these; explicit
  comparison is needed.

- [ ] Track and investigate every behavioral difference:
  - AIMS may produce FEWER RC ops (improvement)
  - AIMS must NOT produce DIFFERENT behavior (bug)

- [ ] **Golden corpus definition** (referenced by Stage 1A gate in 00-overview):
  The golden corpus is the set of programs used for shadow comparison. It
  consists of:
  1. All files in `tests/spec/` (existing spec test suite)
  2. Rust-level ARC unit tests run via `cargo test -p ori_arc` (inline test
     modules in sibling `tests.rs` files per the project's test convention)
  3. AOT compilation tests run via `cargo test -p ori_llvm`
  4. The 10 hand-traced validation corpus programs from Section 02.7
  5. A dedicated `tests/aims/` directory (to be created) containing programs
     that exercise specific AIMS edge cases not covered above:
     - Closure capture with multiple escaping variables
     - Deeply nested pattern match with cross-branch liveness
     - COW chain (multiple sequential mutations)
     - Recursive tree traversal with reuse
     - Mixed borrowed/owned call patterns
  For each golden corpus program, the expected ARC IR output (specifically:
  arg_ownership, cow_annotations, and RC operation count) is recorded as a
  snapshot. Shadow analysis compares against these snapshots.

- [ ] **Stage 1A shadow analysis comparison targets** (solutions.md Decision 3):
  Compare ONLY artifacts the old pipeline already computes:
  - Compare `ArcParam.ownership` per function param: AIMS vs old borrow inference
  - Compare `Apply.arg_ownership` / `Invoke.arg_ownership` per call site
  - Compare `ArcFunction.cow_annotations` modes (semantic comparison — same
    `CowMode` per COW operation site, not positional key comparison since
    production timing differs. One variable may participate in multiple
    distinct COW sites; compare each site's mode individually.)
  - Compare return uniqueness: AIMS `MemoryContract.return_info.uniqueness`
    vs old `UniquenessSummary.return_val`
  - **Not compared:** cardinality, locality, shape, effect — AIMS-only
    dimensions with no old-pipeline equivalent. Validated internally via
    Section 02.7 validation corpus.
  - Log mismatches as `tracing::warn!` for investigation — do NOT fail the build
    (AIMS may compute tighter facts than the old pipeline)
  - Document each mismatch category: improvement (AIMS is tighter) vs regression
    (AIMS is wrong)
  - **Function set alignment**: Both pipelines analyze the same set of `ArcFunction`s
    (produced by the same lowering pass). The comparison iterates over functions
    present in BOTH output sets. If a function appears in one but not the other,
    log `tracing::error!` — this indicates a pipeline structural bug, not an
    analysis difference.

---

## 08.2 RC Operation Count Comparison

Measure whether AIMS actually reduces RC operations.

- [ ] Build an RC counting tool:
  ```bash
  # Count RcInc/RcDec in ARC IR dump
  ORI_DUMP_AFTER_ARC=1 ori build file.ori 2>&1 | grep -c "RcInc\|RcDec"
  ```

- [ ] Run on representative programs and compare:
  | Program | Old RC Ops | AIMS RC Ops | Reduction |
  |---------|-----------|-------------|-----------|
  | Simple function | ? | ? | ? |
  | List operations | ? | ? | ? |
  | Tree manipulation | ? | ? | ? |
  | Pattern matching | ? | ? | ? |
  | Closure-heavy | ? | ? | ? |

- [ ] RC count parity follows the staged cutover gates:
  - **Stage 1C**: RC regressions are tracked and investigated but not automatic
    blockers (correctness first — see Section 06.4 Stage 1C gate).
  - **Stage 1D**: RC count ≤ old pipeline becomes a hard gate (optimization parity
    required before removing old passes).
  - **Post-Stage 1**: AIMS must produce ≤ old pipeline RC ops for every program.
    Any regression at this point is a bug to be fixed, not a trade-off to accept.

---

## 08.2a Allocation Count Comparison (Secondary Metric)

Static allocation-site counts are a **secondary metric**, not the main allocation
story. Once dynamic reuse introduces fast/slow paths, static `Construct`/`PartialApply`
counts stop being a reliable proxy for runtime allocation behavior.

**What this measures:** How many `Construct`/`PartialApply` instructions appear in the
emitted ARC IR (static allocation sites). A `Construct` replaced by `Reuse` eliminates
one allocation site. This reflects reuse effectiveness, not runtime allocation counts.

**What this does NOT measure:** Runtime allocations (how many times those sites execute).
For runtime measurement, use `ORI_TRACE_RC=1` to count actual allocations, or
Valgrind's `--tool=massif` for heap profiling.

**Usage:** Track this metric for directional insight into reuse improvements. Do NOT
use it as a gate criterion or treat it as evidence of runtime allocation reduction.

- [ ] Build a static allocation-site counting tool:
  ```bash
  # Count Construct/PartialApply (static allocation sites) in ARC IR dump
  ORI_DUMP_AFTER_ARC=1 ori build file.ori 2>&1 | grep -c "Construct\|PartialApply"
  ```

- [ ] Compare old-vs-new static allocation-site counts:
  | Program | Old Sites | AIMS Sites | Reduction |
  |---------|-----------|------------|-----------|
  | Simple function | ? | ? | ? |
  | List operations | ? | ? | ? |
  | Tree manipulation | ? | ? | ? |
  | Pattern matching | ? | ? | ? |
  | Closure-heavy | ? | ? | ? |

- [ ] Track static allocation-site count trends across programs (directional metric,
  not a hard gate — reduction indicates reuse improvements)

---

## 08.2b FIP Certification Coverage (Stage 2)

Measure the coverage and accuracy of FIP certification (available after Stage 2).

- [ ] Count functions with `FipContract::Certified`:
  - How many functions are unconditionally certified FIP?
  - What percentage of all functions?

- [ ] Count functions with `FipContract::Conditional`:
  - How many have conditional FIP (requires unique params)?
  - How many call sites satisfy the conditions at compile time?

- [ ] FBIP achieved vs missed:
  - For functions with `is_fbip` attribute: how many achieve full FBIP?
  - For auto-FBIP functions: how many new auto-FBIP functions does AIMS find
    compared to the old pipeline?

- [ ] Compile-time overhead of FIP certification:
  - Measure analysis time with and without FIP inference
  - FIP must not add more than 5% to total analysis time

---

## 08.3 Test Matrix

Comprehensive testing across all compiler features.

- [ ] **Unit tests:** `cargo test -p ori_arc --features aims -- aims` — all AIMS-specific tests
- [ ] **Integration tests:** `cargo test --workspace --features aims` — all tests with AIMS
- [ ] **Spec tests (interp):** `cargo build --features aims && ./target/debug/ori test tests/`
- [ ] **Spec tests (LLVM):** `cargo build --features aims --release && ./target/release/ori test --backend=llvm tests/`
- [ ] **AOT tests:** `cargo test -p ori_llvm --features aims` — LLVM codegen + AOT tests
- [ ] **Full suite (old pipeline):** `./test-all.sh` — confirms old pipeline not broken
- [ ] **Cache feature:** `cargo test -p ori_arc --features cache,aims` — verify AIMS output
  is serialization-compatible (no new non-skipped fields that break deserialization)
- [ ] **Clippy:** `./clippy-all.sh` — no warnings
- [ ] **Formatting:** `./fmt-all.sh` — no formatting drift

---

## 08.4 Safety Verification

- [ ] **Valgrind:** Run `diagnostics/valgrind-aot.sh` on representative programs
- [ ] **Leak check:** `ORI_CHECK_LEAKS=1` on all spec tests — 0 leaks
- [ ] **RC balance:** `diagnostics/rc-stats.sh` on all programs — balanced
- [ ] **Codegen audit:** `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_STRICT=1` — no warnings
- [ ] **Stress test:** Programs with 10,000+ allocations, deep recursion, complex
  pattern matching — no crashes, no leaks

---

## 08.5 Performance Validation

- [ ] **Compilation speed:**
  - Measure ARC pipeline cost by timing `ori build` on representative Ori programs
    with `ORI_LOG=ori_arc=trace` to isolate pipeline time. Compare wall-clock times
    for the same programs under both pipelines. `cargo test` timing measures test
    infrastructure overhead, not pipeline performance on real programs.
  - AIMS must not regress compilation time by more than 10%
  - If `cargo bench -p oric` benchmarks exist, run those too
  - Until `./test-all.sh` is updated with `--features aims` support, use the manual
    commands from Section 06.1: `cargo test --workspace --features aims`,
    `cargo build --features aims && ./target/debug/ori test tests/`, etc.
  - **Compile-time overhead of normalization + analysis**: measure AIMS
    analysis time (normalize + prove + realize) vs old pipeline pass time.
    Break down: normalization %, interprocedural %, intraprocedural %, emission %

- [ ] **Codegen quality:**
  - Run `scripts/perf-baseline.sh` with both pipelines
  - Compare generated binary performance on `tests/benchmarks/` programs
  - AIMS-generated binaries must not regress by more than 5% on any benchmark

- [ ] **Benchmark categories** (from improvements.md Change 9):
  - List map / reverse / concat
  - Tree rebalance / insert / rotate-heavy code
  - Closure-heavy higher-order code
  - Pattern-match-heavy code
  - Collection COW stress
  - Deep recursive constructor contexts (Stage 3 TRMC benefits)

- [ ] **Memory usage:**
  - Measure peak RSS during `ori build` on a large program (e.g., 1000+ functions)
    with both pipelines, using `/usr/bin/time -v` or equivalent
  - AIMS state map should use comparable or less memory than the multiple
    intermediate data structures it replaces

---

## 08.6 Documentation

- [ ] Update `docs/compiler/design/09-arc-system/index.md` with AIMS architecture
- [ ] Update `.claude/rules/arc.md` with new pipeline description:
  - Update pipeline listing (current shows ~14 steps)
  - Update "Key Types" table with AIMS types (`AimsState`, `MemoryContract`, `AimsStateMap`)
  - Update "Crate Structure" table (new `aims/` module)
  - Update "Critical Rules" section (new pass ordering)
- [ ] Update `CLAUDE.md`:
  - Add `--features aims` to build/test commands
  - Update "Cargo Build" section if new aliases added
- [ ] Update `.claude/rules/cargo.md` — document `aims` feature in features section
- [ ] Update memory files with AIMS patterns and learnings
- [ ] Add `//!` module docs to all new AIMS files
- [ ] Update `ori_arc/Cargo.toml` description to mention AIMS

---

## 08.7 Completion Checklist

- [ ] Behavioral equivalence: 0 semantic failures across all spec tests (hard gate)
- [ ] RC count: no meaningful regressions; improvements expected but not required
  for initial cutover (informational metric during Stage 1C, hard gate post-1D)
- [ ] Allocation count: tracked directionally (secondary metric, not a gate)
- [ ] FIP certification coverage measured and documented (Stage 2)
- [ ] FBIP achieved vs missed reuse opportunities documented
- [ ] Full test suite: `./test-all.sh` green (old pipeline unchanged) AND manual
  AIMS test commands green (see Section 06.1 — `./test-all.sh` does not yet
  support `--features aims`; add AIMS path to the script as part of Stage 1D)
- [ ] Clippy: `./clippy-all.sh` green
- [ ] Valgrind: 0 memory errors
- [ ] Leak check: 0 leaks
- [ ] RC balance: all functions balanced
- [ ] Codegen audit: 0 warnings in strict mode
- [ ] Performance: compilation speed equal or better
- [ ] Compile-time breakdown documented (normalize, interprocedural, intraprocedural, emission)
- [ ] Documentation: all design docs updated
- [ ] Same-compiler comparison methodology applied (Exploring Perceus for OCaml doctrine)

**Exit Criteria:** Every verification step above passes. The AIMS pipeline is
demonstrably correct (same behavior), demonstrably at least as efficient as the
old pipeline (RC ops ≤ old for every program post-Stage 1D; allocation count
tracked directionally as a secondary metric, not a universal gate), and
demonstrably maintainable (less code, cleaner architecture). At this point, the
old pipeline can be deleted and AIMS becomes the sole ARC analysis path.
