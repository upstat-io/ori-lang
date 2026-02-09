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
use crate::query::parsed;
use crate::typeck;
use ori_types::TypeCheckResult;

use super::discovery::{discover_tests_in, TestFile};
use super::error_matching::{format_actual, format_expected, match_errors_refs};
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
}

impl Default for TestRunnerConfig {
    fn default() -> Self {
        TestRunnerConfig {
            filter: None,
            verbose: false,
            parallel: true,
            coverage: false,
            backend: Backend::Interpreter,
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
}

impl TestRunner {
    /// Create a new test runner with default config.
    pub fn new() -> Self {
        TestRunner {
            config: TestRunnerConfig::default(),
            interner: crate::ir::SharedInterner::new(),
        }
    }

    /// Create a test runner with custom config.
    pub fn with_config(config: TestRunnerConfig) -> Self {
        TestRunner {
            config,
            interner: crate::ir::SharedInterner::new(),
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
                Self::run_file_with_interner(&file.path, &self.interner, &self.config);
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
                                Self::run_file_with_interner(&file.path, &interner, &config)
                            })
                            .collect::<Vec<_>>()
                    })
                },
            )
            .unwrap_or_else(|e| {
                eprintln!("Warning: failed to create thread pool ({e}), running sequentially");
                files
                    .iter()
                    .map(|file| Self::run_file_with_interner(&file.path, &interner, &config))
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
        Self::run_file_with_interner(path, &self.interner, &self.config)
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

        // Keep a copy of the source for error matching (content is moved into SourceFile)
        let source = content.clone();
        // Create a fresh CompilerDb with the shared interner.
        // Each file gets its own Salsa query cache, but all share the same interner
        // so Name values are comparable across files.
        let db = CompilerDb::with_interner(interner.clone());
        let file = SourceFile::new(&db, path.to_path_buf(), content);

        // Parse the file
        let parse_result = parsed(&db, file);
        if parse_result.has_errors() {
            for error in &parse_result.errors {
                summary.add_error(format!("{}: {}", error.span, error.message));
            }
            return summary;
        }

        // Check if there are any tests
        if parse_result.module.tests.is_empty() {
            return summary;
        }

        let interner = db.interner();

        // Type check with import resolution.
        // Pool is used by the LLVM backend for compound type resolution (sret convention).
        #[allow(unused_variables)]
        let (type_result, pool) =
            typeck::type_check_with_imports_and_pool(&db, &parse_result, path);

        // Separate compile_fail tests from regular tests
        // compile_fail tests don't need evaluation - they just check for type errors
        let (compile_fail_tests, regular_tests): (Vec<_>, Vec<_>) = parse_result
            .module
            .tests
            .iter()
            .partition(|t| t.is_compile_fail());

        // Run compile_fail tests first (they don't need load_module)
        for test in &compile_fail_tests {
            // Apply filter if set
            if let Some(ref filter_str) = config.filter {
                let test_name = interner.lookup(test.name);
                if !test_name.contains(filter_str.as_str()) {
                    continue;
                }
            }

            let inner_result = Self::run_compile_fail_test(test, &type_result, &source, interner);

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
                    .expr_types(&type_result.typed.expr_types)
                    .pattern_resolutions(&type_result.typed.pattern_resolutions)
                    .build();

                evaluator.register_prelude();

                if let Err(e) = evaluator.load_module(&parse_result, path) {
                    summary.add_error(e);
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
        parse_result: &ori_parse::ParseOutput,
        regular_tests: &[&crate::ir::TestDef],
        type_result: &TypeCheckResult,
        pool: &ori_types::Pool,
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
        let mut imported_type_results: Vec<TypeCheckResult> = Vec::new();
        for imp_module in &resolved.modules {
            let imp_path = std::path::PathBuf::from(&imp_module.module_path);
            let (imp_tc, _) =
                typeck::type_check_with_imports_and_pool(db, &imp_module.parse_output, &imp_path);
            imported_type_results.push(imp_tc);
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
                    arena: &parse_output.arena,
                    expr_types: &imported_type_results[fref.module_index].typed.expr_types,
                }
            })
            .collect();

        // Build function signatures aligned with module.functions order.
        // typed.functions is sorted by name (Salsa determinism), but
        // module.functions is in source order — FunctionCompiler::declare_all
        // zips them, so they must be aligned.
        let sig_map: std::collections::HashMap<ori_ir::Name, &ori_types::FunctionSig> = type_result
            .typed
            .functions
            .iter()
            .map(|ft| (ft.name, ft))
            .collect();

        let function_sigs: Vec<ori_types::FunctionSig> = parse_result
            .module
            .functions
            .iter()
            .map(|func| {
                sig_map
                    .get(&func.name)
                    .copied()
                    .cloned()
                    .unwrap_or_else(|| {
                        // Fallback for functions without type info (shouldn't happen
                        // after successful type checking, but be defensive).
                        ori_types::FunctionSig {
                            name: func.name,
                            type_params: vec![],
                            param_names: vec![],
                            param_types: vec![],
                            return_type: ori_types::Idx::UNIT,
                            capabilities: vec![],
                            is_public: false,
                            is_test: false,
                            is_main: false,
                            type_param_bounds: vec![],
                            where_clauses: vec![],
                            generic_param_mapping: vec![],
                            required_params: 0,
                        }
                    })
            })
            .collect();

        // Compile module ONCE with all tests.
        // Wrap in catch_unwind to gracefully handle LLVM fatal errors
        // (e.g., "unable to allocate function return" for unsupported types).
        let compile_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            llvm_eval.compile_module_with_tests(
                &parse_result.module,
                &filtered_tests,
                &parse_result.arena,
                interner,
                &type_result.typed.expr_types,
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
            Err(e) => TestResult::failed(
                test.name,
                test.targets.clone(),
                e.into_eval_error().message,
                start.elapsed(),
            ),
        }
    }

    /// Run a `compile_fail` test.
    ///
    /// The test passes if all expected errors are matched by actual errors.
    ///
    /// Error matching strategy:
    /// 1. First try to match errors within this test's span (isolation for tests
    ///    that produce errors in their body, like `add("hello", 2)`)
    /// 2. If no errors in test span, fall back to matching all module errors
    ///    (for tests checking module-level errors like missing impl members)
    fn run_compile_fail_test(
        test: &TestDef,
        type_result: &TypeCheckResult,
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
        let test_errors: Vec<_> = type_result
            .errors()
            .iter()
            .filter(|e| test.span.contains_span(e.span()))
            .collect();

        // If no errors within test span, use all module errors.
        // This handles tests that check for module-level errors (like missing
        // associated types in impl blocks) where the error is outside the test body.
        let errors_to_match: Vec<&_> = if test_errors.is_empty() {
            type_result.errors().iter().collect()
        } else {
            test_errors
        };

        // If no errors were produced but we expected some
        if errors_to_match.is_empty() {
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

        // Match actual errors against expectations
        let match_result =
            match_errors_refs(&errors_to_match, &test.expected_errors, source, interner);

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

            let actual: Vec<String> = errors_to_match
                .iter()
                .map(|e| format_actual(e, source))
                .collect();

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
            TestOutcome::Skipped(_) | TestOutcome::LlvmCompileFail(_) => {
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

        // Evaluate the test body
        match evaluator.eval(test.body) {
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
        let mut test_map: std::collections::HashMap<crate::ir::Name, Vec<crate::ir::Name>> =
            std::collections::HashMap::new();

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
