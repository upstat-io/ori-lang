//! Bless mode for updating test baselines.
//!
//! Controlled exclusively via the `ORI_BLESS=1` environment variable.
//! [`is_bless_enabled`] is the single query point — no other mechanism exists.
//! `cargo test` rejects unrecognized CLI flags, so env var is the only
//! viable control plane.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::diff;

#[cfg(test)]
mod tests;

/// Check if bless mode is active.
///
/// Only `ORI_BLESS=1` enables bless mode. Any other value (including
/// `0`, `false`, `true`) is treated as disabled.
pub fn is_bless_enabled() -> bool {
    std::env::var("ORI_BLESS").is_ok_and(|v| v == "1")
}

/// Outcome of comparing expected vs actual test output.
#[derive(Debug, PartialEq, Eq)]
pub enum CompareOutcome {
    /// Expected matches actual.
    Match,
    /// Blessed: wrote new/updated baseline.
    Blessed,
    /// Blessed: removed empty baseline file.
    BlessedEmpty,
    /// Mismatch with unified diff.
    Mismatch { diff: String },
}

/// Clean up stale baseline files when revisions change.
///
/// When a test adds revisions, the old non-revision baseline becomes stale.
/// When a test removes revisions, old revision-specific baselines become stale.
/// Call this in bless mode before writing new baselines.
///
/// Returns the list of deleted file paths.
pub fn clean_stale_baselines(
    test_path: &Path,
    suffix: &str,
    active_revisions: &[&str],
) -> Result<Vec<PathBuf>, io::Error> {
    let parent = test_path.parent().unwrap_or(Path::new(""));
    let stem = test_path.file_stem().unwrap_or_default().to_string_lossy();
    let mut deleted = Vec::new();

    let has_revisions = !(active_revisions.is_empty()
        || active_revisions.len() == 1 && active_revisions[0].is_empty());

    if has_revisions {
        // Test has revisions — delete non-revision baseline if it exists.
        // Only deletes the unambiguous non-revision file (stem.suffix).
        // Stale revision-specific cleanup is NOT done here because the
        // naming convention (stem.<rev>.suffix) is ambiguous with artifact
        // role suffixes (stem.before.suffix). Consumers handle specific
        // revision cleanup in their TestStrategy implementation.
        let non_rev = parent.join(format!("{stem}.{suffix}"));
        if non_rev.exists() {
            fs::remove_file(&non_rev)?;
            deleted.push(non_rev);
        }
    }
    // No else branch: when there are no revisions, we do NOT scan for
    // stale revision-specific baselines because the naming convention
    // (stem.<middle>.suffix) is ambiguous with artifact role suffixes
    // (stem.before.suffix) and sibling test baselines. Consumers handle
    // specific cleanup in their TestStrategy::clean_stale_revisions().

    Ok(deleted)
}

/// Compare actual test output against the expected baseline, or bless it.
///
/// The `bless` parameter controls whether to write baselines (true) or
/// compare (false). Callers query `is_bless_enabled()` once at the top
/// of the test run and pass the result here — this avoids process-global
/// env var reads deep in the call stack and prevents test-parallelism
/// race conditions.
///
/// In bless mode:
/// - Non-empty actual → write to `expected_path` (creates parent dirs)
/// - Empty actual → delete `expected_path` if it exists
///
/// In normal mode:
/// - Read expected, compare, return Match or Mismatch with diff
pub fn compare_or_bless(
    expected_path: &Path,
    actual: &str,
    bless: bool,
) -> Result<CompareOutcome, io::Error> {
    if bless {
        // Normalize to LF before blessing so baselines are always LF, even on Windows
        let blessed_content = actual.replace("\r\n", "\n");
        if blessed_content.is_empty() && expected_path.exists() {
            fs::remove_file(expected_path)?;
            return Ok(CompareOutcome::BlessedEmpty);
        }
        if !blessed_content.is_empty() {
            if let Some(parent) = expected_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(expected_path, &blessed_content)?;
            return Ok(CompareOutcome::Blessed);
        }
        return Ok(CompareOutcome::BlessedEmpty);
    }

    // Normal mode: compare (normalize line endings to LF for cross-platform parity)
    let expected = match fs::read_to_string(expected_path) {
        Ok(s) => s.replace("\r\n", "\n"),
        Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    let actual_normalized = actual.replace("\r\n", "\n");
    if expected == actual_normalized {
        Ok(CompareOutcome::Match)
    } else {
        Ok(CompareOutcome::Mismatch {
            diff: diff::generate_diff(&expected, &actual_normalized),
        })
    }
}
