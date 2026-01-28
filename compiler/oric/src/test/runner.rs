//! Test execution engine.
//!
//! Runs tests from parsed modules and collects results.

use std::path::Path;
use std::time::Instant;

use rayon::prelude::*;

use crate::db::{CompilerDb, Db};
use crate::eval::Evaluator;
use crate::input::SourceFile;
use crate::ir::TestDef;
use crate::query::parsed;
use crate::typeck::type_check_with_imports_and_source;

use super::discovery::{discover_tests_in, TestFile};
use super::error_matching::{format_actual, format_expected, match_errors};
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
pub struct TestRunner {
    config: TestRunnerConfig,
}

impl TestRunner {
    /// Create a new test runner with default config.
    pub fn new() -> Self {
        TestRunner {
            config: TestRunnerConfig::default(),
        }
    }

    /// Create a test runner with custom config.
    pub fn with_config(config: TestRunnerConfig) -> Self {
        TestRunner { config }
    }

    /// Run all tests in a path (file or directory).
    pub fn run(&self, path: &Path) -> TestSummary {
        let test_files = discover_tests_in(path);

        if self.config.parallel && test_files.len() > 1 {
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
            let file_summary = self.run_file(&file.path);
            summary.add_file(file_summary);
        }

        summary.duration = start.elapsed();
        summary
    }

    /// Run tests in parallel using rayon.
    fn run_parallel(&self, files: &[TestFile]) -> TestSummary {
        let start = Instant::now();

        let file_summaries: Vec<_> = files
            .par_iter()
            .map(|file| self.run_file(&file.path))
            .collect();

        let mut summary = TestSummary::new();
        for file_summary in file_summaries {
            summary.add_file(file_summary);
        }

        summary.duration = start.elapsed();
        summary
    }

    /// Run all tests in a single file.
    fn run_file(&self, path: &Path) -> FileSummary {
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
        let db = CompilerDb::new();
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

        // Type check with import resolution (enables proper type checking of imported functions)
        let typed_module =
            type_check_with_imports_and_source(&db, &parse_result, path, source.clone());

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
            if let Some(filter) = &self.config.filter {
                let test_name = interner.lookup(test.name);
                if !test_name.contains(filter.as_str()) {
                    continue;
                }
            }

            let inner_result = Self::run_compile_fail_test(test, &typed_module, &source, interner);

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

        // Run regular tests based on backend
        match self.config.backend {
            Backend::Interpreter => {
                // Create evaluator with database for proper Salsa-tracked import resolution
                // load_module enforces type checking - will fail if there are type errors
                let mut evaluator = Evaluator::builder(interner, &parse_result.arena, &db).build();

                evaluator.register_prelude();

                if let Err(e) = evaluator.load_module(&parse_result, path) {
                    summary.add_error(e);
                    return summary;
                }

                // Run each regular test
                for test in &regular_tests {
                    // Apply filter if set
                    if let Some(filter) = &self.config.filter {
                        let test_name = interner.lookup(test.name);
                        if !test_name.contains(filter.as_str()) {
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
                // Use LLVM JIT backend
                self.run_file_llvm(
                    &mut summary,
                    &parse_result,
                    &typed_module,
                    &source,
                    interner,
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

    /// Run tests in a file using the LLVM backend.
    #[cfg(feature = "llvm")]
    fn run_file_llvm(
        &self,
        summary: &mut FileSummary,
        parse_result: &ori_parse::ParseOutput,
        typed_module: &crate::typeck::TypedModule,
        source: &str,
        interner: &crate::ir::StringInterner,
    ) {
        use ori_llvm::evaluator::OwnedLLVMEvaluator;
        use ori_llvm::FunctionSig;

        // Create LLVM evaluator (owns its context)
        let mut llvm_eval = OwnedLLVMEvaluator::new();

        // Load the module
        if let Err(e) = llvm_eval.load_module(&parse_result.module, &parse_result.arena) {
            summary.add_error(e);
            return;
        }

        // Convert function types from typeck to LLVM format
        let function_sigs: Vec<FunctionSig> = typed_module
            .function_types
            .iter()
            .map(|ft| FunctionSig {
                params: ft.params.clone(),
                return_type: ft.return_type,
            })
            .collect();

        // Run each test
        for test in &parse_result.module.tests {
            // Apply filter if set
            if let Some(filter) = &self.config.filter {
                let test_name = interner.lookup(test.name);
                if !test_name.contains(filter.as_str()) {
                    continue;
                }
            }

            // Run the inner test (compile_fail or regular)
            let inner_result = if test.is_compile_fail() {
                // compile_fail: test expects compilation to fail
                Self::run_compile_fail_test(test, typed_module, source, interner)
            } else {
                Self::run_single_test_llvm(
                    &llvm_eval,
                    test,
                    &parse_result.arena,
                    &parse_result.module,
                    interner,
                    &typed_module.expr_types,
                    &function_sigs,
                )
            };

            // If #[fail] is present, wrap the result
            let result = if let Some(expected_failure) = test.fail_expected {
                Self::apply_fail_wrapper(inner_result, expected_failure, interner)
            } else {
                inner_result
            };

            summary.add_result(result);
        }
    }

    /// Run a single test using LLVM.
    #[cfg(feature = "llvm")]
    fn run_single_test_llvm(
        llvm_eval: &ori_llvm::evaluator::OwnedLLVMEvaluator,
        test: &TestDef,
        arena: &crate::ir::ExprArena,
        module: &crate::ir::Module,
        interner: &crate::ir::StringInterner,
        expr_types: &[crate::ir::TypeId],
        function_sigs: &[ori_llvm::FunctionSig],
    ) -> TestResult {
        let test_name = interner.lookup(test.name).to_string();
        let targets: Vec<String> = test
            .targets
            .iter()
            .map(|t| interner.lookup(*t).to_string())
            .collect();

        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            let reason_str = interner.lookup(reason).to_string();
            return TestResult::skipped(test_name, targets, reason_str);
        }

        // Time the test execution
        let start = Instant::now();

        // Evaluate the test body using LLVM JIT
        match llvm_eval.eval_test(
            test.name,
            test.body,
            arena,
            module,
            interner,
            expr_types,
            function_sigs,
        ) {
            Ok(_) => TestResult::passed(test_name, targets, start.elapsed()),
            Err(e) => TestResult::failed(test_name, targets, e.message, start.elapsed()),
        }
    }

    /// Run a `compile_fail` test.
    ///
    /// The test passes if all expected errors are matched by actual errors.
    /// Multiple expected errors can be specified, and each must be matched.
    fn run_compile_fail_test(
        test: &TestDef,
        typed_module: &crate::typeck::TypedModule,
        source: &str,
        interner: &crate::ir::StringInterner,
    ) -> TestResult {
        let test_name = interner.lookup(test.name).to_string();
        let targets: Vec<String> = test
            .targets
            .iter()
            .map(|t| interner.lookup(*t).to_string())
            .collect();

        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            let reason_str = interner.lookup(reason).to_string();
            return TestResult::skipped(test_name, targets, reason_str);
        }

        let start = Instant::now();

        // If no errors were produced but we expected some
        if typed_module.errors.is_empty() {
            let expected_strs: Vec<String> = test
                .expected_errors
                .iter()
                .map(|e| format_expected(e, interner))
                .collect();
            return TestResult::failed(
                test_name,
                targets,
                format!(
                    "expected compilation to fail with {} error(s), but compilation succeeded. Expected: {}",
                    test.expected_errors.len(),
                    expected_strs.join("; ")
                ),
                start.elapsed(),
            );
        }

        // Match actual errors against expectations
        let match_result = match_errors(
            &typed_module.errors,
            &test.expected_errors,
            source,
            interner,
        );

        if match_result.all_matched() {
            // All expectations matched - test passes
            TestResult::passed(test_name, targets, start.elapsed())
        } else {
            // Some expectations were not matched
            let unmatched: Vec<String> = match_result
                .unmatched_expectations
                .iter()
                .map(|&i| format_expected(&test.expected_errors[i], interner))
                .collect();

            let actual: Vec<String> = typed_module
                .errors
                .iter()
                .map(|e| format_actual(e, source))
                .collect();

            TestResult::failed(
                test_name,
                targets,
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
        use super::result::TestOutcome;

        let expected_substr = interner.lookup(expected_failure);

        match inner_result.outcome {
            TestOutcome::Skipped(_) => {
                // Skipped tests remain skipped
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
        let test_name = interner.lookup(test.name).to_string();
        let targets: Vec<String> = test
            .targets
            .iter()
            .map(|t| interner.lookup(*t).to_string())
            .collect();

        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            let reason_str = interner.lookup(reason).to_string();
            return TestResult::skipped(test_name, targets, reason_str);
        }

        // Time the test execution
        let start = Instant::now();

        // Evaluate the test body
        match evaluator.eval(test.body) {
            Ok(_) => TestResult::passed(test_name, targets, start.elapsed()),
            Err(e) => TestResult::failed(test_name, targets, e.message, start.elapsed()),
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
            Self::add_file_coverage(&file.path, &mut report);
        }

        report
    }

    /// Add coverage info for a single file.
    fn add_file_coverage(path: &Path, report: &mut CoverageReport) {
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };

        let db = CompilerDb::new();
        let file = SourceFile::new(&db, path.to_path_buf(), content);
        let parse_result = parsed(&db, file);

        if parse_result.has_errors() {
            return;
        }

        let interner = db.interner();
        let main_name = interner.intern("main");

        // Build map of function -> tests that target it
        let mut test_map: std::collections::HashMap<crate::ir::Name, Vec<String>> =
            std::collections::HashMap::new();

        for test in &parse_result.module.tests {
            let test_name = interner.lookup(test.name).to_string();
            for target in &test.targets {
                test_map.entry(*target).or_default().push(test_name.clone());
            }
        }

        // Add coverage for each function (except main)
        for func in &parse_result.module.functions {
            if func.name == main_name {
                continue;
            }
            let func_name = interner.lookup(func.name).to_string();
            let test_names = test_map.get(&func.name).cloned().unwrap_or_default();
            let has_tests = !test_names.is_empty();
            report.add_function(func_name, has_tests, test_names);
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
        assert!(summary.results[0].name.contains("foo"));
    }
}
