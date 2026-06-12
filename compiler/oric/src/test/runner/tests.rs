use super::super::result::TestOutcome;
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
    std::fs::write(&path, "@add (a: int, b: int) -> int = a + b;").unwrap();

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
@add (a: int, b: int) -> int = a + b;

@test_add tests @add () -> void = {
let result = add(a: 1, b: 2);
print(msg: "done")
}
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
@add (a: int, b: int) -> int = a + b;

@test_add tests @add () -> void = {
let _ = add(a: 1, b: 2);
let _ = 1 / 0;
()
}
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
@foo () -> int = 1;
@bar () -> int = 2;

@test_foo tests @foo () -> void = print(msg: "pass");
@test_bar tests @bar () -> void = print(msg: "pass");
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
    // Use the interner to look up the Name
    let name_str = summary.results[0].name_str(runner.interner());
    assert!(name_str.contains("foo"));
}

#[test]
fn test_blocked_by_type_errors_honors_name_filter() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("blocked.ori");
    // `@bad` carries a type error OUTSIDE any compile_fail test, which blocks
    // every regular test in the file; the blocked-result emission must still
    // honor the name filter.
    std::fs::write(
        &path,
        r#"
@bad () -> int = "not an int";
@foo () -> int = 1;

@test_alpha tests @foo () -> void = print(msg: "x");
@test_beta tests @foo () -> void = print(msg: "x");
"#,
    )
    .unwrap();

    let config = TestRunnerConfig {
        filter: Some("alpha".to_string()),
        ..Default::default()
    };
    let runner = TestRunner::with_config(config);
    let summary = runner.run_file(&path);

    assert_eq!(
        summary.total(),
        1,
        "blocked results must be emitted only for filter-passing tests: {:?}",
        summary.results
    );
    assert_eq!(summary.failed, 1);
    let name = summary.results[0].name_str(runner.interner());
    assert!(
        name.contains("alpha"),
        "the surviving blocked result must be the filtered test: {name}"
    );
    let TestOutcome::Failed(detail) = &summary.results[0].outcome else {
        panic!(
            "blocked test must be Failed: {:?}",
            summary.results[0].outcome
        );
    };
    assert!(
        detail.contains("blocked by type errors"),
        "blocked result must name the type-error block: {detail}"
    );
}

/// `panic_message` extracts String and &str payloads and falls back to a
/// stable label for non-string payloads (the `catch_unwind` failure detail
/// rendered for tests that panic inside the evaluator or LLVM compilation).
#[test]
fn test_panic_message_extracts_string_and_str_payloads() {
    let string_payload =
        std::panic::catch_unwind(|| std::panic::panic_any("owned".to_string())).unwrap_err();
    assert_eq!(panic_message(string_payload.as_ref()), "owned");

    let str_payload = std::panic::catch_unwind(|| std::panic::panic_any("borrowed")).unwrap_err();
    assert_eq!(panic_message(str_payload.as_ref()), "borrowed");

    let other_payload = std::panic::catch_unwind(|| std::panic::panic_any(42_u32)).unwrap_err();
    assert_eq!(
        panic_message(other_payload.as_ref()),
        "panic with non-string payload"
    );
}

/// The LLVM JIT path must honor parent-computed skip decisions: a worker-mode
/// run with `skip_unchanged` set reports those tests as `SkippedUnchanged`
/// without JIT-running them, while the rest run normally.
#[cfg(feature = "llvm")]
#[test]
fn test_llvm_worker_mode_reports_parent_skip_decisions_as_skipped_unchanged() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("skip_honor.ori");
    std::fs::write(&path, LLVM_SKIP_FIXTURE).unwrap();

    let config = TestRunnerConfig {
        backend: Backend::LLVM,
        worker_protocol: true,
        parallel: false,
        skip_unchanged: vec!["double_works".to_string()],
        ..TestRunnerConfig::default()
    };
    let runner = TestRunner::with_config(config);
    let summary = runner.run(&path);

    assert_eq!(
        summary.skipped_unchanged, 1,
        "skip decision must be honored"
    );
    assert_eq!(summary.passed, 1, "the non-skipped test must still run");
    assert_eq!(summary.failed, 0);
    let interner = runner.interner();
    let file = &summary.files[0];
    let outcome = |name: &str| {
        file.results
            .iter()
            .find(|r| r.name_str(interner) == name)
            .map(|r| &r.outcome)
    };
    assert_eq!(
        outcome("double_works"),
        Some(&TestOutcome::SkippedUnchanged),
        "the parent-skipped test must surface as SkippedUnchanged"
    );
    assert_eq!(outcome("triple_works"), Some(&TestOutcome::Passed));
}

/// Negative twin: the same file with NO skip decisions runs everything —
/// proves the skip honoring (not the fixture) produced the skips above.
#[cfg(feature = "llvm")]
#[test]
fn test_llvm_worker_mode_without_skip_decisions_runs_every_test() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("no_skip.ori");
    std::fs::write(&path, LLVM_SKIP_FIXTURE).unwrap();

    let config = TestRunnerConfig {
        backend: Backend::LLVM,
        worker_protocol: true,
        parallel: false,
        ..TestRunnerConfig::default()
    };
    let runner = TestRunner::with_config(config);
    let summary = runner.run(&path);

    assert_eq!(summary.skipped_unchanged, 0);
    assert_eq!(summary.passed, 2, "without skip decisions both tests run");
    assert_eq!(summary.failed, 0);
}

#[cfg(feature = "llvm")]
const LLVM_SKIP_FIXTURE: &str = r"
@double (x: int) -> int = x * 2;

@double_works tests @double () -> void = {
    let _ = double(x: 2);
    ()
}

@triple (x: int) -> int = x * 3;

@triple_works tests @triple () -> void = {
    let _ = triple(x: 2);
    ()
}
";
