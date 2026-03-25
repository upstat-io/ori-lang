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
<!-- reviewed: accuracy fix — hyperfine is available -->
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

<!-- reviewed: accuracy fix — added oric to crate list, noted that ori_llvm --test aot should be measured separately -->
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

<!-- reviewed: accuracy fix — perf, cargo-flamegraph, and hyperfine are all available on this system -->
- [ ] Check if `perf` is available:
  ```bash
  perf --version  # Linux perf_events — available at /usr/local/bin/perf on this WSL2 system
  ```
  `perf` is available. Also available: `cargo-flamegraph` (at `~/.cargo/bin/cargo-flamegraph`) and `hyperfine` (at `~/.cargo/bin/hyperfine`).

- [ ] Create `scripts/flamegraph-tests.sh` that generates flamegraphs for specific test suites:
  ```bash
  #!/bin/bash
  # Usage: ./scripts/flamegraph-tests.sh [crate]
  # Examples:
  #   ./scripts/flamegraph-tests.sh ori_llvm    # AOT tests flamegraph
  #   ./scripts/flamegraph-tests.sh ori_arc     # ARC analysis flamegraph
  #   ./scripts/flamegraph-tests.sh             # Full workspace flamegraph
  ```

<!-- reviewed: feasibility fix — clarified that AOT flamegraph will show test harness, not compiler internals -->
<!-- reviewed: cohesion fix — corrected ranking: ori_eval (4.5s) is bigger than ori_arc (3.4s) per overview table -->
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

- [ ] Save flamegraph SVGs to `diagnostics/flamegraphs/` (gitignored — these are local profiling artifacts, not committed):
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

<!-- reviewed: accuracy/feasibility fix — corrected test count, documented actual file structure -->
The AOT test harness runs ~1,950 `#[test]` functions, each doing: write source → spawn `ori build` subprocess (compile+link) → spawn binary subprocess → check output. Add per-phase timing to understand which phase dominates.

**Actual file structure** (verified):
- `compiler/ori_llvm/tests/aot/main.rs` — module declarations for all test categories (not a dispatcher — Rust's test framework discovers `#[test]` functions)
- `compiler/ori_llvm/tests/aot/util/aot.rs` — core test helpers: `compile_and_run_capture()`, `assert_aot_success()`, `ori_binary()`, `stdlib_path()`
- `compiler/ori_llvm/tests/aot/util/mod.rs` — re-exports from `aot`, `object`, `wasm` submodules
- Each test calls `compile_and_run_capture(source)` which: (1) creates a `TempDir`, (2) writes `.ori` source, (3) spawns `ori build` via `Command::new(ori_binary())`, (4) spawns the compiled binary, (5) captures exit code + stdout + stderr

**Key**: The compilation pipeline runs inside the `ori build` subprocess — NOT in the test process. This means per-phase timing must be added to the `ori build` command path, not to the test harness itself.

- [ ] Read the AOT test harness to understand the current flow:
  - `compiler/ori_llvm/tests/aot/util/aot.rs` — `compile_and_run_capture()` is the core function (line 149)
  - `compiler/ori_llvm/tests/aot/util/mod.rs` — re-exports all utilities
  - `compiler/ori_llvm/Cargo.toml` — `ori_rt` is a path dependency (line 18); `ori_rt` is pre-built as a static lib that the `ori build` linker step links into each test binary
  - Document the actual file paths of the harness, not just assumed locations

<!-- reviewed: feasibility fix — per-phase timing requires changes to `ori build` command, not the test harness -->
<!-- reviewed: accuracy fix — corrected build command file paths -->
- [ ] Map the actual Ori compiler pipeline phases by reading the `ori build` entry point:
  - `compiler/oric/src/commands/build/mod.rs` — the `ori build` command entry point
  - `compiler/oric/src/commands/build/single.rs` — single-file build path
  - `compiler/oric/src/commands/codegen_pipeline.rs` — compilation pipeline orchestration
  - `compiler/oric/src/commands/compile_common.rs` — shared compilation logic
  - The actual phases: lex → parse → typeck → ARC lowering → LLVM codegen → object emission → linker invocation

- [ ] **Two-level timing**: Because each AOT test spawns `ori build` as a subprocess, timing must be split:

  **(a) Per-test timing in the test harness** (`compiler/ori_llvm/tests/aot/util/aot.rs`):
  When `ORI_TEST_TIMING=1` is set, wrap `compile_and_run_capture()` to time:
  1. **source write time** (write `.ori` file to temp dir)
  2. **compile time** (`ori build` subprocess — total wall time)
  3. **exec time** (binary execution subprocess — total wall time)
  4. **cleanup time** (temp dir teardown)
  At the end of all tests, report aggregate totals and percentages.

  **(b) Per-phase timing in `ori build`** (compiler-side):
  When `ORI_BUILD_TIMING=1` is set, `ori build` emits per-phase timing to stderr:
  1. **parse time** (lexer + parser)
  2. **typeck time** (type checking)
  3. **arc time** (ARC lowering)
  4. **codegen time** (LLVM IR generation)
  5. **object time** (LLVM compilation to object file)
  6. **link time** (linker invocation)
  The test harness can set this env var and aggregate across all tests.

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

- [ ] Verify the per-phase totals sum to approximately the overall wall time (within 5% — any larger gap indicates uncaptured overhead like process spawn/teardown).

- [ ] Run with `ORI_TEST_TIMING=1 ORI_BUILD_TIMING=1` and record the phase breakdown. This data directly informs Section 04 (which phase to optimize first).

### Test Strategy

- **Matrix**: N/A — this is instrumentation, not behavioral code.
<!-- reviewed: accuracy fix — updated command to include both env vars -->
- **Validation**: `ORI_TEST_TIMING=1 ORI_BUILD_TIMING=1 cargo test -p ori_llvm --test aot 2>&1 | grep "AOT Test Pipeline Timing"` produces a valid timing breakdown. Phase totals sum to within 5% of overall time.
- **Semantic pin**: The timing instrumentation must not change test behavior — all tests must still pass identically with and without `ORI_TEST_TIMING=1`.

---

## 03.R Third Party Review Findings

- None.

---

## 03.4 Completion Checklist

- [ ] `scripts/bench-tests.sh` exists and produces reproducible timing measurements
- [ ] Canonical baseline recorded (wall time mean +/- stddev, user time, system time)
- [ ] Per-crate timing breakdown recorded
- [ ] Flamegraph generation script works (`scripts/flamegraph-tests.sh`)
<!-- reviewed: cohesion fix — corrected to match Section 03.2 (ori_eval, not ori_arc) -->
- [ ] Flamegraphs generated for: AOT tests, ori_eval, full workspace
- [ ] Top 10 hottest functions identified from flamegraph analysis
- [ ] AOT test harness has per-phase timing (`ORI_TEST_TIMING=1`)
- [ ] Phase breakdown recorded (parse/typeck/arc/codegen/object/link/execute percentages)
- [ ] Phase totals validate against overall time (within 5%)
- [ ] No test regressions: `timeout 150 cargo t` still passes

**Exit Criteria:** We have concrete, reproducible data showing exactly where time is spent in `cargo t`. The flamegraphs identify the top 10 hottest functions. The AOT per-phase breakdown reveals which pipeline phase dominates. This data is sufficient for Sections 04 and 05 to target specific optimizations.
