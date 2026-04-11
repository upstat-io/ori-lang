//! Test runner orchestration via the `TestStrategy` trait.
//!
//! [`run_test_directory`] is the SINGLE canonical test loop. Consumer crates
//! (`ori_arc`, `ori_llvm`) implement [`TestStrategy`] to provide
//! compiler-specific behavior; the harness owns the orchestration algorithm.

use std::fmt;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::artifact::ArtifactPaths;
use crate::directive::{self, DirectiveLine};
use crate::revision;

#[cfg(test)]
mod tests;

// ---------------------------------------------------------------------------
// TestStrategy trait
// ---------------------------------------------------------------------------

/// Consumer-provided strategy for test execution.
///
/// The harness orchestrates the test loop (discover → parse → expand →
/// invoke → diff). The consumer implements this trait to provide
/// compiler-specific behavior.
///
/// Implementations:
/// - `ori_arc` provides `AimsSnapshotStrategy` (§03)
/// - `ori_llvm` provides `FileCheckStrategy` (§07)
pub trait TestStrategy {
    /// The type of error this strategy can produce.
    type Error: fmt::Display;

    /// Execute the test for a specific revision and produce output.
    ///
    /// Responsible for: (1) translating the revision config into compiler
    /// flags/env vars, (2) compiling the test file, (3) capturing the
    /// relevant output. Revision translation is local to this call.
    fn execute(
        &self,
        test_path: &Path,
        revision: &revision::RevisionConfig,
        directives: &[&DirectiveLine],
    ) -> Result<TestOutput, Self::Error>;

    /// Compare the actual output against expectations.
    ///
    /// For snapshot tests (§03): compare against baseline files.
    /// For `FileCheck` tests (§07): match CHECK directives against IR.
    fn verify(
        &self,
        test_path: &Path,
        revision: &revision::RevisionConfig,
        directives: &[&DirectiveLine],
        output: &TestOutput,
    ) -> Result<(), Self::Error>;
}

// ---------------------------------------------------------------------------
// TestOutput / TestSummary
// ---------------------------------------------------------------------------

/// Output produced by a test execution.
#[derive(Debug, Clone)]
pub struct TestOutput {
    /// The captured output (IR text, snapshot text, etc.)
    pub content: String,
    /// Artifact paths produced (for bless mode).
    pub artifacts: Vec<ArtifactPaths>,
}

/// Summary of a test directory run.
#[derive(Debug, Default)]
pub struct TestSummary {
    pub passed: usize,
    pub failed: usize,
    pub failures: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

impl TestSummary {
    /// Returns true if no tests failed and no errors occurred.
    pub fn is_success(&self) -> bool {
        self.failed == 0 && self.errors.is_empty()
    }
}

// ---------------------------------------------------------------------------
// run_test_directory — the canonical orchestration loop
// ---------------------------------------------------------------------------

/// Run all tests in a directory using the given strategy.
///
/// This is the SINGLE canonical test loop. Consumers call this with their
/// `TestStrategy` impl. They never duplicate the traverse → parse → expand
/// → invoke → diff algorithm.
pub fn run_test_directory<S: TestStrategy>(dir: &Path, strategy: &S) -> TestSummary {
    let mut summary = TestSummary::default();

    // 1. Discover test files
    let test_files = discover_test_files(dir);
    if test_files.is_empty() {
        summary.failed += 1;
        summary.failures.push(format!(
            "no .ori test files found in {} (empty corpus = failure)",
            dir.display()
        ));
        return summary;
    }

    for test_path in &test_files {
        // 2a. Read source and parse directives
        let source = match std::fs::read_to_string(test_path) {
            Ok(s) => s,
            Err(e) => {
                summary
                    .errors
                    .push(format!("{}: read failed: {e}", test_path.display()));
                continue;
            }
        };
        let parse_result = directive::parse_directives(&source);

        // 2b. Report parse errors and fail fast if any exist
        if !parse_result.errors.is_empty() {
            for err in &parse_result.errors {
                summary.errors.push(format!(
                    "{}:{}: {}",
                    test_path.display(),
                    err.line_number,
                    err.message
                ));
            }
            summary.failed += 1;
            summary.failures.push(format!(
                "{}: {} parse error(s)",
                test_path.display(),
                parse_result.errors.len()
            ));
            continue;
        }

        // 2c. Fail on zero actionable directives (orphan test prevention)
        if parse_result.directives.is_empty() {
            summary.failed += 1;
            summary.failures.push(format!(
                "{}: no directives found (orphan test — check for typos)",
                test_path.display()
            ));
            continue;
        }

        let directives = parse_result.directives;

        // 3. Expand revisions
        let revisions = revision::expand_revisions(&directives);

        // 4. For each revision: execute → verify
        for rev in &revisions {
            let filtered: Vec<&DirectiveLine> =
                revision::filter_directives_for_revision(&directives, &rev.name);

            match strategy.execute(test_path, rev, &filtered) {
                Ok(output) => match strategy.verify(test_path, rev, &filtered, &output) {
                    Ok(()) => summary.passed += 1,
                    Err(e) => {
                        summary.failed += 1;
                        summary.failures.push(format!(
                            "{}[{}]: {e}",
                            test_path.display(),
                            rev.name
                        ));
                    }
                },
                Err(e) => {
                    summary.failed += 1;
                    summary.failures.push(format!(
                        "{}[{}]: execute failed: {e}",
                        test_path.display(),
                        rev.name
                    ));
                }
            }
        }
    }

    summary
}

// ---------------------------------------------------------------------------
// File discovery
// ---------------------------------------------------------------------------

fn discover_test_files(dir: &Path) -> Vec<PathBuf> {
    let dir_component_count = dir.components().count();
    let mut files: Vec<PathBuf> = WalkDir::new(dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "ori"))
        .filter(|e| {
            // Only filter hidden/target components WITHIN the walk root,
            // not the root path itself (which may be a tempdir like .tmpXXX)
            !e.path().components().skip(dir_component_count).any(|c| {
                c.as_os_str()
                    .to_str()
                    .is_some_and(|s| s.starts_with('.') || s == "target")
            })
        })
        .map(walkdir::DirEntry::into_path)
        .collect();
    files.sort();
    files
}
