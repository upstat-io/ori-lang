//! Test execution engine.
//!
//! Runs tests from parsed modules and collects results.

use std::path::Path;
use std::time::{Duration, Instant};

use rayon::prelude::*;

use crate::db::{CompilerDb, Db};
use crate::eval::Evaluator;
use crate::input::SourceFile;
use crate::ir::TestDef;
use crate::query::{parsed, typed, typed_pool};
use ori_types::TypeCheckResult;

use super::change_detection::{FunctionChangeMap, TestRunCache, TestTargetIndex};
use super::discovery::{discover_tests_in, TestFile};
use super::error_matching::{
    format_actual, format_expected, format_pattern_problem, match_all_errors,
};
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

    /// Run regular (non-compile_fail) tests using the LLVM JIT backend.
    ///
    /// Uses the "compile once, run many" pattern: compiles all functions and test
    /// wrappers into a single JIT engine, then runs each test from that engine.
    /// This avoids O(n²) recompilation that caused LLVM resource exhaustion.
    ///
    /// Note: compile_fail tests are handled in the common path of
    /// `run_file_with_interner()` before backend dispatch — they are NOT
    /// passed here. This avoids double-counting.
    #[cfg(feature = "llvm")]
    fn run_file_llvm(
        summary: &mut FileSummary,
        db: &crate::db::CompilerDb,
        file_path: &Path,
        parse_result: &crate::parser::ParseOutput,
        regular_tests: &[&crate::ir::TestDef],
        type_result: &TypeCheckResult,
        pool: &ori_types::Pool,
        shared_canon: &ori_ir::canon::SharedCanonResult,
        interner: &crate::ir::StringInterner,
        config: &TestRunnerConfig,
    ) {
        use ori_llvm::evaluator::{ImportedFunctionForCodegen, OwnedLLVMEvaluator};

        // Skip LLVM compilation if no regular tests to run
        if regular_tests.is_empty() {
            return;
        }

        // Filter regular tests before compilation
        let filtered_tests: Vec<_> = regular_tests
            .iter()
            .filter(|test| {
                if let Some(ref filter_str) = config.filter {
                    let test_name = interner.lookup(test.name);
                    test_name.contains(filter_str.as_str())
                } else {
                    true
                }
            })
            .copied()
            .collect();

        if filtered_tests.is_empty() {
            return;
        }

        // Install custom LLVM fatal error handler so LLVM errors panic
        // instead of aborting the process (allows catch_unwind recovery).
        ori_llvm::install_fatal_error_handler();

        // Create LLVM evaluator with type pool for proper compound type resolution
        // (needed for sret convention on large struct returns like List, Map, etc.)
        let llvm_eval = OwnedLLVMEvaluator::with_pool(pool);

        // Resolve imports so imported functions can be compiled into the JIT module.
        // Uses the unified import pipeline — same resolution path as the type checker
        // and interpreter.
        let resolved = crate::imports::resolve_imports(db, parse_result, file_path);

        // Type-check each explicitly imported module to get expr_types + function_sigs.
        // Note: prelude functions are NOT compiled into the JIT module because:
        // 1. Most prelude content is traits (no code to compile)
        // 2. Generic utility functions are skipped by codegen
        // 3. Some non-generic prelude functions (e.g., `compare`) use types the
        //    V2 codegen doesn't support yet (sum types), causing IR verification failures
        // Prelude functions that are needed for testing (assert, assert_eq) come from
        // std.testing via explicit import, not from the prelude.
        // Type-check each imported module via Salsa queries (when SourceFile is available).
        // This ensures results are cached in Salsa's dependency graph and the Pool
        // is stored in PoolCache, avoiding redundant work when the same module is
        // imported by multiple test files.
        let mut imported_type_results: Vec<TypeCheckResult> = Vec::new();
        let mut imported_canon_results: Vec<ori_ir::canon::SharedCanonResult> = Vec::new();
        for imp_module in &resolved.modules {
            // Type-check via shared helper (Salsa queries when SourceFile is
            // available, direct type checking otherwise).
            let Some((imp_tc, imp_pool)) = crate::query::type_check_module(
                db,
                &imp_module.parse_output,
                &imp_module.module_path,
                imp_module.source_file,
            ) else {
                // Pool not cached — internal error. Push empty results to
                // maintain index alignment with resolved.modules.
                imported_type_results.push(TypeCheckResult::default());
                imported_canon_results
                    .push(ori_ir::canon::SharedCanonResult::new(Default::default()));
                continue;
            };
            // Use cached canonicalization — avoids re-canonicalizing the same
            // module (e.g., std.testing) when imported by multiple test files.
            let imp_canon = crate::query::canonicalize_cached_by_path(
                db,
                &imp_module.module_path,
                &imp_module.parse_output,
                &imp_tc,
                &imp_pool,
            );
            imported_type_results.push(imp_tc);
            imported_canon_results.push(imp_canon);
        }

        // Build per-function codegen structs for explicitly imported functions only.
        // We need owned FunctionSig values that outlive the ImportedFunctionForCodegen refs.
        let mut imported_sigs_storage: Vec<ori_types::FunctionSig> = Vec::new();

        struct FnRef {
            func_index: usize,
            module_index: usize,
        }
        let mut fn_refs: Vec<FnRef> = Vec::new();

        for func_ref in &resolved.imported_functions {
            if func_ref.is_module_alias {
                continue;
            }
            let imp_module = &resolved.modules[func_ref.module_index];
            let tc = &imported_type_results[func_ref.module_index];

            // Find the function by original_name in the imported module
            if let Some((idx, _func)) = imp_module
                .parse_output
                .module
                .functions
                .iter()
                .enumerate()
                .find(|(_, f)| f.name == func_ref.original_name)
            {
                // Find its type-checked signature
                if let Some(sig) = tc
                    .typed
                    .functions
                    .iter()
                    .find(|s| s.name == func_ref.original_name)
                {
                    if sig.is_generic() {
                        continue;
                    }
                    imported_sigs_storage.push(sig.clone());
                    fn_refs.push(FnRef {
                        func_index: idx,
                        module_index: func_ref.module_index,
                    });
                }
            }
        }

        // Build ImportedFunctionForCodegen from the stable storage
        let imported_for_codegen: Vec<ImportedFunctionForCodegen<'_>> = fn_refs
            .iter()
            .enumerate()
            .map(|(sig_idx, fref)| {
                let parse_output = &resolved.modules[fref.module_index].parse_output;
                ImportedFunctionForCodegen {
                    function: &parse_output.module.functions[fref.func_index],
                    sig: &imported_sigs_storage[sig_idx],
                    canon: &imported_canon_results[fref.module_index],
                }
            })
            .collect();

        // Build function signatures aligned with module.functions source order.
        // Delegates to shared implementation in typeck.
        let function_sigs = crate::typeck::build_function_sigs(parse_result, type_result);

        // Compile module ONCE with all tests.
        // Wrap in catch_unwind to gracefully handle LLVM fatal errors
        // (e.g., "unable to allocate function return" for unsupported types).
        let compile_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            llvm_eval.compile_module_with_tests(
                &parse_result.module,
                &filtered_tests,
                shared_canon,
                interner,
                &function_sigs,
                &type_result.typed.types,
                &type_result.typed.impl_sigs,
                &imported_for_codegen,
            )
        }));

        let compiled = match compile_result {
            Ok(Ok(c)) => c,
            Ok(Err(e)) => {
                // Record the compilation error for display
                summary.add_error(e.message.clone());
                summary.llvm_compile_error = true;
                // Create LlvmCompileFail results for each test — these are
                // tracked separately and don't count as real failures.
                for test in &filtered_tests {
                    summary.add_result(TestResult {
                        name: test.name,
                        targets: test.targets.clone(),
                        outcome: TestOutcome::LlvmCompileFail(format!(
                            "LLVM compilation failed: {}",
                            e.message
                        )),
                        duration: Duration::ZERO,
                    });
                }
                return;
            }
            Err(panic_info) => {
                let msg = if let Some(s) = panic_info.downcast_ref::<String>() {
                    s.clone()
                } else if let Some(s) = panic_info.downcast_ref::<&str>() {
                    (*s).to_string()
                } else {
                    "LLVM compilation panicked".to_string()
                };
                summary.add_error(format!("LLVM backend error: {msg}"));
                summary.llvm_compile_error = true;
                // Create LlvmCompileFail results for each test.
                for test in &filtered_tests {
                    summary.add_result(TestResult {
                        name: test.name,
                        targets: test.targets.clone(),
                        outcome: TestOutcome::LlvmCompileFail(format!("LLVM backend error: {msg}")),
                        duration: Duration::ZERO,
                    });
                }
                return;
            }
        };

        // Run each test from the compiled module (no recompilation!)
        for test in &filtered_tests {
            let inner_result = Self::run_single_test_from_compiled(&compiled, test, interner);

            let result = if let Some(expected_failure) = test.fail_expected {
                Self::apply_fail_wrapper(inner_result, expected_failure, interner)
            } else {
                inner_result
            };

            summary.add_result(result);
        }
    }

    /// Run a single test from an already-compiled module.
    ///
    /// This is the efficient path: the module was compiled once and we just
    /// call into the JIT engine to run each test.
    #[cfg(feature = "llvm")]
    fn run_single_test_from_compiled(
        compiled: &ori_llvm::evaluator::CompiledTestModule,
        test: &TestDef,
        interner: &crate::ir::StringInterner,
    ) -> TestResult {
        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            let reason_str = interner.lookup(reason).to_string();
            return TestResult::skipped(test.name, test.targets.clone(), reason_str);
        }

        // Time the test execution
        let start = Instant::now();

        // Run the test from the compiled module (no recompilation!)
        match compiled.run_test(test.name) {
            Ok(_) => TestResult::passed(test.name, test.targets.clone(), start.elapsed()),
            Err(e) => {
                TestResult::failed(test.name, test.targets.clone(), e.message, start.elapsed())
            }
        }
    }

    /// Run a `compile_fail` test.
    ///
    /// The test passes if all expected errors are matched by actual errors.
    /// Matches against both type errors and pattern problems (exhaustiveness/
    /// redundancy from canonicalization).
    ///
    /// Error matching strategy:
    /// 1. First try to match errors within this test's span (isolation for tests
    ///    that produce errors in their body, like `add("hello", 2)`)
    /// 2. If no errors in test span, fall back to matching all module errors
    ///    (for tests checking module-level errors like missing impl members)
    fn run_compile_fail_test(
        test: &TestDef,
        type_result: &TypeCheckResult,
        pattern_problems: &[ori_ir::canon::PatternProblem],
        source: &str,
        interner: &crate::ir::StringInterner,
    ) -> TestResult {
        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            let reason_str = interner.lookup(reason).to_string();
            return TestResult::skipped(test.name, test.targets.clone(), reason_str);
        }

        let start = Instant::now();

        // Try span-filtered errors first for better isolation.
        // This helps when multiple compile_fail tests exist in the same file,
        // each should only see errors from their own body.
        let test_type_errors: Vec<_> = type_result
            .errors()
            .iter()
            .filter(|e| test.span.contains_span(e.span()))
            .collect();

        // Filter pattern problems by test span too.
        let test_pattern_problems: Vec<_> = pattern_problems
            .iter()
            .filter(|p| {
                let span = match p {
                    ori_ir::canon::PatternProblem::NonExhaustive { match_span, .. } => *match_span,
                    ori_ir::canon::PatternProblem::RedundantArm { arm_span, .. } => *arm_span,
                };
                test.span.contains_span(span)
            })
            .collect();

        // If no errors within test span, use all module errors.
        // This handles tests that check for module-level errors (like missing
        // associated types in impl blocks) where the error is outside the test body.
        let type_errors_to_match: Vec<&_> =
            if test_type_errors.is_empty() && test_pattern_problems.is_empty() {
                type_result.errors().iter().collect()
            } else {
                test_type_errors
            };

        let pattern_problems_to_match: Vec<&_> = if type_errors_to_match.len()
            == type_result.errors().len()
            && test_pattern_problems.is_empty()
        {
            // Fell back to all module errors — also use all pattern problems.
            pattern_problems.iter().collect()
        } else {
            test_pattern_problems
        };

        // If no errors were produced but we expected some
        if type_errors_to_match.is_empty() && pattern_problems_to_match.is_empty() {
            let expected_strs: Vec<String> = test
                .expected_errors
                .iter()
                .map(|e| format_expected(e, interner))
                .collect();
            let error_word = if test.expected_errors.len() == 1 {
                "error"
            } else {
                "errors"
            };
            return TestResult::failed(
                test.name,
                test.targets.clone(),
                format!(
                    "expected compilation to fail with {} {error_word}, but compilation succeeded. Expected: {}",
                    test.expected_errors.len(),
                    expected_strs.join("; ")
                ),
                start.elapsed(),
            );
        }

        // Match actual errors (type + pattern) against expectations
        let match_result = match_all_errors(
            &type_errors_to_match,
            &pattern_problems_to_match,
            &test.expected_errors,
            source,
            interner,
        );

        if match_result.all_matched() {
            // All expectations matched - test passes
            TestResult::passed(test.name, test.targets.clone(), start.elapsed())
        } else {
            // Some expectations were not matched
            let unmatched: Vec<String> = match_result
                .unmatched_expectations
                .iter()
                .map(|&i| format_expected(&test.expected_errors[i], interner))
                .collect();

            let mut actual: Vec<String> = type_errors_to_match
                .iter()
                .map(|e| format_actual(e, source))
                .collect();
            actual.extend(
                pattern_problems_to_match
                    .iter()
                    .map(|p| format_pattern_problem(p, source)),
            );

            TestResult::failed(
                test.name,
                test.targets.clone(),
                format!(
                    "unmatched expectations: [{}]. Actual errors: [{}]",
                    unmatched.join(", "),
                    actual.join(", ")
                ),
                start.elapsed(),
            )
        }
    }

    /// Apply the #[fail] wrapper to a test result.
    ///
    /// The #[fail] attribute expects the inner test to fail.
    /// - If inner test failed with expected message: wrapper passes
    /// - If inner test failed with different message: wrapper fails
    /// - If inner test passed: wrapper fails (expected failure didn't happen)
    /// - If inner test was skipped: remains skipped
    fn apply_fail_wrapper(
        inner_result: TestResult,
        expected_failure: crate::ir::Name,
        interner: &crate::ir::StringInterner,
    ) -> TestResult {
        let expected_substr = interner.lookup(expected_failure);

        match inner_result.outcome {
            TestOutcome::Skipped(_)
            | TestOutcome::SkippedUnchanged
            | TestOutcome::LlvmCompileFail(_) => {
                // Skipped and expected-failure tests pass through unchanged
                inner_result
            }
            TestOutcome::Passed => {
                // Inner test passed, but we expected it to fail
                TestResult::failed(
                    inner_result.name,
                    inner_result.targets,
                    format!("expected test to fail with '{expected_substr}', but test passed"),
                    inner_result.duration,
                )
            }
            TestOutcome::Failed(ref error) => {
                // Inner test failed - check if it's the expected failure
                if error.contains(expected_substr) {
                    // Expected failure occurred - this is a pass
                    TestResult::passed(
                        inner_result.name,
                        inner_result.targets,
                        inner_result.duration,
                    )
                } else {
                    // Wrong failure message
                    TestResult::failed(
                        inner_result.name,
                        inner_result.targets,
                        format!(
                            "expected failure containing '{expected_substr}', but got: {error}"
                        ),
                        inner_result.duration,
                    )
                }
            }
        }
    }

    /// Sort tests by effect class: effectful first, pure last.
    ///
    /// Effectful tests (targets with capabilities like `Http`, `FileSystem`) are more
    /// likely to catch real regressions because they exercise I/O paths. Pure tests
    /// (targets with no capabilities) are deterministic and cacheable, so running
    /// them last allows failures to surface sooner.
    fn prioritize_tests(
        tests: &mut [&TestDef],
        typed: &ori_types::TypedModule,
        interner: &crate::ir::StringInterner,
    ) {
        tests.sort_by(|a, b| {
            let effect_a = Self::max_target_effect(a, typed, interner);
            let effect_b = Self::max_target_effect(b, typed, interner);
            // Reverse: HasEffects (2) > ReadsOnly (1) > Pure (0)
            // We want HasEffects first, so reverse the comparison.
            effect_b.cmp(&effect_a)
        });
    }

    /// Get the maximum effect class across a test's targets.
    ///
    /// If any target has `HasEffects`, the test is effectful.
    /// If any target has `ReadsOnly` (and none has `HasEffects`), it's read-only.
    /// Otherwise it's pure.
    fn max_target_effect(
        test: &TestDef,
        typed: &ori_types::TypedModule,
        interner: &crate::ir::StringInterner,
    ) -> ori_types::EffectClass {
        use ori_types::EffectClass;

        let mut max_effect = EffectClass::Pure;

        for &target in &test.targets {
            if let Some(sig) = typed.function(target) {
                let effect = sig.effect_class(interner);
                if effect > max_effect {
                    max_effect = effect;
                }
                if max_effect == EffectClass::HasEffects {
                    return max_effect; // Short-circuit: can't get higher
                }
            }
        }

        max_effect
    }

    /// Run a single test.
    fn run_single_test(
        evaluator: &mut Evaluator,
        test: &TestDef,
        interner: &crate::ir::StringInterner,
    ) -> TestResult {
        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            let reason_str = interner.lookup(reason).to_string();
            return TestResult::skipped(test.name, test.targets.clone(), reason_str);
        }

        // Time the test execution
        let start = Instant::now();

        let Some(can_id) = evaluator.canon_root_for(test.name) else {
            return TestResult::failed(
                test.name,
                test.targets.clone(),
                "internal error: test has no canonical root".to_string(),
                start.elapsed(),
            );
        };
        let result = evaluator.eval_can(can_id);
        match result {
            Ok(_) => TestResult::passed(test.name, test.targets.clone(), start.elapsed()),
            Err(e) => TestResult::failed(
                test.name,
                test.targets.clone(),
                e.into_eval_error().message,
                start.elapsed(),
            ),
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
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_runner_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.ori");
        std::fs::write(&path, "").unwrap();

        let summary = run_test_file(&path);
        assert_eq!(summary.total(), 0);
        assert!(!summary.has_failures());
    }

    #[test]
    fn test_runner_no_tests() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("no_tests.ori");
        std::fs::write(&path, "@add (a: int, b: int) -> int = a + b").unwrap();

        let summary = run_test_file(&path);
        assert_eq!(summary.total(), 0);
    }

    #[test]
    fn test_runner_passing_test() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pass.ori");
        // Test passes by completing without panic
        std::fs::write(
            &path,
            r#"
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    let result = add(a: 1, b: 2),
    print(msg: "done")
)
"#,
        )
        .unwrap();

        let summary = run_test_file(&path);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 0);
    }

    #[test]
    fn test_runner_failing_test() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("fail.ori");
        // Test fails by causing division by zero
        // (Note: panic() returns Never which doesn't type check in void context,
        // so we use division by zero to cause a runtime failure instead)
        std::fs::write(
            &path,
            r"
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    let _ = add(a: 1, b: 2),
    let _ = 1 / 0,
    ()
)
",
        )
        .unwrap();

        let summary = run_test_file(&path);
        assert_eq!(summary.passed, 0);
        assert_eq!(summary.failed, 1);
    }

    #[test]
    fn test_runner_filter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("filter.ori");
        // Tests pass by completing without panic
        std::fs::write(
            &path,
            r#"
@foo () -> int = 1
@bar () -> int = 2

@test_foo tests @foo () -> void = print(msg: "pass")
@test_bar tests @bar () -> void = print(msg: "pass")
"#,
        )
        .unwrap();

        let config = TestRunnerConfig {
            filter: Some("foo".to_string()),
            ..Default::default()
        };
        let runner = TestRunner::with_config(config);
        let summary = runner.run_file(&path);

        assert_eq!(summary.total(), 1);
        // Use the interner to look up the Name
        let name_str = summary.results[0].name_str(runner.interner());
        assert!(name_str.contains("foo"));
    }
}
