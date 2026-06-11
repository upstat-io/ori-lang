//! Binary and path discovery for AOT test helpers.
//!
//! Provides `ori_binary()`, `stdlib_path()`, and `ir_capture_binary()`
//! for locating the workspace `ori` compiler binary and standard library.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[cfg(test)]
mod tests;

/// Get the workspace root directory (contains `Cargo.toml` + `compiler/`).
pub(super) fn workspace_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .find(|p| p.join("Cargo.toml").exists() && p.join("compiler").exists())
        .map_or_else(|| PathBuf::from("/workspace"), Path::to_path_buf)
}

/// Get the path to the standard library (`library/` in workspace root).
pub fn stdlib_path() -> PathBuf {
    workspace_root().join("library")
}

/// Ensure the workspace `ori` binary for the given profile is freshly built.
///
/// Cached per profile via independent `OnceLock` cells: calling for debug
/// after release (or vice versa) within the same test process does NOT skip
/// the build. This matters because IR-capture tests always need the debug
/// binary (only debug enables `ORI_DEBUG_LLVM` phase dumps via `dbg_set!`)
/// regardless of the test process's own build profile.
fn ensure_ori_binary_fresh_for_profile(release: bool) {
    static DEBUG_RESULT: OnceLock<Result<(), String>> = OnceLock::new();
    static RELEASE_RESULT: OnceLock<Result<(), String>> = OnceLock::new();

    let cell = if release {
        &RELEASE_RESULT
    } else {
        &DEBUG_RESULT
    };

    let result = cell.get_or_init(|| {
        let profile_label = if release { "release" } else { "debug" };

        eprintln!("[aot-tests] ensuring `ori_rt` staticlib + `ori` binary ({profile_label})...");

        // Build a workspace package for this profile. `ori_rt`'s `libori_rt.a`
        // (the static lib every AOT binary links) is produced ONLY when ori_rt
        // is a DIRECT cargo target — a scoped `cargo test -p ori_llvm` builds
        // just its rlib, and a sibling `cargo clean` leaves the `.a` missing,
        // silently breaking every panic/catch AOT test (binaries link nothing
        // and abort on unwind). Build it explicitly — staticlib must be explicit.
        let build = |pkg: &str, bin: Option<&str>| -> Result<(), String> {
            let mut cmd = Command::new(env!("CARGO"));
            cmd.args(["build", "-p", pkg, "--quiet"]);
            if let Some(b) = bin {
                cmd.args(["--bin", b]);
            }
            if release {
                cmd.arg("--release");
            }
            cmd.current_dir(workspace_root());
            match cmd.status() {
                Ok(status) if status.success() => Ok(()),
                Ok(status) => Err(format!("`cargo build -p {pkg}` exited with {status}")),
                Err(e) => Err(format!("failed to spawn `cargo build -p {pkg}`: {e}")),
            }
        };

        build("ori_rt", None)?;
        build("oric", Some("ori"))
    });

    if let Err(msg) = result {
        panic!(
            "AOT test harness cannot proceed: {msg}\n\
             Try running `cargo b` (or `cargo b --release`) manually from the workspace root."
        );
    }
}

/// How a snapshot pins an artifact against concurrent replacement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SnapshotStrategy {
    /// Hard-link the source inode (instant; immune to write-temp-then-rename
    /// replacement, which retargets the directory entry, never the inode).
    /// Falls back to a full copy when the link fails (e.g. cross-device).
    HardLink,
    /// Full byte copy. Used as the `HardLink` fallback and pinned directly
    /// by the harness unit tests.
    Copy,
}

/// Per-artifact record of what a snapshot pinned.
#[derive(Clone, Debug)]
pub(super) struct StagedArtifact {
    /// File name of the artifact inside both `source_dir` and `stage_dir`.
    pub(super) name: String,
    /// Stage-time identity (`dev:inode:mtime:size`) of the pinned artifact.
    /// KEEP IN SYNC with `artifact_identity()` in `test-all.sh` — the AOT
    /// identity gate compares these values against its pre-run baseline.
    pub(super) source_identity: String,
    /// `"hardlink"` or `"copy"` — the strategy that actually took effect.
    pub(super) strategy_used: &'static str,
}

/// Identity tuple `dev:inode:mtime:size` for an on-disk artifact.
///
/// KEEP IN SYNC with `artifact_identity()` in `test-all.sh`: that gate
/// captures the same tuple via GNU `stat -c '%d:%i:%Y:%s'` as its pre-run
/// baseline and string-compares it against the values the harness writes
/// into the stage manifest.
#[cfg(unix)]
fn artifact_identity_of(path: &Path) -> Result<String, String> {
    use std::os::unix::fs::MetadataExt;
    let meta = fs::metadata(path)
        .map_err(|e| format!("cannot stat {} for the stage manifest: {e}", path.display()))?;
    Ok(format!(
        "{}:{}:{}:{}",
        meta.dev(),
        meta.ino(),
        meta.mtime(),
        meta.size()
    ))
}

/// Off-Unix fallback: no dev/inode surface; mtime+size still pin a
/// replacement. The `test-all.sh` gate never matches off-Unix artifact names
/// (exe suffix / lib naming differ), so it falls back to its live compare.
#[cfg(not(unix))]
fn artifact_identity_of(path: &Path) -> Result<String, String> {
    let meta = fs::metadata(path)
        .map_err(|e| format!("cannot stat {} for the stage manifest: {e}", path.display()))?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_secs());
    Ok(format!("0:0:{mtime}:{}", meta.len()))
}

/// Snapshot `required` + any present `optional` artifacts from `source_dir`
/// into `stage_dir`, returning one [`StagedArtifact`] per staged file.
/// Idempotent: an existing staged file is removed and re-snapshotted from
/// the CURRENT source. A missing required source is an error — never a
/// silent fallback to the mutable shared path.
pub(super) fn stage_snapshot(
    source_dir: &Path,
    stage_dir: &Path,
    required: &[&str],
    optional: &[&str],
    strategy: SnapshotStrategy,
) -> Result<Vec<StagedArtifact>, String> {
    fs::create_dir_all(stage_dir)
        .map_err(|e| format!("cannot create staging dir {}: {e}", stage_dir.display()))?;

    let snapshot_one = |name: &str| -> Result<StagedArtifact, String> {
        let src = source_dir.join(name);
        let dst = stage_dir.join(name);
        let _ = fs::remove_file(&dst);
        let copy_into_stage = || {
            fs::copy(&src, &dst).map(|_| ()).map_err(|e| {
                format!(
                    "cannot snapshot {} into {}: {e}",
                    src.display(),
                    stage_dir.display()
                )
            })
        };
        let strategy_used = match strategy {
            SnapshotStrategy::HardLink => {
                if fs::hard_link(&src, &dst).is_ok() {
                    "hardlink"
                } else {
                    copy_into_stage()?;
                    "copy"
                }
            }
            SnapshotStrategy::Copy => {
                copy_into_stage()?;
                "copy"
            }
        };
        // Identity of the artifact actually pinned: a hardlink shares the
        // source inode, so the STAGED path carries the stage-time source
        // identity even if the source is swapped between link and stat; a
        // copy has its own inode, so the source path is statted instead.
        let identity_path: &Path = if strategy_used == "hardlink" {
            &dst
        } else {
            &src
        };
        Ok(StagedArtifact {
            name: name.to_string(),
            source_identity: artifact_identity_of(identity_path)?,
            strategy_used,
        })
    };

    let mut staged = Vec::new();
    for name in required {
        if !source_dir.join(name).exists() {
            return Err(format!(
                "required artifact {} missing from {} — cannot stage a snapshot \
                 (a missing artifact here would otherwise produce mass bogus \
                 failures under concurrent builds)",
                name,
                source_dir.display()
            ));
        }
        staged.push(snapshot_one(name)?);
    }
    for name in optional {
        if source_dir.join(name).exists() {
            staged.push(snapshot_one(name)?);
        } else {
            // Source gone: drop any stale staged copy so the snapshot mirrors
            // the current source set.
            let _ = fs::remove_file(stage_dir.join(name));
        }
    }
    Ok(staged)
}

/// File name of the stage manifest inside each per-PID stage dir.
const STAGE_MANIFEST_NAME: &str = "stage-manifest.txt";

/// Render the stage manifest the `test-all.sh` AOT identity gate consumes.
///
/// One record per line, space-separated:
///
/// ```text
/// schema 1
/// pid <u32>
/// profile <debug|release>
/// stage-dir <path>
/// source-dir <path>
/// artifact <name> <hardlink|copy> <dev:inode:mtime:size>
/// ```
///
/// The gate extracts whitespace field 4 of each `artifact` line and compares
/// it against its own pre-run `artifact_identity()` baseline.
fn render_stage_manifest(
    profile: &str,
    stage_dir: &Path,
    source_dir: &Path,
    artifacts: &[StagedArtifact],
) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    // Writing into a String is infallible; the results are discardable.
    let _ = writeln!(out, "schema 1");
    let _ = writeln!(out, "pid {}", std::process::id());
    let _ = writeln!(out, "profile {profile}");
    let _ = writeln!(out, "stage-dir {}", stage_dir.display());
    let _ = writeln!(out, "source-dir {}", source_dir.display());
    for artifact in artifacts {
        let _ = writeln!(
            out,
            "artifact {} {} {}",
            artifact.name, artifact.strategy_used, artifact.source_identity
        );
    }
    out
}

/// Atomically write `content` to `dest` via a pid-suffixed temp sibling +
/// rename(2): readers never observe a partial manifest, and concurrent
/// harness processes cannot clobber each other's in-flight temp file.
fn write_manifest_atomic(dest: &Path, content: &str) -> Result<(), String> {
    let parent = dest
        .parent()
        .ok_or_else(|| format!("manifest path {} has no parent dir", dest.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("cannot create {}: {e}", parent.display()))?;
    let file_name = dest
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| format!("manifest path {} has no file name", dest.display()))?;
    let tmp = parent.join(format!("{file_name}.{}.tmp", std::process::id()));
    fs::write(&tmp, content).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    fs::rename(&tmp, dest).map_err(|e| {
        format!(
            "cannot rename {} over {}: {e}",
            tmp.display(),
            dest.display()
        )
    })
}

/// Write the stage manifest to both homes: inside the stage dir (travels and
/// dies with the snapshot; covered by the dead-PID cleaner) and at the
/// well-known per-profile pointer path (the discovery channel for
/// out-of-process consumers — per-PID temp stage dirs are not discoverable
/// from the shell). Pointer files are per-profile and overwrite-only: no
/// cleanup pass needed.
fn publish_stage_manifest(
    stage_dir: &Path,
    pointer_path: &Path,
    content: &str,
) -> Result<(), String> {
    write_manifest_atomic(&stage_dir.join(STAGE_MANIFEST_NAME), content)?;
    write_manifest_atomic(pointer_path, content)
}

/// Best-effort removal of `ori-aot-stage-<pid>-*` dirs left by prior test
/// processes whose PID is no longer alive.
pub(super) fn clean_stale_stage_dirs() {
    let Ok(entries) = fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    let own_pid = std::process::id();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(rest) = name.strip_prefix("ori-aot-stage-") else {
            continue;
        };
        let Some(pid_str) = rest.split('-').next() else {
            continue;
        };
        let Ok(pid) = pid_str.parse::<u32>() else {
            continue;
        };
        if pid == own_pid || pid_is_alive(pid) {
            continue;
        }
        let _ = fs::remove_dir_all(entry.path());
    }
}

#[cfg(target_os = "linux")]
fn pid_is_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

// Non-Linux Unix has no /proc; `kill -0` probes liveness without signaling.
// A spawn per candidate dir is fine on this cold cleanup path.
#[cfg(all(unix, not(target_os = "linux")))]
fn pid_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .status()
        .map(|s| s.success())
        .unwrap_or(true)
}

#[cfg(not(unix))]
fn pid_is_alive(_pid: u32) -> bool {
    // Conservative: never reclaim without a cheap liveness probe — stale dirs
    // accumulate in the OS temp dir (reclaimed by OS temp cleanup) rather
    // than risking deletion of a live process's snapshot.
    true
}

/// Snapshot the freshly-built `ori` binary + runtime staticlib into a
/// per-test-process staging directory and return that directory.
///
/// A concurrent `cargo build` in the shared `target/` replaces those artifacts
/// via write-temp-then-rename; a suite resolving them from `target/<profile>/`
/// mid-run sees the swap and aborts en masse (hundreds of bogus failures that
/// look like real test results). The snapshot pins the ORIGINAL inodes, so the
/// whole run executes against one immutable view. The staticlib is staged NEXT
/// TO the binary because the AOT runtime discovery searches the compiler
/// binary's own directory first.
fn staged_artifacts_dir(release: bool) -> &'static Path {
    static DEBUG_STAGE: OnceLock<PathBuf> = OnceLock::new();
    static RELEASE_STAGE: OnceLock<PathBuf> = OnceLock::new();

    let cell = if release {
        &RELEASE_STAGE
    } else {
        &DEBUG_STAGE
    };
    cell.get_or_init(|| {
        clean_stale_stage_dirs();

        let profile = if release { "release" } else { "debug" };
        let stage =
            std::env::temp_dir().join(format!("ori-aot-stage-{}-{profile}", std::process::id()));
        let profile_dir = workspace_root().join("target").join(profile);
        let exe = format!("ori{}", std::env::consts::EXE_SUFFIX);
        let (lib, asan_lib) = if cfg!(windows) {
            ("ori_rt.lib", "ori_rt_asan.lib")
        } else {
            ("libori_rt.a", "libori_rt_asan.a")
        };

        let staged_artifacts = match stage_snapshot(
            &profile_dir,
            &stage,
            &[&exe, lib],
            &[asan_lib],
            SnapshotStrategy::HardLink,
        ) {
            Ok(artifacts) => artifacts,
            Err(e) => panic!("AOT harness: {e}"),
        };

        // Stage manifest: records the stage-time source identities so the
        // test-all.sh AOT identity gate can validate the SNAPSHOT against its
        // pre-run baseline instead of the live (concurrently mutable)
        // target/<profile>/ paths. A write failure is loud but non-fatal:
        // without a manifest the gate falls back to its conservative live
        // compare.
        let manifest = render_stage_manifest(profile, &stage, &profile_dir, &staged_artifacts);
        let pointer = workspace_root()
            .join("build")
            .join(format!("aot-stage-manifest-{profile}.txt"));
        if let Err(e) = publish_stage_manifest(&stage, &pointer, &manifest) {
            eprintln!(
                "[aot-tests] warning: cannot publish stage manifest ({e}); the \
                 test-all.sh identity gate falls back to live artifact comparison"
            );
        }

        eprintln!(
            "[aot-tests] staged `{exe}` + `{lib}` ({profile}) at {} (immutable snapshot \
             for this test process)",
            stage.display()
        );
        stage
    })
}

/// Ensure the workspace `ori` binary matching the current test profile is
/// freshly built, invoking `cargo build -p oric --bin ori [--release]` exactly
/// once per test process.
///
/// **Why this exists**: `cargo test -p ori_llvm` compiles and runs `ori_llvm`
/// tests but does NOT rebuild the workspace `ori` binary at `target/debug/ori`
/// (or `target/release/ori`). The AOT test harness shells out to that binary
/// to compile Ori fixtures. A session that edits `ori_arc`, `ori_rt`, or any
/// `oric` dependency and runs `cargo test -p ori_llvm` directly sees **ghost
/// test results**: the test process loads fresh `ori_llvm.rlib` via Cargo's
/// dep graph, but spawns the stale `ori` binary to compile fixtures.
///
/// **What it does**: Delegates to `ensure_ori_binary_fresh_for_profile()` with
/// the profile matching the test binary (`cfg!(debug_assertions)` → debug,
/// else release). The IR-capture path uses the profile-parameterized helper
/// directly to always refresh debug, since IR-quality tests need debug
/// regardless of how the test binary was built.
fn ensure_ori_binary_fresh() {
    ensure_ori_binary_fresh_for_profile(!cfg!(debug_assertions));
}

/// Get the path to an LLVM-enabled `ori` binary.
///
/// Picks the binary matching the current build profile (`cargo test` = debug,
/// `cargo test --release` = release). Falls back to the other profile only if
/// the matching one has no LLVM support. Panics if neither has LLVM support.
///
/// **Freshness guarantee**: The first call per test process invokes
/// `ensure_ori_binary_fresh()` which runs `cargo build -p oric --bin ori`
/// (profile-matched) via `OnceLock`. Subsequent calls skip the build.
pub fn ori_binary() -> PathBuf {
    ensure_ori_binary_fresh();

    let release = !cfg!(debug_assertions);
    let exe = format!("ori{}", std::env::consts::EXE_SUFFIX);

    // Resolve from the per-process immutable snapshot — never the shared
    // mutable `target/<profile>/` path a concurrent build can swap mid-run.
    let staged = staged_artifacts_dir(release).join(&exe);
    if staged.exists() && has_llvm_support(&staged) {
        return staged;
    }

    // The profile-matched binary lacks LLVM support: build + stage the other
    // profile and use its snapshot instead.
    let profile = if release { "release" } else { "debug" };
    eprintln!("warning: {profile} ori binary has no LLVM support, falling back to other profile");
    ensure_ori_binary_fresh_for_profile(!release);
    let fallback = staged_artifacts_dir(!release).join(&exe);
    if fallback.exists() && has_llvm_support(&fallback) {
        return fallback;
    }

    panic!(
        "No LLVM-enabled ori binary found.\n\
         AOT tests require `ori` built with LLVM (enabled by default).\n\
         Run `cargo build` (debug) or `cargo build --release` (release) first,\n\
         or use `./llvm-test.sh` which builds automatically.\n\
         \n\
         Checked staged snapshots:\n  \
           preferred: {} (exists: {})\n  \
           fallback:  {} (exists: {})",
        staged.display(),
        staged.exists(),
        fallback.display(),
        fallback.exists(),
    );
}

/// Check whether an `ori` binary has LLVM/AOT support by running
/// `ori build --help` and checking for the E5004 "requires LLVM backend" error.
fn has_llvm_support(binary: &Path) -> bool {
    Command::new(binary)
        .args(["build", "--help"])
        .output()
        .map(|o| {
            let stderr = String::from_utf8_lossy(&o.stderr);
            !stderr.contains("E5004")
        })
        .unwrap_or(false)
}

/// Return the debug `ori` binary for IR capture.
///
/// IR dump (`ORI_DEBUG_LLVM=1`) is only available in debug builds — the release
/// binary compiles out phase dumps via `dbg_set!`. IR-quality tests must use the
/// debug binary to capture IR, regardless of the test harness build profile.
///
/// **Freshness guarantee**: The first call per test process invokes
/// `ensure_ori_binary_fresh_for_profile(false)` which runs
/// `cargo build -p oric --bin ori` (always debug, independent of the test
/// process profile) via a debug-specific `OnceLock`. Subsequent calls skip
/// the build. Without this, `cargo test --release ... aot::generics::*`
/// shells to a stale `target/debug/ori` and produces misleading errors —
/// e.g. "unresolved function `identity` — missing mono instance?" — when
/// the working tree is already fixed.
///
/// Panics if no debug binary exists after refresh. This is intentional:
/// IR-quality tests are semantic pins that must never silently degrade to
/// no-ops.
pub fn ir_capture_binary() -> PathBuf {
    ensure_ori_binary_fresh_for_profile(false);

    let exe = format!("ori{}", std::env::consts::EXE_SUFFIX);
    let staged = staged_artifacts_dir(false).join(&exe);

    if staged.exists() && has_llvm_support(&staged) {
        return staged;
    }
    panic!(
        "No staged debug ori binary found for IR capture after refresh.\n\
         Run `cargo build -p oric --bin ori` manually from the workspace root \
         to investigate."
    );
}
