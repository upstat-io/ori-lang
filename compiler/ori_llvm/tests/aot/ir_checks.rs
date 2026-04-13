//! `FileCheck`-style IR assertion tests.
//!
//! Compiles `.ori` files from `tests/codegen/`, captures LLVM IR, and
//! matches `// CHECK:` directives against the output using the shared
//! harness's `run_test_directory()` orchestration loop.
//!
//! **Function-scoped IR slicing**: Each test file may include a
//! `// @function: <name>` custom directive specifying which function's
//! IR to check. If absent, defaults to `_ori_main`. The captured module
//! IR is sliced to the target function via `extract_function_ir()` before
//! matching, ensuring CHECK-NOT patterns are truly function-scoped.
//!
//! **`CheckMode` selection**: If a test file contains any `CHECK-LABEL` or
//! `CHECK-NEXT` directive, `.exact` mode is used (the author is asserting
//! ordering or adjacency). Otherwise `.matches` mode is used.

use std::fmt;
use std::fmt::Write as _;
use std::path::Path;

use ori_test_harness::bless;
use ori_test_harness::check::{self, CheckMode};
use ori_test_harness::directive::{Directive, DirectiveLine};
use ori_test_harness::revision::RevisionConfig;
use ori_test_harness::runner::{self, TestOutput, TestStrategy};

use crate::util::{compile_and_capture_ir, extract_function_ir};

/// `FileCheck`-style IR test strategy.
///
/// Compiles `.ori` source files, captures LLVM IR, slices to the target
/// function, and matches `// CHECK:` directives against the result.
pub struct FileCheckStrategy;

/// Error type for `FileCheck` test failures.
#[derive(Debug)]
pub struct FileCheckError(String);

impl fmt::Display for FileCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TestStrategy for FileCheckStrategy {
    type Error = FileCheckError;

    fn execute(
        &self,
        test_path: &Path,
        _revision: &RevisionConfig,
        directives: &[&DirectiveLine],
    ) -> Result<TestOutput, Self::Error> {
        // Read source from the test file.
        let source = std::fs::read_to_string(test_path)
            .map_err(|e| FileCheckError(format!("failed to read {}: {e}", test_path.display())))?;

        // Extract @function: directive — determines which function's IR to check.
        let target_fn = directives
            .iter()
            .find_map(|d| match &d.directive {
                Directive::Custom { key, value } if key == "function" => Some(value.as_str()),
                _ => None,
            })
            .unwrap_or("_ori_main");

        // Compile and capture the full module IR.
        let full_ir = compile_and_capture_ir(&source);

        // Slice to the target function for function-scoped matching.
        let function_ir = extract_function_ir(&full_ir, target_fn);

        Ok(TestOutput {
            content: function_ir.to_string(),
            artifacts: Vec::new(),
        })
    }

    fn verify(
        &self,
        test_path: &Path,
        _revision: &RevisionConfig,
        directives: &[&DirectiveLine],
        output: &TestOutput,
    ) -> Result<(), Self::Error> {
        // Collect CHECK directives.
        let check_directives: Vec<DirectiveLine> = directives
            .iter()
            .filter(|d| {
                matches!(
                    d.directive,
                    Directive::Check { .. }
                        | Directive::CheckNot { .. }
                        | Directive::CheckLabel { .. }
                        | Directive::CheckNext { .. }
                )
            })
            .copied()
            .cloned()
            .collect();

        if check_directives.is_empty() {
            return Err(FileCheckError(format!(
                "no CHECK directives in {}",
                test_path.display()
            )));
        }

        // Determine CheckMode: if any CHECK-LABEL or CHECK-NEXT is present,
        // use .exact mode (author is asserting ordering/adjacency). Otherwise
        // use .matches mode for backward compatibility.
        let mode = determine_check_mode(&check_directives);

        let result = check::run_checks(&output.content, &check_directives, mode);

        if result.passed {
            Ok(())
        } else {
            let failures: Vec<String> = result.failures.iter().map(|f| format!("  {f}")).collect();
            let ir_preview = &output.content[..output.content.len().min(2000)];
            Err(FileCheckError(format!(
                "FileCheck failed ({mode:?} mode):\n{}\n\n--- IR ---\n{ir_preview}",
                failures.join("\n"),
            )))
        }
    }

    fn baseline_suffix(&self) -> Option<&str> {
        // FileCheck tests use pattern matching, not baseline comparison.
        None
    }
}

/// Determine the [`CheckMode`] from a set of directives.
///
/// If any `CHECK-LABEL` or `CHECK-NEXT` directive is present, returns
/// `Exact` (the author is asserting ordering or adjacency). Otherwise
/// returns `Matches` for backward compatibility with existing tests.
fn determine_check_mode(directives: &[DirectiveLine]) -> CheckMode {
    let has_ordering_directive = directives.iter().any(|d| {
        matches!(
            d.directive,
            Directive::CheckLabel { .. } | Directive::CheckNext { .. }
        )
    });

    if has_ordering_directive {
        CheckMode::Exact
    } else {
        CheckMode::Matches
    }
}

/// Run all `FileCheck` tests in the `codegen/` directory via the shared harness.
///
/// Uses `run_test_directory()` for automatic test discovery — any `.ori` file
/// added to `compiler/ori_llvm/tests/codegen/` (or subdirectories) is
/// automatically picked up. No manual test function registration needed.
#[test]
fn run_all_codegen_filecheck() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let codegen_dir = manifest.join("tests/codegen");

    let strategy = FileCheckStrategy;
    let summary = runner::run_test_directory(&codegen_dir, &strategy, bless::is_bless_enabled());

    if !summary.is_success() {
        let mut msg = format!(
            "FileCheck tests: {} passed, {} failed\n",
            summary.passed, summary.failed
        );
        for failure in &summary.failures {
            let _ = write!(msg, "\n  FAIL: {failure}");
        }
        for error in &summary.errors {
            let _ = write!(msg, "\n  ERROR: {error}");
        }
        for warning in &summary.warnings {
            let _ = write!(msg, "\n  WARN: {warning}");
        }
        panic!("{msg}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ori_test_harness::directive::{Directive, DirectiveLine};

    fn dl(line: usize, dir: Directive) -> DirectiveLine {
        DirectiveLine {
            line_number: line,
            revision: None,
            directive: dir,
        }
    }

    #[test]
    fn check_mode_selects_exact_when_label_present() {
        let directives = vec![
            dl(
                1,
                Directive::Check {
                    pattern: "foo".into(),
                },
            ),
            dl(
                2,
                Directive::CheckLabel {
                    pattern: "define @bar".into(),
                },
            ),
        ];
        assert_eq!(determine_check_mode(&directives), CheckMode::Exact);
    }

    #[test]
    fn check_mode_selects_exact_when_next_present() {
        let directives = vec![
            dl(
                1,
                Directive::Check {
                    pattern: "foo".into(),
                },
            ),
            dl(
                2,
                Directive::CheckNext {
                    pattern: "bar".into(),
                },
            ),
        ];
        assert_eq!(determine_check_mode(&directives), CheckMode::Exact);
    }

    #[test]
    fn check_mode_selects_matches_when_no_ordering_directives() {
        let directives = vec![
            dl(
                1,
                Directive::Check {
                    pattern: "foo".into(),
                },
            ),
            dl(
                2,
                Directive::CheckNot {
                    pattern: "bar".into(),
                },
            ),
        ];
        assert_eq!(determine_check_mode(&directives), CheckMode::Matches);
    }

    #[test]
    fn filecheck_strategy_discovers_all_codegen_tests() {
        // Verify that the codegen directory exists and contains .ori files.
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
        let codegen_dir = manifest.join("tests/codegen");
        assert!(codegen_dir.exists(), "codegen test directory must exist");

        let ori_files: Vec<_> = std::fs::read_dir(&codegen_dir)
            .expect("read codegen dir")
            .filter_map(Result::ok)
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "ori"))
            .collect();

        assert!(
            ori_files.len() >= 12,
            "expected at least 12 codegen test files, found {}",
            ori_files.len()
        );
    }
}
