/// Test that `SUPPORTED_TARGETS` contains expected platform families.
///
/// The actual `list_targets` function does I/O (prints to stdout), so we test
/// the underlying data and logic that it depends on.
#[cfg(feature = "llvm")]
mod llvm_tests {
    use ori_llvm::aot::SUPPORTED_TARGETS;

    #[test]
    fn test_supported_targets_contains_linux() {
        let has_linux = SUPPORTED_TARGETS.iter().any(|t| t.contains("linux"));
        assert!(has_linux, "should have at least one Linux target");
    }

    #[test]
    fn test_supported_targets_contains_darwin() {
        let has_darwin = SUPPORTED_TARGETS.iter().any(|t| t.contains("darwin"));
        assert!(has_darwin, "should have at least one macOS target");
    }

    #[test]
    fn test_supported_targets_contains_windows() {
        let has_windows = SUPPORTED_TARGETS.iter().any(|t| t.contains("windows"));
        assert!(has_windows, "should have at least one Windows target");
    }

    #[test]
    fn test_supported_targets_contains_wasm() {
        let has_wasm = SUPPORTED_TARGETS.iter().any(|t| t.contains("wasm"));
        assert!(has_wasm, "should have at least one WebAssembly target");
    }

    #[test]
    fn test_supported_targets_triple_format() {
        // All targets should follow standard format: arch-vendor-os[-env] or arch-os (WASM)
        for target in SUPPORTED_TARGETS {
            let parts: Vec<&str> = target.split('-').collect();
            // WASM targets use 2-part format (wasm32-unknown-unknown, wasm32-wasi)
            // Standard targets use 3+ parts (arch-vendor-os[-env])
            let min_parts = if target.starts_with("wasm") { 2 } else { 3 };
            assert!(
                parts.len() >= min_parts,
                "target '{target}' should have at least {min_parts} parts"
            );
        }
    }

    #[test]
    fn test_supported_targets_not_empty() {
        assert!(
            !SUPPORTED_TARGETS.is_empty(),
            "should have at least one supported target"
        );
    }

    #[test]
    fn test_supported_targets_unique() {
        let mut seen = std::collections::HashSet::new();
        for target in SUPPORTED_TARGETS {
            assert!(
                seen.insert(target),
                "duplicate target '{target}' in SUPPORTED_TARGETS"
            );
        }
    }

    #[test]
    fn test_native_target_available() {
        // Native target should be detectable
        let result = ori_llvm::aot::TargetConfig::native();
        assert!(
            result.is_ok(),
            "native target should be available: {:?}",
            result.err()
        );
    }
}
