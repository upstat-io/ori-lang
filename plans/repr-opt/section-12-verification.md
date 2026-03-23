---
section: "12"
title: "Verification & Benchmarks"
status: not-started
reviewed: false
third_party_review:
  status: findings
  updated: 2026-03-23
goal: "Prove correctness and measure performance of all representation optimizations through exhaustive testing, dual-execution verification, memory safety validation, and performance benchmarks"
inspired_by:
  - "Rust crater (tools/crater) — whole-ecosystem regression testing"
  - "LLVM test-suite (test-suite/) — comprehensive benchmarking infrastructure"
  - "Go std benchmark suite (test/bench/)"
depends_on: ["01", "02", "03", "04", "05", "06", "07", "08", "09", "10", "11"]
sections:
  - id: "12.1"
    title: "Test Matrix"
    status: not-started
  - id: "12.2"
    title: "Dual-Execution Equivalence"
    status: not-started
  - id: "12.3"
    title: "Memory Safety Verification"
    status: not-started
  - id: "12.4"
    title: "Performance Benchmarks"
    status: not-started
  - id: "12.5"
    title: "Code Journeys"
    status: not-started
  - id: "12.6"
    title: "Documentation"
    status: not-started
  - id: "12.7"
    title: "Completion Checklist"
    status: not-started
---

# Section 12: Verification & Benchmarks

**Context:** Representation optimization is uniquely dangerous because bugs manifest as silent data corruption rather than crashes. If a value is narrowed incorrectly, the program produces wrong results without any error. The only way to catch this is exhaustive comparison between optimized and unoptimized paths.

**Reference implementations:**
- **Rust crater**: Tests compiler changes against the entire crates.io ecosystem
- **LLVM test-suite**: Standardized benchmarks for measuring optimization impact
- **Go** `test/bench/`: Microbenchmarks for individual operations + macrobenchmarks for real programs

**Depends on:** ALL sections (this is the final verification).

---

## 12.1 Test Matrix

Build a comprehensive test matrix covering every optimization through the full pipeline.

- [ ] **§02 Transitive Triviality:**
  - `Option<int>` — trivial (no RC)
  - `Option<str>` — non-trivial (RC)
  - `(int, bool, float)` — trivial
  - `(int, str)` — non-trivial
  - `Result<int, Ordering>` — trivial
  - `Result<int, str>` — non-trivial
  - Nested: `Option<Option<int>>` — trivial
  - Recursive struct — non-trivial
  - Struct with all scalar fields — trivial
  - Newtype `type UserId = int` — trivial (resolves through Named)
  - Newtype `type Name = str` — non-trivial
  - FFI type `CPtr` — trivial (opaque pointer, no RC)
  - Generic `Pair<int>` — trivial vs `Pair<str>` — non-trivial
  - All primitive tags covered (exhaustive — 12 variants)
  - Iterator/DoubleEndedIterator — trivial (Box-allocated, no RC header)

- [ ] **§04 Integer Narrowing:**
  - Loop counter `for i in 0..100` → i8
  - Struct field with bounded constructor → narrowed
  - Function parameter with single call site → narrowed
  - Public function parameter → NOT narrowed
  - Arithmetic on narrowed values → correct overflow handling
  - Cross-module function call → correct ABI widening

- [ ] **§05 Float Narrowing:**
  - Constant 0.5 in struct → f32 storage
  - Arithmetic result → NOT narrowed (conservative)
  - Mixed f32-storage + f64-arithmetic → correct fpext/fptrunc

- [ ] **§06 Struct Layout:**
  - `{ bool, int, bool }` → reordered (int first)
  - `{ int, int }` → unchanged (already optimal)
  - `#repr("c")` struct → declaration order preserved
  - `#repr("transparent")` newtype → same layout as inner field
  - `#repr("aligned", 16)` struct → alignment ≥ 16
  - `#repr("packed")` struct → no padding, alignment = 1
  - Tuple `(bool, int, bool)` → reordered

- [ ] **§07 Enum Repr:**
  - `Option<bool>` → 1 byte (niche)
  - `Option<Ordering>` → 1 byte (niche)
  - `Option<str>` → 24 bytes (null data-ptr niche, no tag field — same size as str itself)
  - All-unit enum → tag only
  - Single-variant enum → newtype erasure

- [ ] **§08 Escape Analysis:**
  - Temporary list → stack-promoted
  - Returned list → NOT stack-promoted
  - Closure-captured value → escapes
  - Borrowed parameter → no escape through callee

- [ ] **§09 ARC Header:**
  - Non-escaping alloc → no header
  - Bounded sharing → narrow header
  - Unbounded sharing → full i64 header

- [ ] **§10 Thread-Local ARC:**
  - Single-threaded program → all non-atomic
  - Multi-threaded with channel → shared values atomic, local values non-atomic

- [ ] **§11 Collections:**
  - Short string → SSO
  - Long string → heap
  - Empty list → inline
  - Large list → heap
  - `[bool]` → packed

---

## 12.2 Dual-Execution Equivalence

Verify that optimized code produces identical results to unoptimized code.

- [ ] Extend `./diagnostics/dual-exec-verify.sh` to compare:
  - **(a) Interpreter (eval)** vs **AOT with all optimizations**
  - **(b) AOT without optimizations** vs **AOT with all optimizations**
  - Mode (b) requires a flag to disable representation optimization

- [ ] Add `--no-repr-opt` flag to `ori build`:
  - Skips all §02-§11 optimizations
  - Uses canonical representations (current behavior)
  - Produces reference output for comparison

- [ ] Run comparison on ALL spec tests:
  ```bash
  ./diagnostics/dual-exec-verify.sh --all --compare-repr-opt
  ```
  - Every test must produce bit-identical output (same values, same ordering)
  - Float comparisons must also be bit-identical — no ULP tolerance. The §05
    narrowing guarantee is "zero precision loss", so any output difference
    indicates a narrowing bug or a printing bug, both of which must be caught.
    (If a future optimization allows lossy narrowing via opt-in `#repr("f32")`,
    those specific tests can use ULP tolerance, but the default must be exact.)

- [ ] Run comparison on benchmark programs:
  - `tests/benchmarks/bench_small.ori`
  - `tests/benchmarks/bench_medium.ori`
  - `tests/benchmarks/` (all)
  - Results must match exactly (bit-identical for both integer and float benchmarks,
    consistent with the zero-precision-loss guarantee in §05)

---

## 12.3 Memory Safety Verification

- [ ] **Valgrind (heap memory):**
  ```bash
  ./diagnostics/valgrind-aot.sh tests/valgrind/
  ```
  - All existing Valgrind tests must pass
  - Add new Valgrind tests for:
    - SSO string operations (inline → heap transitions)
    - SVO list operations (inline → heap transitions)
    - Stack-promoted values with RC'd fields
    - Narrowed struct fields with padding
    - Niche-filled enum pattern matching

- [ ] **Create `diagnostics/asan-test.sh`** (new script — required for AddressSanitizer testing):
  - Build `ori` with AddressSanitizer: `RUSTFLAGS="-Zsanitizer=address" cargo +nightly b --target x86_64-unknown-linux-gnu`
  - Run the spec test suite with the ASan-enabled binary
  - Run the Valgrind test suite programs through the ASan binary
  - Exit non-zero if any ASan report fires (exit code check)
  - Follow the pattern of `valgrind-aot.sh`: support `--no-color` flag, print summary at end
- [ ] **AddressSanitizer (stack memory):**
  ```bash
  ./diagnostics/asan-test.sh
  ```
  - Stack-promoted values must not be accessed after function return
  - No buffer overflows in packed bool arrays
  - No out-of-bounds in narrow-element collections

- [ ] **Stress tests:**
  - Create 10M small allocations → stress RC header compression
  - Create 10K threads sharing values → stress atomic/non-atomic boundary
  - Deeply nested `Option<Option<...<int>>>` → stress niche filling
  - 100MB packed bool array → stress packed operations

---

## 12.4 Performance Benchmarks

- [ ] **Baseline (before optimizations):**
  ```bash
  # NOTE: perf-baseline.sh currently emits a human-readable table, not JSON.
  # Either add a --json flag to perf-baseline.sh, or create perf-compare.sh
  # to parse the existing table format. Using .txt extension to match current output.
  ./scripts/perf-baseline.sh --release > baseline.txt
  ```

- [ ] **Create `scripts/perf-compare.sh`** (new script — required for baseline comparison):
  - Takes two `perf-baseline.sh` output files as arguments
  - Parses the table format from `perf-baseline.sh` (human-readable, not JSON)
  - Reports per-benchmark delta, geometric mean improvement/regression, and highlights any metric exceeding its threshold
  - Exits non-zero if any threshold is violated (per the targets table in §12.4)
  - Follow the pattern of `cow-benchmark.sh --compare` for argument parsing and output formatting
- [ ] **Post-optimization measurement:**
  ```bash
  ./scripts/perf-baseline.sh --release > optimized.txt
  ./scripts/perf-compare.sh baseline.txt optimized.txt
  ```

- [ ] **Metrics to track:**

  | Metric | Measurement | Target |
  |--------|------------|--------|
  | Compile time | `time ori build` | < 10% regression |
  | AOT binary size | `ls -la output` | ≤ 5% increase from extra codepaths |
  | Runtime (bench_medium) | `time ./bench_medium` | ≥ 10% improvement |
  | Runtime (string-heavy) | `time ./string_bench` | ≥ 30% improvement (SSO) |
  | Memory (struct-heavy) | peak RSS | ≥ 20% reduction (narrowing) |
  | Memory (collection-heavy) | peak RSS | ≥ 30% reduction (SVO + SSO) |
  | RC operations | `grep ori_rc output.ll \| wc -l` | ≥ 40% fewer (triviality + escape) |

- [ ] **Microbenchmarks** (add to `compiler/oric/benches/`):
  - `repr_narrowing`: Measure ReprPlan computation time
  - `range_analysis`: Measure range analysis time per function
  - `escape_analysis`: Measure escape analysis time per function
  - `rc_atomic_vs_nonatomic`: Measure RC operation throughput

- [ ] **Macrobenchmarks** (add to `tests/benchmarks/`):
  - `string_processing.ori`: Short string manipulation (SSO benefit)
  - `data_structures.ori`: Small struct creation/destruction (narrowing benefit)
  - `option_heavy.ori`: Option<int> manipulation (niche benefit)
  - `arc_heavy.ori`: Many small heap allocations (header compression benefit)

---

## 12.5 Code Journeys

Run `/code-journey` to test the full pipeline end-to-end with progressively complex programs.

- [ ] Run `/code-journey` with programs exercising each optimization:
  - Journey 1: Simple narrowing (loop counter, struct field)
  - Journey 2: SSO strings (creation, concat, slice)
  - Journey 3: Escape analysis (temporary values, closures)
  - Journey 4: Thread-local ARC (single-threaded vs multi-threaded)
  - Journey 5: Combined (program using all optimizations together)

- [ ] All CRITICAL findings triaged (fixed or tracked)
- [ ] Eval and AOT paths produce identical results for all journeys
- [ ] Journey results archived in `plans/code-journeys/`

---

## 12.6 Documentation

- [ ] Update CLAUDE.md with:
  - `ori_repr` crate description and key paths
  - `MachineRepr` enum variant summary
  - `ReprPlan` query interface and tracing
  - `ReprAttribute` enum and `#repr` attribute interaction
  - New runtime functions (`ori_rc_*_nonatomic`, `ori_rc_*_i8/i16/i32`, SSO, SVO)
  - `--no-repr-opt` flag documentation
  - `ori_repr` dependency chain: `ori_types → ori_arc → ori_repr → ori_llvm`

- [ ] Update spec (`docs/ori_lang/v2026/spec/annex-e-system-considerations.md`):
  - Mark implemented optimizations as "implemented" vs "future"
  - Update the built-in type representations table to match the current runtime baseline before adding new optimizations:
    - `str` is the 24-byte SSO-capable form, not `{ len, data }`
    - `Option<T>` / `Result<T, E>` currently lower with `i64` tags in LLVM, not `i8`
    - RC-managed heap values currently use the V5 32-byte header (`data_size`, `elem_dec_fn`, `elem_count`, `strong_count`)
  - Add SSO/SVO to the built-in type representations table

- [ ] Update `.claude/rules/` with:
  - ReprPlan query patterns
  - How to add new representation optimizations

- [ ] Update `plans/repr-opt/00-overview.md` with final metrics

---

## 12.7 Completion Checklist

**Scripts created in this section:**
- [ ] `diagnostics/asan-test.sh` created and functional (builds ASan binary, runs spec+valgrind tests, exits non-zero on ASan report)
- [ ] `scripts/perf-compare.sh` created and functional (parses `perf-baseline.sh` output, reports deltas, exits non-zero on threshold violations)
- [ ] `tests/valgrind/threads/` directory created with `thread_local_only.ori` and `channel_send.ori`

**Verification:**
- [ ] Test matrix covers all §02-§11 features (every checkbox in 12.1)
- [ ] Dual-execution verification: 0 mismatches across all spec tests
- [ ] Valgrind: 0 errors across all Valgrind tests (old + new)
- [ ] AddressSanitizer: 0 errors (via `diagnostics/asan-test.sh`)
- [ ] Helgrind: 0 races in threading test programs (via `./diagnostics/valgrind-aot.sh --helgrind tests/valgrind/threads/`)
- [ ] Stress tests pass (10M allocations, 10K threads, 100MB packed array)
- [ ] Performance baselined with before/after comparison (via `scripts/perf-compare.sh`)
- [ ] No compile-time regression > 10%
- [ ] Runtime improvement ≥ 10% on benchmark suite
- [ ] Memory reduction ≥ 20% on struct-heavy benchmarks
- [ ] RC operation count reduced ≥ 40% on typical programs
- [ ] Code journeys pass — eval/AOT match
- [ ] All documentation updated
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green

**Exit Criteria:** Running `./scripts/perf-compare.sh baseline.txt optimized.txt` shows:
- Runtime: geometric mean ≥ 10% improvement across benchmark suite
- Memory: geometric mean ≥ 20% reduction across benchmark suite
- RC operations: ≥ 40% fewer in generated LLVM IR
- Correctness: 0 mismatches in dual-execution, 0 Valgrind errors
- All commands: `./test-all.sh`, `./clippy-all.sh`, `./llvm-test.sh` green

---

## 12.R Third Party Review Findings

- [ ] `[TPR-12-001][minor]` `section-12-verification.md:127-131` — **No interpreter vs AOT-unoptimized sanity test.** §12.2 specifies (a) interpreter vs AOT-optimized and (b) AOT-unoptimized vs AOT-optimized. Missing: (c) interpreter vs AOT with `--no-repr-opt` as a sanity check that the bypass flag itself works correctly. If codegen changes in §04/§06/§07 accidentally break the unoptimized path, comparison (b) would produce false positives. **Action:** Add comparison (c): interpreter vs AOT-unoptimized to the dual-execution verification matrix.
