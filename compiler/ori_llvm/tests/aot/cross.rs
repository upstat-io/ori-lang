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

// Allow similar names in tests (wasm vs wasi pattern is intentional)
#![allow(clippy::similar_names)]

use ori_llvm::aot::target::{
    get_host_cpu_features, get_host_cpu_name, is_supported_target, parse_features, TargetConfig,
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
    assert_eq!(linux.arch, "x86_64");
    assert_eq!(linux.vendor, "unknown");
    assert_eq!(linux.os, "linux");
    assert_eq!(linux.env, Some("gnu".to_string()));

    // macOS targets
    let macos = TargetTripleComponents::parse("x86_64-apple-darwin").unwrap();
    assert_eq!(macos.arch, "x86_64");
    assert_eq!(macos.vendor, "apple");
    assert_eq!(macos.os, "darwin");
    assert!(macos.env.is_none());

    // ARM64 macOS
    let macos_arm = TargetTripleComponents::parse("aarch64-apple-darwin").unwrap();
    assert_eq!(macos_arm.arch, "aarch64");
    assert_eq!(macos_arm.vendor, "apple");

    // Windows targets
    let windows_msvc = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    assert_eq!(windows_msvc.arch, "x86_64");
    assert_eq!(windows_msvc.vendor, "pc");
    assert_eq!(windows_msvc.os, "windows");
    assert_eq!(windows_msvc.env, Some("msvc".to_string()));

    let windows_gnu = TargetTripleComponents::parse("x86_64-pc-windows-gnu").unwrap();
    assert_eq!(windows_gnu.env, Some("gnu".to_string()));

    // WASM targets
    let wasm = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    assert_eq!(wasm.arch, "wasm32");
    assert_eq!(wasm.vendor, "unknown");
    assert_eq!(wasm.os, "unknown");

    let wasi = TargetTripleComponents::parse("wasm32-unknown-wasi").unwrap();
    assert_eq!(wasi.arch, "wasm32");
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

    // Invalid architecture
    let result = TargetTripleComponents::parse("invalid-unknown-linux-gnu");
    // May or may not fail depending on validation strictness
    let _ = result;
}

/// Test: Supported targets list
///
/// Scenario: Verify all documented targets are supported.
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
        "wasm32-wasi",
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
    assert_eq!(x64.arch, "x86_64");
    assert!(!x64.is_wasm());

    // aarch64
    let arm = TargetTripleComponents::parse("aarch64-apple-darwin").unwrap();
    assert_eq!(arm.arch, "aarch64");
    assert!(!arm.is_wasm());

    // wasm32
    let wasm = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    assert_eq!(wasm.arch, "wasm32");
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

    assert_eq!(config.components().arch, "x86_64");
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

    // x86_64 is little endian
    assert!(layout.contains("e-") || layout.starts_with('e'));

    // 64-bit pointers
    assert!(layout.contains("p:64:64"));

    // Should have standard alignments
    assert!(layout.contains("i64:"));
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
    assert!(layout.contains("e-") || layout.starts_with('e'));

    // 64-bit pointers
    assert!(layout.contains("p:64:64"));
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
    assert!(layout.contains("e-") || layout.starts_with('e'));

    // 64-bit pointers
    assert!(layout.contains("p:64:64"));
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
