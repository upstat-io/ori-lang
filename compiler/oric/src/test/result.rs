//! Test result types.

use ori_ir::{Name, StringInterner};
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
    /// Test failed as expected (XFAIL) — not counted as a real failure.
    ExpectedFailure(String),
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

    pub fn is_expected_failure(&self) -> bool {
        matches!(self, TestOutcome::ExpectedFailure(_))
    }
}

/// Result of running a single test.
#[derive(Clone, Debug)]
pub struct TestResult {
    /// Name of the test (interned).
    pub name: Name,
    /// Functions being tested (interned).
    pub targets: Vec<Name>,
    /// Outcome of the test.
    pub outcome: TestOutcome,
    /// Time taken to run the test.
    pub duration: Duration,
}

impl TestResult {
    /// Create a passed test result.
    pub fn passed(name: Name, targets: Vec<Name>, duration: Duration) -> Self {
        TestResult {
            name,
            targets,
            outcome: TestOutcome::Passed,
            duration,
        }
    }

    /// Create a failed test result.
    pub fn failed(name: Name, targets: Vec<Name>, error: String, duration: Duration) -> Self {
        TestResult {
            name,
            targets,
            outcome: TestOutcome::Failed(error),
            duration,
        }
    }

    /// Create a skipped test result.
    pub fn skipped(name: Name, targets: Vec<Name>, reason: String) -> Self {
        TestResult {
            name,
            targets,
            outcome: TestOutcome::Skipped(reason),
            duration: Duration::ZERO,
        }
    }

    /// Get the test name as a string.
    pub fn name_str<'a>(&self, interner: &'a StringInterner) -> &'a str {
        interner.lookup(self.name)
    }

    /// Get the target names as strings.
    pub fn targets_str<'a>(&self, interner: &'a StringInterner) -> Vec<&'a str> {
        self.targets.iter().map(|t| interner.lookup(*t)).collect()
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
    /// Number of tests that failed as expected (XFAIL).
    pub xfail: usize,
    /// Total time to run all tests in file.
    pub duration: Duration,
    /// Parse or type errors (not test failures).
    pub errors: Vec<String>,
    /// Whether this file's errors are expected (XFAIL file-level error).
    pub expected_file_error: bool,
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
            TestOutcome::ExpectedFailure(_) => self.xfail += 1,
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

    /// Returns true if there are real failures (excludes expected failures/errors).
    pub fn has_failures(&self) -> bool {
        self.failed > 0 || (!self.errors.is_empty() && !self.expected_file_error)
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
    /// Total tests that failed as expected (XFAIL).
    pub xfail: usize,
    /// Number of files with type/parse errors (real failures, not expected).
    pub error_files: usize,
    /// Number of files with expected errors (XFAIL file-level).
    pub xfail_files: usize,
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
        self.xfail += summary.xfail;
        if !summary.errors.is_empty() {
            if summary.expected_file_error {
                self.xfail_files += 1;
            } else {
                self.error_files += 1;
            }
        }
        self.duration += summary.duration;
        self.files.push(summary);
    }

    pub fn total(&self) -> usize {
        self.passed + self.failed + self.skipped
    }

    /// Returns true if any real test failure or real file error occurred.
    ///
    /// Expected failures (XFAIL) do not count as failures.
    pub fn has_failures(&self) -> bool {
        self.failed > 0 || self.error_files > 0
    }

    /// Returns true if any file had real (non-expected) errors.
    pub fn has_file_errors(&self) -> bool {
        self.error_files > 0
    }

    /// Get exit code: 0 = all pass, 1 = failures (tests or type errors), 2 = no tests found.
    pub fn exit_code(&self) -> i32 {
        if self.total() == 0 && self.error_files == 0 && self.xfail == 0 && self.xfail_files == 0 {
            2
        } else {
            i32::from(self.has_failures())
        }
    }
}

/// Coverage information for a single function.
#[derive(Clone, Debug)]
pub struct FunctionCoverage {
    /// Function name (interned).
    pub name: Name,
    /// Names of tests targeting this function (interned).
    pub test_names: Vec<Name>,
}

impl FunctionCoverage {
    /// Returns whether this function has tests.
    pub fn has_tests(&self) -> bool {
        !self.test_names.is_empty()
    }

    /// Get the function name as a string.
    pub fn name_str<'a>(&self, interner: &'a StringInterner) -> &'a str {
        interner.lookup(self.name)
    }
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

    /// Add a function's coverage information.
    ///
    /// The `has_tests` status is derived from whether `test_names` is non-empty.
    pub fn add_function(&mut self, name: Name, test_names: Vec<Name>) {
        let has_tests = !test_names.is_empty();
        if has_tests {
            self.covered += 1;
        }
        self.total += 1;
        self.functions.push(FunctionCoverage { name, test_names });
    }

    /// Get coverage percentage (0-100).
    pub fn percentage(&self) -> f64 {
        if self.total == 0 {
            return 100.0;
        }
        // Clamp to u32 range for lossless f64 conversion.
        // u32::MAX (~4 billion) is well within f64's 52-bit mantissa.
        // Any realistic test count fits in u32; clamping preserves the ratio.
        let covered = u32::try_from(self.covered).unwrap_or(u32::MAX);
        let total = u32::try_from(self.total).unwrap_or(u32::MAX);
        (f64::from(covered) / f64::from(total)) * 100.0
    }

    /// Check if all functions have tests.
    pub fn is_complete(&self) -> bool {
        self.covered == self.total
    }

    /// Get list of untested function names.
    pub fn untested(&self) -> Vec<Name> {
        self.functions
            .iter()
            .filter(|f| !f.has_tests())
            .map(|f| f.name)
            .collect()
    }

    /// Get list of untested function names as strings.
    pub fn untested_str<'a>(&self, interner: &'a StringInterner) -> Vec<&'a str> {
        self.functions
            .iter()
            .filter(|f| !f.has_tests())
            .map(|f| interner.lookup(f.name))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_interner() -> StringInterner {
        StringInterner::new()
    }

    #[test]
    fn test_outcome_predicates() {
        assert!(TestOutcome::Passed.is_passed());
        assert!(!TestOutcome::Passed.is_failed());
        assert!(TestOutcome::Failed("error".into()).is_failed());
        assert!(TestOutcome::Skipped("reason".into()).is_skipped());
        assert!(TestOutcome::ExpectedFailure("xfail".into()).is_expected_failure());
        assert!(!TestOutcome::ExpectedFailure("xfail".into()).is_failed());
    }

    #[test]
    fn test_file_summary() {
        let interner = test_interner();
        let test1 = interner.intern("test1");
        let test2 = interner.intern("test2");
        let test3 = interner.intern("test3");

        let mut summary = FileSummary::new(PathBuf::from("test.ori"));
        summary.add_result(TestResult::passed(test1, vec![], Duration::from_millis(10)));
        summary.add_result(TestResult::failed(
            test2,
            vec![],
            "error".into(),
            Duration::from_millis(5),
        ));
        summary.add_result(TestResult::skipped(test3, vec![], "skip".into()));

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
        assert_eq!(summary.exit_code(), 1); // Test failures

        // File errors should also cause failure
        let mut summary2 = TestSummary::new();
        summary2.passed = 5;
        summary2.error_files = 1;
        assert_eq!(summary2.exit_code(), 1); // File errors = failure
    }

    #[test]
    fn test_xfail_not_counted_as_failure() {
        let interner = test_interner();
        let test1 = interner.intern("test1");

        let mut file = FileSummary::new(PathBuf::from("test.ori"));
        file.add_result(TestResult {
            name: test1,
            targets: vec![],
            outcome: TestOutcome::ExpectedFailure("xfail reason".into()),
            duration: Duration::from_millis(5),
        });

        assert_eq!(file.xfail, 1);
        assert_eq!(file.failed, 0);
        assert!(!file.has_failures());
    }

    #[test]
    fn test_expected_file_error_not_counted_as_failure() {
        let mut file = FileSummary::new(PathBuf::from("error.ori"));
        file.add_error("type error".into());
        file.expected_file_error = true;

        assert!(!file.has_failures());

        let mut summary = TestSummary::new();
        summary.add_file(file);
        assert_eq!(summary.xfail_files, 1);
        assert_eq!(summary.error_files, 0);
        assert!(!summary.has_failures());
    }

    #[test]
    fn test_summary_xfail_only_is_not_failure() {
        let mut summary = TestSummary::new();
        summary.xfail = 10;
        summary.xfail_files = 5;
        // No real tests passed, but xfail counts mean tests exist
        assert!(!summary.has_failures());
        assert_eq!(summary.exit_code(), 0);
    }

    #[test]
    fn test_summary_xfail_with_real_failures() {
        let mut summary = TestSummary::new();
        summary.passed = 100;
        summary.xfail = 10;
        summary.failed = 1; // One real failure
        assert!(summary.has_failures());
        assert_eq!(summary.exit_code(), 1);
    }

    #[test]
    fn test_result_name_lookup() {
        let interner = test_interner();
        let name = interner.intern("my_test");
        let target = interner.intern("my_function");

        let result = TestResult::passed(name, vec![target], Duration::from_millis(5));

        assert_eq!(result.name_str(&interner), "my_test");
        assert_eq!(result.targets_str(&interner), vec!["my_function"]);
    }

    #[test]
    fn test_coverage_report() {
        let interner = test_interner();
        let func1 = interner.intern("func1");
        let func2 = interner.intern("func2");
        let test1 = interner.intern("test1");

        let mut report = CoverageReport::new();
        report.add_function(func1, vec![test1]); // covered
        report.add_function(func2, vec![]); // not covered

        assert_eq!(report.covered, 1);
        assert_eq!(report.total, 2);
        assert!((report.percentage() - 50.0).abs() < f64::EPSILON);
        assert!(!report.is_complete());
        assert_eq!(report.untested(), vec![func2]);
        assert_eq!(report.untested_str(&interner), vec!["func2"]);
    }
}
