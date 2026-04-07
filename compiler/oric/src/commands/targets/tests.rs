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
        // All targets must follow the LLVM canonical 3+ component format:
        // `arch-vendor-os[-env]`. WASM targets are no exception — the
        // historical 2-component Rust short form `wasm32-wasi` was deprecated
        // upstream in May 2024 (Rust 1.78) in favor of `wasm32-wasip1`, and
        // Ori uses the canonical 3-component spelling `wasm32-unknown-wasip1`.
        // This invariant is what `TargetTripleComponents::parse()` enforces.
        for target in SUPPORTED_TARGETS {
            let parts: Vec<&str> = target.split('-').collect();
            assert!(
                parts.len() >= 3,
                "target '{target}' must have at least 3 parts (arch-vendor-os[-env])"
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
