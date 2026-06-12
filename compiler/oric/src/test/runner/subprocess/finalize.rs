//! Worker exit accounting: crash attribution and protocol-gap detection.

use std::collections::VecDeque;
use std::process::ExitStatus;
use std::time::Duration;

use super::super::super::result::{FileSummary, TestResult};
use super::exit_class::classify_exit;
use super::ProtocolState;

/// Number of trailing worker-stderr lines rendered into crash failures.
const STDERR_TAIL_LINES: usize = 20;

/// Classify the worker's exit and account for tests lost to a crash.
///
/// `kill_error` carries any kill-path failure (group kill / direct-child
/// kill) so it lands in the failure detail — never swallowed.
pub(super) fn finalize_worker_exit(
    state: &ProtocolState,
    status: ExitStatus,
    timeout_budget: Option<Duration>,
    kill_error: Option<&str>,
    stderr_ring: &VecDeque<String>,
    summary: &mut FileSummary,
    interner: &crate::ir::StringInterner,
) {
    if state.done {
        finalize_completed_worker(state, status, summary, interner);
        if let Some(error) = kill_error {
            // No crash-failure entry exists on the completed path; a
            // kill-path failure still surfaces as a file-level error.
            summary.add_error(format!("worker kill error: {error}"));
        }
        return;
    }

    // Protocol truncated: the worker died (or exited) mid-file.
    let mut class = match timeout_budget {
        Some(budget) => format!(
            "watchdog timeout after {budget:.0?} — worker killed ({})",
            classify_exit(status)
        ),
        None => classify_exit(status),
    };
    if let Some(error) = kill_error {
        class = format!("{class}; kill error: {error}");
    }
    let tail = stderr_tail(stderr_ring);
    let mut tail_attached = false;
    let mut crash_attributed = false;

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
            crash_attributed = true;
        }
    }

    // Remaining announced tests never ran: count each FAILED (failed-by-crash).
    // `state.planned` is deduplicated at consume time, so attribution is per
    // unique name.
    let crash_site = state
        .in_flight
        .clone()
        .unwrap_or_else(|| "LLVM compilation/setup".to_string());
    for name in &state.planned {
        if state.resulted.contains(name) || state.in_flight.as_deref() == Some(name.as_str()) {
            continue;
        }
        crash_attributed = true;
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

    if crash_attributed {
        return;
    }
    if state.planned.is_empty() && state.in_flight.is_none() {
        // Crash before test discovery: no per-test anchor exists, so record a
        // file-level error (counts toward the failing exit code).
        summary.add_error(format!(
            "test worker crashed before test discovery: {class}{tail}"
        ));
    } else {
        // Worker died after every planned test reported but before `done`.
        // Why: a crash after green results can indicate corrupt state; one
        // file-level failure keeps the exit nonzero and the crash visible.
        summary.add_error(format!(
            "test worker crashed during teardown ({class}) after all tests reported{tail}"
        ));
    }
}

/// Account a worker that emitted `done`: flag abnormal exits and planned
/// tests that never produced a result record.
fn finalize_completed_worker(
    state: &ProtocolState,
    status: ExitStatus,
    summary: &mut FileSummary,
    interner: &crate::ir::StringInterner,
) {
    for name in &state.planned {
        if state.resulted.contains(name) {
            continue;
        }
        // Why: under the start-of-line protocol guard a result record glued
        // onto unterminated test output is passthrough; surface the gap.
        summary.add_result(TestResult::failed(
            interner.intern(name),
            vec![],
            "no result record: test was planned but never reported a result \
             (worker protocol gap)"
                .to_string(),
            Duration::ZERO,
        ));
    }
    if !status.success() {
        summary.add_error(format!(
            "test worker exited abnormally after completing its file: {}",
            classify_exit(status)
        ));
    }
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
