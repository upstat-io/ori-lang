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

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rustc_hash::FxHashSet;

use super::super::protocol::{self, WorkerEvent, PROTOCOL_PREFIX};
use super::super::result::{FileSummary, TestResult};
use super::TestRunnerConfig;

/// Number of trailing worker-stderr lines rendered into crash failures.
const STDERR_TAIL_LINES: usize = 20;

/// Ring-buffer capacity for captured worker stderr lines.
const STDERR_RING_CAPACITY: usize = 200;

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
pub(super) fn run_file_isolated(
    path: &Path,
    interner: &crate::ir::StringInterner,
    config: &TestRunnerConfig,
) -> FileSummary {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            let mut summary = FileSummary::new(path.to_path_buf());
            summary.add_error(format!(
                "cannot locate the test-runner executable to spawn a worker: {e}"
            ));
            return summary;
        }
    };

    let mut cmd = Command::new(exe);
    cmd.arg("test").arg("--backend=llvm").arg("--__worker");
    if let Some(filter) = &config.filter {
        cmd.arg(format!("--filter={filter}"));
    }
    if config.incremental {
        cmd.arg("--incremental");
    }
    cmd.arg(path);
    cmd.env("RUST_BACKTRACE", "1");

    run_worker_command(cmd, path, interner, worker_timeout())
}

/// Streaming protocol state accumulated while a worker runs.
#[derive(Default)]
struct ProtocolState {
    /// Announced tests, in announcement order.
    planned: Vec<String>,
    /// Tests that produced a result record.
    resulted: FxHashSet<String>,
    /// Test that started but has not yet produced a result.
    in_flight: Option<String>,
    /// Whether the worker emitted its normal-completion record.
    done: bool,
}

/// Spawn the worker command, stream its protocol, classify its exit.
///
/// Takes a pre-built [`Command`] so tests can substitute fake workers, and a
/// per-file wall-clock budget: a worker still alive at the deadline is killed
/// by the watchdog and its in-flight test counted FAILED (timeout).
pub(super) fn run_worker_command(
    mut cmd: Command,
    path: &Path,
    interner: &crate::ir::StringInterner,
    timeout: Duration,
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
            summary.add_error(format!("failed to spawn test worker: {e}"));
            return summary;
        }
    };

    let stderr_drain = child.stderr.take().map(spawn_stderr_drain);
    let stdout = child.stdout.take();

    let child = Arc::new(parking_lot::Mutex::new(child));
    let done = Arc::new(AtomicBool::new(false));
    let timed_out = Arc::new(AtomicBool::new(false));
    let watchdog = spawn_watchdog(&child, &done, &timed_out, timeout);

    let mut state = ProtocolState::default();
    if let Some(stdout) = stdout {
        consume_worker_stdout(stdout, &mut state, &mut summary, interner);
    }

    let status = wait_for_exit(&child);
    done.store(true, Ordering::SeqCst);
    let _ = watchdog.join();
    let stderr_ring = stderr_drain
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default();

    let timeout_budget = timed_out.load(Ordering::SeqCst).then_some(timeout);
    match status {
        Ok(status) => finalize_worker_exit(
            &state,
            status,
            timeout_budget,
            &stderr_ring,
            &mut summary,
            interner,
        ),
        Err(e) => summary.add_error(format!("failed to wait for test worker: {e}")),
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
    timeout: Duration,
) -> std::thread::JoinHandle<()> {
    let child = Arc::clone(child);
    let done = Arc::clone(done);
    let timed_out = Arc::clone(timed_out);
    std::thread::spawn(move || {
        let deadline = Instant::now() + timeout;
        while !done.load(Ordering::SeqCst) {
            if Instant::now() >= deadline {
                timed_out.store(true, Ordering::SeqCst);
                // SIGKILL cannot be caught: the worker WILL die, its pipes
                // close, and the runner's stdout read unblocks.
                kill_worker(&mut child.lock());
                return;
            }
            std::thread::sleep(WATCHDOG_POLL);
        }
    })
}

/// Kill the worker and (on unix) its whole process group.
#[cfg(unix)]
fn kill_worker(child: &mut Child) {
    // Group kill (the worker is its own group leader per `process_group(0)`)
    // via the POSIX `kill` utility — std has no kill-by-pgid.
    let _ = Command::new("kill")
        .args(["-KILL", "--", &format!("-{}", child.id())])
        .status();
    let _ = child.kill();
}

/// Kill the worker process (no process groups off unix).
#[cfg(not(unix))]
fn kill_worker(child: &mut Child) {
    let _ = child.kill();
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

/// Read worker stdout to EOF: protocol lines update state; everything else
/// (test program output) passes through verbatim.
fn consume_worker_stdout(
    stdout: impl Read,
    state: &mut ProtocolState,
    summary: &mut FileSummary,
    interner: &crate::ir::StringInterner,
) {
    let mut reader = BufReader::new(stdout);
    let mut buf: Vec<u8> = Vec::new();
    loop {
        buf.clear();
        match reader.read_until(b'\n', &mut buf) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let line_owned = String::from_utf8_lossy(&buf);
        let line = line_owned.trim_end_matches(['\n', '\r']);

        // A test printing without a trailing newline can glue its output to
        // the front of a protocol record; locate the marker anywhere.
        let Some(marker) = line.find(PROTOCOL_PREFIX.trim_end()) else {
            println!("{line}");
            continue;
        };
        let (passthrough, candidate) = line.split_at(marker);
        match protocol::parse_line(candidate) {
            Some(event) => {
                if !passthrough.is_empty() {
                    println!("{passthrough}");
                }
                consume_event(event, state, summary, interner);
            }
            // Malformed protocol-shaped line: surface verbatim.
            None => println!("{line}"),
        }
    }
}

/// Apply one protocol event to the accumulating state + summary.
fn consume_event(
    event: WorkerEvent,
    state: &mut ProtocolState,
    summary: &mut FileSummary,
    interner: &crate::ir::StringInterner,
) {
    match event {
        WorkerEvent::Plan { name } => state.planned.push(name),
        WorkerEvent::Start { name } => state.in_flight = Some(name),
        WorkerEvent::Result {
            name,
            outcome,
            duration,
        } => {
            if state.in_flight.as_deref() == Some(name.as_str()) {
                state.in_flight = None;
            }
            summary.add_result(TestResult {
                name: interner.intern(&name),
                targets: vec![],
                outcome,
                duration,
            });
            state.resulted.insert(name);
        }
        WorkerEvent::FileError { message } => summary.add_error(message),
        WorkerEvent::LlvmCompileError => summary.llvm_compile_error = true,
        WorkerEvent::Done => state.done = true,
    }
}

/// Classify the worker's exit and account for tests lost to a crash.
fn finalize_worker_exit(
    state: &ProtocolState,
    status: ExitStatus,
    timeout_budget: Option<Duration>,
    stderr_ring: &VecDeque<String>,
    summary: &mut FileSummary,
    interner: &crate::ir::StringInterner,
) {
    if state.done {
        if !status.success() {
            summary.add_error(format!(
                "test worker exited abnormally after completing its file: {}",
                classify_exit(status)
            ));
        }
        return;
    }

    // Protocol truncated: the worker died (or exited) mid-file.
    let class = match timeout_budget {
        Some(budget) => format!(
            "watchdog timeout after {budget:.0?} — worker killed ({})",
            classify_exit(status)
        ),
        None => classify_exit(status),
    };
    let tail = stderr_tail(stderr_ring);
    let mut tail_attached = false;

    // The in-flight test is the crash site: count it FAILED with full detail.
    if let Some(name) = state.in_flight.as_deref() {
        if !state.resulted.contains(name) {
            summary.add_result(TestResult::failed(
                interner.intern(name),
                vec![],
                format!("test crashed: worker terminated by {class}{tail}"),
                Duration::ZERO,
            ));
            tail_attached = true;
        }
    }

    // Remaining announced tests never ran: count each FAILED (failed-by-crash).
    let crash_site = state
        .in_flight
        .clone()
        .unwrap_or_else(|| "LLVM compilation/setup".to_string());
    for name in &state.planned {
        if state.resulted.contains(name) || state.in_flight.as_deref() == Some(name.as_str()) {
            continue;
        }
        let detail_tail = if tail_attached {
            String::new()
        } else {
            tail_attached = true;
            tail.clone()
        };
        summary.add_result(TestResult::failed(
            interner.intern(name),
            vec![],
            format!(
                "not run: test worker crashed ({class}) during '{crash_site}' — \
                 counted as failed-by-crash{detail_tail}"
            ),
            Duration::ZERO,
        ));
    }

    // Crash before test discovery: no per-test anchor exists, so record a
    // file-level error (counts toward the failing exit code).
    if state.planned.is_empty() && state.in_flight.is_none() {
        summary.add_error(format!(
            "test worker crashed before test discovery: {class}{tail}"
        ));
    }
}

/// Drain worker stderr on a thread: pass lines through to the parent's
/// stderr and keep a bounded ring for crash-tail rendering.
fn spawn_stderr_drain(
    stderr: impl Read + Send + 'static,
) -> std::thread::JoinHandle<VecDeque<String>> {
    std::thread::spawn(move || {
        let mut ring: VecDeque<String> = VecDeque::new();
        let mut reader = BufReader::new(stderr);
        let mut buf: Vec<u8> = Vec::new();
        loop {
            buf.clear();
            match reader.read_until(b'\n', &mut buf) {
                Ok(0) | Err(_) => break,
                Ok(_) => {}
            }
            let line_owned = String::from_utf8_lossy(&buf);
            let line = line_owned.trim_end_matches(['\n', '\r']);
            eprintln!("{line}");
            if ring.len() == STDERR_RING_CAPACITY {
                ring.pop_front();
            }
            ring.push_back(line.to_string());
        }
        ring
    })
}

/// Render the last [`STDERR_TAIL_LINES`] captured stderr lines.
fn stderr_tail(ring: &VecDeque<String>) -> String {
    if ring.is_empty() {
        return String::new();
    }
    let skip = ring.len().saturating_sub(STDERR_TAIL_LINES);
    let tail: Vec<&str> = ring.iter().skip(skip).map(String::as_str).collect();
    format!(
        "\n--- worker stderr (last {} lines) ---\n{}",
        tail.len(),
        tail.join("\n")
    )
}

/// Human-readable classification of a worker's exit status.
fn classify_exit(status: ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return format!("signal {signal} ({})", signal_name(signal));
        }
    }
    match status.code() {
        Some(134) => "exit code 134 (SIGABRT-class abort)".to_string(),
        Some(101) => "exit code 101 (Rust panic)".to_string(),
        Some(code) => format!("exit code {code}"),
        None => "unknown termination".to_string(),
    }
}

/// Map common POSIX signal numbers to their names.
#[cfg(unix)]
fn signal_name(signal: i32) -> &'static str {
    match signal {
        4 => "SIGILL",
        5 => "SIGTRAP",
        6 => "SIGABRT",
        7 => "SIGBUS",
        8 => "SIGFPE",
        9 => "SIGKILL",
        11 => "SIGSEGV",
        13 => "SIGPIPE",
        15 => "SIGTERM",
        _ => "unknown signal",
    }
}

#[cfg(test)]
mod tests;
