---
section: "03"
title: "Verification"
status: not-started
reviewed: false
goal: "Verify subprocess isolation works end-to-end: no crashes, correct results, acceptable performance, test gate integrity"
success_criteria:
  - "./test-all.sh passes cleanly with no CRASHED status"
  - "Pass/fail/skip/lcfail counts match between subprocess and in-process runs (for non-crashing files)"
  - "BackendCrash outcomes appear in test-all.sh summary and cause exit code 1"
  - "Performance: LLVM spec test wall-clock time within 2x of pre-change baseline"
  - "Debug AND release builds pass"
  - "Satisfies all mission success criteria"
inspired_by:
  - "Rust compiletest deadline tracking and timeout verification"
  - "Zig compilation test matrix across backends"
depends_on: ["02"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "03.1"
    title: "Behavioral equivalence"
    status: not-started
  - id: "03.2"
    title: "Crash isolation verification"
    status: not-started
  - id: "03.3"
    title: "Performance measurement"
    status: not-started
  - id: "03.4"
    title: "Test gate integrity"
    status: not-started
  - id: "03.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "03.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 03: Verification

**Status:** Not Started
**Goal:** Verify the subprocess isolation works correctly end-to-end. Confirm behavioral equivalence with the old in-process path, verify crash isolation, measure performance overhead, and validate test gate integrity.

**Success Criteria:**

- [ ] `./test-all.sh` passes with no `CRASHED` status
- [ ] Behavioral equivalence: same counts for non-crashing files
- [ ] Crash isolation: parent survives worker SIGSEGV
- [ ] Performance: wall-clock within 2x baseline
- [ ] Satisfies all mission success criteria

**Context:** The subprocess isolation changes how LLVM spec tests are executed — from in-process to per-file subprocesses. This must not change observable results for files that work correctly, must contain crashes for files that don't, and must not unacceptably slow down the test suite.

**Depends on:** Section 02 (orchestrator fully operational).

---

## 03.1 Behavioral Equivalence

Verify that the subprocess-based runner produces identical results to the old in-process runner for all non-crashing test files.

- [ ] **Baseline capture**: Before removing the in-process path, run `ori test --backend=llvm tests/spec/` in-process and record per-file pass/fail/skip/lcfail counts to a JSON file
- [ ] **Subprocess capture**: Run the same test suite through the new subprocess orchestrator and record per-file counts
- [ ] **Diff**: Compare per-file results. Every file that doesn't crash must produce identical outcomes. Differences are bugs in the orchestrator or JSON protocol.
- [ ] **Edge cases to verify**:
  - Files with 0 LLVM-eligible tests (only compile_fail tests)
  - Files where all tests are `#skip`ed
  - Files with `LlvmCompileFail` outcomes (codegen errors caught by `catch_unwind`)
  - Files with mixed outcomes (some pass, some fail)
  - Files with large test counts (>20 tests in one file)

---

## 03.2 Crash Isolation Verification

Verify that worker crashes are contained and correctly reported.

- [ ] **Crash test file**: Identify or create a minimal test file that triggers the known LLVM C++ crash. <!-- reviewed: accuracy fix — the unresolved type variable path now returns TypeInfo::Error (graceful degradation), not a crash. The actual crashes come from LLVM C++ when codegen emits malformed IR from these error paths (e.g., wrong type sizes, null function pointers, bad GEP indices). --> The approach:
  1. **Find an existing crash**: Run `ori test --backend=llvm tests/spec/` and identify files that produce exit code >128 (killed by signal). These are the real crash canaries.
  2. **Isolate the minimal reproducer**: Extract the crashing test into `tests/spec/llvm_worker_crash_canary.ori`.
  3. **If no file currently crashes** (all handled by `catch_unwind` + `LlvmCompileFail`): create a canary using `std::process::abort()` in a Rust test helper to simulate a worker crash, rather than relying on finding a specific Ori pattern that crashes LLVM C++.
  After this plan, crash canary files should produce `BackendCrash` instead of crashing the runner.
- [ ] **Verify parent survival**: Run `ori test --backend=llvm tests/spec/` including the crash canary. The parent process must:
  - NOT crash (exit code is 0 or 1, not 139)
  - Report `BackendCrash` for the canary file
  - Continue processing remaining files after the canary
  - Produce correct results for all non-crashing files
- [ ] **Verify exit code**: The presence of `BackendCrash` outcomes must produce exit code 1 (failure). This blocks the test gate.
- [ ] **Verify timeout**: Create a test that verifies the timeout mechanism by spawning a worker with a very short timeout (1s) against a file that takes longer to compile. The orchestrator should kill the worker and report `BackendCrash` with a timeout message.
- [ ] **Verify multiple crashes**: Run with 3+ crash canary files interspersed with good files. All crashes should be reported, all good files should produce correct results.
- [ ] **Debug AND release**: Verify crash isolation works in both `cargo build` (debug) and `cargo build --release` (release) modes.

---

## 03.3 Performance Measurement

Measure the overhead of subprocess isolation vs in-process execution.

- [ ] **Baseline measurement**: Time the full LLVM spec test run with in-process execution (before this change, or with `--no-parallel` to compare sequential overhead):
  ```bash
  time ori test --backend=llvm tests/spec/
  ```
- [ ] **Subprocess sequential**: Time with subprocess isolation, sequential (`--no-parallel`):
  ```bash
  time ori test --backend=llvm --no-parallel tests/spec/
  ```
- [ ] **Subprocess parallel**: Time with subprocess isolation, default parallelism:
  ```bash
  time ori test --backend=llvm tests/spec/
  ```
- [ ] **Overhead analysis**: Calculate per-file subprocess overhead:
  - Expected: ~10-50ms per file for process spawn + JSON parse
  - With ~200 files sequential: ~2-10s total overhead
  - With parallelism: overhead amortized across CPU count
- [ ] **Acceptance criteria**: Total wall-clock time within 2x of baseline for sequential, within 1.5x for parallel (parallelism should offset subprocess overhead)
- [ ] **If too slow**: Profile to identify bottleneck (process spawn? JSON parse? re-parsing/re-typechecking per worker?). Each worker process re-parses and re-typechecks its file from scratch -- this duplicates work but is necessary for process isolation (no shared memory across process boundary). With subprocess isolation, the LLVM `Context::create()` contention that forced sequential execution no longer applies (each process has its own LLVM context), so parallelism should largely offset the per-file overhead. If still too slow, consider: (1) increasing pool size, (2) batching multiple files per worker (amortize process spawn), (3) passing pre-computed data via temp file (future optimization, not in this plan). <!-- reviewed: feasibility note — clarified why re-parsing is unavoidable and why parallelism helps -->

---

## 03.4 Test Gate Integrity

Verify that the test gate (`./test-all.sh`) correctly reflects the new subprocess-based execution.

- [ ] **test-all.sh output**: The LLVM backend line should show: <!-- reviewed: accuracy fix — aligned with actual test-all.sh summary format (line 461) -->
  - `Ori spec (LLVM backend)   N passed, M failed, K skipped, L llvm compile fail` (or with additional crash count)
  - NOT `CRASHED` -- the parent process no longer crashes (currently test-all.sh shows `CRASHED` when exit code > 128, see line 458-459)
  - If `BackendCrash` outcomes exist, they should appear as a separate count in `print_summary_stats()` output, and `parse_ori_results()` in test-all.sh must extract it
- [ ] **Exit code propagation**: `test-all.sh` exit code is non-zero when `BackendCrash` outcomes exist
- [ ] **JSON output**: If `test-all.sh` emits JSON (`--json` or `--json=<path>`, see test-all.sh lines 33-41), verify `BackendCrash` outcomes appear in the JSON output. The `emit_json()` function (line 480) needs to handle the new crash/backend_crash counts. <!-- reviewed: accuracy fix — flag is --json not --json-results -->
- [ ] **Pre-commit hook**: Verify `./full-check.sh` (runs `./clippy-all.sh` then `./test-all.sh`) passes when no crashes occur and fails when crashes occur. This is the ultimate acceptance test for the plan. <!-- reviewed: accuracy fix — full-check.sh runs clippy first -->
- [ ] **Regression test**: Add a CI-style test that runs `./test-all.sh` and verifies exit code 0. This catches future regressions where the test runner starts crashing again.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] Behavioral equivalence verified: subprocess results match in-process for non-crashing files
- [ ] Crash isolation verified: parent survives worker SIGSEGV, reports BackendCrash
- [ ] Multiple concurrent crashes handled correctly
- [ ] Timeout mechanism verified
- [ ] Debug AND release builds pass
- [ ] Performance measured: wall-clock within 2x baseline
- [ ] test-all.sh output correct (no CRASHED, BackendCrash in counts)
- [ ] Pre-commit hook (full-check.sh) passes
- [ ] Crash canary test file committed
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 03` returns 0 annotations
- [ ] **Plan sync** — update plan metadata:
  - [ ] All section frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference and mission criteria updated
  - [ ] `index.md` statuses updated
  - [ ] JIT EH plan `section-06-lcfail-resolution.md` updated with note that LLVM backend crash is now contained
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `./test-all.sh` passes with exit code 0. The LLVM backend summary line shows pass/fail/crash counts (not `CRASHED`). Worker crashes produce `BackendCrash` outcomes that block the test gate. Performance overhead is within 2x of baseline. The pre-commit hook (`./full-check.sh`) passes for `.rs` file changes. All mission success criteria are met.
