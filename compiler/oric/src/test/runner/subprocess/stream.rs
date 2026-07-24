//! Capped line-oriented reads over worker stdout/stderr pipes.

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::sync::mpsc;
use std::time::Duration;

/// Per-line byte cap on worker stdout/stderr reads.
///
/// INVARIANT: a worker (or JIT'd test code) emitting one enormous line must
/// not OOM the runner — bytes beyond the cap are discarded up to the next
/// newline and the retained prefix is marked truncated.
pub(super) const LINE_CAP_BYTES: usize = 1024 * 1024;

/// Marker appended to a passthrough line truncated at [`LINE_CAP_BYTES`].
pub(super) const LINE_TRUNCATION_MARKER: &str = " [line truncated at 1 MiB by test runner]";

/// Ring-buffer capacity for captured worker stderr lines.
const STDERR_RING_CAPACITY: usize = 200;

/// Bounded wait for the stderr drain's output.
///
/// The drain thread exits when the pipe closes; a worker descendant that
/// survives the kill and holds the write end open would otherwise hang the
/// runner indefinitely.
pub(super) const STDERR_JOIN_TIMEOUT: Duration = Duration::from_secs(5);

/// What the stderr drain hands back: the tail ring plus any read error.
pub(super) type StderrDrainOutput = (VecDeque<String>, Option<String>);

/// Outcome of one capped line read.
pub(super) enum CappedLine {
    /// Stream end with no pending bytes.
    Eof,
    /// One line (newline consumed, not retained in the buffer).
    Line {
        /// Whether bytes beyond the cap were discarded.
        truncated: bool,
    },
}

/// Read one `\n`-terminated line into `buf`, retaining at most `cap` bytes.
///
/// Bytes past the cap are consumed and discarded up to (and including) the
/// newline, so the reader stays line-aligned without ever buffering an
/// unbounded line. A final unterminated line still returns as a `Line`.
pub(super) fn read_line_capped(
    reader: &mut impl BufRead,
    buf: &mut Vec<u8>,
    cap: usize,
) -> std::io::Result<CappedLine> {
    buf.clear();
    let mut truncated = false;
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            if buf.is_empty() && !truncated {
                return Ok(CappedLine::Eof);
            }
            return Ok(CappedLine::Line { truncated });
        }
        let newline = available.iter().position(|&b| b == b'\n');
        let line_end = newline.unwrap_or(available.len());
        let room = cap.saturating_sub(buf.len());
        let take = line_end.min(room);
        buf.extend_from_slice(&available[..take]);
        if take < line_end {
            truncated = true;
        }
        let consumed = newline.map_or(available.len(), |i| i + 1);
        reader.consume(consumed);
        if newline.is_some() {
            return Ok(CappedLine::Line { truncated });
        }
    }
}

/// Drain worker stderr on a thread: pass lines through to the parent's
/// stderr and keep a bounded ring for crash-tail rendering.
///
/// Per-line reads are capped at [`LINE_CAP_BYTES`]. The ring (plus any read
/// error, surfaced as a file-level note by the caller) is delivered over a
/// channel so the caller can bound its wait; the thread itself stays
/// detached and exits when the pipe closes.
pub(super) fn spawn_stderr_drain(
    stderr: impl Read + Send + 'static,
) -> mpsc::Receiver<StderrDrainOutput> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut ring: VecDeque<String> = VecDeque::new();
        let mut reader = BufReader::new(stderr);
        let mut buf: Vec<u8> = Vec::new();
        let mut read_error: Option<String> = None;
        loop {
            let truncated = match read_line_capped(&mut reader, &mut buf, LINE_CAP_BYTES) {
                Ok(CappedLine::Eof) => break,
                Ok(CappedLine::Line { truncated }) => truncated,
                Err(e) => {
                    read_error = Some(e.to_string());
                    break;
                }
            };
            let line_owned = String::from_utf8_lossy(&buf);
            let mut line = line_owned.trim_end_matches(['\n', '\r']).to_string();
            if truncated {
                line.push_str(LINE_TRUNCATION_MARKER);
            }
            eprintln!("{line}");
            if ring.len() == STDERR_RING_CAPACITY {
                ring.pop_front();
            }
            ring.push_back(line);
        }
        // Receiver gone (bounded join timed out): nothing left to deliver.
        let _ = tx.send((ring, read_error));
    });
    rx
}

/// Collect the stderr drain's output with a bounded wait.
///
/// Returns the tail ring plus an optional file-level note (drain read error
/// or join timeout). Joining is bounded: a descendant holding the stderr
/// pipe open past the worker's death costs at most `timeout`, with the tail
/// reported unavailable.
pub(super) fn collect_stderr_ring(
    drain: Option<mpsc::Receiver<StderrDrainOutput>>,
    timeout: Duration,
) -> StderrDrainOutput {
    let Some(rx) = drain else {
        return (VecDeque::new(), None);
    };
    match rx.recv_timeout(timeout) {
        Ok((ring, read_error)) => {
            let note = read_error
                .map(|e| format!("error reading test-worker stderr: {e} (stderr tail truncated)"));
            (ring, note)
        }
        Err(_) => (
            VecDeque::new(),
            Some(format!(
                "test-worker stderr drain did not finish within {timeout:.0?} \
                 (a worker descendant may hold the pipe open); stderr tail unavailable"
            )),
        ),
    }
}

#[cfg(test)]
mod tests;
