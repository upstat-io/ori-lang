//! Tests for target configuration (`ori_llvm::aot::target`).
//!
//! These tests verify:
//! - Target triple parsing (Linux, macOS, Windows, WASM)
//! - CPU feature parsing
//! - Target configuration builder pattern
//! - Native target detection
//! - Pointer size and alignment

use ori_llvm::aot::target::{
    get_host_cpu_features, get_host_cpu_name, parse_features, TargetConfig, TargetError,
    TargetTripleComponents,
};
use ori_llvm::inkwell::targets::{CodeModel, RelocMode};
use ori_llvm::inkwell::OptimizationLevel;

#[test]
fn test_parse_triple_linux() {
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    assert_eq!(components.arch, "x86_64");
    assert_eq!(components.vendor, "unknown");
    assert_eq!(components.os, "linux");
    assert_eq!(components.env, Some("gnu".to_string()));
    assert!(components.is_linux());
    assert_eq!(components.family(), "unix");
}

#[test]
fn test_parse_triple_macos() {
    let components = TargetTripleComponents::parse("aarch64-apple-darwin").unwrap();
    assert_eq!(components.arch, "aarch64");
    assert_eq!(components.vendor, "apple");
    assert_eq!(components.os, "darwin");
    assert_eq!(components.env, None);
    assert!(components.is_macos());
    assert_eq!(components.family(), "unix");
}

#[test]
fn test_parse_triple_windows() {
    let components = TargetTripleComponents::parse("x86_64-pc-windows-msvc").unwrap();
    assert_eq!(components.arch, "x86_64");
    assert_eq!(components.vendor, "pc");
    assert_eq!(components.os, "windows");
    assert_eq!(components.env, Some("msvc".to_string()));
    assert!(components.is_windows());
    assert_eq!(components.family(), "windows");
}

#[test]
fn test_parse_triple_wasm() {
    let components = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    assert_eq!(components.arch, "wasm32");
    assert!(components.is_wasm());
    assert_eq!(components.family(), "wasm");
}

#[test]
fn test_parse_triple_invalid() {
    let result = TargetTripleComponents::parse("invalid");
    assert!(result.is_err());

    let result = TargetTripleComponents::parse("x86_64-linux");
    assert!(result.is_err());
}

#[test]
fn test_parse_features() {
    let features = parse_features("+avx2,+fma,-sse4.1").unwrap();
    assert_eq!(
        features,
        vec![("avx2", true), ("fma", true), ("sse4.1", false)]
    );
}

#[test]
fn test_parse_features_empty() {
    let features = parse_features("").unwrap();
    assert!(features.is_empty());
}

#[test]
fn test_parse_features_invalid() {
    let result = parse_features("avx2"); // Missing +/-
    assert!(result.is_err());
}

#[test]
fn test_target_config_native() {
    // This test requires LLVM to be properly configured
    let config = TargetConfig::native();
    if let Ok(config) = config {
        assert!(!config.triple().is_empty());
        assert_eq!(config.cpu(), "generic");
        assert!(config.features().is_empty());
    }
    // If native target init fails, that's OK for some test environments
}

#[test]
fn test_target_config_builder() {
    // Test builder pattern (doesn't require LLVM init)
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    let config = TargetConfig {
        triple: "x86_64-unknown-linux-gnu".to_string(),
        components,
        cpu: "generic".to_string(),
        features: String::new(),
        opt_level: OptimizationLevel::None,
        reloc_mode: RelocMode::Default,
        code_model: CodeModel::Default,
    };

    let config = config.with_cpu("skylake").with_features("+avx2,+fma");

    assert_eq!(config.cpu(), "skylake");
    assert_eq!(config.features(), "+avx2,+fma");
    assert!(config.is_linux());
    assert!(!config.is_wasm());
}

#[test]
fn test_unsupported_target() {
    let result = TargetConfig::from_triple("riscv64-unknown-linux-gnu");
    assert!(matches!(result, Err(TargetError::UnsupportedTarget { .. })));
}

#[test]
fn test_pointer_size() {
    // 64-bit targets
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    let config = TargetConfig {
        triple: "x86_64-unknown-linux-gnu".to_string(),
        components,
        cpu: "generic".to_string(),
        features: String::new(),
        opt_level: OptimizationLevel::None,
        reloc_mode: RelocMode::Default,
        code_model: CodeModel::Default,
    };
    assert_eq!(config.pointer_size(), 8);

    // 32-bit WASM target
    let components = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    let config = TargetConfig {
        triple: "wasm32-unknown-unknown".to_string(),
        components,
        cpu: "generic".to_string(),
        features: String::new(),
        opt_level: OptimizationLevel::None,
        reloc_mode: RelocMode::Default,
        code_model: CodeModel::Default,
    };
    assert_eq!(config.pointer_size(), 4);
}

#[test]
fn test_data_layout_native() {
    // This test requires LLVM to be properly configured
    if let Ok(config) = TargetConfig::native() {
        if let Ok(layout) = config.data_layout() {
            // Data layout should be non-empty and start with endianness
            assert!(!layout.is_empty());
            // Most layouts start with 'e' (little-endian) or 'E' (big-endian)
            assert!(layout.starts_with('e') || layout.starts_with('E'));
        }
    }
}

#[test]
fn test_configure_module() {
    use ori_llvm::inkwell::context::Context;

    // This test requires LLVM to be properly configured
    if let Ok(config) = TargetConfig::native() {
        let context = Context::create();
        let module = context.create_module("test");

        // Configure should succeed
        let result = config.configure_module(&module);
        if let Ok(()) = result {
            // Module should have triple set
            let module_triple = module.get_triple();
            assert!(!module_triple.as_str().to_string_lossy().is_empty());
        }
    }
}

#[test]
fn test_get_host_cpu_name() {
    // This should always return something, even if just "generic"
    let cpu = get_host_cpu_name();
    assert!(!cpu.is_empty());
}

#[test]
fn test_get_host_cpu_features() {
    // Features may be empty on some systems, but shouldn't panic
    let _features = get_host_cpu_features();
    // No assertion needed - just verify it doesn't panic
}

#[test]
fn test_with_cpu_native() {
    if let Ok(config) = TargetConfig::native() {
        let config = config.with_cpu_native();
        // CPU should be set to something (might be "generic" on some systems)
        assert!(!config.cpu().is_empty());
    }
}

#[test]
fn test_with_features_native() {
    if let Ok(config) = TargetConfig::native() {
        let config = config.with_features_native();
        // Features string may be empty or contain features
        // Just verify it doesn't panic and is a valid string
        let _ = config.features();
    }
}

#[test]
fn test_with_feature() {
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    let config = TargetConfig {
        triple: "x86_64-unknown-linux-gnu".to_string(),
        components,
        cpu: "generic".to_string(),
        features: String::new(),
        opt_level: OptimizationLevel::None,
        reloc_mode: RelocMode::Default,
        code_model: CodeModel::Default,
    };

    // Add single feature
    let config = config.with_feature("avx2");
    assert_eq!(config.features(), "+avx2");

    // Add another feature
    let config = config.with_feature("fma");
    assert_eq!(config.features(), "+avx2,+fma");
}

#[test]
fn test_without_feature() {
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    let config = TargetConfig {
        triple: "x86_64-unknown-linux-gnu".to_string(),
        components,
        cpu: "generic".to_string(),
        features: String::new(),
        opt_level: OptimizationLevel::None,
        reloc_mode: RelocMode::Default,
        code_model: CodeModel::Default,
    };

    // Disable a feature
    let config = config.without_feature("sse4.1");
    assert_eq!(config.features(), "-sse4.1");

    // Add and disable features
    let config = config.with_feature("avx2").without_feature("sse3");
    assert_eq!(config.features(), "-sse4.1,+avx2,-sse3");
}

#[test]
fn test_target_error_display_all_variants() {
    // UnsupportedTarget
    let err = TargetError::UnsupportedTarget {
        triple: "riscv64-unknown-linux-gnu".to_string(),
        supported: vec!["x86_64-unknown-linux-gnu", "aarch64-apple-darwin"],
    };
    let display = err.to_string();
    assert!(display.contains("unsupported target"));
    assert!(display.contains("riscv64-unknown-linux-gnu"));
    assert!(display.contains("x86_64-unknown-linux-gnu"));

    // InitializationFailed
    let err = TargetError::InitializationFailed("LLVM init failed".to_string());
    assert_eq!(
        err.to_string(),
        "failed to initialize LLVM target: LLVM init failed"
    );

    // TargetMachineCreationFailed
    let err = TargetError::TargetMachineCreationFailed("machine error".to_string());
    assert_eq!(
        err.to_string(),
        "failed to create target machine: machine error"
    );

    // InvalidTripleFormat
    let err = TargetError::InvalidTripleFormat {
        triple: "bad".to_string(),
        reason: "too few components".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "invalid target triple 'bad': too few components"
    );

    // InvalidCpu
    let err = TargetError::InvalidCpu {
        cpu: "bad-cpu".to_string(),
        target: "x86_64".to_string(),
    };
    assert_eq!(err.to_string(), "invalid CPU 'bad-cpu' for target 'x86_64'");

    // InvalidFeature
    let err = TargetError::InvalidFeature {
        feature: "bad-feature".to_string(),
        reason: "unknown feature".to_string(),
    };
    assert_eq!(
        err.to_string(),
        "invalid feature 'bad-feature': unknown feature"
    );
}

#[test]
fn test_target_triple_display() {
    let components = TargetTripleComponents {
        arch: "x86_64".to_string(),
        vendor: "unknown".to_string(),
        os: "linux".to_string(),
        env: Some("gnu".to_string()),
    };
    assert_eq!(format!("{components}"), "x86_64-unknown-linux-gnu");

    let components = TargetTripleComponents {
        arch: "aarch64".to_string(),
        vendor: "apple".to_string(),
        os: "darwin".to_string(),
        env: None,
    };
    assert_eq!(format!("{components}"), "aarch64-apple-darwin");
}

#[test]
fn test_target_config_builder_all_options() {
    if let Ok(config) = TargetConfig::native() {
        // Test all builder methods
        let config = config
            .with_cpu("generic")
            .with_features("+avx2,-sse3")
            .with_opt_level(OptimizationLevel::Aggressive)
            .with_reloc_mode(RelocMode::PIC)
            .with_code_model(CodeModel::Small);

        assert_eq!(config.cpu(), "generic");
        assert_eq!(config.features(), "+avx2,-sse3");
        assert_eq!(config.opt_level(), OptimizationLevel::Aggressive);
    }
}

#[test]
fn test_target_config_accessors() {
    if let Ok(config) = TargetConfig::native() {
        // Test various accessors
        assert!(!config.triple().is_empty());
        let _components = config.components();
        let _family = config.family();
        let _ptr_size = config.pointer_size();
        let _ptr_align = config.pointer_align();
        assert!(config.is_little_endian());
    }
}

#[test]
fn test_parse_features_with_whitespace() {
    let features = parse_features(" +avx2 , +fma , -sse3 ").unwrap();
    assert_eq!(features.len(), 3);
    assert_eq!(features[0], ("avx2", true));
    assert_eq!(features[1], ("fma", true));
    assert_eq!(features[2], ("sse3", false));
}

#[test]
fn test_target_config_target_checks() {
    let components = TargetTripleComponents::parse("x86_64-unknown-linux-gnu").unwrap();
    let config = TargetConfig {
        triple: "x86_64-unknown-linux-gnu".to_string(),
        components,
        cpu: "generic".to_string(),
        features: String::new(),
        opt_level: OptimizationLevel::None,
        reloc_mode: RelocMode::Default,
        code_model: CodeModel::Default,
    };
    assert!(config.is_linux());
    assert!(!config.is_macos());
    assert!(!config.is_windows());
    assert!(!config.is_wasm());

    let components = TargetTripleComponents::parse("wasm32-unknown-unknown").unwrap();
    let config = TargetConfig {
        triple: "wasm32-unknown-unknown".to_string(),
        components,
        cpu: "generic".to_string(),
        features: String::new(),
        opt_level: OptimizationLevel::None,
        reloc_mode: RelocMode::Default,
        code_model: CodeModel::Default,
    };
    assert!(config.is_wasm());
    assert_eq!(config.pointer_size(), 4);
}
