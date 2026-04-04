---
section: "02"
title: "Subprocess Orchestrator"
status: not-started
reviewed: false
goal: "Replace in-process LLVM test execution with subprocess-per-file isolation — worker crashes contained, results parsed from JSON"
success_criteria:
  - "LLVM backend spec test run completes without crashing the parent process"
  - "Worker SIGSEGV/SIGABRT produces BackendCrash outcomes (detected via ExitStatus::signal() on Unix)"
  - "Worker timeout produces BackendCrash with timeout message"
  - "Results match: subprocess-based run produces same pass/fail/skip/lcfail counts as in-process run (for non-crashing files)"
  - "Bounded concurrency: worker pool limited to CPU count (no fork-bomb)"
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

**File(s):** `compiler/oric/src/test/runner/llvm_backend.rs` (refactor), new `compiler/oric/src/test/runner/llvm_worker.rs`

Extract the subprocess orchestration logic into a new `llvm_worker.rs` module. The existing `llvm_backend.rs` contains the in-process compilation pipeline (545 lines) -- this should remain as the worker's execution path (used when `ori test --backend=llvm --json` is invoked on a single file). The new module handles spawning and result collection.

- [ ] Create `compiler/oric/src/test/runner/llvm_worker.rs` — the orchestrator module
- [ ] Declare `mod llvm_worker;` in `runner/mod.rs`
- [ ] Binary path resolution: `current_exe()` is resolved once in `run_llvm_tests_isolated()` (see 02.3) and passed to all worker spawn calls. No per-file resolution. Note: `current_exe()` returns the actual binary path (e.g., `target/release/ori` when run from test-all.sh, or `target/debug/ori` in dev). This works correctly because the spawned worker IS the same binary. <!-- reviewed: feasibility note — current_exe() verified correct for self-invocation -->
- [ ] Implement `spawn_llvm_worker(binary: &Path, file: &Path, config: &TestRunnerConfig) -> std::io::Result<std::process::Child>`:
  ```rust
  fn spawn_llvm_worker(
      binary: &Path,
      file: &Path,
      config: &TestRunnerConfig,
  ) -> std::io::Result<Child> {
      Command::new(binary)
          .arg("test")
          .arg("--backend=llvm")
          .arg("--json")
          .arg(file)
          .stdout(Stdio::piped())
          .stderr(Stdio::piped())
          .spawn()
  }
  ```
- [ ] Implement `collect_worker_result(child: Child, file: &Path, interner: &StringInterner) -> FileSummary`: <!-- reviewed: feasibility fix — must extract sentinel-framed JSON, not parse raw stdout -->
  - Wait for child to exit (with timeout -- see 02.2)
  - **Signal death** (Unix: `status.signal().is_some()`): worker crashed -> `crash_summary()` (see 02.2)
  - **Exit code 0 or 1**: extract sentinel-framed JSON from stdout (scan for `---ORI_JSON_BEGIN---` / `---ORI_JSON_END---`, take content between them), parse as `JsonFileSummary` -> `FileSummary`. Anything outside the sentinel frame (Ori `print()` output) is discarded.
  - **Exit code 2**: no tests found -> empty `FileSummary` with `results: []`
  - **JSON parse failure** (no sentinel frame found, or content between sentinels is malformed -- worker crashed before emitting JSON): fall back to `crash_summary()` with message "worker exited {code} with no JSON output"
- [ ] Implement `extract_framed_json(stdout: &str) -> Option<&str>` -- scans for sentinel markers, returns the JSON content between them. Returns `None` if markers are missing (worker crashed before emission).
- [ ] Implement `run_file_llvm_isolated(file: &Path, binary: &Path, config: &TestRunnerConfig, interner: &StringInterner) -> FileSummary` -- the top-level per-file orchestrator function. Note: the `interner` parameter is used only for constructing `crash_summary()` synthetic test results and for `into_file_summary()` re-interning. The worker process creates its own interner internally. <!-- reviewed: feasibility note — clarify interner usage across process boundary -->
- [ ] Rust unit test: `spawn_llvm_worker` with a known-good test file produces exit 0 and sentinel-framed JSON stdout. Test in `compiler/oric/src/test/runner/llvm_worker/tests.rs`
- [ ] Rust unit test: `spawn_llvm_worker` with a nonexistent file produces exit 1 (or 2) and parseable output (or no sentinel frame)

---

## 02.2 Crash and Timeout Detection

**File(s):** `compiler/oric/src/test/runner/llvm_worker.rs`

Handle the two failure modes that in-process execution can't survive: worker signal death and worker hangs.

- [ ] **Crash detection**: In `collect_worker_result`, check for signal death: <!-- reviewed: accuracy fix — status.code() returns None for signal-killed processes on Unix, not 128+N. The 128+N convention is a shell feature (bash), not an OS/Rust feature. -->
  - On Unix: use `status.signal()` from `std::os::unix::process::ExitStatusExt`. If `signal()` returns `Some(sig)`, the process was killed by that signal (e.g., SIGSEGV = 11, SIGABRT = 6). `status.code()` returns `None` in this case.
  - On non-Unix: signal detection is not available via `ExitStatus`. Use `status.code()` and treat unexpected non-zero codes as potential crashes.
  - Create `BackendCrash` outcomes for all tests in the file:
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
  ```
- [ ] **Crash result construction**: When a crash is detected, produce a `FileSummary` where every test in the file gets `BackendCrash(message)`. The orchestrator doesn't know which tests were in the file (it didn't parse it), so use a single synthetic test result:
  ```rust
  fn crash_summary(file: &Path, message: String, interner: &StringInterner) -> FileSummary {
      FileSummary {
          path: file.to_owned(),
          results: vec![TestResult {
              name: interner.intern("llvm_backend_crash"),
              targets: vec![],
              outcome: TestOutcome::BackendCrash(message),
              duration: Duration::ZERO,
          }],
          passed: 0,
          failed: 0,
          skipped: 0,
          llvm_compile_fail: 0,
          backend_crash: 1,
          ..Default::default()
      }
  }
  ```
- [ ] **Timeout detection**: Use `try_wait()` polling with configurable timeout (default 60s per file):
  ```rust
  fn wait_with_timeout(
      child: &mut Child,
      timeout: Duration,
  ) -> Result<ExitStatus, TimeoutError> {
      let start = Instant::now();
      loop {
          match child.try_wait()? {
              Some(status) => return Ok(status),
              None if start.elapsed() > timeout => {
                  child.kill()?;
                  child.wait()?; // reap zombie
                  return Err(TimeoutError {
                      elapsed: start.elapsed(),
                  });
              }
              None => std::thread::sleep(Duration::from_millis(50)),
          }
      }
  }
  ```
- [ ] **Timeout configuration**: Add `worker_timeout: Duration` to `TestRunnerConfig` with default 60 seconds. Allow override via `--worker-timeout=N` CLI flag (parsed in `main.rs`).
- [ ] **Capture stderr on crash**: When a worker crashes, include the last few lines of stderr in the `BackendCrash` message for diagnostic context: <!-- reviewed: accuracy fix — lines().rev().take(5).collect() needs a separator -->
  ```rust
  let stderr_output = String::from_utf8_lossy(&child_stderr);
  let last_lines: String = stderr_output
      .lines()
      .rev()
      .take(5)
      .collect::<Vec<_>>()
      .into_iter()
      .rev() // restore original order
      .collect::<Vec<_>>()
      .join("\n");
  ```
- [ ] Rust unit test: simulate crash by spawning a process that calls `std::process::abort()` — verify `detect_crash` returns the correct signal. Use a helper script or `Command::new("sh").arg("-c").arg("kill -11 $$")`.
- [ ] Rust unit test: simulate timeout by spawning `sleep 999` — verify `wait_with_timeout` returns `TimeoutError` after the configured duration.

---

## 02.3 Bounded Worker Pool

**File(s):** `compiler/oric/src/test/runner/llvm_worker.rs`

The current LLVM backend runs files sequentially because LLVM context creation contends on global state within a single process. With subprocess isolation, each worker has its own address space — parallelism is safe. Use a bounded pool to limit concurrency to ~CPU count.

- [ ] Implement a simple bounded worker pool: <!-- reviewed: feasibility fix — clarified Child ownership and wait_any polling pattern -->
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
  }
  ```
- [ ] Default pool size: `std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4)` — matches CPU count, falls back to 4
- [ ] Add `parallel_workers: Option<usize>` and `worker_timeout: Duration` fields to `TestRunnerConfig` (runner/mod.rs). Parse `--parallel-workers=N` CLI flag in `main.rs` to populate `parallel_workers`. `--no-parallel` sets pool size to 1 (sequential, for debugging). <!-- reviewed: accuracy fix — must add fields to config struct, not just CLI flags -->
- [ ] When `--no-parallel` is specified, run workers sequentially (spawn, wait, parse, next file) — this is the simplest mode and useful for debugging.
- [ ] Top-level orchestrator function:
  <!-- reviewed: accuracy fix — use TestFile not PathBuf to match discovery API -->
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
- [ ] File size limit per section: `llvm_worker.rs` must stay under 500 lines. If the pool logic exceeds ~200 lines, extract to a `worker_pool.rs` submodule.
- [ ] Rust unit test: pool with `max_workers=2` and 5 files never has more than 2 concurrent children (verify via `active.len()` assertions in submit).

---

## 02.4 Integration with Test Runner Dispatch

**File(s):** `compiler/oric/src/test/runner/mod.rs`, `compiler/oric/src/commands/test.rs`

Wire the new subprocess orchestrator into the existing test runner dispatch, replacing the in-process `run_file_llvm()` path.

- [ ] In `runner/mod.rs`, intercept LLVM dispatch at the `run()` method level (line 113-126), NOT inside `run_file_with_interner()`. When `config.backend == Backend::LLVM` and `!config.json`, route the entire file list to the new orchestrator instead of calling `run_sequential()`/`run_file_with_interner()` per file: <!-- reviewed: accuracy fix — dispatch must happen at run() level, not inside per-file function -->
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
  The `config.json == true` path (worker mode) falls through to `run_sequential()`, which calls `run_file_with_interner()` -> `run_file_llvm()` in-process. This is the worker's execution path.
- [ ] **Self-detection**: The `--json` flag distinguishes worker from orchestrator. When `config.json` is true, the process is a worker — run in-process (the existing `run_file_llvm()` path). When `config.json` is false, the process is the orchestrator — spawn workers.
- [ ] Update `commands/test.rs` output formatting to handle `BackendCrash` outcomes:
  - In `print_file_results()`: print crashed files with a distinct marker (e.g., `CRASH` instead of `FAIL`)
  - In `print_summary_stats()`: include `backend_crash` count in the summary line
  - In `print_llvm_error_breakdown()`: include crash count
- [ ] Update `test-all.sh` `parse_ori_results()` function to parse `backend_crash`/`crashed` counts from the summary line (currently parses `passed`, `failed`, `skipped`, `llvm compile fail`). Also update the crash detection logic -- currently `test-all.sh` checks `exit_code > 128` to detect crashes (lines 182-197); with subprocess isolation, the parent process will exit normally with code 0 or 1 instead of being killed by a signal. Remove or adjust the `ORI_LLVM_CRASHED` path. <!-- reviewed: accuracy fix — described actual test-all.sh crash detection logic -->
- [ ] **Backwards compatibility**: `ori test --backend=llvm <file>` without `--json` now spawns a single worker. This is slightly slower (subprocess overhead) but provides crash isolation. For files that don't crash, behavior is identical. For files that crash, the parent survives and reports `BackendCrash`.
- [ ] Integration test: run `ori test --backend=llvm tests/spec/types/` (a directory) — verify it spawns workers and collects results. Compare pass/fail counts against interpreter baseline.
- [ ] Integration test: run `ori test --backend=llvm tests/spec/` (the full spec suite) — verify the parent process survives even when workers crash. The exit code should be 1 (failures from BackendCrash), not 139 (killed by SIGSEGV).

---

## 02.R Third Party Review Findings

- None.

---

## 02.N Completion Checklist

- [ ] `llvm_worker.rs` module created with `spawn_llvm_worker`, `collect_worker_result`, `run_llvm_tests_isolated`
- [ ] Crash detection works: SIGSEGV (signal 11) and SIGABRT (signal 6) produce `BackendCrash` (via `ExitStatus::signal()` on Unix) <!-- reviewed: accuracy fix — signal numbers, not exit codes -->
- [ ] Timeout detection works: hanging workers killed after configurable timeout
- [ ] Worker pool bounds concurrency to CPU count (no fork-bomb)
- [ ] `--parallel-workers=N` and `--no-parallel` flags work
- [ ] `--worker-timeout=N` flag works
- [ ] Test runner dispatch routes LLVM tests through subprocess orchestrator
- [ ] `commands/test.rs` output handles `BackendCrash` outcomes
- [ ] `test-all.sh` parses the updated summary format correctly
- [ ] In-process path still works for `--json` mode (worker serving the orchestrator)
- [ ] `timeout 150 ./test-all.sh` passes — LLVM backend no longer crashes the parent
- [ ] `./clippy-all.sh` passes
- [ ] All 2098+ AOT tests pass (no regressions)
- [ ] llvm_worker.rs under 500 lines (extract pool to submodule if needed)
- [ ] Plan annotation cleanup: `bash .claude/skills/impl-hygiene-review/plan-annotations.sh --plan 02` returns 0 annotations
- [ ] **Plan sync** — update plan metadata
- [ ] `/tpr-review` passed
- [ ] `/impl-hygiene-review last commit` passed

**Exit Criteria:** `ori test --backend=llvm tests/spec/` completes without crashing the parent process. Workers that crash (SIGSEGV) produce `BackendCrash` outcomes that appear in the summary and cause exit code 1. `./test-all.sh` reports the LLVM backend line with pass/fail/crash counts instead of `CRASHED`. All AOT integration tests pass unchanged. Total wall-clock time for LLVM spec tests is within 2x of the current sequential in-process time.
