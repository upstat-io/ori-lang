//! Worker exit-status classification: signal names and exit-code classes.

use std::process::ExitStatus;

/// Human-readable classification of a worker's exit status.
///
/// Signals render as `signal N (SIGNAME)`; signal numbers outside the mapped
/// set render numerically as `signal N` — never as "unknown signal".
pub(super) fn classify_exit(status: ExitStatus) -> String {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return signal_name(signal).map_or_else(
                || format!("signal {signal}"),
                |name| format!("signal {signal} ({name})"),
            );
        }
    }
    match status.code() {
        Some(code) => classify_exit_code(code),
        None => "unknown termination".to_string(),
    }
}

/// Classify a raw exit code into a human-readable class.
fn classify_exit_code(code: i32) -> String {
    #[cfg(windows)]
    if let Some(class) = windows_abort_class(code) {
        return format!("exit code {code} ({class})");
    }
    match code {
        134 => "exit code 134 (SIGABRT-class abort)".to_string(),
        101 => "exit code 101 (Rust panic)".to_string(),
        code => format!("exit code {code}"),
    }
}

/// Windows abort-class exit codes.
///
/// Consulted by [`classify_exit_code`] on windows ONLY (exit code 3 is an
/// ordinary user exit on unix); compiled on every platform so the table is
/// unit-testable without a windows toolchain.
#[cfg_attr(
    all(not(windows), not(test)),
    expect(
        dead_code,
        reason = "windows-only classification table, kept cross-platform for unit testing"
    )
)]
fn windows_abort_class(code: i32) -> Option<&'static str> {
    /// `STATUS_STACK_BUFFER_OVERRUN` (0xC0000409) as the i32 that
    /// `ExitStatus::code` reports it: `__fastfail` / Rust abort on Windows.
    const STATUS_STACK_BUFFER_OVERRUN: i32 = -1_073_740_791;
    match code {
        3 => Some("SIGABRT-class abort (Windows CRT abort)"),
        STATUS_STACK_BUFFER_OVERRUN => {
            Some("SIGABRT-class abort (0xC0000409 STATUS_STACK_BUFFER_OVERRUN / __fastfail)")
        }
        _ => None,
    }
}

/// Map common POSIX signal numbers to their names.
///
/// Numbers identical on every supported unix platform live here; numbers
/// that diverge between Linux and BSD-derived numbering resolve through
/// `os_signal_name`. Unmapped numbers return `None` (numeric fallback).
#[cfg(unix)]
fn signal_name(signal: i32) -> Option<&'static str> {
    let shared = match signal {
        1 => "SIGHUP",
        2 => "SIGINT",
        3 => "SIGQUIT",
        4 => "SIGILL",
        5 => "SIGTRAP",
        6 => "SIGABRT",
        8 => "SIGFPE",
        9 => "SIGKILL",
        11 => "SIGSEGV",
        13 => "SIGPIPE",
        14 => "SIGALRM",
        15 => "SIGTERM",
        24 => "SIGXCPU",
        25 => "SIGXFSZ",
        _ => return os_signal_name(signal),
    };
    Some(shared)
}

/// Signal numbers under Linux numbering.
#[cfg(any(target_os = "linux", target_os = "android"))]
fn os_signal_name(signal: i32) -> Option<&'static str> {
    let name = match signal {
        7 => "SIGBUS",
        10 => "SIGUSR1",
        12 => "SIGUSR2",
        17 => "SIGCHLD",
        18 => "SIGCONT",
        19 => "SIGSTOP",
        20 => "SIGTSTP",
        31 => "SIGSYS",
        _ => return None,
    };
    Some(name)
}

/// Signal numbers under BSD-derived numbering (macOS, *BSD).
#[cfg(all(unix, not(any(target_os = "linux", target_os = "android"))))]
fn os_signal_name(signal: i32) -> Option<&'static str> {
    let name = match signal {
        10 => "SIGBUS",
        12 => "SIGSYS",
        17 => "SIGSTOP",
        18 => "SIGTSTP",
        19 => "SIGCONT",
        20 => "SIGCHLD",
        30 => "SIGUSR1",
        31 => "SIGUSR2",
        _ => return None,
    };
    Some(name)
}

#[cfg(test)]
mod tests;
