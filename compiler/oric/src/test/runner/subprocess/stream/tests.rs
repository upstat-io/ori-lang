//! Pins for capped line reads, the stderr drain, and the bounded join.

use std::collections::VecDeque;
use std::io::Cursor;
use std::sync::mpsc;
use std::time::Duration;

use super::super::test_fixtures::ErrorAfterFirstRead;
use super::{
    collect_stderr_ring, read_line_capped, spawn_stderr_drain, CappedLine, StderrDrainOutput,
    LINE_CAP_BYTES, LINE_TRUNCATION_MARKER,
};

fn recv_drain(rx: &mpsc::Receiver<StderrDrainOutput>) -> StderrDrainOutput {
    rx.recv_timeout(Duration::from_secs(10))
        .unwrap_or_else(|e| panic!("stderr drain did not deliver: {e}"))
}

#[test]
fn test_read_line_capped_truncates_over_cap_line_and_stays_line_aligned() {
    let mut data = vec![b'a'; LINE_CAP_BYTES + 4096];
    data.push(b'\n');
    data.extend_from_slice(b"short\n");
    let mut reader = Cursor::new(data);
    let mut buf: Vec<u8> = Vec::new();

    let first = read_line_capped(&mut reader, &mut buf, LINE_CAP_BYTES)
        .unwrap_or_else(|e| panic!("read failed: {e}"));
    assert!(
        matches!(first, CappedLine::Line { truncated: true }),
        "over-cap line must report truncation"
    );
    assert_eq!(
        buf.len(),
        LINE_CAP_BYTES,
        "retained bytes must stop at the cap"
    );

    let second = read_line_capped(&mut reader, &mut buf, LINE_CAP_BYTES)
        .unwrap_or_else(|e| panic!("read failed: {e}"));
    assert!(
        matches!(second, CappedLine::Line { truncated: false }),
        "the next line must read normally (reader stays line-aligned)"
    );
    assert_eq!(buf, b"short");

    let third = read_line_capped(&mut reader, &mut buf, LINE_CAP_BYTES)
        .unwrap_or_else(|e| panic!("read failed: {e}"));
    assert!(matches!(third, CappedLine::Eof));
}

#[test]
fn test_read_line_capped_returns_final_unterminated_line() {
    let mut reader = Cursor::new(b"tail without newline".to_vec());
    let mut buf: Vec<u8> = Vec::new();
    let read = read_line_capped(&mut reader, &mut buf, LINE_CAP_BYTES)
        .unwrap_or_else(|e| panic!("read failed: {e}"));
    assert!(matches!(read, CappedLine::Line { truncated: false }));
    assert_eq!(buf, b"tail without newline");
}

#[test]
fn test_stderr_drain_truncates_over_cap_line_and_keeps_later_lines() {
    let mut data = vec![b'y'; LINE_CAP_BYTES + 10];
    data.push(b'\n');
    data.extend_from_slice(b"last line\n");
    let rx = spawn_stderr_drain(Cursor::new(data));
    let (ring, read_error) = recv_drain(&rx);
    assert_eq!(read_error, None);
    assert_eq!(ring.len(), 2);
    assert!(
        ring[0].ends_with(LINE_TRUNCATION_MARKER),
        "over-cap stderr line must carry the truncation marker: ...{}",
        &ring[0][ring[0].len().saturating_sub(80)..]
    );
    assert_eq!(
        ring[1], "last line",
        "the run must continue past the over-cap line"
    );
}

#[test]
fn test_stderr_drain_read_error_is_reported_not_silent() {
    let rx = spawn_stderr_drain(ErrorAfterFirstRead::new(b"warm line\n"));
    let (ring, read_error) = recv_drain(&rx);
    assert_eq!(ring.len(), 1);
    assert_eq!(ring[0], "warm line");
    assert!(
        read_error.is_some_and(|e| e.contains("synthetic read failure")),
        "stderr read errors must surface, never silently truncate"
    );
}

#[test]
fn test_collect_stderr_ring_times_out_with_truncation_note() {
    let (tx, rx) = mpsc::channel::<StderrDrainOutput>();
    let (ring, note) = collect_stderr_ring(Some(rx), Duration::from_millis(50));
    assert!(ring.is_empty());
    assert!(
        note.is_some_and(|n| n.contains("did not finish")),
        "a drain that never delivers must yield a bounded-join truncation note"
    );
    drop(tx);
}

#[test]
fn test_collect_stderr_ring_delivers_ring_and_maps_read_error_to_note() {
    let (tx, rx) = mpsc::channel::<StderrDrainOutput>();
    let mut ring = VecDeque::new();
    ring.push_back("a stderr line".to_string());
    tx.send((ring, Some("pipe exploded".to_string())))
        .unwrap_or_else(|e| panic!("send failed: {e}"));
    let (got_ring, note) = collect_stderr_ring(Some(rx), Duration::from_secs(1));
    assert_eq!(got_ring.len(), 1);
    assert!(
        note.is_some_and(|n| n.contains("pipe exploded") && n.contains("stderr")),
        "a drain read error must map to a file-level note"
    );
}

#[test]
fn test_collect_stderr_ring_without_drain_is_empty_and_clean() {
    let (ring, note) = collect_stderr_ring(None, Duration::from_millis(10));
    assert!(ring.is_empty());
    assert_eq!(note, None);
}
