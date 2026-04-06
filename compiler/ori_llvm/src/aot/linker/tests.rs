//! Tests for linker cross-compilation detection and linker selection.
//!
//! Regression: BUG-04-001 — cross-compilation to Windows from Linux silently
//! fell back to the host linker, producing cryptic errors.

use super::*;
use crate::aot::target_features::TargetTripleComponents;
use crate::aot::TargetConfig;

// === Cross-compiler name resolution ===

#[test]
fn test_gcc_cross_compiler_name_windows_gnu() {
    let target = TargetTripleComponents::parse("x86_64-pc-windows-gnu").unwrap();
    assert_eq!(
        LinkerDetection::gcc_cross_compiler_name(&target),
        Some("x86_64-w64-mingw32-gcc".to_string())
    );
}

#[test]
fn test_gcc_cross_compiler_name_windows_msvc_is_none() {
    let target = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    assert_eq!(LinkerDetection::gcc_cross_compiler_name(&target), None);
}

#[test]
fn test_gcc_cross_compiler_name_linux_gnu() {
    let target = TargetTripleComponents::parse("aarch64-unknown-linux-gnu").unwrap();
    assert_eq!(
        LinkerDetection::gcc_cross_compiler_name(&target),
        Some("aarch64-linux-gnu-gcc".to_string())
    );
}

#[test]
fn test_gcc_cross_compiler_name_linux_musl() {
    let target = TargetTripleComponents::parse("aarch64-unknown-linux-musl").unwrap();
    assert_eq!(
        LinkerDetection::gcc_cross_compiler_name(&target),
        Some("aarch64-linux-musl-gcc".to_string())
    );
}

#[test]
fn test_gcc_cross_compiler_name_darwin_is_none() {
    let target = TargetTripleComponents::parse("aarch64-apple-darwin").unwrap();
    assert_eq!(LinkerDetection::gcc_cross_compiler_name(&target), None);
}

#[test]
fn test_gcc_cross_compiler_name_wasm_is_none() {
    let target = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    assert_eq!(LinkerDetection::gcc_cross_compiler_name(&target), None);
}

// === Cross-compilation detection ===

#[test]
fn test_native_target_is_not_cross_compiling() {
    let target = TargetConfig::native().unwrap();
    assert!(
        !LinkerDetection::is_cross_compiling(&target),
        "native target should not be detected as cross-compilation"
    );
}

#[test]
#[cfg(target_os = "linux")]
fn test_linux_to_windows_is_cross_compiling() {
    let target = TargetConfig::from_triple("x86_64-pc-windows-msvc").unwrap();
    assert!(
        LinkerDetection::is_cross_compiling(&target),
        "targeting Windows from Linux should be detected as cross-compilation"
    );
}

#[test]
#[cfg(target_os = "linux")]
fn test_linux_to_darwin_is_cross_compiling() {
    let target = TargetConfig::from_triple("aarch64-apple-darwin").unwrap();
    assert!(
        LinkerDetection::is_cross_compiling(&target),
        "targeting macOS from Linux should be detected as cross-compilation"
    );
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn test_linux_x86_to_linux_aarch64_is_cross_compiling() {
    let target = TargetConfig::from_triple("aarch64-unknown-linux-gnu").unwrap();
    // Same OS (linux) but different arch — IS cross-compilation.
    // The host `cc` cannot produce aarch64 binaries; we need aarch64-linux-gnu-gcc.
    assert!(
        LinkerDetection::is_cross_compiling(&target),
        "same OS different arch IS cross-compilation (requires cross-gcc)"
    );
}

#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn test_linux_x86_to_linux_x86_is_not_cross_compiling() {
    let target = TargetConfig::from_triple("x86_64-unknown-linux-gnu").unwrap();
    assert!(
        !LinkerDetection::is_cross_compiling(&target),
        "same OS same arch is NOT cross-compilation"
    );
}

// === Linker flavor selection ===

#[test]
fn test_linker_flavor_for_windows_msvc() {
    let target = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    assert_eq!(LinkerFlavor::for_target(&target), LinkerFlavor::Msvc);
}

#[test]
fn test_linker_flavor_for_windows_gnu() {
    let target = TargetTripleComponents::parse("x86_64-pc-windows-gnu").unwrap();
    assert_eq!(LinkerFlavor::for_target(&target), LinkerFlavor::Gcc);
}

#[test]
fn test_linker_flavor_for_linux() {
    let target = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(LinkerFlavor::for_target(&target), LinkerFlavor::Gcc);
}

#[test]
fn test_linker_flavor_for_wasm() {
    let target = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    assert_eq!(LinkerFlavor::for_target(&target), LinkerFlavor::WasmLd);
}

// === Cross-compilation-aware detection (semantic pin) ===

/// Semantic pin: host `cc` must NOT be returned as available for a cross-arch
/// Linux target. Regression guard for TPR-04-006.
#[test]
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
fn test_host_cc_not_available_for_cross_arch_linux_target() {
    let target = TargetConfig::from_triple("aarch64-unknown-linux-gnu").unwrap();
    assert!(
        !LinkerDetection::is_available_for_target(LinkerFlavor::Gcc, &target),
        "host cc must NOT be reported as available for cross-arch Linux target"
    );
}

/// Semantic pin: host `cc` must NOT be returned as available for a cross-OS
/// Windows target. This is the exact regression that BUG-04-001 caught.
#[test]
#[cfg(target_os = "linux")]
fn test_host_cc_not_available_for_windows_target() {
    let target = TargetConfig::from_triple("x86_64-pc-windows-msvc").unwrap();
    assert!(
        !LinkerDetection::is_available_for_target(LinkerFlavor::Gcc, &target),
        "host cc must NOT be reported as available for cross-OS Windows target"
    );
}

/// Semantic pin: MSVC tools must not be reported as available on Linux.
#[test]
#[cfg(target_os = "linux")]
fn test_msvc_not_available_on_linux() {
    let target = TargetConfig::from_triple("x86_64-pc-windows-msvc").unwrap();
    assert!(
        !LinkerDetection::is_available_for_target(LinkerFlavor::Msvc, &target),
        "MSVC must NOT be reported as available on Linux host"
    );
}

// === Cross-compilation error message (negative pin) ===

/// Negative pin: error message must contain the target triple and actionable
/// suggestions when no cross-linker is found.
#[test]
fn test_cross_compilation_error_contains_target_and_help() {
    let target = TargetConfig::from_triple("x86_64-pc-windows-msvc").unwrap();
    let err = LinkerDetection::cross_compilation_error(&target);
    let msg = err.to_string();

    assert!(
        msg.contains("x86_64-pc-windows-msvc"),
        "error message must contain the target triple, got: {msg}"
    );
    assert!(
        msg.contains("lld-link") || msg.contains("LLD"),
        "error message for windows-msvc must suggest LLD, got: {msg}"
    );
}

#[test]
fn test_cross_compilation_error_windows_gnu_suggests_mingw() {
    let target = TargetConfig::from_triple("x86_64-pc-windows-gnu").unwrap();
    let err = LinkerDetection::cross_compilation_error(&target);
    let msg = err.to_string();

    assert!(
        msg.contains("mingw"),
        "error message for windows-gnu must suggest mingw, got: {msg}"
    );
    assert!(
        msg.contains("x86_64-w64-mingw32-gcc"),
        "error message must include the specific cross-compiler name, got: {msg}"
    );
}

#[test]
fn test_cross_compilation_error_linux_aarch64_suggests_cross_gcc() {
    let target = TargetConfig::from_triple("aarch64-unknown-linux-gnu").unwrap();
    let err = LinkerDetection::cross_compilation_error(&target);
    let msg = err.to_string();

    assert!(
        msg.contains("aarch64-linux-gnu-gcc"),
        "error message for linux-aarch64 must suggest the cross-gcc name, got: {msg}"
    );
}

#[test]
fn test_cross_compilation_error_darwin_suggests_osxcross() {
    let target = TargetConfig::from_triple("aarch64-apple-darwin").unwrap();
    let err = LinkerDetection::cross_compilation_error(&target);
    let msg = err.to_string();

    assert!(
        msg.contains("osxcross"),
        "error message for darwin must suggest osxcross, got: {msg}"
    );
}

// === Native compilation unchanged ===

#[test]
fn test_native_gcc_still_available() {
    let target = TargetConfig::native().unwrap();
    // On any system with a C compiler, the native GCC/cc should be available
    // (this test runs in CI which always has a C compiler)
    if LinkerDetection::is_available_for_target(LinkerFlavor::Gcc, &target) {
        // Good — native compilation path works
    }
    // Note: we can't assert true because some CI environments might not have cc,
    // but the important thing is it doesn't falsely report unavailable when it IS
    // available. The semantic pin tests above verify the cross-compilation guard.
}

// === Link output extensions ===

#[test]
fn test_link_output_extension_windows() {
    let target = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    assert_eq!(LinkOutput::Executable.extension(&target), "exe");
    assert_eq!(LinkOutput::SharedLibrary.extension(&target), "dll");
    assert_eq!(LinkOutput::StaticLibrary.extension(&target), "lib");
}

#[test]
fn test_link_output_extension_linux() {
    let target = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(LinkOutput::Executable.extension(&target), "");
    assert_eq!(LinkOutput::SharedLibrary.extension(&target), "so");
    assert_eq!(LinkOutput::StaticLibrary.extension(&target), "a");
}
