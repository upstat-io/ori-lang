---
section: "12"
title: "Verification & Benchmarks"
status: not-started
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

**Status:** Not Started
**Goal:** Every optimization in §01-§11 is proven correct by testing (semantic equivalence with unoptimized code), proven safe by Valgrind (no memory errors), and measured for performance impact (speedup/memory reduction vs. baseline).

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
  - `#[repr(C)]` struct → declaration order preserved
  - Tuple `(bool, int, bool)` → reordered

- [ ] **§07 Enum Repr:**
  - `Option<bool>` → 1 byte (niche)
  - `Option<Ordering>` → 1 byte (niche)
  - `Option<str>` → 16 bytes (null niche)
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

- [ ] Extend `./scripts/dual-exec-verify.sh` to compare:
  - **(a) Interpreter (eval)** vs **AOT with all optimizations**
  - **(b) AOT without optimizations** vs **AOT with all optimizations**
  - Mode (b) requires a flag to disable representation optimization

- [ ] Add `--no-repr-opt` flag to `ori build`:
  - Skips all §02-§11 optimizations
  - Uses canonical representations (current behavior)
  - Produces reference output for comparison

- [ ] Run comparison on ALL spec tests:
  ```bash
  ./scripts/dual-exec-verify.sh --all --compare-repr-opt
  ```
  - Every test must produce bit-identical output (same values, same ordering)
  - Float comparisons use ULP (units in last place) tolerance for printing

- [ ] Run comparison on benchmark programs:
  - `tests/benchmarks/fibonacci.ori`
  - `tests/benchmarks/` (all)
  - Results must match exactly (integer benchmarks) or within tolerance (float)

---

## 12.3 Memory Safety Verification

- [ ] **Valgrind (heap memory):**
  ```bash
  ./scripts/valgrind-aot.sh tests/valgrind/
  ```
  - All existing Valgrind tests must pass
  - Add new Valgrind tests for:
    - SSO string operations (inline → heap transitions)
    - SVO list operations (inline → heap transitions)
    - Stack-promoted values with RC'd fields
    - Narrowed struct fields with padding
    - Niche-filled enum pattern matching

- [ ] **AddressSanitizer (stack memory):**
  ```bash
  ORI_ASAN=1 cargo bl && ./scripts/asan-test.sh
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
  ./scripts/perf-baseline.sh --release > baseline.json
  ```

- [ ] **Post-optimization measurement:**
  ```bash
  ./scripts/perf-baseline.sh --release > optimized.json
  ./scripts/perf-compare.sh baseline.json optimized.json
  ```

- [ ] **Metrics to track:**

  | Metric | Measurement | Target |
  |--------|------------|--------|
  | Compile time | `time ori build` | < 10% regression |
  | AOT binary size | `ls -la output` | ≤ 5% increase from extra codepaths |
  | Runtime (fibonacci) | `time ./fibonacci` | ≥ 10% improvement |
  | Runtime (string-heavy) | `time ./string_bench` | ≥ 30% improvement (SSO) |
  | Memory (struct-heavy) | peak RSS | ≥ 20% reduction (narrowing) |
  | Memory (collection-heavy) | peak RSS | ≥ 30% reduction (SVO + SSO) |
  | RC operations | `grep ori_rc output.ll \| wc -l` | ≥ 40% fewer (triviality + escape) |

- [ ] **Microbenchmarks** (add to `oric/benches/`):
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
  - New runtime functions (`ori_rc_*_nonatomic`, SSO, SVO)
  - `--no-repr-opt` flag documentation

- [ ] Update spec (`docs/ori_lang/v2026/spec/22-system-considerations.md`):
  - Mark implemented optimizations as "implemented" vs "future"
  - Add SSO/SVO to the built-in type representations table

- [ ] Update `.claude/rules/` with:
  - ReprPlan query patterns
  - How to add new representation optimizations

- [ ] Update `plans/repr-opt/00-overview.md` with final metrics

---

## 12.7 Completion Checklist

- [ ] Test matrix covers all §02-§11 features (every checkbox in 12.1)
- [ ] Dual-execution verification: 0 mismatches across all spec tests
- [ ] Valgrind: 0 errors across all Valgrind tests (old + new)
- [ ] AddressSanitizer: 0 errors
- [ ] Stress tests pass (10M allocations, 10K threads, 100MB packed array)
- [ ] Performance baselined with before/after comparison
- [ ] No compile-time regression > 10%
- [ ] Runtime improvement ≥ 10% on benchmark suite
- [ ] Memory reduction ≥ 20% on struct-heavy benchmarks
- [ ] RC operation count reduced ≥ 40% on typical programs
- [ ] Code journeys pass — eval/AOT match
- [ ] All documentation updated
- [ ] `./test-all.sh` green
- [ ] `./clippy-all.sh` green

**Exit Criteria:** Running `./scripts/perf-compare.sh baseline.json optimized.json` shows:
- Runtime: geometric mean ≥ 10% improvement across benchmark suite
- Memory: geometric mean ≥ 20% reduction across benchmark suite
- RC operations: ≥ 40% fewer in generated LLVM IR
- Correctness: 0 mismatches in dual-execution, 0 Valgrind errors
- All commands: `./test-all.sh`, `./clippy-all.sh`, `./llvm-test.sh` green
