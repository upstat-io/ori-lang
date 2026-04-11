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
        // Test has revisions — delete non-revision baseline if it exists
        let non_rev = parent.join(format!("{stem}.{suffix}"));
        if non_rev.exists() {
            fs::remove_file(&non_rev)?;
            deleted.push(non_rev);
        }
    } else {
        // Test has no revisions — delete any revision-specific baselines
        // Pattern: stem.<rev>.suffix (single-dot revision name)
        if let Ok(entries) = fs::read_dir(parent) {
            let prefix = format!("{stem}.");
            let ext = format!(".{suffix}");
            for entry in entries.filter_map(Result::ok) {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with(&prefix)
                    && name.ends_with(&ext)
                    && name != format!("{stem}.{suffix}")
                {
                    let middle = &name[prefix.len()..name.len() - ext.len()];
                    if !middle.is_empty() && !middle.contains('.') {
                        fs::remove_file(entry.path())?;
                        deleted.push(entry.path());
                    }
                }
            }
        }
    }

    Ok(deleted)
}

/// Compare actual test output against the expected baseline, or bless it.
///
/// In bless mode (`ORI_BLESS=1`):
/// - Non-empty actual → write to `expected_path` (creates parent dirs)
/// - Empty actual → delete `expected_path` if it exists
///
/// In normal mode:
/// - Read expected, compare, return Match or Mismatch with diff
pub fn compare_or_bless(expected_path: &Path, actual: &str) -> Result<CompareOutcome, io::Error> {
    if is_bless_enabled() {
        if actual.is_empty() && expected_path.exists() {
            fs::remove_file(expected_path)?;
            return Ok(CompareOutcome::BlessedEmpty);
        }
        if !actual.is_empty() {
            if let Some(parent) = expected_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(expected_path, actual)?;
            return Ok(CompareOutcome::Blessed);
        }
        return Ok(CompareOutcome::BlessedEmpty);
    }

    // Normal mode: compare
    let expected = fs::read_to_string(expected_path).unwrap_or_default();
    if expected == actual {
        Ok(CompareOutcome::Match)
    } else {
        Ok(CompareOutcome::Mismatch {
            diff: diff::generate_diff(&expected, actual),
        })
    }
}
