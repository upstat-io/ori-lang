//! Test result types.

use serde::{Serialize, Serializer};

use crate::ir::{Name, StringInterner};
use std::path::PathBuf;
use std::time::Duration;

/// Outcome of a single test.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
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
    /// Counted as a real failure; `llvm_compile_fail` counters track the
    /// reason breakdown as a subset of `failed`.
    LlvmCompileFail(String),
}

impl TestOutcome {
    /// Whether this outcome is `Passed`.
    pub fn is_passed(&self) -> bool {
        matches!(self, TestOutcome::Passed)
    }

    /// Whether this outcome is `Failed`.
    pub fn is_failed(&self) -> bool {
        matches!(self, TestOutcome::Failed(_))
    }

    /// Whether this outcome is `Skipped` (explicit skip with a reason).
    pub fn is_skipped(&self) -> bool {
        matches!(self, TestOutcome::Skipped(_))
    }

    /// Whether this outcome is `SkippedUnchanged` (incremental no-change skip).
    pub fn is_skipped_unchanged(&self) -> bool {
        matches!(self, TestOutcome::SkippedUnchanged)
    }

    /// Whether this outcome is `LlvmCompileFail` (LLVM compilation failure).
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
    /// Number of tests blocked by LLVM compilation failure (subset of `failed`).
    pub llvm_compile_fail: usize,
    /// Total time to run all tests in file.
    pub duration: Duration,
    /// Parse or type errors (not test failures).
    pub errors: Vec<String>,
    /// Whether this file's errors are from LLVM compilation failure (not a real test failure).
    pub llvm_compile_error: bool,
    /// Whether the worker spawn failed because the runner binary was replaced or
    /// lost mid-run (a concurrent rebuild). Signals the per-file loop to abort
    /// the whole run with ONE diagnostic instead of cascading N per-file spawn
    /// failures.
    pub binary_replaced: bool,
}

impl FileSummary {
    /// Construct an empty summary for the file at `path`.
    pub fn new(path: PathBuf) -> Self {
        FileSummary {
            path,
            ..Default::default()
        }
    }

    /// Record a test result, incrementing the matching outcome counter.
    pub fn add_result(&mut self, result: TestResult) {
        match &result.outcome {
            TestOutcome::Passed => self.passed += 1,
            TestOutcome::Failed(_) => self.failed += 1,
            TestOutcome::Skipped(_) => self.skipped += 1,
            TestOutcome::SkippedUnchanged => self.skipped_unchanged += 1,
            TestOutcome::LlvmCompileFail(_) => {
                // A test that cannot compile via LLVM is a failed test; the
                // dedicated counter tracks the reason breakdown.
                self.failed += 1;
                self.llvm_compile_fail += 1;
            }
        }
        self.duration += result.duration;
        self.results.push(result);
    }

    /// Record a file-level error (parse / type / LLVM compilation failure).
    pub fn add_error(&mut self, error: String) {
        self.errors.push(error);
    }

    /// Total tests in this file. Includes skipped-unchanged tests: an
    /// incremental no-change run still has tests, they just did not re-run.
    pub fn total(&self) -> usize {
        self.passed + self.failed + self.skipped + self.skipped_unchanged
    }

    /// Returns true if any test failed (including LLVM compile failures) or
    /// the file had parse/type/LLVM errors.
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
    /// Total tests skipped because targets unchanged.
    pub skipped_unchanged: usize,
    /// Total tests blocked by LLVM compilation failure (subset of `failed`).
    pub llvm_compile_fail: usize,
    /// Number of files with type/parse errors (real failures).
    pub error_files: usize,
    /// Number of files where LLVM compilation failed.
    pub llvm_compile_fail_files: usize,
    /// Total time for all tests.
    pub duration: Duration,
}

impl TestSummary {
    /// Construct an empty summary with all counters zeroed.
    pub fn new() -> Self {
        TestSummary::default()
    }

    /// Fold a per-file summary into the aggregate counts.
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

    /// Total tests across all files. Includes skipped-unchanged tests: an
    /// incremental no-change run still has tests, they just did not re-run.
    pub fn total(&self) -> usize {
        self.passed + self.failed + self.skipped + self.skipped_unchanged
    }

    /// Returns true if any test failure or file error occurred.
    ///
    /// LLVM compile failures count (`llvm_compile_fail` is a subset of
    /// `failed`; `llvm_compile_fail_files` covers error-only files).
    /// Expected failures (XFAIL) do not count as failures.
    pub fn has_failures(&self) -> bool {
        self.failed > 0 || self.error_files > 0 || self.llvm_compile_fail_files > 0
    }

    /// Returns true if any file had real (non-expected) errors.
    pub fn has_file_errors(&self) -> bool {
        self.error_files > 0
    }

    /// Get exit code: 0 = all pass, 1 = failures (tests or type errors), 2 = no tests found.
    ///
    /// Skipped-unchanged tests count as present: an incremental run where
    /// every test was skipped-unchanged exits 0, not 2.
    pub fn exit_code(&self) -> i32 {
        if self.total() == 0 && self.error_files == 0 && self.llvm_compile_fail_files == 0 {
            2
        } else {
            i32::from(self.has_failures())
        }
    }

    /// Render the full machine-readable summary as a JSON object: aggregate
    /// counts plus a `files` array carrying every per-file summary, each with
    /// its per-test results (resolved test/target names, outcome variant,
    /// `duration_ns`). `serde_json` escapes arbitrary failure-message control
    /// chars by construction. Interned `Name`s are resolved to strings via
    /// `interner` so the JSON carries names, never opaque integer indices.
    pub fn render_json(&self, interner: &StringInterner) -> String {
        let dto = TestSummaryJson {
            files: self.files.iter().map(|f| f.to_json(interner)).collect(),
            passed: self.passed,
            failed: self.failed,
            skipped: self.skipped,
            skipped_unchanged: self.skipped_unchanged,
            llvm_compile_fail: self.llvm_compile_fail,
            error_files: self.error_files,
            llvm_compile_fail_files: self.llvm_compile_fail_files,
            duration: self.duration,
        };
        // INVARIANT: serializing a pure-data DTO (primitives, &str, and a
        // serialize_u64 duration shim — no map keys, no fallible serializer)
        // cannot fail. A failure here is a programming error, not a runtime
        // condition, so never emit a lossy fallback object.
        serde_json::to_string(&dto)
            .unwrap_or_else(|e| unreachable!("test summary JSON serialization is infallible: {e}"))
    }
}

/// Serialize a `Duration` as integer nanoseconds (u64). The JSON schema pins
/// the unit: every duration field is named `duration_ns` and carries a count
/// of nanoseconds, so consumers parse one stable integer unit.
fn serialize_duration_ns<S: Serializer>(d: &Duration, s: S) -> Result<S::Ok, S::Error> {
    let ns = u64::try_from(d.as_nanos()).unwrap_or(u64::MAX);
    s.serialize_u64(ns)
}

/// JSON view of a single test result. Interned `Name`s are resolved to their
/// string form before emission so the payload never leaks integer indices.
#[derive(Serialize)]
struct TestResultJson<'a> {
    name: &'a str,
    targets: Vec<&'a str>,
    outcome: &'a TestOutcome,
    #[serde(rename = "duration_ns", serialize_with = "serialize_duration_ns")]
    duration: Duration,
}

/// JSON view of a per-file summary, carrying its per-test results.
#[derive(Serialize)]
struct FileSummaryJson<'a> {
    path: String,
    results: Vec<TestResultJson<'a>>,
    passed: usize,
    failed: usize,
    skipped: usize,
    skipped_unchanged: usize,
    llvm_compile_fail: usize,
    #[serde(rename = "duration_ns", serialize_with = "serialize_duration_ns")]
    duration: Duration,
    errors: &'a [String],
    llvm_compile_error: bool,
}

/// JSON view of the overall summary: aggregate counts plus every per-file view.
#[derive(Serialize)]
struct TestSummaryJson<'a> {
    files: Vec<FileSummaryJson<'a>>,
    passed: usize,
    failed: usize,
    skipped: usize,
    skipped_unchanged: usize,
    llvm_compile_fail: usize,
    error_files: usize,
    llvm_compile_fail_files: usize,
    #[serde(rename = "duration_ns", serialize_with = "serialize_duration_ns")]
    duration: Duration,
}

impl TestResult {
    /// Build the JSON view, resolving `name` and `targets` through `interner`.
    fn to_json<'a>(&'a self, interner: &'a StringInterner) -> TestResultJson<'a> {
        TestResultJson {
            name: interner.lookup(self.name),
            targets: self.targets.iter().map(|t| interner.lookup(*t)).collect(),
            outcome: &self.outcome,
            duration: self.duration,
        }
    }
}

impl FileSummary {
    /// Build the JSON view, resolving every result's interned names.
    fn to_json<'a>(&'a self, interner: &'a StringInterner) -> FileSummaryJson<'a> {
        FileSummaryJson {
            path: self.path.display().to_string(),
            results: self.results.iter().map(|r| r.to_json(interner)).collect(),
            passed: self.passed,
            failed: self.failed,
            skipped: self.skipped,
            skipped_unchanged: self.skipped_unchanged,
            llvm_compile_fail: self.llvm_compile_fail,
            duration: self.duration,
            errors: &self.errors,
            llvm_compile_error: self.llvm_compile_error,
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
    /// Construct an empty coverage report.
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
