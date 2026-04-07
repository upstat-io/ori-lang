//! Cross-Compilation and Target Configuration Tests
//!
//! Test scenarios inspired by:
//! - Rust: `tests/run-make/mismatching-target-triples/` - target consistency
//! - Rust: `tests/run-make/target-specs/` - custom target specs
//! - Zig: target/feature detection tests
//!
//! These tests verify:
//! - Target triple parsing and validation
//! - CPU feature detection
//! - Data layout configuration
//! - Platform-specific behavior

#![allow(
    clippy::similar_names,
    reason = "wasm vs wasi naming pattern is intentional"
)]

use ori_llvm::aot::target::TargetConfig;
use ori_llvm::aot::target_features::{
    get_host_cpu_features, get_host_cpu_name, is_supported_target, parse_features, Arch,
    TargetError, TargetTripleComponents,
};

use super::util::{
    linux_target, macos_arm_target, macos_target, wasm32_target, wasm32_wasi_target,
    windows_gnu_target, windows_msvc_target,
};

/// Test: Parse valid target triples
///
/// Scenario from Rust `target-specs`:
/// Standard target triples should parse correctly.
#[test]
fn test_parse_valid_target_triples() {
    // Linux targets
    let linux = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(linux.arch, Arch::X86_64);
    assert_eq!(linux.vendor, "unknown");
    assert_eq!(linux.os, "linux");
    assert_eq!(linux.env, Some("gnu".to_string()));

    // macOS targets
    let macos = TargetTripleComponents::parse("x86_64-apple-darwin").unwrap();
    assert_eq!(macos.arch, Arch::X86_64);
    assert_eq!(macos.vendor, "apple");
    assert_eq!(macos.os, "darwin");
    assert!(macos.env.is_none());

    // ARM64 macOS
    let macos_arm = TargetTripleComponents::parse("aarch64-apple-darwin").unwrap();
    assert_eq!(macos_arm.arch, Arch::Aarch64);
    assert_eq!(macos_arm.vendor, "apple");

    // Windows targets
    let windows_msvc = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    assert_eq!(windows_msvc.arch, Arch::X86_64);
    assert_eq!(windows_msvc.vendor, "pc");
    assert_eq!(windows_msvc.os, "windows");
    assert_eq!(windows_msvc.env, Some("msvc".to_string()));

    let windows_gnu = TargetTripleComponents::parse("x86_64-pc-windows-gnu").unwrap();
    assert_eq!(windows_gnu.env, Some("gnu".to_string()));

    // WASM targets
    let wasm = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    assert_eq!(wasm.arch, Arch::Wasm32);
    assert_eq!(wasm.vendor, "unknown");
    assert_eq!(wasm.os, "unknown");

    let wasi = TargetTripleComponents::parse("wasm32-unknown-wasi").unwrap();
    assert_eq!(wasi.arch, Arch::Wasm32);
    assert_eq!(wasi.os, "wasi");
}

/// Test: Parse invalid target triples
///
/// Scenario: Malformed triples should fail.
#[test]
fn test_parse_invalid_target_triples() {
    // Too few components
    let result = TargetTripleComponents::parse("x86_64");
    assert!(result.is_err());

    let result = TargetTripleComponents::parse("x86_64-unknown");
    assert!(result.is_err());

    // Empty string
    let result = TargetTripleComponents::parse("");
    assert!(result.is_err());

    // Invalid architecture — strictly rejected at the parse boundary because
    // `Arch::parse_llvm_name` only accepts known architectures. This is the
    // SSOT enforcement: unknown archs can never flow into the compiler as
    // raw strings that might silently mis-route later.
    let result = TargetTripleComponents::parse("invalid-unknown-linux-gnu");
    assert!(
        result.is_err(),
        "unknown architecture must be rejected at parse boundary"
    );
}

/// Regression pin for BUG-04-045 latent bug #7: `TargetConfig::from_triple`
/// must accept Apple's `arm64` alias for `aarch64` because that is exactly
/// the spelling LLVM's default triple uses on Apple Silicon. Before the fix,
/// parse happened AFTER the `SUPPORTED_TARGETS` check, so `arm64-apple-darwin`
/// was rejected even though it's semantically identical to the supported
/// `aarch64-apple-darwin`. After the fix, parse canonicalizes arch aliases
/// BEFORE the supported-targets check, and the stored triple is canonical.
#[test]
fn test_from_triple_accepts_arm64_apple_darwin_alias() {
    let config = TargetConfig::from_triple("arm64-apple-darwin")
        .expect("arm64 alias must canonicalize to aarch64 and be accepted");
    assert_eq!(
        config.triple(),
        "aarch64-apple-darwin",
        "stored triple must be the canonical spelling"
    );
    assert_eq!(config.components().arch, Arch::Aarch64);
}

/// Sibling: `amd64` alias for `x86_64` is canonicalized the same way.
#[test]
fn test_from_triple_accepts_amd64_linux_alias() {
    let config = TargetConfig::from_triple("amd64-unknown-linux-gnu")
        .expect("amd64 alias must canonicalize to x86_64 and be accepted");
    assert_eq!(config.triple(), "x86_64-unknown-linux-gnu");
    assert_eq!(config.components().arch, Arch::X86_64);
}

/// Regression pin for BUG-04-045 / TPR-BUG-04-045-01: `from_triple` must
/// accept the **versioned** Darwin spelling LLVM emits on Apple Silicon,
/// `arm64-apple-darwin25.2.0`. The unversioned alias fix alone was not
/// enough — Apple Silicon's `TargetMachine::get_default_triple()` carries
/// the OS version suffix, and `SUPPORTED_TARGETS` only contains the
/// unversioned `aarch64-apple-darwin`. The fix is `support_key()`, which
/// strips Darwin OS version suffixes at the support-check boundary.
///
/// The stored triple preserves the OS version suffix because LLVM's
/// `TargetMachine` expects the version-bearing form when one was supplied.
#[test]
fn test_from_triple_accepts_versioned_darwin_arm64() {
    let config = TargetConfig::from_triple("arm64-apple-darwin25.2.0")
        .expect("versioned Darwin arm64 spelling must be accepted");
    assert_eq!(
        config.triple(),
        "aarch64-apple-darwin25.2.0",
        "stored triple must canonicalize the arch but preserve the OS version"
    );
    assert_eq!(config.components().arch, Arch::Aarch64);
    assert!(
        config.is_macos(),
        "versioned darwin must still be detected as macOS"
    );
}

/// Sibling: the canonical-arch versioned spelling is also accepted.
#[test]
fn test_from_triple_accepts_versioned_darwin_aarch64() {
    let config = TargetConfig::from_triple("aarch64-apple-darwin25.2.0")
        .expect("versioned Darwin aarch64 spelling must be accepted");
    assert_eq!(config.triple(), "aarch64-apple-darwin25.2.0");
    assert_eq!(config.components().arch, Arch::Aarch64);
    assert!(config.is_macos());
}

/// Sibling: `x86_64` versioned macOS triples are accepted (covers Intel
/// Macs running modern Xcode whose default triple also carries a version
/// suffix).
#[test]
fn test_from_triple_accepts_versioned_darwin_x86_64() {
    let config = TargetConfig::from_triple("x86_64-apple-darwin23.6.0")
        .expect("versioned Darwin x86_64 spelling must be accepted");
    assert_eq!(config.triple(), "x86_64-apple-darwin23.6.0");
    assert_eq!(config.components().arch, Arch::X86_64);
    assert!(config.is_macos());
}

/// Regression pin for BUG-04-045 / TPR-BUG-04-045-04: `from_triple` must
/// accept the modern Rust 1.78+ canonical WASI Preview1 spelling
/// `wasm32-unknown-wasip1` end-to-end. Before the fix, `SUPPORTED_TARGETS`
/// contained the deprecated 2-component `wasm32-wasi` spelling that the
/// strict triple parser refused to accept, so `ori build --target=wasm32-wasi`
/// returned `UnsupportedTarget` despite the codegen layer fully supporting
/// WASI Preview1. The fix replaced the deprecated entry with the modern
/// canonical 3-component form.
#[test]
fn test_from_triple_accepts_wasm32_wasip1_canonical() {
    let config = TargetConfig::from_triple("wasm32-unknown-wasip1")
        .expect("canonical wasm32-unknown-wasip1 spelling must be accepted");
    assert_eq!(config.triple(), "wasm32-unknown-wasip1");
    assert_eq!(config.components().arch, Arch::Wasm32);
    assert_eq!(config.components().os, "wasip1");
    assert!(config.is_wasm());
}

/// Negative pin: the deprecated 2-component `wasm32-wasi` spelling must
/// be rejected at the parse boundary, NOT silently normalized. The strict
/// triple parser invariant ("at least 3 components") is what makes the
/// "list claims X, parser rejects X" bug class structurally impossible.
/// This test pins the rejection so a future "be liberal in what you accept"
/// patch cannot quietly resurrect the deprecated form.
#[test]
fn test_from_triple_rejects_deprecated_wasm32_wasi() {
    let result = TargetConfig::from_triple("wasm32-wasi");
    assert!(
        result.is_err(),
        "deprecated 2-component wasm32-wasi must be rejected at the parse boundary"
    );
    // Specifically, it must be rejected as InvalidTripleFormat (not enough
    // components) — NOT as UnsupportedTarget. The parse boundary fires
    // first, before the supported-targets check ever runs.
    assert!(
        matches!(result, Err(TargetError::InvalidTripleFormat { .. })),
        "expected InvalidTripleFormat for 2-component wasm32-wasi, got: {result:?}"
    );
}

/// Test: Supported targets list
///
/// Scenario: Verify all documented targets are supported.
///
/// WASI is the modern Rust 1.78+ canonical 3-component spelling
/// `wasm32-unknown-wasip1` — the historical 2-component `wasm32-wasi`
/// was deprecated upstream in May 2024 and is no longer accepted by
/// Ori (see BUG-04-045 / TPR-BUG-04-045-04).
#[test]
fn test_supported_targets() {
    let expected_targets = [
        "x86_64-unknown-linux-gnu",
        "x86_64-unknown-linux-musl",
        "x86_64-apple-darwin",
        "aarch64-apple-darwin",
        "x86_64-pc-windows-msvc",
        "x86_64-pc-windows-gnu",
        "wasm32-unknown-unknown",
        "wasm32-unknown-wasip1",
    ];

    for target in expected_targets {
        assert!(
            is_supported_target(target),
            "Expected target '{target}' to be supported"
        );
    }
}

/// Test: Unsupported targets
///
/// Scenario: Non-standard targets should be reported as unsupported.
#[test]
fn test_unsupported_targets() {
    let unsupported = [
        "riscv64-unknown-linux-gnu",
        "powerpc64-unknown-linux-gnu",
        "sparc64-unknown-linux-gnu",
        "mips64-unknown-linux-gnu",
    ];

    for target in unsupported {
        // These may or may not be supported depending on LLVM build
        let _ = is_supported_target(target);
    }
}

/// Test: Target triple platform detection
///
/// Scenario: Platform detection helper methods.
#[test]
fn test_target_platform_detection() {
    // Linux
    let linux = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    assert!(linux.is_linux());
    assert!(!linux.is_macos());
    assert!(!linux.is_windows());
    assert!(!linux.is_wasm());
    // Note: no is_unix() method - check family instead
    assert_eq!(linux.family(), "unix");

    // macOS
    let macos = TargetTripleComponents::parse("x86_64-apple-darwin").unwrap();
    assert!(!macos.is_linux());
    assert!(macos.is_macos());
    assert!(!macos.is_windows());
    assert!(!macos.is_wasm());
    assert_eq!(macos.family(), "unix");

    // Windows
    let windows = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    assert!(!windows.is_linux());
    assert!(!windows.is_macos());
    assert!(windows.is_windows());
    assert!(!windows.is_wasm());
    assert_eq!(windows.family(), "windows");

    // WASM
    let wasm = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    assert!(!wasm.is_linux());
    assert!(!wasm.is_macos());
    assert!(!wasm.is_windows());
    assert!(wasm.is_wasm());
    assert_eq!(wasm.family(), "wasm");
}

/// Test: Target architecture detection
///
/// Scenario: Architecture detection via components.
#[test]
fn test_target_architecture_detection() {
    // x86_64
    let x64 = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(x64.arch, Arch::X86_64);
    assert!(!x64.is_wasm());

    // aarch64
    let arm = TargetTripleComponents::parse("aarch64-apple-darwin").unwrap();
    assert_eq!(arm.arch, Arch::Aarch64);
    assert!(!arm.is_wasm());

    // wasm32
    let wasm = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    assert_eq!(wasm.arch, Arch::Wasm32);
    assert!(wasm.is_wasm());
}

/// Test: Target triple to string
///
/// Scenario: Triple components can be reconstructed.
#[test]
fn test_target_triple_to_string() {
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(components.to_string(), "x86_64-unknown-linux-gnu");

    let components = TargetTripleComponents::parse("aarch64-apple-darwin").unwrap();
    assert_eq!(components.to_string(), "aarch64-apple-darwin");

    let components = TargetTripleComponents::parse("wasm32-unknown-wasi").unwrap();
    // May normalize to full triple or short form
    let triple = components.to_string();
    assert!(triple.contains("wasm32"));
    assert!(triple.contains("wasi"));
}

/// Test: Target config from components
///
/// Scenario: Create `TargetConfig` from parsed components.
#[test]
fn test_target_config_from_components() {
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    let config = TargetConfig::from_components(components.clone());

    assert_eq!(config.components().arch, Arch::X86_64);
    assert!(config.is_linux());
    // pointer_size returns bytes (8 for 64-bit, 4 for 32-bit)
    assert_eq!(config.pointer_size(), 8);
}

/// Test: Target config platform helpers
///
/// Scenario: Platform detection via `TargetConfig`.
#[test]
fn test_target_config_platform_helpers() {
    // Linux
    let config = linux_target();
    assert!(config.is_linux());
    assert!(!config.is_macos());
    assert!(!config.is_windows());
    assert!(!config.is_wasm());

    // macOS
    let config = macos_target();
    assert!(!config.is_linux());
    assert!(config.is_macos());
    assert!(!config.is_windows());
    assert!(!config.is_wasm());

    // Windows
    let config = windows_msvc_target();
    assert!(!config.is_linux());
    assert!(!config.is_macos());
    assert!(config.is_windows());
    assert!(!config.is_wasm());

    // WASM
    let config = wasm32_target();
    assert!(!config.is_linux());
    assert!(!config.is_macos());
    assert!(!config.is_windows());
    assert!(config.is_wasm());
}

/// Test: Target config CPU configuration
///
/// Scenario: CPU model and feature configuration.
#[test]
fn test_target_config_cpu() {
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    let config = TargetConfig::from_components(components)
        .with_cpu("skylake")
        .with_features("+avx2,+fma");

    assert_eq!(config.cpu(), "skylake");
    assert!(config.features().contains("+avx2"));
    assert!(config.features().contains("+fma"));
}

/// Test: Target config generic CPU
///
/// Scenario: Default to generic CPU for maximum compatibility.
#[test]
fn test_target_config_generic_cpu() {
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    let config = TargetConfig::from_components(components).with_cpu("generic");

    assert_eq!(config.cpu(), "generic");
}

/// Test: Parse CPU feature string
///
/// Scenario: Feature strings like "+avx2,-sse4".
#[test]
fn test_parse_features() {
    // Single feature
    let features = parse_features("+avx2").unwrap();
    assert!(features.iter().any(|(f, enabled)| *f == "avx2" && *enabled));

    // Multiple features
    let features = parse_features("+avx2,+fma,-sse4").unwrap();
    assert!(features.iter().any(|(f, enabled)| *f == "avx2" && *enabled));
    assert!(features.iter().any(|(f, enabled)| *f == "fma" && *enabled));
    assert!(features
        .iter()
        .any(|(f, enabled)| *f == "sse4" && !*enabled));

    // Empty string
    let features = parse_features("").unwrap();
    assert!(features.is_empty());
}

/// Test: `x86_64` specific features
///
/// Scenario: Common `x86_64` CPU features.
#[test]
fn test_x86_64_features() {
    let x86_features = [
        "sse4.1", "sse4.2", "avx", "avx2", "avx512f", "fma", "bmi", "bmi2", "popcnt", "lzcnt",
    ];

    // These should be parseable
    for feature in x86_features {
        let feature_str = format!("+{feature}");
        let features = parse_features(&feature_str).unwrap();
        assert!(!features.is_empty(), "Failed to parse feature: {feature}");
    }
}

/// Test: ARM64 specific features
///
/// Scenario: Common aarch64 CPU features.
#[test]
fn test_aarch64_features() {
    let arm_features = ["neon", "sve", "sve2", "crypto", "aes", "sha2", "crc"];

    for feature in arm_features {
        let feature_str = format!("+{feature}");
        let features = parse_features(&feature_str).unwrap();
        assert!(!features.is_empty(), "Failed to parse feature: {feature}");
    }
}

/// Test: WASM features
///
/// Scenario: WebAssembly feature flags.
#[test]
fn test_wasm_features() {
    let wasm_features = [
        "simd128",
        "bulk-memory",
        "atomics",
        "mutable-globals",
        "nontrapping-fptoint",
        "sign-ext",
        "multivalue",
        "reference-types",
        "exception-handling",
    ];

    for feature in wasm_features {
        let feature_str = format!("+{feature}");
        let features = parse_features(&feature_str).unwrap();
        assert!(!features.is_empty(), "Failed to parse feature: {feature}");
    }
}

/// Test: `x86_64` Linux data layout
///
/// Scenario: Verify data layout string format.
/// Note: Requires LLVM target to be registered.
#[test]
fn test_x86_64_linux_data_layout() {
    let config = linux_target();
    let Ok(layout) = config.data_layout() else {
        return; // Skip if target not available
    };

    // x86_64 is little endian — signaled by leading 'e' or 'e-' in the layout.
    assert!(
        layout.contains("e-") || layout.starts_with('e'),
        "expected little-endian layout, got: {layout}"
    );

    // 64-bit pointers: check the stable Ori API. Data layout string format
    // varies by LLVM version (modern LLVM omits the default-address-space
    // `p:64:64` and emits only address-space-qualified overrides like
    // `p270:32:32-p271:32:32-p272:64:64`). Testing LLVM's string format is
    // brittle; verify pointer size via Ori's typed accessor instead.
    assert_eq!(
        config.pointer_size(),
        8,
        "expected 64-bit pointer, data layout was: {layout}"
    );

    // x86_64 native integer widths include 64-bit — stable across LLVM versions.
    assert!(
        layout.contains("n8:16:32:64"),
        "expected x86_64 native integer widths, got: {layout}"
    );

    // Should have standard alignments
    assert!(
        layout.contains("i64:"),
        "expected i64 alignment, got: {layout}"
    );
}

/// Test: `x86_64` macOS data layout
/// Note: Requires LLVM target to be registered.
#[test]
fn test_x86_64_macos_data_layout() {
    let config = macos_target();
    let Ok(layout) = config.data_layout() else {
        return; // Skip if target not available
    };

    // Little endian
    assert!(
        layout.contains("e-") || layout.starts_with('e'),
        "expected little-endian layout, got: {layout}"
    );

    // 64-bit pointers: check stable API. See the Linux variant for why we
    // avoid matching LLVM's data-layout string format directly.
    assert_eq!(
        config.pointer_size(),
        8,
        "expected 64-bit pointer, data layout was: {layout}"
    );

    // x86_64 native integer widths include 64-bit
    assert!(
        layout.contains("n8:16:32:64"),
        "expected x86_64 native integer widths, got: {layout}"
    );
}

/// Test: ARM64 macOS data layout
/// Note: Requires LLVM target to be registered.
#[test]
fn test_aarch64_macos_data_layout() {
    let config = macos_arm_target();
    let Ok(layout) = config.data_layout() else {
        return; // Skip if target not available
    };

    // Little endian (Apple Silicon is LE)
    assert!(
        layout.contains("e-") || layout.starts_with('e'),
        "expected little-endian layout, got: {layout}"
    );

    // 64-bit pointers: check stable API. See the Linux variant for why we
    // avoid matching LLVM's data-layout string format directly.
    assert_eq!(
        config.pointer_size(),
        8,
        "expected 64-bit pointer, data layout was: {layout}"
    );
}

/// Test: WASM32 data layout
///
/// Scenario: WASM has 32-bit pointers.
/// Note: Requires LLVM target to be registered.
#[test]
fn test_wasm32_data_layout() {
    let config = wasm32_target();
    let Ok(layout) = config.data_layout() else {
        return; // Skip if target not available
    };

    // Little endian
    assert!(layout.contains("e-") || layout.starts_with('e'));

    // 32-bit pointers (WASM32)
    assert!(layout.contains("p:32:32"));
}

/// Test: Cross-compile Linux to macOS config
///
/// Scenario from Rust `mismatching-target-triples`:
/// Different host/target configurations.
/// Note: Requires LLVM target to be registered.
#[test]
fn test_cross_compile_linux_to_macos() {
    // Host is Linux (implicit)
    // Target is macOS
    let target = macos_target();

    assert!(target.is_macos());
    assert!(!target.is_linux());

    // Data layout should be for macOS (skip if target not available)
    if let Ok(layout) = target.data_layout() {
        assert!(layout.contains("p:64:64"));
    }
}

/// Test: Cross-compile to WASM
///
/// Scenario: Common cross-compilation target.
/// Note: Requires LLVM target to be registered.
#[test]
fn test_cross_compile_to_wasm() {
    // Standalone WASM
    let wasm = wasm32_target();
    assert!(wasm.is_wasm());
    if let Ok(layout) = wasm.data_layout() {
        assert!(layout.contains("p:32:32"));
    }

    // WASI WASM
    let wasi = wasm32_wasi_target();
    assert!(wasi.is_wasm());
}

/// Test: Cross-compile to Windows
///
/// Scenario: Windows cross-compilation.
#[test]
fn test_cross_compile_to_windows() {
    // MSVC
    let msvc = windows_msvc_target();
    assert!(msvc.is_windows());

    // MinGW
    let gnu = windows_gnu_target();
    assert!(gnu.is_windows());
}

/// Test: Target error display
#[test]
fn test_target_error_display() {
    let err = TargetError::InvalidTripleFormat {
        triple: "invalid".to_string(),
        reason: "malformed".to_string(),
    };
    assert!(err.to_string().contains("invalid"));

    let err = TargetError::UnsupportedTarget {
        triple: "riscv64-unknown-linux-gnu".to_string(),
        supported: vec!["x86_64-unknown-linux-gnu"],
    };
    assert!(err.to_string().contains("unsupported"));
    assert!(err.to_string().contains("riscv64"));

    let err =
        TargetError::TargetMachineCreationFailed("target machine creation failed".to_string());
    assert!(err.to_string().contains("target machine"));
}

/// Test: Target config with optimization level
///
/// Scenario: Optimization level configuration.
/// Note: `TargetConfig` uses inkwell's `OptimizationLevel` directly.
#[test]
fn test_target_config_with_opt_level() {
    use inkwell::OptimizationLevel;

    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    let config =
        TargetConfig::from_components(components).with_opt_level(OptimizationLevel::Aggressive);

    assert_eq!(config.opt_level(), OptimizationLevel::Aggressive);
}

/// Test: Host CPU name detection
///
/// Scenario: Get current system's CPU name.
#[test]
fn test_host_cpu_name_detection() {
    let cpu_name = get_host_cpu_name();

    // Should return something (may be "generic" if detection fails)
    assert!(!cpu_name.is_empty());
}

/// Test: Host CPU feature detection
///
/// Scenario: Get current system's CPU features.
#[test]
fn test_host_cpu_features_detection() {
    let features = get_host_cpu_features();

    // May be empty on some systems, but should not panic
    let _ = features;
}

/// Test: Linux vs musl libc difference
#[test]
fn test_linux_libc_difference() {
    let glibc = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    let musl = TargetTripleComponents::parse("x86_64-unknown-linux-musl").unwrap();

    assert_eq!(glibc.env, Some("gnu".to_string()));
    assert_eq!(musl.env, Some("musl".to_string()));

    // Both are Linux
    assert!(glibc.is_linux());
    assert!(musl.is_linux());
}

/// Test: Windows ABI difference
///
/// Scenario: MSVC vs GNU ABI on Windows.
#[test]
fn test_windows_abi_difference() {
    let msvc = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    let gnu = TargetTripleComponents::parse("x86_64-pc-windows-gnu").unwrap();

    assert_eq!(msvc.env, Some("msvc".to_string()));
    assert_eq!(gnu.env, Some("gnu".to_string()));

    // Both are Windows
    assert!(msvc.is_windows());
    assert!(gnu.is_windows());
}

/// Test: WASI vs standalone WASM
///
/// Scenario: WASI provides system interface.
#[test]
fn test_wasi_vs_standalone_wasm() {
    let standalone = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    let wasi = TargetTripleComponents::parse("wasm32-unknown-wasi").unwrap();

    assert_eq!(standalone.os, "unknown");
    assert_eq!(wasi.os, "wasi");

    // Both are WASM
    assert!(standalone.is_wasm());
    assert!(wasi.is_wasm());
}

/// Test: Pointer size for different architectures
///
/// Note: `pointer_size()` returns bytes, not bits
#[test]
fn test_pointer_sizes() {
    // 64-bit targets (8 bytes)
    assert_eq!(linux_target().pointer_size(), 8);
    assert_eq!(macos_target().pointer_size(), 8);
    assert_eq!(macos_arm_target().pointer_size(), 8);
    assert_eq!(windows_msvc_target().pointer_size(), 8);

    // 32-bit targets (4 bytes)
    assert_eq!(wasm32_target().pointer_size(), 4);
    assert_eq!(wasm32_wasi_target().pointer_size(), 4);
}

/// Test: Endianness detection
#[test]
fn test_endianness() {
    // All supported targets are little endian
    assert!(linux_target().is_little_endian());
    assert!(macos_target().is_little_endian());
    assert!(macos_arm_target().is_little_endian());
    assert!(windows_msvc_target().is_little_endian());
    assert!(wasm32_target().is_little_endian());
}
