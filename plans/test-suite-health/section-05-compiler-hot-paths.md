---
section: "05"
title: "Compiler Hot Path Optimization"
status: not-started
reviewed: false
goal: "Reduce non-AOT test time from ~23s to ≤15s by optimizing compiler hot paths identified through profiling."
inspired_by:
  - "Rust perf.rust-lang.org — data-driven optimization of specific hot functions"
  - "Zig compiler — aggressive inlining and arena allocation for compilation speed"
depends_on: ["03"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "05.1"
    title: "Crate Compilation Time"
    status: not-started
  - id: "05.2"
    title: "Test Execution Hot Paths"
    status: not-started
  - id: "05.3"
    title: "Workspace-Level Optimization"
    status: not-started
  - id: "05.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "05.4"
    title: "Completion Checklist"
    status: not-started
---

# Section 05: Compiler Hot Path Optimization

**Status:** Not Started
**Goal:** The non-AOT portion of `cargo t` drops from ~23s to ≤15s. All optimizations are in compiler code (Rust), not test code. Optimizations are guided by Section 03's profiling data — no speculative changes.

<!-- reviewed: cohesion fix — added parallel execution analysis. The "remaining ~23s" is not a simple subtraction. -->
**Context:** The 59s `cargo t` wall time includes parallel execution of all crate test binaries. `cargo test --workspace` runs crate tests concurrently (up to `--jobs` parallelism). This means the ~23s "non-AOT" figure is NOT independent of the 35.6s AOT time -- non-AOT crates run in parallel with AOT tests. The 59s wall time likely includes: (a) sequential compilation of all test binaries, (b) sequential cargo orchestration overhead, and (c) parallel test execution dominated by AOT. Section 03's measurement methodology will separate compilation time from execution time (via `cargo test --no-run` vs `cargo test`), which is essential for understanding the actual optimization surface. The largest crates by test execution time are ori_eval (4.5s incl. compilation), ori_arc (3.4s), ori_patterns (2.6s), and ori_ir (2.5s).

**Depends on:** Section 03 (Profiling Infrastructure) — flamegraphs and per-crate timing identify which functions to optimize.

---

## 05.1 Crate Compilation Time

**File(s):** Workspace `Cargo.toml`, individual crate `Cargo.toml` files

<!-- reviewed: accuracy fix — verified line count: 60,000 total including tests, 29,464 source-only -->
Part of the `cargo t` wall time is spent COMPILING the test binaries, not running them. This is especially significant for large crates like ori_arc (~60K lines total, ~29.5K source + ~30.5K test code).

- [ ] Measure compilation time separately from test execution time:
  ```bash
  # Compilation only (no test execution):
  time cargo test --workspace --no-run

  # Test execution only (pre-compiled):
  time cargo test --workspace
  # Subtract compilation time = execution time
  ```

- [ ] If compilation is >10s of the total 59s:

  - **Check codegen units**: By default, Rust uses 16 codegen units for debug builds. For test builds, more codegen units = faster compilation but slightly slower code. Check if `Cargo.toml` profiles override this:
    ```bash
    grep -A 5 "\[profile.test\]\|\[profile.dev\]" Cargo.toml
    ```

  - **Check incremental compilation**: Verify incremental is enabled for test profile:
    ```bash
    grep -r "incremental" Cargo.toml .cargo/config.toml
    ```
    Incremental compilation should be ON for test builds (faster rebuild after small changes).

  - **Check for heavy proc-macros**: Proc macros that generate large amounts of code slow compilation:
    ```bash
    grep -r "proc-macro\|derive_more\|serde_derive" compiler/*/Cargo.toml
    ```

  <!-- reviewed: accuracy fix — verified: 60K lines total, 1,012 #[test] functions across 38 test files -->
  - **Large crate split evaluation**: ori_arc (~60K lines, 1,012 test functions across 38 test files) is the largest crate. Any change to ori_arc triggers recompilation of the entire crate + all tests. Evaluate whether splitting ori_arc into smaller crates would reduce incremental rebuild time. **This is a trade-off analysis, not a recommendation** — splitting adds dependency complexity.

- [ ] If compilation is <10s: skip the above and focus on test execution optimization instead.

### Test Strategy

- **Measurement**: Record compilation time vs execution time. Determine which dominates.
- **Validation**: Any `Cargo.toml` changes don't break `cargo t` or `cargo b`.

---

## 05.2 Test Execution Hot Paths

**File(s):** Specific compiler source files identified by Section 03's flamegraphs

The flamegraphs from Section 03 will identify the top 10 hottest functions during test execution. This subsection addresses whatever those functions are.

<!-- reviewed: cohesion fix — replaced deferral language with concrete structure. Profiling-dependent tasks are conditional, not deferred. -->
**This subsection has two parts**: (a) tasks that can be started immediately (known crate analysis), and (b) tasks driven by Section 03's flamegraph findings. Part (b) tasks are conditional on profiling results — they execute with whatever the top 10 functions turn out to be.

<!-- reviewed: cohesion fix — added pre-profiling analysis tasks that don't depend on Section 03 -->
- [ ] **Pre-profiling: identify large test suites**: Run `cargo test -p <crate> -- --list 2>/dev/null | wc -l` for each crate to count test functions. The crates with the most tests are likely where execution time improvements have the highest payoff. Prioritize: ori_arc (1,012 tests), ori_eval, ori_patterns.

- [ ] **Pre-profiling: identify expensive test patterns**: Search for test patterns that are inherently expensive:
  ```bash
  # Tests that compile+run Ori programs (mini-pipelines within the test):
  grep -rn "compile_and_run\|run_source\|eval_source\|check_source" compiler/*/tests/ compiler/*/src/**/tests.rs
  # Tests that spawn subprocesses:
  grep -rn "Command::new" compiler/*/tests/ compiler/*/src/**/tests.rs
  # Tests with large inline source strings (>50 lines):
  grep -c '    "' compiler/*/tests/**/*.rs | sort -t: -k2 -rn | head -20
  ```
  Each of these patterns has different optimization approaches: mini-pipeline tests benefit from shared compilation state; subprocess tests benefit from batching; large-source tests benefit from fixture reuse.

- [ ] **Review flamegraph top 10**: List the 10 hottest functions from Section 03's analysis. For each:
  - Read the function's source code
  - Understand why it's hot (called frequently? expensive per-call? both?)
  - Determine if optimization is possible without changing behavior

- [ ] **Common optimization patterns to look for**:

  1. **Unnecessary cloning**: Look for `.clone()` in hot paths. Replace with borrows or `Cow<>` where possible.
     ```bash
     # Find .clone() calls in hot crate source files
     grep -n "\.clone()" compiler/ori_arc/src/*.rs compiler/ori_types/src/**/*.rs
     ```

  2. **Excessive allocation**: Look for `Vec::new()` or `String::from()` in tight loops. Replace with pre-allocated buffers or arena allocation.

  3. **Hash map overhead**: Look for `HashMap::get()` or `HashMap::insert()` in hot paths. If keys are small integers, consider using `Vec` indexed by the integer (O(1) vs O(1) amortized but with lower constant).

  4. **Missing `#[inline]`**: Cross-crate hot functions without `#[inline]` have call overhead. Check if top-10 hot functions cross crate boundaries. See memory note on parser `#[inline]` optimization (20-30% gain on cross-crate Index trait).

  5. **Redundant computation**: Look for the same value being computed multiple times. Salsa should handle this via memoization, but non-Salsa code may recompute.

  6. **Linear scans over hash lookups**: For collections >8 items, linear scans are slower than hash lookups. Look for `.iter().find()` or `.iter().position()` patterns.

- [ ] For each identified optimization:
  1. Measure the function's cost BEFORE (use `cargo bench` if a benchmark exists, or a targeted timing test)
  2. Implement the optimization
  3. Measure AFTER
  4. Verify no behavioral change (tests pass identically)
  5. Record the improvement

### Test Strategy

- **Matrix**: Every optimization must preserve all existing test behavior. `timeout 150 cargo t` green after each change.
- **Measurement**: Per-function timing before/after. Aggregate improvement on `cargo t` wall time.

---

## 05.3 Workspace-Level Optimization

**File(s):** `test-all.sh`, `.cargo/config.toml`, `Cargo.toml`

Optimizations at the workspace/build system level can reduce overhead that affects all crates.

- [ ] **Parallel test execution**: Check if `cargo t` is already running tests in parallel:
  ```bash
  # Default: Rust runs tests within each crate in parallel (threads)
  # Crate-level parallelism depends on cargo's job count
  cargo t -- --test-threads=N  # N = number of parallel test threads
  ```
  Current `cargo t` uses default parallelism. Check if increasing or decreasing `--test-threads` improves wall time.

- [ ] **Cargo parallel jobs**: Check if the number of parallel compilation jobs is optimal:
  ```bash
  # Check current setting
  grep "jobs" .cargo/config.toml
  # Default: number of CPU cores
  ```

- [ ] **Profile-guided optimization (PGO) for test builds**: Not recommended for test builds (PGO optimizes for specific workloads, but test builds need fast compilation, not fast execution). Document this decision.

- [ ] **LTO for test builds**: Verify LTO is OFF for test builds (LTO is slow to compile). If LTO is accidentally on for the test profile, disable it.

<!-- reviewed: cohesion fix — cargo-nextest is not installed; made installation explicit -->
- [ ] **Evaluate `cargo-nextest`**: `cargo-nextest` runs each test as an individual process (better isolation) and can be faster than `cargo test` for large workspaces due to better parallelism. It is NOT currently installed.
  ```bash
  cargo install cargo-nextest
  time cargo nextest run --workspace
  ```
  Compare wall time with standard `cargo t`. If nextest is faster, document it and add it as the default test command in `scripts/bench-tests.sh`. If slower or no improvement, document the comparison and remove it.

### Test Strategy

- **Validation**: Any workspace-level change must not break `cargo t`, `cargo b`, or `./test-all.sh`.
- **Measurement**: Wall time before/after each change.

---

## 05.R Third Party Review Findings

- None.

---

## 05.4 Completion Checklist

- [ ] Compilation time vs execution time breakdown recorded
- [ ] Flamegraph top 10 hot functions analyzed and documented
- [ ] Each identified optimization measured (before/after per function)
- [ ] Workspace-level optimizations evaluated (parallel threads, nextest, etc.)
- [ ] Non-AOT test time measured: ??? (target: ≤15s)
- [ ] All tests pass identically (no behavioral changes)
- [ ] Optimizations documented with measured impact
- [ ] `timeout 150 cargo t` green

**Exit Criteria:** The non-AOT portion of `cargo t` takes ≤15s. Each optimization is individually measured and documented. No test code was modified. The flamegraph's top 10 hot functions have been addressed (either optimized or documented as "acceptable — not optimizable without behavioral changes").
