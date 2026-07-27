//! Tests for two-build remark diffing.

use crate::{ingest, Stream};

use super::diff_streams;

/// Every fixture carries a current-generation header; ingest refuses remarks without one.
const HEADER: &str = r#"{"record":"header","schema_version":2,"compiler_sha":"000de0232","source_file":"t.ori"}"#;

/// A remark line identified by `function` + `ssa_value`; other fields fixed.
fn remark(function: &str, ssa: u64) -> String {
    format!(
        r#"{{"kind":"missed","pass":"aims-burden-elim","name":"rc-inc-not-elided","rc_op":"burden_inc","function":"{function}","debug_loc":null,"ssa_value":{ssa},"exit_block":null,"cause":null,"burden_net":null,"args":[],"cow_mode":null}}"#
    )
}

fn build(lines: &[String]) -> Stream {
    // A header is MANDATORY: ingest refuses an unversioned remark, so a fixture
    // without one would exercise the refusal path instead of the case under test.
    let body = format!("{HEADER}\n{}", lines.join("\n"));
    ingest(&body).unwrap_or_else(|e| panic!("ingest failed: {e}"))
}

#[test]
fn added_removed_persisted_partition() {
    // baseline: f#1, f#2 ; candidate: f#2 (persisted), g#3 (added). f#1 removed.
    let baseline = build(&[remark("f", 1), remark("f", 2)]);
    let candidate = build(&[remark("f", 2), remark("g", 3)]);
    let diff = diff_streams(&baseline, &candidate);

    assert_eq!(diff.added.len(), 1, "one new survivor (regression)");
    assert_eq!(diff.added[0].function.as_deref(), Some("g"), "added is g#3");
    assert_eq!(diff.added[0].ssa_value, Some(3), "added ssa");
    assert_eq!(diff.removed.len(), 1, "one eliminated survivor (improvement)");
    assert_eq!(diff.removed[0].function.as_deref(), Some("f"), "removed is f#1");
    assert_eq!(diff.removed[0].ssa_value, Some(1), "removed ssa");
    assert_eq!(diff.persisted, 1, "f#2 persisted");
}

#[test]
fn identical_streams_have_no_added_or_removed() {
    let stream = build(&[remark("f", 1), remark("f", 2)]);
    let diff = diff_streams(&stream, &stream);
    assert!(diff.added.is_empty(), "no regressions for identical streams");
    assert!(diff.removed.is_empty(), "no improvements for identical streams");
    assert_eq!(diff.persisted, 2, "both keys persist");
}

#[test]
fn full_elimination_is_all_removed() {
    // candidate eliminated every survivor → all removed, nothing added/persisted.
    let baseline = build(&[remark("f", 1), remark("g", 2)]);
    let candidate = Stream::default();
    let diff = diff_streams(&baseline, &candidate);
    assert_eq!(diff.removed.len(), 2, "every baseline survivor eliminated");
    assert!(diff.added.is_empty(), "nothing added");
    assert_eq!(diff.persisted, 0, "nothing persisted");
}

#[test]
fn new_survivors_from_empty_baseline_are_all_added() {
    let baseline = Stream::default();
    let candidate = build(&[remark("f", 1)]);
    let diff = diff_streams(&baseline, &candidate);
    assert_eq!(diff.added.len(), 1, "regression vs empty baseline");
    assert!(diff.removed.is_empty(), "nothing removed");
    assert_eq!(diff.persisted, 0, "nothing persisted");
}

#[test]
fn added_dedups_repeated_keys() {
    // Two identical-key remarks in the candidate collapse to one added entry.
    let baseline = Stream::default();
    let candidate = build(&[remark("f", 1), remark("f", 1)]);
    let diff = diff_streams(&baseline, &candidate);
    assert_eq!(diff.added.len(), 1, "repeated key dedups to one added remark");
}
