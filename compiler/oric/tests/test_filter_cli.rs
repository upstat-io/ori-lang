//! Production-entry tests for the `ori test` name-filter option.

use std::path::PathBuf;
use std::process::{Command, Output};

const FILTER_FIXTURE: &str = r"
use std.testing { assert_eq };

@subject (value: int) -> int = value;

@alpha_selected tests @subject () -> void =
    assert_eq(actual: subject(value: 1), expected: 1);
@beta_excluded tests @subject () -> void =
    assert_eq(actual: subject(value: 2), expected: 2);
";

fn write_fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap_or_else(|error| panic!("create tempdir: {error}"));
    let fixture = dir.path().join("filter.ori");
    std::fs::write(&fixture, FILTER_FIXTURE)
        .unwrap_or_else(|error| panic!("write filter fixture: {error}"));
    (dir, fixture)
}

fn run_ori_test(args: &[&str]) -> Output {
    let stdlib = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../library")
        .canonicalize()
        .unwrap_or_else(|error| panic!("resolve stdlib: {error}"));
    Command::new(env!("CARGO_BIN_EXE_ori"))
        .arg("test")
        .args(args)
        .env("ORI_STDLIB", stdlib)
        .output()
        .unwrap_or_else(|error| panic!("run ori test: {error}"))
}

fn assert_single_filtered_test(output: &Output) {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "filtered test command failed:\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        stdout.contains("1 passed, 0 failed"),
        "filter must select exactly one passing test:\n{stdout}"
    );
}

#[test]
fn filter_accepts_separated_pattern_value() {
    let (_guard, fixture) = write_fixture();
    let fixture = fixture.to_string_lossy();
    assert_single_filtered_test(&run_ori_test(&["--filter", "alpha", &fixture]));
}

#[test]
fn filter_retains_equals_pattern_value() {
    let (_guard, fixture) = write_fixture();
    let fixture = fixture.to_string_lossy();
    assert_single_filtered_test(&run_ori_test(&["--filter=alpha", &fixture]));
}

#[test]
fn filter_without_pattern_reports_cause_and_both_valid_forms() {
    let output = run_ori_test(&["--filter"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr.contains("--filter requires a non-empty pattern"),
        "diagnostic must name the missing value:\n{stderr}"
    );
    assert!(
        stderr.contains("--filter <value>") && stderr.contains("--filter=<value>"),
        "diagnostic must show both accepted forms:\n{stderr}"
    );
}

#[test]
fn cargo_stf_alias_scopes_discovery_to_the_spec_test_root() {
    let config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.cargo/config.toml");
    let config = std::fs::read_to_string(&config_path)
        .unwrap_or_else(|error| panic!("read {}: {error}", config_path.display()));
    assert!(
        config.contains("stf = \"run -p oric --bin ori -- test tests/ --filter\""),
        "cargo stf must filter the canonical tests/ root instead of scanning compiler fixtures"
    );
}
