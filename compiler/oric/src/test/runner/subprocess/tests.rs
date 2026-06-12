//! Pins for parent-side worker-crash classification and accounting.
//!
//! Fake workers are `sh -c` scripts emitting the wire protocol, so the
//! crash-handling contract is pinned deterministically without depending on
//! any live compiler bug. Scripts write `@@TOK@@` where a tokenized protocol
//! prefix belongs; the harness substitutes `@@ori-test:<token>` so forgery
//! pins can also write the RAW (un-tokenized) marker. All tests are
//! unix-only (`sh`, signals).
#![cfg(unix)]

use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::super::super::result::TestOutcome;
use super::run_worker_command;
use crate::ir::StringInterner;

/// Generous budget for fake workers that exit on their own.
const NO_TIMEOUT: Duration = Duration::from_secs(60);

/// Fixed per-run nonce for fake workers (a real run generates a fresh one).
const TOKEN: &str = "pin-token-0123";

/// Placeholder fake-worker scripts use for the tokenized protocol prefix.
const TOK: &str = "@@TOK@@";

fn fake_worker(script: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.arg("-c")
        .arg(script.replace(TOK, &format!("@@ori-test:{TOKEN}")));
    cmd
}

fn run_fake(script: &str) -> (super::FileSummary, StringInterner) {
    let interner = StringInterner::new();
    let summary = run_worker_command(
        fake_worker(script),
        Path::new("fake.ori"),
        &interner,
        NO_TIMEOUT,
        TOKEN,
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
        "echo '@@TOK@@ plan t_pass'; \
         echo '@@TOK@@ plan t_skip'; \
         echo '@@TOK@@ start t_pass'; \
         echo '@@TOK@@ result t_pass pass 1000'; \
         echo '@@TOK@@ result t_skip skip 0 not today'; \
         echo '@@TOK@@ done'",
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
        "echo '@@TOK@@ plan t_one'; \
         echo '@@TOK@@ plan t_two'; \
         echo '@@TOK@@ plan t_three'; \
         echo '@@TOK@@ start t_one'; \
         echo '@@TOK@@ result t_one pass 5'; \
         echo '@@TOK@@ start t_two'; \
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
        "echo '@@TOK@@ plan t_abrt'; \
         echo '@@TOK@@ start t_abrt'; \
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
        "echo '@@TOK@@ plan t_rc'; \
         echo '@@TOK@@ start t_rc'; \
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
        "echo '@@TOK@@ plan t_df'; \
         echo '@@TOK@@ start t_df'; \
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
        "echo '@@TOK@@ plan t_a'; \
         echo '@@TOK@@ plan t_b'; \
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
fn test_worker_result_glued_to_unterminated_output_is_passthrough_and_gap_failed() {
    // A result record glued onto unterminated test output is NOT a protocol
    // line (start-of-line forgery guard); the planned test surfaces as a
    // failed protocol gap instead of silently vanishing from the totals.
    let (summary, interner) = run_fake(
        "echo '@@TOK@@ plan t_p'; \
         printf 'partial output'; \
         echo '@@TOK@@ result t_p pass 9'; \
         echo '@@TOK@@ done'",
    );
    assert_eq!(summary.passed, 0, "glued result record must not be parsed");
    assert_eq!(summary.failed, 1);
    let TestOutcome::Failed(detail) = outcome_of(&summary, &interner, "t_p") else {
        panic!("planned-but-unresulted test must be Failed");
    };
    assert!(
        detail.contains("no result record"),
        "gap failure must name the missing result record: {detail}"
    );
}

#[test]
fn test_worker_midline_protocol_marker_does_not_forge_result() {
    // A test printing a protocol-shaped record inside its own output must
    // not create a result; mid-line markers pass through as plain output
    // even when they carry the valid token.
    let (summary, interner) = run_fake(
        "echo '@@TOK@@ plan t_real'; \
         echo '@@TOK@@ start t_real'; \
         echo 'output mentioning @@TOK@@ result evil pass 0'; \
         echo '@@TOK@@ result t_real pass 5'; \
         echo '@@TOK@@ done'",
    );
    assert_eq!(summary.passed, 1);
    assert!(!summary.has_failures());
    assert!(
        summary
            .results
            .iter()
            .all(|r| r.name_str(&interner) != "evil"),
        "mid-line protocol marker must not forge a result: {:?}",
        summary.results
    );
    assert_eq!(summary.results.len(), 1, "only the real test may report");
}

#[test]
fn test_worker_linestart_forgery_without_token_does_not_forge_result() {
    // A test printing a protocol-shaped record at LINE START (test print()
    // shares the worker's stdout) must not create a result: without the
    // per-spawn unguessable token — absent or wrong — the line passes
    // through as plain output.
    let (summary, interner) = run_fake(
        "echo '@@TOK@@ plan t_real'; \
         echo '@@TOK@@ start t_real'; \
         echo '@@ori-test result evil pass 0'; \
         echo '@@ori-test:deadbeef result evil2 pass 0'; \
         echo '@@TOK@@ result t_real pass 5'; \
         echo '@@TOK@@ done'",
    );
    assert_eq!(summary.passed, 1, "only the real (tokenized) result counts");
    assert!(!summary.has_failures());
    assert!(
        summary
            .results
            .iter()
            .all(|r| r.name_str(&interner) != "evil" && r.name_str(&interner) != "evil2"),
        "line-start forgery without the token must not create a result: {:?}",
        summary.results
    );
    assert_eq!(summary.results.len(), 1, "only the real test may report");
}

#[test]
fn test_worker_file_error_and_llvm_flag_propagate() {
    let (summary, _) = run_fake(
        "echo '@@TOK@@ plan t_cf'; \
         echo '@@TOK@@ result t_cf llvm-compile-fail 0 codegen rejected'; \
         echo '@@TOK@@ file-error LLVM compilation failed: bad IR'; \
         echo '@@TOK@@ llvm-compile-error'; \
         echo '@@TOK@@ done'",
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
            "echo '@@TOK@@ plan t_hang'; \
             echo '@@TOK@@ start t_hang'; \
             sleep 30",
        ),
        Path::new("fake.ori"),
        &interner,
        Duration::from_millis(300),
        TOKEN,
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
        TOKEN,
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
fn test_worker_crash_after_all_results_before_done_records_teardown_failure() {
    // All planned tests reported, then the worker died before `done`: the
    // crash must surface as a file-level failure, not drop silently.
    let (summary, _) = run_fake(
        "echo '@@TOK@@ plan t_ok'; \
         echo '@@TOK@@ start t_ok'; \
         echo '@@TOK@@ result t_ok pass 5'; \
         kill -SEGV $$",
    );
    assert_eq!(summary.passed, 1);
    assert_eq!(summary.failed, 0, "reported results stay as reported");
    assert!(summary.has_failures(), "teardown crash must fail the file");
    assert!(
        summary.errors.iter().any(|e| e.contains("teardown")
            && e.contains("signal 11 (SIGSEGV)")
            && e.contains("after all tests reported")),
        "teardown crash must be classified: {:?}",
        summary.errors
    );
}

/// Reader that panics on first read, faking a harness-internal consumer bug.
struct PanickingReader;

impl std::io::Read for PanickingReader {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        panic!("synthetic consumer failure")
    }
}

#[test]
fn test_consumer_panic_is_trapped_and_returned_as_message() {
    let interner = StringInterner::new();
    let mut state = super::ProtocolState::default();
    let mut summary = super::FileSummary::new(Path::new("fake.ori").to_path_buf());
    let msg = super::consume_worker_stdout_guarded(
        PanickingReader,
        &mut state,
        &mut summary,
        &interner,
        TOKEN,
    );
    assert_eq!(msg.as_deref(), Some("synthetic consumer failure"));
    assert!(
        summary.results.is_empty(),
        "a trapped consumer panic must not fabricate results"
    );
}

#[test]
fn test_worker_done_then_nonzero_exit_records_error_not_crash() {
    let (summary, _) = run_fake(
        "echo '@@TOK@@ plan t_ok'; \
         echo '@@TOK@@ result t_ok pass 5'; \
         echo '@@TOK@@ done'; \
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

#[test]
fn test_kill_worker_on_reaped_child_reports_error_not_silent() {
    // Kill failures must be observable: killing an already-reaped child
    // fails on both the group and the direct-child path, and the failure
    // description must come back instead of being swallowed.
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("exit 0");
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = cmd
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn fake child: {e}"));
    child
        .wait()
        .unwrap_or_else(|e| panic!("failed to reap fake child: {e}"));

    let Some(detail) = super::kill_worker(&mut child) else {
        panic!("kill of a reaped child must surface an error, not stay silent");
    };
    assert!(
        detail.contains("group kill failed"),
        "kill error must name the failed group kill: {detail}"
    );
}

#[test]
fn test_watchdog_with_already_exited_worker_records_no_timeout_or_kill_error() {
    // Boundary-race pin: the worker completed (and was reaped) right at the
    // timeout deadline; the watchdog must treat that as a normal completion —
    // no timeout attribution, no spurious kill-error note.
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg("exit 0");
    let mut child = cmd
        .spawn()
        .unwrap_or_else(|e| panic!("failed to spawn fake child: {e}"));
    child
        .wait()
        .unwrap_or_else(|e| panic!("failed to reap fake child: {e}"));

    let child = std::sync::Arc::new(parking_lot::Mutex::new(child));
    let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let timed_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let kill_error: std::sync::Arc<parking_lot::Mutex<Option<String>>> =
        std::sync::Arc::new(parking_lot::Mutex::new(None));

    // Zero budget: the deadline has already passed on the watchdog's first
    // poll, exercising the exited-at-deadline path deterministically.
    let watchdog = super::spawn_watchdog(&child, &done, &timed_out, &kill_error, Duration::ZERO);
    watchdog
        .join()
        .unwrap_or_else(|_| panic!("watchdog thread panicked"));

    assert!(
        !timed_out.load(std::sync::atomic::Ordering::SeqCst),
        "an already-exited worker at the deadline is not a timeout"
    );
    let recorded = kill_error.lock().take();
    assert!(
        recorded.is_none(),
        "no kill error may be recorded for an already-exited worker: {recorded:?}"
    );
}

#[test]
fn test_generated_protocol_tokens_are_unique_and_token_shaped() {
    let a = super::generate_protocol_token();
    let b = super::generate_protocol_token();
    assert_eq!(a.len(), 32, "token must be 32 hex chars: {a}");
    assert!(
        a.chars().all(|c| c.is_ascii_hexdigit()),
        "token must be hex: {a}"
    );
    assert_ne!(a, b, "per-spawn tokens must differ");
}

#[test]
fn test_build_worker_command_sets_token_on_spawn_env_only() {
    use crate::test::runner::ENV_LOCK;
    let _guard = ENV_LOCK.lock();
    let config = super::TestRunnerConfig::default();
    let (cmd, token) =
        super::build_worker_command(Path::new("/bin/true"), Path::new("fake.ori"), &config, &[]);

    let var: &std::ffi::OsStr = crate::debug_flags::ORI_TEST_PROTOCOL_TOKEN.as_ref();
    let spawn_env: Vec<_> = cmd.get_envs().filter(|(key, _)| *key == var).collect();
    assert_eq!(
        spawn_env.len(),
        1,
        "spawn env must carry the token exactly once"
    );
    assert_eq!(
        spawn_env[0].1,
        Some(std::ffi::OsStr::new(&token)),
        "spawn env token must match the returned token"
    );
    assert!(
        std::env::var(crate::debug_flags::ORI_TEST_PROTOCOL_TOKEN).is_err(),
        "the parent's own environment must never carry the worker token"
    );
}

#[test]
fn test_build_worker_command_incremental_config_carries_incremental_flag() {
    let config = super::TestRunnerConfig {
        incremental: true,
        ..super::TestRunnerConfig::default()
    };
    let (cmd, _token) =
        super::build_worker_command(Path::new("/bin/true"), Path::new("fake.ori"), &config, &[]);
    let args: Vec<_> = cmd.get_args().map(std::ffi::OsStr::to_os_string).collect();
    assert!(
        args.iter().any(|a| a == "--incremental"),
        "a worker command built under incremental config must carry --incremental: {args:?}"
    );
}

#[test]
fn test_build_worker_command_forwards_parent_skip_decisions() {
    let config = super::TestRunnerConfig::default();
    let (cmd, _token) = super::build_worker_command(
        Path::new("/bin/true"),
        Path::new("fake.ori"),
        &config,
        &["t_a".to_string(), "t_b".to_string()],
    );
    let args: Vec<_> = cmd.get_args().map(std::ffi::OsStr::to_os_string).collect();
    assert!(
        args.iter().any(|a| a == "--__skip-unchanged=t_a,t_b"),
        "parent-computed skip decisions must be forwarded to the worker: {args:?}"
    );
}

#[test]
fn test_build_worker_command_empty_skip_set_omits_skip_flag() {
    let config = super::TestRunnerConfig::default();
    let (cmd, _token) =
        super::build_worker_command(Path::new("/bin/true"), Path::new("fake.ori"), &config, &[]);
    let args: Vec<_> = cmd.get_args().map(std::ffi::OsStr::to_os_string).collect();
    assert!(
        args.iter()
            .all(|a| !a.to_string_lossy().starts_with("--__skip-unchanged")),
        "an empty skip set must not add the skip flag: {args:?}"
    );
}

#[test]
fn test_build_worker_command_default_config_omits_incremental_flag() {
    let config = super::TestRunnerConfig::default();
    let (cmd, _token) =
        super::build_worker_command(Path::new("/bin/true"), Path::new("fake.ori"), &config, &[]);
    let args: Vec<_> = cmd.get_args().map(std::ffi::OsStr::to_os_string).collect();
    assert!(
        args.iter().all(|a| a != "--incremental"),
        "a worker command built without incremental config must not carry --incremental: {args:?}"
    );
}

// Protocol validation / dedup

#[test]
fn test_worker_unplanned_result_ignored_with_anomaly_note() {
    let (summary, interner) = run_fake(
        "echo '@@TOK@@ plan t_real'; \
         echo '@@TOK@@ start t_real'; \
         echo '@@TOK@@ result t_real pass 5'; \
         echo '@@TOK@@ result t_ghost pass 5'; \
         echo '@@TOK@@ done'",
    );
    assert_eq!(summary.passed, 1, "only the planned test may count");
    assert!(
        summary
            .results
            .iter()
            .all(|r| r.name_str(&interner) != "t_ghost"),
        "an unplanned result must never become a counted test: {:?}",
        summary.results
    );
    assert!(
        summary
            .errors
            .iter()
            .any(|e| e.contains("unplanned test 't_ghost'")),
        "the unplanned result must surface as a protocol-anomaly note: {:?}",
        summary.errors
    );
}

#[test]
fn test_worker_duplicate_result_first_wins_with_anomaly_note() {
    let (summary, interner) = run_fake(
        "echo '@@TOK@@ plan t_dup'; \
         echo '@@TOK@@ start t_dup'; \
         echo '@@TOK@@ result t_dup pass 5'; \
         echo '@@TOK@@ result t_dup fail 9 late contradiction'; \
         echo '@@TOK@@ done'",
    );
    assert_eq!(summary.passed, 1);
    assert_eq!(
        summary.failed, 0,
        "the late duplicate must not override the first result"
    );
    assert_eq!(
        outcome_of(&summary, &interner, "t_dup"),
        &TestOutcome::Passed
    );
    assert_eq!(
        summary.results.len(),
        1,
        "a duplicate result must not add a second entry"
    );
    assert!(
        summary
            .errors
            .iter()
            .any(|e| e.contains("duplicate result for test 't_dup'")),
        "the duplicate must surface as a protocol-anomaly note: {:?}",
        summary.errors
    );
}

#[test]
fn test_worker_duplicate_plan_dedups_failed_by_crash_per_unique_name() {
    let (summary, _) = run_fake(
        "echo '@@TOK@@ plan t_x'; \
         echo '@@TOK@@ plan t_x'; \
         kill -SEGV $$",
    );
    assert_eq!(
        summary.failed, 1,
        "failed-by-crash attribution must be per unique name despite a duplicate plan"
    );
    assert_eq!(summary.results.len(), 1);
}

// Buffer caps + IO-error propagation (stdout consumer)

#[test]
fn test_consume_stdout_over_cap_line_truncates_and_run_continues() {
    let interner = StringInterner::new();
    let mut state = super::ProtocolState::default();
    let mut summary = super::FileSummary::new(Path::new("fake.ori").to_path_buf());

    let giant = "x".repeat(super::LINE_CAP_BYTES + 10);
    let input = format!(
        "{giant}\n\
         @@ori-test:{TOKEN} plan t_after\n\
         @@ori-test:{TOKEN} start t_after\n\
         @@ori-test:{TOKEN} result t_after pass 5\n\
         @@ori-test:{TOKEN} done\n"
    );
    super::consume_worker_stdout(
        std::io::Cursor::new(input.into_bytes()),
        &mut state,
        &mut summary,
        &interner,
        TOKEN,
    );
    assert!(
        state.done,
        "protocol must keep flowing after an over-cap line"
    );
    assert_eq!(summary.passed, 1);
    assert!(
        summary.errors.is_empty(),
        "an over-cap output line is truncation, not an error: {:?}",
        summary.errors
    );
}

#[test]
fn test_consume_stdout_read_error_records_file_level_note() {
    let interner = StringInterner::new();
    let mut state = super::ProtocolState::default();
    let mut summary = super::FileSummary::new(Path::new("fake.ori").to_path_buf());
    let payload = format!("@@ori-test:{TOKEN} plan t_io\n");
    super::consume_worker_stdout(
        super::test_fixtures::ErrorAfterFirstRead::new(payload.as_bytes()),
        &mut state,
        &mut summary,
        &interner,
        TOKEN,
    );
    assert_eq!(
        state.planned,
        vec!["t_io".to_string()],
        "pre-error lines still consume"
    );
    assert!(
        summary
            .errors
            .iter()
            .any(|e| e.contains("error reading test-worker stdout")
                && e.contains("synthetic read failure")),
        "a stdout read error must record a file-level note, never silently truncate: {:?}",
        summary.errors
    );
}
