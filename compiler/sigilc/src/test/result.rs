//! Test result types.

use std::path::PathBuf;
use std::time::Duration;

/// Outcome of a single test.
#[derive(Clone, Debug)]
pub enum TestOutcome {
    /// Test passed successfully.
    Passed,
    /// Test failed with an error message.
    Failed(String),
    /// Test was skipped with a reason.
    Skipped(String),
}

impl TestOutcome {
    pub fn is_passed(&self) -> bool {
        matches!(self, TestOutcome::Passed)
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, TestOutcome::Failed(_))
    }

    pub fn is_skipped(&self) -> bool {
        matches!(self, TestOutcome::Skipped(_))
    }
}

/// Result of running a single test.
#[derive(Clone, Debug)]
pub struct TestResult {
    /// Name of the test.
    pub name: String,
    /// Functions being tested.
    pub targets: Vec<String>,
    /// Outcome of the test.
    pub outcome: TestOutcome,
    /// Time taken to run the test.
    pub duration: Duration,
}

impl TestResult {
    /// Create a passed test result.
    pub fn passed(name: String, targets: Vec<String>, duration: Duration) -> Self {
        TestResult {
            name,
            targets,
            outcome: TestOutcome::Passed,
            duration,
        }
    }

    /// Create a failed test result.
    pub fn failed(name: String, targets: Vec<String>, error: String, duration: Duration) -> Self {
        TestResult {
            name,
            targets,
            outcome: TestOutcome::Failed(error),
            duration,
        }
    }

    /// Create a skipped test result.
    pub fn skipped(name: String, targets: Vec<String>, reason: String) -> Self {
        TestResult {
            name,
            targets,
            outcome: TestOutcome::Skipped(reason),
            duration: Duration::ZERO,
        }
    }
}

/// Summary of test results for a single file.
#[derive(Clone, Debug, Default)]
pub struct FileSummary {
    /// Path to the test file.
    pub path: PathBuf,
    /// Individual test results.
    pub results: Vec<TestResult>,
    /// Number of tests that passed.
    pub passed: usize,
    /// Number of tests that failed.
    pub failed: usize,
    /// Number of tests that were skipped.
    pub skipped: usize,
    /// Total time to run all tests in file.
    pub duration: Duration,
    /// Parse or type errors (not test failures).
    pub errors: Vec<String>,
}

impl FileSummary {
    pub fn new(path: PathBuf) -> Self {
        FileSummary {
            path,
            ..Default::default()
        }
    }

    pub fn add_result(&mut self, result: TestResult) {
        match &result.outcome {
            TestOutcome::Passed => self.passed += 1,
            TestOutcome::Failed(_) => self.failed += 1,
            TestOutcome::Skipped(_) => self.skipped += 1,
        }
        self.duration += result.duration;
        self.results.push(result);
    }

    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    pub fn total(&self) -> usize {
        self.passed + self.failed + self.skipped
    }

    pub fn has_failures(&self) -> bool {
        self.failed > 0 || !self.errors.is_empty()
    }
}

/// Overall summary of all test runs.
#[derive(Clone, Debug, Default)]
pub struct TestSummary {
    /// Results for each file.
    pub files: Vec<FileSummary>,
    /// Total tests passed.
    pub passed: usize,
    /// Total tests failed.
    pub failed: usize,
    /// Total tests skipped.
    pub skipped: usize,
    /// Total time for all tests.
    pub duration: Duration,
}

impl TestSummary {
    pub fn new() -> Self {
        TestSummary::default()
    }

    pub fn add_file(&mut self, summary: FileSummary) {
        self.passed += summary.passed;
        self.failed += summary.failed;
        self.skipped += summary.skipped;
        self.duration += summary.duration;
        self.files.push(summary);
    }

    pub fn total(&self) -> usize {
        self.passed + self.failed + self.skipped
    }

    pub fn has_failures(&self) -> bool {
        self.failed > 0 || self.files.iter().any(|f| !f.errors.is_empty())
    }

    /// Get exit code: 0 = all pass, 1 = failures, 2 = no tests found.
    pub fn exit_code(&self) -> i32 {
        if self.total() == 0 {
            2
        } else if self.has_failures() {
            1
        } else {
            0
        }
    }
}

/// Coverage information for a single function.
#[derive(Clone, Debug)]
pub struct FunctionCoverage {
    /// Function name.
    pub name: String,
    /// Whether function has tests.
    pub has_tests: bool,
    /// Names of tests targeting this function.
    pub test_names: Vec<String>,
}

/// Coverage report for a file or project.
#[derive(Clone, Debug, Default)]
pub struct CoverageReport {
    /// Coverage for each function.
    pub functions: Vec<FunctionCoverage>,
    /// Number of functions with tests.
    pub covered: usize,
    /// Total number of functions.
    pub total: usize,
}

impl CoverageReport {
    pub fn new() -> Self {
        CoverageReport::default()
    }

    pub fn add_function(&mut self, name: String, has_tests: bool, test_names: Vec<String>) {
        if has_tests {
            self.covered += 1;
        }
        self.total += 1;
        self.functions.push(FunctionCoverage { name, has_tests, test_names });
    }

    /// Get coverage percentage (0-100).
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            100.0
        } else {
            (self.covered as f64 / self.total as f64) * 100.0
        }
    }

    /// Check if all functions have tests.
    pub fn is_complete(&self) -> bool {
        self.covered == self.total
    }

    /// Get list of untested function names.
    pub fn untested(&self) -> Vec<&str> {
        self.functions
            .iter()
            .filter(|f| !f.has_tests)
            .map(|f| f.name.as_str())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outcome_predicates() {
        assert!(TestOutcome::Passed.is_passed());
        assert!(!TestOutcome::Passed.is_failed());
        assert!(TestOutcome::Failed("error".into()).is_failed());
        assert!(TestOutcome::Skipped("reason".into()).is_skipped());
    }

    #[test]
    fn test_file_summary() {
        let mut summary = FileSummary::new(PathBuf::from("test.si"));
        summary.add_result(TestResult::passed("test1".into(), vec![], Duration::from_millis(10)));
        summary.add_result(TestResult::failed("test2".into(), vec![], "error".into(), Duration::from_millis(5)));
        summary.add_result(TestResult::skipped("test3".into(), vec![], "skip".into()));

        assert_eq!(summary.passed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.total(), 3);
        assert!(summary.has_failures());
    }

    #[test]
    fn test_summary_exit_code() {
        let mut summary = TestSummary::new();
        assert_eq!(summary.exit_code(), 2); // No tests

        summary.passed = 1;
        assert_eq!(summary.exit_code(), 0); // All pass

        summary.failed = 1;
        assert_eq!(summary.exit_code(), 1); // Failures
    }
}
