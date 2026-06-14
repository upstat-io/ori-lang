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

/// Semantic pin: an incremental no-change run (every test skipped-unchanged,
/// nothing failed) has tests — it exits 0, never 2 ("no tests found").
#[test]
fn test_exit_code_all_skipped_unchanged_run_exits_zero() {
    let mut summary = TestSummary::new();
    summary.skipped_unchanged = 7;
    assert_eq!(summary.total(), 7, "skipped-unchanged tests are present");
    assert!(!summary.has_failures());
    assert_eq!(summary.exit_code(), 0);

    // Negative pin: a genuinely-empty run still reports "no tests found".
    let empty = TestSummary::new();
    assert_eq!(empty.total(), 0);
    assert_eq!(empty.exit_code(), 2);
}

/// Skipped-unchanged tests never mask real failures in the exit code.
#[test]
fn test_exit_code_skipped_unchanged_with_failures_exits_one() {
    let mut summary = TestSummary::new();
    summary.skipped_unchanged = 3;
    summary.failed = 1;
    assert_eq!(summary.exit_code(), 1);
}

/// `FileSummary::total()` counts skipped-unchanged results so per-file
/// rendering treats an all-unchanged file as having tests.
#[test]
fn test_file_summary_total_includes_skipped_unchanged() {
    let interner = test_interner();
    let test1 = interner.intern("test1");

    let mut file = FileSummary::new(PathBuf::from("test.ori"));
    file.add_result(TestResult {
        name: test1,
        targets: vec![],
        outcome: TestOutcome::SkippedUnchanged,
        duration: Duration::ZERO,
    });
    assert_eq!(file.skipped_unchanged, 1);
    assert_eq!(file.total(), 1);
    assert!(!file.has_failures());

    let mut summary = TestSummary::new();
    summary.add_file(file);
    assert_eq!(summary.skipped_unchanged, 1);
    assert_eq!(summary.total(), 1);
    assert_eq!(summary.exit_code(), 0);
}

/// A test that cannot compile via LLVM is a failed test: it joins `failed`,
/// `has_failures()`, and the non-zero exit code (with `llvm_compile_fail`
/// tracking the reason breakdown as a subset).
#[test]
fn test_llvm_compile_fail_counts_as_failed_test() {
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
    assert_eq!(file.failed, 1);
    assert!(file.has_failures());

    let mut summary = TestSummary::new();
    summary.add_file(file);
    assert_eq!(summary.llvm_compile_fail, 1);
    assert_eq!(summary.failed, 1);
    assert!(summary.has_failures());
    assert_eq!(summary.exit_code(), 1);
}

/// A file whose LLVM compilation errored (errors recorded, no results) still
/// drives `has_failures()` and a non-zero exit code.
#[test]
fn test_llvm_compile_error_file_counts_as_failure() {
    let mut file = FileSummary::new(PathBuf::from("error.ori"));
    file.add_error("LLVM compilation failed".into());
    file.llvm_compile_error = true;

    assert!(file.has_failures());

    let mut summary = TestSummary::new();
    summary.add_file(file);
    assert_eq!(summary.llvm_compile_fail_files, 1);
    assert_eq!(summary.error_files, 0);
    assert!(summary.has_failures());
    assert_eq!(summary.exit_code(), 1);
}

/// Negative pin against the old accounting: a run whose only problems are
/// LLVM compile failures must NOT exit 0.
#[test]
fn test_summary_llvm_compile_fail_only_exits_nonzero() {
    let mut summary = TestSummary::new();
    summary.failed = 10;
    summary.llvm_compile_fail = 10;
    summary.llvm_compile_fail_files = 5;
    assert!(summary.has_failures());
    assert_eq!(summary.exit_code(), 1);
}

#[test]
fn test_summary_llvm_compile_fail_with_real_failures() {
    let mut summary = TestSummary::new();
    summary.passed = 100;
    summary.llvm_compile_fail = 10;
    summary.failed = 11; // 10 compile-fail tests + 1 assertion failure
    assert!(summary.has_failures());
    assert_eq!(summary.exit_code(), 1);
}

/// `exit_code() == 2` (no tests found) is reserved for genuinely-empty runs;
/// compile-error-only files report failure (1), not "no tests".
#[test]
fn test_exit_code_no_tests_vs_compile_error_files() {
    let summary = TestSummary::new();
    assert_eq!(summary.exit_code(), 2);

    let mut summary2 = TestSummary::new();
    summary2.llvm_compile_fail_files = 1;
    assert_eq!(summary2.exit_code(), 1);
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

/// Semantic pin: `render_json` emits exactly the minimal walking-skeleton
/// object — `passed`/`failed`/`skipped` keys with the summary's counts, in a
/// shape `json.load` accepts.
#[test]
fn test_render_json_emits_passed_failed_skipped() {
    let mut summary = TestSummary::new();
    summary.passed = 12;
    summary.failed = 3;
    summary.skipped = 5;

    assert_eq!(
        summary.render_json(),
        "{\"passed\":12,\"failed\":3,\"skipped\":5}"
    );
}

/// Negative pin: the skeleton object is MINIMAL — it carries exactly the three
/// keys, never `skipped_unchanged` / `error_files` / per-file detail (that is
/// a later full-output-format surface, not the skeleton's).
#[test]
fn test_render_json_excludes_non_skeleton_fields() {
    let mut summary = TestSummary::new();
    summary.passed = 1;
    summary.skipped_unchanged = 9;
    summary.llvm_compile_fail = 4;
    summary.error_files = 2;

    let json = summary.render_json();
    assert!(!json.contains("skipped_unchanged"), "json={json}");
    assert!(!json.contains("error_files"), "json={json}");
    assert!(!json.contains("llvm_compile_fail"), "json={json}");
    assert!(!json.contains("files"), "json={json}");
    // Exactly three keys.
    assert_eq!(json.matches(':').count(), 3, "json={json}");
}

/// A zero summary still renders a well-formed object (all-zero counts), never
/// an empty string or a partial object.
#[test]
fn test_render_json_zero_summary_is_well_formed() {
    let summary = TestSummary::new();
    assert_eq!(
        summary.render_json(),
        "{\"passed\":0,\"failed\":0,\"skipped\":0}"
    );
}
