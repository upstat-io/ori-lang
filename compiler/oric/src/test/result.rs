//! Test result types.

use crate::ir::{Name, StringInterner};
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
    /// Test skipped because all targets are unchanged since last run.
    SkippedUnchanged,
    /// Test could not run because LLVM compilation of its file failed.
    /// Not counted as a real failure — tracked separately.
    LlvmCompileFail(String),
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

    pub fn is_skipped_unchanged(&self) -> bool {
        matches!(self, TestOutcome::SkippedUnchanged)
    }

    pub fn is_llvm_compile_fail(&self) -> bool {
        matches!(self, TestOutcome::LlvmCompileFail(_))
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
    #[cold]
    pub fn failed(name: Name, targets: Vec<Name>, error: String, duration: Duration) -> Self {
        TestResult {
            name,
            targets,
            outcome: TestOutcome::Failed(error),
            duration,
        }
    }

    /// Create a skipped test result.
    #[cold]
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

    /// Iterate over target names as strings.
    pub fn targets_str<'a>(
        &'a self,
        interner: &'a StringInterner,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.targets.iter().map(move |t| interner.lookup(*t))
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
    /// Number of tests skipped because targets unchanged.
    pub skipped_unchanged: usize,
    /// Number of tests blocked by LLVM compilation failure.
    pub llvm_compile_fail: usize,
    /// Total time to run all tests in file.
    pub duration: Duration,
    /// Parse or type errors (not test failures).
    pub errors: Vec<String>,
    /// Whether this file's errors are from LLVM compilation failure (not a real test failure).
    pub llvm_compile_error: bool,
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
            TestOutcome::SkippedUnchanged => self.skipped_unchanged += 1,
            TestOutcome::LlvmCompileFail(_) => self.llvm_compile_fail += 1,
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

    /// Returns true if there are real failures (excludes LLVM compile failures).
    pub fn has_failures(&self) -> bool {
        self.failed > 0 || (!self.errors.is_empty() && !self.llvm_compile_error)
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
    /// Total tests skipped because targets unchanged.
    pub skipped_unchanged: usize,
    /// Total tests blocked by LLVM compilation failure.
    pub llvm_compile_fail: usize,
    /// Number of files with type/parse errors (real failures).
    pub error_files: usize,
    /// Number of files where LLVM compilation failed.
    pub llvm_compile_fail_files: usize,
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
        self.skipped_unchanged += summary.skipped_unchanged;
        self.llvm_compile_fail += summary.llvm_compile_fail;
        if !summary.errors.is_empty() {
            if summary.llvm_compile_error {
                self.llvm_compile_fail_files += 1;
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
        if self.total() == 0
            && self.error_files == 0
            && self.llvm_compile_fail == 0
            && self.llvm_compile_fail_files == 0
        {
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

    /// Iterate over untested function names.
    pub fn untested(&self) -> impl Iterator<Item = Name> + '_ {
        self.functions
            .iter()
            .filter(|f| !f.has_tests())
            .map(|f| f.name)
    }

    /// Iterate over untested function names as strings.
    pub fn untested_str<'a>(
        &'a self,
        interner: &'a StringInterner,
    ) -> impl Iterator<Item = &'a str> + 'a {
        self.functions
            .iter()
            .filter(|f| !f.has_tests())
            .map(move |f| interner.lookup(f.name))
    }
}

#[cfg(test)]
mod tests;
