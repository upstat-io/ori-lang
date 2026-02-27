//! Test execution engine.
//!
//! Runs tests from parsed modules and collects results.

#[cfg(feature = "llvm")]
mod arc_lowering;
#[cfg(feature = "llvm")]
mod llvm_backend;
mod test_execution;

use std::path::Path;
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::db::{CompilerDb, Db};
use crate::eval::Evaluator;
use crate::input::SourceFile;
use crate::ir::TestDef;
use crate::query::{parsed, typed, typed_pool};

use super::change_detection::{FunctionChangeMap, TestRunCache, TestTargetIndex};
use super::discovery::{discover_tests_in, TestFile};
use super::result::TestOutcome;
use super::result::{CoverageReport, FileSummary, TestResult, TestSummary};

/// Backend for test execution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    /// Tree-walking interpreter (default).
    #[default]
    Interpreter,
    /// LLVM JIT compiler.
    LLVM,
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
        }
    }
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
}

impl TestRunner {
    /// Create a new test runner with default config.
    pub fn new() -> Self {
        TestRunner {
            config: TestRunnerConfig::default(),
            interner: crate::ir::SharedInterner::new(),
            cache: parking_lot::Mutex::new(TestRunCache::new()),
        }
    }

    /// Create a test runner with custom config.
    pub fn with_config(config: TestRunnerConfig) -> Self {
        TestRunner {
            config,
            interner: crate::ir::SharedInterner::new(),
            cache: parking_lot::Mutex::new(TestRunCache::new()),
        }
    }

    /// Get the string interner for looking up `Name` values.
    ///
    /// Use this to convert `Name` to `&str` when displaying test results.
    pub fn interner(&self) -> &crate::ir::StringInterner {
        &self.interner
    }

    /// Run all tests in a path (file or directory).
    pub fn run(&self, path: &Path) -> TestSummary {
        let test_files = discover_tests_in(path);

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

    /// Run all tests in a single file with a shared interner.
    ///
    /// This is the core implementation that creates a fresh `CompilerDb` per file
    /// while sharing the interner across all files. This allows parallel execution
    /// (each file gets its own Salsa query cache) while maintaining `Name` comparability
    /// (all `Name` values come from the same interner).
    #[expect(
        clippy::too_many_lines,
        reason = "multi-phase test file execution pipeline"
    )]
    fn run_file_with_interner(
        path: &Path,
        interner: &crate::ir::SharedInterner,
        config: &TestRunnerConfig,
        cache: &parking_lot::Mutex<TestRunCache>,
    ) -> FileSummary {
        let mut summary = FileSummary::new(path.to_path_buf());

        // Read and parse the file
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                summary.add_error(format!("Failed to read file: {e}"));
                return summary;
            }
        };

        // Create a fresh CompilerDb with the shared interner.
        // Each file gets its own Salsa query cache, but all share the same interner
        // so Name values are comparable across files.
        let db = CompilerDb::with_interner(interner.clone());
        let file = SourceFile::new(&db, path.to_path_buf(), content);
        // Retrieve source from SourceFile for error matching (borrows from Salsa).
        // No clone needed: all subsequent `db` usage is shared borrows, so the
        // `&String` returned by `file.text(&db)` remains valid.
        let source = file.text(&db);

        // Parse the file
        let parse_result = parsed(&db, file);
        if parse_result.has_errors() {
            for error in &parse_result.errors {
                summary.add_error(format!("{}: {}", error.span(), error.message()));
            }
            return summary;
        }

        // Check if there are any tests
        if parse_result.module.tests.is_empty() {
            return summary;
        }

        let interner = db.interner();

        // Type check via Salsa query — ensures PoolCache is populated and
        // Salsa dependency tracking is consistent with the query pipeline.
        let type_result = typed(&db, file);
        let Some(pool) = typed_pool(&db, file) else {
            summary.add_error("internal error: Pool not cached after type checking".to_string());
            return summary;
        };

        // Canonicalize once for all tests (compile_fail and regular).
        // Runs even with type errors — pattern problems are independent.
        // Skip only if parse errors exist (AST may be malformed).
        // Store in CanonCache so downstream consumers (evaluator, LLVM) can reuse.
        let shared_canon =
            crate::query::canonicalize_cached(&db, file, &parse_result, &type_result, &pool);

        // Incremental change detection: compute body hashes and determine skippable tests.
        let skippable = if config.incremental {
            let current_map = FunctionChangeMap::from_canon(&shared_canon);
            let path_buf = path.to_path_buf();

            // Single lock acquisition: extract both `changed` set and whether
            // a previous snapshot existed. Avoids redundant re-locking.
            let (changed, had_previous) = {
                let cache_guard = cache.lock();
                if let Some(previous) = cache_guard.get(&path_buf) {
                    (current_map.changed_since(previous), true)
                } else {
                    (rustc_hash::FxHashSet::default(), false)
                }
            };

            let skippable = if had_previous {
                // Have a previous snapshot — compute which tests can be skipped
                // based on which functions changed (may be none, some, or all).
                let index = TestTargetIndex::from_module(&parse_result.module);
                let all_tests: Vec<&TestDef> = parse_result.module.tests.iter().collect();
                index
                    .skippable_tests(&changed, &all_tests)
                    .into_iter()
                    .collect::<rustc_hash::FxHashSet<_>>()
            } else {
                // First run, no previous cache — run everything.
                rustc_hash::FxHashSet::default()
            };

            // Update cache with current snapshot.
            cache.lock().insert(path_buf, current_map);

            skippable
        } else {
            rustc_hash::FxHashSet::default()
        };

        // Separate compile_fail tests from regular tests
        // compile_fail tests don't need evaluation - they just check for type errors
        let (compile_fail_tests, mut regular_tests): (Vec<_>, Vec<_>) = parse_result
            .module
            .tests
            .iter()
            .partition(|t| t.is_compile_fail());

        // Effect-driven prioritization: effectful tests first, pure tests last.
        // Effectful tests are more likely to detect real regressions because they
        // exercise I/O paths and external interactions. Pure tests are deterministic
        // and cacheable, so running them last allows early failure detection.
        if config.incremental {
            Self::prioritize_tests(&mut regular_tests, &type_result.typed, interner);
        }

        // Run compile_fail tests first (they don't need load_module)
        for test in &compile_fail_tests {
            // Apply filter if set
            if let Some(ref filter_str) = config.filter {
                let test_name = interner.lookup(test.name);
                if !test_name.contains(filter_str.as_str()) {
                    continue;
                }
            }

            let inner_result = Self::run_compile_fail_test(
                test,
                &type_result,
                &shared_canon.problems,
                source,
                interner,
            );

            let result = if let Some(expected_failure) = test.fail_expected {
                Self::apply_fail_wrapper(inner_result, expected_failure, interner)
            } else {
                inner_result
            };

            summary.add_result(result);
        }

        // Skip regular test execution if there are no regular tests
        if regular_tests.is_empty() {
            return summary;
        }

        // Check for type errors before running regular tests.
        // Errors within compile_fail test bodies are expected and should not block
        // regular tests. Only errors OUTSIDE compile_fail tests indicate real problems.
        let compile_fail_spans: Vec<_> = compile_fail_tests.iter().map(|t| t.span).collect();
        let non_compile_fail_errors: Vec<_> = type_result
            .errors()
            .iter()
            .filter(|error| {
                let error_span = error.span();
                // Keep error if it's NOT contained in any compile_fail test span
                !compile_fail_spans
                    .iter()
                    .any(|test_span| test_span.contains_span(error_span))
            })
            .collect();

        if !non_compile_fail_errors.is_empty() {
            // Type errors outside compile_fail tests block all regular tests.
            // For interpreter: these are real failures.
            // For LLVM: these are LLVM compile failures (type errors the interpreter
            // handles but LLVM can't codegen yet).
            let is_llvm = matches!(config.backend, Backend::LLVM);

            for test in &regular_tests {
                if is_llvm {
                    summary.add_result(TestResult {
                        name: test.name,
                        targets: test.targets.clone(),
                        outcome: TestOutcome::LlvmCompileFail(
                            "blocked by type errors in file".to_string(),
                        ),
                        duration: Duration::ZERO,
                    });
                } else {
                    summary.add_result(TestResult::failed(
                        test.name,
                        test.targets.clone(),
                        "blocked by type errors in file".to_string(),
                        Duration::ZERO,
                    ));
                }
            }
            for error in non_compile_fail_errors {
                summary.add_error(error.message());
            }
            if is_llvm {
                summary.llvm_compile_error = true;
            }
            return summary;
        }

        // Run regular tests based on backend
        match config.backend {
            Backend::Interpreter => {
                // Create evaluator in TestRun mode with type information
                // TestRun mode: 500-depth recursion limit, test result collection
                let mut evaluator = Evaluator::builder(interner, &parse_result.arena, &db)
                    .mode(ori_eval::EvalMode::TestRun {
                        only_attached: false,
                    })
                    .canon(shared_canon.clone())
                    .build();

                evaluator.register_prelude();

                if let Err(errors) = evaluator.load_module(&parse_result, path, Some(&shared_canon))
                {
                    for error in &errors {
                        summary.add_error(error.message.clone());
                    }
                    return summary;
                }

                // Run each regular test
                for test in &regular_tests {
                    // Apply filter if set
                    if let Some(ref filter_str) = config.filter {
                        let test_name = interner.lookup(test.name);
                        if !test_name.contains(filter_str.as_str()) {
                            continue;
                        }
                    }

                    // Incremental: skip tests whose targets are unchanged.
                    if skippable.contains(&test.name) {
                        summary.add_result(TestResult {
                            name: test.name,
                            targets: test.targets.clone(),
                            outcome: TestOutcome::SkippedUnchanged,
                            duration: Duration::ZERO,
                        });
                        continue;
                    }

                    let inner_result = Self::run_single_test(&mut evaluator, test, interner);

                    // If #[fail] is present, wrap the result
                    let result = if let Some(expected_failure) = test.fail_expected {
                        Self::apply_fail_wrapper(inner_result, expected_failure, interner)
                    } else {
                        inner_result
                    };

                    summary.add_result(result);
                }
            }
            #[cfg(feature = "llvm")]
            Backend::LLVM => {
                // Use LLVM JIT backend — only pass regular_tests since
                // compile_fail tests are already handled in the common path above.
                Self::run_file_llvm(
                    &mut summary,
                    &db,
                    path,
                    &parse_result,
                    &regular_tests,
                    &type_result,
                    &pool,
                    &shared_canon,
                    interner,
                    config,
                );
            }
            #[cfg(not(feature = "llvm"))]
            Backend::LLVM => {
                summary.add_error(
                    "LLVM backend not available (compile with --features llvm)".to_string(),
                );
            }
        }

        summary
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
        let test_files = discover_tests_in(path);
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
