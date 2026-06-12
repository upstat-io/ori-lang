//! Line-oriented worker protocol for subprocess-isolated test execution.
//!
//! A test worker (a child `ori test --backend=llvm --__worker <file>` process)
//! streams one protocol line per event to stdout; the parent runner parses the
//! stream to reconstruct per-test results and to attribute worker crashes to
//! the in-flight test. Every emit flushes immediately so the parent sees the
//! `start` record for a test even when the JIT'd test code kills the worker.
//!
//! Line grammar (one event per line, space-delimited; test names are Ori
//! identifiers and never contain spaces):
//!
//! ```text
//! @@ori-test plan <name>
//! @@ori-test start <name>
//! @@ori-test result <name> pass <nanos>
//! @@ori-test result <name> fail <nanos> <escaped-detail>
//! @@ori-test result <name> skip <nanos> <escaped-detail>
//! @@ori-test result <name> skip-unchanged <nanos>
//! @@ori-test result <name> llvm-compile-fail <nanos> <escaped-detail>
//! @@ori-test file-error <escaped-message>
//! @@ori-test llvm-compile-error
//! @@ori-test done
//! ```
//!
//! Free-text payloads escape `\`, newline, and carriage return so every event
//! stays on one line. Non-protocol lines (test program output) pass through.

use std::io::Write;
use std::time::Duration;

use super::result::TestOutcome;

/// Marker prefix identifying a protocol line.
pub const PROTOCOL_PREFIX: &str = "@@ori-test ";

/// One parsed worker-protocol event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WorkerEvent {
    /// A test that will produce a result record (emitted before execution
    /// begins, so the parent can account for tests lost to a crash).
    Plan { name: String },
    /// A test is about to execute (the crash-attribution anchor).
    Start { name: String },
    /// A test produced a result.
    Result {
        name: String,
        outcome: TestOutcome,
        duration: Duration,
    },
    /// A file-level (parse/type/LLVM) error message.
    FileError { message: String },
    /// The file's errors are LLVM compilation errors.
    LlvmCompileError,
    /// The worker completed the file normally.
    Done,
}

/// Escape a free-text payload onto a single line.
fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

/// Reverse [`escape`].
fn unescape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('\\') | None => out.push('\\'),
            Some(other) => {
                // Unknown escape: preserve verbatim.
                out.push('\\');
                out.push(other);
            }
        }
    }
    out
}

/// Write one protocol line to stdout and flush immediately.
///
/// The flush is load-bearing: stdout is block-buffered on a pipe, and a
/// crashing JIT'd test kills the worker without running buffer flushes.
fn emit_line(line: &str) {
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "{line}");
    let _ = out.flush();
}

/// Emit a `plan` record for a test that will produce a result.
pub fn emit_plan(name: &str) {
    emit_line(&format!("{PROTOCOL_PREFIX}plan {name}"));
}

/// Emit a `start` record immediately before a test executes.
pub fn emit_start(name: &str) {
    emit_line(&format!("{PROTOCOL_PREFIX}start {name}"));
}

/// Emit a `result` record for a completed test.
pub fn emit_result(name: &str, outcome: &TestOutcome, duration: Duration) {
    let nanos = u64::try_from(duration.as_nanos()).unwrap_or(u64::MAX);
    let line = match outcome {
        TestOutcome::Passed => format!("{PROTOCOL_PREFIX}result {name} pass {nanos}"),
        TestOutcome::Failed(detail) => {
            format!(
                "{PROTOCOL_PREFIX}result {name} fail {nanos} {}",
                escape(detail)
            )
        }
        TestOutcome::Skipped(reason) => {
            format!(
                "{PROTOCOL_PREFIX}result {name} skip {nanos} {}",
                escape(reason)
            )
        }
        TestOutcome::SkippedUnchanged => {
            format!("{PROTOCOL_PREFIX}result {name} skip-unchanged {nanos}")
        }
        TestOutcome::LlvmCompileFail(detail) => format!(
            "{PROTOCOL_PREFIX}result {name} llvm-compile-fail {nanos} {}",
            escape(detail)
        ),
    };
    emit_line(&line);
}

/// Emit a file-level error message.
pub fn emit_file_error(message: &str) {
    emit_line(&format!("{PROTOCOL_PREFIX}file-error {}", escape(message)));
}

/// Emit the LLVM-compile-error file flag.
pub fn emit_llvm_compile_error() {
    emit_line(&format!("{PROTOCOL_PREFIX}llvm-compile-error"));
}

/// Emit the normal-completion record.
pub fn emit_done() {
    emit_line(&format!("{PROTOCOL_PREFIX}done"));
}

/// Parse one line into a protocol event.
///
/// Returns `None` for non-protocol lines AND for malformed protocol-prefixed
/// lines; the caller passes both through as ordinary output.
pub fn parse_line(line: &str) -> Option<WorkerEvent> {
    let rest = line.strip_prefix(PROTOCOL_PREFIX.trim_end())?;
    let rest = rest.strip_prefix(' ').unwrap_or(rest);
    let (kind, tail) = rest.split_once(' ').unwrap_or((rest, ""));
    match kind {
        "plan" => parse_single_name(tail).map(|name| WorkerEvent::Plan { name }),
        "start" => parse_single_name(tail).map(|name| WorkerEvent::Start { name }),
        "result" => parse_result(tail),
        "file-error" => Some(WorkerEvent::FileError {
            message: unescape(tail),
        }),
        "llvm-compile-error" if tail.is_empty() => Some(WorkerEvent::LlvmCompileError),
        "done" if tail.is_empty() => Some(WorkerEvent::Done),
        _ => None,
    }
}

/// Parse a single bare test name (no embedded spaces).
fn parse_single_name(tail: &str) -> Option<String> {
    if tail.is_empty() || tail.contains(' ') {
        return None;
    }
    Some(tail.to_string())
}

/// Parse the tail of a `result` record: `<name> <kind> <nanos> [detail]`.
fn parse_result(tail: &str) -> Option<WorkerEvent> {
    let (name, rest) = tail.split_once(' ')?;
    if name.is_empty() {
        return None;
    }
    let (kind, rest) = rest.split_once(' ').unwrap_or((rest, ""));
    let (nanos_str, detail) = rest.split_once(' ').unwrap_or((rest, ""));
    let nanos: u64 = nanos_str.parse().ok()?;
    let duration = Duration::from_nanos(nanos);
    let outcome = match kind {
        "pass" if detail.is_empty() => TestOutcome::Passed,
        "fail" => TestOutcome::Failed(unescape(detail)),
        "skip" => TestOutcome::Skipped(unescape(detail)),
        "skip-unchanged" if detail.is_empty() => TestOutcome::SkippedUnchanged,
        "llvm-compile-fail" => TestOutcome::LlvmCompileFail(unescape(detail)),
        _ => return None,
    };
    Some(WorkerEvent::Result {
        name: name.to_string(),
        outcome,
        duration,
    })
}

#[cfg(test)]
mod tests;
