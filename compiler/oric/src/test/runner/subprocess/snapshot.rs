//! Worker-binary snapshot for the LLVM JIT test backend.
//!
//! `run_file_isolated` re-exec's the runner binary once per test file. A
//! concurrent `cargo build` replacing `target/<profile>/ori`'s inode mid-run
//! makes every later `Command::spawn` return `ENOENT`, cascading to one failure
//! per remaining file. Snapshotting the binary once at runner start to a private
//! temp (a stable inode immune to the replacement) and spawning THAT path keeps
//! in-flight workers spawnable; if even the snapshot is lost, the run collapses
//! to ONE abort via [`classify_spawn_failure`] instead of N per-file errors.

use std::io;
use std::path::{Path, PathBuf};

use tempfile::TempDir;

/// Classification of a worker-spawn failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::test::runner) enum SpawnFailure {
    /// The worker binary path no longer resolves (replaced or lost mid-run).
    /// The run aborts with ONE diagnostic instead of cascading N per-file
    /// errors.
    BinaryReplaced,
    /// Any other spawn failure (permissions, resource limits) — file-local.
    Other,
}

/// Classify a `Command::spawn` error: `NotFound` means the worker binary was
/// replaced or lost during the run; everything else is a file-local failure.
pub(in crate::test::runner) fn classify_spawn_failure(err: &io::Error) -> SpawnFailure {
    match err.kind() {
        io::ErrorKind::NotFound => SpawnFailure::BinaryReplaced,
        _ => SpawnFailure::Other,
    }
}

/// A snapshot of the runner binary at a stable temp path.
///
/// The `TempDir` is the RAII owner: its `Drop` removes the snapshot and its
/// directory when the `ExeSnapshot` drops (at run end). Holding the snapshot on
/// the `TestRunner` for the whole run is load-bearing — dropping it early (e.g.
/// storing only [`ExeSnapshot::path`] and letting the guard fall out of scope)
/// deletes the temp and re-introduces the spawn cascade.
#[derive(Debug)]
pub(in crate::test::runner) struct ExeSnapshot {
    // Field order is drop order: `path` (a plain `PathBuf`) drops first, then
    // `_dir` removes the directory. The leading `_` marks the field as held for
    // its `Drop` side effect, not read.
    path: PathBuf,
    _dir: TempDir,
}

impl ExeSnapshot {
    /// The stable path to spawn workers from.
    pub(in crate::test::runner) fn path(&self) -> &Path {
        &self.path
    }
}

/// Filename of the snapshot inside its private temp dir.
const SNAPSHOT_NAME: &str = "ori-test-worker";

/// Snapshot `src` to a private temp path via a hard link, falling back to a copy
/// when a hard link is unavailable (e.g. the temp dir is on a different
/// filesystem). The returned [`ExeSnapshot`] owns the temp; its path is stable
/// against a later replacement of `src`.
pub(in crate::test::runner) fn snapshot_exe(src: &Path) -> io::Result<ExeSnapshot> {
    let dir = TempDir::new()?;
    let dst = dir.path().join(SNAPSHOT_NAME);
    if std::fs::hard_link(src, &dst).is_ok() {
        Ok(ExeSnapshot {
            path: dst,
            _dir: dir,
        })
    } else {
        // Cross-filesystem (EXDEV) or any hard-link refusal: drop this dir and
        // take the copy path.
        drop(dir);
        snapshot_exe_copy(src)
    }
}

/// Snapshot `src` to a private temp path via a copy — the cross-filesystem
/// fallback path with the same [`ExeSnapshot`] guard contract as
/// [`snapshot_exe`].
pub(in crate::test::runner) fn snapshot_exe_copy(src: &Path) -> io::Result<ExeSnapshot> {
    let dir = TempDir::new()?;
    let dst = dir.path().join(SNAPSHOT_NAME);
    copy_executable(src, &dst)?;
    Ok(ExeSnapshot {
        path: dst,
        _dir: dir,
    })
}

/// Copy `src` to `dst` and restore the executable mode bit on unix (a plain copy
/// of a binary cannot be spawned without it).
fn copy_executable(src: &Path, dst: &Path) -> io::Result<()> {
    std::fs::copy(src, dst)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(dst)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(dst, perms)?;
    }
    Ok(())
}
