//! Binary and path discovery for AOT test helpers.
//!
//! Provides `ori_binary()`, `stdlib_path()`, and `ir_capture_binary()`
//! for locating the workspace `ori` compiler binary and standard library.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

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

        eprintln!(
            "[aot-tests] rebuilding workspace `ori` binary ({profile_label}) via `cargo build -p oric --bin ori{}`...",
            if release { " --release" } else { "" }
        );

        let mut cmd = Command::new(env!("CARGO"));
        cmd.args(["build", "-p", "oric", "--bin", "ori", "--quiet"]);
        if release {
            cmd.arg("--release");
        }
        cmd.current_dir(workspace_root());

        match cmd.status() {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(format!(
                "`cargo build -p oric --bin ori{}` exited with {status}",
                if release { " --release" } else { "" }
            )),
            Err(e) => Err(format!(
                "failed to spawn `cargo build -p oric --bin ori`: {e}"
            )),
        }
    });

    if let Err(msg) = result {
        panic!(
            "AOT test harness cannot proceed: {msg}\n\
             Try running `cargo b` (or `cargo b --release`) manually from the workspace root."
        );
    }
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

    let workspace_root = workspace_root();

    let exe = format!("ori{}", std::env::consts::EXE_SUFFIX);
    let debug_path = workspace_root.join("target/debug").join(&exe);
    let release_path = workspace_root.join("target/release").join(&exe);

    let debug_llvm = debug_path.exists() && has_llvm_support(&debug_path);
    let release_llvm = release_path.exists() && has_llvm_support(&release_path);

    // Pick the binary that matches the current build profile.
    let (preferred, fallback) = if cfg!(debug_assertions) {
        ((debug_llvm, &debug_path), (release_llvm, &release_path))
    } else {
        ((release_llvm, &release_path), (debug_llvm, &debug_path))
    };

    if preferred.0 {
        return preferred.1.clone();
    }
    if fallback.0 {
        let profile = if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        };
        eprintln!(
            "warning: {profile} ori binary has no LLVM support, falling back to other profile"
        );
        return fallback.1.clone();
    }

    panic!(
        "No LLVM-enabled ori binary found.\n\
         AOT tests require `ori` built with LLVM (enabled by default).\n\
         Run `cargo build` (debug) or `cargo build --release` (release) first,\n\
         or use `./llvm-test.sh` which builds automatically.\n\
         \n\
         Checked:\n  \
           debug:   {} (exists: {})\n  \
           release: {} (exists: {})",
        debug_path.display(),
        debug_path.exists(),
        release_path.display(),
        release_path.exists(),
    );
}

/// Check whether an `ori` binary has LLVM/AOT support by running `ori build`
/// with no arguments and checking for the E5004 "requires LLVM backend" error.
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

    let workspace_root = workspace_root();
    let exe = format!("ori{}", std::env::consts::EXE_SUFFIX);
    let debug_path = workspace_root.join("target/debug").join(&exe);

    if debug_path.exists() && has_llvm_support(&debug_path) {
        return debug_path;
    }
    panic!(
        "No debug ori binary found for IR capture after refresh.\n\
         Run `cargo build -p oric --bin ori` manually from the workspace root \
         to investigate."
    );
}
