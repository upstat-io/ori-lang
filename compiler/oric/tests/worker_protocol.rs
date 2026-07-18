//! Integration pins for the REAL worker exec path (`--backend=llvm --__worker`).
//!
//! Spawns the actual `ori` binary against temp `.ori` fixtures (the unit pins
//! in `src/test/runner/subprocess/tests.rs` use `sh -c` fake workers). Pins:
//! the tokenized green and failing plan/start/result/done roundtrips, the
//! token-refusal exit, and parent-side accounting for a normal test failure.
#![cfg(feature = "llvm")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Fixed per-run nonce (a real parent generates a fresh one per spawn).
const TOKEN: &str = "1nt3gr4t10nt0k3n0123456789abcdef";

/// Env var carrying the per-spawn protocol nonce (`debug_flags::ORI_TEST_PROTOCOL_TOKEN`).
const TOKEN_VAR: &str = "ORI_TEST_PROTOCOL_TOKEN";

/// Fixture whose single test completes normally.
const GREEN_FIXTURE: &str = r"
@double (x: int) -> int = x * 2;

@double_of_two_runs tests @double () -> void = {
    let _ = double(x: 2);
    ()
}
";

/// Fixture whose single test reports a recoverable runtime failure.
const FAILING_FIXTURE: &str = r"
@double (x: int) -> int = x * 2;

@double_then_divides_by_zero tests @double () -> void = {
    let _ = double(x: 2);
    let _ = 1 / 0;
    ()
}
";

fn write_fixture(dir: &tempfile::TempDir, name: &str, content: &str) -> PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, content).unwrap_or_else(|e| panic!("failed to write fixture: {e}"));
    path
}

/// Build the real worker invocation the parent runner would spawn.
fn worker_command(fixture: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ori"));
    cmd.arg("test")
        .arg("--backend=llvm")
        .arg("--__worker")
        .arg(fixture)
        .env(TOKEN_VAR, TOKEN);
    cmd
}

fn run_to_output(cmd: &mut Command) -> Output {
    cmd.output()
        .unwrap_or_else(|e| panic!("failed to spawn real ori binary: {e}"))
}

/// One tokenized protocol line: `@@ori-test:<token> <tail>`.
fn protocol_line(tail: &str) -> String {
    format!("@@ori-test:{TOKEN} {tail}")
}

fn assert_has_line_starting(stdout: &str, prefix: &str) {
    assert!(
        stdout.lines().any(|line| line.starts_with(prefix)),
        "worker stdout must carry a line starting with {prefix:?}:\n{stdout}"
    );
}

#[test]
fn test_real_worker_green_fixture_roundtrips_plan_start_result_done() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "green.ori", GREEN_FIXTURE);

    let output = run_to_output(&mut worker_command(&fixture));
    assert!(
        output.status.success(),
        "worker must exit 0 on a green file: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_has_line_starting(&stdout, &protocol_line("plan double_of_two_runs"));
    assert_has_line_starting(&stdout, &protocol_line("start double_of_two_runs"));
    assert_has_line_starting(&stdout, &protocol_line("result double_of_two_runs pass "));
    assert_has_line_starting(&stdout, &protocol_line("done"));
}

#[test]
fn test_real_worker_runtime_failure_reports_result_and_done() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "fails.ori", FAILING_FIXTURE);

    let output = run_to_output(&mut worker_command(&fixture));
    assert!(
        output.status.success(),
        "a completed worker must exit 0 after reporting the test failure: {:?}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_has_line_starting(&stdout, &protocol_line("plan double_then_divides_by_zero"));
    assert_has_line_starting(&stdout, &protocol_line("start double_then_divides_by_zero"));
    assert_has_line_starting(
        &stdout,
        &protocol_line("result double_then_divides_by_zero fail "),
    );
    assert_has_line_starting(&stdout, &protocol_line("done"));
}

#[test]
fn test_real_parent_run_reports_worker_test_failure() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "fails.ori", FAILING_FIXTURE);

    // Full end-to-end: the parent runner spawns the real worker (no
    // `--__worker` here) and counts its reported test failure.
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ori"));
    cmd.arg("test").arg("--backend=llvm").arg(&fixture);
    let output = run_to_output(&mut cmd);

    assert_eq!(
        output.status.code(),
        Some(1),
        "a failing test must fail the parent run\nstdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("FAIL: double_then_divides_by_zero - division by zero"),
        "the worker-reported failure must preserve its diagnostic: {stdout}"
    );
    assert!(
        !stdout.contains("SIGABRT-class abort"),
        "a normal test failure must not be classified as a worker crash: {stdout}"
    );
    assert!(
        stdout.contains("1 failed"),
        "the summary must count the failing test: {stdout}"
    );
}

#[test]
fn test_real_worker_without_token_refuses_with_exit_one() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "green.ori", GREEN_FIXTURE);

    let mut cmd = worker_command(&fixture);
    cmd.env_remove(TOKEN_VAR);
    let output = run_to_output(&mut cmd);

    assert_eq!(
        output.status.code(),
        Some(1),
        "worker without the parent-provided token must refuse with exit 1"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("--__worker requires"),
        "refusal must name the missing token requirement: {stderr}"
    );
}

// === Incremental isolated mode (parent-owned cache) ===

/// Fixture with two targeted tests; `@triple`'s body is edited (same byte
/// length) by the partial-rerun pin so `@double_works` stays unchanged.
const INCREMENTAL_FIXTURE: &str = r"
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

/// Build a REAL parent-runner invocation in incremental isolated mode with
/// an on-disk cache (the parent owns the cache; workers never touch it).
fn incremental_parent_command(fixture: &Path, cache: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_ori"));
    cmd.arg("test")
        .arg("--backend=llvm")
        .arg("--incremental")
        .arg(fixture)
        .env("ORI_TEST_INCREMENTAL_CACHE", cache);
    cmd
}

fn assert_run_ok(output: &Output, label: &str) -> String {
    assert!(
        output.status.success(),
        "{label} run must exit 0: {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn test_incremental_isolated_second_run_skips_unchanged_tests() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "incremental.ori", INCREMENTAL_FIXTURE);
    let cache = dir.path().join("test-cache");

    let first = run_to_output(&mut incremental_parent_command(&fixture, &cache));
    let first_stdout = assert_run_ok(&first, "first incremental");
    assert!(
        first_stdout.contains("2 passed"),
        "first run has no snapshot — every test runs: {first_stdout}"
    );
    assert!(
        !first_stdout.contains("skipped (unchanged)"),
        "first run must skip nothing: {first_stdout}"
    );
    assert!(
        cache.exists(),
        "the parent must persist the incremental cache after the run"
    );

    let second = run_to_output(&mut incremental_parent_command(&fixture, &cache));
    let second_stdout = assert_run_ok(&second, "second incremental");
    assert!(
        second_stdout.contains("0 passed") && second_stdout.contains("2 skipped (unchanged)"),
        "an unchanged second run in isolated mode must skip every targeted test: {second_stdout}"
    );
}

#[test]
fn test_incremental_isolated_rerun_after_edit_runs_only_changed_tests() {
    let dir = tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"));
    let fixture = write_fixture(&dir, "incremental_edit.ori", INCREMENTAL_FIXTURE);
    let cache = dir.path().join("test-cache");

    let first = run_to_output(&mut incremental_parent_command(&fixture, &cache));
    let first_stdout = assert_run_ok(&first, "first incremental");
    assert!(first_stdout.contains("2 passed"), "{first_stdout}");

    // Same-length edit to @triple's body: @double / @double_works are
    // byte-identical, so only the triple-targeting test must re-run.
    std::fs::write(&fixture, INCREMENTAL_FIXTURE.replace("x * 3", "x * 4"))
        .unwrap_or_else(|e| panic!("failed to edit fixture: {e}"));

    let second = run_to_output(&mut incremental_parent_command(&fixture, &cache));
    let second_stdout = assert_run_ok(&second, "post-edit incremental");
    assert!(
        second_stdout.contains("1 passed") && second_stdout.contains("1 skipped (unchanged)"),
        "after editing one target, only its test re-runs (worker honors the \
         parent-forwarded skip set): {second_stdout}"
    );
}
