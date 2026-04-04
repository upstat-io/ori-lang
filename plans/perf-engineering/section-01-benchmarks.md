---
section: "01"
title: "Benchmark Infrastructure"
status: not-started
reviewed: true
goal: "Establish reproducible interpreter performance measurement with Criterion benchmarks and gate tests"
inspired_by:
  - "Roc interpreter benchmarks (crates/compiler/test_mono/)"
  - "Ori parser benchmarks (compiler/oric/benches/parser.rs)"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "01.1"
    title: "Criterion Interpreter Benchmarks"
    status: not-started
  - id: "01.2"
    title: "Ackermann Gate Test"
    status: not-started
  - id: "01.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "01.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 01: Benchmark Infrastructure

**Status:** Not Started
**Goal:** Reproducible, automated interpreter performance measurement. Every future section measures its impact against this baseline. Without measurement, optimization is guesswork.

**Context:** The Ori compiler has Criterion benchmarks for the parser/lexer (`compiler/oric/benches/parser.rs`, `lexer.rs`) but none for the evaluator. The 63µs/call measurement was ad-hoc (`time` command on Ackermann). This section creates proper infrastructure following the established Criterion pattern.

**Reference implementations:**
- **Ori** `compiler/oric/benches/parser.rs`: Criterion pattern with Salsa DB setup, `black_box`, throughput measurement
- **Roc** `crates/compiler/test_mono/`: Benchmark programs that stress specific compiler phases

**Depends on:** Nothing — this is the foundation.

---

## 01.1 Criterion Interpreter Benchmarks

**File(s):** `compiler/oric/benches/interpreter.rs` (new)

Create a Criterion benchmark suite that measures interpreter function call overhead directly. Follow the existing pattern from `compiler/oric/benches/parser.rs`.

- [ ] **Create `compiler/oric/benches/interpreter.rs`** with Criterion benchmarks:
  ```rust
  // Pattern from parser.rs — Salsa DB + SourceFile + evaluated() query
  fn bench_ackermann(c: &mut Criterion) {
      let db = CompilerDb::new();
      let source = r#"
          @ack (0: int, n: int) -> int = n + 1;
          @ack (m: int, 0: int) -> int = ack(m: m - 1, n: 1);
          @ack (m: int, n: int) -> int = ack(m: m - 1, n: ack(m:, n: n - 1));
          @main () -> void = { let $_ = ack(m: 3, n: 5) }
      "#;
      let file = SourceFile::new(&db, PathBuf::from("bench.ori"), source.into());
      c.bench_function("interpreter/ackermann_3_5", |b| {
          b.iter(|| {
              // Reset Salsa DB state between iterations to measure eval, not caching
              black_box(evaluated(&db, file))
          });
      });
  }
  ```

- [ ] **Add benchmark programs** covering key performance dimensions:
  - `ackermann_3_5` — deep recursion, function clause dispatch (~42k calls)
  - `fibonacci_30` — simple recursion, single clause (~2.7M calls)
  - `loop_sum_1m` — tight loop, no function calls (measures loop overhead)
  - `closure_map_10k` — closure creation + call (measures capture cost)
  - `pattern_match_1k` — match expression dispatch (measures pattern matching)
  - `method_dispatch_10k` — method calls on structs (measures resolver chain)

- [ ] **Register in `Cargo.toml`** under `[[bench]]`:
  ```toml
  [[bench]]
  name = "interpreter"
  harness = false
  ```

- [ ] **Verify benchmarks run:** `cargo bench -p oric --bench interpreter -- --test`
- [ ] **Matrix tests** (in `compiler/oric/benches/interpreter.rs`):
  - **Dimensions**: call depth (shallow: fib(10), medium: ack(3,5), deep: ack(3,7)) x dispatch type (single-clause, multi-clause, closure, method) x value type (int-only, string-heavy, collection-heavy)
  - **Semantic pin**: `ackermann_3_5` must produce stable measurement with <5% variance across 3 consecutive runs on the dev machine (Criterion default: 100 iterations)
  - **Negative pin**: benchmark that passes a non-existent source file to `evaluated()` produces a clean error (not a panic or hang) -- validates error path doesn't corrupt benchmarking
  - **TDD ordering**: write benchmark harness, verify it runs and measures the current tree-walker, record baselines, then proceed to Section 02/04

---

## 01.2 Ackermann Gate Test

**File(s):** `tests/run-pass/rosetta/ackermann/ack_perf_test.ori` (exists), new Rust test

Create a hard performance gate: Ackermann A(3,7) must complete and its timing must be recorded through the full `oric` pipeline. This catches regressions and validates improvements without depending on a nonexistent direct `ori_eval` harness.

- [ ] **Create Rust integration test** in an existing full-pipeline test location such as `compiler/oric/tests/phases/eval/` or `compiler/oric/src/query/tests.rs`:
  ```rust
  #[test]
  fn perf_gate_ackermann_3_7() {
      // A(3,7) = 1021, ~694k recursive calls
      // Current: ~55s. Phase 1 target: <10s. Phase 2 target: <1s.
      let db = CompilerDb::new();
      let file = SourceFile::new(&db, PathBuf::from("/ack.ori"), ACKERMANN_SRC.to_string());
      let start = std::time::Instant::now();
      let result = evaluated(&db, file);
      let elapsed = start.elapsed();
      assert!(result.is_ok());
      // Gate: must complete. Time is logged, not asserted (varies by machine).
      eprintln!("A(3,7) completed in {elapsed:?}");
  }
  ```

- [ ] **Record baseline measurements** for all benchmark programs on the dev machine
- [ ] **Profile allocation vs dispatch cost** — the critical decision gate for Sections 02-03:
  - Run Ackermann under `perf` or `heaptrack` to measure: (a) time spent in `malloc`/`free` (allocation churn), (b) time spent in `eval_can` recursive dispatch (branch prediction + call overhead), (c) time spent in `create_function_interpreter` (the combined allocation + field-copy overhead).
  - If allocation churn (a+c) > 60% of total time: Sections 02-03 have high ROI, work them before bytecode.
  - If recursive dispatch (b) > 60% of total time: skip Sections 02-03, proceed directly to Section 04 (bytecode eliminates recursive dispatch entirely).
  - Document the finding in the plan — this is the profile-gated decision point for the entire plan's critical path.
  - If the decision is "skip", record that Sections 02-03 are intentionally non-blocking so later verification does not treat them as incomplete mandatory work.
- [ ] **Add `perf_gate` to test-all.sh exclusion list** (these are slow by design — run manually or in CI)

---

## 01.R Third Party Review Findings

- None.

---

## 01.N Completion Checklist

- [ ] `cargo bench -p oric --bench interpreter` runs without errors
- [ ] All 6 benchmark programs produce stable results (< 5% variance across 3 runs)
- [ ] Ackermann gate test passes (completes, even if slow)
- [ ] Baseline measurements recorded in plan or memory
- [ ] `./test-all.sh` green (no regressions from benchmark additions)
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 01` returns 0 annotations
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `cargo bench -p oric --bench interpreter -- ackermann_3_5` produces a stable measurement with <5% variance. All 6 benchmarks run. Gate test completes. Profiling gate documented: allocation-vs-dispatch split determines whether Sections 02-03 are worked or skipped.
