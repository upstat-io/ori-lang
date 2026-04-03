---
section: "06"
title: "Verification"
status: not-started
reviewed: false
goal: "Comprehensive test matrix, behavioral equivalence verification, and code journey proving all ARC optimizations are correct across the full type × pattern × CFG matrix"
depends_on: ["01", "02", "03", "04", "05"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "06.1"
    title: "Test Matrix"
    status: not-started
  - id: "06.2"
    title: "Behavioral Equivalence"
    status: not-started
  - id: "06.3"
    title: "Code Journey"
    status: not-started
  - id: "06.4"
    title: "Safety Verification"
    status: not-started
  - id: "06.5"
    title: "Performance Validation"
    status: not-started
  - id: "06.6"
    title: "Documentation and Cleanup"
    status: not-started
  - id: "06.7"
    title: "Completion Checklist"
    status: not-started
---

# Section 06: Verification

**Status:** Not Started
**Goal:** Prove that all ARC optimizations from Sections 01-05 are correct, produce identical observable behavior to the unoptimized baseline, maintain zero leaks, and provide measurable performance improvement.

**Depends on:** All prior sections.

---

## 06.1 Test Matrix

Build a comprehensive test matrix covering every optimization through every relevant type × pattern × CFG combination.

- [ ] **Section 01 (Statistics):**
  - Verify `SynergyMetrics` fields are correctly populated for all test programs
  - `rc_ops_post_emission > 0` for programs with heap types (str, [int], closures)
  - `rc_ops_post_emission == 0` for int-only programs
  - `coalesce_reduction_percent()` returns plausible values (0-100%)
  - `CoalesceStats` counts match manual IR inspection for 3+ programs
  - Statistics do not regress program output (pure instrumentation)

- [ ] **Section 02 (Barriers):**
  - Type: str, [int], Option\<str\>, closures, {str: int} map, Set\<str\>
  - Call pattern: all-borrowed, all-owned, mixed, no-contract (FFI)
  - CFG: linear, loop body, conditional call

- [ ] **Section 03 (KnownSafe):**
  - Type: str, [int], Option\<str\>, closures, structs with heap fields
  - Nesting: 1-deep (no elim), 2-deep (one pair), 3-deep (two pairs)
  - CFG: linear, diamond (both arms increment), loop (invariant RC)

- [ ] **Section 04 (COW Contraction):**
  - Type: struct with mutable field, [int], {str: int}
  - CowMode: StaticUnique, Dynamic, StaticShared
  - Pattern: single mutation, loop mutation, conditional mutation

- [ ] **Section 05 (RC Motion):**
  - Type: str, [int], Option\<str\>, closures, maps
  - CFG: diamond (retain in pred, release in succs), triangle, loop, nested loop, early return
  - Pattern: same-block pair, cross-block pair, loop-invariant RC

### 06.1.1 Discovered Gaps

| Gap | Roadmap Location | Test | Severity |
|-----|-----------------|------|----------|
| (to be filled during implementation) | | | |

---

## 06.2 Behavioral Equivalence

Verify that optimized programs produce identical output to unoptimized.

- [ ] **Implement `ORI_SKIP_ARC_OPTS` flag** in the AIMS pipeline:
  - Add environment variable check in `run_aims_pipeline()` to skip Sections 02-05 optimizations (selective barriers, KnownSafe elimination, COW contraction, RC motion) while keeping baseline RC emission
  - This flag enables A/B comparison testing between optimized and unoptimized builds
  - Implementation: check `std::env::var("ORI_SKIP_ARC_OPTS")` at pipeline entry; when set, skip the new passes but run the existing baseline pipeline

- [ ] Build comparison harness:
  - Compile each test program twice: with and without new optimizations
  - Run both and compare stdout, stderr, exit code
  - Report any mismatches

- [ ] Apply to all Ori spec tests:
  ```bash
  for test in tests/spec/**/*.ori; do
      ORI_SKIP_ARC_OPTS=1 ori run "$test" > /tmp/baseline
      ori run "$test" > /tmp/optimized
      diff /tmp/baseline /tmp/optimized
  done
  ```
  Additionally, use `diagnostics/dual-exec-verify.sh` which compares interpreter vs LLVM output — behavioral equivalence with the interpreter (which has no ARC optimization) is a stronger guarantee than comparing two LLVM builds. Both methods should be used.

- [ ] Track and investigate every mismatch.

---

## 06.3 Code Journey

Run `/code-journey` to test the full pipeline end-to-end with progressively complex programs.

- [ ] Run `/code-journey` — journeys escalate until the compiler breaks down
- [ ] All CRITICAL findings from journey results triaged (fixed or tracked)
- [ ] Eval and AOT paths produce identical results for all passing journeys
- [ ] Journey results archived in `plans/code-journeys/`

---

## 06.4 Safety Verification

- [ ] **RC balance:** `diagnostics/rc-stats.sh` on all test programs → balanced
- [ ] **Leak check:** `ORI_CHECK_LEAKS=1` on all test programs → zero leaks
- [ ] **Valgrind:** `diagnostics/valgrind-aot.sh` on representative programs → no memory errors
- [ ] **Codegen audit:** `ORI_AUDIT_CODEGEN=1 ORI_AUDIT_STRICT=1` on all test programs → no findings
- [ ] **Stress test:** 1000+ allocations, 100+ recursion depth, 1000+ list elements — all clean

---

## 06.5 Performance Validation

- [ ] **RC operation reduction:**
  - Baseline: `ORI_LOG=ori_arc=info ori build tests/spec/` with Sections 02-05 disabled
  - Optimized: same with all sections enabled
  - Target: 30%+ total RC operation reduction on typical programs
  - Script: `diagnostics/aims-compare.sh`

- [ ] **Compile-time overhead:**
  - Measure compilation time with and without new passes
  - Target: < 5% compile-time regression
  - Run `cargo bench -p oric` if parser/typeck benchmarks exist

- [ ] **Runtime performance:**
  - Benchmark programs in `tests/benchmarks/`
  - Measure execution time with and without optimizations
  - Target: measurable improvement on RC-heavy programs

---

## 06.6 Documentation and Cleanup

- [ ] Update `CLAUDE.md` with new pipeline steps if applicable
- [ ] Update `.claude/rules/arc.md` with new pass descriptions
- [ ] Update `compiler/ori_arc/src/aims/mod.rs` module docs
- [ ] Add architecture notes to `compiler/ori_arc/src/aims/knownsafe/mod.rs`
- [ ] Add architecture notes to `compiler/ori_arc/src/aims/rc_motion/mod.rs`
- [ ] **Plan annotation cleanup**: strip ALL code annotations referencing this plan (`clang-arc-lessons`, section numbers `01`-`06`, any plan-specific markers) from all source files. Run `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan clang-arc-lessons` and verify 0 annotations remain. Only spec references (`Spec: Clause N.M`) are permanent.

---

## 06.7 Completion Checklist

- [ ] Test matrix covers all sections (01-05) × types × patterns × CFG combinations
- [ ] `ORI_SKIP_ARC_OPTS` flag implemented and functional
- [ ] Behavioral equivalence verified — 0 mismatches across all spec tests (both A/B and dual-exec)
- [ ] Code journey passes — eval/AOT match, no CRITICAL findings
- [ ] RC balance verified via `rc-stats.sh` — all functions balanced
- [ ] `ORI_CHECK_LEAKS=1` — zero leaks on all test programs
- [ ] Valgrind clean on representative programs
- [ ] Codegen audit clean (`ORI_AUDIT_STRICT=1`)
- [ ] Stress tests pass
- [ ] 30%+ total RC operation reduction on typical programs
- [ ] < 5% compile-time regression
- [ ] All documentation updated
- [ ] Plan annotation cleanup: `plan-annotations.sh` returns 0 annotations for this plan
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green
- [ ] `/tpr-review` passed — independent Codex review clean
- [ ] `/impl-hygiene-review last commit` passed — hygiene review clean

**Exit Criteria:** All test programs produce identical output with and without optimizations. Zero leaks. Zero valgrind errors. Zero codegen audit findings. `./test-all.sh` passes with 0 regressions across all ~N tests. RC operation reduction measured and documented. Compile-time overhead < 5%.
