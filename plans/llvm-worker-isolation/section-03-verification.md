---
section: "03"
title: "Verification"
status: not-started
reviewed: true
goal: "Verify subprocess isolation works end-to-end: no crashes, correct results, acceptable performance, test gate integrity"
success_criteria:
  - "./test-all.sh passes cleanly with no CRASHED status"
  - "Pass/fail/skip/lcfail counts match between subprocess and in-process runs (for non-crashing files)"
  - "BackendCrash outcomes appear in test-all.sh summary and cause exit code 1"
  - "Performance: LLVM spec test wall-clock time within 2x of pre-change baseline"
  - "Debug AND release builds pass"
  - "Weakened test gate confirmed reverted: no ORI_LLVM_CRASHED variable or exit-0 escape hatch remains"
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
- [ ] Weakened test gate confirmed reverted: no `ORI_LLVM_CRASHED` variable or exit-0 escape hatch remains in `test-all.sh`
- [ ] Debug AND release builds pass
- [ ] Satisfies all mission success criteria

**Context:** The subprocess isolation changes how LLVM spec tests are executed — from in-process to per-file subprocesses. This must not change observable results for files that work correctly, must contain crashes for files that don't, and must not unacceptably slow down the test suite.

**Depends on:** Section 02 (orchestrator fully operational).

---

## 03.1 Behavioral Equivalence

Verify that the subprocess-based runner produces identical results to the old in-process runner for all non-crashing test files.

**Approach**: Use the `--json` flag for machine-comparable output. The in-process path is still accessible by directly calling `run_file_with_interner()` in a Rust test (bypassing the orchestrator).

- [ ] **Baseline capture**: Before the orchestrator is wired in (or using `--json` in worker mode directly), run `ori test --backend=llvm --json tests/spec/` and save per-file pass/fail/skip/lcfail counts. Script: `timeout 150 ./target/release/ori test --backend=llvm --json tests/spec/ > /tmp/llvm-baseline.json 2>/dev/null`
- [ ] **Subprocess capture**: After wiring in the orchestrator (02.4), run the same test suite through subprocess isolation and capture counts. The orchestrator's human-readable output includes per-file counts in `print_summary_stats()`.
- [ ] **Diff**: Compare per-file results programmatically. Every non-crashing file must produce identical outcomes. Write a Rust test or script that compares the two JSON outputs.
- [ ] **Edge cases to verify** (each becomes a specific test assertion):
  - [ ] File with 0 LLVM-eligible tests (only `compile_fail` tests) — verify `results: []` and counters at 0
  - [ ] File where all tests are `#skip`ed — verify all outcomes are `Skipped`
  - [ ] File with `LlvmCompileFail` outcomes — verify codegen errors produce `LlvmCompileFail`, not `BackendCrash`
  - [ ] File with mixed outcomes (some pass, some fail) — verify pass/fail counts match
  - [ ] File with large test count (>20 tests in one file) — verify all tests appear in JSON output
  - [ ] File that uses `print()` — verify JSON is still extractable despite stdout pollution
  - [ ] File with `SkippedUnchanged` outcomes (incremental mode) — verify JSON correctly represents skipped-unchanged tests if incremental is enabled

- [ ] **Subsection close-out (03.1)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-03.1 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 03.1: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 03.2 Crash Isolation Verification

Verify that worker crashes are contained and correctly reported.

- [ ] **Crash canary identification**: Find or create a minimal test file that triggers the known LLVM C++ crash.
  - **Step 1**: Run `timeout 150 ./target/release/ori test --backend=llvm tests/spec/ 2>&1` and check exit code. If >128, identify the crashing files from stderr.
  - **Step 2**: If a crashing file is found, extract the minimal reproducer into `tests/spec/llvm_worker_crash_canary.ori`.
  - **Step 3**: If no file currently crashes (all handled by `catch_unwind` + `LlvmCompileFail`): create a Rust integration test that uses `Command::new("sh").arg("-c").arg("kill -11 $$")` as a crash simulation, rather than relying on finding a specific Ori crash pattern. This simulates the crash scenario the orchestrator must handle.
  - After this plan, crash canary files should produce `BackendCrash` instead of crashing the runner.
- [ ] **Verify parent survival** (integration test in `compiler/oric/src/test/runner/llvm_worker/tests.rs`):
  - [ ] `test_parent_survives_crash` — run `ori test --backend=llvm` including the crash canary. Verify:
    - Exit code is 0 or 1 (NOT 139 = SIGSEGV)
    - Stdout contains "CRASH" or "BackendCrash" for the canary file
    - Stdout contains "PASS" for non-crashing files (parent continued after crash)
- [ ] **Verify exit code blocking** (integration test):
  - [ ] `test_backend_crash_blocks_gate` — run only the crash canary file, verify exit code == 1
- [ ] **Verify timeout mechanism** (unit test, already covered in 02.2.T):
  - [ ] Confirm `test_wait_with_timeout_kills_slow_process` passes with short timeout
- [ ] **Verify multiple concurrent crashes** (integration test):
  - [ ] `test_multiple_crashes_all_reported` — run with 3+ crash canaries interspersed with good files. All crashes reported, all good files produce correct results. No partial runs or hangs.
- [ ] **Debug AND release build verification**:
  - [ ] Run `timeout 150 cargo build && ./target/debug/ori test --backend=llvm tests/spec/types/primitives.ori` — verify debug build works
  - [ ] Run `timeout 150 cargo build --release && ./target/release/ori test --backend=llvm tests/spec/types/primitives.ori` — verify release build works
  - [ ] Both produce identical pass/fail counts for the same file

- [ ] **Subsection close-out (03.2)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-03.2 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 03.2: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 03.3 Performance Measurement

Measure the overhead of subprocess isolation vs in-process execution.

**Important context**: Each worker process re-parses and re-typechecks its file from scratch. This duplicates work but is necessary for process isolation (no shared memory across process boundary). However, with subprocess isolation, the LLVM `Context::create()` global lock contention that forced sequential execution (see `runner/mod.rs` line 116-120 comment) no longer applies — each process has its own LLVM context. Parallelism should largely offset per-file overhead.

- [ ] **Baseline measurement** (before wiring orchestrator, or using git stash): Time the in-process sequential LLVM spec test run:
  ```bash
  time timeout 150 ./target/release/ori test --backend=llvm tests/spec/
  ```
  Record: wall-clock time, total files processed, total tests. Save as `plans/llvm-worker-isolation/perf-baseline.txt`.
- [ ] **Subprocess sequential**: Time with subprocess isolation, sequential:
  ```bash
  time timeout 150 ./target/release/ori test --backend=llvm --no-parallel tests/spec/
  ```
- [ ] **Subprocess parallel** (default): Time with subprocess isolation, default parallelism:
  ```bash
  time timeout 150 ./target/release/ori test --backend=llvm tests/spec/
  ```
- [ ] **Overhead analysis**: Calculate per-file subprocess overhead:
  - Expected: ~10-50ms per file for process spawn + JSON parse
  - With ~300 files sequential: ~3-15s total overhead
  - With parallelism (N = CPU count): overhead amortized, net speedup if N > 2
- [ ] **Acceptance criteria**:
  - Sequential: wall-clock within 2x of baseline
  - Parallel: wall-clock within 1.5x of baseline (parallelism should offset subprocess overhead)
  - If parallel is FASTER than baseline (likely with CPU count > 2), that's a bonus
- [ ] **If too slow**: Profile to identify bottleneck:
  1. Process spawn overhead? → measure with `time sh -c "for i in $(seq 300); do ./target/release/ori --version; done"`
  2. JSON parse overhead? → benchmark `serde_json::from_str` on a typical `JsonFileSummary`
  3. Re-parsing/re-typechecking? → compare single-file in-process vs subprocess time
  4. Mitigations (not in this plan, future optimization): batch multiple files per worker, pre-compute data via temp file

- [ ] **Subsection close-out (03.3)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-03.3 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 03.3: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 03.4 Test Gate Integrity

Verify that the test gate (`./test-all.sh`) correctly reflects the new subprocess-based execution.

- [ ] **test-all.sh output verification**: Run `timeout 150 ./test-all.sh` and check the LLVM backend line. It should show:
  - `Ori spec (LLVM backend)   N passed, M failed, K skipped, L llvm compile fail` (with optional `B backend crash` count if crashes exist)
  - NOT `CRASHED` — the parent process no longer crashes. `test-all.sh` line 458-459 previously showed `CRASHED` when exit code > 128; this path was removed in 02.4.
  - If `BackendCrash` outcomes exist, they appear as a separate count parsed by `parse_ori_results()`
- [ ] **Exit code propagation**: `test-all.sh` exit code is non-zero when `BackendCrash` outcomes exist:
  - [ ] If crashes exist: `ori test --backend=llvm` exits 1 → `ORI_LLVM_EXIT=1` → `ANY_FAILED > 0` → `test-all.sh` exits 1
  - [ ] If no crashes: exit 0 as before
- [ ] **JSON output**: If `test-all.sh` emits JSON (`--json` or `--json=<path>`, lines 33-41), verify `BackendCrash` outcomes appear. The `emit_json()` function (line 480) was updated in 02.4 to remove the `ORI_LLVM_CRASHED` path — verify it now emits `backend_crash` count in the suite JSON. Specifically:
  - [ ] Run `timeout 150 ./test-all.sh --json=/tmp/test-results.json` and verify the LLVM backend suite entry has numeric `passed`/`failed`/`skipped`/`lcfail` (not `"status": "crashed"`)
- [ ] **Pre-commit hook**: Verify `./full-check.sh` (runs `./clippy-all.sh` then `./test-all.sh`) passes when no crashes occur. This is the ultimate acceptance test:
  - [ ] Run `timeout 150 ./full-check.sh` — verify exit code 0
- [ ] **Weakened gate confirmed removed**: Verify by grep:
  - [ ] `grep -c ORI_LLVM_CRASHED test-all.sh` returns 0
  - [ ] `grep -c ANY_CORE_FAILED test-all.sh` returns 0
  - This is the core deliverable: crashes are real failures that block the gate.
- [ ] **Regression guard**: The tests from 02.2.T and 02.4.T serve as permanent regression guards. No additional CI-style test needed — `test-all.sh` itself IS the regression test (it now reports crashes as failures instead of hiding them).

- [ ] **Subsection close-out (03.4)** — MANDATORY before starting the next subsection. Run `/improve-tooling` retrospectively on THIS subsection's debugging journey (per `.claude/skills/improve-tooling/SKILL.md` "Per-Subsection Workflow"): which `diagnostics/` scripts you ran, where you added `dbg!`/`tracing` calls, where output was hard to interpret, where test failures gave unhelpful messages, where you ran the same command sequence repeatedly. Forward-look: what tool/log/diagnostic would shorten the next regression in this code path by 10 minutes? Implement improvements NOW (zero deferral) and commit each via SEPARATE `/commit-push` using a valid conventional-commit type (`build(diagnostics): ... — surfaced by section-03.4 retrospective` — `build`/`test`/`chore`/`ci`/`docs` are valid; `tools(...)` is rejected by the lefthook commit-msg hook). Mandatory even when nothing felt painful. If genuinely no gaps, document briefly: "Retrospective 03.4: no tooling gaps". Update this subsection's `status` in section frontmatter to `complete`.

---

## 03.R Third Party Review Findings

- None.

---

## 03.N Completion Checklist

- [ ] Behavioral equivalence verified: subprocess results match in-process for all non-crashing files (per-file diff of pass/fail/skip/lcfail counts)
- [ ] Crash isolation verified: parent survives worker SIGSEGV, reports BackendCrash
- [ ] Multiple concurrent crashes handled correctly (3+ canaries interspersed with good files)
- [ ] Timeout mechanism verified (unit tests from 02.2.T)
- [ ] Debug AND release builds pass with identical results
- [ ] Performance measured: wall-clock within 2x baseline (sequential), within 1.5x (parallel)
- [ ] Performance numbers recorded in `plans/llvm-worker-isolation/perf-baseline.txt`
- [ ] test-all.sh output correct: no `CRASHED`, `BackendCrash` in counts
- [ ] Weakened test gate confirmed removed: `grep -c ORI_LLVM_CRASHED test-all.sh` returns 0
- [ ] `ANY_CORE_FAILED` confirmed removed: `grep -c ANY_CORE_FAILED test-all.sh` returns 0
- [ ] test-all.sh `--json` output includes `backend_crash` count (not `"status": "crashed"`)
- [ ] Pre-commit hook (`./full-check.sh`) passes
- [ ] Crash canary test file or simulation committed
- [ ] `timeout 150 ./test-all.sh` passes
- [ ] `./clippy-all.sh` passes
- [ ] **Final file size audit**:
  - [ ] `llvm_worker.rs` under 500 lines
  - [ ] `runner/mod.rs` under 575 lines (goal: net reduction from removing stale LLVM comment)
  - [ ] `result/mod.rs` under 350 lines (only ~10 lines added)
  - [ ] `json_protocol.rs` under 200 lines
  - [ ] `commands/test.rs` under 260 lines (only ~15 lines added)
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 03` returns 0 annotations
- [ ] **Plan sync** — update plan metadata:
  - [ ] All section frontmatter `status` → `complete`
  - [ ] `00-overview.md` Quick Reference and mission criteria checked
  - [ ] `index.md` statuses updated
  - [ ] JIT EH plan `section-06-lcfail-resolution.md` updated with note that LLVM backend crash is now contained
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `./test-all.sh` passes with exit code 0. The LLVM backend summary line shows pass/fail/crash counts (not `CRASHED`). Worker crashes produce `BackendCrash` outcomes that block the test gate. Performance overhead is within 2x of baseline. The pre-commit hook (`./full-check.sh`) passes for `.rs` file changes. All mission success criteria are met.
