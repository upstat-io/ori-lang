//! Artifact path resolution for test baselines and actual output.
//!
//! Provides generic path resolution helpers. Artifact NAMING (what the
//! suffix/extension is) is the consumer's responsibility via `TestStrategy`.
//! The harness resolves WHERE baselines live and how revision suffixes are
//! inserted — not WHAT the file extension or naming convention is.

use std::path::{Component, Path, PathBuf};

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
    let parent = test_path.parent().unwrap_or_else(|| Path::new(""));

    match revision {
        Some(rev) if !rev.is_empty() => {
            parent.join(format!("{}.{rev}.{suffix}", stem.to_string_lossy()))
        }
        _ => parent.join(format!("{}.{suffix}", stem.to_string_lossy())),
    }
}

/// Resolve the actual output path for a test artifact.
///
/// Actual outputs go under `$CARGO_TARGET_DIR/test-harness/`, falling back to
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
    let stem = test_path.file_stem().unwrap_or_default();
    let relative_test_path = working_dir
        .and_then(|root| test_path.strip_prefix(root).ok())
        .unwrap_or(test_path);
    let parent = relative_test_path.parent().unwrap_or_else(|| Path::new(""));
    // Extract only normal components — strips roots, prefixes (C:\, /), and
    // parent refs (..) so Path::join never discards the base. Cross-platform.
    let relative_parent: PathBuf = parent
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .collect();

    let filename = match revision {
        Some(rev) if !rev.is_empty() => {
            format!("{}.{rev}.{suffix}", stem.to_string_lossy())
        }
        _ => format!("{}.{suffix}", stem.to_string_lossy()),
    };

    target_dir
        .join("test-harness")
        .join(relative_parent)
        .join(filename)
}

#[cfg(test)]
mod tests;
