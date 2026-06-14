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

/// Semantic pin: `render_json` emits the FULL payload — aggregate counts plus a
/// `files` array carrying each per-file summary's per-test `results` — and the
/// output parses as JSON.
#[test]
fn test_render_json_emits_full_payload_with_per_file_results() {
    let interner = test_interner();
    let t1 = interner.intern("alpha_test");
    let t2 = interner.intern("beta_test");

    let mut file = FileSummary::new(PathBuf::from("suite.ori"));
    file.add_result(TestResult::passed(t1, vec![], Duration::from_millis(10)));
    file.add_result(TestResult::failed(
        t2,
        vec![],
        "boom".into(),
        Duration::from_millis(5),
    ));

    let mut summary = TestSummary::new();
    summary.add_file(file);

    let json = summary.render_json(&interner);
    let v: serde_json::Value = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("render_json must emit valid JSON: {e}; json={json}"));

    assert_eq!(v["passed"], 1);
    assert_eq!(v["failed"], 1);
    let files = v["files"]
        .as_array()
        .unwrap_or_else(|| panic!("files must be an array; json={json}"));
    assert_eq!(files.len(), 1);
    let results = files[0]["results"]
        .as_array()
        .unwrap_or_else(|| panic!("results must be an array; json={json}"));
    assert_eq!(results.len(), 2);
}

/// `render_json` resolves interned `Name`s to their string form — `name` and
/// `targets` carry test/function names, never the opaque integer indices a
/// naive `#[derive(Serialize)]` on `TestResult` would emit.
#[test]
fn test_render_json_resolves_interned_names_to_strings() {
    let interner = test_interner();
    let name = interner.intern("my_named_test");
    let target = interner.intern("my_target_fn");

    let mut file = FileSummary::new(PathBuf::from("names.ori"));
    file.add_result(TestResult::passed(name, vec![target], Duration::ZERO));
    let mut summary = TestSummary::new();
    summary.add_file(file);

    let json = summary.render_json(&interner);
    let v: serde_json::Value = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("valid JSON required: {e}; json={json}"));
    let result = &v["files"][0]["results"][0];
    assert_eq!(result["name"], "my_named_test");
    assert_eq!(result["targets"][0], "my_target_fn");
    // Negative: the name field is a string, never a bare integer index.
    assert!(
        result["name"].is_string(),
        "name must be a string, json={json}"
    );
}

/// Every `TestOutcome` variant and the error-file count survive into the JSON.
#[test]
fn test_render_json_carries_all_outcome_variants_and_error_count() {
    let interner = test_interner();
    let p = interner.intern("p");
    let f = interner.intern("f");
    let s = interner.intern("s");
    let su = interner.intern("su");
    let lcf = interner.intern("lcf");

    let mut file = FileSummary::new(PathBuf::from("variants.ori"));
    file.add_result(TestResult::passed(p, vec![], Duration::ZERO));
    file.add_result(TestResult::failed(
        f,
        vec![],
        "f-msg".into(),
        Duration::ZERO,
    ));
    file.add_result(TestResult::skipped(s, vec![], "s-msg".into()));
    file.add_result(TestResult {
        name: su,
        targets: vec![],
        outcome: TestOutcome::SkippedUnchanged,
        duration: Duration::ZERO,
    });
    file.add_result(TestResult {
        name: lcf,
        targets: vec![],
        outcome: TestOutcome::LlvmCompileFail("lcf-msg".into()),
        duration: Duration::ZERO,
    });

    // An error-only file drives error_files.
    let mut errfile = FileSummary::new(PathBuf::from("err.ori"));
    errfile.add_error("parse error".into());

    let mut summary = TestSummary::new();
    summary.add_file(file);
    summary.add_file(errfile);

    let json = summary.render_json(&interner);
    let v: serde_json::Value = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("valid JSON required: {e}; json={json}"));
    assert_eq!(v["error_files"], 1);
    for variant in [
        "Passed",
        "Failed",
        "Skipped",
        "SkippedUnchanged",
        "LlvmCompileFail",
    ] {
        assert!(
            json.contains(variant),
            "outcome variant {variant} missing; json={json}"
        );
    }
}

/// Adversarial: a failure message packed with control characters — tabs,
/// newlines, backslashes, double-quotes, and non-ASCII unicode — serializes to
/// JSON that round-trips through a parser, and the parsed message equals the
/// original. serde escaping is what makes this hold; a hand-rendered payload
/// would produce invalid JSON that a downstream consumer could not parse.
#[test]
fn test_render_json_escapes_control_chars_in_failure_message() {
    let interner = test_interner();
    let t = interner.intern("adversarial");
    let nasty = "tab\there\nnewline \"quoted\" back\\slash unicode\u{2192}\u{3bb}";

    let mut file = FileSummary::new(PathBuf::from("nasty.ori"));
    file.add_result(TestResult::failed(t, vec![], nasty.into(), Duration::ZERO));
    let mut summary = TestSummary::new();
    summary.add_file(file);

    let json = summary.render_json(&interner);
    let v: serde_json::Value = serde_json::from_str(&json).unwrap_or_else(|e| {
        panic!("control chars must round-trip as valid JSON: {e}; json={json}")
    });

    let parsed_msg = v["files"][0]["results"][0]["outcome"]["Failed"]
        .as_str()
        .unwrap_or_else(|| panic!("Failed message must be a string; json={json}"));
    assert_eq!(
        parsed_msg, nasty,
        "message must survive escape + parse unchanged"
    );
}

/// `duration_ns` is a numeric nanosecond count — the pinned schema unit.
#[test]
fn test_render_json_duration_is_numeric_nanoseconds() {
    let interner = test_interner();
    let t = interner.intern("timed");
    let mut file = FileSummary::new(PathBuf::from("timed.ori"));
    file.add_result(TestResult::passed(t, vec![], Duration::from_nanos(1500)));
    let mut summary = TestSummary::new();
    summary.add_file(file);

    let json = summary.render_json(&interner);
    let v: serde_json::Value = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("valid JSON required: {e}; json={json}"));
    assert!(
        v["duration_ns"].is_u64(),
        "summary duration_ns must be u64; json={json}"
    );
    assert_eq!(v["files"][0]["results"][0]["duration_ns"], 1500);
}

/// A zero summary renders a well-formed full object — empty `files` array,
/// all-zero counts — never an empty string or a partial object.
#[test]
fn test_render_json_zero_summary_is_well_formed_full_object() {
    let interner = test_interner();
    let summary = TestSummary::new();
    let json = summary.render_json(&interner);
    let v: serde_json::Value = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("zero summary must be valid JSON: {e}; json={json}"));
    assert_eq!(v["passed"], 0);
    assert_eq!(v["failed"], 0);
    let files = v["files"]
        .as_array()
        .unwrap_or_else(|| panic!("files must be an array; json={json}"));
    assert!(
        files.is_empty(),
        "files must be an empty array; json={json}"
    );
}
