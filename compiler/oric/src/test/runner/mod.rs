//! Test execution engine.
//!
//! Runs tests from parsed modules and collects results.

#[cfg(feature = "llvm")]
mod arc_lowering;
mod file_run;
#[cfg(feature = "llvm")]
mod imported_mono;
#[cfg(feature = "llvm")]
mod incremental;
#[cfg(feature = "llvm")]
mod llvm_backend;
#[cfg(feature = "llvm")]
mod subprocess;
mod test_execution;
#[cfg(feature = "llvm")]
mod worker;

#[cfg(feature = "llvm")]
pub use worker::run_worker;

/// Serializes tests that read or mutate the process environment (the
/// worker-protocol token scrub + spawn-env pins) — env vars are process
/// globals, so concurrent test threads would race.
#[cfg(all(test, feature = "llvm"))]
pub(crate) static ENV_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

use std::path::{Path, PathBuf};
use std::time::Instant;

use rayon::prelude::*;

use crate::db::{CompilerDb, Db};
use crate::input::SourceFile;
use crate::ir::TestDef;
use crate::query::parsed;

use super::change_detection::TestRunCache;
use super::discovery::{discover_tests_in_all, TestFile};
use super::protocol;
use super::result::{CoverageReport, FileSummary, TestResult, TestSummary};

/// Extract a human-readable message from a `catch_unwind` payload.
pub(super) fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else {
        "panic with non-string payload".to_string()
    }
}

/// Backend for test execution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    /// Tree-walking interpreter (default).
    #[default]
    Interpreter,
    /// LLVM JIT compiler.
    LLVM,
}

/// Output format for the test-result summary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text summary (default).
    #[default]
    Text,
    /// Machine-readable JSON object carrying the full summary: aggregate
    /// counts plus a `files` array of per-file/per-test results and durations.
    Json,
}

/// Configuration for the test runner.
#[derive(Clone, Debug)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "Config struct: each bool controls an independent flag"
)]
pub struct TestRunnerConfig {
    /// Filter tests by name pattern (substring match).
    pub filter: Option<String>,
    /// Enable verbose output.
    pub verbose: bool,
    /// Run tests in parallel.
    pub parallel: bool,
    /// Generate coverage report.
    pub coverage: bool,
    /// Backend to use for execution.
    pub backend: Backend,
    /// Enable incremental test execution (skip tests whose targets are unchanged).
    pub incremental: bool,
    /// Worker mode: run in-process and stream the per-test line protocol
    /// (`test::protocol`) to stdout. Set by the hidden `--__worker` flag the
    /// parent runner passes when isolating LLVM test files in subprocesses.
    pub worker_protocol: bool,
    /// Per-spawn protocol nonce stamped onto every emitted protocol line
    /// (forgery guard — see `test::protocol` module docs). Populated in
    /// worker mode from `ORI_TEST_PROTOCOL_TOKEN`; `None` otherwise.
    pub protocol_token: Option<String>,
    /// Worker mode: names of tests whose targets the PARENT's incremental
    /// cache proved unchanged. Set by the hidden `--__skip-unchanged=` flag
    /// the parent passes when spawning a worker; the worker reports each as
    /// `SkippedUnchanged` without running it. The parent owns the incremental
    /// cache — a worker never consults a cache of its own.
    pub skip_unchanged: Vec<String>,
    /// Summary output format. `Text` (default) prints the human-readable
    /// summary; `Json` emits the full machine-readable summary object.
    pub format: OutputFormat,
}

impl Default for TestRunnerConfig {
    fn default() -> Self {
        TestRunnerConfig {
            filter: None,
            verbose: false,
            parallel: true,
            coverage: false,
            backend: Backend::Interpreter,
            incremental: false,
            worker_protocol: false,
            protocol_token: None,
            skip_unchanged: Vec::new(),
            format: OutputFormat::Text,
        }
    }
}

/// On-disk incremental-cache path for this config, if persistence applies.
///
/// `None` unless incremental mode is on, the run is NOT a worker (the parent
/// owns the cache file exclusively), and `ORI_TEST_INCREMENTAL_CACHE` names a
/// non-empty path.
fn incremental_cache_path(config: &TestRunnerConfig) -> Option<PathBuf> {
    if !config.incremental || config.worker_protocol {
        return None;
    }
    std::env::var(crate::debug_flags::ORI_TEST_INCREMENTAL_CACHE)
        .ok()
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

/// Test runner.
///
/// The test runner maintains a shared `StringInterner` which is used by all files.
/// Each file gets its own `CompilerDb` for Salsa query storage, but they all share
/// the same interner via Arc. This means all `Name` values are valid and comparable
/// across files, modeling how real Ori projects work: one compilation unit with
/// one shared interner.
pub struct TestRunner {
    config: TestRunnerConfig,
    /// Shared interner - all files use the same interner for comparable Name values.
    interner: crate::ir::SharedInterner,
    /// Cross-run cache for incremental test execution. Thread-safe for parallel runs.
    cache: parking_lot::Mutex<TestRunCache>,
    /// Run-start snapshot of the runner binary (a stable inode immune to a
    /// concurrent rebuild replacing it mid-run). `None` in worker mode (a worker
    /// never spawns sub-workers) and when snapshotting failed at start (the
    /// per-file path then falls back to `current_exe()`). Held for the whole
    /// run so its `Drop` cleanup fires only at run end.
    snapshot: Option<subprocess::snapshot::ExeSnapshot>,
}

impl TestRunner {
    /// Create a new test runner with default config.
    pub fn new() -> Self {
        Self::with_config(TestRunnerConfig::default())
    }

    /// Create a test runner with custom config.
    ///
    /// When incremental mode is on and `ORI_TEST_INCREMENTAL_CACHE` names a
    /// cache file, the per-function body-hash snapshots are loaded from it so
    /// unchanged-test skipping works across runner instances. The parent owns
    /// the cache file exclusively — worker mode never loads (or saves) it.
    pub fn with_config(config: TestRunnerConfig) -> Self {
        let interner = crate::ir::SharedInterner::new();
        let cache = match incremental_cache_path(&config) {
            Some(path) => TestRunCache::load_from(&path, &interner),
            None => TestRunCache::new(),
        };
        // Snapshot the runner binary once at parent-runner start so per-file
        // worker spawns survive a concurrent rebuild replacing the binary inode.
        // A worker (`worker_protocol`) never spawns sub-workers, so it never
        // snapshots. A snapshot failure degrades gracefully: `run_file_isolated`
        // falls back to `current_exe()` per file (the pre-snapshot behavior).
        let snapshot = if config.worker_protocol {
            None
        } else {
            std::env::current_exe()
                .ok()
                .and_then(|exe| subprocess::snapshot::snapshot_exe(&exe).ok())
        };
        TestRunner {
            config,
            interner,
            cache: parking_lot::Mutex::new(cache),
            snapshot,
        }
    }

    /// Save the incremental cache to its configured on-disk path, if any.
    fn persist_incremental_cache(&self) {
        let Some(path) = incremental_cache_path(&self.config) else {
            return;
        };
        if let Err(e) = self.cache.lock().save_to(&path, &self.interner) {
            tracing::warn!(
                "failed to save incremental test cache to {}: {e}",
                path.display()
            );
        }
    }

    /// Get the string interner for looking up `Name` values.
    ///
    /// Use this to convert `Name` to `&str` when displaying test results.
    pub fn interner(&self) -> &crate::ir::StringInterner {
        &self.interner
    }

    /// Whether this runner holds a binary snapshot (parent runners snapshot;
    /// workers never do). Test-only accessor for the worker-mode snapshot pin.
    #[cfg(test)]
    pub(in crate::test::runner) fn snapshot_is_active(&self) -> bool {
        self.snapshot.is_some()
    }

    /// Run all tests in a path (file or directory).
    pub fn run(&self, path: &Path) -> TestSummary {
        self.run_paths(std::slice::from_ref(&path.to_path_buf()))
    }

    /// Run all tests across multiple paths (files and/or directories),
    /// deduplicated, in one combined summary.
    pub fn run_paths(&self, paths: &[PathBuf]) -> TestSummary {
        let summary = self.run_discovered(paths);
        self.persist_incremental_cache();
        summary
    }

    /// Discover test files and dispatch them to the backend-appropriate
    /// execution strategy.
    fn run_discovered(&self, paths: &[PathBuf]) -> TestSummary {
        let test_files = discover_tests_in_all(paths);

        // LLVM backend: per-file subprocess isolation. A crashing test
        // (SIGSEGV in JIT'd code, runtime abort) kills only that file's
        // worker; the parent classifies the death, counts the in-flight test
        // as failed, and continues with the next file. Worker mode itself
        // (`worker_protocol`) runs the file in-process below.
        #[cfg(feature = "llvm")]
        if self.config.backend == Backend::LLVM && !self.config.worker_protocol {
            return self.run_llvm_isolated(&test_files);
        }

        // LLVM backend must run sequentially due to context creation contention.
        // LLVM's Context::create() has global lock contention - when rayon spawns
        // many parallel tasks that each create an LLVM context, they serialize at
        // the LLVM library level despite appearing parallel. Sequential execution
        // is actually faster (1-2s vs 57s) and matches Roc/rustc patterns.
        if self.config.parallel && self.config.backend != Backend::LLVM {
            self.run_parallel(&test_files)
        } else {
            self.run_sequential(&test_files)
        }
    }

    /// Run LLVM-backend tests with one worker subprocess per test file.
    ///
    /// Under `--incremental`, the parent computes per-file skip decisions
    /// against its own cache BEFORE spawning: a fully-unchanged file is
    /// accounted parent-side without a worker spawn; a partially-unchanged
    /// file's skip decisions are forwarded to the worker, which reports the
    /// skipped tests as `SkippedUnchanged` without running them.
    #[cfg(feature = "llvm")]
    fn run_llvm_isolated(&self, files: &[TestFile]) -> TestSummary {
        let mut summary = TestSummary::new();
        let start = Instant::now();

        let snapshot_path = self
            .snapshot
            .as_ref()
            .map(subprocess::snapshot::ExeSnapshot::path);

        for file in files {
            let skip_plan = if self.config.incremental {
                incremental::plan_file_skips(&file.path, &self.interner, &self.config, &self.cache)
            } else {
                incremental::SkipPlan::Spawn {
                    skip_names: Vec::new(),
                }
            };
            let mut file_summary = match skip_plan {
                incremental::SkipPlan::Synthesized(synthesized) => synthesized,
                incremental::SkipPlan::Spawn { skip_names } => subprocess::run_file_isolated(
                    &file.path,
                    &self.interner,
                    &self.config,
                    &skip_names,
                    snapshot_path,
                ),
            };
            // Collapse the worker-binary-replaced cascade: when the binary was
            // replaced or lost mid-run (even the snapshot is gone), attach ONE
            // clear diagnostic and abort the run instead of spawning every
            // remaining file and recording N per-file `failed to spawn` errors.
            let binary_replaced = file_summary.binary_replaced;
            if binary_replaced {
                file_summary.add_error(
                    "test binary was replaced or lost during the run \
                     (concurrent rebuild?) — rerun"
                        .to_string(),
                );
            }
            summary.add_file(file_summary);
            if binary_replaced {
                break;
            }
        }

        summary.duration = start.elapsed();
        summary
    }

    /// Run tests sequentially.
    fn run_sequential(&self, files: &[TestFile]) -> TestSummary {
        let mut summary = TestSummary::new();
        let start = Instant::now();

        for file in files {
            let file_summary =
                Self::run_file_with_interner(&file.path, &self.interner, &self.config, &self.cache);
            summary.add_file(file_summary);
        }

        summary.duration = start.elapsed();
        summary
    }

    /// Run tests in parallel using a scoped rayon thread pool.
    ///
    /// Each parallel task creates its own `CompilerDb` but shares the interner.
    /// This is thread-safe because `SharedInterner` is `Arc<StringInterner>`
    /// and `StringInterner` uses `RwLock` per shard for concurrent access.
    ///
    /// Uses `build_scoped` to create a thread pool that's guaranteed to be
    /// cleaned up before this function returns. This avoids the hang that
    /// occurs with rayon's global pool atexit handlers.
    fn run_parallel(&self, files: &[TestFile]) -> TestSummary {
        let start = Instant::now();

        // Clone the shared interner and config for the parallel closure.
        // SharedInterner is Arc-wrapped, so this is cheap.
        let interner = self.interner.clone();
        let config = self.config.clone();
        let cache = &self.cache;

        // Use build_scoped to create a thread pool that's cleaned up before returning.
        // This avoids atexit handler hangs that occur with the global rayon pool.
        //
        // Explicit stack size ensures sufficient space for deep recursion in type
        // inference and evaluation. Default thread stacks vary by platform (512KB
        // on macOS, 1MB on Windows) and can overflow on complex type expressions.
        // The stacker crate handles growth dynamically, but a larger initial stack
        // reduces the frequency of mmap-based growth on worker threads.
        //
        // 32 MiB accommodates debug builds on Windows/macOS where unoptimized frames
        // are much larger (no inlining, no frame optimization) and the Salsa memo
        // verification + tracing spans + type checking pipeline can exhaust smaller
        // stacks. rustc itself uses 16 MiB for release builds; debug CI needs more.
        let file_summaries = rayon::ThreadPoolBuilder::new()
            .stack_size(32 * 1024 * 1024) // 32 MiB: debug builds + Salsa + tracing overhead
            .build_scoped(
                // Thread initialization wrapper - just run the thread
                rayon::ThreadBuilder::run,
                // Work to execute in the pool
                |pool| {
                    pool.install(|| {
                        files
                            .par_iter()
                            .map(|file| {
                                Self::run_file_with_interner(&file.path, &interner, &config, cache)
                            })
                            .collect::<Vec<_>>()
                    })
                },
            )
            .unwrap_or_else(|e| {
                tracing::warn!("failed to create thread pool ({e}), running sequentially");
                files
                    .iter()
                    .map(|file| Self::run_file_with_interner(&file.path, &interner, &config, cache))
                    .collect()
            });

        let mut summary = TestSummary::new();
        for file_summary in file_summaries {
            summary.add_file(file_summary);
        }

        summary.duration = start.elapsed();
        summary
    }

    /// Run all tests in a single file (instance method for convenience).
    fn run_file(&self, path: &Path) -> FileSummary {
        Self::run_file_with_interner(path, &self.interner, &self.config, &self.cache)
    }

    /// Whether a test passes the configured name filter (substring match).
    pub(super) fn test_passes_filter(
        test: &TestDef,
        config: &TestRunnerConfig,
        interner: &crate::ir::StringInterner,
    ) -> bool {
        config
            .filter
            .as_ref()
            .is_none_or(|f| interner.lookup(test.name).contains(f.as_str()))
    }

    /// The `#skip(backend: "<name>", reason: ...)` reason when `test` names the
    /// CURRENTLY-executing backend, else `None`.
    ///
    /// `BackendSkip` carries the `ori_ir::TestBackend` open-set enum so the
    /// runner's own `Backend` type stays decoupled; this is the single mapping
    /// arm between the two. A test naming the OTHER backend still runs here.
    pub(super) fn backend_skip_reason(test: &TestDef, backend: Backend) -> Option<crate::ir::Name> {
        let current = match backend {
            Backend::Interpreter => ori_ir::TestBackend::Interpreter,
            Backend::LLVM => ori_ir::TestBackend::Llvm,
        };
        test.skip_backends
            .iter()
            .find(|s| s.backend == current)
            .map(|s| s.reason)
    }

    /// The protocol-emission token: present only in worker mode with the
    /// parent-provided non-empty per-spawn nonce.
    fn emit_token(config: &TestRunnerConfig) -> Option<&str> {
        if !config.worker_protocol {
            return None;
        }
        config
            .protocol_token
            .as_deref()
            .filter(|token| !token.is_empty())
    }

    /// Worker protocol: announce every filter-passing test up front so the
    /// parent can account for tests lost to a mid-file worker crash.
    /// No-op outside worker mode.
    fn protocol_plan<'t>(
        tests: impl Iterator<Item = &'t TestDef>,
        config: &TestRunnerConfig,
        interner: &crate::ir::StringInterner,
    ) {
        let Some(token) = Self::emit_token(config) else {
            return;
        };
        for test in tests {
            if Self::test_passes_filter(test, config, interner) {
                protocol::emit_plan(token, interner.lookup(test.name));
            }
        }
    }

    /// Worker protocol: emit a `start` record. No-op outside worker mode.
    pub(super) fn protocol_start(
        name: crate::ir::Name,
        config: &TestRunnerConfig,
        interner: &crate::ir::StringInterner,
    ) {
        if let Some(token) = Self::emit_token(config) {
            protocol::emit_start(token, interner.lookup(name));
        }
    }

    /// Worker protocol: emit a `result` record. No-op outside worker mode.
    pub(super) fn protocol_result(
        result: &TestResult,
        config: &TestRunnerConfig,
        interner: &crate::ir::StringInterner,
    ) {
        if let Some(token) = Self::emit_token(config) {
            protocol::emit_result(
                token,
                interner.lookup(result.name),
                &result.outcome,
                result.duration,
            );
        }
    }
}

impl Default for TestRunner {
    fn default() -> Self {
        TestRunner::new()
    }
}

impl TestRunner {
    /// Generate a coverage report for a path.
    pub fn coverage_report(&self, path: &Path) -> CoverageReport {
        self.coverage_report_paths(std::slice::from_ref(&path.to_path_buf()))
    }

    /// Generate a combined coverage report across multiple paths,
    /// deduplicated per `discover_tests_in_all`.
    pub fn coverage_report_paths(&self, paths: &[PathBuf]) -> CoverageReport {
        let test_files = discover_tests_in_all(paths);
        let mut report = CoverageReport::new();

        for file in &test_files {
            self.add_file_coverage(&file.path, &mut report);
        }

        report
    }

    /// Add coverage info for a single file.
    fn add_file_coverage(&self, path: &Path, report: &mut CoverageReport) {
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };

        // Create a fresh CompilerDb with the shared interner
        let db = CompilerDb::with_interner(self.interner.clone());
        let file = SourceFile::new(&db, path.to_path_buf(), content);
        let parse_result = parsed(&db, file);

        if parse_result.has_errors() {
            return;
        }

        let interner = db.interner();
        let main_name = interner.intern("main");

        // Build map of function -> tests that target it
        let mut test_map: rustc_hash::FxHashMap<crate::ir::Name, Vec<crate::ir::Name>> =
            rustc_hash::FxHashMap::default();

        for test in &parse_result.module.tests {
            for target in &test.targets {
                test_map.entry(*target).or_default().push(test.name);
            }
        }

        // Add coverage for each function (except main)
        for func in &parse_result.module.functions {
            if func.name == main_name {
                continue;
            }
            let test_names = test_map.get(&func.name).cloned().unwrap_or_default();
            report.add_function(func.name, test_names);
        }
    }
}

/// Convenience function to run all tests in a path.
pub fn run_tests(path: &Path) -> TestSummary {
    TestRunner::new().run(path)
}

/// Convenience function to run tests in a single file.
pub fn run_test_file(path: &Path) -> FileSummary {
    TestRunner::new().run_file(path)
}

#[cfg(test)]
#[expect(clippy::unwrap_used, reason = "Tests use unwrap for brevity")]
mod tests;
