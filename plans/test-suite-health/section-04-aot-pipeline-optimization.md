---
section: "04"
title: "AOT Pipeline Optimization"
status: not-started
reviewed: false
goal: "Reduce AOT integration test execution time from 35.6s to ≤15s (-58%) without modifying any test code."
inspired_by:
  - "Rust compiler test suite — uses shared compile sessions and fast linkers to minimize per-test overhead"
  - "Zig compiler — in-process linking eliminates external linker overhead"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "04.1"
    title: "Linker Optimization"
    status: not-started
  - id: "04.2"
    title: "Compilation Pipeline Optimization"
    status: not-started
  - id: "04.3"
    title: "Process Overhead Reduction"
    status: not-started
  - id: "04.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "04.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 04: AOT Pipeline Optimization

**Status:** Not Started
**Goal:** AOT integration test execution drops from 35.6s to ≤15s. All ~1,950 tests pass identically. No test code modified.

<!-- reviewed: accuracy/feasibility fix — corrected test count, clarified subprocess architecture -->
**Context:** The AOT integration tests (`compiler/ori_llvm/tests/aot/`) account for 60% of `cargo t` wall time. Each of the ~1,950 tests spawns TWO subprocesses: (1) `ori build` (full compile pipeline: lex→parse→typeck→ARC→LLVM→link) and (2) the compiled binary. Each test also creates a `TempDir`, writes source to disk, and cleans up afterward. At ~18ms/test average, the per-test cost is already lean, but the cumulative effect of ~1,950 cycles (with ~3,900 process spawns) is the dominant bottleneck. Section 03's per-phase timing will reveal which phase (compile/link/execute/overhead) to attack first.

**Feasibility analysis:**
- Current: 35.6s / ~1,950 tests = ~18.3 ms/test
- Target (≤15s): 15s / ~1,950 tests = ~7.7 ms/test
- Required speedup: 2.4x
- This is aggressive. The per-test overhead includes two `Command::new()` spawns, file I/O, and a full `ori build` invocation. If profiling reveals that most per-test time is in irreducible work (process spawning + LLVM compilation is inherently expensive), the target may need revision. The plan treats ≤15s as aspirational; the actual target is "whatever profiling shows is achievable with reasonable effort." If 20s is the achievable floor, that's still a 44% improvement and should be accepted.

**Depends on:** Section 03 (Profiling Infrastructure) — the per-phase timing data guides which optimizations have the highest ROI.

**Baseline coordination:** Linker changes (04.1) affect baseline measurements. Record a "pre-linker" baseline during Section 03, then record a "post-linker" measurement after Section 04.1. The final comparison in Section 06 uses the original Section 03 baseline as the "before" and Section 06's measurement as the "after."

---

## 04.1 Linker Optimization

<!-- reviewed: accuracy fix — corrected file paths to actual linker module location -->
**File(s):** `compiler/ori_llvm/src/aot/linker/` (linker drivers: `gcc.rs`, `msvc.rs`, `wasm/`), `compiler/ori_llvm/src/aot/linker/driver.rs` (linker driver selection)

Linking is often the slowest phase in compile→link→execute cycles. Switching from the system linker (`ld`/`cc`) to a faster alternative can yield dramatic improvements.

- [ ] Check Section 03's per-phase timing to determine what percentage of AOT test time is spent in the linker phase. If linking is <10% of total, skip this subsection.

- [ ] Check which linker the AOT tests currently use:
  ```bash
  # The linker is selected in the AOT linker module:
  grep -r "Command::new\|cc\|gcc\|ld\|clang" compiler/ori_llvm/src/aot/linker/gcc.rs
  grep -r "linker\|link_command\|link_args" compiler/ori_llvm/src/aot/linker/driver.rs
  ```
  The `GccLinker` (in `gcc.rs`) is used on Linux/macOS and invokes `cc` by default.

<!-- reviewed: accuracy fix — verified tool availability on this system -->
- [ ] Check if `mold` (fastest linker for Linux) is available:
  ```bash
  mold --version  # Not currently installed. Install: sudo apt install mold
  ```

- [ ] Check if `lld` (LLVM's linker, faster than system ld) is available:
  ```bash
  /usr/lib/llvm-21/bin/ld.lld --version  # Available via LLVM 21 (not in PATH by default)
  ```

<!-- reviewed: feasibility fix — clarified that ORI_LINKER must be implemented in the linker driver code -->
- [ ] If `mold` or `lld` is available, configure the AOT pipeline to use it. This requires adding `ORI_LINKER` env var support to `compiler/ori_llvm/src/aot/linker/driver.rs`:
  - Option A: Set linker via environment variable (most flexible — requires code change to read `ORI_LINKER` in the linker driver)
    ```bash
    ORI_LINKER=mold cargo test -p ori_llvm --test aot
    # Or with lld from LLVM 21:
    ORI_LINKER=/usr/lib/llvm-21/bin/ld.lld cargo test -p ori_llvm --test aot
    ```
  - Option B: Add `--linker=<path>` flag to `ori build` CLI
  - Option C: Set via `.cargo/config.toml` for test builds (affects all builds — but this only affects the Rust linker, not the Ori AOT linker)

- [ ] Measure the improvement:
  ```bash
  # Before (system linker):
  ORI_TEST_TIMING=1 cargo test -p ori_llvm --test aot 2>&1 | grep "Link:"

  # After (mold):
  ORI_LINKER=mold ORI_TEST_TIMING=1 cargo test -p ori_llvm --test aot 2>&1 | grep "Link:"
  ```

- [ ] If the linker optimization provides measurable improvement (>10%), make it the default for test builds. Add a note in CLAUDE.md about the linker configuration.

### Test Strategy

- **Matrix**: All ~1,950 AOT tests must pass identically with the new linker. No behavioral changes.
- **Semantic pin**: `timeout 150 cargo test -p ori_llvm --test aot` passes with 0 failures before and after.
- **Measurement**: Compare link-phase timing before and after. Record in this section.

---

## 04.2 Compilation Pipeline Optimization

<!-- reviewed: accuracy fix — corrected file references to reflect that optimizations target `ori build`, not the test harness -->
**File(s):** `compiler/oric/src/commands/build/` (build command — `mod.rs`, `single.rs`, `multi.rs`), `compiler/oric/src/commands/codegen_pipeline.rs` (compilation pipeline), `compiler/ori_llvm/src/aot/` (AOT pipeline), `compiler/ori_llvm/tests/aot/util/aot.rs` (test harness)

<!-- reviewed: feasibility fix — corrected fundamental misunderstanding about AOT test architecture. Each test spawns `ori build` as a separate process — no in-process LLVM Context or Salsa DB sharing is possible without architectural change. -->
Each AOT test spawns `ori build` as a separate subprocess via `Command::new(ori_binary())`. The full compilation pipeline (lex→parse→typeck→ARC→LLVM→object→link) runs inside that subprocess. This means:
- Each test starts a fresh process with fresh LLVM Context, fresh Salsa DB, etc.
- There is no cross-test caching or context reuse
- Optimizations must target either (a) the `ori build` pipeline itself, (b) the subprocess overhead, or (c) restructuring to avoid per-test subprocesses

- [ ] Check Section 03's per-phase timing to determine which compilation phase dominates.

- [ ] **Shared runtime pre-compilation**: The `ori_rt` runtime library (`libori_rt.a`) is linked into every AOT binary. It is a pre-built static library discovered by `ori_llvm/src/aot/runtime.rs` (checked at `<exe>/../lib/libori_rt.a` or `$ORI_WORKSPACE_DIR/target/`). Verify it is NOT being rebuilt per test — the linker just reads it. If the linker re-reads `libori_rt.a` from disk for each of ~1,950 tests, the I/O overhead adds up. Check if the OS page cache handles this effectively.

- [ ] **LLVM optimization level for tests**: Check what optimization level the `ori build` command uses by default:
  ```bash
  grep -r "OptimizationLevel\|opt_level\|O0\|O1\|O2" compiler/ori_llvm/src/aot/
  grep -r "OptimizationLevel\|opt_level" compiler/oric/src/commands/
  ```
  Tests should use `-O0` (no optimization) for fastest compilation. If `ori build` defaults to `-O1` or higher, adding a `--opt-level=0` flag (or defaulting to `-O0` when no flag is given) will speed up LLVM's internal optimization passes.

- [ ] **LLVM Context creation overhead**: Each `ori build` invocation creates a fresh LLVM Context. This happens ~1,950 times. Context creation involves LLVM target initialization. This is NOT optimizable within the current subprocess architecture — it would require an in-process compilation mode (see 04.3 batch test execution).

- [ ] **Object file writing + linking**: The current pipeline writes object files to disk (in the TempDir), then invokes the system linker. Both are per-test I/O operations:
  ```bash
  grep -r "write_to_file\|write_bitcode\|object_file\|emit_object" compiler/ori_llvm/src/aot/
  ```
  The TempDir is typically on `/tmp` which may be `tmpfs` (in-memory) on Linux, reducing disk I/O. Verify this on the target system.

- [ ] **Salsa query caching**: NOT applicable for AOT tests — each test spawns a separate `ori build` process with a fresh Salsa DB. There is no cross-test Salsa caching. This is a fundamental limitation of the subprocess architecture. The only way to get Salsa caching benefits would be an in-process compilation mode or a persistent compiler server.

- [ ] Measure the impact of each optimization individually (not combined) to understand the contribution of each.

### Test Strategy

- **Matrix**: All ~1,950 AOT tests pass identically after each optimization.
- **Measurement**: Record per-phase timing before and after each optimization.

---

## 04.3 Process Overhead Reduction

**File(s):** `compiler/ori_llvm/tests/aot/`

<!-- reviewed: feasibility fix — corrected parallelism analysis, documented actual ORI_CHECK_LEAKS behavior -->
Each AOT test spawns TWO child processes: (1) `ori build` for compilation and (2) the compiled binary for execution. Process creation (fork+exec) and teardown has overhead that multiplies across ~1,950 tests (~3,900 total process spawns).

- [ ] Check Section 03's per-phase timing for the "execute" and "overhead" components.

- [ ] **Temp file management**: Each test creates a new `TempDir` (via the `tempfile` crate) with a unique source file and binary:
  ```rust
  // From compiler/ori_llvm/tests/aot/util/aot.rs:compile_and_run_capture()
  let temp_dir = TempDir::new().expect("Failed to create temp dir");
  let source_path = temp_dir.path().join(format!("test_{id}.ori"));
  let binary_path = temp_dir.path().join(format!("test_{id}"));
  ```
  The filesystem overhead (mkdir + write source + write object + write binary + unlink all) adds up over ~1,950 tests. Consider:
  - Reusing a single temp directory across tests (with unique filenames via the `AtomicU64` counter already in place)
  - Verifying `/tmp` is `tmpfs` on the target system (reduces disk I/O)

- [ ] **Binary execution overhead**: Each test runs a compiled binary with `ORI_CHECK_LEAKS=1` **always enabled** (hardcoded in `compile_and_run_capture()` at line 178):
  ```rust
  let run_result = Command::new(&binary_path)
      .env("ORI_CHECK_LEAKS", "1")
      .output()
  ```
  Leak detection adds per-allocation tracking overhead to the runtime. Consider:
  - Making leak detection opt-in for regular test runs via an env var (e.g., only enable when `ORI_AOT_CHECK_LEAKS=1` is set)
  - Keeping it always-on for CI
  - Measuring the overhead: run the same tests with and without `ORI_CHECK_LEAKS=1` and compare

- [ ] **Parallel AOT tests**: The AOT tests already run in parallel by default. They are standard `#[test]` functions with no `#[serial]` annotation and no shared mutable state (each test creates its own `TempDir` and spawns independent subprocesses). Rust's test framework runs them in parallel threads. However:
  - The degree of parallelism is controlled by `--test-threads=N` (default: number of CPU cores)
  - Each test spawns 2 subprocesses, so with N parallel tests, there are up to 2N concurrent processes
  - Investigate whether the current parallelism level is optimal or if system I/O saturation limits gains
  - There is NO LLVM Context thread-safety concern — the LLVM Context lives inside the `ori build` subprocess, not the test process

<!-- reviewed: cohesion fix — removed deferral language ("significant architectural change" + "Evaluate ROI"), made tasks concrete -->
- [ ] **Batch test execution**: Instead of one `ori build` invocation per test, group multiple small tests into a single compilation unit. Concrete approach:
  1. **Prototype with 10 tests**: Pick 10 independent AOT tests. Concatenate their Ori source into a single `.ori` file with distinct `@main`-like functions selected by a command-line argument. Measure compile+link+run time for the batch vs 10 individual runs.
  2. **Measure overhead split**: From the prototype, determine what fraction of per-test cost is fixed overhead (process spawn, LLVM Context init, `ori_rt` linkage, temp dir creation) vs variable (source-proportional compilation). If fixed overhead is >60% of per-test time, batching will have significant ROI.
  3. **Implement batching if profitable**: If the prototype shows >2x speedup for the batch, implement a `BatchedAotRunner` in `compiler/ori_llvm/tests/aot/util/` that:
     - Groups tests by expected-success vs expected-failure (failures cannot share a compilation unit)
     - Generates a single `.ori` source with all test bodies as separate functions
     - Compiles once, then runs the binary once per test function (or with a dispatch argument)
     - Falls back to individual execution for any test that fails compilation in batch mode
  4. **Failure isolation**: If a batch compilation fails, re-run each test individually to identify the failing test. This preserves test granularity for error reporting.
  - **Fallback alternative**: If batching is insufficient (target still not met after steps 1-4), implement a persistent `ori build --server` mode that accepts multiple compilation requests without process restart. This amortizes LLVM Context creation, Salsa DB initialization, and `ori_rt` linkage across many tests. Only pursue if batching alone is insufficient and profiling confirms LLVM Context creation is a major cost.

### Test Strategy

- **Matrix**: All ~1,950 AOT tests produce identical results.
- **Semantic pin**: No test passes that previously failed, no test fails that previously passed.
- **Measurement**: Per-test overhead (ms/test) before and after. Target: reduce from ~18ms/test to <8ms/test.

---

## 04.R Third Party Review Findings

- None.

---

## 04.4 Completion Checklist

- [ ] Linker optimization evaluated and applied if beneficial (>10% improvement)
- [ ] Compilation pipeline optimizations applied based on per-phase profiling
- [ ] Process overhead reduced (temp files, leak detection, parallelism evaluated)
<!-- reviewed: cohesion fix — added batch test evaluation to checklist since it's now a concrete task in 04.3 -->
- [ ] Batch test execution prototype measured (10-test batch vs 10 individual runs)
- [ ] AOT test execution time measured: ??? (target: <=15s)
- [ ] All ~1,950 AOT tests pass identically (no behavioral changes)
- [ ] Optimizations documented (what was changed, why, measured impact)
- [ ] `timeout 150 cargo t` passes with all tests green

<!-- reviewed: accuracy fix — corrected test count -->
**Exit Criteria:** `ORI_TEST_TIMING=1 cargo test -p ori_llvm --test aot` reports total time ≤15s. All ~1,950 tests pass. No test code was modified. The link-phase, compile-phase, and execute-phase timings are all recorded and show measurable improvement from baseline.
