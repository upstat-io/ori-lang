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
    assert!(TestOutcome::LlvmCompileFail("reason".into()).is_llvm_compile_fail());
    assert!(!TestOutcome::LlvmCompileFail("reason".into()).is_failed());
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
fn test_llvm_compile_fail_not_counted_as_failure() {
    let interner = test_interner();
    let test1 = interner.intern("test1");

    let mut file = FileSummary::new(PathBuf::from("test.ori"));
    file.add_result(TestResult {
        name: test1,
        targets: vec![],
        outcome: TestOutcome::LlvmCompileFail("LLVM compilation failed".into()),
        duration: Duration::from_millis(5),
    });

    assert_eq!(file.llvm_compile_fail, 1);
    assert_eq!(file.failed, 0);
    assert!(!file.has_failures());
}

#[test]
fn test_llvm_compile_error_not_counted_as_failure() {
    let mut file = FileSummary::new(PathBuf::from("error.ori"));
    file.add_error("LLVM compilation failed".into());
    file.llvm_compile_error = true;

    assert!(!file.has_failures());

    let mut summary = TestSummary::new();
    summary.add_file(file);
    assert_eq!(summary.llvm_compile_fail_files, 1);
    assert_eq!(summary.error_files, 0);
    assert!(!summary.has_failures());
}

#[test]
fn test_summary_llvm_compile_fail_only_is_not_failure() {
    let mut summary = TestSummary::new();
    summary.llvm_compile_fail = 10;
    summary.llvm_compile_fail_files = 5;
    // No real tests passed, but llvm_compile_fail counts mean tests exist
    assert!(!summary.has_failures());
    assert_eq!(summary.exit_code(), 0);
}

#[test]
fn test_summary_llvm_compile_fail_with_real_failures() {
    let mut summary = TestSummary::new();
    summary.passed = 100;
    summary.llvm_compile_fail = 10;
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
    assert_eq!(
        result.targets_str(&interner).collect::<Vec<_>>(),
        vec!["my_function"]
    );
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
    assert_eq!(report.untested().collect::<Vec<_>>(), vec![func2]);
    assert_eq!(
        report.untested_str(&interner).collect::<Vec<_>>(),
        vec!["func2"]
    );
}
