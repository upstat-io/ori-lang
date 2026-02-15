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
    std::fs::write(&path, "@add (a: int, b: int) -> int = a + b").unwrap();

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
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
let result = add(a: 1, b: 2),
print(msg: "done")
)
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
@add (a: int, b: int) -> int = a + b

@test_add tests @add () -> void = run(
let _ = add(a: 1, b: 2),
let _ = 1 / 0,
()
)
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
@foo () -> int = 1
@bar () -> int = 2

@test_foo tests @foo () -> void = print(msg: "pass")
@test_bar tests @bar () -> void = print(msg: "pass")
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
