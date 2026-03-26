---
section: "03"
title: "Profiling Infrastructure"
status: not-started
reviewed: true
goal: "Establish reproducible profiling infrastructure that reveals where time is spent during cargo t, with per-phase timing for the AOT test pipeline."
inspired_by:
  - "Rust compiler perf.rust-lang.org — continuous performance tracking with reproducible benchmarks"
  - "cargo-flamegraph — integrated flamegraph generation for Rust binaries"
depends_on: []
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Measurement Methodology"
    status: not-started
  - id: "03.2"
    title: "Flamegraph Generation"
    status: not-started
  - id: "03.3"
    title: "AOT Test Pipeline Timing"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Profiling Infrastructure

**Status:** Not Started
**Goal:** A reproducible profiling setup exists that can generate flamegraphs and per-phase timing breakdowns for `cargo t`. The methodology produces consistent, comparable results across runs. The AOT test pipeline has per-phase instrumentation (compile time vs link time vs execution time).

**Context:** The test suite takes ~59s wall time. Before optimizing anything, we need to know exactly WHERE time is spent. Current observations (system time >> user time, AOT tests = 60%) suggest the bottleneck is I/O-heavy compile→link→execute cycles, but profiling will confirm or refute this hypothesis.

**Depends on:** Nothing — this is independent of the LCFail track.

---

## 03.1 Measurement Methodology

**File(s):** New script `scripts/bench-tests.sh`

Establish a reproducible methodology for measuring test suite performance. Results must be comparable across runs.

- [ ] Create `scripts/bench-tests.sh` that:
  1. Builds all crates first (`cargo b --workspace`) to isolate compilation from test execution
  2. Runs `cargo t --workspace` with `hyperfine` (available at `~/.cargo/bin/hyperfine`)
  3. Reports wall time, user time, system time
  4. Runs 3-5 times and reports mean + stddev
  5. Outputs results in a machine-readable format (JSON)

  ```bash
  #!/bin/bash
  # scripts/bench-tests.sh — Reproducible test suite timing
  # Usage: ./scripts/bench-tests.sh [--runs N] [--output path.json]

  # 1. Ensure clean build (compilation not measured)
  cargo b --workspace -q

  # 2. Run tests N times, capture timing
  for i in $(seq 1 $RUNS); do
      /usr/bin/time -v cargo t --workspace 2>&1 | ...
  done

  # 3. Report mean/stddev
  ```

- [ ] Define the "canonical measurement" conditions:
  - Fresh terminal, no other cargo processes
  - All crates pre-built (`cargo b --workspace`)
  - CPU governor set to performance (if accessible)
  - 3 warmup runs (discarded), then 5 measured runs
  - Report: wall time mean, user time mean, system time mean, stddev for each
  - **Variance tolerance**: stddev must be <5% of mean. If stddev exceeds 5%, add more runs until it converges or investigate system noise (background processes, thermal throttling).
  - **Reproducibility check**: two independent measurement sessions must agree within 10%.

- [ ] Run the canonical measurement and record the baseline. The overview records ~59s wall time from a single run on 2026-03-25; this subsection remeasures with formal methodology to establish a statistically valid baseline:
  ```
  Baseline (date):
    Wall time: ??? +/- ???
    User time: ??? +/- ???
    System time: ??? +/- ???
  ```

- [ ] Record per-crate timing by running individual crate tests:
  ```bash
  for crate in ori_arc ori_types ori_eval ori_parse ori_lexer ori_patterns ori_registry ori_ir ori_llvm ori_diagnostic ori_rt oric; do
      /usr/bin/time -v cargo test -p $crate 2>&1
  done
  # Note: ori_llvm includes both unit tests (--lib) and AOT integration tests (--test aot).
  # Measure them separately:
  /usr/bin/time -v cargo test -p ori_llvm --lib 2>&1
  /usr/bin/time -v cargo test -p ori_llvm --test aot 2>&1
  ```

### Test Strategy

- **Validation**: Run `bench-tests.sh` twice and verify the results are within 10% of each other (reproducibility check).
- **Semantic pin**: The baseline numbers become the "before" measurement that Section 06 compares against.

---

## 03.2 Flamegraph Generation

**File(s):** New script `scripts/flamegraph-tests.sh`

Generate flamegraphs that show WHERE CPU time is spent during test execution. This reveals which compiler functions are hot paths.

- [ ] Check if `perf` is available:
  ```bash
  perf --version  # Linux perf_events — available at /usr/local/bin/perf on this WSL2 system
  ```
  `perf` is available. Also available: `cargo-flamegraph` (at `~/.cargo/bin/cargo-flamegraph`) and `hyperfine` (at `~/.cargo/bin/hyperfine`).

- [ ] Create `scripts/flamegraph-tests.sh` that generates flamegraphs for specific test suites:
  ```bash
  #!/bin/bash
  # Usage: ./scripts/flamegraph-tests.sh [crate] [--output dir]
  # Examples:
  #   ./scripts/flamegraph-tests.sh ori_llvm    # AOT tests flamegraph
  #   ./scripts/flamegraph-tests.sh ori_eval    # Evaluator tests flamegraph
  #   ./scripts/flamegraph-tests.sh             # Full workspace flamegraph
  #
  # Requires: cargo-flamegraph (at ~/.cargo/bin/cargo-flamegraph), perf (at /usr/local/bin/perf)
  #
  # Implementation:
  #   cargo flamegraph --test <test_binary> -p <crate> -o <output.svg>
  #   For workspace: cargo flamegraph -- test --workspace -o <output.svg>
  #
  # Note: cargo-flamegraph wraps perf record + flamegraph.pl. On WSL2,
  # perf may require kernel.perf_event_paranoid=1 (check /proc/sys/kernel/perf_event_paranoid).
  ```

- [ ] Generate flamegraphs for the top 3 time consumers:
  1. **ori_llvm AOT tests** (`cargo test -p ori_llvm --test aot`) — 35.6s, the biggest target
  2. **ori_eval tests** (`cargo test -p ori_eval`) — 4.5s, second biggest (includes compilation time)
  3. **Full workspace** (`cargo t`) — overall picture

- [ ] For the AOT flamegraph, understand the limitation: since each AOT test spawns `ori build` as a subprocess, the flamegraph of `cargo test -p ori_llvm --test aot` will show the **test harness** overhead (process spawning, file I/O, result checking), NOT the Ori compiler internals. To profile the compiler itself during AOT tests:
  - **Option A**: Generate a flamegraph of a single `ori build` invocation:
    ```bash
    cargo flamegraph --bin ori -- build test_file.ori -o /tmp/test_output
    ```
  - **Option B**: Use `perf record -g` with `--all-children` to follow child processes:
    ```bash
    perf record -g --call-graph=dwarf -- cargo test -p ori_llvm --test aot -- test_name
    ```
  - Look specifically for:
    - Time spent in LLVM (`llvm::*`, `inkwell::*`) — compilation overhead
    - Time spent in linking (`ld`, `cc`, `collect2`) — linker overhead
    - Time spent in process spawning (`std::process::Command`, `fork`, `exec`) — per-test overhead
    - Time spent in the Ori compiler pipeline (`ori_types::*`, `ori_parse::*`, `ori_arc::*`) — compiler overhead
    - Time spent in I/O (`write`, `read`, `open`, `close`) — filesystem overhead

- [ ] Save flamegraph SVGs to `diagnostics/flamegraphs/` (must be gitignored — add `diagnostics/flamegraphs/` to `.gitignore` before creating this directory):
  ```
  diagnostics/flamegraphs/
  ├── aot-tests.svg
  ├── ori-eval-tests.svg
  └── full-workspace.svg
  ```

- [ ] Analyze the flamegraphs and produce a written summary:
  - Top 10 hottest functions (% of total time)
  - Which phase of the AOT pipeline dominates (compile vs link vs execute)
  - Any unexpected hot paths (functions that shouldn't be expensive)

### Test Strategy

- **Validation**: Flamegraph generation script runs without error and produces valid SVG files.
- **Deliverable**: Written analysis of the top 10 hottest functions.

---

## 03.3 AOT Test Pipeline Timing

**File(s):** `compiler/ori_llvm/tests/aot/` (test harness)

The AOT test harness runs ~1,950 `#[test]` functions, each doing: write source → spawn `ori build` subprocess (compile+link) → spawn binary subprocess → check output. Add per-phase timing to understand which phase dominates.

**Actual file structure** (verified):
- `compiler/ori_llvm/tests/aot/main.rs` — module declarations for all test categories (not a dispatcher — Rust's test framework discovers `#[test]` functions)
- `compiler/ori_llvm/tests/aot/util/aot.rs` — core test helpers: `compile_and_run_capture()`, `assert_aot_success()`, `ori_binary()`, `stdlib_path()`
- `compiler/ori_llvm/tests/aot/util/mod.rs` — re-exports from `aot`, `object`, `wasm` submodules
- Each test calls `compile_and_run_capture(source)` which: (1) creates a `TempDir`, (2) writes `.ori` source, (3) spawns `ori build` via `Command::new(ori_binary())`, (4) spawns the compiled binary, (5) captures exit code + stdout + stderr

**Key**: The compilation pipeline runs inside the `ori build` subprocess — NOT in the test process. This means per-phase timing must be added to the `ori build` command path, not to the test harness itself.

- [ ] Read the AOT test harness to understand the current flow:
  - `compiler/ori_llvm/tests/aot/util/aot.rs` — `compile_and_run_capture()` is the core function (line 149); creates `TempDir`, writes source, spawns `ori build`, spawns binary with `ORI_CHECK_LEAKS=1` (line 178)
  - `compiler/ori_llvm/tests/aot/util/aot.rs` — `ori_binary()` (line 56) discovers the `ori` binary by build profile
  - `compiler/ori_llvm/tests/aot/util/mod.rs` — re-exports `aot`, `object`, `wasm` submodules
  - `compiler/ori_llvm/Cargo.toml` — `ori_rt` is a path dependency; `ori_rt` is pre-built as a static lib that the `ori build` linker step links into each test binary
  - `compiler/oric/src/commands/build/mod.rs` — `build_file()` entry point (line 56), routes to single-file or multi-file based on `has_imports()`

- [ ] Map the actual Ori compiler pipeline phases by reading the `ori build` entry point:
  - `compiler/oric/src/commands/build/mod.rs` — the `ori build` command entry point
  - `compiler/oric/src/commands/build/single.rs` — single-file build path
  - `compiler/oric/src/commands/codegen_pipeline.rs` — compilation pipeline orchestration
  - `compiler/oric/src/commands/compile_common.rs` — shared compilation logic
  - The actual phases: lex → parse → typeck → ARC lowering → LLVM codegen → object emission → linker invocation

- [ ] **Two-level timing**: Because each AOT test spawns `ori build` as a subprocess, timing must be split:

  **(a) Per-test timing in the test harness** (`compiler/ori_llvm/tests/aot/util/aot.rs`):
  When `ORI_TEST_TIMING=1` is set, wrap `compile_and_run_capture()` (line 149) to time:
  1. **source write time** — wrap `fs::write()` at line 159 with `Instant::now()`
  2. **compile time** — wrap `Command::new(ori_binary())...output()` at lines 161-170
  3. **exec time** — wrap `Command::new(&binary_path)...output()` at lines 177-180
  4. **cleanup time** — `TempDir` drop is implicit; measure by wrapping the `drop(temp_dir)` call
  Aggregate totals in a `static Mutex<TimingStats>` and report in a test fixture or `atexit` handler. Use `std::sync::OnceLock` to check `ORI_TEST_TIMING` once (not per-test).

  **(b) Per-phase timing in `ori build`** (compiler-side):
  When `ORI_BUILD_TIMING=1` is set, `ori build` emits per-phase timing to stderr. Implementation locations:
  1. **parse time**: `compiler/oric/src/commands/build/single.rs` (wraps the lex+parse call)
  2. **typeck time**: same file (wraps the type-check call)
  3. **arc time**: `compiler/oric/src/commands/codegen_pipeline.rs` `run_borrow_inference()` (line ~60)
  4. **codegen time**: same file, `run_codegen_pipeline()` (wraps `compile_module()` or similar)
  5. **object time**: `compiler/ori_llvm/src/aot/object.rs` (wraps `emit_object()`)
  6. **link time**: `compiler/ori_llvm/src/aot/linker/driver.rs` (wraps `LinkerDriver::link()`)
  Precise insertion points in `compiler/oric/src/commands/build/single.rs`:
  - **parse+typeck**: wrap `check_source()` call at line 38 (this covers lex+parse+typeck together — further splitting requires instrumenting inside `check_source` in `compile_common.rs`)
  - **codegen**: wrap `compile_to_llvm()` call at lines 55-66 (covers ARC lowering + LLVM IR generation)
  - **object emission**: wrap `emitter.verify_optimize_emit()` at lines 123-130
  - **linking**: wrap `link_and_finish()` call at line 134
  Output format: `ORI_BUILD_TIMING: phase=parse time_ms=12` (one line per phase to stderr, machine-parseable).
  The test harness can set this env var and aggregate across all tests by parsing stderr.

  Output format:
  ```
  AOT Test Pipeline Timing (~1,950 tests):
    Source Write:  ???s (??%)
    Compile Total: ???s (??%)
      Parse:       ???s (??%)
      Typeck:      ???s (??%)
      ARC:         ???s (??%)
      Codegen:     ???s (??%)
      Object:      ???s (??%)
      Link:        ???s (??%)
    Execute:       ???s (??%)
    Overhead:      ???s (??%)
    Total:         35.6s
  ```

- [ ] Implement the timing as lightweight as possible — `std::time::Instant` around each phase. Do NOT use `tracing` for this (too much overhead for ~1,950 tests x 7 phases).

  **WARNING**: `compiler/oric/src/commands/codegen_pipeline.rs` is 485 lines (limit 500). Adding timing instrumentation may push it over. If it does, extract timing logic into a separate `compiler/oric/src/commands/build_timing.rs` helper module rather than inlining timing code into `codegen_pipeline.rs`.

- [ ] Verify the per-phase totals sum to approximately the overall wall time (within 5% — any larger gap indicates uncaptured overhead like process spawn/teardown).

- [ ] Run with `ORI_TEST_TIMING=1 ORI_BUILD_TIMING=1` and record the phase breakdown. This data directly informs Section 04 (which phase to optimize first).

### Test Strategy

This subsection adds code to both the test harness (`aot.rs`) and the compiler (`single.rs`, `codegen_pipeline.rs`). Although the code is instrumentation, it touches hot paths and must not break existing behavior.

- **TDD ordering**: Write tests for the timing instrumentation BEFORE implementing it:
  - [ ] Write a Rust unit test in `compiler/oric/src/commands/build/tests.rs` that verifies `ORI_BUILD_TIMING=1` produces parseable timing output on stderr (format: `ORI_BUILD_TIMING: phase=X time_ms=N`)
  - [ ] Write a Rust unit test that verifies `ORI_BUILD_TIMING` is OFF by default (no timing output without the env var)
- **Matrix**: The instrumentation is not type- or pattern-dependent, but it affects ALL AOT tests. The matrix dimension is "with instrumentation" vs "without instrumentation":
  - `ORI_TEST_TIMING=0` (or unset): all ~1,950 AOT tests pass identically to pre-change behavior
  - `ORI_TEST_TIMING=1 ORI_BUILD_TIMING=1`: all ~1,950 AOT tests pass AND timing output is produced
- **Semantic pin**: A test that verifies timing output is produced ONLY when `ORI_BUILD_TIMING=1` is set. This test fails if the env var check is broken.
- **Debug and release**: `timeout 150 cargo t` (debug) AND `timeout 150 cargo t --release` (release) must pass after changes.
- **Validation**: `ORI_TEST_TIMING=1 ORI_BUILD_TIMING=1 cargo test -p ori_llvm --test aot 2>&1 | grep "AOT Test Pipeline Timing"` produces a valid timing breakdown. Phase totals sum to within 5% of overall time.

---

## 03.R Third Party Review Findings

- None.

---

## 03.4 Completion Checklist

- [ ] `scripts/bench-tests.sh` exists and produces reproducible timing measurements
- [ ] Canonical baseline recorded (wall time mean +/- stddev, user time, system time)
- [ ] Per-crate timing breakdown recorded
- [ ] Flamegraph generation script works (`scripts/flamegraph-tests.sh`)
- [ ] Flamegraphs generated for: AOT tests, ori_eval, full workspace
- [ ] Top 10 hottest functions identified from flamegraph analysis
- [ ] AOT test harness has per-phase timing (`ORI_TEST_TIMING=1`)
- [ ] Phase breakdown recorded (parse/typeck/arc/codegen/object/link/execute percentages)
- [ ] Phase totals validate against overall time (within 5%)
- [ ] No test regressions: `timeout 150 cargo t` still passes

**Exit Criteria:** We have concrete, reproducible data showing exactly where time is spent in `cargo t`. The flamegraphs identify the top 10 hottest functions. The AOT per-phase breakdown reveals which pipeline phase dominates. This data is sufficient for Sections 04 and 05 to target specific optimizations.
