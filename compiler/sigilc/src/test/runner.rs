//! Test execution engine.
//!
//! Runs tests from parsed modules and collects results.

use std::path::Path;
use std::time::Instant;

use rayon::prelude::*;

use crate::db::{CompilerDb, Db};
use crate::input::SourceFile;
use crate::query::parsed;
use crate::eval::{Evaluator, Value};
use crate::ir::TestDef;
use crate::typeck::type_check;

use super::discovery::{discover_tests_in, TestFile};
use super::result::{TestResult, TestSummary, FileSummary, CoverageReport};

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
}

impl Default for TestRunnerConfig {
    fn default() -> Self {
        TestRunnerConfig {
            filter: None,
            verbose: false,
            parallel: true,
            coverage: false,
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
                summary.add_error(format!("Failed to read file: {}", e));
                return summary;
            }
        };

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

        // Create evaluator and register functions
        let mut evaluator = Evaluator::new(interner, &parse_result.arena);
        evaluator.register_prelude();

        // Register all functions
        for func in &parse_result.module.functions {
            let params: Vec<_> = parse_result.arena.get_params(func.params)
                .iter()
                .map(|p| p.name)
                .collect();
            let captures = evaluator.env().capture();
            let func_value = crate::eval::FunctionValue::with_captures(
                params,
                func.body,
                captures,
            );
            evaluator.env_mut().define(func.name, Value::Function(func_value), false);
        }

        // Type check for compile_fail tests
        let typed_module = type_check(&parse_result, interner);

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
            let inner_result = if let Some(expected_error) = test.compile_fail_expected {
                // compile_fail: test expects compilation to fail
                self.run_compile_fail_test(test, &typed_module, expected_error, interner)
            } else {
                self.run_single_test(&mut evaluator, test, interner)
            };

            // If #[fail] is present, wrap the result
            let result = if let Some(expected_failure) = test.fail_expected {
                self.apply_fail_wrapper(inner_result, expected_failure, interner)
            } else {
                inner_result
            };

            summary.add_result(result);
        }

        summary
    }

    /// Run a compile_fail test.
    ///
    /// The test passes if type checking produces at least one error
    /// whose message contains the expected substring.
    fn run_compile_fail_test(
        &self,
        test: &TestDef,
        typed_module: &crate::typeck::TypedModule,
        expected_error: crate::ir::Name,
        interner: &crate::ir::StringInterner,
    ) -> TestResult {
        let test_name = interner.lookup(test.name).to_string();
        let targets: Vec<String> = test.targets
            .iter()
            .map(|t| interner.lookup(*t).to_string())
            .collect();

        // Check if test is skipped
        if let Some(reason) = test.skip_reason {
            let reason_str = interner.lookup(reason).to_string();
            return TestResult::skipped(test_name, targets, reason_str);
        }

        let start = Instant::now();
        let expected_substr = interner.lookup(expected_error);

        // Check if any error message contains the expected substring
        let matching_error = typed_module.errors.iter().find(|e| {
            e.message.contains(expected_substr)
        });

        if let Some(_) = matching_error {
            // Test passes - compilation failed with expected error
            TestResult::passed(test_name, targets, start.elapsed())
        } else if typed_module.errors.is_empty() {
            // Test fails - compilation succeeded when it should have failed
            TestResult::failed(
                test_name,
                targets,
                format!(
                    "expected compilation to fail with error containing '{}', but compilation succeeded",
                    expected_substr
                ),
                start.elapsed(),
            )
        } else {
            // Test fails - compilation failed but with different errors
            let actual_errors: Vec<&str> = typed_module.errors.iter()
                .map(|e| e.message.as_str())
                .collect();
            TestResult::failed(
                test_name,
                targets,
                format!(
                    "expected error containing '{}', but got: {:?}",
                    expected_substr, actual_errors
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
        &self,
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
                    format!(
                        "expected test to fail with '{}', but test passed",
                        expected_substr
                    ),
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
                            "expected failure containing '{}', but got: {}",
                            expected_substr, error
                        ),
                        inner_result.duration,
                    )
                }
            }
        }
    }

    /// Run a single test.
    fn run_single_test(
        &self,
        evaluator: &mut Evaluator,
        test: &TestDef,
        interner: &crate::ir::StringInterner,
    ) -> TestResult {
        let test_name = interner.lookup(test.name).to_string();
        let targets: Vec<String> = test.targets
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
            Ok(_) => {
                TestResult::passed(test_name, targets, start.elapsed())
            }
            Err(e) => {
                TestResult::failed(test_name, targets, e.message, start.elapsed())
            }
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
        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return,
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
                test_map
                    .entry(*target)
                    .or_default()
                    .push(test_name.clone());
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
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_runner_empty_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("empty.si");
        std::fs::write(&path, "").unwrap();

        let summary = run_test_file(&path);
        assert_eq!(summary.total(), 0);
        assert!(!summary.has_failures());
    }

    #[test]
    fn test_runner_no_tests() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("no_tests.si");
        std::fs::write(&path, "@add (a: int, b: int) -> int = a + b").unwrap();

        let summary = run_test_file(&path);
        assert_eq!(summary.total(), 0);
    }

    #[test]
    fn test_runner_passing_test() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("pass.si");
        // Multi-arg calls require named arguments; functions called without @
        std::fs::write(&path, r#"
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    assert_eq(.actual: add(.a: 1, .b: 2), .expected: 3)
)
"#).unwrap();

        let summary = run_test_file(&path);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 0);
    }

    #[test]
    fn test_runner_failing_test() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("fail.si");
        // Multi-arg calls require named arguments; functions called without @
        std::fs::write(&path, r#"
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
    assert_eq(.actual: add(.a: 1, .b: 2), .expected: 4)
)
"#).unwrap();

        let summary = run_test_file(&path);
        assert_eq!(summary.passed, 0);
        assert_eq!(summary.failed, 1);
    }

    #[test]
    fn test_runner_filter() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("filter.si");
        // Multi-arg calls require named arguments; functions called without @
        std::fs::write(&path, r#"
@foo () -> int = 1
@bar () -> int = 2

@test_foo tests @foo () -> void = assert_eq(.left: foo(), .right: 1)
@test_bar tests @bar () -> void = assert_eq(.left: bar(), .right: 2)
"#).unwrap();

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
