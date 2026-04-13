//! IR capture for Alive2 translation validation.
//!
//! Captures pre-opt and post-opt LLVM IR to files for alive-tv consumption.
//! Lives in oric (not `ori_llvm`) per phase purity: IO is oric-only.

use std::path::{Path, PathBuf};

use ori_llvm::aot::{CaptureHook, CaptureHooks};
use ori_llvm::inkwell::module::Module;

/// Whether any IR capture is requested via environment variables.
pub fn capture_requested() -> bool {
    crate::dbg_set!(crate::debug_flags::ORI_ALIVE2_CAPTURE)
        || crate::dbg_set!(crate::debug_flags::ORI_DUMP_PREOPT_LLVM)
        || crate::dbg_set!(crate::debug_flags::ORI_DUMP_POSTOPT_LLVM)
}

/// Whether pre-optimization IR capture is requested.
fn preopt_requested() -> bool {
    crate::dbg_set!(crate::debug_flags::ORI_ALIVE2_CAPTURE)
        || crate::dbg_set!(crate::debug_flags::ORI_DUMP_PREOPT_LLVM)
}

/// Whether post-optimization IR capture is requested.
fn postopt_requested() -> bool {
    crate::dbg_set!(crate::debug_flags::ORI_ALIVE2_CAPTURE)
        || crate::dbg_set!(crate::debug_flags::ORI_DUMP_POSTOPT_LLVM)
}

/// Compute output path for a captured IR file.
///
/// When `ORI_ALIVE2_CAPTURE` is set, files go into `build/alive2-results/`
/// with a sanitized name derived from the source path. Otherwise, files are
/// placed alongside the source file.
fn capture_path(source_path: &str, suffix: &str) -> PathBuf {
    if crate::dbg_set!(crate::debug_flags::ORI_ALIVE2_CAPTURE) {
        // Structured directory for Alive2 — encode repo-relative path
        // to guarantee uniqueness (multiple test files share basenames
        // like traits.ori, collections.ori across different directories).
        let dir = Path::new("build/alive2-results");
        std::fs::create_dir_all(dir).ok();
        // Sanitize: strip extension safely via Path, then replace / with _
        // Do NOT use .replace(".ori", "") — corrupts paths with ".ori" in dirs.
        let without_ext = Path::new(source_path).with_extension("");
        let sanitized = without_ext
            .to_str()
            .unwrap_or(source_path)
            .trim_start_matches("./")
            .replace('/', "_");
        dir.join(format!("{sanitized}{suffix}"))
    } else {
        // Alongside the source file
        Path::new(source_path).with_extension(suffix.trim_start_matches('.'))
    }
}

/// Build `CaptureHooks` for the given source file path.
///
/// Returns `CaptureHooks::default()` (no-op) when no capture flags are set.
pub fn build_hooks(source_path: &str) -> CaptureHooks<'_> {
    if !capture_requested() {
        return CaptureHooks::default();
    }

    let pre_opt: CaptureHook<'_> = if preopt_requested() {
        let path = capture_path(source_path, ".preopt.ll");
        Some(Box::new(move |m: &Module| {
            m.print_to_file(&path)
                .map_err(|e| format!("pre-opt IR capture: {e}"))
        }))
    } else {
        None
    };

    let post_opt: CaptureHook<'_> = if postopt_requested() {
        let path = capture_path(source_path, ".postopt.ll");
        Some(Box::new(move |m: &Module| {
            m.print_to_file(&path)
                .map_err(|e| format!("post-opt IR capture: {e}"))
        }))
    } else {
        None
    };

    CaptureHooks { pre_opt, post_opt }
}
