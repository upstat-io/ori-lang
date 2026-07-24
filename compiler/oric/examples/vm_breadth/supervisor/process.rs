//! Child completion, bounded cleanup, and sampled process resources.

#[cfg(target_os = "linux")]
use std::fs;
use std::io;
use std::process::{Child, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};

use crate::contract::{Availability, ProcessObservation, Termination, UnavailableReason};
use crate::digest::ByteArtifact;
use crate::errors::{ErrorRecord, HarnessErrorKind};

use super::{harness_error, Supervised};

const POLL_INTERVAL: Duration = Duration::from_millis(2);
const REAP_TIMEOUT: Duration = Duration::from_secs(1);

pub(super) fn supervise_child(
    child: &mut Child,
    child_id: u32,
    started: Instant,
    timeout: Duration,
    peak_rss_kib: &mut Option<u64>,
) -> Termination {
    loop {
        sample_peak_rss(child_id, peak_rss_kib);
        match child.try_wait() {
            Ok(Some(status)) => return termination_from_status(status),
            Ok(None) if started.elapsed() >= timeout => {
                let cleanup_error = terminate_and_reap(child);
                return Termination::TimedOut {
                    limit_ms_decimal: timeout.as_millis().to_string(),
                    kill_error: cleanup_error,
                };
            }
            Ok(None) => thread::sleep(POLL_INTERVAL),
            Err(error) => {
                let mut message = format!("cannot wait for worker process: {error}");
                append_cleanup_error(&mut message, terminate_and_reap(child));
                return Termination::WaitFailed { message };
            }
        }
    }
}

pub(super) fn post_spawn_failure<T>(
    child: &mut Child,
    started: Instant,
    detail: &str,
) -> Supervised<T> {
    let mut message = detail.to_owned();
    append_cleanup_error(&mut message, terminate_and_reap(child));
    Supervised {
        process: ProcessObservation {
            termination: Termination::WaitFailed {
                message: message.clone(),
            },
            elapsed_ns_decimal: Availability::available(started.elapsed().as_nanos().to_string()),
            sampled_peak_rss_kib_lower_bound_decimal: Availability::unavailable(
                UnavailableReason::ObservationMissed,
            ),
            stdout: ByteArtifact::complete(&[]),
            stderr: ByteArtifact::complete(&[]),
        },
        record: None,
        protocol_error: Some(harness_error(HarnessErrorKind::ProcessWait, &message)),
    }
}

pub(super) fn wait_failure_error(termination: &Termination) -> Option<ErrorRecord> {
    let Termination::WaitFailed { message } = termination else {
        return None;
    };
    Some(harness_error(HarnessErrorKind::ProcessWait, message))
}

fn append_cleanup_error(message: &mut String, cleanup_error: Option<String>) {
    if let Some(cleanup_error) = cleanup_error {
        message.push_str("; cleanup: ");
        message.push_str(&cleanup_error);
    }
}

fn terminate_and_reap(child: &mut Child) -> Option<String> {
    let mut errors = Vec::new();
    if let Err(error) = kill_process_group(child) {
        errors.push(error.to_string());
    }
    match bounded_reap(child, REAP_TIMEOUT) {
        Ok(Some(_)) => {}
        Ok(None) => errors.push(format!(
            "worker did not become reapable within {} ms after termination",
            REAP_TIMEOUT.as_millis()
        )),
        Err(error) => errors.push(format!("cannot reap worker process: {error}")),
    }
    (!errors.is_empty()).then(|| errors.join("; "))
}

fn bounded_reap(child: &mut Child, timeout: Duration) -> io::Result<Option<ExitStatus>> {
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait()? {
            Some(status) => return Ok(Some(status)),
            None if Instant::now() >= deadline => return Ok(None),
            None => thread::sleep(POLL_INTERVAL),
        }
    }
}

#[cfg(unix)]
fn termination_from_status(status: ExitStatus) -> Termination {
    use std::os::unix::process::ExitStatusExt;
    match (status.code(), status.signal()) {
        (Some(code), _) => Termination::Exited {
            code_decimal: code.to_string(),
        },
        (_, Some(signal)) => Termination::Signaled {
            signal_decimal: signal.to_string(),
        },
        _ => Termination::Signaled {
            signal_decimal: "unknown".to_owned(),
        },
    }
}

#[cfg(not(unix))]
fn termination_from_status(status: ExitStatus) -> Termination {
    Termination::Exited {
        code_decimal: status
            .code()
            .map_or_else(|| "unknown".to_owned(), |code| code.to_string()),
    }
}

#[cfg(unix)]
fn kill_process_group(child: &mut Child) -> io::Result<()> {
    use rustix::process::{kill_process_group, Pid, Signal};

    let process_group = Pid::from_child(child);
    let group_error = match kill_process_group(process_group, Signal::KILL) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => return Ok(()),
        Err(error) => io::Error::from(error),
    };
    match child.kill() {
        Ok(()) => Err(io::Error::other(format!(
            "cannot signal isolated worker process group: {group_error}; sent termination to the direct child only"
        ))),
        Err(child_error) => Err(io::Error::other(format!(
            "cannot signal isolated worker process group: {group_error}; direct-child fallback also failed: {child_error}"
        ))),
    }
}

#[cfg(not(unix))]
fn kill_process_group(child: &mut Child) -> io::Result<()> {
    child.kill()
}

#[cfg(target_os = "linux")]
pub(super) fn sample_peak_rss(process_id: u32, peak: &mut Option<u64>) {
    let Ok(status) = fs::read_to_string(format!("/proc/{process_id}/status")) else {
        return;
    };
    for line in status.lines() {
        let mut fields = line.split_whitespace();
        let name = fields.next();
        let value = fields.next().and_then(|field| field.parse::<u64>().ok());
        if matches!(name, Some("VmHWM:" | "VmRSS:")) {
            if let Some(value) = value {
                *peak = Some(peak.map_or(value, |current| current.max(value)));
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub(super) fn sample_peak_rss(_process_id: u32, _peak: &mut Option<u64>) {}

#[cfg(target_os = "linux")]
pub(super) fn peak_rss(peak: Option<u64>) -> Availability<String> {
    peak.map_or_else(
        || Availability::unavailable(UnavailableReason::ObservationMissed),
        |value| Availability::available(value.to_string()),
    )
}

#[cfg(not(target_os = "linux"))]
pub(super) fn peak_rss(_peak: Option<u64>) -> Availability<String> {
    Availability::unavailable(UnavailableReason::UnsupportedPlatform)
}
