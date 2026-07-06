use super::stderr_drain_block;

#[test]
fn stderr_drain_block_empty_capture_emits_nothing() {
    assert_eq!(stderr_drain_block(String::new()), None);
}

#[test]
fn stderr_drain_block_newline_terminated_passes_through() {
    assert_eq!(
        stderr_drain_block("line one\nline two\n".to_string()).as_deref(),
        Some("line one\nline two\n")
    );
}

#[test]
fn stderr_drain_block_no_newline_fragment_gains_boundary_newline() {
    assert_eq!(
        stderr_drain_block("fragment without newline".to_string()).as_deref(),
        Some("fragment without newline\n")
    );
}

#[test]
fn stderr_drain_block_mixed_tail_fragment_gains_boundary_newline() {
    assert_eq!(
        stderr_drain_block("full line\ntail fragment".to_string()).as_deref(),
        Some("full line\ntail fragment\n")
    );
}
