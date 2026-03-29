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

- [ ] Check if `mold` (fastest linker for Linux) is available:
  ```bash
  mold --version  # NOT installed as of 2026-03-25. Install: sudo apt install mold
  ```

- [ ] Check if `lld` (LLVM's linker, faster than system ld) is available:
  ```bash
  /usr/lib/llvm-21/bin/ld.lld --version  # AVAILABLE via LLVM 21 at /usr/lib/llvm-21/bin/ld.lld
  ```
  **Recommendation**: Start with `lld` since it is already available. Only install `mold` if `lld` provides <10% improvement over system `ld`.

- [ ] Implement `ORI_LINKER` env var support. Currently `compiler/ori_llvm/src/aot/linker/driver.rs` line 30 selects flavor via `LinkerFlavor::for_target()` with no env var override. Implementation:
  1. **WHERE**: `compiler/ori_llvm/src/aot/linker/driver.rs`, in `LinkerDriver::link()` (line 30), before the `input.linker.unwrap_or_else(...)` call
  2. **WHAT**: Read `std::env::var("ORI_LINKER")`. If set:
     - If value is `"lld"` or path ends in `ld.lld`: set `flavor = LinkerFlavor::Lld`
     - If value is `"mold"`: create `GccLinker::with_path(target, "cc")` and add `-fuse-ld=mold` arg
     - If value is a path: create `GccLinker::with_path(target, &value)` (custom linker binary)
  3. **TEST**: Add a test in `compiler/ori_llvm/src/aot/linker/tests.rs` that verifies `ORI_LINKER` env var is respected
  4. **Usage**:
    ```bash
    ORI_LINKER=lld cargo test -p ori_llvm --test aot
    # Or with explicit path:
    ORI_LINKER=/usr/lib/llvm-21/bin/ld.lld cargo test -p ori_llvm --test aot
    ```

- [ ] Measure the improvement:
  ```bash
  # Before (system linker):
  ORI_TEST_TIMING=1 cargo test -p ori_llvm --test aot 2>&1 | grep "Link:"

  # After (mold):
  ORI_LINKER=mold ORI_TEST_TIMING=1 cargo test -p ori_llvm --test aot 2>&1 | grep "Link:"
  ```

- [ ] If the linker optimization provides measurable improvement (>10%), make it the default for test builds. Add a note in CLAUDE.md about the linker configuration.

### Test Strategy

- **TDD ordering**:
  - [ ] Write a Rust unit test in `compiler/ori_llvm/src/aot/linker/tests.rs` that verifies `ORI_LINKER` env var selects the correct linker flavor BEFORE implementing the feature
  - [ ] Verify test fails (since `ORI_LINKER` support does not exist yet)
  - [ ] Implement `ORI_LINKER` support
  - [ ] Verify test passes unchanged
- **Matrix**: The linker change is not type-dependent but path-dependent. Test matrix:
  - `ORI_LINKER` unset: default linker selected (existing behavior preserved)
  - `ORI_LINKER=lld`: `lld` linker selected
  - `ORI_LINKER=/usr/lib/llvm-21/bin/ld.lld`: explicit path accepted
  - `ORI_LINKER=invalid`: graceful error (not a panic)
  - All ~1,950 AOT tests pass identically with each valid linker. No behavioral changes.
- **Semantic pin**: The `ORI_LINKER=lld` unit test ONLY passes with the new env var support -- reverting the change makes it fail.
- **Debug and release**: `timeout 150 cargo test -p ori_llvm --test aot` passes in both debug and release builds.
- **Measurement**: Compare link-phase timing before and after. Record in this section.

---

## 04.2 Compilation Pipeline Optimization

**File(s):** `compiler/oric/src/commands/build/` (build command — `mod.rs`, `single.rs`, `multi.rs`), `compiler/oric/src/commands/codegen_pipeline.rs` (compilation pipeline), `compiler/ori_llvm/src/aot/` (AOT pipeline), `compiler/ori_llvm/tests/aot/util/aot.rs` (test harness)

Each AOT test spawns `ori build` as a separate subprocess via `Command::new(ori_binary())`. The full compilation pipeline (lex→parse→typeck→ARC→LLVM→object→link) runs inside that subprocess. This means:
- Each test starts a fresh process with fresh LLVM Context, fresh Salsa DB, etc.
- There is no cross-test caching or context reuse
- Optimizations must target either (a) the `ori build` pipeline itself, (b) the subprocess overhead, or (c) restructuring to avoid per-test subprocesses

- [ ] Check Section 03's per-phase timing to determine which compilation phase dominates.

- [ ] **Shared runtime pre-compilation**: The `ori_rt` runtime library (`libori_rt.a`) is linked into every AOT binary. It is a pre-built static library discovered by `ori_llvm/src/aot/runtime.rs` (checked at `<exe>/../lib/libori_rt.a` or `$ORI_WORKSPACE_DIR/target/`). Verify it is NOT being rebuilt per test — the linker just reads it. If the linker re-reads `libori_rt.a` from disk for each of ~1,950 tests, the I/O overhead adds up. Check if the OS page cache handles this effectively.

- [ ] **LLVM optimization level for tests**: Verify `ori build` defaults to `-O0`. Checked: `compiler/oric/src/commands/build_options/mod.rs` line 88 sets `opt_level: OptLevel::O0` as the default. The `--release` flag applies `O2`. AOT tests call `ori build` without `--release`, so they already use `-O0`. **This optimization is already in place -- skip unless profiling reveals LLVM optimization passes are still a bottleneck.**

- [ ] **LLVM Context creation overhead**: Each `ori build` invocation creates a fresh LLVM Context. This happens ~1,950 times. Context creation involves LLVM target initialization. This is NOT optimizable within the current subprocess architecture — it would require an in-process compilation mode (see 04.3 batch test execution).

- [ ] **Object file writing + linking**: The current pipeline writes object files to disk (in the TempDir), then invokes the system linker. Both are per-test I/O operations:
  ```bash
  grep -r "write_to_file\|write_bitcode\|object_file\|emit_object" compiler/ori_llvm/src/aot/
  ```
  The TempDir is on `/tmp` which is ext4 on this WSL2 system (NOT tmpfs). This means object file writes hit the real filesystem. Consider mounting a tmpfs at a custom location and setting `TMPDIR` for AOT tests to reduce I/O overhead:
  ```bash
  # Option: mount tmpfs for test builds
  sudo mount -t tmpfs -o size=512m tmpfs /tmp/ori-test-builds
  TMPDIR=/tmp/ori-test-builds cargo test -p ori_llvm --test aot
  ```

- [ ] **Salsa query caching**: NOT applicable for AOT tests — each test spawns a separate `ori build` process with a fresh Salsa DB. There is no cross-test Salsa caching. This is a fundamental limitation of the subprocess architecture. The only way to get Salsa caching benefits would be an in-process compilation mode or a persistent compiler server.

- [ ] Measure the impact of each optimization individually (not combined) to understand the contribution of each.

### Test Strategy

- **TDD ordering**: These are optimization changes, not bug fixes. The "test" is that all existing tests continue passing with identical results. Before each optimization:
  - [ ] Record the full AOT test output (pass/fail/stdout per test) as a before-snapshot
  - [ ] Apply the optimization
  - [ ] Verify the after-output is identical to the before-snapshot
- **Matrix**: All ~1,950 AOT tests pass identically after each optimization. No test code modified.
- **Semantic pin**: The before/after output comparison IS the semantic pin -- any output difference indicates a behavioral change, not just a performance change.
- **Debug and release**: `timeout 150 cargo t` (debug) passes after each change. Release build (`timeout 150 cargo t --release`) verified at end of subsection.
- **Measurement**: Record per-phase timing before and after each optimization.

---

## 04.3 Process Overhead Reduction

**File(s):** `compiler/ori_llvm/tests/aot/`

Each AOT test spawns TWO child processes: (1) `ori build` for compilation and (2) the compiled binary for execution. Process creation (fork+exec) and teardown has overhead that multiplies across ~1,950 tests (~3,900 total process spawns).

- [ ] Check Section 03's per-phase timing for the "execute" and "overhead" components.

- [ ] **Temp file management**: Each test creates a new `TempDir` (via the `tempfile` crate) with a unique source file and binary:
  ```rust
  // From compiler/ori_llvm/tests/aot/util/aot.rs:compile_and_run_capture() (line 149)
  let temp_dir = TempDir::new().expect("Failed to create temp dir");
  let source_path = temp_dir.path().join(format!("test_{id}.ori"));
  let binary_path = temp_dir.path().join(format!("test_{id}{}", std::env::consts::EXE_SUFFIX));
  ```
  The filesystem overhead (mkdir + write source + write object + write binary + unlink all) adds up over ~1,950 tests. Consider:
  - Reusing a single temp directory across tests (with unique filenames via the `AtomicU64` counter already in place)
  - Verifying `/tmp` is `tmpfs` on the target system (reduces disk I/O)

- [ ] **Binary execution overhead**: Each test runs a compiled binary with `ORI_CHECK_LEAKS=1` **always enabled** (hardcoded in `compile_and_run_capture()` at line 178, and also in `compile_and_run_with_args()` at line 221):
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

- [ ] **Hygiene: split `compiler/ori_llvm/tests/aot/util/aot.rs`** (569 lines, exceeds 500-line limit). Extract IR inspection helpers (`extract_function_ir`, `count_bridge_blocks`, `count_single_pred_phis`, `count_dead_phis`, `is_ssa_var_used_in`, `is_bridge_only`) into a sibling `compiler/ori_llvm/tests/aot/util/ir_inspect.rs`. The compile-and-run functions stay in `aot.rs`. Re-export from `util/mod.rs`. This brings `aot.rs` to ~385 lines and `ir_inspect.rs` to ~185 lines.

### Test Strategy

- **TDD ordering**: For the `aot.rs` split (hygiene task), write a list of all public functions currently exported from `util/aot.rs`, then verify each is still accessible after extraction:
  - [ ] Before split: `cargo test -p ori_llvm --test aot --no-run` compiles successfully
  - [ ] After split: same command compiles successfully, proving re-exports are correct
  - [ ] All ~1,950 AOT tests produce identical results
- **Matrix**: All ~1,950 AOT tests produce identical results after each change in this subsection (temp dir reuse, leak detection opt-in, batch execution).
- **Semantic pin**: No test passes that previously failed, no test fails that previously passed. If `ORI_CHECK_LEAKS` is made opt-in, a specific test must verify that `ORI_AOT_CHECK_LEAKS=1` still enables leak detection.
- **Debug and release**: `timeout 150 cargo t` (debug) AND `timeout 150 cargo t --release` (release) must pass after all changes.
- **Measurement**: Per-test overhead (ms/test) before and after. Target: reduce from ~18ms/test to <8ms/test.

---

## 04.R Third Party Review Findings

- None.

---

## 04.4 Completion Checklist

- [ ] Linker optimization evaluated and applied if beneficial (>10% improvement)
- [ ] Compilation pipeline optimizations applied based on per-phase profiling
- [ ] Process overhead reduced (temp files, leak detection, parallelism evaluated)
- [ ] Batch test execution prototype measured (10-test batch vs 10 individual runs)
- [ ] `compiler/ori_llvm/tests/aot/util/aot.rs` split: IR inspection helpers extracted to `ir_inspect.rs` (569→~385 lines)
- [ ] AOT test execution time measured: ??? (target: <=15s)
- [ ] All ~1,950 AOT tests pass identically (no behavioral changes)
- [ ] Optimizations documented (what was changed, why, measured impact)
- [ ] `timeout 150 cargo t` passes with all tests green
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)

**Exit Criteria:** `ORI_TEST_TIMING=1 cargo test -p ori_llvm --test aot` reports total time ≤15s. All ~1,950 tests pass. No test code was modified. The link-phase, compile-phase, and execute-phase timings are all recorded and show measurable improvement from baseline.
