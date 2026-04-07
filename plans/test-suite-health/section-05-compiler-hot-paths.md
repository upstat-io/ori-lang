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
**Goal:** Reduce compilation overhead and non-AOT test execution time to contribute toward the 30s overall target. All optimizations are in compiler code (Rust), not test code. Optimizations are guided by Section 03's profiling data -- no speculative changes. The primary surfaces are: (a) test binary compilation time (sequential), (b) crate test execution for the slowest non-AOT crates, and (c) workspace-level parallelism configuration.

**Context:** The 59s `cargo t` wall time includes parallel execution of all crate test binaries. `cargo test --workspace` runs crate tests concurrently (up to `--jobs` parallelism). This means the ~23s "non-AOT" figure is NOT independent of the 35.6s AOT time -- non-AOT crates run in parallel with AOT tests. The 59s wall time likely includes: (a) sequential compilation of all test binaries, (b) sequential cargo orchestration overhead, and (c) parallel test execution dominated by AOT. Section 03's measurement methodology will separate compilation time from execution time (via `cargo test --no-run` vs `cargo test`), which is essential for understanding the actual optimization surface. The largest crates by test execution time are ori_eval (4.5s incl. compilation), ori_arc (3.4s), ori_patterns (2.6s), and ori_ir (2.5s).

**Depends on:** Section 03 (Profiling Infrastructure) — flamegraphs and per-crate timing identify which functions to optimize.

---

## 05.1 Crate Compilation Time

**File(s):** Workspace `Cargo.toml`, individual crate `Cargo.toml` files

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

  - **Check codegen units**: By default, Rust uses 16 codegen units for debug builds. No `[profile.test]` or `[profile.dev]` sections exist in `Cargo.toml` (verified 2026-03-25), so defaults apply. Consider adding `[profile.dev] codegen-units = 256` for faster compilation (at cost of slightly slower test execution).

  - **Check incremental compilation**: No `incremental` setting exists in `Cargo.toml` or `.cargo/config.toml` (verified 2026-03-25). Rust defaults to incremental=true for dev/test, which is correct.

  - **Check for heavy proc-macros**: No proc-macro dependencies found in any compiler crate (verified 2026-03-25). This is not a compilation bottleneck.

  - **Large crate split evaluation**: ori_arc (~60K lines, 1,012 test functions across 38 test files) is the largest crate. Any change to ori_arc triggers recompilation of the entire crate + all tests. Evaluate whether splitting ori_arc into smaller crates would reduce incremental rebuild time. **This is a trade-off analysis, not a recommendation** — splitting adds dependency complexity.

- [ ] If compilation is <10s: skip the above and focus on test execution optimization instead.

### Test Strategy

- **TDD ordering**: `Cargo.toml` profile changes are configuration, not code. The test is that the entire suite continues working:
  - [ ] Before change: record `timeout 150 cargo t` pass/fail counts
  - [ ] After change: verify identical pass/fail counts
  - [ ] Before change: record `timeout 150 cargo b` success
  - [ ] After change: verify `timeout 150 cargo b` still succeeds
- **Measurement**: Record compilation time (`cargo test --workspace --no-run`) vs execution time. Determine which dominates.
- **Validation**: Any `Cargo.toml` changes don't break `cargo t` or `cargo b`.
- **Debug and release**: Both `timeout 150 cargo t` and `timeout 150 cargo b --release` must pass after changes.

---

## 05.2 Test Execution Hot Paths

**File(s):** Specific compiler source files identified by Section 03's flamegraphs

The flamegraphs from Section 03 will identify the top 10 hottest functions during test execution. This subsection addresses whatever those functions are.

**This subsection has two parts**: (a) tasks that can be started immediately (known crate analysis), and (b) tasks driven by Section 03's flamegraph findings. Part (b) tasks are conditional on profiling results — they execute with whatever the top 10 functions turn out to be.

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

- **TDD ordering**: Hot path optimizations modify compiler internals. For each optimization:
  - [ ] Record full test pass/fail output BEFORE the change
  - [ ] Apply the optimization (clone reduction, allocation removal, `#[inline]`, etc.)
  - [ ] Verify pass/fail output is identical AFTER the change
  - [ ] If the optimization changes a public API (unlikely but possible), write a targeted unit test first
- **Matrix**: Every optimization must preserve all existing test behavior. `timeout 150 cargo t` green after each change. Since hot path optimizations are not type- or pattern-dependent (they affect all types equally), the existing test suite IS the matrix -- but verify both:
  - `timeout 150 cargo t` (debug build, exercises more assertions)
  - `timeout 150 cargo t --release` (release build, different optimization behavior)
- **Semantic pin**: The existing test suite serves as the semantic pin for behavioral equivalence. Each optimization should also have a before/after timing measurement showing measurable improvement.
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

- [ ] **Evaluate `cargo-nextest`**: `cargo-nextest` runs each test as an individual process (better isolation) and can be faster than `cargo test` for large workspaces due to better parallelism. It is NOT currently installed (verified 2026-03-25).
  1. Install: `cargo install cargo-nextest`
  2. Measure: `hyperfine 'cargo nextest run --workspace' 'cargo test --workspace' --warmup 1 --runs 3`
  3. **Decision**: If nextest wall time is >10% faster, document and add as the default in `scripts/bench-tests.sh`. If slower or equivalent, uninstall (`cargo uninstall cargo-nextest`) and document the comparison result.
  4. **Note**: `cargo-nextest` may interact differently with AOT tests (each test already spawns 2 subprocesses; nextest adds another process layer). Profile to verify nextest doesn't degrade AOT performance specifically.

### Test Strategy

- **TDD ordering**: Workspace-level changes affect the build system, not compiler behavior. The "test" is the full suite:
  - [ ] Before each change: record `timeout 150 cargo t` pass/fail counts AND wall time
  - [ ] After each change: verify identical pass/fail counts AND record wall time delta
- **Validation**: Any workspace-level change must not break `cargo t`, `cargo b`, or `./test-all.sh`.
- **Debug and release**: `timeout 150 cargo t` and `timeout 150 cargo b --release` must both pass.
- **Measurement**: Wall time before/after each change, measured with `hyperfine` for statistical validity (3+ runs).

---

## 05.R Third Party Review Findings

- None.

---

## 05.4 Completion Checklist

- [ ] Compilation time vs execution time breakdown recorded
- [ ] Flamegraph top 10 hot functions analyzed and documented
- [ ] Each identified optimization measured (before/after per function)
- [ ] Workspace-level optimizations evaluated (parallel threads, nextest, etc.)
- [ ] Compilation time measured separately (cargo test --no-run): ???s
- [ ] Non-AOT crate execution times measured individually (per-crate): documented
- [ ] All tests pass identically (no behavioral changes)
- [ ] Optimizations documented with measured impact
- [ ] `timeout 150 cargo t` green
- [ ] `/tpr-review` passed — independent Codex review found no critical or major issues (or all findings triaged)
- [ ] `/impl-hygiene-review` passed — implementation hygiene review clean (phase boundaries, SSOT, algorithmic DRY, naming). MUST run AFTER `/tpr-review` is clean.
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** Each identified optimization is individually measured (before/after) and documented. No test code was modified. The flamegraph's top 10 hot functions have been addressed (either optimized or documented as "acceptable -- not optimizable without behavioral changes"). If Section 03's profiling reveals that non-AOT crates fully overlap with AOT execution (i.e., they finish before AOT does), the optimization focus shifts to compilation time reduction, which benefits wall time by reducing the sequential pre-test build phase.
