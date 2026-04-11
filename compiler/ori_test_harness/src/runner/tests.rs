#![expect(
    clippy::expect_used,
    reason = "test code — panics are the assertion mechanism"
)]

use std::io::Write as _;
use std::path::Path;

use super::*;
use crate::directive::DirectiveLine;
use crate::revision::RevisionConfig;

// ---------------------------------------------------------------------------
// MockTestStrategy for harness-only validation
// ---------------------------------------------------------------------------

struct MockTestStrategy {
    /// If true, `execute()` succeeds. If false, returns an error.
    succeed: bool,
}

impl TestStrategy for MockTestStrategy {
    type Error = String;

    fn execute(
        &self,
        _test_path: &Path,
        _revision: &RevisionConfig,
        _directives: &[DirectiveLine],
    ) -> Result<TestOutput, String> {
        if self.succeed {
            Ok(TestOutput {
                content: "mock output".into(),
                artifacts: vec![],
            })
        } else {
            Err("mock execute error".into())
        }
    }

    fn verify(
        &self,
        _test_path: &Path,
        _revision: &RevisionConfig,
        _directives: &[DirectiveLine],
        _output: &TestOutput,
    ) -> Result<(), String> {
        Ok(())
    }
}

struct FailVerifyStrategy;

impl TestStrategy for FailVerifyStrategy {
    type Error = String;

    fn execute(
        &self,
        _test_path: &Path,
        _revision: &RevisionConfig,
        _directives: &[DirectiveLine],
    ) -> Result<TestOutput, String> {
        Ok(TestOutput {
            content: "output".into(),
            artifacts: vec![],
        })
    }

    fn verify(
        &self,
        _test_path: &Path,
        _revision: &RevisionConfig,
        _directives: &[DirectiveLine],
        _output: &TestOutput,
    ) -> Result<(), String> {
        Err("verify failed: pattern not found".into())
    }
}

fn create_test_file(dir: &Path, name: &str, content: &str) {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent dirs");
    }
    let mut f = std::fs::File::create(&path).expect("create test file");
    f.write_all(content.as_bytes()).expect("write test file");
}

// ---------------------------------------------------------------------------
// Positive (semantic pins)
// ---------------------------------------------------------------------------

#[test]
fn test_run_single_file_invokes_strategy_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    create_test_file(dir.path(), "basic.ori", "// CHECK: some_pattern\n");

    let strategy = MockTestStrategy { succeed: true };
    let summary = run_test_directory(dir.path(), &strategy);
    assert!(
        summary.is_success(),
        "failures: {:?}, errors: {:?}",
        summary.failures,
        summary.errors
    );
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.failed, 0);
}

#[test]
fn test_run_with_revisions_invokes_strategy_per_revision() {
    let dir = tempfile::tempdir().expect("tempdir");
    create_test_file(
        dir.path(),
        "multi.ori",
        "// @revisions: debug release\n// CHECK: pattern\n",
    );

    let strategy = MockTestStrategy { succeed: true };
    let summary = run_test_directory(dir.path(), &strategy);
    assert!(summary.is_success());
    assert_eq!(summary.passed, 2); // once per revision
}

#[test]
fn test_run_summary_reports_correct_pass_fail_counts() {
    let dir = tempfile::tempdir().expect("tempdir");
    create_test_file(dir.path(), "pass.ori", "// CHECK: ok\n");
    create_test_file(dir.path(), "fail.ori", "// CHECK: ok\n");

    let strategy = FailVerifyStrategy;
    let summary = run_test_directory(dir.path(), &strategy);
    // Both files have CHECK directives, both get executed, both fail at verify
    assert_eq!(summary.failed, 2);
    assert!(!summary.is_success());
}

#[test]
fn test_mock_strategy_bless_mode_writes_baseline() {
    let dir = tempfile::tempdir().expect("tempdir");
    create_test_file(dir.path(), "bless.ori", "// CHECK: some_pattern\n");

    // Create a strategy that produces output with artifact paths
    let bless_dir = tempfile::tempdir().expect("bless tempdir");
    let baseline_path = bless_dir.path().join("bless.ll");

    std::env::set_var("ORI_BLESS", "1");
    let result = crate::bless::compare_or_bless(&baseline_path, "define void @main() {}");
    std::env::remove_var("ORI_BLESS");

    assert!(result.is_ok());
    assert!(baseline_path.exists());
    assert_eq!(
        std::fs::read_to_string(&baseline_path).expect("read"),
        "define void @main() {}"
    );
}

#[test]
fn test_mock_strategy_mismatch_produces_diff_in_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    create_test_file(dir.path(), "mismatch.ori", "// CHECK: pattern\n");

    let strategy = FailVerifyStrategy;
    let summary = run_test_directory(dir.path(), &strategy);
    assert!(!summary.is_success());
    assert_eq!(summary.failed, 1);
    // Verify the failure message contains the verification error
    assert!(summary.failures[0].contains("verify failed"));
    assert!(summary.failures[0].contains("pattern not found"));
}

#[test]
fn test_mock_strategy_directive_filtering_by_revision() {
    let dir = tempfile::tempdir().expect("tempdir");
    create_test_file(
        dir.path(),
        "filtered.ori",
        "\
// @revisions: alpha beta
// @[alpha] compile-flags: --opt
// CHECK: base_pattern
// @[alpha] CHECK: alpha_only
// @[beta] CHECK: beta_only
",
    );

    // Strategy that succeeds — validates the runner calls execute per revision
    let strategy = MockTestStrategy { succeed: true };
    let summary = run_test_directory(dir.path(), &strategy);
    assert!(summary.is_success(), "failures: {:?}", summary.failures);
    // Two revisions → two execute calls → two passes
    assert_eq!(summary.passed, 2);
}

// ---------------------------------------------------------------------------
// Negative pins
// ---------------------------------------------------------------------------

#[test]
fn test_run_empty_directory_fails_as_empty_corpus() {
    let dir = tempfile::tempdir().expect("tempdir");

    let strategy = MockTestStrategy { succeed: true };
    let summary = run_test_directory(dir.path(), &strategy);
    assert!(!summary.is_success());
    assert_eq!(summary.failed, 1);
    assert!(summary.failures[0].contains("no .ori test files"));
}

#[test]
fn test_run_file_with_zero_directives_fails_as_orphan() {
    let dir = tempfile::tempdir().expect("tempdir");
    create_test_file(
        dir.path(),
        "orphan.ori",
        "// This file has no directives\nlet x = 42;\n",
    );

    let strategy = MockTestStrategy { succeed: true };
    let summary = run_test_directory(dir.path(), &strategy);
    assert!(!summary.is_success());
    assert_eq!(summary.failed, 1);
    assert!(summary.failures[0].contains("no directives found"));
}

#[test]
fn test_run_strategy_execute_error_counted_as_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    create_test_file(dir.path(), "exec_fail.ori", "// CHECK: pattern\n");

    let strategy = MockTestStrategy { succeed: false };
    let summary = run_test_directory(dir.path(), &strategy);
    assert!(!summary.is_success());
    assert_eq!(summary.failed, 1);
    assert!(summary.failures[0].contains("execute failed"));
}

#[test]
fn test_run_strategy_verify_error_counted_as_failure() {
    let dir = tempfile::tempdir().expect("tempdir");
    create_test_file(dir.path(), "verify_fail.ori", "// CHECK: pattern\n");

    let strategy = FailVerifyStrategy;
    let summary = run_test_directory(dir.path(), &strategy);
    assert!(!summary.is_success());
    assert_eq!(summary.failed, 1);
    assert!(summary.failures[0].contains("verify failed"));
}

#[test]
fn test_run_file_with_parse_errors_reports_them() {
    let dir = tempfile::tempdir().expect("tempdir");
    // Forbidden revision name triggers a parse error
    create_test_file(
        dir.path(),
        "bad.ori",
        "// @[CHECK] compile-flags: --release\n",
    );

    let strategy = MockTestStrategy { succeed: true };
    let summary = run_test_directory(dir.path(), &strategy);
    assert!(!summary.is_success());
    assert_eq!(summary.failed, 1);
    assert!(!summary.errors.is_empty());
    assert!(summary.errors[0].contains("forbidden revision name"));
}
