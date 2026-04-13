//! Clang-delegated sanitizer pass integration.
//!
//! The LLVM 21 C API (`llvm-sys = "211.0.1"`) does not expose sanitizer pass
//! configuration. We delegate to Clang as a compilation driver, which adds
//! sanitizer instrumentation transparently via `-fsanitize=address,undefined`.
//!
//! This is the same strategy used by Rust's `-Zsanitizer` flag (C++ shim to
//! `PassBuilder` internals) — we use Clang as the external driver instead.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use super::config::SanitizerMode;
use super::OptimizationError;

/// Candidates for the Clang binary, tried in order.
///
/// On many Linux distributions (Debian, Ubuntu), versioned packages like
/// `clang-21` don't create the unversioned `clang` symlink. We try the
/// generic name first, then versioned names from newest to oldest.
const CLANG_CANDIDATES: &[&str] = &[
    "clang", "clang-21", "clang-20", "clang-19", "clang-18", "clang-17",
];

/// Cached result of Clang binary detection.
static CLANG_BINARY: OnceLock<Option<String>> = OnceLock::new();

/// Find the Clang binary on PATH.
///
/// Tries candidates in order, caches the result for subsequent calls.
/// Returns `None` if no Clang is found.
pub fn find_clang() -> Option<&'static str> {
    CLANG_BINARY
        .get_or_init(|| {
            for candidate in CLANG_CANDIDATES {
                if Command::new(candidate)
                    .arg("--version")
                    .output()
                    .is_ok_and(|o| o.status.success())
                {
                    tracing::debug!(clang = candidate, "found clang binary");
                    return Some((*candidate).to_string());
                }
            }
            None
        })
        .as_deref()
}

/// Compile LLVM IR to an object file with sanitizer instrumentation via Clang.
///
/// Invokes `clang` with `-fsanitize=...` flags to add sanitizer passes that
/// the LLVM C API cannot configure directly.
///
/// # Arguments
/// * `ir_path` - Path to the LLVM IR file (`.ll`)
/// * `output_path` - Path for the output object file (`.o`)
/// * `sanitizer` - Which sanitizers to enable
/// * `opt_level` - Optimization level string (e.g., `"-O2"`)
/// * `target_triple` - LLVM target triple (e.g., `"x86_64-unknown-linux-gnu"`)
///
/// # Errors
/// Returns [`OptimizationError::PassesFailed`] if Clang is not found or
/// compilation fails.
pub fn clang_compile_with_sanitizers(
    ir_path: &Path,
    output_path: &Path,
    sanitizer: &SanitizerMode,
    opt_level: &str,
    target_triple: &str,
) -> Result<(), OptimizationError> {
    let clang = find_clang().ok_or_else(|| OptimizationError::PassesFailed {
        message: format!(
            "Clang not found on PATH. Tried: {}. Clang is required for sanitizer \
             instrumentation (ORI_SANITIZE). Install clang or disable sanitizers.",
            CLANG_CANDIDATES.join(", ")
        ),
    })?;

    let fsanitize_value = sanitizer
        .clang_flag_value()
        .expect("clang_compile_with_sanitizers called with no sanitizers enabled");

    let mut cmd = Command::new(clang);
    cmd.arg(ir_path)
        .arg("-c")
        .arg("-o")
        .arg(output_path)
        .arg(format!("-fsanitize={fsanitize_value}"))
        .arg(format!("--target={target_triple}"))
        .arg(opt_level);

    tracing::debug!(
        ?ir_path,
        ?output_path,
        clang,
        fsanitize = fsanitize_value,
        target = target_triple,
        "invoking clang for sanitizer instrumentation"
    );

    let output = cmd.output().map_err(|e| OptimizationError::PassesFailed {
        message: format!("failed to run {clang} for sanitizer instrumentation: {e}"),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(OptimizationError::PassesFailed {
            message: format!(
                "{clang} sanitizer compilation failed (exit {}): {}",
                output.status.code().unwrap_or(-1),
                stderr,
            ),
        });
    }

    Ok(())
}

/// Check that Clang is available on PATH.
///
/// Call this early when `ORI_SANITIZE` is set to fail fast with a clear error
/// before doing expensive compilation work. Tries versioned clang names
/// (e.g., `clang-21`) if the unversioned `clang` is not found.
///
/// # Errors
/// Returns [`OptimizationError::PassesFailed`] if no Clang can be found.
pub fn check_clang_available() -> Result<(), OptimizationError> {
    find_clang()
        .map(|_| ())
        .ok_or_else(|| OptimizationError::PassesFailed {
            message: format!(
                "Clang not found on PATH. Tried: {}. Clang is required for sanitizer \
             instrumentation (ORI_SANITIZE). Install clang or disable sanitizers.",
                CLANG_CANDIDATES.join(", ")
            ),
        })
}

#[cfg(test)]
mod tests;
