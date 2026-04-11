//! Test runner orchestration via the `TestStrategy` trait.
//!
//! [`run_test_directory`] is the SINGLE canonical test loop. Consumer crates
//! (`ori_arc`, `ori_llvm`) implement [`TestStrategy`] to provide
//! compiler-specific behavior; the harness owns the orchestration algorithm.

use std::fmt;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

use crate::artifact::ArtifactPaths;
use crate::bless;
use crate::directive::{self, DirectiveLine};
use crate::revision;

#[cfg(test)]
mod tests;

// TestStrategy trait

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

    /// Return the baseline file suffix used by this strategy (e.g., "ll", "arc").
    ///
    /// When `Some(suffix)` is returned and bless mode is active, the runner
    /// calls `clean_stale_baselines()` once per test file to remove stale
    /// non-revision baselines. Return `None` if the strategy does not
    /// produce baseline files.
    fn baseline_suffix(&self) -> Option<&str> {
        None
    }

    /// Hook for consumer-specific stale revision cleanup in bless mode.
    ///
    /// Called once per test file AFTER revision expansion, with the full
    /// list of active revision names. Consumers implement this to clean
    /// up stale revision-specific baselines using their own naming
    /// convention. The default is a no-op.
    fn clean_stale_revisions(
        &self,
        _test_path: &Path,
        _active_revisions: &[&str],
    ) -> Result<(), Self::Error> {
        Ok(())
    }
}

// TestOutput / TestSummary

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

// run_test_directory — the canonical orchestration loop

/// Run all tests in a directory using the given strategy.
///
/// This is the SINGLE canonical test loop. Consumers call this with their
/// `TestStrategy` impl. They never duplicate the traverse → parse → expand
/// → invoke → diff algorithm.
///
/// When `bless` is true and the strategy provides a `baseline_suffix()`,
/// stale revision-specific baseline files are cleaned up per test file.
pub fn run_test_directory<S: TestStrategy>(dir: &Path, strategy: &S, bless: bool) -> TestSummary {
    let mut summary = TestSummary::default();

    // 1. Discover test files (traversal errors → hard errors, not warnings)
    let test_files = discover_test_files(dir, &mut summary.errors);
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

        // 3a-3b. Validate revisions and run bless cleanup
        validate_and_cleanup(
            test_path,
            &directives,
            &revisions,
            bless,
            strategy,
            &mut summary,
        );

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

// Validation and bless cleanup

fn validate_and_cleanup<S: TestStrategy>(
    test_path: &Path,
    directives: &[DirectiveLine],
    revisions: &[revision::RevisionConfig],
    bless: bool,
    strategy: &S,
    summary: &mut TestSummary,
) {
    // Validate that gated directives reference declared revisions
    let declared: std::collections::HashSet<&str> =
        revisions.iter().map(|r| r.name.as_str()).collect();
    let has_declared_revisions = !(declared.len() == 1 && declared.contains(""));
    for d in directives {
        if let Some(ref gate) = d.revision {
            if has_declared_revisions {
                // With declared revisions: warn if gate doesn't match any
                if !declared.contains(gate.as_str()) {
                    summary.warnings.push(format!(
                        "{}:{}: revision gate '{}' not declared in // @revisions:",
                        test_path.display(),
                        d.line_number,
                        gate
                    ));
                }
            } else {
                // No declared revisions: gated directives are always orphaned
                summary.warnings.push(format!(
                    "{}:{}: revision gate '{}' but no // @revisions: declared",
                    test_path.display(),
                    d.line_number,
                    gate
                ));
            }
        }
    }

    // In bless mode, clean stale baselines
    if bless {
        let rev_names: Vec<&str> = revisions.iter().map(|r| r.name.as_str()).collect();
        if let Some(suffix) = strategy.baseline_suffix() {
            if let Err(e) = bless::clean_stale_baselines(test_path, suffix, &rev_names) {
                summary.warnings.push(format!(
                    "{}: stale baseline cleanup failed: {e}",
                    test_path.display()
                ));
            }
        }
        if let Err(e) = strategy.clean_stale_revisions(test_path, &rev_names) {
            summary.warnings.push(format!(
                "{}: consumer revision cleanup failed: {e}",
                test_path.display()
            ));
        }
    }
}

// File discovery

fn discover_test_files(dir: &Path, errors: &mut Vec<String>) -> Vec<PathBuf> {
    let dir_component_count = dir.components().count();
    let mut files = Vec::new();

    for entry in WalkDir::new(dir) {
        match entry {
            Err(e) => {
                errors.push(format!(
                    "walk error in {}: {e}",
                    e.path()
                        .map_or_else(|| dir.display().to_string(), |p| p.display().to_string())
                ));
            }
            Ok(e) => {
                if !e.file_type().is_file() {
                    continue;
                }
                if e.path().extension().is_none_or(|ext| ext != "ori") {
                    continue;
                }
                // Only filter hidden/target components WITHIN the walk root,
                // not the root path itself (which may be a tempdir like .tmpXXX)
                let dominated_by_hidden =
                    e.path().components().skip(dir_component_count).any(|c| {
                        c.as_os_str()
                            .to_str()
                            .is_some_and(|s| s.starts_with('.') || s == "target")
                    });
                if !dominated_by_hidden {
                    files.push(e.into_path());
                }
            }
        }
    }

    files.sort();
    files
}
