//! Stamps build identity into the binary via `cargo:rustc-env`, consumed by
//! `src/version.rs` for the dev `ori version` banner.
//!
//! Emits: `ORI_GIT_SHA` (short commit hash, or `unknown`), `ORI_GIT_DIRTY`
//! (`1` when the worktree had uncommitted changes, else `0`), `ORI_BUILD_DATE`
//! (HEAD commit date `YYYY-MM-DD`, or empty when git is unavailable).

use std::process::Command;

fn main() {
    let sha = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".to_string());

    let dirty = match git(&["status", "--porcelain"]) {
        Some(s) if !s.trim().is_empty() => "1",
        _ => "0",
    };

    let date = git(&[
        "show",
        "-s",
        "--format=%cd",
        "--date=format:%Y-%m-%d",
        "HEAD",
    ])
    .unwrap_or_default();

    println!("cargo:rustc-env=ORI_GIT_SHA={sha}");
    println!("cargo:rustc-env=ORI_GIT_DIRTY={dirty}");
    println!("cargo:rustc-env=ORI_BUILD_DATE={date}");

    // Refresh the stamp when HEAD moves or the index changes. `--git-path`
    // resolves the real locations (correct under worktrees / separate git dirs).
    for ref_name in ["HEAD", "index"] {
        if let Some(path) = git(&["rev-parse", "--git-path", ref_name]) {
            println!("cargo:rerun-if-changed={path}");
        }
    }
}

/// Runs `git <args>` and returns trimmed stdout, or `None` when git is absent
/// or exits non-zero.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}
