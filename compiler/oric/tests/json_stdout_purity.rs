//! Integration pins for `ori test --format json` stdout purity.
//!
//! Drives the REAL `ori` binary (interpreter backend, in-process runner)
//! against temp `.ori` fixtures whose tests call `print()`. The machine-
//! readable summary and test-program output must never share the stdout
//! JSON channel: stdout is exactly one parseable JSON document, program
//! output is replayed to stderr in json mode, and Text mode keeps program
//! output inline on stdout. Counting semantics are pinned against ground
//! truth so an output-path change can never silently alter pass/fail math.

use std::path::PathBuf;
use std::process::{Command, Output};

/// Fixture: two passing tests that print, in one file.
const PRINTING_PASS_FIXTURE: &str = r#"
use std.testing { assert_eq };

@double (x: int) -> int = x * 2;

@double_of_two_prints tests @double () -> void = {
    print(msg: "marker-alpha from passing test");
    assert_eq(actual: double(x: 2), expected: 4)
}

@double_of_three_prints tests @double () -> void = {
    print(msg: "marker-beta from passing test");
    assert_eq(actual: double(x: 3), expected: 6)
}
"#;

/// Fixture: a failing (assert-failure) test that prints, FOLLOWED by a
/// passing printing test in the same file (second file — exercises the
/// multi-file parallel path, pins that buffering never masks a failure, and
/// pins the per-test drain: the failing test's buffered output must reach
/// stderr and the successor's output must still appear afterward).
const PRINTING_FAIL_FIXTURE: &str = r#"
use std.testing { assert_eq };

@triple (x: int) -> int = x * 3;

@triple_of_two_fails_after_print tests @triple () -> void = {
    print(msg: "marker-gamma from failing test");
    assert_eq(actual: triple(x: 2), expected: 999)
}

@triple_of_three_prints_after_failure tests @triple () -> void = {
    print(msg: "marker-delta from test after a failure");
    assert_eq(actual: triple(x: 3), expected: 9)
}
"#;

fn write_fixtures(files: &[(&str, &str)]) -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::TempDir::new().unwrap_or_else(|e| panic!("tempdir: {e}"));
    for (name, content) in files {
        let path = dir.path().join(name);
        std::fs::write(&path, content).unwrap_or_else(|e| panic!("write fixture: {e}"));
    }
    let root = dir.path().to_path_buf();
    (dir, root)
}

fn run_ori_test(dir: &PathBuf, json: bool) -> Output {
    // Fixtures live in a tempdir outside the repo tree, so the ancestor-walk
    // stdlib discovery cannot find `library/`; point ORI_STDLIB at it.
    let stdlib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../library")
        .canonicalize()
        .unwrap_or_else(|e| panic!("stdlib dir: {e}"));
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ori"));
    cmd.arg("test");
    if json {
        cmd.arg("--format").arg("json");
    }
    cmd.arg(dir).env("ORI_STDLIB", stdlib);
    cmd.output()
        .unwrap_or_else(|e| panic!("failed to spawn ori test: {e}"))
}

fn parse_single_json_document(stdout: &[u8]) -> serde_json::Value {
    let text = String::from_utf8_lossy(stdout);
    serde_json::from_str(text.trim()).unwrap_or_else(|e| {
        panic!(
            "stdout is not a single clean JSON document ({e}); first 300 bytes: {:?}",
            &text[..text.len().min(300)]
        )
    })
}

/// Per-result outcome tally. `Passed` serializes as the string "Passed";
/// payload-carrying variants serialize as single-key objects
/// (`{"Failed": "<msg>"}`), so both shapes are decoded.
fn count_outcomes(doc: &serde_json::Value) -> (u64, u64) {
    let mut passed = 0;
    let mut failed = 0;
    let files = doc["files"]
        .as_array()
        .unwrap_or_else(|| panic!("summary JSON has no files array: {doc}"));
    for file in files {
        let results = file["results"]
            .as_array()
            .unwrap_or_else(|| panic!("file entry has no results array: {file}"));
        for result in results {
            let outcome = &result["outcome"];
            if outcome.as_str() == Some("Passed") {
                passed += 1;
            } else if outcome.get("Failed").is_some() {
                failed += 1;
            }
        }
    }
    (passed, failed)
}

/// Top-level aggregate counters emitted by `render_json` alongside `files`.
fn aggregate_counts(doc: &serde_json::Value) -> (u64, u64) {
    let passed = doc["passed"]
        .as_u64()
        .unwrap_or_else(|| panic!("summary JSON has no top-level passed count: {doc}"));
    let failed = doc["failed"]
        .as_u64()
        .unwrap_or_else(|| panic!("summary JSON has no top-level failed count: {doc}"));
    (passed, failed)
}

/// json mode: stdout parses as exactly one JSON document even when every
/// test prints — program output must never precede or trail the summary.
#[test]
fn json_stdout_with_printing_tests_is_single_document() {
    let (_guard, dir) = write_fixtures(&[
        ("printing_pass.ori", PRINTING_PASS_FIXTURE),
        ("printing_fail.ori", PRINTING_FAIL_FIXTURE),
    ]);
    let out = run_ori_test(&dir, true);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("marker-alpha") && !stdout.contains("marker-gamma"),
        "test print() output leaked into the json stdout channel: {:?}",
        &stdout[..stdout.len().min(300)]
    );
    parse_single_json_document(&out.stdout);
}

/// json mode: pass/fail counts match ground truth (3 passing + 1 failing
/// printing tests) — output-path changes must never alter counting.
#[test]
fn json_counts_with_printing_tests_match_ground_truth() {
    let (_guard, dir) = write_fixtures(&[
        ("printing_pass.ori", PRINTING_PASS_FIXTURE),
        ("printing_fail.ori", PRINTING_FAIL_FIXTURE),
    ]);
    let out = run_ori_test(&dir, true);
    let doc = parse_single_json_document(&out.stdout);
    assert_eq!(
        count_outcomes(&doc),
        (3, 1),
        "per-result outcomes diverged from fixture ground truth (3 passing + 1 failing)"
    );
    assert_eq!(
        aggregate_counts(&doc),
        (3, 1),
        "top-level aggregate counts diverged from fixture ground truth (3 passing + 1 failing)"
    );
}

/// json mode: buffered program output is replayed to stderr so it stays
/// observable (the test-all leg captures stderr separately) — from PASSING
/// and FAILING tests alike, with within-file test order preserved.
#[test]
fn json_mode_replays_printed_output_to_stderr() {
    let (_guard, dir) = write_fixtures(&[
        ("printing_pass.ori", PRINTING_PASS_FIXTURE),
        ("printing_fail.ori", PRINTING_FAIL_FIXTURE),
    ]);
    let out = run_ori_test(&dir, true);
    let stderr = String::from_utf8_lossy(&out.stderr);
    for marker in [
        "marker-alpha",
        "marker-beta",
        "marker-gamma",
        "marker-delta",
    ] {
        assert!(
            stderr.contains(marker),
            "printed output {marker} missing from stderr in json mode (the per-test \
             drain must emit a failing test's prints, and the next test's prints \
             must still appear afterward); stderr: {:?}",
            &stderr[..stderr.len().min(400)]
        );
    }
    let alpha = stderr.find("marker-alpha").unwrap_or(usize::MAX);
    let beta = stderr.find("marker-beta").unwrap_or(0);
    assert!(
        alpha < beta,
        "within-file test print order not preserved on stderr (alpha must precede beta)"
    );
    let gamma = stderr.find("marker-gamma").unwrap_or(usize::MAX);
    let delta = stderr.find("marker-delta").unwrap_or(0);
    assert!(
        gamma < delta,
        "per-test drain order not preserved on stderr (failing test's gamma must precede its successor's delta)"
    );
}

/// Text (human) mode is unchanged: program output stays inline on stdout.
#[test]
fn text_mode_keeps_printed_output_on_stdout() {
    let (_guard, dir) = write_fixtures(&[("printing_pass.ori", PRINTING_PASS_FIXTURE)]);
    let out = run_ori_test(&dir, false);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("marker-alpha"),
        "text mode must keep print() output on stdout; stdout: {:?}",
        &stdout[..stdout.len().min(400)]
    );
}
