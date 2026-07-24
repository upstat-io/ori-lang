//! `OnceLock<cargo build>` AOT-binary discovery for the differential target.
//!
//! Lifted verbatim (with workspace-root computation adjusted for the fuzz
//! crate's `CARGO_MANIFEST_DIR`) from
//! `compiler_repo/compiler/ori_llvm/tests/aot/util/binary.rs:31-95`. The
//! fuzz crate cannot depend on `ori_llvm`'s test harness (constraint #1 of
//! §10), so the helper is duplicated here.
//!
//! LEAK:algorithmic-duplication — see §10.C "AOT-binary-discovery extraction"
//! cleanup item. The correct cure is extracting the helper to a shared
//! dev-dep crate consumed by both the AOT test harness and this fuzz crate;
//! deferred until the fuzz crate stabilizes so the shared API can be
//! designed against two real consumers.
//!
//! **Why this exists** (mirrors the original source comment): `cargo +nightly
//! fuzz run` compiles and runs the fuzz target but does NOT rebuild the
//! workspace `ori` binary at `target/{debug,release}/ori`. A session that
//! edits `ori_arc`, `ori_rt`, or any `oric` dependency and runs the fuzzer
//! directly sees **ghost results**: the fuzz process loads fresh fuzz-target
//! code, but spawns a stale `ori` binary to compile generated programs.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

/// Get the workspace root directory (contains `Cargo.toml` + `compiler/`).
///
/// `CARGO_MANIFEST_DIR` for this crate is `compiler_repo/fuzz/`. The workspace
/// root is one level up (`compiler_repo/`), recognised by carrying both
/// `Cargo.toml` (the workspace manifest) and `compiler/` (the workspace
/// members directory).
pub fn workspace_root() -> PathBuf {
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
/// Cached per profile via independent `OnceLock` cells so a fuzz campaign that
/// switches between debug and release subprocess invocations rebuilds each
/// once.
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
            "[ori-fuzz] rebuilding workspace `ori` binary ({profile_label}) via `cargo build -p oric --bin ori{}`...",
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
            "ori-fuzz AOT subprocess setup cannot proceed: {msg}\n\
             Try running `cargo b` (or `cargo b --release`) manually from the workspace root."
        );
    }
}

/// Ensure the workspace `ori` binary matching the current build profile is
/// freshly built, invoking `cargo build -p oric --bin ori [--release]` exactly
/// once per fuzz process.
fn ensure_ori_binary_fresh() {
    ensure_ori_binary_fresh_for_profile(!cfg!(debug_assertions));
}

/// Get the path to the workspace `ori` binary, rebuilding it once per fuzz
/// process.
///
/// Picks the binary matching the current build profile. The fuzz target
/// passes generated `.ori` source files to this binary as a subprocess (per
/// constraint #1 of §10).
pub fn ori_binary() -> PathBuf {
    ensure_ori_binary_fresh();

    let target_dir = workspace_root().join("target");
    let profile_subdir = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };

    target_dir.join(profile_subdir).join("ori")
}
