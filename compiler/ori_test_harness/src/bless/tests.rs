#![expect(
    clippy::expect_used,
    reason = "test code — panics are the assertion mechanism"
)]

use super::*;

// ---------------------------------------------------------------------------
// Positive (semantic pins)
// ---------------------------------------------------------------------------

#[test]
fn test_bless_writes_new_baseline_when_enabled() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("baseline.ll");

    let result = compare_or_bless(&path, "define void @main() {}", true);

    assert_eq!(result.expect("should succeed"), CompareOutcome::Blessed);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read"),
        "define void @main() {}"
    );
}

#[test]
fn test_bless_deletes_empty_baseline_when_enabled() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("baseline.ll");
    std::fs::write(&path, "old content").expect("write");

    let result = compare_or_bless(&path, "", true);

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

    let result = compare_or_bless(&path, "same content", false);
    assert_eq!(result.expect("should succeed"), CompareOutcome::Match);
}

#[test]
fn test_compare_returns_mismatch_with_diff_when_content_differs() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("baseline.ll");
    std::fs::write(&path, "expected line\n").expect("write");

    let result = compare_or_bless(&path, "actual line\n", false);
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

    let result = compare_or_bless(&path, "content", true);

    assert_eq!(result.expect("should succeed"), CompareOutcome::Blessed);
    assert!(path.exists());
}

#[test]
fn test_bless_cleans_old_revision_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let test_path = dir.path().join("basic.ori");

    // Create a stale non-revision baseline (from before revisions were added)
    let stale = dir.path().join("basic.ll");
    std::fs::write(&stale, "old baseline").expect("write");

    // Now the test has revisions — clean up stale baselines
    let deleted = clean_stale_baselines(&test_path, "ll", &["debug", "release"]).expect("clean");
    assert_eq!(deleted.len(), 1);
    assert!(
        !stale.exists(),
        "stale non-revision baseline should be deleted"
    );
}

#[test]
fn test_bless_cleans_old_revision_specific_files() {
    let dir = tempfile::tempdir().expect("tempdir");
    let test_path = dir.path().join("basic.ori");

    // Create stale revision-specific baselines (from before revisions were removed)
    let stale_debug = dir.path().join("basic.debug.ll");
    let stale_release = dir.path().join("basic.release.ll");
    std::fs::write(&stale_debug, "debug baseline").expect("write");
    std::fs::write(&stale_release, "release baseline").expect("write");

    // Now the test has no revisions — clean up stale revision baselines
    let deleted = clean_stale_baselines(&test_path, "ll", &[""]).expect("clean");
    assert_eq!(deleted.len(), 2);
    assert!(!stale_debug.exists());
    assert!(!stale_release.exists());
}

#[test]
fn test_bless_with_revisions_only_deletes_non_revision_baseline() {
    let dir = tempfile::tempdir().expect("tempdir");
    let test_path = dir.path().join("basic.ori");

    // Non-revision baseline + revision baselines (including stale one)
    let non_rev = dir.path().join("basic.ll");
    let active_a = dir.path().join("basic.a.ll");
    let stale_b = dir.path().join("basic.b.ll");
    std::fs::write(&non_rev, "old baseline").expect("write");
    std::fs::write(&active_a, "a baseline").expect("write");
    std::fs::write(&stale_b, "b baseline").expect("write");

    // has_revisions only deletes the non-revision baseline (safe)
    let deleted = clean_stale_baselines(&test_path, "ll", &["a"]).expect("clean");
    assert_eq!(deleted.len(), 1);
    assert!(!non_rev.exists(), "non-revision baseline deleted");
    assert!(active_a.exists(), "revision baselines not touched");
    assert!(stale_b.exists(), "revision baselines not touched");
}

// ---------------------------------------------------------------------------
// Negative pins
// ---------------------------------------------------------------------------

/// Consolidated env var test — runs all `ORI_BLESS` variations sequentially
/// in a single test to avoid parallel `set_var`/`remove_var` races.
#[allow(
    deprecated,
    reason = "set_var deprecated in newer Rust, safe in single-threaded test"
)]
#[test]
fn test_is_bless_enabled_contract() {
    std::env::remove_var("ORI_BLESS");
    assert!(!is_bless_enabled(), "unset = disabled");

    std::env::set_var("ORI_BLESS", "0");
    assert!(!is_bless_enabled(), "env=0 = disabled");

    std::env::set_var("ORI_BLESS", "false");
    assert!(!is_bless_enabled(), "env=false = disabled");

    std::env::set_var("ORI_BLESS", "true");
    assert!(
        !is_bless_enabled(),
        "env=true = disabled (only '1' accepted)"
    );

    std::env::set_var("ORI_BLESS", "1");
    assert!(is_bless_enabled(), "env=1 = enabled");

    std::env::remove_var("ORI_BLESS");
}
