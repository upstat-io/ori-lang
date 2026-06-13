//! Parent-side per-file subprocess isolation for the LLVM JIT test backend.
//!
//! JIT-executed test code shares the runner's address space, so a crashing
//! test (SIGSEGV from a codegen bug, a runtime double-free abort) would kill
//! the whole run before any summary prints. Isolation contract: each test
//! file runs in a fresh worker process (`ori test --backend=llvm --__worker
//! <file>`); the worker streams the `test::protocol` line protocol; the
//! parent classifies worker death (signal name / exit class), counts the
//! in-flight test as FAILED with a stderr tail, accounts the file's
//! remaining tests as failed-by-crash, and continues with the next file.

mod exit_class;
mod finalize;
pub(super) mod snapshot;
mod stream;
#[cfg(test)]
mod test_fixtures;
mod token;

use std::io::{BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustc_hash::FxHashSet;

use self::finalize::finalize_worker_exit;
use self::snapshot::{classify_spawn_failure, SpawnFailure};
use self::stream::{
    collect_stderr_ring, read_line_capped, spawn_stderr_drain, CappedLine, LINE_CAP_BYTES,
    LINE_TRUNCATION_MARKER, STDERR_JOIN_TIMEOUT,
};
use self::token::generate_protocol_token;
use super::super::protocol::{self, WorkerEvent};
use super::super::result::{FileSummary, TestResult};
use super::TestRunnerConfig;

/// Default per-file worker wall-clock budget in seconds.
///
/// A test that never terminates (e.g. a miscompiled loop spinning under the
/// JIT) would otherwise hang the whole run; the watchdog kills the worker at
/// the budget and the in-flight test is counted FAILED (timeout).
const DEFAULT_WORKER_TIMEOUT_SECS: u64 = 120;

/// Per-file worker budget: `ORI_TEST_WORKER_TIMEOUT_SECS` override or default.
fn worker_timeout() -> Duration {
    std::env::var(crate::debug_flags::ORI_TEST_WORKER_TIMEOUT_SECS)
        .ok()
        .and_then(|v| v.parse::<u64>().ok())
        .map_or(
            Duration::from_secs(DEFAULT_WORKER_TIMEOUT_SECS),
            Duration::from_secs,
        )
}

/// Run one test file in a fresh worker process and reconstruct its summary.
///
/// `skip_names` carries the parent-computed incremental skip decisions
/// (tests whose targets are unchanged per the PARENT-owned cache); the
/// worker reports each as `SkippedUnchanged` without running it.
pub(super) fn run_file_isolated(
    path: &Path,
    interner: &crate::ir::StringInterner,
    config: &TestRunnerConfig,
    skip_names: &[String],
    snapshot_path: Option<&Path>,
) -> FileSummary {
    // Prefer the run-start snapshot (a stable inode immune to a concurrent
    // rebuild replacing `target/<profile>/ori`); fall back to `current_exe()`
    // when no snapshot was taken (e.g. snapshotting failed at runner start).
    let exe = match snapshot_path {
        Some(p) => p.to_path_buf(),
        None => match std::env::current_exe() {
            Ok(exe) => exe,
            Err(e) => {
                let mut summary = FileSummary::new(path.to_path_buf());
                summary.add_error(format!(
                    "cannot locate the test-runner executable to spawn a worker: {e}"
                ));
                return summary;
            }
        },
    };

    let (cmd, token) = build_worker_command(&exe, path, config, skip_names);
    run_worker_command(cmd, path, interner, worker_timeout(), &token)
}

/// Build the worker command for one test file, with a fresh per-spawn
/// protocol token.
///
/// INVARIANT: the token is set ONLY on the spawn env (`Command::env`) —
/// the parent's own environment never carries it, so nothing the parent
/// later spawns can see another worker's nonce.
pub(super) fn build_worker_command(
    exe: &Path,
    path: &Path,
    config: &TestRunnerConfig,
    skip_names: &[String],
) -> (Command, String) {
    let mut cmd = Command::new(exe);
    cmd.arg("test").arg("--backend=llvm").arg("--__worker");
    if let Some(filter) = &config.filter {
        cmd.arg(format!("--filter={filter}"));
    }
    if config.incremental {
        cmd.arg("--incremental");
    }
    // Parent-computed incremental skip decisions (the parent owns the
    // cache; the worker never computes its own).
    if !skip_names.is_empty() {
        cmd.arg(format!("--__skip-unchanged={}", skip_names.join(",")));
    }
    cmd.arg(path);
    cmd.env("RUST_BACKTRACE", "1");

    // Per-spawn unguessable nonce: protocol lines parse only when stamped
    // with it, so test code printing protocol-shaped lines at line start
    // cannot forge results (test `print()` shares the worker's stdout).
    let token = generate_protocol_token();
    cmd.env(crate::debug_flags::ORI_TEST_PROTOCOL_TOKEN, &token);
    (cmd, token)
}

/// Streaming protocol state accumulated while a worker runs.
#[derive(Default)]
struct ProtocolState {
    /// Announced tests, in announcement order, deduplicated (set semantics:
    /// a duplicate `plan` record is ignored, so failed-by-crash attribution
    /// is per unique name).
    planned: Vec<String>,
    /// Membership index over [`ProtocolState::planned`].
    planned_set: FxHashSet<String>,
    /// Tests that produced a result record (first result wins).
    resulted: FxHashSet<String>,
    /// Test that started but has not yet produced a result.
    in_flight: Option<String>,
    /// Whether the worker emitted its normal-completion record.
    done: bool,
}

/// Spawn the worker command, stream its protocol, classify its exit.
///
/// Takes a pre-built [`Command`] so tests can substitute fake workers, the
/// per-spawn protocol token the worker's lines must carry, and a per-file
/// wall-clock budget: a worker still alive at the deadline is killed by the
/// watchdog and its in-flight test counted FAILED (timeout).
pub(super) fn run_worker_command(
    mut cmd: Command,
    path: &Path,
    interner: &crate::ir::StringInterner,
    timeout: Duration,
    token: &str,
) -> FileSummary {
    let mut summary = FileSummary::new(path.to_path_buf());
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // The worker leads its own process group so a watchdog kill takes out
    // any descendants too — a surviving descendant holding the inherited
    // stdout pipe would keep the runner blocked past the deadline.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }

    let mut child = match cmd.spawn() {
        Ok(child) => child,
        Err(e) => {
            // A NotFound spawn error means the worker binary was replaced or
            // lost mid-run; flag it so the per-file loop collapses the cascade
            // into ONE abort instead of N per-file spawn errors.
            if classify_spawn_failure(&e) == SpawnFailure::BinaryReplaced {
                summary.binary_replaced = true;
            }
            summary.add_error(format!("failed to spawn test worker: {e}"));
            return summary;
        }
    };

    let stderr_drain = child.stderr.take().map(spawn_stderr_drain);
    let stdout = child.stdout.take();

    let child = Arc::new(parking_lot::Mutex::new(child));
    let done = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    let kill_error: Arc<parking_lot::Mutex<Option<String>>> =
        Arc::new(parking_lot::Mutex::new(None));
    let watchdog = spawn_watchdog(&child, &done, &timed_out, &kill_error, timeout);

    let mut state = ProtocolState::default();
    let consumer_panic = stdout.and_then(|stdout| {
        consume_worker_stdout_guarded(stdout, &mut state, &mut summary, interner, token)
    });
    if consumer_panic.is_some() {
        // Harness-internal failure: take the worker group down NOW instead
        // of leaving it to run out the watchdog budget.
        // INVARIANT: `done` is set BEFORE the kill so the watchdog can never
        // attribute this kill to a timeout — the failure entry stays the
        // consumer-panic one.
        done.store(true, Ordering::SeqCst);
        let error = kill_worker(&mut child.lock());
        record_kill_error(&kill_error, error);
    }

    let status = wait_for_exit(&child);
    done.store(true, Ordering::SeqCst);
    let _ = watchdog.join();
    // Bounded stderr join BEFORE finalize consumes the tail: the drain exits
    // when the pipe closes; a surviving descendant holding the write end open
    // costs at most the timeout instead of hanging the runner.
    let (stderr_ring, stderr_note) = collect_stderr_ring(stderr_drain, STDERR_JOIN_TIMEOUT);

    let timeout_budget = timed_out.load(Ordering::SeqCst).then_some(timeout);
    let kill_failure = kill_error.lock().take();
    match status {
        Ok(status) => finalize_worker_exit(
            &state,
            status,
            timeout_budget,
            kill_failure.as_deref(),
            &stderr_ring,
            &mut summary,
            interner,
        ),
        Err(e) => summary.add_error(format!("failed to wait for test worker: {e}")),
    }
    if let Some(note) = stderr_note {
        summary.add_error(note);
    }
    if let Some(panic_msg) = consumer_panic {
        summary.add_error(format!(
            "internal test-runner error while consuming worker output \
             (worker killed): {panic_msg}"
        ));
    }

    summary
}

/// Watchdog poll interval (also bounds post-EOF reap-poll latency).
const WATCHDOG_POLL: Duration = Duration::from_millis(50);

/// Spawn the per-file watchdog: kills the worker at the deadline unless the
/// runner signals completion first.
fn spawn_watchdog(
    child: &Arc<parking_lot::Mutex<Child>>,
    done: &Arc<AtomicBool>,
    timed_out: &Arc<AtomicBool>,
    kill_error: &Arc<parking_lot::Mutex<Option<String>>>,
    timeout: Duration,
) -> std::thread::JoinHandle<()> {
    let child = Arc::clone(child);
    let done = Arc::clone(done);
    let timed_out = Arc::clone(timed_out);
    let kill_error = Arc::clone(kill_error);
    std::thread::spawn(move || {
        let deadline = Instant::now() + timeout;
        while !done.load(Ordering::SeqCst) {
            if Instant::now() >= deadline {
                let mut guard = child.lock();
                // Boundary guard: a worker that already exited at the
                // deadline — reaped by `wait_for_exit` (`try_wait` returns
                // the cached status) or exited-unreaped — completed
                // normally; attribute no timeout, record no kill error.
                if done.load(Ordering::SeqCst) || matches!(guard.try_wait(), Ok(Some(_))) {
                    return;
                }
                timed_out.store(true, Ordering::SeqCst);
                // SIGKILL cannot be caught: the worker WILL die, its pipes
                // close, and the runner's stdout read unblocks.
                let error = kill_worker(&mut guard);
                record_kill_error(&kill_error, error);
                return;
            }
            std::thread::sleep(WATCHDOG_POLL);
        }
    })
}

/// Record a kill failure for crash-detail rendering (first failure wins).
fn record_kill_error(slot: &parking_lot::Mutex<Option<String>>, error: Option<String>) {
    if let Some(error) = error {
        let mut guard = slot.lock();
        if guard.is_none() {
            *guard = Some(error);
        }
    }
}

/// Kill the worker and (on unix) its whole process group.
///
/// Returns a description of any kill failure so the caller can surface it
/// in the crash-failure detail — never swallowed; `None` when the group
/// kill succeeded.
#[cfg(unix)]
#[expect(
    unsafe_code,
    reason = "kill(2) by negative pid is the only kill-by-process-group path; std has no equivalent"
)]
fn kill_worker(child: &mut Child) -> Option<String> {
    // Group kill: the worker is its own group leader per `process_group(0)`,
    // so a negative pid addresses the whole group. std has no kill-by-pgid.
    let group_result = match i32::try_from(child.id()) {
        // SAFETY: kill(2) takes a pid and a signal number; it has no
        // memory-safety preconditions and reports failure via errno.
        Ok(pid) => match unsafe { libc::kill(-pid, libc::SIGKILL) } {
            0 => Ok(()),
            _ => Err(std::io::Error::last_os_error().to_string()),
        },
        Err(_) => Err(format!("worker pid {} exceeds pid_t range", child.id())),
    };
    // Fallback: always also kill the direct child pid so a group-kill
    // failure still takes the worker itself down.
    let child_result = child.kill();
    match (group_result, child_result) {
        (Ok(()), _) => None,
        (Err(group_err), Ok(())) => Some(format!(
            "group kill failed ({group_err}); direct child kill succeeded"
        )),
        (Err(group_err), Err(child_err)) => Some(format!(
            "group kill failed ({group_err}); direct child kill failed ({child_err})"
        )),
    }
}

/// Kill the worker process (no process groups off unix).
///
/// Returns a description of any kill failure; `None` on success.
#[cfg(not(unix))]
fn kill_worker(child: &mut Child) -> Option<String> {
    child
        .kill()
        .err()
        .map(|e| format!("child kill failed ({e})"))
}

/// Reap the worker after its stdout reached EOF.
///
/// Poll-based (`try_wait`) so the watchdog's `kill()` is never starved by a
/// blocking `wait()` holding the child lock — covers a worker that closes
/// stdout and then hangs without exiting.
fn wait_for_exit(child: &Arc<parking_lot::Mutex<Child>>) -> std::io::Result<ExitStatus> {
    loop {
        if let Some(status) = child.lock().try_wait()? {
            return Ok(status);
        }
        std::thread::sleep(WATCHDOG_POLL);
    }
}

/// Run [`consume_worker_stdout`], trapping harness-internal panics.
///
/// Returns the panic message when the consumer itself failed; the caller
/// kills the worker group immediately and records a file-level error.
fn consume_worker_stdout_guarded(
    stdout: impl Read,
    state: &mut ProtocolState,
    summary: &mut FileSummary,
    interner: &crate::ir::StringInterner,
    token: &str,
) -> Option<String> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        consume_worker_stdout(stdout, state, summary, interner, token);
    }))
    .err()
    .map(|payload| super::panic_message(payload.as_ref()))
}

/// Read worker stdout to EOF: protocol lines update state; everything else
/// (test program output) passes through verbatim.
///
/// A protocol record parses ONLY when the marker starts the line AND the
/// line carries the per-spawn token — a mid-line `@@ori-test` (test output
/// embedding the marker) or a line-start marker without the unguessable
/// token passes through as plain output, so test programs cannot forge
/// protocol events.
///
/// Per-line reads are capped at [`LINE_CAP_BYTES`]; a read error records a
/// file-level note instead of silently truncating the stream (the worker's
/// exit classification still governs the failure accounting).
fn consume_worker_stdout(
    stdout: impl Read,
    state: &mut ProtocolState,
    summary: &mut FileSummary,
    interner: &crate::ir::StringInterner,
    token: &str,
) {
    let mut reader = BufReader::new(stdout);
    let mut buf: Vec<u8> = Vec::new();
    loop {
        let truncated = match read_line_capped(&mut reader, &mut buf, LINE_CAP_BYTES) {
            Ok(CappedLine::Eof) => break,
            Ok(CappedLine::Line { truncated }) => truncated,
            Err(e) => {
                summary.add_error(format!(
                    "error reading test-worker stdout: {e} (worker output truncated)"
                ));
                break;
            }
        };
        let line_owned = String::from_utf8_lossy(&buf);
        let line = line_owned.trim_end_matches(['\n', '\r']);

        match protocol::parse_line(line, token) {
            // A truncated protocol line keeps its (truncated) payload — the
            // prefix/name/kind fields all precede the free-text detail, so
            // the result accounting survives an over-cap failure detail.
            Some(event) => consume_event(event, state, summary, interner),
            // Non-protocol line, mid-line marker, absent/mismatched token,
            // or malformed protocol-shaped line: surface verbatim.
            None if truncated => println!("{line}{LINE_TRUNCATION_MARKER}"),
            None => println!("{line}"),
        }
    }
}

/// Apply one protocol event to the accumulating state + summary.
///
/// Validation/dedup contract:
/// - duplicate `plan` for one name: ignored (set semantics)
/// - `result` for a name never planned: ignored, protocol-anomaly note
/// - duplicate `result` for one name: first wins, protocol-anomaly note
fn consume_event(
    event: WorkerEvent,
    state: &mut ProtocolState,
    summary: &mut FileSummary,
    interner: &crate::ir::StringInterner,
) {
    match event {
        WorkerEvent::Plan { name } => {
            if state.planned_set.insert(name.clone()) {
                state.planned.push(name);
            }
        }
        WorkerEvent::Start { name } => state.in_flight = Some(name),
        WorkerEvent::Result {
            name,
            outcome,
            duration,
        } => {
            if !state.planned_set.contains(&name) {
                // A result the worker never announced is a harness bug (test
                // code cannot forge tokenized lines): keep it out of the test
                // counts but surface it as a file-level anomaly.
                summary.add_error(format!(
                    "worker protocol anomaly: result for unplanned test '{name}' ignored"
                ));
                return;
            }
            if !state.resulted.insert(name.clone()) {
                summary.add_error(format!(
                    "worker protocol anomaly: duplicate result for test '{name}' ignored \
                     (first result wins)"
                ));
                return;
            }
            if state.in_flight.as_deref() == Some(name.as_str()) {
                state.in_flight = None;
            }
            summary.add_result(TestResult {
                name: interner.intern(&name),
                targets: vec![],
                outcome,
                duration,
            });
        }
        WorkerEvent::FileError { message } => summary.add_error(message),
        WorkerEvent::LlvmCompileError => summary.llvm_compile_error = true,
        WorkerEvent::Done => state.done = true,
    }
}

#[cfg(test)]
mod tests;
