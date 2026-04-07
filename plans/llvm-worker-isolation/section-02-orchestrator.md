---
section: "02"
title: "Subprocess Orchestrator"
status: not-started
reviewed: true
goal: "Replace in-process LLVM test execution with subprocess-per-file isolation — worker crashes contained, results parsed from JSON"
success_criteria:
  - "LLVM backend spec test run completes without crashing the parent process"
  - "Worker SIGSEGV/SIGABRT produces BackendCrash outcomes (detected via ExitStatus::signal() on Unix)"
  - "Worker timeout produces BackendCrash with timeout message"
  - "Results match: subprocess-based run produces same pass/fail/skip/lcfail counts as in-process run (for non-crashing files)"
  - "Bounded concurrency: worker pool limited to CPU count (no fork-bomb), verified via --parallel-workers=N and --no-parallel flags"
  - "Weakened test gate reverted: ORI_LLVM_CRASHED exit-0 escape hatch removed from test-all.sh"
  - "Satisfies mission criteria: test-all.sh passes, crashes reported as real failures"
inspired_by:
  - "Zig Compilation.zig:6304-6334 subprocess spawn + wait pattern for clang codegen"
  - "Rust compiletest executor.rs:24-93 per-test isolation with deadline tracking"
  - "LLVM lit per-test timeout with process tree kill"
depends_on: ["01"]
third_party_review:
  status: none
  updated: null
sections:
  - id: "02.1"
    title: "Worker spawning and result collection"
    status: not-started
  - id: "02.2"
    title: "Crash and timeout detection"
    status: not-started
  - id: "02.3"
    title: "Bounded worker pool"
    status: not-started
  - id: "02.4"
    title: "Integration with test runner dispatch"
    status: not-started
  - id: "02.R"
    title: "Third Party Review Findings"
    status: not-started
  - id: "02.N"
    title: "Completion Checklist"
    status: not-started
---

# Section 02: Subprocess Orchestrator

**Status:** Not Started
**Goal:** Replace the in-process `run_file_llvm()` call with subprocess spawning. Each spec test file is compiled and executed in a separate `ori test --backend=llvm --json <file>` process. The orchestrator collects results via JSON stdout, detects crashes via exit codes, and aggregates into the existing `TestSummary`.

**Success Criteria:**

- [ ] LLVM backend spec tests complete without crashing the parent process
- [ ] Worker crashes produce `BackendCrash` outcomes that block the test gate
- [ ] Worker timeouts are detected and reported
- [ ] Pass/fail/skip/lcfail counts match in-process execution for non-crashing files
- [ ] Satisfies mission criteria: `./test-all.sh` passes, crashes are real failures

**Context:** The current architecture calls `run_file_llvm()` in-process for each spec test file. When LLVM C++ encounters malformed IR (from unresolved type variables, missing monomorphization, etc.), it crashes with SIGSEGV. Rust's `catch_unwind` wraps the compilation but cannot catch C++ signals. The crash kills the entire test runner process, failing `./test-all.sh` and blocking the pre-commit hook. Moving to subprocess-per-file provides OS-level fault containment.

**Reference implementations:**
- **Zig** `src/Compilation.zig:6304-6334`: spawns clang as subprocess per C object file, captures exit code + stderr, handles crash via exit code
- **Rust** `src/tools/compiletest/src/executor.rs:66-88`: per-test deadline tracking with `try_wait()` polling

**Depends on:** Section 01 (JSON output protocol — the `--json` flag and `JsonFileSummary` types).

---

## 02.1 Worker Spawning and Result Collection

**File(s):** new `compiler/oric/src/test/runner/llvm_worker.rs` (orchestrator module)

Extract the subprocess orchestration logic into a new `llvm_worker.rs` module. The existing `llvm_backend.rs` (545 lines) remains as the worker's in-process execution path (used when `ori test --backend=llvm --json` is invoked on a single file). The new module handles spawning and result collection.

**File size constraint:** `llvm_worker.rs` must stay under 500 lines. Target: ~200-300 lines for spawn + collect + extract + per-file orchestrator. Pool logic is in 02.3 and may need its own submodule if combined total exceeds 500.

**TDD ordering: write tests FIRST, verify they fail, then implement.**

- [ ] Create `compiler/oric/src/test/runner/llvm_worker.rs` — the orchestrator module
- [ ] Declare `#[cfg(feature = "llvm")] mod llvm_worker;` in `runner/mod.rs` (alongside existing `#[cfg(feature = "llvm")] mod llvm_backend;`)
- [ ] Binary path resolution: `current_exe()` is resolved once in `run_llvm_tests_isolated()` (see 02.3) and passed to all worker spawn calls. No per-file resolution. `current_exe()` returns the actual binary path (e.g., `target/release/ori` from test-all.sh, or `target/debug/ori` in dev).
- [ ] **02.1.T — Tests first** (in `compiler/oric/src/test/runner/llvm_worker/tests.rs` — sibling `tests.rs` pattern):
  - [ ] `test_extract_framed_json_success` — stdout with sentinels and JSON between them returns `Some(json_content)`
  - [ ] `test_extract_framed_json_with_print_pollution` — stdout has Ori `print()` output before/after sentinels, JSON still extracted correctly
  - [ ] `test_extract_framed_json_missing_begin` — no begin sentinel returns `None`
  - [ ] `test_extract_framed_json_missing_end` — begin but no end sentinel returns `None`
  - [ ] `test_extract_framed_json_empty_content` — sentinels with nothing between them returns `Some("")`
  - [ ] `test_spawn_worker_good_file` — spawn `current_exe() test --backend=llvm --json tests/spec/types/primitives.ori`, verify exit 0 and sentinel-framed JSON in stdout
  - [ ] `test_spawn_worker_nonexistent_file` — spawn with nonexistent file, verify non-zero exit (1 or 2) and either no sentinel frame or valid JSON with error
  - [ ] Verify tests fail before implementing
- [ ] Implement `extract_framed_json(stdout: &str) -> Option<&str>`:
  - Scan for `JSON_BEGIN_SENTINEL` and `JSON_END_SENTINEL` (imported from `json_protocol`)
  - Return content between sentinels (trimmed)
  - Return `None` if either sentinel missing
- [ ] Implement `spawn_llvm_worker(binary: &Path, file: &Path, config: &TestRunnerConfig) -> std::io::Result<Child>`:
  ```rust
  fn spawn_llvm_worker(
      binary: &Path,
      file: &Path,
      config: &TestRunnerConfig,
  ) -> std::io::Result<Child> {
      let mut cmd = Command::new(binary);
      cmd.arg("test")
          .arg("--backend=llvm")
          .arg("--json")
          .arg(file)
          .stdout(Stdio::piped())
          .stderr(Stdio::piped());
      // Forward filter if present
      if let Some(ref filter) = config.filter {
          cmd.arg(format!("--filter={filter}"));
      }
      cmd.spawn()
  }
  ```
- [ ] Implement `collect_worker_result(child: Child, file: &Path, timeout: Duration, interner: &StringInterner) -> FileSummary`:
  - Wait for child to exit (with timeout — see 02.2 for `wait_with_timeout`)
  - **Signal death** (Unix: `status.signal().is_some()`): worker crashed -> `crash_summary()` (see 02.2)
  - **Exit code 0 or 1**: read stdout, call `extract_framed_json()`, parse as `Vec<JsonFileSummary>`, convert first element via `into_file_summary()`. Anything outside sentinel frame is discarded.
  - **Exit code 2**: no tests found -> empty `FileSummary` with `results: []`
  - **JSON parse failure** (no sentinel frame, or malformed content): fall back to `crash_summary()` with message "worker exited {code} with no JSON output" and include last 5 lines of stderr
- [ ] Implement `run_file_llvm_isolated(file: &Path, binary: &Path, config: &TestRunnerConfig, interner: &StringInterner) -> FileSummary` — the top-level per-file orchestrator function. The `interner` param is used for `crash_summary()` and `into_file_summary()` re-interning only.
- [ ] Verify all tests from 02.1.T now PASS

---

## 02.2 Crash and Timeout Detection

**File(s):** `compiler/oric/src/test/runner/llvm_worker.rs`

Handle the two failure modes that in-process execution can't survive: worker signal death and worker hangs.

**TDD ordering: write tests FIRST for `detect_crash`, `crash_summary`, and `wait_with_timeout`.**

- [ ] **02.2.T — Tests first** (in `compiler/oric/src/test/runner/llvm_worker/tests.rs`):
  - [ ] `test_detect_crash_sigsegv` — spawn `sh -c "kill -11 $$"`, wait, pass exit status to `detect_crash`, verify returns `Some("worker killed by SIGSEGV (signal 11)")` (Unix-only, `#[cfg(unix)]`)
  - [ ] `test_detect_crash_sigabrt` — spawn `sh -c "kill -6 $$"`, verify `Some("worker killed by SIGABRT (signal 6)")`
  - [ ] `test_detect_crash_normal_exit` — spawn a process that exits 0, verify `detect_crash` returns `None`
  - [ ] `test_detect_crash_error_exit` — spawn a process that exits 1, verify `detect_crash` returns `None`
  - [ ] `test_wait_with_timeout_completes` — spawn `true` (exits immediately), verify `wait_with_timeout(1s)` returns `Ok(status)`
  - [ ] `test_wait_with_timeout_kills_slow_process` — spawn `sleep 999`, verify `wait_with_timeout(100ms)` returns `Err(WaitError::Timeout { .. })` promptly
  - [ ] `test_crash_summary_has_backend_crash` — call `crash_summary`, verify result has `backend_crash == 1`, `has_failures() == true`, and single `BackendCrash` test result
  - [ ] Verify tests fail before implementing
- [ ] **Crash detection** (`detect_crash`): In `collect_worker_result`, check for signal death:
  - On Unix: use `status.signal()` from `std::os::unix::process::ExitStatusExt`. `status.code()` returns `None` for signal-killed processes (the 128+N convention is shell-only, not Rust).
  - On non-Unix: use `status.code()` and treat unexpected non-zero codes as potential crashes.
  ```rust
  #[cfg(unix)]
  fn detect_crash(status: ExitStatus) -> Option<String> {
      use std::os::unix::process::ExitStatusExt;
      if let Some(signal) = status.signal() {
          let sig_name = match signal {
              11 => "SIGSEGV",
              6 => "SIGABRT",
              _ => "unknown signal",
          };
          Some(format!("worker killed by {sig_name} (signal {signal})"))
      } else {
          None
      }
  }

  #[cfg(not(unix))]
  fn detect_crash(_status: ExitStatus) -> Option<String> {
      // Signal detection not available on non-Unix.
      // Crashes in the subprocess won't be distinguished from normal failures.
      None
  }
  ```
- [ ] **Crash result construction** (`crash_summary`): Single synthetic test result (orchestrator doesn't know which tests were in the file):
  ```rust
  fn crash_summary(file: &Path, message: String, interner: &StringInterner) -> FileSummary {
      let mut summary = FileSummary::new(file.to_path_buf());
      summary.add_result(TestResult {
          name: interner.intern("llvm_backend_crash"),
          targets: vec![],
          outcome: TestOutcome::BackendCrash(message),
          duration: Duration::ZERO,
      });
      summary
  }
  ```
  Note: using `add_result()` instead of manual field construction ensures counter bookkeeping is always consistent (single source of truth for the `match` in `add_result`).
- [ ] **Timeout detection** (`wait_with_timeout`): `try_wait()` polling with configurable timeout (default 60s):
  ```rust
  enum WaitError {
      Timeout { elapsed: Duration },
      Io(std::io::Error),
  }

  fn wait_with_timeout(
      child: &mut Child,
      timeout: Duration,
  ) -> Result<ExitStatus, WaitError> {
      let start = Instant::now();
      loop {
          match child.try_wait() {
              Ok(Some(status)) => return Ok(status),
              Ok(None) if start.elapsed() > timeout => {
                  let _ = child.kill();
                  let _ = child.wait(); // reap zombie
                  return Err(WaitError::Timeout { elapsed: start.elapsed() });
              }
              Ok(None) => std::thread::sleep(Duration::from_millis(50)),
              Err(e) => return Err(WaitError::Io(e)),
          }
      }
  }
  ```
  Note: `child.kill()` and `child.wait()` use `let _ =` to avoid propagating IO errors from already-dead processes. `WaitError` is an enum covering both IO errors and timeout, ensuring the `?`-free match on `try_wait()` handles all cases explicitly.
- [ ] **Timeout configuration**: Add `worker_timeout: Duration` to `TestRunnerConfig` with default 60 seconds. Parse `--worker-timeout=N` CLI flag in `main.rs` (inside "test" match arm):
  ```rust
  } else if let Some(secs) = arg.strip_prefix("--worker-timeout=") {
      if let Ok(n) = secs.parse::<u64>() {
          config.worker_timeout = Duration::from_secs(n);
      }
  }
  ```
- [ ] **Capture stderr on crash**: Include last 5 lines of stderr in `BackendCrash` message for diagnostic context. Use `String::from_utf8_lossy` since stderr may contain non-UTF8 from LLVM C++.
- [ ] Verify all tests from 02.2.T now PASS

---

## 02.3 Bounded Worker Pool

**File(s):** `compiler/oric/src/test/runner/llvm_worker.rs`

The current LLVM backend runs files sequentially because LLVM context creation contends on global state within a single process. With subprocess isolation, each worker has its own address space — parallelism is safe. Use a bounded pool to limit concurrency to ~CPU count.

- [ ] Implement a simple bounded worker pool:
  ```rust
  struct WorkerPool {
      max_workers: usize,
      active: Vec<(PathBuf, Child)>,
      timeout: Duration,
  }

  impl WorkerPool {
      fn new(max_workers: usize, timeout: Duration) -> Self { ... }

      /// Submit a file. If pool is full, wait for one to finish first.
      /// Returns the completed worker's result if pool was full.
      fn submit(&mut self, file: PathBuf, child: Child)
          -> Option<(PathBuf, ExitStatus, Vec<u8>, Vec<u8>)>
      {
          let result = if self.active.len() >= self.max_workers {
              // Poll all active children with try_wait() to find one that's done.
              // If none are done, sleep briefly and retry (bounded by timeout).
              Some(self.wait_any())
          } else {
              None
          };
          self.active.push((file, child));
          result
      }

      /// Poll active children until one finishes. Collect its stdout/stderr
      /// via take_stdout()/take_stderr() before wait().
      fn wait_any(&mut self) -> (PathBuf, ExitStatus, Vec<u8>, Vec<u8>) {
          // Note: must call child.stdout.take() and child.stderr.take()
          // BEFORE child.wait(), then read the taken handles to completion.
          // child.wait() closes stdin but stdout/stderr handles are owned
          // by the Stdio::piped() setup — taking them transfers ownership.
          // Alternative: use child.wait_with_output() which handles this
          // automatically, but it consumes the Child.
          loop {
              for i in 0..self.active.len() {
                  if let Ok(Some(status)) = self.active[i].1.try_wait() {
                      let (path, mut child) = self.active.swap_remove(i);
                      let stdout = read_child_pipe(child.stdout.take());
                      let stderr = read_child_pipe(child.stderr.take());
                      return (path, status, stdout, stderr);
                  }
              }
              std::thread::sleep(Duration::from_millis(10));
          }
      }

      /// Wait for all remaining workers to finish.
      fn drain(&mut self) -> Vec<(PathBuf, ExitStatus, Vec<u8>, Vec<u8>)> { ... }

      /// Kill workers that have exceeded the timeout. Called periodically
      /// from wait_any() and drain(). Uses Instant tracking per-worker
      /// (store spawn time alongside child in `active` vec).
      fn kill_timed_out(&mut self) -> Vec<(PathBuf, Vec<u8>, Vec<u8>)> { ... }
  }
  ```
  **Timeout integration**: The pool must track each child's spawn time (add `Instant` to the `active` tuple: `Vec<(PathBuf, Child, Instant)>`). Both `wait_any()` and `drain()` must call `kill_timed_out()` on each polling iteration to kill children that have exceeded `self.timeout`. A killed child returns `BackendCrash` with a timeout message. Without this, a hung worker would block `wait_any()` indefinitely.
- [ ] Default pool size: `std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)` — matches CPU count, falls back to 4
- [ ] Add `parallel_workers: Option<usize>` and `worker_timeout: Duration` fields to `TestRunnerConfig` struct (runner/mod.rs line 43-56). Also update `Default` impl (line 58-68) with `parallel_workers: None` and `worker_timeout: Duration::from_secs(60)`. Note: `worker_timeout` was also mentioned in 02.2 for CLI parsing — both the struct field and CLI parsing are needed. The `#[expect(clippy::struct_excessive_bools)]` is unaffected (new fields are `Option<usize>` and `Duration`, not `bool`).
- [ ] Parse `--parallel-workers=N` CLI flag in `main.rs` (inside "test" match arm, after other flags). `--no-parallel` (already parsed at line 128) sets `config.parallel = false`, which `run_llvm_tests_isolated` uses to set pool size to 1.
- [ ] When `--no-parallel` is specified, run workers sequentially (spawn, wait, parse, next file) — this is the simplest mode and useful for debugging.
- [ ] Top-level orchestrator function:
  ```rust
  pub fn run_llvm_tests_isolated(
      files: &[TestFile],   // TestFile { path: PathBuf } from discovery
      config: &TestRunnerConfig,
      interner: &StringInterner,  // orchestrator's interner (for crash_summary)
  ) -> Vec<FileSummary> {
      let binary = std::env::current_exe().expect("current_exe");
      let pool_size = if config.parallel {
          config.parallel_workers.unwrap_or_else(|| {
              std::thread::available_parallelism()
                  .map(|n| n.get())
                  .unwrap_or(4)
          })
      } else {
          1
      };
      // ... spawn workers through pool, collect results
  }
  ```
- [ ] **File size enforcement**: After implementing 02.1 + 02.2 + 02.3, check line count:
  - `llvm_worker.rs` must stay under 500 lines
  - If pool logic pushes it over, extract `WorkerPool` to `compiler/oric/src/test/runner/worker_pool.rs` and declare `mod worker_pool;` in `llvm_worker.rs`
- [ ] **02.3.T — Tests** (in `llvm_worker/tests.rs` or `worker_pool/tests.rs`):
  - [ ] `test_pool_bounds_concurrency` — pool with `max_workers=2`, submit 5 `sleep 1` children, verify `active.len() <= 2` at all times
  - [ ] `test_pool_sequential_mode` — pool with `max_workers=1`, verify children run one at a time
  - [ ] `test_pool_drain_collects_all` — submit 3 children, call `drain()`, verify 3 results returned
  - [ ] `test_pool_kills_timed_out_worker` — pool with `timeout=200ms`, submit `sleep 999` child, verify `wait_any()` or `drain()` kills it within ~200ms and returns the result (not hang forever)
  - [ ] Verify tests fail before implementing, then pass after

---

## 02.4 Integration with Test Runner Dispatch

**File(s):** `compiler/oric/src/test/runner/mod.rs` (lines 113-126), `compiler/oric/src/commands/test.rs`, `test-all.sh`

Wire the new subprocess orchestrator into the existing test runner dispatch, replacing the in-process `run_file_llvm()` path.

**Hygiene note:** `runner/mod.rs` is currently 572 lines — already over the 500-line limit. This subsection must NOT add net lines. The LLVM sequential comment block (lines 116-120) becomes dead code when the orchestrator handles LLVM dispatch, and should be removed. Target: net reduction of ~5 lines.

- [ ] **[BLOAT]** `runner/mod.rs`:116-120 — Remove the stale LLVM sequential execution comment block. It describes the old in-process approach that the orchestrator replaces.
- [ ] In `runner/mod.rs`, modify `run()` (line 113-126) to intercept LLVM dispatch at the `run()` level. When `config.backend == Backend::LLVM` and `!config.json`, route to orchestrator:
  ```rust
  pub fn run(&self, path: &Path) -> TestSummary {
      let test_files = discover_tests_in(path);

      if self.config.backend == Backend::LLVM && !self.config.json {
          // Orchestrator mode: spawn worker subprocesses per file
          let summaries = llvm_worker::run_llvm_tests_isolated(
              &test_files, &self.config, &self.interner,
          );
          let mut summary = TestSummary::new();
          for file_summary in summaries {
              summary.add_file(file_summary);
          }
          summary
      } else if self.config.parallel && self.config.backend != Backend::LLVM {
          self.run_parallel(&test_files)
      } else {
          self.run_sequential(&test_files)
      }
  }
  ```
  When `config.json == true` (worker mode), it falls through to `run_sequential()` -> `run_file_with_interner()` -> `run_file_llvm()` in-process. This is the worker's execution path.
- [ ] **Self-detection**: `--json` flag distinguishes worker from orchestrator. `json == true` = worker (in-process). `json == false` = orchestrator (spawn workers).
- [ ] Update `commands/test.rs` output formatting to handle `BackendCrash` outcomes:
  - [ ] In `print_file_results()` (line 137): add `TestOutcome::BackendCrash(msg)` match arm with `"  CRASH: {name} - {msg}"` marker
  - [ ] In `print_summary_stats()` (line 159): add `backend_crash` count to the `parts` vector (after `llvm_compile_fail`, same pattern)
  - [ ] In `print_llvm_error_breakdown()` (line 198): include crash count in the output
- [ ] **CRITICAL: Revert the weakened test gate in `test-all.sh`.** With subprocess isolation, the parent process exits normally with code 0 or 1 — never killed by signal. The `ORI_LLVM_CRASHED` escape hatch is unnecessary and must be removed. Specifically:
  - [ ] `test-all.sh` line 220-228: In `parse_ori_results()`, remove the `exit_code > 128` crash branch that sets `${prefix}_CRASHED=1`. With isolation, the orchestrator always exits normally. Remove the `_CRASHED` variable entirely.
  - [ ] `test-all.sh` line 236: Remove `eval "${prefix}_CRASHED=0"` in the non-crash branch
  - [ ] `test-all.sh` line 458: Remove `elif [ "${ORI_LLVM_CRASHED:-0}" -eq 1 ]` display path (shows `CRASHED` status)
  - [ ] `test-all.sh` lines 524-528: Remove `elif [ "${ORI_LLVM_CRASHED:-0}" -eq 1 ]` in `emit_json()` crash suite path
  - [ ] `test-all.sh` line 546: Remove `ANY_CORE_FAILED` variable — with isolation, `ANY_FAILED` is the only check needed
  - [ ] `test-all.sh` lines 556-558: Remove `elif [ "$ANY_CORE_FAILED" -eq 0 ] && [ "${ORI_LLVM_CRASHED:-0}" -eq 1 ]` exit-0 escape hatch
  - [ ] Simplify final status to just `ANY_FAILED` check: exit 0 if `ANY_FAILED == 0`, exit 1 otherwise
  - [ ] **Update `parse_ori_results()` to parse `backend_crash` count**: The new summary line from `print_summary_stats()` includes `N backend crash` — parse it alongside `passed`, `failed`, `skipped`, `llvm compile fail`
  - Satisfies mission criterion: "Weakened test gate reverted"
- [ ] **Backwards compatibility**: `ori test --backend=llvm <file>` without `--json` now spawns a single worker. Slightly slower (subprocess overhead) but provides crash isolation.
- [ ] **02.4.T — Integration tests**:
  - [ ] `test_orchestrator_directory_run` — run `ori test --backend=llvm tests/spec/types/` via `Command::new`, verify parent exits normally and pass counts > 0
  - [ ] `test_orchestrator_survives_crash` — run `ori test --backend=llvm tests/spec/` via `Command::new`, verify exit code is 0 or 1 (NOT 139), and stdout contains `BackendCrash` if any files crash
  - [ ] `test_test_all_no_llvm_crashed_var` — `grep -c ORI_LLVM_CRASHED test-all.sh` returns 0 (gate reversion verified at file level)

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] `llvm_worker.rs` module created with `spawn_llvm_worker`, `collect_worker_result`, `extract_framed_json`, `run_llvm_tests_isolated`
- [ ] Crash detection works: SIGSEGV (signal 11) and SIGABRT (signal 6) produce `BackendCrash` (via `ExitStatus::signal()` on Unix)
- [ ] Non-Unix fallback: `detect_crash` compiles on non-Unix (returns `None`)
- [ ] Timeout detection works: hanging workers killed after configurable timeout
- [ ] Worker pool bounds concurrency to CPU count (no fork-bomb)
- [ ] `--parallel-workers=N` and `--no-parallel` flags work
- [ ] `--worker-timeout=N` flag works (default 60s, parsed in `main.rs`)
- [ ] Test runner dispatch routes LLVM tests through subprocess orchestrator
- [ ] `commands/test.rs` output handles `BackendCrash` outcomes in `print_file_results`, `print_summary_stats`, `print_llvm_error_breakdown`
- [ ] `test-all.sh` parses the updated summary format correctly (including `backend_crash` count)
- [ ] **Weakened test gate reverted**: `ORI_LLVM_CRASHED` exit-0 escape hatch removed from test-all.sh — `grep -c ORI_LLVM_CRASHED test-all.sh` returns 0
- [ ] `ANY_CORE_FAILED` variable removed from test-all.sh — only `ANY_FAILED` used
- [ ] In-process path still works for `--json` mode (worker serving the orchestrator)
- [ ] **TDD verified**: all tests written before implementation
- [ ] Unit tests: 7 spawn/extract tests (02.1.T), 7 crash/timeout tests (02.2.T), 4 pool tests (02.3.T)
- [ ] Integration tests: 3 end-to-end tests (02.4.T)
- [ ] **[BLOAT] runner/mod.rs**: net zero or negative line change (remove stale LLVM comment block)
- [ ] `llvm_worker.rs` under 500 lines (extract pool to `worker_pool.rs` submodule if needed)
- [ ] Debug AND release builds pass: `timeout 150 cargo test` (debug) and `timeout 150 cargo test --release` (release) both succeed
- [ ] `timeout 150 ./test-all.sh` passes — LLVM backend no longer crashes the parent
- [ ] `./clippy-all.sh` passes
- [ ] All 2098+ AOT tests pass (no regressions)
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 02` returns 0 annotations
- [ ] **Plan sync** — update plan metadata
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review` passed
- [ ] `/improve-tooling` retrospective completed — MANDATORY at section close, after both reviews are clean. Reflect on the section's debugging journey (which `diagnostics/` scripts you ran, which command sequences you repeated, where you added ad-hoc `dbg!`/`tracing` calls, where output was hard to interpret) and identify any tool/log/diagnostic improvement that would have made this section materially easier OR that would help the next section touching this area. Implement every accepted improvement NOW (zero deferral) and commit each via SEPARATE `/commit-push`. The retrospective is mandatory even when nothing felt painful — that is exactly when blind spots accumulate. See `.claude/skills/improve-tooling/SKILL.md` "Retrospective Mode" for the full protocol.

**Exit Criteria:** `ori test --backend=llvm tests/spec/` completes without crashing the parent process. Workers that crash (SIGSEGV) produce `BackendCrash` outcomes that appear in the summary and cause exit code 1. `./test-all.sh` reports the LLVM backend line with pass/fail/crash counts instead of `CRASHED`. The `ORI_LLVM_CRASHED` exit-0 escape hatch is removed from `test-all.sh` — crashes are real failures that block the gate. All AOT integration tests pass unchanged. Total wall-clock time for LLVM spec tests is within 2x of the current sequential in-process time.
