//! Artifact path resolution for test baselines and actual output.
//!
//! Provides generic path resolution helpers. Artifact NAMING (what the
//! suffix/extension is) is the consumer's responsibility via `TestStrategy`.
//! The harness resolves WHERE baselines live and how revision suffixes are
//! inserted — not WHAT the file extension or naming convention is.

use std::path::{Path, PathBuf};

#[cfg(test)]
mod tests;

/// Resolved paths for expected and actual artifact files.
#[derive(Debug, Clone)]
pub struct ArtifactPaths {
    /// Expected baseline file (in source tree, alongside test file).
    pub expected: PathBuf,
    /// Actual output file (in build/temp directory).
    pub actual: PathBuf,
}

/// Resolve the expected baseline path for a test artifact.
///
/// Expected baselines live alongside the test source file.
/// Revision suffix is inserted before the extension when present.
///
/// Examples:
/// - `resolve_expected("tests/rc/basic.ori", "ll", None)` → `tests/rc/basic.ll`
/// - `resolve_expected("tests/rc/basic.ori", "ll", Some("release"))` → `tests/rc/basic.release.ll`
pub fn resolve_expected(test_path: &Path, suffix: &str, revision: Option<&str>) -> PathBuf {
    let stem = test_path.file_stem().unwrap_or_default();
    let parent = test_path.parent().unwrap_or(Path::new(""));

    match revision {
        Some(rev) if !rev.is_empty() => {
            parent.join(format!("{}.{rev}.{suffix}", stem.to_string_lossy()))
        }
        _ => parent.join(format!("{}.{suffix}", stem.to_string_lossy())),
    }
}

/// Resolve the actual output path for a test artifact.
///
/// Actual outputs go under `target/test-harness/` (deterministic, survives
/// for debugging — not `$TMPDIR`). Preserves the parent directory structure
/// relative to the test root to prevent same-stem collisions.
pub fn resolve_actual(test_path: &Path, suffix: &str, revision: Option<&str>) -> PathBuf {
    let stem = test_path.file_stem().unwrap_or_default();
    let parent = test_path.parent().unwrap_or(Path::new(""));
    // Strip root from absolute paths so Path::join doesn't discard the base
    let relative_parent = parent.strip_prefix("/").unwrap_or(parent);

    let filename = match revision {
        Some(rev) if !rev.is_empty() => {
            format!("{}.{rev}.{suffix}", stem.to_string_lossy())
        }
        _ => format!("{}.{suffix}", stem.to_string_lossy()),
    };

    Path::new("target/test-harness")
        .join(relative_parent)
        .join(filename)
}
