//! Resolves expected and actual test-artifact paths.
//!
//! Expected artifacts stay beside source tests; actual artifacts live under
//! the Cargo target directory. Callers supply suffixes and revisions.

use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

/// Resolved paths for expected and actual artifact files.
#[derive(Debug, Clone)]
pub struct ArtifactPaths {
    /// Path to the source-controlled baseline.
    pub expected: PathBuf,
    /// Path to generated output in the Cargo target tree.
    pub actual: PathBuf,
}

/// Resolve the expected baseline path for a test artifact.
///
/// Expected baselines live alongside the test source file. A revision suffix
/// precedes the artifact extension when present.
///
/// # Examples
///
/// - `resolve_expected("tests/rc/basic.ori", "ll", None)` -> `tests/rc/basic.ll`
/// - `resolve_expected("tests/rc/basic.ori", "ll", Some("release"))` -> `tests/rc/basic.release.ll`
pub fn resolve_expected(test_path: &Path, suffix: &str, revision: Option<&str>) -> PathBuf {
    let parent = test_path.parent().unwrap_or_else(|| Path::new(""));
    parent.join(artifact_filename(test_path, suffix, revision))
}

/// Resolve the actual output path for a test artifact.
///
/// Actual outputs go under `CARGO_TARGET_DIR/test-harness/`, falling back to
/// `target/test-harness/` when Cargo has no explicit target directory. This is
/// deterministic and survives for debugging without writing into a read-only
/// source snapshot. The source path remains relative to the working tree so
/// same-stem files in different directories cannot collide.
pub fn resolve_actual(test_path: &Path, suffix: &str, revision: Option<&str>) -> PathBuf {
    let target_dir =
        std::env::var_os("CARGO_TARGET_DIR").map_or_else(|| PathBuf::from("target"), PathBuf::from);
    let cwd = std::env::current_dir().ok();
    resolve_actual_under(test_path, suffix, revision, &target_dir, cwd.as_deref())
}

fn resolve_actual_under(
    test_path: &Path,
    suffix: &str,
    revision: Option<&str>,
    target_dir: &Path,
    working_dir: Option<&Path>,
) -> PathBuf {
    let relative_test_path = working_dir
        .and_then(|root| test_path.strip_prefix(root).ok())
        .unwrap_or(test_path);
    let parent = relative_test_path.parent().unwrap_or_else(|| Path::new(""));
    // INVARIANT: Normal components cannot replace the target-tree base during `Path::join`.
    let relative_parent: PathBuf = parent
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .collect();

    target_dir
        .join("test-harness")
        .join(relative_parent)
        .join(artifact_filename(test_path, suffix, revision))
}

fn artifact_filename(test_path: &Path, suffix: &str, revision: Option<&str>) -> OsString {
    let stem = test_path.file_stem().unwrap_or_else(|| {
        panic!(
            "test artifact path must end in a filename such as `case.ori`: {}",
            test_path.display()
        )
    });
    let mut filename = stem.to_os_string();
    if let Some(revision) = revision.filter(|revision| !revision.is_empty()) {
        filename.push(".");
        filename.push(revision);
    }
    filename.push(".");
    filename.push(suffix);
    filename
}

#[cfg(test)]
mod tests;
