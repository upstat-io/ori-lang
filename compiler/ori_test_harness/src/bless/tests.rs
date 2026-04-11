#![allow(
    deprecated,
    reason = "set_var/remove_var deprecated in newer Rust, safe in edition 2021"
)]
#![expect(
    clippy::expect_used,
    reason = "test code — panics are the assertion mechanism"
)]

use super::*;

// ---------------------------------------------------------------------------
// Positive (semantic pins)
// ---------------------------------------------------------------------------

#[test]
fn test_bless_writes_new_baseline_when_env_set_to_1() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("baseline.ll");

    std::env::set_var("ORI_BLESS", "1");
    let result = compare_or_bless(&path, "define void @main() {}");
    std::env::remove_var("ORI_BLESS");

    assert_eq!(result.expect("should succeed"), CompareOutcome::Blessed);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "define void @main() {}"
    );
}

#[test]
fn test_bless_deletes_empty_baseline_when_env_set() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("baseline.ll");
    std::fs::write(&path, "old content").expect("write");

    std::env::set_var("ORI_BLESS", "1");
    let result = compare_or_bless(&path, "");
    std::env::remove_var("ORI_BLESS");

    assert_eq!(
        result.expect("should succeed"),
        CompareOutcome::BlessedEmpty
    );
    assert!(!path.exists());
}

#[test]
fn test_compare_returns_match_when_content_identical() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("baseline.ll");
    std::fs::write(&path, "same content").expect("write");

    std::env::remove_var("ORI_BLESS");
    let result = compare_or_bless(&path, "same content");
    assert_eq!(result.expect("should succeed"), CompareOutcome::Match);
}

#[test]
fn test_compare_returns_mismatch_with_diff_when_content_differs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("baseline.ll");
    std::fs::write(&path, "expected line\n").expect("write");

    std::env::remove_var("ORI_BLESS");
    let result = compare_or_bless(&path, "actual line\n");
    match result.expect("should succeed") {
        CompareOutcome::Mismatch { diff } => {
            assert!(diff.contains("-expected line"));
            assert!(diff.contains("+actual line"));
        }
        other => panic!("expected Mismatch, got {other:?}"),
    }
}

#[test]
fn test_bless_creates_parent_directories() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sub/dir/baseline.ll");

    std::env::set_var("ORI_BLESS", "1");
    let result = compare_or_bless(&path, "content");
    std::env::remove_var("ORI_BLESS");

    assert_eq!(result.expect("should succeed"), CompareOutcome::Blessed);
    assert!(path.exists());
}

// ---------------------------------------------------------------------------
// Negative pins
// ---------------------------------------------------------------------------

#[test]
fn test_bless_disabled_when_env_is_zero() {
    std::env::set_var("ORI_BLESS", "0");
    assert!(!is_bless_enabled());
    std::env::remove_var("ORI_BLESS");
}

#[test]
fn test_bless_disabled_when_env_is_false() {
    std::env::set_var("ORI_BLESS", "false");
    assert!(!is_bless_enabled());
    std::env::remove_var("ORI_BLESS");
}

#[test]
fn test_bless_disabled_when_env_is_true() {
    std::env::set_var("ORI_BLESS", "true");
    assert!(!is_bless_enabled());
    std::env::remove_var("ORI_BLESS");
}

#[test]
fn test_bless_disabled_when_env_unset() {
    std::env::remove_var("ORI_BLESS");
    assert!(!is_bless_enabled());
}
