//! Pins for parent-side worker-crash classification and accounting.
//!
//! Fake workers are `sh -c` scripts emitting the wire protocol, so the
//! crash-handling contract is pinned deterministically without depending on
//! any live compiler bug. All tests are unix-only (`sh`, signals).
#![cfg(unix)]

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::super::super::result::TestOutcome;
use super::run_worker_command;
use crate::ir::StringInterner;

/// Generous budget for fake workers that exit on their own.
const NO_TIMEOUT: Duration = Duration::from_secs(60);

fn fake_worker(script: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(script);
    cmd
}

fn run_fake(script: &str) -> (super::FileSummary, StringInterner) {
    let interner = StringInterner::new();
    let summary = run_worker_command(
        fake_worker(script),
        Path::new("fake.ori"),
        &interner,
        NO_TIMEOUT,
    );
    (summary, interner)
}

fn outcome_of<'a>(
    summary: &'a super::FileSummary,
    interner: &StringInterner,
    name: &str,
) -> &'a TestOutcome {
    let target = summary
        .results
        .iter()
        .find(|r| r.name_str(interner) == name);
    match target {
        Some(result) => &result.outcome,
        None => panic!("no result for test '{name}': {:?}", summary.results),
    }
}

#[test]
fn test_worker_clean_run_reconstructs_results() {
    let (summary, interner) = run_fake(
        "echo '@@ori-test plan t_pass'; \
         echo '@@ori-test plan t_skip'; \
         echo '@@ori-test start t_pass'; \
         echo '@@ori-test result t_pass pass 1000'; \
         echo '@@ori-test result t_skip skip 0 not today'; \
         echo '@@ori-test done'",
    );
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.skipped, 1);
    assert_eq!(summary.failed, 0);
    assert!(!summary.has_failures());
    assert_eq!(
        outcome_of(&summary, &interner, "t_skip"),
        &TestOutcome::Skipped("not today".to_string())
    );
}

#[test]
fn test_worker_sigsegv_marks_in_flight_failed_with_signal_name() {
    let (summary, interner) = run_fake(
        "echo '@@ori-test plan t_one'; \
         echo '@@ori-test plan t_two'; \
         echo '@@ori-test plan t_three'; \
         echo '@@ori-test start t_one'; \
         echo '@@ori-test result t_one pass 5'; \
         echo '@@ori-test start t_two'; \
         kill -SEGV $$",
    );
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.failed, 2, "in-flight + remaining must both fail");
    assert!(summary.has_failures());

    let TestOutcome::Failed(detail) = outcome_of(&summary, &interner, "t_two") else {
        panic!("in-flight test must be Failed");
    };
    assert!(
        detail.contains("signal 11 (SIGSEGV)"),
        "crash detail must carry the signal name: {detail}"
    );

    let TestOutcome::Failed(detail) = outcome_of(&summary, &interner, "t_three") else {
        panic!("remaining planned test must be Failed");
    };
    assert!(
        detail.contains("failed-by-crash") && detail.contains("t_two"),
        "remaining test must be labeled failed-by-crash attributing the crash site: {detail}"
    );
}

#[test]
fn test_worker_sigabrt_classified_with_signal_name() {
    let (summary, interner) = run_fake(
        "echo '@@ori-test plan t_abrt'; \
         echo '@@ori-test start t_abrt'; \
         kill -ABRT $$",
    );
    assert_eq!(summary.failed, 1);
    let TestOutcome::Failed(detail) = outcome_of(&summary, &interner, "t_abrt") else {
        panic!("aborted test must be Failed");
    };
    assert!(
        detail.contains("signal 6 (SIGABRT)"),
        "abort must classify as SIGABRT: {detail}"
    );
}

#[test]
fn test_worker_legacy_exit_134_classified_as_abort() {
    let (summary, interner) = run_fake(
        "echo '@@ori-test plan t_rc'; \
         echo '@@ori-test start t_rc'; \
         exit 134",
    );
    assert_eq!(summary.failed, 1);
    let TestOutcome::Failed(detail) = outcome_of(&summary, &interner, "t_rc") else {
        panic!("exit-134 test must be Failed");
    };
    assert!(
        detail.contains("exit code 134 (SIGABRT-class abort)"),
        "legacy 134 exits must classify as abort-class: {detail}"
    );
}

#[test]
fn test_worker_crash_failure_carries_stderr_tail() {
    let (summary, interner) = run_fake(
        "echo '@@ori-test plan t_df'; \
         echo '@@ori-test start t_df'; \
         echo 'ori: FATAL — double-free in RC codegen' >&2; \
         kill -SEGV $$",
    );
    let TestOutcome::Failed(detail) = outcome_of(&summary, &interner, "t_df") else {
        panic!("crashed test must be Failed");
    };
    assert!(
        detail.contains("worker stderr") && detail.contains("double-free in RC codegen"),
        "crash detail must render the stderr tail: {detail}"
    );
}

#[test]
fn test_worker_crash_before_discovery_records_file_error() {
    let (summary, _) = run_fake("echo 'warming up' >&2; kill -SEGV $$");
    assert_eq!(summary.total(), 0);
    assert!(
        summary.has_failures(),
        "pre-discovery crash must fail the file"
    );
    assert!(
        summary
            .errors
            .iter()
            .any(|e| e.contains("crashed before test discovery")
                && e.contains("signal 11 (SIGSEGV)")),
        "file error must classify the pre-discovery crash: {:?}",
        summary.errors
    );
}

#[test]
fn test_worker_crash_during_compilation_fails_all_planned() {
    // Plan emitted, no start: crash during LLVM compilation/setup.
    let (summary, interner) = run_fake(
        "echo '@@ori-test plan t_a'; \
         echo '@@ori-test plan t_b'; \
         kill -SEGV $$",
    );
    assert_eq!(summary.failed, 2);
    let TestOutcome::Failed(detail) = outcome_of(&summary, &interner, "t_a") else {
        panic!("planned test must be Failed");
    };
    assert!(
        detail.contains("LLVM compilation/setup"),
        "no-start crash attributes to compilation/setup: {detail}"
    );
}

#[test]
fn test_worker_protocol_glued_to_unterminated_output_still_parses() {
    // A test printing without a trailing newline glues onto the protocol line.
    let (summary, _) = run_fake(
        "echo '@@ori-test plan t_p'; \
         printf 'partial output'; \
         echo '@@ori-test result t_p pass 9'; \
         echo '@@ori-test done'",
    );
    assert_eq!(summary.passed, 1);
    assert!(!summary.has_failures());
}

#[test]
fn test_worker_file_error_and_llvm_flag_propagate() {
    let (summary, _) = run_fake(
        "echo '@@ori-test plan t_cf'; \
         echo '@@ori-test result t_cf llvm-compile-fail 0 codegen rejected'; \
         echo '@@ori-test file-error LLVM compilation failed: bad IR'; \
         echo '@@ori-test llvm-compile-error'; \
         echo '@@ori-test done'",
    );
    assert!(summary.llvm_compile_error);
    assert_eq!(summary.llvm_compile_fail, 1);
    assert_eq!(summary.failed, 1, "llvm compile fail counts as failed");
    assert_eq!(summary.errors.len(), 1);
    assert!(summary.has_failures());
}

#[test]
fn test_worker_hang_killed_by_watchdog_and_counted_failed() {
    let interner = StringInterner::new();
    let start = std::time::Instant::now();
    let summary = run_worker_command(
        fake_worker(
            "echo '@@ori-test plan t_hang'; \
             echo '@@ori-test start t_hang'; \
             sleep 30",
        ),
        Path::new("fake.ori"),
        &interner,
        Duration::from_millis(300),
    );
    assert!(
        start.elapsed() < Duration::from_secs(10),
        "watchdog must kill the hung worker promptly, took {:?}",
        start.elapsed()
    );
    assert_eq!(summary.failed, 1);
    let TestOutcome::Failed(detail) = outcome_of(&summary, &interner, "t_hang") else {
        panic!("hung test must be Failed");
    };
    assert!(
        detail.contains("watchdog timeout"),
        "timeout kill must be classified as a watchdog timeout: {detail}"
    );
}

#[test]
fn test_worker_spawn_failure_records_error() {
    let interner = StringInterner::new();
    let summary = run_worker_command(
        Command::new("/nonexistent/ori-test-worker-binary"),
        Path::new("fake.ori"),
        &interner,
        NO_TIMEOUT,
    );
    assert!(summary.has_failures());
    assert!(
        summary
            .errors
            .iter()
            .any(|e| e.contains("failed to spawn test worker")),
        "{:?}",
        summary.errors
    );
}

#[test]
fn test_worker_done_then_nonzero_exit_records_error_not_crash() {
    let (summary, _) = run_fake(
        "echo '@@ori-test plan t_ok'; \
         echo '@@ori-test result t_ok pass 5'; \
         echo '@@ori-test done'; \
         exit 7",
    );
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.failed, 0, "completed protocol: no failed-by-crash");
    assert!(
        summary
            .errors
            .iter()
            .any(|e| e.contains("exited abnormally after completing")),
        "{:?}",
        summary.errors
    );
}
